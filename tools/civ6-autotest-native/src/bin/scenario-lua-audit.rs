//! Compile every scenario Lua chunk without executing game operations.
use mlua::Lua;
use serde_json::Value;
use std::{fs, path::PathBuf};

fn collect<'a>(value: &'a Value, path: &str, chunks: &mut Vec<(String, &'a str)>) {
    match value {
        Value::Object(values) => {
            for (key, value) in values {
                let next = format!("{path}.{key}");
                if key == "code" {
                    if let Some(code) = value.as_str() {
                        chunks.push((next, code));
                        continue;
                    }
                }
                collect(value, &next, chunks);
            }
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                collect(value, &format!("{path}[{index}]"), chunks);
            }
        }
        _ => {}
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let directory = args.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../civ6-autotest/scenarios")
    });
    let expected: usize = args
        .next()
        .ok_or("expected scenario count required")?
        .to_str()
        .ok_or("count must be text")?
        .parse()?;
    if args.next().is_some() || expected == 0 {
        return Err("usage: scenario-lua-audit <scenario directory> <positive count>".into());
    }
    let mut files = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    files.retain(|path| {
        path.extension()
            .is_some_and(|extension| extension == "json")
    });
    files.sort();
    if files.len() != expected {
        return Err(format!("expected {expected} scenarios, found {}", files.len()).into());
    }
    let lua = Lua::new();
    let mut count = 0;
    for file in &files {
        let bytes = fs::read(file)?;
        let value: Value =
            serde_json::from_slice(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes))?;
        let mut chunks = Vec::new();
        collect(&value, "$", &mut chunks);
        for (path, code) in chunks {
            lua.load(format!("return function()\n{code}\nend"))
                .set_name(format!("{}:{path}", file.display()))
                .into_function()?;
            count += 1;
        }
    }
    if count == 0 {
        return Err("no Lua chunks found".into());
    }
    println!(
        "PASS: {} scenarios, {count} Lua 5.1 chunks, no skipped chunks",
        files.len()
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("FAIL: {error}");
        std::process::exit(1);
    }
}
