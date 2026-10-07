//! Run live scenarios one after another (only one Civ6 can run). Each item checks out its branch in the live worktree.
//!
//! live_chain.exe name:scenario.json [name:scenario.json ...]
//! Log: %TEMP%/batch1/live_chain.log (lines "RESULT <name> <scenario> <exit> <artifact dir>")
use crate::*;
use serde_json::json;
use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
pub const CIV_EXECUTABLES: [&str; 2] = ["CivilizationVI.exe", "CivilizationVI_DX12.exe"];

fn live_chain_image(image: &str) -> bool {
    let image = image.to_ascii_lowercase();
    image == "live_chain.exe" || (image.starts_with("live_chain_") && image.ends_with(".exe"))
}

#[cfg(windows)]
pub fn process_created_filetime(pid: u32) -> Result<Option<u64>, String> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME},
        System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        let e = std::io::Error::last_os_error();
        return if e.raw_os_error() == Some(87) {
            Ok(None)
        } else {
            Err(format!("open process {pid} to verify creation time: {e}"))
        };
    }
    let mut created = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut exited = created;
    let mut kernel = created;
    let mut user = created;
    let ok = unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) };
    let error = if ok == 0 {
        Some(std::io::Error::last_os_error())
    } else {
        None
    };
    unsafe { CloseHandle(handle) };
    if let Some(e) = error {
        return Err(format!("read process {pid} creation time: {e}"));
    }
    Ok(Some(
        (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime),
    ))
}
pub const REPORTER_COMMAND: &str = r#"
    $civ6 = Get-Process -Name CivilizationVI,CivilizationVI_DX12 -ErrorAction SilentlyContinue
    if ($civ6) { exit 0 }
    $expected = "C:\Program Files (x86)\Steam\steamapps\common\Sid Meier's Civilization VI\Base\Binaries\Win64Steam\FiraxisBugReporter.exe"
    Get-CimInstance Win32_Process -Filter "Name = 'FiraxisBugReporter.exe'" | ForEach-Object {
        if ($_.ExecutablePath -eq $expected) {
            $parent = Get-Process -Id $_.ParentProcessId -ErrorAction SilentlyContinue
            if (-not $parent -or $parent.ProcessName -notin @('CivilizationVI','CivilizationVI_DX12')) {
                Stop-Process -Id $_.ProcessId -ErrorAction Stop
            }
        }
    }
    "#;

/// Match tasklist's image field for either renderer, never its diagnostic text.
pub fn civ6_running(r: &mut impl Runner) -> Result<bool, String> {
    for executable in CIV_EXECUTABLES {
        let out = r.run(
            Call::new(args(&[
                "tasklist",
                "/FI",
                &format!("IMAGENAME eq {executable}"),
                "/FO",
                "CSV",
                "/NH",
            ]))
            .timeout(30.0),
        )?;
        if out.code != 0 {
            return Err(format!(
                "Could not inspect Civ6 processes: {}",
                tail(&out.stderr, 300)
            ));
        }
        if out
            .stdout
            .lines()
            .any(|line| csv_first(line).eq_ignore_ascii_case(executable))
        {
            return Ok(true);
        }
    }
    Ok(false)
}
fn csv_first(line: &str) -> String {
    if let Some(s) = line.strip_prefix('"') {
        let mut out = String::new();
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    out.push('"');
                } else {
                    break;
                }
            } else {
                out.push(c);
            }
        }
        out
    } else {
        line.split(',').next().unwrap_or("").into()
    }
}
/// Called only by the chain while it holds the shared Civ6 lock.
pub fn close_civ6(r: &mut impl Runner) -> Result<(), String> {
    for executable in CIV_EXECUTABLES {
        r.run(Call::new(args(&["taskkill", "/IM", executable, "/F"])).timeout(30.0))?;
    }
    // taskkill can fail (e.g. access denied). Never treat that as a closed game,
    // and allow a terminated process a short time to disappear from tasklist.
    let deadline = r.monotonic() + 15.0;
    while civ6_running(r)? {
        if r.monotonic() >= deadline {
            return Err(
                "Civ6 did not exit after taskkill; refusing to continue the live scenario".into(),
            );
        }
        r.sleep(0.5)?;
    }
    close_stale_civ6_reporters(r)
}
/// Steam counts an orphaned Civ6 crash reporter as a running game.
///
/// Check the exact Civ6 executable path and skip reporters whose Civ6 parent
/// still lives. Other Firaxis games and active Civ6 processes are untouched.
pub fn close_stale_civ6_reporters(r: &mut impl Runner) -> Result<(), String> {
    let out = r.run(
        Call::new(args(&[
            "powershell",
            "-NoProfile",
            "-Command",
            REPORTER_COMMAND,
        ]))
        .timeout(30.0),
    )?;
    if out.code != 0 {
        return Err(format!(
            "Could not close the stale Civ6 crash reporter: {}",
            tail(&out.stderr, 300)
        ));
    }
    Ok(())
}
/// If a Civ6 process exists but FireTuner does not answer, close it so restart_game can relaunch (test PC only).
pub fn ensure_responsive_civ6(r: &mut impl Runner, p: &Paths) -> Result<&'static str, String> {
    if !civ6_running(r)? {
        close_stale_civ6_reporters(r)?;
        return Ok("absent");
    }
    for _ in 0..3 {
        let mut argv = driver::tuner(p, &p.live())?;
        argv.push("states".into());
        let out = r.run(Call::new(argv).timeout(30.0))?;
        if out.code == 0 {
            break;
        }
        r.sleep(10.0)?;
    }
    // A running-but-answering Civ6 still failed restart_game with "FireTuner に接続できません" twice (prosper-05/06);
    // a fresh launch from "absent" works, so always start from a closed game.
    close_civ6(r)?;
    // 15 s was sometimes too short: restart_game then failed to reach FireTuner (unify-18/20/21).
    r.sleep(45.0)?;
    close_stale_civ6_reporters(r)?;
    Ok("killed")
}
/// Job records take precedence over a PID; the game and Steam cannot own a run.
pub fn holder_alive(r: &mut impl Runner, p: &Paths, info: &Value) -> Result<bool, String> {
    let mut has_job = false;
    for key in ["owner_job", "job"] {
        if !ship::truthy(&info[key]) {
            continue;
        }
        has_job = true;
        let Some(job) = info[key].as_str() else {
            return Ok(false);
        };
        let bytes = match fs::read(p.jobs.join(job).join("job.json")) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e.to_string()),
        };
        // A temporarily unreadable record is not evidence that its owner died.
        let job: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if !matches!(
            job["status"].as_str(),
            Some("queued" | "starting" | "running")
        ) {
            return Ok(false);
        }
    }
    if has_job && info["live_chain"] != true {
        return Ok(true);
    }
    let Some(pid) = info["pid"]
        .as_u64()
        .or_else(|| info["pid"].as_str()?.parse::<u64>().ok())
        .filter(|pid| *pid > 0 && *pid <= u32::MAX as u64)
    else {
        return Ok(false);
    };
    let Some(identity) = r.process_identity(pid as u32, 30.0)? else {
        return Ok(false);
    };
    let image_matches = (info["live_chain"] != true || live_chain_image(&identity.image))
        && ![
            "CivilizationVI.exe",
            "CivilizationVI_DX12.exe",
            "FiraxisBugReporter.exe",
            "steam.exe",
        ]
        .iter()
        .any(|excluded| identity.image.eq_ignore_ascii_case(excluded));
    if !image_matches {
        return Ok(false);
    }
    #[cfg(windows)]
    if info["live_chain"] == true {
        if let Some(expected) = info["pid_created_filetime"].as_u64() {
            let created = match identity.created_filetime {
                Some(created) => Some(created),
                None => process_created_filetime(pid as u32)?,
            };
            return Ok(created == Some(expected));
        }
    }
    Ok(true)
}
/// Multi-hour runs (200/300-turn health and tuning runs) yield to short PR checks (todo 139, 2026-10-01).
pub fn is_long(items: &[String]) -> bool {
    let re = regex::Regex::new(r"(?:^|[^0-9])(1[5-9]\d|[2-9]\d\d)turn").unwrap();
    items.iter().any(|s| re.is_match(s))
}
/// Short runs start first until a long run has waited one lease. Each waiter
/// has one effective arrival time, so every claimant sees the same total order.
pub fn my_turn(r: &mut impl Runner, p: &Paths, me: &Path) -> Result<bool, String> {
    let mine = read_json(me)?;
    let entries = fs::read_dir(p.here.join("queue")).map_err(|e| e.to_string())?;
    for entry in entries.filter_map(Result::ok) {
        let other = entry.path();
        if other == me || other.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(info) = read_json(&other) else {
            continue;
        };
        if !holder_alive(r, p, &info)? {
            remove_if_exists(&other)?;
            continue;
        }
        let rank = |v: &Value| -> Result<(f64, f64), String> {
            let since = v["since"].as_f64().ok_or("waiter since missing")?;
            let long = v["long"].as_i64().ok_or("waiter long missing")?;
            let effective = if long == 1 && v["fifo"] != true {
                since + fairness::LEASE_SECONDS
            } else {
                since
            };
            if !since.is_finite() || !effective.is_finite() {
                return Err("waiter since is not finite".into());
            }
            Ok((effective, since))
        };
        let theirs = rank(&info)?;
        let ours = rank(&mine)?;
        let before = theirs
            .0
            .total_cmp(&ours.0)
            .then_with(|| theirs.1.total_cmp(&ours.1))
            .then_with(|| other.as_path().cmp(me))
            == std::cmp::Ordering::Less;
        if before {
            return Ok(false);
        }
    }
    let items = fairness::items(&fairness::todo_path(p))?;
    let now = if items.is_empty() { 0. } else { r.now() };
    for item in items {
        let assigned = json!({"job":item["scheduler"]["job"]});
        if fairness::same_owner(&mine, &assigned) {
            continue;
        }
        if fairness::ready_item(&item, now)
            && fairness::item_since(&item, now) <= mine["since"].as_f64().unwrap_or(now)
        {
            return Ok(false);
        }
        // A dispatched waiter must reach live_chain before a yielding chain resumes.
        if item["status"] == "doing"
            && fairness::needs_civ6(&item)
            && chain_job_active(p, &item["scheduler"]["job"])?
        {
            return Ok(false);
        }
    }
    Ok(true)
}
fn chain_job_active(p: &Paths, job: &Value) -> Result<bool, String> {
    let Some(job) = job.as_str() else {
        return Ok(false);
    };
    let path = p.jobs.join(job).join("job.json");
    if !path.exists() {
        return Ok(false);
    }
    Ok(matches!(
        read_json(path)?["status"].as_str(),
        Some("queued" | "starting" | "running")
    ))
}
fn remove_if_exists(p: &Path) -> Result<(), String> {
    match fs::remove_file(p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

static LOCK_SEQUENCE: AtomicU64 = AtomicU64::new(0);
const RELEASE_WAIT_SECONDS: f64 = 5.0;
const RELEASE_RETRY_SECONDS: f64 = 0.05;

fn new_lock_token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{}-{nanos:x}-{:x}",
        std::process::id(),
        LOCK_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}

fn queue_entry_name() -> String {
    format!("{}.json", new_lock_token())
}

#[cfg(windows)]
fn open_for_owned_delete(path: &Path) -> std::io::Result<File> {
    use std::os::windows::{ffi::OsStrExt, io::FromRawHandle};
    use windows_sys::Win32::{
        Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE},
        Storage::FileSystem::{
            CreateFileW, DELETE, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, OPEN_EXISTING,
        },
    };

    let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // DELETE is requested at open time. While this handle lives, a writer or
    // renamer cannot change the path between ownership check and disposition.
    let handle = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE | DELETE,
            FILE_SHARE_READ,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_handle(handle) })
    }
}

#[cfg(not(windows))]
fn open_for_owned_delete(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().read(true).write(true).open(path)
}

struct OwnedLock {
    file: File,
    #[cfg(unix)]
    _guard: fairness::UnixGuard,
}

/// Inspect through the same protected handle that will perform the deletion.
/// An absent or different owner is never ours to close or release.
fn file_is_owned(file: &mut File, path: &Path, token: &str) -> Result<bool, String> {
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("read owned lock {}: {e}", path.display()))?;
    let info: Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("parse owned lock {}: {e}", path.display()))?;
    Ok(info["live_chain"] == true
        && info["chain_pid"].as_u64() == Some(std::process::id() as u64)
        && info["lock_token"].as_str() == Some(token))
}

fn open_owned_lock(
    r: &mut impl Runner,
    path: &Path,
    token: &str,
) -> Result<Option<OwnedLock>, String> {
    #[cfg(unix)]
    let _ = r;
    #[cfg(unix)]
    let guard = fairness::UnixGuard::acquire(path)?;
    #[cfg(windows)]
    let mut first_conflict = None;
    loop {
        match open_for_owned_delete(path) {
            Ok(mut file) => {
                if file_is_owned(&mut file, path, token)? {
                    return Ok(Some(OwnedLock {
                        file,
                        #[cfg(unix)]
                        _guard: guard,
                    }));
                }
                return Ok(None);
            }
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            #[cfg(windows)]
            Err(e) if matches!(e.raw_os_error(), Some(32 | 33)) => {
                let since = *first_conflict.get_or_insert_with(|| r.monotonic());
                if r.monotonic() - since >= RELEASE_WAIT_SECONDS {
                    return Err(format!(
                        "open owned lock {} for release: {e}",
                        path.display()
                    ));
                }
                r.sleep(RELEASE_RETRY_SECONDS).map_err(|e| {
                    format!(
                        "wait to open owned lock {} for release: {e}",
                        path.display()
                    )
                })?;
            }
            Err(e) => {
                return Err(format!(
                    "open owned lock {} for release: {e}",
                    path.display()
                ))
            }
        }
    }
}

/// A persistent read-only inspector can block DELETE access. The chain still
/// closes its own game under read protection and reports the release failure.
#[cfg(windows)]
fn close_owned_after_release_error(
    r: &mut impl Runner,
    path: &Path,
    token: &str,
) -> Result<bool, String> {
    let mut opts = OpenOptions::new();
    opts.read(true);
    use std::os::windows::fs::OpenOptionsExt;
    opts.share_mode(1);
    let mut file = match opts.open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(format!("open owned lock {} for close: {e}", path.display())),
    };
    if !file_is_owned(&mut file, path, token)? {
        return Ok(false);
    }
    let closed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| close_civ6(r)))
        .unwrap_or_else(|_| Err("Civ6 cleanup panicked".into()));
    drop(file);
    closed?;
    Ok(true)
}

#[cfg(windows)]
fn delete_owned_handle(
    lock: OwnedLock,
    path: &Path,
    r: &mut impl Runner,
    _token: &str,
) -> Result<(), String> {
    use std::{mem::size_of, os::windows::io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::{
        FileDispositionInfo, SetFileInformationByHandle, FILE_DISPOSITION_INFO,
    };

    let disposition = FILE_DISPOSITION_INFO { DeleteFile: true };
    let mut first_conflict = None;
    loop {
        let ok = unsafe {
            SetFileInformationByHandle(
                lock.file.as_raw_handle(),
                FileDispositionInfo,
                &disposition as *const _ as *const _,
                size_of::<FILE_DISPOSITION_INFO>() as u32,
            )
        };
        if ok != 0 {
            drop(lock); // The verified file is deleted when its final handle closes.
            return Ok(());
        }
        let e = std::io::Error::last_os_error();
        if matches!(e.raw_os_error(), Some(32 | 33)) {
            let since = *first_conflict.get_or_insert_with(|| r.monotonic());
            if r.monotonic() - since < RELEASE_WAIT_SECONDS {
                r.sleep(RELEASE_RETRY_SECONDS).map_err(|e| {
                    format!(
                        "wait to mark owned lock {} for deletion: {e}",
                        path.display()
                    )
                })?;
                continue;
            }
        }
        return Err(format!(
            "mark owned lock {} for deletion: {e}",
            path.display()
        ));
    }
}

#[cfg(not(windows))]
fn delete_owned_handle(
    mut lock: OwnedLock,
    path: &Path,
    _r: &mut impl Runner,
    token: &str,
) -> Result<(), String> {
    // On Unix, reject a replaced path before unlinking. Windows uses the
    // handle operation above so replacement cannot race the final check.
    use std::os::unix::fs::MetadataExt;
    let opened = lock
        .file
        .metadata()
        .map_err(|e| format!("stat owned lock {}: {e}", path.display()))?;
    let current =
        fs::metadata(path).map_err(|e| format!("stat owned lock path {}: {e}", path.display()))?;
    if (opened.dev(), opened.ino()) != (current.dev(), current.ino()) {
        return Err(format!("owned lock path {} was replaced", path.display()));
    }
    lock.file
        .seek(SeekFrom::Start(0))
        .map_err(|e| format!("read owned lock {}: {e}", path.display()))?;
    if !file_is_owned(&mut lock.file, path, token)? {
        return Err(format!(
            "owned lock {} changed before unlink",
            path.display()
        ));
    }
    fs::remove_file(path).map_err(|e| format!("unlink owned lock {}: {e}", path.display()))
}
/// Only one live Civ6 run at a time across sessions and Codex jobs (single game instance).
pub fn acquire(
    r: &mut impl Runner,
    p: &Paths,
    owner: &Value,
    wait_s: f64,
    long_run: bool,
) -> Result<(), String> {
    let deadline = r.now() + wait_s;
    fs::create_dir_all(p.here.join("queue")).map_err(|e| e.to_string())?;
    let me = p.here.join("queue").join(queue_entry_name());
    let mut mine = owner.clone();
    mine["long"] = json!(i32::from(long_run));
    mine["since"] = json!(r.now());
    // A Unix claimant must publish its priority under the same persistent
    // guard that protects another claimant's my_turn check and lock creation.
    #[cfg(unix)]
    {
        let _guard = fairness::UnixGuard::acquire(&p.here.join("civ6.lock"))?;
        write_json(&me, &mine)?;
    }
    #[cfg(windows)]
    write_json(&me, &mine)?;
    let result = acquire_inner(r, p, owner, deadline, &me);
    #[cfg(unix)]
    let cleanup = {
        let _guard = fairness::UnixGuard::acquire(&p.here.join("civ6.lock"))?;
        remove_if_exists(&me)
    };
    #[cfg(windows)]
    let cleanup = remove_if_exists(&me);
    result.and(cleanup)
}
fn acquire_inner(
    r: &mut impl Runner,
    p: &Paths,
    owner: &Value,
    deadline: f64,
    me: &Path,
) -> Result<(), String> {
    let lock = p.here.join("civ6.lock");
    // ERROR_ACCESS_DENIED can mean a deleted lock still has a compatible
    // reader. Keep this much shorter than the ordinary six-hour queue wait:
    // an ACL denial must fail with its path and operation, not wait for hours.
    #[cfg(windows)]
    let mut access_denied_since = None;
    loop {
        #[cfg(unix)]
        let guard = fairness::UnixGuard::acquire(&lock)?;
        if !my_turn(r, p, me)? {
            #[cfg(windows)]
            {
                access_denied_since = None;
            }
            if r.now() > deadline {
                return Err("civ6.lock: gave up waiting behind higher-priority runs".into());
            }
            #[cfg(unix)]
            drop(guard);
            r.sleep(30.0)?;
            continue;
        }
        let mut opts = OpenOptions::new();
        opts.read(true).write(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            opts.share_mode(1); // FILE_SHARE_READ: protect inspection and owner replacement.
        }
        match opts.clone().create_new(true).open(&lock) {
            Ok(mut fd) => {
                let owner = acquired_owner(r, owner);
                fd.write_all(python_json(&owner).as_bytes())
                    .map_err(|e| e.to_string())?;
                return Ok(());
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                let mut fd = match opts.open(&lock) {
                    Ok(fd) => fd,
                    Err(e) if e.kind() == ErrorKind::NotFound => continue,
                    #[cfg(windows)]
                    Err(e) if is_delete_pending_or_denied(&e) => {
                        wait_for_transient_open_denial(
                            r,
                            &lock,
                            &mut access_denied_since,
                            "open",
                            e,
                        )?;
                        continue;
                    }
                    // A competing claimant or the reaper owns the write handle.
                    #[cfg(windows)]
                    Err(e) if matches!(e.raw_os_error(), Some(32 | 33)) => {
                        access_denied_since = None;
                        if r.now() > deadline {
                            return Err("civ6.lock is being claimed by another process".into());
                        }
                        r.sleep(0.5)?;
                        continue;
                    }
                    Err(e) => return Err(e.to_string()),
                };
                #[cfg(windows)]
                {
                    access_denied_since = None;
                }
                let mut bytes = Vec::new();
                fd.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
                let info = serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({}));
                if !holder_alive(r, p, &info)? {
                    // Adopt in place under the protected handle: never delete a
                    // path another claimant could have replaced after our check.
                    fd.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
                    fd.set_len(0).map_err(|e| e.to_string())?;
                    let owner = acquired_owner(r, owner);
                    fd.write_all(python_json(&owner).as_bytes())
                        .and_then(|_| fd.sync_all())
                        .map_err(|e| e.to_string())?;
                    return Ok(());
                }
                drop(fd);
                if r.now() > deadline {
                    return Err(format!("civ6.lock held by {}", python_repr(&info)));
                }
                r.print(&format!(
                    "waiting for civ6.lock held by {}",
                    python_repr(&info)
                ))?;
                #[cfg(unix)]
                drop(guard);
                r.sleep(60.0)?;
            }
            #[cfg(windows)]
            Err(e) if is_delete_pending_or_denied(&e) => {
                wait_for_transient_open_denial(r, &lock, &mut access_denied_since, "create", e)?;
            }
            #[cfg(windows)]
            Err(e) if matches!(e.raw_os_error(), Some(32 | 33)) => {
                access_denied_since = None;
                if r.now() > deadline {
                    return Err(format!("create civ6.lock {}: {e}", lock.display()));
                }
                r.sleep(0.5)?;
            }
            Err(e) => return Err(e.to_string()),
        }
    }
}

#[cfg(windows)]
fn is_delete_pending_or_denied(error: &std::io::Error) -> bool {
    use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_DELETE_PENDING};
    matches!(
        error.raw_os_error(),
        Some(code) if code == ERROR_ACCESS_DENIED as i32 || code == ERROR_DELETE_PENDING as i32
    )
}

#[cfg(windows)]
fn wait_for_transient_open_denial(
    r: &mut impl Runner,
    path: &Path,
    since: &mut Option<f64>,
    operation: &str,
    error: std::io::Error,
) -> Result<(), String> {
    let first = *since.get_or_insert_with(|| r.monotonic());
    if r.monotonic() - first >= RELEASE_WAIT_SECONDS {
        return Err(format!(
            "{operation} civ6.lock {} after bounded access-denied wait: {error}",
            path.display()
        ));
    }
    r.sleep(RELEASE_RETRY_SECONDS).map_err(|e| {
        format!(
            "wait to {operation} civ6.lock {} after access denied: {e}",
            path.display()
        )
    })
}
fn acquired_owner(r: &mut impl Runner, owner: &Value) -> Value {
    let mut owner = owner.clone();
    if owner["live_chain"] == true {
        owner["acquired_at"] = json!(r.now());
    }
    owner
}
fn python_repr(v: &Value) -> String {
    match v {
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("'{}': {}", k, python_repr(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::String(s) => format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
        Value::Null => "None".into(),
        Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        _ => v.to_string(),
    }
}

pub fn main(r: &mut impl Runner, p: &Paths, a: &[String]) -> Result<i32, String> {
    let options = Options::parse(a)?;
    if options.help {
        r.print(HELP)?;
        return Ok(0);
    }
    if options.reliability {
        return cli(r, p, a);
    }
    main_owned(r, p, &options, None, None, &mut false, &mut None)
}

struct Reliability {
    revision: Option<String>,
    summary_path: std::path::PathBuf,
    summary: Value,
}
impl Reliability {
    fn round(&mut self) -> &mut Value {
        self.summary["rounds"]
            .as_array_mut()
            .unwrap()
            .last_mut()
            .unwrap()
    }
    fn save(&self) -> Result<(), String> {
        write_json(&self.summary_path, &self.summary)
    }
}

/// Preserve the original tracked inputs alongside the prepared scenario.
fn source_manifest(
    r: &mut impl Runner,
    p: &Paths,
    scenario: &str,
    prepared: &str,
    deadline: f64,
) -> Result<Value, String> {
    let timeout = 30.0_f64.min(time_left(r, deadline)?);
    let out = r.run(
        Call::new(args(&[
            "git",
            "-C",
            &path(p.live()),
            "show",
            "--format=%H%n%T",
            "--no-patch",
            "HEAD",
        ]))
        .managed()
        .timeout(timeout),
    )?;
    let ids: Vec<_> = out
        .stdout
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if out.code != 0
        || ids.len() != 2
        || ids
            .iter()
            .any(|id| !matches!(id.len(), 40 | 64) || !id.bytes().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(format!(
            "could not identify prepared SOURCE (exit {}): {} {}",
            out.code, out.stdout, out.stderr
        ));
    }
    let (revision, tree) = (ids[0], ids[1]);
    let mut files = serde_json::Map::new();
    for relative in [
        driver::MANIFEST.to_owned(),
        "tools/Cargo.toml".into(),
        "tools/Cargo.lock".into(),
        "tools/civ6-autotest-rs/Cargo.lock".into(),
        format!("tools/civ6-autotest/scenarios/{scenario}"),
    ] {
        let file = p.live().join(&relative);
        let contents = match fs::read_to_string(&file) {
            Ok(text) => json!(text),
            Err(error) if error.kind() == ErrorKind::NotFound => Value::Null,
            Err(error) => return Err(format!("read SOURCE {}: {error}", file.display())),
        };
        files.insert(relative, contents);
    }
    Ok(
        json!({"revision":revision,"source_tree":tree,"worktree":path(p.live()),"files":files,
        "prepared_scenario_path":prepared,"prepared_scenario":read_json(prepared)?}),
    )
}

pub(crate) struct WithinDeadline<'a, R> {
    pub(crate) runner: &'a mut R,
    pub(crate) deadline: f64,
}
impl<R: Runner> Runner for WithinDeadline<'_, R> {
    fn process_identity(
        &mut self,
        pid: u32,
        timeout: f64,
    ) -> Result<Option<process::ProcessIdentity>, String> {
        let left = time_left(self.runner, self.deadline)?;
        self.runner.process_identity(pid, timeout.min(left))
    }
    fn run(&mut self, mut call: Call) -> Result<Output, String> {
        let left = time_left(self.runner, self.deadline)?;
        call.timeout = Some(call.timeout.unwrap_or(left).min(left));
        self.runner.run(call)
    }
    fn now(&mut self) -> f64 {
        self.runner.now()
    }
    fn monotonic(&mut self) -> f64 {
        self.runner.monotonic()
    }
    fn sleep(&mut self, seconds: f64) -> Result<(), String> {
        let left = time_left(self.runner, self.deadline)?;
        if seconds >= left {
            self.runner.sleep(left)?;
            return Err("Civ6 scenario deadline exhausted while waiting".into());
        }
        self.runner.sleep(seconds)
    }
    fn print(&mut self, text: &str) -> Result<(), String> {
        self.runner.print(text)
    }
    fn error(&mut self, text: &str) -> Result<(), String> {
        self.runner.error(text)
    }
}

const HELP: &str = "Usage: live_chain [--test-mode] [--stop-on-failure] [--reliability] name:scenario.json [...]\n--reliability: run exactly one scenario 10 times, release the lock after each run, stop on any failure.\n-h, --help: show this help without acquiring the Civ6 lock.";
const CLEANUP_RESERVE_SECONDS: f64 = 240.0;
#[derive(Default)]
struct Options {
    items: Vec<String>,
    test_mode: bool,
    stop: bool,
    reliability: bool,
    help: bool,
}
impl Options {
    fn parse(a: &[String]) -> Result<Self, String> {
        let mut options = Self::default();
        for arg in a {
            match arg.as_str() {
                "--help" | "-h" => options.help = true,
                "--test-mode" => options.test_mode = true,
                "--stop-on-failure" => options.stop = true,
                "--reliability" => options.reliability = true,
                _ if arg.starts_with('-') => return Err(format!("unknown argument: {arg}")),
                _ => {
                    let (name, scenario) =
                        arg.split_once(':').ok_or("expected name:scenario.json")?;
                    if name.is_empty()
                        || scenario.is_empty()
                        || name.trim() != name
                        || scenario.trim() != scenario
                    {
                        return Err(format!("expected nonempty name:scenario.json: {arg}"));
                    }
                    options.items.push(arg.clone());
                }
            }
        }
        if options.reliability && options.items.len() != 1 && !options.help {
            return Err("--reliability needs exactly one name:scenario.json".into());
        }
        if options.items.is_empty() && !options.help {
            return Err("at least one name:scenario.json is required; use --help for usage".into());
        }
        Ok(options)
    }
}

/// Reserve bounded cleanup time and account for a competitor already waiting.
/// A waiter arriving during the synchronous child run still waits at most one
/// scenario window (30 minutes including preparation and cleanup).
fn scenario_window(r: &mut impl Runner, p: &Paths, token: Option<&str>) -> Result<f64, String> {
    let mut remaining = fairness::LEASE_SECONDS;
    if let Some(token) = token {
        let mut lock =
            fairness::Lock::open(p)?.ok_or("owned Civ6 lock unavailable before preparation")?;
        if lock.info["lock_token"].as_str() != Some(token) {
            return Err("Civ6 lock ownership changed before preparation".into());
        }
        lock.expired(r, p, &fairness::todo_path(p))?;
        if let Some(since) = lock.info["competitor_since"].as_f64() {
            remaining = remaining.min(fairness::LEASE_SECONDS - (r.now() - since).max(0.0));
        }
    }
    let remaining = remaining - CLEANUP_RESERVE_SECONDS;
    if remaining <= 0.0 {
        return Err("Civ6 lease has insufficient time for another scenario and cleanup".into());
    }
    Ok(remaining)
}

fn time_left(r: &mut impl Runner, deadline: f64) -> Result<f64, String> {
    let remaining = deadline - r.monotonic();
    if remaining > 0.0 {
        Ok(remaining)
    } else {
        Err("Civ6 scenario deadline exhausted".into())
    }
}

fn main_owned(
    r: &mut impl Runner,
    p: &Paths,
    options: &Options,
    token: Option<&str>,
    mut reliability: Option<&mut Reliability>,
    game_closed: &mut bool,
    cleanup_deadline: &mut Option<f64>,
) -> Result<i32, String> {
    let mut first_failure = 0;
    for item in &options.items {
        *game_closed = false;
        if let Some(token) = token {
            yield_if_expired(r, p, token)?;
        }
        let (name, scenario) = item.split_once(':').ok_or("expected name:scenario.json")?;
        let window = scenario_window(r, p, token)?;
        let deadline = r.monotonic() + window;
        *cleanup_deadline = Some(deadline + CLEANUP_RESERVE_SECONDS);
        if let Some(state) = reliability.as_deref_mut() {
            state.round()["prepare_window_seconds"] = json!(window);
        }
        // Close while holding the acquired shared lock, before checkout can
        // touch mod files. Never close and replay a failed preparation.
        let protection = if let Some(token) = token {
            Some(
                open_owned_lock(r, &p.here.join("civ6.lock"), token)?
                    .ok_or("Civ6 lock ownership changed before preparation close")?,
            )
        } else {
            None
        };
        close_civ6(&mut WithinDeadline {
            runner: r,
            deadline,
        })?;
        drop(protection);
        *game_closed = true;
        let ship = std::env::var("CC_LIVE_SHIP").unwrap_or_else(|_| path(p.here.join("ship.exe")));
        let prepare_args = args(&[&ship, "live", name, scenario]);
        let prepare_timeout = time_left(r, deadline)?.min(1200.0);
        let out = r
            .run(Call::new(prepare_args).timeout(prepare_timeout).managed())
            .map_err(|e| format!("prepare {name} {scenario} failed: {e}"))?;
        if let Some(state) = reliability.as_deref_mut() {
            state.round()["prepare_exit_code"] = json!(windows_exit_code(out.code));
            state.round()["prepare_stderr"] = json!(out.stderr);
            state.round()["prepare_elapsed_seconds"] =
                json!((r.monotonic() - (deadline - window)).max(0.0));
        }
        let lines: Vec<_> = out.stdout.trim().lines().collect();
        if out.code != 0 || lines.len() < 2 {
            let code = if out.code == 0 { 1 } else { out.code };
            r.print(&format!(
                "RESULT {name} {scenario} prepare-failed {} stdout={} stderr={}",
                windows_exit_code(code),
                tail(out.stdout.trim(), 800),
                tail(out.stderr.trim(), 1600)
            ))?;
            if options.stop || options.reliability {
                return Ok(code);
            }
            if first_failure == 0 {
                first_failure = code;
            }
            continue;
        }
        let (live_json, artifacts) = (lines[lines.len() - 2], lines[lines.len() - 1]);
        let scenario_timeout = driver::scenario_timeout(Path::new(live_json))?;
        // Reserve ship's sequence directory before running the driver. This
        // works with the installed ship protocol (only two output lines) and
        // keeps its next save/load suffix unique, including failed spawns.
        let artifacts = if let Some(state) = reliability.as_deref_mut() {
            let id = state.round()["run_id"].as_str().unwrap().to_owned();
            let root = Path::new(artifacts);
            fs::create_dir_all(root.parent().ok_or("artifact root has no parent")?)
                .map_err(|e| e.to_string())?;
            fs::create_dir(root)
                .map_err(|e| format!("reserve reliability artifacts {}: {e}", root.display()))?;
            let artifacts = path(root.join(&id));
            state.round()["artifacts"] = json!(artifacts);
            let source = source_manifest(r, p, scenario, live_json, deadline)?;
            let source_path = root.join("source-manifest.json");
            write_json(&source_path, &source)?;
            let revision = source["revision"].as_str().unwrap().to_owned();
            let tree = source["source_tree"].as_str().unwrap().to_owned();
            state.round()["source"] = source;
            state.round()["source_manifest_path"] = json!(path(source_path));
            r.print(&format!("SOURCE {name} {scenario} {revision}"))?;
            if let Some(expected) = &state.revision {
                if *expected != tree {
                    return Err(format!("reliability SOURCE changed: expected tree {expected}, prepared {tree}; refusing another driver run"));
                }
            } else {
                state.revision = Some(tree.clone());
                state.summary["source_revision"] = json!(revision);
                state.summary["source_tree"] = json!(tree);
            }
            artifacts
        } else {
            artifacts.into()
        };
        r.print(&format!(
            "START {name} {scenario} civ6=closed-before-prepare"
        ))?;
        // Use the driver of the branch under test: main's driver has moved past the old driver-diplo worktree
        // (popup closers from #65, save dismissing the tutorial advisor from #64).
        let mut argv = driver::scenario(p, &p.live())?;
        argv.extend(args(&[
            live_json,
            "--no-auto-restart",
            "--output",
            &artifacts,
        ]));
        if options.test_mode {
            argv.push("--test-mode".into());
        }
        let timeout = scenario_timeout;
        let lease = token.map(|token| process::LeaseLimit::new(p, token, CLEANUP_RESERVE_SECONDS));
        if let Some(limit) = &lease {
            process::observe_lease(r, limit)?;
            if let Some(deadline) = limit.work_deadline() {
                if r.monotonic() >= deadline {
                    return Err(format!(
                        "{}: waiting peer requires the slot before another driver run",
                        process::LEASE_YIELD
                    ));
                }
            }
        }
        *cleanup_deadline = Some(r.monotonic() + timeout + CLEANUP_RESERVE_SECONDS);
        if let Some(state) = reliability.as_deref_mut() {
            state.round()["scenario_budget_seconds"] = json!(scenario_timeout);
            state.round()["driver_timeout_seconds"] = json!(timeout);
        }
        let mut command = Call::new(argv)
            .env("CIV6_LIVE_CHAIN_PID", std::process::id().to_string())
            .env("CIV6_LIVE_LOCK_PATH", path(p.here.join("civ6.lock")))
            .merged()
            .managed()
            .timeout(timeout);
        command.lease = lease.clone();
        let driver_started = r.monotonic();
        *game_closed = false;
        let execution = r.run(command);
        if let Some(deadline) = lease.as_ref().and_then(|limit| limit.hard_deadline()) {
            *cleanup_deadline = Some(cleanup_deadline.unwrap().min(deadline));
            if let Some(state) = reliability.as_deref_mut() {
                state.round()["contention_deadline_monotonic"] = json!(deadline);
            }
        }
        if let Some(state) = reliability.as_deref_mut() {
            state.round()["driver_elapsed_seconds"] =
                json!((r.monotonic() - driver_started).max(0.0));
        }
        let run = match execution {
            Ok(run) => run,
            Err(error) => {
                if let Some(state) = reliability.as_deref_mut() {
                    state.round()["driver_error"] = json!(error);
                }
                let error = format!("driver {name} {scenario} failed (artifacts {artifacts}, deadline {timeout:.1}s): {error}");
                let logged = write_text(format!("{artifacts}.log"), &error);
                r.print(&format!(
                    "RESULT {name} {scenario} driver-error {artifacts}"
                ))?;
                return Err(match logged {
                    Ok(()) => error,
                    Err(log_error) => format!("{error}; log failed: {log_error}"),
                });
            }
        };
        if let Some(state) = reliability.as_deref_mut() {
            state.round()["driver_exit_code"] = json!(windows_exit_code(run.code));
        }
        // A long-running Civ6 slows the PC down; close it as soon as each run ends (owner, 2026-10-01).
        write_text(
            format!("{artifacts}.log"),
            &format!("{}{}", run.stdout, run.stderr),
        )?;
        if let Err(error) = close_civ6(&mut WithinDeadline {
            runner: r,
            deadline: cleanup_deadline.unwrap(),
        }) {
            return Err(format!(
                "driver exit {}; Civ6 close failed: {error}",
                windows_exit_code(run.code)
            ));
        }
        *game_closed = true;
        for line in run.stdout.lines().filter(|s| {
            ["[OK]", "[FAIL]", "PASS", "FAIL"]
                .iter()
                .any(|p| s.starts_with(p))
        }) {
            r.print(&format!("  {}", head(line, 300)))?;
        }
        r.print(&format!(
            "RESULT {name} {scenario} {} {artifacts}",
            windows_exit_code(run.code)
        ))?;
        if (options.stop || options.reliability) && run.code != 0 {
            r.print("CHAIN STOPPED ON FAILURE")?;
            return Ok(run.code);
        }
        if first_failure == 0 && run.code != 0 {
            first_failure = run.code;
        }
    }
    r.print(if first_failure == 0 {
        "CHAIN DONE"
    } else {
        "CHAIN FAILED"
    })?;
    Ok(first_failure)
}
/// Called only at a scenario boundary; never revoke a running chain from tick.
pub fn yield_if_expired(r: &mut impl Runner, p: &Paths, token: &str) -> Result<bool, String> {
    let path = p.here.join("civ6.lock");
    let mut first_conflict = None;
    let mut lock = loop {
        if let Some(lock) = fairness::Lock::open(p)? {
            break lock;
        }
        let since = *first_conflict.get_or_insert_with(|| r.monotonic());
        if r.monotonic() - since >= RELEASE_WAIT_SECONDS {
            return Err(format!(
                "open owned lock {} before scenario: unavailable",
                path.display()
            ));
        }
        r.sleep(RELEASE_RETRY_SECONDS).map_err(|e| {
            format!(
                "wait to open owned lock {} before scenario: {e}",
                path.display()
            )
        })?;
    };
    if lock.info["live_chain"] != true
        || lock.info["chain_pid"].as_u64() != Some(std::process::id() as u64)
        || lock.info["lock_token"].as_str() != Some(token)
    {
        return Err(format!(
            "civ6.lock ownership changed before scenario: {}",
            path.display()
        ));
    }
    if !lock.expired(r, p, &fairness::todo_path(p))? {
        return Ok(false);
    }
    let mut owner = lock.info.clone();
    owner.as_object_mut().unwrap().remove("competitor_since");
    owner["fifo"] = json!(true);
    // The current scenario has ended and its game has already been closed.
    drop(lock);
    let file = open_owned_lock(r, &path, token)?.ok_or_else(|| {
        format!(
            "civ6.lock ownership changed before yield: {}",
            path.display()
        )
    })?;
    delete_owned_handle(file, &path, r, token)?;
    r.print("Civ6 lease expired; re-queued behind waiting runs")?;
    owner["acquired_at"] = json!(r.now());
    acquire(r, p, &owner, 6. * 3600., false)?;
    Ok(true)
}
pub fn cli(r: &mut impl Runner, p: &Paths, a: &[String]) -> Result<i32, String> {
    // All syntax is checked before process inspection, queue registration or
    // lock acquisition. An invalid later item cannot launch an earlier one.
    let options = Options::parse(a)?;
    if options.help {
        r.print(HELP)?;
        return Ok(0);
    }
    ship::check_live_maintenance(p, std::env::var("CODEX_JOB_ID").ok().as_deref())?;
    if !options.reliability {
        return cli_once(r, p, &options, None);
    }
    let session = new_lock_token();
    let mut reliability = Reliability {
        revision: None,
        summary_path: p
            .temp
            .join(format!("live_chain_reliability_{session}.json")),
        summary: json!({"schema_version":1,"session":session,"scenario":options.items[0],
            "required_consecutive_successes":10,"consecutive_successes":0,"result":"running","rounds":[]}),
    };
    reliability.save()?;
    r.print(&format!(
        "RELIABILITY SUMMARY {}",
        path(&reliability.summary_path)
    ))?;
    for round in 1..=10 {
        let id = format!("reliability-{session}-{round:02}");
        reliability.summary["rounds"]
            .as_array_mut()
            .unwrap()
            .push(json!({"round":round,"run_id":id,"result":"running"}));
        r.print(&format!(
            "RELIABILITY START {round}/10 {}",
            options.items[0]
        ))?;
        let result = cli_once(r, p, &options, Some(&mut reliability));
        let code = match &result {
            Ok(code) => *code,
            Err(_) => 1,
        };
        reliability.round()["exit_code"] = json!(windows_exit_code(code));
        let yielded = result
            .as_ref()
            .err()
            .is_some_and(|e| e.contains(process::LEASE_YIELD));
        reliability.round()["result"] = json!(if code == 0 {
            "success"
        } else if yielded {
            "incomplete_shared_slot_yield"
        } else {
            "failed"
        });
        if let Err(error) = &result {
            reliability.round()["error"] = json!(error);
        }
        if code != 0 {
            reliability.summary["result"] = json!(if yielded {
                "incomplete_shared_slot_yield"
            } else {
                "failed"
            });
            let saved = reliability.save();
            r.print(&format!(
                "RELIABILITY FAILED {round}/10 exit={}",
                windows_exit_code(code)
            ))?;
            return match (result, saved) {
                (Err(error), Err(save)) => Err(format!("{error}; summary failed: {save}")),
                (_, Err(save)) => Err(save),
                (result, Ok(())) => result,
            };
        }
        reliability.summary["consecutive_successes"] = json!(round);
        reliability.summary["result"] = json!(if round == 10 { "success" } else { "running" });
        reliability.save()?;
        // Only a completed scenario AND successful cleanup/release count.
        r.print(&format!(
            "RELIABILITY SUCCESS {round}/10 {}",
            options.items[0]
        ))?;
    }
    r.print("RELIABILITY DONE 10/10")?;
    Ok(0)
}

fn cli_once(
    r: &mut impl Runner,
    p: &Paths,
    options: &Options,
    mut reliability: Option<&mut Reliability>,
) -> Result<i32, String> {
    process::begin_cleanup_cycle();
    let mut owner = std::env::var("CODEX_JOB_ID")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|job| json!({"job":job}))
        .unwrap_or_else(|| json!({"pid":std::process::id()}));
    owner["live_chain"] = json!(true);
    owner["pid"] = json!(std::process::id());
    owner["chain_pid"] = json!(std::process::id());
    // Rejoin the existing priority queue at every reliability boundary;
    // preserve its short-run priority and aging rules.
    if reliability.is_some() {
        owner["fifo"] = json!(true);
    }
    #[cfg(windows)]
    {
        owner["pid_created_filetime"] = json!(process_created_filetime(std::process::id())?
            .ok_or("live_chain process disappeared before registration")?);
    }
    let token = new_lock_token();
    owner["lock_token"] = json!(token);
    acquire(r, p, &owner, 6.0 * 3600.0, is_long(&options.items))?;
    let mut game_closed = false;
    let mut cleanup_deadline = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        main_owned(
            r,
            p,
            options,
            Some(&token),
            reliability.as_deref_mut(),
            &mut game_closed,
            &mut cleanup_deadline,
        )
    }))
    .unwrap_or_else(|_| Err("live chain panicked".into()));
    process::clear_interrupt_for_cleanup();
    // Also after a crash or failure of the chain itself. Release even when cleanup raises an error.
    let path = p.here.join("civ6.lock");
    let mut lock_released = false;
    let cleanup_started = r.monotonic();
    let cleanup_end = cleanup_deadline.unwrap_or_else(|| r.monotonic() + CLEANUP_RESERVE_SECONDS);
    let cleanup = (|| {
        let mut budget = WithinDeadline {
            runner: r,
            deadline: cleanup_end,
        };
        let r = &mut budget;
        let file = match open_owned_lock(r, &path, &token) {
            Ok(file) => file,
            Err(release_error) => {
                if game_closed {
                    return Err(release_error);
                }
                #[cfg(windows)]
                {
                    return match close_owned_after_release_error(r, &path, &token) {
                        Ok(true) => Err(release_error),
                        Ok(false) => Err(format!(
                            "{release_error}; lock ownership changed before fallback close"
                        )),
                        Err(close_error) => Err(format!("{release_error}; {close_error}")),
                    };
                }
                #[cfg(not(windows))]
                {
                    return Err(release_error);
                }
            }
        };
        let Some(file) = file else {
            return Err(format!(
                "civ6.lock ownership changed before cleanup: {}",
                path.display()
            ));
        };
        let closed = if game_closed {
            Ok(())
        } else {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match cleanup_deadline {
                Some(deadline) => close_civ6(&mut WithinDeadline {
                    runner: r,
                    deadline,
                }),
                None => close_civ6(r),
            }))
            .unwrap_or_else(|_| Err("Civ6 cleanup panicked".into()))
        };
        game_closed = closed.is_ok();
        let released = delete_owned_handle(file, &path, r, &token);
        lock_released = released.is_ok();
        match (closed, released) {
            (Err(close), Err(release)) => Err(format!("{close}; release failed: {release}")),
            (Err(error), _) | (_, Err(error)) => Err(error),
            _ => Ok(()),
        }
    })();
    process::finish_cleanup();
    if let Some(state) = reliability {
        state.round()["cleanup"] = json!({"game_closed":game_closed,"lock_released":lock_released,
            "status":if cleanup.is_ok() { "success" } else { "failed" },"error":cleanup.as_ref().err(),
            "elapsed_seconds":(r.monotonic() - cleanup_started).max(0.0)});
    }
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => Err(format!("{error}; cleanup failed: {cleanup}")),
        (Ok(code), Err(cleanup)) if code != 0 => Err(format!(
            "chain failed (exit {}); cleanup failed: {cleanup}",
            windows_exit_code(code)
        )),
        (_, Err(cleanup)) => Err(cleanup),
        (result, Ok(())) => result,
    }
}
/// Python represents Windows process status as an unsigned DWORD.
pub fn windows_exit_code(code: i32) -> u32 {
    code as u32
}
/// Python sys.exit on Windows converts a unsigned child DWORD through signed C long.
/// Out-of-range values exit as -1; RESULT retains the unsigned child status.
pub fn python_cli_exit(code: i32) -> i32 {
    if code < 0 {
        -1
    } else {
        code
    }
}

#[cfg(test)]
mod queue_entry_tests {
    use super::*;

    #[test]
    fn same_pid_registrations_have_distinct_paths_and_old_cleanup_keeps_new_entry() {
        let root = std::env::temp_dir().join(format!("cc-live-queue-{}", new_lock_token()));
        fs::create_dir_all(&root).unwrap();
        let old = root.join(queue_entry_name());
        let new = root.join(queue_entry_name());
        assert_ne!(old, new);
        fs::write(&old, "old").unwrap();
        fs::write(&new, "new").unwrap();
        fs::remove_file(&old).unwrap();
        assert_eq!(fs::read_to_string(&new).unwrap(), "new");
        fs::remove_file(new).unwrap();
        fs::remove_dir(root).unwrap();
    }
}

#[cfg(all(test, unix))]
mod unix_guard_tests {
    use super::*;
    use std::{
        fs::TryLockError,
        sync::mpsc,
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    struct Fixture {
        root: std::path::PathBuf,
        paths: Paths,
    }
    impl Fixture {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root =
                std::env::temp_dir().join(format!("cc-live-unix-{}-{unique}", std::process::id()));
            let here = root.join("here");
            fs::create_dir_all(&here).unwrap();
            let paths = Paths {
                here,
                wt: root.join("wt"),
                jobs: root.join("jobs"),
                temp: root.join("temp"),
                profile: root.join("profile"),
                python: "unused-test-value".into(),
            };
            Self { root, paths }
        }
        fn lock(&self) -> std::path::PathBuf {
            self.paths.here.join("civ6.lock")
        }
        fn guard(&self) -> std::path::PathBuf {
            self.paths.here.join("civ6.lock.guard")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    struct RunnerWithWriter {
        lock: std::path::PathBuf,
        guard: std::path::PathBuf,
        writer: Option<thread::JoinHandle<()>>,
        kills: usize,
    }
    impl Runner for RunnerWithWriter {
        fn run(&mut self, call: Call) -> Result<Output, String> {
            if call.args[0] == "taskkill" {
                self.kills += 1;
                if self.writer.is_none() {
                    let probe = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .open(&self.guard)
                        .unwrap();
                    assert!(matches!(probe.try_lock(), Err(TryLockError::WouldBlock)));
                    let lock = self.lock.clone();
                    let guard = self.guard.clone();
                    let (attempted, received) = mpsc::channel();
                    self.writer = Some(thread::spawn(move || {
                        let probe = OpenOptions::new()
                            .read(true)
                            .write(true)
                            .open(guard)
                            .unwrap();
                        attempted
                            .send(matches!(probe.try_lock(), Err(TryLockError::WouldBlock)))
                            .unwrap();
                        let _guard = fairness::UnixGuard::acquire(&lock).unwrap();
                        write_json(&lock, &json!({"live_chain":true,"lock_token":"peer"})).unwrap();
                    }));
                    assert!(received.recv_timeout(Duration::from_secs(2)).unwrap());
                }
            }
            Ok(Output::default())
        }
        fn now(&mut self) -> f64 {
            1000.0
        }
        fn sleep(&mut self, _: f64) -> Result<(), String> {
            panic!("unexpected wait")
        }
        fn print(&mut self, _: &str) -> Result<(), String> {
            Ok(())
        }
    }

    #[test]
    fn cleanup_holds_guard_until_own_game_is_closed_and_lock_unlinked() {
        let f = Fixture::new();
        let mut r = RunnerWithWriter {
            lock: f.lock(),
            guard: f.guard(),
            writer: None,
            kills: 0,
        };
        assert_eq!(
            cli(
                &mut r,
                &f.paths,
                &args(&["fixture:s.json", "--stop-on-failure"])
            )
            .unwrap(),
            1
        );
        r.writer.take().unwrap().join().unwrap();
        assert_eq!(r.kills, 2);
        assert_eq!(read_json(f.lock()).unwrap()["lock_token"], "peer");
        assert!(f.guard().exists());
    }

    #[test]
    fn acquisition_and_fairness_updates_use_the_same_persistent_guard() {
        struct Inspecting {
            guard: std::path::PathBuf,
        }
        impl Runner for Inspecting {
            fn run(&mut self, _: Call) -> Result<Output, String> {
                let probe = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&self.guard)
                    .unwrap();
                assert!(matches!(probe.try_lock(), Err(TryLockError::WouldBlock)));
                Ok(Output::default())
            }
            fn now(&mut self) -> f64 {
                1000.0
            }
            fn sleep(&mut self, _: f64) -> Result<(), String> {
                panic!("unexpected wait")
            }
            fn print(&mut self, _: &str) -> Result<(), String> {
                Ok(())
            }
        }
        let f = Fixture::new();
        write_json(f.lock(), &json!({"pid":42})).unwrap();
        acquire(
            &mut Inspecting { guard: f.guard() },
            &f.paths,
            &json!({"pid":77}),
            1.0,
            false,
        )
        .unwrap();
        assert_eq!(read_json(f.lock()).unwrap(), json!({"pid":77}));
        let mut lock = fairness::Lock::open(&f.paths).unwrap().unwrap();
        let probe = OpenOptions::new()
            .read(true)
            .write(true)
            .open(f.guard())
            .unwrap();
        assert!(matches!(probe.try_lock(), Err(TryLockError::WouldBlock)));
        lock.info["lease_probe"] = json!(true);
        lock.save().unwrap();
        drop(lock);
        probe.try_lock().unwrap();
        assert_eq!(read_json(f.lock()).unwrap()["lease_probe"], true);
    }

    #[test]
    fn queue_registration_waits_for_the_guard_before_publishing_priority() {
        struct QueueRunner {
            clock_reads: usize,
            ready: mpsc::Sender<()>,
            proceed: mpsc::Receiver<()>,
        }
        impl Runner for QueueRunner {
            fn run(&mut self, _: Call) -> Result<Output, String> {
                panic!("no process query is needed for an empty queue")
            }
            fn now(&mut self) -> f64 {
                self.clock_reads += 1;
                if self.clock_reads == 2 {
                    self.ready.send(()).unwrap();
                    self.proceed.recv().unwrap();
                }
                1000.0
            }
            fn sleep(&mut self, _: f64) -> Result<(), String> {
                panic!("an empty queue must not sleep")
            }
            fn print(&mut self, _: &str) -> Result<(), String> {
                Ok(())
            }
        }

        let f = Fixture::new();
        let guard = fairness::UnixGuard::acquire(&f.lock()).unwrap();
        let (ready_tx, ready_rx) = mpsc::channel();
        let (proceed_tx, proceed_rx) = mpsc::channel();
        let paths = f.paths.clone();
        let worker = thread::spawn(move || {
            acquire(
                &mut QueueRunner {
                    clock_reads: 0,
                    ready: ready_tx,
                    proceed: proceed_rx,
                },
                &paths,
                &json!({"pid":77}),
                1.0,
                false,
            )
        });
        ready_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        proceed_tx.send(()).unwrap();
        let queue = f.paths.here.join("queue");
        thread::sleep(Duration::from_millis(100));
        assert_eq!(
            fs::read_dir(&queue).unwrap().count(),
            0,
            "priority was published outside the guard"
        );
        drop(guard);
        worker.join().unwrap().unwrap();
        assert_eq!(read_json(f.lock()).unwrap(), json!({"pid":77}));
        assert!(
            fs::read_dir(&queue).unwrap().next().is_none(),
            "successful claim must remove its queue entry"
        );
        assert!(f.guard().exists(), "guard inode must remain persistent");
    }
}
