//! Driver transport for the existing owned, capture-only order observer.
//! The observer parent alone grants attach and owns restoration/reaping.
use crate::{Error, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn identity() -> Value {
    use sha2::{Digest, Sha256};
    json!({"protocol":"order-driver-v1",
        "driver":format!("{:x}",Sha256::digest(concat!(include_str!("native_order.rs"),
            include_str!("runner.rs"), include_str!("scenario.rs"),
            include_str!("bin/civ6-startup-diagnostics.rs"), include_str!("../Cargo.toml")))),
        "observer":civ6_native_probe_rs::order_capture::source_identity()["compiled_source_sha256"]})
}

/// These entry points share the helper already rebuilt by live_chain.
/// `observe` is the fixed child command used by the original parent protocol.
pub fn worker(args: &[String]) -> Option<Result<()>> {
    let operation = args.first()?.as_str();
    if ![
        "order-identity",
        "order-parent",
        "observe",
        "audit-thread-visibility",
    ]
    .contains(&operation)
    {
        return None;
    }
    Some((|| {
        if operation == "order-identity" {
            if args.len() != 1 {
                return Err(Error::input("order-identity takes no arguments"));
            }
            println!("{}", identity());
            return Ok(());
        }
        if args.len() != 2 {
            return Err(Error::input("order observer needs one request file"));
        }
        #[cfg(all(windows, target_arch = "x86_64"))]
        {
            let value: Value = serde_json::from_slice(&std::fs::read(&args[1])?)?;
            let req = civ6_native_probe_rs::order_capture::Request::parse(&value)
                .map_err(Error::input)?;
            let result = if operation == "audit-thread-visibility" {
                civ6_native_probe_rs::windows::order_capture::audit(req)
            } else if operation == "order-parent" {
                civ6_native_probe_rs::windows::order_capture::parent(value)
            } else {
                civ6_native_probe_rs::windows::order_capture::observe(req)
            };
            result.map_err(Error::input)
        }
        #[cfg(not(all(windows, target_arch = "x86_64")))]
        Err(Error::input("order observation requires Windows x64"))
    })())
}

pub fn validate_options(step: &Value) -> Result<()> {
    let source_ok = match (step.get("source_dll"), step.get("source_dll_sha256")) {
        (None, None) => true,
        (Some(path), Some(sha)) => {
            path.as_str().is_some_and(|p| {
                let bytes = p.as_bytes();
                bytes.len() > 3
                    && bytes[0].is_ascii_alphabetic()
                    && bytes[1] == b':'
                    && matches!(bytes[2], b'/' | b'\\')
                    && p.to_ascii_lowercase().ends_with(".dll")
            }) && sha
                .as_str()
                .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        }
        _ => false,
    };
    if !source_ok
        || !crate::scenario::matches(r"^[A-Za-z0-9][A-Za-z0-9_.-]{0,79}$", &step["name"])
        || !step["max_seconds"]
            .as_u64()
            .is_some_and(|v| (1..=300).contains(&v))
        || !step["max_hits"]
            .as_u64()
            .is_some_and(|v| (1..=256).contains(&v))
        || !["front_prefix", "load_prefix"]
            .iter()
            .all(|key| step[key].as_u64().is_some_and(|v| (1..=4).contains(&v)))
        || !matches!(
            step["phase"].as_str(),
            Some("front" | "load" | "native-outer" | "native-shutdown")
        )
        || step.as_object().is_none_or(|o| {
            o.keys().any(|k| {
                ![
                    "action",
                    "name",
                    "max_seconds",
                    "max_hits",
                    "front_prefix",
                    "load_prefix",
                    "phase",
                    "timeout",
                    "source_dll",
                    "source_dll_sha256",
                ]
                .contains(&k.as_str())
            })
        })
    {
        return Err(Error::scenario(
            "invalid bounded native order capture options",
        ));
    }
    Ok(())
}

pub fn validate_receipt(
    row: &Value,
    pid: u32,
    created: u64,
    sha: &str,
    observer: u32,
) -> Result<()> {
    if row["type"] != "order_parent_receipt"
        || row["schema"] != civ6_native_probe_rs::order_capture::SCHEMA
        || row["game_pid"].as_u64() != Some(u64::from(pid))
        || row["game_created_filetime"].as_u64() != Some(created)
        || row["source_dll_sha256"] != sha
        || observer == 0
        || row["child_pid"].as_u64() != Some(u64::from(observer))
        || row["permit_granted"] != true
        || row["never_attached"] != false
        || row["stop_writes"].as_u64().is_none_or(|n| n > 1)
        || row["child_reaped"] != true
        || row["cleanup_confirmed"] != true
        || row["restored"] != true
        || row["live_receipt"] != true
        || row["child_exit_code"] != 0
        || row.get("error") != Some(&Value::Null)
        || row["target_close_requested"] != false
        || row["child_force_terminated"] != false
    {
        return Err(Error::input(
            "native order capture did not prove restored, reaped success",
        ));
    }
    Ok(())
}

pub fn validate_audit_options(step: &Value) -> Result<()> {
    let object = step
        .as_object()
        .ok_or_else(|| Error::scenario("audit options must be an object"))?;
    if object.keys().any(|key| {
        ![
            "action",
            "name",
            "max_seconds",
            "timeout",
            "source_dll",
            "source_dll_sha256",
        ]
        .contains(&key.as_str())
    }) || step["action"] != "audit_thread_visibility"
        || !step["max_seconds"]
            .as_u64()
            .is_some_and(|s| (1..=30).contains(&s))
        || !step["source_dll"].is_string()
        || !step["source_dll_sha256"].is_string()
    {
        return Err(Error::scenario(
            "invalid finite thread visibility audit options",
        ));
    }
    let mut capture = step.clone();
    for (key, value) in [
        ("max_hits", json!(1)),
        ("front_prefix", json!(1)),
        ("load_prefix", json!(1)),
        ("phase", json!("front")),
    ] {
        capture[key] = value;
    }
    validate_options(&capture)
}
pub fn validate_audit_receipt(
    row: &Value,
    pid: u32,
    created: u64,
    sha: &str,
    observer_source: &Value,
) -> Result<()> {
    if row["type"] != "order_thread_audit"
        || row["schema"] != civ6_native_probe_rs::order_capture::SCHEMA
        || row["game_pid"].as_u64() != Some(u64::from(pid))
        || row["game_created_filetime"].as_u64() != Some(created)
        || row["source_dll_sha256"] != sha
        || row["observer_source"] != *observer_source
        || !row["threads"].is_array()
        || row["threads"].as_array().is_some_and(|a| a.len() > 1024)
    {
        return Err(Error::input("thread audit receipt identity differs"));
    }
    for key in [
        "debug_attach_calls",
        "debug_register_writes",
        "thread_suspend_calls",
        "thread_hide_set_calls",
        "memory_writes",
        "dll_install_calls",
    ] {
        if row[key].as_u64() != Some(0) {
            return Err(Error::input(
                "thread audit receipt contains intervention or lacks zero counters",
            ));
        }
    }
    for key in [
        "snapshot_complete_over_interval",
        "future_thread_visibility_proven",
        "hardware_capture_admitted",
        "rebuild_authorized",
        "root_generation_verified",
        "native_quiescence_proven",
        "runtime_activation_allowed",
        "root_activation_allowed",
        "all_producer_domains_covered",
        "same_set_wait_proven",
        "pending_publication_drained",
    ] {
        if row[key] != false {
            return Err(Error::input(
                "thread audit receipt claims unproven authority",
            ));
        }
    }
    if row["native_event_root_identity"] != "unknown" {
        return Err(Error::input(
            "thread audit cannot identify a native event root",
        ));
    }
    for thread in row["threads"].as_array().unwrap() {
        if !thread["tid"].as_u64().is_some_and(|tid| tid > 0)
            || thread["expected_owner_pid"] != pid
            || thread["read_only"] != true
            || !matches!(
                thread["visibility"].as_str(),
                Some("hidden" | "visible" | "unknown")
            )
        {
            return Err(Error::input(
                "thread audit row lacks bounded identity or visibility status",
            ));
        }
    }
    Ok(())
}

#[cfg(all(windows, target_arch = "x86_64"))]
mod platform {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::{
        fs::{self, File},
        io::{BufRead, BufReader, Write},
        os::windows::{fs::OpenOptionsExt, io::AsRawHandle, process::CommandExt},
        process::{Child, ChildStdin, Command, Stdio},
        sync::mpsc::{self, Receiver},
        thread::JoinHandle,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME, WAIT_TIMEOUT},
        System::Threading::{
            GetProcessTimes, OpenProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION,
        },
    };

    fn created(pid: u32) -> Result<u64> {
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return Err(Error::input("order process identity unavailable"));
        }
        let (mut a, mut b, mut c, mut d) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        let ok = unsafe { GetProcessTimes(handle, &mut a, &mut b, &mut c, &mut d) };
        unsafe {
            CloseHandle(handle);
        }
        if ok == 0 {
            return Err(Error::input("order process creation time unavailable"));
        }
        Ok((u64::from(a.dwHighDateTime) << 32) | u64::from(a.dwLowDateTime))
    }
    pub fn check_parent_active(child: &Child) -> Result<()> {
        if unsafe { WaitForSingleObject(child.as_raw_handle().cast(), 0) } != WAIT_TIMEOUT {
            return Err(Error::input(
                "native order observer parent ended during the scenario",
            ));
        }
        Ok(())
    }

    fn audit_command(
        executable: &Path,
        args: &[&std::ffi::OsStr],
        stdout: &Path,
        stderr: &Path,
        deadline: Instant,
    ) -> Result<()> {
        let out = File::create_new(stdout)?;
        let errors = File::create_new(stderr)?;
        if Instant::now() >= deadline {
            return Err(Error::input("thread audit deadline before helper spawn"));
        }
        let child = Command::new(executable)
            .args(args)
            .stdin(Stdio::null())
            .stdout(out)
            .stderr(errors)
            .creation_flags(0x08000000)
            .spawn()?;
        struct ReadOnlyChild(Child);
        impl Drop for ReadOnlyChild {
            fn drop(&mut self) {
                if self.0.try_wait().is_ok_and(|status| status.is_some()) {
                    return;
                }
                let _ = self.0.kill();
                if unsafe { WaitForSingleObject(self.0.as_raw_handle().cast(), 5_000) } == 0 {
                    let _ = self.0.try_wait();
                }
            }
        }
        let mut owner = ReadOnlyChild(child);
        let child = &mut owner.0;
        let mut expired = false;
        loop {
            if let Some(status) = child.try_wait()? {
                if !status.success() || expired || Instant::now() >= deadline {
                    return Err(Error::input("thread audit helper failed or exceeded deadline; see retained audit stderr"));
                }
                return Ok(());
            }
            if Instant::now() >= deadline {
                expired = true;
                if child.kill().is_err() && child.try_wait()?.is_none() {
                    return Err(Error::input(
                        "owned read-only audit helper termination failed",
                    ));
                }
                if unsafe { WaitForSingleObject(child.as_raw_handle().cast(), 5_000) } != 0 {
                    return Err(Error::input(
                        "owned read-only audit helper exit unconfirmed",
                    ));
                }
            } else {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
    pub fn audit_thread_visibility(
        pid: u32,
        started: f64,
        output: &Path,
        step: &Value,
        budget: f64,
    ) -> Result<PathBuf> {
        validate_audit_options(step)?;
        if !budget.is_finite() || budget <= 0.0 {
            return Err(Error::input("thread audit deadline exhausted"));
        }
        let deadline = Instant::now()
            + Duration::from_secs_f64(budget.min(step["max_seconds"].as_u64().unwrap() as f64));
        let created = created(pid)?;
        if created as f64 / 10_000_000.0 - 11_644_473_600.0 != started {
            return Err(Error::input("thread audit game generation changed"));
        }
        let chain: u32 = std::env::var("CIV6_LIVE_CHAIN_PID")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|p| *p > 0)
            .ok_or_else(|| Error::input("thread audit requires live_chain"))?;
        let executable = std::env::current_exe()?.with_file_name("civ6-startup-diagnostics.exe");
        let _helper_image = fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&executable)?;
        let base = output.join(format!("{}.audit", step["name"].as_str().unwrap()));
        let artifact = base.with_extension("audit.json");
        let proof_path = base.with_extension("audit.identity.json");
        audit_command(
            &executable,
            &[std::ffi::OsStr::new("order-identity")],
            &proof_path,
            &base.with_extension("audit.identity.stderr.log"),
            deadline,
        )?;
        if serde_json::from_slice::<Value>(&fs::read(&proof_path)?)? != identity() {
            return Err(Error::input("thread audit helper source differs"));
        }
        let dll = PathBuf::from(step["source_dll"].as_str().unwrap());
        let _dll_pin = fs::OpenOptions::new().read(true).share_mode(1).open(&dll)?;
        let sha = format!("{:x}", Sha256::digest(fs::read(&dll)?));
        if !sha.eq_ignore_ascii_case(step["source_dll_sha256"].as_str().unwrap()) {
            return Err(Error::input(
                "thread audit source DLL differs from frozen build",
            ));
        }
        let request = base.with_extension("audit.driver-request.json");
        let value = json!({"schema":civ6_native_probe_rs::order_capture::SCHEMA,
            "pid":pid,"created_filetime":created,"chain_pid":chain,"chain_created_filetime":self::created(chain)?,
            "controller_pid":std::process::id(),"controller_created_filetime":self::created(std::process::id())?,
            "source_dll":dll,"source_dll_sha256":sha,"output":artifact,
            "max_seconds":step["max_seconds"],"max_hits":1,"front_prefix":1,"load_prefix":1,"phase":"front"});
        let mut file = File::create_new(&request)?;
        serde_json::to_writer_pretty(&mut file, &value)?;
        file.sync_all()?;
        audit_command(
            &executable,
            &[
                std::ffi::OsStr::new("audit-thread-visibility"),
                request.as_os_str(),
            ],
            &base.with_extension("audit.transport.json"),
            &base.with_extension("audit.stderr.log"),
            deadline,
        )?;
        let row: Value = serde_json::from_slice(&fs::read(&artifact)?)?;
        validate_audit_receipt(&row, pid, created, &sha, &identity()["observer"])?;
        for key in [
            "chain_pid",
            "chain_created_filetime",
            "controller_pid",
            "controller_created_filetime",
        ] {
            if row[key] != value[key] {
                return Err(Error::input(
                    "thread audit owner generation differs from request",
                ));
            }
        }
        let receipt = json!({"type":"order_audit_driver_receipt","game_pid":pid,"game_created_filetime":created,"created_filetime":created,
            "helper_reaped":true,"helper_exit_code":0,"helper_sha256":format!("{:x}",Sha256::digest(fs::read(&executable)?)),
            "source_identity":identity(),"source_dll_sha256":sha,"attached":false,"hardware_capture_admitted":false,"runtime_activation_allowed":false});
        let mut file = File::create_new(base.with_extension("audit.driver.json"))?;
        serde_json::to_writer_pretty(&mut file, &receipt)?;
        file.sync_all()?;
        if Instant::now() >= deadline {
            return Err(Error::input(
                "thread audit deadline exhausted after receipt retention",
            ));
        }
        Ok(artifact)
    }

    pub struct Session {
        child: Child,
        input: Option<ChildStdin>,
        lines: Receiver<std::result::Result<Value, String>>,
        reader: Option<JoinHandle<()>>,
        receipt: Option<Value>,
        artifact: PathBuf,
        pid: u32,
        created: u64,
        sha: String,
        finished: bool,
        phase: String,
        observer: u32,
        _helper_image: File,
    }
    impl Session {
        pub fn check_active(&self) -> Result<()> {
            if self.finished {
                return Err(Error::input("native order capture has already stopped"));
            }
            check_parent_active(&self.child)
        }
        pub fn start(pid: u32, started: f64, output: &Path, step: &Value) -> Result<Self> {
            validate_options(step)?;
            let created = created(pid)?;
            if created as f64 / 10_000_000.0 - 11_644_473_600.0 != started {
                return Err(Error::input("order game process changed"));
            }
            let chain: u32 = std::env::var("CIV6_LIVE_CHAIN_PID")
                .ok()
                .and_then(|s| s.parse().ok())
                .filter(|&p| p > 0)
                .ok_or_else(|| Error::input("order capture needs live_chain ownership"))?;
            let executable =
                std::env::current_exe()?.with_file_name("civ6-startup-diagnostics.exe");
            // Keep the identity query, recorded SHA, parent and observe child on
            // the same executable until both children have been reaped.
            let helper_image = fs::OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(&executable)?;
            let proof = Command::new(&executable)
                .arg("order-identity")
                .creation_flags(0x08000000)
                .output()?;
            if !proof.status.success()
                || serde_json::from_slice::<Value>(&proof.stdout)? != identity()
            {
                return Err(Error::input("order helper source differs from this driver"));
            }
            let dll = step["source_dll"]
                .as_str()
                .map(PathBuf::from)
                .unwrap_or_else(|| executable.with_file_name("civ6_native_bridge_rs.dll"));
            let sha = format!("{:x}", Sha256::digest(fs::read(&dll)?));
            if step["source_dll_sha256"]
                .as_str()
                .is_some_and(|expected| !sha.eq_ignore_ascii_case(expected))
            {
                return Err(Error::input(
                    "order source DLL differs from the scenario's frozen build",
                ));
            }
            let artifact = output.join(format!("{}.jsonl", step["name"].as_str().unwrap()));
            if artifact.exists() || artifact.with_extension("request.json").exists() {
                return Err(Error::scenario("order capture output already exists"));
            }
            let request = artifact.with_extension("driver-request.json");
            let driver_proof = json!({"helper_path":executable,
                "helper_sha256":format!("{:x}",Sha256::digest(fs::read(&executable)?)),
                "runner_path":std::env::current_exe()?,"source_identity":identity(),
                "source_checkout":crate::repo_root(),"game_pid":pid,"created_filetime":created,
                "product_acceptance":false});
            serde_json::to_writer_pretty(
                File::create_new(artifact.with_extension("driver.json"))?,
                &driver_proof,
            )?;
            let value = json!({"schema":civ6_native_probe_rs::order_capture::SCHEMA,
                "pid":pid,"created_filetime":created,"chain_pid":chain,"chain_created_filetime":self::created(chain)?,
                "controller_pid":std::process::id(),"controller_created_filetime":self::created(std::process::id())?,
                "source_dll":dll,"source_dll_sha256":sha,"output":artifact,
                "max_seconds":step["max_seconds"],"max_hits":step["max_hits"],
                "front_prefix":step["front_prefix"],"load_prefix":step["load_prefix"],"phase":step["phase"]});
            let mut file = File::create_new(&request)?;
            serde_json::to_writer_pretty(&mut file, &value)?;
            file.flush()?;
            let errors = File::create_new(artifact.with_extension("stderr.log"))?;
            let mut transport = File::create_new(artifact.with_extension("transport.jsonl"))?;
            let mut child = Command::new(&executable)
                .arg("order-parent")
                .arg(request)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(errors)
                .creation_flags(0x08000000)
                .spawn()?;
            // Stdio::piped guarantees these handles after successful spawn.
            let input = child.stdin.take();
            let stdout = child.stdout.take().expect("piped order stdout");
            let (tx, lines) = mpsc::channel();
            let reader = std::thread::Builder::new()
                .name("order-capture-output".into())
                .spawn(move || {
                    for row in BufReader::new(stdout).lines() {
                        let value = row.map_err(|e| e.to_string()).and_then(|line| {
                            writeln!(transport, "{line}")
                                .and_then(|_| transport.flush())
                                .map_err(|e| e.to_string())?;
                            serde_json::from_str::<Value>(&line).map_err(|e| e.to_string())
                        });
                        if tx.send(value).is_err() {
                            break;
                        }
                    }
                });
            let reader = match reader {
                Ok(reader) => reader,
                Err(error) => {
                    drop(input);
                    // EOF asks the original parent to restore; keep its ownership
                    // and wait instead of killing it while its child is attached.
                    let _ = child.wait();
                    return Err(error.into());
                }
            };
            let mut session = Self {
                child,
                input,
                lines,
                reader: Some(reader),
                receipt: None,
                artifact,
                pid,
                created,
                sha,
                finished: false,
                phase: step["phase"].as_str().unwrap().to_owned(),
                observer: 0,
                _helper_image: helper_image,
            };
            let phase = step["phase"].as_str().unwrap();
            session.wait_ready(phase)?;
            Ok(session)
        }
        fn wait_ready(&mut self, phase: &str) -> Result<()> {
            let deadline = Instant::now() + Duration::from_secs(12);
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(Error::input("order readiness expired"));
                }
                let row = self
                    .lines
                    .recv_timeout(remaining)
                    .map_err(|e| Error::input(format!("order readiness unavailable: {e}")))?
                    .map_err(Error::input)?;
                if row["type"] == "order_parent_receipt" {
                    self.receipt = Some(row);
                    return Err(Error::input("order observer ended before readiness"));
                }
                if row["type"] == "attach_ready"
                    && row["game_pid"].as_u64() == Some(u64::from(self.pid))
                    && row["game_created_filetime"].as_u64() == Some(self.created)
                {
                    let observer = row["observer_pid"]
                        .as_u64()
                        .and_then(|v| u32::try_from(v).ok())
                        .filter(|&v| v > 0)
                        .ok_or_else(|| Error::input("order observer identity missing"))?;
                    if self.observer != 0 && self.observer != observer {
                        return Err(Error::input("order observer identity changed"));
                    }
                    self.observer = observer;
                }
                if row["pid"].as_u64() == Some(u64::from(self.pid))
                    && row["ready"] == true
                    && row["capture_only"] == true
                    && row["phase"] == phase
                    && self.observer != 0
                {
                    return Ok(());
                }
            }
        }
        pub fn phase(&mut self, phase: &str) -> Result<()> {
            if !matches!(phase, "front" | "load" | "native-outer" | "native-shutdown") {
                return Err(Error::scenario("unknown order phase"));
            }
            if self.phase == phase {
                return Err(Error::scenario("native order phase must change"));
            }
            let pipe = self
                .input
                .as_mut()
                .ok_or_else(|| Error::input("order input closed"))?;
            writeln!(pipe, "phase {phase}")?;
            pipe.flush()?;
            let deadline = Instant::now() + Duration::from_secs(12);
            loop {
                let row = self
                    .lines
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .map_err(|e| {
                        Error::input(format!("order phase acknowledgment unavailable: {e}"))
                    })?
                    .map_err(Error::input)?;
                if row["type"] == "order_parent_receipt" {
                    self.receipt = Some(row);
                    return Err(Error::input("order observer ended during phase change"));
                }
                if row["pid"].as_u64() == Some(u64::from(self.pid)) && row["phase_ready"] == phase {
                    self.phase = phase.to_owned();
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err(Error::input("order phase deadline"));
                }
            }
        }
        pub fn stop(&mut self) -> Result<PathBuf> {
            if self.finished {
                return Ok(self.artifact.clone());
            }
            if let Some(mut pipe) = self.input.take() {
                let _ = writeln!(pipe, "stop");
                let _ = pipe.flush();
                // EOF also requests restoration. Never terminate the owning parent.
            }
            // The existing parent performs bounded restore then retains critical
            // recovery ownership if necessary. Keep this same child until reaped.
            let status = self.child.wait()?;
            if let Some(reader) = self.reader.take() {
                let _ = reader.join();
            }
            let mut parse_error = None;
            for row in self.lines.try_iter() {
                match row {
                    Ok(row) if row["type"] == "order_parent_receipt" => {
                        // Replacing the receipt consumes row; a match guard
                        // cannot perform that move.
                        let prior_receipt = self.receipt.replace(row);
                        if prior_receipt.is_some() {
                            parse_error = Some("duplicate order receipt".to_string());
                        }
                    }
                    Err(e) => {
                        parse_error = Some(e);
                    }
                    _ => {}
                }
            }
            self.finished = true;
            if let Some(row) = &self.receipt {
                fs::write(
                    self.artifact.with_extension("parent.json"),
                    serde_json::to_vec_pretty(row)?,
                )?;
            }
            if !status.success() {
                return Err(Error::input("order parent exited unsuccessfully"));
            }
            if let Some(error) = parse_error {
                return Err(Error::input(error));
            }
            let row = self
                .receipt
                .as_ref()
                .ok_or_else(|| Error::input("order parent receipt missing"))?;
            validate_receipt(row, self.pid, self.created, &self.sha, self.observer)?;
            if !self.artifact.is_file() {
                return Err(Error::input("native order raw capture missing"));
            }
            Ok(self.artifact.clone())
        }
    }
    impl Drop for Session {
        fn drop(&mut self) {
            if !self.finished {
                let _ = self.stop();
            }
        }
    }
}
#[cfg(all(windows, target_arch = "x86_64"))]
pub use platform::{audit_thread_visibility, check_parent_active, Session};

#[cfg(not(all(windows, target_arch = "x86_64")))]
pub fn audit_thread_visibility(_: u32, _: f64, _: &Path, _: &Value, _: f64) -> Result<PathBuf> {
    Err(Error::input("thread visibility audit requires Windows x64"))
}

#[cfg(not(all(windows, target_arch = "x86_64")))]
pub struct Session;
#[cfg(not(all(windows, target_arch = "x86_64")))]
impl Session {
    pub fn check_active(&self) -> Result<()> {
        Err(Error::input("order capture requires Windows x64"))
    }
    pub fn start(_: u32, _: f64, _: &Path, _: &Value) -> Result<Self> {
        Err(Error::input("order capture requires Windows x64"))
    }
    pub fn phase(&mut self, _: &str) -> Result<()> {
        Err(Error::input("order capture requires Windows x64"))
    }
    pub fn stop(&mut self) -> Result<PathBuf> {
        Err(Error::input("order capture requires Windows x64"))
    }
}
