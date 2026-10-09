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

#[derive(serde::Serialize, serde::Deserialize)]
struct State {
    journal: protocol::Journal,
    baseline: serde_json::Value,
    processed: usize,
    prefix_hash: String,
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
                Some(protocol::Action::Begin { .. })
                | Some(protocol::Action::AcceptDialog { .. }) => {
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
        let begin = frame(2, "LOAD_READY".into());
        assert!(matches!(
            value.journal.consume(&begin).unwrap(),
            Some(protocol::Action::Begin { .. })
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
