//! Compose the existing controller's exact handles, lease, OwnedJob and remote
//! call implementation. There is deliberately no CreateRemoteThread here.
use super::*;
use civ6_bootstrap_rs::{protocol as wire, Prepare};
use std::os::windows::fs::OpenOptionsExt;

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
    verify_lease_for_image(chain_pid, &chain, req.seconds, &chain_path)?;
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
    let prepared = exports(
        &req.bootstrap,
        &[
            b"RustBridgeBootstrapPrepare\0",
            b"RustBridgeBootstrapStopObservation\0",
            b"BOOTSTRAP_TRACE_POINTER\0",
        ],
    )?;
    let runtime_exports = exports(
        &req.runtime,
        &[
            b"BridgeRuntimeInstall\0",
            b"BridgeRuntimeClose\0",
            b"BridgeRuntimeRebuild\0",
            b"BridgeRuntimeStopObservation\0",
        ],
    )?;
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
        json!({"type":"bootstrap_identity","pid":req.pid,"created_filetime":req.created,"chain_pid":chain_pid,"chain_created_filetime":process_created(&chain)?,"runner_pid":runner_pid,"runner_created_filetime":process_created(&runner)?,"bootstrap_sha256":req.bootstrap_sha,"runtime_sha256":req.runtime_sha,"prepare_version":civ6_bootstrap_rs::VERSION,"menu_context":menu_context,"runtime_installed":false}),
    )?;
    let mut job = OwnedJob::attach(&game, req.pid, req.created)?;
    let result = (|| -> Result<(), String> {
        let runtime = load(&game, req.pid, &req.runtime, loader)?;
        let bootstrap = load(&game, req.pid, &req.bootstrap, loader)?;
        if prepared.iter().any(|r| *r >= bootstrap.size)
            || runtime_exports.iter().any(|r| *r >= runtime.size)
        {
            return Err("export outside source image".into());
        }
        let config = Prepare {
            version: civ6_bootstrap_rs::VERSION,
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
        let deadline = Instant::now() + Duration::from_secs(req.seconds as u64);
        loop {
            if !alive(&runner) || !alive(&game) {
                return Err("pinned runner/game exited".into());
            }
            verify_lease_for_image(chain_pid, &chain, 0, &chain_path)?;
            match rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Ok(command)) if command == "stop" => break,
                Ok(_) => return Err("bootstrap input ended or was invalid".into()),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("bootstrap input reader lost".into())
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if Instant::now() >= deadline {
                return Err("bootstrap observation deadline before explicit stop".into());
            }
        }
        if remote_call(
            &game,
            bootstrap.base + prepared[1],
            0,
            "RustBridgeBootstrapStopObservation",
        )? != 0
        {
            return Err("bootstrap observation drain failed".into());
        }
        let (trace, bytes) = stable_trace(&game, address, req.pid, req.created)?;
        fs::OpenOptions::new().write(true).create_new(true).open(req.output.with_extension("bootstrap.bin")).and_then(|mut f|f.write_all(&bytes)).map_err(|e|e.to_string())?;
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
        if native != again {
            return Err("native drained trace is not stable".into());
        }
        wire::native_trace(&native)?;
        fs::OpenOptions::new().write(true).create_new(true).open(req.output.with_extension("runtime.bin")).and_then(|mut f|f.write_all(&native)).map_err(|e|e.to_string())?;
        write_json(&mut writer, trace)?;
        if !alive(&game) || process_created(&game)? != req.created {
            return Err("resident game changed before stop".into());
        }
        write_json(
            &mut writer,
            json!({"type":"bootstrap_stop","pid":req.pid,"created_filetime":req.created,"observation_stopped":true,"resident_code_retained":true,"product_acceptance":false}),
        )?;
        job.disarm_after_verified_operation()?;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(()),
        Err(e) => Err(fail_owned_call(&mut job, &game, e, RECOVERY_SECONDS * 1000)),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
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
}
