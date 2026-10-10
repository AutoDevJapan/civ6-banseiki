//! Windows controller of the Oath diagnostic runtime DLL (observation only).
//!
//! Loads the repository's own runtime DLL into the already pinned, fresh own
//! game, installs the diagnostic observer, starts it once, reads receipts and
//! stops it for good. It never calls game Lua, never changes the world and
//! never accepts a quantity: every value read back is observation metadata.
//!
//! Ownership rules kept here:
//! - the DLL file stays open with writes/deletes denied until the session
//!   object goes away, and its SHA256 and PE exports are measured from that
//!   very handle;
//! - the game handle is pinned (PID + exact FILETIME + image path) before and
//!   after every operation;
//! - a remote call is made once. If its end is not observed (timeout, exit
//!   code unreadable) the same pinned game is terminated; nothing is resent;
//! - remote allocations are never freed: the game outlives them, and the DLL
//!   pins itself for the process lifetime.
use crate::{
    click,
    error::{Error, Result},
    oath_wire as wire,
    startup::{GameProcess, RecoveryLease},
    time,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    ffi::c_void,
    fs::{File, OpenOptions},
    io::Read,
    mem::size_of,
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, FILETIME, HANDLE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{GetFinalPathNameByHandleW, FILE_NAME_NORMALIZED, VOLUME_NAME_DOS},
    System::{
        Diagnostics::{
            Debug::{ReadProcessMemory, WriteProcessMemory},
            ToolHelp::{
                CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W,
                TH32CS_SNAPMODULE,
            },
        },
        Memory::{
            VirtualAllocEx, VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, MEM_IMAGE,
            MEM_RESERVE, PAGE_READWRITE,
        },
        Threading::{
            CreateRemoteThread, GetExitCodeThread, GetProcessTimes, IsWow64Process, OpenProcess,
            TerminateProcess, WaitForSingleObject, PROCESS_QUERY_INFORMATION,
            PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
            PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
        },
    },
};

#[link(name = "kernel32")]
#[allow(non_snake_case)]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    fn GetModuleFileNameW(module: *mut c_void, name: *mut u16, size: u32) -> u32;
}

const KNOWN_EXE: &str = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/Base/Binaries/Win64Steam/CivilizationVI.exe";
const MAX_DLL_BYTES: u64 = 8 * 1024 * 1024;
const PROCESS_CREATE_THREAD: u32 = 0x0002;
const WAIT_TIMEOUT: u32 = 258;
const CALL_LIMIT: Duration = Duration::from_secs(8);
const KILL_LIMIT: Duration = Duration::from_secs(30);
const MAX_MODULES: usize = 4096;

struct Handle(HANDLE);
impl Handle {
    fn new(raw: HANDLE, what: &str) -> Result<Self> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            Err(win_error(what))
        } else {
            Ok(Self(raw))
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn win_error(what: &str) -> Error {
    Error::input(format!("oath controller {what}: Win32 error {}", unsafe {
        GetLastError()
    }))
}
fn path_eq(a: &Path, b: &Path) -> bool {
    a.as_os_str()
        .to_string_lossy()
        .eq_ignore_ascii_case(&b.as_os_str().to_string_lossy())
}
fn wide_text(bytes: &[u16]) -> Result<String> {
    let end = bytes
        .iter()
        .position(|c| *c == 0)
        .ok_or_else(|| Error::input("truncated module text"))?;
    String::from_utf16(&bytes[..end]).map_err(|_| Error::input("invalid module text"))
}
fn final_path(file: &File) -> Result<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    let length = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle() as HANDLE,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
        )
    } as usize;
    if length == 0 || length >= buffer.len() {
        return Err(win_error("final file path"));
    }
    Ok(PathBuf::from(
        String::from_utf16(&buffer[..length]).map_err(|_| Error::input("invalid final path"))?,
    ))
}
/// Opens read-only with writes and rename/delete denied for as long as the
/// handle lives, and proves the opened file is the requested path.
fn pinned_file(path: &Path) -> Result<(File, PathBuf)> {
    let file = OpenOptions::new().read(true).share_mode(1).open(path)?;
    let actual = final_path(&file)?;
    if !path_eq(&actual, &path.canonicalize()?) {
        return Err(Error::input("pinned file final path differs"));
    }
    Ok((file, actual))
}
fn creation(handle: HANDLE) -> Result<u64> {
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    if unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) } == 0 {
        return Err(win_error("GetProcessTimes"));
    }
    Ok((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}
fn unix_start(created: u64) -> f64 {
    created as f64 / 10_000_000.0 - 11_644_473_600.0
}
fn display_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

/// The repository's own DLL as measured from one pinned file handle.
struct LocalDll {
    _file: File,
    path: PathBuf,
    bytes: Vec<u8>,
    sha: String,
    image: wire::PeImage,
    exports: [u32; 4],
}
impl LocalDll {
    #[cfg(test)]
    fn open(path: &Path, expected_sha: &str) -> Result<Self> {
        Self::open_mode(path, expected_sha, false)
    }
    fn open_mode(path: &Path, expected_sha: &str, lifecycle: bool) -> Result<Self> {
        if !wire::lower_hex_sha(expected_sha) {
            return Err(Error::input(
                "expected DLL SHA256 must be 64 lowercase hex digits",
            ));
        }
        let (mut file, path) = pinned_file(path)?;
        let length = file.metadata()?.len();
        if length == 0 || length > MAX_DLL_BYTES {
            return Err(Error::input("runtime DLL size is outside 1..=8 MiB"));
        }
        let mut bytes = Vec::with_capacity(length as usize);
        (&mut file)
            .take(MAX_DLL_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 != length {
            return Err(Error::input("runtime DLL changed while it was read"));
        }
        let sha = format!("{:x}", Sha256::digest(&bytes));
        if sha != expected_sha {
            return Err(Error::input(
                "runtime DLL SHA256 differs from the expected value",
            ));
        }
        let image = wire::parse_pe(&bytes).map_err(Error::input)?;
        let exports = if lifecycle {
            let mut exports = [0; 4];
            for (index, name) in crate::oath_lifecycle_wire::EXPORTS.iter().enumerate() {
                exports[index] = image
                    .export(name)
                    .ok_or_else(|| Error::input(format!("missing lifecycle export {name}")))?;
                if !image.rva_is_executable(exports[index])
                    || image.bytes_at(&bytes, exports[index], 32).is_none()
                {
                    return Err(Error::input("lifecycle export outside measured file"));
                }
            }
            exports
        } else {
            wire::runtime_exports(&image).map_err(Error::input)?
        };
        Ok(Self {
            _file: file,
            path,
            bytes,
            sha,
            image,
            exports,
        })
    }
}

#[derive(Clone)]
struct Module {
    path: PathBuf,
    base: usize,
    size: usize,
}

/// One pinned live process and the remote-call discipline around it.
struct Target {
    process: Handle,
    pid: u32,
    created: u64,
    exe: Option<PathBuf>,
    /// A remote thread was created and its end has not been observed.
    flight: bool,
    poisoned: bool,
    /// A termination was attempted and not observed to complete.
    unconfirmed: bool,
    fail_terminate: bool,
    call_limit: Duration,
    kill_limit: Duration,
    /// Remote allocations, kept for the life of the game and never freed.
    allocations: Vec<(usize, usize)>,
}
// The raw handle is used from one thread at a time through &mut self.
unsafe impl Send for Target {}

impl Target {
    fn alive_pinned(&self) -> Result<()> {
        if creation(self.process.0)? != self.created
            || unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_TIMEOUT
        {
            return Err(Error::input(
                "owned process handle is no longer the pinned live process",
            ));
        }
        if let Some(exe) = &self.exe {
            let current =
                PathBuf::from(click::image_path(self.process.0 as isize)?).canonicalize()?;
            if !path_eq(&current, exe) {
                return Err(Error::input("pinned exe real path changed"));
            }
        }
        Ok(())
    }

    fn alloc(&mut self, length: usize) -> Result<usize> {
        self.alive_pinned()?;
        let address = unsafe {
            VirtualAllocEx(
                self.process.0,
                std::ptr::null(),
                length,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        } as usize;
        if address == 0 {
            return Err(win_error("VirtualAllocEx"));
        }
        self.allocations.push((address, length));
        Ok(address)
    }
    fn write(&self, address: usize, bytes: &[u8]) -> Result<()> {
        self.alive_pinned()?;
        let mut written = 0usize;
        if unsafe {
            WriteProcessMemory(
                self.process.0,
                address as *const c_void,
                bytes.as_ptr().cast(),
                bytes.len(),
                &mut written,
            )
        } == 0
            || written != bytes.len()
        {
            return Err(win_error("WriteProcessMemory"));
        }
        Ok(())
    }
    fn read(&self, address: usize, length: usize) -> Result<Vec<u8>> {
        if length == 0 || length > wire::MAX_TRACE_READ {
            return Err(Error::input("remote read length is outside the finite cap"));
        }
        self.alive_pinned()?;
        let mut out = vec![0u8; length];
        let mut read = 0usize;
        if unsafe {
            ReadProcessMemory(
                self.process.0,
                address as *const c_void,
                out.as_mut_ptr().cast(),
                length,
                &mut read,
            )
        } == 0
            || read != length
        {
            return Err(win_error("ReadProcessMemory (complete copy required)"));
        }
        self.alive_pinned()?;
        Ok(out)
    }

    /// True while the end of a remote thread or of a termination is not
    /// observed: handle, files and allocations must then be kept.
    fn unresolved(&self) -> bool {
        self.flight || self.unconfirmed
    }

    /// Terminates the same pinned process (handle plus original FILETIME,
    /// never a PID lookup) after a call whose end is unknown. Only the
    /// process object's own signal confirms the end.
    fn kill(&mut self) -> bool {
        self.poisoned = true;
        // A handle that no longer shows the pinned creation time is not
        // proof of anything: nothing is killed and nothing is confirmed.
        let pinned = creation(self.process.0).ok() == Some(self.created);
        let pinned = pinned && !self.fail_terminate;
        let confirmed = pinned && {
            unsafe {
                TerminateProcess(self.process.0, 1);
            }
            let wait = self.kill_limit.as_millis().min(u128::from(u32::MAX - 1)) as u32;
            let ended = unsafe { WaitForSingleObject(self.process.0, wait) };
            ended == 0
        };
        self.unconfirmed = !confirmed;
        if confirmed {
            self.flight = false;
        }
        confirmed
    }

    fn kill_report(&mut self, why: &str) -> Error {
        if self.kill() {
            Error::unknown(format!("{why}; the pinned game was terminated"))
        } else {
            Error::unknown(format!(
                "{why}; termination of the pinned game is NOT confirmed, ownership is retained (retention=known_pending)"
            ))
        }
    }

    /// A completed runtime call can still report unresolved hook writes.
    /// End this same owned process rather than leave a failed session with
    /// published entrances, or repeat any remote operation blindly.
    fn runtime_refusal(&mut self, operation: &str, status: u32) -> Error {
        let why = format!(
            "{operation} refused: {} ({status})",
            wire::status_name(status)
        );
        if matches!(status, 6 | 99) {
            self.kill_report(&why)
        } else {
            Error::input(why)
        }
    }

    fn verify_after_install<T>(&mut self, check: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        // Install has already published entrances. A malformed receipt or
        // unsupported trace cannot be treated like a pre-install refusal.
        match check(self) {
            Ok(value) => Ok(value),
            Err(error) => {
                Err(self.kill_report(&format!("installed observer validation failed: {error}")))
            }
        }
    }

    /// Observes, without any new termination, whether the pinned process
    /// object has signalled. Only the same handle with the original creation
    /// time counts; anything else leaves the obligation open.
    fn observe_end(&mut self) -> bool {
        if !self.unresolved() {
            return true;
        }
        if creation(self.process.0).ok() != Some(self.created) {
            return false;
        }
        if unsafe { WaitForSingleObject(self.process.0, 0) } == 0 {
            self.flight = false;
            self.unconfirmed = false;
            return true;
        }
        false
    }

    /// Runs one remote thread to completion exactly once. No retry exists: a
    /// call whose end is unknown ends the game instead of being repeated.
    fn call(&mut self, start: usize, argument: usize) -> Result<u32> {
        if self.poisoned || self.flight {
            return Err(Error::input(
                "remote call refused: an earlier call is unresolved",
            ));
        }
        time::check_interrupt()?;
        self.alive_pinned()?;
        self.flight = true;
        let routine: Option<unsafe extern "system" fn(*mut c_void) -> u32> = Some(unsafe {
            std::mem::transmute::<usize, unsafe extern "system" fn(*mut c_void) -> u32>(start)
        });
        let mut thread_id = 0u32;
        let raw = unsafe {
            CreateRemoteThread(
                self.process.0,
                std::ptr::null(),
                0,
                routine,
                argument as *const c_void,
                0,
                &mut thread_id,
            )
        };
        if raw.is_null() {
            // No thread exists, so nothing can be in flight.
            self.flight = false;
            return Err(win_error("CreateRemoteThread"));
        }
        let thread = Handle(raw);
        let wait = self.call_limit.as_millis().min(u128::from(u32::MAX - 1)) as u32;
        if unsafe { WaitForSingleObject(thread.0, wait) } != 0 {
            return Err(self.kill_report("remote call did not finish in time"));
        }
        let mut code = 0u32;
        if unsafe { GetExitCodeThread(thread.0, &mut code) } == 0 {
            return Err(self.kill_report("remote call exit code is unreadable"));
        }
        self.flight = false;
        self.alive_pinned()?;
        Ok(code)
    }

    fn modules(&self) -> Result<Vec<Module>> {
        self.alive_pinned()?;
        let snapshot = Handle::new(
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE, self.pid) },
            "module snapshot",
        )?;
        let mut row = MODULEENTRY32W {
            dwSize: size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };
        let mut out = Vec::new();
        let mut more = unsafe { Module32FirstW(snapshot.0, &mut row) } != 0;
        while more {
            if out.len() >= MAX_MODULES {
                return Err(Error::input("module enumeration exceeded the finite limit"));
            }
            out.push(Module {
                path: PathBuf::from(wide_text(&row.szExePath)?),
                base: row.modBaseAddr as usize,
                size: row.modBaseSize as usize,
            });
            more = unsafe { Module32NextW(snapshot.0, &mut row) } != 0;
        }
        if unsafe { GetLastError() } != 18 {
            return Err(win_error("incomplete module enumeration"));
        }
        Ok(out)
    }
}
impl Drop for Target {
    fn drop(&mut self) {
        // Never leave an unobserved remote thread running against memory that
        // this controller is about to forget. If the end still cannot be
        // observed, the process handle is kept open (leaked) as the receipt.
        if self.unresolved() && !self.kill() {
            let kept = std::mem::replace(&mut self.process, Handle(std::ptr::null_mut()));
            std::mem::forget(kept);
        }
    }
}

/// Everything an unresolved game obligation keeps alive: the pinned process
/// handle and remote allocations (in the target), and the lease, exe and DLL
/// file pins.
struct Held {
    target: Target,
    #[allow(dead_code)]
    keep: Vec<Box<dyn std::any::Any>>,
}
// Used from one thread at a time under the PENDING lock.
unsafe impl Send for Held {}

static PENDING: std::sync::Mutex<Vec<Held>> = std::sync::Mutex::new(Vec::new());

fn retain(held: Held) {
    PENDING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .push(held);
}

/// Releases retained owners whose process has been observed to end on the
/// same handle. Called by the driver at an operation start or tick; returns
/// how many are still pending. Nothing here terminates a process.
pub(crate) fn drain_pending() -> usize {
    let mut pending = PENDING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    pending.retain_mut(|held| !held.target.observe_end());
    pending.len()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn pending_count() -> usize {
    PENDING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .len()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Installed,
    Started,
    Stopped,
    Failed,
}

/// An installed diagnostic observer in one pinned game. Dropping the session
/// neither stops the observer nor repeats any call; the DLL stays pinned in
/// the game and the observer ends by its own deadline.
struct Inner {
    target: Target,
    _lease: RecoveryLease,
    _exe_file: File,
    _dll: LocalDll,
    exports: [usize; 4],
    module_base: usize,
    trace_address: usize,
    receipt_buffer: usize,
    max_seconds: u32,
    phase: Phase,
    stop_value: Option<Value>,
    callee: bool,
    lifecycle: bool,
}
unsafe impl Send for Inner {}

/// Public handle. If the end of a remote call or of a termination was never
/// observed, dropping it keeps the pinned process handle, the DLL and exe file
/// pins, the lease and the remote allocations (they are leaked on purpose and
/// reported as unresolved) instead of claiming a clean close.
pub(crate) struct Session {
    inner: std::mem::ManuallyDrop<Inner>,
}

impl Session {
    #[cfg(test)]
    pub(crate) fn install(
        game: GameProcess,
        dll_path: &Path,
        expected_sha: &str,
        max_seconds: u32,
    ) -> Result<Self> {
        Self::install_capture(game, dll_path, expected_sha, max_seconds, false)
    }
    #[cfg(test)]
    pub(crate) fn install_capture(
        game: GameProcess,
        dll_path: &Path,
        expected_sha: &str,
        max_seconds: u32,
        callee: bool,
    ) -> Result<Self> {
        Self::install_mode(game, dll_path, expected_sha, max_seconds, callee, false)
    }
    pub(crate) fn install_mode(
        game: GameProcess,
        dll_path: &Path,
        expected_sha: &str,
        max_seconds: u32,
        callee: bool,
        lifecycle: bool,
    ) -> Result<Self> {
        Inner::install(game, dll_path, expected_sha, max_seconds, callee, lifecycle).map(|inner| {
            Self {
                inner: std::mem::ManuallyDrop::new(inner),
            }
        })
    }
    pub(crate) fn identity(&self) -> Value {
        self.inner.identity()
    }
    pub(crate) fn start(&mut self) -> Result<Value> {
        self.inner.start()
    }
    pub(crate) fn snapshot(&mut self) -> Result<Value> {
        self.inner.snapshot()
    }
    pub(crate) fn stop(&mut self) -> Result<Value> {
        self.inner.stop()
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        // One last attempt on the same pinned handle. Without a confirmed
        // end every pin is handed to the pending list, not released.
        if self.inner.target.unresolved() && !self.inner.target.kill() {
            let inner = unsafe { std::mem::ManuallyDrop::take(&mut self.inner) };
            let Inner {
                target,
                _lease,
                _exe_file,
                _dll,
                ..
            } = inner;
            retain(Held {
                target,
                keep: vec![Box::new(_lease), Box::new(_exe_file), Box::new(_dll)],
            });
        } else {
            unsafe { std::mem::ManuallyDrop::drop(&mut self.inner) };
        }
    }
}

impl Inner {
    /// Loads and installs the observer. The expected SHA is only compared with
    /// the file this process measures; it authorizes nothing by itself.
    pub(crate) fn install(
        game: GameProcess,
        dll_path: &Path,
        expected_sha: &str,
        max_seconds: u32,
        callee: bool,
        lifecycle: bool,
    ) -> Result<Self> {
        if callee && lifecycle {
            return Err(Error::input("incompatible oath capture modes"));
        }
        if game.pid == 0
            || game.pid == std::process::id()
            || !game.started.is_finite()
            || game.started <= 0.0
        {
            return Err(Error::input(
                "invalid caller-pinned game process (own process refused)",
            ));
        }
        drain_pending();
        if !(1..=wire::MAX_SECONDS).contains(&max_seconds) {
            return Err(Error::input("max_seconds is outside 1..=300"));
        }
        // Everything about the DLL is settled before the game is touched.
        let dll = LocalDll::open_mode(dll_path, expected_sha, lifecycle)?;
        let mut lease = RecoveryLease::acquire()?;
        lease.check()?;
        if click::civ_process_ids()?
            .iter()
            .copied()
            .collect::<Vec<_>>()
            != [game.pid]
        {
            return Err(Error::input(
                "the oath observer needs the single pinned Civ6",
            ));
        }
        let access = PROCESS_QUERY_LIMITED_INFORMATION
            | PROCESS_QUERY_INFORMATION
            | PROCESS_SYNCHRONIZE
            | PROCESS_TERMINATE
            | PROCESS_VM_OPERATION
            | PROCESS_VM_READ
            | PROCESS_VM_WRITE
            | PROCESS_CREATE_THREAD;
        let process = Handle::new(unsafe { OpenProcess(access, 0, game.pid) }, "OpenProcess")?;
        let created = creation(process.0)?;
        if unix_start(created) != game.started
            || unsafe { WaitForSingleObject(process.0, 0) } != WAIT_TIMEOUT
        {
            return Err(Error::input("pinned game PID/creation time changed"));
        }
        let image = PathBuf::from(click::image_path(process.0 as isize)?).canonicalize()?;
        let (exe_file, exe) = pinned_file(Path::new(KNOWN_EXE))?;
        if !path_eq(&image, &exe) {
            return Err(Error::input(
                "game exe real path is not the audited installation",
            ));
        }
        let mut wow64 = 0;
        if unsafe { IsWow64Process(process.0, &mut wow64) } == 0 || wow64 != 0 {
            return Err(Error::input("target is not native Win64"));
        }
        let mut target = Target {
            process,
            pid: game.pid,
            created,
            exe: Some(exe),
            flight: false,
            poisoned: false,
            unconfirmed: false,
            fail_terminate: false,
            call_limit: CALL_LIMIT,
            kill_limit: KILL_LIMIT,
            allocations: Vec::new(),
        };
        let built = bring_up(
            &mut target,
            &dll,
            game.pid,
            max_seconds,
            created,
            callee,
            lifecycle,
        );
        let (base, exports, trace_address, receipt_buffer) = match built {
            Ok(done) => done,
            Err(error) => {
                if target.unresolved() {
                    // The game may still run our thread: keep every pin.
                    retain(Held {
                        target,
                        keep: vec![Box::new(lease), Box::new(exe_file), Box::new(dll)],
                    });
                }
                return Err(error);
            }
        };
        Ok(Self {
            target,
            _lease: lease,
            _exe_file: exe_file,
            exports,
            module_base: base,
            trace_address: trace_address as usize,
            receipt_buffer,
            max_seconds,
            phase: Phase::Installed,
            stop_value: None,
            _dll: dll,
            callee,
            lifecycle,
        })
    }

    /// Facts proven at install, for the scenario report.
    pub(crate) fn identity(&self) -> Value {
        json!({
            "pid": self.target.pid,
            "created_filetime": self.target.created,
            "dll_sha256": self._dll.sha,
            "dll_path": display_path(&self._dll.path),
            "module_base": format!("0x{:x}", self.module_base),
            "trace_address": format!("0x{:x}", self.trace_address),
            "trace_size": self.trace_size(),
            "capture":if self.lifecycle { "lifecycle" } else if self.callee { "callee" } else { "adapter" },
            "max_seconds": self.max_seconds,
            "quantity_accepted": false,
        })
    }

    fn status_value(&self, action: &str, status: u32) -> Value {
        json!({
            "action": action,
            "status": status,
            "status_name": wire::status_name(status),
            "quantity_accepted": false,
            "accepted": false,
        })
    }
    fn trace_size(&self) -> usize {
        if self.lifecycle {
            crate::oath_lifecycle_wire::SIZE
        } else if self.callee {
            crate::oath_recipient_wire::SIZE
        } else {
            wire::TRACE_SIZE
        }
    }

    /// Starts the one observation window. A second start is refused locally
    /// without any remote call.
    pub(crate) fn start(&mut self) -> Result<Value> {
        if self.phase != Phase::Installed {
            return Err(Error::input(format!(
                "start refused in phase {:?}: the window opens once",
                self.phase
            )));
        }
        let status = self.call(1)?;
        if status != 0 {
            self.phase = Phase::Failed;
            return Err(self
                .target
                .runtime_refusal("OathRuntimeStartObservation", status));
        }
        self.phase = Phase::Started;
        Ok(self.status_value("start", status))
    }

    /// Stops for good. Repeating it returns the first result and calls nothing.
    pub(crate) fn stop(&mut self) -> Result<Value> {
        if self.phase == Phase::Stopped {
            let mut repeated = self.stop_value.clone().unwrap_or(Value::Null);
            repeated["repeated"] = json!(true);
            return Ok(repeated);
        }
        if !matches!(self.phase, Phase::Installed | Phase::Started) {
            return Err(Error::input("stop refused: the observer failed earlier"));
        }
        let status = self.call(2)?;
        if status != 0 {
            self.phase = Phase::Failed;
            return Err(self
                .target
                .runtime_refusal("OathRuntimeStopObservation", status));
        }
        self.phase = Phase::Stopped;
        let value = self.status_value("stop", status);
        self.stop_value = Some(value.clone());
        Ok(value)
    }

    fn call(&mut self, export: usize) -> Result<u32> {
        let argument = if export == 3 { self.receipt_buffer } else { 0 };
        self.target.call(self.exports[export], argument)
    }

    /// Receipt plus two raw copies of the trace. While recording, rows that
    /// differ between the copies are Unknown. After a stop the call waits,
    /// within the call limit and without further remote calls, for nothing to
    /// be in flight; if that does not happen the result says it is incomplete.
    pub(crate) fn snapshot(&mut self) -> Result<Value> {
        if self.phase == Phase::Failed {
            return Err(Error::input(
                "snapshot refused: the observer failed earlier",
            ));
        }
        let status = self.call(3)?;
        if status != 0 {
            self.phase = Phase::Failed;
            return Err(self
                .target
                .runtime_refusal("OathRuntimeReadReceipt", status));
        }
        if self.lifecycle {
            let first = self.target.read(self.trace_address, self.trace_size())?;
            let second = self.target.read(self.trace_address, self.trace_size())?;
            let mut value =
                crate::oath_lifecycle_wire::project(&first, &second).map_err(Error::input)?;
            value["controller_phase"] = json!(format!("{:?}", self.phase));
            value["trace_address"] = json!(format!("0x{:x}", self.trace_address));
            return Ok(value);
        }
        let receipt_bytes = self.target.read(self.receipt_buffer, wire::RECEIPT_SIZE)?;
        if wire::u32_at(&receipt_bytes, 12) != Some(u32::from(self.callee)) {
            return Err(Error::input("receipt capture differs from install"));
        }
        let receipt = wire::decode_receipt(&receipt_bytes).map_err(Error::input)?;
        if receipt.trace_address != self.trace_address as u64
            || receipt.trace_size as usize != self.trace_size()
            || receipt.max_seconds != self.max_seconds
        {
            return Err(Error::input(
                "receipt describes a different trace than the install",
            ));
        }
        // 1 installed, 2 started, 3 stopped. The runtime may stop by itself at
        // its deadline, which only moves the phase forward.
        let expected = match self.phase {
            Phase::Installed => 1,
            Phase::Started => 2,
            Phase::Stopped => 3,
            Phase::Failed => 4,
        };
        if receipt.phase != expected && !(self.phase == Phase::Started && receipt.phase == 3) {
            return Err(Error::input(format!(
                "receipt phase {} does not match controller phase {:?}",
                receipt.phase, self.phase
            )));
        }
        let stopped = receipt.phase == 3;
        let until = Instant::now() + self.target.call_limit;
        loop {
            time::check_interrupt()?;
            if self.callee {
                let first = self.target.read(self.trace_address, self.trace_size())?;
                let second = self.target.read(self.trace_address, self.trace_size())?;
                let mut value = crate::oath_recipient_wire::project(&first, &second, &receipt)
                    .map_err(Error::input)?;
                if !stopped
                    || (first == second && value["in_flight"] == 0)
                    || Instant::now() >= until
                {
                    value["controller_phase"] = json!(format!("{:?}", self.phase));
                    value["trace_address"] = json!(format!("0x{:x}", self.trace_address));
                    return Ok(value);
                }
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            let first =
                wire::TraceView::parse(&self.target.read(self.trace_address, wire::TRACE_SIZE)?)
                    .map_err(Error::input)?;
            let second =
                wire::TraceView::parse(&self.target.read(self.trace_address, wire::TRACE_SIZE)?)
                    .map_err(Error::input)?;
            let quiet = first == second && second.in_flight() == 0;
            if !stopped || quiet || Instant::now() >= until {
                let mut value = wire::project_trace(&first, &second, &receipt);
                value["controller_phase"] = json!(format!("{:?}", self.phase));
                value["trace_address"] = json!(format!("0x{:x}", self.trace_address));
                return Ok(value);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

/// Local owner of a function address: VirtualQuery (must be MEM_IMAGE), the
/// allocation base's file name and its SizeOfImage.
fn local_owner(function: usize) -> Result<wire::ModuleFact> {
    let mut info = MEMORY_BASIC_INFORMATION::default();
    if unsafe {
        VirtualQuery(
            function as *const c_void,
            &mut info,
            size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    } == 0
        || info.Type != MEM_IMAGE
        || info.AllocationBase.is_null()
    {
        return Err(Error::input(
            "function address is not inside a mapped image",
        ));
    }
    let base = info.AllocationBase as usize;
    let mut buffer = vec![0u16; 32768];
    let length = unsafe {
        GetModuleFileNameW(
            base as *mut c_void,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
        )
    } as usize;
    if length == 0 || length >= buffer.len() {
        return Err(win_error("GetModuleFileNameW of the function owner"));
    }
    let path = PathBuf::from(
        String::from_utf16(&buffer[..length]).map_err(|_| Error::input("invalid owner path"))?,
    )
    .canonicalize()?;
    // SizeOfImage from the mapped headers of this very module.
    let pe = unsafe { std::ptr::read_unaligned((base + 0x3c) as *const u32) } as usize;
    let size = unsafe { std::ptr::read_unaligned((base + pe + 24 + 56) as *const u32) } as usize;
    Ok(wire::ModuleFact {
        path: display_path(&path),
        base,
        size,
    })
}

fn remote_facts(modules: &[Module]) -> Vec<wire::ModuleFact> {
    modules
        .iter()
        .map(|m| wire::ModuleFact {
            path: m
                .path
                .canonicalize()
                .map(|p| display_path(&p))
                .unwrap_or_else(|_| display_path(&m.path)),
            base: m.base,
            size: m.size,
        })
        .collect()
}

/// A local function address may be sent to the target only if its real owner
/// module (not the module it was looked up in) is the same file at the same
/// base remotely, and the first 32 bytes of the function agree.
fn verify_function_owner(target: &Target, function: usize) -> Result<()> {
    let owner = local_owner(function)?;
    let modules = target.modules()?;
    let facts = remote_facts(&modules);
    wire::function_owner_in_remote(function, &owner, &facts).map_err(Error::input)?;
    let local = unsafe { std::slice::from_raw_parts(function as *const u8, 32) };
    let remote = target.read(function, 32)?;
    if remote != local {
        // Code only: retain the finite evidence before any remote call. An
        // address match is not a reason to silently ignore modified code.
        let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let jump_target = if remote[0] == 0xe9 {
            let displacement = i32::from_le_bytes(remote[1..5].try_into().unwrap());
            function
                .checked_add(5)
                .and_then(|next| next.checked_add_signed(displacement as isize))
        } else if remote[..6] == [0xff, 0x25, 0, 0, 0, 0] {
            usize::try_from(u64::from_le_bytes(remote[6..14].try_into().unwrap())).ok()
        } else {
            None
        };
        let jump_owner = jump_target.and_then(|address| {
            facts.iter().find(|m| {
                address >= m.base && address.checked_sub(m.base).is_some_and(|rva| rva < m.size)
            })
        });
        return Err(Error::input(format!(
            "remote function bytes differ from the local function; evidence={}",
            json!({"owner":owner.path,"rva":function-owner.base,"local_code":hex(local),
                "remote_code":hex(&remote),"jump_target":jump_target,"jump_owner":jump_owner.map(|m| &m.path)})
        )));
    }
    Ok(())
}

/// Resolve the local documented entry's import to the same named KernelBase
/// implementation. The remote kernel32 entry can be intercepted; we never
/// follow that unknown redirect. The implementation still needs the full
/// remote owner/address/code proof before it is called.
fn local_load_library() -> Result<usize> {
    let local_kernel32 = unsafe { GetModuleHandleW(wide("kernel32.dll").as_ptr()) } as usize;
    let load_library = unsafe {
        GetProcAddress(
            local_kernel32 as *mut c_void,
            c"LoadLibraryW".as_ptr().cast(),
        )
    } as usize;
    if local_kernel32 == 0 || load_library == 0 {
        return Err(Error::input("local LoadLibraryW is unavailable"));
    }
    let owner = local_owner(load_library)?;
    if load_library
        .checked_add(7)
        .is_none_or(|end| end > owner.base.saturating_add(owner.size))
    {
        return Err(Error::input("local LoadLibraryW entry leaves its image"));
    }
    let code = unsafe { std::slice::from_raw_parts(load_library as *const u8, 7) };
    let Some(slot) = wire::local_import_slot(load_library, &owner, code).map_err(Error::input)?
    else {
        return Ok(load_library);
    };
    let destination = unsafe { std::ptr::read_unaligned(slot as *const usize) };
    let kernelbase = unsafe { GetModuleHandleW(wide("KernelBase.dll").as_ptr()) };
    if kernelbase.is_null() {
        return Err(Error::input("local KernelBase is unavailable"));
    }
    let implementation =
        unsafe { GetProcAddress(kernelbase, c"LoadLibraryW".as_ptr().cast()) } as usize;
    if implementation == 0
        || destination != implementation
        || local_owner(implementation)?.base != kernelbase as usize
    {
        return Err(Error::input(
            "local import is not the same named KernelBase LoadLibraryW implementation",
        ));
    }
    Ok(implementation)
}

/// Fresh remote load of the pinned DLL; returns its verified base.
fn load_dll(target: &mut Target, dll: &LocalDll) -> Result<usize> {
    let load_library = local_load_library()?;
    verify_function_owner(target, load_library)?;
    let ours = display_path(&dll.path);
    let file_name = dll
        .path
        .file_name()
        .map(|d| d.to_string_lossy().into_owned())
        .unwrap_or_default();
    let before = remote_facts(&target.modules()?);
    if before.iter().any(|m| {
        m.path.eq_ignore_ascii_case(&ours)
            || m.path
                .rsplit(['\\', '/'])
                .next()
                .is_some_and(|n| n.eq_ignore_ascii_case(&file_name))
    }) {
        return Err(Error::input(
            "the runtime DLL is already loaded: a fresh game is required",
        ));
    }
    let path_text = wide(&ours);
    let path_bytes: Vec<u8> = path_text.iter().flat_map(|c| c.to_le_bytes()).collect();
    let remote_path = target.alloc(path_bytes.len())?;
    target.write(remote_path, &path_bytes)?;
    // From here until the module is proven, any return leaves the target
    // poisoned: the load may have happened whatever the call reported.
    let result = target.call(load_library, remote_path);
    target.poisoned = true;
    let low = result?;
    let matching: Vec<wire::ModuleFact> = remote_facts(&target.modules()?)
        .into_iter()
        .filter(|m| m.path.eq_ignore_ascii_case(&ours))
        .collect();
    match wire::judge_load(low, dll.image.size_of_image as usize, &matching) {
        wire::LoadVerdict::Loaded(base) => {
            target.alive_pinned()?;
            target.poisoned = false;
            Ok(base)
        }
        wire::LoadVerdict::Unknown(why) => Err(target.kill_report(why)),
    }
}

/// The mapped image must still show the measured file's headers and the first
/// bytes of every export.
fn verify_remote_image(
    target: &Target,
    dll: &LocalDll,
    base: usize,
    exports: &[usize; 4],
) -> Result<()> {
    let head = target.read(base, dll.image.size_of_headers as usize)?;
    if !wire::header_matches(&dll.bytes, &head, &dll.image) {
        return Err(Error::input(
            "remote image headers differ from the measured DLL",
        ));
    }
    for (address, rva) in exports.iter().zip(dll.exports) {
        let remote = target.read(*address, 32)?;
        let disk = dll
            .image
            .bytes_at(&dll.bytes, rva, 32)
            .ok_or_else(|| Error::input("export bytes are outside the file"))?;
        if remote != disk {
            return Err(Error::input(
                "remote export code differs from the measured DLL",
            ));
        }
    }
    Ok(())
}

type BroughtUp = (usize, [usize; 4], u64, usize);

/// Load, verify and install; every step after the load runs on the poisoned-
/// until-proven target.
fn bring_up(
    target: &mut Target,
    dll: &LocalDll,
    pid: u32,
    max_seconds: u32,
    created: u64,
    callee: bool,
    lifecycle: bool,
) -> Result<BroughtUp> {
    let base = load_dll(target, dll)?;
    let exports = dll.exports.map(|rva| base + rva as usize);
    verify_remote_image(target, dll, base, &exports)?;
    let mut config = wire::encode_config(pid, max_seconds, created);
    config[28..32].copy_from_slice(&u32::from(callee).to_le_bytes());
    let config_at = target.alloc(wire::CONFIG_SIZE)?;
    target.write(config_at, &config)?;
    let status = target.call(exports[0], config_at)?;
    if status != 0 {
        return Err(target.runtime_refusal("OathRuntimeInstall", status));
    }
    target.verify_after_install(|target| {
        let after = target.read(config_at, wire::CONFIG_SIZE)?;
        let (trace_address, _) = if lifecycle {
            if after[..32] != config[..32]
                || after[44..48] != config[44..48]
                || wire::u32_at(&after, 40) != Some(crate::oath_lifecycle_wire::SIZE as u32)
            {
                return Err(Error::input("invalid lifecycle installed configuration"));
            }
            let address = wire::u64_at(&after, 32).unwrap();
            if address == 0 || !address.is_multiple_of(8) {
                return Err(Error::input("invalid lifecycle trace address"));
            }
            (address, crate::oath_lifecycle_wire::SIZE as u32)
        } else {
            wire::check_installed_config(&config, &after).map_err(Error::input)?
        };
        if lifecycle {
            crate::oath_lifecycle_wire::validate(
                &target.read(trace_address as usize, crate::oath_lifecycle_wire::SIZE)?,
            )
            .map_err(Error::input)?;
        } else if callee {
            crate::oath_recipient_wire::validate(
                &target.read(trace_address as usize, crate::oath_recipient_wire::SIZE)?,
            )
            .map_err(Error::input)?;
        } else {
            wire::TraceView::parse(&target.read(trace_address as usize, wire::TRACE_SIZE)?)
                .map_err(Error::input)?;
        }
        let receipt_buffer = target.alloc(wire::RECEIPT_SIZE)?;
        Ok((base, exports, trace_address, receipt_buffer))
    })
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oath_wire::tests::synthetic_dll;
    use std::os::windows::process::CommandExt;

    fn game(pid: u32) -> GameProcess {
        GameProcess {
            pid,
            started: 1.0e9,
        }
    }
    fn sha(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }
    fn dll_file(bytes: &[u8]) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oath_runtime_test.dll");
        std::fs::write(&path, bytes).unwrap();
        (dir, path)
    }

    #[test]
    fn install_refuses_this_process_and_bad_parameters_before_touching_anything() {
        let (_dir, path) = dll_file(&synthetic_dll(&wire::EXPORTS, false));
        let good = sha(&std::fs::read(&path).unwrap());
        let own = Session::install(game(std::process::id()), &path, &good, 30);
        assert!(own.err().unwrap().message.contains("own process refused"));
        assert!(Session::install(game(0), &path, &good, 30).is_err());
        let nan = GameProcess {
            pid: 4_000_000_000,
            started: f64::NAN,
        };
        assert!(Session::install(nan, &path, &good, 30).is_err());
        for seconds in [0u32, 301] {
            let error = Session::install(game(4_000_000_000), &path, &good, seconds).err();
            assert!(error.unwrap().message.contains("max_seconds"));
        }
    }

    #[test]
    fn the_dll_must_match_the_measured_hash_size_and_exports() {
        let bytes = synthetic_dll(&wire::EXPORTS, false);
        let (_dir, path) = dll_file(&bytes);
        let local = LocalDll::open(&path, &sha(&bytes)).unwrap();
        assert_eq!(local.exports, [0x1010, 0x1020, 0x1030, 0x1040]);
        drop(local);
        // wrong hash, uppercase hash, short hash
        assert!(LocalDll::open(&path, &"0".repeat(64)).is_err());
        assert!(LocalDll::open(&path, &sha(&bytes).to_uppercase()).is_err());
        assert!(LocalDll::open(&path, "abc").is_err());
        // a DLL without the four exports
        let partial = synthetic_dll(&wire::EXPORTS[..2], false);
        let (_d2, p2) = dll_file(&partial);
        assert!(LocalDll::open(&p2, &sha(&partial)).is_err());
        // bigger than 8 MiB is refused before hashing
        let big = vec![0u8; MAX_DLL_BYTES as usize + 1];
        let (_d3, p3) = dll_file(&big);
        assert!(LocalDll::open(&p3, &sha(&big))
            .err()
            .unwrap()
            .message
            .contains("8 MiB"));
        // empty and missing files
        let (_d4, p4) = dll_file(&[]);
        assert!(LocalDll::open(&p4, &sha(&[])).is_err());
        assert!(LocalDll::open(Path::new("Z:/does/not/exist.dll"), &sha(&bytes)).is_err());
    }

    #[test]
    fn the_measured_dll_cannot_be_replaced_or_written_while_it_is_pinned() {
        let bytes = synthetic_dll(&wire::EXPORTS, false);
        let (dir, path) = dll_file(&bytes);
        let local = LocalDll::open(&path, &sha(&bytes)).unwrap();
        assert!(
            OpenOptions::new().write(true).open(&path).is_err(),
            "write denied"
        );
        assert!(std::fs::remove_file(&path).is_err(), "delete denied");
        assert!(
            std::fs::rename(&path, dir.path().join("other.dll")).is_err(),
            "rename denied"
        );
        drop(local);
        assert!(
            OpenOptions::new().write(true).open(&path).is_ok(),
            "released with the handle"
        );
    }

    struct Sleeper(std::process::Child);
    impl Drop for Sleeper {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    /// A throw-away helper process that this test owns; it is not the game.
    fn sleeper() -> Sleeper {
        Sleeper(
            std::process::Command::new("ping")
                .args(["-n", "120", "127.0.0.1"])
                .stdout(std::process::Stdio::null())
                .creation_flags(0x0800_0000)
                .spawn()
                .unwrap(),
        )
    }
    fn target_of(child: &Sleeper, call_limit: Duration) -> Target {
        let access = PROCESS_QUERY_LIMITED_INFORMATION
            | PROCESS_SYNCHRONIZE
            | PROCESS_TERMINATE
            | PROCESS_VM_OPERATION
            | PROCESS_VM_READ
            | PROCESS_VM_WRITE
            | PROCESS_CREATE_THREAD;
        let process = Handle::new(unsafe { OpenProcess(access, 0, child.0.id()) }, "open").unwrap();
        let created = creation(process.0).unwrap();
        Target {
            process,
            pid: child.0.id(),
            created,
            exe: None,
            flight: false,
            poisoned: false,
            unconfirmed: false,
            fail_terminate: false,
            call_limit,
            kill_limit: Duration::from_secs(10),
            allocations: Vec::new(),
        }
    }
    fn kernel32_export(name: &[u8]) -> usize {
        let module = unsafe { GetModuleHandleW(wide("kernel32.dll").as_ptr()) };
        unsafe { GetProcAddress(module, name.as_ptr()) as usize }
    }

    #[test]
    fn kernel32_functions_are_proven_by_their_real_owner_in_a_live_process() {
        let child = sleeper();
        let target = target_of(&child, Duration::from_secs(8));
        // the helper was just created: wait until it has mapped kernel32
        let until = Instant::now() + Duration::from_secs(10);
        while !target
            .modules()
            .map(|all| {
                all.iter().any(|m| {
                    m.path
                        .file_name()
                        .is_some_and(|n| n.eq_ignore_ascii_case("kernel32.dll"))
                })
            })
            .unwrap_or(false)
        {
            assert!(Instant::now() < until, "helper never loaded kernel32");
            std::thread::sleep(Duration::from_millis(20));
        }
        for name in [&b"LoadLibraryW\0"[..], &b"GetCurrentProcessId\0"[..]] {
            let f = kernel32_export(name);
            let owner = local_owner(f).unwrap();
            assert!(f >= owner.base && f < owner.base + owner.size);
            verify_function_owner(&target, f).unwrap();
        }
        let implementation = local_load_library().unwrap();
        verify_function_owner(&target, implementation).unwrap();
        // an address in a non-image page has no owner
        let heap = Box::new(0u64);
        assert!(local_owner(&*heap as *const u64 as usize).is_err());
    }

    #[test]
    fn a_remote_call_runs_once_and_returns_the_thread_dword() {
        let child = sleeper();
        let mut target = target_of(&child, Duration::from_secs(8));
        let code = target
            .call(kernel32_export(b"GetCurrentProcessId\0"), 0)
            .unwrap();
        assert_eq!(code, child.0.id(), "the thread ran inside the helper");
        assert!(!target.flight && !target.poisoned);
        // memory round trip through a retained allocation
        let at = target.alloc(16).unwrap();
        target.write(at, b"0123456789abcdef").unwrap();
        assert_eq!(target.read(at, 16).unwrap(), b"0123456789abcdef");
        assert_eq!(target.allocations.len(), 1);
        assert!(target.read(at, 0).is_err());
        assert!(target.read(at, wire::MAX_TRACE_READ + 1).is_err());
    }

    #[test]
    fn an_unfinished_call_ends_the_pinned_process_and_is_never_resent() {
        let mut child = sleeper();
        let mut target = target_of(&child, Duration::from_millis(200));
        // Sleep(60000) cannot finish within the 200 ms limit.
        let error = target
            .call(kernel32_export(b"Sleep\0"), 60_000)
            .unwrap_err();
        assert!(error.message.contains("terminated"), "{}", error.message);
        assert!(target.poisoned);
        assert!(
            child.0.try_wait().unwrap().is_some(),
            "same pinned process was terminated"
        );
        let again = target.call(kernel32_export(b"GetCurrentProcessId\0"), 0);
        assert!(again.unwrap_err().message.contains("refused"), "no resend");
    }

    #[test]
    fn an_unobserved_termination_keeps_ownership_and_reports_unknown() {
        let mut child = sleeper();
        let mut target = target_of(&child, Duration::from_millis(100));
        target.fail_terminate = true; // the process end cannot be observed
        let error = target
            .call(kernel32_export(b"Sleep\0"), 60_000)
            .unwrap_err();
        assert!(error.message.contains("NOT confirmed"), "{}", error.message);
        assert!(target.flight && target.unconfirmed && target.unresolved());
        assert!(target.poisoned);
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "nothing else was killed"
        );
        assert!(target
            .call(kernel32_export(b"GetCurrentProcessId\0"), 0)
            .is_err());
        // Dropping while still unresolved keeps the pinned handle open.
        drop(target);
        assert!(child.0.try_wait().unwrap().is_none());
    }

    #[test]
    fn an_unresolved_owner_stays_pending_until_the_same_handle_signals() {
        let bytes = synthetic_dll(&wire::EXPORTS, false);
        let (_dir, path) = dll_file(&bytes);
        let local = LocalDll::open(&path, &sha(&bytes)).unwrap();
        let mut child = sleeper();
        let mut target = target_of(&child, Duration::from_millis(100));
        target.fail_terminate = true; // the end of the process cannot be observed
        assert!(target.call(kernel32_export(b"Sleep\0"), 60_000).is_err());
        let before = pending_count();
        retain(Held {
            target,
            keep: vec![Box::new(local)],
        });
        assert_eq!(pending_count(), before + 1);
        let locked = || OpenOptions::new().write(true).open(&path).is_err();
        assert!(locked(), "the file pin is kept while the end is unknown");
        // a drain with the process still running releases nothing
        assert_eq!(drain_pending(), before + 1);
        assert!(locked());
        // the process ends on its own; only now does the same handle signal
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        assert_eq!(drain_pending(), before);
        assert!(!locked(), "pin released after the observed end");
    }

    #[test]
    fn a_pending_owner_whose_handle_lost_its_creation_time_is_never_released() {
        let mut child = sleeper();
        let mut target = target_of(&child, Duration::from_millis(100));
        target.fail_terminate = true;
        assert!(target.call(kernel32_export(b"Sleep\0"), 60_000).is_err());
        target.created += 1; // as if the PID now named another process
        let before = pending_count();
        retain(Held {
            target,
            keep: Vec::new(),
        });
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        assert_eq!(drain_pending(), before + 1, "false success is refused");
    }

    #[test]
    fn the_obligation_ends_only_after_the_termination_is_observed() {
        let mut child = sleeper();
        let mut target = target_of(&child, Duration::from_millis(100));
        target.fail_terminate = true;
        assert!(target.call(kernel32_export(b"Sleep\0"), 60_000).is_err());
        assert!(target.unresolved());
        target.fail_terminate = false;
        assert!(target.kill(), "second attempt on the same pinned handle");
        assert!(!target.unresolved());
        assert!(child.0.try_wait().unwrap().is_some());
    }

    #[test]
    fn critical_runtime_refusals_end_only_the_same_owned_process() {
        for operation in [
            "OathRuntimeInstall",
            "OathRuntimeStartObservation",
            "OathRuntimeStopObservation",
            "OathRuntimeReadReceipt",
        ] {
            for status in [6, 99] {
                let mut child = sleeper();
                let mut other = sleeper();
                let mut target = target_of(&child, Duration::from_secs(8));
                let error = target.runtime_refusal(operation, status);
                assert!(error.to_string().contains("the pinned game was terminated"));
                assert!(target.poisoned && !target.unresolved());
                assert!(child.0.try_wait().unwrap().is_some());
                assert!(other.0.try_wait().unwrap().is_none());
                assert!(target
                    .call(kernel32_export(b"GetCurrentProcessId\0"), 0)
                    .is_err());
            }
        }
    }

    #[test]
    fn critical_refusal_keeps_ownership_when_termination_is_not_confirmed() {
        let mut child = sleeper();
        let mut target = target_of(&child, Duration::from_secs(8));
        target.fail_terminate = true;
        let error = target.runtime_refusal("OathRuntimeInstall", 6);
        assert!(error.to_string().contains("retention=known_pending"));
        assert!(target.poisoned && target.unresolved());
        assert!(child.0.try_wait().unwrap().is_none());
        target.fail_terminate = false;
        assert!(target.kill());
    }

    #[test]
    fn ordinary_runtime_refusal_does_not_terminate_an_unchanged_process() {
        let mut child = sleeper();
        let mut target = target_of(&child, Duration::from_secs(8));
        let error = target.runtime_refusal("OathRuntimeInstall", 5);
        assert!(error.to_string().contains("OathRuntimeInstall refused"));
        assert!(!target.poisoned && !target.unresolved());
        assert!(child.0.try_wait().unwrap().is_none());
    }

    #[test]
    fn incompatible_installed_trace_ends_owned_process_instead_of_abandoning_entrances() {
        let mut child = sleeper();
        let mut target = target_of(&child, Duration::from_secs(8));
        let mut bytes = vec![0; crate::oath_recipient_wire::SIZE];
        bytes[..4].copy_from_slice(&0x4a41_4352u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&1u32.to_le_bytes()); // the previous DLL's schema
        let result = target.verify_after_install(|_| {
            crate::oath_recipient_wire::validate(&bytes).map_err(Error::input)
        });
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("the pinned game was terminated"));
        assert!(child.0.try_wait().unwrap().is_some());
        assert!(target.poisoned && !target.unresolved());
    }

    #[test]
    fn a_handle_that_lost_its_creation_time_is_never_killed_or_confirmed() {
        let mut child = sleeper();
        let mut target = target_of(&child, Duration::from_secs(8));
        target.created += 1; // as if the PID had been reused
        target.flight = true;
        assert!(!target.kill());
        assert!(target.unresolved());
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "a different process is untouched"
        );
        target.flight = false;
        target.unconfirmed = false;
    }

    #[test]
    fn dropping_a_target_with_an_unobserved_flight_ends_the_process() {
        let mut child = sleeper();
        {
            let mut target = target_of(&child, Duration::from_secs(8));
            target.flight = true; // as after a panic between thread creation and its end
        }
        assert!(
            child.0.try_wait().unwrap().is_some(),
            "drop killed the pinned process"
        );
    }

    #[test]
    fn a_handle_whose_creation_time_changed_is_not_the_pinned_process() {
        let child = sleeper();
        let mut target = target_of(&child, Duration::from_secs(8));
        target.created += 1;
        assert!(target.alive_pinned().is_err());
        assert!(target
            .call(kernel32_export(b"GetCurrentProcessId\0"), 0)
            .is_err());
        assert!(target.read(0x1000, 8).is_err());
        assert!(target.write(0x1000, b"x").is_err());
        assert!(target.modules().is_err());
    }
}
