//! Compose the existing controller's exact handles, lease, OwnedJob and remote
//! call implementation. There is deliberately no CreateRemoteThread here.
use super::*;
use civ6_bootstrap_rs::{protocol as wire, Prepare};
use std::os::windows::fs::OpenOptionsExt;
use windows_sys::Win32::System::Memory::{
    VirtualQuery, VirtualQueryEx, MEMORY_BASIC_INFORMATION, PAGE_EXECUTE, PAGE_EXECUTE_READ,
    PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_GUARD, PAGE_READONLY, PAGE_WRITECOPY,
};

fn typed_exports(path: &Path, names: &[(&[u8], bool)]) -> Result<Vec<usize>, String> {
    let local = LocalDll(unsafe {
        LoadLibraryExW(
            wide(&path.to_string_lossy()).as_ptr(),
            null_mut(),
            DONT_RESOLVE_DLL_REFERENCES,
        )
    });
    if local.0.is_null() {
        return Err("root export image inspection failed".into());
    }
    names
        .iter()
        .map(|(name, executable)| {
            let p = unsafe { GetProcAddress(local.0, name.as_ptr()) }.ok_or("root export absent")?
                as usize;
            let mut info: MEMORY_BASIC_INFORMATION = unsafe { zeroed() };
            if unsafe { VirtualQuery(p as _, &mut info, size_of::<MEMORY_BASIC_INFORMATION>()) }
                == 0
                || info.AllocationBase != local.0
                || info.State != MEM_COMMIT
                || info.Protect & PAGE_GUARD != 0
            {
                return Err("root export outside committed source module".into());
            }
            let protection = info.Protect & 0xff;
            let valid = if *executable {
                matches!(
                    protection,
                    PAGE_EXECUTE
                        | PAGE_EXECUTE_READ
                        | PAGE_EXECUTE_READWRITE
                        | PAGE_EXECUTE_WRITECOPY
                )
            } else {
                p.is_multiple_of(8)
                    && p + 8 <= info.BaseAddress as usize + info.RegionSize
                    && matches!(protection, PAGE_READONLY | PAGE_READWRITE | PAGE_WRITECOPY)
            };
            if !valid {
                return Err("root export function/data type mismatch".into());
            }
            p.checked_sub(local.0 as usize)
                .ok_or("root export RVA underflow".into())
        })
        .collect()
}

// Preserve the actual copied prefix, including a failed/partial RPM, before a
// second attempt. Zero-filled unread bytes must never become captured evidence.
#[cfg(test)]
fn root_samples(
    pid: u32,
    created: u64,
    stop_succeeded: bool,
    read: impl FnMut(&mut [u8]) -> (usize, Option<String>),
    save: impl FnMut(&str, &[u8]) -> Result<(), String>,
) -> Value {
    capture_samples(pid, created, stop_succeeded, false, read, save)
}
fn capture_samples(
    pid: u32,
    created: u64,
    stop_succeeded: bool,
    submission: bool,
    mut read: impl FnMut(&mut [u8]) -> (usize, Option<String>),
    mut save: impl FnMut(&str, &[u8]) -> Result<(), String>,
) -> Value {
    let mut result = json!({"schema":"todo880-root-controller-receipt-v1","pid":pid,
        "created_filetime":created,"stop_response_succeeded":stop_succeeded,
        "status":"unknown","samples":[],"not_established_by_this_capture":
        ["native producer lifetime","world lifetime","all-domain drainage","runtime activation authority"]});
    let mut samples = Vec::new();
    if submission {
        result["schema"] = json!("todo880-native-submission-controller-receipt-v1");
    }
    let size = if submission {
        wire::SUBMISSION_BYTES
    } else {
        wire::ROOT_BYTES
    };
    let names = if submission {
        ["submission-first.bin", "submission-second.bin"]
    } else {
        ["root-first.bin", "root-second.bin"]
    };
    for name in names {
        let mut bytes = vec![0u8; size];
        let (copied, error) = read(&mut bytes);
        if copied > bytes.len() {
            result["error"] = json!("root read returned impossible size");
            return result;
        }
        if let Err(error) = save(name, &bytes[..copied]) {
            result["error"] = json!(format!("preserve {name}: {error}"));
            return result;
        }
        result["samples"]
            .as_array_mut()
            .unwrap()
            .push(json!({"file":name,"copied":copied,"error":error}));
        samples.push(bytes[..copied].to_vec());
        if copied != size || error.is_some() {
            result["error"] = json!("root original sample was partial or unreadable");
            return result;
        }
    }
    let equal = samples[0] == samples[1];
    result["copies_equal"] = json!(equal);
    let trace = if submission {
        wire::submission_trace(&samples[0], pid, created)
    } else {
        wire::root_trace(&samples[0], pid, created)
    };
    match trace {
        Ok(trace) => {
            let complete = stop_succeeded && equal && trace["capture_complete"] == true;
            result["status"] = json!(if complete { "captured" } else { "unknown" });
            result["trace"] = trace;
        }
        Err(error) => result["error"] = json!(error),
    }
    result
}
fn capture_root(
    game: &Handle,
    address: usize,
    output: &Path,
    pid: u32,
    created: u64,
    stopped: bool,
    submission: bool,
) -> Value {
    capture_samples(
        pid,
        created,
        stopped,
        submission,
        |bytes| {
            let mut copied = 0usize;
            let ok = unsafe {
                ReadProcessMemory(
                    game.0,
                    address as _,
                    bytes.as_mut_ptr().cast(),
                    bytes.len(),
                    &mut copied,
                )
            };
            let error = (ok == 0).then(|| std::io::Error::last_os_error().to_string());
            (copied, error)
        },
        |name, bytes| preserve(output, name, bytes),
    )
}
fn root_address(
    game: &Handle,
    module: &Module,
    data: usize,
    size: usize,
    expected: usize,
) -> Result<usize, String> {
    if pointer(game, module.base + size)? != expected {
        return Err("root trace exported size mismatch".into());
    }
    let p = pointer(game, module.base + data)?;
    let mut info: MEMORY_BASIC_INFORMATION = unsafe { zeroed() };
    if !p.is_multiple_of(8)
        || unsafe {
            VirtualQueryEx(
                game.0,
                p as _,
                &mut info,
                size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        } == 0
        || info.AllocationBase as usize != module.base
        || info.State != MEM_COMMIT
        || info.Protect & PAGE_GUARD != 0
        || !matches!(
            info.Protect & 0xff,
            PAGE_READONLY | PAGE_READWRITE | PAGE_WRITECOPY
        )
        || p.checked_add(expected)
            .is_none_or(|end| end > info.BaseAddress as usize + info.RegionSize)
    {
        return Err("root trace outside source data image".into());
    }
    Ok(p)
}

fn pin(path: &Path, expected: &str) -> Result<File, String> {
    wire::digest(expected)?;
    let mut file = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(path)
        .map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut b = [0; 65536];
    loop {
        let n = file.read(&mut b).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        digest.update(&b[..n]);
    }
    if format!("{:x}", digest.finalize()) != expected.to_ascii_lowercase() {
        return Err("pinned DLL/image SHA differs".into());
    }
    Ok(file)
}
fn exports(path: &Path, names: &[&[u8]]) -> Result<Vec<usize>, String> {
    let local = LocalDll(unsafe {
        LoadLibraryExW(
            wide(&path.to_string_lossy()).as_ptr(),
            null_mut(),
            DONT_RESOLVE_DLL_REFERENCES,
        )
    });
    if local.0.is_null() {
        return Err("bootstrap export inspection failed".into());
    }
    names
        .iter()
        .map(|name| {
            unsafe { GetProcAddress(local.0, name.as_ptr()) }
                .map(|p| p as usize - local.0 as usize)
                .ok_or("required bootstrap export absent".into())
        })
        .collect()
}
fn load(game: &Handle, pid: u32, path: &Path, loader: usize) -> Result<Module, String> {
    let text = wide(&path.to_string_lossy());
    let bytes = unsafe { std::slice::from_raw_parts(text.as_ptr().cast(), text.len() * 2) };
    let slot = remote_alloc(game, bytes.len())?;
    write_remote(game, slot, bytes)?;
    // The remote thread return is DWORD; identify the full-width HMODULE via
    // the pinned process snapshot, never truncate it from the exit code.
    remote_call(game, loader, slot, "bootstrap LoadLibraryW")?;
    modules(pid)?
        .into_iter()
        .find(|m| same_path(&m.path, path))
        .ok_or("explicit DLL not loaded at requested path".into())
}
fn pointer(game: &Handle, address: usize) -> Result<usize, String> {
    let mut b = [0; 8];
    read_remote(game, address, &mut b)?;
    Ok(u64::from_le_bytes(b) as usize)
}
fn stable_trace(
    game: &Handle,
    address: usize,
    pid: u32,
    created: u64,
) -> Result<(Value, Vec<u8>), String> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut a = vec![0; wire::TRACE_BYTES];
    let mut b = vec![0; wire::TRACE_BYTES];
    loop {
        read_remote(game, address, &mut a)?;
        read_remote(game, address, &mut b)?;
        if a == b {
            return Ok((wire::trace(&a, pid, created)?, a));
        }
        if Instant::now() >= deadline {
            return Err("bootstrap trace remained unstable".into());
        }
        std::thread::yield_now();
    }
}

// Failure evidence is sampled without calling any target export or waiting for
// a drain. Keep each original sample before attempting the next read. Even a
// valid, identical pair is diagnostic evidence, never successful completion.
#[derive(Clone, Copy)]
enum ObservationStopStage {
    NotRequested,
    Requested,
    ReturnedSuccess,
}

fn failure_trace_snapshot(
    pid: u32,
    created: u64,
    stop_stage: ObservationStopStage,
    mut read: impl FnMut(&mut [u8]) -> Result<(), String>,
    mut save: impl FnMut(&str, &[u8]) -> Result<(), String>,
) -> Value {
    let mut evidence = json!({"type":"bootstrap_failure_snapshot",
        "pid":pid,"created_filetime":created,"product_acceptance":false,
        "drained":null,"stop_observation_stage":match stop_stage {
            ObservationStopStage::NotRequested => "not_requested",
            ObservationStopStage::Requested => "requested",
            ObservationStopStage::ReturnedSuccess => "returned_success",
        },"samples":[]});
    let mut snapshots = Vec::new();
    for name in [
        "failure-bootstrap-first.bin",
        "failure-bootstrap-second.bin",
    ] {
        evidence["attempted_sample"] = json!(name);
        let mut bytes = vec![0; wire::TRACE_BYTES];
        if let Err(error) = read(&mut bytes) {
            evidence["error"] = json!(format!("read {name}: {error}"));
            return evidence;
        }
        if let Err(error) = save(name, &bytes) {
            evidence["error"] = json!(format!("preserve {name}: {error}"));
            return evidence;
        }
        evidence["samples"]
            .as_array_mut()
            .unwrap()
            .push(json!(name));
        snapshots.push(bytes);
    }
    evidence.as_object_mut().unwrap().remove("attempted_sample");
    evidence["identical_reads"] = json!(snapshots[0] == snapshots[1]);
    match wire::trace(&snapshots[0], pid, created) {
        Ok(trace) => evidence["first_trace_diagnostic"] = trace,
        Err(error) => evidence["first_trace_error"] = json!(error),
    }
    evidence
}

fn failure_evidence_error(
    mut error: String,
    evidence: Option<Value>,
    record: impl FnOnce(Value) -> Result<(), String>,
) -> String {
    if let Some(evidence) = evidence {
        if let Some(capture_error) = evidence["error"].as_str() {
            error = format!("{error}; failure evidence: {capture_error}");
        }
        if let Err(log_error) = record(evidence) {
            error = format!("{error}; failure evidence log: {log_error}");
        }
    }
    error
}

pub(super) fn run(raw: &[String]) -> Result<(), String> {
    if raw.len() != 2 || raw[0] != "--request" {
        return Err("usage: civ6-native-bridge bootstrap --request ABSOLUTE.json".into());
    }
    let req = wire::Request::parse(
        &serde_json::from_slice::<Value>(&fs::read(&raw[1]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?,
    )?;
    let chain_pid = std::env::var("CIV6_LIVE_CHAIN_PID")
        .map_err(|_| "chain PID missing")?
        .parse::<u32>()
        .map_err(|_| "chain PID")?;
    let runner_pid = std::env::var("CIV6_AUTOTEST_CONTROLLER_PID")
        .map_err(|_| "runner PID missing")?
        .parse::<u32>()
        .map_err(|_| "runner PID")?;
    if parent_pid(unsafe { GetCurrentProcessId() })? != runner_pid {
        return Err("bootstrap controller OS parent differs".into());
    }
    let runner = open_process(
        runner_pid,
        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
    )?;
    let chain = open_process(
        chain_pid,
        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
    )?;
    let own = open_process(
        unsafe { GetCurrentProcessId() },
        PROCESS_QUERY_LIMITED_INFORMATION,
    )?;
    if !alive(&runner)
        || !same_path(
            &process_path(&runner)?,
            &std::env::current_exe()
                .map_err(|e| e.to_string())?
                .with_file_name("civ6-autotest.exe"),
        )
        || process_created(&runner)? > process_created(&own)?
    {
        return Err("runner path/FILETIME differs".into());
    }
    let chain_path = process_path(&chain)?;
    let _chain_pin = if same_path(&chain_path, Path::new(CHAIN)) {
        None
    } else if same_path(
        &chain_path,
        Path::new("D:/CC/Tools/civ6-live/live_chain_pr86_amd.exe"),
    ) {
        Some(pin(
            &chain_path,
            "267ee55fdbe47e507bbefcbb8154780bb2a4070412802f923e0762764d7dc82b",
        )?)
    } else {
        return Err("bootstrap chain image is not allowlisted".into());
    };
    verify_lease_for_image(chain_pid, &chain, req.controller_seconds, &chain_path)?;
    let game = open_process(
        req.pid,
        PROCESS_QUERY_LIMITED_INFORMATION
            | PROCESS_SYNCHRONIZE
            | PROCESS_CREATE_THREAD
            | PROCESS_VM_OPERATION
            | PROCESS_VM_WRITE
            | PROCESS_VM_READ
            | PROCESS_TERMINATE
            | PROCESS_SET_QUOTA,
    )?;
    if !alive(&game)
        || process_created(&game)? != req.created
        || !same_path(&process_path(&game)?, Path::new(GAME_EXE))
        || req.created < process_created(&chain)?
    {
        return Err("bootstrap game PID/FILETIME/image differs".into());
    }
    let _exe_pin = pin(Path::new(GAME_EXE), civ6_bootstrap_rs::EXE_SHA)?;
    let _runtime_pin = pin(&req.runtime, &req.runtime_sha)?;
    let _bootstrap_pin = pin(&req.bootstrap, &req.bootstrap_sha)?;
    let prepare_version = if req.submission_only {
        wire::SUBMISSION_PREPARE_VERSION
    } else if req.root_only {
        wire::ROOT_PREPARE_VERSION
    } else {
        civ6_bootstrap_rs::VERSION
    };
    let receipt_name = if req.submission_only {
        "submission-receipt.json"
    } else {
        "root-receipt.json"
    };
    let prepared = if req.root_only {
        typed_exports(
            &req.bootstrap,
            &[
                (
                    if req.submission_only {
                        b"RustBridgeBootstrapPrepareSubmissionObservation\0"
                    } else {
                        b"RustBridgeBootstrapPrepareRootObservation\0"
                    },
                    true,
                ),
                (b"RustBridgeBootstrapStopObservation\0", true),
                (b"BOOTSTRAP_TRACE_POINTER\0", false),
            ],
        )?
    } else {
        exports(
            &req.bootstrap,
            &[
                b"RustBridgeBootstrapPrepare\0",
                b"RustBridgeBootstrapStopObservation\0",
                b"BOOTSTRAP_TRACE_POINTER\0",
            ],
        )?
    };
    let runtime_exports = if req.root_only {
        typed_exports(
            &req.runtime,
            &[
                (
                    if req.submission_only {
                        b"BridgeSubmissionObservationInstall\0"
                    } else {
                        b"BridgeRootObservationInstall\0"
                    },
                    true,
                ),
                (
                    if req.submission_only {
                        b"BridgeSubmissionObservationStop\0"
                    } else {
                        b"BridgeRootObservationStop\0"
                    },
                    true,
                ),
                (
                    if req.submission_only {
                        b"BRIDGE_SUBMISSION_OBSERVATION_TRACE\0"
                    } else {
                        b"BRIDGE_ROOT_OBSERVATION_TRACE\0"
                    },
                    false,
                ),
                (
                    if req.submission_only {
                        b"BRIDGE_SUBMISSION_OBSERVATION_TRACE_SIZE\0"
                    } else {
                        b"BRIDGE_ROOT_OBSERVATION_TRACE_SIZE\0"
                    },
                    false,
                ),
            ],
        )?
    } else {
        exports(
            &req.runtime,
            &[
                b"BridgeRuntimeInstall\0",
                b"BridgeRuntimeClose\0",
                b"BridgeRuntimeRebuild\0",
                b"BridgeRuntimeStopObservation\0",
                b"BRIDGE_RUNTIME_PROPERTY_LOG\0",
            ],
        )?
    };
    let list = modules(req.pid)?;
    if list.iter().any(|m| same_path(&m.path, Path::new(GAMECORE))) {
        return Err("cold bootstrap requires target XP2 absent before Prepare".into());
    }
    let exe = list
        .iter()
        .find(|m| same_path(&m.path, Path::new(GAME_EXE)))
        .ok_or("game EXE module missing")?;
    let menu_context = pointer(&game, exe.base + 0x22d60c0)?;
    let loader = remote_load_library(req.pid)?;
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&req.output)
        .map_err(|e| e.to_string())?;
    let mut writer = BufWriter::new(file);
    write_json(
        &mut writer,
        json!({"type":"bootstrap_identity","pid":req.pid,"created_filetime":req.created,"chain_pid":chain_pid,"chain_created_filetime":process_created(&chain)?,"runner_pid":runner_pid,"runner_created_filetime":process_created(&runner)?,"bootstrap_sha256":req.bootstrap_sha,"runtime_sha256":req.runtime_sha,"prepare_version":prepare_version,"menu_context":menu_context,"runtime_installed":false}),
    )?;
    let mut job = OwnedJob::attach(&game, req.pid, req.created)?;
    let mut prepared_trace_address = None;
    let mut root_trace_address = None;
    let mut root_evidence_saved = false;
    let mut stop_observation_stage = ObservationStopStage::NotRequested;
    let result = (|| -> Result<(), String> {
        let runtime = load(&game, req.pid, &req.runtime, loader)?;
        let bootstrap = load(&game, req.pid, &req.bootstrap, loader)?;
        if prepared.iter().any(|r| *r >= bootstrap.size)
            || runtime_exports.iter().any(|r| r + 8 > runtime.size)
        {
            return Err("export outside source image".into());
        }
        if req.root_only {
            root_trace_address = Some(root_address(
                &game,
                &runtime,
                runtime_exports[2],
                runtime_exports[3],
                if req.submission_only {
                    wire::SUBMISSION_BYTES
                } else {
                    wire::ROOT_BYTES
                },
            )?);
        }
        let config = Prepare {
            version: prepare_version,
            size: size_of::<Prepare>() as u32,
            pid: req.pid,
            observation_seconds: req.seconds,
            created_filetime: req.created,
            bridge_module: runtime.base as u64,
            bridge_sha256: wire::digest(&req.runtime_sha)?,
            expected_menu_context: menu_context as u64,
        };
        let bytes = unsafe {
            std::slice::from_raw_parts((&config as *const Prepare).cast(), size_of::<Prepare>())
        };
        let slot = remote_alloc(&game, bytes.len())?;
        write_remote(&game, slot, bytes)?;
        let status = remote_call(
            &game,
            bootstrap.base + prepared[0],
            slot,
            "RustBridgeBootstrapPrepare",
        )?;
        write_json(
            &mut writer,
            json!({"type":"bootstrap_prepare","status":status,"runtime_installed":false}),
        )?;
        if status != 0 {
            return Err("Prepare refused; no resend".into());
        }
        let address = pointer(&game, bootstrap.base + prepared[2])?;
        let mut header = [0u8; wire::HEADER];
        read_remote(&game, address, &mut header)?;
        if wire::word(&header, 0)? != 2
            || wire::word(&header, 16)? != req.pid as u64
            || wire::word(&header, 24)? != req.created
            || wire::word(&header, 32)? != 0
            || wire::word(&header, 40)? != 0
        {
            return Err("prepared trace identity differs".into());
        }
        prepared_trace_address = Some(address);
        // Preparation/injection consumed part of the lease. Recheck the whole
        // controller budget at its actual start, including the existing
        // recovery reserve; a runner-side check is not controller authority.
        verify_lease_for_image(chain_pid, &chain, req.controller_seconds, &chain_path)?;
        writeln!(
            std::io::stdout(),
            "READY pid={} created={} bootstrap=v2 prepared=true runtime_installed=false",
            req.pid,
            req.created
        )
        .and_then(|_| std::io::stdout().flush())
        .map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut input = std::io::stdin().lock();
            let mut line = String::new();
            let r = input.read_line(&mut line).map(|n| {
                if n == 0 {
                    "EOF".into()
                } else {
                    line.trim().to_string()
                }
            });
            let _ = tx.send(r);
        });
        let deadline = Instant::now() + Duration::from_secs(req.controller_seconds as u64);
        loop {
            if !alive(&runner) || !alive(&game) {
                return Err("pinned runner/game exited".into());
            }
            verify_lease_for_image(chain_pid, &chain, 0, &chain_path)?;
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Ok(command)) if command == "stop" => break,
                Ok(_) => return Err("bootstrap input ended or was invalid".into()),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("bootstrap input reader lost".into());
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if Instant::now() >= deadline {
                return Err("bootstrap controller deadline before explicit stop".into());
            }
        }
        stop_observation_stage = ObservationStopStage::Requested;
        if remote_call(
            &game,
            bootstrap.base + prepared[1],
            0,
            "RustBridgeBootstrapStopObservation",
        )? != 0
        {
            return Err("bootstrap observation drain failed".into());
        }
        stop_observation_stage = ObservationStopStage::ReturnedSuccess;
        if req.root_only {
            let evidence = capture_root(
                &game,
                root_trace_address.ok_or("root trace address absent")?,
                &req.output,
                req.pid,
                req.created,
                true,
                req.submission_only,
            );
            root_evidence_saved = true;
            let captured = evidence["status"] == "captured";
            preserve(
                &req.output,
                receipt_name,
                &serde_json::to_vec(&evidence).map_err(|e| e.to_string())?,
            )?;
            write_json(&mut writer, evidence)?;
            if !captured {
                return Err("root observation remained unknown; original samples preserved".into());
            }
            if !alive(&game) || process_created(&game)? != req.created {
                return Err("root owned game changed before stop".into());
            }
            write_json(
                &mut writer,
                json!({"type":if req.submission_only {"native_submission_stop"} else {"root_observation_stop"},"pid":req.pid,
                "created_filetime":req.created,"observation_stopped":true,
                "resident_code_retained":true,"not_established_by_this_capture":
                ["all-domain drainage","world lifetime","runtime activation authority"]}),
            )?;
            job.disarm_after_verified_operation()?;
            return Ok(());
        }
        let (trace, bytes) = stable_trace(&game, address, req.pid, req.created)?;
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(req.output.with_extension("bootstrap.bin"))
            .and_then(|mut f| f.write_all(&bytes))
            .map_err(|e| e.to_string())?;
        let native_address = trace["runtime_trace"]
            .as_u64()
            .ok_or("native trace missing")? as usize;
        let size = trace["runtime_trace_size"]
            .as_u64()
            .filter(|s| *s > 0 && *s <= 16 * 1024 * 1024)
            .ok_or("native trace size")? as usize;
        let mut native = vec![0; size];
        let mut again = vec![0; size];
        read_remote(&game, native_address, &mut native)?;
        read_remote(&game, native_address, &mut again)?;
        // Preserve both original observations even when they fail validation.
        // Ongoing policy guards and sampled rows must not be reconstructed from
        // another process or a later successful read after the owned game ends.
        for (extension, bytes) in [("native-first.bin", &native), ("native-second.bin", &again)] {
            preserve(&req.output, extension, bytes)?;
        }
        let log = property_log_pair(&game, runtime.base + runtime_exports[4])?;
        for (extension, bytes) in [
            ("property-log-first.bin", &log.0),
            ("property-log-second.bin", &log.1),
        ] {
            preserve(&req.output, extension, bytes)?;
        }
        if native != again {
            return Err("native drained trace is not stable".into());
        }
        if log.0 != log.1 {
            return Err("runtime property log is not stable".into());
        }
        wire::native_trace(&native, Some(&log.0), req.pid, req.created)?;
        let summary = wire::property_log(&log.0, req.pid, req.created)?;
        write_json(
            &mut writer,
            json!({"type":"bootstrap_property_log","pid":req.pid,"created_filetime":req.created,"rows":summary.rows,"dropped":0,"in_flight":0,"filtered":summary.filtered,"capacity":wire::PROPERTY_LOG_CAPACITY,"bytes":wire::PROPERTY_LOG_BYTES,"captures_equal":true}),
        )?;
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(req.output.with_extension("runtime.bin"))
            .and_then(|mut f| f.write_all(&native))
            .map_err(|e| e.to_string())?;
        write_json(&mut writer, trace)?;
        if !alive(&game) || process_created(&game)? != req.created {
            return Err("resident game changed before stop".into());
        }
        write_json(
            &mut writer,
            json!({"type":"bootstrap_stop","pid":req.pid,"created_filetime":req.created,"observation_stopped":true,"resident_code_retained":true,"runtime_observation_seconds":req.seconds,"controller_seconds":req.controller_seconds,"product_acceptance":false}),
        )?;
        job.disarm_after_verified_operation()?;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            if req.root_only {
                let evidence = if !root_evidence_saved {
                    root_trace_address.map(|address| {
                        capture_root(
                            &game,
                            address,
                            &req.output,
                            req.pid,
                            req.created,
                            matches!(
                                stop_observation_stage,
                                ObservationStopStage::ReturnedSuccess
                            ),
                            req.submission_only,
                        )
                    })
                } else {
                    None
                };
                let evidence = evidence.or_else(|| {
                    (!root_evidence_saved).then(|| {
                        json!({
                        "schema":if req.submission_only {"todo880-native-submission-controller-receipt-v1"} else {"todo880-root-controller-receipt-v1"},"pid":req.pid,
                        "created_filetime":req.created,"status":"unknown","samples":[],
                        "stop_response_succeeded":false,"error":"root trace address unavailable"})
                    })
                });
                let error = failure_evidence_error(e, evidence, |record| {
                    preserve(
                        &req.output,
                        receipt_name,
                        &serde_json::to_vec(&record).map_err(|e| e.to_string())?,
                    )?;
                    write_json(&mut writer, record)
                        .and_then(|_| writer.flush().map_err(|e| e.to_string()))
                });
                return Err(fail_owned_call(
                    &mut job,
                    &game,
                    error,
                    RECOVERY_SECONDS * 1000,
                ));
            }
            let evidence = prepared_trace_address.map(|address| {
                failure_trace_snapshot(
                    req.pid,
                    req.created,
                    stop_observation_stage,
                    |bytes| read_remote(&game, address, bytes),
                    |extension, bytes| preserve(&req.output, extension, bytes),
                )
            });
            let error = failure_evidence_error(e, evidence, |evidence| {
                write_json(&mut writer, evidence)
                    .and_then(|_| writer.flush().map_err(|error| error.to_string()))
            });
            // Evidence collection must never replace or prevent containment.
            Err(fail_owned_call(
                &mut job,
                &game,
                error,
                RECOVERY_SECONDS * 1000,
            ))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn root_controller_preserves_actual_partial_bytes_and_never_completes_empty() {
        let mut reads = 0;
        let mut saved = Vec::new();
        let evidence = root_samples(
            41,
            42,
            true,
            |bytes| {
                reads += 1;
                bytes[..3].copy_from_slice(&[7, 8, 9]);
                (3, Some("owned partial RPM".into()))
            },
            |name, bytes| {
                saved.push((name.to_owned(), bytes.to_vec()));
                Ok(())
            },
        );
        assert_eq!(reads, 1);
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].1, vec![7, 8, 9]);
        assert_eq!(evidence["status"], "unknown");
        assert_eq!(evidence["samples"][0]["copied"], 3);
        assert!(evidence.get("copies_equal").is_none());
        let mut copies = 0;
        let empty = root_samples(
            41,
            42,
            true,
            |bytes| {
                copies += 1;
                bytes.fill(0);
                (bytes.len(), None)
            },
            |_, _| Ok(()),
        );
        assert_eq!(copies, 2);
        assert_eq!(empty["copies_equal"], true);
        assert_eq!(empty["status"], "unknown");
    }
    #[test]
    fn root_controller_create_new_failure_does_not_replace_or_resample() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture.jsonl");
        let original = path.with_extension("root-first.bin");
        fs::write(&original, [5, 6]).unwrap();
        let mut reads = 0;
        let result = root_samples(
            41,
            42,
            false,
            |b| {
                reads += 1;
                b.fill(0);
                (b.len(), None)
            },
            |extension, b| preserve(&path, extension, b),
        );
        assert_eq!(reads, 1);
        assert_eq!(fs::read(original).unwrap(), vec![5, 6]);
        assert_eq!(result["status"], "unknown");
    }

    #[test]
    fn bootstrap_failure_summary_retains_original_capture_and_log_errors() {
        let mut recorded = false;
        let error = failure_evidence_error(
            "original controller deadline".into(),
            Some(json!({"error":"owned process unreadable","product_acceptance":false})),
            |record| {
                recorded = true;
                assert_eq!(record["product_acceptance"], false);
                Err("output write failed".into())
            },
        );
        assert!(recorded);
        assert!(error.starts_with("original controller deadline;"));
        assert!(error.contains("failure evidence: owned process unreadable"));
        assert!(error.contains("failure evidence log: output write failed"));
    }

    #[test]
    fn bootstrap_failure_preserves_first_sample_when_second_read_fails() {
        let mut reads = 0;
        let mut saved = Vec::new();
        let evidence = failure_trace_snapshot(
            41,
            42,
            ObservationStopStage::NotRequested,
            |bytes| {
                reads += 1;
                if reads == 2 {
                    return Err("owned process exited".into());
                }
                bytes.fill(7);
                Ok(())
            },
            |name, bytes| {
                saved.push((name.to_owned(), bytes.to_vec()));
                Ok(())
            },
        );
        assert_eq!(reads, 2);
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].0, "failure-bootstrap-first.bin");
        assert_eq!(saved[0].1, vec![7; wire::TRACE_BYTES]);
        assert_eq!(evidence["samples"].as_array().unwrap().len(), 1);
        assert!(evidence["error"].as_str().unwrap().contains("second"));
        assert_eq!(evidence["product_acceptance"], false);
        assert_eq!(evidence["stop_observation_stage"], "not_requested");
        assert!(evidence["drained"].is_null());
        assert!(evidence.get("identical_reads").is_none());
    }

    #[test]
    fn bootstrap_failure_equal_or_changed_samples_never_mean_completion() {
        for changed in [false, true] {
            let mut reads = 0;
            let mut saved = Vec::new();
            let evidence = failure_trace_snapshot(
                41,
                42,
                ObservationStopStage::Requested,
                |bytes| {
                    reads += 1;
                    bytes.fill(if changed && reads == 2 { 1 } else { 0 });
                    Ok(())
                },
                |name, bytes| {
                    saved.push((name.to_owned(), bytes.to_vec()));
                    Ok(())
                },
            );
            assert_eq!(saved.len(), 2);
            assert_eq!(saved[0].1, vec![0; wire::TRACE_BYTES]);
            assert_eq!(saved[1].1, vec![u8::from(changed); wire::TRACE_BYTES]);
            assert_eq!(evidence["identical_reads"], !changed);
            assert!(evidence.get("attempted_sample").is_none());
            assert_eq!(evidence["product_acceptance"], false);
            assert_eq!(evidence["stop_observation_stage"], "requested");
            assert!(evidence["drained"].is_null());
            assert!(evidence["first_trace_error"].as_str().is_some());
        }
    }

    #[test]
    fn bootstrap_failure_preservation_error_does_not_attempt_another_read() {
        let mut reads = 0;
        let evidence = failure_trace_snapshot(
            41,
            42,
            ObservationStopStage::ReturnedSuccess,
            |_| {
                reads += 1;
                Ok(())
            },
            |_, _| Err("destination already exists".into()),
        );
        assert_eq!(reads, 1);
        assert!(evidence["samples"].as_array().unwrap().is_empty());
        assert!(evidence["error"].as_str().unwrap().contains("preserve"));
        assert_eq!(evidence["product_acceptance"], false);
        assert_eq!(evidence["attempted_sample"], "failure-bootstrap-first.bin");
        assert_eq!(evidence["stop_observation_stage"], "returned_success");
        assert!(evidence["drained"].is_null());
    }

    #[test]
    fn bootstrap_controller_owned_file_pin_rejects_write_delete_and_changed_hash() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("owned.dll");
        fs::write(&p, b"todo880-bootstrap-owned-pin").unwrap();
        let expected = "b4537aa24423d159a52e53306bf5621b689796e776c0eba7fb767c9d4d572b38";
        let file = pin(&p, expected).unwrap();
        assert!(fs::OpenOptions::new().write(true).open(&p).is_err());
        assert!(fs::remove_file(&p).is_err());
        drop(file);
        fs::write(&p, b"changed").unwrap();
        assert!(pin(&p, expected).is_err());
        fs::remove_file(p).unwrap();
    }
    #[test]
    fn submission_controller_preserves_partial_original_bytes_and_distinct_schema() {
        let mut saved = Vec::new();
        let receipt = capture_samples(
            23,
            29,
            true,
            true,
            |b| {
                b[..13].copy_from_slice(&[0x5a; 13]);
                (13, Some("owned partial read".into()))
            },
            |name, b| {
                saved.push((name.to_owned(), b.to_vec()));
                Ok(())
            },
        );
        assert_eq!(
            receipt["schema"],
            "todo880-native-submission-controller-receipt-v1"
        );
        assert_eq!(receipt["status"], "unknown");
        assert_eq!(saved, vec![("submission-first.bin".into(), vec![0x5a; 13])]);
        let empty = capture_samples(23, 29, true, true, |b| (b.len(), None), |_, _| Ok(()));
        assert_eq!(empty["status"], "unknown");
    }
    #[test]
    fn submission_controller_existing_destination_never_overwrites_or_resamples() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("submission.jsonl");
        let target = path.with_extension("submission-first.bin");
        fs::write(&target, b"original").unwrap();
        let mut reads = 0;
        let receipt = capture_samples(
            23,
            29,
            false,
            true,
            |b| {
                reads += 1;
                (b.len(), None)
            },
            |name, b| preserve(&path, name, b),
        );
        assert_eq!(reads, 1);
        assert_eq!(fs::read(target).unwrap(), b"original");
        assert_eq!(receipt["status"], "unknown");
        assert!(receipt["error"].as_str().unwrap().contains("preserve"));
    }
}
