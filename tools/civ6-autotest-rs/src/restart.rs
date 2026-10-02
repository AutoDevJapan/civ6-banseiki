//! Process lifecycle and MOD freshness checks for the Civ VI driver.
use crate::{time::monotonic, Error, Result};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};

pub const STEAM_URI: &str = "steam://launch/289070/option2";
const REPORTER_SUFFIX: &str =
    "\\sid meier's civilization vi\\base\\binaries\\win64steam\\firaxisbugreporter.exe";

pub fn reporter_finished(wait_result: u32) -> bool {
    wait_result == 0
}

pub fn default_mods_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join("Documents")
        .join("My Games")
        .join("Sid Meier's Civilization VI")
        .join("Mods")
}

fn extension_is(path: &Path, extension: &str) -> bool {
    let Some(actual) = path.extension().and_then(|s| s.to_str()) else {
        return false;
    };
    if cfg!(windows) {
        actual.eq_ignore_ascii_case(extension)
    } else {
        actual == extension
    }
}

pub fn enabled_mod_folders(scenario: &Value, mods_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut ids = BTreeSet::new();
    let steps = scenario
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| Error::input("Scenario steps must be an array"))?;
    for step in steps {
        if step.get("action").and_then(Value::as_str) == Some("enable_mods") {
            let listed = step
                .get("ids")
                .and_then(Value::as_array)
                .ok_or_else(|| Error::input("enable_mods ids must be an array"))?;
            for id in listed {
                ids.insert(
                    id.as_str()
                        .ok_or_else(|| Error::input("Mod ID must be text"))?
                        .to_lowercase(),
                );
            }
        }
    }
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut found = BTreeMap::new();
    for entry in fs::read_dir(mods_dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        let folder = entry.path().canonicalize()?;
        if !folder.is_dir() {
            continue;
        }
        for info in fs::read_dir(&folder)? {
            let info = info?;
            let path = info.path();
            if !extension_is(&path, "modinfo") {
                continue;
            }
            let xml = fs::read_to_string(&path)?;
            let Ok(doc) = roxmltree::Document::parse(&xml) else {
                continue;
            };
            let id = doc
                .root_element()
                .attribute("id")
                .unwrap_or("")
                .to_lowercase();
            if ids.contains(&id) {
                if let Some(previous) = found.insert(id.clone(), folder.clone()) {
                    return Err(Error::input(format!(
                        "Duplicate installed mod ID {id}: {} and {}",
                        previous.display(),
                        folder.display()
                    )));
                }
            }
        }
    }
    let missing: Vec<_> = ids
        .difference(&found.keys().cloned().collect())
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(Error::input(format!(
            "Scenario mod is not installed under {}: {}",
            mods_dir.display(),
            missing.join(", ")
        )));
    }
    Ok(ids.iter().map(|id| found[id].clone()).collect())
}

fn walk_files(folder: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut dirs = vec![folder.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            if ty.is_dir() {
                dirs.push(entry.path());
            } else if ty.is_file() || (ty.is_symlink() && entry.path().is_file()) {
                out.push(entry.path());
            }
        }
    }
    out.sort();
    Ok(out)
}

pub fn newest_mod_file(folders: &[PathBuf]) -> Result<Option<(PathBuf, f64)>> {
    let mut newest: Option<(PathBuf, f64)> = None;
    for folder in folders {
        for path in walk_files(folder)? {
            let modified = fs::metadata(&path)?
                .modified()?
                .duration_since(UNIX_EPOCH)
                .map_err(|e| Error::input(e.to_string()))?
                .as_secs_f64();
            if newest.as_ref().is_none_or(|(_, old)| modified > *old) {
                newest = Some((path, modified));
            }
        }
    }
    if !folders.is_empty() && newest.is_none() {
        return Err(Error::input("Enabled mod folders contain no files"));
    }
    Ok(newest)
}

fn contains_bytes(path: &Path, needle: &[u8]) -> Result<bool> {
    let mut file = fs::File::open(path)?;
    let mut carry = Vec::new();
    let mut block = [0u8; 65536];
    loop {
        let count = file.read(&mut block)?;
        if count == 0 {
            return Ok(false);
        }
        carry.extend_from_slice(&block[..count]);
        if carry.windows(needle.len()).any(|part| part == needle) {
            return Ok(true);
        }
        let keep = needle.len().saturating_sub(1).min(carry.len());
        carry.drain(..carry.len() - keep);
    }
}

pub fn missing_atlas_textures(folders: &[PathBuf]) -> Result<Vec<String>> {
    let mut missing = Vec::new();
    for folder in folders {
        let files = walk_files(folder)?;
        let mut names = BTreeSet::new();
        for path in files.iter().filter(|p| extension_is(p, "xml")) {
            let xml = fs::read_to_string(path)?;
            let Ok(doc) = roxmltree::Document::parse(&xml) else {
                continue;
            };
            for atlases in doc
                .root_element()
                .children()
                .filter(|n| n.has_tag_name("IconTextureAtlases"))
            {
                for row in atlases.children().filter(|n| n.has_tag_name("Row")) {
                    if let Some(filename) = row.attribute("Filename") {
                        let stem = Path::new(filename)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("");
                        if !stem.is_empty() {
                            names.insert(stem.to_string());
                        }
                    }
                }
            }
        }
        for path in files.iter().filter(|p| extension_is(p, "dds")) {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                names.remove(stem);
            }
        }
        for package in files.iter().filter(|p| extension_is(p, "blp")) {
            if names.is_empty() {
                break;
            }
            if fs::metadata(package)?.len() == 0 {
                continue;
            }
            let candidates: Vec<_> = names.iter().cloned().collect();
            for name in candidates {
                if contains_bytes(package, name.as_bytes())? {
                    names.remove(&name);
                }
            }
        }
        let label = folder.file_name().and_then(|s| s.to_str()).unwrap_or("");
        missing.extend(names.into_iter().map(|name| format!("{label}: {name}")));
    }
    Ok(missing)
}

pub fn restart_reason(
    newest: Option<&(PathBuf, f64)>,
    process_start: Option<f64>,
) -> Option<String> {
    let Some(start) = process_start else {
        return Some("Civ6 is not running".to_string());
    };
    newest
        .filter(|(_, modified)| *modified > start)
        .map(|(path, modified)| {
            format!(
                "mod file newer than Civ6 process: {} (mtime={modified:.3}, started={start:.3})",
                path.display()
            )
        })
}

/// Injectable process operations keep all lifecycle decisions testable without Civ VI.
pub trait LifecycleBackend {
    fn civ_ids(&mut self) -> Result<BTreeSet<u32>>;
    fn process_start(&mut self, pid: u32) -> Result<f64>;
    fn terminate_civ(&mut self, pid: u32, expected_start: f64) -> Result<()>;
    fn reporter_ids(&mut self) -> Result<BTreeSet<u32>>;
    fn reporter_path(&mut self, pid: u32) -> Result<Option<String>>;
    fn terminate_reporter(&mut self, pid: u32) -> Result<()>;
    fn launch_uri(&mut self, uri: &str) -> Result<()>;
    fn find_window(&mut self) -> Result<isize>;
    fn process_name(&mut self, hwnd: isize) -> Result<String>;
    fn now(&mut self) -> f64 {
        monotonic()
    }
    fn sleep(&mut self, seconds: f64) {
        std::thread::sleep(Duration::from_secs_f64(seconds));
    }
}

pub fn single_civ_process_with<B: LifecycleBackend>(
    backend: &mut B,
    settle: f64,
) -> Result<Option<(u32, f64)>> {
    let mut ids = backend.civ_ids()?;
    let end = backend.now() + settle.max(0.0);
    while ids.len() > 1 && backend.now() < end {
        let wait = (end - backend.now()).max(0.0).min(0.5);
        backend.sleep(wait);
        ids = backend.civ_ids()?;
    }
    if ids.len() > 1 {
        return Err(Error::input(format!(
            "Multiple Civ VI processes found ({}); driver requires one local game",
            ids.len()
        )));
    }
    let Some(&pid) = ids.iter().next() else {
        return Ok(None);
    };
    match backend.process_start(pid) {
        Ok(start) => Ok(Some((pid, start))),
        Err(error) => {
            if !backend.civ_ids()?.contains(&pid) {
                Ok(None)
            } else {
                Err(error)
            }
        }
    }
}

pub fn wait_for_exit_with<B: LifecycleBackend>(
    backend: &mut B,
    pid: u32,
    end: f64,
) -> Result<bool> {
    while backend.now() < end {
        if !backend.civ_ids()?.contains(&pid) {
            return Ok(true);
        }
        let remaining = (end - backend.now()).max(0.0).min(0.5);
        backend.sleep(remaining);
    }
    Ok(!backend.civ_ids()?.contains(&pid))
}

pub fn terminate_civ_process_with<B: LifecycleBackend>(
    backend: &mut B,
    pid: u32,
    expected_start: f64,
) -> Result<()> {
    let ids = backend.civ_ids()?;
    if ids.len() > 1 {
        return Err(Error::input(format!(
            "Multiple Civ VI processes found ({}); refusing termination",
            ids.len()
        )));
    }
    if !ids.contains(&pid) {
        return Ok(());
    }
    if backend.process_start(pid)? != expected_start {
        return Err(Error::input(format!(
            "Civ VI process {pid} changed before termination"
        )));
    }
    backend.terminate_civ(pid, expected_start)
}

pub fn close_orphan_civ_reporters_with<B: LifecycleBackend>(backend: &mut B) -> Result<()> {
    if !backend.civ_ids()?.is_empty() {
        return Ok(());
    }
    for pid in backend.reporter_ids()? {
        let Some(path) = backend.reporter_path(pid)? else {
            if !backend.reporter_ids()?.contains(&pid) {
                continue;
            }
            return Err(Error::input(format!(
                "Cannot open remaining crash reporter {pid}"
            )));
        };
        if !path.to_lowercase().ends_with(REPORTER_SUFFIX) {
            continue;
        }
        if !backend.civ_ids()?.is_empty() {
            return Ok(());
        }
        backend.terminate_reporter(pid)?;
    }
    Ok(())
}

pub fn launch_via_steam_with<B: LifecycleBackend>(backend: &mut B) -> Result<()> {
    close_orphan_civ_reporters_with(backend)?;
    backend.launch_uri(STEAM_URI)
}

pub fn require_dx11_window_with<B: LifecycleBackend>(backend: &mut B, hwnd: isize) -> Result<()> {
    let name = backend.process_name(hwnd)?;
    if name != "civilizationvi.exe" {
        return Err(Error::input(format!(
            "Steam DX11 launch opened {name}; expected CivilizationVI.exe"
        )));
    }
    Ok(())
}

pub fn wait_for_civ_window_with<B: LifecycleBackend>(backend: &mut B, end: f64) -> Result<isize> {
    let mut last = "no Civ VI process yet".to_string();
    while backend.now() < end {
        let process = match single_civ_process_with(backend, 0.0) {
            Ok(p) => p,
            Err(e) if e.message.starts_with("Multiple Civ VI processes found") => {
                last = e.message;
                None
            }
            Err(e) => return Err(e),
        };
        if process.is_some() {
            match backend.find_window() {
                Ok(hwnd) => return Ok(hwnd),
                Err(e) => last = e.message,
            }
        }
        let remaining = (end - backend.now()).max(0.0).min(0.5);
        backend.sleep(remaining);
    }
    Err(Error::input(format!(
        "Civ VI window did not appear after Steam launch: {last}"
    )))
}

#[cfg(windows)]
mod win {
    use super::*;
    use crate::click::{self, image_path, with_process};
    use std::ptr;
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetProcessTimes(
            h: isize,
            created: *mut FileTime,
            exited: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
        fn TerminateProcess(h: isize, code: u32) -> i32;
        fn WaitForSingleObject(h: isize, millis: u32) -> u32;
    }
    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            hwnd: isize,
            verb: *const u16,
            file: *const u16,
            params: *const u16,
            dir: *const u16,
            show: i32,
        ) -> isize;
    }
    fn start_on_handle(h: isize, pid: u32) -> Result<f64> {
        let (mut a, mut b, mut c, mut d) = (
            FileTime::default(),
            FileTime::default(),
            FileTime::default(),
            FileTime::default(),
        );
        if unsafe { GetProcessTimes(h, &mut a, &mut b, &mut c, &mut d) } == 0 {
            return Err(Error::input(format!(
                "Cannot read Civ VI process start time for {pid}"
            )));
        }
        Ok((((a.high as u64) << 32 | a.low as u64) as f64) / 10_000_000.0 - 11_644_473_600.0)
    }
    pub struct WindowsLifecycle;
    impl LifecycleBackend for WindowsLifecycle {
        fn civ_ids(&mut self) -> Result<BTreeSet<u32>> {
            click::civ_process_ids()
        }
        fn process_start(&mut self, pid: u32) -> Result<f64> {
            with_process(pid, 0x1000, |h| start_on_handle(h, pid)).map_err(|e| {
                if e.message.starts_with("Cannot open") {
                    Error::input(format!("Cannot inspect Civ VI process {pid}"))
                } else {
                    e
                }
            })
        }
        fn terminate_civ(&mut self, pid: u32, expected: f64) -> Result<()> {
            with_process(pid, 0x1001, |h| {
                let path = image_path(h)?;
                let name = path
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or(&path)
                    .to_lowercase();
                if !click::CIV_EXECUTABLES.contains(&name.as_str())
                    || start_on_handle(h, pid)? != expected
                {
                    return Err(Error::input(format!(
                        "Civ VI process {pid} changed before termination"
                    )));
                }
                if unsafe { TerminateProcess(h, 1) } == 0 {
                    return Err(Error::input(format!(
                        "Could not terminate Civ VI process {pid}"
                    )));
                }
                Ok(())
            })
        }
        fn reporter_ids(&mut self) -> Result<BTreeSet<u32>> {
            click::process_ids(&BTreeSet::from(["firaxisbugreporter.exe".to_string()]))
        }
        fn reporter_path(&mut self, pid: u32) -> Result<Option<String>> {
            let Some(h) = crate::click::open_process(pid, 0x101001) else {
                return Ok(None);
            };
            let path = image_path(h)
                .map_err(|_| Error::input(format!("Cannot inspect crash reporter {pid}")));
            unsafe {
                CloseHandle(h);
            }
            path.map(Some)
        }
        fn terminate_reporter(&mut self, pid: u32) -> Result<()> {
            with_process(pid, 0x101001, |h| {
                let path = image_path(h)
                    .map_err(|_| Error::input(format!("Cannot inspect crash reporter {pid}")))?;
                if !path.to_lowercase().ends_with(REPORTER_SUFFIX) {
                    return Err(Error::input(format!(
                        "Crash reporter {pid} changed before termination"
                    )));
                }
                if !click::civ_process_ids()?.is_empty() {
                    return Ok(());
                }
                if unsafe { TerminateProcess(h, 1) } == 0 {
                    return Err(Error::input(format!(
                        "Could not close orphan Civ VI crash reporter {pid}"
                    )));
                }
                if !reporter_finished(unsafe { WaitForSingleObject(h, 10_000) }) {
                    return Err(Error::input(format!(
                        "Orphan Civ VI crash reporter {pid} did not finish exiting"
                    )));
                }
                Ok(())
            })
        }
        fn launch_uri(&mut self, uri: &str) -> Result<()> {
            let wide: Vec<u16> = uri.encode_utf16().chain(std::iter::once(0)).collect();
            if unsafe { ShellExecuteW(0, ptr::null(), wide.as_ptr(), ptr::null(), ptr::null(), 1) }
                <= 32
            {
                return Err(Error::input("Could not launch Civ VI through Steam"));
            }
            Ok(())
        }
        fn find_window(&mut self) -> Result<isize> {
            click::find_civ_window()
        }
        fn process_name(&mut self, hwnd: isize) -> Result<String> {
            click::process_name(hwnd)
        }
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn CloseHandle(h: isize) -> i32;
    }
    pub fn single_civ_process(settle: f64) -> Result<Option<(u32, f64)>> {
        single_civ_process_with(&mut WindowsLifecycle, settle)
    }
    pub fn wait_for_exit(pid: u32, end: f64) -> Result<bool> {
        wait_for_exit_with(&mut WindowsLifecycle, pid, end)
    }
    pub fn terminate_civ_process(pid: u32, start: f64) -> Result<()> {
        terminate_civ_process_with(&mut WindowsLifecycle, pid, start)
    }
    pub fn close_orphan_civ_reporters() -> Result<()> {
        close_orphan_civ_reporters_with(&mut WindowsLifecycle)
    }
    pub fn launch_via_steam() -> Result<()> {
        launch_via_steam_with(&mut WindowsLifecycle)
    }
    pub fn require_dx11_window(hwnd: isize) -> Result<()> {
        require_dx11_window_with(&mut WindowsLifecycle, hwnd)
    }
    pub fn wait_for_civ_window(end: f64) -> Result<isize> {
        wait_for_civ_window_with(&mut WindowsLifecycle, end)
    }
}
#[cfg(windows)]
pub use win::{
    close_orphan_civ_reporters, launch_via_steam, require_dx11_window, single_civ_process,
    terminate_civ_process, wait_for_civ_window, wait_for_exit,
};
#[cfg(not(windows))]
fn unsupported<T>() -> Result<T> {
    Err(Error::input("Civ VI process control requires Windows"))
}
#[cfg(not(windows))]
pub fn single_civ_process(_: f64) -> Result<Option<(u32, f64)>> {
    unsupported()
}
#[cfg(not(windows))]
pub fn wait_for_exit(_: u32, _: f64) -> Result<bool> {
    unsupported()
}
#[cfg(not(windows))]
pub fn terminate_civ_process(_: u32, _: f64) -> Result<()> {
    unsupported()
}
#[cfg(not(windows))]
pub fn close_orphan_civ_reporters() -> Result<()> {
    unsupported()
}
#[cfg(not(windows))]
pub fn launch_via_steam() -> Result<()> {
    unsupported()
}
#[cfg(not(windows))]
pub fn require_dx11_window(_: isize) -> Result<()> {
    unsupported()
}
#[cfg(not(windows))]
pub fn wait_for_civ_window(_: f64) -> Result<isize> {
    unsupported()
}
