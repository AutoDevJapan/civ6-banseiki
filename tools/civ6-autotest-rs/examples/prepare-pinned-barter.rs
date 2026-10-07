//! A task-specific CC_LIVE_SHIP preparer for the historical C25 save.
//! The standard live_chain still owns the lease, process cleanup and execution.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
};
const BRANCH: &str = "agent/todo880-citizen-12";
const HEAD: &str = "5a792461e3ab4bcc0bc393ed5cb75e70fd05f4ff";
const SCENE_SHA: &str = "99fb9e5f20e897414403434504d0c0e9aad3f546577e5b650bea153dadf1127d";
const JOB: &str = "20261006-171922-orch-todo-1665";
const LIVE: &str = "C:/Users/makhmeto/Desktop/dev/ADJ/civ6-banseiki-wt/live";
const OUT: &str = "D:/CC/Work/todo1665/20261007-current/pinned-barter-live.json";
static COMMAND_INDEX: AtomicUsize = AtomicUsize::new(0);
fn check_input(args: &[String], bytes: &[u8]) -> Result<Value, String> {
    if args.len() != 3 || args[0] != "live" || args[1] != BRANCH {
        return Err("only the fixed barter preparation is supported".into());
    }
    if format!("{:x}", Sha256::digest(bytes)) != SCENE_SHA {
        return Err("barter scenario differs from the reviewed bytes".into());
    }
    let scene: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    civ6_autotest_rs::scenario::validate_scenario(&scene).map_err(|e| e.to_string())?;
    Ok(scene)
}
fn check_lock(v: &Value, job: &str) -> Result<(), String> {
    if job != JOB
        || v["job"] != JOB
        || v["live_chain"] != true
        || v["pid"].as_u64().is_none()
        || v["chain_pid"] != v["pid"]
        || v["lock_token"].as_str().is_none_or(str::is_empty)
        || v["pid_created_filetime"].as_u64().is_none()
    {
        return Err("standard chain ownership evidence is missing".into());
    }
    Ok(())
}
fn command(program: &str, args: &[&str]) -> Result<String, String> {
    let prefix = format!(
        "D:/CC/Work/todo1665/20261007-current/pinned-prep-{}-{}",
        std::process::id(),
        COMMAND_INDEX.fetch_add(1, Ordering::Relaxed)
    );
    let stdout_path = format!("{prefix}.stdout.log");
    let stderr_path = format!("{prefix}.stderr.log");
    let stdout = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&stdout_path)
        .map_err(|e| e.to_string())?;
    let stderr = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&stderr_path)
        .map_err(|e| e.to_string())?;
    let meta = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(format!("{prefix}.command.json"))
        .map_err(|e| e.to_string())?;
    serde_json::to_writer(&meta, &json!({"program":program,"args":args}))
        .map_err(|e| e.to_string())?;
    meta.sync_all().map_err(|e| e.to_string())?;
    eprintln!("PREP command logs: {stdout_path} / {stderr_path}");
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let status = command
        .args(args)
        .stdout(Stdio::from(stdout.try_clone().map_err(|e| e.to_string())?))
        .stderr(Stdio::from(stderr.try_clone().map_err(|e| e.to_string())?))
        .status()
        .map_err(|e| e.to_string())?;
    stdout.sync_all().map_err(|e| e.to_string())?;
    stderr.sync_all().map_err(|e| e.to_string())?;
    let out = std::fs::read_to_string(&stdout_path).map_err(|e| e.to_string())?;
    if !status.success() {
        let err = std::fs::read_to_string(&stderr_path).map_err(|e| e.to_string())?;
        let tail = |s: &str| {
            s.chars()
                .rev()
                .take(1200)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<String>()
        };
        return Err(format!(
            "{program} failed; stdout={} stderr={}; logs {stdout_path} / {stderr_path}",
            tail(&out),
            tail(&err)
        ));
    }
    Ok(out)
}
fn extra_inputs() -> Result<Value, String> {
    let output = git(&[
        "ls-files",
        "--others",
        "--exclude-standard",
        "-z",
        "--",
        "poc",
        "tools",
    ])?;
    let root = Path::new(LIVE).canonicalize().map_err(|e| e.to_string())?;
    let mut paths: Vec<_> = output.split('\0').filter(|s| !s.is_empty()).collect();
    paths.sort();
    let mut rows = Vec::new();
    let mut folders = BTreeMap::new();
    for name in paths {
        let p = Path::new(LIVE)
            .join(name)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !p.starts_with(&root) {
            return Err("extra input leaves game worktree".into());
        }
        let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
        rows.push(json!({"path":name,"sha256":format!("{:x}",Sha256::digest(&bytes)),"bytes":bytes.len()}));
        if name.starts_with("tools/") {
            return Err(format!(
                "untracked tool input requires reconciliation: {name}"
            ));
        }
        let folder = name.split('/').take(2).collect::<Vec<_>>().join("/");
        folders.insert(folder, ());
    }
    let mut modules = Vec::new();
    for (folder, _) in folders {
        let mut found = Vec::new();
        for entry in std::fs::read_dir(Path::new(LIVE).join(&folder)).map_err(|e| e.to_string())? {
            let p = entry.map_err(|e| e.to_string())?.path();
            if p.extension().is_some_and(|e| e == "modinfo") {
                let xml = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
                let doc = roxmltree::Document::parse(&xml).map_err(|e| e.to_string())?;
                let root = doc.root_element();
                if root.tag_name().name() != "Mod" {
                    return Err("extra module metadata differs".into());
                }
                let id = uuid::Uuid::parse_str(root.attribute("id").ok_or("module id missing")?)
                    .map_err(|e| e.to_string())?;
                found.push(id.to_string());
            }
        }
        if found.len() != 1 {
            return Err(format!("extra input module ambiguous: {folder}"));
        }
        modules.push(json!({"folder":folder,"id":found[0]}));
    }
    Ok(json!({"rows":rows,"excluded_modules":modules}))
}
fn mod_guard(inventory: &Value) -> String {
    let entries = inventory["excluded_modules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| format!("['{}']=true", m["id"].as_str().unwrap()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"
local blocked={{{entries}}};local active=assert(Modding.GetActiveMods(),'active mods unavailable')
assert(type(active)=='table' and #active>0,'active mods empty/not array')
for k in pairs(active)do assert(type(k)=='number' and k%1==0 and k>=1 and k<=#active,'active mods not array')end
local ids={{}};local boundary=false
for _,m in ipairs(active)do
 assert(type(m)=='table' and type(m.Id)=='string' and m.Id~='','mod ID missing')
 local id=string.lower(m.Id);assert(not blocked[id],'untracked module active: '..id)
 if id=='4a72b1c0-958b-4bd5-81e4-a094f86ce643'then boundary=true end
 ids[#ids+1]=id
end
assert(#ids==#active and boundary,'active mods sparse/required Boundary missing')
table.sort(ids);return 'ACTIVE_MOD_INPUTS_VERIFIED:'..table.concat(ids,',')
"#
    )
}
fn git(args: &[&str]) -> Result<String, String> {
    let mut argv = vec!["-C", LIVE];
    argv.extend_from_slice(args);
    command("git", &argv)
}
fn same_path(a: &str, b: &Path) -> bool {
    Path::new(a)
        .canonicalize()
        .ok()
        .is_some_and(|p| Some(p) == b.canonicalize().ok())
}
fn reported_binary(log: &str, name: &str, manifest: &Path, executable: &Path) -> bool {
    log.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .any(|m| {
            m["reason"] == "compiler-artifact"
                && m["target"]["name"] == name
                && m["target"]["kind"]
                    .as_array()
                    .is_some_and(|k| k.iter().any(|v| v == "bin"))
                && m["manifest_path"]
                    .as_str()
                    .is_some_and(|s| same_path(s, manifest))
                && m["executable"]
                    .as_str()
                    .is_some_and(|s| same_path(s, executable))
        })
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("expected live fixed-branch reviewed-scenario".into());
    }
    let mut scene = check_input(&args, &std::fs::read(&args[2])?)?;
    let lock_path = Path::new("D:/CC/Tools/civ6-live/civ6.lock");
    let lock: Value = serde_json::from_slice(&std::fs::read(lock_path)?)?;
    check_lock(&lock, &std::env::var("CODEX_JOB_ID")?)?;
    // Maintenance is also checked by live_chain before it invokes its preparer.
    let maintenance = Path::new("D:/CC/Tools/civ6-live/reliability-maintenance.json");
    if maintenance.exists() {
        let v: Value = serde_json::from_slice(&std::fs::read(maintenance)?)?;
        if !v["allowed_jobs"]
            .as_array()
            .is_some_and(|a| a.iter().any(|j| j == JOB))
        {
            return Err("live maintenance excludes this job".into());
        }
    }
    git(&["diff", "--quiet", "HEAD", "--"])?;
    git(&["fetch", "-q", "origin"])?;
    let published = git(&["rev-parse", "origin/agent/todo880-citizen-12"])?;
    if published.trim() != HEAD {
        return Err("published fixed game has changed".into());
    }
    git(&[
        "checkout",
        "-q",
        "--detach",
        "origin/agent/todo880-citizen-12",
    ])?;
    if git(&["rev-parse", "HEAD"])?.trim() != HEAD {
        return Err("checked-out game differs".into());
    }
    git(&["diff", "--quiet", "HEAD", "--"])?;
    let inventory = extra_inputs()?;
    std::fs::write(
        "D:/CC/Work/todo1665/20261007-current/pinned-barter-inputs.json",
        serde_json::to_vec_pretty(&inventory)?,
    )?;
    let manifest = Path::new(LIVE).join("tools/civ6-autotest-rs/Cargo.toml");
    let target = Path::new(LIVE).join("tools/target");
    let mut names = vec!["civ6-autotest", "civ6-tuner"];
    let helper = Path::new(LIVE).join("tools/civ6-autotest-rs/src/bin/civ6-startup-diagnostics.rs");
    if Path::new(LIVE)
        .join("tools/civ6-autotest-rs/src/startup.rs")
        .is_file()
        && !helper.is_file()
    {
        return Err("startup diagnostic source missing".into());
    }
    if helper.is_file() {
        names.push("civ6-startup-diagnostics")
    }
    let mut argv = vec![
        "build",
        "--locked",
        "--release",
        "--message-format=json",
        "--manifest-path",
        manifest.to_str().ok_or("manifest encoding")?,
        "--target-dir",
        target.to_str().ok_or("target encoding")?,
    ];
    for name in &names {
        argv.extend(["--bin", *name]);
    }
    let log = command("D:/CC/Tools/codex-jobs/cargo_gate.exe", &argv)?;
    for name in names {
        let executable = target.join("release").join(format!("{name}.exe"));
        if !reported_binary(&log, name, &manifest, &executable) {
            return Err(format!("branch build did not report {name}").into());
        }
    }
    let after: Value = serde_json::from_slice(&std::fs::read(lock_path)?)?;
    if after != lock || git(&["rev-parse", "HEAD"])?.trim() != HEAD {
        return Err("lease or game changed during build".into());
    }
    git(&["diff", "--quiet", "HEAD", "--"])?;
    if extra_inputs()? != inventory {
        return Err("untracked input inventory changed during preparation".into());
    }
    scene["steps"]
        .as_array_mut()
        .ok_or("steps")?
        .insert(0, json!({"action":"restart_game","timeout":420}));
    // Read-only guard after load/wait, before any barter witness or mutation.
    scene["steps"].as_array_mut().unwrap().insert(
        3,
        json!({"action":"ui_lua","state_name":"InGame",
        "code":mod_guard(&inventory),"preserve_diplomacy_session":true}),
    );
    civ6_autotest_rs::scenario::validate_scenario(&scene)?;
    let mut run = 6;
    let artifacts = loop {
        let p = Path::new(LIVE).join(format!(
            "tools/civ6-autotest/artifacts/todo880-citizen-12-{run:02}"
        ));
        if !p.exists() {
            break p;
        }
        run += 1;
    };
    let out = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(OUT)?;
    serde_json::to_writer_pretty(&out, &scene)?;
    out.sync_all()?;
    eprintln!("PINNED GAME {HEAD}; historical-save diagnostic; integration acceptance=false");
    println!("{OUT}");
    println!("{}", artifacts.display());
    Ok(())
}
#[test]
fn changed_scenario_and_unowned_chain_are_rejected() {
    assert!(check_input(&["live".into(), BRANCH.into(), "x".into()], b"{}").is_err());
    assert!(check_input(&["live".into(), "main".into(), "x".into()], b"{}").is_err());
    let good = json!({"job":JOB,"live_chain":true,"pid":10,"chain_pid":10,"pid_created_filetime":1,"lock_token":"t"});
    assert!(check_lock(&good, JOB).is_ok());
    for changed in [
        json!({}),
        json!({"job":JOB,"live_chain":true,"pid":10,"chain_pid":11,"pid_created_filetime":1,"lock_token":"t"}),
    ] {
        assert!(check_lock(&changed, JOB).is_err());
    }
    assert!(check_lock(&good, "other").is_err());
}
#[test]
fn stale_or_missing_cargo_artifact_does_not_authorize_execution() {
    assert!(!reported_binary(
        "",
        "civ6-autotest",
        Path::new("missing"),
        Path::new("missing")
    ));
    let row = json!({"reason":"compiler-artifact","target":{"name":"civ6-autotest","kind":["bin"]},"manifest_path":"missing","executable":"missing"});
    assert!(!reported_binary(
        &row.to_string(),
        "civ6-autotest",
        Path::new("missing"),
        Path::new("missing")
    ));
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let executable = std::env::current_exe().unwrap();
    let valid = json!({"reason":"compiler-artifact","target":{"name":"civ6-autotest","kind":["bin"]},
        "manifest_path":manifest,"executable":executable});
    assert!(reported_binary(
        &valid.to_string(),
        "civ6-autotest",
        &manifest,
        &executable
    ));
    assert!(!reported_binary(
        &valid.to_string(),
        "civ6-tuner",
        &manifest,
        &executable
    ));
}
#[test]
fn active_untracked_module_is_rejected_before_any_barter_preparation() {
    let id = "198d4e36-d6e2-48c4-8d7c-6334df238367";
    let inv = json!({"excluded_modules":[{"id":id}]});
    let lua = mlua::Lua::new();
    lua.load("Modding={GetActiveMods=function()return {{Id='fixed-game-mod'},{Id='4a72b1c0-958b-4bd5-81e4-a094f86ce643'}}end}")
        .exec()
        .unwrap();
    assert!(lua
        .load(mod_guard(&inv))
        .eval::<String>()
        .unwrap()
        .contains("fixed-game-mod"));
    lua.load(format!(
        "Modding.GetActiveMods=function()return {{{{Id='{id}'}},{{Id='4a72b1c0-958b-4bd5-81e4-a094f86ce643'}}}}end"
    ))
    .exec()
    .unwrap();
    let err = lua.load(mod_guard(&inv)).exec().unwrap_err().to_string();
    assert!(err.contains("untracked module active"), "{err}");
    let mutant = mod_guard(&inv).replace(
        "assert(not blocked[id],'untracked module active: '..id)",
        "",
    );
    assert!(
        lua.load(mutant).eval::<String>().is_ok(),
        "independent exclusion mutation must be detected"
    );
    lua.load("Modding.GetActiveMods=function()return {{Name='missing ID'}}end")
        .exec()
        .unwrap();
    assert!(lua.load(mod_guard(&inv)).exec().is_err());
    for list in [
        "{}",
        "{item={Id='4a72b1c0-958b-4bd5-81e4-a094f86ce643'}}",
        "{{Id=''}}",
        "{{Id='other-mod'}}",
    ] {
        lua.load(format!("Modding.GetActiveMods=function()return {list} end"))
            .exec()
            .unwrap();
        assert!(lua.load(mod_guard(&inv)).exec().is_err(), "{list}");
    }
}
