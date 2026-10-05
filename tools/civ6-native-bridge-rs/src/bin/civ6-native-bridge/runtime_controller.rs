//! Explicit ownership transfer, separate from the bounded uninstall contract.
use super::*;

fn exports(path: &Path) -> Result<(usize, usize), String> {
    let dll = LocalDll(unsafe {
        LoadLibraryExW(
            wide(&path.to_string_lossy()).as_ptr(),
            null_mut(),
            DONT_RESOLVE_DLL_REFERENCES,
        )
    });
    if dll.0.is_null() {
        return Err("cannot inspect runtime DLL".into());
    }
    let install = unsafe { GetProcAddress(dll.0, c"BridgeRuntimeInstall".as_ptr().cast()) }
        .ok_or("runtime install export missing")? as usize;
    let stop = unsafe { GetProcAddress(dll.0, c"BridgeRuntimeStopObservation".as_ptr().cast()) }
        .ok_or("runtime observation stop export missing")? as usize;
    Ok((install - dll.0 as usize, stop - dll.0 as usize))
}

fn patched_entries(
    game: &Handle,
    base: usize,
    can_original: [u8; 8],
    property_original: [u8; 8],
    assignment_original: [u8; 8],
) -> Result<(), String> {
    for (rva, expected) in [
        (PINNED_CAN_EVER_RVA, can_original),
        (
            civ6_native_bridge_rs::PINNED_CAN_ASSIGN_RVA,
            assignment_original,
        ),
    ] {
        let mut bytes = [0u8; 8];
        read_remote(game, base + rva, &mut bytes)?;
        if bytes[0] != 0xe9 || bytes[5..] != expected[5..] {
            return Err("runtime entry publication differs".into());
        }
    }
    let mut bytes = [0u8; 8];
    read_remote(game, base + PINNED_PROPERTY_GET_RVA, &mut bytes)?;
    if bytes[..2] != [0xeb, 0xf9] || bytes[2..] != property_original[2..] {
        return Err("runtime property publication differs".into());
    }
    let mut prefix = [0u8; PROPERTY_PREFIX_LEN];
    read_remote(
        game,
        base + PINNED_PROPERTY_GET_RVA - PROPERTY_PREFIX_LEN,
        &mut prefix,
    )?;
    if prefix[0] != 0xe9 {
        return Err("runtime prefix missing".into());
    }
    Ok(())
}

pub(super) fn run(
    parsed: &Args,
    game: &Handle,
    chain: &Handle,
    chain_pid: u32,
    target: usize,
    original: [u8; 8],
    property_original: [u8; 8],
    _original_prefix: [u8; PROPERTY_PREFIX_LEN],
) -> Result<(), String> {
    let base = target - PINNED_CAN_EVER_RVA;
    let assignment_original = pinned_entry_bytes(
        game,
        base + civ6_native_bridge_rs::PINNED_CAN_ASSIGN_RVA,
        BridgeMode::AssignCalibration,
    )?;
    let dll = bridge_dll()?;
    let bridge_sha = sha256(&dll)?;
    let (install_rva, stop_rva) = exports(&dll)?;
    let loader = remote_load_library(parsed.pid)?;
    let mut writer = BufWriter::new(File::create_new(&parsed.out).map_err(|e| e.to_string())?);
    write_json(
        &mut writer,
        json!({"kind":"identity", "mode":"paid-assignment-runtime",
        "pid":parsed.pid,"created_filetime":parsed.created,"gamecore_sha256":PINNED_GAMECORE_SHA256,
        "bridge_sha256":bridge_sha,"observation_limit_seconds":parsed.duration,
        "runtime_lifetime":"process_exit","reload_hook_observed":false,
        "official_ai_payment_observed":false}),
    )?;
    // Recovery job applies only to the finite installation transaction. It is
    // disarmed after verifying all entries and the game-owned allocation.
    let mut job = OwnedJob::attach(game, parsed.pid, parsed.created)?;
    let transfer = (|| {
        let path = wide(&dll.to_string_lossy());
        let bytes =
            unsafe { std::slice::from_raw_parts(path.as_ptr().cast::<u8>(), path.len() * 2) };
        let remote_path = remote_alloc(game, bytes.len())?;
        write_remote(game, remote_path, bytes)?;
        remote_call(game, loader, remote_path, "LoadLibraryW")?;
        let bridge = modules(parsed.pid)?
            .into_iter()
            .find(|m| same_path(&m.path, &dll))
            .ok_or("runtime DLL missing")?;
        if install_rva >= bridge.size || stop_rva >= bridge.size {
            return Err("runtime exports outside remote DLL".into());
        }
        let mut config = BridgeConfig::for_production(parsed.pid, parsed.created, 0);
        config.version = civ6_native_bridge_rs::RUNTIME_CONFIG_VERSION;
        config.trace_size = 0;
        config.max_seconds = parsed.duration;
        let bytes = unsafe {
            std::slice::from_raw_parts(
                (&config as *const BridgeConfig).cast::<u8>(),
                size_of::<BridgeConfig>(),
            )
        };
        let remote_config = remote_alloc(game, bytes.len())?;
        write_remote(game, remote_config, bytes)?;
        let status = remote_call(
            game,
            bridge.base + install_rva,
            remote_config,
            "BridgeRuntimeInstall",
        )?;
        if status != BridgeStatus::Ok as u32 {
            return Err(format!("runtime install refused: {status}"));
        }
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(
                (&mut config as *mut BridgeConfig).cast::<u8>(),
                size_of::<BridgeConfig>(),
            )
        };
        read_remote(game, remote_config, bytes)?;
        if config.version != civ6_native_bridge_rs::RUNTIME_CONFIG_VERSION
            || config.game_pid != parsed.pid
            || config.created_filetime != parsed.created
            || config.trace_address == 0
            || config.trace_size as usize != size_of::<civ6_native_bridge_rs::CompositeTrace>()
        {
            return Err("runtime ownership receipt differs".into());
        }
        patched_entries(game, base, original, property_original, assignment_original)?;
        // Check the retained allocation is readable before ownership transfer.
        let mut header = [0u8; 8];
        read_remote(game, config.trace_address as usize, &mut header)?;
        write_json(
            &mut writer,
            json!({"kind":"runtime_install","status":status,
            "three_entries_published":true,"trace_address":config.trace_address,
            "trace_owner":"game_process","bounded_uninstall_allowed":false}),
        )?;
        job.disarm_after_verified_runtime_transfer()?;
        Ok(bridge.base + stop_rva)
    })();
    let stop = match transfer {
        Ok(stop) => stop,
        Err(error) => {
            return Err(fail_owned_call(
                &mut job,
                game,
                error,
                RECOVERY_SECONDS * 1000,
            ))
        }
    };
    println!(
        "READY pid={} observation_seconds={} runtime=resident",
        parsed.pid, parsed.duration
    );
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = sender.send(std::io::stdin().lock().read_line(&mut line).map(|_| line));
    });
    let deadline = Instant::now() + Duration::from_secs(u64::from(parsed.duration));
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match receiver.recv_timeout(remaining.min(Duration::from_secs(1))) {
            Ok(_) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if verify_lease(chain_pid, chain, 0).is_err() || !alive(game) {
            break;
        }
    }
    let status = remote_call(game, stop, 0, "BridgeRuntimeStopObservation")?;
    if status != BridgeStatus::Ok as u32 {
        return Err(format!(
            "observation drain failed: {status}; runtime retained"
        ));
    }
    patched_entries(game, base, original, property_original, assignment_original)?;
    write_json(
        &mut writer,
        json!({"kind":"observation_stop","status":status,
        "runtime_retained":true,"three_entries_published":true,"trace_retained":true}),
    )?;
    Ok(())
}
