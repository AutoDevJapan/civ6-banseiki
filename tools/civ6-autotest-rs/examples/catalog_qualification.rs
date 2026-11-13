//! Generate only; the parent owns live execution and interpretation.
use civ6_autotest_rs::{catalog_scenario, Result};
use std::{fs::OpenOptions, io::Write, path::PathBuf};

fn run() -> Result<()> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| {
            civ6_autotest_rs::Error::input("usage: catalog_qualification OUTPUT.json")
        })?;
    let template = serde_json::from_str(catalog_scenario::NORMAL)?;
    let scenario = catalog_scenario::build(&template)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    writeln!(file, "{}", serde_json::to_string_pretty(&scenario)?)?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
