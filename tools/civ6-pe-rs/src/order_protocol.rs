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

/// Call only after the event's ownership and continuation disposition have
/// been stored. An I/O error is evidence loss, never a new exception owner.
pub fn log_classified<T>(
    disposition: T,
    write: impl FnOnce() -> io::Result<()>,
) -> (T, io::Result<()>) {
    (disposition, write())
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

#[derive(Clone)]
pub enum Witness {
    Native {
        pid: u32,
        created: u64,
    },
    /// Actual OS helper resource test. Cannot produce a native/Civ6 receipt.
    #[cfg(test)]
    Helper {
        token: String,
    },
}
impl Witness {
    fn matches(&self, row: &Value, _child: u32) -> bool {
        match self {
            Self::Native { pid, created } => {
                row["schema"] == SCHEMA
                    && row["game_pid"].as_u64() == Some(u64::from(*pid))
                    && row["game_created_filetime"].as_u64() == Some(*created)
            }
            #[cfg(test)]
            Self::Helper { token } => {
                row["kind"] == "owned_os_test_helper"
                    && row["token"] == *token
                    && row["child_pid"].as_u64() == Some(u64::from(_child))
                    && row["live"] == false
            }
        }
    }
    fn restored(&self, row: &Value, _child: u32) -> bool {
        match self {
            Self::Native { pid, created } => restored_stop(row, *pid, *created),
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

#[derive(Debug)]
struct Finish {
    started: Instant,
    deadline: Instant,
    stop_sent: bool,
}
#[derive(Debug)]
pub struct Receipt {
    pub child_pid: u32,
    pub child_reaped: bool,
    pub exit_code: Option<i32>,
    pub restored: bool,
    pub attach_attempted: bool,
    pub stop_writes: u32,
    pub target_close_requested: bool,
    pub target_exit_observed: bool,
    pub child_force_terminated: bool,
    pub error: Option<String>,
    pub finish_elapsed_ms: u128,
    pub cleanup_confirmed: bool,
}
impl Receipt {
    pub fn json(&self) -> Value {
        json!({"child_pid":self.child_pid,"child_reaped":self.child_reaped,"child_exit_code":self.exit_code,"restored":self.restored,"attach_attempted":self.attach_attempted,"stop_writes":self.stop_writes,"target_close_requested":self.target_close_requested,"target_exit_observed":self.target_exit_observed,"child_force_terminated":self.child_force_terminated,"error":self.error,"finish_elapsed_ms":self.finish_elapsed_ms,"cleanup_confirmed":self.cleanup_confirmed})
    }
}

pub struct ChildGuard<F: FnMut() -> Result<(bool, bool), String>> {
    child: Child,
    lines: PipeLines<std::process::ChildStdout>,
    witness: Witness,
    recover: F,
    started: Instant,
    ready_at: Option<Instant>,
    max_capture: Duration,
    finish: Option<Finish>,
    error: Option<String>,
    sink_failed: bool,
    restored: bool,
    attach_attempted: bool,
    critical: bool,
    status: Option<ExitStatus>,
    stop_writes: u32,
    target_close_requested: bool,
    target_exit_observed: bool,
    close_ack_sent: bool,
    child_force_terminated: bool,
    done: bool,
}
impl<F: FnMut() -> Result<(bool, bool), String>> ChildGuard<F> {
    pub fn new(mut child: Child, witness: Witness, max_capture: Duration, recover: F) -> Self {
        // The caller constructed Stdio::piped. Keep the Child under RAII before
        // any output, receiver allocation, or other fallible protocol work.
        let out = child
            .stdout
            .take()
            .expect("supervised child requires piped stdout");
        Self {
            child,
            lines: PipeLines::new(out),
            witness,
            recover,
            started: Instant::now(),
            ready_at: None,
            max_capture,
            finish: None,
            error: None,
            sink_failed: false,
            restored: false,
            attach_attempted: false,
            critical: false,
            status: None,
            stop_writes: 0,
            target_close_requested: false,
            target_exit_observed: false,
            close_ack_sent: false,
            child_force_terminated: false,
            done: false,
        }
    }
    fn finish(&mut self, error: Option<String>) {
        if self.error.is_none() {
            self.error = error;
        }
        self.finish.get_or_insert_with(|| {
            let started = Instant::now();
            Finish {
                started,
                deadline: started + FINISH_BUDGET,
                stop_sent: false,
            }
        });
    }
    fn send(&mut self, text: &str) -> io::Result<()> {
        let pipe =
            self.child.stdin.as_mut().ok_or_else(|| {
                io::Error::new(io::ErrorKind::BrokenPipe, "observer stdin closed")
            })?;
        writeln!(pipe, "{text}")?;
        pipe.flush()
    }
    fn receive(&mut self, row: &Value) {
        if self.witness.matches(row, self.child.id()) {
            if row["type"] == "order_attach_start" || row["type"] == "helper_ready" {
                self.attach_attempted = true;
            }
            if row["type"] == "helper_ready" {
                self.ready_at = Some(Instant::now());
            }
            self.restored |= self.witness.restored(row, self.child.id());
        }
        #[allow(irrefutable_let_patterns)]
        if let Witness::Native { pid, .. } = &self.witness {
            if row["pid"].as_u64() == Some(u64::from(*pid)) {
                if row["ready"] == true {
                    self.ready_at = Some(Instant::now());
                }
                if row["critical_cleanup"] == true {
                    self.critical = true;
                    self.finish(Some("observer reported unconfirmed restoration".into()));
                }
            }
        }
    }
    fn tick(&mut self, output: &mut impl Write, input: Option<&Receiver<Result<String, String>>>) {
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
        if let Some(input) = input {
            for _ in 0..64 {
                match input.try_recv() {
                    Ok(Ok(cmd)) if cmd == "stop" => self.finish(None),
                    Ok(Ok(cmd)) if matches!(cmd.as_str(), "phase front" | "phase load") => {
                        if self.finish.is_none() {
                            if let Err(e) = self.send(&cmd) {
                                self.finish(Some(format!("phase input: {e}")));
                            }
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
        match self.child.try_wait() {
            Ok(Some(s)) => self.status = Some(s),
            Ok(None) => {}
            Err(e) => self.finish(Some(format!("child wait: {e}"))),
        }
        if self.status.is_some() && self.lines.closed {
            if self.restored || !self.attach_attempted || self.target_exit_observed {
                self.done = true;
                return;
            }
            self.critical = true;
            self.finish(Some("child exited without restoration".into()));
        }
        if self.ready_at.is_none() && self.started.elapsed() >= Duration::from_secs(8) {
            self.finish(Some("observer ready deadline".into()));
            if let Some(f) = self.finish.as_mut() {
                f.deadline = f.deadline.min(self.started + FINISH_BUDGET);
            }
        }
        if self
            .ready_at
            .is_some_and(|t| t.elapsed() >= self.max_capture)
        {
            self.finish(None);
        }
        let mut send_stop = false;
        if let Some(f) = self.finish.as_mut() {
            if !f.stop_sent {
                f.stop_sent = true;
                self.stop_writes += 1;
                send_stop = true;
            }
        }
        if send_stop {
            if let Err(e) = self.send("stop") {
                self.error.get_or_insert(format!("STOP pipe: {e}"));
                self.child.stdin.take();
            }
        }
        if let Some(f) = &self.finish {
            let finish_started = f.started;
            let deadline = f.deadline;
            let now = Instant::now();
            let restore_until =
                (finish_started + RESTORE_GRACE).min(deadline - Duration::from_secs(8));
            if !self.restored && (self.critical || now >= restore_until) {
                self.target_close_requested = true;
                match (self.recover)() {
                    Ok((requested, dead)) => {
                        self.target_exit_observed |= dead;
                        if requested && !self.close_ack_sent {
                            self.close_ack_sent = true;
                            let _ = self.send("game_close_requested");
                        }
                    }
                    Err(e) => {
                        self.error.get_or_insert(e);
                    }
                }
            }
            if now >= deadline - Duration::from_secs(2)
                && self.status.is_none()
                && (self.restored || self.target_exit_observed || !self.attach_attempted)
                && self.child.kill().is_ok()
            {
                self.child_force_terminated = true;
            }
            if now >= deadline {
                self.error
                    .get_or_insert("absolute finish deadline exceeded".into());
                self.done = true;
            }
        }
    }
    pub fn run(
        &mut self,
        output: &mut impl Write,
        input: &Receiver<Result<String, String>>,
    ) -> Receipt {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            while !self.done {
                self.tick(output, Some(input));
                std::thread::sleep(Duration::from_millis(10));
            }
        }));
        if result.is_err() {
            self.finish(Some("parent protocol panic".into()));
            self.finish_without_output();
        }
        self.receipt()
    }
    fn finish_without_output(&mut self) {
        self.finish(Some("parent guard cleanup".into()));
        let mut sink = io::sink();
        while !self.done {
            self.tick(&mut sink, None);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn receipt(&mut self) -> Receipt {
        if self.status.is_some() {
            self.status = self.child.wait().ok();
        }
        self.child.stdin.take();
        Receipt {
            child_pid: self.child.id(),
            child_reaped: self.status.is_some(),
            exit_code: self.status.and_then(|s| s.code()),
            restored: self.restored,
            attach_attempted: self.attach_attempted,
            stop_writes: self.stop_writes,
            target_close_requested: self.target_close_requested,
            target_exit_observed: self.target_exit_observed,
            child_force_terminated: self.child_force_terminated,
            error: self.error.clone(),
            finish_elapsed_ms: self
                .finish
                .as_ref()
                .map_or(0, |f| f.started.elapsed().as_millis()),
            cleanup_confirmed: self.status.is_some()
                && (self.restored || !self.attach_attempted || self.target_exit_observed),
        }
    }
}
impl<F: FnMut() -> Result<(bool, bool), String>> Drop for ChildGuard<F> {
    fn drop(&mut self) {
        if !self.done {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.finish_without_output()
            }));
        }
    }
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
    #[ignore = "subprocess entry; parent tests supply the owned helper token"]
    fn owned_helper_worker() {
        let Ok(token) = std::env::var("TODO880_PROTOCOL_HELPER") else {
            return;
        };
        let path = std::path::PathBuf::from(std::env::var_os("TODO880_PROTOCOL_RESOURCE").unwrap());
        let resource = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let base = json!({"kind":"owned_os_test_helper","token":token,"child_pid":std::process::id(),"live":false});
        let mut ready = base.clone();
        ready["type"] = json!("helper_ready");
        println!("{ready}");
        io::stdout().flush().unwrap();
        let mut line = String::new();
        while io::stdin().read_line(&mut line).unwrap() > 0 {
            if line.trim() == "stop" {
                break;
            }
            line.clear();
        }
        drop(resource);
        fs::remove_file(&path).unwrap();
        let mut stop = base;
        stop["type"] = json!("helper_stop");
        stop["helper_resource_restored"] = json!(true);
        stop["native_debug_restored"] = json!(false);
        println!("{stop}");
        io::stdout().flush().unwrap();
    }
    fn child(case: &str) -> (Child, String, std::path::PathBuf) {
        let token = format!("{}-{case}-{:?}", std::process::id(), Instant::now());
        let path =
            std::env::temp_dir().join(format!("todo880-{}-{case}.resource", std::process::id()));
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", WORKER, "--ignored", "--nocapture"])
            .env("TODO880_PROTOCOL_HELPER", &token)
            .env("TODO880_PROTOCOL_RESOURCE", &path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .creation_flags(0x08000000)
            .spawn()
            .unwrap();
        (child, token, path)
    }
    fn check(case: &str, mut output: impl Write, command: Option<Result<String, String>>) {
        let (child, token, path) = child(case);
        let calls = Arc::new(AtomicUsize::new(0));
        let count = calls.clone();
        let mut guard = ChildGuard::new(
            child,
            Witness::Helper { token },
            Duration::from_secs(10),
            move || {
                count.fetch_add(1, Ordering::SeqCst);
                Ok((false, false))
            },
        );
        let (tx, rx) = mpsc::channel();
        if let Some(c) = command {
            tx.send(c).unwrap();
        }
        let r = guard.run(&mut output, &rx);
        assert!(r.child_reaped && r.restored && r.cleanup_confirmed, "{r:?}");
        assert_eq!(r.stop_writes, 1);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(!r.child_force_terminated);
        assert!(!path.exists());
        assert!(r.finish_elapsed_ms < 20_000);
        println!(
            "TODO880_OS_RECEIPT={}",
            json!({"case":case,"kind":"owned_os_helper","receipt":r.json(),"resource_removed":!path.exists(),"native_debug_restore_test":false,"civ6_live":false,"game_launches":0})
        );
    }
    #[test]
    fn unknown_command_stops_restores_and_reaps_actual_child() {
        check("unknown", io::sink(), Some(Ok("bad".into())));
    }
    #[test]
    fn empty_command_stops_restores_and_reaps_actual_child() {
        check("empty", io::sink(), Some(Ok(String::new())));
    }
    #[test]
    fn reader_error_stops_restores_and_reaps_actual_child() {
        use std::os::windows::io::FromRawHandle;
        use windows_sys::Win32::System::Pipes::CreatePipe;
        let (mut read, mut write) = (std::ptr::null_mut(), std::ptr::null_mut());
        assert_ne!(
            unsafe { CreatePipe(&mut read, &mut write, std::ptr::null(), 0) },
            0
        );
        let input = unsafe { fs::File::from_raw_handle(read) };
        let mut output = unsafe { fs::File::from_raw_handle(write) };
        output.write_all(&[0xff, b'\n']).unwrap();
        drop(output);
        let error = PipeLines::new(input).poll().unwrap_err().to_string();
        check("reader", io::sink(), Some(Err(error)));
    }
    #[test]
    fn closed_os_output_pipe_stops_restores_and_reaps_actual_child() {
        use std::os::windows::io::FromRawHandle;
        use windows_sys::Win32::{
            Foundation::{CloseHandle, HANDLE},
            System::Pipes::CreatePipe,
        };
        let (mut read, mut write): (HANDLE, HANDLE) = (std::ptr::null_mut(), std::ptr::null_mut());
        assert_ne!(
            unsafe { CreatePipe(&mut read, &mut write, std::ptr::null(), 0) },
            0
        );
        unsafe {
            CloseHandle(read);
        }
        let output = unsafe { fs::File::from_raw_handle(write) };
        check("broken-output", output, None);
    }
    struct PanicSink;
    impl Write for PanicSink {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            panic!("intentional parent unwind")
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    #[test]
    fn panic_uses_same_finish_transaction_and_reaps_actual_child() {
        check("panic", PanicSink, None);
    }
}
