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
        sync::mpsc::{self, Receiver, TryRecvError},
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
        messages: Receiver<String>,
        output: PathBuf,
        stopped: bool,
        stop_sent: bool,
        stop_deadline: Option<Instant>,
        game_pid: u32,
        game_start: f64,
        critical: Option<String>,
        close_attempted: bool,
        close_error: Option<String>,
        close_owned_game: Box<dyn FnMut(u32, f64) -> Result<()> + Send>,
    }
    impl Session {
        pub fn start(
            pid: u32,
            start: f64,
            output: &Path,
            name: &str,
            seconds: u64,
            close_owned_game: Box<dyn FnMut(u32, f64) -> Result<()> + Send>,
        ) -> Result<Self> {
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
            let handshake_deadline = Instant::now() + Duration::from_secs(20);
            let (tx, rx) = mpsc::channel();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    match line {
                        Ok(line) => {
                            if tx.send(line).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
            let mut session = Self {
                child,
                stdin,
                messages: rx,
                output: trace,
                stopped: false,
                stop_sent: false,
                stop_deadline: Some(handshake_deadline),
                game_pid: pid,
                game_start: start,
                critical: None,
                close_attempted: false,
                close_error: None,
                close_owned_game,
            };
            loop {
                let left = handshake_deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(Error::input("native observer ready handshake timed out"));
                }
                match session.messages.recv_timeout(left) {
                    Ok(line) => {
                        let message: Value = serde_json::from_str(&line)
                            .map_err(|_| Error::input("native observer handshake was not JSON"))?;
                        if message["critical_cleanup"] == true {
                            session.handle_critical(&message)?;
                            return Err(Error::input(format!(
                                "native observer attach failed in critical_cleanup: {}",
                                session.critical.as_deref().unwrap_or("critical_cleanup")
                            )));
                        }
                        if message["ready"] == true
                            && message["pid"].as_u64() == Some(u64::from(pid))
                        {
                            session.stop_deadline = None;
                            return Ok(session);
                        }
                        return Err(Error::input(
                            "native observer ready handshake did not match game PID",
                        ));
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return Err(Error::input(format!(
                            "native observer exited before ready handshake: {}",
                            session.error_text()
                        )))
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        return Err(Error::input("native observer ready handshake timed out"))
                    }
                }
            }
        }
        fn handle_critical(&mut self, message: &Value) -> Result<()> {
            if message["pid"].as_u64() != Some(u64::from(self.game_pid)) {
                return Err(Error::input(
                    "critical cleanup PID does not match the pinned game",
                ));
            }
            if self.critical.is_none() {
                self.critical = Some(
                    message["reason"]
                        .as_str()
                        .unwrap_or("observer cleanup failed")
                        .to_owned(),
                );
            }
            self.close_game_once();
            Ok(())
        }
        fn close_game_once(&mut self) {
            if self.close_attempted {
                return;
            }
            self.close_attempted = true;
            if let Err(e) = (self.close_owned_game)(self.game_pid, self.game_start) {
                self.close_error = Some(e.to_string());
            }
        }
        fn drain_messages(&mut self) -> Result<()> {
            loop {
                match self.messages.try_recv() {
                    Ok(line) => {
                        let message: Value = serde_json::from_str(&line)
                            .map_err(|_| Error::input("native observer emitted invalid JSON"))?;
                        if message["critical_cleanup"] == true {
                            self.handle_critical(&message)?;
                        }
                    }
                    Err(TryRecvError::Empty | TryRecvError::Disconnected) => return Ok(()),
                }
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
                return self.finished_result();
            }
            let deadline = *self
                .stop_deadline
                .get_or_insert_with(|| Instant::now() + Duration::from_secs(20));
            if !self.stop_sent && self.child.try_wait()?.is_none() {
                self.stop_sent = true;
                if let Some(mut stdin) = self.stdin.take() {
                    match stdin.write_all(b"stop\n").and_then(|_| stdin.flush()) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
                        Err(e) => self.close_error = Some(format!("observer stop pipe: {e}")),
                    }
                }
            }
            loop {
                self.drain_messages()?;
                match self.child.try_wait()? {
                    Some(_) => {
                        self.stopped = true;
                        return self.finished_result();
                    }
                    None if Instant::now() >= deadline => return Err(Error::input(format!("critical_cleanup: native observer did not detach within the single 20 second deadline; game closure: {}", self.close_error.as_deref().unwrap_or("requested")))),
                    None => {}
                }
                if Instant::now() + Duration::from_secs(2) >= deadline && !self.close_attempted {
                    self.critical.get_or_insert_with(|| {
                        "observer failed to detach before stop deadline".into()
                    });
                    self.close_game_once();
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        fn finished_result(&mut self) -> Result<PathBuf> {
            self.drain_messages()?;
            let status = self
                .child
                .try_wait()?
                .ok_or_else(|| Error::input("observer exit status unavailable"))?;
            if let Some(reason) = &self.critical {
                return Err(Error::input(format!(
                    "critical_cleanup: {reason}; owned game close: {}",
                    self.close_error.as_deref().unwrap_or("requested")
                )));
            }
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
            Ok(self.output.clone())
        }
    }
    impl Drop for Session {
        fn drop(&mut self) {
            if !self.stopped {
                let _ = self.stop();
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };

        #[test]
        fn fixture_observer_worker() {
            if std::env::var_os("PR86_TRACE_STOP_FIXTURE").is_some() {
                std::thread::sleep(Duration::from_millis(350));
            }
        }
        fn session(
            deadline: Option<Instant>,
            closes: Arc<AtomicUsize>,
        ) -> (Session, mpsc::Sender<String>) {
            let mut child = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "native_trace::platform::tests::fixture_observer_worker",
                    "--nocapture",
                ])
                .env("PR86_TRACE_STOP_FIXTURE", "1")
                .creation_flags(CREATE_NO_WINDOW)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let stdin = child.stdin.take();
            let (tx, rx) = mpsc::channel();
            let pid = child.id();
            (
                Session {
                    child,
                    stdin,
                    messages: rx,
                    output: PathBuf::from("fixture.jsonl"),
                    stopped: false,
                    stop_sent: false,
                    stop_deadline: deadline,
                    game_pid: pid,
                    game_start: 0.0,
                    critical: None,
                    close_attempted: false,
                    close_error: None,
                    close_owned_game: Box::new(move |_, _| {
                        closes.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    }),
                },
                tx,
            )
        }
        #[test]
        fn stop_and_drop_share_one_deadline_and_one_close_request() {
            let closes = Arc::new(AtomicUsize::new(0));
            let (mut session, _tx) = session(
                Some(Instant::now() + Duration::from_millis(120)),
                Arc::clone(&closes),
            );
            let started = Instant::now();
            assert!(session
                .stop()
                .unwrap_err()
                .to_string()
                .contains("single 20 second deadline"));
            drop(session);
            assert!(
                started.elapsed() < Duration::from_millis(280),
                "Drop repeated the stop wait"
            );
            assert_eq!(closes.load(Ordering::SeqCst), 1);
            std::thread::sleep(Duration::from_millis(360)); // The owned Rust fixture exits itself.
        }
        #[test]
        fn repeated_critical_messages_close_the_owned_target_once() {
            let closes = Arc::new(AtomicUsize::new(0));
            let (mut session, tx) = session(None, Arc::clone(&closes));
            let line = serde_json::json!({"critical_cleanup":true,"pid":session.game_pid,"reason":"injected SetThreadContext refusal"}).to_string();
            tx.send(line.clone()).unwrap();
            tx.send(line).unwrap();
            let error = session.stop().unwrap_err().to_string();
            assert!(error.contains("critical_cleanup"));
            assert_eq!(closes.load(Ordering::SeqCst), 1);
            drop(session);
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
        _: u32,
        _: f64,
        _: &Path,
        _: &str,
        _: u64,
        _: Box<dyn FnMut(u32, f64) -> Result<()> + Send>,
    ) -> Result<Self> {
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
