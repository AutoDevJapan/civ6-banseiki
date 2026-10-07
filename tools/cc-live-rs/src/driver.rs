//! Build and run the driver from the checked-out branch, never an installed copy.
use crate::*;
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "tools/civ6-autotest-rs/Cargo.toml";

/// Match the driver's step defaults and allow its single internal startup
/// recovery (second attempt plus bounded diagnostics). The chain further caps
/// this budget at the remaining shared lease; it never reruns the scenario.
pub fn scenario_timeout(scenario: &Path) -> Result<f64, String> {
    let data = read_json(scenario)?;
    let steps = data["steps"]
        .as_array()
        .filter(|s| !s.is_empty())
        .ok_or("scenario needs a nonempty steps array")?;
    let mut seconds = 60.0;
    if steps[0]["action"] != "restart_game" {
        seconds += 2.0 * 360.0 + 240.0;
    }
    for step in steps {
        let action = step["action"]
            .as_str()
            .ok_or("scenario step action missing")?;
        if action == "manual" {
            return Err(
                "live_chain requires bounded automated steps; manual is unsupported".into(),
            );
        }
        let restart = action == "restart_game";
        let timeout = match step.get("timeout") {
            Some(v) => v
                .as_f64()
                .filter(|n| n.is_finite() && *n > 0.0)
                .ok_or("scenario timeout must be finite and positive")?,
            None => {
                if restart {
                    360.0
                } else {
                    120.0
                }
            }
        };
        seconds += if restart {
            2.0 * timeout + 240.0
        } else {
            timeout
        };
        if !seconds.is_finite() {
            return Err("scenario timeout sum overflowed".into());
        }
    }
    Ok(seconds)
}

fn binary(worktree: &Path, name: &str) -> PathBuf {
    let name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    };
    worktree.join("tools/target/release").join(name)
}

/// Rebuild even when the previous branch left a binary in the target directory.
pub fn prepare(r: &mut impl Runner, p: &Paths, worktree: &Path) -> Result<(), String> {
    let manifest = worktree.join(MANIFEST);
    if !manifest.is_file() {
        return Err("Rust driver manifest is missing; Python drivers are unsupported".into());
    }
    let cargo = std::env::var("CC_LIVE_CARGO").unwrap_or_else(|_| {
        if cfg!(windows) {
            path(p.profile.join(".cargo/bin/cargo.exe"))
        } else {
            "cargo".into()
        }
    });
    let mut names = vec!["civ6-autotest", "civ6-tuner"];
    let helper = worktree.join("tools/civ6-autotest-rs/src/bin/civ6-startup-diagnostics.rs");
    if worktree
        .join("tools/civ6-autotest-rs/src/startup.rs")
        .is_file()
        && !helper.is_file()
    {
        return Err(
            "Startup recovery is present but its diagnostic helper source is missing".into(),
        );
    }
    if helper.is_file() {
        names.push("civ6-startup-diagnostics");
    }
    let mut argv = args(&[
        &cargo,
        "build",
        "--locked",
        "--release",
        "--message-format=json",
        "--manifest-path",
        &path(manifest),
        "--target-dir",
        &path(worktree.join("tools/target")),
    ]);
    for name in &names {
        argv.extend(args(&["--bin", name]));
    }
    let out = r.run(Call::new(argv).timeout(1200.0).managed())?;
    if out.code != 0 {
        return Err(format!(
            "Rust driver build failed (exit {}): {} {}",
            out.code,
            tail(&out.stdout, 800),
            tail(&out.stderr, 1600)
        ));
    }
    for name in names {
        let expected = binary(worktree, name);
        if !expected.is_file() {
            return Err(format!("Rust driver build did not produce {name}"));
        }
        let expected = expected.canonicalize().map_err(|e| e.to_string())?;
        let manifest = worktree
            .join(MANIFEST)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let built_here = out
            .stdout
            .lines()
            .filter_map(|line| serde_json::from_str::<Value>(line).ok())
            .any(|message| {
                message["reason"] == "compiler-artifact"
                    && message["target"]["name"] == name
                    && message["target"]["kind"]
                        .as_array()
                        .is_some_and(|k| k.iter().any(|v| v == "bin"))
                    && message["manifest_path"]
                        .as_str()
                        .and_then(|s| Path::new(s).canonicalize().ok())
                        .as_ref()
                        == Some(&manifest)
                    && message["executable"]
                        .as_str()
                        .and_then(|s| Path::new(s).canonicalize().ok())
                        .as_ref()
                        == Some(&expected)
            });
        if !built_here {
            return Err(format!(
                "Cargo did not report {name} from this branch at {}; refusing a stale executable",
                expected.display()
            ));
        }
    }
    Ok(())
}

pub fn scenario(_p: &Paths, worktree: &Path) -> Result<Vec<String>, String> {
    command(worktree, "civ6-autotest")
}

pub fn tuner(_p: &Paths, worktree: &Path) -> Result<Vec<String>, String> {
    command(worktree, "civ6-tuner")
}

fn command(worktree: &Path, name: &str) -> Result<Vec<String>, String> {
    if worktree.join(MANIFEST).is_file() {
        let executable = binary(worktree, name);
        if !executable.is_file() {
            return Err(format!(
                "Rust driver is present but {} has not been built",
                executable.display()
            ));
        }
        Ok(vec![path(executable)])
    } else {
        Err("Rust driver manifest is missing; Python drivers are unsupported".into())
    }
}
