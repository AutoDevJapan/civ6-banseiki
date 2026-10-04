use core::arch::global_asm;
use sha2::{Digest, Sha256};
use std::ffi::{c_void, OsString};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::ptr::{self, null_mut};
use std::sync::atomic::{AtomicPtr, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use windows_sys::Win32::Foundation::FILETIME;
use windows_sys::Win32::System::Diagnostics::Debug::FlushInstructionCache;
use windows_sys::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW};
use windows_sys::Win32::System::Memory::{
    VirtualAlloc, VirtualFree, VirtualProtect, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE,
    PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, GetProcessTimes,
};

pub const TRACE_CAPACITY: usize = 4096;
const TRACE_MAGIC: u32 = 0x4a41_4342; // JACB
const TRACE_VERSION: u32 = 1;
const CODE_PAGE: usize = 4096;
const RELAY_OFFSET: usize = 0;
const TRAMPOLINE_OFFSET: usize = 32;
const PROLOGUE_LEN: usize = 5;
const PINNED_PROLOGUE: [u8; PROLOGUE_LEN] = [0x48, 0x89, 0x5c, 0x24, 0x08];
pub const PINNED_GAMECORE_SHA256: &str =
    "324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c";
pub const PINNED_CAN_EVER_RVA: usize = 0x2d_ddd0;
const PINNED_CAN_EVER_FILE_OFFSET: u64 = 0x2dd1d0;
pub const GAME_EXE: &str = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/Base/Binaries/Win64Steam/CivilizationVI.exe";
pub const GAMECORE: &str = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll";
const CONFIG_VERSION: u32 = 1;
pub const MAX_ACTIVE_SECONDS: u32 = 300;

/// Passed only by the bounded controller into its own remote allocation.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BridgeConfig {
    pub version: u32,
    pub size: u32,
    pub game_pid: u32,
    pub max_seconds: u32,
    pub created_filetime: u64,
    pub trace_address: u64,
    pub trace_size: u32,
    pub reserved: u32,
}

impl BridgeConfig {
    pub fn new(game_pid: u32, created_filetime: u64, trace_address: u64) -> Self {
        Self {
            version: CONFIG_VERSION,
            size: std::mem::size_of::<Self>() as u32,
            game_pid,
            max_seconds: MAX_ACTIVE_SECONDS,
            created_filetime,
            trace_address,
            trace_size: std::mem::size_of::<SharedTrace>() as u32,
            reserved: 0,
        }
    }
}

#[repr(u32)]
pub enum BridgeStatus {
    Ok = 0,
    InvalidInput = 1,
    ProcessIdentity = 2,
    GameCoreIdentity = 3,
    EntryBytes = 4,
    HookFailure = 5,
    Critical = 6,
    Panic = 99,
}

#[repr(C)]
#[derive(Default)]
pub struct TraceRow {
    pub sequence: AtomicU32,
    pub governor_index: AtomicU32,
    pub governors_this: AtomicU64,
    pub return_address: AtomicU64,
}

#[repr(C)]
pub struct SharedTrace {
    pub magic: u32,
    pub version: u32,
    pub next: AtomicU32,
    pub dropped: AtomicU32,
    pub rows: [TraceRow; TRACE_CAPACITY],
}

impl Default for SharedTrace {
    fn default() -> Self {
        Self {
            magic: TRACE_MAGIC,
            version: TRACE_VERSION,
            next: AtomicU32::new(0),
            dropped: AtomicU32::new(0),
            rows: std::array::from_fn(|_| TraceRow::default()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BridgeTrace {
    pub sequence: u32,
    pub governor_index: u32,
    pub governors_this: u64,
    pub return_address: u64,
}

impl SharedTrace {
    pub fn rows(&self) -> Vec<BridgeTrace> {
        let end = self.next.load(Ordering::Acquire).min(TRACE_CAPACITY as u32);
        (0..end)
            .filter_map(|slot| {
                let row = &self.rows[slot as usize];
                (row.sequence.load(Ordering::Acquire) == 1).then(|| BridgeTrace {
                    sequence: slot + 1,
                    governor_index: row.governor_index.load(Ordering::Relaxed),
                    governors_this: row.governors_this.load(Ordering::Relaxed),
                    return_address: row.return_address.load(Ordering::Relaxed),
                })
            })
            .collect()
    }
}

// The gateway deliberately has no Rust ABI for GameCore's unknown C++ type.
// It preserves flags and every register it uses, records RCX/EDX and the
// original return address, restores the caller stack, and tail-jumps through
// the five displaced bytes. The original function returns to its own caller.
global_asm!(
    ".globl bridge_gateway",
    "bridge_gateway:",
    "pushfq",
    "push rax",
    "push r10",
    "push r11",
    "mov r10, qword ptr [rip + BRIDGE_TRACE_PTR]",
    "test r10, r10",
    "jz 2f",
    "mov eax, 1",
    "lock xadd dword ptr [r10 + 8], eax",
    "cmp eax, 4096",
    "jae 1f",
    "imul rax, rax, 24",
    "lea r11, [r10 + rax + 16]",
    "mov dword ptr [r11 + 4], edx",
    "mov qword ptr [r11 + 8], rcx",
    "mov rax, qword ptr [rsp + 32]",
    "mov qword ptr [r11 + 16], rax",
    "mov eax, 1",
    "xchg dword ptr [r11], eax",
    "jmp 2f",
    "1:",
    "lock inc dword ptr [r10 + 12]",
    "2:",
    "pop r11",
    "pop r10",
    "pop rax",
    "popfq",
    "jmp qword ptr [rip + BRIDGE_TRAMPOLINE_PTR]",
);

unsafe extern "system" {
    fn bridge_gateway();
}

#[unsafe(no_mangle)]
static BRIDGE_TRACE_PTR: AtomicPtr<SharedTrace> = AtomicPtr::new(null_mut());
#[unsafe(no_mangle)]
static BRIDGE_TRAMPOLINE_PTR: AtomicPtr<u8> = AtomicPtr::new(null_mut());

struct Hook {
    target: *mut u8,
    original: [u8; 8],
    patched: [u8; 8],
    // Keep relay/trampoline allocated even after restoration. Another thread
    // could have entered the relay immediately before the entry was restored.
    _code: *mut u8,
}
unsafe impl Send for Hook {}

static HOOK: Mutex<Option<Hook>> = Mutex::new(None);

fn rel32_jump(from: usize, to: usize) -> Option<[u8; 5]> {
    let displacement = (to as i128) - (from as i128) - 5;
    let displacement = i32::try_from(displacement).ok()?;
    let mut jump = [0u8; 5];
    jump[0] = 0xe9;
    jump[1..].copy_from_slice(&displacement.to_le_bytes());
    Some(jump)
}

unsafe fn executable_near(target: usize) -> Option<*mut u8> {
    // VirtualAlloc's address hint is allocation-granularity aligned. Both
    // rel32 jumps must fit; a failed or occupied hint only advances the scan.
    let base = target & !0xffffusize;
    for step in 1..=0x4000usize {
        let delta = step * 0x10000;
        for candidate in [base.checked_add(delta), base.checked_sub(delta)] {
            let Some(candidate) = candidate else { continue };
            if rel32_jump(target, candidate).is_none()
                || rel32_jump(candidate + TRAMPOLINE_OFFSET + 5, target + 5).is_none()
            {
                continue;
            }
            let region = unsafe {
                VirtualAlloc(
                    candidate as *const c_void,
                    CODE_PAGE,
                    MEM_COMMIT | MEM_RESERVE,
                    PAGE_EXECUTE_READWRITE,
                )
            };
            if !region.is_null() {
                return Some(region.cast());
            }
        }
    }
    None
}

struct PatchOutcome {
    exchanged: bool,
    protection_restored: bool,
    cache_flushed: bool,
}

unsafe fn exchange_entry(
    target: *mut u8,
    expected: u64,
    replacement: u64,
) -> Result<PatchOutcome, &'static str> {
    let mut previous = 0u32;
    if unsafe { VirtualProtect(target.cast(), 8, PAGE_EXECUTE_READWRITE, &mut previous) } == 0 {
        return Err("cannot make the pinned entry writable");
    }
    let exchanged = unsafe { &*target.cast::<std::sync::atomic::AtomicU64>() }
        .compare_exchange(expected, replacement, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok();
    let mut replaced = 0u32;
    let protection_restored =
        unsafe { VirtualProtect(target.cast(), 8, previous, &mut replaced) } != 0;
    let cache_flushed =
        unsafe { FlushInstructionCache(GetCurrentProcess(), target.cast(), 8) } != 0;
    Ok(PatchOutcome {
        exchanged,
        protection_restored,
        cache_flushed,
    })
}

unsafe fn install_at(
    target: *mut u8,
    trace: *mut SharedTrace,
    expected: [u8; 8],
) -> Result<(), &'static str> {
    if target.is_null() || trace.is_null() || !(target as usize).is_multiple_of(8) {
        return Err("unaligned or missing hook input");
    }
    let mut guard = HOOK.lock().map_err(|_| "bridge lock poisoned")?;
    if guard.is_some() {
        return Err("bridge already installed");
    }
    if unsafe { (*trace).magic } != TRACE_MAGIC || unsafe { (*trace).version } != TRACE_VERSION {
        return Err("trace mapping version mismatch");
    }
    let original = unsafe { ptr::read(target.cast::<[u8; 8]>()) };
    if original != expected || expected[..PROLOGUE_LEN] != PINNED_PROLOGUE {
        return Err("pinned GameCore entry bytes differ");
    }
    let code = unsafe { executable_near(target as usize) }.ok_or("no rel32 relay near entry")?;
    let relay = unsafe { code.add(RELAY_OFFSET) };
    let trampoline = unsafe { code.add(TRAMPOLINE_OFFSET) };
    let jump_to_relay = rel32_jump(target as usize, relay as usize).ok_or("relay out of range")?;
    let jump_back = rel32_jump(trampoline as usize + 5, target as usize + 5)
        .ok_or("trampoline return out of range")?;
    unsafe {
        // FF 25 00000000 followed by the absolute gateway address.
        ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), relay, 6);
        ptr::copy_nonoverlapping(
            (bridge_gateway as *const () as usize as u64)
                .to_le_bytes()
                .as_ptr(),
            relay.add(6),
            8,
        );
        ptr::copy_nonoverlapping(original.as_ptr(), trampoline, PROLOGUE_LEN);
        ptr::copy_nonoverlapping(jump_back.as_ptr(), trampoline.add(PROLOGUE_LEN), 5);
    }
    let mut previous = 0u32;
    if unsafe { VirtualProtect(code.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut previous) } == 0
        || unsafe { FlushInstructionCache(GetCurrentProcess(), code.cast(), CODE_PAGE) } == 0
    {
        unsafe { VirtualFree(code.cast(), 0, MEM_RELEASE) };
        return Err("cannot publish the relay and trampoline");
    }
    BRIDGE_TRACE_PTR.store(trace, Ordering::Release);
    BRIDGE_TRAMPOLINE_PTR.store(trampoline, Ordering::Release);
    let mut patched = original;
    patched[..PROLOGUE_LEN].copy_from_slice(&jump_to_relay);
    let old_word = u64::from_le_bytes(original);
    let new_word = u64::from_le_bytes(patched);
    let outcome = unsafe { exchange_entry(target, old_word, new_word) }?;
    if !outcome.exchanged {
        BRIDGE_TRACE_PTR.store(null_mut(), Ordering::Release);
        BRIDGE_TRAMPOLINE_PTR.store(null_mut(), Ordering::Release);
        // A failed protection restoration leaves the page's state unknown.
        // Keep the relay resident until the owned game closes in that case.
        if outcome.protection_restored {
            unsafe { VirtualFree(code.cast(), 0, MEM_RELEASE) };
        }
        return Err("pinned entry changed before the atomic install");
    }
    *guard = Some(Hook {
        target,
        original,
        patched,
        _code: code,
    });
    if !outcome.protection_restored || !outcome.cache_flushed {
        return Err("critical: installed entry protection or cache unverified");
    }
    Ok(())
}

unsafe fn uninstall() -> Result<(), &'static str> {
    let mut guard = HOOK.lock().map_err(|_| "bridge lock poisoned")?;
    let hook = guard.as_ref().ok_or("bridge not installed")?;
    let old_word = u64::from_le_bytes(hook.patched);
    let new_word = u64::from_le_bytes(hook.original);
    let outcome = unsafe { exchange_entry(hook.target, old_word, new_word) }?;
    if !outcome.exchanged
        || !outcome.protection_restored
        || !outcome.cache_flushed
        || unsafe { ptr::read(hook.target.cast::<[u8; 8]>()) } != hook.original
    {
        return Err("pinned entry was not restored and verified");
    }
    // DLL and trampoline remain resident until process exit. There is no
    // FreeLibrary in this prototype, including after successful restoration.
    *guard = None;
    Ok(())
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn module_path(module: *mut c_void) -> Option<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    let len = unsafe { GetModuleFileNameW(module, buffer.as_mut_ptr(), buffer.len() as u32) };
    (len > 0 && (len as usize) < buffer.len())
        .then(|| PathBuf::from(OsString::from_wide(&buffer[..len as usize])))
}

fn same_file(actual: &Path, expected: &str) -> bool {
    let (Ok(actual), Ok(expected)) = (actual.canonicalize(), Path::new(expected).canonicalize())
    else {
        return false;
    };
    actual
        .to_string_lossy()
        .eq_ignore_ascii_case(&expected.to_string_lossy())
}

fn current_created_filetime() -> Option<u64> {
    let mut created = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut exited = created;
    let mut kernel = created;
    let mut user = created;
    if unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    } == 0
    {
        return None;
    }
    Some((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}

fn pinned_gamecore(path: &Path) -> Option<[u8; 8]> {
    let mut file = File::open(path).ok()?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let len = file.read(&mut buffer).ok()?;
        if len == 0 {
            break;
        }
        hash.update(&buffer[..len]);
    }
    if format!("{:x}", hash.finalize()) != PINNED_GAMECORE_SHA256 {
        return None;
    }
    file.seek(SeekFrom::Start(PINNED_CAN_EVER_FILE_OFFSET))
        .ok()?;
    let mut first = [0u8; 8];
    file.read_exact(&mut first).ok()?;
    (first[..PROLOGUE_LEN] == PINNED_PROLOGUE).then_some(first)
}

fn guarded_install(config: BridgeConfig) -> BridgeStatus {
    if config.version != CONFIG_VERSION
        || config.size as usize != std::mem::size_of::<BridgeConfig>()
        || config.game_pid == 0
        || config.created_filetime == 0
        || !(1..=MAX_ACTIVE_SECONDS).contains(&config.max_seconds)
        || config.trace_address == 0
        || config.trace_size as usize != std::mem::size_of::<SharedTrace>()
        || config.reserved != 0
    {
        return BridgeStatus::InvalidInput;
    }
    if unsafe { GetCurrentProcessId() } != config.game_pid
        || current_created_filetime() != Some(config.created_filetime)
        || !module_path(null_mut()).is_some_and(|path| same_file(&path, GAME_EXE))
    {
        return BridgeStatus::ProcessIdentity;
    }
    let module = unsafe { GetModuleHandleW(wide("GameCore_XP2_FinalRelease.dll").as_ptr()) };
    if module.is_null() || !module_path(module).is_some_and(|path| same_file(&path, GAMECORE)) {
        return BridgeStatus::GameCoreIdentity;
    }
    let Some(disk_bytes) = pinned_gamecore(Path::new(GAMECORE)) else {
        return BridgeStatus::GameCoreIdentity;
    };
    let Some(entry) = (module as usize).checked_add(PINNED_CAN_EVER_RVA) else {
        return BridgeStatus::EntryBytes;
    };
    let entry = entry as *mut u8;
    if unsafe { ptr::read(entry.cast::<[u8; 8]>()) } != disk_bytes {
        return BridgeStatus::EntryBytes;
    }
    let trace = config.trace_address as *mut SharedTrace;
    unsafe { ptr::write(trace, SharedTrace::default()) };
    match unsafe { install_at(entry, trace, disk_bytes) } {
        Ok(()) => BridgeStatus::Ok,
        Err(reason) if reason.starts_with("critical:") => BridgeStatus::Critical,
        Err(_) => BridgeStatus::HookFailure,
    }
}

/// Called on a bounded controller-created thread after LoadLibraryW returns.
/// Merely loading the DLL has no side effects on the game.
#[unsafe(no_mangle)]
pub extern "system" fn BridgeInstall(config: *mut c_void) -> u32 {
    if config.is_null() {
        return BridgeStatus::InvalidInput as u32;
    }
    std::panic::catch_unwind(|| {
        let config = unsafe { ptr::read_unaligned(config.cast::<BridgeConfig>()) };
        guarded_install(config) as u32
    })
    .unwrap_or(BridgeStatus::Panic as u32)
}

/// Restores and verifies the original entry. The DLL remains loaded until
/// the owned game exits, so a thread already in the relay cannot execute
/// unmapped code. A critical result requires the controller to close that game.
#[unsafe(no_mangle)]
pub extern "system" fn BridgeUninstall(_: *mut c_void) -> u32 {
    std::panic::catch_unwind(|| match unsafe { uninstall() } {
        Ok(()) => BridgeStatus::Ok as u32,
        Err(_) => BridgeStatus::Critical as u32,
    })
    .unwrap_or(BridgeStatus::Panic as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    static HOOK_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn gateway_preserves_original_result_and_captures_actual_call_arguments() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<TraceRow>(), 24);
        assert_eq!(std::mem::offset_of!(SharedTrace, rows), 16);
        let target = unsafe {
            VirtualAlloc(
                null_mut(),
                CODE_PAGE,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            )
        }
        .cast::<u8>();
        assert!(!target.is_null());
        // An owned executable test function with the pinned first instruction:
        // mov [rsp+8],rbx; mov rax,rcx; xor eax,edx; ret.
        let bytes = [
            0x48, 0x89, 0x5c, 0x24, 0x08, 0x48, 0x89, 0xc8, 0x31, 0xd0, 0xc3,
        ];
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len()) };
        let f: extern "system" fn(usize, u32) -> u32 = unsafe { std::mem::transmute(target) };
        assert_eq!(f(0x1234, 37), 0x1234 ^ 37);
        let mut trace = Box::new(SharedTrace::default());
        unsafe { install_at(target, trace.as_mut(), bytes[..8].try_into().unwrap()) }.unwrap();
        assert_eq!(f(0x1234, 37), 0x1234 ^ 37);
        assert_eq!(f(0x4567, 42), 0x4567 ^ 42);
        assert_eq!(
            trace
                .rows()
                .iter()
                .map(|r| (r.governors_this, r.governor_index))
                .collect::<Vec<_>>(),
            [(0x1234, 37), (0x4567, 42)]
        );
        assert!(trace.rows().iter().all(|r| r.return_address != 0));
        for i in 0..TRACE_CAPACITY + 2 {
            assert_eq!(f(0x1234, i as u32), 0x1234 ^ i as u32);
        }
        assert_eq!(trace.rows().len(), TRACE_CAPACITY);
        assert_eq!(trace.dropped.load(Ordering::Acquire), 4);
        unsafe { uninstall() }.unwrap();
        assert_eq!(f(0x9999, 99), 0x9999 ^ 99);
        assert_eq!(trace.rows().len(), TRACE_CAPACITY);
        assert_eq!(unsafe { ptr::read(target.cast::<[u8; 8]>()) }, bytes[..8]);
        assert_ne!(unsafe { VirtualFree(target.cast(), 0, MEM_RELEASE) }, 0);
    }

    #[test]
    fn atomic_restore_keeps_concurrent_synthetic_callers_on_original_behavior() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        let target = unsafe {
            VirtualAlloc(
                null_mut(),
                CODE_PAGE,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            )
        }
        .cast::<u8>();
        assert!(!target.is_null());
        let bytes = [
            0x48, 0x89, 0x5c, 0x24, 0x08, 0x48, 0x89, 0xc8, 0x31, 0xd0, 0xc3,
        ];
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len()) };
        let mut trace = Box::new(SharedTrace::default());
        unsafe { install_at(target, trace.as_mut(), bytes[..8].try_into().unwrap()) }.unwrap();
        let running = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let callers = (0..4u32)
            .map(|owner| {
                let running = Arc::clone(&running);
                let address = target as usize;
                std::thread::spawn(move || {
                    let f: extern "system" fn(usize, u32) -> u32 =
                        unsafe { std::mem::transmute(address) };
                    let mut calls = 0;
                    while running.load(Ordering::Acquire) && calls < 1_000_000 {
                        assert_eq!(f(0x1000 + owner as usize, owner), 0x1000);
                        calls += 1;
                    }
                    calls
                })
            })
            .collect::<Vec<_>>();
        std::thread::sleep(Duration::from_millis(5));
        unsafe { uninstall() }.unwrap();
        running.store(false, Ordering::Release);
        assert!(
            callers
                .into_iter()
                .map(|thread| thread.join().unwrap())
                .sum::<u32>()
                > 0
        );
        assert!(trace.next.load(Ordering::Acquire) > 0);
        assert_ne!(unsafe { VirtualFree(target.cast(), 0, MEM_RELEASE) }, 0);
    }

    #[test]
    fn rel32_rejects_out_of_range_and_accepts_exact_displacement() {
        assert!(rel32_jump(0, usize::MAX).is_none());
        assert_eq!(rel32_jump(0x1000, 0x2000), Some([0xe9, 0xfb, 0x0f, 0, 0]));
    }

    #[test]
    fn exported_install_rejects_missing_identity_before_touching_any_code() {
        assert_eq!(std::mem::size_of::<BridgeConfig>(), 40);
        let mut bad = BridgeConfig::new(0, 0, 0);
        assert_eq!(
            BridgeInstall((&mut bad as *mut BridgeConfig).cast()),
            BridgeStatus::InvalidInput as u32
        );
        bad.game_pid = unsafe { GetCurrentProcessId() };
        bad.created_filetime = current_created_filetime().unwrap();
        bad.trace_address = 1;
        assert_eq!(
            BridgeInstall((&mut bad as *mut BridgeConfig).cast()),
            BridgeStatus::ProcessIdentity as u32
        );
    }
}
