//! Prepare installed DLC data before launching the shared live checkout.
//! The cache is a generated deployment artifact, not a modification of tracked
//! MOD files or the official game. The next checkout always refreshes its copy.
use crate::*;
use std::{fs, path::Path};

pub const TOOL: &str = "banseiki-ethiopia-prepare";
pub const MOD: &str = "JAC_PoC_BanseikiRuleset";
pub const GAME: &str = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI";

fn location(p: &Path) -> String {
    let value = path(p);
    if cfg!(windows) {
        value
            .trim_start_matches(r"\\?\")
            .trim_start_matches(r"\??\")
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    } else {
        value
    }
}

fn literal(p: &Path) -> String {
    format!("'{}'", path(p).replace('\'', "''"))
}

/// Called by ship only after the shared chain has closed Civ6 and checked out
/// the pushed branch. Missing legacy tooling restores the ordinary source MOD.
pub fn prepare(r: &mut impl Runner, p: &Paths, worktree: &Path) -> Result<(), String> {
    let source = worktree.join("poc").join(MOD);
    if !source.join(format!("{MOD}.modinfo")).is_file() {
        return restore(r, p, worktree);
    }
    let link = p
        .profile
        .join("Documents/My Games/Sid Meier's Civilization VI/Mods")
        .join(MOD);
    let cache = worktree.join("tools/target/local-mods").join(MOD);
    let current = fs::read_link(&link).map_err(|e| {
        format!(
            "shared ruleset must be installed as a junction at {}: {e}",
            link.display()
        )
    })?;
    if location(&current) != location(&source) && location(&current) != location(&cache) {
        return Err(
            "ruleset junction points outside the shared source and its deployment cache".into(),
        );
    }
    let helper_source = worktree
        .join("tools/civ6-data-rs/src/bin")
        .join(format!("{TOOL}.rs"));
    let desired = if helper_source.is_file() {
        let binary = driver::prepare_data_tool(r, p, worktree, TOOL)?;
        let out = r.run(
            Call::new(args(&[
                &path(binary),
                "--game-dir",
                GAME,
                "--output",
                &path(&cache),
                "--refresh",
            ]))
            .timeout(120.0)
            .managed(),
        )?;
        if out.code != 0 {
            return Err(format!(
                "local DLC preparation failed (exit {}): {} {}",
                out.code,
                tail(&out.stdout, 800),
                tail(&out.stderr, 1600)
            ));
        }
        if !cache.join(format!("{MOD}.modinfo")).is_file() {
            return Err("local DLC preparation returned no ruleset manifest".into());
        }
        &cache
    } else {
        &source
    };
    point(r, &link, &current, desired)
}

/// Restore the shared checkout before releasing the chain's single-game slot.
/// Other jobs still using an older ship must not inherit this run's copy.
pub fn restore(r: &mut impl Runner, p: &Paths, worktree: &Path) -> Result<(), String> {
    let source = worktree.join("poc").join(MOD);
    if !source.is_dir() {
        return Ok(());
    }
    let link = p
        .profile
        .join("Documents/My Games/Sid Meier's Civilization VI/Mods")
        .join(MOD);
    let current = match fs::read_link(&link) {
        Ok(target) => target,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("read shared ruleset junction: {error}")),
    };
    let cache = worktree.join("tools/target/local-mods").join(MOD);
    if location(&current) != location(&source) && location(&current) != location(&cache) {
        return Err("cannot restore a ruleset junction owned by another worktree".into());
    }
    point(r, &link, &current, &source)
}

fn point(r: &mut impl Runner, link: &Path, current: &Path, desired: &Path) -> Result<(), String> {
    if location(current) == location(desired) {
        return Ok(());
    }
    // Remove the junction itself, never its target. Recheck the observed target
    // in the same shell and restore it if creating the replacement fails.
    let script = format!(
        r#"$ErrorActionPreference = 'Stop'
$pr126Entry = Get-Item -LiteralPath {link}
if ($pr126Entry.LinkType -ne 'Junction') {{ throw 'Ruleset is not a junction' }}
$pr126Old = [string]@($pr126Entry.Target)[0]
$pr126Normal = $pr126Old.Replace('\\?\','').Replace('\??\','').Replace('/','\').TrimEnd('\').ToLowerInvariant()
if ($pr126Normal -cne '{observed}') {{ throw 'Ruleset junction changed during preparation' }}
Remove-Item -LiteralPath {link}
try {{ New-Item -ItemType Junction -Path {link} -Target {desired} | Out-Null }}
catch {{ if (-not (Test-Path -LiteralPath {link})) {{ New-Item -ItemType Junction -Path {link} -Target $pr126Old | Out-Null }}; throw }}
"#,
        link = literal(link),
        desired = literal(desired),
        observed = location(current).replace('\'', "''"),
    );
    let out = r.run(
        Call::new(args(&[
            "pwsh",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
        ]))
        .timeout(30.0)
        .managed(),
    )?;
    if out.code != 0 {
        return Err(format!(
            "ruleset junction update failed: {}",
            tail(&out.stderr, 1600)
        ));
    }
    let installed = fs::read_link(link).map_err(|e| e.to_string())?;
    if location(&installed) != location(desired) {
        return Err("ruleset junction update was not confirmed".into());
    }
    Ok(())
}
