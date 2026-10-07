//! Commit a finished job's worktree, push, and open a draft PR; or prepare a live run.
//!
//! ship.exe pr <name> "<title>" "<commit message>"
//! ship.exe live <name> <scenario.json> [--no-restart] -> prints the live scenario path and output dir
use crate::*;
use regex::Regex;
use serde_json::json;
use std::path::Path;
pub const REPO: &str = "AutoDevJapan/civ6-banseiki";

pub fn git(r: &mut impl Runner, p: &Path, a: &[&str], check: bool) -> Result<String, String> {
    let mut argv = args(&["git", "-C", &path(p)]);
    argv.extend(args(a));
    let out = r.run(Call::new(argv))?;
    if check && out.code != 0 {
        return Err(format!(
            "git {} failed (exit {}): {}",
            a.join(" "),
            out.code,
            out.stderr
        ));
    }
    Ok(out.stdout)
}
pub fn last_message(p: &Paths, name: &str) -> Result<String, String> {
    let mut runs = std::fs::read_dir(&p.jobs)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .ends_with(&format!("-{name}"))
        })
        .collect::<Vec<_>>();
    runs.sort();
    let text = if let Some(run) = runs.last() {
        read_text(run.join("last_message.md"))?
    } else {
        String::new()
    };
    // Local absolute links are meaningless on GitHub; keep the link text.
    let regex = Regex::new(r"\[([^\]]+)\]\((?:[A-Za-z]:|/)[^)]*\)").unwrap();
    Ok(regex
        .replace_all(&text, "`$1`")
        .replace("``", "`")
        .trim()
        .into())
}
pub fn pr(
    r: &mut impl Runner,
    p: &Paths,
    name: &str,
    title: &str,
    message: &str,
) -> Result<(), String> {
    let wt = p.wt.join(name);
    git(r, &wt, &["add", "-A"], true)?;
    git(r, &wt, &["commit", "-q", "-m", message], true)?;
    git(
        r,
        &wt,
        &["push", "-q", "-u", "origin", &format!("feat/{name}")],
        true,
    )?;
    let body=format!("## 概要\n\n{}\n\n## 実機確認\n\nメインセッションで自動シナリオを実行し、結果をコメントに記録する。\n",last_message(p,name)?);
    let payload = p.temp.join(format!("pr_{name}.json"));
    write_json(
        &payload,
        &json!({"title":title,"head":format!("feat/{name}"),"base":"main","body":body,"draft":true}),
    )?;
    let out = r.run(Call::new(args(&[
        "gh",
        "api",
        "-X",
        "POST",
        &format!("repos/{REPO}/pulls"),
        "--input",
        &path(payload),
        "--jq",
        ".html_url",
    ])))?;
    r.print(&if !out.stdout.trim().is_empty() {
        out.stdout.trim().into()
    } else {
        head(out.stderr.trim(), 300)
    })?;
    Ok(())
}
pub fn live(
    r: &mut impl Runner,
    p: &Paths,
    name: &str,
    scenario: &str,
    restart: bool,
) -> Result<(String, String), String> {
    let live = p.live();
    git(r, &live, &["fetch", "-q", "origin"], true)?;
    let branch = if name.contains('/') || name == "main" {
        name.into()
    } else {
        format!("feat/{name}")
    };
    git(
        r,
        &live,
        &["checkout", "-q", "--detach", &format!("origin/{branch}")],
        true,
    )?;
    // Test the branch as it would be merged, so driver fixes on main reach every PR run
    // (a branch behind #92 still launched DX12 and crashed, pr86-no-art-01, 2026-10-01). Local only, never pushed.
    if branch != "main"
        && r.run(Call::new(args(&[
            "git",
            "-C",
            &path(&live),
            "merge",
            "-q",
            "--no-edit",
            "origin/main",
        ])))?
        .code
            != 0
    {
        git(r, &live, &["merge", "--abort"], false)?;
        r.error(&format!(
            "warning: origin/main does not merge cleanly into {branch}; testing the branch alone"
        ))?;
    }
    let name = name.rsplit('/').next().unwrap();
    // A branch without a mod folder deletes it; a dangling Mods junction then breaks Civ6's mod scan (live 2026-09-29).
    let mods = p
        .profile
        .join("Documents/My Games/Sid Meier's Civilization VI/Mods");
    if let Ok(links) = std::fs::read_dir(mods) {
        for link in links
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("JAC_"))
        {
            let target = live.join("poc").join(link.file_name());
            if !target.exists() {
                std::fs::create_dir_all(target).map_err(|e| e.to_string())?;
            }
        }
    }
    let mut data = read_json(live.join("tools/civ6-autotest/scenarios").join(scenario))?;
    let steps = data["steps"]
        .as_array_mut()
        .ok_or("scenario steps is not an array")?;
    if restart && steps.first().ok_or("scenario has no steps")?["action"] != "restart_game" {
        steps.insert(0, json!({"action":"restart_game","timeout":420}));
    }
    driver::prepare(r, p, &live)?;
    let mut run = 1;
    while live
        .join(format!("tools/civ6-autotest/artifacts/{name}-{run:02}"))
        .exists()
    {
        run += 1;
    }
    for step in steps {
        if matches!(step["action"].as_str(), Some("save" | "load")) && !truthy(&step["exact_name"])
        {
            step["name"] = json!(format!(
                "{}-{name}-{run:02}",
                step["name"].as_str().ok_or("save/load name missing")?
            ));
        }
    }
    let target = p.temp.join(format!("live_{name}_{run:02}.json"));
    write_json(&target, &data)?;
    let artifacts = live.join(format!("tools/civ6-autotest/artifacts/{name}-{run:02}"));
    r.print(&path(&target))?;
    r.print(&path(&artifacts))?;
    Ok((path(target), path(artifacts)))
}
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}
pub fn cli(r: &mut impl Runner, p: &Paths, a: &[String]) -> Result<i32, String> {
    match a.first().map(String::as_str) {
        Some("pr") => {
            if a.len() < 4 {
                return Err("pr needs name, title and commit message".into());
            }
            pr(r, p, &a[1], &a[2], &a[3])?;
        }
        Some("live") => {
            if a.len() < 3 {
                return Err("live needs name and scenario".into());
            }
            live(r, p, &a[1], &a[2], !a.iter().any(|s| s == "--no-restart"))?;
        }
        None => return Err("missing subcommand".into()),
        _ => {}
    }
    Ok(0)
}
