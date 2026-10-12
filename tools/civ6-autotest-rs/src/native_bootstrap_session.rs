// Included inside native_bridge::platform to reuse its pinned process handle,
// child transport, stop/reap and failure cleanup rather than clone a controller.
fn bootstrap_terminal(path: &Path, pid: u32, created: u64, root_only: bool) -> Result<()> {
    if root_only {
        return root_bootstrap_terminal(path, pid, created);
    }
    let bytes = std::fs::read(path.with_extension("bootstrap.bin"))?;
    civ6_bootstrap_rs::protocol::trace(&bytes, pid, created).map_err(Error::input)?;
    // Do not rely on the controller receipt alone: the stopped runtime prefix
    // needs both preserved whole captures of its separate property log.
    let log = std::fs::read(path.with_extension("property-log-first.bin"))?;
    if log != std::fs::read(path.with_extension("property-log-second.bin"))? {
        return Err(Error::input("bootstrap property log captures differ"));
    }
    let native = std::fs::read(path.with_extension("runtime.bin"))?;
    civ6_bootstrap_rs::protocol::native_trace(&native, Some(&log), pid, created)
        .map_err(Error::input)?;
    let text = std::fs::read_to_string(path)?;
    let row: Value = serde_json::from_str(
        text.lines()
            .last()
            .ok_or_else(|| Error::input("bootstrap terminal missing"))?,
    )?;
    if row["type"] != "bootstrap_stop"
        || row["pid"] != pid
        || row["created_filetime"] != created
        || row["observation_stopped"] != true
        || row["resident_code_retained"] != true
    {
        return Err(Error::input("bootstrap stop identity/receipt differs"));
    }
    Ok(())
}

fn root_bootstrap_terminal(path: &Path, pid: u32, created: u64) -> Result<()> {
    // The existing Session owns both profiles. Its exact saved request selects
    // the terminal; a submission trace cannot satisfy the old root certificate.
    let submission = match std::fs::read(path.with_extension("request.json")) {
        Ok(bytes) => {
            civ6_bootstrap_rs::protocol::Request::parse(&serde_json::from_slice::<Value>(&bytes)?)
                .map_err(Error::input)?
                .submission_only
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(e.into()),
    };
    let first = std::fs::read(path.with_extension(if submission {
        "submission-first.bin"
    } else {
        "root-first.bin"
    }))?;
    let second = std::fs::read(path.with_extension(if submission {
        "submission-second.bin"
    } else {
        "root-second.bin"
    }))?;
    if first != second {
        return Err(Error::input("root observation raw copies differ"));
    }
    let trace = if submission {
        civ6_bootstrap_rs::protocol::submission_trace(&first, pid, created)
    } else {
        civ6_bootstrap_rs::protocol::root_trace(&first, pid, created)
    }
    .map_err(Error::input)?;
    if trace["capture_complete"] != true {
        return Err(Error::input(
            "root observation lacks a complete matching Wait return",
        ));
    }
    let text = std::fs::read_to_string(path)?;
    let rows: Vec<Value> = text
        .lines()
        .filter(|s| !s.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    let terminal = rows
        .last()
        .ok_or_else(|| Error::input("root terminal missing"))?;
    if terminal["type"]
        != if submission {
            "native_submission_stop"
        } else {
            "root_observation_stop"
        }
        || terminal["pid"] != pid
        || terminal["created_filetime"] != created
        || terminal["observation_stopped"] != true
        || terminal["resident_code_retained"] != true
    {
        return Err(Error::input("root stop identity/receipt differs"));
    }
    let receipts: Vec<_> = rows
        .iter()
        .filter(|v| {
            v["schema"]
                == if submission {
                    "todo880-native-submission-controller-receipt-v1"
                } else {
                    "todo880-root-controller-receipt-v1"
                }
        })
        .collect();
    if receipts.len() != 1 {
        return Err(Error::input("root receipt missing or duplicated"));
    }
    let receipt = receipts[0];
    if submission {
        let independent: Value = serde_json::from_slice(&std::fs::read(
            path.with_extension("submission-receipt.json"),
        )?)?;
        if independent != *receipt {
            return Err(Error::input("independent capture receipt differs"));
        }
    }
    if receipt["pid"] != pid
        || receipt["created_filetime"] != created
        || receipt["status"] != "captured"
        || receipt["stop_response_succeeded"] != true
        || receipt["copies_equal"] != true
        || receipt["trace"] != trace
    {
        return Err(Error::input("root receipt does not match original samples"));
    }
    Ok(())
}
impl Session {
    pub fn start_bootstrap(
        pid: u32,
        started: f64,
        output: &Path,
        step: &Value,
        close_game: Box<dyn FnMut(u32, f64) -> Result<()> + Send>,
        preparation_deadline: std::time::Instant,
    ) -> Result<Self> {
        if std::time::Instant::now() >= preparation_deadline {
            return Err(Error::scenario(
                "bootstrap preparation deadline expired before process pin",
            ));
        }
        let (created, target) = pinned_created(pid, started)?;
        let name = step["name"]
            .as_str()
            .ok_or_else(|| Error::scenario("bootstrap name missing"))?;
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err(Error::scenario(
                "bootstrap artifact name must be a simple identifier",
            ));
        }
        let artifact = output.join(format!("{name}.jsonl"));
        let request = civ6_bootstrap_rs::protocol::scenario_request(step, pid, created, &artifact);
        let root_only =
            civ6_bootstrap_rs::protocol::Request::root_scenario(step).map_err(Error::scenario)?;
        civ6_bootstrap_rs::protocol::Request::parse(&request).map_err(Error::scenario)?;
        if artifact.exists() {
            return Err(Error::scenario("bootstrap artifact exists"));
        }
        let request_path = artifact.with_extension("request.json");
        serde_json::to_writer(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&request_path)?,
            &request,
        )?;
        let executable = std::env::current_exe()?.with_file_name("civ6-startup-diagnostics.exe");
        // Keep the exact helper and both explicit DLLs immutable while the
        // shared Session owns startup, stop and failure recovery.
        use std::os::windows::fs::OpenOptionsExt;
        let parsed =
            civ6_bootstrap_rs::protocol::Request::parse(&request).map_err(Error::scenario)?;
        let mut binary_pins = Vec::new();
        for (path, expected) in [
            (&executable, None),
            (&parsed.runtime, Some(parsed.runtime_sha.as_str())),
            (&parsed.bootstrap, Some(parsed.bootstrap_sha.as_str())),
        ] {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(path)?;
            let actual = format!("{:x}", Sha256::digest(std::fs::read(path)?));
            if let Some(expected) = expected {
                verify_override_hash(Some(expected), &actual)?;
            }
            binary_pins.push(file);
        }
        // File validation belongs to the same preparation budget as READY.
        // If it consumed the budget, never spawn an injecting controller.
        if std::time::Instant::now() >= preparation_deadline {
            return Err(Error::scenario(
                "bootstrap preparation deadline expired before controller spawn",
            ));
        }
        let child = Command::new(executable)
            .arg("bootstrap")
            .arg("--request")
            .arg(request_path)
            .env(
                "CIV6_AUTOTEST_CONTROLLER_PID",
                std::process::id().to_string(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?;
        let mut session = Self::from_child(
            child,
            artifact,
            pid,
            created,
            started,
            target,
            BridgeMode::ProductionAdmission,
            close_game,
            false,
            binary_pins,
            true,
            Some(preparation_deadline),
        )?;
        session.bootstrap_root = root_only;
        Ok(session)
    }
    pub fn bootstrap_healthy(&self) -> Result<()> {
        if !self.bootstrap
            || self.finished
            || unsafe { WaitForSingleObject(self.target.0, 0) } != WAIT_TIMEOUT
            || unsafe { WaitForSingleObject(self.child.as_raw_handle() as HANDLE, 0) }
                != WAIT_TIMEOUT
        {
            return Err(Error::input("bootstrap controller or pinned game stopped"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod root_terminal_tests {
    use super::*;
    use civ6_bootstrap_rs::protocol as wire;

    fn fixture(path: &Path) -> (Vec<u8>, Vec<Value>) {
        let core = 0x6000_0000;
        let world = 0x1234_0000;
        let manager = 0x2345_0000;
        let mut b = vec![0; wire::ROOT_BYTES];
        let put = |b: &mut [u8], offset: usize, value: u64| {
            b[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        };
        for (offset, value) in [
            (0, wire::ROOT_MAGIC),
            (8, wire::ROOT_VERSION),
            (16, wire::ROOT_BYTES as u64),
            (24, wire::ROOT_ROWS as u64),
            (32, wire::ROOT_ROW as u64),
            (40, 23),
            (48, 29),
            (56, core),
            (72, 9),
            (80, 6),
            (96, wire::ROOT_CLOSED),
            (120, 7),
            (184, 2),
            (152, 3),
            (192, 6),
        ] {
            put(&mut b, offset, value);
        }
        for (i, rva, raw) in [
            (
                0,
                0x63e160,
                [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x80, 0xb9],
            ),
            (
                1,
                0x4a1150,
                [0x48, 0x89, 0x5c, 0x24, 0x10, 0x56, 0x48, 0x83],
            ),
            (
                2,
                0x9922c0,
                [0x48, 0x89, 0x5c, 0x24, 0x08, 0x57, 0x48, 0x83],
            ),
        ] {
            for (field, value) in [
                (0, rva),
                (1, u64::from_le_bytes(raw)),
                (2, u64::from_le_bytes(raw)),
                (3, 1),
                (4, 1),
            ] {
                put(&mut b, 256 + i * 64 + field * 8, value);
            }
        }
        for (index, kind, edge, invocation, parent, event) in [
            (0, 1, 1, 1, 0, 1),
            (1, 1, 2, 1, 0, 6),
            (2, 2, 1, 3, 1, 2),
            (3, 2, 2, 3, 1, 5),
            (4, 3, 1, 5, 3, 3),
            (5, 3, 2, 5, 3, 4),
        ] {
            let (caller, rsp, native, rbx) = match kind {
                1 => (core + 0x471c2, 0x8100, 0x8000, 0x9999),
                2 => (core + 0x63e1d5, 0x7fd0, 0x7e00, world),
                _ => (core + 0x4a1196, 0x7dc0, 0x7ca0, manager),
            };
            for (field, value) in [
                (0, index as u64 + 1),
                (1, kind),
                (2, edge),
                (3, invocation),
                (4, parent),
                (5, 123),
                (6, 111),
                (7, 9),
                (8, caller),
                (9, rsp),
                (10, world),
                (11, manager),
                (12, manager + 0x400),
                (21, native),
                (22, rbx),
                (23, event),
            ] {
                put(
                    &mut b,
                    wire::ROOT_HEADER + index * wire::ROOT_ROW + field * 8,
                    value,
                );
            }
        }
        let trace = wire::root_trace(&b, 23, 29).unwrap();
        assert_eq!(trace["capture_complete"], true);
        let rows = vec![
            serde_json::json!({"schema":"todo880-root-controller-receipt-v1",
            "pid":23,"created_filetime":29,"status":"captured","stop_response_succeeded":true,
            "copies_equal":true,"trace":trace}),
            serde_json::json!({"type":"root_observation_stop",
            "pid":23,"created_filetime":29,"observation_stopped":true,"resident_code_retained":true}),
        ];
        std::fs::write(path.with_extension("root-first.bin"), &b).unwrap();
        std::fs::write(path.with_extension("root-second.bin"), &b).unwrap();
        save(path, &rows);
        (b, rows)
    }
    fn save(path: &Path, rows: &[Value]) {
        std::fs::write(
            path,
            rows.iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        )
        .unwrap();
    }
    fn submission_fixture(path: &Path) -> Vec<Value> {
        let mut b = vec![0; wire::SUBMISSION_V1_BYTES];
        let put = |b: &mut [u8], word: usize, v: u64| {
            b[word * 8..word * 8 + 8].copy_from_slice(&v.to_le_bytes())
        };
        for (i, v) in [
            wire::SUBMISSION_MAGIC,
            1,
            wire::SUBMISSION_V1_BYTES as u64,
            128,
            256,
            23,
            29,
            0x180000000,
            0x140000000,
            1,
            4,
            0,
            wire::ROOT_CLOSED,
            0,
            0,
            3,
            0,
            0,
            0,
            3,
            0,
            0,
            0,
            2,
            4,
            3,
            3,
            1,
        ]
        .into_iter()
        .enumerate()
        {
            put(&mut b, i, v);
        }
        for (i, (rva, bytes)) in [
            (0x992210, [0x48, 0x89, 0x5c, 0x24, 8, 0x48, 0x89, 0x6c]),
            (0x992280, [0x40, 0x53, 0x48, 0x83, 0xec, 0x20, 0x48, 0x8b]),
        ]
        .into_iter()
        .enumerate()
        {
            for (j, v) in [
                rva,
                u64::from_le_bytes(bytes),
                u64::from_le_bytes(bytes),
                1,
                1,
            ]
            .into_iter()
            .enumerate()
            {
                put(&mut b, 32 + i * 8 + j, v);
            }
        }
        for i in 0..4 {
            let mut r = [0; 32];
            r[0] = i + 1;
            r[1] = if i < 2 { 4 } else { 5 };
            r[2] = if i % 2 == 0 { 1 } else { 2 };
            r[3] = if i < 2 { 1 } else { 3 };
            r[5] = 31;
            r[6] = 37;
            r[7] = 1;
            r[8] = 0x180000000 + if i < 2 { 0x388871 } else { 0x38888e };
            r[9] = 0x1000;
            r[10] = 7;
            if i < 2 {
                r[11] = 0x10000;
                r[12] = 0x20000;
                r[13] = 0x30000;
                r[24] = 32;
                r[26] = 16;
                r[30] = 1;
                r[31] = 1;
            }
            r[23] = i + 1;
            r[29] = 0xe30;
            for (j, v) in r.into_iter().enumerate() {
                put(&mut b, wire::ROOT_HEADER / 8 + i as usize * 32 + j, v);
            }
        }
        let request = serde_json::json!({"schema":"todo880-native-submission-controller-v1","pid":23,
            "created_filetime":29,"max_seconds":30,"controller_seconds":60,
            "runtime_dll":path,"runtime_sha256":"11".repeat(32),"bootstrap_dll":path,
            "bootstrap_sha256":"22".repeat(32),"output":path});
        std::fs::write(path.with_extension("request.json"), request.to_string()).unwrap();
        let trace = wire::submission_trace(&b, 23, 29).unwrap();
        assert_eq!(trace["capture_complete"], true);
        let rows = vec![
            serde_json::json!({"schema":"todo880-native-submission-controller-receipt-v1",
            "pid":23,"created_filetime":29,"status":"captured","stop_response_succeeded":true,"copies_equal":true,"trace":trace}),
            serde_json::json!({"type":"native_submission_stop","pid":23,"created_filetime":29,"observation_stopped":true,"resident_code_retained":true}),
        ];
        std::fs::write(path.with_extension("submission-first.bin"), &b).unwrap();
        std::fs::write(path.with_extension("submission-second.bin"), &b).unwrap();
        std::fs::write(
            path.with_extension("submission-receipt.json"),
            rows[0].to_string(),
        )
        .unwrap();
        save(path, &rows);
        rows
    }
    #[test]
    fn submission_terminal_requires_its_saved_request_raw_bytes_and_independent_receipt() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("submission.jsonl");
        let rows = submission_fixture(&path);
        bootstrap_terminal(&path, 23, 29, true).unwrap();
        assert!(!path.with_extension("runtime.bin").exists());
        assert!(!path.with_extension("root-first.bin").exists());
        let mut changed = rows.clone();
        changed[1]["type"] = serde_json::json!("root_observation_stop");
        save(&path, &changed);
        assert!(bootstrap_terminal(&path, 23, 29, true).is_err());
        save(&path, &rows);
        let mut different = rows[0].clone();
        different["stop_response_succeeded"] = serde_json::json!(false);
        std::fs::write(
            path.with_extension("submission-receipt.json"),
            different.to_string(),
        )
        .unwrap();
        assert!(bootstrap_terminal(&path, 23, 29, true).is_err());
    }

    #[test]
    fn original_root_copies_and_one_matching_terminal_are_required_without_runtime_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("root.jsonl");
        let (bytes, rows) = fixture(&path);
        bootstrap_terminal(&path, 23, 29, true).unwrap();
        assert!(!path.with_extension("runtime.bin").exists());
        assert!(bootstrap_terminal(&path, 23, 29, false).is_err());
        assert!(bootstrap_terminal(&path, 24, 29, true).is_err());
        for (index, field, value) in [
            (0, "pid", serde_json::json!(24)),
            (0, "status", serde_json::json!("unknown")),
            (0, "copies_equal", serde_json::json!(false)),
            (0, "stop_response_succeeded", serde_json::json!(false)),
            (1, "created_filetime", serde_json::json!(30)),
            (1, "resident_code_retained", serde_json::json!(false)),
            (1, "type", serde_json::json!("bootstrap_stop")),
        ] {
            let mut changed = rows.clone();
            changed[index][field] = value;
            save(&path, &changed);
            assert!(bootstrap_terminal(&path, 23, 29, true).is_err(), "{field}");
        }
        let mut duplicate = rows.clone();
        duplicate.insert(0, rows[0].clone());
        save(&path, &duplicate);
        assert!(bootstrap_terminal(&path, 23, 29, true).is_err());
        save(&path, &rows);
        let mut different = bytes.clone();
        different[wire::ROOT_HEADER + 18 * 8] ^= 1;
        std::fs::write(path.with_extension("root-second.bin"), different).unwrap();
        assert!(bootstrap_terminal(&path, 23, 29, true).is_err());
        std::fs::write(
            path.with_extension("root-second.bin"),
            &bytes[..bytes.len() - 1],
        )
        .unwrap();
        assert!(bootstrap_terminal(&path, 23, 29, true).is_err());
    }
}
