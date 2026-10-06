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
    if !["order-identity", "order-parent", "observe"].contains(&operation) {
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
            let result = if operation == "order-parent" {
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
        || !matches!(step["phase"].as_str(), Some("front" | "load"))
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

pub fn validate_receipt(row: &Value, pid: u32, created: u64, sha: &str) -> Result<()> {
    if row["type"] != "order_parent_receipt"
        || row["schema"] != civ6_native_probe_rs::order_capture::SCHEMA
        || row["game_pid"].as_u64() != Some(u64::from(pid))
        || row["game_created_filetime"].as_u64() != Some(created)
        || row["source_dll_sha256"] != sha
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

#[cfg(all(windows, target_arch = "x86_64"))]
mod platform {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::{
        fs::{self, File},
        io::{BufRead, BufReader, Write},
        os::windows::process::CommandExt,
        process::{Child, ChildStdin, Command, Stdio},
        sync::mpsc::{self, Receiver},
        thread::JoinHandle,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME},
        System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
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
    }
    impl Session {
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
                if row["pid"].as_u64() == Some(u64::from(self.pid)) {
                    if row["ready"] == true && row["capture_only"] == true && row["phase"] == phase
                    {
                        return Ok(());
                    }
                }
            }
        }
        pub fn phase(&mut self, phase: &str) -> Result<()> {
            if !matches!(phase, "front" | "load") {
                return Err(Error::scenario("unknown order phase"));
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
                        if self.receipt.replace(row).is_some() {
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
            validate_receipt(row, self.pid, self.created, &self.sha)?;
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
pub use platform::Session;

#[cfg(not(all(windows, target_arch = "x86_64")))]
pub struct Session;
#[cfg(not(all(windows, target_arch = "x86_64")))]
impl Session {
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
