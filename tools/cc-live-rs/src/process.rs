use std::{
    collections::BTreeMap,
    io::Read,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
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
    pub deadline: Option<f64>,
    pub merge_stderr: bool,
    pub creationflags: u32,
    pub kill_tree: bool,
    pub lease: Option<LeaseLimit>,
    pub native_recovery: Option<NativeRecovery>,
}
pub const NATIVE_RECOVERY_REQUIRED: &str = "CIV6_NATIVE_RECOVERY_REQUIRED_V1";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeTarget {
    pub pid: u32,
    pub created: u64,
}
type RecoveryNotice = Mutex<Option<Result<NativeTarget, String>>>;
#[derive(Clone, Debug)]
pub struct NativeRecovery {
    pub paths: crate::Paths,
    pub token: String,
    pub artifacts: std::path::PathBuf,
    pub deadline: f64,
    pub nonce: String,
    pub(crate) renderer: Option<crate::chain::RendererRun>,
    pub(crate) notice: Arc<RecoveryNotice>,
    pub(crate) outcome: Arc<Mutex<Option<NativeRecoveryOutcome>>>,
}
#[derive(Clone, Debug)]
pub(crate) struct NativeRecoveryOutcome {
    pub evidence: serde_json::Value,
    pub result: Result<(), String>,
}
impl PartialEq for NativeRecovery {
    fn eq(&self, other: &Self) -> bool {
        self.paths == other.paths
            && self.token == other.token
            && self.artifacts == other.artifacts
            && self.deadline == other.deadline
            && self.nonce == other.nonce
            && self.renderer == other.renderer
            && Arc::ptr_eq(&self.notice, &other.notice)
            && Arc::ptr_eq(&self.outcome, &other.outcome)
    }
}
impl NativeRecovery {
    pub(crate) fn target(&self) -> Result<Option<NativeTarget>, String> {
        self.notice
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
            .transpose()
    }
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
const NATIVE_RECOVERY_FAILURE: &str = "native recovery requested";
pub fn is_lease_yield(error: &str) -> bool {
    error.contains(LEASE_YIELD) && !error.contains(NATIVE_RECOVERY_FAILURE)
}
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
        if first && probe == LeaseProbe::Unavailable {
            r.error("Civ6 lease record unavailable (missing or sharing conflict); retrying within the original 5s without changing ownership")?;
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
    #[cfg(unix)]
    let guard = crate::fairness::UnixGuard::acquire(&limit.paths.here.join("civ6.lock"))?;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};
        options.share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE);
    }
    // This is an observation, not an ownership pin. The tick/acquisition
    // writer can keep its write handle while checking competitors. Permit
    // that existing access without requesting write access ourselves.
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
    // The existing writer saves by truncating and rewriting. A partial JSON
    // observation cannot clear a known lease deadline or establish ownership;
    // the caller retries, retaining the deadline and the five-second bound.
    let owner: serde_json::Value = match serde_json::from_slice(&bytes) {
        Ok(owner) => owner,
        Err(_) => return Ok(LeaseProbe::Unavailable),
    };
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
            deadline: None,
            merge_stderr: false,
            creationflags: CREATE_NO_WINDOW,
            kill_tree: false,
            lease: None,
            native_recovery: None,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub image: String,
    pub created_filetime: Option<u64>,
}

/// One pinned handle supplies image, creation time and signalled exit state.
/// Access failures remain errors; an exited process is never a live waiter.
#[cfg(windows)]
pub fn native_process_identity(pid: u32) -> std::io::Result<Option<ProcessIdentity>> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME, WAIT_OBJECT_0, WAIT_TIMEOUT},
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
        let error = std::io::Error::last_os_error();
        return if error.raw_os_error() == Some(87) {
            Ok(None)
        } else {
            Err(error)
        };
    }
    let handle = Handle(raw);
    let alive = || match unsafe { WaitForSingleObject(handle.0, 0) } {
        WAIT_OBJECT_0 => Ok(false),
        WAIT_TIMEOUT => Ok(true),
        _ => Err(std::io::Error::last_os_error()),
    };
    if !alive()? {
        return Ok(None);
    }
    let mut created = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut exited = created;
    let mut kernel = created;
    let mut user = created;
    if unsafe { GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) } == 0
    {
        let error = std::io::Error::last_os_error();
        if !alive()? {
            return Ok(None);
        }
        return Err(error);
    }
    let mut path = vec![0u16; 32768];
    let mut length = path.len() as u32;
    if unsafe { QueryFullProcessImageNameW(handle.0, 0, path.as_mut_ptr(), &mut length) } == 0 {
        let error = std::io::Error::last_os_error();
        if !alive()? {
            return Ok(None);
        }
        return Err(error);
    }
    let path = String::from_utf16(&path[..length as usize])
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let image = path
        .rsplit(['/', '\\'])
        .next()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "waiter executable name is empty",
            )
        })?
        .to_owned();
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

/// If SYNCHRONIZE is unavailable, use the original snapshot/creation
/// proof instead. An access failure itself never means the waiter is absent.
#[cfg(any(windows, test))]
fn native_identity_or_fallback(
    result: std::io::Result<Option<ProcessIdentity>>,
) -> Option<Result<Option<ProcessIdentity>, String>> {
    match result {
        Err(error) if error.raw_os_error() == Some(5) => None,
        other => Some(other.map_err(|error| error.to_string())),
    }
}

#[cfg(any(windows, test))]
fn denied_snapshot_identity(
    pid: u32,
    snapshot: Result<Option<ProcessIdentity>, String>,
) -> Result<Option<ProcessIdentity>, String> {
    match snapshot? {
        Some(identity) => Ok(Some(identity)),
        None => Err(format!(
            "inspect Civ6 queue process {pid}: native query access denied; no snapshot identity; exit state unknown"
        )),
    }
}

/// All external commands (including gh/network) and waits are injectable. Tests never launch a game or call an API.
pub trait Runner {
    fn run(&mut self, call: Call) -> Result<Output, String>;
    fn terminate_game(
        &mut self,
        _pid: u32,
        _image: &str,
        _created: u64,
        _deadline: f64,
    ) -> Result<Output, String> {
        Err("pinned game termination unavailable".into())
    }
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
        _ => match native_identity_or_fallback(Err(error)) {
            None => denied_snapshot_identity(pid, snapshot_process_identity(pid)),
            Some(result) => {
                result.map_err(|error| format!("inspect Civ6 queue process {pid}: {error}"))
            }
        },
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
    match native_process_identity(pid) {
        Ok(identity) => Ok(identity),
        Err(error) => {
            let os_error = error.raw_os_error();
            let result = process_open_failure(pid, error);
            // A name-only fallback cannot prove a game generation. Retain the
            // original native query failure without exposing executable paths.
            eprintln!(
                "process_identity_diagnostic={}",
                serde_json::json!({
                    "pid":pid,"native_query_os_error":os_error,"exited":null,
                    "game_image_approved":result.as_ref().ok().and_then(|v|v.as_ref()).map(|v|crate::chain::CIV_EXECUTABLES.iter().any(|image|v.image.eq_ignore_ascii_case(image))),
                    "created_filetime":result.as_ref().ok().and_then(|v|v.as_ref()).and_then(|v|v.created_filetime),
                    "inspection_error":result.as_ref().err()
                })
            );
            result
        }
    }
}
#[cfg(windows)]
fn terminate_pinned_game(
    pid: u32,
    expected_image: &str,
    expected_created: u64,
    deadline: f64,
    clock: &mut impl FnMut() -> f64,
) -> Result<Output, String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{
            GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, TerminateProcess,
            WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
            PROCESS_TERMINATE,
        },
    };
    let check_deadline = |clock: &mut dyn FnMut() -> f64| {
        if clock() < deadline {
            Ok(())
        } else {
            Err("Civ6 termination deadline exhausted".to_owned())
        }
    };
    check_deadline(clock)?;
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
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE | PROCESS_TERMINATE,
            0,
            pid,
        )
    };
    if raw.is_null() {
        return Err(format!(
            "open pinned game {pid}: {}",
            std::io::Error::last_os_error()
        ));
    }
    let handle = Handle(raw);
    let mut path = vec![0u16; 32768];
    let mut length = path.len() as u32;
    if unsafe { QueryFullProcessImageNameW(handle.0, 0, path.as_mut_ptr(), &mut length) } == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let path = String::from_utf16(&path[..length as usize]).map_err(|e| e.to_string())?;
    let image = std::path::Path::new(&path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("pinned game image unavailable")?;
    let mut created = FILETIME::default();
    let mut exited = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    if unsafe { GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) } == 0
    {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let actual_created =
        (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
    if !image.eq_ignore_ascii_case(expected_image) || actual_created != expected_created {
        return Err("pinned game identity changed; termination refused".into());
    }
    check_deadline(clock)?;
    match unsafe { WaitForSingleObject(handle.0, 0) } {
        WAIT_OBJECT_0 => return Ok(Output::default()),
        WAIT_TIMEOUT => {}
        _ => return Err(std::io::Error::last_os_error().to_string()),
    }
    // Identity and termination share this same retained HANDLE. PID reuse after
    // a prior observation can only open a different creation time and reject.
    check_deadline(clock)?;
    if unsafe { TerminateProcess(handle.0, 1) } == 0 {
        let error = std::io::Error::last_os_error();
        if unsafe { WaitForSingleObject(handle.0, 0) } != WAIT_OBJECT_0 {
            return Err(error.to_string());
        }
    }
    Ok(Output::default())
}

#[cfg(all(test, windows))]
mod native_identity_tests {
    use super::*;
    use std::os::windows::process::CommandExt;

    #[test]
    fn real_runner_does_not_start_even_a_fast_child_after_absolute_deadline() {
        let mut call = Call::new(crate::args(&["cmd.exe", "/d", "/c", "exit 0"]))
            .timeout(10.0)
            .managed();
        call.deadline = Some(RealRunner.monotonic() - 1.0);
        assert_eq!(RealRunner.run(call).unwrap_err(), "command timed out");
    }

    #[test]
    fn pinned_termination_rejects_changed_creation_and_expired_deadline_before_owned_child_exit() {
        struct OwnedChild(std::process::Child);
        impl Drop for OwnedChild {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut owned = OwnedChild(
            Command::new("cmd.exe")
                .args(["/d", "/q", "/c", "set /p cc_termination_fixture=waiting"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .unwrap(),
        );
        let child = &mut owned.0;
        let identity = windows_process_identity(child.id()).unwrap().unwrap();
        let created = identity.created_filetime.unwrap();
        assert!(
            terminate_pinned_game(child.id(), "cmd.exe", created + 1, 10.0, &mut || 0.0).is_err()
        );
        assert!(child.try_wait().unwrap().is_none());
        let mut checks = 0;
        assert!(
            terminate_pinned_game(child.id(), "cmd.exe", created, 1.0, &mut || {
                checks += 1;
                if checks == 1 {
                    0.0
                } else {
                    2.0
                }
            })
            .is_err()
        );
        assert!(child.try_wait().unwrap().is_none());
        terminate_pinned_game(child.id(), "cmd.exe", created, 10.0, &mut || 0.0).unwrap();
        assert!(!child.wait().unwrap().success());
    }

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
    if INTERRUPTED.load(Ordering::SeqCst) && !NATIVE_CLEANUP.with(std::cell::Cell::get) {
        Err("interrupted".into())
    } else {
        Ok(())
    }
}
thread_local! {
    static NATIVE_CLEANUP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
fn with_native_cleanup<T>(action: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            NATIVE_CLEANUP.with(|state| state.set(self.0));
        }
    }
    let _restore = Restore(NATIVE_CLEANUP.with(|state| state.replace(true)));
    action()
}

impl Runner for RealRunner {
    #[cfg(windows)]
    fn terminate_game(
        &mut self,
        pid: u32,
        image: &str,
        created: u64,
        deadline: f64,
    ) -> Result<Output, String> {
        terminate_pinned_game(pid, image, created, deadline, &mut || self.monotonic())
    }
    #[cfg(windows)]
    fn process_identity(
        &mut self,
        pid: u32,
        _timeout: f64,
    ) -> Result<Option<ProcessIdentity>, String> {
        interrupted()?;
        windows_process_identity(pid)
    }
    fn run(&mut self, call: Call) -> Result<Output, String> {
        interrupted()?;
        // Preparation and spawn consume the same budget as waiting. Never
        // reset a relative timeout after creating the child/readers.
        let started = self.monotonic();
        let deadline = match (call.deadline, call.timeout) {
            (Some(deadline), Some(timeout)) => Some(deadline.min(started + timeout)),
            (Some(deadline), None) => Some(deadline),
            (None, Some(timeout)) => Some(started + timeout),
            (None, None) => None,
        };
        let before_spawn = |runner: &mut Self| {
            if deadline.is_some_and(|deadline| runner.monotonic() >= deadline) {
                Err("command timed out".to_owned())
            } else {
                Ok(())
            }
        };
        let mut cmd = Command::new(call.args.first().ok_or("empty command")?);
        if let Some(scope) = &call.native_recovery {
            cmd.env("CIV6_NATIVE_RECOVERY_NONCE", &scope.nonce);
        }
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
            before_spawn(self)?;
            let (mut child, mut tree) =
                crate::process_tree::Tree::spawn(&mut cmd, call.creationflags, call.kill_tree)?;
            drop(cmd);
            let scope = call.native_recovery.clone();
            let reader = thread::spawn(move || {
                read_driver_output(
                    &mut source,
                    scope
                        .as_ref()
                        .map(|s| (s.notice.as_ref(), s.nonce.as_str())),
                )
            });
            let status = wait_driver(
                &mut child,
                &mut tree,
                deadline,
                call.lease.as_ref(),
                call.native_recovery.as_ref(),
            );
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
        before_spawn(self)?;
        let (mut child, mut tree) =
            crate::process_tree::Tree::spawn(&mut cmd, call.creationflags, call.kill_tree)?;
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let out = thread::spawn(move || read_driver_output(&mut stdout, None));
        let scope = call.native_recovery.clone();
        let err = thread::spawn(move || {
            read_driver_output(
                &mut stderr,
                scope
                    .as_ref()
                    .map(|s| (s.notice.as_ref(), s.nonce.as_str())),
            )
        });
        let status = wait_driver(
            &mut child,
            &mut tree,
            deadline,
            call.lease.as_ref(),
            call.native_recovery.as_ref(),
        );
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
fn read_driver_output(
    mut source: impl Read,
    notice: Option<(&RecoveryNotice, &str)>,
) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let Some((notice, nonce)) = notice else {
        source.read_to_end(&mut bytes)?;
        return Ok(bytes);
    };
    let prefix = format!("{NATIVE_RECOVERY_REQUIRED} {nonce} ");
    let mut line = Vec::with_capacity(256);
    let mut oversized = false;
    let mut chunk = [0u8; 8192];
    loop {
        let count = match source.read(&mut chunk) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            other => other?,
        };
        if count == 0 {
            return Ok(bytes);
        }
        bytes.extend_from_slice(&chunk[..count]);
        for &byte in &chunk[..count] {
            if byte == b'\n' {
                if !oversized {
                    let message = line.strip_suffix(b"\r").unwrap_or(&line);
                    if let Some(payload) = message.strip_prefix(prefix.as_bytes()) {
                        let parsed = parse_native_target(payload);
                        let mut current = notice.lock().unwrap_or_else(|e| e.into_inner());
                        match current.as_ref() {
                            None => *current = Some(parsed),
                            Some(Ok(previous)) if parsed.as_ref() != Ok(previous) => {
                                *current = Some(Err(
                                    "conflicting native recovery target identities".into(),
                                ));
                            }
                            _ => {}
                        }
                    }
                }
                line.clear();
                oversized = false;
            } else if line.len() < 256 && !oversized {
                line.push(byte);
            } else {
                oversized = true;
                line.clear();
            }
        }
    }
}
fn parse_native_target(payload: &[u8]) -> Result<NativeTarget, String> {
    let payload = std::str::from_utf8(payload).map_err(|_| "invalid native recovery identity")?;
    let fields = payload.split(' ').collect::<Vec<_>>();
    if fields.len() != 2 {
        return Err("invalid native recovery identity".into());
    }
    let pid = fields[0]
        .parse::<u32>()
        .map_err(|_| "invalid native recovery PID")?;
    let created = fields[1]
        .parse::<u64>()
        .map_err(|_| "invalid native recovery creation time")?;
    if pid == 0 || created == 0 {
        return Err("empty native recovery identity".into());
    }
    Ok(NativeTarget { pid, created })
}
fn wait_driver(
    child: &mut std::process::Child,
    tree: &mut crate::process_tree::Tree,
    deadline: Option<f64>,
    lease: Option<&LeaseLimit>,
    recovery: Option<&NativeRecovery>,
) -> Result<i32, String> {
    wait_with_recovery(
        child,
        tree,
        deadline,
        lease,
        recovery.map(|scope| (scope.notice.as_ref(), scope.deadline)),
        &mut |target, deadline| {
            let scope = recovery.ok_or("native recovery scope unavailable")?;
            with_native_cleanup(|| {
                crate::chain::recover_native_driver(&mut RealRunner, scope, target, deadline)
            })
        },
        &mut native_target_exited,
    )
}
fn native_target_exited(target: NativeTarget) -> Result<bool, String> {
    #[cfg(windows)]
    {
        native_exit_from_identity(target, native_process_identity(target.pid))
    }
    #[cfg(not(windows))]
    {
        let _ = target;
        Err("native recovery requires Windows process identity".into())
    }
}
#[cfg(any(windows, test))]
fn native_exit_from_identity(
    target: NativeTarget,
    identity: std::io::Result<Option<ProcessIdentity>>,
) -> Result<bool, String> {
    match identity.map_err(|e| e.to_string())? {
        None => Ok(true),
        Some(identity) => identity
            .created_filetime
            .map(|created| created != target.created)
            .ok_or("native recovery exit state unknown".into()),
    }
}
fn wait_with_recovery(
    child: &mut std::process::Child,
    tree: &mut crate::process_tree::Tree,
    mut deadline: Option<f64>,
    lease: Option<&LeaseLimit>,
    recovery: Option<(&RecoveryNotice, f64)>,
    cleanup: &mut impl FnMut(NativeTarget, f64) -> Result<(), String>,
    exited: &mut impl FnMut(NativeTarget) -> Result<bool, String>,
) -> Result<i32, String> {
    let mut next_probe = Instant::now();
    let mut recovery_error = None;
    let mut retained_target = None;
    let mut driver_grace = None;
    let mut ownership_announced = false;
    loop {
        if recovery_error.is_none() {
            if let Some((target, reserved_end)) = recovery.and_then(|(notice, end)| {
                notice
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone()
                    .map(|target| (target, end))
            }) {
                let end = lease
                    .and_then(LeaseLimit::hard_deadline)
                    .map_or(reserved_end, |d| reserved_end.min(d));
                retained_target = target.as_ref().ok().copied();
                let result = if RealRunner.monotonic() >= end {
                    Err("original cleanup deadline exhausted".into())
                } else {
                    match child.try_wait() {
                        Ok(Some(_)) => Err("driver exited before native recovery dispatch".into()),
                        Err(error) => Err(format!(
                            "native recovery driver exit state unknown: {error}"
                        )),
                        Ok(None) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            target.and_then(|target| cleanup(target, end))
                        }))
                        .unwrap_or_else(|_| Err("native recovery cleanup panicked".into())),
                    }
                };
                recovery_error = Some(format!(
                    "{NATIVE_RECOVERY_FAILURE}; owned target cleanup: {result:?}"
                ));
                deadline = Some(end);
            }
        }
        if recovery_error.is_some() {
            if let Some(hard) = lease.and_then(LeaseLimit::hard_deadline) {
                deadline = Some(deadline.unwrap_or(hard).min(hard));
            }
            let known_exit =
                retained_target.is_some_and(|target| matches!(exited(target), Ok(true)));
            let now = RealRunner.monotonic();
            if known_exit {
                let grace = driver_grace
                    .get_or_insert_with(|| (now + 5.0).min(deadline.unwrap_or(f64::MAX)));
                if child.try_wait().map_err(|e| e.to_string())?.is_some() || now >= *grace {
                    // Only an OS-confirmed exit of this generation permits
                    // stopping its remaining native controllers/job tree.
                    tree.stop(child)?;
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(recovery_error.take().unwrap());
                }
            }
            if deadline.is_some_and(|end| now >= end) {
                if !known_exit && !ownership_announced {
                    use std::io::Write;
                    let _ = writeln!(std::io::stderr(), "native recovery: original cleanup budget exhausted; work is stopped, target exit is unconfirmed, retaining driver/job ownership without extending control authority");
                    ownership_announced = true;
                }
            } else if let Some(limit) = lease.filter(|_| Instant::now() >= next_probe) {
                next_probe = Instant::now() + Duration::from_secs(2);
                // Observation can shorten the existing cleanup budget. It
                // cannot dispatch scenario work or grant a fresh lease.
                let _ = observe_lease(&mut RealRunner, limit);
            }
            thread::sleep(Duration::from_millis(50));
            continue;
        }
        if deadline.is_some_and(|deadline| RealRunner.monotonic() >= deadline) {
            tree.stop(child)?;
            let _ = child.kill();
            let _ = child.wait();
            return Err(recovery_error.unwrap_or_else(|| "command timed out".into()));
        }
        if let Some(s) = child.try_wait().map_err(|e| e.to_string())? {
            tree.stop(child)?;
            return match recovery_error {
                Some(error) => Err(error),
                None => Ok(s.code().unwrap_or(1)),
            };
        }
        let mut lease_error = None;
        if let Some(limit) = lease {
            if recovery_error.is_none()
                && limit
                    .work_deadline()
                    .is_some_and(|d| RealRunner.monotonic() >= d)
            {
                lease_error=Some(format!("{LEASE_YIELD}: live waiter requires the shared slot; unfinished scenario, not a product failure"));
            } else if Instant::now() >= next_probe {
                next_probe = Instant::now() + Duration::from_secs(2);
                let observed = if recovery_error.is_some() {
                    observe_lease(&mut RealRunner, limit).map(|_| ())
                } else {
                    monitor_lease(&mut RealRunner, limit)
                };
                if let Err(error) = observed {
                    lease_error = Some(error);
                }
            }
            if recovery_error.is_none()
                && limit
                    .observation_deadline()
                    .is_some_and(|d| RealRunner.monotonic() >= d)
                && lease_error.is_none()
            {
                lease_error = Some(LEASE_OBSERVATION_EXHAUSTED.into());
            }
            if recovery_error.is_none()
                && limit
                    .work_deadline()
                    .is_some_and(|d| RealRunner.monotonic() >= d)
            {
                lease_error=Some(format!("{LEASE_YIELD}: live waiter requires the shared slot; unfinished scenario, not a product failure"));
            }
        }
        if lease_error.is_some()
            || (recovery_error.is_none() && interrupted().is_err())
            || deadline.is_some_and(|deadline| RealRunner.monotonic() >= deadline)
        {
            tree.stop(child)?;
            let _ = child.kill();
            let _ = child.wait();
            if let Some(error) = lease_error {
                return Err(recovery_error
                    .map_or_else(|| error.clone(), |recovery| format!("{recovery}; {error}")));
            }
            if let Some(error) = recovery_error {
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

#[cfg(all(test, windows))]
mod native_recovery_tests {
    use super::*;
    use std::io::Write;
    use std::os::windows::process::CommandExt;
    struct Owned(std::process::Child);
    impl Drop for Owned {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    fn refused_scope() -> NativeRecovery {
        // A known owned file is deliberately not a directory. Every lock or
        // artifact access must fail before any game command can be dispatched.
        // This fixture needs no temporary directory or real shared lock.
        let file = std::env::current_exe().unwrap();
        NativeRecovery {
            paths: crate::Paths {
                here: file.clone(),
                wt: file.clone(),
                jobs: file.clone(),
                temp: file.clone(),
                profile: file.clone(),
                python: "unused".into(),
            },
            token: "unowned fixture token".into(),
            artifacts: file,
            deadline: RealRunner.monotonic() + 5.0,
            nonce: "abc-123".into(),
            renderer: None,
            notice: Arc::new(Mutex::new(None)),
            outcome: Arc::new(Mutex::new(None)),
        }
    }

    #[test]
    fn real_runner_detects_bound_exit_notice_and_refuses_unowned_close_without_game_access() {
        let scope = refused_scope();
        let script = format!("$fixture=[Diagnostics.Process]::GetCurrentProcess(); [Console]::Error.WriteLine(('{NATIVE_RECOVERY_REQUIRED} {{0}} {{1}} {{2}}' -f $env:CIV6_NATIVE_RECOVERY_NONCE, $PID, $fixture.StartTime.ToFileTimeUtc())); [Threading.Thread]::Sleep(1000); exit 9");
        let mut call = Call::new(crate::args(&[
            "powershell.exe",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
        ]))
        .merged()
        .managed()
        .timeout(4.0);
        call.native_recovery = Some(scope.clone());
        let error = RealRunner.run(call).unwrap_err();
        assert!(error.contains(NATIVE_RECOVERY_FAILURE), "{error}");
        let outcome = scope.outcome.lock().unwrap().clone().unwrap();
        assert!(outcome.result.is_err());
        assert_eq!(outcome.evidence["native_recovery_close_confirmed"], false);
        assert!(scope.target().unwrap().is_some());
    }

    #[test]
    fn real_runner_ignores_an_unbound_notice_and_keeps_normal_exit_and_raw_output() {
        let scope = refused_scope();
        let text = format!("{NATIVE_RECOVERY_REQUIRED} wrong 42 71");
        let script = format!("[Console]::Error.WriteLine('{text}'); exit 0");
        let mut call = Call::new(crate::args(&[
            "powershell.exe",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
        ]))
        .merged()
        .managed()
        .timeout(4.0);
        call.native_recovery = Some(scope.clone());
        let output = RealRunner.run(call).unwrap();
        assert_eq!(output.code, 0);
        assert_eq!(output.stdout, format!("{text}\n"));
        assert!(scope.outcome.lock().unwrap().is_none());
        assert_eq!(scope.target().unwrap(), None);
    }

    #[test]
    fn recovery_notice_closes_owned_fixture_before_driver_exit_and_preserves_failure() {
        let mut peer = Owned(
            Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "Start-Sleep -Seconds 60",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .unwrap(),
        );
        let mut command = Command::new("powershell.exe");
        command.args(["-NoProfile", "-NonInteractive", "-Command", &format!("[Console]::Error.WriteLine('{NATIVE_RECOVERY_REQUIRED} abc-123 42 71'); [Console]::ReadLine() | Out-Null; exit 9")])
            .stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped());
        let (mut child, mut tree) =
            crate::process_tree::Tree::spawn(&mut command, CREATE_NO_WINDOW, true).unwrap();
        let mut input = child.stdin.take().unwrap();
        let error = child.stderr.take().unwrap();
        let notice = Arc::new(Mutex::new(None));
        let reader_notice = Arc::clone(&notice);
        let reader =
            thread::spawn(move || read_driver_output(error, Some((&reader_notice, "abc-123"))));
        let started = RealRunner.monotonic();
        let mut calls = 0;
        let result = wait_with_recovery(
            &mut child,
            &mut tree,
            Some(started + 2.0),
            None,
            Some((&notice, started + 4.0)),
            &mut |target, deadline| {
                calls += 1;
                assert_eq!(
                    target,
                    NativeTarget {
                        pid: 42,
                        created: 71
                    }
                );
                assert!(deadline <= started + 4.0);
                input
                    .write_all(b"owned target closed\n")
                    .map_err(|e| e.to_string())
            },
            &mut |_| Ok(true),
        );
        drop(tree);
        let bytes = reader.join().unwrap().unwrap();
        let peer_alive = peer.0.try_wait().unwrap().is_none();
        drop(peer);
        assert_eq!(calls, 1, "{result:?}; {}", decode(&bytes));
        let error = result.unwrap_err();
        assert!(error.contains("native recovery requested"), "{error}");
        assert!(!error.contains(LEASE_YIELD), "{error}");
        assert!(peer_alive);
    }

    #[test]
    fn recovery_notice_requires_a_complete_exact_line_and_keeps_original_bytes() {
        for (text, expected) in [
            (
                format!("{NATIVE_RECOVERY_REQUIRED} abc-123 42 71\r\n"),
                true,
            ),
            (format!("{NATIVE_RECOVERY_REQUIRED} abc-123 42 71"), false),
            (format!("{NATIVE_RECOVERY_REQUIRED} wrong 42 71\n"), false),
            (
                format!("prefix {NATIVE_RECOVERY_REQUIRED} abc-123 42 71\n"),
                false,
            ),
            (
                format!("{NATIVE_RECOVERY_REQUIRED}-other abc-123 42 71\n"),
                false,
            ),
        ] {
            let notice = Mutex::new(None);
            assert_eq!(
                read_driver_output(text.as_bytes(), Some((&notice, "abc-123"))).unwrap(),
                text.as_bytes()
            );
            assert_eq!(notice.lock().unwrap().is_some(), expected);
        }
    }

    #[test]
    fn invalid_and_conflicting_recovery_identities_never_become_close_authority() {
        for text in [format!("{NATIVE_RECOVERY_REQUIRED} abc-123 0 71\n"),
            format!("{NATIVE_RECOVERY_REQUIRED} abc-123 42 0\n"),
            format!("{NATIVE_RECOVERY_REQUIRED} abc-123 42 71 extra\n"),
            format!("{NATIVE_RECOVERY_REQUIRED} abc-123 42 71\n{NATIVE_RECOVERY_REQUIRED} abc-123 43 72\n")] {
            let notice = Mutex::new(None);
            read_driver_output(text.as_bytes(), Some((&notice, "abc-123"))).unwrap();
            assert!(notice.lock().unwrap().as_ref().unwrap().is_err());
        }
    }

    #[test]
    fn scanner_handles_every_chunk_boundary_and_ignores_oversized_lines_without_losing_raw() {
        struct SingleByte<'a>(&'a [u8]);
        impl Read for SingleByte<'_> {
            fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
                self.0.read(&mut output[..1])
            }
        }
        let text = format!("{NATIVE_RECOVERY_REQUIRED} abc-123 42 71{}\r\n{NATIVE_RECOVERY_REQUIRED} abc-123 43 72\r\n", "x".repeat(16384));
        let notice = Mutex::new(None);
        assert_eq!(
            read_driver_output(SingleByte(text.as_bytes()), Some((&notice, "abc-123"))).unwrap(),
            text.as_bytes()
        );
        assert_eq!(
            *notice.lock().unwrap(),
            Some(Ok(NativeTarget {
                pid: 43,
                created: 72
            }))
        );
    }

    #[test]
    fn access_denial_and_missing_creation_stay_unknown_while_absence_is_distinct() {
        let target = NativeTarget {
            pid: 42,
            created: 71,
        };
        assert!(
            native_exit_from_identity(target, Err(std::io::Error::from_raw_os_error(5))).is_err()
        );
        assert!(native_exit_from_identity(
            target,
            Ok(Some(ProcessIdentity {
                image: "fixture.exe".into(),
                created_filetime: None
            }))
        )
        .is_err());
        assert_eq!(native_exit_from_identity(target, Ok(None)), Ok(true));
        assert_eq!(
            native_exit_from_identity(
                target,
                Ok(Some(ProcessIdentity {
                    image: "fixture.exe".into(),
                    created_filetime: Some(71)
                }))
            ),
            Ok(false)
        );
        assert_eq!(
            native_exit_from_identity(
                target,
                Ok(Some(ProcessIdentity {
                    image: "fixture.exe".into(),
                    created_filetime: Some(72)
                }))
            ),
            Ok(true)
        );
    }

    #[test]
    fn expired_budget_keeps_driver_alive_until_exact_owned_target_exits() {
        use std::os::windows::io::{AsHandle, AsRawHandle};
        use windows_sys::Win32::{
            Foundation::{HANDLE, WAIT_TIMEOUT},
            System::Threading::WaitForSingleObject,
        };
        let mut command = Command::new("powershell.exe");
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 60",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let target = Owned(
            Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "Start-Sleep -Seconds 60",
                ])
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .unwrap(),
        );
        let identity = native_process_identity(target.0.id()).unwrap().unwrap();
        let expected = NativeTarget {
            pid: target.0.id(),
            created: identity.created_filetime.unwrap(),
        };
        let (mut child, mut tree) =
            crate::process_tree::Tree::spawn(&mut command, CREATE_NO_WINDOW, true).unwrap();
        let driver_handle = child.as_handle().try_clone_to_owned().unwrap();
        let (expired_tx, expired_rx) = std::sync::mpsc::sync_channel(0);
        let closer = thread::spawn(move || {
            let mut target = target;
            expired_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let driver_retained =
                unsafe { WaitForSingleObject(driver_handle.as_raw_handle() as HANDLE, 0) }
                    == WAIT_TIMEOUT;
            target.0.kill().unwrap();
            target.0.wait().unwrap();
            driver_retained
        });
        let now = RealRunner.monotonic();
        let end = now + 0.1;
        let notice = Mutex::new(Some(Ok(expected)));
        let mut announced = false;
        let error = wait_with_recovery(
            &mut child,
            &mut tree,
            Some(now + 1.0),
            None,
            Some((&notice, end)),
            &mut |_, _| Err("owned fixture close refused".into()),
            &mut |target| {
                if !announced && RealRunner.monotonic() >= end {
                    announced = true;
                    expired_tx.send(()).unwrap();
                }
                native_target_exited(target)
            },
        )
        .unwrap_err();
        assert!(closer.join().unwrap());
        assert!(error.contains("owned fixture close refused"), "{error}");
        assert!(child.try_wait().unwrap().is_some());
    }

    #[test]
    fn expired_native_cleanup_budget_refuses_callback_and_keeps_failure_kind() {
        let mut command = Command::new("powershell.exe");
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 60",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let (mut child, mut tree) =
            crate::process_tree::Tree::spawn(&mut command, CREATE_NO_WINDOW, true).unwrap();
        let notice = Mutex::new(Some(Ok(NativeTarget {
            pid: 42,
            created: 71,
        })));
        let now = RealRunner.monotonic();
        let mut called = false;
        let result = wait_with_recovery(
            &mut child,
            &mut tree,
            Some(now + 10.0),
            None,
            Some((&notice, now - 1.0)),
            &mut |_, _| {
                called = true;
                Ok(())
            },
            &mut |_| Ok(true),
        );
        assert!(!called);
        assert!(child.try_wait().unwrap().is_some());
        let error = result.unwrap_err();
        assert!(
            error.contains("native recovery requested")
                && error.contains("original cleanup deadline exhausted"),
            "{error}"
        );
    }

    #[test]
    fn native_recovery_uses_original_lease_hard_deadline_and_never_yield_classification() {
        let mut command = Command::new("powershell.exe");
        command
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 60",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let (mut child, mut tree) =
            crate::process_tree::Tree::spawn(&mut command, CREATE_NO_WINDOW, true).unwrap();
        let notice = Mutex::new(Some(Ok(NativeTarget {
            pid: 42,
            created: 71,
        })));
        let now = RealRunner.monotonic();
        let limit = LeaseLimit::new(&crate::Paths::default(), "unused fixture", 240.0);
        *limit.hard_deadline.lock().unwrap() = Some(now + 0.1);
        let mut called = 0;
        let error = wait_with_recovery(
            &mut child,
            &mut tree,
            Some(now + 2.0),
            Some(&limit),
            Some((&notice, now + 10.0)),
            &mut |_, deadline| {
                called += 1;
                assert!(deadline <= now + 0.1);
                Err("owned fixture close refusal".into())
            },
            &mut |_| Ok(true),
        )
        .unwrap_err();
        assert_eq!(called, 1);
        assert!(child.try_wait().unwrap().is_some());
        assert!(error.contains("owned fixture close refusal"), "{error}");
        assert!(!error.contains(LEASE_YIELD), "{error}");
    }

    #[test]
    fn native_cleanup_failure_cannot_be_requeued_as_a_clean_lease_yield() {
        assert!(is_lease_yield(&format!(
            "driver error: {LEASE_YIELD}: waiting peer"
        )));
        assert!(!is_lease_yield(&format!(
            "driver error: {NATIVE_RECOVERY_FAILURE}; nested cleanup: {LEASE_YIELD}"
        )));
        assert!(!is_lease_yield("ordinary product failure"));
    }
}

#[cfg(test)]
mod native_termination_tests {
    use super::*;
    #[test]
    fn native_waiter_access_denial_requires_the_existing_proof_not_a_dead_result() {
        assert!(native_identity_or_fallback(Err(std::io::Error::from_raw_os_error(5))).is_none());
        assert_eq!(native_identity_or_fallback(Ok(None)), Some(Ok(None)));
        let identity = ProcessIdentity {
            image: "live_chain.exe".into(),
            created_filetime: Some(41),
        };
        assert_eq!(
            native_identity_or_fallback(Ok(Some(identity.clone()))),
            Some(Ok(Some(identity)))
        );
        for code in [6, 87] {
            assert!(
                native_identity_or_fallback(Err(std::io::Error::from_raw_os_error(code)))
                    .unwrap()
                    .is_err()
            );
        }
    }
    #[test]
    fn native_denied_query_without_snapshot_identity_does_not_mean_exited() {
        assert!(denied_snapshot_identity(12, Ok(None))
            .unwrap_err()
            .contains("exit state unknown"));
        let identity = ProcessIdentity {
            image: "CivilizationVI.exe".into(),
            created_filetime: None,
        };
        assert_eq!(
            denied_snapshot_identity(12, Ok(Some(identity.clone()))).unwrap(),
            Some(identity)
        );
        assert_eq!(
            denied_snapshot_identity(12, Err("snapshot unavailable".into())).unwrap_err(),
            "snapshot unavailable"
        );
    }
}
