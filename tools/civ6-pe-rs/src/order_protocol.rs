//! One owner and one finite finish transaction for the observer subprocess.
//! Pipe/application failures request STOP. Only unconfirmed restoration permits
//! recovery against the already pinned target; PIDs are never reopened here.
use super::{restored_stop, SCHEMA};
use serde_json::{json, Value};
use std::{
    io::{self, Read, Write},
    os::windows::io::AsRawHandle,
    process::{Child, ExitStatus},
    sync::mpsc::{Receiver, TryRecvError},
    time::{Duration, Instant},
};
use windows_sys::Win32::System::Pipes::PeekNamedPipe;

pub const FINISH_BUDGET: Duration = Duration::from_secs(20);
const RESTORE_GRACE: Duration = Duration::from_secs(12);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChildPhase {
    AwaitGo,
    Granted,
    EnterSent,
    NativeCallAttempted,
    NeverAttached,
}
pub struct Admission {
    phase: ChildPhase,
    identity: Value,
    token: String,
    buffer: Vec<u8>,
    pending_stop: bool,
}
fn emit_control(row: &Value) -> Result<(), String> {
    writeln!(io::stdout(), "{row}")
        .and_then(|_| io::stdout().flush())
        .map_err(|e| e.to_string())
}
impl Admission {
    pub fn new(identity: Value, token: String) -> Self {
        Self {
            phase: ChildPhase::AwaitGo,
            identity,
            token,
            buffer: Vec::new(),
            pending_stop: false,
        }
    }
    pub fn await_go(&mut self) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut seen = false;
        loop {
            if Instant::now() >= deadline {
                return Err("GO timeout before attach".into());
            }
            for line in commands(&mut self.buffer)? {
                if !seen && line == format!("GO {}", self.token) {
                    seen = true;
                } else {
                    return Err("cancelled or invalid coalesced GO input".into());
                }
            }
            if seen && self.buffer.is_empty() {
                self.phase = ChildPhase::Granted;
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    pub fn enter_and_call<T>(
        &mut self,
        call: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        if self.phase != ChildPhase::Granted {
            return Err("attach requires granted admission".into());
        }
        if !commands(&mut self.buffer)?.is_empty() || !self.buffer.is_empty() {
            return Err("cancelled before ATTACH_ENTER".into());
        }
        // Once ENTER is published no NEVER_ATTACHED may be emitted. The parent
        // commits its recovery ownership before acknowledging this transition.
        self.phase = ChildPhase::EnterSent;
        let mut row = self.identity.clone();
        row["type"] = json!("ATTACH_ENTER");
        emit_control(&row)?;
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            if Instant::now() >= deadline {
                return Err("ENTER acknowledgement timeout".into());
            }
            let mut ack = false;
            for line in commands(&mut self.buffer)? {
                if !ack && line == format!("ENTER_ACK {}", self.token) {
                    ack = true;
                } else if ack {
                    self.pending_stop = true;
                } else {
                    return Err("ENTER cancelled before native invocation".into());
                }
            }
            if ack {
                self.pending_stop |= !self.buffer.is_empty();
                self.phase = ChildPhase::NativeCallAttempted;
                return call();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    pub fn stop_requested(&self) -> bool {
        self.pending_stop
    }
}
impl Drop for Admission {
    fn drop(&mut self) {
        if matches!(self.phase, ChildPhase::AwaitGo | ChildPhase::Granted) {
            self.phase = ChildPhase::NeverAttached;
            let mut row = self.identity.clone();
            row["type"] = json!("NEVER_ATTACHED");
            row["native_call_attempted"] = json!(false);
            row["terminal_kind"] = json!("never_attached");
            let _ = emit_control(&row);
        }
    }
}

/// Call only after the event's ownership and continuation disposition have
/// been stored. An I/O error is evidence loss, never a new exception owner.
pub fn log_classified<T>(
    disposition: T,
    write: impl FnOnce() -> io::Result<()>,
) -> (T, io::Result<()>) {
    (disposition, write())
}

pub fn classify_then_record<S>(
    state: &mut S,
    classify: impl FnOnce(&mut S) -> Result<i32, String>,
    record: impl FnOnce(&mut S) -> io::Result<()>,
) -> Result<(), String> {
    let disposition = classify(state)?;
    let (_, result) = log_classified(disposition, || record(state));
    result.map_err(|e| e.to_string())
}

pub struct PipeLines<R> {
    reader: R,
    buffer: Vec<u8>,
    pub closed: bool,
}
impl<R: Read + AsRawHandle> PipeLines<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
            closed: false,
        }
    }
    pub fn poll(&mut self) -> io::Result<Vec<String>> {
        if self.closed {
            return Ok(Vec::new());
        }
        let mut available = 0;
        if unsafe {
            PeekNamedPipe(
                self.reader.as_raw_handle(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            )
        } == 0
        {
            let e = io::Error::last_os_error();
            if e.raw_os_error() == Some(109) {
                self.closed = true;
            } else {
                return Err(e);
            }
        } else if available > 0 {
            let mut bytes = vec![0; available.min(8192) as usize];
            let n = self.reader.read(&mut bytes)?;
            if n == 0 {
                self.closed = true;
            }
            self.buffer.extend_from_slice(&bytes[..n]);
        }
        if self.buffer.len() > 1_048_576 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "observer row too long",
            ));
        }
        let mut lines = Vec::new();
        while let Some(end) = self.buffer.iter().position(|b| *b == b'\n') {
            let line: Vec<_> = self.buffer.drain(..=end).collect();
            lines.push(
                std::str::from_utf8(&line)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                    .trim_end_matches(['\r', '\n'])
                    .into(),
            );
        }
        if self.closed && !self.buffer.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "partial observer row",
            ));
        }
        Ok(lines)
    }
}

/// Recovery authority is supplied by the existing pinned native controller.
/// The critical wait returns only after OS-confirmed target exit. It owns the
/// original target HANDLE throughout; no new PID lookup is allowed.
pub trait Recovery {
    fn authorize(&mut self) -> Result<(), String>;
    fn request_close(&mut self) -> Result<bool, String>;
    fn target_exited(&mut self) -> bool;
    fn wait_target_exit(&mut self);
}
#[derive(Clone)]
pub enum Witness {
    Native {
        pid: u32,
        created: u64,
        chain_pid: u32,
        chain_created: u64,
        controller_pid: u32,
        controller_created: u64,
        source_dll_sha: String,
        token: String,
    },
    #[cfg(test)]
    Helper { token: String },
}
impl Witness {
    fn token(&self) -> &str {
        match self {
            Self::Native { token, .. } => token,
            #[cfg(test)]
            Self::Helper { token } => token,
        }
    }
    fn matches(&self, row: &Value, child: u32) -> bool {
        match self {
            Self::Native {
                pid,
                created,
                chain_pid,
                chain_created,
                controller_pid,
                controller_created,
                source_dll_sha,
                token,
            } => {
                row["schema"] == SCHEMA
                    && row["game_pid"].as_u64() == Some(u64::from(*pid))
                    && row["game_created_filetime"].as_u64() == Some(*created)
                    && row["chain_pid"].as_u64() == Some(u64::from(*chain_pid))
                    && row["chain_created_filetime"].as_u64() == Some(*chain_created)
                    && row["controller_pid"].as_u64() == Some(u64::from(*controller_pid))
                    && row["controller_created_filetime"].as_u64() == Some(*controller_created)
                    && row["source_dll_sha256"] == *source_dll_sha
                    && row["attach_token"] == *token
                    && row["observer_pid"].as_u64() == Some(u64::from(child))
            }
            #[cfg(test)]
            Self::Helper { token } => {
                row["kind"] == "owned_os_test_helper"
                    && row["token"] == *token
                    && row["child_pid"].as_u64() == Some(u64::from(child))
                    && row["live"] == false
            }
        }
    }
    fn restored(&self, row: &Value, _child: u32) -> bool {
        match self {
            Self::Native { pid, created, .. } => restored_stop(row, *pid, *created),
            #[cfg(test)]
            Self::Helper { .. } => {
                self.matches(row, _child)
                    && row["type"] == "helper_stop"
                    && row["helper_resource_restored"] == true
                    && row["native_debug_restored"] == false
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ownership {
    Ungranted,
    Granted,
    Attached,
    NeverAttached { granted: bool },
    Restored,
    TargetExited,
}
impl Ownership {
    fn permit(self) -> bool {
        match self {
            Self::Ungranted => false,
            Self::NeverAttached { granted } => granted,
            _ => true,
        }
    }
    fn safe(self) -> bool {
        matches!(
            self,
            Self::Ungranted
                | Self::Granted
                | Self::NeverAttached { .. }
                | Self::Restored
                | Self::TargetExited
        )
    }
}
#[derive(Debug)]
pub enum State {
    Active(Ownership),
    Finishing {
        owner: Ownership,
        started: Instant,
        deadline: Instant,
        stop_sent: bool,
    },
    CriticalRecovery {
        owner: Ownership,
        started: Instant,
    },
    Reaped {
        owner: Ownership,
        status: ExitStatus,
    },
}
impl State {
    fn owner(&self) -> Ownership {
        match self {
            Self::Active(o)
            | Self::Finishing { owner: o, .. }
            | Self::CriticalRecovery { owner: o, .. }
            | Self::Reaped { owner: o, .. } => *o,
        }
    }
    fn set_owner(&mut self, o: Ownership) {
        match self {
            Self::Active(x)
            | Self::Finishing { owner: x, .. }
            | Self::CriticalRecovery { owner: x, .. }
            | Self::Reaped { owner: x, .. } => *x = o,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Reaped,
    CriticalRecovery,
}
#[derive(Debug)]
pub struct Receipt {
    pub child_pid: u32,
    pub child_reaped: bool,
    pub exit_code: Option<i32>,
    pub restored: bool,
    pub never_attached: bool,
    pub permit_granted: bool,
    pub stop_writes: u32,
    pub target_close_requested: bool,
    pub child_force_terminated: bool,
    pub cleanup_confirmed: bool,
    pub error: Option<String>,
}
impl Receipt {
    pub fn json(&self) -> Value {
        json!({"child_pid":self.child_pid,"child_reaped":self.child_reaped,"child_exit_code":self.exit_code,"restored":self.restored,"never_attached":self.never_attached,"permit_granted":self.permit_granted,"stop_writes":self.stop_writes,"target_close_requested":self.target_close_requested,"child_force_terminated":self.child_force_terminated,"cleanup_confirmed":self.cleanup_confirmed,"error":self.error})
    }
}

pub struct ChildGuard<R: Recovery> {
    child: Child,
    lines: PipeLines<std::process::ChildStdout>,
    witness: Witness,
    recovery: R,
    pub state: State,
    started: Instant,
    ready_at: Option<Instant>,
    max_capture: Duration,
    permit_ready: bool,
    error: Option<String>,
    sink_failed: bool,
    status: Option<ExitStatus>,
    stop_writes: u32,
    target_close_requested: bool,
    close_ack_sent: bool,
    child_force_terminated: bool,
}
impl<R: Recovery> ChildGuard<R> {
    pub fn new(mut child: Child, witness: Witness, max_capture: Duration, recovery: R) -> Self {
        let out = child
            .stdout
            .take()
            .expect("supervised child requires piped stdout");
        Self {
            child,
            lines: PipeLines::new(out),
            witness,
            recovery,
            state: State::Active(Ownership::Ungranted),
            started: Instant::now(),
            ready_at: None,
            max_capture,
            permit_ready: false,
            error: None,
            sink_failed: false,
            status: None,
            stop_writes: 0,
            target_close_requested: false,
            close_ack_sent: false,
            child_force_terminated: false,
        }
    }
    fn finish(&mut self, error: Option<String>) {
        if self.error.is_none() {
            self.error = error;
        }
        if let State::Active(owner) = self.state {
            let started = Instant::now();
            let deadline = if self.ready_at.is_none() {
                (started + FINISH_BUDGET).min(self.started + FINISH_BUDGET)
            } else {
                started + FINISH_BUDGET
            };
            self.state = State::Finishing {
                owner,
                started,
                deadline,
                stop_sent: false,
            };
        }
    }
    fn send(&mut self, text: &str) -> io::Result<()> {
        let pipe = self
            .child
            .stdin
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "child stdin closed"))?;
        writeln!(pipe, "{text}")?;
        pipe.flush()
    }
    fn receive(&mut self, row: &Value) {
        if self.witness.matches(row, self.child.id()) {
            match row["type"].as_str() {
                Some("attach_ready") if self.state.owner() == Ownership::Ungranted => {
                    self.permit_ready = true;
                }
                Some("ATTACH_ENTER") => {
                    if matches!(self.state, State::Active(Ownership::Granted)) {
                        match self.recovery.authorize() {
                            Ok(()) => {
                                self.state.set_owner(Ownership::Attached);
                                let ack = format!("ENTER_ACK {}", self.witness.token());
                                if let Err(e) = self.send(&ack) {
                                    self.finish(Some(e.to_string()));
                                }
                            }
                            Err(e) => self.finish(Some(e)),
                        }
                    }
                }
                Some("NEVER_ATTACHED")
                    if matches!(
                        self.state.owner(),
                        Ownership::Ungranted | Ownership::Granted
                    ) && row["native_call_attempted"] == false
                        && row["terminal_kind"] == "never_attached" =>
                {
                    let granted = self.state.owner().permit();
                    self.state.set_owner(Ownership::NeverAttached { granted });
                }
                _ => {}
            }
            #[cfg(test)]
            if row["type"] == "helper_attached" {
                self.ready_at = Some(Instant::now());
            }
        }
        if self.state.owner() == Ownership::Attached && self.witness.restored(row, self.child.id())
        {
            self.state.set_owner(Ownership::Restored);
        }
        #[allow(irrefutable_let_patterns)]
        if let Witness::Native { pid, .. } = &self.witness {
            if self.state.owner().permit() && row["pid"].as_u64() == Some(u64::from(*pid)) {
                if row["ready"] == true {
                    self.ready_at = Some(Instant::now());
                }
                if row["critical_cleanup"] == true {
                    self.finish(Some("observer restoration unconfirmed".into()));
                    let started = match self.state {
                        State::Finishing { started, .. } => started,
                        _ => Instant::now(),
                    };
                    self.state = State::CriticalRecovery {
                        owner: self.state.owner(),
                        started,
                    };
                }
            }
        }
    }
    fn request_recovery(&mut self) {
        if self.state.owner().safe() {
            return;
        }
        self.target_close_requested = true;
        match self.recovery.request_close() {
            Ok(requested) => {
                if requested && !self.close_ack_sent {
                    self.close_ack_sent = true;
                    let _ = self.send("game_close_requested");
                }
            }
            Err(e) => {
                self.error.get_or_insert(e);
            }
        }
        if self.recovery.target_exited() {
            self.state.set_owner(Ownership::TargetExited);
        }
    }
    fn tick(&mut self, output: &mut impl Write, input: Option<&Receiver<Result<String, String>>>) {
        // Root cancellation wins over ATTACH_READY in the same iteration.
        if let Some(input) = input {
            for _ in 0..64 {
                match input.try_recv() {
                    Ok(Ok(cmd)) if cmd == "stop" => self.finish(None),
                    Ok(Ok(cmd))
                        if matches!(
                            cmd.as_str(),
                            "phase front" | "phase load" | "phase native-outer"
                        ) =>
                    {
                        if matches!(self.state, State::Active(Ownership::Attached)) {
                            if let Err(e) = self.send(&cmd) {
                                self.finish(Some(e.to_string()));
                            }
                        } else {
                            self.finish(Some("phase requested before attached".into()));
                        }
                    }
                    Ok(Ok(_)) => self.finish(Some("unknown or empty controller command".into())),
                    Ok(Err(e)) => self.finish(Some(format!("controller reader: {e}"))),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.finish(None);
                        break;
                    }
                }
            }
        }
        match self.lines.poll() {
            Ok(lines) => {
                for line in lines {
                    if let Ok(row) = serde_json::from_str::<Value>(&line) {
                        self.receive(&row);
                    }
                    if !self.sink_failed {
                        if let Err(e) = writeln!(output, "{line}").and_then(|_| output.flush()) {
                            self.sink_failed = true;
                            self.finish(Some(format!("forward output: {e}")));
                        }
                    }
                }
            }
            Err(e) => {
                self.lines.closed = true;
                self.finish(Some(format!("child output: {e}")));
            }
        }
        // State is committed BEFORE sending GO: ambiguous pipe delivery keeps
        // responsibility. An ungranted timeout can only stop/reap this child.
        if self.started.elapsed() >= Duration::from_secs(8) && self.ready_at.is_none() {
            self.finish(Some("ready deadline before GO".into()));
        }
        if self.permit_ready && matches!(self.state, State::Active(Ownership::Ungranted)) {
            match self.recovery.authorize() {
                Ok(()) => {
                    self.state = State::Active(Ownership::Granted);
                    let go = format!("GO {}", self.witness.token());
                    if let Err(e) = self.send(&go) {
                        self.finish(Some(e.to_string()));
                    }
                }
                Err(e) => self.finish(Some(e)),
            }
        }
        match self.child.try_wait() {
            Ok(Some(s)) => self.status = Some(s),
            Ok(None) => {}
            Err(e) => self.finish(Some(e.to_string())),
        }
        if let Some(status) = self.status {
            if self.lines.closed {
                if self.state.owner().safe() {
                    self.state = State::Reaped {
                        owner: self.state.owner(),
                        status,
                    };
                    return;
                }
                self.finish(Some("child exited without restoration".into()));
            }
        }
        if self.ready_at.is_none() && self.started.elapsed() >= Duration::from_secs(8) {
            self.finish(Some("ready deadline".into()));
        }
        if self
            .ready_at
            .is_some_and(|t| t.elapsed() >= self.max_capture)
        {
            self.finish(None);
        }
        let send_stop = matches!(
            self.state,
            State::Finishing {
                stop_sent: false,
                ..
            }
        );
        if send_stop {
            if let State::Finishing { stop_sent, .. } = &mut self.state {
                *stop_sent = true;
            }
            self.stop_writes += 1;
            if let Err(e) = self.send("stop") {
                self.error.get_or_insert(e.to_string());
                self.child.stdin.take();
            }
        }
        if let State::Finishing {
            started, deadline, ..
        } = self.state
        {
            let now = Instant::now();
            if now >= (started + RESTORE_GRACE).min(deadline - Duration::from_secs(8)) {
                self.request_recovery();
            }
            if now >= deadline - Duration::from_secs(2)
                && self.status.is_none()
                && self.state.owner().safe()
                && self.child.kill().is_ok()
            {
                self.child_force_terminated = true;
            }
            if now >= deadline {
                self.error
                    .get_or_insert("bounded API expired; recovery still owned".into());
                self.state = State::CriticalRecovery {
                    owner: self.state.owner(),
                    started,
                };
            }
        }
    }
    fn drive(
        &mut self,
        output: &mut impl Write,
        input: Option<&Receiver<Result<String, String>>>,
    ) -> Outcome {
        loop {
            match self.state {
                State::Reaped { .. } => return Outcome::Reaped,
                State::CriticalRecovery { .. } => return Outcome::CriticalRecovery,
                _ => {
                    self.tick(output, input);
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
        }
    }
    pub fn run(
        &mut self,
        output: &mut impl Write,
        input: &Receiver<Result<String, String>>,
    ) -> Outcome {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.drive(output, Some(input))
        })) {
            Ok(outcome) => outcome,
            Err(_) => {
                self.finish(Some("parent protocol panic".into()));
                self.drive(&mut io::sink(), None)
            }
        }
    }
    /// The 20-second API has returned CriticalRecovery, not completion. Keep
    /// both original handles on this stack, reuse native critical recovery, and
    /// wait on OS exit events. No daemon or new unbounded polling loop exists.
    pub fn resolve_critical(&mut self) {
        if !matches!(self.state, State::CriticalRecovery { .. }) {
            return;
        }
        if !self.state.owner().safe() {
            self.request_recovery();
            if !self.state.owner().safe() {
                self.recovery.wait_target_exit();
                self.state.set_owner(Ownership::TargetExited);
            }
        }
        if self.status.is_none() {
            if self.child.kill().is_ok() {
                self.child_force_terminated = true;
            }
            // Event wait retains the Child handle even if OS termination is delayed.
            self.status = self.child.wait().ok();
        }
        if let Some(status) = self.status {
            self.state = State::Reaped {
                owner: self.state.owner(),
                status,
            };
        } else {
            hold_recovery_handles("owned observer wait failed");
        }
    }
    pub fn receipt(&mut self) -> Option<Receipt> {
        let State::Reaped { owner, status } = self.state else {
            return None;
        };
        self.child.stdin.take();
        Some(Receipt {
            child_pid: self.child.id(),
            child_reaped: true,
            exit_code: status.code(),
            restored: owner == Ownership::Restored,
            never_attached: matches!(owner, Ownership::NeverAttached { .. }),
            permit_granted: owner.permit(),
            stop_writes: self.stop_writes,
            target_close_requested: self.target_close_requested,
            child_force_terminated: self.child_force_terminated,
            cleanup_confirmed: owner.safe(),
            error: self.error.clone(),
        })
    }
}
/// Last-resort wait failure is not permission to release recovery authority.
/// Parking is event-driven and keeps the owning stack/resources alive; it emits
/// no repeated notifications and cannot be confused with successful cleanup.
pub fn hold_recovery_handles(reason: &str) -> ! {
    let _ = writeln!(io::stderr(), "critical_recovery_retained: {reason}");
    loop {
        std::thread::park();
    }
}
impl<R: Recovery> Drop for ChildGuard<R> {
    fn drop(&mut self) {
        if matches!(self.state, State::Reaped { .. }) {
            return;
        }
        self.finish(Some("parent RAII finish".into()));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if self.drive(&mut io::sink(), None) == Outcome::CriticalRecovery {
                self.resolve_critical();
            }
        }));
        if result.is_err() {
            hold_recovery_handles("panic during retained critical recovery");
        }
    }
}

pub fn commands(buffer: &mut Vec<u8>) -> Result<Vec<String>, String> {
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_INPUT_HANDLE};
    let h = unsafe { GetStdHandle(STD_INPUT_HANDLE) };
    let mut available = 0;
    if unsafe {
        PeekNamedPipe(
            h,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut available,
            std::ptr::null_mut(),
        )
    } == 0
    {
        if std::io::Error::last_os_error().raw_os_error() == Some(109) {
            return Ok(vec!["eof".into()]);
        }
        return Err("controller stdin must be a live pipe".into());
    }
    if available > 0 {
        let mut bytes = vec![0; available.min(1024) as usize];
        let n = std::io::stdin()
            .read(&mut bytes)
            .map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(vec!["eof".into()]);
        }
        buffer.extend_from_slice(&bytes[..n]);
    }
    if buffer.len() > 4096 {
        return Err("controller command exceeds limit".into());
    }
    let mut out = Vec::new();
    while let Some(n) = buffer.iter().position(|b| *b == b'\n') {
        let bytes: Vec<_> = buffer.drain(..=n).collect();
        out.push(
            std::str::from_utf8(&bytes)
                .map_err(|e| e.to_string())?
                .trim()
                .into(),
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::windows::process::CommandExt,
        process::{Command, Stdio},
        sync::{
            atomic::{AtomicUsize, Ordering},
            mpsc, Arc,
        },
    };
    const WORKER: &str = "order_capture::protocol::tests::owned_helper_worker";
    #[test]
    #[ignore = "parent-owned subprocess entry"]
    fn owned_helper_worker() {
        let Ok(token) = std::env::var("TODO880_PROTOCOL_HELPER") else {
            return;
        };
        let path = std::path::PathBuf::from(std::env::var_os("TODO880_PROTOCOL_RESOURCE").unwrap());
        let mut row = json!({"kind":"owned_os_test_helper","token":token,"child_pid":std::process::id(),"live":false,"type":"attach_ready"});
        println!("{row}");
        io::stdout().flush().unwrap();
        let mut admission = Admission::new(row.clone(), token);
        if admission.await_go().is_err() {
            return;
        }
        if std::env::var("TODO880_RECHECK_FAIL").as_deref() == Ok("1") {
            return;
        }
        let Ok(resource) = admission.enter_and_call(|| {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .map_err(|e| e.to_string())
        }) else {
            return;
        };
        let mut line = String::new();
        row["type"] = json!("helper_attached");
        println!("{row}");
        io::stdout().flush().unwrap();
        line.clear();
        while !admission.stop_requested() && io::stdin().read_line(&mut line).unwrap() > 0 {
            if line.trim() == "stop" {
                break;
            }
            if line.trim() == "emit" {
                let mut progress = row.clone();
                progress["type"] = json!("helper_progress");
                println!("{progress}");
                io::stdout().flush().unwrap();
            }
            line.clear();
        }
        drop(resource);
        fs::remove_file(&path).unwrap();
        row["type"] = json!("helper_stop");
        row["helper_resource_restored"] = json!(true);
        row["native_debug_restored"] = json!(false);
        println!("{row}");
        io::stdout().flush().unwrap();
    }
    struct HelperRecovery {
        calls: Arc<AtomicUsize>,
        fail: bool,
    }
    impl Recovery for HelperRecovery {
        fn authorize(&mut self) -> Result<(), String> {
            if self.fail {
                Err("injected pre-GO identity refusal".into())
            } else {
                Ok(())
            }
        }
        fn request_close(&mut self) -> Result<bool, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err("no target exists in protocol helper test".into())
        }
        fn target_exited(&mut self) -> bool {
            false
        }
        fn wait_target_exit(&mut self) {
            panic!("must not request target recovery in helper test")
        }
    }
    fn child(case: &str) -> (Child, String, std::path::PathBuf) {
        let token = format!("{}-{case}", std::process::id());
        let path = std::env::temp_dir().join(format!("todo880-ede-{token}.resource"));
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", WORKER, "--ignored", "--nocapture"])
            .env("TODO880_PROTOCOL_HELPER", &token)
            .env("TODO880_PROTOCOL_RESOURCE", &path)
            .env(
                "TODO880_RECHECK_FAIL",
                if case == "recheck" { "1" } else { "0" },
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        (child, token, path)
    }
    fn check(case: &str, command: Result<String, String>, after_go: bool, deny: bool) {
        let (child, token, path) = child(case);
        let calls = Arc::new(AtomicUsize::new(0));
        let mut guard = ChildGuard::new(
            child,
            Witness::Helper { token },
            Duration::from_secs(2),
            HelperRecovery {
                calls: calls.clone(),
                fail: deny,
            },
        );
        let (tx, rx) = mpsc::channel();
        if after_go {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !matches!(guard.state, State::Active(Ownership::Attached)) {
                assert!(Instant::now() < deadline);
                guard.tick(&mut io::sink(), Some(&rx));
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        if !deny {
            tx.send(command).unwrap();
        }
        assert_eq!(guard.run(&mut io::sink(), &rx), Outcome::Reaped);
        let r = guard.receipt().unwrap();
        assert!(r.child_reaped && r.cleanup_confirmed);
        assert_eq!(r.permit_granted, after_go);
        assert_eq!(r.restored, after_go);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(!path.exists());
        println!(
            "TODO880_OS_RECEIPT={}",
            json!({"case":case,"receipt":r.json(),"civ6_live":false,"actual_os_child":true,"resource_absent":!path.exists(),"target_termination_calls":0})
        );
    }
    #[test]
    fn pre_go_unknown_input_reaps_without_game_recovery() {
        check("pregounknown", Ok("garbage".into()), false, false);
    }
    #[test]
    fn pre_go_empty_input_reaps_without_game_recovery() {
        check("pregoempty", Ok(String::new()), false, false);
    }
    #[test]
    fn pre_go_reader_error_reaps_without_game_recovery() {
        check("pregoreader", Err(os_reader_error()), false, false);
    }
    #[test]
    fn pre_go_identity_refusal_reaps_without_game_recovery() {
        check("pregorefuse", Ok("unused".into()), false, true);
    }
    #[test]
    fn late_real_ready_never_receives_go() {
        let (child, token, path) = child("late-ready");
        let calls = Arc::new(AtomicUsize::new(0));
        let mut guard = ChildGuard::new(
            child,
            Witness::Helper { token },
            Duration::from_secs(2),
            HelperRecovery {
                calls: calls.clone(),
                fail: false,
            },
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        while !guard.permit_ready {
            assert!(Instant::now() < deadline);
            for line in guard.lines.poll().unwrap() {
                if let Ok(row) = serde_json::from_str::<Value>(&line) {
                    guard.receive(&row);
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        guard.started = Instant::now() - Duration::from_secs(9);
        let (_tx, rx) = mpsc::channel();
        assert_eq!(guard.run(&mut io::sink(), &rx), Outcome::Reaped);
        let r = guard.receipt().unwrap();
        assert!(!r.permit_granted);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(!path.exists());
        println!(
            "TODO880_OS_RECEIPT={}",
            json!({"case":"late_real_ready_rejected","receipt":r.json(),"civ6_live":false,"target_termination_calls":0})
        );
    }
    #[test]
    fn post_go_unknown_input_restores_then_reaps() {
        check("postgounknown", Ok("garbage".into()), true, false);
    }
    #[test]
    fn post_go_reader_error_restores_then_reaps() {
        check("postgoreader", Err(os_reader_error()), true, false);
    }
    fn os_reader_error() -> String {
        use std::os::windows::io::FromRawHandle;
        use windows_sys::Win32::System::Pipes::CreatePipe;
        let (mut r, mut w) = (std::ptr::null_mut(), std::ptr::null_mut());
        assert_ne!(
            unsafe { CreatePipe(&mut r, &mut w, std::ptr::null(), 0) },
            0
        );
        let reader = unsafe { fs::File::from_raw_handle(r) };
        let mut writer = unsafe { fs::File::from_raw_handle(w) };
        writer.write_all(&[0xff, b'\n']).unwrap();
        drop(writer);
        PipeLines::new(reader).poll().unwrap_err().to_string()
    }
    #[test]
    fn post_go_closed_os_output_restores_without_target_close() {
        use std::os::windows::io::FromRawHandle;
        use windows_sys::Win32::{Foundation::CloseHandle, System::Pipes::CreatePipe};
        let (child, token, path) = child("postgobroken");
        let calls = Arc::new(AtomicUsize::new(0));
        let mut guard = ChildGuard::new(
            child,
            Witness::Helper { token },
            Duration::from_millis(100),
            HelperRecovery {
                calls: calls.clone(),
                fail: false,
            },
        );
        let (_tx, rx) = mpsc::channel();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !matches!(guard.state, State::Active(Ownership::Attached)) {
            assert!(Instant::now() < deadline);
            guard.tick(&mut io::sink(), Some(&rx));
            std::thread::sleep(Duration::from_millis(10));
        }
        let (mut r, mut w) = (std::ptr::null_mut(), std::ptr::null_mut());
        assert_ne!(
            unsafe { CreatePipe(&mut r, &mut w, std::ptr::null(), 0) },
            0
        );
        unsafe {
            CloseHandle(r);
        }
        let mut broken = unsafe { fs::File::from_raw_handle(w) };
        guard.send("emit").unwrap();
        assert_eq!(guard.run(&mut broken, &rx), Outcome::Reaped);
        let result = guard.receipt().unwrap();
        assert!(result.restored && result.child_reaped && result.error.is_some());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(!path.exists());
        println!(
            "TODO880_OS_RECEIPT={}",
            json!({"case":"postgo_brokenpipe","receipt":result.json(),"civ6_live":false,"actual_os_pipe":true})
        );
    }
    #[test]
    fn expired_api_retains_child_until_explicit_recovery() {
        let (child, token, path) = child("critical");
        let calls = Arc::new(AtomicUsize::new(0));
        let mut guard = ChildGuard::new(
            child,
            Witness::Helper { token },
            Duration::from_secs(2),
            HelperRecovery { calls, fail: false },
        );
        let (_tx, rx) = mpsc::channel();
        let started = Instant::now() - FINISH_BUDGET;
        guard.state = State::Finishing {
            owner: Ownership::Ungranted,
            started,
            deadline: Instant::now(),
            stop_sent: true,
        };
        assert_eq!(guard.run(&mut io::sink(), &rx), Outcome::CriticalRecovery);
        assert!(guard.receipt().is_none());
        assert!(matches!(guard.state, State::CriticalRecovery { .. }));
        guard.resolve_critical();
        let r = guard.receipt().unwrap();
        assert!(r.child_reaped && r.cleanup_confirmed);
        assert!(!path.exists());
        println!(
            "TODO880_OS_RECEIPT={}",
            json!({"case":"critical_retention","bounded_result":"CriticalRecovery","receipt":r.json(),"civ6_live":false})
        );
    }
    struct PartialRecovery {
        target: Child,
        calls: Arc<AtomicUsize>,
        waits: Arc<AtomicUsize>,
    }
    impl Recovery for PartialRecovery {
        fn authorize(&mut self) -> Result<(), String> {
            Ok(())
        }
        fn request_close(&mut self) -> Result<bool, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err("injected partial termination failure".into())
        }
        fn target_exited(&mut self) -> bool {
            self.target.try_wait().unwrap().is_some()
        }
        fn wait_target_exit(&mut self) {
            // Real target handle is still retained and alive after the bounded
            // API returned; recovery is an explicit second responsibility.
            assert!(self.target.try_wait().unwrap().is_none());
            self.waits.fetch_add(1, Ordering::SeqCst);
            self.target.kill().unwrap();
            self.target.wait().unwrap();
        }
    }
    impl Drop for PartialRecovery {
        fn drop(&mut self) {
            if self.target.try_wait().ok().flatten().is_none() {
                let _ = self.target.kill();
                let _ = self.target.wait();
            }
        }
    }
    #[test]
    fn granted_partial_failure_retains_both_real_handles_until_recovered() {
        let (target, _, target_path) = child("retained-target");
        let target_pid = target.id();
        let (observer, token, path) = child("retained-observer");
        let calls = Arc::new(AtomicUsize::new(0));
        let waits = Arc::new(AtomicUsize::new(0));
        let recovery = PartialRecovery {
            target,
            calls: calls.clone(),
            waits: waits.clone(),
        };
        let mut guard = ChildGuard::new(
            observer,
            Witness::Helper { token },
            Duration::from_secs(2),
            recovery,
        );
        let (_tx, rx) = mpsc::channel();
        let deadline = Instant::now() + Duration::from_secs(3);
        while !matches!(guard.state, State::Active(Ownership::Attached)) {
            assert!(Instant::now() < deadline);
            guard.tick(&mut io::sink(), Some(&rx));
            std::thread::sleep(Duration::from_millis(10));
        }
        let started = Instant::now() - FINISH_BUDGET;
        guard.state = State::Finishing {
            owner: Ownership::Attached,
            started,
            deadline: Instant::now(),
            stop_sent: true,
        };
        assert_eq!(guard.run(&mut io::sink(), &rx), Outcome::CriticalRecovery);
        assert!(guard.receipt().is_none());
        assert!(guard.child.try_wait().unwrap().is_none());
        assert!(!guard.recovery.target_exited());
        guard.resolve_critical();
        let receipt = guard.receipt().unwrap();
        assert!(receipt.child_reaped && receipt.cleanup_confirmed && !receipt.restored);
        assert_eq!(waits.load(Ordering::SeqCst), 1);
        assert!(calls.load(Ordering::SeqCst) > 0);
        if path.exists() {
            fs::remove_file(&path).unwrap();
        }
        assert!(!target_path.exists());
        println!(
            "TODO880_OS_RECEIPT={}",
            json!({"case":"granted_partial_failure_retained","target_helper_pid":target_pid,"receipt":receipt.json(),"two_live_handles_retained_at_api_timeout":true,"target_helper_explicitly_terminated_and_reaped":true,"civ6_live":false})
        );
    }
    #[test]
    fn classification_runs_before_failing_record() {
        let mut state = 0;
        let result = classify_then_record(
            &mut state,
            |s| {
                *s = 0x10002;
                Ok(*s)
            },
            |s| {
                assert_eq!(*s, 0x10002);
                Err(io::Error::other("disk full"))
            },
        );
        assert!(result.is_err());
        assert_eq!(state, 0x10002);
    }
    #[test]
    fn coalesced_go_stop_shared_os_reader_proves_never_attached() {
        let (child, token, path) = child("coalesced");
        let calls = Arc::new(AtomicUsize::new(0));
        let mut guard = ChildGuard::new(
            child,
            Witness::Helper {
                token: token.clone(),
            },
            Duration::from_secs(2),
            HelperRecovery {
                calls: calls.clone(),
                fail: false,
            },
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        while !guard.permit_ready {
            assert!(Instant::now() < deadline);
            for line in guard.lines.poll().unwrap() {
                if let Ok(row) = serde_json::from_str::<Value>(&line) {
                    guard.receive(&row);
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        guard.state = State::Active(Ownership::Granted);
        let pipe = guard.child.stdin.as_mut().unwrap();
        pipe.write_all(format!("GO {token}\nstop\n").as_bytes())
            .unwrap();
        pipe.flush().unwrap();
        let (_tx, rx) = mpsc::channel();
        assert_eq!(guard.run(&mut io::sink(), &rx), Outcome::Reaped);
        assert!(matches!(
            guard.state,
            State::Reaped {
                owner: Ownership::NeverAttached { granted: true },
                ..
            }
        ));
        let r = guard.receipt().unwrap();
        assert!(r.child_reaped && r.cleanup_confirmed && !r.restored);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(!path.exists());
        println!(
            "TODO880_OS_RECEIPT={}",
            json!({"case":"coalesced_go_stop","authenticated_never_attached":true,"receipt":r.json(),"same_production_pipe_reader":true,"single_write_batch":true,"civ6_live":false})
        );
    }
    #[test]
    fn post_go_recheck_failure_proves_never_attached() {
        let (child, token, path) = child("recheck");
        let calls = Arc::new(AtomicUsize::new(0));
        let mut guard = ChildGuard::new(
            child,
            Witness::Helper { token },
            Duration::from_secs(2),
            HelperRecovery {
                calls: calls.clone(),
                fail: false,
            },
        );
        let (_tx, rx) = mpsc::channel();
        assert_eq!(guard.run(&mut io::sink(), &rx), Outcome::Reaped);
        assert!(matches!(
            guard.state,
            State::Reaped {
                owner: Ownership::NeverAttached { granted: true },
                ..
            }
        ));
        let r = guard.receipt().unwrap();
        assert!(r.child_reaped && r.cleanup_confirmed);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(!path.exists());
        println!(
            "TODO880_OS_RECEIPT={}",
            json!({"case":"postgo_recheck_refusal","authenticated_never_attached":true,"receipt":r.json(),"target_termination_calls":0,"civ6_live":false})
        );
    }
    #[test]
    fn authenticated_never_attached_cannot_override_entered() {
        let (child, token, path) = child("terminal-auth");
        let calls = Arc::new(AtomicUsize::new(0));
        let mut guard = ChildGuard::new(
            child,
            Witness::Helper {
                token: token.clone(),
            },
            Duration::from_secs(2),
            HelperRecovery { calls, fail: false },
        );
        let row = json!({"kind":"owned_os_test_helper","token":token,"child_pid":guard.child.id(),"live":false,"type":"NEVER_ATTACHED","native_call_attempted":false,"terminal_kind":"never_attached"});
        guard.state = State::Active(Ownership::Attached);
        guard.receive(&row);
        assert_eq!(guard.state.owner(), Ownership::Attached);
        // Test assigned state did not authorize the actual helper; it remains at
        // its real pre-GO pipe. Restore bookkeeping before normal child reaping.
        guard.state = State::Active(Ownership::Ungranted);
        let (tx, rx) = mpsc::channel();
        tx.send(Ok("stop".into())).unwrap();
        assert_eq!(guard.run(&mut io::sink(), &rx), Outcome::Reaped);
        assert!(!path.exists());
    }
}
