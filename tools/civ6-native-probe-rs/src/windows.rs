//! Windows x64 hardware breakpoint observer. No target memory or executable
//! bytes are written. The only target changes are per-thread debug registers,
//! which are snapshotted and restored before detach.
use crate::return_correlation::{EntryInput, EntryKind, Incomplete, ReturnCorrelation};

// Reuse this debugger's private attach identity, pinned handles and cleanup gate.
// Existing governor/property entry points remain withheld.
#[cfg(feature = "bootstrap-order")]
#[path = "../../civ6-pe-rs/src/order_windows.rs"]
pub mod order_capture;
use crate::startup_contract::{
    CHILD_READY as ATTACH_READY_BUDGET, CRITICAL_WAIT as CRITICAL_CLOSE_BUDGET,
    RESTORE as RESTORE_BUDGET,
};
use crate::{
    bool_from_al, direct_call_rva, is_pinned_dx11_exe, known_path, CAN_APPOINT_RVA, CAN_EVER_RVA,
    EXPECTED_SHA256, GAME_ROOT, MAX_ROWS,
};
use civ6_native_admission_rs::{owner_fields, MemoryRead, PropertyCapture};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, File},
    io::{BufRead, BufWriter, Read, Write},
    mem::{size_of, zeroed},
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Wdk::System::Threading::ThreadQuerySetWin32StartAddress;
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, DBG_CONTINUE, DBG_EXCEPTION_NOT_HANDLED, EXCEPTION_BREAKPOINT,
        EXCEPTION_SINGLE_STEP, FILETIME, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    System::{
        Diagnostics::{
            Debug::{
                ContinueDebugEvent, DebugActiveProcess, DebugActiveProcessStop,
                DebugSetProcessKillOnExit, GetThreadContext, ReadProcessMemory, SetThreadContext,
                WaitForDebugEventEx, CONTEXT, CONTEXT_CONTROL_AMD64, CONTEXT_DEBUG_REGISTERS_AMD64,
                CONTEXT_INTEGER_AMD64, CREATE_PROCESS_DEBUG_EVENT, CREATE_THREAD_DEBUG_EVENT,
                DEBUG_EVENT, EXCEPTION_DEBUG_EVENT, EXIT_PROCESS_DEBUG_EVENT,
                EXIT_THREAD_DEBUG_EVENT, LOAD_DLL_DEBUG_EVENT, UNLOAD_DLL_DEBUG_EVENT,
            },
            ToolHelp::{
                CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW,
                Process32NextW, MODULEENTRY32W, PROCESSENTRY32W, TH32CS_SNAPMODULE,
                TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
            },
        },
        LibraryLoader::{GetModuleHandleW, GetProcAddress},
        Threading::{
            GetExitCodeProcess, GetProcessIdOfThread, GetProcessTimes, GetThreadId, OpenProcess,
            OpenThread, QueryFullProcessImageNameW, ResumeThread, SuspendThread,
            WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
            PROCESS_VM_READ, THREAD_GET_CONTEXT, THREAD_QUERY_INFORMATION,
            THREAD_QUERY_LIMITED_INFORMATION, THREAD_SET_CONTEXT, THREAD_SUSPEND_RESUME,
            THREAD_SYNCHRONIZE,
        },
    },
};

const MODULE_REL: &str = "DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll";
const EVER_FILE_OFFSET: usize = 0x2dd1d0;
const APPOINT_FILE_OFFSET: usize = 0x2dcc20;
const PLAYER_GETTER_RVA: u64 = 0x6e_dc76;
const PROPERTY_NAME_RVA: u64 = 0x1_5339;
const PROPERTY_BEFORE_RVA: u64 = 0x1_535e;
const PROPERTY_AFTER_RVA: u64 = 0x1_5361;
// Exact instruction starts from LLVM disassembly of EXPECTED_SHA256. A byte
// match starting in the middle of an instruction is not an executable site.
const PINNED_BREAKPOINT_SITES: [(u64, usize, &[u8]); 6] = [
    (
        CAN_EVER_RVA,
        EVER_FILE_OFFSET,
        &[0x48, 0x89, 0x5c, 0x24, 0x08],
    ),
    (
        CAN_APPOINT_RVA,
        APPOINT_FILE_OFFSET,
        &[0x48, 0x89, 0x5c, 0x24, 0x08],
    ),
    (
        PLAYER_GETTER_RVA,
        0x6ed076,
        &[0x4c, 0x8d, 0x80, 0x28, 0x05, 0x00, 0x00],
    ),
    (PROPERTY_NAME_RVA, 0x14739, &[0x40, 0x38, 0x28]),
    (PROPERTY_BEFORE_RVA, 0x1475e, &[0xff, 0x50, 0x60]),
    (PROPERTY_AFTER_RVA, 0x14761, &[0x48, 0x85, 0xc0]),
];
fn is_pinned_instruction_start(rva: u64, bytes: &[u8]) -> bool {
    PINNED_BREAKPOINT_SITES
        .iter()
        .find(|(site, _, _)| *site == rva)
        .is_some_and(|(_, _, instruction)| bytes.starts_with(instruction))
}
const ACCESSOR_NUMBER: &str = "JAC_ACCESSOR_853_NUMBER";
const ACCESSOR_TEXT: &str = "JAC_ACCESSOR_853_TEXT";
const DEBUG_CONTEXT: u32 =
    CONTEXT_DEBUG_REGISTERS_AMD64 | CONTEXT_CONTROL_AMD64 | CONTEXT_INTEGER_AMD64;
// The Windows SDK declares AMD64 CONTEXT with DECLSPEC_ALIGN(16), while the
// windows-sys binding's bare CONTEXT has 8-byte Rust alignment.
#[repr(C, align(16))]
struct AlignedContext(CONTEXT);
const RF: u32 = 1 << 16;
const THREAD_RIGHTS: u32 = THREAD_GET_CONTEXT
    | THREAD_SET_CONTEXT
    | THREAD_SUSPEND_RESUME
    | THREAD_QUERY_INFORMATION
    | THREAD_SYNCHRONIZE;
const LIFECYCLE_THREAD_ROW_LIMIT: usize = 1024;
type NtQueryThreadFn =
    unsafe extern "system" fn(HANDLE, i32, *mut std::ffi::c_void, u32, *mut u32) -> i32;
#[derive(Clone, Copy, Debug)]
struct Win32StartQuery {
    address: Option<u64>,
    status: i32,
    returned_len: u32,
}
#[derive(Clone, Copy, Debug, Default)]
struct ThreadOriginObservation {
    rip: Option<u64>,
    win32_start: Option<u64>,
}
fn query_win32_start(handle: &Handle, query: NtQueryThreadFn) -> Win32StartQuery {
    let mut address: *mut std::ffi::c_void = std::ptr::null_mut();
    let mut returned_len = 0u32;
    let expected_len = size_of::<*mut std::ffi::c_void>() as u32;
    let status = unsafe {
        query(
            handle.0,
            ThreadQuerySetWin32StartAddress,
            &mut address as *mut _ as *mut _,
            expected_len,
            &mut returned_len,
        )
    };
    Win32StartQuery {
        address: (status == 0 && returned_len == expected_len && !address.is_null())
            .then_some(address as u64),
        status,
        returned_len,
    }
}
fn inspect_thread_origin(pid: u32, tid: u32, query: NtQueryThreadFn) -> ThreadOriginObservation {
    (|| {
        let handle = Handle::new(
            unsafe {
                OpenThread(
                    THREAD_GET_CONTEXT
                        | THREAD_QUERY_INFORMATION
                        | THREAD_QUERY_LIMITED_INFORMATION
                        | THREAD_SYNCHRONIZE,
                    0,
                    tid,
                )
            },
            "OpenThread(attach origin)",
        )
        .ok()?;
        if unsafe { GetThreadId(handle.0) } != tid
            || unsafe { GetProcessIdOfThread(handle.0) } != pid
            || unsafe { WaitForSingleObject(handle.0, 0) } != WAIT_TIMEOUT
        {
            return None;
        }
        Some(ThreadOriginObservation {
            rip: context(&handle).ok().map(|c| c.Rip),
            win32_start: query_win32_start(&handle, query).address,
        })
    })()
    .unwrap_or_default()
}
#[derive(Clone, Copy, Debug)]
struct CriticalExit {
    code: Option<u32>,
    source: &'static str,
}
fn queried_exit_code(game: &Handle) -> CriticalExit {
    let mut code = 0u32;
    if unsafe { GetExitCodeProcess(game.0, &mut code) } != 0 {
        CriticalExit {
            code: Some(code),
            source: "GetExitCodeProcess",
        }
    } else {
        CriticalExit {
            code: None,
            source: "unknown",
        }
    }
}
fn critical_exit_from_debug_event(event: &DEBUG_EVENT) -> Option<CriticalExit> {
    if event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
        Some(CriticalExit {
            code: Some(unsafe { event.u.ExitProcess.dwExitCode }),
            source: "exit_process_debug_event",
        })
    } else {
        None
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Keep pending Win32 event delivery, attach provenance and owned-process closure state explicit at critical recovery calls"
)]
fn announce_critical_and_wait_for_owned_game(
    game: &Handle,
    pid: u32,
    reason: &str,
    mut pending: Option<(DEBUG_EVENT, i32)>,
    debug_attached: bool,
    close_ack: Option<&AtomicBool>,
    mut attach_origin: Option<&mut AttachOrigin>,
    query_thread_start: Option<NtQueryThreadFn>,
    already_announced: bool,
) -> Result<CriticalExit, String> {
    if !already_announced {
        println!(
            "{}",
            json!({"critical_cleanup":true,"pid":pid,"reason":reason,"game_close_required":true})
        );
        let _ = std::io::stdout().flush();
    }
    let deadline = Instant::now() + CRITICAL_CLOSE_BUDGET;
    while Instant::now() < deadline {
        let target_exited = matches!(alive(game), Ok(false));
        // An exception with unverified restoration must remain pending until the controller has
        // successfully requested termination of this pinned game. This avoids
        // treating an application exception as handled while normal play runs.
        if !target_exited && close_ack.is_some_and(|ack| !ack.load(Ordering::SeqCst)) {
            thread::sleep(Duration::from_millis(50));
            continue;
        }
        // The controller can terminate a suspended debuggee, but Windows may
        // withhold process exit notification until the outstanding debug event
        // has been continued. Keep the debugger attached and all manually held
        // threads suspended while draining debug events through process exit.
        if let Some((event, status)) = pending {
            if unsafe { ContinueDebugEvent(event.dwProcessId, event.dwThreadId, status) } != 0 {
                pending = None;
                if let Some(exit) = critical_exit_from_debug_event(&event) {
                    return Ok(exit);
                }
            } else {
                thread::sleep(Duration::from_millis(50));
                continue;
            }
        }
        if target_exited {
            return Ok(queried_exit_code(game));
        }
        let mut event = DEBUG_EVENT::default();
        if debug_attached && unsafe { WaitForDebugEventEx(&mut event, 50) } != 0 {
            match event.dwDebugEventCode {
                CREATE_PROCESS_DEBUG_EVENT => {
                    let h = unsafe { event.u.CreateProcessInfo.hFile };
                    if !h.is_null() {
                        unsafe {
                            CloseHandle(h);
                        }
                    }
                }
                LOAD_DLL_DEBUG_EVENT => {
                    let h = unsafe { event.u.LoadDll.hFile };
                    if !h.is_null() {
                        unsafe {
                            CloseHandle(h);
                        }
                    }
                }
                CREATE_THREAD_DEBUG_EVENT => {
                    if let Some(origin) = attach_origin.as_deref_mut() {
                        let start = unsafe { event.u.CreateThread.lpStartAddress }
                            .map(|f| f as usize as u64);
                        origin.note_thread_start(event.dwThreadId, start);
                    }
                }
                EXIT_THREAD_DEBUG_EVENT => {
                    if let Some(origin) = attach_origin.as_deref_mut() {
                        origin.note_thread_exit(event.dwThreadId);
                    }
                }
                _ => {}
            }
            let status = if event.dwDebugEventCode == EXCEPTION_DEBUG_EVENT {
                let info = unsafe { event.u.Exception };
                let proven = info.ExceptionRecord.ExceptionCode == EXCEPTION_BREAKPOINT
                    && attach_origin.as_deref_mut().is_some_and(|origin| {
                        let observed = query_thread_start
                            .map(|query| inspect_thread_origin(pid, event.dwThreadId, query))
                            .unwrap_or_default();
                        origin.classify(
                            event.dwThreadId,
                            info.ExceptionRecord.ExceptionAddress as u64,
                            info.dwFirstChance != 0,
                            observed,
                        ) == BreakpointOrigin::SystemAttach
                    });
                if proven {
                    DBG_CONTINUE
                } else {
                    DBG_EXCEPTION_NOT_HANDLED
                }
            } else {
                DBG_CONTINUE
            };
            let exit = critical_exit_from_debug_event(&event);
            if unsafe { ContinueDebugEvent(event.dwProcessId, event.dwThreadId, status) } == 0 {
                pending = Some((event, status));
                continue;
            }
            if let Some(exit) = exit {
                return Ok(exit);
            }
        } else {
            thread::sleep(Duration::from_millis(50));
        }
        if let Ok(false) = alive(game) {
            return Ok(queried_exit_code(game));
        }
    }
    Err(format!("{reason}; critical_cleanup unresolved: controller did not close the owned game within 12 seconds"))
}

#[derive(Clone, Debug)]
pub struct ObserverConfig {
    pub pid: u32,
    pub created: u64,
    pub exe: PathBuf,
    pub chain_pid: u32,
    pub lock: PathBuf,
    pub controller_pid: u32,
    pub controller_created: u64,
    pub output: PathBuf,
    pub seconds: u64,
    pub mode: String,
}

struct Handle(HANDLE);
impl Handle {
    fn new(raw: HANDLE, action: &str) -> Result<Self, String> {
        if raw.is_null() || raw == INVALID_HANDLE_VALUE {
            Err(win_error(action))
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
fn win_error(action: &str) -> String {
    format!("{action}: {}", std::io::Error::last_os_error())
}
fn normal(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}
fn same_path(a: &Path, b: &Path) -> bool {
    normal(a) == normal(b)
}
fn wide_path(chars: &[u16]) -> Result<PathBuf, String> {
    let len = chars
        .iter()
        .position(|c| *c == 0)
        .ok_or("Windows path was truncated")?;
    Ok(PathBuf::from(OsString::from_wide(&chars[..len])))
}
fn process(pid: u32, read_memory: bool) -> Result<Handle, String> {
    if pid == 0 {
        return Err("zero PID is not allowed".into());
    }
    let rights = PROCESS_QUERY_LIMITED_INFORMATION
        | PROCESS_SYNCHRONIZE
        | if read_memory { PROCESS_VM_READ } else { 0 };
    Handle::new(unsafe { OpenProcess(rights, 0, pid) }, "OpenProcess")
}
fn process_created(handle: &Handle) -> Result<u64, String> {
    let (mut a, mut b, mut c, mut d): (FILETIME, FILETIME, FILETIME, FILETIME) =
        unsafe { zeroed() };
    if unsafe { GetProcessTimes(handle.0, &mut a, &mut b, &mut c, &mut d) } == 0 {
        return Err(win_error("GetProcessTimes"));
    }
    Ok((u64::from(a.dwHighDateTime) << 32) | u64::from(a.dwLowDateTime))
}
fn process_path(handle: &Handle) -> Result<PathBuf, String> {
    let mut buf = vec![0u16; 32768];
    let mut len = buf.len() as u32;
    if unsafe { QueryFullProcessImageNameW(handle.0, 0, buf.as_mut_ptr(), &mut len) } == 0 {
        return Err(win_error("QueryFullProcessImageNameW"));
    }
    if len == 0 || len as usize >= buf.len() {
        return Err("process image path truncated".into());
    }
    Ok(PathBuf::from(OsString::from_wide(&buf[..len as usize])))
}
fn alive(handle: &Handle) -> Result<bool, String> {
    match unsafe { WaitForSingleObject(handle.0, 0) } {
        WAIT_TIMEOUT => Ok(true),
        WAIT_OBJECT_0 => Ok(false),
        _ => Err(win_error("WaitForSingleObject")),
    }
}
fn exact_process(pid: u32, created: u64, path: &Path, read_memory: bool) -> Result<Handle, String> {
    let h = process(pid, read_memory)?;
    if !alive(&h)?
        || created == 0
        || process_created(&h)? != created
        || !same_path(&process_path(&h)?, path)
    {
        return Err(format!("process {pid} identity or liveness changed"));
    }
    Ok(h)
}
fn filetime_as_unix(v: u64) -> f64 {
    v as f64 / 10_000_000.0 - 11_644_473_600.0
}

fn verify_lock(lock: &Path, chain_pid: u32, chain: &Handle) -> Result<(), String> {
    verify_lock_for_image(
        lock,
        chain_pid,
        chain,
        Path::new("D:/CC/Tools/civ6-live/live_chain.exe"),
    )
}

// The default observer keeps its original image contract. The order observer
// supplies only an image selected and pinned by its fixed allowlist.
fn verify_lock_for_image(
    lock: &Path,
    chain_pid: u32,
    chain: &Handle,
    image: &Path,
) -> Result<(), String> {
    let expected = Path::new("D:/CC/Tools/civ6-live/civ6.lock");
    if !same_path(lock, expected) {
        return Err("standard live_chain lock path required".into());
    }
    let value: Value = serde_json::from_slice(
        &fs::read(lock).map_err(|e| format!("live_chain lock unavailable: {e}"))?,
    )
    .map_err(|_| "invalid live_chain lock JSON")?;
    if value["live_chain"] != true
        || value["chain_pid"].as_u64() != Some(u64::from(chain_pid))
        || value["pid"].as_u64() != Some(u64::from(chain_pid))
    {
        return Err("live_chain lock owner changed".into());
    }
    let acquired = value["acquired_at"]
        .as_f64()
        .ok_or("live_chain acquisition time missing")?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock unavailable")?
        .as_secs_f64();
    if !acquired.is_finite()
        || acquired > now
        || now >= acquired + 1800.0
        || filetime_as_unix(process_created(chain)?) > acquired + 1.0
    {
        return Err("live_chain 30 minute lease or identity is invalid".into());
    }
    if !alive(chain)? || !same_path(&process_path(chain)?, image) {
        return Err("live_chain process identity changed".into());
    }
    Ok(())
}

struct Module {
    base: u64,
    size: u64,
    path: PathBuf,
}
struct NtdllEvidence {
    path: PathBuf,
    disk_sha256: String,
    disk_len: u64,
    pe_timestamp: u32,
    image_size: u32,
    local_base: u64,
    remote_base: u64,
    breakpoint_rva: u64,
    breakin_rva: u64,
    query_rva: u64,
}
fn pe_image_metadata(path: &Path) -> Result<(u64, u32, u32), String> {
    use std::io::{Seek, SeekFrom};
    let mut file = File::open(path).map_err(|e| format!("PE image unavailable: {e}"))?;
    let file_len = file.metadata().map_err(|e| e.to_string())?.len();
    let mut dos = [0u8; 64];
    file.read_exact(&mut dos).map_err(|e| e.to_string())?;
    if &dos[..2] != b"MZ" {
        return Err("NTDLL disk image lacks an MZ header".into());
    }
    let pe_offset = u32::from_le_bytes(dos[0x3c..0x40].try_into().unwrap()) as u64;
    if pe_offset > 1_048_576 || pe_offset + 84 > file_len {
        return Err("NTDLL PE header offset is invalid".into());
    }
    file.seek(SeekFrom::Start(pe_offset))
        .map_err(|e| e.to_string())?;
    let mut pe = [0u8; 84];
    file.read_exact(&mut pe).map_err(|e| e.to_string())?;
    if &pe[..4] != b"PE\0\0"
        || u16::from_le_bytes(pe[4..6].try_into().unwrap()) != 0x8664
        || u16::from_le_bytes(pe[24..26].try_into().unwrap()) != 0x20b
    {
        return Err("NTDLL is not the expected x64 PE image".into());
    }
    let timestamp = u32::from_le_bytes(pe[8..12].try_into().unwrap());
    let image_size = u32::from_le_bytes(pe[80..84].try_into().unwrap());
    if image_size == 0 {
        return Err("NTDLL PE SizeOfImage is zero".into());
    }
    Ok((file_len, timestamp, image_size))
}
fn module_named(pid: u32, module_name: &str) -> Result<Module, String> {
    let snapshot = Handle::new(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) },
        "module snapshot",
    )?;
    let mut entry = MODULEENTRY32W {
        dwSize: size_of::<MODULEENTRY32W>() as u32,
        ..Default::default()
    };
    if unsafe { Module32FirstW(snapshot.0, &mut entry) } == 0 {
        return Err(win_error("Module32FirstW"));
    }
    loop {
        let path = wide_path(&entry.szExePath)?;
        if path
            .file_name()
            .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(module_name))
        {
            return Ok(Module {
                base: entry.modBaseAddr as u64,
                size: u64::from(entry.modBaseSize),
                path,
            });
        }
        if unsafe { Module32NextW(snapshot.0, &mut entry) } == 0 {
            break;
        }
    }
    Err(format!("{module_name} is not loaded in process {pid}"))
}
fn loaded_module(pid: u32, expected: &Path) -> Result<Module, String> {
    let module = module_named(pid, "GameCore_XP2_FinalRelease.dll")?;
    if !same_path(&module.path, expected) {
        return Err("loaded GameCore module is at an unexpected path".into());
    }
    Ok(module)
}
fn local_export_rva(module: &Module, handle: HANDLE, name: &[u8]) -> Result<u64, String> {
    let function = unsafe { GetProcAddress(handle, name.as_ptr()) }
        .ok_or_else(|| format!("ntdll export {} unavailable", String::from_utf8_lossy(name)))?;
    let address = function as usize as u64;
    address
        .checked_sub(module.base)
        .filter(|rva| *rva < module.size)
        .ok_or("ntdll export resolved outside its local module".into())
}
fn resolve_attach_origin(
    pid: u32,
) -> Result<(AttachOrigin, NtQueryThreadFn, NtdllEvidence), String> {
    let local = module_named(std::process::id(), "ntdll.dll")?;
    let remote = module_named(pid, "ntdll.dll")?;
    if !same_path(&local.path, &remote.path) || local.size != remote.size {
        return Err("local and target 64-bit ntdll module metadata differ".into());
    }
    let (disk_len, pe_timestamp, image_size) = pe_image_metadata(&local.path)?;
    if local.size != u64::from(image_size) || remote.size != u64::from(image_size) {
        return Err("local or target NTDLL image size differs from its x64 PE header".into());
    }
    let disk_sha256 = sha256(&local.path)?;
    let name: Vec<u16> = "ntdll.dll"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let handle = unsafe { GetModuleHandleW(name.as_ptr()) };
    if handle.is_null() || handle as u64 != local.base {
        return Err("local ntdll handle differs from module snapshot".into());
    }
    let breakpoint_rva = local_export_rva(&local, handle, b"DbgBreakPoint\0")?;
    let breakin_rva = local_export_rva(&local, handle, b"DbgUiRemoteBreakin\0")?;
    let query_rva = local_export_rva(&local, handle, b"NtQueryInformationThread\0")?;
    let query: NtQueryThreadFn = unsafe { std::mem::transmute((local.base + query_rva) as usize) };
    if unsafe { std::ptr::read((local.base + breakpoint_rva) as *const u8) } != 0xcc {
        return Err("local ntdll DbgBreakPoint is not an INT3 instruction".into());
    }
    if breakpoint_rva >= remote.size || breakin_rva >= remote.size {
        return Err("target ntdll export RVA exceeds its module size".into());
    }
    let evidence = NtdllEvidence {
        path: remote.path,
        disk_sha256,
        disk_len,
        pe_timestamp,
        image_size,
        local_base: local.base,
        remote_base: remote.base,
        breakpoint_rva,
        breakin_rva,
        query_rva,
    };
    Ok((
        AttachOrigin::new(
            remote
                .base
                .checked_add(breakpoint_rva)
                .ok_or("ntdll address overflow")?,
            remote
                .base
                .checked_add(breakin_rva)
                .ok_or("ntdll address overflow")?,
        ),
        query,
        evidence,
    ))
}
fn sha256(path: &Path) -> Result<String, String> {
    let mut input = File::open(path).map_err(|e| format!("module file unavailable: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = input
            .read(&mut buf)
            .map_err(|e| format!("module read failed: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
fn read_exact<const N: usize>(handle: &Handle, address: u64) -> Result<[u8; N], String> {
    let mut bytes = [0u8; N];
    let mut got = 0usize;
    if unsafe {
        ReadProcessMemory(
            handle.0,
            address as *const _,
            bytes.as_mut_ptr() as *mut _,
            N,
            &mut got,
        )
    } == 0
        || got != N
    {
        return Err(win_error(
            "ReadProcessMemory of target instruction/return address",
        ));
    }
    Ok(bytes)
}

struct RemoteMemory<'a>(&'a Handle);
impl MemoryRead for RemoteMemory<'_> {
    fn u64_at(&self, address: u64) -> Option<u64> {
        Some(u64::from_le_bytes(read_exact::<8>(self.0, address).ok()?))
    }
    fn u32_at(&self, address: u64) -> Option<u32> {
        Some(u32::from_le_bytes(read_exact::<4>(self.0, address).ok()?))
    }
}
fn verify_entry_bytes(game: &Handle, module: &Module) -> Result<(), String> {
    let mut file = File::open(&module.path).map_err(|e| e.to_string())?;
    use std::io::{Seek, SeekFrom};
    for (rva, offset, _) in PINNED_BREAKPOINT_SITES {
        if rva + 16 > module.size {
            return Err("GameCore module too small for target RVA".into());
        }
        file.seek(SeekFrom::Start(offset as u64))
            .map_err(|e| e.to_string())?;
        let mut disk = [0u8; 16];
        file.read_exact(&mut disk).map_err(|e| e.to_string())?;
        if !is_pinned_instruction_start(rva, &disk) {
            return Err(format!(
                "GameCore RVA 0x{rva:x} is not the pinned instruction start"
            ));
        }
        if read_exact::<16>(game, module.base + rva)? != disk {
            return Err(format!(
                "GameCore instruction bytes differ at RVA 0x{rva:x}"
            ));
        }
    }
    Ok(())
}

struct Verified {
    game: Handle,
    chain: Handle,
    controller: Handle,
    module: Module,
    config: ObserverConfig,
}
fn verify(config: ObserverConfig, mode: ProbeMode) -> Result<Verified, String> {
    if !(1..=crate::MAX_SECONDS).contains(&config.seconds) {
        return Err("native trace duration must be 1..300 seconds".into());
    }
    if mode == ProbeMode::Lifecycle && config.seconds > crate::MAX_LIFECYCLE_SECONDS {
        return Err("lifecycle observation is limited to 20 seconds".into());
    }
    if config.pid == config.chain_pid
        || config.pid == config.controller_pid
        || config.chain_pid == config.controller_pid
    {
        return Err("game, chain and controller PIDs must differ".into());
    }
    let expected_module = Path::new(GAME_ROOT).join(MODULE_REL);
    if !is_pinned_dx11_exe(&config.exe) {
        return Err("target is not the exact Steam DX11 Civ6 executable".into());
    }
    let chain = process(config.chain_pid, false)?;
    verify_lock(&config.lock, config.chain_pid, &chain)?;
    let controller = process(config.controller_pid, false)?;
    if !alive(&controller)? || process_created(&controller)? != config.controller_created {
        return Err("observer controller identity changed".into());
    }
    if parent_pid(std::process::id())? != config.controller_pid {
        return Err("observer controller is not the OS parent process".into());
    }
    let game = exact_process(
        config.pid,
        config.created,
        &config.exe,
        mode != ProbeMode::Lifecycle,
    )?;
    let module = loaded_module(config.pid, &expected_module)?;
    if sha256(&module.path)? != EXPECTED_SHA256 {
        return Err("GameCore SHA-256 differs from PR86 pinned build".into());
    }
    if mode != ProbeMode::Lifecycle {
        verify_entry_bytes(&game, &module)?;
    }
    Ok(Verified {
        game,
        chain,
        controller,
        module,
        config,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Registers {
    dr: [u64; 4],
    dr6: u64,
    dr7: u64,
}
impl Registers {
    fn from(c: &CONTEXT) -> Self {
        Self {
            dr: [c.Dr0, c.Dr1, c.Dr2, c.Dr3],
            dr6: c.Dr6,
            dr7: c.Dr7,
        }
    }
    fn write(self, c: &mut CONTEXT) {
        [c.Dr0, c.Dr1, c.Dr2, c.Dr3] = self.dr;
        c.Dr6 = self.dr6;
        c.Dr7 = self.dr7;
    }
}
fn context(thread: &Handle) -> Result<CONTEXT, String> {
    let mut c = AlignedContext(unsafe { zeroed() });
    c.0.ContextFlags = DEBUG_CONTEXT;
    if unsafe { GetThreadContext(thread.0, &mut c.0) } == 0 {
        return Err(win_error("GetThreadContext"));
    }
    Ok(c.0)
}
fn set_context(thread: &Handle, c: &CONTEXT) -> Result<(), String> {
    let mut changes = AlignedContext(*c);
    changes.0.ContextFlags = CONTEXT_DEBUG_REGISTERS_AMD64 | CONTEXT_CONTROL_AMD64;
    if unsafe { SetThreadContext(thread.0, &changes.0) } == 0 {
        return Err(win_error("SetThreadContext"));
    }
    Ok(())
}
fn installed_sites_match_context(sites: &[Option<u64>; 4], c: &CONTEXT) -> bool {
    let actual = [c.Dr0, c.Dr1, c.Dr2, c.Dr3];
    sites.iter().enumerate().all(|(slot, site)| {
        let enabled = c.Dr7 & (3u64 << (slot * 2));
        let condition = c.Dr7 & (0xfu64 << (16 + slot * 4));
        match site {
            Some(address) => {
                actual[slot] == *address && enabled == (1u64 << (slot * 2)) && condition == 0
            }
            None => actual[slot] == 0 && enabled == 0,
        }
    })
}
fn set_debug_registers(
    thread: &Handle,
    current: &CONTEXT,
    original: Registers,
) -> Result<(), String> {
    let mut changes = AlignedContext(*current);
    original.write(&mut changes.0);
    // Only the debug registers are restored. RIP/RSP, integer registers and
    // EFlags are read from the live thread and are never rolled back.
    changes.0.ContextFlags = CONTEXT_DEBUG_REGISTERS_AMD64;
    if unsafe { SetThreadContext(thread.0, &changes.0) } == 0 {
        return Err(win_error("SetThreadContext(debug registers)"));
    }
    Ok(())
}
fn thread_handle(tid: u32) -> Result<Handle, String> {
    Handle::new(unsafe { OpenThread(THREAD_RIGHTS, 0, tid) }, "OpenThread")
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SuspendFailureState {
    Gone,
    TargetExited,
    Unsignaled,
    Unknown,
}
fn suspend_failure_state(thread_wait: u32, game_wait: u32) -> SuspendFailureState {
    if thread_wait == WAIT_OBJECT_0 {
        SuspendFailureState::Gone
    } else if game_wait == WAIT_OBJECT_0 {
        SuspendFailureState::TargetExited
    } else if thread_wait == WAIT_TIMEOUT && game_wait == WAIT_TIMEOUT {
        SuspendFailureState::Unsignaled
    } else {
        SuspendFailureState::Unknown
    }
}

fn parent_pid(pid: u32) -> Result<u32, String> {
    let snapshot = Handle::new(
        unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) },
        "process snapshot",
    )?;
    let mut entry = PROCESSENTRY32W {
        dwSize: size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    if unsafe { Process32FirstW(snapshot.0, &mut entry) } == 0 {
        return Err(win_error("Process32FirstW"));
    }
    loop {
        if entry.th32ProcessID == pid {
            return Ok(entry.th32ParentProcessID);
        }
        if unsafe { Process32NextW(snapshot.0, &mut entry) } == 0 {
            break;
        }
    }
    Err(format!("process {pid} vanished from parent snapshot"))
}

struct ThreadState {
    handle: Handle,
    original: Registers,
    returns: ReturnCorrelation,
    armed_sites: [Option<u64>; 4],
    property_capture: PropertyCapture,
    property_owner_id: Option<u32>,
    property_name: Option<&'static str>,
    property_value_name: Option<&'static str>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OwnedTrap {
    CanEver,
    CanAppoint,
    Return,
    PlayerGetter,
    PropertyBefore,
    PropertyAfter,
    PropertyName,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DispatchOutcome {
    OwnedProven(OwnedTrap),
    Unrelated,
    Ambiguous(&'static str),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BreakpointOrigin {
    SystemAttach,
    Application,
}
#[derive(Clone, Copy, Debug)]
struct AttachOrigin {
    breakpoint_address: u64,
    breakin_start: u64,
    breakin_thread: Option<u32>,
    uncertain: bool,
    accepted: bool,
}
impl AttachOrigin {
    fn new(breakpoint_address: u64, breakin_start: u64) -> Self {
        Self {
            breakpoint_address,
            breakin_start,
            breakin_thread: None,
            uncertain: false,
            accepted: false,
        }
    }
    fn note_thread_start(&mut self, tid: u32, start: Option<u64>) {
        if self.accepted || start != Some(self.breakin_start) {
            return;
        }
        if self.breakin_thread.is_some() {
            self.uncertain = true;
        } else {
            self.breakin_thread = Some(tid);
        }
    }
    fn note_thread_exit(&mut self, tid: u32) {
        if !self.accepted && self.breakin_thread == Some(tid) {
            self.uncertain = true;
        }
    }
    fn classify(
        &mut self,
        tid: u32,
        address: u64,
        first_chance: bool,
        observed: ThreadOriginObservation,
    ) -> BreakpointOrigin {
        // DbgBreakPoint is a verified one-byte INT3. The Windows child-process
        // attach observes the stopped RIP immediately after that instruction.
        let rip_matches = self
            .breakpoint_address
            .checked_add(1)
            .is_some_and(|next| observed.rip == Some(next));
        if !self.accepted
            && !self.uncertain
            && first_chance
            && self.breakin_thread == Some(tid)
            && address == self.breakpoint_address
            && observed.win32_start == Some(self.breakin_start)
            && rip_matches
        {
            self.accepted = true;
            BreakpointOrigin::SystemAttach
        } else {
            BreakpointOrigin::Application
        }
    }
}
/// The debug event records where Windows reported the exception. DR6 is
/// useful diagnostic evidence, but it is not a prerequisite for recognizing
/// an address at which this debugger installed a hardware breakpoint.
fn classify_single_step(
    mode: ProbeMode,
    returns: &ReturnCorrelation,
    armed_sites: &[Option<u64>; 4],
    exception_address: u64,
    context: Option<&CONTEXT>,
) -> DispatchOutcome {
    let event_slot = armed_sites
        .iter()
        .position(|address| *address == Some(exception_address));
    let Some(context) = context else {
        return DispatchOutcome::Ambiguous("context_unavailable");
    };
    let rip_slot = armed_sites
        .iter()
        .position(|address| *address == Some(context.Rip));
    let Some(slot) = event_slot else {
        return if rip_slot.is_some() {
            DispatchOutcome::Ambiguous("exception_address_rip_disagree")
        } else {
            DispatchOutcome::Unrelated
        };
    };
    if rip_slot != Some(slot) {
        return DispatchOutcome::Ambiguous("exception_address_rip_disagree");
    }
    let actual_dr = match slot {
        0 => context.Dr0,
        1 => context.Dr1,
        2 => context.Dr2,
        _ => context.Dr3,
    };
    let enable = 1u64 << (slot * 2);
    let enable_pair = 3u64 << (slot * 2);
    let condition = 0xfu64 << (16 + slot * 4);
    if actual_dr != exception_address
        || context.Dr7 & enable_pair != enable
        || context.Dr7 & condition != 0
    {
        return DispatchOutcome::Ambiguous("armed_debug_registers_disagree");
    }
    match (mode, slot) {
        (ProbeMode::Property, 0) => DispatchOutcome::OwnedProven(OwnedTrap::PlayerGetter),
        (ProbeMode::Property, 1) => DispatchOutcome::OwnedProven(OwnedTrap::PropertyBefore),
        (ProbeMode::Property, 2) => DispatchOutcome::OwnedProven(OwnedTrap::PropertyAfter),
        (ProbeMode::Property, 3) => DispatchOutcome::OwnedProven(OwnedTrap::PropertyName),
        (ProbeMode::Governor, 0) => DispatchOutcome::OwnedProven(OwnedTrap::CanEver),
        (ProbeMode::Governor, 1) => DispatchOutcome::OwnedProven(OwnedTrap::CanAppoint),
        (ProbeMode::Governor, 2 | 3) if returns.has_site(exception_address) => {
            DispatchOutcome::OwnedProven(OwnedTrap::Return)
        }
        _ => DispatchOutcome::Ambiguous("return_breakpoint_has_no_pending_call"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProbeMode {
    Governor,
    Property,
    Lifecycle,
}
impl ProbeMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "governor" => Ok(Self::Governor),
            "property" => Ok(Self::Property),
            "lifecycle" => Ok(Self::Lifecycle),
            _ => Err("--mode must be governor, property or lifecycle".into()),
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Governor => "governor",
            Self::Property => "property",
            Self::Lifecycle => "lifecycle",
        }
    }
    fn fixed_sites(self, base: u64) -> [Option<u64>; 4] {
        match self {
            Self::Governor => [
                Some(base + CAN_EVER_RVA),
                Some(base + CAN_APPOINT_RVA),
                None,
                None,
            ],
            Self::Property => [
                Some(base + PLAYER_GETTER_RVA),
                Some(base + PROPERTY_BEFORE_RVA),
                Some(base + PROPERTY_AFTER_RVA),
                Some(base + PROPERTY_NAME_RVA),
            ],
            Self::Lifecycle => [None; 4],
        }
    }
}
struct Jsonl {
    writer: BufWriter<File>,
    rows: usize,
}

#[derive(Default)]
struct LifecycleEvidence {
    thread_events: usize,
    thread_ids: BTreeSet<u32>,
    context_reads: usize,
    unknown_thread_status: usize,
    exception_events: usize,
    application_exception_events: usize,
    exit_code: Option<u32>,
    exit_code_source: Option<&'static str>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalKind {
    Attached,
    Detached,
    TargetExited,
}
impl TerminalKind {
    fn label(self) -> &'static str {
        match self {
            Self::Attached => "attached_unresolved",
            Self::Detached => "detached",
            Self::TargetExited => "target_exited",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CleanupPhase {
    Freeze,
    Restore,
    Continue,
    Detach,
    Resume,
    Done,
}
trait CleanupOps {
    fn game_exited(&mut self) -> bool {
        false
    }
    fn freeze(&mut self) -> Result<(), String>;
    fn restore_verified(&mut self) -> Result<(), String>;
    fn continue_pending(&mut self) -> Result<(), String>;
    fn detach(&mut self) -> Result<(), String>;
    fn resume(&mut self) -> Result<(), String>;
}
struct CleanupGate {
    phase: CleanupPhase,
    deadline: Instant,
    last_error: String,
    slowest_step: Option<(CleanupPhase, Duration)>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AttachAbortOutcome {
    Detached,
    GameExited,
    KillDisabledWithoutDetach,
    Critical,
}
trait AttachAbortOps {
    fn detach(&mut self) -> bool;
    fn disable_kill_on_exit(&mut self) -> bool;
    fn game_alive(&mut self) -> Result<bool, String>;
}
struct AttachAbortGate {
    deadline: Instant,
    kill_disabled: bool,
}
impl AttachAbortGate {
    fn new() -> Self {
        Self {
            deadline: Instant::now() + RESTORE_BUDGET,
            kill_disabled: false,
        }
    }
    fn step(&mut self, ops: &mut impl AttachAbortOps) -> Option<AttachAbortOutcome> {
        if ops.detach() {
            return Some(AttachAbortOutcome::Detached);
        }
        if matches!(ops.game_alive(), Ok(false)) {
            return Some(AttachAbortOutcome::GameExited);
        }
        if ops.disable_kill_on_exit() {
            self.kill_disabled = true;
        }
        None
    }
    fn drive(&mut self, ops: &mut impl AttachAbortOps) -> AttachAbortOutcome {
        while Instant::now() < self.deadline {
            if let Some(outcome) = self.step(ops) {
                return outcome;
            }
            thread::sleep(Duration::from_millis(40));
        }
        if self.kill_disabled {
            AttachAbortOutcome::KillDisabledWithoutDetach
        } else {
            AttachAbortOutcome::Critical
        }
    }
}
struct WinAttachAbort<'a> {
    pid: u32,
    game: &'a Handle,
}
impl AttachAbortOps for WinAttachAbort<'_> {
    fn detach(&mut self) -> bool {
        (unsafe { DebugActiveProcessStop(self.pid) }) != 0
    }
    fn disable_kill_on_exit(&mut self) -> bool {
        (unsafe { DebugSetProcessKillOnExit(0) }) != 0
    }
    fn game_alive(&mut self) -> Result<bool, String> {
        alive(self.game)
    }
}
impl CleanupGate {
    fn new() -> Self {
        Self {
            phase: CleanupPhase::Freeze,
            deadline: Instant::now() + RESTORE_BUDGET,
            last_error: String::new(),
            slowest_step: None,
        }
    }
    fn step(&mut self, ops: &mut impl CleanupOps) -> Result<bool, String> {
        let started = Instant::now();
        let result = match self.phase {
            CleanupPhase::Freeze => ops.freeze(),
            CleanupPhase::Restore => ops.restore_verified(),
            CleanupPhase::Continue => ops.continue_pending(),
            CleanupPhase::Detach => ops.detach(),
            CleanupPhase::Resume => ops.resume(),
            CleanupPhase::Done => return Ok(true),
        };
        let elapsed = started.elapsed();
        if self
            .slowest_step
            .is_none_or(|(_, slowest)| elapsed > slowest)
        {
            self.slowest_step = Some((self.phase, elapsed));
        }
        if let Err(e) = result {
            if self.phase == CleanupPhase::Restore {
                self.phase = CleanupPhase::Freeze;
            }
            return Err(e);
        }
        self.phase = match self.phase {
            CleanupPhase::Freeze => CleanupPhase::Restore,
            CleanupPhase::Restore => CleanupPhase::Continue,
            CleanupPhase::Continue => CleanupPhase::Detach,
            CleanupPhase::Detach => CleanupPhase::Resume,
            CleanupPhase::Resume | CleanupPhase::Done => CleanupPhase::Done,
        };
        Ok(self.phase == CleanupPhase::Done)
    }
    fn drive(&mut self, ops: &mut impl CleanupOps) -> Result<(), String> {
        while Instant::now() < self.deadline {
            if ops.game_exited() {
                return Err("target_exited_during_cleanup".into());
            }
            let step = self.step(ops);
            // A synchronous OS call can return after the absolute deadline.
            // Even a successful final Resume must not convert that into ok.
            if Instant::now() >= self.deadline {
                if let Err(error) = step {
                    self.last_error = error;
                }
                break;
            }
            match step {
                Ok(true) => return Ok(()),
                Ok(false) => {}
                Err(e) => {
                    self.last_error = e;
                    thread::sleep(Duration::from_millis(40));
                }
            }
        }
        Err(format!(
            "critical_cleanup: {:?} did not finish before the cleanup deadline: {}; slowest step: {:?}",
            self.phase, self.last_error, self.slowest_step
        ))
    }
}
impl Jsonl {
    fn new(path: &Path) -> Result<Self, String> {
        let file = File::create_new(path).map_err(|e| format!("trace output must be new: {e}"))?;
        Ok(Self {
            writer: BufWriter::new(file),
            rows: 0,
        })
    }
    fn row(&mut self, value: Value) -> Result<(), String> {
        if self.rows >= MAX_ROWS {
            return Err("trace row limit reached".into());
        }
        serde_json::to_writer(&mut self.writer, &value).map_err(|e| e.to_string())?;
        self.writer.write_all(b"\n").map_err(|e| e.to_string())?;
        self.writer.flush().map_err(|e| e.to_string())?;
        self.rows += 1;
        Ok(())
    }
}

struct Debugger {
    pid: u32,
    game: Handle,
    module_base: u64,
    module_size: u64,
    mode: ProbeMode,
    attached_at: Instant,
    event_ordinal: u64,
    lifecycle: LifecycleEvidence,
    threads: BTreeMap<u32, ThreadState>,
    // Only threads whose debug registers we changed need suspension. Reuse
    // their already opened handles; reopening can be denied later even while
    // the original handle still has THREAD_SUSPEND_RESUME access.
    held: BTreeSet<u32>,
    cleanup_event: Option<(DEBUG_EVENT, i32)>,
    cleanup_gate: Option<CleanupGate>,
    critical: bool,
    #[cfg(test)]
    restore_faults: usize,
    #[cfg(test)]
    freeze_refused: bool,
    #[cfg(test)]
    critical_signal: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    output: Jsonl,
    next_id: u64,
    attached: bool,
    terminal: TerminalKind,
    ready: bool,
    attach_ready_deadline: Option<Instant>,
    attach_origin: AttachOrigin,
    query_thread_start: NtQueryThreadFn,
    ntdll_evidence: NtdllEvidence,
    last_attach_breakpoint_owned: bool,
    last_dispatch: DispatchOutcome,
    close_ack: Arc<AtomicBool>,
    #[cfg(test)]
    zero_context_dr6_for_test: bool,
    #[cfg(test)]
    force_context_failure: bool,
    #[cfg(test)]
    force_reported_rip_mismatch: bool,
    #[cfg(test)]
    force_initial_breakpoint_late: bool,
    #[cfg(test)]
    panic_on_owned_event: bool,
    #[cfg(test)]
    continue_faults: usize,
    #[cfg(test)]
    exit_continue_fault: bool,
}
impl Debugger {
    fn record_critical_exit(&mut self, exit: CriticalExit) {
        self.lifecycle.exit_code = exit.code;
        self.lifecycle.exit_code_source = Some(exit.source);
        let _ = self.output.row(json!({"type":"target_exit_observed",
            "exit_code":exit.code,"source":exit.source}));
    }
    fn classify_attach_breakpoint(
        &mut self,
        tid: u32,
        address: u64,
        first_chance: bool,
        observed: ThreadOriginObservation,
    ) -> bool {
        let owned = self
            .attach_origin
            .classify(tid, address, first_chance, observed)
            == BreakpointOrigin::SystemAttach;
        self.last_attach_breakpoint_owned = owned;
        owned
    }
    fn mark_attach_ready(&mut self) -> Result<(), String> {
        #[cfg(test)]
        if self.force_initial_breakpoint_late {
            self.attach_ready_deadline = Some(Instant::now() - Duration::from_millis(1));
        }
        if self.attach_deadline_expired() {
            return Err(
                "initial debugger breakpoint arrived after the attach ready deadline".into(),
            );
        }
        self.ready = true;
        Ok(())
    }
    fn stamp(&self, row: &mut Value) {
        row["event_ordinal"] = json!(self.event_ordinal);
        row["elapsed_ms"] = json!(self.attached_at.elapsed().as_millis());
        row["observed_unix_ms"] = json!(SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok()));
    }
    fn attach_deadline_expired(&self) -> bool {
        self.attach_ready_deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    }
    fn attach(
        pid: u32,
        game: Handle,
        module_base: u64,
        module_size: u64,
        output: Jsonl,
        mode: ProbeMode,
    ) -> Result<Self, String> {
        #[cfg(not(test))]
        if mode != ProbeMode::Lifecycle {
            return Err("hardware breakpoints are withheld until per-thread exception delivery is proven; use lifecycle mode".into());
        }
        // Refuse an attach that cannot distinguish the OS breakpoint from an
        // application's own breakpoint using supported module/thread metadata.
        let (mut attach_origin, query_thread_start, ntdll_evidence) = resolve_attach_origin(pid)?;
        if unsafe { DebugActiveProcess(pid) } == 0 {
            return Err(win_error("DebugActiveProcess"));
        }
        // The default is kill-on-debugger-exit. If changing it fails, this
        // thread may leave only after a confirmed detach or owned-game exit.
        if unsafe { DebugSetProcessKillOnExit(0) } == 0 {
            let error = win_error("DebugSetProcessKillOnExit(FALSE)");
            let outcome = AttachAbortGate::new().drive(&mut WinAttachAbort { pid, game: &game });
            return Err(match outcome {
                AttachAbortOutcome::Detached => format!("{error}; debugger detached before exit"),
                AttachAbortOutcome::GameExited => format!("{error}; owned game already exited"),
                AttachAbortOutcome::KillDisabledWithoutDetach => format!("{error}; explicit detach was not confirmed, but kill-on-exit is now disabled; no breakpoints were installed"),
                AttachAbortOutcome::Critical => match announce_critical_and_wait_for_owned_game(&game, pid, &error, None, true, None, Some(&mut attach_origin), Some(query_thread_start), false) {
                    Ok(exit) => format!("{error}; critical_cleanup: controller closed the owned game; exit code {:?} from {}", exit.code, exit.source),
                    Err(critical) => critical,
                },
            });
        }
        Ok(Self {
            pid,
            game,
            module_base,
            module_size,
            mode,
            attached_at: Instant::now(),
            event_ordinal: 0,
            lifecycle: LifecycleEvidence::default(),
            threads: BTreeMap::new(),
            held: BTreeSet::new(),
            cleanup_event: None,
            cleanup_gate: None,
            critical: false,
            #[cfg(test)]
            restore_faults: 0,
            #[cfg(test)]
            freeze_refused: false,
            #[cfg(test)]
            critical_signal: None,
            output,
            next_id: 1,
            attached: true,
            terminal: TerminalKind::Attached,
            ready: false,
            attach_ready_deadline: None,
            attach_origin,
            query_thread_start,
            ntdll_evidence,
            last_attach_breakpoint_owned: false,
            last_dispatch: DispatchOutcome::Unrelated,
            close_ack: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            zero_context_dr6_for_test: false,
            #[cfg(test)]
            force_context_failure: false,
            #[cfg(test)]
            force_reported_rip_mismatch: false,
            #[cfg(test)]
            force_initial_breakpoint_late: false,
            #[cfg(test)]
            panic_on_owned_event: false,
            #[cfg(test)]
            continue_faults: 0,
            #[cfg(test)]
            exit_continue_fault: false,
        })
    }
    fn lifecycle_thread_event(
        &mut self,
        tid: u32,
        source: &'static str,
        start_address: Option<u64>,
    ) -> Result<ThreadOriginObservation, String> {
        let mut observed = ThreadOriginObservation::default();
        self.lifecycle.thread_events += 1;
        if self.lifecycle.thread_ids.len() < LIFECYCLE_THREAD_ROW_LIMIT {
            self.lifecycle.thread_ids.insert(tid);
        }
        if self.lifecycle.thread_events > LIFECYCLE_THREAD_ROW_LIMIT {
            return Ok(observed);
        }
        let mut row = json!({"type":"thread_status","thread_id":tid,"source":source,
            "start_address_candidate":start_address.map(|v| format!("0x{v:016x}")),
            "attach_thread_candidate":self.attach_origin.breakin_thread == Some(tid),
            "handle_identity":"unknown","liveness":"unknown","context":"unknown",
            "future_exception_delivery":"unknown"});
        let handle = Handle::new(
            unsafe {
                OpenThread(
                    THREAD_GET_CONTEXT
                        | THREAD_QUERY_INFORMATION
                        | THREAD_QUERY_LIMITED_INFORMATION
                        | THREAD_SYNCHRONIZE,
                    0,
                    tid,
                )
            },
            "OpenThread(read-only lifecycle)",
        );
        match handle {
            Err(error) => {
                row["error"] = json!(error);
                self.lifecycle.unknown_thread_status += 1;
            }
            Ok(handle) => {
                let actual_tid = unsafe { GetThreadId(handle.0) };
                let owner_pid = unsafe { GetProcessIdOfThread(handle.0) };
                row["owner_pid"] = json!(owner_pid);
                let identity_matched = actual_tid == tid && owner_pid == self.pid;
                row["handle_identity"] = json!(if identity_matched {
                    "matched"
                } else {
                    "unknown"
                });
                let wait = unsafe { WaitForSingleObject(handle.0, 0) };
                row["liveness"] = json!(match wait {
                    WAIT_TIMEOUT => "unsignaled",
                    WAIT_OBJECT_0 => "exited",
                    _ => "unknown",
                });
                if identity_matched && wait == WAIT_TIMEOUT {
                    if source == "exception_debug_event"
                        || start_address == Some(self.attach_origin.breakin_start)
                    {
                        let query = query_win32_start(&handle, self.query_thread_start);
                        row["win32_start_query_status"] =
                            json!(format!("0x{:08x}", query.status as u32));
                        row["win32_start_query_returned_len"] = json!(query.returned_len);
                        row["win32_start_address"] =
                            json!(query.address.map(|v| format!("0x{v:016x}")));
                        observed.win32_start = query.address;
                        if query.address.is_none() {
                            self.lifecycle.unknown_thread_status += 1;
                        }
                    }
                    match context(&handle) {
                        Ok(c) => {
                            observed.rip = Some(c.Rip);
                            row["context"] = json!("read_at_debug_event");
                            row["rip"] = json!(format!("0x{:016x}", c.Rip));
                            row["dr"] = json!([c.Dr0, c.Dr1, c.Dr2, c.Dr3]);
                            row["dr6"] = json!(format!("0x{:016x}", c.Dr6));
                            row["dr7"] = json!(format!("0x{:016x}", c.Dr7));
                            self.lifecycle.context_reads += 1;
                        }
                        Err(error) => {
                            row["error"] = json!(error);
                            self.lifecycle.unknown_thread_status += 1;
                        }
                    }
                } else {
                    self.lifecycle.unknown_thread_status += 1;
                }
            }
        }
        self.stamp(&mut row);
        self.output.row(row)?;
        Ok(observed)
    }
    fn add_thread(
        &mut self,
        tid: u32,
        source: &'static str,
        start_address: Option<u64>,
    ) -> Result<(), String> {
        if self.mode == ProbeMode::Lifecycle {
            return self
                .lifecycle_thread_event(tid, source, start_address)
                .map(|_| ());
        }
        if self.threads.contains_key(&tid) {
            return Ok(());
        }
        let handle = thread_handle(tid)?;
        let mut c = context(&handle)?;
        let original = Registers::from(&c);
        if c.Dr7 & 0xff != 0 {
            return Err(format!("thread {tid} already uses hardware breakpoints"));
        }
        let armed_sites = self.mode.fixed_sites(self.module_base);
        match self.mode {
            ProbeMode::Governor => {
                c.Dr0 = self.module_base + CAN_EVER_RVA;
                c.Dr1 = self.module_base + CAN_APPOINT_RVA;
                c.Dr2 = 0;
                c.Dr3 = 0;
            }
            ProbeMode::Property => {
                c.Dr0 = self.module_base + PLAYER_GETTER_RVA;
                c.Dr1 = self.module_base + PROPERTY_BEFORE_RVA;
                c.Dr2 = self.module_base + PROPERTY_AFTER_RVA;
                c.Dr3 = self.module_base + PROPERTY_NAME_RVA;
            }
            ProbeMode::Lifecycle => unreachable!("lifecycle never writes debug registers"),
        }
        c.Dr6 &= !0xf;
        c.Dr7 = (c.Dr7 & !0xff & !(0xffff << 16))
            | if self.mode == ProbeMode::Property {
                0b01010101
            } else {
                0b0101
            };
        // Track the original registers before asking Windows to install ours.
        // Even an OS error after a partial write must enter the restore gate.
        self.threads.insert(
            tid,
            ThreadState {
                handle,
                original,
                returns: ReturnCorrelation::default(),
                armed_sites,
                property_capture: PropertyCapture::default(),
                property_owner_id: None,
                property_name: None,
                property_value_name: None,
            },
        );
        set_context(&self.threads[&tid].handle, &c)?;
        let after = context(&self.threads[&tid].handle)?;
        if !installed_sites_match_context(&armed_sites, &after) {
            return Err(format!(
                "thread {tid} hardware breakpoints differed after SetThreadContext"
            ));
        }
        Ok(())
    }
    fn refresh_returns(state: &mut ThreadState, c: &mut CONTEXT) -> Result<(), String> {
        let sites = state.returns.sites();
        if sites.len() > 2 {
            return Err("more than two distinct pending return sites".into());
        }
        c.Dr2 = sites.first().copied().unwrap_or(0);
        c.Dr3 = sites.get(1).copied().unwrap_or(0);
        c.Dr7 = (c.Dr7 & !0xf0)
            | if !sites.is_empty() { 0x10 } else { 0 }
            | if sites.len() > 1 { 0x40 } else { 0 };
        c.Dr6 &= !0xf;
        c.EFlags |= RF;
        set_context(&state.handle, c)?;
        state.armed_sites[2] = sites.first().copied();
        state.armed_sites[3] = sites.get(1).copied();
        Ok(())
    }
    fn entry(
        &mut self,
        tid: u32,
        c: &mut CONTEXT,
        entry: EntryKind,
        target_rva: u64,
    ) -> Result<(), String> {
        if self.mode != ProbeMode::Governor {
            return Err("governor return trap cannot be armed in property mode".into());
        }
        let return_address = u64::from_le_bytes(read_exact::<8>(&self.game, c.Rsp)?);
        let return_rva = return_address
            .checked_sub(self.module_base)
            .filter(|r| *r < self.module_size);
        let call_rva = return_rva.filter(|r| *r >= 5).and_then(|r| {
            let bytes = read_exact::<5>(&self.game, return_address - 5).ok()?;
            direct_call_rva(r, bytes, target_rva)
        });
        let expected_rsp = c.Rsp.checked_add(8).ok_or("stack pointer overflow")?;
        let id = self.next_id;
        self.next_id += 1;
        let state = self
            .threads
            .get_mut(&tid)
            .ok_or("debug event thread missing")?;
        let decision = state.returns.enter(EntryInput {
            id,
            entry,
            return_address,
            expected_rsp,
            this_pointer: c.Rcx,
            governor_index: c.Rdx as u32,
            direct_call_verified: call_rva.is_some(),
            return_site_valid: return_rva.is_some(),
        });
        Self::refresh_returns(state, c)?;
        self.log_incomplete(tid, decision.incomplete, None, None)?;
        let tail = decision.linked_parent.is_some();
        let path = known_path(call_rva, return_rva, tail);
        // The addresses and ID are candidates until a GameCore getter and
        // lifecycle trace confirm the same owner on each execution path.
        // Read only the three statically identified fields, once per hit.
        let owner = owner_fields(&RemoteMemory(&self.game), c.Rcx);
        let property_vtable = owner
            .and_then(|fields| RemoteMemory(&self.game).u64_at(fields.property_interface_address));
        self.output.row(
            json!({"type":"entry","id":id,"thread_id":tid,"function":entry.label(),
            "function_rva":target_rva,"return_site_rva":return_rva,"caller_rva":call_rva,
            "expected_rsp":format!("0x{expected_rsp:016x}"),
            "path_candidate":path,"tail_from_can_appoint":tail,
            "linked_tail_parent_id":decision.linked_parent,"this":format!("0x{:016x}",c.Rcx),
            "governor_index":c.Rdx as u32,"return_breakpoint_armed":decision.armed,
            "owner_id_candidate":owner.map(|x| x.player_id),
            "player_pointer_candidate":owner.map(|x| format!("0x{:016x}",x.player_address)),
            "property_interface_candidate":owner.map(|x| format!("0x{:016x}",x.property_interface_address)),
            "property_vtable_candidate":property_vtable.map(|x| format!("0x{x:016x}"))}),
        )?;
        if let Some(reason) = decision.unarmed_reason {
            self.log_incomplete(
                tid,
                vec![Incomplete { id, reason }],
                Some(return_address),
                Some(expected_rsp),
            )?;
        }
        Ok(())
    }
    fn returned(&mut self, tid: u32, c: &mut CONTEXT) -> Result<(), String> {
        if self.mode != ProbeMode::Governor {
            return Err("governor return trap cannot be rearmed in property mode".into());
        }
        let state = self
            .threads
            .get_mut(&tid)
            .ok_or("return event thread missing")?;
        let decision = state.returns.returned(c.Rip, c.Rsp);
        Self::refresh_returns(state, c)?;
        self.log_incomplete(tid, decision.incomplete, None, None)?;
        if decision.unmatched {
            self.output
                .row(json!({"type":"return_incomplete","thread_id":tid,
                "id":null,"reason":"return_without_matching_frame",
                "return_address":format!("0x{:016x}",c.Rip),"rsp":format!("0x{:016x}",c.Rsp)}))?;
        }
        for p in decision.complete {
            let al = c.Rax as u8;
            self.output.row(
                json!({"type":"return","id":p.id,"thread_id":tid,"function":p.entry.label(),
                "al":al,"bool_candidate":bool_from_al(al),
                "linked_tail_group_id":decision.linked_tail_group}),
            )?;
        }
        Ok(())
    }
    fn log_incomplete(
        &mut self,
        tid: u32,
        incomplete: Vec<Incomplete>,
        return_address: Option<u64>,
        rsp: Option<u64>,
    ) -> Result<(), String> {
        for item in incomplete {
            self.output
                .row(json!({"type":"return_incomplete","thread_id":tid,
                "id":item.id,"reason":item.reason,
                "return_address":return_address.map(|v| format!("0x{v:016x}")),
                "rsp":rsp.map(|v| format!("0x{v:016x}"))}))?;
        }
        Ok(())
    }
    fn invalidate_returns_for_exception(&mut self, tid: u32) -> Result<(), String> {
        let incomplete = self
            .threads
            .get_mut(&tid)
            .map(|state| state.returns.invalidate_for_exception())
            .unwrap_or_default();
        self.log_incomplete(tid, incomplete, None, None)
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "Record the raw Win32 exception and optional context separately from its ownership classification without inventing missing evidence"
    )]
    fn log_single_step(
        &mut self,
        tid: u32,
        code: i32,
        first_chance: bool,
        exception_address: u64,
        context: Option<&CONTEXT>,
        observed_dr6: Option<u64>,
        outcome: DispatchOutcome,
        reason: &str,
    ) -> Result<(), String> {
        self.output.row(json!({
            "type":"debug_exception", "thread_id":tid,
            "code":format!("0x{:08x}", code as u32), "first_chance":first_chance,
            "exception_address":format!("0x{exception_address:016x}"),
            "rip":context.map(|c| format!("0x{:016x}",c.Rip)),
            "dr6":observed_dr6.map(|v| format!("0x{v:016x}")),
            "dr7":context.map(|c| format!("0x{:016x}",c.Dr7)),
            "owned_breakpoint":matches!(outcome,DispatchOutcome::OwnedProven(_)),
            "dispatch":match outcome { DispatchOutcome::OwnedProven(_) => "owned_proven", DispatchOutcome::Unrelated => "unrelated", DispatchOutcome::Ambiguous(_) => "ambiguous" },
            "reason":reason,
        }))
    }
    fn property_hit(&mut self, tid: u32, c: &mut CONTEXT) -> Result<(), String> {
        if self.mode != ProbeMode::Property {
            return Err("property breakpoint received in governor mode".into());
        }
        let rva = c
            .Rip
            .checked_sub(self.module_base)
            .ok_or("property hit before module")?;
        let state = self
            .threads
            .get_mut(&tid)
            .ok_or("property event thread missing")?;
        let event = if rva == PLAYER_GETTER_RVA {
            state.property_capture.player_getter(c.Rax);
            state.property_name = None;
            state.property_value_name = None;
            let memory = RemoteMemory(&self.game);
            let owner_id = c.Rax.checked_add(0xd8).and_then(|at| memory.u32_at(at));
            state.property_owner_id = owner_id;
            let property_vtable = c.Rax.checked_add(0x528).and_then(|at| memory.u64_at(at));
            json!({"type":"player_getter","thread_id":tid,
                "owner_id_candidate":owner_id,
                "player_pointer_candidate":(c.Rax != 0).then(|| format!("0x{:016x}",c.Rax)),
                "property_interface_candidate":c.Rax.checked_add(0x528)
                    .filter(|_| c.Rax != 0).map(|p| format!("0x{p:016x}")),
                "property_vtable_candidate":property_vtable.map(|p| format!("0x{p:016x}"))})
        } else if rva == PROPERTY_NAME_RVA {
            let name = read_exact::<24>(&self.game, c.Rax).ok().and_then(|raw| {
                let len = raw.iter().position(|byte| *byte == 0)?;
                match std::str::from_utf8(&raw[..len]).ok()? {
                    ACCESSOR_NUMBER => Some(ACCESSOR_NUMBER),
                    ACCESSOR_TEXT => Some(ACCESSOR_TEXT),
                    _ => None,
                }
            });
            state.property_name = name;
            json!({"type":"property_name","thread_id":tid,"key_name_candidate":name})
        } else if rva == PROPERTY_BEFORE_RVA {
            let matched = state.property_capture.before_value(c.Rcx, c.Rdx as u32);
            state.property_value_name = if matched {
                state.property_name.take()
            } else {
                None
            };
            json!({"type":"property_before","thread_id":tid,
                "owner_id_candidate":state.property_owner_id,
                "property_interface_candidate":format!("0x{:016x}",c.Rcx),
                "key_id_candidate":c.Rdx as u32,"player_getter_matched":matched,
                "key_name_candidate":state.property_value_name})
        } else if rva == PROPERTY_AFTER_RVA {
            let key = state.property_capture.after_value();
            let name = state.property_value_name.take();
            let bytes = name
                .and_then(|_| (c.Rax != 0).then_some(c.Rax))
                .and_then(|pointer| read_exact::<16>(&self.game, pointer).ok())
                .map(|raw| raw.iter().map(|b| format!("{b:02x}")).collect::<String>());
            json!({"type":"property_after","thread_id":tid,
                "owner_id_candidate":state.property_owner_id,
                "key_id_candidate":key,
                "key_name_candidate":name,
                "value_pointer_candidate":(c.Rax != 0).then(|| format!("0x{:016x}",c.Rax)),
                "value_first_16_bytes_candidate":bytes})
        } else {
            return Err("unknown property-mode breakpoint".into());
        };
        c.Dr6 &= !0xf;
        c.EFlags |= RF;
        set_context(&state.handle, c)?;
        self.output.row(event)
    }
    fn handle_event(&mut self, event: &DEBUG_EVENT) -> Result<i32, String> {
        self.event_ordinal += 1;
        self.last_attach_breakpoint_owned = false;
        self.last_dispatch = DispatchOutcome::Unrelated;
        let tid = event.dwThreadId;
        match event.dwDebugEventCode {
            CREATE_PROCESS_DEBUG_EVENT => {
                let info = unsafe { event.u.CreateProcessInfo };
                if !info.hFile.is_null() {
                    unsafe {
                        CloseHandle(info.hFile);
                    }
                }
                self.add_thread(tid, "create_process_debug_event", None)?;
            }
            CREATE_THREAD_DEBUG_EVENT => {
                let start = unsafe { event.u.CreateThread.lpStartAddress };
                let start = start.map(|f| f as usize as u64);
                self.attach_origin.note_thread_start(tid, start);
                self.add_thread(tid, "create_thread_debug_event", start)?;
            }
            EXIT_THREAD_DEBUG_EVENT => {
                self.attach_origin.note_thread_exit(tid);
                self.threads.remove(&tid);
                if self.mode == ProbeMode::Lifecycle {
                    let code = unsafe { event.u.ExitThread.dwExitCode };
                    let mut row = json!({"type":"thread_exit","thread_id":tid,"exit_code":code});
                    self.stamp(&mut row);
                    self.output.row(row)?;
                }
            }
            LOAD_DLL_DEBUG_EVENT => {
                let info = unsafe { event.u.LoadDll };
                if !info.hFile.is_null() {
                    unsafe {
                        CloseHandle(info.hFile);
                    }
                }
            }
            EXIT_PROCESS_DEBUG_EVENT => {
                let code = unsafe { event.u.ExitProcess.dwExitCode };
                self.lifecycle.exit_code = Some(code);
                self.lifecycle.exit_code_source = Some("exit_process_debug_event");
                if self.mode == ProbeMode::Lifecycle {
                    let mut row = json!({"type":"process_exit","thread_id":tid,"exit_code":code});
                    self.stamp(&mut row);
                    self.output.row(row)?;
                }
                return Ok(DBG_CONTINUE);
            }
            UNLOAD_DLL_DEBUG_EVENT
                if unsafe { event.u.UnloadDll.lpBaseOfDll } as u64 == self.module_base =>
            {
                return Err("observed GameCore module unloaded".into());
            }
            EXCEPTION_DEBUG_EVENT => {
                let info = unsafe { event.u.Exception };
                let code = info.ExceptionRecord.ExceptionCode;
                if self.mode == ProbeMode::Lifecycle {
                    self.lifecycle.exception_events += 1;
                    let observed =
                        self.lifecycle_thread_event(tid, "exception_debug_event", None)?;
                    let initial = code == EXCEPTION_BREAKPOINT
                        && self.classify_attach_breakpoint(
                            tid,
                            info.ExceptionRecord.ExceptionAddress as u64,
                            info.dwFirstChance != 0,
                            observed,
                        );
                    if !initial {
                        self.lifecycle.application_exception_events += 1;
                    }
                    let mut row = json!({"type":"debug_exception","mode":"lifecycle",
                        "thread_id":tid,"code":format!("0x{code:08x}"),
                        "first_chance":info.dwFirstChance != 0,
                        "exception_address":format!("0x{:016x}", info.ExceptionRecord.ExceptionAddress as u64),
                        "stopped_rip":observed.rip.map(|v| format!("0x{v:016x}")),
                        "verified_win32_start":observed.win32_start.map(|v| format!("0x{v:016x}")),
                        "disposition":if initial { "proven_attach_breakpoint" } else { "application_handler" },
                        "attach_origin_proven":initial} );
                    self.stamp(&mut row);
                    self.output.row(row)?;
                    if initial {
                        self.mark_attach_ready()?;
                        return Ok(DBG_CONTINUE);
                    }
                    return Ok(DBG_EXCEPTION_NOT_HANDLED);
                }
                if code == EXCEPTION_BREAKPOINT {
                    let observed = inspect_thread_origin(self.pid, tid, self.query_thread_start);
                    let initial = self.classify_attach_breakpoint(
                        tid,
                        info.ExceptionRecord.ExceptionAddress as u64,
                        info.dwFirstChance != 0,
                        observed,
                    );
                    if initial {
                        self.mark_attach_ready()?;
                        return Ok(DBG_CONTINUE);
                    }
                    let c = self
                        .threads
                        .get(&tid)
                        .and_then(|state| context(&state.handle).ok());
                    self.log_single_step(
                        tid,
                        code,
                        info.dwFirstChance != 0,
                        info.ExceptionRecord.ExceptionAddress as u64,
                        c.as_ref(),
                        c.as_ref().map(|v| v.Dr6),
                        DispatchOutcome::Unrelated,
                        "unmatched_breakpoint",
                    )?;
                    self.invalidate_returns_for_exception(tid)?;
                    return Ok(DBG_EXCEPTION_NOT_HANDLED);
                } else if code == EXCEPTION_SINGLE_STEP {
                    let exception_address = info.ExceptionRecord.ExceptionAddress as u64;
                    let first_chance = info.dwFirstChance != 0;
                    let Some(state) = self.threads.get(&tid) else {
                        let outcome = if self
                            .mode
                            .fixed_sites(self.module_base)
                            .contains(&Some(exception_address))
                        {
                            DispatchOutcome::Ambiguous("thread_not_registered_at_target")
                        } else {
                            DispatchOutcome::Unrelated
                        };
                        self.last_dispatch = outcome;
                        self.log_single_step(
                            tid,
                            code,
                            first_chance,
                            exception_address,
                            None,
                            None,
                            outcome,
                            "thread_not_registered",
                        )?;
                        return if matches!(outcome, DispatchOutcome::Ambiguous(_)) {
                            Err("single-step at target on an unregistered thread".into())
                        } else {
                            Ok(DBG_EXCEPTION_NOT_HANDLED)
                        };
                    };
                    #[cfg(test)]
                    let context_result = if self.force_context_failure {
                        Err("injected GetThreadContext failure".into())
                    } else {
                        context(&state.handle)
                    };
                    #[cfg(not(test))]
                    let context_result = context(&state.handle);
                    let mut c = match context_result {
                        Ok(c) => c,
                        Err(e) => {
                            let outcome = DispatchOutcome::Ambiguous("context_unavailable");
                            self.last_dispatch = outcome;
                            self.log_single_step(
                                tid,
                                code,
                                first_chance,
                                exception_address,
                                None,
                                None,
                                outcome,
                                "context_unavailable",
                            )?;
                            return Err(format!("GetThreadContext at single-step: {e}"));
                        }
                    };
                    #[cfg(test)]
                    if self.zero_context_dr6_for_test {
                        c.Dr6 = 0;
                    }
                    #[cfg(test)]
                    let mut reported = c;
                    #[cfg(not(test))]
                    let reported = c;
                    #[cfg(test)]
                    if self.force_reported_rip_mismatch {
                        reported.Rip = reported.Rip.wrapping_add(1);
                    }
                    let outcome = classify_single_step(
                        self.mode,
                        &state.returns,
                        &state.armed_sites,
                        exception_address,
                        Some(&reported),
                    );
                    self.last_dispatch = outcome;
                    let trap = match outcome {
                        DispatchOutcome::OwnedProven(trap) => trap,
                        DispatchOutcome::Unrelated => {
                            self.log_single_step(
                                tid,
                                code,
                                first_chance,
                                exception_address,
                                Some(&reported),
                                Some(reported.Dr6),
                                outcome,
                                "unmatched_single_step",
                            )?;
                            self.invalidate_returns_for_exception(tid)?;
                            return Ok(DBG_EXCEPTION_NOT_HANDLED);
                        }
                        DispatchOutcome::Ambiguous(reason) => {
                            self.log_single_step(
                                tid,
                                code,
                                first_chance,
                                exception_address,
                                Some(&reported),
                                Some(reported.Dr6),
                                outcome,
                                reason,
                            )?;
                            return Err(format!("ambiguous single-step: {reason}; refusing normal detach or ABI interpretation"));
                        }
                    };
                    let expected = match trap {
                        OwnedTrap::CanEver => self.module_base + CAN_EVER_RVA,
                        OwnedTrap::CanAppoint => self.module_base + CAN_APPOINT_RVA,
                        OwnedTrap::PlayerGetter => self.module_base + PLAYER_GETTER_RVA,
                        OwnedTrap::PropertyBefore => self.module_base + PROPERTY_BEFORE_RVA,
                        OwnedTrap::PropertyAfter => self.module_base + PROPERTY_AFTER_RVA,
                        OwnedTrap::PropertyName => self.module_base + PROPERTY_NAME_RVA,
                        OwnedTrap::Return => state
                            .returns
                            .has_site(exception_address)
                            .then_some(exception_address)
                            .ok_or("owned return site disappeared")?,
                    };
                    if c.Rip != expected {
                        let outcome =
                            DispatchOutcome::Ambiguous("live_rip_disagrees_after_qualification");
                        self.last_dispatch = outcome;
                        self.log_single_step(
                            tid,
                            code,
                            first_chance,
                            exception_address,
                            Some(&c),
                            Some(c.Dr6),
                            outcome,
                            "owned_address_context_rip_mismatch",
                        )?;
                        return Err(format!("owned hardware breakpoint at 0x{expected:x} has context RIP 0x{:x}; refusing to interpret ABI",c.Rip));
                    }
                    #[cfg(test)]
                    assert!(
                        !self.panic_on_owned_event,
                        "injected panic while a proven hardware exception is pending"
                    );
                    let expected_bit = match trap {
                        OwnedTrap::CanEver => 1,
                        OwnedTrap::CanAppoint => 2,
                        OwnedTrap::Return => 0xc,
                        OwnedTrap::PlayerGetter => 1,
                        OwnedTrap::PropertyBefore => 2,
                        OwnedTrap::PropertyAfter => 4,
                        OwnedTrap::PropertyName => 8,
                    };
                    if c.Dr6 & expected_bit == 0 {
                        self.log_single_step(
                            tid,
                            code,
                            first_chance,
                            exception_address,
                            Some(&c),
                            Some(c.Dr6),
                            outcome,
                            "owned_address_without_dr6_status",
                        )?;
                    }
                    match trap {
                        OwnedTrap::CanEver => {
                            self.entry(tid, &mut c, EntryKind::CanEver, CAN_EVER_RVA)?
                        }
                        OwnedTrap::CanAppoint => {
                            self.entry(tid, &mut c, EntryKind::CanAppoint, CAN_APPOINT_RVA)?
                        }
                        OwnedTrap::Return => self.returned(tid, &mut c)?,
                        OwnedTrap::PlayerGetter
                        | OwnedTrap::PropertyBefore
                        | OwnedTrap::PropertyAfter
                        | OwnedTrap::PropertyName => self.property_hit(tid, &mut c)?,
                    }
                } else {
                    self.invalidate_returns_for_exception(tid)?;
                    return Ok(DBG_EXCEPTION_NOT_HANDLED);
                }
            }
            _ => {}
        }
        Ok(DBG_CONTINUE)
    }
    fn status_after_error(&self, event: &DEBUG_EVENT) -> i32 {
        if event.dwDebugEventCode != EXCEPTION_DEBUG_EVENT {
            return DBG_CONTINUE;
        }
        let info = unsafe { event.u.Exception };
        if info.ExceptionRecord.ExceptionCode == EXCEPTION_BREAKPOINT {
            return if self.last_attach_breakpoint_owned {
                DBG_CONTINUE
            } else {
                DBG_EXCEPTION_NOT_HANDLED
            };
        }
        if info.ExceptionRecord.ExceptionCode == EXCEPTION_SINGLE_STEP {
            return if matches!(self.last_dispatch, DispatchOutcome::OwnedProven(_)) {
                DBG_CONTINUE
            } else {
                DBG_EXCEPTION_NOT_HANDLED
            };
        }
        DBG_EXCEPTION_NOT_HANDLED
    }
    fn cleanup(&mut self, pending_event: Option<(&DEBUG_EVENT, i32)>) -> Result<(), String> {
        if self.critical {
            return Err("critical_cleanup: the owned game still requires closure".into());
        }
        if !self.attached && self.held.is_empty() {
            return Ok(());
        }
        if let Some((event, status)) = pending_event {
            self.cleanup_event = Some((*event, status));
        } else if let Some((event, _)) = self.cleanup_event {
            // Also covers unwinding while handle_event owns a stopped target.
            self.cleanup_event = Some((event, self.status_after_error(&event)));
        }
        let mut gate = self.cleanup_gate.take().unwrap_or_else(CleanupGate::new);
        if self
            .cleanup_event
            .is_some_and(|(event, _)| event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT)
        {
            // All target threads have exited. Acknowledge this real terminal
            // event; do not run restoration or label its exit as a detach.
            gate.phase = CleanupPhase::Continue;
        }
        let result = gate.drive(self);
        self.cleanup_gate = Some(gate);
        if self.terminal == TerminalKind::TargetExited {
            return Err("target_exited_during_cleanup; exit debug event acknowledged".into());
        }
        match result {
            Ok(()) => Ok(()),
            Err(reason) => {
                if matches!(alive(&self.game), Ok(false)) && self.cleanup_event.is_none() {
                    let exit = queried_exit_code(&self.game);
                    self.record_critical_exit(exit);
                    self.attached = false;
                    self.terminal = TerminalKind::TargetExited;
                    self.held.clear();
                    Err(format!("{reason}; target process already exited"))
                } else {
                    self.critical_wait_for_game_close(reason)
                }
            }
        }
    }
    fn critical_wait_for_game_close(&mut self, reason: String) -> Result<(), String> {
        self.critical = true;
        #[cfg(test)]
        if let Some(signal) = &self.critical_signal {
            signal.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        let _ = self.output.row(json!({"type":"critical_cleanup","game_pid":self.pid,"reason":reason,"game_close_required":true}));
        match announce_critical_and_wait_for_owned_game(
            &self.game,
            self.pid,
            &reason,
            self.cleanup_event.take(),
            self.attached,
            Some(&self.close_ack),
            Some(&mut self.attach_origin),
            Some(self.query_thread_start),
            false,
        ) {
            Ok(exit) => {
                self.record_critical_exit(exit);
                self.attached = false;
                self.terminal = TerminalKind::TargetExited;
                self.held.clear();
                Err(format!("{reason}; the controller closed the owned game"))
            }
            Err(critical) => Err(critical),
        }
    }
    fn critical_ambiguous_exception(
        &mut self,
        event: &DEBUG_EVENT,
        reason: &str,
    ) -> Result<(), String> {
        self.critical = true;
        self.cleanup_event = Some((*event, DBG_EXCEPTION_NOT_HANDLED));
        let message = format!("ambiguous debug exception: {reason}; normal detach is forbidden");
        let _ = self.output.row(json!({"type":"critical_cleanup","game_pid":self.pid,"reason":message,"game_close_required":true,"ambiguous_exception":true}));
        #[cfg(test)]
        if let Some(signal) = &self.critical_signal {
            signal.store(true, Ordering::SeqCst);
        }
        println!(
            "{}",
            json!({"critical_cleanup":true,"pid":self.pid,"reason":message,"game_close_required":true})
        );
        let _ = std::io::stdout().flush();
        // Debug registers are restored when possible while the exception stays
        // pending. A failed restoration never transitions to normal detach.
        let restore_deadline = Instant::now() + Duration::from_millis(500);
        while Instant::now() < restore_deadline && !matches!(alive(&self.game), Ok(false)) {
            if self.freeze().and_then(|_| self.restore_verified()).is_ok() {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        match announce_critical_and_wait_for_owned_game(
            &self.game,
            self.pid,
            &message,
            self.cleanup_event.take(),
            self.attached,
            Some(&self.close_ack),
            Some(&mut self.attach_origin),
            Some(self.query_thread_start),
            true,
        ) {
            Ok(exit) => {
                self.record_critical_exit(exit);
                self.attached = false;
                self.terminal = TerminalKind::TargetExited;
                self.held.clear();
                Err(format!("{message}; controller closed the owned game"))
            }
            Err(e) => Err(e),
        }
    }
    fn run(
        &mut self,
        stop: Receiver<()>,
        budget: Duration,
        mut guard: impl FnMut() -> Result<(), String>,
    ) -> Result<(), String> {
        let started = Instant::now();
        self.attach_ready_deadline = Some(started + ATTACH_READY_BUDGET);
        loop {
            if self.attach_deadline_expired() {
                let cleanup = self.cleanup(None);
                return Err(format!(
                    "debugger attach handshake timed out before ready; detach: {cleanup:?}"
                ));
            }
            if stop.try_recv().is_ok() || started.elapsed() >= budget {
                self.cleanup(None)?;
                return Ok(());
            }
            if let Err(e) = guard() {
                let cleanup = self.cleanup(None);
                return Err(format!("{e}; detach: {cleanup:?}"));
            }
            let mut event = DEBUG_EVENT::default();
            if unsafe { WaitForDebugEventEx(&mut event, 100) } == 0 {
                if std::io::Error::last_os_error().raw_os_error() == Some(121) {
                    continue;
                }
                let error = win_error("WaitForDebugEventEx");
                let cleanup = self.cleanup(None);
                return Err(format!("{error}; detach: {cleanup:?}"));
            }
            self.cleanup_event = Some((event, DBG_EXCEPTION_NOT_HANDLED));
            let status = match self.handle_event(&event) {
                Ok(status) => status,
                Err(e) => {
                    if matches!(self.last_dispatch, DispatchOutcome::Ambiguous(_)) {
                        let critical = self.critical_ambiguous_exception(&event, &e);
                        return Err(format!("{e}; critical cleanup: {critical:?}"));
                    }
                    let status = self.status_after_error(&event);
                    let cleanup = self.cleanup(Some((&event, status)));
                    return Err(format!("{e}; detach: {cleanup:?}"));
                }
            };
            if let Err(error) = self.continue_pending_with_status(status) {
                let cleanup = self.cleanup(None);
                return Err(format!("{error}; detach: {cleanup:?}"));
            }
            if event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
                self.attached = false;
                self.terminal = TerminalKind::TargetExited;
                self.held.clear();
                return Err("target process exited during observation".into());
            }
            if self.ready {
                if self.attach_deadline_expired() {
                    let cleanup = self.cleanup(None);
                    return Err(format!("debugger attach handshake expired before ready notification; detach: {cleanup:?}"));
                }
                let mut row = json!({"type":"ready","thread_count":if self.mode == ProbeMode::Lifecycle {self.lifecycle.thread_ids.len()} else {self.threads.len()}});
                self.stamp(&mut row);
                self.output.row(row)?;
                println!("{}", json!({"ready":true,"pid":self.pid}));
                std::io::stdout().flush().map_err(|e| e.to_string())?;
                self.ready = false;
                // The attach breakpoint is the only point at which ready is sent.
                break;
            }
        }
        loop {
            if stop.try_recv().is_ok() || started.elapsed() >= budget {
                self.cleanup(None)?;
                return Ok(());
            }
            if let Err(e) = guard() {
                let cleanup = self.cleanup(None);
                return Err(format!("{e}; detach: {cleanup:?}"));
            }
            let mut event = DEBUG_EVENT::default();
            if unsafe { WaitForDebugEventEx(&mut event, 100) } == 0 {
                if std::io::Error::last_os_error().raw_os_error() == Some(121) {
                    continue;
                }
                let error = win_error("WaitForDebugEventEx");
                let cleanup = self.cleanup(None);
                return Err(format!("{error}; detach: {cleanup:?}"));
            }
            self.cleanup_event = Some((event, DBG_EXCEPTION_NOT_HANDLED));
            let status = match self.handle_event(&event) {
                Ok(v) => v,
                Err(e) => {
                    if matches!(self.last_dispatch, DispatchOutcome::Ambiguous(_)) {
                        let critical = self.critical_ambiguous_exception(&event, &e);
                        return Err(format!("{e}; critical cleanup: {critical:?}"));
                    }
                    let status = self.status_after_error(&event);
                    let cleanup = self.cleanup(Some((&event, status)));
                    return Err(format!("{e}; detach: {cleanup:?}"));
                }
            };
            if let Err(error) = self.continue_pending_with_status(status) {
                let cleanup = self.cleanup(None);
                return Err(format!("{error}; detach: {cleanup:?}"));
            }
            if event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
                self.attached = false;
                self.terminal = TerminalKind::TargetExited;
                self.held.clear();
                return Err("target process exited during observation".into());
            }
        }
    }

    fn continue_pending_with_status(&mut self, status: i32) -> Result<(), String> {
        if let Some((event, _)) = self.cleanup_event {
            self.cleanup_event = Some((event, status));
        }
        self.continue_pending()
    }
}
impl CleanupOps for Debugger {
    fn game_exited(&mut self) -> bool {
        matches!(alive(&self.game), Ok(false))
    }
    fn freeze(&mut self) -> Result<(), String> {
        #[cfg(test)]
        if self.freeze_refused {
            return Err("injected SuspendThread refusal before verified restoration".into());
        }
        if self.mode == ProbeMode::Lifecycle {
            return Ok(());
        }
        // This debugger thread is the sole writer of the tracked set. While
        // cleaning up it installs no new breakpoints, so only these pinned
        // handles need restoration. Enumerating *all system threads* twice
        // consumed the whole six-second budget under parallel Windows tests.
        for (&tid, state) in &self.threads {
            if self.held.contains(&tid) {
                continue; // Do not increment a suspend count again on retry.
            }
            let handle = &state.handle;
            match unsafe { WaitForSingleObject(handle.0, 0) } {
                WAIT_OBJECT_0 => continue,
                WAIT_TIMEOUT => {}
                _ => {
                    return Err(format!(
                        "cannot confirm thread {tid} liveness before suspension: {}",
                        win_error("WaitForSingleObject")
                    ))
                }
            }
            if unsafe { SuspendThread(handle.0) } == u32::MAX {
                let suspend_error = win_error("SuspendThread");
                let thread_wait = unsafe { WaitForSingleObject(handle.0, 0) };
                let game_wait = unsafe { WaitForSingleObject(self.game.0, 0) };
                match suspend_failure_state(thread_wait, game_wait) {
                    SuspendFailureState::Gone => continue,
                    SuspendFailureState::TargetExited => return Err(format!("target exited while suspending thread {tid}: {suspend_error}")),
                    SuspendFailureState::Unsignaled => return Err(format!("thread {tid} was unsignaled when suspension failed: {suspend_error}")),
                    SuspendFailureState::Unknown => return Err(format!("thread {tid} liveness was unconfirmed after suspension failure (thread wait {thread_wait}, game wait {game_wait}): {suspend_error}")),
                }
            } else {
                self.held.insert(tid);
            }
        }
        Ok(())
    }
    fn restore_verified(&mut self) -> Result<(), String> {
        if self.mode == ProbeMode::Lifecycle {
            return Ok(());
        }
        #[cfg(test)]
        if self.restore_faults > 0 {
            self.restore_faults -= 1;
            return Err("injected SetThreadContext refusal".into());
        }
        // The saved HANDLE identifies every thread whose registers we wrote,
        // even if its TID has since been recycled.
        for (tid, state) in &self.threads {
            match unsafe { WaitForSingleObject(state.handle.0, 0) } {
                WAIT_OBJECT_0 => continue,
                WAIT_TIMEOUT => {}
                _ => {
                    return Err(format!(
                        "cannot check thread {tid} liveness: {}",
                        win_error("WaitForSingleObject")
                    ))
                }
            }
            if !self.held.contains(tid) {
                return Err(format!(
                    "live thread {tid} was not suspended for restoration"
                ));
            }
            let current = context(&state.handle)?;
            set_debug_registers(&state.handle, &current, state.original)?;
            let after = context(&state.handle)?;
            if Registers::from(&after) != state.original {
                return Err(format!("thread {tid} debug registers did not match the saved values after SetThreadContext"));
            }
        }
        Ok(())
    }
    fn continue_pending(&mut self) -> Result<(), String> {
        if let Some((event, status)) = self.cleanup_event {
            #[cfg(test)]
            if self.exit_continue_fault && event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
                self.exit_continue_fault = false;
                return Err("injected exit ContinueDebugEvent refusal".into());
            }
            #[cfg(test)]
            if self.continue_faults > 0 {
                self.continue_faults -= 1;
                return Err("injected ContinueDebugEvent refusal".into());
            }
            if unsafe { ContinueDebugEvent(event.dwProcessId, event.dwThreadId, status) } == 0 {
                return Err(win_error("ContinueDebugEvent before detach"));
            }
            self.cleanup_event = None;
            if let Some(exit) = critical_exit_from_debug_event(&event) {
                self.attached = false;
                self.terminal = TerminalKind::TargetExited;
                self.held.clear();
                self.record_critical_exit(exit);
            }
        }
        Ok(())
    }
    fn detach(&mut self) -> Result<(), String> {
        if !self.attached {
            return Ok(());
        }
        if unsafe { DebugActiveProcessStop(self.pid) } == 0 {
            return Err(win_error("DebugActiveProcessStop"));
        }
        self.attached = false;
        self.terminal = TerminalKind::Detached;
        Ok(())
    }
    fn resume(&mut self) -> Result<(), String> {
        if self.mode == ProbeMode::Lifecycle {
            return Ok(());
        }
        let tids: Vec<u32> = self.held.iter().copied().collect();
        for tid in tids {
            let handle = &self.threads[&tid].handle;
            if unsafe { ResumeThread(handle.0) } == u32::MAX
                && unsafe { WaitForSingleObject(handle.0, 0) } != WAIT_OBJECT_0
            {
                return Err(format!(
                    "live thread {tid} remains suspended: {}",
                    win_error("ResumeThread")
                ));
            }
            self.held.remove(&tid);
        }
        Ok(())
    }
}
impl Drop for Debugger {
    fn drop(&mut self) {
        if !self.critical && (self.attached || !self.held.is_empty()) {
            if let (Some((event, _)), DispatchOutcome::Ambiguous(reason)) =
                (self.cleanup_event, self.last_dispatch)
            {
                let _ = self.critical_ambiguous_exception(&event, reason);
            } else {
                let _ = self.cleanup(None);
            }
        }
    }
}

pub fn run_observer(config: ObserverConfig) -> Result<(), String> {
    let mode = ProbeMode::parse(&config.mode)?;
    if mode != ProbeMode::Lifecycle {
        return Err("hardware breakpoints are withheld until per-thread exception delivery is proven; use lifecycle mode".into());
    }
    let verified = verify(config, mode)?;
    let output = Jsonl::new(&verified.config.output)?;
    let mut debugger = Debugger::attach(
        verified.config.pid,
        verified.game,
        verified.module.base,
        verified.module.size,
        output,
        mode,
    )?;
    let mut identity = json!({"type":"identity","schema_version":crate::NATIVE_TRACE_SCHEMA_VERSION,
        "game_pid":verified.config.pid,
        "game_created_filetime":verified.config.created,"game_exe":verified.config.exe,
        "module_path":verified.module.path,"module_base":format!("0x{:016x}",verified.module.base),
        "module_sha256":EXPECTED_SHA256,"abi":"not_observed","mode":mode.label(),
        "attach_breakpoint_address":format!("0x{:016x}", debugger.attach_origin.breakpoint_address),
        "attach_breakin_start":format!("0x{:016x}", debugger.attach_origin.breakin_start),
        "attach_origin_source":"local_and_target_ntdll_module_metadata_and_exports",
        "attach_ntdll_disk_path":debugger.ntdll_evidence.path,
        "attach_ntdll_disk_sha256":debugger.ntdll_evidence.disk_sha256,
        "attach_ntdll_disk_length":debugger.ntdll_evidence.disk_len,
        "attach_ntdll_pe_timestamp":debugger.ntdll_evidence.pe_timestamp,
        "attach_ntdll_image_size":debugger.ntdll_evidence.image_size,
        "attach_ntdll_local_base":format!("0x{:016x}", debugger.ntdll_evidence.local_base),
        "attach_ntdll_remote_base":format!("0x{:016x}", debugger.ntdll_evidence.remote_base),
        "attach_ntdll_export_rvas":{
            "DbgBreakPoint":debugger.ntdll_evidence.breakpoint_rva,
            "DbgUiRemoteBreakin":debugger.ntdll_evidence.breakin_rva,
            "NtQueryInformationThread":debugger.ntdll_evidence.query_rva},
        "attach_ntdll_remote_bytes_read":false});
    debugger.stamp(&mut identity);
    debugger.output.row(identity)?;
    let (tx, rx) = mpsc::channel();
    let close_ack = Arc::clone(&debugger.close_ack);
    thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            match line.as_deref() {
                Ok("stop") => {
                    let _ = tx.send(());
                }
                Ok("game_close_requested") => close_ack.store(true, Ordering::SeqCst),
                _ => {}
            }
        }
        let _ = tx.send(());
    });
    let game_guard = exact_process(
        verified.config.pid,
        verified.config.created,
        &verified.config.exe,
        false,
    )?;
    let guard = || {
        verify_lock(
            &verified.config.lock,
            verified.config.chain_pid,
            &verified.chain,
        )?;
        if !alive(&verified.controller)?
            || process_created(&verified.controller)? != verified.config.controller_created
        {
            return Err("observer controller exited or changed".into());
        }
        if !alive(&game_guard)? {
            return Err("game process exited".into());
        }
        Ok(())
    };
    let result = debugger.run(rx, Duration::from_secs(verified.config.seconds), guard);
    let mut summary = json!({"type":"lifecycle_summary","mode":"lifecycle",
        "thread_events":debugger.lifecycle.thread_events,
        "unique_thread_ids_seen":debugger.lifecycle.thread_ids.len(),
        "thread_status_rows_truncated":debugger.lifecycle.thread_events > LIFECYCLE_THREAD_ROW_LIMIT,
        "context_reads_at_debug_events":debugger.lifecycle.context_reads,
        "unknown_thread_status":debugger.lifecycle.unknown_thread_status,
        "exception_events":debugger.lifecycle.exception_events,
        "application_exception_events":debugger.lifecycle.application_exception_events,
        "process_exit_code":debugger.lifecycle.exit_code,
        "process_exit_code_source":debugger.lifecycle.exit_code_source,
        "hardware_breakpoints_installed":0,"debug_register_writes":0,
        "future_exception_delivery":"unknown"});
    debugger.stamp(&mut summary);
    debugger.output.row(summary)?;
    let mut stop_row = json!({"type":"stop","schema_version":crate::NATIVE_TRACE_SCHEMA_VERSION,
        "terminal_kind":debugger.terminal.label(),
        "detached":debugger.terminal == TerminalKind::Detached,
        "target_exited":debugger.terminal == TerminalKind::TargetExited,
        "process_exit_code":debugger.lifecycle.exit_code,
        "process_exit_code_source":debugger.lifecycle.exit_code_source,
        "result":result.as_ref().map(|_| "ok").unwrap_or("error")});
    debugger.stamp(&mut stop_row);
    debugger.output.row(stop_row)?;
    result
}

#[cfg(test)]
#[path = "windows/test_support.rs"]
mod test_support;

#[cfg(test)]
mod tests {
    use super::test_support::{
        enter_worker, isolated_test, join_observer, verify_watchdog_failure, Fixture,
    };
    use super::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::{
        AddVectoredExceptionHandler, RaiseException, RemoveVectoredExceptionHandler,
        EXCEPTION_CONTINUE_EXECUTION, EXCEPTION_CONTINUE_SEARCH, EXCEPTION_DEBUG_INFO,
        EXCEPTION_POINTERS, EXCEPTION_RECORD, EXIT_PROCESS_DEBUG_INFO,
    };

    static UNKNOWN_STEP_HANDLED: AtomicBool = AtomicBool::new(false);

    #[test]
    fn pinned_instruction_starts_and_context_layout_are_checked_before_attach() {
        assert_eq!(std::mem::align_of::<CONTEXT>(), 8);
        assert_eq!(std::mem::align_of::<AlignedContext>(), 16);
        let context = AlignedContext(unsafe { zeroed() });
        assert_eq!((&context.0 as *const CONTEXT as usize) % 16, 0);
        assert_eq!(PROPERTY_NAME_RVA, 0x15339); // Actual82 failure RVA is our Dr3 site.
        assert_eq!(PROPERTY_BEFORE_RVA + 3, PROPERTY_AFTER_RVA);
        for (rva, _, instruction) in PINNED_BREAKPOINT_SITES {
            assert!(is_pinned_instruction_start(rva, instruction));
            assert!(!is_pinned_instruction_start(rva + 1, instruction));
            assert!(!is_pinned_instruction_start(rva, &instruction[1..]));
        }
        let sites = ProbeMode::Property.fixed_sites(0x180000000);
        let mut c: CONTEXT = unsafe { zeroed() };
        [c.Dr0, c.Dr1, c.Dr2, c.Dr3] = sites.map(|site| site.unwrap());
        c.Dr7 = 0x55;
        assert!(installed_sites_match_context(&sites, &c));
        c.Dr3 += 1;
        assert!(!installed_sites_match_context(&sites, &c));
        c.Dr3 -= 1;
        c.Dr7 = 0x15;
        assert!(!installed_sites_match_context(&sites, &c));
    }

    #[test]
    fn wrong_same_named_dx11_image_is_rejected_before_process_or_game_access() {
        let wrong = Path::new(GAME_ROOT).join("Other/CivilizationVI.exe");
        let result = verify(
            ObserverConfig {
                pid: 1001,
                created: 1,
                exe: wrong,
                chain_pid: 1002,
                lock: PathBuf::from("D:/CC/Tools/civ6-live/civ6.lock"),
                controller_pid: 1003,
                controller_created: 1,
                output: PathBuf::from("never-written.jsonl"),
                seconds: 1,
                mode: "governor".into(),
            },
            ProbeMode::Governor,
        );
        assert_eq!(
            result.err().unwrap(),
            "target is not the exact Steam DX11 Civ6 executable"
        );
    }
    #[test]
    fn live_hardware_modes_are_rejected_before_attach_or_process_access() {
        for mode in ["governor", "property"] {
            let error = run_observer(ObserverConfig {
                pid: 1001,
                created: 1,
                exe: PathBuf::from("never-opened.exe"),
                chain_pid: 1002,
                lock: PathBuf::from("never-opened.lock"),
                controller_pid: 1003,
                controller_created: 1,
                output: PathBuf::from("never-written.jsonl"),
                seconds: 1,
                mode: mode.into(),
            })
            .unwrap_err();
            assert!(error.contains("withheld"));
        }
    }
    unsafe extern "system" fn unknown_step_handler(info: *mut EXCEPTION_POINTERS) -> i32 {
        if !info.is_null() {
            let code = unsafe { (*(*info).ExceptionRecord).ExceptionCode };
            if code == EXCEPTION_SINGLE_STEP {
                UNKNOWN_STEP_HANDLED.store(true, Ordering::SeqCst);
                return EXCEPTION_CONTINUE_EXECUTION;
            }
        }
        EXCEPTION_CONTINUE_SEARCH
    }

    #[test]
    fn synthetic_owner_fields_use_read_process_memory_and_current_lifetime() {
        let game = process(std::process::id(), true).unwrap();
        let mut governors = Box::new([0u8; 0x78]);
        let mut player_a = Box::new([0u8; 0x530]);
        let mut player_b = Box::new([0u8; 0x530]);
        player_a[0xd8..0xdc].copy_from_slice(&2u32.to_le_bytes());
        player_b[0xd8..0xdc].copy_from_slice(&7u32.to_le_bytes());
        governors[0x70..0x78].copy_from_slice(&(player_a.as_ptr() as u64).to_le_bytes());
        let address = governors.as_ptr() as u64;
        let first = owner_fields(&RemoteMemory(&game), address).unwrap();
        assert_eq!(first.player_id, 2);
        assert_eq!(
            first.property_interface_address,
            player_a.as_ptr() as u64 + 0x528
        );
        governors[0x70..0x78].copy_from_slice(&(player_b.as_ptr() as u64).to_le_bytes());
        let second = owner_fields(&RemoteMemory(&game), address).unwrap();
        assert_eq!(second.player_id, 7);
        assert_eq!(
            second.property_interface_address,
            player_b.as_ptr() as u64 + 0x528
        );
        governors[0x70..0x78].fill(0);
        assert_eq!(owner_fields(&RemoteMemory(&game), address), None);
    }

    #[inline(never)]
    extern "C-unwind" fn fixture_target(this: u64, governor: u32) -> u8 {
        let this = std::hint::black_box(this);
        let governor = std::hint::black_box(governor);
        if governor == 99 {
            panic!("fixture exits this observed call without returning");
        }
        if this == 0x1234 && governor == 7 {
            1
        } else {
            0
        }
    }

    fn child_registers(tid: u32, sentinel: bool) -> String {
        thread::spawn(move || {
            let handle = thread_handle(tid).unwrap();
            assert_ne!(unsafe { SuspendThread(handle.0) }, u32::MAX);
            let mut c = context(&handle).unwrap();
            if sentinel {
                c.Dr2 = fixture_target as *const () as u64;
                let set = set_context(&handle, &c);
                assert_ne!(unsafe { ResumeThread(handle.0) }, u32::MAX);
                set.unwrap();
                assert_ne!(unsafe { SuspendThread(handle.0) }, u32::MAX);
                c = context(&handle).unwrap();
            }
            assert_ne!(unsafe { ResumeThread(handle.0) }, u32::MAX);
            let r = Registers::from(&c);
            format!(
                "{:x}:{:x}:{:x}:{:x}:{:x}:{:x}",
                r.dr[0], r.dr[1], r.dr[2], r.dr[3], r.dr6, r.dr7
            )
        })
        .join()
        .unwrap()
    }

    #[test]
    fn fixture_child_worker() {
        if std::env::var_os("PR86_PROBE_TEST_CHILD").is_none() {
            return;
        }
        let mut governors = Box::new([0u8; 0x78]);
        let mut player_a = Box::new([0u8; 0x530]);
        let mut player_b = Box::new([0u8; 0x530]);
        player_a[0xd8..0xdc].copy_from_slice(&2u32.to_le_bytes());
        player_b[0xd8..0xdc].copy_from_slice(&7u32.to_le_bytes());
        governors[0x70..0x78].copy_from_slice(&(player_a.as_ptr() as u64).to_le_bytes());
        let main_tid = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
        let baseline = child_registers(main_tid, true);
        println!(
            "PROBE_FIXTURE_READY:{}:{}",
            fixture_target as *const () as usize, baseline
        );
        std::io::stdout().flush().unwrap();
        let input = std::io::stdin();
        for line in input.lock().lines() {
            match line.unwrap().as_str() {
                "T" => println!("PROBE_OUT:{}", fixture_target(0x1234, 7)),
                "F" => println!("PROBE_OUT:{}", fixture_target(0x1234, 8)),
                "M" => {
                    let a = thread::spawn(|| fixture_target(0x1234, 7));
                    let b = thread::spawn(|| fixture_target(0x1234, 8));
                    println!("PROBE_OUT:M{}{}", a.join().unwrap(), b.join().unwrap());
                }
                "W" => {
                    let mut result = 0;
                    for governor in [99, 7] {
                        if let Ok(value) =
                            std::panic::catch_unwind(|| fixture_target(0x1234, governor))
                        {
                            result = value;
                        }
                    }
                    println!("PROBE_OUT:W{result}");
                }
                "U" => {
                    UNKNOWN_STEP_HANDLED.store(false, Ordering::SeqCst);
                    let handler =
                        unsafe { AddVectoredExceptionHandler(1, Some(unknown_step_handler)) };
                    assert!(!handler.is_null());
                    unsafe {
                        RaiseException(EXCEPTION_SINGLE_STEP as u32, 0, 0, std::ptr::null());
                    }
                    assert_ne!(unsafe { RemoveVectoredExceptionHandler(handler) }, 0);
                    println!("PROBE_OUT:U{}", UNKNOWN_STEP_HANDLED.load(Ordering::SeqCst));
                }
                "R" => println!("PROBE_REG:{}", child_registers(main_tid, false)),
                "O" => println!("PROBE_OWNER:{:x}", governors.as_ptr() as u64),
                "P" => {
                    governors[0x70..0x78]
                        .copy_from_slice(&(player_b.as_ptr() as u64).to_le_bytes());
                    println!("PROBE_OWNER:{:x}", governors.as_ptr() as u64);
                }
                "N" => {
                    governors[0x70..0x78].fill(0);
                    println!("PROBE_OWNER:{:x}", governors.as_ptr() as u64);
                }
                "H" => loop {
                    thread::park();
                },
                "X" => break,

                _ => panic!("invalid fixture command"),
            }
            std::io::stdout().flush().unwrap();
        }
    }

    #[test]
    fn fixture_unresponsive_pipe_has_a_deadline_and_releases_its_owned_child() {
        if isolated_test("fixture_unresponsive_pipe_has_a_deadline_and_releases_its_owned_child") {
            return;
        }
        let child = Fixture::start();
        assert_ne!(child.child.id(), 0);
        let game = process(child.child.id(), false).unwrap();
        assert!(alive(&game).unwrap());
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let mut child = child;
            let result = child.command_with_budget("H", "NEVER:", Duration::from_millis(100));
            panic!("{}", result.unwrap_err());
        }));
        let failure = failed.expect_err("the unresponsive child must reach the read deadline");
        let error = failure.downcast_ref::<String>().unwrap();
        assert!(error.contains("did not answer NEVER:") && error.contains("100ms"));
        assert!(!alive(&game).unwrap());
    }

    #[test]
    fn panic_before_critical_close_ack_collects_observer_child_and_fixture_lane() {
        if isolated_test("panic_before_critical_close_ack_collects_observer_child_and_fixture_lane")
        {
            return;
        }
        let child = Fixture::start();
        let child_pid = child.child.id();
        assert_ne!(child_pid, 0);
        let game = process(child_pid, false).unwrap();
        assert!(alive(&game).unwrap());
        let path =
            std::env::temp_dir().join(format!("pr86-fixture-panic-{}.jsonl", std::process::id()));
        let _ = fs::remove_file(&path);
        let output = Jsonl::new(&path).unwrap();
        let read_path = path.clone();
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let mut child = child;
            let base = child.entry - CAN_EVER_RVA;
            let signal = Arc::new(AtomicBool::new(false));
            let observed = Arc::clone(&signal);
            let (stop_tx, stop_rx) = mpsc::channel();
            let _observer = child.observe(&stop_tx, move |close_ack| {
                let game = process(child_pid, true).unwrap();
                let mut debugger =
                    Debugger::attach(child_pid, game, base, 0x800000, output, ProbeMode::Governor)
                        .unwrap();
                debugger.close_ack = close_ack;
                debugger.critical_signal = Some(observed);
                debugger.force_context_failure = true;
                debugger.run(stop_rx, Duration::from_secs(10), || Ok(()))
            });
            let start = Instant::now();
            while !fs::read_to_string(&read_path)
                .is_ok_and(|text| text.contains("\"type\":\"ready\""))
            {
                assert!(start.elapsed() < Duration::from_secs(5));
                thread::sleep(Duration::from_millis(10));
            }
            child.send("T").unwrap();
            while !signal.load(Ordering::SeqCst) {
                assert!(start.elapsed() < Duration::from_secs(5));
                thread::sleep(Duration::from_millis(10));
            }
            panic!("injected panic before controller close_ack");
        }));
        let failure = failed.expect_err("the fixture must reach the injected close-ack panic");
        assert_eq!(
            failure.downcast_ref::<&str>().copied(),
            Some("injected panic before controller close_ack")
        );
        assert!(!alive(&game).unwrap());
        let rows = fs::read_to_string(&path).unwrap();
        assert!(rows.contains("ambiguous debug exception") || rows.contains("critical_cleanup"));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn synthetic_child_owner_data_and_lifetime_are_read_without_stale_pointers() {
        if isolated_test("synthetic_child_owner_data_and_lifetime_are_read_without_stale_pointers")
        {
            return;
        }
        let mut child = Fixture::start();
        let game = process(child.child.id(), true).unwrap();
        let address = u64::from_str_radix(&child.command("O", "PROBE_OWNER:"), 16).unwrap();
        let first = owner_fields(&RemoteMemory(&game), address).unwrap();
        assert_eq!(first.player_id, 2);
        assert_eq!(
            first.property_interface_address,
            first.player_address + 0x528
        );
        assert_eq!(
            address,
            u64::from_str_radix(&child.command("P", "PROBE_OWNER:"), 16).unwrap()
        );
        let second = owner_fields(&RemoteMemory(&game), address).unwrap();
        assert_eq!(second.player_id, 7);
        assert_ne!(first.player_address, second.player_address);
        child.command("N", "PROBE_OWNER:");
        assert_eq!(owner_fields(&RemoteMemory(&game), address), None);
    }

    #[test]
    fn silent_fixture_times_out_and_is_reaped_without_an_unbounded_read() {
        if isolated_test("silent_fixture_times_out_and_is_reaped_without_an_unbounded_read") {
            return;
        }
        let mut child = Fixture::start();
        let game = process(child.child.id(), false).unwrap();
        let started = Instant::now();
        let response = child.command_with_budget("H", "PROBE_OUT:", Duration::from_millis(100));
        assert!(response.unwrap_err().contains("did not answer"));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(alive(&game).unwrap());
        drop(child);
        assert!(!alive(&game).unwrap(), "silent fixture survived cleanup");
    }

    #[test]
    fn synchronous_observer_panic_restores_before_the_join_reports_failure() {
        if isolated_test("synchronous_observer_panic_restores_before_the_join_reports_failure") {
            return;
        }
        panic_child_case(false);
    }

    #[test]
    fn panic_with_a_pending_hardware_exception_restores_then_continues_that_event() {
        if isolated_test(
            "panic_with_a_pending_hardware_exception_restores_then_continues_that_event",
        ) {
            return;
        }
        panic_child_case(true);
    }

    fn panic_child_case(pending: bool) {
        let mut child = Fixture::start();
        let pid = child.child.id();
        let path =
            std::env::temp_dir().join(format!("pr86-panic-{}-{pid}.jsonl", std::process::id()));
        let output = Jsonl::new(&path).unwrap();
        let base = child.entry - CAN_EVER_RVA;
        let panic_now = Arc::new(AtomicBool::new(false));
        let observer_panic = Arc::clone(&panic_now);
        let (_stop_tx, stop_rx) = mpsc::channel();
        let observer = thread::spawn(move || {
            let game = process(pid, true).unwrap();
            let mut debugger =
                Debugger::attach(pid, game, base, 0x500000, output, ProbeMode::Governor).unwrap();
            debugger.panic_on_owned_event = pending;
            debugger.run(stop_rx, Duration::from_secs(10), || {
                assert!(
                    !observer_panic.load(Ordering::SeqCst),
                    "injected synchronous observer panic"
                );
                Ok(())
            })
        });
        let started = Instant::now();
        while !fs::read_to_string(&path).is_ok_and(|s| s.contains("\"type\":\"ready\"")) {
            assert!(started.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(20));
        }
        if pending {
            assert_eq!(child.command("T", "PROBE_OUT:"), "1");
        } else {
            panic_now.store(true, Ordering::SeqCst);
        }
        assert!(join_observer(observer).is_err());
        assert_eq!(child.command("R", "PROBE_REG:"), child.original_regs);
        assert_eq!(child.command("T", "PROBE_OUT:"), "1");
        drop(child);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn case_watchdog_recovers_stalled_and_panicking_trees_without_masking_failure() {
        if isolated_test(
            "case_watchdog_recovers_stalled_and_panicking_trees_without_masking_failure",
        ) {
            return;
        }
        verify_watchdog_failure();
    }

    #[test]
    fn unassigned_worker_reap_requires_native_exit_signal() {
        if isolated_test("unassigned_worker_reap_requires_native_exit_signal") {
            return;
        }
        test_support::verify_unassigned_worker_reap();
    }

    #[test]
    fn failed_event_continuation_retries_the_pending_event_before_verified_detach() {
        if isolated_test(
            "failed_event_continuation_retries_the_pending_event_before_verified_detach",
        ) {
            return;
        }
        let mut child = Fixture::start();
        let pid = child.child.id();
        let path =
            std::env::temp_dir().join(format!("pr86-continue-{}-{pid}.jsonl", std::process::id()));
        let output = Jsonl::new(&path).unwrap();
        let base = child.entry - CAN_EVER_RVA;
        let (_stop_tx, stop_rx) = mpsc::channel();
        let observer = thread::spawn(move || {
            let game = process(pid, true).unwrap();
            let mut debugger =
                Debugger::attach(pid, game, base, 0x500000, output, ProbeMode::Governor).unwrap();
            debugger.continue_faults = 1;
            let result = debugger.run(stop_rx, Duration::from_secs(10), || Ok(()));
            (result, debugger.terminal, debugger.cleanup_event.is_none())
        });
        let (result, terminal, acknowledged) = join_observer(observer).unwrap();
        assert!(result
            .unwrap_err()
            .contains("injected ContinueDebugEvent refusal"));
        assert_eq!(terminal, TerminalKind::Detached);
        assert!(acknowledged, "detach lost an unacknowledged debug event");
        assert_eq!(child.command("R", "PROBE_REG:"), child.original_regs);
        assert_eq!(child.command("T", "PROBE_OUT:"), "1");
        drop(child);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn refused_exit_acknowledgement_keeps_the_real_event_code_and_reaps_the_child() {
        if isolated_test(
            "refused_exit_acknowledgement_keeps_the_real_event_code_and_reaps_the_child",
        ) {
            return;
        }
        let mut child = Fixture::start();
        let pid = child.child.id();
        let path = std::env::temp_dir().join(format!(
            "pr86-exit-retry-{}-{pid}.jsonl",
            std::process::id()
        ));
        let output = Jsonl::new(&path).unwrap();
        let base = child.entry - CAN_EVER_RVA;
        let (_stop_tx, stop_rx) = mpsc::channel();
        let observer = thread::spawn(move || {
            let game = process(pid, true).unwrap();
            let mut debugger =
                Debugger::attach(pid, game, base, 0x500000, output, ProbeMode::Lifecycle).unwrap();
            debugger.exit_continue_fault = true;
            let result = debugger.run(stop_rx, Duration::from_secs(10), || Ok(()));
            (
                result,
                debugger.terminal,
                debugger.lifecycle.exit_code,
                debugger.cleanup_event.is_none(),
            )
        });
        let start = Instant::now();
        while !fs::read_to_string(&path).is_ok_and(|s| s.contains("\"type\":\"ready\"")) {
            assert!(start.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(20));
        }
        let status = child.terminate();
        let (result, terminal, code, acknowledged) = join_observer(observer).unwrap();
        assert!(result
            .unwrap_err()
            .contains("injected exit ContinueDebugEvent refusal"));
        assert_eq!(terminal, TerminalKind::TargetExited);
        assert_eq!(code, status.code().map(|value| value as u32));
        assert!(acknowledged);
        drop(child);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn watchdog_stalled_fixture_worker() {
        if !enter_worker("watchdog_stalled_fixture_worker") {
            return;
        }
        let mut child = Fixture::start();
        child.send("H").unwrap();
        println!("WATCHDOG_FIXTURE:{}", child.child.id());
        std::io::stdout().flush().unwrap();
        join_observer(thread::spawn(|| loop {
            thread::park();
        }))
        .unwrap();
    }

    #[test]
    fn watchdog_panicking_fixture_worker() {
        if !enter_worker("watchdog_panicking_fixture_worker") {
            return;
        }
        let mut child = Fixture::start();
        child.send("H").unwrap();
        println!("WATCHDOG_FIXTURE:{}", child.child.id());
        std::io::stdout().flush().unwrap();
        panic!("injected fixture-owner panic");
    }

    #[test]
    fn property_mode_four_breakpoints_restore_after_synthetic_child_hit() {
        if isolated_test("property_mode_four_breakpoints_restore_after_synthetic_child_hit") {
            return;
        }
        let mut child = Fixture::start();
        let path = std::env::temp_dir().join(format!(
            "pr86-property-mode-{}-{}.jsonl",
            std::process::id(),
            child.child.id()
        ));
        let _ = fs::remove_file(&path);
        let output = Jsonl::new(&path).unwrap();
        let base = child.entry - PLAYER_GETTER_RVA;
        let pid = child.child.id();
        let (stop_tx, stop_rx) = mpsc::channel();
        let observer = child.observe(&stop_tx, move |close_ack| {
            let game = process(pid, true).unwrap();
            let mut debugger =
                Debugger::attach(pid, game, base, 0x800000, output, ProbeMode::Property).unwrap();
            debugger.close_ack = close_ack;
            debugger.zero_context_dr6_for_test = true;
            debugger.run(stop_rx, Duration::from_secs(3), || Ok(()))
        });
        let started = Instant::now();
        while !fs::read_to_string(&path).is_ok_and(|s| s.contains("\"type\":\"ready\"")) {
            assert!(started.elapsed() < Duration::from_secs(15));
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(child.command("U", "PROBE_OUT:"), "Utrue");
        assert_eq!(child.command("T", "PROBE_OUT:"), "1");
        let active_registers = child.command("R", "PROBE_REG:");
        let fields: Vec<_> = active_registers.split(':').collect();
        assert_eq!(
            u64::from_str_radix(fields[2], 16).unwrap(),
            base + PROPERTY_AFTER_RVA
        );
        assert_eq!(
            u64::from_str_radix(fields[3], 16).unwrap(),
            base + PROPERTY_NAME_RVA
        );
        assert_eq!(u64::from_str_radix(fields[5], 16).unwrap() & 0xff, 0x55);
        assert_eq!(child.command("M", "PROBE_OUT:"), "M10");
        stop_tx.send(()).unwrap();
        assert!(join_observer(observer).unwrap().is_ok());
        assert_eq!(child.command("R", "PROBE_REG:"), child.original_regs);
        let trace = fs::read_to_string(&path).unwrap();
        assert!(trace.contains("\"type\":\"player_getter\""));
        let rows: Vec<Value> = trace
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(rows.iter().any(|row| row["type"] == "debug_exception"
            && row["owned_breakpoint"] == true
            && row["dr6"] == "0x0000000000000000"
            && row["exception_address"] == format!("0x{:016x}", child.entry)));
        assert!(rows
            .iter()
            .any(|row| row["type"] == "debug_exception" && row["dispatch"] == "unrelated"));
        assert!(!rows
            .iter()
            .any(|row| row["type"] == "entry" || row["type"] == "return"));
        let getter_threads: BTreeSet<_> = rows
            .iter()
            .filter(|row| row["type"] == "player_getter")
            .map(|row| row["thread_id"].as_u64().unwrap())
            .collect();
        assert!(
            getter_threads.len() >= 2,
            "exited worker threads were not observed"
        );
        fs::remove_file(path).unwrap();
    }

    fn lifecycle_child_case(guard_failure: bool) {
        let mut child = Fixture::start();
        let pid = child.child.id();
        let path = std::env::temp_dir().join(format!(
            "pr86-lifecycle-{}-{pid}-{}.jsonl",
            std::process::id(),
            guard_failure
        ));
        let _ = fs::remove_file(&path);
        let output = Jsonl::new(&path).unwrap();
        let base = child.entry - PLAYER_GETTER_RVA;
        let (stop_tx, stop_rx) = mpsc::channel();
        let fail = Arc::new(AtomicBool::new(false));
        let fail_observer = Arc::clone(&fail);
        let observer = child.observe(&stop_tx, move |close_ack| {
            let game = process(pid, false).unwrap();
            let mut debugger =
                Debugger::attach(pid, game, base, 0x800000, output, ProbeMode::Lifecycle).unwrap();
            debugger.close_ack = close_ack;
            // Inject only into the dispatcher, before any real OS event. This
            // is not a child-generated breakpoint and is never continued.
            let mut app_event = DEBUG_EVENT {
                dwDebugEventCode: EXCEPTION_DEBUG_EVENT,
                dwProcessId: pid,
                dwThreadId: u32::MAX,
                ..Default::default()
            };
            let record = EXCEPTION_RECORD {
                ExceptionCode: EXCEPTION_BREAKPOINT,
                ExceptionAddress: debugger.attach_origin.breakpoint_address as *mut _,
                ..Default::default()
            };
            app_event.u.Exception = EXCEPTION_DEBUG_INFO {
                ExceptionRecord: record,
                dwFirstChance: 1,
            };
            assert_eq!(
                debugger.handle_event(&app_event).unwrap(),
                DBG_EXCEPTION_NOT_HANDLED
            );
            assert!(!debugger.ready);
            let result = debugger.run(stop_rx, Duration::from_secs(10), || {
                if fail_observer.load(Ordering::SeqCst) {
                    Err("synthetic controller closed".into())
                } else {
                    Ok(())
                }
            });
            (
                result,
                debugger.attached,
                debugger.lifecycle.thread_events,
                debugger.threads.len(),
                debugger.attach_origin,
                debugger.terminal,
            )
        });
        let started = Instant::now();
        while !fs::read_to_string(&path).is_ok_and(|s| s.contains("\"type\":\"ready\"")) {
            assert!(started.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(child.command("R", "PROBE_REG:"), child.original_regs);
        assert_eq!(child.command("T", "PROBE_OUT:"), "1");
        assert_eq!(child.command("U", "PROBE_OUT:"), "Utrue");
        assert_eq!(child.command("M", "PROBE_OUT:"), "M10");
        if guard_failure {
            fail.store(true, Ordering::SeqCst);
        } else {
            stop_tx.send(()).unwrap();
        }
        let (result, attached, thread_events, armed_threads, origin, terminal) =
            join_observer(observer).unwrap();
        if guard_failure {
            assert!(result.unwrap_err().contains("synthetic controller closed"));
        } else {
            assert!(result.is_ok(), "{result:?}");
        }
        assert!(!attached);
        assert_eq!(terminal, TerminalKind::Detached);
        assert_eq!(armed_threads, 0);
        assert!(thread_events >= 2);
        assert_eq!(child.command("R", "PROBE_REG:"), child.original_regs);
        assert_eq!(child.command("T", "PROBE_OUT:"), "1");
        let rows: Vec<Value> = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert!(origin.accepted);
        let attach = rows
            .iter()
            .find(|row| {
                row["type"] == "debug_exception" && row["disposition"] == "proven_attach_breakpoint"
            })
            .unwrap();
        let stopped = rows
            .iter()
            .find(|row| {
                row["type"] == "thread_status"
                    && row["source"] == "exception_debug_event"
                    && row["thread_id"] == attach["thread_id"]
            })
            .unwrap();
        assert_eq!(stopped["win32_start_query_status"], "0x00000000");
        assert_eq!(
            stopped["win32_start_query_returned_len"],
            size_of::<usize>() as u64
        );
        assert_eq!(
            stopped["win32_start_address"],
            format!("0x{:016x}", origin.breakin_start)
        );
        let rip = u64::from_str_radix(
            stopped["rip"].as_str().unwrap().trim_start_matches("0x"),
            16,
        )
        .unwrap();
        assert_eq!(rip, origin.breakpoint_address + 1);
        assert_eq!(attach["stopped_rip"], stopped["rip"]);
        assert_eq!(
            attach["verified_win32_start"],
            stopped["win32_start_address"]
        );
        assert!(rows.iter().any(|row| row["type"] == "debug_exception"
            && row["disposition"] == "application_handler"
            && row["thread_id"] == u32::MAX
            && row["event_ordinal"].as_u64().unwrap() < attach["event_ordinal"].as_u64().unwrap()));
        assert_eq!(attach["thread_id"], origin.breakin_thread.unwrap());
        assert_eq!(
            attach["exception_address"],
            format!("0x{:016x}", origin.breakpoint_address)
        );
        assert!(rows.iter().any(|row| row["type"] == "thread_status"
            && row["thread_id"] == origin.breakin_thread.unwrap()
            && row["start_address_candidate"] == format!("0x{:016x}", origin.breakin_start)
            && row["attach_thread_candidate"] == true));
        assert!(rows.iter().any(
            |row| row["type"] == "ready" && row["thread_count"].as_u64().is_some_and(|n| n > 0)
        ));
        let ready = rows.iter().find(|row| row["type"] == "ready").unwrap();
        let step = rows
            .iter()
            .find(|row| {
                row["type"] == "debug_exception"
                    && row["code"] == "0x80000004"
                    && row["disposition"] == "application_handler"
            })
            .unwrap();
        assert!(step["event_ordinal"].as_u64().unwrap() > ready["event_ordinal"].as_u64().unwrap());
        assert!(step["observed_unix_ms"].as_u64().is_some());
        assert!(rows
            .iter()
            .any(|row| row["type"] == "thread_status"
                && row["future_exception_delivery"] == "unknown"));
        assert!(rows.iter().any(|row| row["type"] == "thread_status"
            && row["handle_identity"] == "matched"
            && row["owner_pid"] == pid));
        assert!(!rows.iter().any(|row| matches!(
            row["type"].as_str(),
            Some("entry" | "return" | "player_getter" | "property_after")
        )));
        drop(child);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn lifecycle_stop_keeps_debug_registers_unchanged_and_forwards_foreign_step() {
        if isolated_test("lifecycle_stop_keeps_debug_registers_unchanged_and_forwards_foreign_step")
        {
            return;
        }
        lifecycle_child_case(false);
    }

    #[test]
    fn lifecycle_controller_error_detaches_without_debug_register_writes() {
        if isolated_test("lifecycle_controller_error_detaches_without_debug_register_writes") {
            return;
        }
        lifecycle_child_case(true);
    }

    #[test]
    fn lifecycle_parent_close_reports_exit_without_inventing_an_exception() {
        if isolated_test("lifecycle_parent_close_reports_exit_without_inventing_an_exception") {
            return;
        }
        let mut child = Fixture::start();
        let pid = child.child.id();
        let path = std::env::temp_dir().join(format!(
            "pr86-lifecycle-exit-{}-{pid}.jsonl",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let output = Jsonl::new(&path).unwrap();
        let base = child.entry - PLAYER_GETTER_RVA;
        let (stop_tx, stop_rx) = mpsc::channel();
        let observer = child.observe(&stop_tx, move |close_ack| {
            let game = process(pid, false).unwrap();
            let mut debugger =
                Debugger::attach(pid, game, base, 0x800000, output, ProbeMode::Lifecycle).unwrap();
            debugger.close_ack = close_ack;
            let result = debugger.run(stop_rx, Duration::from_secs(10), || Ok(()));
            (result, debugger.terminal, debugger.lifecycle.exit_code)
        });
        let started = Instant::now();
        while !fs::read_to_string(&path).is_ok_and(|s| s.contains("\"type\":\"ready\"")) {
            assert!(started.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(child.command("R", "PROBE_REG:"), child.original_regs);
        child.terminate(); // Only this case's synthetic child.
        let (result, terminal, exit_code) = join_observer(observer).unwrap();
        assert!(result.unwrap_err().contains("target process exited"));
        assert_eq!(terminal, TerminalKind::TargetExited);
        assert!(exit_code.is_some());
        let rows: Vec<Value> = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let ready = rows.iter().find(|row| row["type"] == "ready").unwrap();
        let exit = rows
            .iter()
            .find(|row| row["type"] == "process_exit" && row["exit_code"].as_u64().is_some())
            .unwrap();
        assert!(exit["event_ordinal"].as_u64().unwrap() > ready["event_ordinal"].as_u64().unwrap());
        assert!(!rows
            .iter()
            .any(|row| row["type"] == "debug_exception" && row["code"] == "0x80000004"));
        drop(child);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn critical_pump_preserves_exit_event_and_signaled_child_codes() {
        if isolated_test("critical_pump_preserves_exit_event_and_signaled_child_codes") {
            return;
        }
        let mut event = DEBUG_EVENT {
            dwDebugEventCode: EXIT_PROCESS_DEBUG_EVENT,
            ..Default::default()
        };
        event.u.ExitProcess = EXIT_PROCESS_DEBUG_INFO {
            dwExitCode: 0xc0000005,
        };
        let observed = critical_exit_from_debug_event(&event).unwrap();
        assert_eq!(observed.code, Some(0xc0000005));
        assert_eq!(observed.source, "exit_process_debug_event");

        let mut child = Fixture::start();
        let game = process(child.child.id(), false).unwrap();
        child.terminate(); // Only this case's synthetic child.
        let observed = announce_critical_and_wait_for_owned_game(
            &game,
            child.child.id(),
            "synthetic close",
            None,
            false,
            None,
            None,
            None,
            true,
        )
        .unwrap();
        assert!(observed.code.is_some());
        assert_eq!(observed.source, "GetExitCodeProcess");
    }

    fn run_case(
        timeout: Duration,
        trigger: bool,
        fail_guard: bool,
        fail_recording: bool,
        restore_faults: usize,
        mask_dr6: bool,
    ) {
        let mut child = Fixture::start();
        let probe = process(child.child.id(), true).unwrap();
        let created = process_created(&probe).unwrap();
        let image = process_path(&probe).unwrap();
        assert!(alive(&probe).unwrap());
        assert!(exact_process(child.child.id(), created, &image, true).is_ok());
        let path = std::env::temp_dir().join(format!(
            "pr86-probe-test-{}-{}.jsonl",
            std::process::id(),
            child.child.id()
        ));
        let _ = fs::remove_file(&path);
        let mut output = Jsonl::new(&path).unwrap();
        if fail_recording {
            output.rows = MAX_ROWS - 1;
        }
        let base = child.entry - CAN_EVER_RVA;
        let (stop_tx, stop_rx): (Sender<()>, Receiver<()>) = mpsc::channel();
        let pid = child.child.id();
        let fail = Arc::new(AtomicBool::new(false));
        let fail_in_observer = Arc::clone(&fail);
        let observer = child.observe(&stop_tx, move |close_ack| {
            let game = process(pid, true).unwrap();
            let mut debugger =
                Debugger::attach(pid, game, base, 0x500000, output, ProbeMode::Governor).unwrap();
            debugger.close_ack = close_ack;
            debugger.restore_faults = restore_faults;
            debugger.zero_context_dr6_for_test = mask_dr6;
            let result = debugger.run(stop_rx, timeout, || {
                if fail_in_observer.load(Ordering::SeqCst) {
                    Err("fixture guard lost".into())
                } else {
                    Ok(())
                }
            });
            (result, debugger.attached)
        });
        let started = Instant::now();
        loop {
            if fs::read_to_string(&path).is_ok_and(|s| s.contains("\"type\":\"ready\"")) {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(15),
                "fixture debugger did not become ready"
            );
            thread::sleep(Duration::from_millis(20));
        }
        if fail_guard {
            fail.store(true, Ordering::SeqCst);
        }
        if fail_recording {
            assert_eq!(
                child.command("T", "PROBE_OUT:"),
                "1",
                "child died on recording failure"
            );
        }
        if trigger {
            assert_eq!(child.command("T", "PROBE_OUT:"), "1");
            assert_eq!(child.command("F", "PROBE_OUT:"), "0");
            assert_eq!(child.command("M", "PROBE_OUT:"), "M10");
            stop_tx.send(()).unwrap();
        }
        let (result, attached) = join_observer(observer).unwrap();
        if fail_guard || fail_recording {
            assert!(result.is_err(), "failure did not abort the observer");
        } else {
            assert!(result.is_ok(), "{result:?}");
        }
        assert!(!attached);
        let rows: Vec<Value> = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        if trigger {
            let entries: Vec<_> = rows
                .iter()
                .filter(|r| r["type"] == "entry" && r["function"] == "can_ever")
                .collect();
            let returns: Vec<_> = rows
                .iter()
                .filter(|r| r["type"] == "return" && r["function"] == "can_ever")
                .collect();
            assert!(entries.len() >= 4, "entries: {rows:?}");
            assert!(
                returns.iter().any(|r| r["bool_candidate"] == true),
                "returns: {rows:?}"
            );
            assert!(
                returns.iter().any(|r| r["bool_candidate"] == false),
                "returns: {rows:?}"
            );
            assert!(entries.iter().all(|r| r["this"] == "0x0000000000001234"));
            assert!(entries.iter().any(|r| r["governor_index"] == 7));
            assert!(entries.iter().any(|r| r["governor_index"] == 8));
            let tids: BTreeSet<_> = entries
                .iter()
                .map(|r| r["thread_id"].as_u64().unwrap())
                .collect();
            assert!(tids.len() >= 2, "new threads were not traced: {rows:?}");
            if mask_dr6 {
                assert!(
                    rows.iter().any(|r| r["type"] == "debug_exception"
                        && r["code"] == "0x80000004"
                        && r["dr6"] == "0x0000000000000000"
                        && r["exception_address"] == format!("0x{:016x}", child.entry)
                        && r["owned_breakpoint"] == true),
                    "missing sanitized-DR6 diagnostic: {rows:?}"
                );
            }
        }
        assert_eq!(
            child.command("R", "PROBE_REG:"),
            child.original_regs,
            "debug registers were not restored"
        );
        assert_eq!(
            child.command("T", "PROBE_OUT:"),
            "1",
            "child did not survive detach"
        );
        drop(child);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn entry_return_threads_and_normal_detach_restore_registers() {
        if isolated_test("entry_return_threads_and_normal_detach_restore_registers") {
            return;
        }
        run_case(Duration::from_secs(20), true, false, false, 0, false);
    }
    #[test]
    fn real_child_unwind_and_same_frame_reuse_never_assigns_old_id_a_new_al() {
        if isolated_test("real_child_unwind_and_same_frame_reuse_never_assigns_old_id_a_new_al") {
            return;
        }
        let mut child = Fixture::start();
        let pid = child.child.id();
        let path = std::env::temp_dir().join(format!(
            "pr86-return-reuse-{}-{pid}.jsonl",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let output = Jsonl::new(&path).unwrap();
        let base = child.entry - CAN_EVER_RVA;
        let (stop_tx, stop_rx) = mpsc::channel();
        let observer = child.observe(&stop_tx, move |close_ack| {
            let game = process(pid, true).unwrap();
            let mut debugger =
                Debugger::attach(pid, game, base, 0x500000, output, ProbeMode::Governor).unwrap();
            debugger.close_ack = close_ack;
            debugger.run(stop_rx, Duration::from_secs(10), || Ok(()))
        });
        let started = Instant::now();
        while !fs::read_to_string(&path).is_ok_and(|s| s.contains("\"type\":\"ready\"")) {
            assert!(started.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(child.command("W", "PROBE_OUT:"), "W1");
        stop_tx.send(()).unwrap();
        assert!(join_observer(observer).unwrap().is_ok());
        let rows: Vec<Value> = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|row| serde_json::from_str(row).unwrap())
            .collect();
        let old = rows
            .iter()
            .find(|r| r["type"] == "entry" && r["governor_index"] == 99)
            .unwrap();
        let new = rows
            .iter()
            .find(|r| r["type"] == "entry" && r["governor_index"] == 7)
            .unwrap();
        assert_eq!(old["return_site_rva"], new["return_site_rva"]);
        assert_eq!(old["expected_rsp"], new["expected_rsp"]);
        assert_eq!(old["thread_id"], new["thread_id"]);
        assert!(rows
            .iter()
            .any(|r| r["type"] == "return_incomplete" && r["id"] == old["id"]));
        assert!(!rows
            .iter()
            .any(|r| r["type"] == "return" && r["id"] == old["id"]));
        assert!(rows
            .iter()
            .any(|r| r["type"] == "return" && r["id"] == new["id"] && r["bool_candidate"] == true));
        assert_eq!(child.command("R", "PROBE_REG:"), child.original_regs);
        assert_eq!(child.command("T", "PROBE_OUT:"), "1");
        drop(child);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn timeout_detach_preserves_the_child_and_registers() {
        if isolated_test("timeout_detach_preserves_the_child_and_registers") {
            return;
        }
        run_case(Duration::from_millis(500), false, false, false, 0, false);
    }
    #[test]
    fn failure_detach_preserves_the_child_and_registers() {
        if isolated_test("failure_detach_preserves_the_child_and_registers") {
            return;
        }
        run_case(Duration::from_secs(20), false, true, false, 0, false);
    }
    #[test]
    fn recording_failure_detaches_without_killing_the_child() {
        if isolated_test("recording_failure_detaches_without_killing_the_child") {
            return;
        }
        run_case(Duration::from_secs(20), false, false, true, 0, false);
    }
    #[test]
    fn one_injected_context_failure_retries_and_restores_the_live_child() {
        if isolated_test("one_injected_context_failure_retries_and_restores_the_live_child") {
            return;
        }
        run_case(Duration::from_millis(500), false, false, false, 1, false);
    }
    #[test]
    fn real_windows_hardware_exception_survives_sanitized_dr6_view() {
        if isolated_test("real_windows_hardware_exception_survives_sanitized_dr6_view") {
            return;
        }
        // The old dispatcher required DR6.B0 and would forward this real
        // STATUS_SINGLE_STEP to the child, which has no handler for it.
        run_case(Duration::from_secs(20), true, false, false, 0, true);
    }
    fn run_ambiguous_real_child_case(mode: ProbeMode, context_failure: bool) {
        let mut child = Fixture::start();
        let pid = child.child.id();
        let path =
            std::env::temp_dir().join(format!("pr86-ambiguous-{}-{pid}.jsonl", std::process::id()));
        let _ = fs::remove_file(&path);
        let output = Jsonl::new(&path).unwrap();
        let base = child.entry
            - if mode == ProbeMode::Property {
                PLAYER_GETTER_RVA
            } else {
                CAN_EVER_RVA
            };
        let signal = Arc::new(AtomicBool::new(false));
        let observed_signal = Arc::clone(&signal);
        let (ack_tx, ack_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let observer = child.observe(&stop_tx, move |close_ack| {
            let game = process(pid, true).unwrap();
            let mut debugger = Debugger::attach(pid, game, base, 0x800000, output, mode).unwrap();
            debugger.close_ack = close_ack;
            debugger.critical_signal = Some(observed_signal);
            debugger.force_context_failure = context_failure;
            debugger.force_reported_rip_mismatch = !context_failure;
            ack_tx.send(Arc::clone(&debugger.close_ack)).unwrap();
            let result = debugger.run(stop_rx, Duration::from_secs(10), || Ok(()));
            (result, debugger.critical, debugger.cleanup_gate.is_some())
        });
        let close_ack = ack_rx.recv().unwrap();
        let start = Instant::now();
        while !fs::read_to_string(&path).is_ok_and(|s| s.contains("\"type\":\"ready\"")) {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "fixture observer did not become ready"
            );
            thread::sleep(Duration::from_millis(20));
        }
        child.send("T").unwrap();
        while !signal.load(Ordering::SeqCst) {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "ambiguous exception did not enter critical cleanup"
            );
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            child.child.try_wait().unwrap().is_none(),
            "the pending exception was released before owned-game closure"
        );
        thread::sleep(Duration::from_millis(100));
        assert!(
            child.child.try_wait().unwrap().is_none(),
            "critical cleanup resumed normal execution before closure ack"
        );
        child.child.kill().unwrap();
        close_ack.store(true, Ordering::SeqCst); // Controller acknowledged closing this owned Rust child.
        child.wait_for_exit();
        let (result, critical, normal_cleanup_gate) = join_observer(observer).unwrap();
        assert!(result.unwrap_err().contains("ambiguous debug exception"));
        assert!(critical);
        assert!(
            !normal_cleanup_gate,
            "ambiguous exception entered normal detach gate"
        );
        let rows: Vec<Value> = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert!(rows.iter().any(|r| r["type"] == "debug_exception"
            && r["dispatch"] == "ambiguous"
            && r["code"] == "0x80000004"
            && r["first_chance"] == true
            && r["exception_address"] == format!("0x{:016x}", child.entry)));
        assert!(rows
            .iter()
            .any(|r| r["type"] == "critical_cleanup" && r["ambiguous_exception"] == true));
        assert!(!rows.iter().any(|r| {
            r["type"] == "entry" || r["type"] == "return" || r["type"] == "player_getter"
        }));
        drop(stop_tx);
        drop(child);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn rip_mismatch_on_real_hardware_event_requires_same_child_close() {
        if isolated_test("rip_mismatch_on_real_hardware_event_requires_same_child_close") {
            return;
        }
        run_ambiguous_real_child_case(ProbeMode::Governor, false);
        run_ambiguous_real_child_case(ProbeMode::Property, false);
    }
    #[test]
    fn unreadable_context_on_real_hardware_event_requires_same_child_close() {
        if isolated_test("unreadable_context_on_real_hardware_event_requires_same_child_close") {
            return;
        }
        run_ambiguous_real_child_case(ProbeMode::Governor, true);
        run_ambiguous_real_child_case(ProbeMode::Property, true);
    }
    #[test]
    fn late_real_windows_attach_breakpoint_never_announces_ready() {
        if isolated_test("late_real_windows_attach_breakpoint_never_announces_ready") {
            return;
        }
        let mut child = Fixture::start();
        let pid = child.child.id();
        let path = std::env::temp_dir().join(format!(
            "pr86-late-attach-{}-{pid}.jsonl",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let output = Jsonl::new(&path).unwrap();
        let base = child.entry - CAN_EVER_RVA;
        let (stop_tx, stop_rx) = mpsc::channel();
        let observer = child.observe(&stop_tx, move |close_ack| {
            let game = process(pid, true).unwrap();
            let mut debugger =
                Debugger::attach(pid, game, base, 0x500000, output, ProbeMode::Governor).unwrap();
            debugger.close_ack = close_ack;
            debugger.force_initial_breakpoint_late = true;
            let result = debugger.run(stop_rx, Duration::from_secs(10), || Ok(()));
            (result, debugger.attached)
        });
        let (result, attached) = join_observer(observer).unwrap();
        assert!(result
            .unwrap_err()
            .contains("initial debugger breakpoint arrived after"));
        assert!(!attached);
        let rows: Vec<Value> = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert!(!rows.iter().any(|r| r["type"] == "ready"));
        assert_eq!(child.command("T", "PROBE_OUT:"), "1");
        assert_eq!(child.command("R", "PROBE_REG:"), child.original_regs);
        drop(child);
        fs::remove_file(path).unwrap();
    }
    #[test]
    fn unrelated_real_single_step_reaches_the_child_exception_handler() {
        if isolated_test("unrelated_real_single_step_reaches_the_child_exception_handler") {
            return;
        }
        let mut child = Fixture::start();
        let pid = child.child.id();
        let path = std::env::temp_dir().join(format!(
            "pr86-unowned-step-{}-{pid}.jsonl",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let output = Jsonl::new(&path).unwrap();
        let base = child.entry - CAN_EVER_RVA;
        let (stop_tx, stop_rx) = mpsc::channel();
        let observer = child.observe(&stop_tx, move |close_ack| {
            let game = process(pid, true).unwrap();
            let mut debugger =
                Debugger::attach(pid, game, base, 0x500000, output, ProbeMode::Governor).unwrap();
            debugger.close_ack = close_ack;
            debugger.run(stop_rx, Duration::from_secs(10), || Ok(()))
        });
        let started = Instant::now();
        while !fs::read_to_string(&path).is_ok_and(|s| s.contains("\"type\":\"ready\"")) {
            assert!(
                started.elapsed() < Duration::from_secs(5),
                "fixture debugger did not become ready"
            );
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(child.command("U", "PROBE_OUT:"), "Utrue");
        stop_tx.send(()).unwrap();
        join_observer(observer).unwrap().unwrap();
        let rows: Vec<Value> = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect();
        assert!(rows.iter().any(|r| r["type"] == "debug_exception"
            && r["code"] == "0x80000004"
            && r["first_chance"] == true
            && r["rip"].is_string()
            && r["dr6"].is_string()
            && r["dr7"].is_string()
            && r["owned_breakpoint"] == false));
        assert_eq!(child.command("T", "PROBE_OUT:"), "1");
        drop(child);
        fs::remove_file(path).unwrap();
    }

    #[derive(Default)]
    struct FaultCleanup {
        calls: Vec<&'static str>,
        target_exited: bool,
        freeze_fail: usize,
        set_fail: usize,
        readback_fail: usize,
        detach_fail: usize,
        resume_fail: usize,
        resume_delay: Duration,
    }
    impl CleanupOps for FaultCleanup {
        fn game_exited(&mut self) -> bool {
            self.target_exited
        }
        fn freeze(&mut self) -> Result<(), String> {
            self.calls.push("freeze");
            if self.freeze_fail > 0 {
                self.freeze_fail -= 1;
                Err("SuspendThread refused".into())
            } else {
                Ok(())
            }
        }
        fn restore_verified(&mut self) -> Result<(), String> {
            self.calls.push("restore_readback");
            if self.set_fail > 0 {
                self.set_fail -= 1;
                Err("SetThreadContext refused".into())
            } else if self.readback_fail > 0 {
                self.readback_fail -= 1;
                Err("GetThreadContext disagreed".into())
            } else {
                Ok(())
            }
        }
        fn continue_pending(&mut self) -> Result<(), String> {
            self.calls.push("continue");
            Ok(())
        }
        fn detach(&mut self) -> Result<(), String> {
            self.calls.push("detach");
            if self.detach_fail > 0 {
                self.detach_fail -= 1;
                Err("DebugActiveProcessStop refused".into())
            } else {
                Ok(())
            }
        }
        fn resume(&mut self) -> Result<(), String> {
            self.calls.push("resume");
            thread::sleep(self.resume_delay);
            if self.resume_fail > 0 {
                self.resume_fail -= 1;
                Err("ResumeThread refused".into())
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn os_failures_retry_and_never_detach_before_verified_registers() {
        let mut os = FaultCleanup {
            freeze_fail: 1,
            set_fail: 1,
            readback_fail: 1,
            detach_fail: 1,
            resume_fail: 1,
            ..Default::default()
        };
        let mut gate = CleanupGate::new();
        gate.drive(&mut os).unwrap();
        assert_eq!(gate.phase, CleanupPhase::Done);
        assert_eq!(os.calls.iter().filter(|c| **c == "detach").count(), 2);
        let first_detach = os.calls.iter().position(|c| *c == "detach").unwrap();
        assert_eq!(
            os.calls[..first_detach]
                .iter()
                .filter(|c| **c == "restore_readback")
                .count(),
            3
        );
        assert!(
            os.calls.iter().rposition(|c| *c == "resume").unwrap()
                > os.calls.iter().rposition(|c| *c == "detach").unwrap()
        );
    }
    #[test]
    fn permanent_os_refusal_is_bounded_critical_and_does_not_detach() {
        for set_failure in [false, true] {
            let mut os = FaultCleanup::default();
            if set_failure {
                os.set_fail = usize::MAX;
            } else {
                os.freeze_fail = usize::MAX;
            }
            let mut gate = CleanupGate::new();
            gate.deadline = Instant::now() + Duration::from_millis(90);
            let error = gate.drive(&mut os).unwrap_err();
            assert!(error.contains("critical_cleanup"));
            assert!(!os.calls.contains(&"detach"));
            assert!(!os.calls.contains(&"resume"));
        }
    }
    #[test]
    fn a_successful_resume_after_the_absolute_deadline_is_still_a_failure() {
        let mut os = FaultCleanup {
            resume_delay: Duration::from_millis(60),
            ..Default::default()
        };
        let mut gate = CleanupGate::new();
        gate.deadline = Instant::now() + Duration::from_millis(30);
        let error = gate.drive(&mut os).unwrap_err();
        assert!(error.contains("critical_cleanup"));
        assert!(error.contains("Resume"));
        assert_eq!(
            os.calls,
            ["freeze", "restore_readback", "continue", "detach", "resume"]
        );
    }
    #[test]
    fn suspend_failure_does_not_call_unknown_liveness_alive() {
        assert_eq!(
            suspend_failure_state(WAIT_OBJECT_0, WAIT_TIMEOUT),
            SuspendFailureState::Gone
        );
        assert_eq!(
            suspend_failure_state(WAIT_TIMEOUT, WAIT_OBJECT_0),
            SuspendFailureState::TargetExited
        );
        assert_eq!(
            suspend_failure_state(WAIT_TIMEOUT, WAIT_TIMEOUT),
            SuspendFailureState::Unsignaled
        );
        assert_eq!(
            suspend_failure_state(u32::MAX, WAIT_TIMEOUT),
            SuspendFailureState::Unknown
        );
        let mut os = FaultCleanup {
            target_exited: true,
            ..Default::default()
        };
        let error = CleanupGate::new().drive(&mut os).unwrap_err();
        assert!(error.contains("target_exited_during_cleanup"));
        assert!(!os.calls.contains(&"detach"));
    }
    #[test]
    fn exited_windows_thread_handle_is_not_reported_live_after_suspend_failure() {
        let (tid_tx, tid_rx) = mpsc::channel();
        let (exit_tx, exit_rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            let tid = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
            tid_tx.send(tid).unwrap();
            exit_rx.recv().unwrap();
        });
        let tid = tid_rx.recv().unwrap();
        let pinned = thread_handle(tid).unwrap();
        exit_tx.send(()).unwrap();
        worker.join().unwrap();
        let wait = unsafe { WaitForSingleObject(pinned.0, 0) };
        assert_eq!(wait, WAIT_OBJECT_0);
        assert_eq!(
            suspend_failure_state(wait, WAIT_TIMEOUT),
            SuspendFailureState::Gone
        );
    }

    #[derive(Default)]
    struct FaultAttach {
        detach_fail: usize,
        kill_fail: usize,
        exited: bool,
        calls: Vec<&'static str>,
    }
    impl AttachAbortOps for FaultAttach {
        fn detach(&mut self) -> bool {
            self.calls.push("detach");
            if self.detach_fail > 0 {
                self.detach_fail -= 1;
                false
            } else {
                true
            }
        }
        fn disable_kill_on_exit(&mut self) -> bool {
            self.calls.push("disable_kill");
            if self.kill_fail > 0 {
                self.kill_fail -= 1;
                false
            } else {
                true
            }
        }
        fn game_alive(&mut self) -> Result<bool, String> {
            Ok(!self.exited)
        }
    }
    #[test]
    fn failed_kill_setting_requires_detach_game_exit_or_critical() {
        let mut os = FaultAttach {
            detach_fail: 2,
            kill_fail: usize::MAX,
            ..Default::default()
        };
        let mut gate = AttachAbortGate::new();
        assert_eq!(gate.step(&mut os), None);
        assert_eq!(gate.step(&mut os), None);
        assert_eq!(gate.step(&mut os), Some(AttachAbortOutcome::Detached));
        let mut os = FaultAttach {
            detach_fail: usize::MAX,
            kill_fail: usize::MAX,
            ..Default::default()
        };
        let mut gate = AttachAbortGate::new();
        gate.deadline = Instant::now() + Duration::from_millis(90);
        assert_eq!(gate.drive(&mut os), AttachAbortOutcome::Critical);
        assert!(os.calls.contains(&"detach"));
        let mut os = FaultAttach {
            detach_fail: usize::MAX,
            ..Default::default()
        };
        let mut gate = AttachAbortGate::new();
        gate.deadline = Instant::now() + Duration::from_millis(90);
        assert_eq!(
            gate.drive(&mut os),
            AttachAbortOutcome::KillDisabledWithoutDetach
        );
    }
    #[test]
    fn single_step_requires_matching_address_context_and_our_enabled_slot() {
        let base = 0x1a49b0000;
        let ever = base + CAN_EVER_RVA;
        let appoint = base + CAN_APPOINT_RVA;
        let armed = [Some(ever), Some(appoint), None, None];
        let mut c: CONTEXT = unsafe { zeroed() };
        c.Rip = ever;
        c.Dr0 = ever;
        c.Dr1 = appoint;
        c.Dr7 = 0b0101;
        c.Dr6 = 0; // Missing status does not discard matching owned evidence.
        let empty = ReturnCorrelation::default();
        assert_eq!(
            classify_single_step(ProbeMode::Governor, &empty, &armed, ever, Some(&c)),
            DispatchOutcome::OwnedProven(OwnedTrap::CanEver)
        );
        c.Rip = ever + 1;
        assert_eq!(
            classify_single_step(ProbeMode::Governor, &empty, &armed, ever, Some(&c)),
            DispatchOutcome::Ambiguous("exception_address_rip_disagree")
        );
        c.Rip = ever;
        c.Dr7 = 0;
        assert_eq!(
            classify_single_step(ProbeMode::Governor, &empty, &armed, ever, Some(&c)),
            DispatchOutcome::Ambiguous("armed_debug_registers_disagree")
        );
        c.Dr7 = 0b0101 | (1u64 << 16);
        assert_eq!(
            classify_single_step(ProbeMode::Governor, &empty, &armed, ever, Some(&c)),
            DispatchOutcome::Ambiguous("armed_debug_registers_disagree")
        );
        assert_eq!(
            classify_single_step(ProbeMode::Governor, &empty, &armed, ever, None),
            DispatchOutcome::Ambiguous("context_unavailable")
        );
        c.Rip = base + 0x1235;
        assert_eq!(
            classify_single_step(ProbeMode::Governor, &empty, &armed, base + 0x1234, Some(&c)),
            DispatchOutcome::Unrelated
        );
        let mut returns = ReturnCorrelation::default();
        returns.enter(EntryInput {
            id: 2,
            entry: EntryKind::CanEver,
            return_address: base + 0x2500,
            expected_rsp: 0x1008,
            this_pointer: 0x1234,
            governor_index: 7,
            direct_call_verified: true,
            return_site_valid: true,
        });
        c.Rip = base + 0x2500;
        c.Dr2 = c.Rip;
        c.Dr7 = 0x10;
        let armed_return = [Some(ever), Some(appoint), Some(c.Rip), None];
        assert_eq!(
            classify_single_step(
                ProbeMode::Governor,
                &returns,
                &armed_return,
                c.Rip,
                Some(&c)
            ),
            DispatchOutcome::OwnedProven(OwnedTrap::Return)
        );
    }
    #[test]
    fn property_slots_require_matching_event_rip_register_and_enable_bit() {
        let base = 0x1a49b0000;
        let sites = ProbeMode::Property.fixed_sites(base);
        let empty = ReturnCorrelation::default();
        let mut c: CONTEXT = unsafe { zeroed() };
        c.Dr0 = sites[0].unwrap();
        c.Dr1 = sites[1].unwrap();
        c.Dr2 = sites[2].unwrap();
        c.Dr3 = sites[3].unwrap();
        c.Dr7 = 0x55;
        c.Dr6 = 0;
        for (slot, trap) in [
            OwnedTrap::PlayerGetter,
            OwnedTrap::PropertyBefore,
            OwnedTrap::PropertyAfter,
            OwnedTrap::PropertyName,
        ]
        .into_iter()
        .enumerate()
        {
            let address = sites[slot].unwrap();
            c.Rip = address;
            assert_eq!(
                classify_single_step(ProbeMode::Property, &empty, &sites, address, Some(&c)),
                DispatchOutcome::OwnedProven(trap)
            );
            c.Rip = address + 1;
            assert_eq!(
                classify_single_step(ProbeMode::Property, &empty, &sites, address, Some(&c)),
                DispatchOutcome::Ambiguous("exception_address_rip_disagree")
            );
            c.Rip = address;
            c.Dr7 &= !(1u64 << (slot * 2));
            assert_eq!(
                classify_single_step(ProbeMode::Property, &empty, &sites, address, Some(&c)),
                DispatchOutcome::Ambiguous("armed_debug_registers_disagree")
            );
            c.Dr7 = 0x55 | (1u64 << (slot * 2 + 1));
            assert_eq!(
                classify_single_step(ProbeMode::Property, &empty, &sites, address, Some(&c)),
                DispatchOutcome::Ambiguous("armed_debug_registers_disagree")
            );
            c.Dr7 = 0x55 | (1u64 << (16 + slot * 4));
            assert_eq!(
                classify_single_step(ProbeMode::Property, &empty, &sites, address, Some(&c)),
                DispatchOutcome::Ambiguous("armed_debug_registers_disagree")
            );
            c.Dr7 = 0x55;
        }
        c.Rip = base + 0x1234;
        assert_eq!(
            classify_single_step(ProbeMode::Property, &empty, &sites, c.Rip, Some(&c)),
            DispatchOutcome::Unrelated
        );
        assert_eq!(
            classify_single_step(ProbeMode::Property, &empty, &sites, sites[0].unwrap(), None),
            DispatchOutcome::Ambiguous("context_unavailable")
        );
    }
    #[test]
    fn application_breakpoint_before_attach_never_consumes_ready() {
        let verified = ThreadOriginObservation {
            rip: Some(0x1001),
            win32_start: Some(0x2000),
        };
        let mut origin = AttachOrigin::new(0x1000, 0x2000);
        origin.note_thread_start(11, Some(0x3000));
        assert_eq!(
            origin.classify(11, 0x1000, true, verified),
            BreakpointOrigin::Application
        );
        assert_eq!(
            origin.classify(11, 0x4000, true, verified),
            BreakpointOrigin::Application
        );
        assert!(!origin.accepted);
        origin.note_thread_start(22, Some(0x2000));
        assert_eq!(
            origin.classify(11, 0x1000, true, verified),
            BreakpointOrigin::Application
        );
        assert_eq!(
            origin.classify(22, 0x1000, false, verified),
            BreakpointOrigin::Application
        );
        assert_eq!(
            origin.classify(22, 0x1000, true, ThreadOriginObservation::default()),
            BreakpointOrigin::Application
        );
        assert_eq!(
            origin.classify(
                22,
                0x1000,
                true,
                ThreadOriginObservation {
                    win32_start: Some(0x3000),
                    ..verified
                }
            ),
            BreakpointOrigin::Application
        );
        assert_eq!(
            origin.classify(
                22,
                0x1000,
                true,
                ThreadOriginObservation {
                    rip: Some(0x1002),
                    ..verified
                }
            ),
            BreakpointOrigin::Application
        );
        assert!(!origin.accepted);
        assert_eq!(
            origin.classify(22, 0x1000, true, verified),
            BreakpointOrigin::SystemAttach
        );
        assert_eq!(
            origin.classify(22, 0x1000, true, verified),
            BreakpointOrigin::Application
        );
        let mut ambiguous = AttachOrigin::new(0x1000, 0x2000);
        ambiguous.note_thread_start(22, Some(0x2000));
        ambiguous.note_thread_start(33, Some(0x2000));
        assert_eq!(
            ambiguous.classify(22, 0x1000, true, verified),
            BreakpointOrigin::Application
        );
        let mut exited = AttachOrigin::new(0x1000, 0x2000);
        exited.note_thread_start(22, Some(0x2000));
        exited.note_thread_exit(22);
        assert_eq!(
            exited.classify(22, 0x1000, true, verified),
            BreakpointOrigin::Application
        );
        let mut instruction_start_rip = AttachOrigin::new(0x1000, 0x2000);
        instruction_start_rip.note_thread_start(22, Some(0x2000));
        assert_eq!(
            instruction_start_rip.classify(
                22,
                0x1000,
                true,
                ThreadOriginObservation {
                    rip: Some(0x1000),
                    ..verified
                }
            ),
            BreakpointOrigin::Application
        );
        assert!(!instruction_start_rip.accepted);
        let mut overflowed_rip = AttachOrigin::new(u64::MAX, 0x2000);
        overflowed_rip.note_thread_start(22, Some(0x2000));
        assert_eq!(
            overflowed_rip.classify(
                22,
                u64::MAX,
                true,
                ThreadOriginObservation {
                    rip: None,
                    ..verified
                }
            ),
            BreakpointOrigin::Application
        );
        assert!(!overflowed_rip.accepted);
    }
    #[test]
    fn unavailable_or_malformed_win32_start_query_never_supplies_provenance() {
        unsafe extern "system" fn unavailable(
            _: HANDLE,
            class: i32,
            out: *mut std::ffi::c_void,
            _: u32,
            returned: *mut u32,
        ) -> i32 {
            assert_eq!(class, ThreadQuerySetWin32StartAddress);
            unsafe {
                *(out as *mut usize) = 0x2000;
                *returned = size_of::<usize>() as u32;
            }
            -1
        }
        unsafe extern "system" fn malformed(
            _: HANDLE,
            _: i32,
            out: *mut std::ffi::c_void,
            _: u32,
            returned: *mut u32,
        ) -> i32 {
            unsafe {
                *(out as *mut usize) = 0x2000;
                *returned = 0;
            }
            0
        }
        let tid = unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() };
        let handle = Handle::new(
            unsafe { OpenThread(THREAD_QUERY_INFORMATION, 0, tid) },
            "OpenThread(test)",
        )
        .unwrap();
        let failed = query_win32_start(&handle, unavailable);
        assert_eq!(failed.status, -1);
        assert_eq!(failed.address, None);
        let short = query_win32_start(&handle, malformed);
        assert_eq!(short.returned_len, 0);
        assert_eq!(short.address, None);
    }
    #[test]
    fn fixture_parent_pid_is_the_spawning_test_process() {
        if isolated_test("fixture_parent_pid_is_the_spawning_test_process") {
            return;
        }
        let child = Fixture::start();
        assert_eq!(parent_pid(child.child.id()).unwrap(), std::process::id());
    }
    #[test]
    fn ntdll_origin_records_disk_pe_and_target_module_evidence() {
        if isolated_test("ntdll_origin_records_disk_pe_and_target_module_evidence") {
            return;
        }
        let child = Fixture::start();
        let (origin, _, evidence) = resolve_attach_origin(child.child.id()).unwrap();
        let remote = module_named(child.child.id(), "ntdll.dll").unwrap();
        let (disk_len, timestamp, image_size) = pe_image_metadata(&evidence.path).unwrap();
        assert!(same_path(&remote.path, &evidence.path));
        assert_eq!(evidence.remote_base, remote.base);
        assert_eq!(evidence.image_size, image_size);
        assert_eq!(remote.size, u64::from(image_size));
        assert_eq!(evidence.disk_len, disk_len);
        assert_eq!(evidence.pe_timestamp, timestamp);
        assert_eq!(evidence.disk_sha256.len(), 64);
        assert_eq!(evidence.disk_sha256, sha256(&evidence.path).unwrap());
        assert_eq!(
            origin.breakpoint_address,
            remote.base + evidence.breakpoint_rva
        );
        assert_eq!(origin.breakin_start, remote.base + evidence.breakin_rva);
        assert!(evidence.query_rva < u64::from(image_size));
    }
    #[test]
    fn permanent_context_refusal_reports_critical_until_controller_closes_child() {
        if isolated_test("permanent_context_refusal_reports_critical_until_controller_closes_child")
        {
            return;
        }
        let mut child = Fixture::start();
        let pid = child.child.id();
        let path = std::env::temp_dir().join(format!(
            "pr86-critical-test-{}-{pid}.jsonl",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let output = Jsonl::new(&path).unwrap();
        let base = child.entry - CAN_EVER_RVA;
        let signal = Arc::new(AtomicBool::new(false));
        let observer_signal = Arc::clone(&signal);
        let (ack_tx, ack_rx) = mpsc::channel();
        let (stop_tx, stop_rx) = mpsc::channel();
        let observer = child.observe(&stop_tx, move |close_ack| {
            let game = process(pid, true).unwrap();
            let mut debugger =
                Debugger::attach(pid, game, base, 0x500000, output, ProbeMode::Governor).unwrap();
            debugger.close_ack = close_ack;
            debugger.restore_faults = usize::MAX;
            debugger.critical_signal = Some(observer_signal);
            ack_tx.send(Arc::clone(&debugger.close_ack)).unwrap();
            let mut gate = CleanupGate::new();
            gate.deadline = Instant::now() + Duration::from_millis(250);
            debugger.cleanup_gate = Some(gate);
            let result = debugger.run(stop_rx, Duration::from_secs(10), || Ok(()));
            (
                result,
                debugger.attached,
                debugger.critical,
                debugger.terminal,
                debugger.lifecycle.exit_code,
            )
        });
        let close_ack = ack_rx.recv().unwrap();
        let start = Instant::now();
        while !fs::read_to_string(&path).is_ok_and(|s| s.contains("\"type\":\"ready\"")) {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "fixture observer did not become ready"
            );
            thread::sleep(Duration::from_millis(20));
        }
        stop_tx.send(()).unwrap();
        while !signal.load(Ordering::SeqCst) {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "critical cleanup was not reported"
            );
            thread::sleep(Duration::from_millis(20));
        }
        child.child.kill().unwrap(); // Only this case's synthetic child.
        close_ack.store(true, Ordering::SeqCst);
        let child_status = child.wait_for_exit();
        let (result, attached, critical, terminal, exit_code) = join_observer(observer).unwrap();
        assert!(result
            .unwrap_err()
            .contains("controller closed the owned game"));
        assert!(critical);
        assert!(!attached);
        assert_eq!(terminal, TerminalKind::TargetExited);
        assert_eq!(exit_code, child_status.code().map(|code| code as u32));
        let trace = fs::read_to_string(&path).unwrap();
        assert!(trace.contains("injected SetThreadContext refusal"));
        assert!(trace.contains("\"type\":\"target_exit_observed\""));
        drop(child);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn pending_restore_refusal_requires_owned_close_ack() {
        if isolated_test("pending_restore_refusal_requires_owned_close_ack") {
            return;
        }
        let mut child = Fixture::start();
        let pid = child.child.id();
        let path = std::env::temp_dir().join(format!(
            "pr86-pending-refusal-{}-{pid}.jsonl",
            std::process::id()
        ));
        let output = Jsonl::new(&path).unwrap();
        let base = child.entry - CAN_EVER_RVA;
        let signal = Arc::new(AtomicBool::new(false));
        let observer_signal = Arc::clone(&signal);
        let (ack_tx, ack_rx) = mpsc::channel();
        let (_stop_tx, stop_rx) = mpsc::channel();
        let observer = thread::spawn(move || {
            let mut debugger = Debugger::attach(
                pid,
                process(pid, true).unwrap(),
                base,
                0x500000,
                output,
                ProbeMode::Governor,
            )
            .unwrap();
            debugger.panic_on_owned_event = true;
            debugger.freeze_refused = true;
            debugger.critical_signal = Some(observer_signal);
            ack_tx.send(Arc::clone(&debugger.close_ack)).unwrap();
            let mut gate = CleanupGate::new();
            gate.deadline = Instant::now() + Duration::from_millis(500);
            debugger.cleanup_gate = Some(gate);
            debugger.run(stop_rx, Duration::from_secs(10), || Ok(()))
        });
        let close_ack = ack_rx.recv().unwrap();
        let started = Instant::now();
        while !fs::read_to_string(&path).is_ok_and(|s| s.contains("\"type\":\"ready\"")) {
            assert!(started.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(10));
        }
        child.send("T").unwrap();
        while !signal.load(Ordering::SeqCst) {
            assert!(started.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(10));
        }
        assert!(child.child.try_wait().unwrap().is_none());
        // Even the original call may not complete while registers are unverified.
        let response = child.command_with_budget("T", "PROBE_OUT:", Duration::from_millis(200));
        assert!(response.unwrap_err().contains("did not answer"));
        assert!(child.child.try_wait().unwrap().is_none());
        child.child.kill().unwrap();
        close_ack.store(true, Ordering::SeqCst);
        child.wait_for_exit();
        assert!(
            join_observer(observer).is_err(),
            "injected panic became success"
        );
        let trace = fs::read_to_string(&path).unwrap();
        assert!(trace.contains("\"type\":\"critical_cleanup\""));
        assert!(trace.contains("\"type\":\"target_exit_observed\""));
        drop(child);
        fs::remove_file(path).unwrap();
    }
}
