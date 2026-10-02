use crate::Result;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

pub(crate) fn read_text(path: &Path) -> Result<String> {
    Ok(fs::read_to_string(path)?
        .replace("\r\n", "\n")
        .replace('\r', "\n"))
}

pub(crate) fn read_steps(artifact: &Path, strip_blank: bool) -> Result<Vec<Value>> {
    read_text(&artifact.join("steps.jsonl"))?
        .lines()
        .filter(|line| {
            if strip_blank {
                !line.trim().is_empty()
            } else {
                !line.is_empty()
            }
        })
        .map(|line| Ok(serde_json::from_str(line)?))
        .collect()
}

pub(crate) fn read_lua(artifact: &Path) -> Result<(String, String)> {
    let bytes = fs::read(artifact.join("Lua.log"))?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let content =
        String::from_utf8_lossy(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes));
    Ok((content.into_owned(), hash))
}

pub(crate) fn artifact_name(artifact: &Path) -> String {
    artifact
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64() != Some(0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}

pub(crate) fn field<'a>(row: &'a Value, key: &str) -> Result<&'a str> {
    row.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing or invalid field: {key}").into())
}

pub(crate) fn increment(map: &mut Map<String, Value>, key: &str, amount: i64) {
    let next = map.get(key).and_then(Value::as_i64).unwrap_or(0) + amount;
    map.insert(key.to_owned(), next.into());
}

pub(crate) fn counter(map: &Value, key: &str) -> i64 {
    map.get(key).and_then(Value::as_i64).unwrap_or(0)
}

pub(crate) fn character_position(text: &str, byte: usize) -> usize {
    text[..byte].chars().count()
}

/// Path.write_text uses the platform's text newline convention in the originals.
pub fn write_json(path: &Path, value: &Value) -> Result<()> {
    let text = serde_json::to_string_pretty(value)? + "\n";
    #[cfg(windows)]
    let text = text.replace('\n', "\r\n");
    fs::write(path, text)?;
    Ok(())
}

pub(crate) fn print_text(text: &str) -> Result<()> {
    use std::io::{self, Write};
    #[cfg(windows)]
    let text = text.replace('\n', "\r\n");
    io::stdout().lock().write_all(text.as_bytes())?;
    Ok(())
}
