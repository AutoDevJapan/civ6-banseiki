//! Civ VI input is sent only after a unique process and window are verified.
use crate::{time::monotonic, Error, Result};
use std::{collections::BTreeSet, time::Duration};

pub const VK_ESCAPE: u16 = 0x1b;
pub const TITLE_PREFIX: &str = "Sid Meier's Civilization VI";
pub const CIV_EXECUTABLES: [&str; 2] = ["civilizationvi.exe", "civilizationvi_dx12.exe"];
pub const WM_CLOSE: u32 = 0x10;
pub const WM_KEYDOWN: u32 = 0x100;
pub const WM_KEYUP: u32 = 0x101;
pub const LEFT_DOWN: u32 = 2;
pub const LEFT_UP: u32 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyDelivery {
    Window,
    SendInput,
}

/// The decision path uses this backend on both Windows and in deterministic tests.
pub trait InputBackend {
    fn process_ids(&mut self, names: &BTreeSet<String>) -> Result<BTreeSet<u32>>;
    fn windows(&mut self) -> Result<Vec<(isize, String)>>;
    fn process_name(&mut self, hwnd: isize) -> Result<String>;
    fn foreground(&mut self) -> isize;
    fn restore(&mut self, hwnd: isize);
    fn activate(&mut self, hwnd: isize) -> bool;
    fn rect(&mut self, hwnd: isize) -> Result<(i32, i32, i32, i32)>;
    fn cursor(&mut self, x: i32, y: i32) -> bool;
    fn mouse(&mut self, flag: u32);
    fn scan(&mut self, vk: u16) -> u32;
    fn post(&mut self, hwnd: isize, message: u32, vk: usize, param: isize) -> bool;
    fn send(&mut self, vk: u16, up: bool) -> bool;
    fn now(&mut self) -> f64 {
        monotonic()
    }
    fn sleep(&mut self, seconds: f64) {
        std::thread::sleep(Duration::from_secs_f64(seconds));
    }
}

pub fn civ_names() -> BTreeSet<String> {
    CIV_EXECUTABLES.iter().map(|s| (*s).to_string()).collect()
}

pub fn matches_executable(name: &str, requested: &BTreeSet<String>) -> bool {
    requested.contains(&name.to_lowercase())
}

pub fn require_single_with<B: InputBackend>(backend: &mut B) -> Result<()> {
    let ids = backend.process_ids(&civ_names())?;
    match ids.len() {
        0 => Err(Error::input(
            "FireTuner に接続できません（Civ6 が起動していない可能性）",
        )),
        1 => Ok(()),
        n => Err(Error::input(format!(
            "Multiple Civ VI processes found ({n}); driver requires one local game; candidate_pids={ids:?}"
        ))),
    }
}

pub fn find_civ_window_with<B: InputBackend>(backend: &mut B) -> Result<isize> {
    require_single_with(backend)?;
    let mut found = Vec::new();
    for (hwnd, title) in backend.windows()? {
        if title.starts_with(TITLE_PREFIX)
            && CIV_EXECUTABLES.contains(&backend.process_name(hwnd)?.to_lowercase().as_str())
        {
            found.push(hwnd);
        }
    }
    if found.len() != 1 {
        return Err(Error::input(format!(
            "Expected one visible Civ VI executable window; found {}",
            found.len()
        )));
    }
    Ok(found[0])
}

fn foreground<B: InputBackend>(backend: &mut B, hwnd: isize) -> Result<()> {
    if find_civ_window_with(backend)? != hwnd || backend.foreground() != hwnd {
        return Err(Error::input(
            "Civ VI window is not the verified foreground window; input aborted",
        ));
    }
    Ok(())
}

fn focus<B: InputBackend>(backend: &mut B, hwnd: isize) -> Result<()> {
    backend.restore(hwnd);
    if !backend.activate(hwnd) {
        return Err(Error::input(
            "Could not bring Civ VI window to foreground; input aborted",
        ));
    }
    backend.sleep(0.4);
    foreground(backend, hwnd)
}

fn deadline<B: InputBackend>(backend: &mut B, end: Option<f64>) -> Result<()> {
    if end.is_some_and(|d| backend.now() >= d) {
        Err(Error::input("Step deadline expired before key input"))
    } else {
        Ok(())
    }
}

pub fn click_fraction_with<B: InputBackend>(
    backend: &mut B,
    fx: f64,
    fy: f64,
) -> Result<(i32, i32)> {
    if !fx.is_finite()
        || !fy.is_finite()
        || !(0.0..=1.0).contains(&fx)
        || !(0.0..=1.0).contains(&fy)
    {
        return Err(Error::input("fractions must be within 0..1"));
    }
    let hwnd = find_civ_window_with(backend)?;
    focus(backend, hwnd)?;
    let (l, t, r, b) = backend.rect(hwnd)?;
    if r <= l || b <= t {
        return Err(Error::input("Civ VI window has no visible area"));
    }
    let x = l + ((r - l) as f64 * fx) as i32;
    let y = t + ((b - t) as f64 * fy) as i32;
    foreground(backend, hwnd)?;
    if !backend.cursor(x, y) {
        return Err(Error::input("SetCursorPos failed"));
    }
    backend.sleep(0.2);
    foreground(backend, hwnd)?;
    backend.mouse(LEFT_DOWN);
    backend.sleep(0.08);
    let check = foreground(backend, hwnd);
    backend.mouse(LEFT_UP);
    check?;
    Ok((x, y))
}

pub fn press_key_with<B: InputBackend>(
    backend: &mut B,
    vk: u16,
    end: Option<f64>,
    delivery: KeyDelivery,
) -> Result<()> {
    let hwnd = find_civ_window_with(backend)?;
    match delivery {
        KeyDelivery::Window => {
            let param = (1 | backend.scan(vk) << 16) as isize;
            if find_civ_window_with(backend)? != hwnd {
                return Err(Error::input("Civ VI window changed before key input"));
            }
            deadline(backend, end)?; // immediately before sending
            if !backend.post(hwnd, WM_KEYDOWN, vk as usize, param) {
                return Err(Error::input("Could not post Civ VI key down"));
            }
            backend.sleep(0.08);
            if !backend.post(hwnd, WM_KEYUP, vk as usize, param | 0xC000_0000u32 as isize) {
                return Err(Error::input("Could not post Civ VI key up"));
            }
        }
        KeyDelivery::SendInput => {
            focus(backend, hwnd)?;
            foreground(backend, hwnd)?;
            deadline(backend, end)?; // immediately before sending
            if !backend.send(vk, false) {
                return Err(Error::input("Could not send Civ VI key input"));
            }
            let check = foreground(backend, hwnd);
            if check.is_ok() {
                backend.sleep(0.08);
            }
            let released = backend.send(vk, true);
            if !released {
                return Err(Error::input("Could not send Civ VI key input"));
            }
            check?;
            foreground(backend, hwnd)?;
        }
    }
    Ok(())
}

pub fn request_civ_window_close_with<B: InputBackend>(backend: &mut B) -> Result<()> {
    let hwnd = find_civ_window_with(backend)?;
    if find_civ_window_with(backend)? != hwnd || !backend.post(hwnd, WM_CLOSE, 0, 0) {
        return Err(Error::input("Could not request Civ VI window close"));
    }
    Ok(())
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::{mem::size_of, sync::Once};
    type Handle = isize;
    #[derive(Default)]
    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[derive(Clone, Debug, serde::Serialize)]
    struct ProcessCandidate {
        pid: u32,
        snapshot_image: String,
        // A reused PID can belong to an unrelated app. Keep its name private.
        #[serde(skip_serializing)]
        image: Option<String>,
        image_matches_snapshot: Option<bool>,
        created_filetime: Option<u64>,
        exited: Option<bool>,
        absent: bool,
        wait_result: Option<u32>,
        inspection_error: Option<String>,
    }
    impl ProcessCandidate {
        fn new(pid: u32, snapshot_image: String) -> Self {
            Self {
                pid,
                snapshot_image,
                image: None,
                image_matches_snapshot: None,
                created_filetime: None,
                exited: None,
                absent: false,
                wait_result: None,
                inspection_error: None,
            }
        }
        fn error(&mut self, operation: &str) {
            self.inspection_error = Some(format!(
                "{operation} os_error={:?}",
                std::io::Error::last_os_error().raw_os_error()
            ));
        }
        fn observe_exit(&mut self, handle: Handle) {
            let wait = unsafe { WaitForSingleObject(handle, 0) };
            self.wait_result = Some(wait);
            self.exited = match wait {
                0 => Some(true),
                258 => Some(false),
                _ => {
                    self.error("WaitForSingleObject");
                    None
                }
            };
        }
    }
    #[repr(C)]
    struct Rect {
        l: i32,
        t: i32,
        r: i32,
        b: i32,
    }
    #[repr(C)]
    struct Entry {
        size: u32,
        usage: u32,
        pid: u32,
        heap: usize,
        module: u32,
        threads: u32,
        parent: u32,
        priority: i32,
        flags: u32,
        name: [u16; 260],
    }
    #[repr(C)]
    struct Key {
        vk: u16,
        scan: u16,
        flags: u32,
        time: u32,
        extra: usize,
    }
    #[repr(C)]
    union Data {
        key: std::mem::ManuallyDrop<Key>,
        padding: [u8; 32],
    }
    #[repr(C)]
    struct Input {
        kind: u32,
        data: Data,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
        fn Process32FirstW(h: Handle, e: *mut Entry) -> i32;
        fn Process32NextW(h: Handle, e: *mut Entry) -> i32;
        fn GetLastError() -> u32;
        fn CloseHandle(h: Handle) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn WaitForSingleObject(h: Handle, milliseconds: u32) -> u32;
        fn GetProcessTimes(
            h: Handle,
            created: *mut FileTime,
            exited: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
        fn QueryFullProcessImageNameW(
            h: Handle,
            flags: u32,
            buffer: *mut u16,
            len: *mut u32,
        ) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        fn SetProcessDPIAware() -> i32;
        fn EnumWindows(cb: unsafe extern "system" fn(Handle, isize) -> i32, param: isize) -> i32;
        fn IsWindowVisible(h: Handle) -> i32;
        fn GetWindowTextLengthW(h: Handle) -> i32;
        fn GetWindowTextW(h: Handle, buffer: *mut u16, len: i32) -> i32;
        fn GetWindowThreadProcessId(h: Handle, pid: *mut u32) -> u32;
        fn GetForegroundWindow() -> Handle;
        fn ShowWindow(h: Handle, cmd: i32) -> i32;
        fn SetForegroundWindow(h: Handle) -> i32;
        fn GetWindowRect(h: Handle, rect: *mut Rect) -> i32;
        fn SetCursorPos(x: i32, y: i32) -> i32;
        fn mouse_event(flags: u32, x: u32, y: u32, data: u32, extra: usize);
        fn MapVirtualKeyW(vk: u32, kind: u32) -> u32;
        fn PostMessageW(h: Handle, msg: u32, w: usize, l: isize) -> i32;
        fn SendInput(n: u32, input: *const Input, size: i32) -> u32;
    }
    struct Owned(Handle);
    impl Drop for Owned {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    pub struct WindowsBackend;
    impl WindowsBackend {
        pub fn new() -> Self {
            static DPI: Once = Once::new();
            DPI.call_once(|| unsafe {
                SetProcessDPIAware();
            });
            Self
        }
    }
    fn from_wide(raw: &[u16]) -> String {
        String::from_utf16_lossy(&raw[..raw.iter().position(|&x| x == 0).unwrap_or(raw.len())])
    }
    pub(crate) fn open_process(pid: u32, access: u32) -> Option<isize> {
        let h = unsafe { OpenProcess(access, 0, pid) };
        (h != 0).then_some(h)
    }
    pub(crate) fn with_process<T>(
        pid: u32,
        access: u32,
        f: impl FnOnce(isize) -> Result<T>,
    ) -> Result<T> {
        let guard = Owned(
            open_process(pid, access)
                .ok_or_else(|| Error::input(format!("Cannot open process {pid}")))?,
        );
        f(guard.0)
    }
    pub(crate) fn image_path(h: Handle) -> Result<String> {
        let mut buf = vec![0u16; 32768];
        let mut len = buf.len() as u32;
        if unsafe { QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len) } == 0 {
            return Err(Error::input("Cannot read Civ VI window executable"));
        }
        Ok(String::from_utf16_lossy(&buf[..len as usize]))
    }
    unsafe extern "system" fn enumerate(h: Handle, param: isize) -> i32 {
        if IsWindowVisible(h) == 0 {
            return 1;
        }
        let len = GetWindowTextLengthW(h);
        if len <= 0 {
            return 1;
        }
        let mut buf = vec![0u16; len as usize + 1];
        GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32);
        (*(param as *mut Vec<(isize, String)>)).push((h, from_wide(&buf)));
        1
    }
    fn snapshot_candidates(names: &BTreeSet<String>) -> Result<Vec<(u32, String)>> {
        let h = unsafe { CreateToolhelp32Snapshot(2, 0) };
        if h == 0 || h == -1 {
            return Err(Error::input("Could not enumerate Civ VI processes"));
        }
        let snapshot = Owned(h);
        let mut e = Entry {
            size: size_of::<Entry>() as u32,
            usage: 0,
            pid: 0,
            heap: 0,
            module: 0,
            threads: 0,
            parent: 0,
            priority: 0,
            flags: 0,
            name: [0; 260],
        };
        if unsafe { Process32FirstW(snapshot.0, &mut e) } == 0 {
            return Err(Error::input("Could not read Civ VI process list"));
        }
        let mut found = Vec::new();
        loop {
            let name = from_wide(&e.name);
            if matches_executable(&name, names) {
                found.push((e.pid, name));
            }
            if unsafe { Process32NextW(snapshot.0, &mut e) } == 0 {
                if unsafe { GetLastError() } != 18 {
                    return Err(Error::input("Civ VI process list became unavailable"));
                }
                break;
            }
        }
        Ok(found)
    }
    fn inspect_candidate(pid: u32, snapshot_image: String) -> ProcessCandidate {
        let mut row = ProcessCandidate::new(pid, snapshot_image);
        // Image, creation time and both exit checks use this one HANDLE.
        let handle = unsafe { OpenProcess(0x1000 | 0x0010_0000, 0, pid) };
        if handle == 0 {
            let error = std::io::Error::last_os_error();
            row.absent = error.raw_os_error() == Some(87);
            row.inspection_error = Some(format!("OpenProcess os_error={:?}", error.raw_os_error()));
            return row;
        }
        let handle = Owned(handle);
        row.observe_exit(handle.0);
        if row.exited.is_none() {
            return row;
        }
        let (mut created, mut exited, mut kernel, mut user) = (
            FileTime::default(),
            FileTime::default(),
            FileTime::default(),
            FileTime::default(),
        );
        if unsafe { GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) }
            == 0
        {
            row.error("GetProcessTimes");
        } else {
            row.created_filetime = Some((u64::from(created.high) << 32) | u64::from(created.low));
        }
        match image_path(handle.0) {
            Ok(path) => {
                // The inspected path is never included in diagnostics.
                row.image = path.rsplit(['\\', '/']).next().map(str::to_owned);
                row.image_matches_snapshot = row
                    .image
                    .as_ref()
                    .map(|image| image.eq_ignore_ascii_case(&row.snapshot_image));
            }
            Err(_) => row.error("QueryFullProcessImageNameW"),
        }
        row.observe_exit(handle.0);
        row
    }
    fn select_candidates(
        rows: &[ProcessCandidate],
        names: &BTreeSet<String>,
    ) -> Result<BTreeSet<u32>> {
        let mut found = BTreeSet::new();
        let mut failures = Vec::new();
        for row in rows {
            if row.exited == Some(true) || row.absent {
                continue;
            }
            let reason = if row.exited != Some(false) {
                Some("exit signal unavailable")
            } else if row.inspection_error.is_some() {
                Some("identity query failed while handle was not signaled")
            } else if row.created_filetime.is_none_or(|created| created == 0) {
                Some("creation FILETIME missing or zero")
            } else if row.image.as_ref().is_none_or(|image| {
                !matches_executable(image, names)
                    || !image.eq_ignore_ascii_case(&row.snapshot_image)
            }) {
                Some("snapshot and retained handle image differ or image is unapproved")
            } else {
                None
            };
            if let Some(reason) = reason {
                failures.push(serde_json::json!({"pid":row.pid,"reason":reason}));
            } else {
                found.insert(row.pid);
            }
        }
        let diagnostics =
            serde_json::json!({"candidates":rows,"validation_errors":failures}).to_string();
        if !failures.is_empty() {
            return Err(Error::input(format!(
                "Cannot verify process candidates; process_candidates={diagnostics}"
            )));
        }
        if found.len() > 1
            || rows
                .iter()
                .any(|row| row.exited == Some(true) || row.absent)
        {
            eprintln!("process_candidates={diagnostics}");
        }
        Ok(found)
    }
    fn process_ids_from_snapshot(
        candidates: Vec<(u32, String)>,
        names: &BTreeSet<String>,
    ) -> Result<BTreeSet<u32>> {
        let rows: Vec<_> = candidates
            .into_iter()
            .map(|(pid, name)| inspect_candidate(pid, name))
            .collect();
        select_candidates(&rows, names)
    }
    impl InputBackend for WindowsBackend {
        fn process_ids(&mut self, names: &BTreeSet<String>) -> Result<BTreeSet<u32>> {
            process_ids_from_snapshot(snapshot_candidates(names)?, names)
        }
        fn windows(&mut self) -> Result<Vec<(isize, String)>> {
            let mut out = Vec::new();
            if unsafe { EnumWindows(enumerate, &mut out as *mut _ as isize) } == 0 {
                return Err(Error::input("Could not verify Civ VI windows"));
            }
            Ok(out)
        }
        fn process_name(&mut self, h: isize) -> Result<String> {
            let mut pid = 0;
            if unsafe { GetWindowThreadProcessId(h, &mut pid) } == 0 || pid == 0 {
                return Err(Error::input("Cannot identify Civ VI window process"));
            }
            let path = with_process(pid, 0x1000, image_path)
                .map_err(|_| Error::input("Cannot read Civ VI window executable"))?;
            Ok(path
                .rsplit(['\\', '/'])
                .next()
                .unwrap_or(&path)
                .to_lowercase())
        }
        fn foreground(&mut self) -> isize {
            unsafe { GetForegroundWindow() }
        }
        fn restore(&mut self, h: isize) {
            unsafe {
                ShowWindow(h, 9);
            }
        }
        fn activate(&mut self, h: isize) -> bool {
            unsafe { SetForegroundWindow(h) != 0 }
        }
        fn rect(&mut self, h: isize) -> Result<(i32, i32, i32, i32)> {
            let mut r = Rect {
                l: 0,
                t: 0,
                r: 0,
                b: 0,
            };
            if unsafe { GetWindowRect(h, &mut r) } == 0 {
                return Err(Error::input("GetWindowRect failed"));
            }
            Ok((r.l, r.t, r.r, r.b))
        }
        fn cursor(&mut self, x: i32, y: i32) -> bool {
            unsafe { SetCursorPos(x, y) != 0 }
        }
        fn mouse(&mut self, flag: u32) {
            unsafe {
                mouse_event(flag, 0, 0, 0, 0);
            }
        }
        fn scan(&mut self, vk: u16) -> u32 {
            unsafe { MapVirtualKeyW(vk as u32, 0) }
        }
        fn post(&mut self, h: isize, msg: u32, w: usize, l: isize) -> bool {
            unsafe { PostMessageW(h, msg, w, l) != 0 }
        }
        fn send(&mut self, vk: u16, up: bool) -> bool {
            let input = Input {
                kind: 1,
                data: Data {
                    key: std::mem::ManuallyDrop::new(Key {
                        vk,
                        scan: 0,
                        flags: if up { 2 } else { 0 },
                        time: 0,
                        extra: 0,
                    }),
                },
            };
            unsafe { SendInput(1, &input, size_of::<Input>() as i32) == 1 }
        }
    }
    pub fn process_ids(names: &BTreeSet<String>) -> Result<BTreeSet<u32>> {
        WindowsBackend::new().process_ids(names)
    }
    pub fn process_name(h: isize) -> Result<String> {
        WindowsBackend::new().process_name(h)
    }
    pub fn find_civ_window() -> Result<isize> {
        find_civ_window_with(&mut WindowsBackend::new())
    }
    pub fn click_fraction(fx: f64, fy: f64) -> Result<(i32, i32)> {
        click_fraction_with(&mut WindowsBackend::new(), fx, fy)
    }
    pub fn press_key(vk: u16, end: Option<f64>) -> Result<()> {
        press_key_with(&mut WindowsBackend::new(), vk, end, KeyDelivery::Window)
    }
    pub fn request_civ_window_close() -> Result<()> {
        let mut lease = crate::startup::RecoveryLease::acquire()?;
        let game = crate::restart::single_civ_process(0.0)?
            .ok_or_else(|| Error::input("owned Civ6 process is absent"))?;
        crate::startup::request_owned_close(&mut lease, game.into())
    }
    pub fn require_single_civ_process() -> Result<()> {
        require_single_with(&mut WindowsBackend::new())
    }
    #[cfg(test)]
    mod snapshot_exit_tests {
        use super::*;
        use std::{
            io::Write,
            os::windows::{io::AsRawHandle, process::CommandExt},
            process::{Child, Command, Stdio},
        };
        use windows_sys::Win32::{
            Foundation::WAIT_OBJECT_0, System::Threading::WaitForSingleObject,
        };

        struct OwnedChild(Child);
        impl OwnedChild {
            fn new() -> Self {
                Self(
                    Command::new("cmd.exe")
                        .args([
                            "/d",
                            "/q",
                            "/c",
                            "set /p CC_SNAPSHOT_TEST=waiting & exit /b 259",
                        ])
                        .stdin(Stdio::piped())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .creation_flags(0x0800_0000)
                        .spawn()
                        .unwrap(),
                )
            }
            fn finish(&mut self) {
                self.0.stdin.take().unwrap().write_all(b"exit\r\n").unwrap();
                assert_eq!(
                    unsafe { WaitForSingleObject(self.0.as_raw_handle(), 5_000) },
                    WAIT_OBJECT_0,
                    "only the owned child must exit within the test bound"
                );
                assert_eq!(self.0.wait().unwrap().code(), Some(259));
            }
        }
        impl Drop for OwnedChild {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }

        fn owned_snapshot(pid: u32, names: &BTreeSet<String>) -> Vec<(u32, String)> {
            let candidates: Vec<_> = snapshot_candidates(names)
                .unwrap()
                .into_iter()
                .filter(|(candidate, _)| *candidate == pid)
                .collect();
            assert_eq!(
                candidates.len(),
                1,
                "live child must be in the actual OS snapshot"
            );
            candidates
        }

        #[test]
        fn snapshot_exited_child_with_retained_handle_is_not_a_live_candidate() {
            let names = BTreeSet::from(["cmd.exe".to_string()]);
            let mut child = OwnedChild::new();
            let pid = child.0.id();
            let candidates = owned_snapshot(pid, &names);
            assert_eq!(
                process_ids_from_snapshot(candidates.clone(), &names).unwrap(),
                BTreeSet::from([pid])
            );
            child.finish();
            let listed_after_exit = snapshot_candidates(&names)
                .unwrap()
                .iter()
                .any(|(id, _)| *id == pid);
            eprintln!("snapshot_exit_observation pid={pid} exit_code=259 retained_handle=true fresh_snapshot_contains_pid={listed_after_exit}");
            // The frozen snapshot definitely contains the child. Its strong
            // Child handle remains open, and exit 259 is not STILL_ACTIVE.
            assert!(process_ids_from_snapshot(candidates, &names)
                .unwrap()
                .is_empty());
        }

        #[test]
        fn snapshot_child_disappearing_before_inspection_is_not_a_live_candidate() {
            let names = BTreeSet::from(["cmd.exe".to_string()]);
            let mut child = OwnedChild::new();
            let pid = child.0.id();
            let candidates = owned_snapshot(pid, &names);
            child.finish();
            drop(child);
            assert!(process_ids_from_snapshot(candidates, &names)
                .unwrap()
                .is_empty());
        }

        fn live_candidate(pid: u32) -> ProcessCandidate {
            let mut row = ProcessCandidate::new(pid, "cmd.exe".into());
            row.image = Some("cmd.exe".into());
            row.created_filetime = Some(41);
            row.exited = Some(false);
            row.wait_result = Some(258);
            row
        }

        #[test]
        fn snapshot_two_live_owned_children_remain_two_candidates() {
            let names = BTreeSet::from(["cmd.exe".to_string()]);
            let mut first = OwnedChild::new();
            let mut second = OwnedChild::new();
            let expected = BTreeSet::from([first.0.id(), second.0.id()]);
            let candidates: Vec<_> = snapshot_candidates(&names)
                .unwrap()
                .into_iter()
                .filter(|(pid, _)| expected.contains(pid))
                .collect();
            assert_eq!(candidates.len(), 2);
            assert_eq!(
                process_ids_from_snapshot(candidates, &names).unwrap(),
                expected
            );
            first.finish();
            second.finish();
        }

        #[test]
        fn snapshot_unknown_denied_reused_or_missing_creation_never_becomes_absent() {
            let names = BTreeSet::from(["cmd.exe".to_string()]);
            for case in [
                "denied",
                "wait-error",
                "missing-time",
                "zero-time",
                "reused",
                "wrong-image",
            ] {
                let mut row = live_candidate(12);
                match case {
                    "denied" => {
                        row.exited = None;
                        row.created_filetime = None;
                        row.inspection_error = Some("OpenProcess os_error=Some(5)".into());
                    }
                    "wait-error" => {
                        row.exited = None;
                        row.wait_result = Some(u32::MAX);
                        row.inspection_error = Some("WaitForSingleObject os_error=Some(6)".into());
                    }
                    "missing-time" => row.created_filetime = None,
                    "zero-time" => row.created_filetime = Some(0),
                    "reused" => row.snapshot_image = "another.exe".into(),
                    "wrong-image" => row.image = Some("another.exe".into()),
                    _ => unreachable!(),
                }
                let error = select_candidates(&[live_candidate(11), row], &names).unwrap_err();
                assert!(error.message.contains("process_candidates="), "{case}");
                assert!(error.message.contains("\"pid\":12"), "{case}");
                assert!(error.message.contains("\"created_filetime\":"), "{case}");
                assert!(error.message.contains("\"exited\":"), "{case}");
            }
            assert_eq!(
                select_candidates(&[live_candidate(11), live_candidate(12)], &names).unwrap(),
                BTreeSet::from([11, 12])
            );
            let mut exited = live_candidate(12);
            exited.exited = Some(true);
            exited.wait_result = Some(0);
            // Once the same HANDLE is signaled, failed metadata is diagnostic.
            exited.inspection_error = Some("QueryFullProcessImageNameW os_error=Some(5)".into());
            assert_eq!(
                select_candidates(&[live_candidate(11), exited], &names).unwrap(),
                BTreeSet::from([11])
            );
        }
    }
}
#[cfg(windows)]
pub use win::{
    click_fraction, find_civ_window, press_key, process_ids, process_name,
    request_civ_window_close, require_single_civ_process,
};
#[cfg(windows)]
pub(crate) use win::{image_path, open_process, with_process};
#[cfg(not(windows))]
fn unsupported<T>() -> Result<T> {
    Err(Error::input("Civ VI window input requires Windows"))
}
#[cfg(not(windows))]
pub fn process_ids(_: &BTreeSet<String>) -> Result<BTreeSet<u32>> {
    unsupported()
}
#[cfg(not(windows))]
pub fn process_name(_: isize) -> Result<String> {
    unsupported()
}
#[cfg(not(windows))]
pub fn find_civ_window() -> Result<isize> {
    unsupported()
}
#[cfg(not(windows))]
pub fn click_fraction(_: f64, _: f64) -> Result<(i32, i32)> {
    unsupported()
}
#[cfg(not(windows))]
pub fn press_key(_: u16, _: Option<f64>) -> Result<()> {
    unsupported()
}
#[cfg(not(windows))]
pub fn request_civ_window_close() -> Result<()> {
    unsupported()
}
#[cfg(not(windows))]
pub fn require_single_civ_process() -> Result<()> {
    unsupported()
}
pub fn civ_process_ids() -> Result<BTreeSet<u32>> {
    process_ids(&civ_names())
}
