use core::arch::global_asm;
use sha2::{Digest, Sha256};
use std::ffi::{c_void, OsString};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::ptr::{self, null_mut};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use windows_sys::Win32::Foundation::FILETIME;
use windows_sys::Win32::System::Diagnostics::Debug::FlushInstructionCache;
use windows_sys::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows_sys::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW};
use windows_sys::Win32::System::Memory::{
    VirtualAlloc, VirtualFree, VirtualProtect, VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT,
    MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY,
    PAGE_GUARD, PAGE_NOACCESS, PAGE_READONLY, PAGE_READWRITE, PAGE_WRITECOPY,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId, GetProcessTimes,
};

pub const TRACE_CAPACITY: usize = 4096;
const TRACE_MAGIC: u32 = 0x4a41_4342; // JACB
const TRACE_VERSION: u32 = 2;
const PROPERTY_TRACE_MAGIC: u32 = 0x4a41_4350; // JACP
const PROPERTY_TRACE_VERSION: u32 = 1;
const CODE_PAGE: usize = 4096;
const RELAY_OFFSET: usize = 0;
const TRAMPOLINE_OFFSET: usize = 32;
const PROLOGUE_LEN: usize = 5;
const PINNED_PROLOGUE: [u8; PROLOGUE_LEN] = [0x48, 0x89, 0x5c, 0x24, 0x08];
// LLVM disassembly of the pinned image: mov rax,[rcx+40h] (4 bytes),
// lea r9,[rcx+30h] (4 bytes). Both whole instructions are relocated.
const PROPERTY_PROLOGUE: [u8; 8] = [0x48, 0x8b, 0x41, 0x40, 0x4c, 0x8d, 0x49, 0x30];
const PROPERTY_FIRST_LEN: usize = 4;
const PROPERTY_PREFIX_LEN: usize = 5;
const PROPERTY_PREFIX_PAD: [u8; PROPERTY_PREFIX_LEN] = [0xcc; PROPERTY_PREFIX_LEN];
// EB F9 jumps from entry+2 to entry-5. Entry+2..+7 are never modified.
const PROPERTY_SHORT_JUMP: [u8; 2] = [0xeb, 0xf9];
pub const PINNED_GAMECORE_SHA256: &str =
    "324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c";
pub const PINNED_CAN_EVER_RVA: usize = 0x2d_ddd0;
const PINNED_CAN_EVER_FILE_OFFSET: u64 = 0x2dd1d0;
pub const PINNED_PROPERTY_GET_RVA: usize = 0x99_f5c0;
const PINNED_PROPERTY_GET_FILE_OFFSET: u64 = 0x99_e9c0;
pub const GAME_EXE: &str = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/Base/Binaries/Win64Steam/CivilizationVI.exe";
pub const GAMECORE: &str = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll";
const CONFIG_VERSION: u32 = 1;
use crate::MAX_ACTIVE_SECONDS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum BridgeMode {
    CanEver = 0,
    Property = 1,
}

impl BridgeMode {
    pub const fn rva(self) -> usize {
        match self {
            Self::CanEver => PINNED_CAN_EVER_RVA,
            Self::Property => PINNED_PROPERTY_GET_RVA,
        }
    }

    const fn file_offset(self) -> u64 {
        match self {
            Self::CanEver => PINNED_CAN_EVER_FILE_OFFSET,
            Self::Property => PINNED_PROPERTY_GET_FILE_OFFSET,
        }
    }

    const fn prologue(self) -> &'static [u8] {
        match self {
            Self::CanEver => &PINNED_PROLOGUE,
            Self::Property => &PROPERTY_PROLOGUE,
        }
    }

    pub const fn trace_size(self) -> usize {
        match self {
            Self::CanEver => std::mem::size_of::<SharedTrace>(),
            Self::Property => std::mem::size_of::<PropertyTrace>(),
        }
    }
}

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
    pub mode: u32,
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
            mode: BridgeMode::CanEver as u32,
        }
    }

    pub fn for_property(game_pid: u32, created_filetime: u64, trace_address: u64) -> Self {
        let mut config = Self::new(game_pid, created_filetime, trace_address);
        config.mode = BridgeMode::Property as u32;
        config.trace_size = BridgeMode::Property.trace_size() as u32;
        config
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
    pub in_flight: AtomicU32,
    pub reserved: u32,
    pub rows: [TraceRow; TRACE_CAPACITY],
}

impl Default for SharedTrace {
    fn default() -> Self {
        Self {
            magic: TRACE_MAGIC,
            version: TRACE_VERSION,
            next: AtomicU32::new(0),
            dropped: AtomicU32::new(0),
            in_flight: AtomicU32::new(0),
            reserved: 0,
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
    pub fn complete_rows(&self) -> Result<Vec<BridgeTrace>, &'static str> {
        if self.in_flight.load(Ordering::Acquire) != 0 {
            return Err("gateway calls remain in flight");
        }
        if self.dropped.load(Ordering::Acquire) != 0 {
            return Err("gateway trace capacity exceeded");
        }
        let next = self.next.load(Ordering::Acquire);
        if next as usize > TRACE_CAPACITY {
            return Err("gateway trace sequence exceeded capacity");
        }
        let rows = self.rows();
        if rows.len() != next as usize {
            return Err("gateway trace contains an unfinished slot");
        }
        Ok(rows)
    }

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

/// Fixed-size, string-free telemetry for the direct native getter. `read_state`
/// is 0 for a missing key, 1 for a complete 24-byte copy, 2 for an unreadable
/// returned address. A returned pointer is never retained for a later read.
#[repr(C)]
#[derive(Default)]
pub struct PropertyRow {
    pub sequence: AtomicU32,
    pub key_id: AtomicU32,
    pub property: AtomicU64,
    pub returned: AtomicU64,
    pub thread_id: AtomicU32,
    pub read_state: AtomicU32,
    pub tag: AtomicU32,
    pub flags: AtomicU32,
    pub payload: AtomicU64,
}

#[repr(C)]
pub struct PropertyTrace {
    pub magic: u32,
    pub version: u32,
    pub next: AtomicU32,
    pub dropped: AtomicU32,
    pub in_flight: AtomicU32,
    pub reserved: u32,
    pub rows: [PropertyRow; TRACE_CAPACITY],
}

impl Default for PropertyTrace {
    fn default() -> Self {
        Self {
            magic: PROPERTY_TRACE_MAGIC,
            version: PROPERTY_TRACE_VERSION,
            next: AtomicU32::new(0),
            dropped: AtomicU32::new(0),
            in_flight: AtomicU32::new(0),
            reserved: 0,
            rows: std::array::from_fn(|_| PropertyRow::default()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PropertyValueTrace {
    pub sequence: u32,
    pub key_id: u32,
    pub property: u64,
    pub returned: u64,
    pub thread_id: u32,
    pub read_state: u32,
    pub tag: u16,
    pub payload: u64,
    pub flags: u32,
}

impl PropertyTrace {
    pub fn complete_rows(&self) -> Result<Vec<PropertyValueTrace>, &'static str> {
        if self.dropped.load(Ordering::Acquire) != 0 {
            return Err("property getter trace capacity exceeded");
        }
        self.snapshot_rows()
    }

    /// All published rows, including a capacity-limited partial capture.
    /// The caller separately reports `next` and `dropped` in the artifact.
    pub fn snapshot_rows(&self) -> Result<Vec<PropertyValueTrace>, &'static str> {
        if self.in_flight.load(Ordering::Acquire) != 0 {
            return Err("property getter calls remain in flight");
        }
        let next = self.next.load(Ordering::Acquire);
        let dropped = self.dropped.load(Ordering::Acquire);
        if next.saturating_sub(TRACE_CAPACITY as u32) != dropped {
            return Err("property getter count and dropped count disagree");
        }
        let end = next.min(TRACE_CAPACITY as u32) as usize;
        let mut rows = Vec::with_capacity(end);
        for (index, row) in self.rows[..end].iter().enumerate() {
            if row.sequence.load(Ordering::Acquire) != 1 {
                return Err("property getter row was not published");
            }
            rows.push(PropertyValueTrace {
                sequence: index as u32 + 1,
                key_id: row.key_id.load(Ordering::Relaxed),
                property: row.property.load(Ordering::Relaxed),
                returned: row.returned.load(Ordering::Relaxed),
                thread_id: row.thread_id.load(Ordering::Relaxed),
                read_state: row.read_state.load(Ordering::Relaxed),
                tag: row.tag.load(Ordering::Relaxed) as u16,
                payload: row.payload.load(Ordering::Relaxed),
                flags: row.flags.load(Ordering::Relaxed),
            });
        }
        Ok(rows)
    }
}

// Called only after the original getter returned. This path performs no
// allocation, formatting, locking, Lua access, or C++ method invocation.
// ReadProcessMemory copies all 24 bytes or records unreadable; the borrowed
// returned pointer is never read after this synchronous call.
unsafe extern "system" fn record_property(property: u64, key_id: u32, returned: u64) {
    let trace = PROPERTY_TRACE_PTR.load(Ordering::Acquire);
    if trace.is_null() {
        return;
    }
    let trace = unsafe { &*trace };
    trace.in_flight.fetch_add(1, Ordering::AcqRel);
    let slot = trace.next.fetch_add(1, Ordering::AcqRel) as usize;
    if slot >= TRACE_CAPACITY {
        trace.dropped.fetch_add(1, Ordering::Relaxed);
        trace.in_flight.fetch_sub(1, Ordering::Release);
        return;
    }
    let row = &trace.rows[slot];
    row.key_id.store(key_id, Ordering::Relaxed);
    row.property.store(property, Ordering::Relaxed);
    row.returned.store(returned, Ordering::Relaxed);
    row.thread_id
        .store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
    if returned == 0 {
        row.read_state.store(0, Ordering::Relaxed);
    } else {
        let mut bytes = [0u8; 24];
        let mut read = 0usize;
        let ok = unsafe {
            ReadProcessMemory(
                GetCurrentProcess(),
                returned as *const c_void,
                bytes.as_mut_ptr().cast(),
                bytes.len(),
                &mut read,
            )
        } != 0
            && read == bytes.len();
        if ok {
            row.tag.store(
                u16::from_le_bytes([bytes[0], bytes[1]]) as u32,
                Ordering::Relaxed,
            );
            row.payload.store(
                u64::from_le_bytes([
                    bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14],
                    bytes[15],
                ]),
                Ordering::Relaxed,
            );
            row.flags.store(
                u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
                Ordering::Relaxed,
            );
            row.read_state.store(1, Ordering::Relaxed);
        } else {
            row.read_state.store(2, Ordering::Relaxed);
        }
    }
    row.sequence.store(1, Ordering::Release);
    trace.in_flight.fetch_sub(1, Ordering::Release);
}

// Win64 entry: RCX=property, EDX=key. .seh_* emits pdata/xdata for this
// nonleaf frame. The pinned 0x99F5C0 getter has no covering pdata entry and
// only leaf instructions; the allocated trampoline is a MOV plus direct JMP.
// The 0x68-byte frame supplies aligned
// calls and shadow space. The original sees the exact argument registers once;
// its RAX is saved across telemetry and restored before returning to its caller.
global_asm!(
    ".globl property_gateway",
    ".seh_proc property_gateway",
    "property_gateway:",
    "sub rsp, 0x68",
    ".seh_stackalloc 0x68",
    ".seh_endprologue",
    "mov qword ptr [rsp + 0x20], rcx",
    "mov qword ptr [rsp + 0x28], rdx",
    "call qword ptr [rip + PROPERTY_TRAMPOLINE_PTR]",
    "mov qword ptr [rsp + 0x30], rax",
    "mov rcx, qword ptr [rsp + 0x20]",
    "mov edx, dword ptr [rsp + 0x28]",
    "mov r8, rax",
    "call {record}",
    "mov rax, qword ptr [rsp + 0x30]",
    "add rsp, 0x68",
    "ret",
    ".seh_endproc",
    record = sym record_property,
);

unsafe extern "system" {
    fn property_gateway();
}

#[unsafe(no_mangle)]
static PROPERTY_TRACE_PTR: AtomicPtr<PropertyTrace> = AtomicPtr::new(null_mut());
#[unsafe(no_mangle)]
static PROPERTY_TRAMPOLINE_PTR: AtomicPtr<u8> = AtomicPtr::new(null_mut());

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
    "lock inc dword ptr [r10 + 16]",
    "mov eax, 1",
    "lock xadd dword ptr [r10 + 8], eax",
    "cmp eax, 4096",
    "jae 1f",
    "imul rax, rax, 24",
    "lea r11, [r10 + rax + 24]",
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
    "test r10, r10",
    "jz 3f",
    "lock dec dword ptr [r10 + 16]",
    "3:",
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
    mode: BridgeMode,
    target: *mut u8,
    original: [u8; 8],
    patched: [u8; 8],
    // The property prefix stays executable until process exit. A thread may
    // have fetched the short jump before the entry is restored.
    prefix: Option<(*mut u8, [u8; PROPERTY_PREFIX_LEN])>,
    // Keep relay/trampoline allocated even after restoration. Another thread
    // could have entered the relay immediately before the entry was restored.
    _code: *mut u8,
}
unsafe impl Send for Hook {}

static HOOK: Mutex<Option<Hook>> = Mutex::new(None);
static PROPERTY_USED: AtomicBool = AtomicBool::new(false);

fn rel32_jump(from: usize, to: usize) -> Option<[u8; 5]> {
    let displacement = (to as i128) - (from as i128) - 5;
    let displacement = i32::try_from(displacement).ok()?;
    let mut jump = [0u8; 5];
    jump[0] = 0xe9;
    jump[1..].copy_from_slice(&displacement.to_le_bytes());
    Some(jump)
}

unsafe fn executable_near(jump_from: usize, target: usize, prologue_len: usize) -> Option<*mut u8> {
    // VirtualAlloc's address hint is allocation-granularity aligned. Both
    // rel32 jumps must fit; a failed or occupied hint only advances the scan.
    let base = target & !0xffffusize;
    for step in 1..=0x4000usize {
        let delta = step * 0x10000;
        for candidate in [base.checked_add(delta), base.checked_sub(delta)] {
            let Some(candidate) = candidate else { continue };
            if rel32_jump(jump_from, candidate).is_none()
                || rel32_jump(
                    candidate + TRAMPOLINE_OFFSET + prologue_len,
                    target + prologue_len,
                )
                .is_none()
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

unsafe fn exchange_property_entry(
    target: *mut u8,
    expected: [u8; 2],
    replacement: [u8; 2],
) -> Result<PatchOutcome, &'static str> {
    if !(target as usize).is_multiple_of(2) {
        return Err("property entry is not two-byte aligned");
    }
    let mut previous = 0u32;
    if unsafe { VirtualProtect(target.cast(), 2, PAGE_EXECUTE_READWRITE, &mut previous) } == 0 {
        return Err("cannot make property entry writable");
    }
    let exchanged = unsafe { &*target.cast::<AtomicU16>() }
        .compare_exchange(
            u16::from_le_bytes(expected),
            u16::from_le_bytes(replacement),
            Ordering::SeqCst,
            Ordering::SeqCst,
        )
        .is_ok();
    let mut replaced = 0u32;
    let protection_restored =
        unsafe { VirtualProtect(target.cast(), 2, previous, &mut replaced) } != 0;
    let cache_flushed =
        unsafe { FlushInstructionCache(GetCurrentProcess(), target.cast(), 2) } != 0;
    Ok(PatchOutcome {
        exchanged,
        protection_restored,
        cache_flushed,
    })
}

unsafe fn publish_property_prefix(
    slot: *mut u8,
    expected: [u8; PROPERTY_PREFIX_LEN],
    jump: [u8; PROPERTY_PREFIX_LEN],
) -> Result<(), &'static str> {
    let mut previous = 0u32;
    if unsafe {
        VirtualProtect(
            slot.cast(),
            PROPERTY_PREFIX_LEN,
            PAGE_EXECUTE_READWRITE,
            &mut previous,
        )
    } == 0
    {
        return Err("cannot make verified property prefix writable");
    }
    if unsafe { ptr::read(slot.cast::<[u8; PROPERTY_PREFIX_LEN]>()) } != expected {
        let mut replaced = 0u32;
        if unsafe { VirtualProtect(slot.cast(), PROPERTY_PREFIX_LEN, previous, &mut replaced) } == 0
        {
            return Err("critical: property prefix protection unverified");
        }
        return Err("verified property prefix changed before publication");
    }
    unsafe { ptr::copy_nonoverlapping(jump.as_ptr(), slot, PROPERTY_PREFIX_LEN) };
    let mut replaced = 0u32;
    let restored =
        unsafe { VirtualProtect(slot.cast(), PROPERTY_PREFIX_LEN, previous, &mut replaced) } != 0;
    let flushed =
        unsafe { FlushInstructionCache(GetCurrentProcess(), slot.cast(), PROPERTY_PREFIX_LEN) }
            != 0;
    if !restored
        || !flushed
        || unsafe { ptr::read(slot.cast::<[u8; PROPERTY_PREFIX_LEN]>()) } != jump
    {
        return Err("critical: property prefix publication unverified");
    }
    Ok(())
}

unsafe fn install_property_hotpatch(
    target: *mut u8,
    trace: *mut PropertyTrace,
    expected: [u8; 8],
) -> Result<(), &'static str> {
    if target.is_null()
        || trace.is_null()
        || !(target as usize).is_multiple_of(8)
        || (target as usize & (CODE_PAGE - 1)) < PROPERTY_PREFIX_LEN
    {
        return Err("unaligned or missing property hook input");
    }
    let mut guard = HOOK.lock().map_err(|_| "bridge lock poisoned")?;
    if guard.is_some() {
        return Err("bridge already installed");
    }
    let slot = unsafe { target.sub(PROPERTY_PREFIX_LEN) };
    if !committed_range(slot as usize, PROPERTY_PREFIX_LEN, 1, false) {
        return Err("property prefix is not fully readable");
    }
    let original = unsafe { ptr::read(target.cast::<[u8; 8]>()) };
    let prefix = unsafe { ptr::read(slot.cast::<[u8; PROPERTY_PREFIX_LEN]>()) };
    if original != expected || original != PROPERTY_PROLOGUE || prefix != PROPERTY_PREFIX_PAD {
        return Err("pinned property entry or five-byte prefix differs");
    }
    let code = unsafe { executable_near(slot as usize, target as usize, PROPERTY_FIRST_LEN) }
        .ok_or("no rel32 relay near property prefix")?;
    let relay = unsafe { code.add(RELAY_OFFSET) };
    let trampoline = unsafe { code.add(TRAMPOLINE_OFFSET) };
    let prefix_jump =
        rel32_jump(slot as usize, relay as usize).ok_or("property prefix relay out of range")?;
    let jump_back = rel32_jump(
        trampoline as usize + PROPERTY_FIRST_LEN,
        target as usize + PROPERTY_FIRST_LEN,
    )
    .ok_or("property trampoline return out of range")?;
    unsafe {
        ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), relay, 6);
        ptr::copy_nonoverlapping(
            (property_gateway as *const () as usize as u64)
                .to_le_bytes()
                .as_ptr(),
            relay.add(6),
            8,
        );
        ptr::copy_nonoverlapping(original.as_ptr(), trampoline, PROPERTY_FIRST_LEN);
        ptr::copy_nonoverlapping(jump_back.as_ptr(), trampoline.add(PROPERTY_FIRST_LEN), 5);
    }
    let mut previous = 0u32;
    if unsafe { VirtualProtect(code.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut previous) } == 0
        || unsafe { FlushInstructionCache(GetCurrentProcess(), code.cast(), CODE_PAGE) } == 0
    {
        unsafe { VirtualFree(code.cast(), 0, MEM_RELEASE) };
        return Err("cannot publish property relay and leaf trampoline");
    }
    PROPERTY_TRACE_PTR.store(trace, Ordering::Release);
    PROPERTY_TRAMPOLINE_PTR.store(trampoline, Ordering::Release);
    // The prefix was five INT3 padding bytes and the previous function ends
    // in RET. Publish the prefix first; no valid original path executes it.
    // Once the entry is switched, even a thread at entry+4 executes unchanged
    // original bytes. The prefix and allocations remain until game exit.
    unsafe { publish_property_prefix(slot, prefix, prefix_jump) }?;
    let old_first: [u8; 2] = original[..2]
        .try_into()
        .map_err(|_| "property entry width")?;
    let outcome = unsafe { exchange_property_entry(target, old_first, PROPERTY_SHORT_JUMP) }
        .map_err(|_| "critical: property entry exchange failed after prefix publication")?;
    if !outcome.exchanged {
        return Err("critical: property entry changed after prefix publication");
    }
    let mut patched = original;
    patched[..2].copy_from_slice(&PROPERTY_SHORT_JUMP);
    *guard = Some(Hook {
        mode: BridgeMode::Property,
        target,
        original,
        patched,
        prefix: Some((slot, prefix_jump)),
        _code: code,
    });
    if !outcome.protection_restored
        || !outcome.cache_flushed
        || unsafe { ptr::read(target.cast::<[u8; 8]>()) } != patched
    {
        return Err("critical: property entry publication unverified");
    }
    Ok(())
}

unsafe fn install_hook(
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
    let original = unsafe { ptr::read(target.cast::<[u8; 8]>()) };
    let prologue = &PINNED_PROLOGUE;
    if original != expected || &expected[..prologue.len()] != prologue {
        return Err("pinned GameCore entry bytes differ");
    }
    let code = unsafe { executable_near(target as usize, target as usize, prologue.len()) }
        .ok_or("no rel32 relay near entry")?;
    let relay = unsafe { code.add(RELAY_OFFSET) };
    let trampoline = unsafe { code.add(TRAMPOLINE_OFFSET) };
    let jump_to_relay = rel32_jump(target as usize, relay as usize).ok_or("relay out of range")?;
    let jump_back = rel32_jump(
        trampoline as usize + prologue.len(),
        target as usize + prologue.len(),
    )
    .ok_or("trampoline return out of range")?;
    let gateway = bridge_gateway as *const () as usize;
    unsafe {
        // FF 25 00000000 followed by the absolute gateway address.
        ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), relay, 6);
        ptr::copy_nonoverlapping((gateway as u64).to_le_bytes().as_ptr(), relay.add(6), 8);
        ptr::copy_nonoverlapping(original.as_ptr(), trampoline, prologue.len());
        ptr::copy_nonoverlapping(jump_back.as_ptr(), trampoline.add(prologue.len()), 5);
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
    patched[..5].copy_from_slice(&jump_to_relay);
    patched[5..prologue.len()].fill(0x90);
    let old_word = u64::from_le_bytes(original);
    let new_word = u64::from_le_bytes(patched);
    let outcome = unsafe { exchange_entry(target, old_word, new_word) }?;
    if !outcome.exchanged {
        // A prior invocation may still be paused before reading the global
        // trampoline pointer. Retain the new pointer and code until the
        // controller closes its owned game after this failed install.
        return Err("pinned entry changed before the atomic install");
    }
    *guard = Some(Hook {
        mode: BridgeMode::CanEver,
        target,
        original,
        patched,
        prefix: None,
        _code: code,
    });
    if !outcome.protection_restored || !outcome.cache_flushed {
        return Err("critical: installed entry protection or cache unverified");
    }
    Ok(())
}

unsafe fn install_at(
    target: *mut u8,
    trace: *mut SharedTrace,
    expected: [u8; 8],
) -> Result<(), &'static str> {
    if trace.is_null()
        || unsafe { (*trace).magic } != TRACE_MAGIC
        || unsafe { (*trace).version } != TRACE_VERSION
    {
        return Err("trace mapping version mismatch");
    }
    unsafe { install_hook(target, trace, expected) }
}

unsafe fn install_property_at(
    target: *mut u8,
    trace: *mut PropertyTrace,
    expected: [u8; 8],
) -> Result<(), &'static str> {
    if trace.is_null()
        || unsafe { (*trace).magic } != PROPERTY_TRACE_MAGIC
        || unsafe { (*trace).version } != PROPERTY_TRACE_VERSION
    {
        return Err("property trace mapping version mismatch");
    }
    unsafe { install_property_hotpatch(target, trace, expected) }
}

unsafe fn uninstall() -> Result<(), &'static str> {
    let mut guard = HOOK.lock().map_err(|_| "bridge lock poisoned")?;
    let hook = guard.as_ref().ok_or("bridge not installed")?;
    let outcome = match hook.mode {
        BridgeMode::CanEver => unsafe {
            exchange_entry(
                hook.target,
                u64::from_le_bytes(hook.patched),
                u64::from_le_bytes(hook.original),
            )
        }?,
        BridgeMode::Property => {
            let first: [u8; 2] = hook.original[..2]
                .try_into()
                .map_err(|_| "property entry width")?;
            unsafe { exchange_property_entry(hook.target, PROPERTY_SHORT_JUMP, first) }?
        }
    };
    if !outcome.exchanged
        || !outcome.protection_restored
        || !outcome.cache_flushed
        || unsafe { ptr::read(hook.target.cast::<[u8; 8]>()) } != hook.original
        || hook.prefix.is_some_and(|(slot, jump)| unsafe {
            ptr::read(slot.cast::<[u8; PROPERTY_PREFIX_LEN]>()) != jump
        })
    {
        return Err("pinned entry was not restored and verified");
    }
    // Property prefix remains a valid branch until game exit. A caller that
    // fetched EB F9 before restoration may still be about to enter it.
    // DLL, trace, relay and trampoline remain resident for that same period.
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

fn committed_range(address: usize, bytes: usize, alignment: usize, writable: bool) -> bool {
    if address == 0 || bytes == 0 || !address.is_multiple_of(alignment) {
        return false;
    }
    let Some(end) = address.checked_add(bytes) else {
        return false;
    };
    let mut cursor = address;
    while cursor < end {
        let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe {
            VirtualQuery(
                cursor as *const c_void,
                &mut info,
                std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
            )
        } != std::mem::size_of::<MEMORY_BASIC_INFORMATION>()
            || info.State != MEM_COMMIT
            || info.Protect & (PAGE_GUARD | PAGE_NOACCESS) != 0
        {
            return false;
        }
        let access = info.Protect & 0xff;
        let permitted = if writable {
            matches!(
                access,
                PAGE_READWRITE | PAGE_WRITECOPY | PAGE_EXECUTE_READWRITE | PAGE_EXECUTE_WRITECOPY
            )
        } else {
            matches!(
                access,
                PAGE_READONLY
                    | PAGE_READWRITE
                    | PAGE_WRITECOPY
                    | PAGE_EXECUTE_READ
                    | PAGE_EXECUTE_READWRITE
                    | PAGE_EXECUTE_WRITECOPY
            )
        };
        let base = info.BaseAddress as usize;
        let Some(region_end) = base.checked_add(info.RegionSize) else {
            return false;
        };
        if !permitted || cursor < base || region_end <= cursor {
            return false;
        }
        cursor = region_end.min(end);
    }
    true
}

fn pinned_gamecore(path: &Path, mode: BridgeMode) -> Option<[u8; 8]> {
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
    if mode == BridgeMode::Property {
        file.seek(SeekFrom::Start(
            PINNED_PROPERTY_GET_FILE_OFFSET - PROPERTY_PREFIX_LEN as u64,
        ))
        .ok()?;
        let mut pad = [0u8; PROPERTY_PREFIX_LEN];
        file.read_exact(&mut pad).ok()?;
        if pad != PROPERTY_PREFIX_PAD {
            return None;
        }
    }
    file.seek(SeekFrom::Start(mode.file_offset())).ok()?;
    let mut first = [0u8; 8];
    file.read_exact(&mut first).ok()?;
    (&first[..mode.prologue().len()] == mode.prologue()).then_some(first)
}

fn guarded_install(config: BridgeConfig) -> BridgeStatus {
    if config.version != CONFIG_VERSION
        || config.size as usize != std::mem::size_of::<BridgeConfig>()
        || config.game_pid == 0
        || config.created_filetime == 0
        || !(1..=MAX_ACTIVE_SECONDS).contains(&config.max_seconds)
        || config.trace_address == 0
        || !matches!(config.mode, 0 | 1)
        || config.trace_size as usize
            != if config.mode == 0 {
                BridgeMode::CanEver.trace_size()
            } else {
                BridgeMode::Property.trace_size()
            }
        || !committed_range(
            config.trace_address as usize,
            config.trace_size as usize,
            std::mem::align_of::<SharedTrace>(),
            true,
        )
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
    let mode = if config.mode == 0 {
        BridgeMode::CanEver
    } else {
        BridgeMode::Property
    };
    let Some(disk_bytes) = pinned_gamecore(Path::new(GAMECORE), mode) else {
        return BridgeStatus::GameCoreIdentity;
    };
    let Some(entry) = (module as usize).checked_add(mode.rva()) else {
        return BridgeStatus::EntryBytes;
    };
    let entry = entry as *mut u8;
    if unsafe { ptr::read(entry.cast::<[u8; 8]>()) } != disk_bytes {
        return BridgeStatus::EntryBytes;
    }
    if mode == BridgeMode::Property {
        if (entry as usize & (CODE_PAGE - 1)) < PROPERTY_PREFIX_LEN {
            return BridgeStatus::EntryBytes;
        }
        let prefix = unsafe { entry.sub(PROPERTY_PREFIX_LEN) };
        if !committed_range(prefix as usize, PROPERTY_PREFIX_LEN, 1, false)
            || unsafe { ptr::read(prefix.cast::<[u8; PROPERTY_PREFIX_LEN]>()) }
                != PROPERTY_PREFIX_PAD
        {
            return BridgeStatus::EntryBytes;
        }
        // The prefix remains published until the process exits; a second
        // property run in this process could redirect an older paused jump.
        if PROPERTY_USED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return BridgeStatus::HookFailure;
        }
    }
    let installed = match mode {
        BridgeMode::CanEver => {
            let trace = config.trace_address as *mut SharedTrace;
            unsafe { ptr::write(trace, SharedTrace::default()) };
            unsafe { install_at(entry, trace, disk_bytes) }
        }
        BridgeMode::Property => {
            let trace = config.trace_address as *mut PropertyTrace;
            unsafe { ptr::write(trace, PropertyTrace::default()) };
            unsafe { install_property_at(entry, trace, disk_bytes) }
        }
    };
    match installed {
        Ok(()) => BridgeStatus::Ok,
        Err(reason) if reason.starts_with("critical:") => BridgeStatus::Critical,
        Err(_) => BridgeStatus::HookFailure,
    }
}

/// Called on a bounded controller-created thread after LoadLibraryW returns.
/// Merely loading the DLL has no side effects on the game.
#[unsafe(no_mangle)]
pub extern "system" fn BridgeInstall(config: *mut c_void) -> u32 {
    if !committed_range(
        config as usize,
        std::mem::size_of::<BridgeConfig>(),
        std::mem::align_of::<BridgeConfig>(),
        false,
    ) {
        return BridgeStatus::InvalidInput as u32;
    }
    std::panic::catch_unwind(|| {
        let config = unsafe { ptr::read_unaligned(config.cast::<BridgeConfig>()) };
        guarded_install(config) as u32
    })
    .unwrap_or(BridgeStatus::Panic as u32)
}

/// Restores and verifies the original entry. Property mode retains its
/// pre-entry branch because a thread may already have fetched the short jump.
/// The DLL, trace and relay remain mapped until game exit. A critical result
/// requires the controller to close that owned game.
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
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry;
    use windows_sys::Win32::System::Threading::{
        CreateThread, GetExitCodeThread, ResumeThread, WaitForSingleObject, THREAD_CREATE_SUSPENDED,
    };

    static HOOK_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn property_gateway_has_win64_unwind_metadata() {
        let mut image_base = 0u64;
        let entry = unsafe {
            RtlLookupFunctionEntry(
                property_gateway as *const () as u64,
                &mut image_base,
                null_mut(),
            )
        };
        assert!(!entry.is_null(), "stack-changing gateway needs pdata/xdata");
        assert_ne!(image_base, 0);
        let rva = property_gateway as *const () as u64 - image_base;
        assert!(unsafe { (*entry).BeginAddress as u64 } <= rva);
        assert!(rva < unsafe { (*entry).EndAddress as u64 });
        let unwind_address = image_base + unsafe { (*entry).Anonymous.UnwindData as u64 };
        let unwind = unsafe { ptr::read(unwind_address as *const [u8; 6]) };
        assert_eq!(unwind[0] & 0x07, 1); // UNWIND_INFO version 1
        assert_eq!(unwind[2], 1); // one unwind operation
        assert_eq!(unwind[4], 4); // four-byte sub rsp, 0x68
        assert_eq!(unwind[5], 0xc2); // UWOP_ALLOC_SMALL: 12*8+8 = 0x68
    }

    #[test]
    fn suspended_second_instruction_and_concurrent_callers_survive_property_patch_cycle() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        let page = unsafe {
            VirtualAlloc(
                null_mut(),
                CODE_PAGE,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            )
        }
        .cast::<u8>();
        assert!(!page.is_null());
        let target = unsafe { page.add(16) };
        let slot = unsafe { target.sub(PROPERTY_PREFIX_LEN) };
        unsafe {
            ptr::copy_nonoverlapping(PROPERTY_PREFIX_PAD.as_ptr(), slot, PROPERTY_PREFIX_LEN)
        };
        // mov rax,[rcx+40h]; lea r9,[rcx+30h]; mov eax,r9d; ret.
        // A thread starting at +4 proves the second instruction remains whole.
        let bytes = [
            0x48, 0x8b, 0x41, 0x40, 0x4c, 0x8d, 0x49, 0x30, 0x44, 0x89, 0xc8, 0xc3,
        ];
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len()) };
        let mut property = [0u64; 10];
        let argument = property.as_mut_ptr() as usize;
        let expected = (argument + 0x30) as u32;
        let second: unsafe extern "system" fn(*mut c_void) -> u32 =
            unsafe { std::mem::transmute(target.add(PROPERTY_FIRST_LEN)) };
        let mut thread_id = 0u32;
        let paused = unsafe {
            CreateThread(
                null_mut(),
                0,
                Some(second),
                argument as *const c_void,
                THREAD_CREATE_SUSPENDED,
                &mut thread_id,
            )
        };
        assert!(!paused.is_null() && thread_id != 0);
        let running = Arc::new(AtomicBool::new(true));
        let workers = (0..4)
            .map(|_| {
                let running = Arc::clone(&running);
                let address = target as usize;
                std::thread::spawn(move || {
                    let first: extern "system" fn(usize, u32) -> u32 =
                        unsafe { std::mem::transmute(address) };
                    let second: extern "system" fn(usize) -> u32 =
                        unsafe { std::mem::transmute(address + PROPERTY_FIRST_LEN) };
                    let mut calls = 0u32;
                    while running.load(Ordering::Acquire) && calls < 100_000 {
                        assert_eq!(first(argument, 0), expected);
                        assert_eq!(second(argument), expected);
                        calls += 1;
                    }
                    calls
                })
            })
            .collect::<Vec<_>>();
        std::thread::sleep(Duration::from_millis(5));
        let mut trace = Box::new(PropertyTrace::default());
        unsafe { install_property_at(target, trace.as_mut(), PROPERTY_PROLOGUE) }.unwrap();
        assert_eq!(
            unsafe { ptr::read(target.cast::<[u8; 8]>()) }[2..],
            bytes[2..8]
        );
        assert_eq!(unsafe { ResumeThread(paused) }, 1);
        assert_eq!(unsafe { WaitForSingleObject(paused, 5_000) }, WAIT_OBJECT_0);
        let mut exit_code = 0u32;
        assert_ne!(unsafe { GetExitCodeThread(paused, &mut exit_code) }, 0);
        assert_eq!(exit_code, expected);
        assert_ne!(unsafe { CloseHandle(paused) }, 0);
        unsafe { uninstall() }.unwrap();
        running.store(false, Ordering::Release);
        assert!(workers.into_iter().map(|w| w.join().unwrap()).sum::<u32>() > 0);
        assert_eq!(
            unsafe { ptr::read(target.cast::<[u8; 8]>()) },
            PROPERTY_PROLOGUE
        );
        assert_eq!(
            unsafe { ptr::read(slot.cast::<[u8; PROPERTY_PREFIX_LEN]>()) }[0],
            0xe9
        );
        assert_ne!(unsafe { VirtualFree(page.cast(), 0, MEM_RELEASE) }, 0);
    }

    #[test]
    fn property_trampoline_calls_original_once_and_preserves_pointer_or_missing() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<PropertyRow>(), 48);
        assert_eq!(std::mem::offset_of!(PropertyTrace, rows), 24);
        let page = unsafe {
            VirtualAlloc(
                null_mut(),
                CODE_PAGE,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            )
        }
        .cast::<u8>();
        assert!(!page.is_null());
        let target = unsafe { page.add(16) };
        let slot = unsafe { target.sub(PROPERTY_PREFIX_LEN) };
        unsafe {
            ptr::copy_nonoverlapping(PROPERTY_PREFIX_PAD.as_ptr(), slot, PROPERTY_PREFIX_LEN)
        };
        // Exactly the pinned two-instruction prologue, then an owned counter
        // and key-dependent return. This fixture never invokes a game method.
        let bytes = [
            0x48, 0x8b, 0x41, 0x40, 0x4c, 0x8d, 0x49, 0x30, 0xff, 0x41, 0x48, 0x85, 0xd2, 0x75,
            0x01, 0xc3, 0x31, 0xc0, 0xc3,
        ];
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len()) };
        let f: extern "system" fn(usize, u32) -> usize = unsafe { std::mem::transmute(target) };
        let mut object = [0u64; 10];
        let mut value = [0u8; 24];
        value[..2].copy_from_slice(&1u16.to_le_bytes());
        value[8] = 1;
        value[20..24].copy_from_slice(&0x2000_0000u32.to_le_bytes());
        object[8] = value.as_ptr() as u64;
        let property = object.as_mut_ptr() as usize;
        let mut trace = Box::new(PropertyTrace::default());
        assert!(unsafe {
            install_property_at(target, trace.as_mut(), bytes[..8].try_into().unwrap())
        }
        .is_ok());
        let trampoline = PROPERTY_TRAMPOLINE_PTR.load(Ordering::Acquire);
        assert!(!trampoline.is_null());
        let leaf = unsafe { ptr::read(trampoline.cast::<[u8; 9]>()) };
        assert_eq!(
            &leaf[..PROPERTY_FIRST_LEN],
            &PROPERTY_PROLOGUE[..PROPERTY_FIRST_LEN]
        );
        assert_eq!(leaf[PROPERTY_FIRST_LEN], 0xe9); // direct jump back, no stack change
        let mut base = 0u64;
        assert!(
            unsafe { RtlLookupFunctionEntry(trampoline as u64, &mut base, null_mut()) }.is_null()
        );
        assert_eq!(f(property, 0), value.as_ptr() as usize);
        assert_eq!(f(property, 1), 0);
        object[8] = 1; // Valid original pointer value, unreadable for the copy.
        assert_eq!(f(property, 0), 1);
        assert_eq!(object[9], 3, "the original ran exactly once per call");
        let rows = trace.complete_rows().unwrap();
        assert_eq!(rows.len(), 3);
        assert!(rows
            .iter()
            .all(|r| r.property == property as u64 && r.thread_id != 0));
        assert_eq!(
            (
                rows[0].key_id,
                rows[0].returned,
                rows[0].read_state,
                rows[0].tag,
                rows[0].payload,
                rows[0].flags
            ),
            (0, value.as_ptr() as u64, 1, 1, 1, 0x2000_0000)
        );
        assert_eq!(
            (rows[1].key_id, rows[1].returned, rows[1].read_state),
            (1, 0, 0)
        );
        assert_eq!(
            (rows[2].key_id, rows[2].returned, rows[2].read_state),
            (0, 1, 2)
        );
        for _ in 0..TRACE_CAPACITY {
            assert_eq!(f(property, 1), 0);
        }
        unsafe { uninstall() }.unwrap();
        assert_eq!(PROPERTY_TRAMPOLINE_PTR.load(Ordering::Acquire), trampoline);
        assert_eq!(
            PROPERTY_TRACE_PTR.load(Ordering::Acquire),
            trace.as_mut() as *mut PropertyTrace
        );
        let mut region: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe {
                VirtualQuery(
                    trampoline.cast(),
                    &mut region,
                    std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
                )
            },
            std::mem::size_of::<MEMORY_BASIC_INFORMATION>()
        );
        assert_eq!(region.State, MEM_COMMIT);
        assert_eq!(region.Protect & 0xff, PAGE_EXECUTE_READ);
        assert_eq!(
            trace.next.load(Ordering::Acquire),
            TRACE_CAPACITY as u32 + 3
        );
        assert_eq!(trace.dropped.load(Ordering::Acquire), 3);
        assert_eq!(trace.snapshot_rows().unwrap().len(), TRACE_CAPACITY);
        assert!(trace.complete_rows().is_err());
        assert_eq!(unsafe { ptr::read(target.cast::<[u8; 8]>()) }, bytes[..8]);
        assert_eq!(
            unsafe { ptr::read(slot.cast::<[u8; PROPERTY_PREFIX_LEN]>()) }[0],
            0xe9
        );
        assert_ne!(unsafe { VirtualFree(page.cast(), 0, MEM_RELEASE) }, 0);
    }

    #[test]
    fn property_hook_rejects_partial_or_changed_prologue_before_mutation() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        let page = unsafe {
            VirtualAlloc(
                null_mut(),
                CODE_PAGE,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            )
        }
        .cast::<u8>();
        assert!(!page.is_null());
        let target = unsafe { page.add(16) };
        let slot = unsafe { target.sub(PROPERTY_PREFIX_LEN) };
        unsafe {
            ptr::copy_nonoverlapping(PROPERTY_PREFIX_PAD.as_ptr(), slot, PROPERTY_PREFIX_LEN)
        };
        let mut wrong = PROPERTY_PROLOGUE;
        wrong[7] ^= 1;
        unsafe { ptr::copy_nonoverlapping(wrong.as_ptr(), target, wrong.len()) };
        let mut trace = Box::new(PropertyTrace::default());
        assert!(unsafe { install_property_at(target, trace.as_mut(), wrong) }.is_err());
        assert_eq!(unsafe { ptr::read(target.cast::<[u8; 8]>()) }, wrong);
        assert_eq!(
            unsafe { ptr::read(slot.cast::<[u8; PROPERTY_PREFIX_LEN]>()) },
            PROPERTY_PREFIX_PAD
        );
        unsafe {
            ptr::copy_nonoverlapping(PROPERTY_PROLOGUE.as_ptr(), target, PROPERTY_PROLOGUE.len())
        };
        unsafe { *slot = 0x90 };
        assert!(unsafe { install_property_at(target, trace.as_mut(), PROPERTY_PROLOGUE) }.is_err());
        assert_eq!(
            unsafe { ptr::read(target.cast::<[u8; 8]>()) },
            PROPERTY_PROLOGUE
        );
        assert_ne!(unsafe { VirtualFree(page.cast(), 0, MEM_RELEASE) }, 0);
    }

    #[test]
    fn gateway_preserves_original_result_and_captures_actual_call_arguments() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<TraceRow>(), 24);
        assert_eq!(std::mem::offset_of!(SharedTrace, rows), 24);
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
        assert!(trace.complete_rows().is_err());
        unsafe { uninstall() }.unwrap();
        assert_eq!(trace.in_flight.load(Ordering::Acquire), 0);
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
        assert_eq!(trace.in_flight.load(Ordering::Acquire), 0);
        if trace.dropped.load(Ordering::Acquire) == 0 {
            assert_eq!(
                trace.complete_rows().unwrap().len(),
                trace.next.load(Ordering::Acquire) as usize
            );
        } else {
            assert!(trace.complete_rows().is_err());
        }
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
            BridgeStatus::InvalidInput as u32
        );
    }

    #[test]
    fn install_rejects_bad_config_and_trace_ranges_before_dereference() {
        assert_eq!(
            BridgeInstall(1usize as *mut c_void),
            BridgeStatus::InvalidInput as u32
        );
        assert_eq!(
            BridgeInstall(usize::MAX as *mut c_void),
            BridgeStatus::InvalidInput as u32
        );
        let mut config = BridgeConfig::new(1, 1, 1);
        assert_eq!(
            BridgeInstall((&mut config as *mut BridgeConfig).cast()),
            BridgeStatus::InvalidInput as u32
        );
        let mut property = BridgeConfig::for_property(1, 1, 1);
        assert_eq!(
            property.trace_size as usize,
            std::mem::size_of::<PropertyTrace>()
        );
        assert_eq!(
            BridgeInstall((&mut property as *mut BridgeConfig).cast()),
            BridgeStatus::InvalidInput as u32
        );
        property.mode = 2;
        assert_eq!(
            BridgeInstall((&mut property as *mut BridgeConfig).cast()),
            BridgeStatus::InvalidInput as u32
        );
        let page = unsafe {
            VirtualAlloc(
                null_mut(),
                CODE_PAGE,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
        };
        assert!(!page.is_null());
        config.trace_address = page as u64;
        assert!(!committed_range(
            page as usize,
            std::mem::size_of::<SharedTrace>(),
            std::mem::align_of::<SharedTrace>(),
            true
        ));
        assert_eq!(
            BridgeInstall((&mut config as *mut BridgeConfig).cast()),
            BridgeStatus::InvalidInput as u32
        );
        let mut previous = 0u32;
        assert_ne!(
            unsafe { VirtualProtect(page, CODE_PAGE, PAGE_READONLY, &mut previous) },
            0
        );
        assert!(!committed_range(
            page as usize,
            CODE_PAGE,
            std::mem::align_of::<SharedTrace>(),
            true
        ));
        assert!(committed_range(
            page as usize,
            CODE_PAGE,
            std::mem::align_of::<BridgeConfig>(),
            false
        ));
        assert_ne!(unsafe { VirtualFree(page, 0, MEM_RELEASE) }, 0);
        let misaligned = ((&config as *const BridgeConfig as usize) + 1) as *mut c_void;
        assert_eq!(BridgeInstall(misaligned), BridgeStatus::InvalidInput as u32);
    }

    #[test]
    fn complete_trace_refuses_unfinished_slots_and_inflight_calls() {
        let trace = SharedTrace::default();
        trace.next.store(1, Ordering::Release);
        assert!(trace.complete_rows().is_err());
        trace.rows[0].sequence.store(1, Ordering::Release);
        assert_eq!(trace.complete_rows().unwrap().len(), 1);
        trace.in_flight.store(1, Ordering::Release);
        assert!(trace.complete_rows().is_err());
        trace.in_flight.store(0, Ordering::Release);
        trace.dropped.store(1, Ordering::Release);
        assert!(trace.complete_rows().is_err());
    }
}
