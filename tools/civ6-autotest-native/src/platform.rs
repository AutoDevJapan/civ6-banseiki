//! Windows process identity, DX11 launch, and input for one verified Civ VI window.

#[cfg(any(windows, test))]
use std::path::Path;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct CivProcess {
    pub pid: u32,
    /// Raw Windows FILETIME ticks: 100 ns since 1601-01-01 UTC.
    pub created: u64,
    pub path: PathBuf,
}

#[cfg(any(windows, test))]
const TITLE_PREFIX: &str = "Sid Meier's Civilization VI";
#[cfg(windows)]
const STEAM_URI: &str = "steam://launch/289070/option2";

#[cfg(any(windows, test))]
fn is_civ_executable(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("CivilizationVI.exe")
                || name.eq_ignore_ascii_case("CivilizationVI_DX12.exe")
        })
}

#[cfg(any(windows, test))]
fn require_dx11(process: &CivProcess) -> Result<(), String> {
    if process
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("CivilizationVI.exe"))
    {
        Ok(())
    } else {
        Err(format!(
            "Steam DX11 launch opened {}; expected CivilizationVI.exe",
            process.path.display()
        ))
    }
}

#[cfg(any(windows, test))]
fn unique_pid(pids: &[u32]) -> Result<Option<u32>, String> {
    match pids {
        [] => Ok(None),
        [pid] => Ok(Some(*pid)),
        _ => Err(format!(
            "Multiple Civ VI processes found ({}); refusing operation",
            pids.len()
        )),
    }
}

#[cfg(any(windows, test))]
fn require_identity(expected: &CivProcess, actual: &CivProcess) -> Result<(), String> {
    // Compare the full path as well as the exact executable basename. A mismatch
    // fails closed, including a case-only change to the queried full path.
    if expected.pid != 0
        && expected.created != 0
        && is_civ_executable(&actual.path)
        && expected.pid == actual.pid
        && expected.created == actual.created
        && expected.path == actual.path
    {
        Ok(())
    } else {
        Err(format!(
            "Civ VI process {} changed; refusing operation",
            expected.pid
        ))
    }
}

#[cfg(any(windows, test))]
fn key_message_params(scan: u32) -> (isize, isize) {
    let down = 1u32 | ((scan & 0xff) << 16);
    (down as isize, (down | 0xc000_0000) as isize)
}

#[cfg(any(windows, test))]
fn is_civ_title(title: &str) -> bool {
    title.starts_with(TITLE_PREFIX)
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::ffi::OsString;
    use std::mem::size_of;
    use std::os::windows::ffi::OsStringExt;
    use std::ptr::{null, null_mut};
    use std::thread;
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_NO_MORE_FILES, FILETIME, GetLastError, HANDLE, HWND,
        INVALID_HANDLE_VALUE, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        PROCESS_TERMINATE, QueryFullProcessImageNameW, TerminateProcess, WaitForSingleObject,
    };
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MAPVK_VK_TO_VSC, MapVirtualKeyW};
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible, PostMessageW, SW_SHOWNORMAL, WM_KEYDOWN, WM_KEYUP,
    };

    const POLL: Duration = Duration::from_millis(500);
    const KEY_HOLD: Duration = Duration::from_millis(80);
    const EXIT_WAIT_MS: u32 = 10_000;

    struct OwnedHandle(HANDLE);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            // Only successful OpenProcess/CreateToolhelp32Snapshot results enter here.
            unsafe { CloseHandle(self.0) };
        }
    }

    fn win_error(action: &str) -> String {
        format!("{action}: {}", std::io::Error::last_os_error())
    }

    fn process_ids() -> Result<Vec<u32>, String> {
        // Enumerate processes, not windows: hidden/loading/DX12 processes count too.
        let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            return Err(win_error("Could not enumerate Civ VI processes"));
        }
        let snapshot = OwnedHandle(raw);
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if unsafe { Process32FirstW(snapshot.0, &mut entry) } == 0 {
            let code = unsafe { GetLastError() };
            if code == ERROR_NO_MORE_FILES {
                return Ok(Vec::new());
            }
            return Err(format!("Could not read process list: Windows error {code}"));
        }
        let mut pids = Vec::new();
        loop {
            let length = entry
                .szExeFile
                .iter()
                .position(|ch| *ch == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = OsString::from_wide(&entry.szExeFile[..length]);
            if is_civ_executable(Path::new(&name)) {
                pids.push(entry.th32ProcessID);
            }
            if unsafe { Process32NextW(snapshot.0, &mut entry) } == 0 {
                let code = unsafe { GetLastError() };
                if code != ERROR_NO_MORE_FILES {
                    return Err(format!(
                        "Process list became unavailable: Windows error {code}"
                    ));
                }
                break;
            }
        }
        pids.sort_unstable();
        pids.dedup();
        Ok(pids)
    }

    fn open_process(pid: u32, access: u32) -> Result<OwnedHandle, String> {
        let handle = unsafe { OpenProcess(access, 0, pid) };
        if handle.is_null() {
            Err(win_error(&format!("Cannot open Civ VI process {pid}")))
        } else {
            Ok(OwnedHandle(handle))
        }
    }

    fn identity(handle: &OwnedHandle, pid: u32) -> Result<CivProcess, String> {
        let mut image = vec![0u16; 32_768];
        let mut length = image.len() as u32;
        if unsafe { QueryFullProcessImageNameW(handle.0, 0, image.as_mut_ptr(), &mut length) } == 0
        {
            return Err(win_error("Cannot read Civ VI process executable"));
        }
        if length == 0 || length as usize >= image.len() {
            return Err("Civ VI executable path is empty or truncated".into());
        }
        let path = PathBuf::from(OsString::from_wide(&image[..length as usize]));
        if !is_civ_executable(&path) {
            return Err(format!(
                "Process {pid} is not an exact Civ VI executable: {}",
                path.display()
            ));
        }
        let mut created = FILETIME::default();
        let mut exited = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        if unsafe { GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) }
            == 0
        {
            return Err(win_error("Cannot read Civ VI process creation time"));
        }
        let created = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
        if created == 0 {
            return Err("Civ VI process creation time is unavailable".into());
        }
        Ok(CivProcess { pid, created, path })
    }

    fn is_running(handle: &OwnedHandle) -> Result<bool, String> {
        match unsafe { WaitForSingleObject(handle.0, 0) } {
            WAIT_TIMEOUT => Ok(true),
            WAIT_OBJECT_0 => Ok(false),
            WAIT_FAILED => Err(win_error("Cannot check Civ VI process exit")),
            result => Err(format!("Unexpected Civ VI process wait result: {result}")),
        }
    }

    enum ProcessProbe {
        Absent,
        Overlap(usize),
        One(CivProcess, OwnedHandle),
    }

    fn probe_process() -> Result<ProcessProbe, String> {
        let pids = process_ids()?;
        if pids.len() > 1 {
            return Ok(ProcessProbe::Overlap(pids.len()));
        }
        let Some(pid) = unique_pid(&pids)? else {
            return Ok(ProcessProbe::Absent);
        };
        let handle =
            match open_process(pid, PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE) {
                Ok(handle) => handle,
                Err(error) => {
                    let current = process_ids()?;
                    if current.len() > 1 {
                        return Ok(ProcessProbe::Overlap(current.len()));
                    }
                    if current.is_empty() {
                        return Ok(ProcessProbe::Absent);
                    }
                    return Err(error);
                }
            };
        if !is_running(&handle)? {
            return Ok(ProcessProbe::Absent);
        }
        let process = match identity(&handle, pid) {
            Ok(process) => process,
            Err(error) => {
                // The game may finish exiting after OpenProcess succeeds. Check
                // the pinned handle rather than opening a possibly reused PID.
                if !is_running(&handle)? {
                    return Ok(ProcessProbe::Absent);
                }
                return Err(error);
            }
        };
        let current = process_ids()?;
        if current.len() > 1 {
            return Ok(ProcessProbe::Overlap(current.len()));
        }
        match unique_pid(&current)? {
            None => Ok(ProcessProbe::Absent),
            Some(current_pid) if current_pid == pid => {
                if is_running(&handle)? {
                    Ok(ProcessProbe::One(process, handle))
                } else {
                    Ok(ProcessProbe::Absent)
                }
            }
            Some(_) => Err("Civ VI process changed while inspecting it".into()),
        }
    }

    fn sleep_until(deadline: Instant, interval: Duration) {
        thread::sleep(interval.min(deadline.saturating_duration_since(Instant::now())));
    }

    pub fn single_civ_process(settle: Duration) -> Result<Option<CivProcess>, String> {
        let deadline = Instant::now()
            .checked_add(settle)
            .ok_or("Civ VI process settling duration is too large")?;
        loop {
            match probe_process()? {
                ProcessProbe::Absent => return Ok(None),
                ProcessProbe::One(process, _) => return Ok(Some(process)),
                ProcessProbe::Overlap(count) => {
                    if Instant::now() >= deadline {
                        return Err(format!(
                            "Multiple Civ VI processes found ({count}); refusing operation"
                        ));
                    }
                    sleep_until(deadline, POLL);
                }
            }
        }
    }

    pub fn launch_via_steam() -> Result<(), String> {
        if single_civ_process(Duration::ZERO)?.is_some() {
            return Err("Civ VI is already running; refusing another Steam launch".into());
        }
        // Do not kill reporter/helper PIDs here: this API has no owned identity
        // for them. The caller owns process cleanup and the live-chain lock.
        let open: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
        let uri: Vec<u16> = STEAM_URI.encode_utf16().chain(Some(0)).collect();
        let result = unsafe {
            ShellExecuteW(
                null_mut(),
                open.as_ptr(),
                uri.as_ptr(),
                null(),
                null(),
                SW_SHOWNORMAL,
            )
        } as isize;
        if result <= 32 {
            Err(format!(
                "Could not launch Civ VI DX11 through Steam: ShellExecuteW code {result}"
            ))
        } else {
            Ok(())
        }
    }

    pub fn terminate_civ_process(expected: &CivProcess) -> Result<(), String> {
        require_identity(expected, expected)?;
        let Some(pid) = unique_pid(&process_ids()?)? else {
            return Ok(());
        };
        if pid != expected.pid {
            return Err(
                "The expected Civ VI process has been replaced; refusing termination".into(),
            );
        }
        let handle = match open_process(
            pid,
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE,
        ) {
            Ok(handle) => handle,
            Err(error) => {
                if unique_pid(&process_ids()?)?.is_none() {
                    return Ok(());
                }
                return Err(error);
            }
        };
        // Pin ONE handle, and query both identity fields on THAT handle before
        // terminating it. A PID reused between snapshots must never be killed.
        require_identity(expected, &identity(&handle, pid)?)?;
        if !is_running(&handle)? {
            return Ok(()); // WAIT_OBJECT_0 has already confirmed exit on this handle.
        }
        if unique_pid(&process_ids()?)? != Some(expected.pid) {
            return Err("Civ VI process list changed before termination".into());
        }
        if unsafe { TerminateProcess(handle.0, 1) } == 0 {
            let error = win_error("Could not terminate Civ VI process");
            if !is_running(&handle)? {
                return Ok(());
            }
            return Err(error);
        }
        match unsafe { WaitForSingleObject(handle.0, EXIT_WAIT_MS) } {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_FAILED => Err(win_error("Could not wait for Civ VI process exit")),
            result => Err(format!(
                "Civ VI process {} did not finish exiting (wait={result})",
                expected.pid
            )),
        }
    }

    struct WindowEnumeration {
        pid: u32,
        windows: Vec<HWND>,
    }

    fn window_title(hwnd: HWND) -> Option<String> {
        let length = unsafe { GetWindowTextLengthW(hwnd) };
        if !(1..32_768).contains(&length) {
            return None;
        }
        let mut text = vec![0u16; length as usize + 1];
        let copied = unsafe { GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32) };
        if copied <= 0 {
            None
        } else {
            Some(String::from_utf16_lossy(&text[..copied as usize]))
        }
    }

    unsafe extern "system" fn enum_window(hwnd: HWND, param: isize) -> i32 {
        // EnumWindows calls synchronously; the stack-owned context outlives it.
        let state = unsafe { &mut *(param as *mut WindowEnumeration) };
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return 1;
        }
        let mut pid = 0;
        if unsafe { GetWindowThreadProcessId(hwnd, &mut pid) } == 0 || pid != state.pid {
            return 1;
        }
        if window_title(hwnd).is_some_and(|title| is_civ_title(&title)) {
            state.windows.push(hwnd);
        }
        1
    }

    fn matching_windows(pid: u32) -> Result<Vec<HWND>, String> {
        let mut state = WindowEnumeration {
            pid,
            windows: Vec::new(),
        };
        if unsafe {
            EnumWindows(
                Some(enum_window),
                &mut state as *mut WindowEnumeration as isize,
            )
        } == 0
        {
            return Err(win_error("Could not enumerate Civ VI windows"));
        }
        Ok(state.windows)
    }

    struct VerifiedWindow {
        hwnd: HWND,
        process: CivProcess,
        handle: OwnedHandle,
    }

    enum WindowProbe {
        Pending(String),
        Ready(VerifiedWindow),
    }

    fn probe_window() -> Result<WindowProbe, String> {
        match probe_process()? {
            ProcessProbe::Absent => Ok(WindowProbe::Pending("no Civ VI process yet".into())),
            ProcessProbe::Overlap(count) => Ok(WindowProbe::Pending(format!(
                "Multiple Civ VI processes found ({count}); waiting without input"
            ))),
            ProcessProbe::One(process, handle) => {
                require_dx11(&process)?;
                let windows = matching_windows(process.pid)?;
                if windows.len() != 1 {
                    return Ok(WindowProbe::Pending(format!(
                        "Expected one visible Civ VI executable window; found {}",
                        windows.len()
                    )));
                }
                let window = VerifiedWindow {
                    hwnd: windows[0],
                    process,
                    handle,
                };
                require_window(&window)?;
                Ok(WindowProbe::Ready(window))
            }
        }
    }

    // This check deliberately uses the retained handle, never another handle
    // opened solely from the saved PID. It also guards a key-up cleanup message.
    fn require_target(window: &VerifiedWindow) -> Result<(), String> {
        if !is_running(&window.handle)? {
            return Err("Civ VI process exited before window input".into());
        }
        require_identity(
            &window.process,
            &identity(&window.handle, window.process.pid)?,
        )?;
        let mut pid = 0;
        if unsafe { IsWindowVisible(window.hwnd) } == 0
            || unsafe { GetWindowThreadProcessId(window.hwnd, &mut pid) } == 0
            || pid != window.process.pid
            || !window_title(window.hwnd).is_some_and(|title| is_civ_title(&title))
        {
            return Err("Civ VI window changed; input aborted".into());
        }
        Ok(())
    }

    fn require_window(window: &VerifiedWindow) -> Result<(), String> {
        if unique_pid(&process_ids()?)? != Some(window.process.pid)
            || matching_windows(window.process.pid)? != vec![window.hwnd]
        {
            return Err("Civ VI window/process is no longer unique; input aborted".into());
        }
        require_target(window)?;
        Ok(())
    }

    fn require_time(deadline: Instant, needed: Duration) -> Result<(), String> {
        if deadline.saturating_duration_since(Instant::now()) <= needed {
            Err("Step deadline expired before Civ VI key input".into())
        } else {
            Ok(())
        }
    }

    pub fn wait_for_window(deadline: Instant) -> Result<(), String> {
        let mut last = "no Civ VI process yet".to_string();
        while Instant::now() < deadline {
            match probe_window()? {
                WindowProbe::Ready(_) => {
                    if Instant::now() < deadline {
                        return Ok(());
                    }
                    break;
                }
                WindowProbe::Pending(reason) => last = reason,
            }
            sleep_until(deadline, POLL);
        }
        Err(format!(
            "Civ VI DX11 window did not appear before the deadline: {last}"
        ))
    }

    pub fn focus_and_key(expected: &CivProcess, vk: u16, deadline: Instant) -> Result<(), String> {
        if !(1..=255).contains(&vk) {
            return Err(format!("Invalid virtual key: {vk}"));
        }
        require_time(deadline, KEY_HOLD)?;
        let window = match probe_window()? {
            WindowProbe::Ready(window) => window,
            WindowProbe::Pending(reason) => return Err(reason),
        };
        require_identity(expected, &window.process)?;
        require_window(&window)?;
        let scan = unsafe { MapVirtualKeyW(u32::from(vk), MAPVK_VK_TO_VSC) };
        if scan == 0 {
            return Err(format!("Cannot map virtual key {vk} to a scan code"));
        }
        let (down, up) = key_message_params(scan);
        require_window(&window)?;
        require_time(deadline, KEY_HOLD)?;
        // Retain the proven WM delivery mode used by the existing loading-screen
        // driver; never send global keyboard input to whatever happens to have focus.
        if unsafe { PostMessageW(window.hwnd, WM_KEYDOWN, usize::from(vk), down) } == 0 {
            return Err(win_error("Could not post Civ VI key down"));
        }
        thread::sleep(KEY_HOLD);
        let target_result = require_window(&window);
        // Release even after deadline expiry, but only to the original,
        // still-owned Civ VI target. HWND/PID reuse must not receive cleanup input.
        require_target(&window)?;
        if unsafe { PostMessageW(window.hwnd, WM_KEYUP, usize::from(vk), up) } == 0 {
            return Err(win_error("Could not post Civ VI key up"));
        }
        target_result?;
        require_time(deadline, Duration::ZERO)?;
        require_window(&window)
    }
}

#[cfg(windows)]
pub use native::{
    focus_and_key, launch_via_steam, single_civ_process, terminate_civ_process, wait_for_window,
};

#[cfg(not(windows))]
pub fn single_civ_process(_: Duration) -> Result<Option<CivProcess>, String> {
    Err("Civ VI platform operations require Windows".into())
}
#[cfg(not(windows))]
pub fn launch_via_steam() -> Result<(), String> {
    Err("Civ VI platform operations require Windows".into())
}
#[cfg(not(windows))]
pub fn terminate_civ_process(_: &CivProcess) -> Result<(), String> {
    Err("Civ VI platform operations require Windows".into())
}
#[cfg(not(windows))]
pub fn wait_for_window(_: Instant) -> Result<(), String> {
    Err("Civ VI platform operations require Windows".into())
}
#[cfg(not(windows))]
pub fn focus_and_key(_: &CivProcess, _: u16, _: Instant) -> Result<(), String> {
    Err("Civ VI platform operations require Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn process() -> CivProcess {
        CivProcess {
            pid: 42,
            created: 134_000_000_000_000_000,
            path: PathBuf::from("CivilizationVI.exe"),
        }
    }

    #[test]
    fn executable_match_is_exact_and_case_insensitive() {
        for name in [
            "CivilizationVI.exe",
            "CIVILIZATIONVI.EXE",
            "CivilizationVI_DX12.exe",
        ] {
            assert!(is_civ_executable(Path::new(name)), "{name}");
        }
        for name in [
            "OtherGame.exe",
            "CivilizationVI.exe.bak",
            "NotCivilizationVI.exe",
            "FiraxisBugReporter.exe",
        ] {
            assert!(!is_civ_executable(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn hidden_second_process_cannot_be_ignored() {
        assert_eq!(unique_pid(&[]), Ok(None));
        assert_eq!(unique_pid(&[42]), Ok(Some(42)));
        assert!(
            unique_pid(&[42, 43])
                .unwrap_err()
                .contains("Multiple Civ VI processes")
        );
    }

    #[test]
    fn identity_rejects_pid_reuse_or_a_different_image() {
        let expected = process();
        assert!(require_identity(&expected, &expected).is_ok());
        let changed = [
            CivProcess {
                pid: 43,
                ..expected.clone()
            },
            CivProcess {
                created: expected.created + 1,
                ..expected.clone()
            },
            CivProcess {
                path: PathBuf::from("OtherGame.exe"),
                ..expected.clone()
            },
            CivProcess {
                path: PathBuf::from("OtherFolder/CivilizationVI.exe"),
                ..expected.clone()
            },
        ];
        for actual in changed {
            assert!(require_identity(&expected, &actual).is_err());
        }
    }

    #[test]
    fn dx12_is_visible_to_process_discovery_but_rejected_for_input() {
        let dx12 = CivProcess {
            path: PathBuf::from("CivilizationVI_DX12.exe"),
            ..process()
        };
        assert!(is_civ_executable(&dx12.path));
        assert!(require_dx11(&dx12).is_err());
        assert!(require_dx11(&process()).is_ok());
    }

    #[test]
    fn window_key_messages_have_repeat_scan_transition_and_release_bits() {
        let (down, up) = key_message_params(1); // Escape
        assert_eq!(down as u32, 0x0001_0001);
        assert_eq!(up as u32, 0xc001_0001);
    }

    #[test]
    fn only_the_civ_title_prefix_is_accepted() {
        assert!(is_civ_title("Sid Meier's Civilization VI (DX11)"));
        assert!(!is_civ_title("Another Game - Sid Meier's Civilization VI"));
        assert!(!is_civ_title("Sid Meier's Civilization V"));
        assert!(!is_civ_title(""));
    }
}
