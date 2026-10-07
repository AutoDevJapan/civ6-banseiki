//! Steam warning recovery. Actions require the shared chain lease and a fresh
//! snapshot of the same registered Steam process, window and Invoke element.
use crate::{Error, Result};
use serde_json::Value;
use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Session {
    pub root: PathBuf,
    pub console_offset: u64,
    pub since: chrono::DateTime<chrono::Local>,
    pub owner_pid: u32,
    pub owner_started: u64,
    pub owner_floor: chrono::NaiveDateTime,
}
impl Session {
    pub fn begin() -> Result<Self> {
        let root = registered_root()?;
        let offset = match std::fs::metadata(root.join("logs/console_log.txt")) {
            Ok(m) => m.len(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
            Err(e) => return Err(e.into()),
        };
        let (owner_pid, owner_started, owner_floor) = owner_identity(&root)?;
        Ok(Self {
            root,
            console_offset: offset,
            since: chrono::Local::now(),
            owner_pid,
            owner_started,
            owner_floor,
        })
    }
    pub fn console_proves_civ6(&self) -> Result<bool> {
        let mut f = std::fs::File::open(self.root.join("logs/console_log.txt"))?;
        let len = f.metadata()?.len();
        if len > 4 * 1024 * 1024 {
            return Ok(false);
        }
        f.seek(SeekFrom::Start(0))?;
        let mut text = String::new();
        f.take(len).read_to_string(&mut text)?;
        // Existing modals keep an old timestamp while Steam waits. Re-read the
        // current action state, tied to this Steam process's creation time.
        Ok(current_console_proves(
            &text,
            self.owner_floor,
            chrono::Local::now().naive_local(),
        ))
    }
}

pub fn current_console_proves(
    text: &str,
    floor: chrono::NaiveDateTime,
    now: chrono::NaiveDateTime,
) -> bool {
    let re = regex::Regex::new(
        r"^\[([^\]]+)\] GameAction \[AppID (\d+), ActionID (\d+)\]\s*:\s*LaunchApp (.*)$",
    )
    .unwrap();
    let mut last = None;
    let mut pending = std::collections::BTreeMap::new();
    for line in text.lines() {
        let Some(c) = re.captures(line) else { continue };
        let Ok(when) = chrono::NaiveDateTime::parse_from_str(&c[1], "%Y-%m-%d %H:%M:%S") else {
            return false;
        };
        if when < floor || when > now {
            continue;
        }
        let key = (
            c[2].parse::<u32>().unwrap_or(0),
            c[3].parse::<u64>().unwrap_or(0),
        );
        let waiting = c[4].contains("waiting for user response to")
            && (c[4].contains("SynchronizingControllerConfig")
                || c[4].contains("SynchronizingCloud"));
        let civ6_failure = waiting && key.0 == 289070 && c[4].contains("syncfailed");
        pending.insert(key, waiting);
        last = Some(civ6_failure);
    }
    last == Some(true) && pending.values().filter(|v| **v).count() == 1
}

pub fn console_proves(
    text: &str,
    since: chrono::NaiveDateTime,
    now: chrono::NaiveDateTime,
) -> bool {
    let re = regex::Regex::new(
        r"^\[([^\]]+)\] GameAction \[AppID (\d+), ActionID (\d+)\]\s*:\s*LaunchApp (.*)$",
    )
    .unwrap();
    let mut actions = std::collections::BTreeMap::new();
    for line in text.lines() {
        let Some(c) = re.captures(line) else { continue };
        let Ok(when) = chrono::NaiveDateTime::parse_from_str(&c[1], "%Y-%m-%d %H:%M:%S") else {
            continue;
        };
        if when < since - chrono::Duration::seconds(1)
            || when > now
            || now - when > chrono::Duration::seconds(120)
        {
            continue;
        }
        // Keep other games too: a generic warning is bound only if Civ6 is
        // the unique action awaiting a sync response.
        let app = c[2].parse::<u32>().unwrap_or(0);
        let id = c[3].parse::<u64>().unwrap_or(0);
        let pending = c[4].contains("waiting for user response to SynchronizingControllerConfig")
            && c[4].contains("syncfailed");
        actions.insert((app, id), pending);
    }
    let pending: Vec<_> = actions
        .iter()
        .filter(|(_, v)| **v)
        .map(|(k, _)| *k)
        .collect();
    pending.len() == 1 && pending[0].0 == 289070
}

#[cfg(windows)]
fn record(path: &Path, value: Value) -> Result<()> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    serde_json::to_writer(&mut f, &value)?;
    f.write_all(b"\n")?;
    f.sync_data()?;
    Ok(())
}

#[cfg(windows)]
mod win {
    use super::*;
    use windows::{
        core::{w, Interface},
        Win32::{
            Foundation::HWND,
            System::{
                Com::{
                    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
                    COINIT_MULTITHREADED,
                },
                Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ},
            },
            UI::Accessibility::{
                AccessibleObjectFromWindow, CUIAutomation8, IAccessible, IUIAutomation,
                IUIAutomation2, IUIAutomationElement, IUIAutomationInvokePattern,
                TreeScope_Descendants, UIA_ButtonControlTypeId, UIA_InvokePatternId,
            },
        },
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            Threading::{
                GetProcessTimes, OpenProcess, WaitForSingleObject,
                PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
            },
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
        },
    };
    fn native(e: windows::core::Error) -> Error {
        Error::input(format!("Steam accessibility: {e}"))
    }
    struct Com;
    impl Com {
        fn new() -> Result<Self> {
            unsafe {
                CoInitializeEx(None, COINIT_MULTITHREADED)
                    .ok()
                    .map_err(native)?;
            }
            Ok(Self)
        }
    }
    impl Drop for Com {
        fn drop(&mut self) {
            unsafe { CoUninitialize() }
        }
    }
    struct Pin {
        handle: windows_sys::Win32::Foundation::HANDLE,
        pid: u32,
        started: u64,
        image: String,
    }
    impl Drop for Pin {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.handle);
            }
        }
    }
    fn started(handle: windows_sys::Win32::Foundation::HANDLE) -> Result<u64> {
        let mut c = FILETIME::default();
        let mut e = c;
        let mut k = c;
        let mut u = c;
        if unsafe { GetProcessTimes(handle, &mut c, &mut e, &mut k, &mut u) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok((u64::from(c.dwHighDateTime) << 32) | u64::from(c.dwLowDateTime))
    }
    impl Pin {
        fn open(pid: u32) -> Result<Self> {
            let handle = unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                    0,
                    pid,
                )
            };
            if handle.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            let mut p = Self {
                handle,
                pid,
                started: 0,
                image: String::new(),
            };
            p.started = started(handle)?;
            p.image = crate::click::image_path(handle as isize)?;
            Ok(p)
        }
        fn check(&self) -> Result<()> {
            if unsafe { WaitForSingleObject(self.handle, 0) } != 258
                || started(self.handle)? != self.started
            {
                return Err(Error::input("Steam process changed before action"));
            }
            Ok(())
        }
    }
    fn location(p: impl AsRef<Path>) -> String {
        p.as_ref()
            .to_string_lossy()
            .replace('/', "\\")
            .trim_start_matches("\\\\?\\")
            .to_lowercase()
    }
    pub fn root() -> Result<PathBuf> {
        let mut buf = vec![0u16; 32768];
        let mut bytes = (buf.len() * 2) as u32;
        let result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Software\\Valve\\Steam"),
                w!("SteamExe"),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut bytes),
            )
        };
        if result.0 != 0 {
            return Err(Error::input("registered Steam installation is unavailable"));
        }
        let len = buf.iter().position(|v| *v == 0).unwrap_or(buf.len());
        let exe = PathBuf::from(String::from_utf16_lossy(&buf[..len]).replace('/', "\\"));
        if exe
            .file_name()
            .is_none_or(|n| !n.to_string_lossy().eq_ignore_ascii_case("steam.exe"))
        {
            return Err(Error::input("registered SteamExe is not steam.exe"));
        }
        Ok(exe
            .parent()
            .ok_or_else(|| Error::input("SteamExe has no directory"))?
            .to_owned())
    }
    fn rows() -> Result<Vec<(u32, u32, String)>> {
        let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snap == (-1isize as _) {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut row = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut out = Vec::new();
        let mut ok = unsafe { Process32FirstW(snap, &mut row) };
        while ok != 0 {
            let len = row
                .szExeFile
                .iter()
                .position(|n| *n == 0)
                .unwrap_or(row.szExeFile.len());
            out.push((
                row.th32ProcessID,
                row.th32ParentProcessID,
                String::from_utf16_lossy(&row.szExeFile[..len]),
            ));
            ok = unsafe { Process32NextW(snap, &mut row) }
        }
        let err = std::io::Error::last_os_error();
        unsafe {
            CloseHandle(snap);
        }
        if err.raw_os_error() != Some(18) {
            return Err(err.into());
        }
        Ok(out)
    }
    fn owner(root: &Path, rows: &[(u32, u32, String)]) -> Result<Pin> {
        let mut owners = Vec::new();
        for (pid, _, name) in rows {
            if name.eq_ignore_ascii_case("steam.exe") {
                let p = Pin::open(*pid)?;
                if location(&p.image) == location(root.join("steam.exe")) {
                    owners.push(p)
                }
            }
        }
        if owners.len() != 1 {
            return Err(Error::input(
                "a unique registered Steam process is required",
            ));
        }
        Ok(owners.remove(0))
    }
    pub fn identity(root: &Path) -> Result<(u32, u64, chrono::NaiveDateTime)> {
        let p = owner(root, &rows()?)?;
        let seconds = (p.started / 10_000_000)
            .checked_sub(11_644_473_600)
            .ok_or_else(|| Error::input("invalid Steam process time"))?;
        let floor = chrono::DateTime::<chrono::Utc>::from_timestamp(seconds as i64, 0)
            .ok_or_else(|| Error::input("invalid Steam creation time"))?
            .with_timezone(&chrono::Local)
            .naive_local();
        Ok((p.pid, p.started, floor))
    }
    fn trusted(pid: u32, owner: &Pin, root: &Path, rows: &[(u32, u32, String)]) -> bool {
        if pid == owner.pid {
            return owner.check().is_ok();
        }
        let Ok(p) = Pin::open(pid) else { return false };
        let image = location(&p.image);
        let prefix = location(root) + "\\";
        if !image.starts_with(&prefix)
            || !image.ends_with("\\steamwebhelper.exe")
            || p.started < owner.started
        {
            return false;
        }
        let mut current = pid;
        for _ in 0..32 {
            let Some((_, parent, _)) = rows.iter().find(|r| r.0 == current) else {
                return false;
            };
            if *parent == owner.pid {
                return true;
            }
            if *parent == 0 || *parent == current {
                return false;
            }
            current = *parent
        }
        false
    }
    unsafe extern "system" fn enumerate(
        hwnd: windows_sys::Win32::Foundation::HWND,
        param: isize,
    ) -> i32 {
        let list = unsafe { &mut *(param as *mut Vec<(isize, u32, String)>) };
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return 1;
        }
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(hwnd, &mut pid);
        }
        let mut title = [0u16; 1024];
        let len = unsafe { GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32) };
        if len > 0 {
            list.push((
                hwnd as isize,
                pid,
                String::from_utf16_lossy(&title[..len as usize]),
            ))
        }
        1
    }
    struct Snapshot {
        hwnd: isize,
        pid: u32,
        title: String,
        body: String,
        buttons: Vec<(String, IUIAutomationElement)>,
        pin: Pin,
    }
    impl Snapshot {
        fn value(&self) -> Value {
            serde_json::json!({"hwnd":self.hwnd,"pid":self.pid,"process_started":self.pin.started,"title":self.title,"body":self.body,"buttons":self.buttons.iter().map(|b|&b.0).collect::<Vec<_>>()})
        }
    }
    fn read(
        uia: &IUIAutomation,
        hwnd: isize,
        pid: u32,
        title: String,
        owner: &Pin,
        root: &Path,
        rows: &[(u32, u32, String)],
    ) -> Result<Snapshot> {
        unsafe {
            let mut current_pid = 0;
            GetWindowThreadProcessId(hwnd as _, &mut current_pid);
            let mut current_title = [0u16; 1024];
            let count = GetWindowTextW(
                hwnd as _,
                current_title.as_mut_ptr(),
                current_title.len() as i32,
            );
            if current_pid != pid
                || count <= 0
                || IsWindowVisible(hwnd as _) == 0
                || String::from_utf16_lossy(&current_title[..count as usize]) != title
            {
                return Err(Error::input(
                    "Steam window title or owner changed during scan",
                ));
            }
            let element = uia.ElementFromHandle(HWND(hwnd as _)).map_err(native)?;
            let cond = uia.CreateTrueCondition().map_err(native)?;
            let mut all = element
                .FindAll(TreeScope_Descendants, &cond)
                .map_err(native)?;
            // Requesting the client accessibility object activates Chromium's
            // provider without changing Steam flags or preferences.
            if all.Length().map_err(native)? < 6 {
                let mut ptr = std::ptr::null_mut();
                let result = AccessibleObjectFromWindow(
                    HWND(hwnd as _),
                    0xffff_fffc,
                    &IAccessible::IID,
                    &mut ptr,
                );
                if result.is_ok() && !ptr.is_null() {
                    drop(IAccessible::from_raw(ptr));
                }
                all = element
                    .FindAll(TreeScope_Descendants, &cond)
                    .map_err(native)?;
            }
            let len = all.Length().map_err(native)?;
            if !(0..=1024).contains(&len) {
                return Err(Error::input(
                    "Steam window accessibility tree exceeds the bounded scan",
                ));
            }
            let mut body = String::new();
            let mut buttons = Vec::new();
            for i in 0..len {
                let e = all.GetElement(i).map_err(native)?;
                let provider = e.CurrentProcessId().map_err(native)? as u32;
                if !trusted(provider, owner, root, rows) {
                    return Err(Error::input(
                        "Steam window has a foreign accessibility provider",
                    ));
                }
                if e.CurrentIsOffscreen().map_err(native)?.as_bool() {
                    continue;
                }
                let name = e.CurrentName().map_err(native)?.to_string();
                if name.is_empty() {
                    continue;
                }
                if name.len() > 4096 || body.len() > 65536 {
                    return Err(Error::input("Steam warning text exceeds the bounded scan"));
                }
                if e.CurrentControlType().map_err(native)? == UIA_ButtonControlTypeId {
                    if e.CurrentIsEnabled().map_err(native)?.as_bool() {
                        buttons.push((name, e));
                    }
                } else {
                    body.push_str(&name);
                    body.push('\n');
                }
            }
            Ok(Snapshot {
                hwnd,
                pid,
                title,
                body,
                buttons,
                pin: Pin::open(pid)?,
            })
        }
    }
    fn scan(session: &Session) -> Result<(Com, IUIAutomation, Pin, Vec<Snapshot>)> {
        let com = Com::new()?;
        let uia: IUIAutomation =
            unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }
                .map_err(native)?;
        let bounded: IUIAutomation2 = uia.cast().map_err(native)?;
        unsafe {
            bounded.SetConnectionTimeout(500).map_err(native)?;
            bounded.SetTransactionTimeout(1000).map_err(native)?;
        }
        let rows = rows()?;
        let owner = owner(&session.root, &rows)?;
        if owner.pid != session.owner_pid || owner.started != session.owner_started {
            return Err(Error::input(
                "registered Steam process changed during launch",
            ));
        }
        let mut windows: Vec<(isize, u32, String)> = Vec::new();
        if unsafe { EnumWindows(Some(enumerate), &mut windows as *mut _ as isize) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut snapshots = Vec::new();
        for (hwnd, pid, title) in windows {
            if !trusted(pid, &owner, &session.root, &rows) {
                continue;
            }
            if snapshots.len() >= 16 {
                return Err(Error::input("too many Steam windows"));
            }
            match read(&uia, hwnd, pid, title.clone(), &owner, &session.root, &rows) {
                Ok(s) => snapshots.push(s),
                Err(e) if title.to_ascii_lowercase().contains("sync") || title.contains("同期") => {
                    return Err(e)
                }
                Err(e)
                    if title.eq_ignore_ascii_case("steam") && session.console_proves_civ6()? =>
                {
                    return Err(e)
                }
                Err(_) => {} // An ordinary unreadable library is not a dialog.
            }
        }
        Ok((com, uia, owner, snapshots))
    }
    pub fn probe(session: &Session) -> Result<Value> {
        let (_com, _uia, _owner, windows) = scan(session)?;
        Ok(
            serde_json::json!({"mode":"read_only","console_proves_civ6":session.console_proves_civ6()?,"steam_owner_pid":session.owner_pid,"steam_owner_started":session.owner_started,"windows":windows.iter().map(Snapshot::value).collect::<Vec<_>>()}),
        )
    }
    pub fn handle(session: &Session, journal: &Path) -> Result<bool> {
        use crate::steam_policy::{decide, Action};
        let (_com, uia, owner, windows) = scan(session)?;
        if windows.is_empty() {
            return Ok(false);
        }
        let proof = session.console_proves_civ6()?;
        let mut candidates = Vec::new();
        for window in &windows {
            let names = window
                .buttons
                .iter()
                .map(|b| b.0.clone())
                .collect::<Vec<_>>();
            let action = decide(&window.title, &window.body, &names, proof);
            if action != Action::None {
                candidates.push((window, action));
            }
        }
        if candidates.is_empty() {
            return Ok(false);
        }
        if candidates.len() != 1 {
            return Err(Error::input(
                "multiple Steam warnings; refusing ambiguous action",
            ));
        }
        let (window, action) = candidates.remove(0);
        let mut lease = crate::startup::RecoveryLease::acquire()?;
        lease.check()?;
        let event = serde_json::json!({"time":chrono::Utc::now(),"window":window.value(),"console_proves_civ6":proof,"action":format!("{action:?}")});
        match action {
            Action::None => Ok(false),
            Action::Wait => {
                record(journal, serde_json::json!({"phase":"wait","event":event}))?;
                Ok(true)
            }
            Action::Stop(reason) => {
                record(
                    journal,
                    serde_json::json!({"phase":"stopped","event":event,"reason":reason}),
                )?;
                Err(Error::input(
                    "Steam warning cannot be safely resolved; local saves retained",
                ))
            }
            Action::Select(index) => {
                // Never resend a dispatched action, including after an unknown
                // Invoke result or a journal failure after dispatch.
                let key = format!("{}:{}:{}", window.hwnd, window.pid, window.pin.started);
                let history = match std::fs::read_to_string(journal) {
                    Ok(s) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                    Err(e) => return Err(e.into()),
                };
                if history.len() > 1024 * 1024 {
                    return Err(Error::input(
                        "Steam warning journal exceeds bounded replay check",
                    ));
                }
                {
                    for line in history.lines() {
                        let v = serde_json::from_str::<Value>(line).map_err(|_| {
                            Error::input(
                                "Steam warning journal is incomplete; refusing uncertain replay",
                            )
                        })?;
                        if v["phase"] == "before" && v["key"] == key {
                            return Err(Error::input(
                                "Steam warning action was already attempted; refusing replay",
                            ));
                        }
                    }
                }
                record(
                    journal,
                    serde_json::json!({"phase":"before","key":key,"event":event}),
                )?;
                owner.check()?;
                window.pin.check()?;
                lease.check()?;
                if proof && !session.console_proves_civ6()? {
                    return Err(Error::input(
                        "Steam launch action changed before warning selection",
                    ));
                }
                let rows = rows()?;
                let now = read(
                    &uia,
                    window.hwnd,
                    window.pid,
                    window.title.clone(),
                    &owner,
                    &session.root,
                    &rows,
                )?;
                if now.body != window.body
                    || now.buttons.iter().map(|b| &b.0).collect::<Vec<_>>()
                        != window.buttons.iter().map(|b| &b.0).collect::<Vec<_>>()
                {
                    return Err(Error::input("Steam warning changed before action"));
                }
                let old = &window
                    .buttons
                    .get(index)
                    .ok_or_else(|| Error::input("invalid Steam button index"))?
                    .1;
                let current = &now.buttons[index].1;
                unsafe {
                    if !uia.CompareElements(old, current).map_err(native)?.as_bool() {
                        return Err(Error::input("Steam button identity changed before action"));
                    }
                    let invoke: IUIAutomationInvokePattern = current
                        .GetCurrentPatternAs(UIA_InvokePatternId)
                        .map_err(native)?;
                    owner.check()?;
                    window.pin.check()?;
                    lease.check()?;
                    if proof && !session.console_proves_civ6()? {
                        return Err(Error::input("Steam launch action changed before dispatch"));
                    }
                    invoke.Invoke().map_err(|e| {
                        Error::input(format!("Steam Invoke result unknown; do not retry: {e}"))
                    })?;
                }
                record(
                    journal,
                    serde_json::json!({"phase":"selected","key":key,"event":event,"choice":"keep_pc_files"}),
                )?;
                Ok(true)
            }
        }
    }
}
#[cfg(windows)]
use win::identity as owner_identity;
#[cfg(windows)]
pub use win::{handle as handle_warning, probe, root as registered_root};
#[cfg(not(windows))]
pub fn registered_root() -> Result<PathBuf> {
    Err(Error::input("Steam warning recovery requires Windows"))
}
#[cfg(not(windows))]
fn owner_identity(_: &Path) -> Result<(u32, u64, chrono::NaiveDateTime)> {
    Err(Error::input("Steam warning recovery requires Windows"))
}
#[cfg(not(windows))]
pub fn probe(_: &Session) -> Result<Value> {
    Err(Error::input("Steam warning recovery requires Windows"))
}
#[cfg(not(windows))]
pub fn handle_warning(_: &Session, _: &Path) -> Result<bool> {
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inherited_warning_uses_current_action_state_tied_to_steam_creation() {
        let floor =
            chrono::NaiveDateTime::parse_from_str("2026-10-06 11:11:31", "%Y-%m-%d %H:%M:%S")
                .unwrap();
        let now = floor + chrono::Duration::days(2);
        let actual="[2026-10-08 03:58:54] GameAction [AppID 289070, ActionID 248] : LaunchApp waiting for user response to SynchronizingControllerConfig \"syncfailed\"\n";
        assert!(current_console_proves(actual, floor, now));
        assert!(!current_console_proves(actual, now, now));
        let other="[2026-10-08 04:00:00] GameAction [AppID 123, ActionID 249] : LaunchApp changed task to SynchronizingStats with \"\"\n";
        assert!(!current_console_proves(
            &(actual.to_owned() + other),
            floor,
            now
        ));
        let resolved="[2026-10-08 04:00:00] GameAction [AppID 289070, ActionID 248] : LaunchApp continues with user response\n";
        assert!(!current_console_proves(
            &(actual.to_owned() + resolved),
            floor,
            now
        ));
    }
    #[test]
    fn only_one_fresh_civ6_pending_action_can_bind_a_generic_warning() {
        let at = chrono::NaiveDateTime::parse_from_str("2026-10-08 03:58:13", "%Y-%m-%d %H:%M:%S")
            .unwrap();
        let now = at + chrono::Duration::seconds(42);
        let actual="[2026-10-08 03:58:54] GameAction [AppID 289070, ActionID 248] : LaunchApp waiting for user response to SynchronizingControllerConfig \"syncfailed\"\n";
        assert!(console_proves(actual, at, now));
        assert!(!console_proves(
            actual,
            now + chrono::Duration::seconds(1),
            now + chrono::Duration::seconds(1)
        ));
        assert!(!console_proves(&actual.replace("289070", "123"), at, now));
        assert!(!console_proves(
            &(actual.to_owned() + &actual.replace("289070", "123")),
            at,
            now
        ));
        assert!(!console_proves(&(actual.to_owned()+"[2026-10-08 03:58:55] GameAction [AppID 289070, ActionID 248] : LaunchApp continues with user response\n"),at,now));
    }
}
