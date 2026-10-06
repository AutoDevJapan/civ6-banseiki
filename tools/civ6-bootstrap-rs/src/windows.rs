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
const HOLDER: usize = 0x22d6070;
const CONTEXT: usize = 0x22d60c0;
type Api = unsafe extern "system" fn(*mut c_void) -> u32;

#[unsafe(no_mangle)]
static BOOT_RELEASE_CONTINUE: AtomicUsize = AtomicUsize::new(0);
#[unsafe(no_mangle)]
static BOOT_SET_CONTINUE: AtomicUsize = AtomicUsize::new(0);
static STATE: Mutex<Option<Resident>> = Mutex::new(None);
static CALLS: AtomicU64 = AtomicU64::new(0);
static LAST_STATUS: AtomicUsize = AtomicUsize::new(0);
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
    version: 1,
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
    // Public D21 CompositeTrace begins with CalibrationTrace v4: a 24-byte
    // header and 1024 repr(C) 72-byte CalibrationRows, then PropertyTrace v3.
    // These read-only checkpoints are NOT an authorization/publication fence.
    const PROPERTY: usize = 24 + 1024 * 72;
    let base = TRACE.runtime_trace.load(Ordering::Acquire) as usize;
    if base == 0 || TRACE.runtime_trace_size.load(Ordering::Acquire) < (PROPERTY + 20) as u64 {
        return [u64::MAX; 4];
    }
    let sample = |offset: usize, magic: u32, version: u32| -> Option<[u64; 2]> {
        let b = read(base.checked_add(offset)?, 20).ok()?;
        let u = |n: usize| u32::from_le_bytes(b[n..n + 4].try_into().unwrap());
        (u(0) == magic && u(4) == version).then_some([u(8) as u64, u(16) as u64])
    };
    match (sample(0, 0x4a414341, 4), sample(PROPERTY, 0x4a414350, 3)) {
        (Some(a), Some(b)) => [a[0], a[1], b[0], b[1]],
        _ => [u64::MAX; 4],
    }
}

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
);
// Native callees are not proven noexcept; their unwind may cross this wrapper.
extern "system-unwind" {
    fn boot_release_original(a: usize, b: usize) -> usize;
    fn boot_set_original(a: usize, b: usize) -> usize;
}

struct Resident {
    exe: usize,
    core: usize,
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
    if is_set && rebuild_before_set(s.phase, old as u64, incoming as u64, from) {
        if s.close_tid != unsafe { GetCurrentThreadId() } {
            return Err("world transition thread changed".into());
        }
        record(6, receiver, old, incoming, from, s.generation, 0);
        let status = unsafe { (s.rebuild)(std::ptr::null_mut()) };
        LAST_STATUS.store(status as usize, Ordering::Release);
        record(7, receiver, old, incoming, from, s.generation, status);
        if status != 0 {
            return Err("runtime Rebuild failed".into());
        }
        s.phase = Phase::Open;
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
    let core = module("GameCore_XP2_FinalRelease.dll");
    let core_path = path(core)?;
    if core == 0
        || !same(
            &core_path,
            &Path::new(GAME_ROOT)
                .join("DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll"),
        )
    {
        return Err("Core path differs".into());
    }
    let _core_file = fixed_hash(&core_path, CORE_SHA)?;
    if ptr_at(exe + CONTEXT)? != input.expected_context as usize
        || !typed(core, input.expected_context as usize)
    {
        return Err("MainMenu context identity differs".into());
    }
    let bridge = input.bridge_module as usize;
    let source = pin_file(&path(bridge)?, &input.bridge_sha256)?;
    let install = api(bridge, b"BridgeRuntimeInstall\0")?;
    let close = api(bridge, b"BridgeRuntimeClose\0")?;
    let rebuild = api(bridge, b"BridgeRuntimeRebuild\0")?;
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
    let mut config = RuntimeConfig {
        version: 0x03240001,
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
        core,
        close,
        rebuild,
        phase: Phase::Prepared,
        generation: 0,
        close_tid: 0,
        config,
        _source: source,
        _hooks: vec![a, b],
    });
    TRACE.pid.store(input.pid as u64, Ordering::Release);
    TRACE
        .filetime
        .store(input.created_filetime, Ordering::Release);
    TRACE.core.store(core as u64, Ordering::Release);
    record(0, 0, 0, 0, 0, 0, 0);
    let status = unsafe { install((&mut config as *mut RuntimeConfig).cast()) };
    LAST_STATUS.store(status as usize, Ordering::Release);
    TRACE
        .runtime_trace
        .store(config.trace_address, Ordering::Release);
    TRACE
        .runtime_trace_size
        .store(config.trace_size as u64, Ordering::Release);
    record(1, 0, 0, 0, 0, 0, status);
    if status != 0 {
        return Err("Mode6 Install failed; owner must stop/reap this candidate process".into());
    }
    let s = lock.as_mut().unwrap();
    s.config = config;
    s.phase = Phase::Open;
    for hook in &s._hooks {
        hook.publish()?;
    }
    record(11, 0, 0, 0, 0, 0, 0); // Both EXE entries published; still no product proof.
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

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
