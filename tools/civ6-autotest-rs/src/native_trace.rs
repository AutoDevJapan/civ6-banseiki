//! Lifecycle of the separate PR86 read-only native observer.
use crate::{Error, Result};
#[cfg(windows)]
use civ6_native_probe_rs::startup_contract::{
    CLOSE_RESERVE, PARENT_READY as READY_BUDGET, TOTAL as STARTUP_TOTAL,
};
use std::path::{Path, PathBuf};

#[cfg(windows)]
mod platform {
    use super::*;
    use serde_json::Value;
    use std::{
        ffi::OsString,
        io::{BufRead, BufReader, Read, Write},
        os::windows::{
            ffi::OsStringExt,
            io::{AsRawHandle, FromRawHandle, OwnedHandle},
            process::CommandExt,
        },
        process::{Child, ChildStdin, Command, Stdio},
        sync::mpsc::{self, Receiver, TryRecvError},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{FILETIME, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{
            GetCurrentProcess, GetProcessTimes, OpenProcess, QueryFullProcessImageNameW,
            WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
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
    fn image_path(handle: HANDLE) -> Result<PathBuf> {
        let mut image = vec![0u16; 32768];
        let mut len = image.len() as u32;
        if unsafe { QueryFullProcessImageNameW(handle, 0, image.as_mut_ptr(), &mut len) } == 0
            || len == 0
            || len as usize >= image.len()
        {
            return Err(Error::input("cannot read complete Civ6 executable path"));
        }
        Ok(PathBuf::from(OsString::from_wide(&image[..len as usize])))
    }
    /// The restart backend uses this exact floating point operation. Tolerance
    /// is zero seconds: accepting a nearby creation time could authorize a
    /// different process after PID reuse.
    fn restart_unix_start(raw_filetime: u64) -> f64 {
        (raw_filetime as f64) / 10_000_000.0 - 11_644_473_600.0
    }
    fn lease_covers_start(acquired: f64, now: f64) -> bool {
        acquired.is_finite()
            && now.is_finite()
            && acquired <= now
            && now + STARTUP_TOTAL.as_secs_f64() < acquired + 1800.0
    }
    struct PinnedGameIdentity {
        handle: OwnedHandle,
        pid: u32,
        created: u64,
        restart_start: f64,
        exe: PathBuf,
    }
    impl PinnedGameIdentity {
        fn pin(pid: u32, expected_restart_start: f64) -> Result<Self> {
            if pid == 0 || !expected_restart_start.is_finite() {
                return Err(Error::input("invalid pinned Civ6 identity"));
            }
            let raw = unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                    0,
                    pid,
                )
            };
            if raw.is_null() {
                return Err(Error::input(format!(
                    "cannot pin Civ6 PID {pid}: {}",
                    std::io::Error::last_os_error()
                )));
            }
            let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
            let created = created(handle.as_raw_handle() as HANDLE)?;
            let actual = restart_unix_start(created);
            if expected_restart_start != actual {
                return Err(Error::input(format!("Civ6 PID {pid} creation time changed before native observation (zero tolerance)")));
            }
            let exe = image_path(handle.as_raw_handle() as HANDLE)?;
            let identity = Self {
                handle,
                pid,
                created,
                restart_start: actual,
                exe,
            };
            if identity.exited()? {
                return Err(Error::input("pinned Civ6 exited before native observation"));
            }
            Ok(identity)
        }
        fn exited(&self) -> Result<bool> {
            match unsafe { WaitForSingleObject(self.handle.as_raw_handle() as HANDLE, 0) } {
                WAIT_OBJECT_0 => Ok(true),
                WAIT_TIMEOUT => Ok(false),
                _ => Err(Error::input("cannot verify pinned Civ6 liveness")),
            }
        }
        fn verify_for_close(&self) -> Result<bool> {
            if self.exited()? {
                return Ok(false);
            }
            let handle = self.handle.as_raw_handle() as HANDLE;
            if created(handle)? != self.created || image_path(handle)? != self.exe {
                return Err(Error::input("pinned Civ6 identity changed before close"));
            }
            Ok(true)
        }
    }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Phase {
        Attaching,
        Ready,
        Stopping,
        Critical,
        Detached,
        TargetExited,
        Unresolved,
    }
    enum ObserverMessage {
        Line(String),
        Closed,
    }
    impl Phase {
        fn terminal(self) -> bool {
            matches!(self, Self::Detached | Self::TargetExited | Self::Unresolved)
        }
    }
    pub struct Session {
        child: Child,
        stdin: Option<ChildStdin>,
        messages: Receiver<ObserverMessage>,
        reader_closed: bool,
        output: PathBuf,
        identity: PinnedGameIdentity,
        phase: Phase,
        failure: Option<String>,
        stop_sent: bool,
        stop_deadline: Option<Instant>,
        close_reserve: Duration,
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
            let started = Instant::now();
            let ready_deadline = started + READY_BUDGET;
            let total_deadline = started + STARTUP_TOTAL;
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
            let record: Value = serde_json::from_slice(&std::fs::read(&lock)?)?;
            let acquired = record["acquired_at"]
                .as_f64()
                .ok_or_else(|| Error::input("live_chain acquisition time missing"))?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| Error::input("live_chain clock unavailable"))?
                .as_secs_f64();
            if !lease_covers_start(acquired, now) {
                return Err(Error::input(
                    "live_chain lease cannot cover native observer startup",
                ));
            }
            let identity = PinnedGameIdentity::pin(pid, start)?;
            if Instant::now() >= ready_deadline {
                return Err(Error::input(
                    "native observer preflight exceeded the ready budget before spawn",
                ));
            }
            let controller_pid = std::process::id();
            let controller_created = created(unsafe { GetCurrentProcess() })?;
            let exe = std::env::current_exe()?.with_file_name("civ6-native-probe.exe");
            let trace = output.join(format!("{name}.jsonl"));
            let child = Command::new(&exe)
                .args(["observe", "--pid"])
                .arg(pid.to_string())
                .arg("--created")
                .arg(identity.created.to_string())
                .arg("--exe")
                .arg(&identity.exe)
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
            let (tx, rx) = mpsc::channel();
            let mut session = Self {
                child,
                stdin: None,
                messages: rx,
                reader_closed: false,
                output: trace,
                identity,
                phase: Phase::Attaching,
                failure: None,
                stop_sent: false,
                stop_deadline: Some(total_deadline),
                close_reserve: CLOSE_RESERVE,
                critical: None,
                close_attempted: false,
                close_error: None,
                close_owned_game,
            };
            session.stdin = session.child.stdin.take();
            let stdout = match session.child.stdout.take() {
                Some(stdout) => stdout,
                None => {
                    drop(tx);
                    return Err(session.abort_start("observer stdout unavailable"));
                }
            };
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    match line {
                        Ok(line) => {
                            if tx.send(ObserverMessage::Line(line)).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let _ = tx.send(ObserverMessage::Closed);
            });
            match session.wait_ready(ready_deadline) {
                Ok(()) => {
                    session.phase = Phase::Ready;
                    session.stop_deadline = None;
                    Ok(session)
                }
                Err(e) => Err(session.abort_start(&e.to_string())),
            }
        }
        fn wait_ready(&mut self, ready_deadline: Instant) -> Result<()> {
            loop {
                let left = ready_deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Err(Error::input("native observer ready handshake timed out"));
                }
                match self.messages.recv_timeout(left) {
                    Ok(raw) => {
                        let Some(message) = self.process_message(raw)? else {
                            return Err(Error::input("native observer output closed before ready"));
                        };
                        if message["critical_cleanup"] == true {
                            return Err(Error::input(
                                "native observer attach failed in critical_cleanup",
                            ));
                        }
                        if message["ready"] == true
                            && message["pid"].as_u64() == Some(u64::from(self.identity.pid))
                            && Instant::now() < ready_deadline
                        {
                            return Ok(());
                        }
                        return Err(Error::input(
                            "native observer ready handshake was late or did not match game PID",
                        ));
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        self.reader_closed = true;
                        return Err(Error::input(
                            "native observer exited before ready handshake",
                        ));
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        return Err(Error::input("native observer ready handshake timed out"))
                    }
                }
            }
        }
        fn abort_start(&mut self, reason: &str) -> Error {
            self.failure.get_or_insert_with(|| reason.to_owned());
            if self.phase == Phase::Attaching {
                self.phase = Phase::Stopping;
            }
            let outcome = self.stop();
            Error::input(format!(
                "{reason}; cleanup: {}",
                outcome
                    .err()
                    .map_or_else(|| "detached".into(), |e| e.to_string())
            ))
        }
        fn handle_critical(&mut self, message: &Value) -> Result<()> {
            if message["pid"].as_u64() != Some(u64::from(self.identity.pid)) {
                return Err(Error::input(
                    "critical cleanup PID does not match the pinned game",
                ));
            }
            if !self.phase.terminal() {
                self.phase = Phase::Critical;
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
        fn process_message(&mut self, raw: ObserverMessage) -> Result<Option<Value>> {
            match raw {
                ObserverMessage::Closed => {
                    self.reader_closed = true;
                    Ok(None)
                }
                ObserverMessage::Line(line) => {
                    let message: Value = serde_json::from_str(&line)
                        .map_err(|_| Error::input("native observer emitted invalid JSON"))?;
                    if message["critical_cleanup"] == true {
                        self.handle_critical(&message)?;
                    }
                    Ok(Some(message))
                }
            }
        }
        fn close_game_once(&mut self) {
            if self.close_attempted {
                return;
            }
            self.close_attempted = true;
            match self.identity.verify_for_close() {
                Ok(false) => self.phase = Phase::TargetExited,
                Ok(true) => {
                    if let Err(e) =
                        (self.close_owned_game)(self.identity.pid, self.identity.restart_start)
                    {
                        self.close_error = Some(e.to_string());
                    }
                }
                Err(e) => self.close_error = Some(e.to_string()),
            }
        }
        fn drain_messages(&mut self) -> Result<()> {
            loop {
                match self.messages.try_recv() {
                    Ok(raw) => {
                        self.process_message(raw)?;
                    }
                    Err(TryRecvError::Empty) => return Ok(()),
                    Err(TryRecvError::Disconnected) => {
                        self.reader_closed = true;
                        return Ok(());
                    }
                }
            }
        }
        fn drain_through_output_close(&mut self) -> Result<()> {
            while !self.reader_closed {
                let deadline = self.stop_deadline.unwrap_or_else(Instant::now);
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    self.close_game_once();
                    self.phase = Phase::Unresolved;
                    return Err(Error::input("critical_cleanup unresolved: observer output did not close by the shared deadline"));
                }
                match self
                    .messages
                    .recv_timeout(remaining.min(Duration::from_millis(50)))
                {
                    Ok(raw) => {
                        self.process_message(raw)?;
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => self.reader_closed = true,
                }
            }
            Ok(())
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
            if self.phase.terminal() {
                return self.finished_result();
            }
            if matches!(self.phase, Phase::Ready | Phase::Attaching) {
                self.phase = Phase::Stopping;
            }
            let deadline = *self
                .stop_deadline
                .get_or_insert_with(|| Instant::now() + STARTUP_TOTAL);
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
                        return self.finished_result();
                    }
                    None if Instant::now() >= deadline => {
                        if !self.close_attempted {
                            self.critical.get_or_insert_with(|| {
                                "observer did not detach by the shared deadline".into()
                            });
                            self.phase = Phase::Critical;
                            self.close_game_once();
                        }
                        self.phase = if self.identity.exited().unwrap_or(false) {
                            Phase::TargetExited
                        } else {
                            Phase::Unresolved
                        };
                        return Err(Error::input(format!("critical_cleanup: native observer did not confirm detach within the single 20 second deadline; game closure: {}", self.close_error.as_deref().unwrap_or("requested"))));
                    }
                    None => {}
                }
                if Instant::now() + self.close_reserve >= deadline && !self.close_attempted {
                    self.critical.get_or_insert_with(|| {
                        "observer failed to detach before stop deadline".into()
                    });
                    self.phase = Phase::Critical;
                    self.close_game_once();
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        fn terminal_proof(&self) -> Result<(bool, bool)> {
            let file = match std::fs::File::open(&self.output) {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((false, false)),
                Err(e) => return Err(e.into()),
            };
            let mut last = None;
            for line in BufReader::new(file).lines() {
                last = Some(line?);
            }
            let Some(last) = last else {
                return Ok((false, false));
            };
            let Ok(row) = serde_json::from_str::<Value>(&last) else {
                return Ok((false, false));
            };
            let detached = row["type"] == "stop" && row["detached"] == true;
            Ok((detached, detached && row["result"] == "ok"))
        }
        fn finished_result(&mut self) -> Result<PathBuf> {
            self.drain_messages()?;
            if self.phase == Phase::Unresolved {
                return Err(Error::input("critical_cleanup unresolved: observer outcome is unknown after the shared deadline"));
            }
            let Some(status) = self.child.try_wait()? else {
                return Err(Error::input(
                    "observer exit status unavailable within the shared deadline",
                ));
            };
            self.drain_through_output_close()?;
            let (detached, observer_ok) = self.terminal_proof().unwrap_or((false, false));
            let exited = self.identity.exited().unwrap_or(false);
            if detached {
                self.phase = Phase::Detached;
            } else if exited {
                self.phase = Phase::TargetExited;
            } else {
                self.close_game_once();
                let deadline = self.stop_deadline.unwrap_or_else(Instant::now);
                while Instant::now() < deadline {
                    if self.identity.exited().unwrap_or(false) {
                        self.phase = Phase::TargetExited;
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                if self.phase != Phase::TargetExited {
                    self.phase = Phase::Unresolved;
                    return Err(Error::input("critical_cleanup unresolved: observer exited without confirmed detach or game exit"));
                }
            }
            if let Some(reason) = &self.critical {
                return Err(Error::input(format!(
                    "critical_cleanup: {reason}; owned game close: {}",
                    self.close_error.as_deref().unwrap_or("requested")
                )));
            }
            if let Some(reason) = &self.failure {
                return Err(Error::input(format!(
                    "native observer startup failed: {reason}; terminal state: {:?}",
                    self.phase
                )));
            }
            if !status.success() {
                return Err(Error::input(format!(
                    "native observer exited with {status}: {}; inspect {}",
                    self.error_text(),
                    self.output.display()
                )));
            }
            if !observer_ok {
                return Err(Error::input(
                    "observer did not record a successful terminal detach",
                ));
            }
            if self.phase == Phase::TargetExited {
                return Err(Error::input(
                    "owned Civ6 exited before a confirmed normal detach",
                ));
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
            if !self.phase.terminal() {
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
        ) -> (Session, mpsc::Sender<ObserverMessage>) {
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
            let raw_created = created(child.as_raw_handle() as HANDLE).unwrap();
            let identity = PinnedGameIdentity::pin(pid, restart_unix_start(raw_created)).unwrap();
            let expected_start = identity.restart_start;
            (
                Session {
                    child,
                    stdin,
                    messages: rx,
                    reader_closed: true,
                    output: PathBuf::from("fixture.jsonl"),
                    identity,
                    phase: Phase::Ready,
                    failure: None,
                    stop_sent: false,
                    stop_deadline: deadline,
                    close_reserve: Duration::from_millis(20),
                    critical: None,
                    close_attempted: false,
                    close_error: None,
                    close_owned_game: Box::new(move |request_pid, request_start| {
                        assert_eq!(request_pid, pid);
                        assert_eq!(request_start, expected_start);
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
            let line = serde_json::json!({"critical_cleanup":true,"pid":session.identity.pid,"reason":"injected SetThreadContext refusal"}).to_string();
            tx.send(ObserverMessage::Line(line.clone())).unwrap();
            tx.send(ObserverMessage::Line(line)).unwrap();
            let error = session.stop().unwrap_err().to_string();
            assert!(error.contains("critical_cleanup"));
            assert_eq!(closes.load(Ordering::SeqCst), 1);
            drop(session);
        }
        #[test]
        fn captured_start_mismatch_is_rejected_before_observer_spawn() {
            let closes = Arc::new(AtomicUsize::new(0));
            let (mut session, _tx) = session(
                Some(Instant::now() + Duration::from_millis(120)),
                Arc::clone(&closes),
            );
            assert!(PinnedGameIdentity::pin(
                session.identity.pid,
                session.identity.restart_start + 1.0
            )
            .err()
            .unwrap()
            .to_string()
            .contains("creation time changed"));
            assert_eq!(closes.load(Ordering::SeqCst), 0);
            session.phase = Phase::Unresolved;
            drop(session);
            std::thread::sleep(Duration::from_millis(360));
        }
        #[test]
        fn shared_lease_must_cover_the_whole_startup_budget() {
            assert!(lease_covers_start(
                100.0,
                1879.0 - STARTUP_TOTAL.as_secs_f64()
            ));
            assert!(!lease_covers_start(
                100.0,
                1900.0 - STARTUP_TOTAL.as_secs_f64()
            ));
            assert!(!lease_covers_start(100.0, 99.0));
            assert!(!lease_covers_start(f64::NAN, 100.0));
        }
        #[test]
        fn wrong_pinned_game_identity_never_calls_termination() {
            let closes = Arc::new(AtomicUsize::new(0));
            let (mut session, _tx) = session(
                Some(Instant::now() + Duration::from_millis(120)),
                Arc::clone(&closes),
            );
            session.identity.created += 10_000_000;
            session.close_game_once();
            assert_eq!(closes.load(Ordering::SeqCst), 0);
            assert!(session
                .close_error
                .as_deref()
                .unwrap()
                .contains("identity changed"));
            session.phase = Phase::Unresolved;
            drop(session);
            std::thread::sleep(Duration::from_millis(360));
        }
        #[test]
        fn ready_is_accepted_only_inside_its_reserved_window() {
            let closes = Arc::new(AtomicUsize::new(0));
            let (mut first, tx) =
                session(Some(Instant::now() + Duration::from_millis(500)), closes);
            first.phase = Phase::Attaching;
            tx.send(ObserverMessage::Line(
                serde_json::json!({"ready":true,"pid":first.identity.pid}).to_string(),
            ))
            .unwrap();
            assert!(first
                .wait_ready(Instant::now() + Duration::from_millis(100))
                .is_ok());
            assert_eq!(first.phase, Phase::Attaching); // start() performs the Ready transition.
            first.phase = Phase::Unresolved;
            drop(first);
            std::thread::sleep(Duration::from_millis(360));

            let closes = Arc::new(AtomicUsize::new(0));
            let (mut session, tx) =
                session(Some(Instant::now() + Duration::from_millis(500)), closes);
            session.phase = Phase::Attaching;
            tx.send(ObserverMessage::Line(
                serde_json::json!({"ready":true,"pid":session.identity.pid}).to_string(),
            ))
            .unwrap();
            assert!(session
                .wait_ready(Instant::now() - Duration::from_millis(1))
                .is_err());
            session.phase = Phase::Unresolved;
            drop(session);
            std::thread::sleep(Duration::from_millis(360));
        }
        #[test]
        fn critical_just_after_ready_timeout_is_still_consumed_once() {
            let closes = Arc::new(AtomicUsize::new(0));
            let (mut session, tx) = session(
                Some(Instant::now() + Duration::from_millis(500)),
                Arc::clone(&closes),
            );
            session.phase = Phase::Attaching;
            let pid = session.identity.pid;
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(100));
                let _ = tx.send(ObserverMessage::Line(
                    serde_json::json!({"ready":true,"pid":pid}).to_string(),
                ));
                std::thread::sleep(Duration::from_millis(25));
                let _ = tx.send(ObserverMessage::Line(serde_json::json!({"critical_cleanup":true,"pid":pid,"reason":"late restore failure"}).to_string()));
            });
            let ready_error = session
                .wait_ready(Instant::now() + Duration::from_millis(80))
                .unwrap_err();
            let result = session.abort_start(&ready_error.to_string());
            assert!(result.to_string().contains("ready handshake timed out"));
            assert_eq!(closes.load(Ordering::SeqCst), 1);
            assert_eq!(session.phase, Phase::TargetExited);
        }
        #[test]
        fn critical_just_before_ready_timeout_closes_once_and_fails_startup() {
            let closes = Arc::new(AtomicUsize::new(0));
            let (mut session, tx) = session(
                Some(Instant::now() + Duration::from_millis(500)),
                Arc::clone(&closes),
            );
            session.phase = Phase::Attaching;
            let pid = session.identity.pid;
            tx.send(ObserverMessage::Line(serde_json::json!({"critical_cleanup":true,"pid":pid,"reason":"restore refused before ready"}).to_string())).unwrap();
            assert!(session
                .wait_ready(Instant::now() + Duration::from_millis(100))
                .unwrap_err()
                .to_string()
                .contains("critical_cleanup"));
            let result = session.abort_start("critical cleanup during attach");
            assert!(result.to_string().contains("critical_cleanup"));
            assert_eq!(closes.load(Ordering::SeqCst), 1);
            assert_eq!(session.phase, Phase::TargetExited);
        }
        #[test]
        fn notification_after_total_deadline_cannot_skip_owned_close() {
            let closes = Arc::new(AtomicUsize::new(0));
            let (mut session, tx) = session(
                Some(Instant::now() + Duration::from_millis(180)),
                Arc::clone(&closes),
            );
            session.phase = Phase::Attaching;
            let pid = session.identity.pid;
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(230));
                let _ = tx.send(ObserverMessage::Line(
                    serde_json::json!({"critical_cleanup":true,"pid":pid,"reason":"too late"})
                        .to_string(),
                ));
            });
            let ready_error = session
                .wait_ready(Instant::now() + Duration::from_millis(40))
                .unwrap_err();
            let result = session.abort_start(&ready_error.to_string());
            assert!(result.to_string().contains("critical_cleanup"));
            assert_eq!(closes.load(Ordering::SeqCst), 1);
            assert_eq!(session.phase, Phase::Unresolved);
            drop(session);
            std::thread::sleep(Duration::from_millis(220));
        }
        #[test]
        fn terminal_detach_record_requires_exact_json_state() {
            let closes = Arc::new(AtomicUsize::new(0));
            let (mut session, _tx) =
                session(Some(Instant::now() + Duration::from_millis(500)), closes);
            let path = std::env::temp_dir().join(format!(
                "pr86-native-terminal-{}-{}.jsonl",
                std::process::id(),
                session.identity.pid
            ));
            session.output = path.clone();
            assert_eq!(session.terminal_proof().unwrap(), (false, false));
            for (row, expected) in [
                ("{bad json}\n", (false, false)),
                (
                    "{\"type\":\"stop\",\"detached\":false,\"result\":\"ok\"}\n",
                    (false, false),
                ),
                (
                    "{\"type\":\"entry\",\"detached\":true,\"result\":\"ok\"}\n",
                    (false, false),
                ),
                (
                    "{\"type\":\"stop\",\"detached\":true,\"result\":\"error\"}\n",
                    (true, false),
                ),
                (
                    "{\"type\":\"stop\",\"detached\":true,\"result\":\"ok\"}\n",
                    (true, true),
                ),
            ] {
                std::fs::write(&path, row).unwrap();
                assert_eq!(session.terminal_proof().unwrap(), expected);
            }
            std::fs::remove_file(path).unwrap();
            session.phase = Phase::Unresolved;
            drop(session);
            std::thread::sleep(Duration::from_millis(360));
        }
        #[test]
        fn exited_target_without_detach_proof_is_never_success() {
            let closes = Arc::new(AtomicUsize::new(0));
            let (mut session, _tx) = session(
                Some(Instant::now() + Duration::from_millis(600)),
                Arc::clone(&closes),
            );
            std::thread::sleep(Duration::from_millis(380));
            let error = session.finished_result().unwrap_err().to_string();
            assert!(error.contains("detach") || error.contains("exited"));
            assert_eq!(session.phase, Phase::TargetExited);
            assert_eq!(closes.load(Ordering::SeqCst), 0);
        }
        #[test]
        fn child_exit_waits_for_late_critical_output_before_terminal_decision() {
            let closes = Arc::new(AtomicUsize::new(0));
            let (mut session, tx) = session(
                Some(Instant::now() + Duration::from_millis(850)),
                Arc::clone(&closes),
            );
            session.reader_closed = false;
            let pid = session.identity.pid;
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(410));
                let _ = tx.send(ObserverMessage::Line(serde_json::json!({"critical_cleanup":true,"pid":pid,"reason":"late output after child exit"}).to_string()));
                let _ = tx.send(ObserverMessage::Closed);
            });
            std::thread::sleep(Duration::from_millis(380));
            let error = session.finished_result().unwrap_err().to_string();
            assert!(error.contains("late output after child exit"));
            assert_eq!(session.phase, Phase::TargetExited);
            assert_eq!(closes.load(Ordering::SeqCst), 0);
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
