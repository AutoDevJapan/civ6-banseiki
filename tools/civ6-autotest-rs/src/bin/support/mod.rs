use clap::builder::{OsStringValueParser, TypedValueParser};
use std::path::PathBuf;

pub fn path_value_parser() -> impl TypedValueParser<Value = PathBuf> {
    // Match pathlib.Path('') without trimming or converting native path bytes.
    OsStringValueParser::new().map(|value| {
        if value.is_empty() {
            PathBuf::from(".")
        } else {
            PathBuf::from(value)
        }
    })
}
