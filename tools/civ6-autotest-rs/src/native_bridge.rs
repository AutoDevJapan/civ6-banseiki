//! Bounded, separate native bridge telemetry. This never routes through the
//! read-only hardware-breakpoint observer or claims admission is working.
use crate::{Error, Result};
use std::path::{Path, PathBuf};

#[cfg(windows)]
mod platform {
    use super::*;
    use serde_json::Value;
    use std::ffi::OsString;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::windows::{ffi::OsStringExt, io::AsRawHandle, process::CommandExt};
    use std::process::{Child, ChildStdin, Command, Stdio};
    use std::sync::mpsc::{self, Receiver};
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, HANDLE, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn terminal_artifact(path: &Path, pid: u32, created: u64) -> Result<usize> {
        let mut identity = 0usize;
        let mut installed = 0usize;
        let mut restored = 0usize;
        let mut stopped = 0usize;
        let mut entries = 0usize;
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
                        && row["gamecore_sha256"]
                            == civ6_native_bridge_rs::PINNED_GAMECORE_SHA256 =>
                {
                    identity += 1
                }
                Some("install") if row["status"] == 0 => installed += 1,
                Some("uninstall") if row["status"] == 0 => restored += 1,
                Some("entry") if row["sequence"].as_u64() == Some(entries as u64 + 1) => {
                    entries += 1
                }
                Some("stop") if row["result"] == "ok" && row["rows"] == entries => stopped += 1,
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
        Ok(entries)
    }

    fn pinned_created(pid: u32, expected: f64) -> Result<u64> {
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return Err(Error::input("native bridge game process unavailable"));
        }
        let mut a = FILETIME::default();
        let mut b = FILETIME::default();
        let mut c = FILETIME::default();
        let mut d = FILETIME::default();
        let ok = unsafe { GetProcessTimes(handle, &mut a, &mut b, &mut c, &mut d) } != 0;
        let mut image = vec![0u16; 32768];
        let mut len = image.len() as u32;
        let path_ok =
            unsafe { QueryFullProcessImageNameW(handle, 0, image.as_mut_ptr(), &mut len) } != 0;
        unsafe { CloseHandle(handle) };
        if !ok || !path_ok || len == 0 || len as usize >= image.len() {
            return Err(Error::input("native bridge game identity unreadable"));
        }
        let created = (u64::from(a.dwHighDateTime) << 32) | u64::from(a.dwLowDateTime);
        let unix = created as f64 / 10_000_000.0 - 11_644_473_600.0;
        let path = PathBuf::from(OsString::from_wide(&image[..len as usize]));
        if unix != expected
            || !path
                .canonicalize()
                .ok()
                .zip(
                    Path::new(civ6_native_bridge_rs::GAME_EXE)
                        .canonicalize()
                        .ok(),
                )
                .is_some_and(|(a, b)| {
                    a.to_string_lossy()
                        .eq_ignore_ascii_case(&b.to_string_lossy())
                })
        {
            return Err(Error::input("native bridge pinned game identity changed"));
        }
        Ok(created)
    }

    pub struct Session {
        child: Child,
        input: Option<ChildStdin>,
        lines: Receiver<String>,
        artifact: PathBuf,
        pid: u32,
        created: u64,
        started: f64,
        close_game: Box<dyn FnMut(u32, f64) -> Result<()> + Send>,
        finished: bool,
    }

    impl Session {
        pub fn start(
            pid: u32,
            started: f64,
            output: &Path,
            name: &str,
            seconds: u64,
            mut close_game: Box<dyn FnMut(u32, f64) -> Result<()> + Send>,
        ) -> Result<Self> {
            if !(1..=u64::from(civ6_native_bridge_rs::MAX_ACTIVE_SECONDS)).contains(&seconds) {
                return Err(Error::scenario(
                    "native bridge duration must be 1..300 seconds",
                ));
            }
            let created = pinned_created(pid, started)?;
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
                let _ = close_game(pid, started);
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::input("bridge stdout unavailable; owned game closed"));
            };
            if input.is_none() {
                let _ = close_game(pid, started);
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::input(
                    "bridge stop pipe unavailable; owned game closed",
                ));
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
                close_game,
                finished: false,
            };
            match session.lines.recv_timeout(Duration::from_secs(30)) {
                Ok(line) if line.starts_with(&format!("READY pid={pid} ")) => Ok(session),
                _ => {
                    let _ = (session.close_game)(pid, started);
                    let _ = session.child.kill();
                    let _ = session.child.wait();
                    session.finished = true;
                    Err(Error::input(
                        "native bridge did not prove bounded readiness; owned game closed",
                    ))
                }
            }
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
            terminal_artifact(&self.artifact, self.pid, self.created)?;
            Ok(self.artifact.clone())
        }

        pub fn stop(&mut self) -> Result<PathBuf> {
            if self.finished {
                return Err(Error::scenario("native bridge already stopped"));
            }
            let outcome = self.stop_inner();
            if outcome.is_err() {
                let _ = (self.close_game)(self.pid, self.started);
                let _ = self.child.kill();
                let _ = self.child.wait();
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

        #[test]
        fn terminal_proof_requires_one_restoration_and_one_real_entry() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("trace.jsonl");
            let rows = [
                json!({"kind":"identity","pid":7,"created_filetime":123,
                    "gamecore_sha256":civ6_native_bridge_rs::PINNED_GAMECORE_SHA256}),
                json!({"kind":"install","status":0}),
                json!({"kind":"uninstall","status":0}),
                json!({"kind":"entry","sequence":1}),
                json!({"kind":"stop","result":"ok","rows":1}),
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
            assert_eq!(terminal_artifact(&path, 7, 123).unwrap(), 1);
            for index in [0, 1, 2, 3, 4] {
                let mut missing = rows.to_vec();
                missing.remove(index);
                write(&missing);
                assert!(terminal_artifact(&path, 7, 123).is_err());
            }
            let mut wrong = rows.to_vec();
            wrong[2]["status"] = json!(6);
            write(&wrong);
            assert!(terminal_artifact(&path, 7, 123).is_err());
        }
    }
}

#[cfg(windows)]
pub use platform::Session;

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
        _close_game: Box<dyn FnMut(u32, f64) -> Result<()> + Send>,
    ) -> Result<Self> {
        Err(Error::input("native bridge requires Windows x64"))
    }
    pub fn stop(&mut self) -> Result<PathBuf> {
        Err(Error::input("native bridge requires Windows x64"))
    }
}
