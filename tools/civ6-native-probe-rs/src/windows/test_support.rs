//! Bound each real debugger test with a separate process and an owned Job.
//! A wedged Win32 call or a panic cannot strand its fixture or the test runner.
use super::*;
use std::{
    os::windows::{io::AsRawHandle, process::CommandExt},
    process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio},
};
use windows_sys::Win32::System::{
    JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
        JobObjectBasicProcessIdList, JobObjectExtendedLimitInformation, QueryInformationJobObject,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        JOBOBJECT_BASIC_PROCESS_ID_LIST, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    },
    Pipes::PeekNamedPipe,
};

const CASE_ENV: &str = "PR86_PROBE_ISOLATED_CASE";
const IO_BUDGET: Duration = Duration::from_secs(8);
const REAP_BUDGET: Duration = Duration::from_secs(15);
const CASE_BUDGET: Duration = Duration::from_secs(50);

pub(super) trait ObserverJoin {
    type Output;
    fn bounded_join(self) -> thread::Result<Self::Output>;
}
impl<T> ObserverJoin for thread::JoinHandle<T> {
    type Output = T;
    fn bounded_join(self) -> thread::Result<T> {
        let deadline = Instant::now() + Duration::from_secs(40);
        while !self.is_finished() {
            assert!(
                Instant::now() < deadline,
                "debugger thread did not finish; the outer case Job will recover its tree"
            );
            thread::sleep(Duration::from_millis(10));
        }
        self.join()
    }
}
pub(super) fn join_observer<O: ObserverJoin>(observer: O) -> thread::Result<O::Output> {
    observer.bounded_join()
}
pub(super) struct FixtureObserver<T> {
    result: std::sync::mpsc::Receiver<thread::Result<T>>,
}
impl<T> FixtureObserver<T> {
    pub(super) fn join(self) -> thread::Result<T> {
        self.result
            .recv_timeout(Duration::from_secs(30))
            .unwrap_or_else(|error| {
                Err(Box::new(format!(
                    "fixture observer did not finish: {error}"
                )))
            })
    }
}
impl<T> ObserverJoin for FixtureObserver<T> {
    type Output = T;
    fn bounded_join(self) -> thread::Result<T> {
        self.join()
    }
}

fn wait_child(child: &mut Child, budget: Duration) -> Result<ExitStatus, String> {
    let deadline = Instant::now() + budget;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "child {} did not exit within {budget:?}",
                child.id()
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

struct OwnedCase {
    child: Child,
    job: Handle,
    members: BTreeMap<u32, Handle>,
}
impl OwnedCase {
    fn new(mut child: Child) -> Self {
        let job = match Handle::new(
            unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) },
            "test Job",
        ) {
            Ok(job) => job,
            Err(error) => {
                let _ = child.kill();
                let _ = wait_child(&mut child, REAP_BUDGET);
                panic!("{error}");
            }
        };
        let mut owner = Self {
            child,
            job,
            members: BTreeMap::new(),
        };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        assert_ne!(
            unsafe {
                SetInformationJobObject(
                    owner.job.0,
                    JobObjectExtendedLimitInformation,
                    (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            },
            0,
            "{}",
            win_error("test Job limits")
        );
        assert_ne!(
            unsafe { AssignProcessToJobObject(owner.job.0, owner.child.as_raw_handle().cast()) },
            0,
            "{}",
            win_error("assign test worker to Job")
        );
        // The worker cannot spawn a fixture until this Job owns its tree.
        owner
            .child
            .stdin
            .take()
            .unwrap()
            .write_all(b"start\n")
            .unwrap();
        owner
    }

    fn active_processes(&self) -> Result<u32, String> {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        if unsafe {
            QueryInformationJobObject(
                self.job.0,
                JobObjectBasicAccountingInformation,
                (&mut info as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                std::ptr::null_mut(),
            )
        } == 0
        {
            return Err(win_error("query owned test processes"));
        }
        Ok(info.ActiveProcesses)
    }

    fn wait(&mut self, budget: Duration) -> Result<ExitStatus, String> {
        let deadline = Instant::now() + budget;
        loop {
            // Pin descendants while the worker is running, before accounting
            // can remove them during asynchronous termination.
            self.capture_members()?;
            if let Some(status) = self.child.try_wait().map_err(|e| e.to_string())? {
                return Ok(status);
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "child {} did not exit within {budget:?}",
                    self.child.id()
                ));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn reap(&mut self) -> Result<(), String> {
        let captured = self.capture_members();
        // Also recover the handshake worker if Job assignment itself failed.
        // Killing the debugger first releases any outstanding debug event.
        if self.child.try_wait().map_err(|e| e.to_string())?.is_none() {
            if let Err(error) = self.child.kill() {
                if self.child.try_wait().map_err(|e| e.to_string())?.is_none() {
                    return Err(error.to_string());
                }
            }
        }
        if self.active_processes()? != 0 && unsafe { TerminateJobObject(self.job.0, 1) } == 0 {
            return Err(win_error("terminate owned test Job"));
        }
        let deadline = Instant::now() + REAP_BUDGET;
        loop {
            let worker_exited = self.child.try_wait().map_err(|e| e.to_string())?.is_some();
            let members_exited = self
                .members
                .values()
                .all(|member| matches!(alive(member), Ok(false)));
            if worker_exited && self.active_processes()? == 0 && members_exited {
                // Empty accounting does not mean process handles are signaled.
                // Require the worker's own exit even if assignment failed,
                // and termination of every member pinned before cleanup.
                return captured;
            }
            if Instant::now() >= deadline {
                return Err("owned test Job still has processes after termination".into());
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn capture_members(&mut self) -> Result<(), String> {
        // Two DWORDs precede the variable ULONG_PTR array. A usize buffer
        // provides the Windows x64 alignment required by both parts.
        let snapshot_deadline = Instant::now() + Duration::from_millis(50);
        loop {
            let mut buffer = [0usize; 257];
            if unsafe {
                QueryInformationJobObject(
                    self.job.0,
                    JobObjectBasicProcessIdList,
                    buffer.as_mut_ptr().cast(),
                    size_of::<[usize; 257]>() as u32,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                return Err(win_error("list owned test processes"));
            }
            let list = unsafe { &*(buffer.as_ptr().cast::<JOBOBJECT_BASIC_PROCESS_ID_LIST>()) };
            let count = list.NumberOfProcessIdsInList as usize;
            let assigned = list.NumberOfAssignedProcesses as usize;
            if count > 256 || assigned > 256 {
                return Err(format!("owned test Job exceeded its bounded process list: assigned={assigned}, returned={count}, capacity=256"));
            }
            for &pid in &buffer[1..1 + count] {
                let pid = u32::try_from(pid).map_err(|e| e.to_string())?;
                if let std::collections::btree_map::Entry::Vacant(entry) = self.members.entry(pid) {
                    match process(pid, false) {
                        Ok(handle) => {
                            entry.insert(handle);
                        }
                        Err(_) if std::io::Error::last_os_error().raw_os_error() == Some(87) => {
                            // The process object has already ceased to exist.
                        }
                        Err(error) => return Err(error),
                    }
                }
            }
            if assigned <= count {
                return Ok(());
            }
            // An incomplete PID list is not evidence that our capacity was
            // exceeded. Pin the returned members, then obtain a complete snapshot
            // before accepting it. Accounting and signaled-handle checks in reap
            // remain mandatory, including after this bounded retry fails.
            if Instant::now() >= snapshot_deadline {
                return Err(format!("owned test Job PID snapshot remained incomplete: assigned={assigned}, returned={count}, capacity=256"));
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn wait_empty(&self, budget: Duration) -> Result<u32, String> {
        let deadline = Instant::now() + budget;
        loop {
            let remaining = self.active_processes()?;
            if remaining == 0 || Instant::now() >= deadline {
                return Ok(remaining);
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for OwnedCase {
    fn drop(&mut self) {
        if let Err(error) = self.reap() {
            eprintln!("{error}");
        }
    }
}

/// Return true in the outer test, after checking the worker's actual exit.
pub(super) fn isolated_test(name: &str) -> bool {
    if enter_worker(name) {
        return false;
    }
    let outcome = run_worker(name, CASE_BUDGET, false);
    assert!(
        outcome.reaped.is_ok(),
        "{name}: {:?}; {}",
        outcome.reaped,
        outcome.log
    );
    assert!(
        outcome.status.as_ref().is_ok_and(|status| status.success()),
        "{name}: {:?}; {}",
        outcome.status,
        outcome.log
    );
    assert_eq!(
        outcome.remaining.unwrap(),
        0,
        "{name} left a live fixture: {}",
        outcome.log
    );
    true
}

pub(super) fn enter_worker(name: &str) -> bool {
    if std::env::var(CASE_ENV).as_deref() == Ok(&format!("windows::tests::{name}")) {
        let mut start = String::new();
        std::io::stdin().read_line(&mut start).unwrap();
        assert_eq!(start, "start\n");
        return true;
    }
    false
}

struct CaseOutcome {
    status: Result<ExitStatus, String>,
    remaining: Result<u32, String>,
    reaped: Result<(), String>,
    fixture_exited: Option<bool>,
    log: String,
}
fn run_worker(name: &str, budget: Duration, expect_fixture: bool) -> CaseOutcome {
    let full_name = format!("windows::tests::{name}");
    let path = std::env::temp_dir().join(format!("pr86-case-{}-{name}.log", std::process::id()));
    let output = File::create(&path).unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &full_name, "--nocapture"])
        .env(CASE_ENV, &full_name)
        .creation_flags(0x0800_0000)
        .stdin(Stdio::piped())
        .stdout(output.try_clone().unwrap())
        .stderr(output)
        .spawn()
        .unwrap();
    let mut owner = OwnedCase::new(child);
    let mut fixture = None;
    if expect_fixture {
        let deadline = Instant::now() + IO_BUDGET;
        loop {
            let log = fs::read_to_string(&path).unwrap_or_default();
            if let Some(pid) = log.lines().find_map(|line| {
                line.strip_prefix("WATCHDOG_FIXTURE:")
                    .and_then(|s| s.parse().ok())
            }) {
                fixture = Some(process(pid, false).unwrap());
                break;
            }
            assert!(
                Instant::now() < deadline,
                "fault worker did not publish its fixture: {log}"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    let status = owner.wait(budget);
    // Process signaling can precede the Job's accounting notification.
    // Require an empty Job before cleanup; survivors still fail the test.
    let remaining = owner.wait_empty(Duration::from_secs(1));
    let reaped = owner.reap();
    let log = fs::read_to_string(&path).unwrap_or_default();
    let fixture_exited = fixture.as_ref().map(|target| !alive(target).unwrap());
    fs::remove_file(path).unwrap();
    CaseOutcome {
        status,
        remaining,
        reaped,
        fixture_exited,
        log,
    }
}

pub(super) fn verify_watchdog_failure() {
    let mut peer = Fixture::start();
    let peer_process = process(peer.child.id(), false).unwrap();
    for (name, budget, timeout) in [
        (
            "watchdog_stalled_fixture_worker",
            Duration::from_millis(100),
            true,
        ),
        (
            "watchdog_panicking_fixture_worker",
            Duration::from_secs(10),
            false,
        ),
    ] {
        let result = run_worker(name, budget, true);
        assert!(result.reaped.is_ok(), "{:?}; {}", result.reaped, result.log);
        assert_eq!(result.fixture_exited, Some(true));
        if timeout {
            assert!(result.status.unwrap_err().contains("did not exit"));
            assert!(
                result.remaining.unwrap() > 0,
                "the watchdog did not actually interrupt a live tree"
            );
        } else {
            assert!(
                !result.status.unwrap().success(),
                "panic was turned into success"
            );
            assert!(result.log.contains("injected fixture-owner panic"));
        }
        assert!(
            alive(&peer_process).unwrap(),
            "watchdog terminated a peer case"
        );
        assert_eq!(peer.command("T", "PROBE_OUT:"), "1");
    }
}

pub(super) fn verify_unassigned_worker_reap() {
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "windows::tests::fixture_child_worker",
            "--nocapture",
        ])
        .env("PR86_PROBE_TEST_CHILD", "1")
        .creation_flags(0x0800_0000)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let job = Handle::new(
        unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) },
        "unassigned test Job",
    )
    .unwrap();
    let mut owner = OwnedCase {
        child,
        job,
        members: BTreeMap::new(),
    };
    let pid = owner.child.id();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let debugger = thread::spawn(move || {
        assert_ne!(unsafe { DebugActiveProcess(pid) }, 0);
        assert_ne!(unsafe { DebugSetProcessKillOnExit(0) }, 0);
        let mut event = DEBUG_EVENT::default();
        assert_ne!(unsafe { WaitForDebugEventEx(&mut event, 3000) }, 0);
        ready_tx.send(()).unwrap();
        // Native termination signaling cannot complete until this real pending
        // debugger event is released. The empty Job is deliberately unassigned.
        thread::sleep(Duration::from_millis(250));
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match event.dwDebugEventCode {
                CREATE_PROCESS_DEBUG_EVENT => {
                    let file = unsafe { event.u.CreateProcessInfo.hFile };
                    if !file.is_null() {
                        unsafe {
                            CloseHandle(file);
                        }
                    }
                }
                LOAD_DLL_DEBUG_EVENT => {
                    let file = unsafe { event.u.LoadDll.hFile };
                    if !file.is_null() {
                        unsafe {
                            CloseHandle(file);
                        }
                    }
                }
                _ => {}
            }
            let exited = event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT;
            assert_ne!(
                unsafe { ContinueDebugEvent(event.dwProcessId, event.dwThreadId, DBG_CONTINUE) },
                0
            );
            if exited {
                return;
            }
            assert!(Instant::now() < deadline);
            assert_ne!(unsafe { WaitForDebugEventEx(&mut event, 3000) }, 0);
        }
    });
    ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(owner.active_processes().unwrap(), 0);
    assert!(owner.members.is_empty());
    owner.reap().unwrap();
    assert!(
        owner.child.try_wait().unwrap().is_some(),
        "empty unassigned Job hid a live worker"
    );
    debugger.join().unwrap();
}

pub(super) struct Fixture {
    pub child: Child,
    input: BufWriter<ChildStdin>,
    output: ChildStdout,
    buffered: Vec<u8>,
    pub entry: u64,
    pub original_regs: String,
    observers: Vec<thread::JoinHandle<()>>,
    stops: Vec<std::sync::mpsc::Sender<()>>,
    close_ack: std::sync::Arc<AtomicBool>,
}
impl Fixture {
    pub fn start() -> Self {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "windows::tests::fixture_child_worker",
                "--nocapture",
            ])
            .env("PR86_PROBE_TEST_CHILD", "1")
            .creation_flags(0x0800_0000)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Inherit the isolated case's file: panic output cannot fill a pipe.
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let mut fixture = Self {
            input: BufWriter::new(child.stdin.take().unwrap()),
            output: child.stdout.take().unwrap(),
            child,
            buffered: Vec::new(),
            entry: 0,
            original_regs: String::new(),
            observers: Vec::new(),
            stops: Vec::new(),
            close_ack: std::sync::Arc::new(AtomicBool::new(false)),
        };
        let ready = fixture.response("PROBE_FIXTURE_READY:", IO_BUDGET).unwrap();
        let (address, regs) = ready.split_once(':').unwrap();
        fixture.entry = address.parse().unwrap();
        fixture.original_regs = regs.into();
        fixture
    }
    fn response(&mut self, prefix: &str, budget: Duration) -> Result<String, String> {
        let deadline = Instant::now() + budget;
        loop {
            while let Some(end) = self.buffered.iter().position(|b| *b == b'\n') {
                let line: Vec<u8> = self.buffered.drain(..=end).collect();
                let line = String::from_utf8(line).map_err(|e| e.to_string())?;
                if let Some(rest) = line.trim().strip_prefix(prefix) {
                    return Ok(rest.into());
                }
            }
            if Instant::now() >= deadline {
                return Err(format!(
                    "fixture {} did not answer {prefix} within {budget:?}",
                    self.child.id()
                ));
            }
            let mut available = 0;
            if unsafe {
                PeekNamedPipe(
                    self.output.as_raw_handle().cast(),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                return Err(format!(
                    "fixture pipe closed: {}",
                    std::io::Error::last_os_error()
                ));
            }
            if available == 0 {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            let mut bytes = [0u8; 4096];
            let count = (available as usize).min(bytes.len());
            let read = self
                .output
                .read(&mut bytes[..count])
                .map_err(|e| e.to_string())?;
            if read == 0 {
                return Err("fixture pipe reached EOF".into());
            }
            self.buffered.extend_from_slice(&bytes[..read]);
            if self.buffered.len() > 64 * 1024 {
                return Err("fixture response exceeded 64 KiB".into());
            }
        }
    }
    pub fn command_with_budget(
        &mut self,
        cmd: &str,
        prefix: &str,
        budget: Duration,
    ) -> Result<String, String> {
        self.send(cmd)?;
        self.response(prefix, budget)
    }
    pub(super) fn observe<T: Send + 'static>(
        &mut self,
        stop: &std::sync::mpsc::Sender<()>,
        work: impl FnOnce(std::sync::Arc<AtomicBool>) -> T + Send + 'static,
    ) -> FixtureObserver<T> {
        self.stops.push(stop.clone());
        let ack = std::sync::Arc::clone(&self.close_ack);
        let (tx, rx) = std::sync::mpsc::channel();
        self.observers.push(thread::spawn(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(ack)));
            let _ = tx.send(outcome);
        }));
        FixtureObserver { result: rx }
    }
    pub fn send(&mut self, cmd: &str) -> Result<(), String> {
        writeln!(self.input, "{cmd}").map_err(|e| e.to_string())?;
        self.input.flush().map_err(|e| e.to_string())
    }
    pub fn command(&mut self, cmd: &str, prefix: &str) -> String {
        self.command_with_budget(cmd, prefix, IO_BUDGET).unwrap()
    }
    pub fn terminate(&mut self) -> ExitStatus {
        self.child.kill().unwrap();
        self.wait_for_exit()
    }
    pub fn wait_for_exit(&mut self) -> ExitStatus {
        wait_child(&mut self.child, REAP_BUDGET).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for stop in &self.stops {
            let _ = stop.send(());
        }
        if !self.observers.is_empty() {
            // The parent version's panic recovery is preserved inside the
            // isolated Job: close this pinned child before releasing events.
            if self.child.kill().is_ok()
                || self.child.try_wait().is_ok_and(|status| status.is_some())
            {
                self.close_ack.store(true, Ordering::SeqCst);
            }
            let deadline = Instant::now() + Duration::from_secs(30);
            for observer in self.observers.drain(..) {
                while !observer.is_finished() && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(10));
                }
                assert!(
                    observer.is_finished(),
                    "owned observer not reaped; outer isolated Job will recover its tree"
                );
                let _ = observer.join();
            }
        }
        let _ = writeln!(self.input, "X");
        let _ = self.input.flush();
        if wait_child(&mut self.child, Duration::from_secs(1)).is_err() {
            let _ = self.child.kill();
            if let Err(error) = wait_child(&mut self.child, REAP_BUDGET) {
                // The outer Job still owns this child and reports any survivor.
                eprintln!("fixture cleanup failed: {error}");
            }
        }
    }
}
