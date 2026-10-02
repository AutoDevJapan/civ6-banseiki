#![allow(dead_code)]

use mlua::Lua;
use serde_json::Value;
use std::path::PathBuf;

pub fn root() -> PathBuf {
    civ6_autotest_rs::repo_root()
}

pub fn scenario(name: &str) -> Value {
    let path = root().join("tools/civ6-autotest/scenarios").join(name);
    serde_json::from_slice(
        &std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
    .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

pub fn steps(data: &Value) -> &[Value] {
    data["steps"].as_array().expect("steps")
}

pub fn action(step: &Value, name: &str) -> bool {
    step["action"] == name
}

pub fn code(step: &Value) -> &str {
    step["code"].as_str().expect("code")
}

pub fn compile_lua(lua: &Lua, source: &str) {
    lua.load(source)
        .into_function()
        .unwrap_or_else(|e| panic!("Lua compile failed: {e}\n{source}"));
}

pub fn expect_error(result: civ6_autotest_rs::Result<()>, needle: &str) {
    let err = result.expect_err("scenario unexpectedly valid");
    assert!(
        err.message.contains(needle),
        "{err:?} does not contain {needle:?}"
    );
}

pub fn lua_error<T>(result: mlua::Result<T>, needle: &str) {
    let err = match result {
        Ok(_) => panic!("Lua unexpectedly succeeded"),
        Err(e) => e,
    };
    assert!(
        err.to_string().contains(needle),
        "{err} does not contain {needle:?}"
    );
}
