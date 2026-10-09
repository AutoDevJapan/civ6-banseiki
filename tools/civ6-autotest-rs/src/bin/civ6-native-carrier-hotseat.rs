//! Native-carrier witness controller; separate from the growth witness.
use civ6_autotest_rs::{
    hotseat_bridge::{SystemTransport, TransportIo},
    judge,
    runner::Runner,
    scenario,
};
use civ6_autotest_rs::{Error, Result};
use clap::{Parser, Subcommand};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

#[path = "../hotseat_native_carrier.rs"]
mod protocol;

#[derive(Parser)]
#[command(about = "Validate the fixed native-carrier hotseat log protocol")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Prepare {
        output: PathBuf,
        #[arg(long)]
        repo: PathBuf,
    },
    Pregame {
        output: PathBuf,
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        logs: PathBuf,
        #[arg(long)]
        saves: PathBuf,
        #[arg(long, default_value_t = 4318)]
        port: u16,
    },
    Audit {
        input: PathBuf,
        #[arg(long)]
        nonce: String,
    },
    Init {
        output: PathBuf,
        #[arg(long)]
        nonce: String,
        #[arg(long)]
        logs: PathBuf,
        #[arg(long)]
        saves: PathBuf,
    },
    Drive {
        output: PathBuf,
        #[arg(long)]
        logs: PathBuf,
        #[arg(long)]
        saves: PathBuf,
        #[arg(long, default_value_t = 1200.)]
        timeout: f64,
        #[arg(long, default_value_t = 4318)]
        port: u16,
    },
}

const NONCE_KEY: &str = "JAC_NATIVE_CARRIER_WITNESS_NONCE";
const BINDINGS: [&str; 3] = [
    "poc/JAC_PoC_Absorption/Scripts/JAC_NativeCarrierWitness_GP.lua",
    "poc/JAC_PoC_Absorption/UI/JAC_NativeCarrierWitness_UI.lua",
    "poc/JAC_PoC_JapanMapScale/UI/JAC_NativeCarrierWitness_Staging.lua",
];

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Bundle {
    version: u8,
    nonce: String,
    bindings: std::collections::BTreeMap<String, String>,
}

fn file_hash(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}

fn bundle(output: &Path, repo: &Path) -> Result<Bundle> {
    let b: Bundle = serde_json::from_slice(&fs::read(output.join("bundle.json"))?)?;
    if b.version != 1 || !protocol::nonce_valid(&b.nonce) || b.bindings.len() != BINDINGS.len() {
        return Err(Error::scenario("invalid fixed witness bundle"));
    }
    for relative in BINDINGS {
        if b.bindings.get(relative) != Some(&file_hash(&repo.join(relative))?) {
            return Err(Error::scenario(
                "deployed witness differs from prepared bytes",
            ));
        }
    }
    Ok(b)
}

fn pregame(output: &Path, repo: &Path, logs: &Path, saves: &Path, port: u16) -> Result<()> {
    verify_lease()?;
    let b = bundle(output, repo)?;
    let state = state(&output.join("controller.jsonl"))?;
    if state.journal.nonce != b.nonce || state.processed != 0 {
        return Err(Error::scenario(
            "pregame requires this new initialized controller",
        ));
    }
    let mut io = SystemTransport { logs };
    verify_owner(&mut io, &state)?;
    // Fixed shipped configuration only; never accept arbitrary runtime Lua.
    let fixed: serde_json::Value = serde_json::from_str(include_str!(
        "../../../civ6-autotest/scenarios/absorption-native-carrier-hotseat-save.json"
    ))?;
    let mut runner = Runner::new(
        fixed.clone(),
        output.to_owned(),
        logs.to_owned(),
        saves.to_owned(),
        port,
    );
    let sent = output.join("pregame-sent.jsonl");
    let mut receipt = OpenOptions::new().write(true).create_new(true).open(sent)?;
    for index in 0..2 {
        verify_owner(&mut io, &state)?;
        writeln!(receipt, "setup {index}")?;
        receipt.sync_all()?;
        runner.run_frontend_setup_step_checked(&fixed["steps"][index], || {
            verify_owner(&mut io, &state)
        })?;
    }
    let flag=format!("assert(UI.IsInFrontEnd(),'frontend required')\nGameConfiguration.SetValue({}, {})\nGameConfiguration.SetGameModeEnabled('GAMEMODE_HEROES',true)\nreturn GameConfiguration.GetValue({})",scenario::lua_string(NONCE_KEY),scenario::lua_string(&b.nonce),scenario::lua_string(NONCE_KEY));
    writeln!(receipt, "nonce")?;
    receipt.sync_all()?;
    let deadline = io.now() + 30.;
    let answer = runner
        .send_frontend_transition_checked(&flag, deadline, || verify_owner(&mut io, &state))?;
    if answer.trim().trim_matches('"') != b.nonce {
        return Err(Error::scenario("witness nonce was not observed"));
    }
    writeln!(receipt, "host")?;
    receipt.sync_all()?;
    let code="assert(UI.IsInFrontEnd() and GameConfiguration.IsHotseat(),'hotseat frontend required')\nNetwork.HostGame(ServerType.SERVER_TYPE_HOTSEAT)\nreturn true";
    runner
        .send_frontend_transition_checked(code, io.now() + 15., || verify_owner(&mut io, &state))?;
    runner.disconnect();
    println!("HostGame sent once; fixed staging witness handles the official Ready button");
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct State {
    journal: protocol::Journal,
    baseline: serde_json::Value,
    processed: usize,
    prefix_hash: String,
    #[serde(default)]
    load_baselines: std::collections::BTreeMap<u8, serde_json::Value>,
}

fn append_state(path: &Path, state: &State) -> Result<()> {
    let mut file = OpenOptions::new().append(true).open(path)?;
    serde_json::to_writer(&mut file, state)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

fn state(path: &Path) -> Result<State> {
    let bytes = fs::read(path)?;
    if !bytes.ends_with(b"\n") {
        return Err(Error::scenario(
            "incomplete controller journal; do not replay",
        ));
    }
    let line = bytes
        .split(|b| *b == b'\n')
        .rev()
        .find(|row| !row.is_empty())
        .ok_or_else(|| Error::scenario("empty controller journal"))?;
    Ok(serde_json::from_slice(line)?)
}

fn digest(frames: &[protocol::Frame]) -> Result<String> {
    use sha2::{Digest, Sha256};
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(frames)?)))
}

fn verify_owner(io: &mut SystemTransport<'_>, state: &State) -> Result<()> {
    verify_lease()?;
    if io.owner()? != state.journal.owner {
        return Err(Error::input("Civ6 PID/start/window changed"));
    }
    Ok(())
}

fn verify_lease() -> Result<()> {
    let expected = std::env::var("CODEX_JOB_ID")
        .map_err(|_| Error::input("CODEX_JOB_ID is required for the shared live lease"))?;
    let path = std::env::var_os("CIV6_LIVE_LOCK_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("D:/CC/Tools/civ6-live/civ6.lock"));
    let value: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    if value["job"].as_str() != Some(expected.as_str()) || value["live_chain"] != true {
        return Err(Error::input("shared Civ6 lease is not owned by this job"));
    }
    Ok(())
}

fn completed_lines(text: &str) -> &str {
    text.rfind('\n').map_or("", |end| &text[..=end])
}

fn drive(output: &Path, logs: &Path, saves: &Path, timeout: f64, port: u16) -> Result<()> {
    if !timeout.is_finite() || !(1.0..=1200.).contains(&timeout) {
        return Err(Error::input("drive timeout must be 1..1200 seconds"));
    }
    let path = output.join("controller.jsonl");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(output.join("controller.lock"))?;
    lock.try_lock()
        .map_err(|_| Error::scenario("native carrier controller is already active"))?;
    let mut state = state(&path)?;
    if state.journal.complete {
        return Err(Error::scenario(
            "completed controller will not execute again",
        ));
    }
    let mut io = SystemTransport { logs };
    verify_owner(&mut io, &state)?;
    let deadline = io.now() + timeout;
    loop {
        verify_owner(&mut io, &state)?;
        if io.now() >= deadline {
            return Err(Error::scenario(
                "native carrier fixture incomplete at deadline",
            ));
        }
        let fresh = judge::read_since(logs, &state.baseline)?;
        let failed: Vec<_> = judge::judge(&fresh, "generic", None, true)
            .into_iter()
            .filter(|c| !c.passed)
            .map(|c| c.name)
            .collect();
        if !failed.is_empty() {
            return Err(Error::scenario(format!(
                "native runtime log failed: {failed:?}"
            )));
        }
        let frames = protocol::parse_available_frames(
            completed_lines(fresh.get("Lua.log").map(String::as_str).unwrap_or("")),
            &state.journal.nonce,
        )?;
        if frames.len() < state.processed
            || digest(&frames[..state.processed])? != state.prefix_hash
        {
            return Err(Error::scenario("consumed log frames changed"));
        }
        for frame in &frames[state.processed..] {
            let action = state.journal.consume(frame)?;
            state.processed += 1;
            state.prefix_hash = digest(&frames[..state.processed])?;
            // Write before any key or load: unknown outcomes retain sent markers.
            append_state(&path, &state)?;
            match action {
                Some(protocol::Action::Begin { .. }) => {
                    let baseline = state
                        .load_baselines
                        .get(&frame.generation)
                        .unwrap_or(&state.baseline);
                    loop {
                        verify_owner(&mut io, &state)?;
                        let loading = judge::read_since(logs, baseline)?;
                        if loading.get("Lua.log").is_some_and(|text| {
                            text.contains(
                                civ6_autotest_rs::native_carrier_witness::ENGINE_LOAD_READY,
                            )
                        }) {
                            break;
                        }
                        if io.now() >= deadline {
                            return Err(Error::scenario(
                                "early hook is armed but engine load completion is unproved",
                            ));
                        }
                        io.sleep(0.2);
                    }
                    // Stock LoadScreen.OnLoadGameViewStateDone automatically
                    // calls OnActivateButtonClicked for AnyMultiplayer. A key
                    // here could instead dismiss PlayerChange or end a turn.
                }
                Some(protocol::Action::AcceptDialog { .. }) => {
                    verify_owner(&mut io, &state)?;
                    io.key(0x0D, deadline)?;
                }
                Some(protocol::Action::Load { generation }) => {
                    let name = protocol::save_name(&state.journal.nonce, generation)?;
                    let save = saves.join(format!("{name}.Civ6Save"));
                    while !save.is_file() || fs::metadata(&save)?.len() == 0 {
                        verify_owner(&mut io, &state)?;
                        if io.now() >= deadline {
                            return Err(Error::scenario(
                                "SaveComplete lacks a new nonempty physical file",
                            ));
                        }
                        io.sleep(0.2);
                    }
                    let mut runner = Runner::new(
                        serde_json::json!({"session_type":"hotseat"}),
                        output.to_owned(),
                        logs.to_owned(),
                        saves.to_owned(),
                        port,
                    );
                    let menu_deadline = deadline.min(io.now() + 30.);
                    let ready = runner.send_frontend_transition_checked(
                        "return UI.IsInFrontEnd()",
                        menu_deadline,
                        || verify_owner(&mut io, &state),
                    )?;
                    if ready.trim() != "true" {
                        return Err(Error::scenario("frontend is not confirmed; no LoadGame"));
                    }
                    let code=format!("if not UI.IsInFrontEnd() then error('frontend required') end\nNetwork.LeaveGame()\n{}",scenario::load_code(&name,scenario::SessionType::Hotseat));
                    state
                        .load_baselines
                        .insert(generation, judge::snapshot_logs(logs)?);
                    append_state(&path, &state)?;
                    runner.send_frontend_transition_checked(
                        &code,
                        deadline.min(io.now() + 15.),
                        || verify_owner(&mut io, &state),
                    )?;
                    runner.disconnect();
                }
                Some(protocol::Action::Complete) => {
                    println!("native carrier exact saved reload complete");
                    return Ok(());
                }
                None => {}
            }
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn run(args: Args) -> Result<()> {
    match args.command {
        Command::Prepare { output, repo } => {
            if fs::read_dir(&output)?.next().is_some() {
                return Err(Error::scenario(
                    "prepared output must be an empty owned directory",
                ));
            }
            let mut bindings = std::collections::BTreeMap::new();
            for relative in BINDINGS {
                bindings.insert(relative.to_owned(), file_hash(&repo.join(relative))?);
            }
            let value = Bundle {
                version: 1,
                nonce: uuid::Uuid::new_v4().simple().to_string(),
                bindings,
            };
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output.join("bundle.json"))?;
            serde_json::to_writer_pretty(&mut file, &value)?;
            file.sync_all()?;
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        Command::Pregame {
            output,
            repo,
            logs,
            saves,
            port,
        } => pregame(&output, &repo, &logs, &saves, port)?,
        Command::Audit { input, nonce } => {
            let frames = protocol::parse_available_frames(&fs::read_to_string(input)?, &nonce)?;
            let mut receipts = protocol::Receipts::default();
            for frame in &frames {
                frame.payload()?;
                receipts.accept(frame)?;
            }
            println!("{}", serde_json::to_string_pretty(&frames)?);
        }
        Command::Init {
            output,
            nonce,
            logs,
            saves,
        } => {
            verify_lease()?;
            for generation in [1, 2] {
                let name = protocol::save_name(&nonce, generation)?;
                if saves.join(format!("{name}.Civ6Save")).exists() {
                    return Err(Error::scenario(
                        "unique native-carrier save exists; choose a new run, do not overwrite",
                    ));
                }
            }
            let mut io = SystemTransport { logs: &logs };
            let owner = io.owner()?;
            let journal = protocol::Journal::new(&nonce, owner)?;
            let path = output.join("controller.jsonl");
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            let value = State {
                journal,
                baseline: judge::snapshot_logs(&logs)?,
                processed: 0,
                prefix_hash: digest(&[])?,
                load_baselines: std::collections::BTreeMap::new(),
            };
            serde_json::to_writer(&mut file, &value)?;
            file.write_all(b"\n")?;
            file.sync_all()?;
            println!("controller initialized; no operation issued");
        }
        Command::Drive {
            output,
            logs,
            saves,
            timeout,
            port,
        } => drive(&output, &logs, &saves, timeout, port)?,
    }
    Ok(())
}

fn main() {
    if let Err(error) = run(Args::parse()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const NONCE: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn incomplete_final_journal_record_never_restores_an_older_unsent_state() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("controller.jsonl");
        let value = State {
            journal: protocol::Journal::new(
                NONCE,
                civ6_autotest_rs::hotseat_bridge::WindowOwner {
                    pid: 1,
                    started: 1.,
                    hwnd: 1,
                },
            )
            .unwrap(),
            baseline: serde_json::json!({}),
            processed: 0,
            prefix_hash: digest(&[]).unwrap(),
            load_baselines: std::collections::BTreeMap::new(),
        };
        fs::write(
            &path,
            format!("{}\n", serde_json::to_string(&value).unwrap()),
        )
        .unwrap();
        assert_eq!(state(&path).unwrap().processed, 0);
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"{\"processed\":1")
            .unwrap();
        assert!(state(&path).is_err());
    }

    #[test]
    fn written_input_receipt_survives_restart_and_cannot_be_reissued() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("controller.jsonl");
        let mut value = State {
            journal: protocol::Journal::new(
                NONCE,
                civ6_autotest_rs::hotseat_bridge::WindowOwner {
                    pid: 1,
                    started: 1.,
                    hwnd: 1,
                },
            )
            .unwrap(),
            baseline: serde_json::json!({}),
            processed: 0,
            prefix_hash: digest(&[]).unwrap(),
            load_baselines: std::collections::BTreeMap::new(),
        };
        fs::write(&path, b"").unwrap();
        let frame = |serial, kind| protocol::Frame {
            nonce: NONCE.into(),
            scene: "scene0".into(),
            serial,
            generation: 0,
            step: 0,
            kind,
            body: serde_json::json!({"generation":0,"canonical":"actual"}).to_string(),
        };
        value.journal.consume(&frame(1, "BOOT".into())).unwrap();
        let begin = frame(2, "DIALOG_OPEN".into());
        assert!(matches!(
            value.journal.consume(&begin).unwrap(),
            Some(protocol::Action::AcceptDialog { .. })
        ));
        append_state(&path, &value).unwrap();
        let mut restored = state(&path).unwrap();
        assert_eq!(restored.journal.consume(&begin).unwrap(), None);
    }

    #[test]
    fn unfinished_log_line_is_not_decoded_as_a_malformed_finished_frame() {
        assert_eq!(completed_lines("noise\nJAC_NC|1|partial"), "noise\n");
        assert_eq!(completed_lines("JAC_NC|1|partial"), "");
    }
}
