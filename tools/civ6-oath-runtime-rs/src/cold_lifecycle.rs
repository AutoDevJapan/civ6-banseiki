//! One-shot diagnostic at the checked F8510 native return boundary. READY
//! certifies Install+Start before this one caller resumes, not first-root,
//! other-root, all-thread coverage, native lifetime or any product quantity.
use crate::identity_windows as identity;
use crate::lifecycle_transport::{MappedTrace, SharedLifecycleConfig, MAPPING_SIZE};
use crate::native::*;
use crate::runtime::{own_process, validate, OathStatus};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::sync::OnceLock;
use windows_sys::Win32::Foundation::{GetLastError, SetLastError, FILETIME};
use windows_sys::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::{GetSystemTimeAsFileTime, GetTickCount64};

pub const EXPORTS: [&str; 4] = [
    "Civ6OathColdPrepare",
    "Civ6OathColdReadReceipt",
    "Civ6OathLifecycleStop",
    "Civ6OathLifecyclePoll",
];
pub const MAGIC: u32 = 0x434f_3134;
pub const VERSION: u32 = 1;
pub const PREPARED: u32 = 1;
pub const READY: u32 = 2;
pub const FAILED: u32 = 3;
/// A normal non-XP2 return is outside this boundary's coverage (not a failure).
pub const OUTSIDE_COVERAGE: u32 = 0x100;
pub const CALLER_RETURN_RVA: u64 = 0x24b532;
const LOADER: usize = 0xf8510;
const PREFIX: [u8; 7] = [0x48, 0x81, 0xec, 0x48, 0x02, 0, 0];
const MENU_PATH: &str = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/Base/Binaries/Win64Steam/GameCore_Base_FinalRelease.dll";
const MENU_SHA: &str = "d7ed520e1334947fc7b3be50b00c78eea55e69eb079583f24d87742e90f2a5c3";

/// Cold-only envelope. SharedLifecycleInstall remains exactly 64 bytes.
/// Parent supplies an absolute GetTickCount64 deadline before dispatch.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ColdPrepareConfig {
    pub shared: SharedLifecycleConfig,
    pub deadline_tick: u64,
    pub reserved: u64,
}
const _: () = assert!(std::mem::size_of::<ColdPrepareConfig>() == 80);
const _: () = assert!(std::mem::offset_of!(ColdPrepareConfig, deadline_tick) == 64);

/// Exactly 96 bytes. Every mutable field is atomic, including the first
/// preparation publication. Readers must take two equal copies; neither a
/// single copy nor READY certifies quiescence or the first root being observed.
#[repr(C)]
pub struct ColdReceipt {
    pub magic: AtomicU32,
    pub version: AtomicU32,
    pub state: AtomicU32,
    pub status: AtomicU32,
    pub pid: AtomicU32,
    pub thread: AtomicU32,
    pub filetime: AtomicU64,
    pub exe_base: AtomicU64,
    pub core_base: AtomicU64,
    pub caller_return_rva: AtomicU64,
    pub initialized_tick: AtomicU64,
    pub ready_tick: AtomicU64,
    pub deadline_tick: AtomicU64,
    pub reserved0: AtomicU64,
    pub reserved1: AtomicU64,
}
impl ColdReceipt {
    const fn new() -> Self {
        Self {
            magic: AtomicU32::new(MAGIC),
            version: AtomicU32::new(VERSION),
            state: AtomicU32::new(0),
            status: AtomicU32::new(0),
            pid: AtomicU32::new(0),
            thread: AtomicU32::new(0),
            filetime: AtomicU64::new(0),
            exe_base: AtomicU64::new(0),
            core_base: AtomicU64::new(0),
            caller_return_rva: AtomicU64::new(0),
            initialized_tick: AtomicU64::new(0),
            ready_tick: AtomicU64::new(0),
            deadline_tick: AtomicU64::new(0),
            reserved0: AtomicU64::new(0),
            reserved1: AtomicU64::new(0),
        }
    }
    fn fail(&self, status: OathStatus) {
        self.status.store(status as u32, Ordering::Release);
        self.state.store(FAILED, Ordering::Release);
    }
}
const _: () = assert!(std::mem::size_of::<ColdReceipt>() == 96);
const _: () = assert!(std::mem::offset_of!(ColdReceipt, deadline_tick) == 72);
static RECEIPT: ColdReceipt = ColdReceipt::new();
static USED: AtomicU32 = AtomicU32::new(0);
static FLIGHT: AtomicU32 = AtomicU32::new(0);
static RESIDENT: OnceLock<Resident> = OnceLock::new();
#[unsafe(no_mangle)]
static COLD_CONTINUE: AtomicU64 = AtomicU64::new(0);

struct Resident {
    exe: usize,
    request: SharedLifecycleConfig,
    // The parent owns and retains this allocation through process termination.
    _input: usize,
    // Source files are denied write/delete sharing and retained even on failure.
    _sources: Vec<File>,
}
fn now() -> u64 {
    #[cfg(test)]
    if let Some(test) = tests::SYNTHETIC.get() {
        let tick = test.clock.load(Ordering::Acquire);
        if tick != 0 {
            return tick;
        }
    }
    unsafe { GetTickCount64() }
}
fn module(name: &str) -> usize {
    let wide: Vec<u16> = name.encode_utf16().chain([0]).collect();
    unsafe { GetModuleHandleW(wide.as_ptr()) as usize }
}
fn pointer(at: usize) -> Option<usize> {
    copy_current::<8>(at as u64).map(|b| u64::from_le_bytes(b) as usize)
}

global_asm!(
    ".text", ".globl cold_original", ".seh_proc cold_original", "cold_original:",
    "sub rsp,0x248", ".seh_stackalloc 0x248", ".seh_endprologue",
    // RIP-indirect JMP is recognized as a tail epilogue even without ADD
    // RSP. Register JMP keeps the 0x248 frame live for the system unwinder.
    "mov rax,qword ptr [rip+COLD_CONTINUE]", "jmp rax", ".seh_endproc",
    ".globl cold_gateway", ".seh_proc cold_gateway", "cold_gateway:",
    "sub rsp,0x38", ".seh_stackalloc 0x38", ".seh_endprologue",
    // Fifth argument is the untouched native return address. All four actual
    // register arguments reach the system-unwind wrapper without substitution.
    "mov rax,[rsp+0x38]", "mov [rsp+0x20],rax", "call {wrapper}",
    "add rsp,0x38", "ret", ".seh_endproc",
    wrapper = sym loader_wrapper,
);
unsafe extern "system-unwind" {
    fn cold_original(a: usize, b: usize, c: usize, d: usize) -> usize;
    fn cold_gateway(a: usize, b: usize, c: usize, d: usize) -> usize;
}
struct Flight {
    completed: bool,
}
impl Drop for Flight {
    fn drop(&mut self) {
        if !self.completed {
            RECEIPT.fail(OathStatus::Critical);
        }
        FLIGHT.store(0, Ordering::Release);
    }
}
fn fatal(status: OathStatus) -> ! {
    RECEIPT.fail(status);
    std::process::abort()
}
extern "system-unwind" fn loader_wrapper(
    a: usize,
    b: usize,
    c: usize,
    d: usize,
    ret: usize,
) -> usize {
    let incoming_error = unsafe { GetLastError() };
    // Publication is one-shot. Later native loads (including a Base-menu
    // transition while the pinned XP2 module still exists) are original calls,
    // not another cold activation. Preserve the successful first receipt.
    if RECEIPT.state.load(Ordering::Acquire) == READY {
        unsafe { SetLastError(incoming_error) };
        return unsafe { cold_original(a, b, c, d) };
    }
    if FLIGHT
        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        fatal(OathStatus::WrongState);
    }
    let mut flight = Flight { completed: false };
    // No helper lock is held across the original. Its return and LastError
    // survive all identity checks, callbacks and the synchronous Install/Start.
    unsafe { SetLastError(incoming_error) };
    let result = unsafe { cold_original(a, b, c, d) };
    let error = unsafe { GetLastError() };
    if let Err(status) =
        std::panic::catch_unwind(|| after_original(result, ret)).unwrap_or(Err(OathStatus::Panic))
    {
        fatal(status);
    }
    flight.completed = true;
    drop(flight);
    unsafe { SetLastError(error) };
    result
}

fn after_original(result: usize, ret: usize) -> Result<(), OathStatus> {
    let s = RESIDENT.get().ok_or(OathStatus::WrongState)?;
    let core = observed_core();
    if core == 0 {
        // Missing Core never becomes READY, even for the fixed caller.
        RECEIPT.status.store(OUTSIDE_COVERAGE, Ordering::Release);
        return Ok(());
    }
    if result & 0xff == 0 {
        RECEIPT.fail(OathStatus::HookFailure);
        return Ok(()); // Preserve the original failure, never report success.
    }
    if RECEIPT.state.load(Ordering::Acquire) != PREPARED
        || ret.checked_sub(s.exe) != Some(CALLER_RETURN_RVA as usize)
        || now() >= RECEIPT.deadline_tick.load(Ordering::Acquire)
    {
        return Err(OathStatus::WrongState);
    }
    verify_loaded_boundary(s, core, ret)?;
    RECEIPT
        .thread
        .store(unsafe { GetCurrentThreadId() }, Ordering::Release);
    RECEIPT.core_base.store(core as u64, Ordering::Release);
    RECEIPT
        .caller_return_rva
        .store(CALLER_RETURN_RVA, Ordering::Release);
    let deadline = RECEIPT.deadline_tick.load(Ordering::Acquire);
    activate(s, deadline)?;
    let ready = now();
    if ready >= deadline {
        return Err(OathStatus::WrongState);
    }
    RECEIPT.ready_tick.store(ready, Ordering::Release);
    RECEIPT.status.store(0, Ordering::Release);
    RECEIPT.state.store(READY, Ordering::Release);
    Ok(())
}

fn observed_core() -> usize {
    #[cfg(test)]
    if let Some(s) = tests::SYNTHETIC.get() {
        return s.core.load(Ordering::Acquire);
    }
    module("GameCore_XP2_FinalRelease.dll")
}
fn verify_loaded_boundary(s: &Resident, core: usize, ret: usize) -> Result<(), OathStatus> {
    #[cfg(test)]
    if let Some(test) = tests::SYNTHETIC.get() {
        return test.verify_boundary(s, core, ret);
    }
    if !identity::same_path(
        &identity::module_path(core as _)?,
        crate::preflight::CORE_PATH,
    ) || crate::runtime::pin_module(core) != Some(core)
        || pointer(s.exe + 0x22d60d8) != Some(core)
        || pointer(s.exe + 0x22d64e8) != Some(core + 0x752d50)
        || pointer(s.exe + 0x22d60c0) != Some(0)
        || copy_current::<5>(core as u64 + 0x752d50) != Some([0xe9, 0x0b, 0xcf, 0xe3, 0xff])
        || !direct_caller(s.exe, ret)
    {
        return Err(OathStatus::EntryBytes);
    }
    Ok(())
}
fn direct_caller(exe: usize, ret: usize) -> bool {
    if ret != exe + CALLER_RETURN_RVA as usize {
        return false;
    }
    copy_current::<5>((ret - 5) as u64).is_some_and(|b| {
        b[0] == 0xe8
            && ret.checked_add_signed(i32::from_le_bytes(b[1..].try_into().unwrap()) as isize)
                == Some(exe + LOADER)
    })
}
fn activate(s: &Resident, deadline: u64) -> Result<(), OathStatus> {
    crate::lifecycle_trace::bound_cold_start(deadline)?;
    let mut request = s.request;
    // This retained copy still has zero out fields; the parent's original
    // config already names its own retained view of the same shared mapping.
    if install_callback(&mut request) != 0 {
        return Err(OathStatus::HookFailure);
    }
    #[cfg(test)]
    if let Some(test) = tests::SYNTHETIC.get() {
        test.before_start();
    }
    if now() >= deadline || crate::lifecycle_trace::Civ6OathLifecycleStart() != 0 {
        return Err(OathStatus::HookFailure);
    }
    #[cfg(test)]
    if let Some(test) = tests::SYNTHETIC.get() {
        test.after_start(deadline);
    }
    Ok(())
}
fn install_callback(request: &mut SharedLifecycleConfig) -> u32 {
    #[cfg(test)]
    if let Some(test) = tests::SYNTHETIC.get() {
        return test.install(request);
    }
    crate::lifecycle_trace::Civ6OathLifecycleInstallShared(
        (request as *mut SharedLifecycleConfig).cast(),
    )
}

/// Call1 remote-thread ABI; never DllMain or a loader-lock/breakpoint wait.
/// 80-byte ColdPrepareConfig (shared64 + absolute deadline + reserved=0).
/// On success ONLY the two shared out fields
/// change, immediately. The parent must retain input and mapping handle until
/// target exit. No reprepare or rearm, including a failed preparation attempt.
#[unsafe(no_mangle)]
pub extern "system" fn Civ6OathColdPrepare(input: *mut c_void) -> u32 {
    let initialized = now();
    if USED
        .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return OathStatus::WrongState as u32;
    }
    let status =
        std::panic::catch_unwind(|| prepare(input, initialized)).unwrap_or(Err(OathStatus::Panic));
    match status {
        Ok(()) => 0,
        Err(e) => {
            RECEIPT.fail(e);
            e as u32
        }
    }
}
fn validate_shared(c: &SharedLifecycleConfig) -> Result<(), OathStatus> {
    validate(&c.config)?;
    if c.config.flags != 0
        || c.mapping_size != MAPPING_SIZE
        || c.reserved != 0
        || c.mapping_handle == 0
        || c.mapping_handle == u64::MAX
    {
        return Err(OathStatus::InvalidInput);
    }
    Ok(())
}
fn prepare(input: *mut c_void, initialized: u64) -> Result<(), OathStatus> {
    prepare_verified(input, initialized, cold_identity)
}
// Private synthetic injection supplies source proof only; the public export
// always calls cold_identity even in a test binary. Everything after identity,
// including mapping, config output, retention and the real CAS, is shared.
fn prepare_verified(
    input: *mut c_void,
    initialized: u64,
    proof: impl FnOnce(&SharedLifecycleConfig) -> Result<(usize, Vec<File>), OathStatus>,
) -> Result<(), OathStatus> {
    if !committed_range(input as usize, 80, 8, true) {
        return Err(OathStatus::InvalidInput);
    }
    let cold = unsafe { ptr::read(input.cast::<ColdPrepareConfig>()) };
    let request = cold.shared;
    validate_shared(&request)?;
    let deadline = cold.deadline_tick;
    let current = now();
    if cold.reserved != 0
        || deadline <= current
        || deadline > initialized.saturating_add(u64::from(request.config.max_seconds) * 1000)
    {
        return Err(OathStatus::InvalidInput);
    }
    own_process(&request.config)?;
    RECEIPT
        .pid
        .store(request.config.game_pid, Ordering::Release);
    RECEIPT
        .filetime
        .store(request.config.created_filetime, Ordering::Release);
    RECEIPT
        .initialized_tick
        .store(initialized, Ordering::Release);
    RECEIPT.deadline_tick.store(deadline, Ordering::Release);
    let view = unsafe { MappedTrace::map(request.mapping_handle as _, request.mapping_size) }
        .map_err(|_| OathStatus::InvalidInput)?;
    if !view.trace().initialized() {
        return Err(OathStatus::InvalidInput);
    }
    let (exe, sources) = proof(&request)?;
    if crate::runtime::pin_module(Civ6OathColdPrepare as *const () as usize).is_none() {
        return Err(OathStatus::ImageIdentity);
    }
    if now() >= deadline {
        return Err(OathStatus::WrongState);
    }
    // Everything fetched by a gateway is resident before any entry mutation.
    let t = view.retain();
    let s = Resident {
        exe,
        request,
        _input: input as usize,
        _sources: sources,
    };
    RESIDENT.set(s).map_err(|_| OathStatus::WrongState)?;
    COLD_CONTINUE.store((exe + LOADER + PREFIX.len()) as u64, Ordering::Release);
    RECEIPT.exe_base.store(exe as u64, Ordering::Release);
    let output = unsafe { &mut *input.cast::<ColdPrepareConfig>() };
    output.shared.config.out_trace_address = t as *const _ as u64;
    output.shared.config.trace_size = MAPPING_SIZE;
    RECEIPT.state.store(PREPARED, Ordering::Release);
    // Only now publish the native entry. Any failed attempt is terminal; the
    // parent must terminate its owned process after a nonzero Prepare status.
    publish(exe + LOADER)?;
    if now() >= deadline {
        return Err(OathStatus::WrongState);
    }
    Ok(())
}
fn cold_identity(request: &SharedLifecycleConfig) -> Result<(usize, Vec<File>), OathStatus> {
    let exe = unsafe { GetModuleHandleW(ptr::null()) } as usize;
    if exe == 0
        || !identity::same_path(
            &identity::module_path(exe as _)?,
            crate::preflight::EXE_PATH,
        )
    {
        return Err(OathStatus::ProcessIdentity);
    }
    let mut wall: FILETIME = unsafe { std::mem::zeroed() };
    unsafe { GetSystemTimeAsFileTime(&mut wall) };
    let wall = (u64::from(wall.dwHighDateTime) << 32) | u64::from(wall.dwLowDateTime);
    if wall
        .checked_sub(request.config.created_filetime)
        .is_none_or(|age| age > 300 * 10_000_000)
        || module("GameCore_XP2_FinalRelease.dll") != 0
    {
        return Err(OathStatus::ProcessIdentity);
    }
    let menu = module("GameCore_Base_FinalRelease.dll");
    if menu == 0 || !identity::same_path(&identity::module_path(menu as _)?, MENU_PATH) {
        return Err(OathStatus::ImageIdentity);
    }
    let mut files = Vec::new();
    let mut disks = Vec::new();
    for (path, hash) in [
        (crate::preflight::EXE_PATH, crate::preflight::EXE_SHA256),
        (MENU_PATH, MENU_SHA),
        (crate::preflight::CORE_PATH, crate::preflight::CORE_SHA256),
    ] {
        let (file, bytes) = identity::pinned_disk(path)?;
        if format!("{:x}", Sha256::digest(&bytes)) != hash {
            return Err(OathStatus::ImageIdentity);
        }
        files.push(file);
        disks.push(bytes);
    }
    if !identity::header_matches(exe, &disks[0])
        || !identity::header_matches(menu, &disks[1])
        || crate::runtime::pin_module(exe) != Some(exe)
        || crate::runtime::pin_module(menu) != Some(menu)
    {
        return Err(OathStatus::ImageIdentity);
    }
    let context = pointer(exe + 0x22d60c0)
        .filter(|&n| n != 0)
        .ok_or(OathStatus::EntryBytes)?;
    if pointer(context) != Some(menu + 0x79e020)
        || pointer(exe + 0x22d64e8) != Some(menu + 0x58dd30)
    {
        return Err(OathStatus::EntryBytes);
    }
    identity::match_function(exe, &disks[0], LOADER as u32)?;
    identity::match_window(exe, &disks[0], (LOADER - 5) as u32, 5)?;
    if identity::source_rva(&disks[0], LOADER as u32, 7)? != PREFIX
        || copy_current::<5>((exe + LOADER - 5) as u64) != Some([0xcc; 5])
        || !direct_caller(exe, exe + CALLER_RETURN_RVA as usize)
    {
        return Err(OathStatus::EntryBytes);
    }
    // Padding must have no function owner; the complete caller's fixed bytes
    // and unwind (not merely an E8 opcode) must also match the hashed source.
    for offset in 1..=5 {
        let mut owner = 0;
        if !unsafe {
            RtlLookupFunctionEntry((exe + LOADER - offset) as u64, &mut owner, ptr::null_mut())
        }
        .is_null()
        {
            return Err(OathStatus::EntryBytes);
        }
    }
    let mut owner = 0;
    let entry = unsafe {
        RtlLookupFunctionEntry(
            exe as u64 + CALLER_RETURN_RVA - 5,
            &mut owner,
            ptr::null_mut(),
        )
    };
    if entry.is_null() || owner != exe as u64 {
        return Err(OathStatus::EntryBytes);
    }
    let begin = copy_current::<4>(entry as u64)
        .map(u32::from_le_bytes)
        .ok_or(OathStatus::EntryBytes)?;
    identity::match_function(exe, &disks[0], begin)?;
    Ok((exe, files))
}

fn publish(entry: usize) -> Result<(), OathStatus> {
    if !entry.is_multiple_of(2)
        || copy_current::<7>(entry as u64) != Some(PREFIX)
        || copy_current::<5>((entry - 5) as u64) != Some([0xcc; 5])
    {
        return Err(OathStatus::EntryBytes);
    }
    let relay = unsafe { executable_near(entry - 5) }.ok_or(OathStatus::HookFailure)?;
    // RIP-indirect tail jump is a leaf and leaves all four register arguments
    // and the native return address untouched. Relay is never freed.
    let mut code = [0u8; 14];
    code[..6].copy_from_slice(&[0xff, 0x25, 0, 0, 0, 0]);
    code[6..].copy_from_slice(&(cold_gateway as *const () as u64).to_le_bytes());
    unsafe { ptr::copy_nonoverlapping(code.as_ptr(), relay, code.len()) };
    let mut old = 0;
    if unsafe { VirtualProtect(relay.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut old) } == 0
        || unsafe { FlushInstructionCache(GetCurrentProcess(), relay.cast(), code.len()) } == 0
    {
        return Err(OathStatus::HookFailure);
    }
    let jump = rel32_jump(entry - 5, relay as usize).ok_or(OathStatus::HookFailure)?;
    write_padding(entry - 5, &jump)?;
    let mut protection = 0;
    if unsafe { VirtualProtect(entry as _, 2, PAGE_EXECUTE_READWRITE, &mut protection) } == 0 {
        return Err(OathStatus::HookFailure);
    }
    let exchanged = unsafe { &*(entry as *const AtomicU16) }
        .compare_exchange(
            u16::from_le_bytes(PREFIX[..2].try_into().unwrap()),
            u16::from_le_bytes(PROPERTY_SHORT_JUMP),
            Ordering::SeqCst,
            Ordering::SeqCst,
        )
        .is_ok();
    let mut ignored = 0;
    let restored = unsafe { VirtualProtect(entry as _, 2, protection, &mut ignored) } != 0;
    let flushed = unsafe { FlushInstructionCache(GetCurrentProcess(), entry as _, 2) } != 0;
    if !exchanged || !restored || !flushed {
        return Err(OathStatus::Critical);
    }
    Ok(())
}
fn write_padding(at: usize, bytes: &[u8; 5]) -> Result<(), OathStatus> {
    if copy_current::<5>(at as u64) != Some([0xcc; 5]) {
        return Err(OathStatus::EntryBytes);
    }
    let mut old = 0;
    if unsafe { VirtualProtect(at as _, 5, PAGE_EXECUTE_READWRITE, &mut old) } == 0 {
        return Err(OathStatus::HookFailure);
    }
    unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), at as *mut u8, 5) };
    let mut ignored = 0;
    let restored = unsafe { VirtualProtect(at as _, 5, old, &mut ignored) } != 0;
    let flushed = unsafe { FlushInstructionCache(GetCurrentProcess(), at as _, 5) } != 0;
    if !restored || !flushed {
        return Err(OathStatus::Critical);
    }
    Ok(())
}

/// Call1 ABI, aligned writable 96-byte output, remote thread exit status zero.
/// Copies each field through atomic loads/stores. Parent samples twice.
#[unsafe(no_mangle)]
pub extern "system" fn Civ6OathColdReadReceipt(input: *mut c_void) -> u32 {
    if !committed_range(input as usize, 96, 8, true) {
        return OathStatus::InvalidInput as u32;
    }
    let out = unsafe { &*input.cast::<ColdReceipt>() };
    macro_rules! fields { ($($name:ident),*) => { $(out.$name.store(RECEIPT.$name.load(Ordering::Acquire), Ordering::Release);)* }; }
    // State is acquired first and published last, conservatively retaining an
    // older state if READY/FAILED changes while payload fields are sampled.
    let state = RECEIPT.state.load(Ordering::Acquire);
    fields!(
        magic,
        version,
        status,
        pid,
        thread,
        filetime,
        exe_base,
        core_base,
        caller_return_rva,
        initialized_tick,
        ready_tick,
        deadline_tick,
        reserved0,
        reserved1
    );
    out.state.store(state, Ordering::Release);
    0
}

#[cfg(test)]
#[path = "cold_lifecycle_tests.rs"]
mod tests;
