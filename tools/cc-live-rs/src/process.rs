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
        }
    }
    pub fn hard_deadline(&self) -> Option<f64> {
        *self.hard_deadline.lock().unwrap()
    }
    pub fn work_deadline(&self) -> Option<f64> {
        self.hard_deadline()
            .map(|d| d - self.cleanup_reserve_seconds)
    }
    pub fn record_probe(&self, probe: &LeaseProbe) {
        let mut deadline = self.hard_deadline.lock().unwrap();
        match *probe {
            LeaseProbe::Clear => *deadline = None,
            LeaseProbe::Contended(d) => *deadline = Some(deadline.map_or(d, |old| old.min(d))),
            LeaseProbe::Unavailable => {}
        }
    }
}
pub const LEASE_YIELD: &str = "CIV6_LEASE_YIELD";
#[derive(Debug, PartialEq)]
pub enum LeaseProbe {
    Clear,
    Unavailable,
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
    let deadline = limit.work_deadline().unwrap_or(f64::MAX).min(now + 2.0);
    let probe = lease_probe(
        &mut crate::chain::WithinDeadline {
            runner: r,
            deadline,
        },
        limit,
    )?;
    limit.record_probe(&probe);
    Ok(probe)
}

/// Use the same live-waiter definition as acquisition, without holding the
/// lock while process/job queries run. A dead waiter never creates a deadline.
pub fn lease_probe(r: &mut impl Runner, limit: &LeaseLimit) -> Result<LeaseProbe, String> {
    let anchor = r.monotonic();
    let now = r.now();
    #[cfg(unix)]
    let guard = crate::fairness::UnixGuard::acquire(&limit.paths.here.join("civ6.lock"))?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    // Recovery pins a read-only handle. A monitor must be another reader,
    // rather than requesting write access and mistaking that pin for failure.
    let file = match options.open(limit.paths.here.join("civ6.lock")) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(LeaseProbe::Unavailable),
        #[cfg(windows)]
        Err(e) if matches!(e.raw_os_error(), Some(32 | 33)) => return Ok(LeaseProbe::Unavailable),
        Err(e) => return Err(e.to_string()),
    };
    let mut bytes = Vec::new();
    (&file)
        .take(32769)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 32768 {
        return Err("Civ6 lease record too large".into());
    }
    let owner: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    drop(file);
    #[cfg(unix)]
    drop(guard);
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

#[derive(Debug, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub image: String,
    pub created_filetime: Option<u64>,
}

/// All external commands (including gh/network) and waits are injectable. Tests never launch a game or call an API.
pub trait Runner {
    fn run(&mut self, call: Call) -> Result<Output, String>;
    /// The command fallback keeps non-Windows runners and fixtures injectable.
    /// Windows uses one pinned handle, without spawning tasklist per waiter.
    fn process_identity(
        &mut self,
        pid: u32,
        timeout: f64,
    ) -> Result<Option<ProcessIdentity>, String> {
        let out = self.run(
            Call::new(crate::args(&[
                "tasklist",
                "/FI",
                &format!("PID eq {pid}"),
                "/FO",
                "CSV",
                "/NH",
            ]))
            .timeout(timeout),
        )?;
        if out.code != 0 {
            return Err(format!(
                "Could not inspect the Civ6 lock holder: {}",
                crate::tail(&out.stderr, 300)
            ));
        }
        Ok(out.stdout.lines().find_map(|line| {
            let (image, fields) = line.strip_prefix('"')?.split_once("\",")?;
            let listed = fields
                .split(',')
                .next()?
                .trim()
                .trim_matches('"')
                .parse::<u32>()
                .ok()?;
            (listed == pid).then(|| ProcessIdentity {
                image: image.into(),
                created_filetime: None,
            })
        }))
    }
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

#[cfg(windows)]
fn process_open_failure(
    pid: u32,
    error: std::io::Error,
) -> Result<Option<ProcessIdentity>, String> {
    match error.raw_os_error() {
        Some(87) => Ok(None),
        // A stale PID can now belong to a protected process. Snapshot lookup
        // preserves the old image rejection without a tasklist subprocess.
        Some(5) => snapshot_process_identity(pid),
        _ => Err(format!("inspect Civ6 queue process {pid}: {error}")),
    }
}

#[cfg(windows)]
fn snapshot_process_identity(pid: u32) -> Result<Option<ProcessIdentity>, String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE},
        System::Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
    };
    struct Snapshot(windows_sys::Win32::Foundation::HANDLE);
    impl Drop for Snapshot {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if raw == INVALID_HANDLE_VALUE {
        return Err(format!(
            "inspect Civ6 queue process snapshot: {}",
            std::io::Error::last_os_error()
        ));
    }
    let snapshot = Snapshot(raw);
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut found = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    loop {
        if found == 0 {
            let error = std::io::Error::last_os_error();
            return if error.raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) {
                Ok(None)
            } else {
                Err(format!("inspect Civ6 queue process snapshot: {error}"))
            };
        }
        if entry.th32ProcessID == pid {
            let end = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            let image = String::from_utf16(&entry.szExeFile[..end]).map_err(|e| e.to_string())?;
            // Snapshot proves the image only. A matching chain still needs the
            // original creation-time check; denied access never grants a lease.
            return Ok(Some(ProcessIdentity {
                image,
                created_filetime: None,
            }));
        }
        found = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
}

#[cfg(windows)]
fn windows_process_identity(pid: u32) -> Result<Option<ProcessIdentity>, String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME, WAIT_TIMEOUT},
        System::Threading::{
            GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject,
            PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        },
    };
    struct Handle(windows_sys::Win32::Foundation::HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    let raw = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    };
    if raw.is_null() {
        return process_open_failure(pid, std::io::Error::last_os_error());
    }
    let handle = Handle(raw);
    let alive = || match unsafe { WaitForSingleObject(handle.0, 0) } {
        WAIT_TIMEOUT => Ok(true),
        0 => Ok(false),
        _ => Err(format!(
            "inspect Civ6 waiter process {pid} state: {}",
            std::io::Error::last_os_error()
        )),
    };
    if !alive()? {
        return Ok(None);
    }
    let mut path = vec![0u16; 32768];
    let mut length = path.len() as u32;
    if unsafe { QueryFullProcessImageNameW(handle.0, 0, path.as_mut_ptr(), &mut length) } == 0 {
        return Err(format!(
            "inspect Civ6 waiter process {pid} image: {}",
            std::io::Error::last_os_error()
        ));
    }
    let path = String::from_utf16(&path[..length as usize]).map_err(|e| e.to_string())?;
    let image = std::path::Path::new(&path)
        .file_name()
        .and_then(|p| p.to_str())
        .ok_or("waiter image has no file name")?
        .to_owned();
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    if unsafe { GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) } == 0
    {
        return Err(format!(
            "inspect Civ6 waiter process {pid} creation time: {}",
            std::io::Error::last_os_error()
        ));
    }
    if !alive()? {
        return Ok(None);
    }
    Ok(Some(ProcessIdentity {
        image,
        created_filetime: Some(
            (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime),
        ),
    }))
}

#[cfg(all(test, windows))]
mod native_identity_tests {
    use super::*;
    use std::os::windows::process::CommandExt;

    #[test]
    fn current_process_identity_uses_one_handle_and_no_commands() {
        let identity = RealRunner
            .process_identity(std::process::id(), 2.0)
            .unwrap()
            .unwrap();
        assert_eq!(
            identity.image,
            std::env::current_exe()
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
        );
        assert_eq!(
            identity.created_filetime,
            crate::chain::process_created_filetime(std::process::id()).unwrap()
        );
        assert!(identity.created_filetime.unwrap() > 0);
    }

    #[test]
    fn denied_process_query_uses_image_snapshot_without_granting_creation_identity() {
        let identity =
            process_open_failure(std::process::id(), std::io::Error::from_raw_os_error(5))
                .unwrap()
                .unwrap();
        assert_eq!(
            identity.image,
            std::env::current_exe()
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
        );
        assert_eq!(identity.created_filetime, None);
        assert_eq!(
            process_open_failure(u32::MAX, std::io::Error::from_raw_os_error(87)).unwrap(),
            None
        );
        assert!(
            process_open_failure(std::process::id(), std::io::Error::from_raw_os_error(6))
                .unwrap_err()
                .contains("queue process")
        );
    }

    #[test]
    fn exited_owned_process_and_invalid_pid_are_not_live_waiters() {
        let mut child = Command::new("cmd.exe")
            .args(["/c", "exit", "0"])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .unwrap();
        assert!(child.wait().unwrap().success());
        assert_eq!(windows_process_identity(child.id()).unwrap(), None);
        assert_eq!(windows_process_identity(u32::MAX).unwrap(), None);
    }
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
    #[cfg(windows)]
    fn process_identity(
        &mut self,
        pid: u32,
        _timeout: f64,
    ) -> Result<Option<ProcessIdentity>, String> {
        windows_process_identity(pid)
    }
    fn run(&mut self, call: Call) -> Result<Output, String> {
        interrupted()?;
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
    let mut unavailable_since: Option<Instant> = None;
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
                match observe_lease(&mut RealRunner, limit) {
                    Ok(LeaseProbe::Clear) => {
                        limit.record_probe(&LeaseProbe::Clear);
                        unavailable_since = None;
                    }
                    Ok(probe @ LeaseProbe::Contended(_)) => {
                        unavailable_since = None;
                        limit.record_probe(&probe);
                    }
                    Ok(LeaseProbe::Unavailable) => {
                        let since = *unavailable_since.get_or_insert_with(Instant::now);
                        if since.elapsed() >= Duration::from_secs(5) {
                            lease_error = Some("Civ6 lease state unavailable for 5s".to_string());
                        }
                    }
                    Err(error) => lease_error = Some(error),
                }
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
