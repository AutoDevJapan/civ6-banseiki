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
const SALE_SHA: &str = "e84d34a97d4c10406b97dcf4e22716ab788453d282e76b6ccac10a86c9bc43cc";
const RECOVERY_OUT: &str =
    "D:/CC/Work/todo1665/20261007-current/pinned-border-sale-load-recovery-live.json";
const RECOVERY_ROOT: &str = "D:/CC/Work/todo1665/20261007-current";
const SAVE_SHA: &str = "6529c3edefbd5db0a36c5bfb6c23ae066c78fee52448c7d3954c1d6dfbd10487";
const MOD_IDS: &str = "26c967d0-c10d-4eeb-8aaf-c83dc7e621e7,403be889-1455-4de7-967b-ad54c0ff3431,4873eb62-8ccc-4574-b784-dda455e74e68,4a72b1c0-958b-4bd5-81e4-a094f86ce643,7696e937-2044-4f3c-8e13-4e27632e2710,8424840c-92ef-4426-a9b4-b4e0cb818049,91cf7b65-54bb-4f69-a452-19a65527cde2,ba3a1c58-9287-4654-a22f-85f9e6841e9b,bba3a0d2-8f08-4715-9263-00a773e6ae86,bd7a82c5-6b38-4b69-a7f5-8dc9af534249";
fn audit_failed_load(
    scene: &Value,
    raw: &str,
    result: &Value,
    claim: &Value,
) -> Result<(), String> {
    let rows: Vec<Value> = raw
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let steps = scene["steps"]
        .as_array()
        .ok_or("prior scene steps missing")?;
    if steps.len() != 28 || steps[0] != json!({"action":"restart_game","timeout":420})
        || steps[1]["action"] != "load" || steps[1]["save_sha256"] != SAVE_SHA
        || steps[1]["resume_once"] != true || rows.len() != 5
        || rows[0]["action"] != "start" || rows[0]["status"] != "OK"
        || rows[1]["action"] != "1/28 restart_game" || rows[1]["status"] != "START"
        || rows[2]["action"] != "1/28 restart_game" || rows[2]["status"] != "OK"
        || rows[3]["action"] != "2/28 load" || rows[3]["status"] != "START"
        || rows[4]["action"] != "2/28 load" || rows[4]["status"] != "FAIL"
        || result["passed"] != false || result["completed_steps"] != 1
        || result["checks"] != json!([])
        || result["failure"] != format!("2/28 load: {}", rows[4]["detail"].as_str().ok_or("failure detail missing")?)
        || claim["sha256"] != SAVE_SHA || claim["save"] != steps[1]["name"]
        || claim["output"] != "C:/Users/makhmeto/Desktop/dev/ADJ/civ6-banseiki-wt/live\\tools/civ6-autotest/artifacts/todo880-citizen-12-07"
    { return Err("prior run does not prove a stopped load before all transaction operations".into()); }
    for (row, step) in [(1, 0), (3, 1)] {
        let sent: Value =
            serde_json::from_str(rows[row]["detail"].as_str().ok_or("START detail missing")?)
                .map_err(|e| e.to_string())?;
        if sent != steps[step] {
            return Err("prior START differs from pinned scene".into());
        }
    }
    Ok(())
}
fn recovery_admission() -> Result<Value, Box<dyn std::error::Error>> {
    let pinned = [
        (
            "border-sale-live-15/steps.jsonl",
            "1e972c199a6900978d17bd3050e8aae767ce5ef33ea544c975ef798413bb23d8",
        ),
        (
            "border-sale-live-15/result.json",
            "c74d67b443cab03e05fb5d9dfbfa923a9f55785f2e9601c41cef23ca6ee06746",
        ),
        (
            "pinned-border-sale-live.json",
            "6934ff56280bd5491c516ca36e8d976b98e62b90089cabc41212a9852e8575c1",
        ),
        (
            "barter-pinned-live-14/steps.jsonl",
            "6dbcecd10f9cd686be881ca7204d3508cb193bc217c976809b3ee0aeea8458b8",
        ),
    ];
    let mut bytes = Vec::new();
    for (path, hash) in pinned {
        let data = std::fs::read(Path::new(RECOVERY_ROOT).join(path))?;
        if format!("{:x}", Sha256::digest(&data)) != hash {
            return Err(format!("recovery evidence changed: {path}").into());
        }
        bytes.push(data);
    }
    let scene: Value = serde_json::from_slice(&bytes[2])?;
    let result: Value = serde_json::from_slice(&bytes[1])?;
    let saves =
        Path::new("C:/Users/makhmeto/Documents/My Games/Sid Meier's Civilization VI/Saves/Single");
    let claim_path = saves.join(format!("resume-once-{SAVE_SHA}.json"));
    let claim: Value = serde_json::from_slice(&std::fs::read(&claim_path)?)?;
    audit_failed_load(&scene, std::str::from_utf8(&bytes[0])?, &result, &claim)?;
    let save = saves.join(format!(
        "{}.Civ6Save",
        claim["save"].as_str().ok_or("save missing")?
    ));
    if format!("{:x}", Sha256::digest(std::fs::read(&save)?)) != SAVE_SHA {
        return Err("latest save changed".into());
    }
    let mods: Vec<Value> = std::str::from_utf8(&bytes[3])?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    if mods
        .iter()
        .filter(|r| {
            r["action"] == "4/25 ui_lua"
                && r["status"] == "OK"
                && r["detail"]
                    == format!("ran Lua in InGame; return=ACTIVE_MOD_INPUTS_VERIFIED:{MOD_IDS}")
        })
        .count()
        != 1
    {
        return Err("preserved active mod set differs".into());
    }
    Ok(
        json!({"prior_artifact":"todo880-citizen-12-07","source_hashes":pinned,
        "claim":claim,"save_sha256":SAVE_SHA,"all_prior_started_operations_match":true,
        "transaction_steps_started":0,"old_claim_preserved":true,"new_admission":"create_new recovery output and admission file",
        "mods":MOD_IDS.split(',').collect::<Vec<_>>()}),
    )
}
fn output_for_hash(hash: &str) -> Result<&'static str, String> {
    match hash {
        SCENE_SHA => Ok("D:/CC/Work/todo1665/20261007-current/pinned-barter-live.json"),
        SALE_SHA => Ok("D:/CC/Work/todo1665/20261007-current/pinned-border-sale-live.json"),
        _ => Err("scenario differs from reviewed task profiles".into()),
    }
}
static COMMAND_INDEX: AtomicUsize = AtomicUsize::new(0);
fn check_input(args: &[String], bytes: &[u8]) -> Result<Value, String> {
    if args.len() != 3 || args[0] != "live" || args[1] != BRANCH {
        return Err("only the fixed barter preparation is supported".into());
    }
    output_for_hash(&format!("{:x}", Sha256::digest(bytes)))?;
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
#[cfg(test)]
fn mod_guard(inventory: &Value) -> String {
    mod_guard_expected(inventory, None)
}
fn mod_guard_expected(inventory: &Value, expected: Option<&str>) -> String {
    let verify = expected.map_or(String::new(), |ids| format!(
        "assert(table.concat(ids,',')=='{ids}','active mods differ; expected={ids}; actual='..table.concat(ids,','))"));
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
table.sort(ids)
{verify}
return 'ACTIVE_MOD_INPUTS_VERIFIED:'..table.concat(ids,',')
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
fn prepare_scene(scene: &mut Value, recovery: bool, inventory: &Value) -> Result<(), String> {
    scene["steps"]
        .as_array_mut()
        .ok_or("steps")?
        .insert(0, json!({"action":"restart_game","timeout":420}));
    if recovery {
        // The old resume-once claim remains intact. The new output is admitted
        // only after auditing every event in the stopped prior run.
        scene["steps"][1]
            .as_object_mut()
            .ok_or("load missing")?
            .remove("resume_once");
        scene["steps"].as_array_mut().unwrap().insert(
            1,
            json!({"action":"enable_mods",
            "ids":MOD_IDS.split(',').collect::<Vec<_>>(),"exclusive_user_mods":true}),
        );
    }
    scene["steps"].as_array_mut().unwrap().insert(
        if recovery { 4 } else { 3 },
        json!({"action":"ui_lua","state_name":"InGame",
        "code":mod_guard_expected(inventory, recovery.then_some(MOD_IDS)),"preserve_diplomacy_session":true}),
    );
    civ6_autotest_rs::scenario::validate_scenario(scene).map_err(|e| e.to_string())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("expected live fixed-branch reviewed-scenario".into());
    }
    let bytes = std::fs::read(&args[2])?;
    let mut scene = check_input(&args, &bytes)?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let recovery = match std::env::var("CIV6_BORDER_SALE_LOAD_RECOVERY").as_deref() {
        Err(std::env::VarError::NotPresent) => false,
        Ok("1") => true,
        _ => return Err("invalid recovery option".into()),
    };
    if recovery && hash != SALE_SHA {
        return Err("recovery only supports the unsent border sale".into());
    }
    let output = if recovery {
        RECOVERY_OUT
    } else {
        output_for_hash(&hash)?
    };
    if Path::new(output).exists() {
        return Err("prepared output already exists; no repeat admission".into());
    }
    let lock_path = Path::new("D:/CC/Tools/civ6-live/civ6.lock");
    let lock: Value = serde_json::from_slice(&std::fs::read(lock_path)?)?;
    check_lock(&lock, &std::env::var("CODEX_JOB_ID")?)?;
    let admission = if recovery {
        Some(recovery_admission()?)
    } else {
        None
    };
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
        output.replace(".json", "-inputs.json"),
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
    if recovery && Some(recovery_admission()?) != admission {
        return Err("recovery admission changed during preparation".into());
    }
    prepare_scene(&mut scene, recovery, &inventory)?;
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
        .open(output)?;
    if let Some(admission) = admission {
        let proof = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output.replace(".json", "-recovery-admission.json"))?;
        serde_json::to_writer_pretty(&proof, &admission)?;
        proof.sync_all()?;
    }
    serde_json::to_writer_pretty(&out, &scene)?;
    out.sync_all()?;
    eprintln!("PINNED GAME {HEAD}; historical-save diagnostic; integration acceptance=false");
    println!("{output}");
    println!("{}", artifacts.display());
    Ok(())
}
#[cfg(test)]
mod load_recovery_tests {
    use super::*;
    fn fixture() -> (Value, String, Value, Value) {
        // Original, unedited -07 records are embedded so CI needs no game,
        // drive letter, user profile, or external diagnostic directory.
        (
            serde_json::from_str(include_str!(
                "../tests/fixtures/border-sale-load-failure/scene.json"
            ))
            .unwrap(),
            include_str!("../tests/fixtures/border-sale-load-failure/steps.jsonl").to_owned(),
            serde_json::from_str(include_str!(
                "../tests/fixtures/border-sale-load-failure/result.json"
            ))
            .unwrap(),
            serde_json::from_str(include_str!(
                "../tests/fixtures/border-sale-load-failure/claim.json"
            ))
            .unwrap(),
        )
    }
    #[test]
    fn actual_stopped_load_has_no_transaction_steps_and_complete_evidence() {
        let (scene, raw, result, claim) = fixture();
        audit_failed_load(&scene, &raw, &result, &claim).unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(raw.as_bytes())),
            "1e972c199a6900978d17bd3050e8aae767ce5ef33ea544c975ef798413bb23d8"
        );
    }
    #[test]
    fn any_extra_send_or_event_rejects_recovery() {
        let (scene, raw, result, claim) = fixture();
        let raw = format!("{raw}{{\"action\":\"3/28 ui_lua\",\"status\":\"START\",\"detail\":\"SendWorkingDeal\"}}\n");
        assert!(audit_failed_load(&scene, &raw, &result, &claim).is_err());
    }
    #[test]
    fn truncated_transcript_and_changed_sent_load_are_rejected() {
        let (scene, raw, result, claim) = fixture();
        let truncated = raw.lines().take(4).collect::<Vec<_>>().join("\n");
        assert!(audit_failed_load(&scene, &truncated, &result, &claim).is_err());
        assert!(
            audit_failed_load(&scene, &raw.replace(SAVE_SHA, SALE_SHA), &result, &claim).is_err()
        );
    }
    #[test]
    fn completed_load_and_unrelated_claim_do_not_authorize_recovery() {
        let (scene, raw, mut result, mut claim) = fixture();
        result["completed_steps"] = json!(2);
        assert!(audit_failed_load(&scene, &raw, &result, &claim).is_err());
        result["completed_steps"] = json!(1);
        claim["output"] = json!("other run");
        assert!(audit_failed_load(&scene, &raw, &result, &claim).is_err());
    }
    #[test]
    fn recovery_activates_measured_mods_before_load_and_preserves_all_sale_steps() {
        let (mut original, _, _, _) = fixture();
        // Remove only the preparer's guard and restart from the actual fixture;
        // retain all original sale payloads and the exact load step.
        original["steps"].as_array_mut().unwrap().remove(3);
        original["steps"].as_array_mut().unwrap().remove(0);
        let inventory = json!({"excluded_modules":[{"id":"3f9ce208-1092-4141-935c-e5359a8abd93"}]});
        let mut scene = original.clone();
        prepare_scene(&mut scene, true, &inventory).unwrap();
        let steps = scene["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 29);
        assert_eq!(steps[1]["action"], "enable_mods");
        assert_eq!(steps[2]["save_sha256"], SAVE_SHA);
        assert!(steps[2].get("resume_once").is_none());
        assert_eq!(steps[3], original["steps"][1]);
        assert_eq!(&steps[5..], &original["steps"].as_array().unwrap()[2..]);
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("preview.json"),
            serde_json::to_vec_pretty(&scene).unwrap(),
        )
        .unwrap();
        println!(
            "RECOVERY_SCENE_JSON:{}",
            serde_json::to_string(&scene).unwrap()
        );
    }
    #[test]
    fn exact_recovery_mod_set_rejects_missing_extra_and_duplicate_mods() {
        let lua = mlua::Lua::new();
        let inv = json!({"excluded_modules":[]});
        let code = mod_guard_expected(&inv, Some(MOD_IDS));
        let make = |ids: Vec<&str>| {
            format!(
                "Modding={{GetActiveMods=function()return {{{}}}end}}",
                ids.iter()
                    .map(|id| format!("{{Id='{id}'}}"))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        let ids: Vec<_> = MOD_IDS.split(',').collect();
        lua.load(make(ids.clone())).exec().unwrap();
        assert_eq!(
            lua.load(&code).eval::<String>().unwrap(),
            format!("ACTIVE_MOD_INPUTS_VERIFIED:{MOD_IDS}")
        );
        let mut missing = ids.clone();
        missing.remove(0);
        let mut extra = ids.clone();
        extra.push("unrelated");
        let mut duplicate = ids.clone();
        duplicate.push(ids[0]);
        for changed in [missing, extra, duplicate] {
            lua.load(make(changed)).exec().unwrap();
            let error = lua.load(&code).exec().unwrap_err().to_string();
            assert!(error.contains("active mods differ; expected="), "{error}");
        }
    }
}
#[test]
fn changed_scenario_and_unowned_chain_are_rejected() {
    assert_ne!(
        output_for_hash(SCENE_SHA).unwrap(),
        output_for_hash(SALE_SHA).unwrap()
    );
    assert!(output_for_hash("unknown").is_err());
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
