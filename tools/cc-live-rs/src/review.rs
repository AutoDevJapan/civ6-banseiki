//! Start a Codex review of a batch PR (no sandbox: the Windows sandbox service is hung; told not to modify).
//!
//! review.exe <name> <pr_number> <facts_file>
use crate::*;
use serde_json::json;
pub const TEMPLATE: &str = "Review PR #{pr} of the Civ6 MOD 「Civ6用万世紀」 (current folder = the PR branch feat/{name}; compare with `git diff origin/main...HEAD`). Follow documents/repository-rules.md and the SSOT in documents/spec/ssot (read only the relevant sections). Answer in Japanese.\n\nThis is a review only: do NOT modify, create or delete any file, do not run git commands that change state, do not use gh or the GitHub API, no API keys, do not launch Civ6/Steam, no browser tools, no sub-agents. The sandbox is read-only.\n\nCheck correctness against the SSOT and the task, engine behavior against the official files under \"C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI\" (cite file:line), save/load and multiplayer determinism where relevant, scoping (only RULESET_JAC_BANSEIKI / Sengoku pack / Japan maps as intended), interactions with other features, tests and the live scenario's power to detect failure, and docs. Look for the same class of problem elsewhere, not only where it first appears.\n\nVerified facts (live or by tests) — if a concern contradicts them, still report it but mark it 要実機確認 with your reasoning:\n{facts}\n\nReport findings as Must / Suggestion / Nit with file:line and confidence (高/中/低). Include uncertain concerns too. End with a one-line verdict.\n";

pub fn cli(r: &mut impl Runner, p: &Paths, a: &[String]) -> Result<i32, String> {
    if a.len() < 3 {
        return Err("review needs name, pr_number and facts_file".into());
    }
    let (name, pr, facts) = (&a[0], &a[1], read_text(&a[2])?);
    let prompt = p.here.join(format!("rev_{name}_in.txt"));
    // Replace placeholders in a single pass; braces/placeholders inside facts remain literal.
    let text = TEMPLATE
        .split("{facts}")
        .map(|s| s.replace("{pr}", pr).replace("{name}", name))
        .collect::<Vec<_>>()
        .join(facts.trim());
    write_text(&prompt, &text)?;
    let cj = p
        .jobs
        .parent()
        .ok_or("jobs parent missing")?
        .join("codex_jobs.exe");
    let out = r.run(Call::new(args(&[
        &path(cj),
        "run",
        "--name",
        &format!("rev-{name}"),
        "--model",
        cc_job_policy::model_profile(&format!("rev-{name}"), "review").model,
        "--effort",
        cc_job_policy::model_profile(&format!("rev-{name}"), "review").effort,
        "--sandbox",
        "read-only",
        "--cwd",
        &path(p.wt.join(name)),
        "--prompt-file",
        &path(prompt),
        "--task-type",
        "review",
    ])))?;
    r.print(
        &out.stdout
            .trim()
            .lines()
            .last()
            .map(str::to_owned)
            .unwrap_or_else(|| tail(out.stderr.trim(), 300)),
    )?;
    Ok(0)
}
/// merge_pr.exe <pr> <comment.md> -> post comment, mark ready, squash-merge
pub fn merge_cli(r: &mut impl Runner, p: &Paths, a: &[String]) -> Result<i32, String> {
    if a.len() < 2 {
        return Err("merge_pr needs pr and comment.md".into());
    }
    let pr = &a[0];
    let payload = p.temp.join(format!("c{pr}.json"));
    write_json(&payload, &json!({"body":read_text(&a[1])?}))?;
    let out = r.run(Call::new(args(&[
        "gh",
        "api",
        "-X",
        "POST",
        &format!("repos/{}/issues/{pr}/comments", ship::REPO),
        "--input",
        &path(payload),
        "--jq",
        ".html_url",
    ])))?;
    r.print(out.stdout.trim())?;
    r.run(Call::new(args(&[
        "gh",
        "pr",
        "ready",
        pr,
        "-R",
        ship::REPO,
    ])))?;
    let out = r.run(Call::new(args(&[
        "gh",
        "pr",
        "merge",
        pr,
        "-R",
        ship::REPO,
        "--squash",
    ])))?;
    r.print(&if out.code == 0 {
        "merged".into()
    } else {
        head(out.stderr.trim(), 300)
    })?;
    Ok(0)
}
