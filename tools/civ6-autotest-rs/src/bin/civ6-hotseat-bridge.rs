//! Explicitly bounded hotseat test transport. Never starts a game or Steam.
use civ6_autotest_rs::{hotseat_bridge as hs, judge, runner::Runner, scenario, Error, Result};
use clap::{Parser, Subcommand};
use hs::TransportIo;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Parser)]
#[command(about = "Prepare and drive the test-only Civ6 hotseat log bridge")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Prepare {
        scenario: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        input_diagnostic: bool,
    },
    Pregame {
        bundle: PathBuf,
        #[arg(long)]
        logs: PathBuf,
        #[arg(long)]
        saves: PathBuf,
        #[arg(long)]
        mods: PathBuf,
        #[arg(long, default_value_t = 4318)]
        port: u16,
    },
    Init {
        bundle: PathBuf,
        #[arg(long)]
        logs: PathBuf,
    },
    ProbeInput {
        bundle: PathBuf,
        #[arg(long)]
        logs: PathBuf,
    },
    Step {
        bundle: PathBuf,
        #[arg(long)]
        logs: PathBuf,
        #[arg(long)]
        saves: PathBuf,
        #[arg(long)]
        number: usize,
        #[arg(long, default_value_t = 120.0)]
        timeout: f64,
    },
    MenuLoad {
        bundle: PathBuf,
        #[arg(long)]
        logs: PathBuf,
        #[arg(long)]
        saves: PathBuf,
        #[arg(long, default_value_t = 4318)]
        port: u16,
    },
    AdoptLoad {
        bundle: PathBuf,
        #[arg(long)]
        logs: PathBuf,
    },
}

fn pregame(bundle_path: &Path, logs: &Path, saves: &Path, mods: &Path, port: u16) -> Result<()> {
    let (bundle, scenario, journal_path) = bundle_data(bundle_path)?;
    hs::verify_installed(&bundle, &scenario, mods)?;
    let sent_path = journal_path.with_file_name("pregame-sent.txt");
    if sent_path.exists() {
        return Err(Error::scenario(
            "pregame already attempted; no command will be resent",
        ));
    }
    let mut owner_check = hs::SystemTransport { logs };
    let owner = owner_check.owner()?;
    let audit_baseline = judge::snapshot_logs(logs)?;
    fs::write(
        journal_path.with_file_name("pregame-log-baseline.json"),
        serde_json::to_vec_pretty(&audit_baseline)?,
    )?;
    let mut runner = Runner::new(
        scenario.clone(),
        journal_path.parent().unwrap().to_path_buf(),
        logs.to_path_buf(),
        saves.to_path_buf(),
        port,
    );
    let deadline = runner.clock.now() + 30.0;
    let ready =
        runner.send_frontend_transition_checked("return UI.IsInFrontEnd()", deadline, || {
            verify_owner(&mut owner_check, &owner)
        })?;
    if ready.trim() != "true" {
        return Err(Error::scenario("Civ6 main menu is not confirmed"));
    }
    let mut sent = OpenOptions::new()
        .create_new(true)
        .append(true)
        .open(&sent_path)?;
    for index in 0..2 {
        if owner_check.owner()? != owner {
            return Err(Error::input(
                "Civ6 process or window changed during pregame",
            ));
        }
        writeln!(sent, "sent step {}", index + 1)?;
        sent.sync_all()?;
        runner.run_frontend_setup_step_checked(&scenario["steps"][index], || {
            verify_owner(&mut owner_check, &owner)
        })?;
    }
    if owner_check.owner()? != owner {
        return Err(Error::input(
            "Civ6 process or window changed before HostGame",
        ));
    }
    writeln!(sent, "sent HostGame")?;
    sent.sync_all()?;
    let code = "if not UI.IsInFrontEnd() or not GameConfiguration.IsHotseat() then error('hotseat main menu required') end\nNetwork.HostGame(ServerType.SERVER_TYPE_HOTSEAT)\nreturn true";
    let deadline = runner.clock.now() + 10.0;
    let result = runner.send_frontend_transition_checked(code, deadline, || {
        verify_owner(&mut owner_check, &owner)
    });
    runner.disconnect();
    result?;
    validate_runtime_logs(logs, &audit_baseline)?;
    println!(
        "HostGame sent once. Verify StagingRoom ReadyButton geometry before one click; then wait for the nonce-matched InGame BOOT and DIALOG_OPEN."
    );
    Ok(())
}

fn verify_owner(io: &mut hs::SystemTransport<'_>, owner: &hs::WindowOwner) -> Result<()> {
    if &io.owner()? != owner {
        return Err(Error::input("Civ6 process or window ownership changed"));
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Journal {
    nonce: String,
    owner: hs::WindowOwner,
    scene: String,
    boot_count: usize,
    next: usize,
    sent: BTreeSet<usize>,
    return_sent: BTreeSet<usize>,
    accepted_turns: BTreeSet<usize>,
    load_sent: BTreeSet<usize>,
    captures: BTreeMap<String, (String, hs::WitnessSnapshot)>,
    saved: BTreeMap<usize, hs::WitnessSnapshot>,
    post_exit_baseline: Option<Value>,
    audit_baseline: Value,
}

fn validate_runtime_logs(logs: &Path, baseline: &Value) -> Result<()> {
    for name in judge::LOG_NAMES {
        let marker = &baseline["files"][name];
        if !marker.is_object() {
            return Err(Error::scenario(format!("missing {name} audit baseline")));
        }
        let (bytes, inode) = judge::read_log_file(&logs.join(name))?;
        if judge::log_marker_replaced(&bytes, marker, inode) {
            return Err(Error::scenario(format!(
                "{name} generation changed during hotseat audit"
            )));
        }
    }
    let fresh = judge::read_since(logs, baseline)?;
    let failed: Vec<_> = judge::judge(&fresh, "generic", None, true)
        .into_iter()
        .filter(|check| !check.passed)
        .map(|check| format!("{}: {}", check.name, check.detail))
        .collect();
    if !failed.is_empty() {
        return Err(Error::scenario(format!(
            "hotseat runtime log checks failed: {}",
            failed.join("; ")
        )));
    }
    Ok(())
}

fn bundle_data(path: &Path) -> Result<(hs::Bundle, Value, PathBuf)> {
    let bundle: hs::Bundle = serde_json::from_slice(&fs::read(path)?)?;
    if !hs::nonce_valid(&bundle.nonce) {
        return Err(Error::scenario("invalid bundle nonce"));
    }
    let scenario: Value = serde_json::from_slice(&fs::read(&bundle.scenario)?)?;
    hs::check_scenario(&scenario)?;
    if scenario["hotseat_bridge_nonce"] != bundle.nonce {
        return Err(Error::scenario("bundle and scenario nonce differ"));
    }
    if scenario["steps"][0]["ids"]
        .as_array()
        .and_then(|ids| ids.last())
        .and_then(Value::as_str)
        != Some(bundle.mod_id.as_str())
    {
        return Err(Error::scenario("bundle and scenario MOD ID differ"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::scenario("bundle has no directory"))?;
    Ok((bundle, scenario, parent.join("bridge-journal.jsonl")))
}

fn append_journal(path: &Path, state: &Journal) -> Result<()> {
    let mut file = OpenOptions::new().append(true).create(true).open(path)?;
    serde_json::to_writer(&mut file, state)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

fn nonempty_file(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
}

fn journal(path: &Path) -> Result<Journal> {
    let bytes = fs::read_to_string(path)?;
    let mut last = None;
    for line in bytes.lines() {
        last = Some(serde_json::from_str(line)?);
    }
    last.ok_or_else(|| Error::scenario("empty hotseat journal"))
}

fn current_frames(logs: &Path, run_id: &str) -> Result<(Vec<hs::Frame>, usize)> {
    let bytes = fs::read(logs.join("Lua.log"))?;
    let raw = String::from_utf8_lossy(&bytes);
    let needle = format!("JAC_HS|1|{run_id}|");
    let matching = raw
        .split_inclusive('\n')
        .filter(|line| line.contains(&needle))
        .collect::<Vec<_>>();
    let boots = matching
        .iter()
        .filter(|line| line.contains("|BOOT|1/"))
        .count();
    let start = matching
        .iter()
        .rposition(|line| line.contains("|BOOT|1/"))
        .ok_or_else(|| Error::scenario("nonce-matched BOOT is missing"))?;
    Ok((
        hs::parse_available_frames(&matching[start..].concat(), run_id)?,
        boots,
    ))
}

fn last_boot(frames: &[hs::Frame]) -> Result<&hs::Frame> {
    frames
        .iter()
        .rev()
        .find(|f| f.kind == "BOOT" && f.step == 0)
        .ok_or_else(|| Error::scenario("nonce-matched InGame BOOT is missing"))
}

fn last_open<'a>(frames: &'a [hs::Frame], scene: &str) -> Result<&'a hs::Frame> {
    let latest = frames
        .iter()
        .filter(|f| {
            f.scene == scene
                && (f.kind == "DIALOG_OPEN" || f.kind == "DIALOG_WAIT" || f.kind == "DIALOG_CLOSED")
        })
        .last();
    latest
        .filter(|f| f.kind == "DIALOG_OPEN")
        .ok_or_else(|| Error::scenario("actual visible PlayerChange dialog was not witnessed"))
}

fn init(bundle_path: &Path, logs: &Path) -> Result<()> {
    let (bundle, _, path) = bundle_data(bundle_path)?;
    if path.exists() {
        return Err(Error::scenario(
            "hotseat journal already exists; input will not be repeated",
        ));
    }
    let audit_baseline: Value =
        serde_json::from_slice(&fs::read(path.with_file_name("pregame-log-baseline.json"))?)?;
    validate_runtime_logs(logs, &audit_baseline)?;
    let (frames, boot_count) = current_frames(logs, &bundle.nonce)?;
    if boot_count != 1 {
        return Err(Error::scenario("initial hotseat scene is not unique"));
    }
    let boot = last_boot(&frames)?;
    let opened = hs::WitnessSnapshot::parse(&last_open(&frames, &boot.scene)?.body)?;
    let io = hs::SystemTransport { logs };
    let mut controller = hs::Controller::new(io, &bundle.nonce, boot)?;
    let mut state = Journal {
        nonce: bundle.nonce,
        owner: controller.owner,
        scene: boot.scene.clone(),
        boot_count,
        next: 4,
        sent: BTreeSet::new(),
        return_sent: BTreeSet::new(),
        accepted_turns: BTreeSet::new(),
        load_sent: BTreeSet::new(),
        captures: BTreeMap::new(),
        saved: BTreeMap::new(),
        post_exit_baseline: None,
        audit_baseline,
    };
    append_journal(&path, &state)?;
    let deadline = controller.io.now() + 120.0;
    controller.accept_turn(3, &opened, deadline, |n| {
        state.return_sent.insert(n);
        append_journal(&path, &state)
    })?;
    state.accepted_turns.insert(3);
    append_journal(&path, &state)?;
    validate_runtime_logs(logs, &state.audit_baseline)?;
    if bundle.input_diagnostic {
        println!(
            "initial PlayerChange accepted once; next command is probe-input; acceptance steps are disabled"
        );
    } else {
        println!("initial PlayerChange accepted once; next step 4");
    }
    Ok(())
}

fn step(bundle_path: &Path, logs: &Path, saves: &Path, number: usize, timeout: f64) -> Result<()> {
    if !timeout.is_finite() || !(0.1..=600.0).contains(&timeout) {
        return Err(Error::scenario("timeout must be 0.1..600 seconds"));
    }
    let (bundle, scenario, path) = bundle_data(bundle_path)?;
    require_acceptance_bundle(&bundle)?;
    let mut state = journal(&path)?;
    validate_runtime_logs(logs, &state.audit_baseline)?;
    if state.nonce != bundle.nonce || state.next != number {
        return Err(Error::scenario(format!(
            "hotseat step out of order: expected {}, got {number}",
            state.next
        )));
    }
    let required_turn = match number {
        4 | 5 | 6 => 3,
        7 | 8 | 9 | 10 => 6,
        12 | 13 => 11,
        14 | 15 | 16 => 13,
        17 | 18 | 19 => 16,
        21 => 20,
        _ => return Err(Error::scenario("unsupported hotseat command")),
    };
    if !state.accepted_turns.contains(&required_turn) {
        return Err(Error::scenario(
            "previous PlayerChange Return is not verified",
        ));
    }
    let (frames, boot_count) = current_frames(logs, &bundle.nonce)?;
    if boot_count != state.boot_count {
        return Err(Error::scenario("unadopted load scene generation"));
    }
    let boot = last_boot(&frames)?;
    if boot.scene != state.scene {
        return Err(Error::scenario("unadopted load scene"));
    }
    let io = hs::SystemTransport { logs };
    let mut controller = hs::Controller::new(io, &bundle.nonce, boot)?;
    if controller.owner != state.owner {
        return Err(Error::input("PID reuse or window ownership loss"));
    }
    controller.sent = state.sent.clone();
    controller.return_sent = state.return_sent.clone();
    controller.captures = state.captures.clone();
    let name = if number == 9 || number == 18 {
        Some(format!(
            "{}-{}",
            scenario["steps"][number - 1]["name"].as_str().unwrap(),
            bundle.nonce
        ))
    } else {
        None
    };
    let save_path = name
        .as_ref()
        .map(|name| saves.join(format!("{name}.Civ6Save")));
    if save_path.as_ref().is_some_and(|p| p.exists()) {
        return Err(Error::scenario(
            "unique hotseat save already exists; refusing overwrite",
        ));
    }
    let deadline = controller.io.now() + timeout;
    let receipt = controller.step(number, &scenario, deadline, |n| {
        state.sent.insert(n);
        append_journal(&path, &state)
    })?;
    if let Some(target) = save_path {
        if receipt.kind != "SAVE_COMPLETE"
            || receipt.response_body != name.as_deref().unwrap()
            || receipt.before.dialog
            || receipt.before.paused
        {
            return Err(Error::scenario(
                "save did not complete in an active visible turn",
            ));
        }
        let until = std::time::Instant::now() + Duration::from_secs(5);
        while std::time::Instant::now() < until && !nonempty_file(&target) {
            std::thread::sleep(Duration::from_millis(100));
        }
        if !nonempty_file(&target) {
            return Err(Error::scenario(format!(
                "SaveComplete had no fresh nonempty file at {}",
                target.display()
            )));
        }
        state.saved.insert(number, receipt.before.clone());
    }
    if number == 6 || number == 13 || number == 16 {
        let opened = receipt
            .dialog_open
            .as_ref()
            .ok_or_else(|| Error::scenario("missing PlayerChange witness"))?;
        let expected = match number {
            6 => (receipt.before.turn, 1),
            13 => (receipt.before.turn + 1, 0),
            _ => (receipt.before.turn, 1),
        };
        if (receipt.before.turn, receipt.before.local)
            != match number {
                6 => (receipt.before.turn, 0),
                13 => (receipt.before.turn, 1),
                _ => (receipt.before.turn, 0),
            }
            || (opened.turn, opened.local) != expected
        {
            return Err(Error::scenario("hotseat handoff skipped a player or round"));
        }
        controller.accept_turn(number, opened, deadline, |n| {
            state.return_sent.insert(n);
            append_journal(&path, &state)
        })?;
        state.accepted_turns.insert(number);
        append_journal(&path, &state)?;
    }
    state.captures = controller.captures;
    if number == 10 || number == 19 {
        state.post_exit_baseline = Some(judge::snapshot_logs(logs)?);
        state.next = number + 1; // load is deliberately external and unproven.
        append_journal(&path, &state)?;
        println!("exit requested once; menu return and load still need separate evidence");
    } else {
        state.next = number + 1;
        append_journal(&path, &state)?;
        println!("step {number} witnessed; next step {}", state.next);
    }
    validate_runtime_logs(logs, &state.audit_baseline)?;
    Ok(())
}

fn adopt_load(bundle_path: &Path, logs: &Path) -> Result<()> {
    let (bundle, _, path) = bundle_data(bundle_path)?;
    require_acceptance_bundle(&bundle)?;
    let mut state = journal(&path)?;
    let (load_step, save_step, next) = match state.next {
        11 => (11, 9, 12),
        20 => (20, 18, 21),
        _ => return Err(Error::scenario("no pending hotseat load")),
    };
    if !state.load_sent.contains(&load_step) {
        return Err(Error::scenario(
            "official menu LoadGame was not sent by this run",
        ));
    }
    let baseline = state
        .post_exit_baseline
        .as_ref()
        .cloned()
        .ok_or_else(|| Error::scenario("missing pre-load log baseline"))?;
    validate_runtime_logs(logs, &baseline)?;
    let frames = hs::fresh_frames(logs, &baseline, &bundle.nonce)?;
    let boot = last_boot(&frames)?;
    let saved = state
        .saved
        .get(&save_step)
        .ok_or_else(|| Error::scenario("missing save witness"))?;
    let (_, boot_count) = current_frames(logs, &bundle.nonce)?;
    hs::validate_load_epoch(saved, boot, &bundle.nonce, state.boot_count, boot_count)?;
    let opened = hs::WitnessSnapshot::parse(&last_open(&frames, &boot.scene)?.body)?;
    let io = hs::SystemTransport { logs };
    let mut controller = hs::Controller::new(io, &bundle.nonce, boot)?;
    if controller.owner != state.owner {
        return Err(Error::input(
            "PID reuse or window ownership loss after load",
        ));
    }
    state.scene = boot.scene.clone();
    state.boot_count = boot_count;
    state.next = next;
    state.post_exit_baseline = None;
    state.audit_baseline = baseline.clone();
    append_journal(&path, &state)?;
    let deadline = controller.io.now() + 120.0;
    controller.accept_turn(load_step, &opened, deadline, |n| {
        state.return_sent.insert(n);
        append_journal(&path, &state)
    })?;
    state.accepted_turns.insert(load_step);
    append_journal(&path, &state)?;
    validate_runtime_logs(logs, &state.audit_baseline)?;
    println!("fresh load scene and full four-unit equality witnessed; next step {next}");
    Ok(())
}

fn menu_load(bundle_path: &Path, logs: &Path, saves: &Path, port: u16) -> Result<()> {
    let (bundle, scenario_data, path) = bundle_data(bundle_path)?;
    require_acceptance_bundle(&bundle)?;
    let mut state = journal(&path)?;
    validate_runtime_logs(logs, &state.audit_baseline)?;
    let number = state.next;
    if number != 11 && number != 20 {
        return Err(Error::scenario("no pending menu load"));
    }
    if state.load_sent.contains(&number) {
        return Err(Error::scenario(
            "LoadGame was already sent; outcome may be unknown",
        ));
    }
    let name = format!(
        "{}-{}",
        scenario_data["steps"][number - 1]["name"].as_str().unwrap(),
        bundle.nonce
    );
    let target = saves.join(format!("{name}.Civ6Save"));
    if !nonempty_file(&target) {
        return Err(Error::scenario("hotseat save file is absent or empty"));
    }
    let mut owner_check = hs::SystemTransport { logs };
    if owner_check.owner()? != state.owner {
        return Err(Error::input("PID reuse or window ownership loss at menu"));
    }
    let mut runner = Runner::new(
        scenario_data,
        path.parent().unwrap().to_path_buf(),
        logs.to_path_buf(),
        saves.to_path_buf(),
        port,
    );
    let deadline = runner.clock.now() + 30.0;
    let ready =
        runner.send_frontend_transition_checked("return UI.IsInFrontEnd()", deadline, || {
            verify_owner(&mut owner_check, &state.owner)
        })?;
    if ready.trim() != "true" {
        return Err(Error::scenario("Civ6 main menu is not confirmed"));
    }
    if owner_check.owner()? != state.owner {
        return Err(Error::input(
            "PID reuse or window ownership loss before LoadGame",
        ));
    }
    state.load_sent.insert(number);
    append_journal(&path, &state)?;
    let code = format!(
        "if not UI.IsInFrontEnd() then error('main menu required') end\nNetwork.LeaveGame()\n{}",
        scenario::load_code(&name, scenario::SessionType::Hotseat)
    );
    let deadline = runner.clock.now() + 15.0;
    let result = runner.send_frontend_transition_checked(&code, deadline, || {
        verify_owner(&mut owner_check, &state.owner)
    });
    runner.disconnect();
    result?;
    println!(
        "official LeaveGame and LoadGame sent once. Verify any StagingRoom Ready geometry and wait for a fresh InGame BOOT."
    );
    Ok(())
}

fn lock_bundle(path: &Path) -> Result<std::fs::File> {
    // Windows byte-range locks also reject reads through another handle in
    // this process. Keep the lock separate from the bundle we must deserialize.
    let lock_path = fs::canonicalize(path)?.with_extension("lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;
    file.try_lock()
        .map_err(|_| Error::scenario("hotseat bundle is already in use"))?;
    Ok(file)
}

fn require_acceptance_bundle(bundle: &hs::Bundle) -> Result<()> {
    if bundle.input_diagnostic {
        return Err(Error::scenario(
            "input-only bundle cannot run acceptance steps",
        ));
    }
    Ok(())
}

fn probe_input(bundle_path: &Path, logs: &Path) -> Result<()> {
    let (bundle, _, path) = bundle_data(bundle_path)?;
    if !bundle.input_diagnostic {
        return Err(Error::scenario("input-only diagnostic bundle required"));
    }
    let state = journal(&path)?;
    if state.nonce != bundle.nonce || state.next != 4 || !state.accepted_turns.contains(&3) {
        return Err(Error::scenario(
            "initial human handoff must finish before the input probe",
        ));
    }
    validate_runtime_logs(logs, &state.audit_baseline)?;
    let (frames, _) = current_frames(logs, &bundle.nonce)?;
    let boot = last_boot(&frames)?;
    let mut controller = hs::Controller::new(hs::SystemTransport { logs }, &bundle.nonce, boot)?;
    if controller.owner != state.owner || boot.scene != state.scene {
        return Err(Error::input("input probe owner or scene changed"));
    }
    let baseline = controller.io.baseline()?;
    let mut sent = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path.with_file_name("input-probe-sent.json"))?;
    sent.write_all(&serde_json::to_vec(&serde_json::json!({
        "nonce": bundle.nonce, "owner": state.owner, "key": 124
    }))?)?;
    sent.sync_all()?;
    let deadline = controller.io.now() + 30.0;
    verify_owner(&mut controller.io, &state.owner)?;
    controller.io.key(124, deadline)?;
    while controller.io.now() < deadline {
        verify_owner(&mut controller.io, &state.owner)?;
        let fresh = controller.io.frames(&baseline, &bundle.nonce)?;
        for frame in fresh {
            if frame.scene != state.scene {
                return Err(Error::scenario("input probe scene changed"));
            }
            if frame.kind == "INPUT_PROBE"
                && frame.step == 0
                && frame
                    .body
                    .strip_prefix("key_up|F13|")
                    .and_then(|key| key.parse::<u16>().ok())
                    .is_some()
            {
                validate_runtime_logs(logs, &state.audit_baseline)?;
                println!(
                    "F13 KeyUp reached the input-only witness. No original scenario step was executed."
                );
                return Ok(());
            }
        }
        controller.io.sleep(0.1);
    }
    Err(Error::scenario(
        "input probe outcome unknown; input will not be resent",
    ))
}

fn run(args: Args) -> Result<()> {
    // Hold one OS file lock across each state transition. The journal alone
    // cannot prevent two commands from reading the same `next` concurrently.
    let _bundle_lock = match &args.command {
        Command::Prepare { .. } => None,
        Command::Pregame { bundle, .. }
        | Command::Init { bundle, .. }
        | Command::ProbeInput { bundle, .. }
        | Command::Step { bundle, .. }
        | Command::MenuLoad { bundle, .. }
        | Command::AdoptLoad { bundle, .. } => Some(lock_bundle(bundle)?),
    };
    match args.command {
        Command::Prepare {
            scenario,
            output,
            input_diagnostic,
        } => {
            let data: Value = serde_json::from_slice(&fs::read(scenario)?)?;
            let bundle = hs::prepare_mode(
                &data,
                &output,
                &uuid::Uuid::new_v4().simple().to_string(),
                &uuid::Uuid::new_v4().to_string(),
                input_diagnostic,
            )?;
            println!("{}", serde_json::to_string_pretty(&bundle)?);
            Ok(())
        }
        Command::Pregame {
            bundle,
            logs,
            saves,
            mods,
            port,
        } => pregame(&bundle, &logs, &saves, &mods, port),
        Command::Init { bundle, logs } => init(&bundle, &logs),
        Command::ProbeInput { bundle, logs } => probe_input(&bundle, &logs),
        Command::Step {
            bundle,
            logs,
            saves,
            number,
            timeout,
        } => step(&bundle, &logs, &saves, number, timeout),
        Command::MenuLoad {
            bundle,
            logs,
            saves,
            port,
        } => menu_load(&bundle, &logs, &saves, port),
        Command::AdoptLoad { bundle, logs } => adopt_load(&bundle, &logs),
    }
}

fn main() -> std::process::ExitCode {
    match run(Args::parse()) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("hotseat bridge: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_only_bundles_reject_all_acceptance_commands_before_game_access() {
        let dir = tempfile::tempdir().unwrap();
        let data: Value = serde_json::from_str(include_str!(
            "../../../civ6-autotest/scenarios/multiplayer-hotseat-growth.json"
        ))
        .unwrap();
        hs::prepare_mode(
            &data,
            dir.path(),
            &uuid::Uuid::new_v4().simple().to_string(),
            &uuid::Uuid::new_v4().to_string(),
            true,
        )
        .unwrap();
        let bundle = dir.path().join("bundle.json");
        let absent_logs = dir.path().join("no-game-logs");
        let absent_saves = dir.path().join("no-game-saves");
        for result in [
            step(&bundle, &absent_logs, &absent_saves, 4, 1.0),
            menu_load(&bundle, &absent_logs, &absent_saves, 4318),
            adopt_load(&bundle, &absent_logs),
        ] {
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("input-only bundle cannot run acceptance steps"));
        }
        assert!(!dir.path().join("bridge-journal.jsonl").exists());
        assert!(!dir.path().join("input-probe-sent.json").exists());
    }

    #[test]
    fn bundle_is_readable_while_other_commands_are_excluded() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("bundle.json");
        fs::write(&bundle, b"{\"nonce\":\"example\"}").unwrap();
        let first = lock_bundle(&bundle).unwrap();
        let parsed: Value = serde_json::from_slice(&fs::read(&bundle).unwrap()).unwrap();
        assert_eq!(parsed["nonce"], "example");
        assert!(lock_bundle(&bundle).is_err());
        // Aliases to the same bundle must use the same canonical lock.
        assert!(lock_bundle(&dir.path().join("./bundle.json")).is_err());
        drop(first);
        assert!(lock_bundle(&bundle).is_ok());
    }

    #[test]
    fn runtime_audit_rejects_unframed_lua_errors() {
        let dir = tempfile::tempdir().unwrap();
        for name in judge::LOG_NAMES {
            fs::write(dir.path().join(name), b"baseline\n").unwrap();
        }
        let baseline = judge::snapshot_logs(dir.path()).unwrap();
        validate_runtime_logs(dir.path(), &baseline).unwrap();
        fs::write(
            dir.path().join("Lua.log"),
            b"baseline\nRuntime Error: original scenario failed\n",
        )
        .unwrap();
        assert!(validate_runtime_logs(dir.path(), &baseline).is_err());
    }
}
