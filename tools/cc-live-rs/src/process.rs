use std::{
    collections::BTreeMap,
    io::Read,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const CREATE_NO_WINDOW: u32 = 0x08000000;
static INTERRUPTED: AtomicBool = AtomicBool::new(false);
static CLEANING: AtomicBool = AtomicBool::new(false);
static CLEANUP_DONE: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub timeout: Option<f64>,
    pub merge_stderr: bool,
    pub creationflags: u32,
    pub kill_tree: bool,
    pub lease: Option<LeaseLimit>,
}
#[derive(Clone, Debug)]
pub struct LeaseLimit {
    pub paths: crate::Paths,
    pub token: String,
    pub cleanup_reserve_seconds: f64,
    hard_deadline: std::sync::Arc<std::sync::Mutex<Option<f64>>>,
    unknown_since: std::sync::Arc<std::sync::Mutex<Option<f64>>>,
}
impl PartialEq for LeaseLimit {
    fn eq(&self, other: &Self) -> bool {
        self.paths == other.paths
            && self.token == other.token
            && self.cleanup_reserve_seconds == other.cleanup_reserve_seconds
    }
}
impl LeaseLimit {
    pub fn new(paths: &crate::Paths, token: &str, cleanup_reserve_seconds: f64) -> Self {
        Self {
            paths: paths.clone(),
            token: token.into(),
            cleanup_reserve_seconds,
            hard_deadline: std::sync::Arc::new(std::sync::Mutex::new(None)),
            unknown_since: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }
    pub fn hard_deadline(&self) -> Option<f64> {
        *self.hard_deadline.lock().unwrap()
    }
    pub fn work_deadline(&self) -> Option<f64> {
        self.hard_deadline()
            .map(|d| d - self.cleanup_reserve_seconds)
    }
    pub fn observation_deadline(&self) -> Option<f64> {
        self.unknown_since.lock().unwrap().map(|since| since + 5.0)
    }
    pub fn record_probe(&self, probe: &LeaseProbe) {
        let mut deadline = self.hard_deadline.lock().unwrap();
        match *probe {
            LeaseProbe::Clear => *deadline = None,
            LeaseProbe::Contended(d) => *deadline = Some(deadline.map_or(d, |old| old.min(d))),
            LeaseProbe::Unavailable | LeaseProbe::TimedOut => {}
        }
        if matches!(probe, LeaseProbe::Clear | LeaseProbe::Contended(_)) {
            *self.unknown_since.lock().unwrap() = None;
        }
    }
}
pub const LEASE_YIELD: &str = "CIV6_LEASE_YIELD";
#[derive(Debug, PartialEq)]
pub enum LeaseProbe {
    Clear,
    Unavailable,
    TimedOut,
    Contended(f64),
}

pub fn observe_lease(r: &mut impl Runner, limit: &LeaseLimit) -> Result<LeaseProbe, String> {
    let now = r.monotonic();
    if limit
        .work_deadline()
        .is_some_and(|deadline| now >= deadline)
    {
        return Ok(LeaseProbe::Contended(limit.hard_deadline().unwrap()));
    }
    if limit.observation_deadline().is_some_and(|d| now >= d) {
        return Err(LEASE_OBSERVATION_EXHAUSTED.into());
    }
    let deadline = limit
        .work_deadline()
        .unwrap_or(f64::MAX)
        .min(limit.observation_deadline().unwrap_or(f64::MAX))
        .min(now + 2.0);
    let observation = lease_probe(
        &mut crate::chain::WithinDeadline {
            runner: r,
            deadline,
        },
        limit,
    );
    // These exact errors originate from the bounded command runner, rather
    // than the authoritative lock/token read. Do not interpret them as Clear.
    let probe = match observation {
        Err(error)
            if error == "command timed out" || error == "Civ6 scenario deadline exhausted" =>
        {
            LeaseProbe::TimedOut
        }
        other => other?,
    };
    if matches!(probe, LeaseProbe::Unavailable | LeaseProbe::TimedOut) {
        let first = {
            let mut since = limit.unknown_since.lock().unwrap();
            let first = since.is_none();
            since.get_or_insert(now);
            first
        };
        if first && probe == LeaseProbe::TimedOut {
            r.error("Civ6 lease observation timed out; retrying without changing ownership or the competition deadline")?;
        }
        if limit
            .observation_deadline()
            .is_some_and(|d| r.monotonic() >= d)
        {
            return Err(LEASE_OBSERVATION_EXHAUSTED.into());
        }
    }
    limit.record_probe(&probe);
    Ok(probe)
}

const LEASE_OBSERVATION_EXHAUSTED: &str =
    "Civ6 lease observation unavailable for 5s (including probe timeouts)";
/// Startup and the running child share both absolute deadlines via LeaseLimit.
pub fn monitor_lease(r: &mut impl Runner, limit: &LeaseLimit) -> Result<(), String> {
    observe_lease(r, limit)?;
    if limit.work_deadline().is_some_and(|d| r.monotonic() >= d) {
        return Err(format!("{LEASE_YIELD}: live waiter requires the shared slot; unfinished scenario, not a product failure"));
    }
    Ok(())
}

/// Use the same live-waiter definition as acquisition, without holding the
/// lock while process/job queries run. A dead waiter never creates a deadline.
pub fn lease_probe(r: &mut impl Runner, limit: &LeaseLimit) -> Result<LeaseProbe, String> {
    let anchor = r.monotonic();
    let now = r.now();
    let path = limit.paths.here.join("civ6.lock");
    let Some(snapshot) = crate::fairness::Snapshot::open(&path)
        .map_err(|e| format!("monitor lease initial snapshot: {e}"))?
    else {
        return Ok(LeaseProbe::Unavailable);
    };
    let owner = snapshot.info()?;
    if owner["lock_token"].as_str() != Some(limit.token.as_str()) {
        return Err("Civ6 lease ownership changed while child was running".into());
    }
    let competitor = crate::fairness::competitor_since(
        r,
        &limit.paths,
        &owner,
        &crate::fairness::todo_path(&limit.paths),
        now,
    )?;
    // A probe can outlive its original owner. Validate again before publishing
    // Clear or a deadline; a stale successful probe cannot revive an old lease.
    let Some(current) = crate::fairness::Snapshot::open(&path)
        .map_err(|e| format!("monitor lease final snapshot: {e}"))?
    else {
        return Ok(LeaseProbe::Unavailable);
    };
    if !snapshot.same_file(&current)?
        || !crate::fairness::same_acquisition(&owner, &current.info()?)
    {
        return Err("Civ6 lease ownership changed while child was running".into());
    }
    let Some(since) = competitor else {
        return Ok(LeaseProbe::Clear);
    };
    let acquired = owner["acquired_at"].as_f64().unwrap_or(now).min(now);
    let since = since.max(acquired).min(now);
    Ok(LeaseProbe::Contended(
        anchor + crate::fairness::LEASE_SECONDS - (now - since),
    ))
}
impl Call {
    pub fn new(args: Vec<String>) -> Self {
        Self {
            args,
            env: BTreeMap::new(),
            timeout: None,
            merge_stderr: false,
            creationflags: CREATE_NO_WINDOW,
            kill_tree: false,
            lease: None,
        }
    }
    pub fn timeout(mut self, seconds: f64) -> Self {
        self.timeout = Some(seconds);
        self
    }
    pub fn env(mut self, key: &str, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }
    pub fn merged(mut self) -> Self {
        self.merge_stderr = true;
        self
    }
    /// Contain a build and all descendants until it completes or is cancelled.
    pub fn managed(mut self) -> Self {
        self.kill_tree = true;
        self
    }
    pub fn lease(
        mut self,
        paths: &crate::Paths,
        token: &str,
        cleanup_reserve_seconds: f64,
    ) -> Self {
        self.lease = Some(LeaseLimit::new(paths, token, cleanup_reserve_seconds));
        self
    }
}
#[derive(Clone, Debug, Default)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// All external commands (including gh/network) and waits are injectable. Tests never launch a game or call an API.
pub trait Runner {
    fn run(&mut self, call: Call) -> Result<Output, String>;
    fn now(&mut self) -> f64;
    fn monotonic(&mut self) -> f64 {
        self.now()
    }
    fn sleep(&mut self, seconds: f64) -> Result<(), String>;
    fn print(&mut self, text: &str) -> Result<(), String> {
        use std::io::Write;
        let mut stdout = std::io::stdout().lock();
        writeln!(stdout, "{text}")
            .and_then(|_| stdout.flush())
            .map_err(|e| e.to_string())
    }
    fn error(&mut self, text: &str) -> Result<(), String> {
        use std::io::Write;
        writeln!(std::io::stderr().lock(), "{text}").map_err(|e| e.to_string())
    }
}
pub struct RealRunner;

// Lease monitoring has a two-second budget. Spawning tasklist for every queued
// PID can exhaust it on a busy machine and terminate a healthy driver.
#[cfg(any(windows, test))]
fn tasklist_pid(args: &[String]) -> Option<u32> {
    if args.len() != 6
        || !args[0].eq_ignore_ascii_case("tasklist")
        || !args[1].eq_ignore_ascii_case("/FI")
        || !args[3].eq_ignore_ascii_case("/FO")
        || !args[4].eq_ignore_ascii_case("CSV")
        || !args[5].eq_ignore_ascii_case("/NH")
    {
        return None;
    }
    args[2]
        .strip_prefix("PID eq ")?
        .parse::<u32>()
        .ok()
        .filter(|pid| *pid > 0)
}

#[cfg(windows)]
fn native_pid_tasklist(pid: u32) -> Result<Output, String> {
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    // SYNCHRONIZE permits a zero-time read of this exact object's exit state.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | 0x00100000, 0, pid) };
    if handle.is_null() {
        let error = std::io::Error::last_os_error();
        return if error.raw_os_error() == Some(87) {
            Ok(Output::default())
        } else {
            Err(format!("inspect lease process {pid}: {error}"))
        };
    }
    let result = native_pid_tasklist_handle(handle, pid);
    unsafe { CloseHandle(handle) };
    result
}

#[cfg(windows)]
fn native_pid_tasklist_handle(
    handle: windows_sys::Win32::Foundation::HANDLE,
    pid: u32,
) -> Result<Output, String> {
    use windows_sys::Win32::{
        Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{QueryFullProcessImageNameW, WaitForSingleObject},
    };
    let exited = || match unsafe { WaitForSingleObject(handle, 0) } {
        WAIT_OBJECT_0 => Ok(true),
        WAIT_TIMEOUT => Ok(false),
        _ => Err(format!(
            "inspect lease process {pid} exit state: {}",
            std::io::Error::last_os_error()
        )),
    };
    if exited()? {
        return Ok(Output::default());
    }
    let mut buffer = vec![0u16; 32768];
    let mut length = buffer.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut length) };
    let error = (ok == 0).then(std::io::Error::last_os_error);
    if let Some(error) = error {
        // The handle pins the object, so PID reuse cannot turn this check into
        // a query of a different process between image lookup and exit read.
        if exited()? {
            return Ok(Output::default());
        }
        return Err(format!("inspect lease process {pid} image: {error}"));
    }
    let path = String::from_utf16(&buffer[..length as usize]).map_err(|e| e.to_string())?;
    let image = path
        .rsplit(['\\', '/'])
        .next()
        .ok_or("empty process image")?;
    Ok(Output {
        code: 0,
        stdout: format!("\"{image}\",\"{pid}\"\n"),
        stderr: String::new(),
    })
}

pub fn install_interrupt_handler() {
    #[cfg(windows)]
    unsafe {
        unsafe extern "system" fn handler(event: u32) -> i32 {
            if matches!(event, 0 | 1 | 2 | 5 | 6) {
                if !CLEANING.load(Ordering::SeqCst) {
                    INTERRUPTED.store(true, Ordering::SeqCst);
                }
                if matches!(event, 2 | 5 | 6) {
                    let deadline = Instant::now() + Duration::from_secs(4);
                    while !CLEANUP_DONE.load(Ordering::SeqCst) && Instant::now() < deadline {
                        thread::sleep(Duration::from_millis(10));
                    }
                }
                1
            } else {
                0
            }
        }
        windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(handler), 1);
    }
}
/// Cleanup must still run after Ctrl+C; no new scenario is started once interruption is observed.
pub fn clear_interrupt_for_cleanup() {
    CLEANING.store(true, Ordering::SeqCst);
    INTERRUPTED.store(false, Ordering::SeqCst);
}
/// A reliability round has its own cleanup lifecycle. Console-close handling
/// must wait for this round, rather than seeing the previous round as done.
pub fn begin_cleanup_cycle() {
    CLEANUP_DONE.store(false, Ordering::SeqCst);
}
pub fn finish_cleanup() {
    CLEANUP_DONE.store(true, Ordering::SeqCst);
    CLEANING.store(false, Ordering::SeqCst);
}
fn interrupted() -> Result<(), String> {
    if INTERRUPTED.load(Ordering::SeqCst) {
        Err("interrupted".into())
    } else {
        Ok(())
    }
}

impl Runner for RealRunner {
    fn run(&mut self, call: Call) -> Result<Output, String> {
        interrupted()?;
        #[cfg(windows)]
        if let Some(pid) = tasklist_pid(&call.args) {
            return native_pid_tasklist(pid);
        }
        let mut cmd = Command::new(call.args.first().ok_or("empty command")?);
        cmd.args(&call.args[1..])
            .envs(&call.env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(call.creationflags);
        }
        // Using the same pipe preserves scenario.py's stdout/stderr line order.
        if call.merge_stderr {
            let (mut source, sink) = std::io::pipe().map_err(|e| e.to_string())?;
            cmd.stdout(Stdio::from(sink.try_clone().map_err(|e| e.to_string())?))
                .stderr(Stdio::from(sink));
            let (mut child, mut tree) =
                crate::process_tree::Tree::spawn(&mut cmd, call.creationflags, call.kill_tree)?;
            drop(cmd);
            let reader = thread::spawn(move || {
                let mut b = Vec::new();
                source.read_to_end(&mut b).map(|_| b)
            });
            let status = wait(&mut child, &mut tree, call.timeout, call.lease.as_ref());
            if let (false, Err(error)) = (call.kill_tree, &status) {
                return Err(error.clone());
            }
            #[cfg(windows)]
            drop(tree);
            let bytes = reader
                .join()
                .map_err(|_| "stdout reader failed")?
                .map_err(|e| e.to_string())?;
            let stdout = decode(&bytes);
            return Ok(Output {
                code: status.map_err(|e| format!("{e}\n{}", crate::tail(&stdout, 4000)))?,
                stdout,
                stderr: String::new(),
            });
        }
        cmd.stderr(Stdio::piped());
        let (mut child, mut tree) =
            crate::process_tree::Tree::spawn(&mut cmd, call.creationflags, call.kill_tree)?;
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let out = thread::spawn(move || {
            let mut b = Vec::new();
            stdout.read_to_end(&mut b).map(|_| b)
        });
        let err = thread::spawn(move || {
            let mut b = Vec::new();
            stderr.read_to_end(&mut b).map(|_| b)
        });
        let status = wait(&mut child, &mut tree, call.timeout, call.lease.as_ref());
        if let (false, Err(error)) = (call.kill_tree, &status) {
            return Err(error.clone());
        }
        #[cfg(windows)]
        drop(tree);
        let stdout = decode(
            &out.join()
                .map_err(|_| "stdout reader failed")?
                .map_err(|e| e.to_string())?,
        );
        let stderr = decode(
            &err.join()
                .map_err(|_| "stderr reader failed")?
                .map_err(|e| e.to_string())?,
        );
        Ok(Output {
            code: status.map_err(|e| {
                format!(
                    "{e}\n{}{}",
                    crate::tail(&stdout, 2000),
                    crate::tail(&stderr, 2000)
                )
            })?,
            stdout,
            stderr,
        })
    }
    fn now(&mut self) -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
    }
    fn monotonic(&mut self) -> f64 {
        static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        START.get_or_init(Instant::now).elapsed().as_secs_f64()
    }
    fn sleep(&mut self, seconds: f64) -> Result<(), String> {
        let until = Instant::now() + Duration::from_secs_f64(seconds);
        while Instant::now() < until {
            interrupted()?;
            thread::sleep(
                until
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_millis(100)),
            );
        }
        Ok(())
    }
}
fn decode(b: &[u8]) -> String {
    String::from_utf8_lossy(b)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}
fn wait(
    child: &mut std::process::Child,
    tree: &mut crate::process_tree::Tree,
    timeout: Option<f64>,
    lease: Option<&LeaseLimit>,
) -> Result<i32, String> {
    let start = Instant::now();
    let mut next_probe = start;
    loop {
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            tree.stop(child)?;
            return Ok(s.code().unwrap_or(1));
        }
        let mut lease_error = None;
        if let Some(limit) = lease {
            if limit
                .work_deadline()
                .is_some_and(|d| RealRunner.monotonic() >= d)
            {
                lease_error=Some(format!("{LEASE_YIELD}: live waiter requires the shared slot; unfinished scenario, not a product failure"));
            } else if Instant::now() >= next_probe {
                next_probe = Instant::now() + Duration::from_secs(2);
                if let Err(error) = monitor_lease(&mut RealRunner, limit) {
                    lease_error = Some(error);
                }
            }
            if limit
                .observation_deadline()
                .is_some_and(|d| RealRunner.monotonic() >= d)
                && lease_error.is_none()
            {
                lease_error = Some(LEASE_OBSERVATION_EXHAUSTED.into());
            }
            if limit
                .work_deadline()
                .is_some_and(|d| RealRunner.monotonic() >= d)
            {
                lease_error=Some(format!("{LEASE_YIELD}: live waiter requires the shared slot; unfinished scenario, not a product failure"));
            }
        }
        if lease_error.is_some()
            || interrupted().is_err()
            || timeout.is_some_and(|t| start.elapsed().as_secs_f64() >= t)
        {
            tree.stop(child)?;
            let _ = child.kill();
            let _ = child.wait();
            if let Some(error) = lease_error {
                return Err(error);
            }
            return Err(if INTERRUPTED.load(Ordering::SeqCst) {
                "interrupted"
            } else {
                "command timed out"
            }
            .into());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod pid_query_tests {
    use super::*;

    #[test]
    fn native_query_only_accepts_the_existing_exact_pid_protocol() {
        let args: Vec<String> = ["tasklist", "/FI", "PID eq 42", "/FO", "CSV", "/NH"]
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(tasklist_pid(&args), Some(42));
        for filter in [
            "PID eq 0",
            "PID eq -1",
            "PID eq 4294967296",
            "PID eq 42 extra",
            "IMAGENAME eq live_chain.exe",
        ] {
            let mut other = args.clone();
            other[2] = filter.into();
            assert_eq!(tasklist_pid(&other), None);
        }
        let mut extra = args;
        extra.push("/V".into());
        assert_eq!(tasklist_pid(&extra), None);
    }

    #[cfg(windows)]
    #[test]
    fn native_query_reports_the_real_current_image_and_absent_pid() {
        let pid = std::process::id();
        let expected = std::env::current_exe().unwrap();
        let image = expected.file_name().unwrap().to_str().unwrap();
        assert_eq!(
            native_pid_tasklist(pid).unwrap().stdout,
            format!("\"{image}\",\"{pid}\"\n")
        );
        assert!(native_pid_tasklist(u32::MAX).unwrap().stdout.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn native_query_observes_exit_with_the_original_handle_still_pinned() {
        use std::{
            os::windows::{io::AsRawHandle, process::CommandExt},
            process::{Command, Stdio},
        };
        if std::env::var_os("CC_LIVE_NATIVE_PID_TEST_CHILD").is_some() {
            return;
        }
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "process::pid_query_tests::native_query_observes_exit_with_the_original_handle_still_pinned"])
            .env("CC_LIVE_NATIVE_PID_TEST_CHILD", "1")
            .creation_flags(CREATE_NO_WINDOW)
            .stdout(Stdio::null()).stderr(Stdio::null())
            .spawn().unwrap();
        let pid = child.id();
        let handle = child.as_raw_handle();
        assert!(child.wait().unwrap().success());
        assert!(native_pid_tasklist_handle(handle, pid)
            .unwrap()
            .stdout
            .is_empty());
        assert!(native_pid_tasklist_handle(std::ptr::null_mut(), pid).is_err());
    }
}
