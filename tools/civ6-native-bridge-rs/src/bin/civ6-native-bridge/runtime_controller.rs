//! Explicit ownership transfer, separate from the bounded uninstall contract.
use super::*;

struct Exports {
    install: usize,
    stop: usize,
    property_log: usize,
    gateways: [usize; 4],
}

fn exports(path: &Path) -> Result<Exports, String> {
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
    let property_log =
        unsafe { GetProcAddress(dll.0, c"BRIDGE_RUNTIME_PROPERTY_LOG".as_ptr().cast()) }
            .ok_or("runtime property log export missing")? as usize;
    let mut gateways = [0usize; 4];
    for (slot, name) in gateways.iter_mut().zip([
        c"BRIDGE_RUNTIME_APPOINTMENT_GATEWAY",
        c"BRIDGE_RUNTIME_PROPERTY_GATEWAY",
        c"BRIDGE_RUNTIME_ASSIGNMENT_GATEWAY",
        c"BRIDGE_RUNTIME_PROMOTION_GATEWAY",
    ]) {
        *slot = unsafe { GetProcAddress(dll.0, name.as_ptr().cast()) }
            .ok_or("runtime gateway data export missing")? as usize
            - dll.0 as usize;
    }
    Ok(Exports {
        install: install - dll.0 as usize,
        stop: stop - dll.0 as usize,
        property_log: property_log - dll.0 as usize,
        gateways,
    })
}

#[derive(Clone, Copy)]
struct BridgeImage {
    base: usize,
    size: usize,
    gateways: [u64; 4],
}

#[derive(Debug, PartialEq, Eq)]
struct PatchedEntries {
    appointment: [u8; 8],
    assignment: [u8; 8],
    property: [u8; 8],
    prefix: [u8; PROPERTY_PREFIX_LEN],
    promotion: [[u8; 16]; 2],
    promotion_boundary: [u8; 13],
    relays: [[u8; 14]; 5],
}

fn verify_relay(
    game: &Handle,
    entry: usize,
    branch: &[u8],
    bridge: BridgeImage,
    expected_gateway: u64,
) -> Result<[u8; 14], String> {
    if branch.len() < 5 || !matches!(branch[0], 0xe9 | 0xe8) {
        return Err("runtime branch missing".into());
    }
    let displacement = i32::from_le_bytes(branch[1..5].try_into().unwrap()) as isize;
    let relay = entry
        .checked_add(5)
        .and_then(|next| next.checked_add_signed(displacement))
        .ok_or("runtime relay displacement overflows")?;
    let mut code = [0u8; 14];
    read_remote(game, relay, &mut code)?;
    if code[..6] != [0xff, 0x25, 0, 0, 0, 0] {
        return Err("runtime displacement does not point to a published relay".into());
    }
    let gateway = u64::from_le_bytes(code[6..].try_into().unwrap()) as usize;
    if gateway < bridge.base || gateway - bridge.base >= bridge.size {
        return Err("runtime relay gateway is outside its pinned bridge DLL".into());
    }
    if gateway as u64 != expected_gateway {
        return Err("runtime relay gateway differs from the expected entry".into());
    }
    Ok(code)
}

fn patched_entries(
    game: &Handle,
    base: usize,
    can_original: [u8; 8],
    property_original: [u8; 8],
    assignment_original: [u8; 8],
    bridge: BridgeImage,
) -> Result<PatchedEntries, String> {
    let mut entries = [[0u8; 8]; 2];
    let mut relays = [[0u8; 14]; 5];
    for (slot, (rva, expected)) in [
        (PINNED_CAN_EVER_RVA, can_original),
        (
            civ6_native_bridge_rs::PINNED_CAN_ASSIGN_RVA,
            assignment_original,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut bytes = [0u8; 8];
        read_remote(game, base + rva, &mut bytes)?;
        if bytes[0] != 0xe9 || bytes[5..] != expected[5..] {
            return Err("runtime entry publication differs".into());
        }
        relays[slot] = verify_relay(
            game,
            base + rva,
            &bytes,
            bridge,
            bridge.gateways[if slot == 0 { 0 } else { 2 }],
        )?;
        entries[slot] = bytes;
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
    relays[2] = verify_relay(
        game,
        base + PINNED_PROPERTY_GET_RVA - PROPERTY_PREFIX_LEN,
        &prefix,
        bridge,
        bridge.gateways[1],
    )?;
    let mut promotion = [[0u8; 16]; 2];
    for (i, (rva, expected)) in civ6_native_bridge_rs::RUNTIME_PROMOTION_CALL_WINDOWS
        .into_iter()
        .enumerate()
    {
        let call = base
            .checked_add(rva)
            .ok_or("runtime promotion address overflows")?;
        read_remote(game, call, &mut promotion[i])?;
        if call.checked_add(1).is_none_or(|p| !p.is_multiple_of(4))
            || promotion[i][0] != 0xe8
            || promotion[i][5..] != expected[5..]
        {
            return Err("runtime promotion operand publication differs".into());
        }
        relays[3 + i] = verify_relay(game, call, &promotion[i], bridge, bridge.gateways[3])?;
    }
    let mut promotion_boundary = [0u8; 13];
    read_remote(
        game,
        base + civ6_native_bridge_rs::PINNED_PROMOTION_CALLEE_RVA - 5,
        &mut promotion_boundary,
    )?;
    if promotion_boundary
        != [
            0xe9, 0xd0, 0xfc, 0xff, 0xff, 0x40, 0x53, 0x56, 0x57, 0x48, 0x83, 0xec, 0x30,
        ]
    {
        return Err("runtime CanPromote callee boundary changed".into());
    }
    Ok(PatchedEntries {
        appointment: entries[0],
        assignment: entries[1],
        property: bytes,
        prefix,
        promotion,
        promotion_boundary,
        relays,
    })
}

pub(super) fn run(
    parsed: &Args,
    game: &Handle,
    chain: &Handle,
    chain_pid: u32,
    target: usize,
    original_entries: ([u8; 8], [u8; 8]),
) -> Result<(), String> {
    let (original, property_original) = original_entries;
    let base = target - PINNED_CAN_EVER_RVA;
    let assignment_original = pinned_entry_bytes(
        game,
        base + civ6_native_bridge_rs::PINNED_CAN_ASSIGN_RVA,
        BridgeMode::AssignCalibration,
    )?;
    let dll = bridge_dll()?;
    let bridge_sha = sha256(&dll)?;
    let exports = exports(&dll)?;
    let (install_rva, stop_rva) = (exports.install, exports.stop);
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
        if install_rva >= bridge.size
            || stop_rva >= bridge.size
            || exports.property_log + 8 > bridge.size
            || exports
                .gateways
                .iter()
                .any(|rva| rva.checked_add(8).is_none_or(|end| end > bridge.size))
        {
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
        let mut gateways = [0u64; 4];
        for (slot, rva) in gateways.iter_mut().zip(exports.gateways) {
            let mut bytes = [0u8; 8];
            read_remote(game, bridge.base + rva, &mut bytes)?;
            *slot = u64::from_le_bytes(bytes);
            if *slot < bridge.base as u64 || *slot - bridge.base as u64 >= bridge.size as u64 {
                return Err("runtime expected gateway export is outside loaded DLL".into());
            }
        }
        let image = BridgeImage {
            base: bridge.base,
            size: bridge.size,
            gateways,
        };
        let entries = patched_entries(
            game,
            base,
            original,
            property_original,
            assignment_original,
            image,
        )?;
        // Check the retained allocation is readable before ownership transfer.
        let mut header = [0u8; 8];
        read_remote(game, config.trace_address as usize, &mut header)?;
        write_json(
            &mut writer,
            json!({"kind":"runtime_install","status":status,
            "three_entries_published":true,"two_promotion_calls_published":true,"trace_address":config.trace_address,
            "trace_owner":"game_process","bounded_uninstall_allowed":false}),
        )?;
        job.disarm_after_verified_runtime_transfer()?;
        Ok((
            bridge.base + stop_rva,
            entries,
            image,
            config.trace_address as usize,
        ))
    })();
    let (stop, installed, image, trace_address) = match transfer {
        Ok(transfer) => transfer,
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
    if patched_entries(
        game,
        base,
        original,
        property_original,
        assignment_original,
        image,
    )? != installed
    {
        return Err("runtime branch bytes changed while observing; runtime retained".into());
    }
    // Both whole captures of the stopped prefix and of the separate property
    // log are preserved before any comparison or validation can fail.
    let mut native = vec![0; size_of::<civ6_native_bridge_rs::CompositeTrace>()];
    let mut again = vec![0; native.len()];
    read_remote(game, trace_address, &mut native)?;
    read_remote(game, trace_address, &mut again)?;
    for (extension, bytes) in [("native-first.bin", &native), ("native-second.bin", &again)] {
        preserve(&parsed.out, extension, bytes)?;
    }
    let log = property_log_pair(game, image.base + exports.property_log)?;
    for (extension, bytes) in [
        ("property-log-first.bin", &log.0),
        ("property-log-second.bin", &log.1),
    ] {
        preserve(&parsed.out, extension, bytes)?;
    }
    if native != again || log.0 != log.1 {
        return Err("stopped runtime observation is not stable; runtime retained".into());
    }
    let summary =
        civ6_bootstrap_rs::protocol::runtime_property(&native, &log.0, parsed.pid, parsed.created)?;
    write_json(
        &mut writer,
        json!({"kind":"property_log","rows":summary.rows,"dropped":0,"in_flight":0,
        "filtered":summary.filtered,"capacity":civ6_bootstrap_rs::protocol::PROPERTY_LOG_CAPACITY,
        "captures_equal":true}),
    )?;
    write_json(
        &mut writer,
        json!({"kind":"observation_stop","status":status,
        "runtime_retained":true,"three_entries_published":true,"two_promotion_calls_published":true,"trace_retained":true}),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_relay_displacement_and_exact_branch_snapshot_are_verified() {
        let game = open_process(unsafe { GetCurrentProcessId() }, PROCESS_VM_READ).unwrap();
        let mut core = vec![0u8; PINNED_PROPERTY_GET_RVA + 16];
        let base = core.as_ptr() as usize;
        let image = BridgeImage {
            base,
            size: 96,
            gateways: [
                (base + 1) as u64,
                (base + 2) as u64,
                (base + 3) as u64,
                (base + 4) as u64,
            ],
        };
        let originals = [
            [0x48, 0x89, 0x5c, 0x24, 0x08, 0x48, 0x89, 0x6c],
            [0x48, 0x8b, 0x41, 0x40, 0x4c, 0x8d, 0x49, 0x30],
        ];
        for offset in [0, 16, 32, 48, 64, 80] {
            core[offset..offset + 6].copy_from_slice(&[0xff, 0x25, 0, 0, 0, 0]);
            let gateway = image.gateways[match offset {
                16 => 2,
                32 => 1,
                64 | 80 => 3,
                _ => 0,
            }];
            core[offset + 6..offset + 14].copy_from_slice(&gateway.to_le_bytes());
        }
        let jump = |entry: usize, relay: usize| {
            let delta = i32::try_from(relay as i64 - (entry + 5) as i64).unwrap();
            let mut bytes = [0u8; 5];
            bytes[0] = 0xe9;
            bytes[1..].copy_from_slice(&delta.to_le_bytes());
            bytes
        };
        for (rva, relay) in [
            (PINNED_CAN_EVER_RVA, image.base),
            (
                civ6_native_bridge_rs::PINNED_CAN_ASSIGN_RVA,
                image.base + 16,
            ),
        ] {
            core[rva..rva + 8].copy_from_slice(&originals[0]);
            core[rva..rva + 5].copy_from_slice(&jump(base + rva, relay));
        }
        let property = PINNED_PROPERTY_GET_RVA;
        core[property..property + 8].copy_from_slice(&originals[1]);
        core[property..property + 2].copy_from_slice(&[0xeb, 0xf9]);
        core[property - 5..property].copy_from_slice(&jump(base + property - 5, image.base + 32));
        for (rva, original) in civ6_native_bridge_rs::RUNTIME_PROMOTION_CALL_WINDOWS {
            core[rva..rva + 16].copy_from_slice(&original);
            core[rva + 1..rva + 5].copy_from_slice(&jump(base + rva, image.base + 64)[1..]);
        }
        let boundary = civ6_native_bridge_rs::PINNED_PROMOTION_CALLEE_RVA - 5;
        core[boundary..boundary + 13].copy_from_slice(&[
            0xe9, 0xd0, 0xfc, 0xff, 0xff, 0x40, 0x53, 0x56, 0x57, 0x48, 0x83, 0xec, 0x30,
        ]);
        let capture =
            || patched_entries(&game, base, originals[0], originals[1], originals[0], image);
        let installed = capture().unwrap();
        assert_eq!(capture().unwrap(), installed);
        let (ai, original) = civ6_native_bridge_rs::RUNTIME_PROMOTION_CALL_WINDOWS[1];
        core[ai + 1..ai + 5].copy_from_slice(&jump(base + ai, image.base + 80)[1..]);
        assert_ne!(
            capture().unwrap(),
            installed,
            "promotion operand changes must invalidate the stop snapshot"
        );
        core[ai + 1..ai + 5].copy_from_slice(&jump(base + ai, image.base + 64)[1..]);
        core[ai + 5] ^= 1;
        assert!(capture().unwrap_err().contains("promotion operand"));
        core[ai + 5] = original[5];
        core[boundary + 5] ^= 1;
        assert!(capture().unwrap_err().contains("callee boundary"));
        core[boundary + 5] ^= 1;
        // Change only rel32, leaving E9 and the original tail intact. Even a
        // second valid relay must not pass the stop snapshot equality check.
        core[PINNED_CAN_EVER_RVA..PINNED_CAN_EVER_RVA + 5]
            .copy_from_slice(&jump(base + PINNED_CAN_EVER_RVA, image.base + 48));
        assert_ne!(capture().unwrap(), installed);
        core[PINNED_CAN_EVER_RVA..PINNED_CAN_EVER_RVA + 5]
            .copy_from_slice(&jump(base + PINNED_CAN_EVER_RVA, image.base + 1));
        assert!(capture().unwrap_err().contains("published relay"));
        core[PINNED_CAN_EVER_RVA..PINNED_CAN_EVER_RVA + 5]
            .copy_from_slice(&jump(base + PINNED_CAN_EVER_RVA, image.base));
        core[6..14].copy_from_slice(&0u64.to_le_bytes());
        assert!(capture()
            .unwrap_err()
            .contains("outside its pinned bridge DLL"));
        core[6..14].copy_from_slice(&image.gateways[1].to_le_bytes());
        assert!(capture()
            .unwrap_err()
            .contains("differs from the expected entry"));
    }
}
