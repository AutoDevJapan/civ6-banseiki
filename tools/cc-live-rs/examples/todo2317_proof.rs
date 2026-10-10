//! One finite observation through the installed standard chain, without another lock.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path, process::Command, time::Duration};

fn hash(p: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let mut file = fs::File::open(p)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:X}", hash.finalize()))
}
fn manifest(root: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    fn visit(
        root: &Path,
        dir: &Path,
        files: &mut Vec<Value>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_symlink() {
                return Err("linked autosave entry".into());
            }
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, files)?;
            } else {
                files.push(json!({"relative":path.strip_prefix(root)?.to_string_lossy().replace('\\',"/"), "bytes":entry.metadata()?.len(), "sha256":hash(&path)?}));
            }
        }
        Ok(())
    }
    let mut files = vec![];
    visit(root, root, &mut files)?;
    files.sort_by(|a, b| a["relative"].as_str().cmp(&b["relative"].as_str()));
    Ok(json!(files))
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let argv: Vec<_> = std::env::args().skip(1).collect();
    if argv.len() != 4 {
        return Err("expected evidence-dir branch scenario-json installed-exe-sha256".into());
    }
    let evidence = Path::new(&argv[0]);
    let paths = cc_live::Paths::default();
    let baseline: Value = serde_json::from_slice(&fs::read(evidence.join("baseline.json"))?)?;
    let auto = Path::new(
        baseline["autosave_path"]
            .as_str()
            .ok_or("missing autosave path")?,
    );
    let options = paths
        .profile
        .join("AppData/Local/Firaxis Games/Sid Meier's Civilization VI/AppOptions.txt");
    let seed = paths.profile.join("Documents/My Games/Sid Meier's Civilization VI/Saves/Single/JAC-Reliability-Seed-1971.Civ6Save");
    let state = paths.here.join("reliability-auto-state.json");
    let executable = paths.here.join("live_chain.exe");
    if manifest(auto)? != baseline["autosaves"]
        || hash(&options)? != baseline["appoptions_sha256"]
        || hash(&seed)? != baseline["seed_sha256"]
    {
        return Err("preflight originals differ from the saved baseline".into());
    }
    if state.exists()
        || paths.here.join("civ6.lock").exists()
        || cc_live::chain::civ6_running(&mut cc_live::process::RealRunner)?
    {
        return Err("preflight requires no existing game, lease or recovery receipt".into());
    }
    if hash(&executable)? != argv[3] {
        return Err("installed chain SHA differs from reviewed candidate".into());
    }
    // CREATE_NEW makes a second invocation fail before launching another game.
    let stdout = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(evidence.join("one-run.stdout.log"))?;
    let stderr = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(evidence.join("one-run.stderr.log"))?;
    let mut command = Command::new(&executable);
    command
        .arg(format!("{}:{}", argv[1], argv[2]))
        .arg("--stop-on-failure")
        .env("CC_LIVE_CARGO", "D:/CC/Tools/codex-jobs/cargo_gate.exe")
        .stdin(std::process::Stdio::null())
        .stdout(stdout)
        .stderr(stderr);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(cc_live::process::CREATE_NO_WINDOW);
    }
    let mut child = command.spawn()?;
    let pid = child.id();
    let started = std::time::SystemTime::now();
    let mut observations = vec![];
    let mut errors = vec![];
    let mut isolated = false;
    let mut owned = false;
    let mut archive_path = None;
    let mut renderer = None;
    // Once spawned, observation failures are recorded; they cannot orphan the chain.
    let exit = loop {
        if let Ok(bytes) = fs::read(&state) {
            let observed = (|| -> Result<Value, Box<dyn std::error::Error>> {
                let first = bytes.split(|b| *b == b'\n').next().ok_or("empty receipt")?;
                let receipt: Value = serde_json::from_slice(first)?;
                let archive =
                    std::path::PathBuf::from(receipt["archive"].as_str().ok_or("missing archive")?);
                let owner = archive.join("owner-auto-original");
                if !owner.is_dir() {
                    return Ok(json!({"phase":"planned"}));
                }
                let originals = manifest(&owner)?;
                if originals != baseline["autosaves"] {
                    return Err("isolated originals changed".into());
                }
                let lock: Value = serde_json::from_slice(&fs::read(paths.here.join("civ6.lock"))?)?;
                if lock["chain_pid"] == pid && lock["live_chain"] == true {
                    owned = true;
                }
                isolated = true;
                archive_path = Some(archive);
                if !evidence.join("isolation-state.raw").exists() {
                    fs::write(evidence.join("isolation-state.raw"), &bytes)?;
                }
                Ok(
                    json!({"phase":"isolated", "originals":originals,"working_auto":manifest(auto)?,"lease":lock}),
                )
            })();
            match observed {
                Ok(value) => observations.push(value),
                Err(error) => errors.push(error.to_string()),
            }
        }
        let renderer_path = paths
            .profile
            .join("Documents/My Games/Sid Meier's Civilization VI/Logs/Renderer.log");
        if let Ok(meta) = fs::metadata(&renderer_path) {
            if meta.modified().is_ok_and(|time| time >= started) {
                if let Ok(text) = fs::read_to_string(&renderer_path) {
                    if text.contains("Microsoft Basic Render Driver") || text.contains("WARP") {
                        renderer = Some(text.clone());
                        let _ = fs::write(evidence.join("Renderer.log"), text);
                    }
                }
            }
        }
        let checkpoint = json!({"pid":pid,"observed_isolation":isolated,"observed_owned_lease":owned,"observations":observations,"observation_errors":errors});
        if let Err(error) = fs::write(
            evidence.join("one-run-observations.json"),
            serde_json::to_vec_pretty(&checkpoint)?,
        ) {
            errors.push(error.to_string());
        }
        match child.try_wait() {
            Ok(Some(exit)) => break exit,
            Ok(None) => (),
            Err(error) => {
                errors.push(error.to_string());
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    };
    let mut journals = vec![];
    for entry in fs::read_dir(&paths.temp)? {
        let entry = entry?;
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(&format!("civ6-steam-exit-{pid}-"))
        {
            let bytes = fs::read(entry.path())?;
            fs::write(evidence.join(entry.file_name()), &bytes)?;
            journals.push(String::from_utf8_lossy(&bytes).into_owned());
        }
    }
    let steam_released = journals.iter().any(|text| {
        text.lines().last().is_some_and(|line| {
            serde_json::from_str::<Value>(line).is_ok_and(|v| {
                matches!(
                    v["phase"].as_str(),
                    Some("exit_sync_complete" | "steam_released_without_new_sync_request")
                )
            })
        })
    });
    let restored = manifest(auto)?;
    let settings_hash = hash(&options)?;
    let seed_hash = hash(&seed)?;
    let game_absent = !cc_live::chain::civ6_running(&mut cc_live::process::RealRunner)?;
    let receipt_absent = !state.exists();
    let lease_absent = !paths.here.join("civ6.lock").exists();
    let success = exit.success()
        && isolated
        && owned
        && errors.is_empty()
        && restored == baseline["autosaves"]
        && settings_hash == baseline["appoptions_sha256"]
        && seed_hash == baseline["seed_sha256"]
        && game_absent
        && receipt_absent
        && lease_absent
        && renderer.is_some()
        && steam_released;
    let result = json!({"todo":2317,"pid":pid,"scenario_runs":1,"branch":argv[1],"scenario":argv[2],"installed_exe_sha256":argv[3],"exit_code":exit.code(),"success":success,"observed_isolation":isolated,"observed_owned_lease":owned,"archive":archive_path,"restored_autosaves":restored,"appoptions_sha256":settings_hash,"seed_sha256":seed_hash,"game_absent":game_absent,"recovery_receipt_absent":receipt_absent,"lease_absent":lease_absent,"cpu_renderer_observed":renderer.is_some(),"steam_released":steam_released,"observation_errors":errors});
    fs::write(
        evidence.join("one-run-result.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    if success {
        Ok(())
    } else {
        Err("one-run evidence incomplete; retain originals and recovery receipts".into())
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
