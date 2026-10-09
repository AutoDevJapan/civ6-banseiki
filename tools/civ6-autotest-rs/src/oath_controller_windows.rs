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
    fn open(path: &Path, expected_sha: &str) -> Result<Self> {
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
        let exports = wire::runtime_exports(&image).map_err(Error::input)?;
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
    name: String,
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

    /// Terminates the same pinned process after a call whose end is unknown.
    fn kill(&mut self) {
        self.poisoned = true;
        if creation(self.process.0).ok() != Some(self.created) {
            return;
        }
        unsafe {
            TerminateProcess(self.process.0, 1);
        }
        let wait = self.kill_limit.as_millis().min(u128::from(u32::MAX - 1)) as u32;
        if unsafe { WaitForSingleObject(self.process.0, wait) } == 0 {
            self.flight = false;
        }
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
            self.kill();
            return Err(Error::unknown(
                "remote call did not finish in time; the pinned game was terminated",
            ));
        }
        let mut code = 0u32;
        if unsafe { GetExitCodeThread(thread.0, &mut code) } == 0 {
            self.kill();
            return Err(Error::unknown(
                "remote call exit code is unreadable; the pinned game was terminated",
            ));
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
                name: wide_text(&row.szModule)?,
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
        // this controller is about to forget.
        if self.flight {
            self.kill();
        }
    }
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
pub(crate) struct Session {
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
}
unsafe impl Send for Session {}

impl Session {
    /// Loads and installs the observer. The expected SHA is only compared with
    /// the file this process measures; it authorizes nothing by itself.
    pub(crate) fn install(
        game: GameProcess,
        dll_path: &Path,
        expected_sha: &str,
        max_seconds: u32,
    ) -> Result<Self> {
        if game.pid == 0
            || game.pid == std::process::id()
            || !game.started.is_finite()
            || game.started <= 0.0
        {
            return Err(Error::input(
                "invalid caller-pinned game process (own process refused)",
            ));
        }
        if !(1..=wire::MAX_SECONDS).contains(&max_seconds) {
            return Err(Error::input("max_seconds is outside 1..=300"));
        }
        // Everything about the DLL is settled before the game is touched.
        let dll = LocalDll::open(dll_path, expected_sha)?;
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
            call_limit: CALL_LIMIT,
            kill_limit: KILL_LIMIT,
            allocations: Vec::new(),
        };
        let base = load_dll(&mut target, &dll)?;
        let exports = dll.exports.map(|rva| base + rva as usize);
        verify_remote_image(&target, &dll, base, &exports)?;

        let config = wire::encode_config(game.pid, max_seconds, created);
        let config_at = target.alloc(wire::CONFIG_SIZE)?;
        target.write(config_at, &config)?;
        let status = target.call(exports[0], config_at)?;
        if status != 0 {
            return Err(Error::input(format!(
                "OathRuntimeInstall refused: {} ({status})",
                wire::status_name(status)
            )));
        }
        let (trace_address, _) =
            wire::check_installed_config(&config, &target.read(config_at, wire::CONFIG_SIZE)?)
                .map_err(Error::input)?;
        wire::TraceView::parse(&target.read(trace_address as usize, wire::TRACE_SIZE)?)
            .map_err(Error::input)?;
        let receipt_buffer = target.alloc(wire::RECEIPT_SIZE)?;
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
            "trace_size": wire::TRACE_SIZE,
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
            return Err(Error::input(format!(
                "OathRuntimeStartObservation refused: {} ({status})",
                wire::status_name(status)
            )));
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
            return Err(Error::input(format!(
                "OathRuntimeStopObservation refused: {} ({status})",
                wire::status_name(status)
            )));
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
            return Err(Error::input(format!(
                "OathRuntimeReadReceipt refused: {} ({status})",
                wire::status_name(status)
            )));
        }
        let receipt =
            wire::decode_receipt(&self.target.read(self.receipt_buffer, wire::RECEIPT_SIZE)?)
                .map_err(Error::input)?;
        if receipt.trace_address != self.trace_address as u64
            || receipt.trace_size as usize != wire::TRACE_SIZE
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
    if target.read(function, 32)? != local {
        return Err(Error::input(
            "remote function bytes differ from the local function",
        ));
    }
    Ok(())
}

/// Fresh remote load of the pinned DLL; returns its verified base.
fn load_dll(target: &mut Target, dll: &LocalDll) -> Result<usize> {
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
        wire::LoadVerdict::Unknown(why) => {
            target.kill();
            Err(Error::unknown(format!(
                "{why}; the pinned game was terminated"
            )))
        }
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
        for name in [&b"LoadLibraryW\0"[..], &b"GetCurrentProcessId\0"[..]] {
            let f = kernel32_export(name);
            let owner = local_owner(f).unwrap();
            assert!(f >= owner.base && f < owner.base + owner.size);
            verify_function_owner(&target, f).unwrap();
        }
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
