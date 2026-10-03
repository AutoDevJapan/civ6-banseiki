//! Lifecycle of the separate PR86 read-only native observer.
use crate::{Error, Result};
use std::path::{Path, PathBuf};

#[cfg(windows)]
mod platform {
    use super::*;
    use serde_json::Value;
    use std::{
        ffi::OsString,
        io::{BufRead, BufReader, Read, Write},
        os::windows::{ffi::OsStringExt, process::CommandExt},
        process::{Child, ChildStdin, Command, Stdio},
        sync::mpsc,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE},
        System::Threading::{
            GetCurrentProcess, GetProcessTimes, OpenProcess, QueryFullProcessImageNameW,
            PROCESS_QUERY_LIMITED_INFORMATION,
        },
    };

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const LOCK: &str = "D:/CC/Tools/civ6-live/civ6.lock";

    fn created(handle: HANDLE) -> Result<u64> {
        let mut a = FILETIME::default();
        let mut b = FILETIME::default();
        let mut c = FILETIME::default();
        let mut d = FILETIME::default();
        if unsafe { GetProcessTimes(handle, &mut a, &mut b, &mut c, &mut d) } == 0 {
            return Err(Error::input(format!(
                "cannot read process creation time: {}",
                std::io::Error::last_os_error()
            )));
        }
        Ok((u64::from(a.dwHighDateTime) << 32) | u64::from(a.dwLowDateTime))
    }
    fn game_identity(pid: u32) -> Result<(u64, PathBuf)> {
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return Err(Error::input(format!(
                "cannot pin Civ6 PID {pid}: {}",
                std::io::Error::last_os_error()
            )));
        }
        let result = (|| {
            let started = created(handle)?;
            let mut image = vec![0u16; 32768];
            let mut len = image.len() as u32;
            if unsafe { QueryFullProcessImageNameW(handle, 0, image.as_mut_ptr(), &mut len) } == 0
                || len == 0
                || len as usize >= image.len()
            {
                return Err(Error::input("cannot read complete Civ6 executable path"));
            }
            Ok((
                started,
                PathBuf::from(OsString::from_wide(&image[..len as usize])),
            ))
        })();
        unsafe {
            CloseHandle(handle);
        }
        result
    }
    pub struct Session {
        child: Child,
        stdin: Option<ChildStdin>,
        output: PathBuf,
        stopped: bool,
    }
    impl Session {
        pub fn start(pid: u32, output: &Path, name: &str, seconds: u64) -> Result<Self> {
            let chain_pid = std::env::var(crate::startup::CHAIN_PID_ENV)
                .map_err(|_| Error::input("live_chain PID is required for native trace"))?
                .parse::<u32>()
                .map_err(|_| Error::input("invalid live_chain PID"))?;
            let lock = PathBuf::from(std::env::var_os(crate::startup::LOCK_PATH_ENV).ok_or_else(
                || Error::input("live_chain lock path is required for native trace"),
            )?);
            if lock
                .to_string_lossy()
                .replace('\\', "/")
                .to_ascii_lowercase()
                != LOCK.to_ascii_lowercase()
            {
                return Err(Error::input(
                    "standard live_chain lock path required for native trace",
                ));
            }
            let (game_created, game_exe) = game_identity(pid)?;
            let controller_pid = std::process::id();
            let controller_created = created(unsafe { GetCurrentProcess() })?;
            let exe = std::env::current_exe()?.with_file_name("civ6-native-probe.exe");
            let trace = output.join(format!("{name}.jsonl"));
            let mut child = Command::new(&exe)
                .args(["observe", "--pid"])
                .arg(pid.to_string())
                .arg("--created")
                .arg(game_created.to_string())
                .arg("--exe")
                .arg(game_exe)
                .arg("--chain-pid")
                .arg(chain_pid.to_string())
                .arg("--lock")
                .arg(lock)
                .arg("--controller-pid")
                .arg(controller_pid.to_string())
                .arg("--controller-created")
                .arg(controller_created.to_string())
                .arg("--output")
                .arg(&trace)
                .arg("--max-seconds")
                .arg(seconds.to_string())
                .creation_flags(CREATE_NO_WINDOW)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| {
                    Error::input(format!(
                        "cannot start native observer {}: {e}",
                        exe.display()
                    ))
                })?;
            let stdout = child
                .stdout
                .take()
                .ok_or_else(|| Error::input("observer stdout unavailable"))?;
            let stdin = child.stdin.take();
            let mut session = Self {
                child,
                stdin,
                output: trace,
                stopped: false,
            };
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                let mut line = String::new();
                let result = BufReader::new(stdout)
                    .read_line(&mut line)
                    .map(|n| (n, line));
                let _ = tx.send(result);
            });
            match rx.recv_timeout(Duration::from_secs(20)) {
                Ok(Ok((n, line))) if n > 0 => {
                    let message: Value = serde_json::from_str(&line).map_err(|_| {
                        Error::input("native observer ready handshake was not JSON")
                    })?;
                    if message["ready"] != true || message["pid"].as_u64() != Some(u64::from(pid)) {
                        return Err(Error::input(
                            "native observer ready handshake did not match game PID",
                        ));
                    }
                    Ok(session)
                }
                Ok(Ok(_)) => Err(Error::input(format!(
                    "native observer exited before ready handshake: {}",
                    session.error_text()
                ))),
                Ok(Err(e)) => Err(Error::input(format!(
                    "native observer handshake: {e}; {}",
                    session.error_text()
                ))),
                Err(_) => Err(Error::input("native observer ready handshake timed out")),
            }
        }
        fn error_text(&mut self) -> String {
            if self.child.try_wait().ok().flatten().is_none() {
                return "observer has not exited".into();
            }
            let mut text = String::new();
            if let Some(stderr) = self.child.stderr.as_mut() {
                let _ = stderr.take(4096).read_to_string(&mut text);
            }
            text.trim().to_string()
        }
        pub fn stop(&mut self) -> Result<PathBuf> {
            if self.stopped {
                return Ok(self.output.clone());
            }
            if self.child.try_wait()?.is_none() {
                if let Some(mut stdin) = self.stdin.take() {
                    match stdin.write_all(b"stop\n").and_then(|_| stdin.flush()) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
                        Err(e) => return Err(e.into()),
                    }
                }
            }
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                match self.child.try_wait()? {
                    Some(status) => {
                        self.stopped = true;
                        if !status.success() {
                            return Err(Error::input(format!(
                                "native observer exited with {status}: {}; inspect {}",
                                self.error_text(),
                                self.output.display()
                            )));
                        }
                        if !self.output.is_file() {
                            return Err(Error::input(
                                "native trace artifact missing after observer exit",
                            ));
                        }
                        return Ok(self.output.clone());
                    }
                    None if Instant::now() < deadline => {
                        std::thread::sleep(Duration::from_millis(50))
                    }
                    None => {
                        return Err(Error::input(
                            "native observer did not detach within 20 seconds",
                        ))
                    }
                }
            }
        }
    }
    impl Drop for Session {
        fn drop(&mut self) {
            if !self.stopped {
                let _ = self.stop();
            }
        }
    }
}
#[cfg(windows)]
pub use platform::Session;

#[cfg(not(windows))]
pub struct Session;
#[cfg(not(windows))]
impl Session {
    pub fn start(_: u32, _: &Path, _: &str, _: u64) -> Result<Self> {
        Err(Error::input(
            "native governor observation requires Windows x86-64",
        ))
    }
    pub fn stop(&mut self) -> Result<PathBuf> {
        Err(Error::input(
            "native governor observation requires Windows x86-64",
        ))
    }
}
