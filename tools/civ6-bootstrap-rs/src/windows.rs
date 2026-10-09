use super::*;
use sha2::{Digest, Sha256};
use std::{
    ffi::c_void,
    fs::{File, OpenOptions},
    io::Read,
    mem::size_of,
    os::windows::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Mutex,
    },
};
use windows_sys::Win32::{
    Foundation::*,
    System::{
        Diagnostics::Debug::{FlushInstructionCache, ReadProcessMemory},
        LibraryLoader::*,
        Memory::*,
        Threading::*,
    },
};
#[path = "hotpatch.rs"]
mod hotpatch;
const GAME_ROOT: &str = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI";
const RELEASE: usize = 0xf7110;
const SET: usize = 0xf7830;
const LOADER: usize = 0xf8510;
const HOLDER: usize = 0x22d6070;
const CONTEXT: usize = 0x22d60c0;
type Api = unsafe extern "system" fn(*mut c_void) -> u32;

#[unsafe(no_mangle)]
static BOOT_RELEASE_CONTINUE: AtomicUsize = AtomicUsize::new(0);
#[unsafe(no_mangle)]
static BOOT_SET_CONTINUE: AtomicUsize = AtomicUsize::new(0);
#[unsafe(no_mangle)]
static BOOT_LOADER_CONTINUE: AtomicUsize = AtomicUsize::new(0);
static STATE: Mutex<Option<Resident>> = Mutex::new(None);
static CALLS: AtomicU64 = AtomicU64::new(0);
static LAST_STATUS: AtomicUsize = AtomicUsize::new(0);
static OBSERVATION: crate::observation::ObservationGate =
    crate::observation::ObservationGate::new(true);
struct RecordFlight;
impl RecordFlight {
    fn try_enter() -> Option<Self> {
        OBSERVATION.try_acquire().then(|| Self)
    }
}
impl Drop for RecordFlight {
    fn drop(&mut self) {
        OBSERVATION.release();
    }
}
#[repr(C)]
struct TraceRow {
    sequence: AtomicU64,
    kind: AtomicU64,
    tid: AtomicU64,
    receiver: AtomicU64,
    old: AtomicU64,
    incoming: AtomicU64,
    caller: AtomicU64,
    generation: AtomicU64,
    status: AtomicU64,
    calibration_next: AtomicU64,
    calibration_in_flight: AtomicU64,
    property_next: AtomicU64,
    property_in_flight: AtomicU64,
}
#[repr(C)]
pub struct Trace {
    version: u64,
    capacity: u64,
    pid: AtomicU64,
    filetime: AtomicU64,
    core: AtomicU64,
    runtime_trace: AtomicU64,
    runtime_trace_size: AtomicU64,
    reserved: AtomicU64,
    dropped: AtomicU64,
    rows: [TraceRow; 128],
}
static TRACE: Trace = Trace {
    version: 2,
    capacity: 128,
    pid: AtomicU64::new(0),
    filetime: AtomicU64::new(0),
    core: AtomicU64::new(0),
    runtime_trace: AtomicU64::new(0),
    runtime_trace_size: AtomicU64::new(0),
    reserved: AtomicU64::new(0),
    dropped: AtomicU64::new(0),
    rows: [const {
        TraceRow {
            sequence: AtomicU64::new(0),
            kind: AtomicU64::new(0),
            tid: AtomicU64::new(0),
            receiver: AtomicU64::new(0),
            old: AtomicU64::new(0),
            incoming: AtomicU64::new(0),
            caller: AtomicU64::new(0),
            generation: AtomicU64::new(0),
            status: AtomicU64::new(0),
            calibration_next: AtomicU64::new(u64::MAX),
            calibration_in_flight: AtomicU64::new(u64::MAX),
            property_next: AtomicU64::new(u64::MAX),
            property_in_flight: AtomicU64::new(u64::MAX),
        }
    }; 128],
};
#[unsafe(no_mangle)]
pub static BOOTSTRAP_TRACE_POINTER: &Trace = &TRACE;
fn record(
    kind: u64,
    receiver: usize,
    old: usize,
    incoming: usize,
    caller: u64,
    generation: u64,
    status: u32,
) {
    let Some(_flight) = RecordFlight::try_enter() else {
        return;
    };
    let seq = TRACE.reserved.fetch_add(1, Ordering::AcqRel);
    let Some(row) = TRACE.rows.get(seq as usize) else {
        TRACE.dropped.fetch_add(1, Ordering::Relaxed);
        return;
    };
    row.kind.store(kind, Ordering::Relaxed);
    row.tid
        .store(unsafe { GetCurrentThreadId() } as u64, Ordering::Relaxed);
    row.receiver.store(receiver as u64, Ordering::Relaxed);
    row.old.store(old as u64, Ordering::Relaxed);
    row.incoming.store(incoming as u64, Ordering::Relaxed);
    row.caller.store(caller, Ordering::Relaxed);
    row.generation.store(generation, Ordering::Relaxed);
    row.status.store(status as u64, Ordering::Relaxed);
    let counts = native_counts();
    row.calibration_next.store(counts[0], Ordering::Relaxed);
    row.calibration_in_flight
        .store(counts[1], Ordering::Relaxed);
    row.property_next.store(counts[2], Ordering::Relaxed);
    row.property_in_flight.store(counts[3], Ordering::Relaxed);
    row.sequence.store(seq + 1, Ordering::Release);
}
fn native_counts() -> [u64; 4] {
    // The calibration version fixes the PropertyTrace prefix offset. The
    // runtime marks that prefix external and writes every property
    // row to its separate log, so the property counts come from the log.
    // These read-only checkpoints are NOT an authorization/publication fence.
    use protocol::*;
    let base = TRACE.runtime_trace.load(Ordering::Acquire) as usize;
    if base == 0 || TRACE.runtime_trace_size.load(Ordering::Acquire) < 20 {
        return [u64::MAX; 4];
    }
    let Ok(header) = read(base, 20) else {
        return [u64::MAX; 4];
    };
    let Ok((_, _, property_offset)) = calibration_layout(&header) else {
        return [u64::MAX; 4];
    };
    if TRACE.runtime_trace_size.load(Ordering::Acquire) < (property_offset + 32) as u64 {
        return [u64::MAX; 4];
    }
    let u = |b: &[u8], n: usize| u32::from_le_bytes(b[n..n + 4].try_into().unwrap());
    let property = || -> Option<[u64; 2]> {
        let prefix = read(base.checked_add(property_offset)?, 32).ok()?;
        if u(&prefix, 0) != 0x4a414350
            || u(&prefix, 4) != PROPERTY_EXTERNAL_VERSION
            || u(&prefix, 28) != PROPERTY_LOG_MAGIC
        {
            return None;
        }
        let log = ptr_at(LOG_EXPORT.load(Ordering::Acquire)).ok()?;
        let b = read(log, PROPERTY_LOG_HEADER).ok()?;
        let w = |n: usize| u64::from_le_bytes(b[n..n + 8].try_into().unwrap());
        (log != 0 && u(&b, 0) == PROPERTY_LOG_MAGIC && u(&b, 4) == PROPERTY_LOG_VERSION)
            .then(|| [w(PROPERTY_LOG_NEXT), w(PROPERTY_LOG_IN_FLIGHT)])
    };
    match property() {
        Some(b) => [u(&header, 8) as u64, u(&header, 16) as u64, b[0], b[1]],
        None => [u64::MAX; 4],
    }
}
/// Address of the pinned runtime's `BRIDGE_RUNTIME_PROPERTY_LOG` data export.
static LOG_EXPORT: AtomicUsize = AtomicUsize::new(0);

core::arch::global_asm!(
    ".text",
    ".globl boot_release_original",
    ".seh_proc boot_release_original",
    "boot_release_original:",
    "push rbx",
    ".seh_pushreg rbx",
    ".seh_endprologue",
    "jmp qword ptr [rip + BOOT_RELEASE_CONTINUE]",
    ".seh_endproc",
    ".globl boot_set_original",
    ".seh_proc boot_set_original",
    "boot_set_original:",
    "mov [rsp + 8], rbx",
    ".seh_savereg rbx, 8",
    ".seh_endprologue",
    "jmp qword ptr [rip + BOOT_SET_CONTINUE]",
    ".seh_endproc",
    ".globl boot_loader_original",
    ".seh_proc boot_loader_original",
    "boot_loader_original:",
    "sub rsp,0x248",
    ".seh_stackalloc 0x248",
    ".seh_endprologue",
    "jmp qword ptr [rip + BOOT_LOADER_CONTINUE]",
    ".seh_endproc",
);
// Native callees are not proven noexcept; their unwind may cross this wrapper.
extern "system-unwind" {
    fn boot_release_original(a: usize, b: usize) -> usize;
    fn boot_set_original(a: usize, b: usize) -> usize;
    fn boot_loader_original(a: usize, b: usize) -> usize;
}

struct Resident {
    exe: usize,
    core: usize,
    install: Api,
    stop: Api,
    close: Api,
    rebuild: Api,
    phase: Phase,
    generation: u64,
    close_tid: u32,
    config: RuntimeConfig,
    _source: File,
    // Retained even after partial publication. Parent must close its owned
    // process on a Prepare error after Mode6 became resident; no reattempt.
    _hooks: Vec<hotpatch::Hook>,
}

fn read(address: usize, n: usize) -> Result<Vec<u8>, String> {
    let mut bytes = vec![0; n];
    let mut got = 0;
    if unsafe {
        ReadProcessMemory(
            GetCurrentProcess(),
            address as _,
            bytes.as_mut_ptr().cast(),
            n,
            &mut got,
        )
    } == 0
        || got != n
    {
        return Err("unreadable in-process range".into());
    }
    Ok(bytes)
}
fn ptr_at(address: usize) -> Result<usize, String> {
    Ok(u64::from_le_bytes(read(address, 8)?.try_into().unwrap()) as usize)
}
fn path(module: usize) -> Result<PathBuf, String> {
    let mut b = vec![0u16; 32768];
    let n = unsafe { GetModuleFileNameW(module as _, b.as_mut_ptr(), b.len() as u32) } as usize;
    if n == 0 || n >= b.len() {
        return Err("module path unavailable".into());
    }
    Ok(PathBuf::from(
        String::from_utf16(&b[..n]).map_err(|_| "module path encoding")?,
    ))
}
fn same(a: &Path, b: &Path) -> bool {
    a.to_string_lossy()
        .replace('\\', "/")
        .eq_ignore_ascii_case(&b.to_string_lossy().replace('\\', "/"))
}
fn pin_file(p: &Path, sha: &[u8; 32]) -> Result<File, String> {
    let mut f = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(p)
        .map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut b = [0u8; 65536];
    loop {
        let n = f.read(&mut b).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&b[..n]);
    }
    if hash.finalize().as_slice() != sha {
        return Err("source SHA differs".into());
    }
    Ok(f)
}
fn fixed_hash(p: &Path, expected: &str) -> Result<File, String> {
    let mut sha = [0u8; 32];
    for (i, v) in sha.iter_mut().enumerate() {
        *v = u8::from_str_radix(&expected[i * 2..i * 2 + 2], 16).map_err(|_| "fixed SHA")?;
    }
    pin_file(p, &sha)
}
fn created() -> u64 {
    let (mut a, mut b, mut c, mut d) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    if unsafe { GetProcessTimes(GetCurrentProcess(), &mut a, &mut b, &mut c, &mut d) } == 0 {
        return 0;
    }
    ((a.dwHighDateTime as u64) << 32) | u64::from(a.dwLowDateTime)
}
fn module(name: &str) -> usize {
    let w: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    unsafe { GetModuleHandleW(w.as_ptr()) as usize }
}
fn api(module: usize, name: &[u8]) -> Result<Api, String> {
    let f = unsafe { GetProcAddress(module as _, name.as_ptr()) }.ok_or("runtime export absent")?;
    let mut m = MEMORY_BASIC_INFORMATION::default();
    if unsafe {
        VirtualQuery(
            f as *const () as _,
            &mut m,
            size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    } == 0
        || m.AllocationBase as usize != module
        || m.State != MEM_COMMIT
        || m.Protect & PAGE_GUARD != 0
        || !matches!(
            m.Protect & 0xff,
            PAGE_EXECUTE | PAGE_EXECUTE_READ | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY
        )
    {
        return Err("runtime export outside executable source module".into());
    }
    Ok(unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, Api>(f) })
}
/// A pointer-sized data export of `module`; its current value is read later.
fn data(module: usize, name: &[u8]) -> Result<usize, String> {
    let p = unsafe { GetProcAddress(module as _, name.as_ptr()) }.ok_or("runtime export absent")?
        as usize;
    let mut m = MEMORY_BASIC_INFORMATION::default();
    if unsafe { VirtualQuery(p as _, &mut m, size_of::<MEMORY_BASIC_INFORMATION>()) } == 0
        || m.AllocationBase as usize != module
        || m.State != MEM_COMMIT
        || !p.is_multiple_of(8)
        || p + 8 > m.BaseAddress as usize + m.RegionSize
        || read(p, 8).is_err()
    {
        return Err("runtime data export outside readable source module".into());
    }
    Ok(p)
}
fn typed(core: usize, p: usize) -> bool {
    p != 0
        && ptr_at(p).ok() == Some(core + 0xa101e8)
        && ptr_at(core + 0xa101f0).ok() == Some(core + 0x8972d0)
        && ptr_at(core + 0xa10388).ok() == Some(core + 0x58ff70)
}
fn caller(exe: usize, ret: usize, entry: usize) -> Result<u64, String> {
    let rva = ret.checked_sub(exe).ok_or("foreign caller")?;
    let bytes = read(ret.checked_sub(5).ok_or("caller underflow")?, 5)?;
    let target =
        ret.checked_add_signed(i32::from_le_bytes(bytes[1..5].try_into().unwrap()) as isize);
    if bytes[0] != 0xe8 || target != Some(exe + entry) {
        return Err("caller is not the fixed direct call".into());
    }
    Ok((rva - 5) as u64)
}

fn event(
    receiver: usize,
    incoming: usize,
    ret: usize,
    is_set: bool,
) -> Result<Option<u64>, String> {
    let mut lock = STATE.lock().map_err(|_| "resident mutex poisoned")?;
    let s = lock.as_mut().ok_or("resident not prepared")?;
    if s.phase == Phase::Prepared {
        // Menu BaseContext has no XP2 runtime authority. Original menu
        // transitions run untouched until the target loader returns.
        record(15, receiver, usize::MAX, incoming, ret as u64, 0, 0);
        return Ok(None);
    }
    if module("GameCore_XP2_FinalRelease.dll") != s.core {
        return Err("pinned Core base changed".into());
    }
    if ptr_at(s.exe + HOLDER)? != receiver {
        return Err("holder differs from fixed singleton".into());
    }
    let from = caller(s.exe, ret, if is_set { SET } else { RELEASE })?;
    let old = ptr_at(receiver.checked_add(0x450).ok_or("holder overflow")?)?;
    if is_set && old == incoming {
        record(5, receiver, old, incoming, from, s.generation, 0);
        CALLS.fetch_add(1, Ordering::Relaxed);
        return Ok(None);
    }
    if old != 0 && !typed(s.core, old) {
        return Err("old context type differs".into());
    }
    if is_set && incoming != 0 && (!typed(s.core, incoming) || ptr_at(s.exe + CONTEXT)? != incoming)
    {
        return Err("new context differs from factory global".into());
    }
    let mut close_token = None;
    if old != 0 && s.phase == Phase::Open {
        record(2, receiver, old, incoming, from, s.generation, 0);
        let status = unsafe { (s.close)(std::ptr::null_mut()) };
        LAST_STATUS.store(status as usize, Ordering::Release);
        record(3, receiver, old, incoming, from, s.generation, status);
        if status != 0 {
            return Err("runtime Close failed".into());
        }
        s.phase = if is_set {
            Phase::UnsupportedInlineExchange
        } else {
            Phase::Closing
        };
        s.generation = s.generation.checked_add(1).ok_or("generation exhausted")?;
        s.close_tid = unsafe { GetCurrentThreadId() };
        if !is_set {
            close_token = Some(s.generation);
        }
    }
    if is_set && s.phase == Phase::Closed && incoming != 0 {
        // Rebuild here would erase natural RESET/COMMIT already issued by the
        // factory and Context+60. Only the earlier verified loader may reopen.
        return Err("SetContext reached before loader Rebuild".into());
    }
    record(
        if is_set { 8 } else { 4 },
        receiver,
        old,
        incoming,
        from,
        s.generation,
        0,
    );
    CALLS.fetch_add(1, Ordering::Release);
    Ok(close_token)
}

fn guarded_event(receiver: usize, incoming: usize, ret: usize, is_set: bool) -> Option<u64> {
    match std::panic::catch_unwind(|| event(receiver, incoming, ret, is_set)) {
        Ok(Ok(token)) => token,
        _ => {
            // An unproved Close must never let destructive native code proceed
            // with old grants. This is a critical owned-process failure, not a
            // successful detach. The driver retains its ownership/reap handle.
            LAST_STATUS.store(usize::MAX, Ordering::Release);
            std::process::abort();
        }
    }
}
extern "system-unwind" fn release_entry(receiver: usize, unused: usize, ret: usize) -> usize {
    let error = unsafe { GetLastError() };
    let token = guarded_event(receiver, 0, ret, false);
    unsafe { SetLastError(error) };
    let result = unsafe { boot_release_original(receiver, unused) };
    let error = unsafe { GetLastError() };
    if let Some(generation) = token {
        let mut lock = STATE.lock().unwrap_or_else(|_| std::process::abort());
        let state = lock.as_mut().unwrap_or_else(|| std::process::abort());
        if state.phase != Phase::Closing || state.generation != generation {
            std::process::abort();
        }
        state.phase = Phase::Closed;
    }
    record(9, receiver, 0, 0, ret as u64, token.unwrap_or(u64::MAX), 0);
    unsafe { SetLastError(error) };
    result
}
extern "system-unwind" fn set_entry(receiver: usize, incoming: usize, ret: usize) -> usize {
    let error = unsafe { GetLastError() };
    let _ = guarded_event(receiver, incoming, ret, true);
    unsafe { SetLastError(error) };
    let result = unsafe { boot_set_original(receiver, incoming) };
    let error = unsafe { GetLastError() };
    record(10, receiver, 0, incoming, ret as u64, u64::MAX, 0);
    unsafe { SetLastError(error) };
    result
}

fn target_loader_return(result: usize, ret: usize) -> Result<(), String> {
    let mut lock = STATE.lock().map_err(|_| "resident lock")?;
    let s = lock.as_mut().ok_or("resident absent")?;
    if !matches!(s.phase, Phase::Prepared | Phase::Closed) || result & 0xff == 0 {
        return Ok(());
    }
    let core = module("GameCore_XP2_FinalRelease.dll");
    if core == 0 {
        return Ok(());
    } // A Base/other loader is not target activation.
    let action = loader_action(s.phase, result, caller(s.exe, ret, LOADER).ok())?;
    if action == LoaderAction::Ignore {
        return Ok(());
    }
    let core_path = path(core)?;
    if !same(
        &core_path,
        &Path::new(GAME_ROOT).join("DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll"),
    ) {
        return Err("target Core path differs".into());
    }
    let _core_file = fixed_hash(&core_path, CORE_SHA)?;
    if ptr_at(s.exe + 0x22d60d8)? != core
        || ptr_at(s.exe + 0x22d64e8)? != core + 0x752d50
        || ptr_at(s.exe + CONTEXT)? != 0
        || read(core + 0x752d50, 5)? != [0xe9, 0x0b, 0xcf, 0xe3, 0xff]
    {
        return Err("target loader/factory-before-context boundary differs".into());
    }
    if action == LoaderAction::Rebuild {
        let holder = ptr_at(s.exe + HOLDER)?;
        let held_context = if holder == 0 {
            0
        } else {
            ptr_at(holder.checked_add(0x450).ok_or("holder overflow")?)?
        };
        let boundary = ReloadBoundary {
            loader_result: result,
            caller: 0x24b52d, // caller() above verified the actual return site.
            close_tid: s.close_tid,
            current_tid: unsafe { GetCurrentThreadId() },
            pinned_core: s.core,
            current_core: core,
            context: ptr_at(s.exe + CONTEXT)?,
            holder,
            held_context,
        };
        let generation = s.generation;
        let rebuild = s.rebuild;
        let status = rebuild_at_loader(&mut s.phase, boundary, || {
            record(6, holder, 0, 0, 0x24b52d, generation, 0);
            let status = unsafe { rebuild(std::ptr::null_mut()) };
            LAST_STATUS.store(status as usize, Ordering::Release);
            record(7, holder, 0, 0, 0x24b52d, generation, status);
            status
        })?
        .ok_or("reload did not execute Rebuild")?;
        if status != 0 {
            return Err("runtime Rebuild failed before factory".into());
        }
        return Ok(());
    }
    s.core = core;
    if let Some(_flight) = RecordFlight::try_enter() {
        TRACE.core.store(core as u64, Ordering::Release);
    }
    record(13, 0, 0, core, 0x24b52d, 0, 0);
    record(0, 0, 0, 0, 0, 0, 0);
    let status = unsafe { (s.install)((&mut s.config as *mut RuntimeConfig).cast()) };
    LAST_STATUS.store(status as usize, Ordering::Release);
    if let Some(_flight) = RecordFlight::try_enter() {
        TRACE
            .runtime_trace
            .store(s.config.trace_address, Ordering::Release);
        TRACE
            .runtime_trace_size
            .store(s.config.trace_size as u64, Ordering::Release);
    }
    record(1, 0, 0, 0, 0, 0, status);
    if status != 0 {
        return Err("deferred Mode6 Install failed before factory".into());
    }
    s.phase = Phase::Open;
    if OBSERVATION.is_closed() && unsafe { (s.stop)(std::ptr::null_mut()) } != 0 {
        return Err("deferred observation stop failed".into());
    }
    Ok(())
}
extern "system-unwind" fn loader_entry(argument: usize, unused: usize, ret: usize) -> usize {
    let result = unsafe { boot_loader_original(argument, unused) };
    let error = unsafe { GetLastError() };
    if !std::panic::catch_unwind(|| target_loader_return(result, ret)).is_ok_and(|r| r.is_ok()) {
        LAST_STATUS.store(usize::MAX, Ordering::Release);
        std::process::abort();
    }
    unsafe { SetLastError(error) };
    result
}

fn prepare(input: Prepare) -> Result<(), String> {
    if input.version != VERSION
        || input.size as usize != size_of::<Prepare>()
        || input.pid != unsafe { GetCurrentProcessId() }
        || input.created_filetime == 0
        || input.created_filetime != created()
        || !(1..=300).contains(&input.observation_seconds)
    {
        return Err("bootstrap config/identity differs".into());
    }
    let mut lock = STATE.lock().map_err(|_| "resident lock")?;
    if lock.is_some() {
        return Err("resident already prepared; no retry".into());
    }
    let exe = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
    let exe_path = path(exe)?;
    if !same(
        &exe_path,
        &Path::new(GAME_ROOT).join("Base/Binaries/Win64Steam/CivilizationVI.exe"),
    ) {
        return Err("not pinned game EXE".into());
    }
    let _exe_pin = fixed_hash(&exe_path, EXE_SHA)?;
    if module("GameCore_XP2_FinalRelease.dll") != 0 {
        return Err("cold preparation refuses an already-loaded target Core".into());
    }
    let menu_core = module("GameCore_Base_FinalRelease.dll");
    let core_path = path(menu_core)?;
    if menu_core == 0
        || !same(
            &core_path,
            &Path::new(GAME_ROOT).join("Base/Binaries/Win64Steam/GameCore_Base_FinalRelease.dll"),
        )
    {
        return Err("Core path differs".into());
    }
    let _core_file = fixed_hash(&core_path, MENU_CORE_SHA)?;
    if input.expected_menu_context == 0
        || ptr_at(exe + CONTEXT)? != input.expected_menu_context as usize
        || ptr_at(input.expected_menu_context as usize)? != menu_core + 0x79e020
        || ptr_at(exe + 0x22d64e8)? != menu_core + 0x58dd30
    {
        return Err("MainMenu context identity differs".into());
    }
    let bridge = input.bridge_module as usize;
    let source = pin_file(&path(bridge)?, &input.bridge_sha256)?;
    let install = api(bridge, b"BridgeRuntimeInstall\0")?;
    let close = api(bridge, b"BridgeRuntimeClose\0")?;
    let rebuild = api(bridge, b"BridgeRuntimeRebuild\0")?;
    let stop = api(bridge, b"BridgeRuntimeStopObservation\0")?;
    let log_export = data(bridge, b"BRIDGE_RUNTIME_PROPERTY_LOG\0")?;
    // Own the lifetime of saved API addresses before any runtime invocation.
    // Mode6 also pins itself, but a file handle alone is not a module reference.
    let mut runtime_module = std::ptr::null_mut();
    if unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            install as *const () as _,
            &mut runtime_module,
        )
    } == 0
        || runtime_module as usize != bridge
    {
        return Err("runtime module lifetime could not be retained".into());
    }
    let mut pinned = std::ptr::null_mut();
    if unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            RustBridgeBootstrapPrepare as *const () as _,
            &mut pinned,
        )
    } == 0
    {
        return Err("bootstrap pin failed".into());
    }
    BOOT_RELEASE_CONTINUE.store(exe + RELEASE + 2, Ordering::Release);
    BOOT_SET_CONTINUE.store(exe + SET + 5, Ordering::Release);
    BOOT_LOADER_CONTINUE.store(exe + LOADER + 7, Ordering::Release);
    let a = hotpatch::prepare(
        exe + RELEASE,
        &[0x40, 0x53, 0x48, 0x83, 0xec, 0x20],
        release_entry as *const () as usize,
    )?;
    let b = hotpatch::prepare(
        exe + SET,
        &[0x48, 0x89, 0x5c, 0x24, 8, 0x57, 0x48, 0x83, 0xec, 0x20],
        set_entry as *const () as usize,
    )?;
    let c = hotpatch::prepare(
        exe + LOADER,
        &[0x48, 0x81, 0xec, 0x48, 0x02, 0, 0],
        loader_entry as *const () as usize,
    )?;
    let config = RuntimeConfig {
        version: 0x03240002,
        size: 40,
        pid: input.pid,
        max_seconds: input.observation_seconds,
        created_filetime: input.created_filetime,
        trace_address: 0,
        trace_size: 0,
        mode: 6,
    };
    // Publish retained responsibility before the first runtime mutation.
    *lock = Some(Resident {
        exe,
        core: 0,
        install,
        stop,
        close,
        rebuild,
        phase: Phase::Prepared,
        generation: 0,
        close_tid: 0,
        config,
        _source: source,
        _hooks: vec![a, b, c],
    });
    LOG_EXPORT.store(log_export, Ordering::Release);
    if let Some(_flight) = RecordFlight::try_enter() {
        TRACE.pid.store(input.pid as u64, Ordering::Release);
        TRACE
            .filetime
            .store(input.created_filetime, Ordering::Release);
    }
    record(
        12,
        menu_core,
        input.expected_menu_context as usize,
        0,
        0,
        0,
        0,
    );
    let s = lock.as_mut().unwrap();
    for hook in &s._hooks {
        hook.publish()?;
    }
    record(11, 0, 0, 0, 0, 0, 0); // Prepared entries, not XP2/Mode6 readiness.
    Ok(())
}

/// Explicit call by the pinned owning driver in a fresh MainMenu. No DllMain
/// work, automatic DLL search or default enable flag exists.
#[unsafe(no_mangle)]
pub extern "system" fn RustBridgeBootstrapPrepare(input: *const Prepare) -> u32 {
    std::panic::catch_unwind(|| {
        let bytes = read(input as usize, size_of::<Prepare>()).map_err(|_| ())?;
        let config = unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast::<Prepare>()) };
        prepare(config).map_err(|_| ())
    })
    .ok()
    .and_then(Result::ok)
    .map_or(1, |_| 0)
}

/// Fixed POD snapshot: caller reads returned immutable-address atomics through
/// the existing owned controller, not borrowed JSON or an external mapping.
#[unsafe(no_mangle)]
pub extern "system" fn RustBridgeBootstrapCalls() -> u64 {
    CALLS.load(Ordering::Acquire)
}
#[unsafe(no_mangle)]
pub extern "system" fn RustBridgeBootstrapLastStatus() -> usize {
    LAST_STATUS.load(Ordering::Acquire)
}
#[unsafe(no_mangle)]
pub extern "system" fn RustBridgeBootstrapTrace() -> *const c_void {
    std::ptr::addr_of!(TRACE).cast()
}
#[unsafe(no_mangle)]
pub extern "system" fn RustBridgeBootstrapTraceSize() -> usize {
    size_of::<Trace>()
}
#[unsafe(no_mangle)]
pub extern "system" fn RustBridgeBootstrapStopObservation(_: *mut c_void) -> u32 {
    std::panic::catch_unwind(|| -> Result<(), ()> {
        OBSERVATION.close();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !OBSERVATION.drained() {
            if std::time::Instant::now() >= deadline {
                return Err(());
            }
            std::thread::yield_now();
        }
        let lock = STATE.lock().map_err(|_| ())?;
        let s = lock.as_ref().ok_or(())?;
        if s.phase != Phase::Prepared && unsafe { (s.stop)(std::ptr::null_mut()) } != 0 {
            return Err(());
        }
        Ok(())
    })
    .ok()
    .and_then(Result::ok)
    .map_or(1, |_| 0)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
