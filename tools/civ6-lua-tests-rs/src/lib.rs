//! Shared support for executing the shipped MOD scripts with vendored Lua 5.1.

use mlua::{IntoLuaMulti, Lua, MultiValue, Value};
use rusqlite::{types::ValueRef, Connection};
use serde_json::{Map, Value as Json};
use std::path::{Path, PathBuf};

pub mod official;

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

pub fn read_repo(path: impl AsRef<Path>) -> String {
    let path = repo_root().join(path);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

pub fn new_lua() -> Lua {
    let lua = Lua::new();
    // Legacy test fixtures use Lua 5.2's load(string). Only the test VM is
    // adapted; the MOD sources are always read verbatim from this worktree.
    lua.load(
        "local original = load; load = function(source, ...) if type(source) == 'string' then return loadstring(source, ...) end; return original(source, ...) end",
    )
    .exec()
    .unwrap();
    lua
}

pub fn exec(lua: &Lua, source: &str) {
    lua.load(source).exec().unwrap();
}

pub fn exec_args(lua: &Lua, source: &str, args: impl IntoLuaMulti) -> MultiValue {
    lua.load(source).call(args).unwrap()
}

pub fn json_to_lua(lua: &Lua, value: &Json) -> Value {
    match value {
        Json::Null => Value::Nil,
        Json::Bool(v) => Value::Boolean(*v),
        Json::Number(v) => {
            if let Some(v) = v.as_i64() {
                Value::Integer(v)
            } else {
                Value::Number(v.as_f64().unwrap())
            }
        }
        Json::String(v) => Value::String(lua.create_string(v).unwrap()),
        Json::Array(values) => {
            let table = lua.create_table().unwrap();
            for (i, value) in values.iter().enumerate() {
                table.raw_set(i + 1, json_to_lua(lua, value)).unwrap();
            }
            Value::Table(table)
        }
        Json::Object(values) => {
            let table = lua.create_table().unwrap();
            for (key, value) in values {
                table
                    .raw_set(key.as_str(), json_to_lua(lua, value))
                    .unwrap();
            }
            Value::Table(table)
        }
    }
}

pub fn query_json(db: &Connection, sql: &str) -> Vec<Json> {
    let mut statement = db
        .prepare(sql)
        .unwrap_or_else(|error| panic!("{sql}: {error}"));
    let names: Vec<_> = statement
        .column_names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    statement
        .query_map([], |row| {
            let mut object = Map::new();
            for (i, name) in names.iter().enumerate() {
                let value = match row.get_ref(i)? {
                    ValueRef::Null => Json::Null,
                    ValueRef::Integer(value) => Json::from(value),
                    ValueRef::Real(value) => Json::from(value),
                    ValueRef::Text(value) => Json::from(std::str::from_utf8(value).unwrap()),
                    ValueRef::Blob(_) => panic!("unexpected blob in {sql}"),
                };
                object.insert(name.clone(), value);
            }
            Ok(Json::Object(object))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}
