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
    match backend.process_ids(&civ_names())?.len() {
        0 => Err(Error::input(
            "FireTuner に接続できません（Civ6 が起動していない可能性）",
        )),
        1 => Ok(()),
        n => Err(Error::input(format!(
            "Multiple Civ VI processes found ({n}); driver requires one local game"
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
    if !backend.activate(hwnd) && backend.foreground() != hwnd {
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
    #[repr(C)]
    struct Rect {
        l: i32,
        t: i32,
        r: i32,
        b: i32,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Point {
        x: i32,
        y: i32,
    }
    fn titlebar_focus_point(window: &Rect, client: &Rect, origin: Point) -> Option<Point> {
        let width = window.r.checked_sub(window.l)?;
        let title_height = origin.y.checked_sub(window.t)?;
        if client.r <= client.l
            || client.b <= client.t
            || width < 100
            || !(16..=64).contains(&title_height)
        {
            return None;
        }
        Some(Point {
            x: window.l.checked_add(width / 2)?,
            y: window.t.checked_add(title_height / 2)?,
        })
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
        fn GetClientRect(h: Handle, rect: *mut Rect) -> i32;
        fn ClientToScreen(h: Handle, point: *mut Point) -> i32;
        fn WindowFromPoint(point: Point) -> Handle;
        fn GetAncestor(h: Handle, flags: u32) -> Handle;
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
    impl InputBackend for WindowsBackend {
        fn process_ids(&mut self, names: &BTreeSet<String>) -> Result<BTreeSet<u32>> {
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
            let mut found = BTreeSet::new();
            loop {
                if matches_executable(&from_wide(&e.name), names) {
                    found.insert(e.pid);
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
            if unsafe { GetForegroundWindow() } == h {
                return true;
            }
            if unsafe { SetForegroundWindow(h) } != 0 {
                return true;
            }
            let mut window = Rect {
                l: 0,
                t: 0,
                r: 0,
                b: 0,
            };
            let mut client = Rect {
                l: 0,
                t: 0,
                r: 0,
                b: 0,
            };
            let mut origin = Point { x: 0, y: 0 };
            if unsafe { GetWindowRect(h, &mut window) } == 0
                || unsafe { GetClientRect(h, &mut client) } == 0
                || unsafe { ClientToScreen(h, &mut origin) } == 0
            {
                return false;
            }
            let Some(point) = titlebar_focus_point(&window, &client, origin) else {
                return false;
            };
            let visible = || unsafe { GetAncestor(WindowFromPoint(point), 2) == h };
            if !visible() || !self.cursor(point.x, point.y) || !visible() {
                return false;
            }
            self.mouse(LEFT_DOWN);
            self.sleep(0.08);
            self.mouse(LEFT_UP);
            self.sleep(0.15);
            unsafe { GetForegroundWindow() == h }
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
        request_civ_window_close_with(&mut WindowsBackend::new())
    }
    pub fn require_single_civ_process() -> Result<()> {
        require_single_with(&mut WindowsBackend::new())
    }
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn titlebar_focus_requires_a_normal_nonclient_band() {
            let window = Rect {
                l: 0,
                t: 0,
                r: 1456,
                b: 933,
            };
            let client = Rect {
                l: 0,
                t: 0,
                r: 1456,
                b: 903,
            };
            assert_eq!(
                titlebar_focus_point(&window, &client, Point { x: 0, y: 30 }),
                Some(Point { x: 728, y: 15 })
            );
            assert_eq!(
                titlebar_focus_point(&window, &client, Point { x: 0, y: 0 }),
                None
            );
            assert_eq!(
                titlebar_focus_point(&window, &client, Point { x: 0, y: 100 }),
                None
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
