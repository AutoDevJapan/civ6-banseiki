//! Assertion boundary: inspect Lua kinds and result counts before decoding.
//! Snapshot tables retain their key types, including throughout save/load.

use mlua::{Chunk, Function, IntoLua, IntoLuaMulti, Lua, MultiValue, Table, Value};
use serde_json::Value as Json;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::{Index, IndexMut};

fn error(message: impl Into<String>) -> mlua::Error {
    mlua::Error::RuntimeError(message.into())
}

pub trait FromLua: Sized {
    fn from_value(value: Value) -> mlua::Result<Self>;
}

pub trait FromLuaMulti: Sized {
    fn from_results(values: MultiValue) -> mlua::Result<Self>;
}

impl<T: FromLua> FromLuaMulti for T {
    fn from_results(mut values: MultiValue) -> mlua::Result<Self> {
        if values.len() != 1 {
            return Err(error(format!(
                "expected 1 Lua result, got {}: {values:?}",
                values.len()
            )));
        }
        T::from_value(values.pop_front().unwrap())
    }
}

macro_rules! tuples {
    ($($n:literal => ($($t:ident),+)),+ $(,)?) => {$ (
        impl<$($t: FromLua),+> FromLuaMulti for ($($t,)+) {
            fn from_results(mut values: MultiValue) -> mlua::Result<Self> {
                if values.len() != $n {
                    return Err(error(format!("expected {} Lua results, got {}: {values:?}", $n, values.len())));
                }
                Ok(($($t::from_value(values.pop_front().unwrap())?,)+))
            }
        }
    )+};
}
tuples!(2 => (A, B), 3 => (A, B, C), 4 => (A, B, C, D), 5 => (A, B, C, D, E), 6 => (A, B, C, D, E, F));

impl FromLua for Value {
    fn from_value(value: Value) -> mlua::Result<Self> {
        Ok(value)
    }
}

impl FromLua for i64 {
    fn from_value(value: Value) -> mlua::Result<Self> {
        match value {
            Value::Integer(n) => Ok(n),
            // Python equality allows 120 == 120.0, but not 120 == 120.5.
            Value::Number(n)
                if n.is_finite()
                    && n.fract() == 0.0
                    && n >= i64::MIN as f64
                    && n < -(i64::MIN as f64) =>
            {
                Ok(n as i64)
            }
            other => Err(error(format!(
                "expected integral Lua number, got {other:?}"
            ))),
        }
    }
}

impl FromLua for f64 {
    fn from_value(value: Value) -> mlua::Result<Self> {
        match value {
            Value::Integer(n) => Ok(n as f64),
            Value::Number(n) => Ok(n),
            other => Err(error(format!("expected Lua number, got {other:?}"))),
        }
    }
}

macro_rules! exact_kind {
    ($t:ty, $variant:ident, $name:literal) => {
        impl FromLua for $t {
            fn from_value(value: Value) -> mlua::Result<Self> {
                match value {
                    Value::$variant(v) => Ok(v),
                    other => Err(error(format!("expected Lua {}, got {other:?}", $name))),
                }
            }
        }
    };
}
exact_kind!(bool, Boolean, "boolean");
exact_kind!(Table, Table, "table");
exact_kind!(Function, Function, "function");
impl FromLua for String {
    fn from_value(value: Value) -> mlua::Result<Self> {
        match value {
            Value::String(s) => Ok(s.to_str()?.to_string()),
            other => Err(error(format!("expected Lua string, got {other:?}"))),
        }
    }
}
impl<T: FromLua> FromLua for Option<T> {
    fn from_value(value: Value) -> mlua::Result<Self> {
        if matches!(value, Value::Nil) {
            Ok(None)
        } else {
            T::from_value(value).map(Some)
        }
    }
}

pub trait ChunkExt {
    fn strict_eval<T: FromLuaMulti>(self) -> mlua::Result<T>;
    fn strict_call<T: FromLuaMulti>(self, args: impl IntoLuaMulti) -> mlua::Result<T>;
    fn python_truthy(self) -> mlua::Result<bool>;
}
impl ChunkExt for Chunk<'_> {
    fn strict_eval<T: FromLuaMulti>(self) -> mlua::Result<T> {
        T::from_results(self.eval::<MultiValue>()?)
    }
    fn strict_call<T: FromLuaMulti>(self, args: impl IntoLuaMulti) -> mlua::Result<T> {
        T::from_results(self.call::<MultiValue>(args)?)
    }
    fn python_truthy(self) -> mlua::Result<bool> {
        Ok(truthy_results(self.eval::<MultiValue>()?))
    }
}

pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Nil | Value::Boolean(false) => false,
        Value::Integer(n) => *n != 0,
        Value::Number(n) => *n != 0.0,
        Value::String(s) => !s.as_bytes().is_empty(),
        // Lupa's _LuaObject.__bool__ returns True even for {}:
        // https://github.com/scoder/lupa/blob/master/lupa/_lupa.pyx
        _ => true,
    }
}
pub fn truthy_results(values: MultiValue) -> bool {
    match values.len() {
        0 => false,
        1 => truthy(&values[0]),
        _ => true,
    }
}

pub trait TableExt {
    fn strict_get<T: FromLua>(&self, key: impl IntoLua) -> mlua::Result<T>;
    fn strict_pairs<K: FromLua, V: FromLua>(&self) -> impl Iterator<Item = mlua::Result<(K, V)>>;
    fn strict_sequence_values<T: FromLua>(&self) -> impl Iterator<Item = mlua::Result<T>>;
}
impl TableExt for Table {
    fn strict_get<T: FromLua>(&self, key: impl IntoLua) -> mlua::Result<T> {
        T::from_value(self.get::<Value>(key)?)
    }
    fn strict_pairs<K: FromLua, V: FromLua>(&self) -> impl Iterator<Item = mlua::Result<(K, V)>> {
        self.pairs::<Value, Value>().map(|pair| {
            let (key, value) = pair?;
            Ok((K::from_value(key)?, V::from_value(value)?))
        })
    }
    fn strict_sequence_values<T: FromLua>(&self) -> impl Iterator<Item = mlua::Result<T>> {
        // Check the complete key set first; ipairs alone silently stops at holes.
        let entries: mlua::Result<BTreeMap<i64, Value>> = self.strict_pairs().collect();
        let values = entries.and_then(|entries| {
            if entries.keys().copied().ne(1..=entries.len() as i64) {
                return Err(error(format!(
                    "expected consecutive integer keys starting at 1, got {:?}",
                    entries.keys()
                )));
            }
            Ok(entries.into_values().collect::<Vec<_>>())
        });
        let values = match values {
            Ok(values) => values.into_iter().map(T::from_value).collect(),
            Err(e) => vec![Err(e)],
        };
        values.into_iter()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    Integer(i64),
    String(String),
    Boolean(bool),
    Fraction(u64),
}
impl Key {
    fn from_value(value: Value) -> mlua::Result<Self> {
        match value {
            Value::Integer(n) => Ok(Self::Integer(n)),
            Value::Number(n) => match i64::from_value(Value::Number(n)) {
                Ok(n) => Ok(Self::Integer(n)),
                Err(_) if n.is_finite() => Ok(Self::Fraction(n.to_bits())),
                Err(e) => Err(e),
            },
            Value::String(s) => Ok(Self::String(s.to_str()?.to_string())),
            Value::Boolean(b) => Ok(Self::Boolean(b)),
            other => Err(error(format!("unsupported snapshot key {other:?}"))),
        }
    }
    fn to_lua(&self, lua: &Lua) -> Value {
        match self {
            Self::Integer(n) => Value::Integer(*n),
            Self::String(s) => Value::String(lua.create_string(s).unwrap()),
            Self::Boolean(b) => Value::Boolean(*b),
            Self::Fraction(n) => Value::Number(f64::from_bits(*n)),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Data {
    Missing,
    Nil,
    Boolean(bool),
    Integer(i64),
    Number(f64),
    String(String),
    Array(Vec<Data>),
    Object(BTreeMap<String, Data>),
    Table(BTreeMap<Key, Data>),
    Results(Vec<Data>),
}
impl PartialEq for Data {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Missing, Self::Missing) | (Self::Nil, Self::Nil) => true,
            (Self::Boolean(a), Self::Boolean(b)) => a == b,
            (Self::Integer(a), Self::Integer(b)) => a == b,
            (Self::Number(a), Self::Number(b)) => a == b,
            (Self::Integer(a), Self::Number(b)) | (Self::Number(b), Self::Integer(a)) => {
                i64::from_value(Value::Number(*b)).is_ok_and(|b| *a == b)
            }
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::Object(a), Self::Object(b)) => a == b,
            (Self::Table(a), Self::Table(b)) => a == b,
            (Self::Results(a), Self::Results(b)) => a == b,
            _ => false,
        }
    }
}
impl PartialEq<Json> for Data {
    fn eq(&self, other: &Json) -> bool {
        match other {
            Json::Null => matches!(self, Self::Nil),
            Json::Bool(b) => self.as_bool() == Some(*b),
            Json::String(s) => self.as_str() == Some(s.as_str()),
            Json::Number(n) => {
                if let Some(n) = n.as_i64() {
                    self.as_i64() == Some(n)
                } else {
                    match self {
                        Self::Integer(actual) => {
                            i64::from_value(Value::Number(n.as_f64().unwrap()))
                                .is_ok_and(|n| *actual == n)
                        }
                        Self::Number(actual) => Some(*actual) == n.as_f64(),
                        _ => false,
                    }
                }
            }
            Json::Array(expected) => {
                matches!(self,Self::Array(actual) if actual.len()==expected.len() && actual.iter().zip(expected).all(|(a,b)| a==b))
            }
            Json::Object(expected) => {
                matches!(self,Self::Object(actual) if actual.len()==expected.len() && expected.iter().all(|(key,value)| actual.get(key).is_some_and(|a| a==value)))
            }
        }
    }
}
impl PartialEq<Data> for Json {
    fn eq(&self, other: &Data) -> bool {
        other == self
    }
}
macro_rules! scalar_eq {
    ($t:ty, $variant:ident) => {
        impl PartialEq<$t> for Data {
            fn eq(&self, other: &$t) -> bool {
                *self == Self::$variant(*other)
            }
        }
    };
}
scalar_eq!(i64, Integer);
scalar_eq!(f64, Number);
scalar_eq!(bool, Boolean);
impl PartialEq<&str> for Data {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == Some(*other)
    }
}
impl PartialEq<String> for Data {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == Some(other.as_str())
    }
}
impl From<Json> for Data {
    fn from(value: Json) -> Self {
        match value {
            Json::Null => Self::Nil,
            Json::Bool(b) => Self::Boolean(b),
            Json::Number(n) => n
                .as_i64()
                .map(Self::Integer)
                .unwrap_or_else(|| Self::Number(n.as_f64().unwrap())),
            Json::String(s) => Self::String(s),
            Json::Array(a) => Self::Array(a.into_iter().map(Self::from).collect()),
            Json::Object(o) => {
                Self::Object(o.into_iter().map(|(k, v)| (k, Self::from(v))).collect())
            }
        }
    }
}
impl FromLua for Data {
    fn from_value(value: Value) -> mlua::Result<Self> {
        Self::capture(value, &mut Vec::new())
    }
}
impl Data {
    pub fn from_lupa_results(mut values: MultiValue) -> mlua::Result<Self> {
        match values.len() {
            0 => Ok(Self::Nil),
            1 => Self::from_value(values.pop_front().unwrap()),
            _ => Ok(Self::Results(
                values
                    .into_iter()
                    .map(Self::from_value)
                    .collect::<mlua::Result<_>>()?,
            )),
        }
    }
    pub fn python_truthy(&self) -> bool {
        match self {
            Self::Missing => panic!("missing dictionary key"),
            Self::Nil => false,
            Self::Boolean(b) => *b,
            Self::Integer(n) => *n != 0,
            Self::Number(n) => *n != 0.0,
            Self::String(s) => !s.is_empty(),
            // plain() produces Python containers; their emptiness matters.
            Self::Array(a) | Self::Results(a) => !a.is_empty(),
            Self::Object(o) => !o.is_empty(),
            Self::Table(o) => !o.is_empty(),
        }
    }
    fn capture(value: Value, stack: &mut Vec<usize>) -> mlua::Result<Self> {
        Ok(match value {
            Value::Nil => Self::Nil,
            Value::Boolean(b) => Self::Boolean(b),
            Value::Integer(n) => Self::Integer(n),
            Value::Number(n) => Self::Number(n),
            Value::String(s) => Self::String(s.to_str()?.to_string()),
            Value::Table(t) => {
                let id = t.to_pointer() as usize;
                if stack.contains(&id) {
                    return Err(error("cyclic snapshot table"));
                }
                stack.push(id);
                let mut entries = BTreeMap::new();
                for pair in t.pairs::<Value, Value>() {
                    let (key, value) = pair?;
                    entries.insert(Key::from_value(key)?, Self::capture(value, stack)?);
                }
                stack.pop();
                if !entries.is_empty()
                    && entries.keys().eq((1..=entries.len() as i64)
                        .map(Key::Integer)
                        .collect::<Vec<_>>()
                        .iter())
                {
                    Self::Array(entries.into_values().collect())
                } else if entries.keys().all(|k| matches!(k, Key::String(_))) {
                    Self::Object(
                        entries
                            .into_iter()
                            .map(|(k, v)| {
                                let Key::String(k) = k else { unreachable!() };
                                (k, v)
                            })
                            .collect(),
                    )
                } else {
                    Self::Table(entries)
                }
            }
            other => return Err(error(format!("unsupported snapshot value {other:?}"))),
        })
    }
    pub fn to_lua(&self, lua: &Lua) -> Value {
        match self {
            Self::Missing => panic!("cannot restore a missing key as nil"),
            Self::Results(_) => panic!("multiple results are not a Lua table"),
            Self::Nil => Value::Nil,
            Self::Boolean(b) => Value::Boolean(*b),
            Self::Integer(n) => Value::Integer(*n),
            Self::Number(n) => Value::Number(*n),
            Self::String(s) => Value::String(lua.create_string(s).unwrap()),
            Self::Array(a) => {
                let t = lua.create_table().unwrap();
                for (i, v) in a.iter().enumerate() {
                    t.raw_set(i + 1, v.to_lua(lua)).unwrap();
                }
                Value::Table(t)
            }
            Self::Object(o) => {
                let t = lua.create_table().unwrap();
                for (k, v) in o {
                    t.raw_set(k.as_str(), v.to_lua(lua)).unwrap();
                }
                Value::Table(t)
            }
            Self::Table(o) => {
                let t = lua.create_table().unwrap();
                for (k, v) in o {
                    t.raw_set(k.to_lua(lua), v.to_lua(lua)).unwrap();
                }
                Value::Table(t)
            }
        }
    }
    pub fn as_str(&self) -> Option<&str> {
        if let Self::String(s) = self {
            Some(s)
        } else {
            None
        }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Integer(n) => Some(*n),
            Self::Number(n) => i64::from_value(Value::Number(*n)).ok(),
            _ => None,
        }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Integer(n) => Some(*n as f64),
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }
    pub fn as_bool(&self) -> Option<bool> {
        if let Self::Boolean(b) = self {
            Some(*b)
        } else {
            None
        }
    }
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Nil)
    }
    pub fn as_array(&self) -> Option<&Vec<Data>> {
        if let Self::Array(a) = self {
            Some(a)
        } else {
            None
        }
    }
    pub fn as_object(&self) -> Option<&BTreeMap<String, Data>> {
        if let Self::Object(o) = self {
            Some(o)
        } else {
            None
        }
    }
    pub fn as_object_mut(&mut self) -> Option<&mut BTreeMap<String, Data>> {
        if let Self::Object(o) = self {
            Some(o)
        } else {
            None
        }
    }
    pub fn get(&self, key: &str) -> Option<&Data> {
        match self {
            Self::Object(o) => o.get(key),
            Self::Table(o) => o.get(&Key::String(key.into())),
            _ => panic!("expected dictionary, got {self:?}"),
        }
    }
    pub fn entries(&self) -> BTreeMap<Key, Data> {
        match self {
            Self::Array(a) => a
                .iter()
                .enumerate()
                .map(|(i, v)| (Key::Integer(i as i64 + 1), v.clone()))
                .collect(),
            Self::Object(o) => o
                .iter()
                .map(|(k, v)| (Key::String(k.clone()), v.clone()))
                .collect(),
            Self::Table(o) => o.clone(),
            _ => panic!("expected table, got {self:?}"),
        }
    }
}
static MISSING: Data = Data::Missing;
impl Index<&str> for Data {
    type Output = Data;
    fn index(&self, key: &str) -> &Data {
        match self {
            Self::Object(o) => o.get(key).unwrap_or(&MISSING),
            Self::Table(o) => o.get(&Key::String(key.into())).unwrap_or(&MISSING),
            _ => &MISSING,
        }
    }
}
impl Index<&String> for Data {
    type Output = Data;
    fn index(&self, key: &String) -> &Data {
        &self[key.as_str()]
    }
}
impl Index<usize> for Data {
    type Output = Data;
    fn index(&self, key: usize) -> &Data {
        match self {
            Self::Array(a) | Self::Results(a) => a.get(key).unwrap_or(&MISSING),
            _ => &MISSING,
        }
    }
}
impl IndexMut<&str> for Data {
    fn index_mut(&mut self, key: &str) -> &mut Data {
        self.as_object_mut()
            .expect("expected string-keyed table")
            .entry(key.into())
            .or_insert(Data::Nil)
    }
}
impl fmt::Display for Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Integer(n) => n.fmt(f),
            Self::Number(n) => n.fmt(f),
            Self::String(s) => s.fmt(f),
            Self::Boolean(b) => b.fmt(f),
            _ => Err(fmt::Error),
        }
    }
}
