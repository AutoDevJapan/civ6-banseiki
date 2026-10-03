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
        collections::{BTreeMap, BTreeSet},
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
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum TerminalProof {
        Missing,
        Detached { observer_ok: bool },
        TargetExited { exit_code: Option<u32> },
    }
    enum ObserverMessage {
        Line(String),
        Closed,
    }

    fn property_evidence(path: &Path) -> Result<()> {
        let file = std::fs::File::open(path)?;
        let mut mode_seen = false;
        let mut owner_seen = false;
        let mut reads: BTreeMap<String, Vec<(u64, Option<String>, bool)>> = BTreeMap::new();
        for line in BufReader::new(file).lines() {
            let row: Value = serde_json::from_str(&line?)?;
            if row["type"] == "identity" && row["mode"] == "property" {
                mode_seen = true;
            }
            if row["type"] == "player_getter"
                && row["owner_id_candidate"] == 0
                && row["property_vtable_candidate"].as_str().is_some()
            {
                owner_seen = true;
            }
            if row["type"] == "property_after" && row["owner_id_candidate"] == 0 {
                if let (Some(name), Some(key)) = (
                    row["key_name_candidate"].as_str(),
                    row["key_id_candidate"].as_u64(),
                ) {
                    if matches!(name, "JAC_ACCESSOR_853_NUMBER" | "JAC_ACCESSOR_853_TEXT") {
                        reads.entry(name.into()).or_default().push((
                            key,
                            row["value_first_16_bytes_candidate"]
                                .as_str()
                                .map(str::to_owned),
                            row["value_pointer_candidate"].as_str().is_some(),
                        ));
                    }
                }
            }
        }
        if !mode_seen || !owner_seen {
            return Err(Error::input(
                "property trace has no verified mode or owner-0 getter",
            ));
        }
        let mut key_ids = BTreeSet::new();
        for name in ["JAC_ACCESSOR_853_NUMBER", "JAC_ACCESSOR_853_TEXT"] {
            let Some(events) = reads.get(name) else {
                return Err(Error::input(format!("property trace missed {name}")));
            };
            let stable_ids: BTreeSet<_> = events.iter().map(|(id, _, _)| *id).collect();
            let observed_values: BTreeSet<_> = events
                .iter()
                .filter(|(_, _, pointer)| *pointer)
                .filter_map(|(_, bytes, _)| bytes.as_ref())
                .collect();
            if stable_ids.len() != 1
                || observed_values.len() < 2
                || !events.last().is_some_and(|(_, _, pointer)| !pointer)
            {
                return Err(Error::input(format!(
                    "property trace lacks stable read/update/delete evidence for {name}"
                )));
            }
            key_ids.extend(stable_ids);
        }
        if key_ids.len() != 2 {
            return Err(Error::input(
                "property trace number and text resolved to the same key ID",
            ));
        }
        Ok(())
    }

    fn lifecycle_evidence(path: &Path) -> Result<()> {
        let file = std::fs::File::open(path)?;
        let mut identity = false;
        let mut initial_breakpoint = false;
        let mut ready = false;
        let mut summary = None;
        for line in BufReader::new(file).lines() {
            let row: Value = serde_json::from_str(&line?)?;
            if row["type"] == "identity" && row["mode"] == "lifecycle" {
                identity = true;
            }
            if row["type"] == "debug_exception"
                && row["mode"] == "lifecycle"
                && row["disposition"] == "proven_attach_breakpoint"
                && row["attach_origin_proven"] == true
            {
                initial_breakpoint = true;
            }
            if row["type"] == "ready" {
                ready = true;
            }
            if row["type"].as_str().is_some_and(|kind| {
                matches!(kind, "entry" | "return" | "player_getter")
                    || kind.starts_with("property_")
            }) {
                return Err(Error::input("lifecycle trace contains an ABI observation"));
            }
            if row["type"] == "lifecycle_summary" {
                summary = Some(row);
            }
        }
        let Some(row) = summary else {
            return Err(Error::input("lifecycle trace lacks a terminal summary"));
        };
        if !identity
            || !initial_breakpoint
            || !ready
            || row["mode"] != "lifecycle"
            || row["hardware_breakpoints_installed"] != 0
            || row["debug_register_writes"] != 0
            || row["future_exception_delivery"] != "unknown"
            || !row["thread_events"].as_u64().is_some_and(|count| count > 0)
            || !row["exception_events"]
                .as_u64()
                .is_some_and(|count| count > 0)
            || !row["application_exception_events"].as_u64().is_some()
        {
            return Err(Error::input(
                "lifecycle trace does not prove a no-breakpoint run",
            ));
        }
        Ok(())
    }

    fn trace_mode_matches(path: &Path, mode: &str) -> bool {
        std::fs::File::open(path)
            .ok()
            .and_then(|file| BufReader::new(file).lines().next()?.ok())
            .and_then(|line| serde_json::from_str::<Value>(&line).ok())
            .is_some_and(|row| row["type"] == "identity" && row["mode"] == mode)
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
        mode: String,
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
            mode: &str,
            close_owned_game: Box<dyn FnMut(u32, f64) -> Result<()> + Send>,
        ) -> Result<Self> {
            if !matches!(mode, "governor" | "property" | "lifecycle") {
                return Err(Error::input(
                    "native trace mode must be governor, property or lifecycle",
                ));
            }
            if mode != "lifecycle" {
                return Err(Error::input("hardware breakpoint observation is withheld until per-thread exception delivery is proven; use lifecycle mode"));
            }
            if !(1..=civ6_native_probe_rs::MAX_LIFECYCLE_SECONDS).contains(&seconds) {
                return Err(Error::input(
                    "lifecycle observation must last 1..20 seconds",
                ));
            }
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
            if !civ6_native_probe_rs::is_pinned_dx11_exe(&identity.exe) {
                return Err(Error::input(
                    "native observer target is not the pinned Steam DX11 executable",
                ));
            }
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
                .arg("--mode")
                .arg(mode)
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
                mode: mode.to_owned(),
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
                    if !trace_mode_matches(&session.output, mode) {
                        return Err(session.abort_start(
                            "observer identity mode does not match the requested trace mode",
                        ));
                    }
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
                    } else if let Some(stdin) = self.stdin.as_mut() {
                        if let Err(e) = stdin
                            .write_all(b"game_close_requested\n")
                            .and_then(|_| stdin.flush())
                        {
                            self.close_error = Some(format!(
                                "cannot acknowledge owned game closure to observer: {e}"
                            ));
                        }
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
                if let Some(stdin) = self.stdin.as_mut() {
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
        fn terminal_proof(&self) -> Result<TerminalProof> {
            let file = match std::fs::File::open(&self.output) {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(TerminalProof::Missing)
                }
                Err(e) => return Err(e.into()),
            };
            let mut last: Option<Value> = None;
            let mut process_exit: Option<Option<u32>> = None;
            for line in BufReader::new(file).lines() {
                let Ok(row) = serde_json::from_str::<Value>(&line?) else {
                    return Ok(TerminalProof::Missing);
                };
                if row["type"] == "process_exit" {
                    process_exit = Some(
                        row["exit_code"]
                            .as_u64()
                            .and_then(|v| u32::try_from(v).ok()),
                    );
                }
                last = Some(row);
            }
            let Some(row) = last else {
                return Ok(TerminalProof::Missing);
            };
            if process_exit.is_some()
                || row["terminal_kind"] == "target_exited"
                || row["target_exited"] == true
            {
                let code = process_exit.flatten().or_else(|| {
                    row["process_exit_code"]
                        .as_u64()
                        .and_then(|v| u32::try_from(v).ok())
                });
                return Ok(TerminalProof::TargetExited { exit_code: code });
            }
            if row["type"] == "stop"
                && row["terminal_kind"] == "detached"
                && row["detached"] == true
                && row["target_exited"] == false
            {
                return Ok(TerminalProof::Detached {
                    observer_ok: row["result"] == "ok",
                });
            }
            Ok(TerminalProof::Missing)
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
            let proof = self.terminal_proof().unwrap_or(TerminalProof::Missing);
            let exited = self.identity.exited().unwrap_or(false);
            match proof {
                TerminalProof::TargetExited { .. } => self.phase = Phase::TargetExited,
                TerminalProof::Detached { .. } if !exited => self.phase = Phase::Detached,
                _ if exited => self.phase = Phase::TargetExited,
                _ => {
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
            }
            if let Some(reason) = &self.critical {
                let exit_note = match proof {
                    TerminalProof::TargetExited {
                        exit_code: Some(code),
                    } => format!("; target exit code 0x{code:08x}"),
                    _ => String::new(),
                };
                return Err(Error::input(format!(
                    "critical_cleanup: {reason}{exit_note}; owned game close: {}",
                    self.close_error.as_deref().unwrap_or("requested")
                )));
            }
            if self.phase == Phase::TargetExited {
                let code = match proof {
                    TerminalProof::TargetExited { exit_code } => exit_code
                        .map(|v| format!("0x{v:08x}"))
                        .unwrap_or_else(|| "unknown".into()),
                    _ => "unknown".into(),
                };
                return Err(Error::input(format!("native observer target exited during observation (exit code {code}); observer status {status}")));
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
            if !matches!(proof, TerminalProof::Detached { observer_ok: true }) {
                return Err(Error::input(
                    "observer did not record a successful terminal detach",
                ));
            }
            if self.mode == "property" {
                property_evidence(&self.output)?;
            } else if self.mode == "lifecycle" {
                lifecycle_evidence(&self.output)?;
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
        use windows_sys::Win32::System::Threading::{TerminateProcess, PROCESS_TERMINATE};

        #[test]
        fn property_trace_requires_named_owner_reads_updates_and_deletes() {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("property.jsonl");
            let mut rows = vec![
                serde_json::json!({"type":"identity","mode":"property"}),
                serde_json::json!({"type":"player_getter","owner_id_candidate":0,
                    "property_vtable_candidate":"0x1000"}),
            ];
            for (name, id) in [
                ("JAC_ACCESSOR_853_NUMBER", 17),
                ("JAC_ACCESSOR_853_TEXT", 23),
            ] {
                for bytes in [Some("11".repeat(16)), Some("22".repeat(16)), None] {
                    rows.push(serde_json::json!({"type":"property_after",
                        "owner_id_candidate":0,"key_name_candidate":name,"key_id_candidate":id,
                        "value_pointer_candidate":bytes.as_ref().map(|_| "0x2000"),
                        "value_first_16_bytes_candidate":bytes}));
                }
            }
            let write = |rows: &[Value]| {
                std::fs::write(
                    &path,
                    rows.iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                        + "\n",
                )
                .unwrap();
            };
            write(&rows);
            assert!(property_evidence(&path).is_ok());
            assert!(trace_mode_matches(&path, "property"));
            rows[0]["mode"] = serde_json::json!("governor");
            write(&rows);
            assert!(property_evidence(&path).is_err());
            assert!(!trace_mode_matches(&path, "property"));
            rows[0]["mode"] = serde_json::json!("property");
            rows.pop(); // missing text deletion
            write(&rows);
            assert!(property_evidence(&path).is_err());
        }

        #[test]
        fn lifecycle_trace_requires_zero_breakpoint_changes_and_an_explicit_summary() {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("lifecycle.jsonl");
            let mut rows = vec![
                serde_json::json!({"type":"identity","mode":"lifecycle"}),
                serde_json::json!({"type":"thread_status","thread_id":7,
                    "future_exception_delivery":"unknown"}),
                serde_json::json!({"type":"debug_exception","mode":"lifecycle",
                    "disposition":"proven_attach_breakpoint","attach_origin_proven":true}),
                serde_json::json!({"type":"ready","thread_count":1}),
                serde_json::json!({"type":"lifecycle_summary","mode":"lifecycle",
                    "thread_events":1,"hardware_breakpoints_installed":0,
                    "debug_register_writes":0,"exception_events":1,
                    "application_exception_events":0,"future_exception_delivery":"unknown"}),
            ];
            let write = |rows: &[Value]| {
                std::fs::write(
                    &path,
                    rows.iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join("\n")
                        + "\n",
                )
                .unwrap()
            };
            write(&rows);
            assert!(lifecycle_evidence(&path).is_ok());
            rows[4]["debug_register_writes"] = serde_json::json!(1);
            write(&rows);
            assert!(lifecycle_evidence(&path).is_err());
            rows[4]["debug_register_writes"] = serde_json::json!(0);
            rows.push(serde_json::json!({"type":"property_after"}));
            write(&rows);
            assert!(lifecycle_evidence(&path).is_err());
            rows.pop();
            rows.pop();
            write(&rows);
            assert!(lifecycle_evidence(&path).is_err());
        }

        #[test]
        fn hardware_modes_fail_before_any_observer_or_game_access() {
            for mode in ["governor", "property"] {
                let error = Session::start(
                    999,
                    0.0,
                    Path::new("."),
                    "none",
                    1,
                    mode,
                    Box::new(|_, _| Ok(())),
                )
                .err()
                .unwrap()
                .to_string();
                assert!(error.contains("withheld"));
            }
        }

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
            session_with_close(deadline, closes, false)
        }
        fn session_with_close(
            deadline: Option<Instant>,
            closes: Arc<AtomicUsize>,
            terminate_owned_fixture: bool,
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
                    mode: "governor".into(),
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
                        if terminate_owned_fixture {
                            let raw = unsafe { OpenProcess(PROCESS_TERMINATE, 0, request_pid) };
                            if raw.is_null() {
                                return Err(Error::input(
                                    "cannot open owned Rust fixture for termination",
                                ));
                            }
                            let owned = unsafe { OwnedHandle::from_raw_handle(raw) };
                            if unsafe { TerminateProcess(owned.as_raw_handle() as HANDLE, 1) } == 0
                            {
                                return Err(Error::input("cannot terminate owned Rust fixture"));
                            }
                        }
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
                .wait_ready(Instant::now() + Duration::from_secs(3))
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
            let (mut session, tx) = session_with_close(
                Some(Instant::now() + Duration::from_millis(500)),
                Arc::clone(&closes),
                true,
            );
            session.phase = Phase::Attaching;
            let pid = session.identity.pid;
            let ready_error = session
                .wait_ready(Instant::now() - Duration::from_millis(1))
                .unwrap_err();
            tx.send(ObserverMessage::Line(
                serde_json::json!({"ready":true,"pid":pid}).to_string(),
            ))
            .unwrap();
            tx.send(ObserverMessage::Line(serde_json::json!({"critical_cleanup":true,"pid":pid,"reason":"late restore failure"}).to_string())).unwrap();
            let result = session.abort_start(&ready_error.to_string());
            assert!(result.to_string().contains("ready handshake timed out"));
            assert_eq!(closes.load(Ordering::SeqCst), 1);
            assert_eq!(session.phase, Phase::TargetExited);
        }
        #[test]
        fn critical_just_before_ready_timeout_closes_once_and_fails_startup() {
            let closes = Arc::new(AtomicUsize::new(0));
            let (mut session, tx) = session_with_close(
                Some(Instant::now() + Duration::from_millis(500)),
                Arc::clone(&closes),
                true,
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
            let ready_error = session
                .wait_ready(Instant::now() + Duration::from_millis(40))
                .unwrap_err();
            let result = session.abort_start(&ready_error.to_string());
            tx.send(ObserverMessage::Line(
                serde_json::json!({"critical_cleanup":true,"pid":pid,"reason":"too late"})
                    .to_string(),
            ))
            .unwrap();
            assert!(result.to_string().contains("critical_cleanup"));
            assert_eq!(closes.load(Ordering::SeqCst), 1);
            assert_eq!(session.phase, Phase::Unresolved);
            drop(session);
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
            assert_eq!(session.terminal_proof().unwrap(), TerminalProof::Missing);
            for (row, expected) in [
                ("{bad json}\n", TerminalProof::Missing),
                (
                    "{\"type\":\"stop\",\"detached\":false,\"result\":\"ok\"}\n",
                    TerminalProof::Missing,
                ),
                (
                    "{\"type\":\"entry\",\"detached\":true,\"result\":\"ok\"}\n",
                    TerminalProof::Missing,
                ),
                (
                    "{\"type\":\"stop\",\"terminal_kind\":\"detached\",\"detached\":true,\"target_exited\":false,\"result\":\"error\"}\n",
                    TerminalProof::Detached { observer_ok: false },
                ),
                (
                    "{\"type\":\"stop\",\"terminal_kind\":\"detached\",\"detached\":true,\"target_exited\":false,\"result\":\"ok\"}\n",
                    TerminalProof::Detached { observer_ok: true },
                ),
                (
                    "{\"type\":\"process_exit\",\"exit_code\":0}\n{\"type\":\"stop\",\"terminal_kind\":\"detached\",\"detached\":true,\"target_exited\":false,\"result\":\"ok\"}\n",
                    TerminalProof::TargetExited { exit_code: Some(0) },
                ),
                (
                    "{\"type\":\"stop\",\"terminal_kind\":\"target_exited\",\"detached\":false,\"target_exited\":true,\"process_exit_code\":3221225477,\"result\":\"error\"}\n",
                    TerminalProof::TargetExited { exit_code: Some(3221225477) },
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
            let (mut session, _tx) = session(None, Arc::clone(&closes));
            session.child.wait().unwrap();
            assert!(session.identity.exited().unwrap());
            session.stop_deadline = Some(Instant::now() + Duration::from_secs(2));
            let error = session.finished_result().unwrap_err().to_string();
            assert!(error.contains("detach") || error.contains("exited"));
            assert_eq!(session.phase, Phase::TargetExited);
            assert_eq!(closes.load(Ordering::SeqCst), 0);
        }
        #[test]
        fn even_zero_exit_code_cannot_become_a_successful_detach() {
            for (code, expected) in [(0u32, "0x00000000"), (0xc0000005, "0xc0000005")] {
                let closes = Arc::new(AtomicUsize::new(0));
                let (mut session, _tx) = session(None, Arc::clone(&closes));
                let path = std::env::temp_dir().join(format!(
                    "pr86-terminal-exit-{}-{}-{code}.jsonl",
                    std::process::id(),
                    session.identity.pid
                ));
                std::fs::write(&path, format!(
                    "{{\"type\":\"process_exit\",\"exit_code\":{code}}}\n{{\"type\":\"stop\",\"terminal_kind\":\"detached\",\"detached\":true,\"target_exited\":false,\"result\":\"ok\"}}\n"
                )).unwrap();
                session.output = path.clone();
                session.child.wait().unwrap();
                let error = session.finished_result().unwrap_err().to_string();
                assert!(error.contains(expected), "{error}");
                assert_eq!(session.phase, Phase::TargetExited);
                assert_eq!(closes.load(Ordering::SeqCst), 0);
                std::fs::remove_file(path).unwrap();
            }
        }
        #[test]
        fn child_exit_waits_for_late_critical_output_before_terminal_decision() {
            let closes = Arc::new(AtomicUsize::new(0));
            let (mut session, tx) = session(None, Arc::clone(&closes));
            session.reader_closed = false;
            let pid = session.identity.pid;
            session.child.wait().unwrap();
            assert!(session.identity.exited().unwrap());
            session.stop_deadline = Some(Instant::now() + Duration::from_secs(2));
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(30));
                let _ = tx.send(ObserverMessage::Line(serde_json::json!({"critical_cleanup":true,"pid":pid,"reason":"late output after child exit"}).to_string()));
                let _ = tx.send(ObserverMessage::Closed);
            });
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
        _: &str,
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
