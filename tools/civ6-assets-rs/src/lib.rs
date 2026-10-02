//! Japan-map and JAC art tools. The migration never invokes Python.
pub mod build_art;
pub mod cli;
pub mod constants;
pub mod draw_crests;
pub mod generate;
pub mod generate_portraits;
pub mod prosper_start;
pub mod prosperity;
pub mod start_1534;
pub mod unification_test_map;
pub mod write_city_names;
pub mod write_custom_civs;
pub mod write_feuds;
pub mod write_support;
use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
pub fn root() -> PathBuf {
    if let Some(root) = std::env::var_os("CIV6_ASSETS_ROOT") {
        return root.into();
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|p| p.join(".claude/CLAUDE.md").is_file())
        .expect("repository root")
        .to_owned()
}
pub fn read_json(path: impl AsRef<Path>) -> Result<Value> {
    let path = path.as_ref();
    serde_json::from_slice(&fs::read(path).with_context(|| format!("read {}", path.display()))?)
        .with_context(|| format!("parse {}", path.display()))
}
pub fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes).with_context(|| format!("write {}", path.display()))
}
pub fn write_json(path: impl AsRef<Path>, value: &Value) -> Result<()> {
    write(path, format!("{}\n", serde_json::to_string_pretty(value)?))
}
pub fn s(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}
pub fn n(value: &Value) -> i32 {
    value.as_i64().unwrap_or(0) as i32
}
pub fn f(value: &Value) -> f64 {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0.0)
}
pub fn arr(value: &Value) -> &[Value] {
    value.as_array().map(Vec::as_slice).unwrap_or(&[])
}
pub fn round(value: f64, digits: i32) -> f64 {
    if digits == 0 {
        return value.round_ties_even();
    }
    if digits > 0 {
        return format!("{:.*}", digits as usize, value)
            .parse()
            .expect("formatted float");
    }
    let scale = 10f64.powi(-digits);
    (value / scale).round_ties_even() * scale
}
pub fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}
pub fn hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(not(windows))]
    {
        let _ = command;
    }
}
pub fn xml(path: impl AsRef<Path>) -> Result<xmltree::Element> {
    Ok(xmltree::Element::parse(fs::File::open(path)?)?)
}
pub fn xml_write(path: impl AsRef<Path>, element: &xmltree::Element) -> Result<()> {
    let mut data = Vec::new();
    element.write_with_config(
        &mut data,
        xmltree::EmitterConfig::new()
            .perform_indent(true)
            .indent_string("\t"),
    )?;
    write(path, data)
}
pub fn elements(element: &xmltree::Element) -> impl Iterator<Item = &xmltree::Element> {
    element
        .children
        .iter()
        .filter_map(xmltree::XMLNode::as_element)
}
pub fn rows<'a>(element: &'a xmltree::Element, table: &str) -> Vec<&'a xmltree::Element> {
    element
        .get_child(table)
        .map(|t| elements(t).filter(|r| r.name == "Row").collect())
        .unwrap_or_default()
}
pub fn attr<'a>(element: &'a xmltree::Element, name: &str) -> &'a str {
    element
        .attributes
        .get(name)
        .map(String::as_str)
        .unwrap_or("")
}
pub fn text_of(element: &xmltree::Element, name: &str) -> String {
    element
        .get_child(name)
        .and_then(|e| e.get_text())
        .map(|s| s.into_owned())
        .unwrap_or_default()
}
pub fn node(name: &str, attributes: &[(&str, String)]) -> xmltree::Element {
    let mut e = xmltree::Element::new(name);
    for (k, v) in attributes {
        e.attributes.insert((*k).to_owned(), v.clone());
    }
    e
}
pub fn push(parent: &mut xmltree::Element, element: xmltree::Element) {
    parent.children.push(xmltree::XMLNode::Element(element));
}
