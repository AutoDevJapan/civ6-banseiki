//! Data generators and static contracts for the Sengoku prototype.

use anyhow::{bail, Context, Result};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub type Outputs = BTreeMap<PathBuf, String>;

#[derive(Debug)]
pub struct CliUsageError(String);

impl std::fmt::Display for CliUsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for CliUsageError {}

/// Preserve argparse's unambiguous long-option prefixes and `--` separator.
pub fn parse_check_args(binary: &str, args: &[String], description: &str) -> Result<Option<bool>> {
    let usage = format!("usage: cargo run --manifest-path tools/Cargo.toml -p civ6-data-rs --bin {binary} -- [-h] [--check]");
    let mut check = false;
    let mut positional = false;
    let mut unknown = Vec::new();
    for arg in args {
        if !positional && arg == "--" {
            positional = true;
            continue;
        }
        let long = !positional && arg.starts_with("--") && arg.len() > 2;
        if !positional && (arg == "-h" || (long && "--help".starts_with(arg))) {
            println!("{usage}\n\n{description}\n\noptions:\n  -h, --help  show this help message and exit\n  --check     fail if generated files are stale");
            return Ok(None);
        }
        if long && "--check".starts_with(arg) {
            check = true;
        } else {
            unknown.push(arg.as_str());
        }
    }
    if !unknown.is_empty() {
        return Err(CliUsageError(format!(
            "{usage}\n{binary}: error: unrecognized arguments: {}",
            unknown.join(" ")
        ))
        .into());
    }
    Ok(Some(check))
}

pub fn cli_exit_code(error: &anyhow::Error) -> i32 {
    if error.downcast_ref::<CliUsageError>().is_some() {
        2
    } else {
        1
    }
}

#[derive(Clone, Debug)]
pub struct Repository {
    pub root: PathBuf,
}

impl Repository {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn checked_in() -> Self {
        Self::new(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap(),
        )
    }

    pub fn path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.root.join(path)
    }

    pub fn read(&self, path: impl AsRef<Path>) -> Result<String> {
        let path = self.path(path);
        read_utf8(&path)
    }

    pub fn json(&self, path: impl AsRef<Path>) -> Result<serde_json::Value> {
        let path = path.as_ref();
        serde_json::from_str(&self.read(path)?)
            .with_context(|| format!("parse {}", self.path(path).display()))
    }

    /// Source fixtures keep all official-data checks active on Linux CI.
    pub fn game_path(&self, path: impl AsRef<Path>) -> PathBuf {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/game")
            .join(path.as_ref());
        fixture
    }

    pub fn game_read(&self, path: impl AsRef<Path>) -> Result<String> {
        read_utf8(&self.game_path(path))
    }
}

pub fn read_utf8(path: &Path) -> Result<String> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(text.trim_start_matches('\u{feff}').replace("\r\n", "\n"))
}

pub fn sql_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(crate) const SENGOKU_AUDIT_COMMAND: &str =
    "cargo run --locked --manifest-path tools/Cargo.toml -p civ6-data-rs --bin sengoku-cascade-audit";
pub(crate) const SENGOKU_AUDIT_TEST_REFERENCE: &str =
    "`tools/civ6-data-rs/src/cascade.rs`、関連する静的検査は `tools/civ6-data-rs/src/data_tests.rs`";

/// Only the audit command and its test references change during migration;
/// every other byte of the legacy document remains an exact parity oracle.
pub fn normalize_legacy_document(path: &Path, text: String) -> String {
    if path == Path::new("documents/poc/sengoku-tree-unlocks.md") {
        text.replace("python tools/sengoku_cascade_audit.py", SENGOKU_AUDIT_COMMAND)
            .replace(
                "cargo run --manifest-path tools/Cargo.toml -p civ6-data-rs --bin sengoku-cascade-audit",
                SENGOKU_AUDIT_COMMAND,
            )
            .replace("`tests/test_sengoku_cascade.py`", SENGOKU_AUDIT_TEST_REFERENCE)
    } else {
        text
    }
}

pub fn apply_outputs(repo: &Repository, outputs: Outputs, check: bool) -> Result<()> {
    for (relative, content) in outputs {
        let path = repo.path(&relative);
        if check {
            if !path.is_file() || normalize_legacy_document(&relative, read_utf8(&path)?) != content
            {
                bail!("stale generated file: {}", path.display());
            }
        } else {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, content.as_bytes())
                .with_context(|| format!("write {}", path.display()))?;
        }
    }
    Ok(())
}

pub mod assassin;
/// Source-derived map/art/city-name/feud ports that the assets worker can reuse.
pub mod asset_dependencies;
pub mod cascade;
#[cfg(test)]
mod data_tests;
pub mod events;
#[cfg(test)]
mod hero_tests;
pub mod oath_effect_audit;
pub mod oaths;
pub mod reigns;
pub mod trees;
pub mod unification;
