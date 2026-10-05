use civ6_native_bridge_rs::{
    AssignCalibrationTrace, BridgeConfig, BridgeMode, BridgeStatus, CalibrationTrace, JoinedTrace,
    PromotionTrace, PropertyTrace, SharedTrace, ASHINA_ADMISSION_KEY_ID, CALIBRATION_CAPACITY,
    GAMECORE, GAME_EXE, MAX_ACTIVE_SECONDS, PINNED_CAN_EVER_RVA, PINNED_GAMECORE_SHA256,
    PINNED_PROMOTION_CALLEE_RVA, PINNED_PROMOTION_CALL_RVA, PINNED_PROPERTY_GET_RVA,
    PROPERTY_TEST_PHASE_KEY_ID, PROPERTY_TEST_VALUE_KEY_ID, TRACE_CAPACITY,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::ffi::{c_void, OsString};
use std::fs::{self, File};
use std::io::{BufRead, BufWriter, Read, Seek, SeekFrom, Write};
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use windows_sys::Win32::Foundation::{
    CloseHandle, FreeLibrary, ERROR_BAD_LENGTH, FILETIME, HANDLE, INVALID_HANDLE_VALUE,
    WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::System::Diagnostics::{
    Debug::{ReadProcessMemory, WriteProcessMemory},
    ToolHelp::{
        CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW,
        MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32,
        TH32CS_SNAPPROCESS,
    },
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicProcessIdList,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject, JOBOBJECT_BASIC_PROCESS_ID_LIST, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK,
};
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleHandleW, GetProcAddress, LoadLibraryExW, DONT_RESOLVE_DLL_REFERENCES,
};
use windows_sys::Win32::System::Memory::{VirtualAllocEx, MEM_COMMIT, MEM_RESERVE, PAGE_READWRITE};
use windows_sys::Win32::System::Threading::{
    CreateRemoteThread, GetCurrentProcessId, GetExitCodeThread, GetProcessId, GetProcessTimes,
    OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject, PROCESS_CREATE_THREAD,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
    PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
};

const LOCK: &str = "D:/CC/Tools/civ6-live/civ6.lock";
const CHAIN: &str = "D:/CC/Tools/civ6-live/live_chain.exe";
const BRIDGE_DLL: &str = "civ6_native_bridge_rs.dll";
const REMOTE_CALL_SECONDS: u32 = 8;
const RECOVERY_SECONDS: u32 = 30;
const CAN_EVER_FILE_OFFSET: u64 = 0x2dd1d0;
const PROPERTY_GET_FILE_OFFSET: u64 = 0x99e9c0;
const PROMOTION_CALL_FILE_OFFSET: u64 = 0x30e00f;
const PROMOTION_CALL_WINDOW: [u8; 16] = [
    0xe8, 0x9c, 0xef, 0xfc, 0xff, 0x84, 0xc0, 0x0f, 0x84, 0x75, 0x01, 0x00, 0x00, 0x44, 0x8b, 0xc7,
];
const PROPERTY_PREFIX_LEN: usize = 5;
const PROPERTY_PREFIX_PAD: [u8; PROPERTY_PREFIX_LEN] = [0xcc; PROPERTY_PREFIX_LEN];
const CAN_EVER_PROLOGUE: [u8; 5] = [0x48, 0x89, 0x5c, 0x24, 0x08];
const PROPERTY_PROLOGUE: [u8; 8] = [0x48, 0x8b, 0x41, 0x40, 0x4c, 0x8d, 0x49, 0x30];

struct Handle(HANDLE);
impl Handle {
    fn checked(value: HANDLE, label: &str) -> Result<Self, String> {
        if value.is_null() || value == INVALID_HANDLE_VALUE {
            Err(format!("{label}: {}", std::io::Error::last_os_error()))
        } else {
            Ok(Self(value))
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

/// An unnamed, non-inheritable job containing only the already pinned target.
/// While the kill flag is armed, process exit of this controller closes its
/// sole job handle and makes Windows terminate the target in the kernel.
struct OwnedJob {
    handle: Option<Handle>,
}

impl OwnedJob {
    fn attach(target: &Handle, pid: u32, created: u64) -> Result<Self, String> {
        if pid == 0
            || !alive(target)
            || unsafe { GetProcessId(target.0) } != pid
            || process_created(target)? != created
        {
            return Err("job target identity changed before assignment".into());
        }
        let job = Handle::checked(
            unsafe { CreateJobObjectW(null_mut(), null_mut()) },
            "CreateJobObjectW",
        )?;
        let owned = Self { handle: Some(job) };
        owned.set_limits(
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK,
        )?;
        if unsafe { AssignProcessToJobObject(owned.raw(), target.0) } == 0 {
            return Err(format!(
                "pinned target job assignment failed before mutation: {}",
                std::io::Error::last_os_error()
            ));
        }
        let mut members = JOBOBJECT_BASIC_PROCESS_ID_LIST::default();
        let mut returned = 0u32;
        if unsafe {
            QueryInformationJobObject(
                owned.raw(),
                JobObjectBasicProcessIdList,
                (&mut members as *mut JOBOBJECT_BASIC_PROCESS_ID_LIST).cast(),
                size_of::<JOBOBJECT_BASIC_PROCESS_ID_LIST>() as u32,
                &mut returned,
            )
        } == 0
            || members.NumberOfAssignedProcesses != 1
            || members.NumberOfProcessIdsInList != 1
            || members.ProcessIdList[0] != pid as usize
        {
            // If assignment succeeded, Drop closes the armed job and ends
            // that exact target. No remote allocation has occurred yet.
            return Err("pinned job does not contain exactly one target".into());
        }
        Ok(owned)
    }

    fn raw(&self) -> HANDLE {
        self.handle.as_ref().expect("owned job already closed").0
    }

    fn set_limits(&self, flags: u32) -> Result<(), String> {
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = flags;
        if unsafe {
            SetInformationJobObject(
                self.raw(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        } == 0
        {
            return Err(format!(
                "job limits unavailable: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    fn disarm_after_verified_entry_restoration(&self) -> Result<(), String> {
        self.set_limits(JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK)?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        if unsafe {
            QueryInformationJobObject(
                self.raw(),
                JobObjectExtendedLimitInformation,
                (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                null_mut(),
            )
        } == 0
            || limits.BasicLimitInformation.LimitFlags != JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK
        {
            return Err("job kill-on-close state unverified after entry restoration".into());
        }
        Ok(())
    }

    fn terminate_and_confirm(&mut self, target: &Handle, wait_ms: u32) -> Result<(), String> {
        let terminated = unsafe { TerminateJobObject(self.raw(), 1) } != 0;
        // Also close the last handle. KILL_ON_JOB_CLOSE remains armed, so this
        // covers an unexpected controller exit or TerminateJobObject failure.
        self.handle.take();
        if unsafe { WaitForSingleObject(target.0, wait_ms) } != WAIT_OBJECT_0 {
            return Err(format!(
                "job close initiated kernel target termination; exit unconfirmed (TerminateJobObject={terminated})"
            ));
        }
        Ok(())
    }
}

fn fail_owned_call(job: &mut OwnedJob, target: &Handle, error: String, wait_ms: u32) -> String {
    let cleanup = job.terminate_and_confirm(target, wait_ms);
    format!("{error}; contained target cleanup={cleanup:?}")
}

struct Args {
    pid: u32,
    created: u64,
    duration: u32,
    out: PathBuf,
    mode: BridgeMode,
}

fn args(raw: &[String]) -> Result<Args, String> {
    if raw.len() != 8 && raw.len() != 10 {
        return Err("usage: civ6-native-bridge --pid N --created-filetime N --duration 1..300 --out ABSOLUTE.jsonl [--mode can-ever|property|property-scoped|can-ever-calibration|property-admission-scoped|joined-admission|production-admission|promotion-call]".into());
    }
    let mut fields = std::collections::BTreeMap::new();
    for pair in raw.chunks_exact(2) {
        if ![
            "--pid",
            "--created-filetime",
            "--duration",
            "--out",
            "--mode",
        ]
        .contains(&pair[0].as_str())
            || fields.insert(pair[0].as_str(), pair[1].as_str()).is_some()
        {
            return Err("duplicate or unknown bridge argument".into());
        }
    }
    let pid = fields
        .get("--pid")
        .ok_or("missing PID")?
        .parse::<u32>()
        .map_err(|_| "invalid PID")?;
    let created = fields
        .get("--created-filetime")
        .ok_or("missing creation FILETIME")?
        .parse::<u64>()
        .map_err(|_| "invalid creation FILETIME")?;
    let duration = fields
        .get("--duration")
        .ok_or("missing duration")?
        .parse::<u32>()
        .map_err(|_| "invalid duration")?;
    let out = PathBuf::from(*fields.get("--out").ok_or("missing output")?);
    let mode = match fields.get("--mode").copied().unwrap_or("can-ever") {
        "can-ever" => BridgeMode::CanEver,
        "property" => BridgeMode::Property,
        "property-scoped" => BridgeMode::PropertyScoped,
        "can-ever-calibration" => BridgeMode::CanEverCalibration,
        "property-admission-scoped" => BridgeMode::PropertyAdmissionScoped,
        "assign-calibration" => BridgeMode::AssignCalibration,
        "joined-admission" => BridgeMode::JoinedAdmission,
        "production-admission" => BridgeMode::ProductionAdmission,
        "promotion-call" => BridgeMode::PromotionCall,
        _ => return Err("invalid bridge mode".into()),
    };
    if pid == 0
        || created == 0
        || !(1..=MAX_ACTIVE_SECONDS).contains(&duration)
        || !out.is_absolute()
        || out.extension().is_none_or(|ext| ext != "jsonl")
        || !out.parent().is_some_and(Path::is_dir)
        || out.exists()
    {
        return Err("invalid PID, duration, or new absolute JSONL output".into());
    }
    Ok(Args {
        pid,
        created,
        duration,
        out,
        mode,
    })
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
fn from_wide(value: &[u16]) -> PathBuf {
    let len = value.iter().position(|c| *c == 0).unwrap_or(value.len());
    PathBuf::from(OsString::from_wide(&value[..len]))
}
fn same_path(actual: &Path, expected: &Path) -> bool {
    let (Ok(actual), Ok(expected)) = (actual.canonicalize(), expected.canonicalize()) else {
        return false;
    };
    actual
        .to_string_lossy()
        .eq_ignore_ascii_case(&expected.to_string_lossy())
}
fn sha256(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("cannot read pinned file: {e}"))?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let len = file
            .read(&mut buffer)
            .map_err(|e| format!("hash read: {e}"))?;
        if len == 0 {
            break;
        }
        hash.update(&buffer[..len]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
fn filetime(value: FILETIME) -> u64 {
    (u64::from(value.dwHighDateTime) << 32) | u64::from(value.dwLowDateTime)
}
fn process_created(handle: &Handle) -> Result<u64, String> {
    let mut created: FILETIME = unsafe { zeroed() };
    let mut exited = created;
    let mut kernel = created;
    let mut user = created;
    if unsafe { GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) } == 0
    {
        return Err("cannot read process creation time".into());
    }
    Ok(filetime(created))
}
fn process_path(handle: &Handle) -> Result<PathBuf, String> {
    let mut buffer = vec![0u16; 32768];
    let mut len = buffer.len() as u32;
    if unsafe { QueryFullProcessImageNameW(handle.0, 0, buffer.as_mut_ptr(), &mut len) } == 0
        || len == 0
        || len as usize >= buffer.len()
    {
        return Err("cannot read complete process image path".into());
    }
    Ok(PathBuf::from(OsString::from_wide(&buffer[..len as usize])))
}
fn alive(handle: &Handle) -> bool {
    (unsafe { WaitForSingleObject(handle.0, 0) }) == WAIT_TIMEOUT
}
fn open_process(pid: u32, rights: u32) -> Result<Handle, String> {
    Handle::checked(unsafe { OpenProcess(rights, 0, pid) }, "OpenProcess")
}
fn parent_pid(pid: u32) -> Result<u32, String> {
    let snapshot = Handle::checked(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) },
        "process snapshot",
    )?;
    let mut entry: PROCESSENTRY32W = unsafe { zeroed() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    if unsafe { Process32FirstW(snapshot.0, &mut entry) } == 0 {
        return Err("process snapshot empty".into());
    }
    loop {
        if entry.th32ProcessID == pid {
            return Ok(entry.th32ParentProcessID);
        }
        if unsafe { Process32NextW(snapshot.0, &mut entry) } == 0 {
            break;
        }
    }
    Err("game missing from process snapshot".into())
}
fn now_unix() -> Result<f64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock unavailable")?
        .as_secs_f64())
}
fn verify_lease(chain_pid: u32, chain: &Handle, duration: u32) -> Result<(), String> {
    let actual_lock =
        std::env::var("CIV6_LIVE_LOCK_PATH").map_err(|_| "live_chain lock environment missing")?;
    if !same_path(Path::new(&actual_lock), Path::new(LOCK)) {
        return Err("standard live_chain lock required".into());
    }
    let value: Value = serde_json::from_slice(&fs::read(LOCK).map_err(|e| e.to_string())?)
        .map_err(|_| "live_chain lock JSON invalid")?;
    if value["live_chain"] != true
        || value["chain_pid"].as_u64() != Some(u64::from(chain_pid))
        || value["pid"].as_u64() != Some(u64::from(chain_pid))
        || !alive(chain)
        || !same_path(&process_path(chain)?, Path::new(CHAIN))
    {
        return Err("live_chain lock owner changed".into());
    }
    let acquired = value["acquired_at"]
        .as_f64()
        .ok_or("live_chain acquisition time missing")?;
    let now = now_unix()?;
    let created = process_created(chain)? as f64 / 10_000_000.0 - 11_644_473_600.0;
    if !acquired.is_finite()
        || acquired > now
        || now + f64::from(duration) + f64::from(RECOVERY_SECONDS) + 15.0 >= acquired + 1800.0
        || created > acquired + 1.0
    {
        return Err("live_chain lease cannot cover the bounded bridge run".into());
    }
    Ok(())
}

struct Module {
    base: usize,
    size: usize,
    path: PathBuf,
}
fn retry_module_snapshot<T>(mut open: impl FnMut() -> std::io::Result<T>) -> Result<T, String> {
    // Windows documents ERROR_BAD_LENGTH as transient when a module list
    // changes during TH32CS_SNAPMODULE. Keep the retry finite for this
    // bounded controller, and propagate every other error immediately.
    for attempt in 0..16 {
        match open() {
            Ok(snapshot) => return Ok(snapshot),
            Err(error) if error.raw_os_error() == Some(ERROR_BAD_LENGTH as i32) && attempt < 15 => {
                std::thread::yield_now();
            }
            Err(error) => return Err(format!("module snapshot: {error}")),
        }
    }
    Err("module snapshot retry exhausted".into())
}
fn modules(pid: u32) -> Result<Vec<Module>, String> {
    let snapshot = retry_module_snapshot(|| {
        let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) };
        if !raw.is_null() && raw != INVALID_HANDLE_VALUE {
            Ok(Handle(raw))
        } else {
            Err(std::io::Error::last_os_error())
        }
    })?;
    let mut entry: MODULEENTRY32W = unsafe { zeroed() };
    entry.dwSize = size_of::<MODULEENTRY32W>() as u32;
    if unsafe { Module32FirstW(snapshot.0, &mut entry) } == 0 {
        return Err("module snapshot empty".into());
    }
    let mut result = Vec::new();
    loop {
        result.push(Module {
            base: entry.modBaseAddr as usize,
            size: entry.modBaseSize as usize,
            path: from_wide(&entry.szExePath),
        });
        if unsafe { Module32NextW(snapshot.0, &mut entry) } == 0 {
            break;
        }
    }
    Ok(result)
}

fn pinned_target(pid: u32, game: &Handle, mode: BridgeMode) -> Result<usize, String> {
    if mode == BridgeMode::PromotionCall
        && !civ6_native_bridge_rs::supports_atomic_promotion_fetch()
    {
        return Err("promotion call operand exchange requires documented AMD atomic instruction-fetch support".into());
    }
    if !same_path(&process_path(game)?, Path::new(GAME_EXE)) {
        return Err("unpinned game executable".into());
    }
    let module = modules(pid)?
        .into_iter()
        .find(|m| same_path(&m.path, Path::new(GAMECORE)))
        .ok_or("pinned GameCore not loaded")?;
    if sha256(Path::new(GAMECORE))? != PINNED_GAMECORE_SHA256
        || module.size
            <= mode.rva()
                + if mode == BridgeMode::PromotionCall {
                    16
                } else {
                    8
                }
        || (mode == BridgeMode::PromotionCall && module.size <= PINNED_PROMOTION_CALLEE_RVA + 8)
    {
        return Err("GameCore hash or image size differs".into());
    }
    Ok(module.base + mode.rva())
}

fn read_remote(game: &Handle, address: usize, output: &mut [u8]) -> Result<(), String> {
    let mut read = 0usize;
    if unsafe {
        ReadProcessMemory(
            game.0,
            address as *const c_void,
            output.as_mut_ptr().cast(),
            output.len(),
            &mut read,
        )
    } == 0
        || read != output.len()
    {
        return Err("bounded target read failed".into());
    }
    Ok(())
}
fn write_remote(game: &Handle, address: usize, input: &[u8]) -> Result<(), String> {
    let mut written = 0usize;
    if unsafe {
        WriteProcessMemory(
            game.0,
            address as *mut c_void,
            input.as_ptr().cast(),
            input.len(),
            &mut written,
        )
    } == 0
        || written != input.len()
    {
        return Err("bounded controller allocation write failed".into());
    }
    Ok(())
}
fn remote_alloc(game: &Handle, bytes: usize) -> Result<usize, String> {
    let result = unsafe {
        VirtualAllocEx(
            game.0,
            null_mut(),
            bytes,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        )
    };
    (!result.is_null())
        .then_some(result as usize)
        .ok_or_else(|| "controller allocation in pinned game failed".into())
}
fn remote_call(game: &Handle, entry: usize, parameter: usize, label: &str) -> Result<u32, String> {
    if entry == 0 {
        return Err(format!("{label} address missing"));
    }
    let start = unsafe {
        std::mem::transmute::<usize, unsafe extern "system" fn(*mut c_void) -> u32>(entry)
    };
    let mut tid = 0u32;
    let thread = Handle::checked(
        unsafe {
            CreateRemoteThread(
                game.0,
                null_mut(),
                0,
                Some(start),
                parameter as *const c_void,
                0,
                &mut tid,
            )
        },
        label,
    )?;
    if unsafe { WaitForSingleObject(thread.0, REMOTE_CALL_SECONDS * 1000) } != WAIT_OBJECT_0 {
        return Err(format!("{label} timed out; close the pinned game"));
    }
    let mut code = 0u32;
    if unsafe { GetExitCodeThread(thread.0, &mut code) } == 0 {
        return Err(format!(
            "{label} exit code unavailable; close the pinned game"
        ));
    }
    Ok(code)
}

struct LocalDll(*mut c_void);
impl Drop for LocalDll {
    fn drop(&mut self) {
        unsafe { FreeLibrary(self.0) };
    }
}
fn exported_rvas(path: &Path) -> Result<(usize, usize), String> {
    let dll = LocalDll(unsafe {
        LoadLibraryExW(
            wide(&path.to_string_lossy()).as_ptr(),
            null_mut(),
            DONT_RESOLVE_DLL_REFERENCES,
        )
    });
    if dll.0.is_null() {
        return Err("cannot inspect local bridge DLL exports".into());
    }
    let install = unsafe { GetProcAddress(dll.0, c"BridgeInstall".as_ptr().cast()) }
        .ok_or("bridge install export missing")? as usize;
    let uninstall = unsafe { GetProcAddress(dll.0, c"BridgeUninstall".as_ptr().cast()) }
        .ok_or("bridge uninstall export missing")? as usize;
    Ok((install - dll.0 as usize, uninstall - dll.0 as usize))
}
fn remote_load_library(pid: u32) -> Result<usize, String> {
    let local = unsafe { GetModuleHandleW(wide("kernel32.dll").as_ptr()) };
    if local.is_null() {
        return Err("local kernel32 unavailable".into());
    }
    let function = unsafe { GetProcAddress(local, c"LoadLibraryW".as_ptr().cast()) }
        .ok_or("LoadLibraryW export unavailable")? as usize;
    let local_module = modules(unsafe { GetCurrentProcessId() })?
        .into_iter()
        .find(|m| function >= m.base && function - m.base < m.size)
        .ok_or("LoadLibraryW owner module unknown")?;
    let remote_module = modules(pid)?
        .into_iter()
        .find(|m| same_path(&m.path, &local_module.path))
        .ok_or("remote LoadLibraryW owner module missing")?;
    if remote_module.base != local_module.base {
        return Err("system loader base differs across processes".into());
    }
    Ok(function)
}
fn bridge_dll() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|e| format!("controller path: {e}"))?;
    let path = executable.with_file_name(BRIDGE_DLL);
    if !path.is_file() || path.file_name().is_none_or(|name| name != BRIDGE_DLL) {
        return Err("bridge DLL must sit beside this controller".into());
    }
    path.canonicalize()
        .map_err(|e| format!("bridge DLL path: {e}"))
}
fn pinned_entry_bytes(game: &Handle, target: usize, mode: BridgeMode) -> Result<[u8; 8], String> {
    let mut file = File::open(GAMECORE).map_err(|e| format!("pinned GameCore read: {e}"))?;
    let (offset, prologue): (u64, &[u8]) = match mode {
        BridgeMode::CanEver
        | BridgeMode::CanEverCalibration
        | BridgeMode::JoinedAdmission
        | BridgeMode::ProductionAdmission => (CAN_EVER_FILE_OFFSET, &CAN_EVER_PROLOGUE),
        BridgeMode::AssignCalibration => (0x2dcc80, &[0x48, 0x89, 0x5c, 0x24, 0x08]),
        BridgeMode::Property | BridgeMode::PropertyScoped | BridgeMode::PropertyAdmissionScoped => {
            (PROPERTY_GET_FILE_OFFSET, &PROPERTY_PROLOGUE)
        }
        BridgeMode::PromotionCall => (PROMOTION_CALL_FILE_OFFSET, &PROMOTION_CALL_WINDOW[..8]),
    };
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| format!("pinned entry seek: {e}"))?;
    let mut disk = [0u8; 8];
    file.read_exact(&mut disk)
        .map_err(|e| format!("pinned entry read: {e}"))?;
    let mut memory = [0u8; 8];
    read_remote(game, target, &mut memory)?;
    if memory != disk || &disk[..prologue.len()] != prologue {
        return Err("pinned in-memory GameCore entry differs".into());
    }
    Ok(disk)
}

fn pinned_promotion_window(game: &Handle, call: usize) -> Result<[u8; 16], String> {
    let mut file = File::open(GAMECORE).map_err(|e| format!("pinned GameCore read: {e}"))?;
    file.seek(SeekFrom::Start(PROMOTION_CALL_FILE_OFFSET))
        .map_err(|e| e.to_string())?;
    let mut disk = [0u8; 16];
    file.read_exact(&mut disk).map_err(|e| e.to_string())?;
    let mut memory = [0u8; 16];
    read_remote(game, call, &mut memory)?;
    if memory != disk || disk != PROMOTION_CALL_WINDOW {
        return Err("complete promotion call window differs".into());
    }
    let original_target =
        (call + 5) as i128 + i32::from_le_bytes(disk[1..5].try_into().unwrap()) as i128;
    if !call.wrapping_add(1).is_multiple_of(4)
        || ((call + 1) & 63) > 60
        || original_target
            != (call - PINNED_PROMOTION_CALL_RVA + PINNED_PROMOTION_CALLEE_RVA) as i128
    {
        return Err("promotion call target or atomic operand invalid".into());
    }
    Ok(disk)
}

fn pinned_property_prefix(
    game: &Handle,
    target: usize,
) -> Result<[u8; PROPERTY_PREFIX_LEN], String> {
    if target & 0xfff < PROPERTY_PREFIX_LEN {
        return Err("property prefix crosses a page boundary".into());
    }
    let mut file = File::open(GAMECORE).map_err(|e| format!("pinned GameCore read: {e}"))?;
    file.seek(SeekFrom::Start(
        PROPERTY_GET_FILE_OFFSET - PROPERTY_PREFIX_LEN as u64,
    ))
    .map_err(|e| format!("pinned property prefix seek: {e}"))?;
    let mut disk = [0u8; PROPERTY_PREFIX_LEN];
    file.read_exact(&mut disk)
        .map_err(|e| format!("pinned property prefix read: {e}"))?;
    let mut memory = [0u8; PROPERTY_PREFIX_LEN];
    read_remote(game, target - PROPERTY_PREFIX_LEN, &mut memory)?;
    if disk != PROPERTY_PREFIX_PAD || memory != disk {
        return Err("pinned property prefix is not five untouched INT3 bytes".into());
    }
    Ok(disk)
}
fn write_json(writer: &mut BufWriter<File>, value: Value) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, &value).map_err(|e| e.to_string())?;
    writer.write_all(b"\n").map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())
}

fn read_trace(game: &Handle, address: usize) -> Result<Box<SharedTrace>, String> {
    let mut snapshot = Box::new(SharedTrace::default());
    let bytes = unsafe {
        std::slice::from_raw_parts_mut(
            (&mut *snapshot as *mut SharedTrace).cast::<u8>(),
            size_of::<SharedTrace>(),
        )
    };
    read_remote(game, address, bytes)?;
    if snapshot.magic != 0x4a41_4342 || snapshot.version != SharedTrace::VERSION {
        return Err("bridge trace version differs".into());
    }
    Ok(snapshot)
}

fn drained_trace(game: &Handle, address: usize) -> Result<Box<SharedTrace>, String> {
    // Restoration stops new entry jumps. Existing gateway calls may still be
    // writing their reserved slots. Require a bounded quiet interval.
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut stable = 0u32;
    let mut prior = None;
    while Instant::now() < deadline {
        let snapshot = read_trace(game, address)?;
        let next = snapshot.next.load(std::sync::atomic::Ordering::Acquire);
        let in_flight = snapshot
            .in_flight
            .load(std::sync::atomic::Ordering::Acquire);
        if in_flight == 0 && prior == Some(next) && snapshot.complete_rows().is_ok() {
            stable += 1;
            if stable >= 3 {
                return Ok(snapshot);
            }
        } else {
            stable = 0;
        }
        prior = Some(next);
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("gateway trace did not drain or contains dropped/unfinished rows".into())
}

fn drained_promotion_trace(game: &Handle, address: usize) -> Result<Box<PromotionTrace>, String> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut prior = None;
    let mut stable = 0;
    while Instant::now() < deadline {
        let mut snapshot = Box::new(PromotionTrace::default());
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(
                (&mut *snapshot as *mut PromotionTrace).cast::<u8>(),
                size_of::<PromotionTrace>(),
            )
        };
        read_remote(game, address, bytes)?;
        let next = snapshot.next.load(std::sync::atomic::Ordering::Acquire);
        if snapshot.complete_rows().is_ok() && prior == Some(next) {
            stable += 1;
            if stable >= 3 {
                return Ok(snapshot);
            }
        } else {
            stable = 0;
        }
        prior = Some(next);
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("promotion trace header, overflow, inflight, or partial row invalid".into())
}

fn drained_calibration_trace(
    game: &Handle,
    address: usize,
) -> Result<Box<CalibrationTrace>, String> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut stable = 0u32;
    let mut prior = None;
    while Instant::now() < deadline {
        let mut snapshot = Box::new(CalibrationTrace::default());
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(
                (&mut *snapshot as *mut CalibrationTrace).cast::<u8>(),
                size_of::<CalibrationTrace>(),
            )
        };
        read_remote(game, address, bytes)?;
        if snapshot.magic != 0x4a41_4341 || snapshot.version != CalibrationTrace::VERSION {
            return Err("calibration trace version differs".into());
        }
        let next = snapshot.next.load(std::sync::atomic::Ordering::Acquire);
        let in_flight = snapshot
            .in_flight
            .load(std::sync::atomic::Ordering::Acquire);
        if in_flight == 0 && prior == Some(next) && snapshot.complete_rows().is_ok() {
            stable += 1;
            if stable >= 3 {
                return Ok(snapshot);
            }
        } else {
            stable = 0;
        }
        prior = Some(next);
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("calibration calls did not drain or lost an original return".into())
}

fn drained_assign_calibration_trace(
    game: &Handle,
    address: usize,
) -> Result<Box<AssignCalibrationTrace>, String> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut stable = 0u32;
    let mut prior = None;
    while Instant::now() < deadline {
        let mut snapshot = AssignCalibrationTrace::boxed();
        let bytes = unsafe {
            std::slice::from_raw_parts_mut(
                (&mut *snapshot as *mut AssignCalibrationTrace).cast::<u8>(),
                size_of::<AssignCalibrationTrace>(),
            )
        };
        read_remote(game, address, bytes)?;
        if snapshot.magic != 0x4a41_4353 || snapshot.version != AssignCalibrationTrace::VERSION {
            return Err("assignment trace version differs".into());
        }
        let next = snapshot.next.load(std::sync::atomic::Ordering::Acquire);
        let in_flight = snapshot
            .in_flight
            .load(std::sync::atomic::Ordering::Acquire);
        if in_flight == 0 && prior == Some(next) && snapshot.complete_rows().is_ok() {
            stable += 1;
            if stable >= 3 {
                return Ok(snapshot);
            }
        } else {
            stable = 0;
        }
        prior = Some(next);
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("assignment calls did not drain or lost an original return".into())
}

fn read_property_trace(game: &Handle, address: usize) -> Result<Box<PropertyTrace>, String> {
    let mut snapshot = Box::new(PropertyTrace::default());
    let bytes = unsafe {
        std::slice::from_raw_parts_mut(
            (&mut *snapshot as *mut PropertyTrace).cast::<u8>(),
            size_of::<PropertyTrace>(),
        )
    };
    read_remote(game, address, bytes)?;
    if snapshot.magic != 0x4a41_4350 || snapshot.version != PropertyTrace::VERSION {
        return Err("property trace version differs".into());
    }
    Ok(snapshot)
}

fn drained_property_trace(game: &Handle, address: usize) -> Result<Box<PropertyTrace>, String> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut stable = 0u32;
    let mut prior = None;
    while Instant::now() < deadline {
        let snapshot = read_property_trace(game, address)?;
        let next = snapshot.next.load(std::sync::atomic::Ordering::Acquire);
        let in_flight = snapshot
            .in_flight
            .load(std::sync::atomic::Ordering::Acquire);
        if in_flight == 0 && prior == Some(next) && snapshot.snapshot_rows().is_ok() {
            stable += 1;
            if stable >= 3 {
                return Ok(snapshot);
            }
        } else {
            stable = 0;
        }
        prior = Some(next);
        std::thread::sleep(Duration::from_millis(50));
    }
    Err("property getter trace did not drain or contains dropped/unfinished rows".into())
}

pub fn run() -> Result<(), String> {
    let parsed = args(&std::env::args().skip(1).collect::<Vec<_>>())?;
    let chain_pid = std::env::var("CIV6_LIVE_CHAIN_PID")
        .map_err(|_| "live_chain PID environment missing")?
        .parse::<u32>()
        .map_err(|_| "invalid live_chain PID")?;
    let controller_pid = std::env::var("CIV6_AUTOTEST_CONTROLLER_PID")
        .map_err(|_| "runner parent PID environment missing")?
        .parse::<u32>()
        .map_err(|_| "invalid runner parent PID")?;
    if controller_pid == 0 || parent_pid(unsafe { GetCurrentProcessId() })? != controller_pid {
        return Err("runner is not this controller's OS parent".into());
    }
    let runner = open_process(
        controller_pid,
        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
    )?;
    let expected_runner = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("civ6-autotest.exe");
    if !alive(&runner) || !same_path(&process_path(&runner)?, &expected_runner) {
        return Err("pinned runner process identity changed".into());
    }
    let chain = open_process(
        chain_pid,
        PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
    )?;
    verify_lease(chain_pid, &chain, parsed.duration)?;
    let game = open_process(
        parsed.pid,
        PROCESS_QUERY_LIMITED_INFORMATION
            | PROCESS_SYNCHRONIZE
            | PROCESS_CREATE_THREAD
            | PROCESS_VM_OPERATION
            | PROCESS_VM_WRITE
            | PROCESS_VM_READ
            | PROCESS_TERMINATE
            | PROCESS_SET_QUOTA,
    )?;
    if !alive(&game) || process_created(&game)? != parsed.created {
        return Err("pinned game identity differs".into());
    }
    let target = pinned_target(parsed.pid, &game, parsed.mode)?;
    let original = pinned_entry_bytes(&game, target, parsed.mode)?;
    let promotion_window = if parsed.mode == BridgeMode::PromotionCall {
        Some(pinned_promotion_window(&game, target)?)
    } else {
        None
    };
    let property_target = if matches!(
        parsed.mode,
        BridgeMode::JoinedAdmission | BridgeMode::ProductionAdmission
    ) {
        Some(target - PINNED_CAN_EVER_RVA + PINNED_PROPERTY_GET_RVA)
    } else if parsed.mode.is_property() {
        Some(target)
    } else {
        None
    };
    let property_original = if matches!(
        parsed.mode,
        BridgeMode::JoinedAdmission | BridgeMode::ProductionAdmission
    ) {
        Some(pinned_entry_bytes(
            &game,
            property_target.unwrap(),
            BridgeMode::PropertyAdmissionScoped,
        )?)
    } else {
        None
    };
    let original_prefix = property_target
        .map(|property| pinned_property_prefix(&game, property))
        .transpose()?;
    let dll = bridge_dll()?;
    let bridge_sha = sha256(&dll)?;
    let (install_rva, uninstall_rva) = exported_rvas(&dll)?;
    let loader = remote_load_library(parsed.pid)?;
    let output = File::create_new(&parsed.out).map_err(|e| format!("new JSONL output: {e}"))?;
    let mut writer = BufWriter::new(output);
    let mut identity = json!({"kind":"identity","pid":parsed.pid,"created_filetime":parsed.created,
            "gamecore_sha256":PINNED_GAMECORE_SHA256,"mode":match parsed.mode {
                BridgeMode::CanEver => "can-ever", BridgeMode::Property => "property",
                BridgeMode::PropertyScoped => "property-scoped",
                BridgeMode::CanEverCalibration => "can-ever-calibration",
                BridgeMode::PropertyAdmissionScoped => "property-admission-scoped",
                BridgeMode::AssignCalibration => "assign-calibration", BridgeMode::JoinedAdmission => "joined-admission",
                BridgeMode::ProductionAdmission => "production-admission",
                BridgeMode::PromotionCall => "promotion-call"},
            "entry_rva":parsed.mode.rva(),
            "entry_bytes":format!("{:02x?}",original),"bridge_sha256":bridge_sha,
            "duration_limit_seconds":parsed.duration});
    if matches!(
        parsed.mode,
        BridgeMode::CanEver
            | BridgeMode::CanEverCalibration
            | BridgeMode::JoinedAdmission
            | BridgeMode::ProductionAdmission
    ) {
        identity["can_ever_rva"] = json!(PINNED_CAN_EVER_RVA);
        if matches!(
            parsed.mode,
            BridgeMode::JoinedAdmission | BridgeMode::ProductionAdmission
        ) {
            identity["property_rva"] = json!(PINNED_PROPERTY_GET_RVA);
            identity["property_entry_bytes"] =
                json!(format!("{:02x?}", property_original.unwrap()));
            identity["prefix_bytes"] = json!(format!("{:02x?}", original_prefix.unwrap()));
            if parsed.mode == BridgeMode::ProductionAdmission {
                identity["scope_key_ranges"] =
                    json!([[900000, 900255], [901000, 901255], [903000, 903255]]);
                identity["reset_key_id"] = json!(902000);
            } else {
                identity["scope_key_ids"] = json!([ASHINA_ADMISSION_KEY_ID]);
            }
        }
    } else if parsed.mode == BridgeMode::PromotionCall {
        identity["call_window"] = json!(format!("{:02x?}", promotion_window.unwrap()));
        identity["original_callee_rva"] = json!(PINNED_PROMOTION_CALLEE_RVA);
        identity["original_return_site_rva"] = json!(PINNED_PROMOTION_CALL_RVA + 5);
        identity["observation_scope"] = json!("one_official_promote_call_entry_only");
    } else if parsed.mode.is_property() {
        identity["prefix_bytes"] = json!(format!("{:02x?}", original_prefix.unwrap()));
        if parsed.mode == BridgeMode::PropertyScoped {
            identity["scope_key_ids"] =
                json!([PROPERTY_TEST_VALUE_KEY_ID, PROPERTY_TEST_PHASE_KEY_ID]);
        } else if parsed.mode == BridgeMode::PropertyAdmissionScoped {
            identity["scope_key_ids"] = json!([ASHINA_ADMISSION_KEY_ID]);
        }
    }
    write_json(&mut writer, identity)?;
    // All remotely executing or writing operations are below this point.
    // A failed assignment is a no-mutation refusal; an assigned job remains
    // armed across every call, including timeouts and controller crashes.
    let mut job = OwnedJob::attach(&game, parsed.pid, parsed.created)?;
    let outcome = (|| -> Result<(), String> {
        let path_wide = wide(&dll.to_string_lossy());
        let path_bytes = unsafe {
            std::slice::from_raw_parts(path_wide.as_ptr().cast::<u8>(), path_wide.len() * 2)
        };
        let path_remote = remote_alloc(&game, path_bytes.len())?;
        write_remote(&game, path_remote, path_bytes)?;
        let _ = remote_call(&game, loader, path_remote, "LoadLibraryW")?;
        let bridge = modules(parsed.pid)?
            .into_iter()
            .find(|m| same_path(&m.path, &dll))
            .ok_or("bridge DLL not loaded into the pinned game")?;
        if install_rva >= bridge.size || uninstall_rva >= bridge.size {
            return Err("bridge exports outside remote DLL image".into());
        }
        let trace_remote = remote_alloc(&game, parsed.mode.trace_size())?;
        let mut config = match parsed.mode {
            BridgeMode::AssignCalibration => BridgeConfig::for_assign_calibration(
                parsed.pid,
                parsed.created,
                trace_remote as u64,
            ),
            BridgeMode::CanEver => {
                BridgeConfig::new(parsed.pid, parsed.created, trace_remote as u64)
            }
            BridgeMode::Property => {
                BridgeConfig::for_property(parsed.pid, parsed.created, trace_remote as u64)
            }
            BridgeMode::PropertyScoped => {
                BridgeConfig::for_property_scoped(parsed.pid, parsed.created, trace_remote as u64)
            }
            BridgeMode::CanEverCalibration => {
                BridgeConfig::for_calibration(parsed.pid, parsed.created, trace_remote as u64)
            }
            BridgeMode::PropertyAdmissionScoped => BridgeConfig::for_admission_property(
                parsed.pid,
                parsed.created,
                trace_remote as u64,
            ),
            BridgeMode::JoinedAdmission => {
                BridgeConfig::for_joined(parsed.pid, parsed.created, trace_remote as u64)
            }
            BridgeMode::ProductionAdmission => {
                BridgeConfig::for_production(parsed.pid, parsed.created, trace_remote as u64)
            }
            BridgeMode::PromotionCall => {
                BridgeConfig::for_promotion_call(parsed.pid, parsed.created, trace_remote as u64)
            }
        };
        config.max_seconds = parsed.duration;
        let config_bytes = unsafe {
            std::slice::from_raw_parts(
                (&config as *const BridgeConfig).cast::<u8>(),
                size_of::<BridgeConfig>(),
            )
        };
        let config_remote = remote_alloc(&game, config_bytes.len())?;
        write_remote(&game, config_remote, config_bytes)?;
        let install_status = remote_call(
            &game,
            bridge.base + install_rva,
            config_remote,
            "BridgeInstall",
        )?;
        write_json(
            &mut writer,
            json!({"kind":"install","status":install_status}),
        )?;
        if install_status != BridgeStatus::Ok as u32 {
            return Err(
                "bridge installation refused; a critical status requires closing the pinned game"
                    .into(),
            );
        }
        let installed_prefix = if let Some(property_target) = property_target {
            let property_bytes = property_original.unwrap_or(original);
            let mut prefix = [0u8; PROPERTY_PREFIX_LEN];
            read_remote(&game, property_target - PROPERTY_PREFIX_LEN, &mut prefix)?;
            let mut entry = [0u8; 8];
            read_remote(&game, property_target, &mut entry)?;
            if prefix[0] != 0xe9
                || prefix == original_prefix.unwrap()
                || entry[..2] != [0xeb, 0xf9]
                || entry[2..] != property_bytes[2..]
            {
                return Err("property hotpatch did not preserve entry+2..+7".into());
            }
            if matches!(
                parsed.mode,
                BridgeMode::JoinedAdmission | BridgeMode::ProductionAdmission
            ) {
                let mut can_entry = [0u8; 8];
                read_remote(&game, target, &mut can_entry)?;
                if can_entry[0] != 0xe9 || can_entry[5..] != original[5..] {
                    return Err("joined CanEver entry changed outside its first instruction".into());
                }
            }
            Some(prefix)
        } else {
            None
        };
        if parsed.mode == BridgeMode::PromotionCall {
            let mut patched = [0u8; 16];
            read_remote(&game, target, &mut patched)?;
            let original = promotion_window.unwrap();
            if patched[0] != 0xe8
                || patched[1..5] == original[1..5]
                || patched[5..] != original[5..]
            {
                return Err("promotion call exchange changed opcode or neighbor bytes".into());
            }
        }
        println!("READY pid={} max_seconds={}", parsed.pid, parsed.duration);
        std::io::stdout().flush().map_err(|e| e.to_string())?;
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let result = std::io::stdin().lock().read_line(&mut line);
            let _ = sender.send(result.map(|_| line));
        });
        let deadline = Instant::now() + Duration::from_secs(u64::from(parsed.duration));
        let mut control_error = None;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break;
            }
            match receiver.recv_timeout(remaining.min(Duration::from_secs(1))) {
                Ok(Ok(line)) if line.trim() == "stop" => break,
                Ok(_) => {
                    control_error = Some("bridge stop pipe closed or invalid".to_string());
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    control_error = Some("bridge stop pipe disconnected".to_string());
                    break;
                }
            }
            if verify_lease(chain_pid, &chain, 0).is_err()
                || !alive(&game)
                || process_created(&game) != Ok(parsed.created)
            {
                control_error = Some("bridge lost its pinned game or live_chain lease".into());
                break;
            }
        }
        let uninstall_status =
            remote_call(&game, bridge.base + uninstall_rva, 0, "BridgeUninstall")?;
        if uninstall_status != BridgeStatus::Ok as u32 {
            return Err("bridge entry was not restored; close the pinned game".into());
        }
        let mut restored = [0u8; 8];
        read_remote(&game, target, &mut restored)?;
        if restored != original {
            return Err("original GameCore entry differs after remote uninstall returned".into());
        }
        if let Some(original_window) = promotion_window {
            let mut restored_window = [0u8; 16];
            read_remote(&game, target, &mut restored_window)?;
            if restored_window != original_window {
                return Err("complete promotion call window differs after uninstall".into());
            }
        }
        if let Some(patched) = installed_prefix {
            let mut retained = [0u8; PROPERTY_PREFIX_LEN];
            let property_target = property_target.unwrap();
            read_remote(&game, property_target - PROPERTY_PREFIX_LEN, &mut retained)?;
            if retained != patched {
                return Err("property prefix changed before game exit".into());
            }
            if let Some(property_original) = property_original {
                let mut restored_property = [0u8; 8];
                read_remote(&game, property_target, &mut restored_property)?;
                if restored_property != property_original {
                    return Err("joined property entry was not restored".into());
                }
            }
        }
        write_json(
            &mut writer,
            json!({"kind":"uninstall","status":uninstall_status,
                "entry_restored":true,
                "prefix_retained_until_exit":installed_prefix.is_some()}),
        )?;
        let (row_count, dropped, in_flight, observed_calls, filtered_calls) = match parsed.mode {
            BridgeMode::PromotionCall => {
                let snapshot = drained_promotion_trace(&game, trace_remote)?;
                let rows = snapshot.complete_rows().map_err(str::to_string)?;
                let base = target - PINNED_PROMOTION_CALL_RVA;
                for row in &rows {
                    if row.return_address != (base + PINNED_PROMOTION_CALL_RVA + 5) as u64
                        || row.thread_id == 0
                    {
                        return Err("promotion gateway did not retain the exact call return".into());
                    }
                    write_json(
                        &mut writer,
                        json!({"kind":"promotion_call_entry",
                        "sequence":row.sequence,"thread_id":row.thread_id,
                        "raw_rcx":format!("0x{:016x}",row.rcx),
                        "raw_rdx":format!("0x{:016x}",row.rdx),
                        "raw_r8":format!("0x{:016x}",row.r8),
                        "raw_r9":format!("0x{:016x}",row.r9),
                        "return_site_rva":PINNED_PROMOTION_CALL_RVA + 5,
                        "entry_only":true}),
                    )?;
                }
                (
                    rows.len(),
                    0,
                    0,
                    snapshot.next.load(std::sync::atomic::Ordering::Acquire),
                    0,
                )
            }
            BridgeMode::AssignCalibration => {
                let snapshot = drained_assign_calibration_trace(&game, trace_remote)?;
                let rows = snapshot.complete_rows().map_err(str::to_string)?;
                let gamecore_size = modules(parsed.pid)?
                    .into_iter()
                    .find(|m| same_path(&m.path, Path::new(GAMECORE)))
                    .ok_or("GameCore unloaded before assignment collection")?
                    .size;
                for row in &rows {
                    let return_rva = (row.return_address as usize)
                        .checked_sub(target - parsed.mode.rva())
                        .filter(|rva| *rva < gamecore_size);
                    write_json(
                        &mut writer,
                        json!({"kind":"assign_calibration", "sequence":row.sequence,
                        "thread_id":row.thread_id,"read_flags":row.read_flags,
                        "governors_this":format!("0x{:016x}",row.governors_this),
                        "governor_pointer":format!("0x{:016x}",row.governor_pointer),
                        "city_pointer":format!("0x{:016x}",row.city_pointer),
                        "raw_r9":format!("0x{:016x}",row.raw_r9),
                        "player_address":format!("0x{:016x}",row.player_address),
                        "player_id":row.player_id,"governor_index":row.governor_index,
                        "city_a8":row.city_a8,"city_c0":row.city_c0,"city_c4":row.city_c4,"city_d8":row.city_d8,
                        "return_site_rva":return_rva,"original_al":row.original_al,"effective_al":row.original_al,
                        "original_rax":format!("0x{:016x}",row.original_rax),
                        "promotion_snapshot_status":row.promotion_status,
                        "promotion_owner":row.promotion_owner,"promotion_index":row.promotion_index,
                        "promotion_count":row.promotion_count,
                        "owned_promotion_indices":(0..row.promotion_count.min(civ6_native_bridge_rs::governor_payment::PROMOTION_LIMIT as u32))
                            .filter(|index| row.promotion_owned[*index as usize / 64] & (1 << (index % 64)) != 0)
                            .collect::<Vec<_>>(),
                        "payment_authority":false}),
                    )?;
                }
                (
                    rows.len(),
                    snapshot.dropped.load(std::sync::atomic::Ordering::Acquire),
                    snapshot
                        .in_flight
                        .load(std::sync::atomic::Ordering::Acquire),
                    snapshot.next.load(std::sync::atomic::Ordering::Acquire),
                    0,
                )
            }
            BridgeMode::CanEver => {
                let snapshot = drained_trace(&game, trace_remote)?;
                let rows = snapshot.complete_rows().map_err(str::to_string)?;
                let gamecore_size = modules(parsed.pid)?
                    .into_iter()
                    .find(|m| same_path(&m.path, Path::new(GAMECORE)))
                    .ok_or("GameCore unloaded before trace collection")?
                    .size;
                for row in &rows {
                    let return_rva = (row.return_address as usize)
                        .checked_sub(target - parsed.mode.rva())
                        .filter(|rva| *rva < gamecore_size);
                    write_json(
                        &mut writer,
                        json!({"kind":"entry","sequence":row.sequence,
                        "governor_index":row.governor_index,
                        "governors_this":format!("0x{:x}",row.governors_this),
                        "return_site_rva":return_rva}),
                    )?;
                }
                (
                    rows.len(),
                    snapshot.dropped.load(std::sync::atomic::Ordering::Acquire),
                    snapshot
                        .in_flight
                        .load(std::sync::atomic::Ordering::Acquire),
                    snapshot.next.load(std::sync::atomic::Ordering::Acquire),
                    0,
                )
            }
            BridgeMode::CanEverCalibration => {
                let snapshot = drained_calibration_trace(&game, trace_remote)?;
                let rows = snapshot.complete_rows().map_err(str::to_string)?;
                let gamecore_size = modules(parsed.pid)?
                    .into_iter()
                    .find(|m| same_path(&m.path, Path::new(GAMECORE)))
                    .ok_or("GameCore unloaded before calibration collection")?
                    .size;
                for row in &rows {
                    let return_rva = (row.return_address as usize)
                        .checked_sub(target - parsed.mode.rva())
                        .filter(|rva| *rva < gamecore_size);
                    write_json(
                        &mut writer,
                        json!({"kind":"calibration","sequence":row.sequence,
                            "governor_index":row.governor_index,
                            "governors_this":format!("0x{:016x}",row.governors_this),
                            "return_site_rva":return_rva,
                            "thread_id":row.thread_id,
                            "owner_read_state":row.owner_read_state,
                            "player_address":format!("0x{:016x}",row.player_address),
                            "player_id":row.player_id,
                            "property_receiver":format!("0x{:016x}",row.property_receiver),
                            "original_al":row.original_al,
                            "effective_al":row.effective_al}),
                    )?;
                }
                (
                    rows.len(),
                    snapshot.dropped.load(std::sync::atomic::Ordering::Acquire),
                    snapshot
                        .in_flight
                        .load(std::sync::atomic::Ordering::Acquire),
                    snapshot.next.load(std::sync::atomic::Ordering::Acquire),
                    0,
                )
            }
            BridgeMode::Property
            | BridgeMode::PropertyScoped
            | BridgeMode::PropertyAdmissionScoped => {
                let snapshot = drained_property_trace(&game, trace_remote)?;
                if snapshot.scope != parsed.mode as u32
                    || (parsed.mode == BridgeMode::Property
                        && snapshot.filtered.load(std::sync::atomic::Ordering::Acquire) != 0)
                {
                    return Err("property trace scope differs from controller mode".into());
                }
                let rows = snapshot.snapshot_rows().map_err(str::to_string)?;
                for row in &rows {
                    let copied = row.read_state == 1;
                    write_json(
                        &mut writer,
                        json!({"kind":"property_get","sequence":row.sequence,
                        "property":format!("0x{:016x}",row.property),"key_id":row.key_id,
                        "returned":format!("0x{:016x}",row.returned),"thread_id":row.thread_id,
                        "read_state":row.read_state,
                        "tag":copied.then_some(row.tag),
                        "payload8":copied.then(|| format!("0x{:016x}",row.payload)),
                        "flags14":copied.then_some(row.flags)}),
                    )?;
                }
                (
                    rows.len(),
                    snapshot.dropped.load(std::sync::atomic::Ordering::Acquire),
                    snapshot
                        .in_flight
                        .load(std::sync::atomic::Ordering::Acquire),
                    snapshot.next.load(std::sync::atomic::Ordering::Acquire),
                    snapshot.filtered.load(std::sync::atomic::Ordering::Acquire),
                )
            }
            BridgeMode::JoinedAdmission | BridgeMode::ProductionAdmission => {
                let calibration = drained_calibration_trace(
                    &game,
                    trace_remote + std::mem::offset_of!(JoinedTrace, calibration),
                )?;
                let properties = drained_property_trace(
                    &game,
                    trace_remote + std::mem::offset_of!(JoinedTrace, property),
                )?;
                if properties.scope != parsed.mode as u32 {
                    return Err("joined property scope differs".into());
                }
                let calls = calibration.complete_rows().map_err(str::to_string)?;
                let values = properties.complete_rows().map_err(str::to_string)?;
                for row in &calls {
                    let return_rva =
                        (row.return_address as usize).checked_sub(target - parsed.mode.rva());
                    write_json(
                        &mut writer,
                        json!({"kind":"calibration",
                        "sequence":row.sequence,"governor_index":row.governor_index,
                        "governors_this":format!("0x{:016x}",row.governors_this),
                        "return_site_rva":return_rva,"thread_id":row.thread_id,
                        "owner_read_state":row.owner_read_state,
                        "player_address":format!("0x{:016x}",row.player_address),
                        "player_id":row.player_id,
                        "property_receiver":format!("0x{:016x}",row.property_receiver),
                        "original_al":row.original_al,"effective_al":row.effective_al}),
                    )?;
                }
                for row in &values {
                    let copied = row.read_state == 1;
                    write_json(
                        &mut writer,
                        json!({"kind":"property_get",
                        "sequence":row.sequence,"property":format!("0x{:016x}",row.property),
                        "key_id":row.key_id,"returned":format!("0x{:016x}",row.returned),
                        "thread_id":row.thread_id,"read_state":row.read_state,
                        "tag":copied.then_some(row.tag),
                        "payload8":copied.then(|| format!("0x{:016x}",row.payload)),
                        "flags14":copied.then_some(row.flags),
"denial_owner":row.denial_owner,"denial_index":row.denial_index,
"denial_checked":row.denial_checked == 1}),
                    )?;
                }
                (
                    calls.len() + values.len(),
                    calibration
                        .dropped
                        .load(std::sync::atomic::Ordering::Acquire)
                        + properties
                            .dropped
                            .load(std::sync::atomic::Ordering::Acquire),
                    calibration
                        .in_flight
                        .load(std::sync::atomic::Ordering::Acquire)
                        + properties
                            .in_flight
                            .load(std::sync::atomic::Ordering::Acquire),
                    calibration.next.load(std::sync::atomic::Ordering::Acquire)
                        + properties.next.load(std::sync::atomic::Ordering::Acquire),
                    properties
                        .filtered
                        .load(std::sync::atomic::Ordering::Acquire),
                )
            }
        };
        let result = if control_error.is_some() {
            "error"
        } else if parsed.mode.is_property() && dropped != 0 {
            "partial_raw"
        } else if parsed.mode.is_property() {
            "raw_collected"
        } else {
            "ok"
        };
        write_json(
            &mut writer,
            json!({"kind":"stop","rows":row_count,
            "dropped":dropped,
            "observed_calls":observed_calls,
            "filtered_calls":filtered_calls,
            "getter_calls_seen":u64::from(observed_calls) + u64::from(filtered_calls),
            "counter_scope":if parsed.mode == BridgeMode::ProductionAdmission {
                "bounded_prefix_samples_not_complete_call_history"
            } else { "snapshot_observation_not_complete_call_history" },
            "calibration_sample_limit":CALIBRATION_CAPACITY,
            "property_sample_limit":TRACE_CAPACITY,
            "in_flight":in_flight,
            "snapshot_stable":true,
            "quiescence_proven":false,
            "classification":match parsed.mode {
                BridgeMode::CanEver => "can_ever_entry",
                BridgeMode::CanEverCalibration => "can_ever_calibration",
                BridgeMode::AssignCalibration => "assign_calibration",
                BridgeMode::JoinedAdmission => "joined_admission",
                BridgeMode::ProductionAdmission => "production_admission",
                BridgeMode::PromotionCall => "promotion_call_entry_only",
                _ => "raw_collection",
            },
            "prefix_retained_until_exit":installed_prefix.is_some(),
            "result":result}),
        )?;
        if let Some(error) = control_error {
            return Err(error);
        }
        Ok(())
    })();
    if let Err(error) = outcome {
        let wait_ms = if verify_lease(chain_pid, &chain, 0).is_ok() {
            RECOVERY_SECONDS * 1000
        } else {
            0
        };
        return Err(fail_owned_call(&mut job, &game, error, wait_ms));
    }
    // The original entry is restored. In property mode the prefix is still a
    // published branch and stays mapped along with the DLL/relay/trace until
    // game exit; no quiescence proof permits restoring those five bytes here.
    job.disarm_after_verified_entry_restoration()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::os::windows::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::Ordering;

    struct SleepingChild(Child);
    impl SleepingChild {
        fn start() -> Self {
            let child = Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "Start-Sleep -Seconds 20",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(0x0800_0000)
                .spawn()
                .unwrap();
            Self(child)
        }
        fn handle(&self, rights: u32) -> Handle {
            open_process(self.0.id(), rights).unwrap()
        }
    }
    impl Drop for SleepingChild {
        fn drop(&mut self) {
            if self.0.try_wait().ok().flatten().is_none() {
                let _ = self.0.kill();
            }
            let _ = self.0.wait();
        }
    }

    const OWNED_TEST_RIGHTS: u32 = PROCESS_QUERY_LIMITED_INFORMATION
        | PROCESS_SYNCHRONIZE
        | PROCESS_TERMINATE
        | PROCESS_SET_QUOTA;

    #[test]
    fn module_snapshot_retries_only_bad_length_and_has_a_finite_limit() {
        let calls = Cell::new(0);
        let value = retry_module_snapshot(|| {
            calls.set(calls.get() + 1);
            if calls.get() < 3 {
                Err(std::io::Error::from_raw_os_error(ERROR_BAD_LENGTH as i32))
            } else {
                Ok(853u32)
            }
        })
        .unwrap();
        assert_eq!((value, calls.get()), (853, 3));
        calls.set(0);
        assert!(retry_module_snapshot::<u32>(|| {
            calls.set(calls.get() + 1);
            Err(std::io::Error::from_raw_os_error(5))
        })
        .is_err());
        assert_eq!(calls.get(), 1);
        calls.set(0);
        assert!(retry_module_snapshot::<u32>(|| {
            calls.set(calls.get() + 1);
            Err(std::io::Error::from_raw_os_error(ERROR_BAD_LENGTH as i32))
        })
        .is_err());
        assert_eq!(calls.get(), 16);
    }

    #[test]
    fn unexpected_controller_drop_kills_only_its_owned_job_target() {
        let target = SleepingChild::start();
        let peer = SleepingChild::start();
        let target_handle = target.handle(OWNED_TEST_RIGHTS);
        let peer_handle = peer.handle(PROCESS_SYNCHRONIZE);
        let job = OwnedJob::attach(
            &target_handle,
            target.0.id(),
            process_created(&target_handle).unwrap(),
        )
        .unwrap();
        assert!(alive(&target_handle));
        drop(job);
        assert_eq!(
            unsafe { WaitForSingleObject(target_handle.0, 5_000) },
            WAIT_OBJECT_0
        );
        assert!(alive(&peer_handle));
    }

    #[test]
    fn fixture_job_owner_abrupt_exit() {
        let Ok(pid_text) = std::env::var("CIV6_BRIDGE_TEST_TARGET_PID") else {
            return;
        };
        let pid: u32 = pid_text.parse().unwrap();
        let created: u64 = std::env::var("CIV6_BRIDGE_TEST_TARGET_CREATED")
            .unwrap()
            .parse()
            .unwrap();
        let parent = parent_pid(unsafe { GetCurrentProcessId() }).unwrap();
        assert_eq!(parent_pid(pid).unwrap(), parent);
        let target = open_process(pid, OWNED_TEST_RIGHTS).unwrap();
        let _job = OwnedJob::attach(&target, pid, created).unwrap();
        // Bypass Rust destructors. Windows must close the sole job handle.
        std::process::exit(0);
    }

    #[test]
    fn kernel_closes_pinned_target_after_owner_process_exits_abruptly() {
        let target = SleepingChild::start();
        let peer = SleepingChild::start();
        let target_handle = target.handle(OWNED_TEST_RIGHTS);
        let peer_handle = peer.handle(PROCESS_SYNCHRONIZE);
        let mut controller = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "windows::tests::fixture_job_owner_abrupt_exit",
                "--nocapture",
            ])
            .env("CIV6_BRIDGE_TEST_TARGET_PID", target.0.id().to_string())
            .env(
                "CIV6_BRIDGE_TEST_TARGET_CREATED",
                process_created(&target_handle).unwrap().to_string(),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .creation_flags(0x0800_0000)
            .spawn()
            .unwrap();
        let status = controller.wait().unwrap();
        if !status.success() {
            let mut output = String::new();
            if let Some(mut stderr) = controller.stderr.take() {
                stderr.read_to_string(&mut output).unwrap();
            }
            panic!("fixture controller failed: {output}");
        }
        assert_eq!(
            unsafe { WaitForSingleObject(target_handle.0, 5_000) },
            WAIT_OBJECT_0
        );
        assert!(alive(&peer_handle));
    }

    #[test]
    fn synthetic_remote_timeout_closes_exact_target_and_retains_peer() {
        let target = SleepingChild::start();
        let peer = SleepingChild::start();
        let target_handle = target.handle(OWNED_TEST_RIGHTS);
        let peer_handle = peer.handle(PROCESS_SYNCHRONIZE);
        let mut job = OwnedJob::attach(
            &target_handle,
            target.0.id(),
            process_created(&target_handle).unwrap(),
        )
        .unwrap();
        let failure = fail_owned_call(
            &mut job,
            &target_handle,
            "synthetic BridgeInstall remote timeout".into(),
            5_000,
        );
        assert!(failure.contains("contained target cleanup=Ok(())"));
        assert_eq!(
            unsafe { WaitForSingleObject(target_handle.0, 0) },
            WAIT_OBJECT_0
        );
        assert!(alive(&peer_handle));
    }

    #[test]
    fn restored_fixture_can_disarm_without_closing_target() {
        let target = SleepingChild::start();
        let target_handle = target.handle(OWNED_TEST_RIGHTS);
        let job = OwnedJob::attach(
            &target_handle,
            target.0.id(),
            process_created(&target_handle).unwrap(),
        )
        .unwrap();
        job.disarm_after_verified_entry_restoration().unwrap();
        drop(job);
        assert!(alive(&target_handle));
    }

    #[test]
    fn restored_joined_job_can_transfer_the_same_fixture_to_assignment_job() {
        let target = SleepingChild::start();
        let peer = SleepingChild::start();
        let target_handle = target.handle(OWNED_TEST_RIGHTS);
        let peer_handle = peer.handle(PROCESS_SYNCHRONIZE);
        let created = process_created(&target_handle).unwrap();
        let joined = OwnedJob::attach(&target_handle, target.0.id(), created).unwrap();
        joined.disarm_after_verified_entry_restoration().unwrap();
        drop(joined);
        assert!(alive(&target_handle));
        let assignment = OwnedJob::attach(&target_handle, target.0.id(), created).unwrap();
        assert!(alive(&target_handle));
        drop(assignment);
        assert_eq!(
            unsafe { WaitForSingleObject(target_handle.0, 5_000) },
            WAIT_OBJECT_0
        );
        assert!(alive(&peer_handle));
    }

    #[test]
    fn missing_assignment_rights_fail_before_target_mutation() {
        let target = SleepingChild::start();
        let target_handle = target.handle(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE);
        let assigned = OwnedJob::attach(
            &target_handle,
            target.0.id(),
            process_created(&target_handle).unwrap(),
        );
        assert!(assigned.is_err());
        assert!(alive(&target_handle));
    }

    #[test]
    fn drain_waits_for_a_published_slot_in_an_owned_process() {
        let process = open_process(unsafe { GetCurrentProcessId() }, PROCESS_VM_READ).unwrap();
        let trace = Box::new(SharedTrace::default());
        let address = (&*trace as *const SharedTrace) as usize;
        trace.next.store(1, Ordering::Release);
        trace.in_flight.store(1, Ordering::Release);
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            trace.rows[0].governor_index.store(42, Ordering::Release);
            trace.rows[0].sequence.store(1, Ordering::Release);
            trace.in_flight.store(0, Ordering::Release);
            trace
        });
        let snapshot = drained_trace(&process, address).unwrap();
        assert_eq!(snapshot.complete_rows().unwrap()[0].governor_index, 42);
        writer.join().unwrap();
    }

    #[test]
    fn controller_accepts_current_calibration_layout_and_rejects_old_version() {
        let process = open_process(unsafe { GetCurrentProcessId() }, PROCESS_VM_READ).unwrap();
        let mut trace = Box::new(CalibrationTrace::default());
        let address = (&*trace as *const CalibrationTrace) as usize;
        let snapshot = drained_calibration_trace(&process, address).unwrap();
        assert_eq!(snapshot.version, trace.version);
        trace.version -= 1;
        assert_eq!(
            drained_calibration_trace(&process, address).err().unwrap(),
            "calibration trace version differs"
        );
    }

    #[test]
    fn assignment_controller_checks_current_trace_version_and_mode_ordinal() {
        let process = open_process(unsafe { GetCurrentProcessId() }, PROCESS_VM_READ).unwrap();
        let mut trace = AssignCalibrationTrace::boxed();
        let address = (&*trace as *const AssignCalibrationTrace) as usize;
        assert_eq!(
            drained_assign_calibration_trace(&process, address)
                .unwrap()
                .version,
            AssignCalibrationTrace::VERSION
        );
        trace.version = 0;
        assert_eq!(
            drained_assign_calibration_trace(&process, address)
                .err()
                .unwrap(),
            "assignment trace version differs"
        );
        let config = BridgeConfig::for_assign_calibration(7, 123, address as u64);
        assert_eq!(config.mode, 7);
        assert_eq!(
            config.trace_size as usize,
            size_of::<AssignCalibrationTrace>()
        );
        assert_eq!(BridgeMode::AssignCalibration.rva(), 0x2dd880);
    }

    #[test]
    fn overflowing_property_snapshot_keeps_all_published_rows_and_drop_count() {
        let process = open_process(unsafe { GetCurrentProcessId() }, PROCESS_VM_READ).unwrap();
        let trace = Box::new(PropertyTrace::default());
        for row in &trace.rows {
            row.sequence.store(1, Ordering::Release);
        }
        trace.next.store(
            civ6_native_bridge_rs::TRACE_CAPACITY as u32 + 2,
            Ordering::Release,
        );
        trace.dropped.store(2, Ordering::Release);
        let address = (&*trace as *const PropertyTrace) as usize;
        let snapshot = drained_property_trace(&process, address).unwrap();
        assert_eq!(
            snapshot.snapshot_rows().unwrap().len(),
            civ6_native_bridge_rs::TRACE_CAPACITY
        );
        assert_eq!(snapshot.dropped.load(Ordering::Acquire), 2);
        assert!(snapshot.complete_rows().is_err());
    }

    #[test]
    fn bounded_arguments_reject_unpinned_or_overlong_runs_before_process_access() {
        let base = [
            "--pid",
            "7",
            "--created-filetime",
            "1",
            "--duration",
            "300",
            "--out",
            "C:/missing/trace.jsonl",
        ];
        assert!(args(&base.map(str::to_string)).is_err());
        let mut bad = base.map(str::to_string);
        bad[5] = "301".into();
        assert!(args(&bad).is_err());
        bad[4] = "--pid".into();
        assert!(args(&bad).is_err());
        let dir = tempfile::tempdir().unwrap();
        let mut valid = base.map(str::to_string).to_vec();
        valid[7] = dir.path().join("new.jsonl").to_string_lossy().into_owned();
        valid.extend(["--mode".into(), "property".into()]);
        assert_eq!(args(&valid).unwrap().mode, BridgeMode::Property);
        valid[9] = "unknown".into();
        assert!(args(&valid).is_err());
        valid[9] = "property-scoped".into();
        assert_eq!(args(&valid).unwrap().mode, BridgeMode::PropertyScoped);
    }

    #[test]
    fn remote_loader_reaches_only_the_own_test_process() {
        let pid = unsafe { GetCurrentProcessId() };
        let process = open_process(
            pid,
            PROCESS_QUERY_LIMITED_INFORMATION
                | PROCESS_SYNCHRONIZE
                | PROCESS_CREATE_THREAD
                | PROCESS_VM_OPERATION
                | PROCESS_VM_WRITE
                | PROCESS_VM_READ,
        )
        .unwrap();
        let system_dll = modules(pid)
            .unwrap()
            .into_iter()
            .find(|m| {
                m.path
                    .file_name()
                    .is_some_and(|name| name.eq_ignore_ascii_case("kernel32.dll"))
            })
            .expect("kernel32 must already be loaded in this test process")
            .path;
        let loader = remote_load_library(pid).unwrap();
        let name = wide(&system_dll.to_string_lossy());
        let bytes =
            unsafe { std::slice::from_raw_parts(name.as_ptr().cast::<u8>(), name.len() * 2) };
        let path_remote = remote_alloc(&process, bytes.len()).unwrap();
        write_remote(&process, path_remote, bytes).unwrap();
        let _ = remote_call(&process, loader, path_remote, "test LoadLibraryW").unwrap();
        assert!(modules(pid)
            .unwrap()
            .into_iter()
            .any(|m| same_path(&m.path, &system_dll)));
    }
}
