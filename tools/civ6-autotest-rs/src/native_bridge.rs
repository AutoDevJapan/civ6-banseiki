//! Bounded, separate native bridge telemetry. This never routes through the
//! read-only hardware-breakpoint observer or claims admission is working.
use crate::{Error, Result};
use std::path::{Path, PathBuf};

#[cfg(windows)]
mod platform {
    use super::*;
    use civ6_native_bridge_rs::BridgeMode;
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use std::ffi::OsString;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::windows::{ffi::OsStringExt, io::AsRawHandle, process::CommandExt};
    use std::process::{Child, ChildStdin, Command, Stdio};
    use std::sync::mpsc::{self, Receiver};
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{
        CloseHandle, FILETIME, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W, TH32CS_SNAPMODULE,
        TH32CS_SNAPMODULE32,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, TerminateProcess,
        WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        PROCESS_TERMINATE,
    };

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn terminal_artifact(path: &Path, pid: u32, created: u64, mode: BridgeMode) -> Result<usize> {
        let mut identity = 0usize;
        let mut installed = 0usize;
        let mut restored = 0usize;
        let mut stopped = 0usize;
        let mut entries = 0usize;
        let mut partial_dropped = None;
        for line in BufReader::new(std::fs::File::open(path)?).lines() {
            let row: Value = serde_json::from_str(&line?)?;
            if stopped != 0 {
                return Err(Error::input(
                    "native bridge wrote after its terminal record",
                ));
            }
            match row["kind"].as_str() {
                Some("identity")
                    if row["pid"] == pid
                        && row["created_filetime"] == created
                        && row["mode"]
                            == if mode == BridgeMode::CanEver {
                                "can-ever"
                            } else {
                                "property"
                            }
                        && row["entry_rva"] == mode.rva()
                        && (mode == BridgeMode::CanEver
                            || row["prefix_bytes"] == "[cc, cc, cc, cc, cc]")
                        && row["gamecore_sha256"]
                            == civ6_native_bridge_rs::PINNED_GAMECORE_SHA256 =>
                {
                    identity += 1
                }
                Some("install") if row["status"] == 0 => installed += 1,
                Some("uninstall")
                    if row["status"] == 0
                        && row["entry_restored"] == true
                        && row["prefix_retained_until_exit"] == (mode == BridgeMode::Property) =>
                {
                    restored += 1
                }
                Some("entry")
                    if mode == BridgeMode::CanEver
                        && row["sequence"].as_u64() == Some(entries as u64 + 1) =>
                {
                    entries += 1
                }
                Some("property_get")
                    if mode == BridgeMode::Property
                        && row["sequence"].as_u64() == Some(entries as u64 + 1)
                        && row["property"]
                            .as_str()
                            .is_some_and(|s| s.starts_with("0x") && s.len() == 18)
                        && row["returned"]
                            .as_str()
                            .is_some_and(|s| s.starts_with("0x") && s.len() == 18)
                        && row["key_id"].as_u64().is_some_and(|n| n <= u32::MAX as u64)
                        && row["thread_id"]
                            .as_u64()
                            .is_some_and(|n| n > 0 && n <= u32::MAX as u64)
                        && row["read_state"].as_u64().is_some_and(|n| n <= 2) =>
                {
                    let returned = row["returned"].as_str().unwrap();
                    if row["read_state"] == 0 {
                        if returned != "0x0000000000000000"
                            || !row["tag"].is_null()
                            || !row["payload8"].is_null()
                            || !row["flags14"].is_null()
                        {
                            return Err(Error::input("invalid missing property row"));
                        }
                    } else if row["read_state"] == 1 {
                        if returned == "0x0000000000000000"
                            || !row["tag"].as_u64().is_some_and(|n| n <= u16::MAX as u64)
                            || !row["payload8"]
                                .as_str()
                                .is_some_and(|s| s.starts_with("0x") && s.len() == 18)
                            || !row["flags14"]
                                .as_u64()
                                .is_some_and(|n| n <= u32::MAX as u64)
                        {
                            return Err(Error::input("invalid copied property row"));
                        }
                    } else if returned == "0x0000000000000000"
                        || !row["tag"].is_null()
                        || !row["payload8"].is_null()
                        || !row["flags14"].is_null()
                    {
                        return Err(Error::input("invalid unreadable property row"));
                    }
                    entries += 1;
                }
                Some("stop")
                    if row["rows"] == entries
                        && row["in_flight"] == 0
                        && row["snapshot_stable"] == true
                        && row["observed_calls"].as_u64().is_some_and(|n| {
                            n == entries as u64 + row["dropped"].as_u64().unwrap_or(u64::MAX)
                        })
                        && (if mode == BridgeMode::CanEver {
                            row["result"] == "ok"
                                && row["dropped"] == 0
                                && row["classification"] == "can_ever_entry"
                                && row["prefix_retained_until_exit"] == false
                        } else {
                            row["classification"] == "raw_collection"
                                && row["prefix_retained_until_exit"] == true
                                && row["quiescence_proven"] == false
                                && row["dropped"]
                                    .as_u64()
                                    .is_some_and(|n| n <= u32::MAX as u64)
                                && (if row["dropped"] == 0 {
                                    row["result"] == "raw_collected"
                                } else {
                                    row["result"] == "partial_raw"
                                })
                        }) =>
                {
                    if mode == BridgeMode::Property {
                        partial_dropped = row["dropped"].as_u64();
                    }
                    stopped += 1
                }
                _ => {
                    return Err(Error::input(
                        "native bridge artifact has an invalid proof row",
                    ))
                }
            }
        }
        if identity != 1 || installed != 1 || restored != 1 || stopped != 1 || entries == 0 {
            return Err(Error::input(
                "native bridge terminal proof or caller evidence missing",
            ));
        }
        if let Some(dropped) = partial_dropped.filter(|n| *n > 0) {
            return Err(Error::input(format!(
                "native property raw collection is partial: {entries} rows, {dropped} dropped; inspect {}",
                path.display()
            )));
        }
        Ok(entries)
    }

    struct PinnedTarget(HANDLE);
    // Kernel process/snapshot handles are usable from any thread; this wrapper
    // owns the sole CloseHandle and never exposes a borrowed raw handle.
    unsafe impl Send for PinnedTarget {}
    impl Drop for PinnedTarget {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }

    fn same_file(actual: &Path, expected: &str) -> bool {
        actual
            .canonicalize()
            .ok()
            .zip(Path::new(expected).canonicalize().ok())
            .is_some_and(|(a, b)| {
                a.to_string_lossy()
                    .eq_ignore_ascii_case(&b.to_string_lossy())
            })
    }

    fn pinned_created(pid: u32, expected: f64) -> Result<(u64, PinnedTarget)> {
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE | PROCESS_TERMINATE,
                0,
                pid,
            )
        };
        if handle.is_null() {
            return Err(Error::input("native bridge game process unavailable"));
        }
        let handle = PinnedTarget(handle);
        let mut a = FILETIME::default();
        let mut b = FILETIME::default();
        let mut c = FILETIME::default();
        let mut d = FILETIME::default();
        let ok = unsafe { GetProcessTimes(handle.0, &mut a, &mut b, &mut c, &mut d) } != 0;
        let mut image = vec![0u16; 32768];
        let mut len = image.len() as u32;
        let path_ok =
            unsafe { QueryFullProcessImageNameW(handle.0, 0, image.as_mut_ptr(), &mut len) } != 0;
        if !ok || !path_ok || len == 0 || len as usize >= image.len() {
            return Err(Error::input("native bridge game identity unreadable"));
        }
        let created = (u64::from(a.dwHighDateTime) << 32) | u64::from(a.dwLowDateTime);
        let unix = created as f64 / 10_000_000.0 - 11_644_473_600.0;
        let path = PathBuf::from(OsString::from_wide(&image[..len as usize]));
        if unix != expected || !same_file(&path, civ6_native_bridge_rs::GAME_EXE) {
            return Err(Error::input("native bridge pinned game identity changed"));
        }
        Ok((created, handle))
    }

    fn pinned_module(pid: u32) -> Result<()> {
        let snapshot =
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) };
        if snapshot.is_null() || snapshot == INVALID_HANDLE_VALUE {
            return Err(Error::input("pinned GameCore module snapshot unavailable"));
        }
        let snapshot = PinnedTarget(snapshot);
        let mut entry: MODULEENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;
        if unsafe { Module32FirstW(snapshot.0, &mut entry) } == 0 {
            return Err(Error::input("pinned GameCore module list unavailable"));
        }
        let found = loop {
            let end = entry
                .szExePath
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExePath.len());
            if same_file(
                &PathBuf::from(OsString::from_wide(&entry.szExePath[..end])),
                civ6_native_bridge_rs::GAMECORE,
            ) && entry.modBaseSize as usize > civ6_native_bridge_rs::PINNED_CAN_EVER_RVA + 8
            {
                break true;
            }
            if unsafe { Module32NextW(snapshot.0, &mut entry) } == 0 {
                break false;
            }
        };
        if !found {
            return Err(Error::input("pinned GameCore module changed before close"));
        }
        let bytes = std::fs::read(civ6_native_bridge_rs::GAMECORE)?;
        if format!("{:x}", Sha256::digest(bytes)) != civ6_native_bridge_rs::PINNED_GAMECORE_SHA256 {
            return Err(Error::input("pinned GameCore hash changed before close"));
        }
        Ok(())
    }

    fn finish_failed_cleanup(
        closure: Result<()>,
        target: &PinnedTarget,
        controller: &mut Child,
    ) -> Result<()> {
        if unsafe { WaitForSingleObject(target.0, 0) } != WAIT_OBJECT_0 {
            return Err(Error::input(format!(
                "pinned target is still running; controller retained; close={closure:?}"
            )));
        }
        if controller.try_wait()?.is_none() {
            controller.kill()?;
            controller.wait()?;
        }
        closure
    }

    /// Close only the PID with the exact creation FILETIME, DX11 image and pinned XP2 module.
    pub fn close_pinned_game(pid: u32, started: f64) -> Result<()> {
        let (created, inspected) = pinned_created(pid, started)?;
        if unsafe { WaitForSingleObject(inspected.0, 0) } == WAIT_OBJECT_0 {
            return Ok(());
        }
        pinned_module(pid)?;
        let mut a = FILETIME::default();
        let mut b = FILETIME::default();
        let mut c = FILETIME::default();
        let mut d = FILETIME::default();
        if unsafe { GetProcessTimes(inspected.0, &mut a, &mut b, &mut c, &mut d) } == 0
            || ((u64::from(a.dwHighDateTime) << 32) | u64::from(a.dwLowDateTime)) != created
        {
            return Err(Error::input(
                "pinned game creation time changed before close",
            ));
        }
        if unsafe { TerminateProcess(inspected.0, 1) } == 0
            && unsafe { WaitForSingleObject(inspected.0, 0) } != WAIT_OBJECT_0
        {
            return Err(Error::input("pinned game termination failed"));
        }
        if unsafe { WaitForSingleObject(inspected.0, 10_000) } != WAIT_OBJECT_0 {
            return Err(Error::input("pinned game exit not confirmed"));
        }
        Ok(())
    }

    pub struct Session {
        child: Child,
        input: Option<ChildStdin>,
        lines: Receiver<String>,
        artifact: PathBuf,
        pid: u32,
        created: u64,
        started: f64,
        target: PinnedTarget,
        close_game: Box<dyn FnMut(u32, f64) -> Result<()> + Send>,
        finished: bool,
        mode: BridgeMode,
    }

    impl Session {
        pub fn start(
            pid: u32,
            started: f64,
            output: &Path,
            name: &str,
            seconds: u64,
            mode: &str,
            mut close_game: Box<dyn FnMut(u32, f64) -> Result<()> + Send>,
        ) -> Result<Self> {
            let mode = match mode {
                "can-ever" => BridgeMode::CanEver,
                "property" => BridgeMode::Property,
                _ => return Err(Error::scenario("unknown native bridge mode")),
            };
            if !(1..=u64::from(civ6_native_bridge_rs::MAX_ACTIVE_SECONDS)).contains(&seconds) {
                return Err(Error::scenario(
                    "native bridge duration must be 1..300 seconds",
                ));
            }
            let (created, target) = pinned_created(pid, started)?;
            let executable = std::env::current_exe()?.with_file_name("civ6-native-bridge.exe");
            let dll = executable.with_file_name("civ6_native_bridge_rs.dll");
            if !executable.is_file() || !dll.is_file() {
                return Err(Error::input(
                    "native bridge controller or DLL missing beside runner",
                ));
            }
            let artifact = output.join(format!("{name}.jsonl"));
            if artifact.exists() {
                return Err(Error::scenario("native bridge artifact already exists"));
            }
            let mut child = Command::new(executable)
                .args([
                    "--pid",
                    &pid.to_string(),
                    "--created-filetime",
                    &created.to_string(),
                    "--duration",
                    &seconds.to_string(),
                    "--out",
                    &artifact.to_string_lossy(),
                    "--mode",
                    if mode == BridgeMode::Property {
                        "property"
                    } else {
                        "can-ever"
                    },
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .env(
                    "CIV6_AUTOTEST_CONTROLLER_PID",
                    std::process::id().to_string(),
                )
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()?;
            let input = child.stdin.take();
            let Some(stdout) = child.stdout.take() else {
                let close = close_game(pid, started);
                if unsafe { WaitForSingleObject(target.0, 10_000) } == WAIT_OBJECT_0 {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                return Err(Error::input(format!(
                    "bridge stdout unavailable; pinned game close={close:?}; exit_confirmed={}",
                    unsafe { WaitForSingleObject(target.0, 0) } == WAIT_OBJECT_0,
                )));
            };
            if input.is_none() {
                let close = close_game(pid, started);
                if unsafe { WaitForSingleObject(target.0, 10_000) } == WAIT_OBJECT_0 {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                return Err(Error::input(format!(
                    "bridge stop pipe unavailable; pinned game close={close:?}; exit_confirmed={}",
                    unsafe { WaitForSingleObject(target.0, 0) } == WAIT_OBJECT_0,
                )));
            }
            let (sender, lines) = mpsc::channel();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if sender.send(line).is_err() {
                        break;
                    }
                }
            });
            let mut session = Self {
                child,
                input,
                lines,
                artifact,
                pid,
                created,
                started,
                target,
                close_game,
                finished: false,
                mode,
            };
            match session.lines.recv_timeout(Duration::from_secs(30)) {
                Ok(line) if line.starts_with(&format!("READY pid={pid} ")) => Ok(session),
                _ => {
                    let close = session.close_target();
                    if unsafe { WaitForSingleObject(session.target.0, 0) } == WAIT_OBJECT_0 {
                        let _ = session.child.kill();
                        let _ = session.child.wait();
                        session.finished = true;
                    }
                    Err(Error::input(format!(
                        "native bridge did not prove bounded readiness; pinned game close={close:?}"
                    )))
                }
            }
        }

        fn close_target(&mut self) -> Result<()> {
            if unsafe { WaitForSingleObject(self.target.0, 0) } == WAIT_OBJECT_0 {
                return Ok(());
            }
            let callback = (self.close_game)(self.pid, self.started);
            let exited = unsafe { WaitForSingleObject(self.target.0, 10_000) } == WAIT_OBJECT_0;
            if !exited {
                return Err(Error::input(format!(
                    "pinned game closure incomplete: callback={callback:?}; target still alive"
                )));
            }
            callback
        }

        fn stop_inner(&mut self) -> Result<PathBuf> {
            if let Some(mut input) = self.input.take() {
                input.write_all(b"stop\n")?;
                input.flush()?;
            }
            let waited =
                unsafe { WaitForSingleObject(self.child.as_raw_handle() as HANDLE, 12_000) };
            if waited != WAIT_OBJECT_0 {
                return Err(Error::input("native bridge stop timed out"));
            }
            let status = self.child.wait()?;
            let mut stderr = String::new();
            if let Some(mut stream) = self.child.stderr.take() {
                let _ = stream.read_to_string(&mut stderr);
            }
            if !status.success() {
                return Err(Error::input(format!(
                    "native bridge failed: {}",
                    stderr.trim()
                )));
            }
            terminal_artifact(&self.artifact, self.pid, self.created, self.mode)?;
            Ok(self.artifact.clone())
        }

        pub fn stop(&mut self) -> Result<PathBuf> {
            if self.finished {
                return Err(Error::scenario("native bridge already stopped"));
            }
            let outcome = self.stop_inner();
            if let Err(original) = outcome {
                let closure = self.close_target();
                let cleanup = finish_failed_cleanup(closure, &self.target, &mut self.child);
                if unsafe { WaitForSingleObject(self.target.0, 0) } == WAIT_OBJECT_0
                    && self.child.try_wait()?.is_some()
                {
                    self.finished = true;
                }
                return Err(Error::input(format!(
                    "native bridge incomplete: {original}; cleanup={cleanup:?}"
                )));
            }
            self.finished = true;
            outcome
        }
    }

    impl Drop for Session {
        fn drop(&mut self) {
            if !self.finished {
                let _ = self.stop();
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use serde_json::json;

        fn sleeping_child() -> Child {
            Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "Start-Sleep -Seconds 20",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .unwrap()
        }

        fn test_target(child: &Child) -> PinnedTarget {
            let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, child.id()) };
            assert!(!handle.is_null());
            PinnedTarget(handle)
        }

        #[test]
        fn failed_stop_closes_only_owned_target_before_controller_and_keeps_peer() {
            let mut target = sleeping_child();
            let mut controller = sleeping_child();
            let mut peer = sleeping_child();
            let pinned = test_target(&target);
            let close = target
                .kill()
                .and_then(|_| target.wait().map(|_| ()))
                .map_err(Error::from);
            finish_failed_cleanup(close, &pinned, &mut controller).unwrap();
            assert!(target.try_wait().unwrap().is_some());
            assert!(controller.try_wait().unwrap().is_some());
            assert!(peer.try_wait().unwrap().is_none());
            peer.kill().unwrap();
            peer.wait().unwrap();
        }

        #[test]
        fn callback_refusal_preserves_live_target_and_controller() {
            let mut target = sleeping_child();
            let mut controller = sleeping_child();
            let mut peer = sleeping_child();
            let pinned = test_target(&target);
            assert!(finish_failed_cleanup(
                Err(Error::input("fixture close refused")),
                &pinned,
                &mut controller,
            )
            .is_err());
            assert!(target.try_wait().unwrap().is_none());
            assert!(controller.try_wait().unwrap().is_none());
            assert!(peer.try_wait().unwrap().is_none());
            target.kill().unwrap();
            target.wait().unwrap();
            controller.kill().unwrap();
            controller.wait().unwrap();
            peer.kill().unwrap();
            peer.wait().unwrap();
        }

        #[test]
        fn terminal_proof_requires_one_restoration_and_one_real_entry() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("trace.jsonl");
            let rows = [
                json!({"kind":"identity","pid":7,"created_filetime":123,
                    "mode":"can-ever","entry_rva":civ6_native_bridge_rs::PINNED_CAN_EVER_RVA,
                    "gamecore_sha256":civ6_native_bridge_rs::PINNED_GAMECORE_SHA256}),
                json!({"kind":"install","status":0}),
                json!({"kind":"uninstall","status":0,"entry_restored":true,
                    "prefix_retained_until_exit":false}),
                json!({"kind":"entry","sequence":1}),
                json!({"kind":"stop","result":"ok","rows":1,
                    "dropped":0,"observed_calls":1,"in_flight":0,
                    "snapshot_stable":true,"classification":"can_ever_entry",
                    "prefix_retained_until_exit":false}),
            ];
            let write = |rows: &[Value]| {
                std::fs::write(
                    &path,
                    rows.iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .unwrap();
            };
            write(&rows);
            assert_eq!(
                terminal_artifact(&path, 7, 123, BridgeMode::CanEver).unwrap(),
                1
            );
            for index in [0, 1, 2, 3, 4] {
                let mut missing = rows.to_vec();
                missing.remove(index);
                write(&missing);
                assert!(terminal_artifact(&path, 7, 123, BridgeMode::CanEver).is_err());
            }
            let mut wrong = rows.to_vec();
            wrong[2]["status"] = json!(6);
            write(&wrong);
            assert!(terminal_artifact(&path, 7, 123, BridgeMode::CanEver).is_err());
            for (field, value) in [
                ("dropped", json!(1)),
                ("in_flight", json!(1)),
                ("snapshot_stable", json!(false)),
            ] {
                let mut incomplete = rows.to_vec();
                incomplete[4][field] = value;
                write(&incomplete);
                assert!(
                    terminal_artifact(&path, 7, 123, BridgeMode::CanEver).is_err(),
                    "{field}"
                );
            }
        }

        #[test]
        fn property_terminal_classifies_raw_rows_and_keeps_partial_evidence() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("property.jsonl");
            let rows = [
                json!({"kind":"identity","pid":7,"created_filetime":123,
                    "mode":"property","entry_rva":civ6_native_bridge_rs::PINNED_PROPERTY_GET_RVA,
                    "prefix_bytes":"[cc, cc, cc, cc, cc]",
                    "gamecore_sha256":civ6_native_bridge_rs::PINNED_GAMECORE_SHA256}),
                json!({"kind":"install","status":0}),
                json!({"kind":"uninstall","status":0,"entry_restored":true,
                    "prefix_retained_until_exit":true}),
                json!({"kind":"property_get","sequence":1,"property":"0x0000000000001000",
                    "key_id":853,"returned":"0x0000000000000000","thread_id":4,
                    "read_state":0,"tag":null,"payload8":null,"flags14":null}),
                json!({"kind":"property_get","sequence":2,"property":"0x0000000000001000",
                    "key_id":853,"returned":"0x0000000000002000","thread_id":4,
                    "read_state":1,"tag":2,"payload8":"0x00000000000d0517","flags14":0}),
                json!({"kind":"stop","result":"raw_collected","rows":2,"dropped":0,
                    "observed_calls":2,"in_flight":0,"snapshot_stable":true,
                    "quiescence_proven":false,"classification":"raw_collection",
                    "prefix_retained_until_exit":true}),
            ];
            let write = |rows: &[Value]| {
                std::fs::write(
                    &path,
                    rows.iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .unwrap()
            };
            write(&rows);
            assert_eq!(
                terminal_artifact(&path, 7, 123, BridgeMode::Property).unwrap(),
                2
            );
            let mut one_unmapped = rows.to_vec();
            one_unmapped.remove(4);
            one_unmapped[4]["rows"] = json!(1);
            one_unmapped[4]["observed_calls"] = json!(1);
            write(&one_unmapped);
            assert_eq!(
                terminal_artifact(&path, 7, 123, BridgeMode::Property).unwrap(),
                1
            );
            let mut partial = rows.to_vec();
            partial[5]["result"] = json!("partial_raw");
            partial[5]["dropped"] = json!(3);
            partial[5]["observed_calls"] = json!(5);
            write(&partial);
            assert!(terminal_artifact(&path, 7, 123, BridgeMode::Property)
                .unwrap_err()
                .to_string()
                .contains("3 dropped"));
            for index in 0..rows.len() {
                let mut missing = rows.to_vec();
                missing.remove(index);
                write(&missing);
                assert!(
                    terminal_artifact(&path, 7, 123, BridgeMode::Property).is_err(),
                    "{index}"
                );
            }
            for (field, value) in [
                ("returned", json!("0x0000000000000000")),
                ("tag", json!(null)),
                ("read_state", json!(2)),
            ] {
                let mut bad = rows.to_vec();
                bad[4][field] = value;
                write(&bad);
                assert!(
                    terminal_artifact(&path, 7, 123, BridgeMode::Property).is_err(),
                    "{field}"
                );
            }
        }
    }
}

#[cfg(windows)]
pub use platform::{close_pinned_game, Session};

#[cfg(not(windows))]
pub fn close_pinned_game(_: u32, _: f64) -> Result<()> {
    Err(Error::input("native bridge requires Windows x64"))
}

#[cfg(not(windows))]
pub struct Session;
#[cfg(not(windows))]
impl Session {
    pub fn start(
        _pid: u32,
        _started: f64,
        _output: &Path,
        _name: &str,
        _seconds: u64,
        _mode: &str,
        _close_game: Box<dyn FnMut(u32, f64) -> Result<()> + Send>,
    ) -> Result<Self> {
        Err(Error::input("native bridge requires Windows x64"))
    }
    pub fn stop(&mut self) -> Result<PathBuf> {
        Err(Error::input("native bridge requires Windows x64"))
    }
}
