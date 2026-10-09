use crate::PINNED_GAMECORE_SHA256;
use core::arch::global_asm;
use sha2::{Digest, Sha256};
use std::cell::UnsafeCell;
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

#[path = "windows/assignment.rs"]
mod assignment;
#[path = "windows/composite.rs"]
mod composite;
pub use composite::CompositeTrace;
#[path = "windows/runtime.rs"]
mod runtime;
#[path="windows/trace_init.rs"]
mod trace_init;
#[path="windows/install_progress.rs"]
mod install_progress;
#[path = "windows/ai_runtime.rs"]
mod ai_runtime;
#[path = "windows/ai_observation.rs"]
mod ai_observation;
pub use ai_observation::AiCapture;
#[path = "windows/ai_operation.rs"]
mod ai_operation;
pub use ai_runtime::{AiObservationConfig,AiRuntimeState,AI_OBSERVATION_VERSION};
pub use runtime::RUNTIME_CONFIG_VERSION;
#[path = "windows/paid_assignment.rs"]
mod paid_assignment;
use assignment::{
    assign_calibration_gateway, install_assign_calibration_at, ASSIGN_FILE_OFFSET,
    ASSIGN_TRACE_PTR, ASSIGN_TRAMPOLINE_PTR,
};
pub use assignment::{AssignCalibrationObservation, AssignCalibrationTrace, PINNED_CAN_ASSIGN_RVA};
pub use paid_assignment::{
    PaidAssignmentConsumer, PaidAssignmentRead, PAID_ALLOW_BASE, PAID_CLOSE_BASE,
    PAID_MANIFEST_BASE, PAID_RESET_BASE,
};

pub const TRACE_CAPACITY: usize = 4096;
const TRACE_MAGIC: u32 = 0x4a41_4342; // JACB
const TRACE_VERSION: u32 = 2;
const PROPERTY_TRACE_MAGIC: u32 = 0x4a41_4350; // JACP
const PROPERTY_TRACE_VERSION: u32 = 3;
const CALIBRATION_TRACE_MAGIC: u32 = 0x4a41_4341; // JACA
const CALIBRATION_TRACE_VERSION: u32 = 4;
pub const CALIBRATION_CAPACITY: usize = 1024;
pub const PROPERTY_TEST_VALUE_KEY_ID: u32 = 853_271;
pub const PROPERTY_TEST_PHASE_KEY_ID: u32 = 853_272;
pub const ASHINA_ADMISSION_KEY_ID: u32 = 853_304;
pub const PRODUCTION_MANIFEST_BASE: u32 = 900_000;
pub const PRODUCTION_ALLOW_BASE: u32 = 901_000;
pub const PRODUCTION_RESET_KEY: u32 = 902_000;
pub const PRODUCTION_RESET_LAST_KEY: u32 = PRODUCTION_RESET_KEY + PRODUCTION_OWNER_LIMIT as u32 - 1;
pub const PRODUCTION_CLOSE_BASE: u32 = 903_000;
pub const PRODUCTION_INDEX_LIMIT: usize = 256;
pub const PRODUCTION_OWNER_LIMIT: usize = 64;
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
pub const PINNED_CAN_EVER_RVA: usize = 0x2d_ddd0;
const PINNED_CAN_EVER_FILE_OFFSET: u64 = 0x2dd1d0;
pub const PINNED_PROPERTY_GET_RVA: usize = 0x99_f5c0;
const PINNED_PROPERTY_GET_FILE_OFFSET: u64 = 0x99_e9c0;
pub const PINNED_PROMOTION_CALL_RVA: usize = 0x30_ec0f;
pub const PINNED_PROMOTION_CALLEE_RVA: usize = 0x2d_dbb0;
const PINNED_PROMOTION_CALL_FILE_OFFSET: u64 = 0x30_e00f;
const PROMOTION_CALL_WINDOW: [u8; 16] = [
    0xe8, 0x9c, 0xef, 0xfc, 0xff, 0x84, 0xc0, 0x0f, 0x84, 0x75, 0x01, 0x00, 0x00, 0x44, 0x8b, 0xc7,
];
const PROMOTION_CALL_HEAD: [u8; 8] = [0xe8, 0x9c, 0xef, 0xfc, 0xff, 0x84, 0xc0, 0x0f];
const PROMOTION_TRACE_MAGIC: u32 = 0x4a41_4338; // JAC8
const PROMOTION_TRACE_VERSION: u32 = 1;
pub const PROMOTION_TRACE_CAPACITY: usize = 2048;
pub const GAME_EXE: &str = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/Base/Binaries/Win64Steam/CivilizationVI.exe";
pub const GAMECORE: &str = "C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll";
const CONFIG_VERSION: u32 = 1;
pub const EXPERIMENTAL_PAID_CONFIG_VERSION: u32 = 0x0853_0001;
use crate::MAX_ACTIVE_SECONDS;

/// AMD64 APM 3.11 permits an asynchronous single atomic store to a code
/// region wholly within an aligned eight-byte word. The fixed operand meets
/// that condition. Intel's documented cross-modifying-code contract differs,
/// so this optional mode refuses other CPU vendors before any mutation.
pub fn supports_atomic_promotion_fetch() -> bool {
    let leaf = core::arch::x86_64::__cpuid(0);
    let mut vendor = [0u8; 12];
    vendor[..4].copy_from_slice(&leaf.ebx.to_le_bytes());
    vendor[4..8].copy_from_slice(&leaf.edx.to_le_bytes());
    vendor[8..].copy_from_slice(&leaf.ecx.to_le_bytes());
    &vendor == b"AuthenticAMD"
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum BridgeMode {
    CanEver = 0,
    Property = 1,
    PropertyScoped = 2,
    CanEverCalibration = 3,
    PropertyAdmissionScoped = 4,
    JoinedAdmission = 5,
    ProductionAdmission = 6,
    AssignCalibration = 7,
    PromotionCall = 8,
}

impl BridgeMode {
    pub const fn is_property(self) -> bool {
        matches!(
            self,
            Self::Property | Self::PropertyScoped | Self::PropertyAdmissionScoped
        )
    }

    pub const fn rva(self) -> usize {
        match self {
            Self::CanEver
            | Self::CanEverCalibration
            | Self::JoinedAdmission
            | Self::ProductionAdmission => PINNED_CAN_EVER_RVA,
            Self::AssignCalibration => PINNED_CAN_ASSIGN_RVA,
            Self::Property | Self::PropertyScoped | Self::PropertyAdmissionScoped => {
                PINNED_PROPERTY_GET_RVA
            }
            Self::PromotionCall => PINNED_PROMOTION_CALL_RVA,
        }
    }

    const fn file_offset(self) -> u64 {
        match self {
            Self::CanEver
            | Self::CanEverCalibration
            | Self::JoinedAdmission
            | Self::ProductionAdmission => PINNED_CAN_EVER_FILE_OFFSET,
            Self::AssignCalibration => ASSIGN_FILE_OFFSET,
            Self::Property | Self::PropertyScoped | Self::PropertyAdmissionScoped => {
                PINNED_PROPERTY_GET_FILE_OFFSET
            }
            Self::PromotionCall => PINNED_PROMOTION_CALL_FILE_OFFSET,
        }
    }

    const fn prologue(self) -> &'static [u8] {
        match self {
            Self::CanEver
            | Self::CanEverCalibration
            | Self::JoinedAdmission
            | Self::ProductionAdmission => &PINNED_PROLOGUE,
            Self::AssignCalibration => &PINNED_PROLOGUE,
            Self::Property | Self::PropertyScoped | Self::PropertyAdmissionScoped => {
                &PROPERTY_PROLOGUE
            }
            Self::PromotionCall => &PROMOTION_CALL_HEAD,
        }
    }

    pub const fn trace_size(self) -> usize {
        match self {
            Self::AssignCalibration => std::mem::size_of::<AssignCalibrationTrace>(),
            Self::CanEver => std::mem::size_of::<SharedTrace>(),
            Self::CanEverCalibration => std::mem::size_of::<CalibrationTrace>(),
            Self::JoinedAdmission | Self::ProductionAdmission => std::mem::size_of::<JoinedTrace>(),
            Self::Property | Self::PropertyScoped | Self::PropertyAdmissionScoped => {
                std::mem::size_of::<PropertyTrace>()
            }
            Self::PromotionCall => std::mem::size_of::<PromotionTrace>(),
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

    pub fn for_property_scoped(game_pid: u32, created_filetime: u64, trace_address: u64) -> Self {
        let mut config = Self::for_property(game_pid, created_filetime, trace_address);
        config.mode = BridgeMode::PropertyScoped as u32;
        config
    }

    pub fn for_calibration(game_pid: u32, created_filetime: u64, trace_address: u64) -> Self {
        let mut config = Self::new(game_pid, created_filetime, trace_address);
        config.mode = BridgeMode::CanEverCalibration as u32;
        config.trace_size = BridgeMode::CanEverCalibration.trace_size() as u32;
        config
    }

    pub fn for_admission_property(
        game_pid: u32,
        created_filetime: u64,
        trace_address: u64,
    ) -> Self {
        let mut config = Self::for_property(game_pid, created_filetime, trace_address);
        config.mode = BridgeMode::PropertyAdmissionScoped as u32;
        config
    }

    pub fn for_assign_calibration(
        game_pid: u32,
        created_filetime: u64,
        trace_address: u64,
    ) -> Self {
        let mut config = Self::new(game_pid, created_filetime, trace_address);
        config.mode = BridgeMode::AssignCalibration as u32;
        config.trace_size = BridgeMode::AssignCalibration.trace_size() as u32;
        config
    }

    pub fn for_joined(game_pid: u32, created_filetime: u64, trace_address: u64) -> Self {
        let mut config = Self::new(game_pid, created_filetime, trace_address);
        config.mode = BridgeMode::JoinedAdmission as u32;
        config.trace_size = BridgeMode::JoinedAdmission.trace_size() as u32;
        config
    }

    pub fn for_production(game_pid: u32, created_filetime: u64, trace_address: u64) -> Self {
        let mut config = Self::for_joined(game_pid, created_filetime, trace_address);
        config.mode = BridgeMode::ProductionAdmission as u32;
        config
    }
    pub fn for_promotion_call(game_pid: u32, created_filetime: u64, trace_address: u64) -> Self {
        let mut config = Self::new(game_pid, created_filetime, trace_address);
        config.mode = BridgeMode::PromotionCall as u32;
        config.trace_size = BridgeMode::PromotionCall.trace_size() as u32;
        config
    }
}

/// Entry-only raw Win64 register labels. No owner, index, ABI type, or return
/// value is inferred from this call site.
#[repr(C)]
#[derive(Default)]
pub struct PromotionRow {
    pub sequence: AtomicU32,
    pub thread_id: AtomicU32,
    pub rcx: AtomicU64,
    pub rdx: AtomicU64,
    pub r8: AtomicU64,
    pub r9: AtomicU64,
    pub return_address: AtomicU64,
}

#[repr(C)]
pub struct PromotionTrace {
    pub magic: u32,
    pub version: u32,
    pub mode: u32,
    pub capacity: u32,
    pub next: AtomicU32,
    pub dropped: AtomicU32,
    pub in_flight: AtomicU32,
    pub reserved: u32,
    pub rows: [PromotionRow; PROMOTION_TRACE_CAPACITY],
}

impl Default for PromotionTrace {
    fn default() -> Self {
        Self {
            magic: PROMOTION_TRACE_MAGIC,
            version: PROMOTION_TRACE_VERSION,
            mode: BridgeMode::PromotionCall as u32,
            capacity: PROMOTION_TRACE_CAPACITY as u32,
            next: AtomicU32::new(0),
            dropped: AtomicU32::new(0),
            in_flight: AtomicU32::new(0),
            reserved: 0,
            rows: std::array::from_fn(|_| PromotionRow::default()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PromotionObservation {
    pub sequence: u32,
    pub thread_id: u32,
    pub rcx: u64,
    pub rdx: u64,
    pub r8: u64,
    pub r9: u64,
    pub return_address: u64,
}

impl PromotionTrace {
    pub const VERSION: u32 = PROMOTION_TRACE_VERSION;

    pub fn complete_rows(&self) -> Result<Vec<PromotionObservation>, &'static str> {
        if self.magic != PROMOTION_TRACE_MAGIC
            || self.version != PROMOTION_TRACE_VERSION
            || self.mode != BridgeMode::PromotionCall as u32
            || self.capacity != PROMOTION_TRACE_CAPACITY as u32
            || self.reserved != 0
        {
            return Err("promotion trace header mismatch");
        }
        if self.in_flight.load(Ordering::Acquire) != 0 {
            return Err("promotion gateway calls remain in flight");
        }
        if self.dropped.load(Ordering::Acquire) != 0 {
            return Err("promotion trace overflow");
        }
        let count = self.next.load(Ordering::Acquire) as usize;
        if count > PROMOTION_TRACE_CAPACITY {
            return Err("promotion sequence exceeds capacity");
        }
        self.rows[..count]
            .iter()
            .enumerate()
            .map(|(i, row)| {
                if row.sequence.load(Ordering::Acquire) != i as u32 + 1 {
                    return Err("partial promotion row");
                }
                Ok(PromotionObservation {
                    sequence: i as u32 + 1,
                    thread_id: row.thread_id.load(Ordering::Relaxed),
                    rcx: row.rcx.load(Ordering::Relaxed),
                    rdx: row.rdx.load(Ordering::Relaxed),
                    r8: row.r8.load(Ordering::Relaxed),
                    r9: row.r9.load(Ordering::Relaxed),
                    return_address: row.return_address.load(Ordering::Relaxed),
                })
            })
            .collect()
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
    pub const VERSION: u32 = TRACE_VERSION;
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

/// One original CanEver call. Field addresses are copied while its caller is
/// still on the stack; they are observations, never retained as admission state.
#[repr(C)]
#[derive(Default)]
pub struct CalibrationRow {
    pub sequence: AtomicU32,
    pub governor_index: AtomicU32,
    pub thread_id: AtomicU32,
    /// 1: all owner fields copied; 0: null owner; 2: unreadable field.
    pub owner_read_state: AtomicU32,
    pub governors_this: AtomicU64,
    pub return_address: AtomicU64,
    pub player_address: AtomicU64,
    pub player_id: AtomicU32,
    pub original_al: AtomicU32,
    pub effective_al: AtomicU32,
    pub property_receiver: AtomicU64,
    pub joined_session: AtomicU32,
    pub entry_ticket: AtomicU32,
}

#[repr(C)]
pub struct CalibrationTrace {
    pub magic: u32,
    pub version: u32,
    pub next: AtomicU32,
    pub dropped: AtomicU32,
    pub in_flight: AtomicU32,
    pub reserved: u32,
    pub rows: [CalibrationRow; CALIBRATION_CAPACITY],
}

#[repr(C)]
pub struct JoinedTrace {
    pub calibration: CalibrationTrace,
    pub property: PropertyTrace,
}

impl Default for JoinedTrace {
    fn default() -> Self {
        Self {
            calibration: CalibrationTrace::default(),
            property: PropertyTrace::joined_scoped(),
        }
    }
}

impl Default for CalibrationTrace {
    fn default() -> Self {
        Self {
            magic: CALIBRATION_TRACE_MAGIC,
            version: CALIBRATION_TRACE_VERSION,
            next: AtomicU32::new(0),
            dropped: AtomicU32::new(0),
            in_flight: AtomicU32::new(0),
            reserved: 0,
            rows: std::array::from_fn(|_| CalibrationRow::default()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CalibrationObservation {
    pub sequence: u32,
    pub governor_index: u32,
    pub thread_id: u32,
    pub owner_read_state: u32,
    pub governors_this: u64,
    pub return_address: u64,
    pub player_address: u64,
    pub player_id: u32,
    pub property_receiver: u64,
    pub original_al: u32,
    pub effective_al: u32,
}

impl CalibrationTrace {
    pub const VERSION: u32 = CALIBRATION_TRACE_VERSION;
    pub fn complete_rows(&self) -> Result<Vec<CalibrationObservation>, &'static str> {
        if self.in_flight.load(Ordering::Acquire) != 0 {
            return Err("calibration calls remain in flight");
        }
        if self.reserved != BridgeMode::ProductionAdmission as u32
            && self.dropped.load(Ordering::Acquire) != 0
        {
            return Err("calibration capacity exceeded");
        }
        let observed = self.next.load(Ordering::Acquire) as usize;
        let next = observed.min(CALIBRATION_CAPACITY);
        if observed.saturating_sub(CALIBRATION_CAPACITY)
            != self.dropped.load(Ordering::Acquire) as usize
        {
            return Err("calibration sequence exceeded capacity");
        }
        let mut rows = Vec::with_capacity(next);
        for (index, row) in self.rows[..next].iter().enumerate() {
            if row.sequence.load(Ordering::Acquire) != 1 {
                return Err("calibration call has no original return");
            }
            rows.push(CalibrationObservation {
                sequence: index as u32 + 1,
                governor_index: row.governor_index.load(Ordering::Relaxed),
                thread_id: row.thread_id.load(Ordering::Relaxed),
                owner_read_state: row.owner_read_state.load(Ordering::Relaxed),
                governors_this: row.governors_this.load(Ordering::Relaxed),
                return_address: row.return_address.load(Ordering::Relaxed),
                player_address: row.player_address.load(Ordering::Relaxed),
                player_id: row.player_id.load(Ordering::Relaxed),
                property_receiver: row.property_receiver.load(Ordering::Relaxed),
                original_al: row.original_al.load(Ordering::Relaxed),
                effective_al: row.effective_al.load(Ordering::Relaxed),
            });
        }
        Ok(rows)
    }
}

fn copy_current<const N: usize>(address: u64) -> Option<[u8; N]> {
    if address == 0 {
        return None;
    }
    let mut bytes = [0u8; N];
    let mut read = 0usize;
    let ok = unsafe {
        ReadProcessMemory(
            GetCurrentProcess(),
            address as *const c_void,
            bytes.as_mut_ptr().cast(),
            N,
            &mut read,
        )
    } != 0;
    (ok && read == N).then_some(bytes)
}

// One compare-exchanged word contains the entire admission decision:
// Core receiver (low 64), session (next 32), getter/bind ticket (next 31),
// and permission (top bit). A pointer change and its fail-closed permission
// are therefore indivisible. Tickets are assigned before the native getter;
// a late completion cannot overtake a newer false or a receiver change.
const JOINED_TICKET_MAX: u32 = 0x7fff_ffff;

// Stable Rust 1.95 has no AtomicU128 API. Win64's cmpxchg16b publishes the
// receiver, session, ticket, and bit as one aligned value. RBX is preserved and
// the helper has unwind metadata because it changes the stack.
global_asm!(
    ".globl joined_cas128",
    ".seh_proc joined_cas128",
    "joined_cas128:",
    "push rbx",
    ".seh_pushreg rbx",
    ".seh_endprologue",
    "mov r10, rcx",
    "mov rax, rdx",
    "mov rdx, r8",
    "mov rbx, r9",
    "mov rcx, qword ptr [rsp + 0x30]",
    "lock cmpxchg16b xmmword ptr [r10]",
    "mov r10, qword ptr [rsp + 0x38]",
    "mov qword ptr [r10], rax",
    "mov qword ptr [r10 + 8], rdx",
    "setz al",
    "movzx eax, al",
    "pop rbx",
    "ret",
    ".seh_endproc",
);

unsafe extern "system" {
    fn joined_cas128(
        target: *mut u128,
        expected_lo: u64,
        expected_hi: u64,
        replacement_lo: u64,
        replacement_hi: u64,
        observed: *mut u128,
    ) -> u8;
}

#[repr(align(16))]
struct AtomicJoinedWord(UnsafeCell<u128>);

// All access to the aligned cell goes through the locked 16-byte operation.
unsafe impl Sync for AtomicJoinedWord {}

impl AtomicJoinedWord {
    const fn new() -> Self {
        Self(UnsafeCell::new(0))
    }

    fn compare_exchange(&self, current: u128, replacement: u128) -> Result<(), u128> {
        let mut observed = 0u128;
        let success = unsafe {
            joined_cas128(
                self.0.get(),
                current as u64,
                (current >> 64) as u64,
                replacement as u64,
                (replacement >> 64) as u64,
                &mut observed,
            )
        };
        if success != 0 {
            Ok(())
        } else {
            Err(observed)
        }
    }

    fn load(&self) -> u128 {
        let mut observed = 0u128;
        unsafe { joined_cas128(self.0.get(), 0, 0, 0, 0, &mut observed) };
        observed
    }

    fn store(&self, replacement: u128) {
        let mut current = self.load();
        loop {
            match self.compare_exchange(current, replacement) {
                Ok(()) => return,
                Err(observed) => current = observed,
            }
        }
    }
}

static JOINED_RECEIVER_PERMISSION: AtomicJoinedWord = AtomicJoinedWord::new();
static JOINED_NEXT_TICKET: AtomicU32 = AtomicU32::new(0);
static JOINED_ACTIVE: AtomicBool = AtomicBool::new(false);
static JOINED_TICKET_EXHAUSTED: AtomicBool = AtomicBool::new(false);

fn joined_word(receiver: u64, session: u32, ticket: u32, permitted: bool) -> u128 {
    u128::from(receiver)
        | (u128::from(ticket) << 64)
        | (u128::from(session) << 96)
        | (u128::from(permitted) << 95)
}

fn joined_receiver(word: u128) -> u64 {
    word as u64
}

fn joined_session(word: u128) -> u32 {
    (word >> 96) as u32
}

fn joined_ticket(word: u128) -> u32 {
    ((word >> 64) as u32) & JOINED_TICKET_MAX
}

fn joined_permitted(word: u128) -> bool {
    word & (1u128 << 95) != 0
}

fn joined_next_ticket() -> Option<u32> {
    match JOINED_NEXT_TICKET.fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
        old.checked_add(1).filter(|next| *next <= JOINED_TICKET_MAX)
    }) {
        Ok(old) => Some(old + 1),
        Err(_) => {
            // Keep the joined gate installed and force it closed. Turning off
            // JOINED_ACTIVE would pass through the original CanEver result.
            JOINED_TICKET_EXHAUSTED.store(true, Ordering::Release);
            None
        }
    }
}

fn joined_activate() -> bool {
    let current = JOINED_RECEIVER_PERMISSION.load();
    let Some(session) = joined_session(current).checked_add(1) else {
        return false;
    };
    JOINED_NEXT_TICKET.store(0, Ordering::Release);
    JOINED_RECEIVER_PERMISSION.store(joined_word(0, session, 0, false));
    JOINED_TICKET_EXHAUSTED.store(false, Ordering::Release);
    JOINED_ACTIVE.store(true, Ordering::Release);
    true
}

fn joined_deactivate() {
    JOINED_ACTIVE.store(false, Ordering::Release);
    let current = JOINED_RECEIVER_PERMISSION.load();
    let session = joined_session(current).saturating_add(1);
    JOINED_RECEIVER_PERMISSION.store(joined_word(0, session, 0, false));
}

// A bounded, keyed cache for the normal 22-person route. The private Ashina
// word above is deliberately independent. One cell is owned by each
// (owner, GameInfo Index, manifest/permission) tuple; each cell retains the
// same receiver/session/ticket/bit atomic layout as the private gate.
const PRODUCT_CELL_COUNT: usize = PRODUCTION_OWNER_LIMIT * PRODUCTION_INDEX_LIMIT;
const PRODUCT_SESSION_MAX: u32 = (1 << 27) - 1;
static PRODUCT_OWNERS: [AtomicJoinedWord; PRODUCTION_OWNER_LIMIT] =
    [const { AtomicJoinedWord::new() }; PRODUCTION_OWNER_LIMIT];
static PRODUCT_MANIFEST: [AtomicJoinedWord; PRODUCT_CELL_COUNT] =
    [const { AtomicJoinedWord::new() }; PRODUCT_CELL_COUNT];
static PRODUCT_PERMISSION: [AtomicJoinedWord; PRODUCT_CELL_COUNT] =
    [const { AtomicJoinedWord::new() }; PRODUCT_CELL_COUNT];
static PRODUCT_CLOSURE: [AtomicJoinedWord; PRODUCT_CELL_COUNT] =
    [const { AtomicJoinedWord::new() }; PRODUCT_CELL_COUNT];
static PRODUCT_CLOSED_ACK: [AtomicJoinedWord; PRODUCT_CELL_COUNT] =
    [const { AtomicJoinedWord::new() }; PRODUCT_CELL_COUNT];
static PRODUCT_SESSION: AtomicU32 = AtomicU32::new(0);
static PRODUCT_NEXT_TICKET: AtomicU32 = AtomicU32::new(0);
static PRODUCT_ACTIVE: AtomicBool = AtomicBool::new(false);
static PRODUCT_EXHAUSTED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static PRODUCT_TEST_RESET_PAUSE: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static PRODUCT_TEST_RESET_PAUSED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static PRODUCT_TEST_RESET_RELEASE: AtomicBool = AtomicBool::new(false);

fn product_cell(owner: usize, index: usize, manifest: bool) -> &'static AtomicJoinedWord {
    let offset = owner * PRODUCTION_INDEX_LIMIT + index;
    if manifest {
        &PRODUCT_MANIFEST[offset]
    } else {
        &PRODUCT_PERMISSION[offset]
    }
}

fn product_next_ticket() -> Option<u32> {
    match PRODUCT_NEXT_TICKET.fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
        old.checked_add(1).filter(|next| *next <= JOINED_TICKET_MAX)
    }) {
        Ok(old) => Some(old + 1),
        Err(_) => {
            PRODUCT_EXHAUSTED.store(true, Ordering::Release);
            None
        }
    }
}

fn product_activate() -> bool {
    let Ok(session) = PRODUCT_SESSION.fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
        old.checked_add(1)
            .filter(|next| *next <= PRODUCT_SESSION_MAX)
    }) else {
        return false;
    };
    let session = session + 1;
    PRODUCT_NEXT_TICKET.store(0, Ordering::Release);
    for owner in &PRODUCT_OWNERS {
        owner.store(joined_word(0, session, 0, false));
    }
    PRODUCT_EXHAUSTED.store(false, Ordering::Release);
    PRODUCT_ACTIVE.store(true, Ordering::Release);
    true
}

fn product_deactivate() {
    PRODUCT_ACTIVE.store(false, Ordering::Release);
}

fn product_bind(owner: usize, receiver: u64, session: u32, ticket: u32) {
    if owner >= PRODUCTION_OWNER_LIMIT || receiver == 0 || receiver & 7 != 0 || ticket == 0 {
        return;
    }
    if !PRODUCT_ACTIVE.load(Ordering::Acquire)
        || PRODUCT_EXHAUSTED.load(Ordering::Acquire)
        || PRODUCT_SESSION.load(Ordering::Acquire) != session
    {
        return;
    }
    // Reuse by another owner closes its earlier binding before it may be read
    // as this owner's source. Old in-flight binds carry smaller tickets.
    for (other_id, other) in PRODUCT_OWNERS.iter().enumerate() {
        if other_id == owner {
            continue;
        }
        loop {
            let current = other.load();
            if joined_session(current) != session
                || joined_receiver(current) != receiver
                || joined_ticket(current) >= ticket
            {
                break;
            }
            if other
                .compare_exchange(current, joined_word(0, session, ticket, false))
                .is_ok()
            {
                break;
            }
        }
    }
    let target = &PRODUCT_OWNERS[owner];
    loop {
        let current = target.load();
        if joined_session(current) != session
            || joined_ticket(current) >= ticket
            || joined_receiver(current) == receiver
            || !PRODUCT_ACTIVE.load(Ordering::Acquire)
        {
            return;
        }
        if target
            .compare_exchange(current, joined_word(receiver, session, ticket, false))
            .is_ok()
        {
            return;
        }
    }
}

fn product_key(key: u32) -> Option<(usize, bool)> {
    if (PRODUCTION_MANIFEST_BASE..PRODUCTION_MANIFEST_BASE + PRODUCTION_INDEX_LIMIT as u32)
        .contains(&key)
    {
        Some(((key - PRODUCTION_MANIFEST_BASE) as usize, true))
    } else if (PRODUCTION_ALLOW_BASE..PRODUCTION_ALLOW_BASE + PRODUCTION_INDEX_LIMIT as u32)
        .contains(&key)
    {
        Some(((key - PRODUCTION_ALLOW_BASE) as usize, false))
    } else {
        None
    }
}

fn product_close_index(key: u32) -> Option<usize> {
    (PRODUCTION_CLOSE_BASE..PRODUCTION_CLOSE_BASE + PRODUCTION_INDEX_LIMIT as u32)
        .contains(&key)
        .then(|| (key - PRODUCTION_CLOSE_BASE) as usize)
}

fn product_reset_owner(key: u32) -> Option<usize> {
    (PRODUCTION_RESET_KEY..=PRODUCTION_RESET_LAST_KEY)
        .contains(&key)
        .then(|| (key - PRODUCTION_RESET_KEY) as usize)
}

fn product_publish_word(
    cell: &AtomicJoinedWord,
    receiver: u64,
    session: u32,
    ticket: u32,
    bit: bool,
) {
    let _ = product_try_publish_word(cell, receiver, session, ticket, bit);
}

fn product_try_publish_word(
    cell: &AtomicJoinedWord,
    receiver: u64,
    session: u32,
    ticket: u32,
    bit: bool,
) -> Option<u128> {
    loop {
        if !PRODUCT_ACTIVE.load(Ordering::Acquire)
            || PRODUCT_SESSION.load(Ordering::Acquire) != session
            || PRODUCT_EXHAUSTED.load(Ordering::Acquire)
        {
            return None;
        }
        let current = cell.load();
        if joined_session(current) > session
            || (joined_session(current) == session && joined_ticket(current) >= ticket)
        {
            return None;
        }
        let published = joined_word(receiver, session, ticket, bit);
        if cell.compare_exchange(current, published).is_ok() {
            return Some(published);
        }
    }
}

fn product_property_begin(receiver: u64, key: u32) -> u64 {
    let reset_owner = product_reset_owner(key);
    if (product_key(key).is_none() && product_close_index(key).is_none() && reset_owner.is_none())
        || receiver == 0
        || receiver & 7 != 0
        || !PRODUCT_ACTIVE.load(Ordering::Acquire)
        || PRODUCT_EXHAUSTED.load(Ordering::Acquire)
    {
        return 0;
    }
    let session = PRODUCT_SESSION.load(Ordering::Acquire);
    let owner = if let Some(owner) = reset_owner {
        owner
    } else {
        let mut found = None;
        for (owner, binding) in PRODUCT_OWNERS.iter().enumerate() {
            let current = binding.load();
            if joined_session(current) == session && joined_receiver(current) == receiver {
                if found.is_some() {
                    return 0;
                }
                found = Some(owner);
            }
        }
        let Some(owner) = found else {
            return 0;
        };
        owner
    };
    let Some(ticket) = product_next_ticket() else {
        return 0;
    };
    if reset_owner.is_some() {
        #[cfg(test)]
        runtime::pause_reset_after_ticket();
        // The synchronized producer reads only its own owner key. Binding and
        // closing at getter entry also work when Gameplay has no CanEver API.
        // Invalidate the previous permission before clearing duplicate owners:
        // a concurrent CanEver cannot observe the old word in that interval.
        if runtime::owned() {
            if !runtime::reset_owner(owner, receiver, session, ticket) {
                return 0;
            }
        } else {
            product_publish_word(&PRODUCT_OWNERS[owner], receiver, session, ticket, false);
        }
        #[cfg(test)]
        if PRODUCT_TEST_RESET_PAUSE.load(Ordering::Acquire) {
            PRODUCT_TEST_RESET_PAUSED.store(true, Ordering::Release);
            while !PRODUCT_TEST_RESET_RELEASE.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
        }
        product_bind(owner, receiver, session, ticket);
    }
    let binding = PRODUCT_OWNERS[owner].load();
    if !PRODUCT_ACTIVE.load(Ordering::Acquire)
        || PRODUCT_EXHAUSTED.load(Ordering::Acquire)
        || PRODUCT_SESSION.load(Ordering::Acquire) != session
        || joined_session(binding) != session
        || joined_receiver(binding) != receiver
        || (reset_owner.is_some() && joined_ticket(binding) != ticket)
        || (reset_owner.is_none() && joined_ticket(binding) >= ticket)
    {
        return 0;
    }
    // Closure happens at entry, independent of the getter's stale/raw return.
    if let Some(index) = product_close_index(key) {
        product_publish_word(
            &PRODUCT_CLOSURE[owner * PRODUCTION_INDEX_LIMIT + index],
            receiver,
            session,
            ticket,
            false,
        );
    }
    (u64::from(session) << 37) | ((owner as u64) << 31) | u64::from(ticket)
}

fn product_observe_property(
    receiver: u64,
    key: u32,
    token: u64,
    state: u32,
    tag: u32,
    payload: u64,
    flags: u32,
) {
    let cell_key = product_key(key);
    if cell_key.is_none()
        && product_close_index(key).is_none()
        && product_reset_owner(key).is_none()
    {
        return;
    }
    if token == 0
        || !PRODUCT_ACTIVE.load(Ordering::Acquire)
        || PRODUCT_EXHAUSTED.load(Ordering::Acquire)
    {
        return;
    }
    let session = (token >> 37) as u32;
    let owner = ((token >> 31) & 63) as usize;
    let ticket = (token as u32) & JOINED_TICKET_MAX;
    if session != PRODUCT_SESSION.load(Ordering::Acquire) || ticket == 0 {
        return;
    }
    let binding = PRODUCT_OWNERS[owner].load();
    if joined_session(binding) != session
        || joined_receiver(binding) != receiver
        || joined_ticket(binding) >= ticket
    {
        return;
    }
    if product_close_index(key).is_some() {
        return;
    }
    if product_reset_owner(key).is_some() {
        // The binding threshold was advanced at entry, before the raw read.
        return;
    }
    let (index, manifest) = cell_key.expect("validated production property key");
    let offset = owner * PRODUCTION_INDEX_LIMIT + index;
    let closure = PRODUCT_CLOSURE[offset].load();
    if joined_receiver(closure) != receiver
        || joined_session(closure) != session
        || joined_ticket(closure) <= joined_ticket(binding)
        || ticket <= joined_ticket(closure)
    {
        return;
    }
    let valid_bool = state == 1 && tag == 1 && flags == 0 && payload <= 1;
    if !manifest && valid_bool && payload == 0 {
        product_publish_word(&PRODUCT_CLOSED_ACK[offset], receiver, session, ticket, true);
    }
    let ack = PRODUCT_CLOSED_ACK[offset].load();
    let closed_ack = joined_receiver(ack) == receiver
        && joined_session(ack) == session
        && joined_ticket(ack) > joined_ticket(closure)
        && joined_permitted(ack);
    let permit = valid_bool && payload == 1 && closed_ack;
    product_publish_word(
        product_cell(owner, index, manifest),
        receiver,
        session,
        ticket,
        permit,
    );
}
fn product_index_known(index: usize, session: u32) -> bool {
    index < PRODUCTION_INDEX_LIMIT
        && (0..PRODUCTION_OWNER_LIMIT).any(|owner| {
            let manifest = product_cell(owner, index, true).load();
            let closure = PRODUCT_CLOSURE[owner * PRODUCTION_INDEX_LIMIT + index].load();
            (joined_session(manifest) == session && joined_receiver(manifest) != 0)
                || (joined_session(closure) == session && joined_receiver(closure) != 0)
        })
}

// None means a non-persona governor: preserve GameCore's own result.
// Some(false) means a manifested persona with no current positive permission.
fn product_permission(owner: usize, index: usize, receiver: u64, session: u32) -> Option<bool> {
    if composite::ENABLED.load(Ordering::Acquire) {
        let Some(catalog) = composite::ready_catalog(owner, receiver, session) else {
            return Some(false);
        };
        if !catalog
            .iter()
            .any(|definition| definition.governor_index as usize == index)
        {
            return None;
        }
    }
    if index >= PRODUCTION_INDEX_LIMIT || !PRODUCT_ACTIVE.load(Ordering::Acquire) {
        return None;
    }
    let active_session = PRODUCT_SESSION.load(Ordering::Acquire);
    if session != active_session || owner >= PRODUCTION_OWNER_LIMIT {
        return product_index_known(index, active_session).then_some(false);
    }
    let binding = PRODUCT_OWNERS[owner].load();
    if receiver == 0 || joined_receiver(binding) != receiver || joined_session(binding) != session {
        return product_index_known(index, session).then_some(false);
    }
    for (other, row) in PRODUCT_OWNERS.iter().enumerate() {
        if other != owner && joined_receiver(row.load()) == receiver {
            return Some(false);
        }
    }
    let closure = PRODUCT_CLOSURE[owner * PRODUCTION_INDEX_LIMIT + index].load();
    let manifest = product_cell(owner, index, true).load();
    if joined_receiver(manifest) != receiver || joined_session(manifest) != session {
        return product_index_known(index, session).then_some(false);
    }
    if PRODUCT_EXHAUSTED.load(Ordering::Acquire) {
        return Some(false);
    }
    if joined_ticket(manifest) <= joined_ticket(binding)
        || !joined_permitted(manifest)
        || joined_session(closure) != session
        || joined_receiver(closure) != receiver
        || joined_ticket(closure) <= joined_ticket(binding)
        || joined_ticket(manifest) <= joined_ticket(closure)
    {
        return Some(false);
    }
    let permission = product_cell(owner, index, false).load();
    let allowed = joined_receiver(permission) == receiver
        && joined_session(permission) == session
        && joined_ticket(permission) > joined_ticket(manifest)
        && joined_permitted(permission);
    // A concurrent reset/rebind cannot be combined with a prior true cell.
    let final_binding = PRODUCT_OWNERS[owner].load();
    Some(
        allowed
            && final_binding == binding
            && product_cell(owner, index, true).load() == manifest
            && product_cell(owner, index, false).load() == permission
            && PRODUCT_CLOSURE[owner * PRODUCTION_INDEX_LIMIT + index].load() == closure
            && PRODUCT_ACTIVE.load(Ordering::Acquire)
            && !PRODUCT_EXHAUSTED.load(Ordering::Acquire),
    )
}

#[cfg(test)]
static JOINED_TEST_BIND_RECEIVER: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static JOINED_TEST_BIND_PAUSED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static JOINED_TEST_BIND_RELEASE: AtomicBool = AtomicBool::new(false);

fn joined_bind(receiver: u64, session: u32, ticket: u32) {
    if receiver == 0 || receiver & 7 != 0 || ticket == 0 {
        return;
    }
    loop {
        if !JOINED_ACTIVE.load(Ordering::Acquire) || JOINED_TICKET_EXHAUSTED.load(Ordering::Acquire)
        {
            return;
        }
        let current = JOINED_RECEIVER_PERMISSION.load();
        if joined_session(current) != session || joined_receiver(current) == receiver {
            return;
        }
        if ticket <= joined_ticket(current) {
            return;
        }
        let replacement = joined_word(receiver, session, ticket, false);
        #[cfg(test)]
        if receiver == JOINED_TEST_BIND_RECEIVER.load(Ordering::Acquire)
            && !JOINED_TEST_BIND_RELEASE.load(Ordering::Acquire)
        {
            JOINED_TEST_BIND_PAUSED.store(true, Ordering::Release);
            while !JOINED_TEST_BIND_RELEASE.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
        }
        if JOINED_RECEIVER_PERMISSION
            .compare_exchange(current, replacement)
            .is_ok()
        {
            return;
        }
    }
}

// Returns a session/ticket token only for the one numeric Core property and
// the currently bound receiver. This runs before the original getter once.
unsafe extern "system" fn joined_property_begin(property: u64, key: u32) -> u64 {
    if composite::ENABLED.load(Ordering::Acquire) && composite::handles(key) {
        return composite::begin(property, key);
    }
    if PRODUCT_ACTIVE.load(Ordering::Acquire) {
        return product_property_begin(property, key);
    }
    if key != ASHINA_ADMISSION_KEY_ID
        || !JOINED_ACTIVE.load(Ordering::Acquire)
        || JOINED_TICKET_EXHAUSTED.load(Ordering::Acquire)
    {
        return 0;
    }
    let current = JOINED_RECEIVER_PERMISSION.load();
    if property == 0 || joined_receiver(current) != property {
        return 0;
    }
    let Some(ticket) = joined_next_ticket() else {
        return 0;
    };
    if !JOINED_ACTIVE.load(Ordering::Acquire)
        || JOINED_TICKET_EXHAUSTED.load(Ordering::Acquire)
        || joined_session(JOINED_RECEIVER_PERMISSION.load()) != joined_session(current)
    {
        return 0;
    }
    (u64::from(joined_session(current)) << 32) | u64::from(ticket)
}

fn joined_observe_property(
    property: u64,
    key: u32,
    token: u64,
    state: u32,
    tag: u32,
    payload: u64,
    flags: u32,
) {
    if !JOINED_ACTIVE.load(Ordering::Acquire)
        || JOINED_TICKET_EXHAUSTED.load(Ordering::Acquire)
        || key != ASHINA_ADMISSION_KEY_ID
        || token == 0
    {
        return;
    }
    let permitted = state == 1 && tag == 1 && flags == 0 && payload == 1;
    loop {
        let current = JOINED_RECEIVER_PERMISSION.load();
        if joined_receiver(current) != property
            || property == 0
            || joined_session(current) != (token >> 32) as u32
            || joined_ticket(current) >= token as u32
        {
            return;
        }
        let replacement = joined_word(property, joined_session(current), token as u32, permitted);
        if JOINED_RECEIVER_PERMISSION
            .compare_exchange(current, replacement)
            .is_ok()
        {
            return;
        }
    }
}

// No Lua/C++ calls, allocation, formatting or lock acquisition in the hook.
// The original function receives its saved argument registers immediately after
// this bounded copy. A failed copy is recorded, never treated as permission.
const _: () = assert!(std::mem::size_of::<CalibrationRow>() <= 0x58);
unsafe extern "system" fn calibration_begin(
    governors_this: u64,
    governor_index: u32,
    return_address: u64,
    scratch: *mut CalibrationRow,
) -> *const CalibrationRow {
    let runtime_generation = runtime::generation();
    if JOINED_ACTIVE.load(Ordering::Acquire) && governor_index != 4 {
        return null_mut();
    }
    let production = PRODUCT_ACTIVE.load(Ordering::Acquire) || runtime::owned();
    let session = if production {
        PRODUCT_SESSION.load(Ordering::Acquire)
    } else {
        joined_session(JOINED_RECEIVER_PERMISSION.load())
    };
    // One order for the whole observation, before reading its owner pointer.
    // A compare-exchange retry keeps this order and cannot become newer work.
    let bind_ticket = if production && PRODUCT_ACTIVE.load(Ordering::Acquire) {
        product_next_ticket().unwrap_or(0)
    } else if JOINED_ACTIVE.load(Ordering::Acquire) {
        joined_next_ticket().unwrap_or(0)
    } else {
        0
    };
    let trace = CALIBRATION_TRACE_PTR.load(Ordering::Acquire);
    if trace.is_null() {
        return null_mut();
    }
    let trace = unsafe { &*trace };
    trace.in_flight.fetch_add(1, Ordering::AcqRel);
    let row = if production {
        // A per-call stack object carries the decision even after the prefix
        // observation sample fills. Never use shared telemetry as authority.
        if scratch.is_null() {
            trace.in_flight.fetch_sub(1, Ordering::Release);
            return null_mut();
        }
        unsafe { ptr::write(scratch, CalibrationRow::default()) };
        unsafe { &*scratch }
    } else {
        let slot = trace.next.fetch_add(1, Ordering::AcqRel) as usize;
        if slot >= CALIBRATION_CAPACITY {
            trace.dropped.fetch_add(1, Ordering::Relaxed);
            trace.in_flight.fetch_sub(1, Ordering::Release);
            return null_mut();
        }
        &trace.rows[slot]
    };
    row.joined_session.store(session, Ordering::Relaxed);
    row.entry_ticket.store(bind_ticket, Ordering::Relaxed);
    row.sequence.store(runtime_generation, Ordering::Relaxed);
    row.governor_index.store(governor_index, Ordering::Relaxed);
    row.governors_this.store(governors_this, Ordering::Relaxed);
    row.return_address.store(return_address, Ordering::Relaxed);
    row.thread_id
        .store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
    let owner = governors_this
        .checked_add(0x70)
        .and_then(copy_current::<8>)
        .map(u64::from_le_bytes);
    match owner {
        Some(0) => row.owner_read_state.store(0, Ordering::Relaxed),
        Some(player) => {
            row.player_address.store(player, Ordering::Relaxed);
            if let (Some(id), Some(receiver)) = (
                player
                    .checked_add(0xd8)
                    .and_then(copy_current::<4>)
                    .map(u32::from_le_bytes),
                player.checked_add(0x528),
            ) {
                row.player_id.store(id, Ordering::Relaxed);
                row.property_receiver.store(receiver, Ordering::Relaxed);
                row.owner_read_state.store(1, Ordering::Relaxed);
            } else {
                row.owner_read_state.store(2, Ordering::Relaxed);
            }
        }
        None => row.owner_read_state.store(2, Ordering::Relaxed),
    }
    if governor_index == 4
        && row.owner_read_state.load(Ordering::Relaxed) == 1
        && row.player_id.load(Ordering::Relaxed) == 0
    {
        joined_bind(
            row.property_receiver.load(Ordering::Relaxed),
            session,
            bind_ticket,
        );
    }
    if production
        && (governor_index as usize) < PRODUCTION_INDEX_LIMIT
        && row.owner_read_state.load(Ordering::Relaxed) == 1
    {
        product_bind(
            row.player_id.load(Ordering::Relaxed) as usize,
            row.property_receiver.load(Ordering::Relaxed),
            session,
            bind_ticket,
        );
    }
    row as *const CalibrationRow
}

unsafe extern "system" fn calibration_end(
    row: *const CalibrationRow,
    original_rax: u64,
    governor_index: u32,
    governors_this: u64,
) -> u64 {
    if row.is_null() {
        if runtime::owned() {
            return original_rax & !0xff;
        }
        if PRODUCT_ACTIVE.load(Ordering::Acquire)
            && (governor_index as usize) < PRODUCTION_INDEX_LIMIT
        {
            let player = governors_this
                .checked_add(0x70)
                .and_then(copy_current::<8>)
                .map(u64::from_le_bytes);
            if let Some(player) = player.filter(|player| *player != 0) {
                let owner = player
                    .checked_add(0xd8)
                    .and_then(copy_current::<4>)
                    .map(u32::from_le_bytes);
                if let (Some(owner), Some(receiver)) = (owner, player.checked_add(0x528)) {
                    if product_permission(
                        owner as usize,
                        governor_index as usize,
                        receiver,
                        PRODUCT_SESSION.load(Ordering::Acquire),
                    )
                    .is_some()
                    {
                        return original_rax & !0xff;
                    }
                }
            }
        }
        return if JOINED_ACTIVE.load(Ordering::Acquire) && governor_index == 4 {
            original_rax & !0xff
        } else {
            original_rax
        };
    }
    let row = unsafe { &*row };
    row.original_al
        .store((original_rax & 0xff) as u32, Ordering::Relaxed);
    let mut result = original_rax;
    if JOINED_ACTIVE.load(Ordering::Acquire) && row.governor_index.load(Ordering::Relaxed) == 4 {
        let receiver = row.property_receiver.load(Ordering::Relaxed);
        let admission = JOINED_RECEIVER_PERMISSION.load();
        let permitted = row.owner_read_state.load(Ordering::Relaxed) == 1
            && row.player_id.load(Ordering::Relaxed) == 0
            && !JOINED_TICKET_EXHAUSTED.load(Ordering::Acquire)
            && receiver != 0
            && joined_session(admission) == row.joined_session.load(Ordering::Relaxed)
            && joined_receiver(admission) == receiver
            && joined_permitted(admission);
        result = (original_rax & !0xff) | u64::from(permitted);
    } else if PRODUCT_ACTIVE.load(Ordering::Acquire) {
        if let Some(permitted) = product_permission(
            row.player_id.load(Ordering::Relaxed) as usize,
            row.governor_index.load(Ordering::Relaxed) as usize,
            row.property_receiver.load(Ordering::Relaxed),
            row.joined_session.load(Ordering::Relaxed),
        ) {
            result = (original_rax & !0xff) | u64::from(permitted);
        }
    }
    if runtime::owned()
        && !runtime::call_current(
            runtime::CallFence {
                session: row.joined_session.load(Ordering::Relaxed),
                ticket: row.entry_ticket.load(Ordering::Relaxed),
                owner: row.player_id.load(Ordering::Relaxed),
                read_state: row.owner_read_state.load(Ordering::Relaxed),
                receiver: row.property_receiver.load(Ordering::Relaxed),
            },
            governors_this,
        )
    {
        result &= !0xff;
    }
    row.effective_al
        .store((result & 0xff) as u32, Ordering::Relaxed);
    row.sequence.store(1, Ordering::Release);
    let trace = CALIBRATION_TRACE_PTR.load(Ordering::Acquire);
    if !trace.is_null() {
        let trace = unsafe { &*trace };
        if trace.reserved == BridgeMode::ProductionAdmission as u32 && runtime::sample_begin() {
            let slot = runtime::next_slot(&trace.next, CALIBRATION_CAPACITY);
            if slot < CALIBRATION_CAPACITY {
                let target = &trace.rows[slot];
                macro_rules! copy_field {
                    ($($field:ident),*) => { $(target.$field.store(row.$field.load(Ordering::Relaxed), Ordering::Relaxed);)* };
                }
                copy_field!(
                    governor_index,
                    thread_id,
                    owner_read_state,
                    governors_this,
                    return_address,
                    player_address,
                    player_id,
                    original_al,
                    effective_al,
                    property_receiver,
                    joined_session
                );
                target.sequence.store(1, Ordering::Release);
            } else {
                trace.dropped.fetch_add(1, Ordering::Relaxed);
            }
            runtime::sample_end();
        }
        trace.in_flight.fetch_sub(1, Ordering::Release);
    }
    result
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
    pub filtered: AtomicU32,
    pub scope: u32,
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
            filtered: AtomicU32::new(0),
            scope: 0,
            reserved: 0,
            rows: std::array::from_fn(|_| PropertyRow::default()),
        }
    }
}

impl PropertyTrace {
    pub const VERSION: u32 = PROPERTY_TRACE_VERSION;
    pub fn scoped() -> Self {
        Self {
            scope: BridgeMode::PropertyScoped as u32,
            ..Self::default()
        }
    }

    pub fn admission_scoped() -> Self {
        Self {
            scope: BridgeMode::PropertyAdmissionScoped as u32,
            ..Self::default()
        }
    }

    pub fn joined_scoped() -> Self {
        Self {
            scope: BridgeMode::JoinedAdmission as u32,
            ..Self::default()
        }
    }

    pub fn production_scoped() -> Self {
        Self {
            scope: BridgeMode::ProductionAdmission as u32,
            ..Self::default()
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
    pub denial_owner: u32,
    pub denial_index: u32,
    pub denial_checked: u32,
}

impl PropertyTrace {
    pub fn complete_rows(&self) -> Result<Vec<PropertyValueTrace>, &'static str> {
        if self.scope != BridgeMode::ProductionAdmission as u32
            && self.dropped.load(Ordering::Acquire) != 0
        {
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
                denial_owner: (row.tag.load(Ordering::Relaxed) >> 25) & 63,
                denial_index: (row.tag.load(Ordering::Relaxed) >> 17) & 255,
                denial_checked: (row.tag.load(Ordering::Relaxed) >> 16) & 1,
            });
        }
        Ok(rows)
    }
}

// Called only after the original getter returned. This path performs no
// allocation, formatting, locking, Lua access, or C++ method invocation.
// ReadProcessMemory copies all 24 bytes or records unreadable; the borrowed
// returned pointer is never read after this synchronous call.
#[cfg(test)]
static JOINED_TEST_DELAY_RETURNED: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static JOINED_TEST_PAUSED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static JOINED_TEST_RELEASE: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
static PROPERTY_TEST_RAW_MUTATE_RETURNED: AtomicU64 = AtomicU64::new(0);
unsafe extern "system" fn record_property(property: u64, key_id: u32, returned: u64, token: u64) {
    let trace = PROPERTY_TRACE_PTR.load(Ordering::Acquire);
    if trace.is_null() {
        return;
    }
    let trace = unsafe { &*trace };
    #[cfg(test)]
    if trace.scope == BridgeMode::JoinedAdmission as u32
        && returned != 0
        && returned == JOINED_TEST_DELAY_RETURNED.load(Ordering::Acquire)
    {
        JOINED_TEST_PAUSED.store(true, Ordering::Release);
        while !JOINED_TEST_RELEASE.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
    }
    if (trace.scope == BridgeMode::PropertyScoped as u32
        && key_id != PROPERTY_TEST_VALUE_KEY_ID
        && key_id != PROPERTY_TEST_PHASE_KEY_ID)
        || ((trace.scope == BridgeMode::PropertyAdmissionScoped as u32
            || trace.scope == BridgeMode::JoinedAdmission as u32)
            && key_id != ASHINA_ADMISSION_KEY_ID)
        || (trace.scope == BridgeMode::ProductionAdmission as u32
            && product_key(key_id).is_none()
            && product_close_index(key_id).is_none()
            && product_reset_owner(key_id).is_none()
            && !(composite::ENABLED.load(Ordering::Acquire) && composite::handles(key_id)))
    {
        trace.filtered.fetch_add(1, Ordering::Relaxed);
        return;
    }
    trace.in_flight.fetch_add(1, Ordering::AcqRel);
    let sampled = runtime::sample_begin();
    let slot = if sampled {
        runtime::next_slot(&trace.next, TRACE_CAPACITY)
    } else {
        TRACE_CAPACITY
    };
    if slot >= TRACE_CAPACITY && trace.scope != BridgeMode::ProductionAdmission as u32 {
        if trace.scope == BridgeMode::JoinedAdmission as u32 {
            joined_observe_property(property, key_id, token, 0, 0, 0, 0);
        }
        trace.dropped.fetch_add(1, Ordering::Relaxed);
        trace.in_flight.fetch_sub(1, Ordering::Release);
        if sampled {
            runtime::sample_end();
        }
        return;
    }
    // One synchronous copy of borrowed native storage. Authority and telemetry
    // consume exactly these same bytes, even if the source is then replaced.
    let (state, tag, payload, flags) = match copy_current::<24>(returned) {
        Some(bytes) => (
            1,
            u16::from_le_bytes(bytes[0..2].try_into().unwrap()) as u32,
            u64::from_le_bytes(bytes[8..16].try_into().unwrap()),
            u32::from_le_bytes(bytes[20..24].try_into().unwrap()),
        ),
        None => (if returned == 0 { 0 } else { 2 }, 0, 0, 0),
    };
    if trace.scope == BridgeMode::ProductionAdmission as u32 {
        if composite::ENABLED.load(Ordering::Acquire) && composite::handles(key_id) {
            composite::observe(property, key_id, token, state, tag, payload, flags);
        } else {
            product_observe_property(property, key_id, token, state, tag, payload, flags);
            if runtime::owned() && !(state == 1 && tag == 1 && flags == 0 && payload <= 1) {
                composite::invalidate(property, token);
            }
        }
    }
    #[cfg(test)]
    if returned != 0 && returned == PROPERTY_TEST_RAW_MUTATE_RETURNED.load(Ordering::Acquire) {
        unsafe { ((returned + 8) as *mut u64).write_unaligned(0) };
    }
    if slot >= TRACE_CAPACITY {
        if sampled {
            trace.dropped.fetch_add(1, Ordering::Relaxed);
            runtime::sample_end();
        }
        trace.in_flight.fetch_sub(1, Ordering::Release);
        return;
    }
    let row = &trace.rows[slot];
    row.key_id.store(key_id, Ordering::Relaxed);
    row.property.store(property, Ordering::Relaxed);
    row.returned.store(returned, Ordering::Relaxed);
    row.thread_id
        .store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
    row.read_state.store(state, Ordering::Relaxed);
    row.tag.store(tag, Ordering::Relaxed);
    row.payload.store(payload, Ordering::Relaxed);
    row.flags.store(flags, Ordering::Relaxed);
    if trace.scope == BridgeMode::JoinedAdmission as u32 {
        joined_observe_property(property, key_id, token, state, tag, payload, flags);
    }
    if trace.scope == BridgeMode::ProductionAdmission as u32 && token != 0 {
        if let Some((index, _)) = product_key(key_id) {
            let owner = ((token >> 31) & 63) as usize;
            let session = (token >> 37) as u32;
            // The native tag is u16; its unused high half stores the closure
            // proof without enlarging the original 48-byte diagnostic row.
            let denied = product_permission(owner, index, property, session) == Some(false);
            row.tag.store(
                tag | ((owner as u32) << 25) | ((index as u32) << 17) | (u32::from(denied) << 16),
                Ordering::Relaxed,
            );
        }
    }
    row.sequence.store(1, Ordering::Release);
    trace.in_flight.fetch_sub(1, Ordering::Release);
    if sampled {
        runtime::sample_end();
    }
}
// Win64 entry: RCX=property, EDX=key. .seh_* emits pdata/xdata for this
// nonleaf frame. The pinned 0x99F5C0 getter has no covering pdata entry and
// only leaf instructions; the allocated trampoline is a MOV plus direct JMP.
// The 0xd8-byte frame supplies aligned calls, shadow space, and saved volatile
// argument registers for the entry ticket call. The original sees them once;
// its RAX is saved across telemetry and restored before returning to its caller.
global_asm!(
    ".globl property_gateway",
    ".seh_proc property_gateway",
    "property_gateway:",
    "sub rsp, 0xd8",
    ".seh_stackalloc 0xd8",
    ".seh_endprologue",
    "mov qword ptr [rsp + 0x20], rcx",
    "mov qword ptr [rsp + 0x28], rdx",
    "mov qword ptr [rsp + 0x30], r8",
    "mov qword ptr [rsp + 0x38], r9",
    "mov qword ptr [rsp + 0x40], rax",
    "mov qword ptr [rsp + 0x48], r10",
    "mov qword ptr [rsp + 0x50], r11",
    "movdqu xmmword ptr [rsp + 0x70], xmm0",
    "movdqu xmmword ptr [rsp + 0x80], xmm1",
    "movdqu xmmword ptr [rsp + 0x90], xmm2",
    "movdqu xmmword ptr [rsp + 0xa0], xmm3",
    "movdqu xmmword ptr [rsp + 0xb0], xmm4",
    "movdqu xmmword ptr [rsp + 0xc0], xmm5",
    "call {begin}",
    "mov qword ptr [rsp + 0x58], rax",
    "mov rcx, qword ptr [rsp + 0x20]",
    "mov rdx, qword ptr [rsp + 0x28]",
    "mov r8, qword ptr [rsp + 0x30]",
    "mov r9, qword ptr [rsp + 0x38]",
    "mov rax, qword ptr [rsp + 0x40]",
    "mov r10, qword ptr [rsp + 0x48]",
    "mov r11, qword ptr [rsp + 0x50]",
    "movdqu xmm0, xmmword ptr [rsp + 0x70]",
    "movdqu xmm1, xmmword ptr [rsp + 0x80]",
    "movdqu xmm2, xmmword ptr [rsp + 0x90]",
    "movdqu xmm3, xmmword ptr [rsp + 0xa0]",
    "movdqu xmm4, xmmword ptr [rsp + 0xb0]",
    "movdqu xmm5, xmmword ptr [rsp + 0xc0]",
    "call qword ptr [rip + PROPERTY_TRAMPOLINE_PTR]",
    "mov qword ptr [rsp + 0x60], rax",
    "mov rcx, qword ptr [rsp + 0x20]",
    "mov edx, dword ptr [rsp + 0x28]",
    "mov r8, rax",
    "mov r9, qword ptr [rsp + 0x58]",
    "call {record}",
    "mov rax, qword ptr [rsp + 0x60]",
    "add rsp, 0xd8",
    "ret",
    ".seh_endproc",
    begin = sym joined_property_begin,
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

// A read-only return wrapper for owner/receiver calibration. Both callbacks
// operate on the current call only. All incoming volatile argument registers
// and XMM0..5 are restored before the original native function runs once.
global_asm!(
    ".globl calibration_gateway",
    ".seh_proc calibration_gateway",
    "calibration_gateway:",
    "sub rsp, 0x138",
    ".seh_stackalloc 0x138",
    ".seh_endprologue",
    "mov qword ptr [rsp + 0x20], rcx",
    "mov qword ptr [rsp + 0x28], rdx",
    "mov qword ptr [rsp + 0x30], r8",
    "mov qword ptr [rsp + 0x38], r9",
    "mov qword ptr [rsp + 0x40], rax",
    "mov qword ptr [rsp + 0x48], r10",
    "mov qword ptr [rsp + 0x50], r11",
    "movdqu xmmword ptr [rsp + 0x70], xmm0",
    "movdqu xmmword ptr [rsp + 0x80], xmm1",
    "movdqu xmmword ptr [rsp + 0x90], xmm2",
    "movdqu xmmword ptr [rsp + 0xa0], xmm3",
    "movdqu xmmword ptr [rsp + 0xb0], xmm4",
    "movdqu xmmword ptr [rsp + 0xc0], xmm5",
    "mov r8, qword ptr [rsp + 0x138]",
    "lea r9, [rsp + 0xe0]",
    "call {begin}",
    "mov qword ptr [rsp + 0x58], rax",
    "mov rcx, qword ptr [rsp + 0x20]",
    "mov rdx, qword ptr [rsp + 0x28]",
    "mov r8, qword ptr [rsp + 0x30]",
    "mov r9, qword ptr [rsp + 0x38]",
    "mov rax, qword ptr [rsp + 0x40]",
    "mov r10, qword ptr [rsp + 0x48]",
    "mov r11, qword ptr [rsp + 0x50]",
    "movdqu xmm0, xmmword ptr [rsp + 0x70]",
    "movdqu xmm1, xmmword ptr [rsp + 0x80]",
    "movdqu xmm2, xmmword ptr [rsp + 0x90]",
    "movdqu xmm3, xmmword ptr [rsp + 0xa0]",
    "movdqu xmm4, xmmword ptr [rsp + 0xb0]",
    "movdqu xmm5, xmmword ptr [rsp + 0xc0]",
    "call qword ptr [rip + BRIDGE_TRAMPOLINE_PTR]",
    "mov qword ptr [rsp + 0x60], rax",
    "mov rcx, qword ptr [rsp + 0x58]",
    "mov rdx, rax",
    "mov r8d, dword ptr [rsp + 0x28]",
    "mov r9, qword ptr [rsp + 0x20]",
    "call {end}",
    "add rsp, 0x138",
    "ret",
    ".seh_endproc",
    begin = sym calibration_begin,
    end = sym calibration_end,
);

unsafe extern "system" {
    fn calibration_gateway();
}

#[unsafe(no_mangle)]
static CALIBRATION_TRACE_PTR: AtomicPtr<CalibrationTrace> = AtomicPtr::new(null_mut());

#[repr(C)]
struct RawPromotionRegisters {
    rcx: u64,
    rdx: u64,
    r8: u64,
    r9: u64,
    return_address: u64,
}

// This is called only by the assembly gateway. It records entry state and
// never calls GameCore or interprets any argument as an object/index.
extern "system" fn record_promotion_call(raw: *const RawPromotionRegisters) {
    let trace = PROMOTION_TRACE_PTR.load(Ordering::Acquire);
    if trace.is_null() {
        return;
    }
    let trace = unsafe { &*trace };
    trace.in_flight.fetch_add(1, Ordering::AcqRel);
    let slot = trace.next.fetch_add(1, Ordering::AcqRel) as usize;
    if slot >= PROMOTION_TRACE_CAPACITY {
        trace.dropped.fetch_add(1, Ordering::Relaxed);
    } else {
        let raw = unsafe { &*raw };
        let row = &trace.rows[slot];
        row.thread_id
            .store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
        row.rcx.store(raw.rcx, Ordering::Relaxed);
        row.rdx.store(raw.rdx, Ordering::Relaxed);
        row.r8.store(raw.r8, Ordering::Relaxed);
        row.r9.store(raw.r9, Ordering::Relaxed);
        row.return_address
            .store(raw.return_address, Ordering::Relaxed);
        row.sequence.store(slot as u32 + 1, Ordering::Release);
    }
    trace.in_flight.fetch_sub(1, Ordering::Release);
}

// The original E8 pushes the original return site. The gateway's only call
// is the recorder above; it restores its exact incoming stack, flags, all
// volatile integer registers and XMM0..5, then jumps to the unmodified native
// callee. No Rust call frame is interposed around the native invocation.
global_asm!(
    ".globl promotion_call_gateway",
    ".seh_proc promotion_call_gateway",
    "promotion_call_gateway:",
    "pushfq",
    ".seh_stackalloc 8",
    "sub rsp, 0xe0",
    ".seh_stackalloc 0xe0",
    ".seh_endprologue",
    "mov qword ptr [rsp + 0x20], rcx",
    "mov qword ptr [rsp + 0x28], rdx",
    "mov qword ptr [rsp + 0x30], r8",
    "mov qword ptr [rsp + 0x38], r9",
    "mov qword ptr [rsp + 0x58], r11",
    "mov r11, qword ptr [rsp + 0xe8]",
    "mov qword ptr [rsp + 0x40], r11",
    "mov qword ptr [rsp + 0x48], rax",
    "mov qword ptr [rsp + 0x50], r10",
    "movdqu xmmword ptr [rsp + 0x70], xmm0",
    "movdqu xmmword ptr [rsp + 0x80], xmm1",
    "movdqu xmmword ptr [rsp + 0x90], xmm2",
    "movdqu xmmword ptr [rsp + 0xa0], xmm3",
    "movdqu xmmword ptr [rsp + 0xb0], xmm4",
    "movdqu xmmword ptr [rsp + 0xc0], xmm5",
    "lea rcx, [rsp + 0x20]",
    "call {record}",
    "movdqu xmm0, xmmword ptr [rsp + 0x70]",
    "movdqu xmm1, xmmword ptr [rsp + 0x80]",
    "movdqu xmm2, xmmword ptr [rsp + 0x90]",
    "movdqu xmm3, xmmword ptr [rsp + 0xa0]",
    "movdqu xmm4, xmmword ptr [rsp + 0xb0]",
    "movdqu xmm5, xmmword ptr [rsp + 0xc0]",
    "mov rcx, qword ptr [rsp + 0x20]",
    "mov rdx, qword ptr [rsp + 0x28]",
    "mov r8, qword ptr [rsp + 0x30]",
    "mov r9, qword ptr [rsp + 0x38]",
    "mov rax, qword ptr [rsp + 0x48]",
    "mov r10, qword ptr [rsp + 0x50]",
    "mov r11, qword ptr [rsp + 0x58]",
    "lea rsp, [rsp + 0xe0]",
    "popfq",
    "jmp qword ptr [rip + PROMOTION_CALLEE_PTR]",
    ".seh_endproc",
    record = sym record_promotion_call,
);

unsafe extern "system" {
    fn promotion_call_gateway();
}

#[unsafe(no_mangle)]
static PROMOTION_TRACE_PTR: AtomicPtr<PromotionTrace> = AtomicPtr::new(null_mut());
#[unsafe(no_mangle)]
static PROMOTION_CALLEE_PTR: AtomicPtr<u8> = AtomicPtr::new(null_mut());

struct Hook {
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
static JOINED_PROPERTY_HOOK: Mutex<Option<Hook>> = Mutex::new(None);
struct PromotionHook {
    call: *mut u8,
    original: [u8; 16],
    patched_operand: u32,
    // A caller may have fetched the old operand before restoration. Retain
    // the relay, gateway DLL and trace allocation until process exit.
    _relay: *mut u8,
}
unsafe impl Send for PromotionHook {}
static PROMOTION_HOOK: Mutex<Option<PromotionHook>> = Mutex::new(None);
static PROMOTION_USED: AtomicBool = AtomicBool::new(false);
static PROPERTY_USED: AtomicBool = AtomicBool::new(false);
// An entry-restored snapshot does not prove old relay entrants are gone.
// Never redirect a delayed assignment callback to a second trace mapping.
static ASSIGN_USED: AtomicBool = AtomicBool::new(false);

fn promotion_operand(call: usize, to: usize) -> Option<u32> {
    let operand = call.checked_add(1)?;
    if !operand.is_multiple_of(4)
        || (operand & 7) > 4
        || (operand & 63) > 60
        || (operand & (CODE_PAGE - 1)) > CODE_PAGE - 4
    {
        return None;
    }
    let next = call.checked_add(5)?;
    let displacement = i32::try_from(to as i128 - next as i128).ok()?;
    Some(u32::from_le_bytes(displacement.to_le_bytes()))
}

fn verified_promotion_window(call: usize, callee: usize, window: &[u8; 16]) -> bool {
    if *window != PROMOTION_CALL_WINDOW || window[0] != 0xe8 {
        return false;
    }
    let original = u32::from_le_bytes(window[1..5].try_into().unwrap());
    promotion_operand(call, callee) == Some(original)
}

fn rel32_jump(from: usize, to: usize) -> Option<[u8; 5]> {
    let displacement = (to as i128) - (from as i128) - 5;
    let displacement = i32::try_from(displacement).ok()?;
    let mut jump = [0u8; 5];
    jump[0] = 0xe9;
    jump[1..].copy_from_slice(&displacement.to_le_bytes());
    Some(jump)
}

unsafe fn promotion_relay_near(call: usize) -> Option<*mut u8> {
    let base = call & !0xffffusize;
    for step in 1..=0x4000usize {
        let delta = step * 0x10000;
        for candidate in [base.checked_add(delta), base.checked_sub(delta)] {
            let Some(candidate) = candidate else { continue };
            if promotion_operand(call, candidate).is_none() {
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

unsafe fn exchange_promotion_operand(
    call: *mut u8,
    expected: u32,
    replacement: u32,
) -> Result<PatchOutcome, &'static str> {
    let operand = unsafe { call.add(1) };
    if promotion_operand(call as usize, call as usize).is_none() {
        return Err("promotion operand alignment or cacheline invalid");
    }
    let mut previous = 0u32;
    if unsafe { VirtualProtect(operand.cast(), 4, PAGE_EXECUTE_READWRITE, &mut previous) } == 0 {
        return Err("cannot make promotion operand writable");
    }
    let exchanged = unsafe { &*operand.cast::<AtomicU32>() }
        .compare_exchange(expected, replacement, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok();
    let mut replaced = 0u32;
    let protection_restored =
        unsafe { VirtualProtect(operand.cast(), 4, previous, &mut replaced) } != 0;
    let cache_flushed = unsafe { FlushInstructionCache(GetCurrentProcess(), call.cast(), 5) } != 0;
    Ok(PatchOutcome {
        exchanged,
        protection_restored,
        cache_flushed,
    })
}

unsafe fn install_promotion_call(
    call: *mut u8,
    callee: *mut u8,
    trace: *mut PromotionTrace,
    expected: [u8; 16],
) -> Result<(), &'static str> {
    if call.is_null()
        || callee.is_null()
        || trace.is_null()
        || unsafe { (*trace).magic } != PROMOTION_TRACE_MAGIC
        || unsafe { (*trace).version } != PROMOTION_TRACE_VERSION
        || unsafe { (*trace).mode } != BridgeMode::PromotionCall as u32
        || unsafe { (*trace).capacity } != PROMOTION_TRACE_CAPACITY as u32
    {
        return Err("invalid promotion call or trace header");
    }
    let mut guard = PROMOTION_HOOK
        .lock()
        .map_err(|_| "promotion lock poisoned")?;
    if guard.is_some() || HOOK.lock().map_err(|_| "bridge lock poisoned")?.is_some() {
        return Err("another bridge hook is installed");
    }
    let original = unsafe { ptr::read(call.cast::<[u8; 16]>()) };
    if original != expected || !verified_promotion_window(call as usize, callee as usize, &original)
    {
        return Err("pinned complete promotion call or target differs");
    }
    let relay = unsafe { promotion_relay_near(call as usize) }
        .ok_or("no rel32 promotion relay near call")?;
    let gateway = promotion_call_gateway as *const () as usize;
    unsafe {
        ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), relay, 6);
        ptr::copy_nonoverlapping((gateway as u64).to_le_bytes().as_ptr(), relay.add(6), 8);
    }
    let mut previous = 0u32;
    if unsafe { VirtualProtect(relay.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut previous) } == 0
        || unsafe { FlushInstructionCache(GetCurrentProcess(), relay.cast(), CODE_PAGE) } == 0
    {
        unsafe { VirtualFree(relay.cast(), 0, MEM_RELEASE) };
        return Err("cannot publish promotion relay");
    }
    let original_operand = u32::from_le_bytes(original[1..5].try_into().unwrap());
    let patched_operand = promotion_operand(call as usize, relay as usize)
        .ok_or("promotion relay displacement out of range")?;
    PROMOTION_TRACE_PTR.store(trace, Ordering::Release);
    PROMOTION_CALLEE_PTR.store(callee, Ordering::Release);
    let outcome = unsafe { exchange_promotion_operand(call, original_operand, patched_operand) }?;
    if !outcome.exchanged {
        return Err("critical: promotion call changed before atomic install");
    }
    *guard = Some(PromotionHook {
        call,
        original,
        patched_operand,
        _relay: relay,
    });
    let mut patched = original;
    patched[1..5].copy_from_slice(&patched_operand.to_le_bytes());
    if !outcome.protection_restored
        || !outcome.cache_flushed
        || unsafe { ptr::read(call.cast::<[u8; 16]>()) } != patched
    {
        return Err("critical: promotion operand publication unverified");
    }
    Ok(())
}

unsafe fn restore_promotion_call() -> Result<bool, &'static str> {
    let mut guard = PROMOTION_HOOK
        .lock()
        .map_err(|_| "promotion lock poisoned")?;
    let Some(hook) = guard.as_ref() else {
        return Ok(false);
    };
    let original_operand = u32::from_le_bytes(hook.original[1..5].try_into().unwrap());
    let outcome =
        unsafe { exchange_promotion_operand(hook.call, hook.patched_operand, original_operand) }?;
    if !outcome.exchanged
        || !outcome.protection_restored
        || !outcome.cache_flushed
        || unsafe { ptr::read(hook.call.cast::<[u8; 16]>()) } != hook.original
    {
        return Err("critical: promotion operand restoration unverified");
    }
    *guard = None;
    Ok(true)
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
    mode: BridgeMode,
) -> Result<(), &'static str> {
    if target.is_null()
        || trace.is_null()
        || !(target as usize).is_multiple_of(8)
        || (target as usize & (CODE_PAGE - 1)) < PROPERTY_PREFIX_LEN
    {
        return Err("unaligned or missing property hook input");
    }
    let lock = if matches!(
        mode,
        BridgeMode::JoinedAdmission | BridgeMode::ProductionAdmission
    ) {
        &JOINED_PROPERTY_HOOK
    } else {
        &HOOK
    };
    let mut guard = lock.lock().map_err(|_| "bridge lock poisoned")?;
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
    mode: BridgeMode,
) -> Result<(), &'static str> {
    if target.is_null() || trace.is_null() || !(target as usize).is_multiple_of(8) {
        return Err("unaligned or missing hook input");
    }
    let hook_slot =
        if mode == BridgeMode::AssignCalibration && composite::ENABLED.load(Ordering::Acquire) {
            &composite::ASSIGNMENT_HOOK
        } else {
            &HOOK
        };
    let mut guard = hook_slot.lock().map_err(|_| "bridge lock poisoned")?;
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
    let gateway = match mode {
        BridgeMode::AssignCalibration => assign_calibration_gateway as *const () as usize,
        BridgeMode::CanEver => bridge_gateway as *const () as usize,
        BridgeMode::CanEverCalibration
        | BridgeMode::JoinedAdmission
        | BridgeMode::ProductionAdmission => calibration_gateway as *const () as usize,
        _ => return Err("invalid CanEver gateway mode"),
    };
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
    match mode {
        BridgeMode::AssignCalibration => ASSIGN_TRACE_PTR.store(trace.cast(), Ordering::Release),
        BridgeMode::CanEver => BRIDGE_TRACE_PTR.store(trace, Ordering::Release),
        BridgeMode::CanEverCalibration
        | BridgeMode::JoinedAdmission
        | BridgeMode::ProductionAdmission => {
            if mode == BridgeMode::ProductionAdmission {
                unsafe {
                    (*trace.cast::<CalibrationTrace>()).reserved = mode as u32;
                }
            }
            CALIBRATION_TRACE_PTR.store(trace.cast(), Ordering::Release)
        }
        _ => unreachable!(),
    }
    if mode == BridgeMode::AssignCalibration {
        ASSIGN_TRAMPOLINE_PTR.store(trampoline, Ordering::Release);
    } else {
        BRIDGE_TRAMPOLINE_PTR.store(trampoline, Ordering::Release);
    }
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
    unsafe { install_hook(target, trace, expected, BridgeMode::CanEver) }
}

unsafe fn install_calibration_at(
    target: *mut u8,
    trace: *mut CalibrationTrace,
    expected: [u8; 8],
) -> Result<(), &'static str> {
    if trace.is_null()
        || unsafe { (*trace).magic } != CALIBRATION_TRACE_MAGIC
        || unsafe { (*trace).version } != CALIBRATION_TRACE_VERSION
    {
        return Err("calibration mapping version mismatch");
    }
    unsafe {
        install_hook(
            target,
            trace.cast(),
            expected,
            BridgeMode::CanEverCalibration,
        )
    }
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
    let mode = if unsafe { (*trace).scope } == BridgeMode::PropertyScoped as u32 {
        BridgeMode::PropertyScoped
    } else if unsafe { (*trace).scope } == BridgeMode::PropertyAdmissionScoped as u32 {
        BridgeMode::PropertyAdmissionScoped
    } else if unsafe { (*trace).scope } == BridgeMode::JoinedAdmission as u32 {
        BridgeMode::JoinedAdmission
    } else if unsafe { (*trace).scope } == BridgeMode::ProductionAdmission as u32 {
        BridgeMode::ProductionAdmission
    } else if unsafe { (*trace).scope } == 0 {
        BridgeMode::Property
    } else {
        return Err("invalid property trace scope");
    };
    unsafe { install_property_hotpatch(target, trace, expected, mode) }
}

unsafe fn restore_one(slot: &Mutex<Option<Hook>>) -> Result<bool, &'static str> {
    let mut guard = slot.lock().map_err(|_| "bridge lock poisoned")?;
    let Some(hook) = guard.as_ref() else {
        return Ok(false);
    };
    let outcome = if hook.prefix.is_none() {
        unsafe {
            exchange_entry(
                hook.target,
                u64::from_le_bytes(hook.patched),
                u64::from_le_bytes(hook.original),
            )
        }?
    } else {
        let first: [u8; 2] = hook.original[..2]
            .try_into()
            .map_err(|_| "property entry width")?;
        unsafe { exchange_property_entry(hook.target, PROPERTY_SHORT_JUMP, first) }?
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
    Ok(true)
}

unsafe fn uninstall() -> Result<(), &'static str> {
    joined_deactivate();
    product_deactivate();
    let promotion = unsafe { restore_promotion_call() }?;
    let assignment = restore_one(&composite::ASSIGNMENT_HOOK)?;
    let main = restore_one(&HOOK)?;
    let property = restore_one(&JOINED_PROPERTY_HOOK)?;
    if !main && !property && !promotion && !assignment {
        return Err("bridge not installed");
    }
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
    if mode == BridgeMode::PromotionCall {
        let call_offset = pe_file_offset(&mut file, PINNED_PROMOTION_CALL_RVA as u32)?;
        let callee_offset = pe_file_offset(&mut file, PINNED_PROMOTION_CALLEE_RVA as u32)?;
        if call_offset != PINNED_PROMOTION_CALL_FILE_OFFSET || callee_offset != 0x2d_cfb0 {
            return None;
        }
        file.seek(SeekFrom::Start(callee_offset - 5)).ok()?;
        let mut callee_boundary = [0u8; 13];
        file.read_exact(&mut callee_boundary).ok()?;
        if callee_boundary
            != [
                0xe9, 0xd0, 0xfc, 0xff, 0xff, 0x40, 0x53, 0x56, 0x57, 0x48, 0x83, 0xec, 0x30,
            ]
        {
            return None;
        }
        file.seek(SeekFrom::Start(call_offset)).ok()?;
        let mut window = [0u8; 16];
        file.read_exact(&mut window).ok()?;
        if !verified_promotion_window(
            PINNED_PROMOTION_CALL_RVA,
            PINNED_PROMOTION_CALLEE_RVA,
            &window,
        ) {
            return None;
        }
    }
    if mode.is_property() {
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

fn pe_file_offset(file: &mut File, rva: u32) -> Option<u64> {
    let mut dos = [0u8; 64];
    file.seek(SeekFrom::Start(0)).ok()?;
    file.read_exact(&mut dos).ok()?;
    if &dos[..2] != b"MZ" {
        return None;
    }
    let pe = u32::from_le_bytes(dos[0x3c..0x40].try_into().ok()?) as u64;
    file.seek(SeekFrom::Start(pe)).ok()?;
    let mut coff = [0u8; 24];
    file.read_exact(&mut coff).ok()?;
    if &coff[..4] != b"PE\0\0" || u16::from_le_bytes(coff[4..6].try_into().ok()?) != 0x8664 {
        return None;
    }
    let sections = u16::from_le_bytes(coff[6..8].try_into().ok()?) as usize;
    let optional_size = u16::from_le_bytes(coff[20..22].try_into().ok()?) as u64;
    if sections == 0 || sections > 96 || optional_size < 0x70 {
        return None;
    }
    let section_start = pe.checked_add(24)?.checked_add(optional_size)?;
    for i in 0..sections {
        file.seek(SeekFrom::Start(section_start.checked_add((i * 40) as u64)?))
            .ok()?;
        let mut section = [0u8; 40];
        file.read_exact(&mut section).ok()?;
        if &section[..5] != b".text" {
            continue;
        }
        let virtual_size = u32::from_le_bytes(section[8..12].try_into().ok()?);
        let section_rva = u32::from_le_bytes(section[12..16].try_into().ok()?);
        let raw_size = u32::from_le_bytes(section[16..20].try_into().ok()?);
        let raw_offset = u32::from_le_bytes(section[20..24].try_into().ok()?);
        let relative = rva.checked_sub(section_rva)?;
        if relative < virtual_size && relative < raw_size {
            return Some(u64::from(raw_offset) + u64::from(relative));
        }
    }
    None
}

fn guarded_install(config: BridgeConfig) -> BridgeStatus {
    let composite_requested = config.version == EXPERIMENTAL_PAID_CONFIG_VERSION
        && config.mode == BridgeMode::ProductionAdmission as u32;
    if (config.version != CONFIG_VERSION && !composite_requested)
        || config.size as usize != std::mem::size_of::<BridgeConfig>()
        || config.game_pid == 0
        || config.created_filetime == 0
        || !(1..=MAX_ACTIVE_SECONDS).contains(&config.max_seconds)
        || config.trace_address == 0
        || !matches!(config.mode, 0..=8)
        || config.trace_size as usize
            != if composite_requested {
                std::mem::size_of::<CompositeTrace>()
            } else {
                match config.mode {
                    0 => BridgeMode::CanEver.trace_size(),
                    3 => BridgeMode::CanEverCalibration.trace_size(),
                    5 | 6 => BridgeMode::JoinedAdmission.trace_size(),
                    8 => BridgeMode::PromotionCall.trace_size(),
                    7 => BridgeMode::AssignCalibration.trace_size(),
                    _ => BridgeMode::Property.trace_size(),
                }
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
    if config.mode == BridgeMode::PromotionCall as u32 && !supports_atomic_promotion_fetch() {
        return BridgeStatus::InvalidInput;
    }
    let module = unsafe { GetModuleHandleW(wide("GameCore_XP2_FinalRelease.dll").as_ptr()) };
    if module.is_null() || !module_path(module).is_some_and(|path| same_file(&path, GAMECORE)) {
        return BridgeStatus::GameCoreIdentity;
    }
    let mode = match config.mode {
        0 => BridgeMode::CanEver,
        1 => BridgeMode::Property,
        2 => BridgeMode::PropertyScoped,
        3 => BridgeMode::CanEverCalibration,
        4 => BridgeMode::PropertyAdmissionScoped,
        5 => BridgeMode::JoinedAdmission,
        6 => BridgeMode::ProductionAdmission,
        8 => BridgeMode::PromotionCall,
        7 => BridgeMode::AssignCalibration,
        _ => return BridgeStatus::InvalidInput,
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
    let promotion_window = if mode == BridgeMode::PromotionCall {
        let call = unsafe { ptr::read(entry.cast::<[u8; 16]>()) };
        let Some(callee_address) = (module as usize).checked_add(PINNED_PROMOTION_CALLEE_RVA)
        else {
            return BridgeStatus::EntryBytes;
        };
        let callee = callee_address as *const u8;
        if !verified_promotion_window(entry as usize, callee_address, &call)
            || unsafe { ptr::read(callee.sub(5).cast::<[u8; 13]>()) }
                != [
                    0xe9, 0xd0, 0xfc, 0xff, 0xff, 0x40, 0x53, 0x56, 0x57, 0x48, 0x83, 0xec, 0x30,
                ]
        {
            return BridgeStatus::EntryBytes;
        }
        Some(call)
    } else {
        None
    };
    let property_entry = if matches!(
        mode,
        BridgeMode::JoinedAdmission | BridgeMode::ProductionAdmission
    ) {
        let Some(address) = (module as usize).checked_add(PINNED_PROPERTY_GET_RVA) else {
            return BridgeStatus::EntryBytes;
        };
        Some(address as *mut u8)
    } else if mode.is_property() {
        Some(entry)
    } else {
        None
    };
    let property_disk = if let Some(property) = property_entry {
        if (property as usize & (CODE_PAGE - 1)) < PROPERTY_PREFIX_LEN {
            return BridgeStatus::EntryBytes;
        }
        let prefix = unsafe { property.sub(PROPERTY_PREFIX_LEN) };
        let Some(bytes) = pinned_gamecore(Path::new(GAMECORE), BridgeMode::PropertyAdmissionScoped)
        else {
            return BridgeStatus::GameCoreIdentity;
        };
        if !committed_range(prefix as usize, PROPERTY_PREFIX_LEN, 1, false)
            || unsafe { ptr::read(prefix.cast::<[u8; PROPERTY_PREFIX_LEN]>()) }
                != PROPERTY_PREFIX_PAD
            || unsafe { ptr::read(property.cast::<[u8; 8]>()) } != bytes
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
        Some(bytes)
    } else {
        None
    };
    let installed = match mode {
        BridgeMode::AssignCalibration => {
            if ASSIGN_USED
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                return BridgeStatus::HookFailure;
            }
            let trace = config.trace_address as *mut AssignCalibrationTrace;
            unsafe { AssignCalibrationTrace::initialize_at(trace) };
            unsafe { install_assign_calibration_at(entry, trace, disk_bytes) }
        }
        BridgeMode::CanEver => {
            let trace = config.trace_address as *mut SharedTrace;
            unsafe { SharedTrace::initialize_at(trace) };
            unsafe { install_at(entry, trace, disk_bytes) }
        }
        BridgeMode::PromotionCall => {
            // A fetched relay may outlive restoration. Never repoint its
            // global trace/callee during a second session in this process.
            if PROMOTION_USED
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                return BridgeStatus::HookFailure;
            }
            let trace = config.trace_address as *mut PromotionTrace;
            unsafe { PromotionTrace::initialize_at(trace) };
            let callee = unsafe { (module as *mut u8).add(PINNED_PROMOTION_CALLEE_RVA) };
            unsafe { install_promotion_call(entry, callee, trace, promotion_window.unwrap()) }
        }
        BridgeMode::CanEverCalibration => {
            let trace = config.trace_address as *mut CalibrationTrace;
            unsafe { CalibrationTrace::initialize_at(trace) };
            unsafe { install_calibration_at(entry, trace, disk_bytes) }
        }
        BridgeMode::JoinedAdmission | BridgeMode::ProductionAdmission => {
            let trace = config.trace_address as *mut JoinedTrace;
            unsafe { JoinedTrace::initialize_at(trace,mode) };
            joined_deactivate();
            product_deactivate();
            let property = property_entry.expect("verified joined property entry");
            let property_bytes = property_disk.expect("verified joined property bytes");
            if composite_requested {
                if ASSIGN_USED
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_err()
                {
                    return BridgeStatus::HookFailure;
                }
                let Some(assign_bytes) =
                    pinned_gamecore(Path::new(GAMECORE), BridgeMode::AssignCalibration)
                else {
                    return BridgeStatus::GameCoreIdentity;
                };
                let assign = unsafe { (module as *mut u8).add(PINNED_CAN_ASSIGN_RVA) };
                let trace = trace.cast::<CompositeTrace>();
                unsafe { CompositeTrace::initialize_at(trace) };
                return match unsafe {
                    composite::install(
                        entry,
                        property,
                        assign,
                        trace,
                        disk_bytes,
                        property_bytes,
                        assign_bytes,
                    )
                } {
                    Ok(()) => BridgeStatus::Ok,
                    Err(reason) if reason.starts_with("critical:") => BridgeStatus::Critical,
                    Err(_) => BridgeStatus::HookFailure,
                };
            }
            let property_result = unsafe {
                install_property_at(
                    property,
                    ptr::addr_of_mut!((*trace).property),
                    property_bytes,
                )
            };
            match property_result {
                Err(error) => Err(error),
                Ok(()) => {
                    let can_result = unsafe {
                        install_hook(
                            entry,
                            ptr::addr_of_mut!((*trace).calibration).cast(),
                            disk_bytes,
                            mode,
                        )
                    };
                    let active = can_result.is_ok()
                        && if mode == BridgeMode::ProductionAdmission {
                            product_activate()
                        } else {
                            joined_activate()
                        };
                    if active {
                        Ok(())
                    } else {
                        let error = can_result
                            .err()
                            .unwrap_or("admission session generation exhausted");
                        match unsafe { uninstall() } {
                            Ok(()) => Err(error),
                            Err(_) => Err("critical: joined hook rollback failed"),
                        }
                    }
                }
            }
        }
        BridgeMode::Property | BridgeMode::PropertyScoped | BridgeMode::PropertyAdmissionScoped => {
            let trace = config.trace_address as *mut PropertyTrace;
            unsafe {
                PropertyTrace::initialize_at(trace,if mode==BridgeMode::Property {0}else{mode as u32})
            };
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
    let Ok(_guard) = runtime::INSTALL_LOCK.lock() else {
        return BridgeStatus::Panic as u32;
    };
    if runtime::owned() {
        return BridgeStatus::InvalidInput as u32;
    }
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
    let Ok(_guard) = runtime::INSTALL_LOCK.lock() else {
        return BridgeStatus::Panic as u32;
    };
    // The bounded controller cannot revoke an explicitly installed runtime.
    if runtime::owned() {
        return BridgeStatus::InvalidInput as u32;
    }
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
    use std::time::{Duration, Instant};

    #[test]
    fn production_stale_true_cannot_open_without_closed_ack() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        product_deactivate();
        assert!(product_activate());
        let session = PRODUCT_SESSION.load(Ordering::Acquire);
        product_bind(2, 0x1000, session, product_next_ticket().unwrap());
        let publish = |key, payload| {
            let token = product_property_begin(0x1000, key);
            product_observe_property(0x1000, key, token, 1, 1, payload, 0);
        };
        publish(903077, 1);
        publish(901077, 1); // stale true while Lua attempted false
        publish(900077, 1);
        publish(901077, 1);
        assert_eq!(
            product_permission(2, 77, 0x1000, session),
            Some(false),
            "manifest and stale true cannot replace a verified closed acknowledgment"
        );
        product_deactivate();
    }
    fn publish_product(receiver: u64, key: u32, payload: u64) {
        let token = product_property_begin(receiver, key);
        product_observe_property(receiver, key, token, 1, 1, payload, 0);
    }

    fn open_product(receiver: u64, index: u32) {
        publish_product(receiver, PRODUCTION_CLOSE_BASE + index, 1);
        publish_product(receiver, PRODUCTION_ALLOW_BASE + index, 0);
        publish_product(receiver, PRODUCTION_MANIFEST_BASE + index, 1);
        publish_product(receiver, PRODUCTION_ALLOW_BASE + index, 1);
    }

    #[test]
    fn production_owner_reset_binds_from_its_natural_property_read() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        product_deactivate();
        assert!(product_activate());
        let session = PRODUCT_SESSION.load(Ordering::Acquire);
        let receiver = 0x1000;
        let reset = product_property_begin(receiver, PRODUCTION_RESET_KEY + 2);
        assert_ne!(
            reset, 0,
            "owner reset must bind without a Gameplay CanEver call"
        );
        product_observe_property(receiver, PRODUCTION_RESET_KEY + 2, reset, 1, 1, 1, 0);
        open_product(receiver, 77);
        assert_eq!(product_permission(2, 77, receiver, session), Some(true));
        assert_ne!(product_permission(1, 77, receiver, session), Some(true));
        let stale = product_property_begin(receiver, PRODUCTION_ALLOW_BASE + 77);
        let replacement = 0x2000;
        assert_ne!(
            product_property_begin(replacement, PRODUCTION_RESET_KEY + 2),
            0
        );
        product_observe_property(receiver, PRODUCTION_ALLOW_BASE + 77, stale, 1, 1, 1, 0);
        assert_ne!(product_permission(2, 77, receiver, session), Some(true));
        assert_eq!(product_permission(2, 77, replacement, session), Some(false));
        open_product(replacement, 77);
        assert_eq!(product_permission(2, 77, replacement, session), Some(true));
        assert_ne!(
            product_property_begin(replacement, PRODUCTION_RESET_KEY + 3),
            0
        );
        assert_ne!(product_permission(2, 77, replacement, session), Some(true));
        assert_eq!(product_permission(3, 77, replacement, session), Some(false));
        product_bind(2, replacement, session, product_next_ticket().unwrap());
        assert_eq!(product_permission(2, 77, replacement, session), Some(false));
        assert_ne!(product_permission(3, 77, replacement, session), Some(true));
        assert_eq!(
            product_property_begin(replacement, PRODUCTION_RESET_LAST_KEY + 1),
            0
        );
        product_deactivate();
    }

    #[test]
    fn production_reset_closes_before_a_concurrent_appointment_read() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        product_deactivate();
        assert!(product_activate());
        let session = PRODUCT_SESSION.load(Ordering::Acquire);
        let receiver = 0x1000;
        product_bind(2, receiver, session, product_next_ticket().unwrap());
        open_product(receiver, 77);
        assert_eq!(product_permission(2, 77, receiver, session), Some(true));
        PRODUCT_TEST_RESET_PAUSED.store(false, Ordering::Release);
        PRODUCT_TEST_RESET_RELEASE.store(false, Ordering::Release);
        PRODUCT_TEST_RESET_PAUSE.store(true, Ordering::Release);
        let reset =
            std::thread::spawn(move || product_property_begin(receiver, PRODUCTION_RESET_KEY + 2));
        let deadline = Instant::now() + Duration::from_secs(3);
        while !PRODUCT_TEST_RESET_PAUSED.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        let paused = PRODUCT_TEST_RESET_PAUSED.load(Ordering::Acquire);
        let observed = product_permission(2, 77, receiver, session);
        PRODUCT_TEST_RESET_RELEASE.store(true, Ordering::Release);
        let token = reset.join().unwrap();
        PRODUCT_TEST_RESET_PAUSE.store(false, Ordering::Release);
        product_deactivate();
        assert!(paused, "reset did not reach the controlled boundary");
        assert_ne!(token, 0);
        assert_ne!(
            observed,
            Some(true),
            "a reset in progress returned old permission"
        );
    }

    #[test]
    fn production_cache_keys_owner_and_gameinfo_index_and_rejects_old_reads() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        product_deactivate();
        assert!(product_activate());
        let session = PRODUCT_SESSION.load(Ordering::Acquire);
        product_bind(1, 0x1000, session, product_next_ticket().unwrap());
        product_bind(2, 0x2000, session, product_next_ticket().unwrap());
        open_product(0x1000, 77);
        open_product(0x1000, 78);
        assert_eq!(product_permission(1, 77, 0x1000, session), Some(true));
        assert_eq!(product_permission(2, 77, 0x2000, session), Some(false));
        assert_eq!(product_permission(1, 79, 0x1000, session), None);
        let delayed = product_property_begin(0x1000, 901077);
        // Closure is effective before the close getter returns.
        let close = product_property_begin(0x1000, 903077);
        assert_ne!(close, 0);
        assert_eq!(product_permission(1, 77, 0x1000, session), Some(false));
        assert_eq!(product_permission(1, 78, 0x1000, session), Some(true));
        product_observe_property(0x1000, 901077, delayed, 1, 1, 1, 0);
        publish_product(0x1000, 901077, 0);
        publish_product(0x1000, 900077, 1);
        assert_eq!(product_permission(1, 77, 0x1000, session), Some(false));
        publish_product(0x1000, 901077, 1);
        assert_eq!(product_permission(1, 77, 0x1000, session), Some(true));
        let bad = product_property_begin(0x1000, 901077);
        product_observe_property(0x1000, 901077, bad, 1, 2, 1, 0);
        assert_eq!(product_permission(1, 77, 0x1000, session), Some(false));
        let stale = product_property_begin(0x1000, 901078);
        let reset = product_property_begin(0x1000, PRODUCTION_RESET_KEY + 1);
        assert_ne!(reset, 0);
        assert_eq!(product_permission(1, 78, 0x1000, session), Some(false));
        product_observe_property(0x1000, 901078, stale, 1, 1, 1, 0);
        assert_eq!(product_permission(1, 78, 0x1000, session), Some(false));
        // True-only publications after a reset cannot use old false ACKs.
        publish_product(0x1000, 900078, 1);
        publish_product(0x1000, 901078, 1);
        assert_eq!(product_permission(1, 78, 0x1000, session), Some(false));
        open_product(0x1000, 78);
        assert_eq!(product_permission(1, 78, 0x1000, session), Some(true));
        let stale = product_property_begin(0x1000, 901078);
        product_bind(2, 0x1000, session, product_next_ticket().unwrap());
        product_observe_property(0x1000, 901078, stale, 1, 1, 1, 0);
        assert_ne!(product_permission(2, 78, 0x1000, session), Some(true));
        open_product(0x1000, 78);
        assert_eq!(product_permission(2, 78, 0x1000, session), Some(true));
        assert_ne!(product_permission(1, 78, 0x1000, session), Some(true));
        assert_eq!(product_property_begin(0x3000, 901078), 0, "UI is not Core");
        let stale = product_property_begin(0x1000, 901078);
        product_deactivate();
        assert!(product_activate());
        product_bind(
            2,
            0x1000,
            PRODUCT_SESSION.load(Ordering::Acquire),
            product_next_ticket().unwrap(),
        );
        product_observe_property(0x1000, 901078, stale, 1, 1, 1, 0);
        assert_ne!(
            product_permission(2, 78, 0x1000, PRODUCT_SESSION.load(Ordering::Acquire)),
            Some(true)
        );
        product_deactivate();
    }
    #[test]
    fn production_malformed_raw_observation_records_the_same_cell_closed() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        product_deactivate();
        assert!(product_activate());
        let session = PRODUCT_SESSION.load(Ordering::Acquire);
        product_bind(2, 0x1000, session, product_next_ticket().unwrap());
        let mut trace = Box::new(PropertyTrace::production_scoped());
        PROPERTY_TRACE_PTR.store(&mut *trace, Ordering::Release);
        for (tag, payload, flags) in [(2u16, 1u64, 0u32), (1, 2, 0), (1, 1, 7)] {
            open_product(0x1000, 77);
            let mut raw = [0u8; 24];
            raw[0..2].copy_from_slice(&tag.to_le_bytes());
            raw[8..16].copy_from_slice(&payload.to_le_bytes());
            raw[20..24].copy_from_slice(&flags.to_le_bytes());
            let token = product_property_begin(0x1000, 901077);
            unsafe { record_property(0x1000, 901077, raw.as_ptr() as u64, token) };
            assert_eq!(product_permission(2, 77, 0x1000, session), Some(false));
        }
        let rows = trace.complete_rows().unwrap();
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|row| row.denial_owner == 2
            && row.denial_index == 77
            && row.denial_checked == 1
            && row.property == 0x1000));
        PROPERTY_TRACE_PTR.store(null_mut(), Ordering::Release);
        product_deactivate();
    }
    #[test]
    fn production_records_the_consumed_raw_copy_without_rereading_borrowed_storage() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        product_deactivate();
        assert!(product_activate());
        let session = PRODUCT_SESSION.load(Ordering::Acquire);
        product_bind(2, 0x1000, session, product_next_ticket().unwrap());
        open_product(0x1000, 77);
        let mut trace = Box::new(PropertyTrace::production_scoped());
        PROPERTY_TRACE_PTR.store(&mut *trace, Ordering::Release);
        let mut raw = [0u8; 24];
        raw[0] = 1;
        raw[8] = 1;
        let address = raw.as_ptr() as u64;
        PROPERTY_TEST_RAW_MUTATE_RETURNED.store(address, Ordering::Release);
        let token = product_property_begin(0x1000, 901077);
        unsafe { record_property(0x1000, 901077, address, token) };
        PROPERTY_TEST_RAW_MUTATE_RETURNED.store(0, Ordering::Release);
        let actual = trace.complete_rows().unwrap()[0].payload;
        let permit = product_permission(2, 77, 0x1000, session);
        PROPERTY_TRACE_PTR.store(null_mut(), Ordering::Release);
        product_deactivate();
        assert_eq!(raw[8], 0, "fault changed the borrowed storage");
        assert_eq!(
            permit,
            Some(true),
            "consumer applied the first captured true"
        );
        assert_eq!(
            actual, 1,
            "telemetry must contain that same consumed capture"
        );
    }
    #[test]
    fn production_mode_uses_two_original_calls_and_preserves_other_governors() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        product_deactivate();
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
        let can_entry = unsafe { page.add(16) };
        let property_entry = unsafe { page.add(128) };
        let prefix = unsafe { property_entry.sub(PROPERTY_PREFIX_LEN) };
        let can_bytes = [
            0x48, 0x89, 0x5c, 0x24, 0x08, 0xff, 0x41, 0x78, 0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3,
        ];
        let property_bytes = [
            0x48, 0x8b, 0x41, 0x40, 0x4c, 0x8d, 0x49, 0x30, 0xff, 0x41, 0x48, 0xc3,
        ];
        unsafe {
            ptr::copy_nonoverlapping(can_bytes.as_ptr(), can_entry, can_bytes.len());
            ptr::copy_nonoverlapping(
                property_bytes.as_ptr(),
                property_entry,
                property_bytes.len(),
            );
            ptr::copy_nonoverlapping(PROPERTY_PREFIX_PAD.as_ptr(), prefix, PROPERTY_PREFIX_LEN);
        }
        #[repr(align(8))]
        struct PlayerBytes([u8; 0x600]);
        let mut owner = PlayerBytes([0; 0x600]);
        owner.0[0xd8..0xdc].copy_from_slice(&2u32.to_le_bytes());
        let mut value = [0u8; 24];
        value[..2].copy_from_slice(&1u16.to_le_bytes());
        owner.0[0x568..0x570].copy_from_slice(&(value.as_ptr() as u64).to_le_bytes());
        let mut gov = [0u8; 0x80];
        gov[0x70..0x78].copy_from_slice(&(owner.0.as_ptr() as u64).to_le_bytes());
        let can: extern "system" fn(usize, u32) -> u8 = unsafe { std::mem::transmute(can_entry) };
        let property: extern "system" fn(usize, u32) -> usize =
            unsafe { std::mem::transmute(property_entry) };
        let receiver = owner.0.as_ptr() as usize + 0x528;
        let mut trace = Box::new(JoinedTrace::default());
        trace.property = PropertyTrace::production_scoped();
        unsafe { install_property_at(property_entry, &mut trace.property, PROPERTY_PROLOGUE) }
            .unwrap();
        unsafe {
            install_hook(
                can_entry,
                (&mut trace.calibration as *mut CalibrationTrace).cast(),
                can_bytes[..8].try_into().unwrap(),
                BridgeMode::ProductionAdmission,
            )
        }
        .unwrap();
        assert!(product_activate());
        let publish = |key, payload| {
            value[8] = payload;
            assert_eq!(property(receiver, key), value.as_ptr() as usize);
        };
        let mut publish = publish;
        publish(PRODUCTION_RESET_KEY + 2, 1);
        assert_eq!(
            joined_receiver(PRODUCT_OWNERS[2].load()),
            receiver as u64,
            "Win64 property gateway binds owner before any CanEver call"
        );
        assert_eq!(can(gov.as_ptr() as usize, 77), 1, "unknown keeps original");
        publish(PRODUCTION_CLOSE_BASE + 77, 1);
        publish(PRODUCTION_ALLOW_BASE + 77, 0);
        publish(PRODUCTION_MANIFEST_BASE + 77, 1);
        assert_eq!(can(gov.as_ptr() as usize, 77), 0);
        publish(PRODUCTION_ALLOW_BASE + 77, 1);
        assert_eq!(can(gov.as_ptr() as usize, 77), 1);
        assert_eq!(
            can(gov.as_ptr() as usize, 78),
            1,
            "other mod governor keeps original"
        );
        // Property sample exhaustion does not affect authority. Both a false
        // and a true after the final sampled row still change the actual gate.
        for _ in 0..TRACE_CAPACITY + 40 {
            publish(PRODUCTION_ALLOW_BASE + 77, 1);
        }
        assert_eq!(can(gov.as_ptr() as usize, 77), 1);
        publish(PRODUCTION_ALLOW_BASE + 77, 0);
        assert_eq!(can(gov.as_ptr() as usize, 77), 0);
        publish(PRODUCTION_ALLOW_BASE + 77, 1);
        assert_eq!(can(gov.as_ptr() as usize, 77), 1);
        for _ in 0..CALIBRATION_CAPACITY + 80 {
            assert_eq!(can(gov.as_ptr() as usize, 77), 1);
        }
        assert_eq!(can(gov.as_ptr() as usize, 78), 1);
        assert_eq!(
            u32::from_le_bytes(gov[0x78..0x7c].try_into().unwrap()),
            trace.calibration.next.load(Ordering::Acquire),
            "original CanEver once for every sampled and unsampled call"
        );
        assert!(trace.calibration.dropped.load(Ordering::Acquire) > 0);
        assert_eq!(
            trace.calibration.complete_rows().unwrap().len(),
            CALIBRATION_CAPACITY
        );
        assert_eq!(
            u32::from_le_bytes(owner.0[0x570..0x574].try_into().unwrap()),
            trace.property.next.load(Ordering::Acquire),
            "original getter once; native consumer invokes no additional getter"
        );
        gov[0x70..0x78].fill(0);
        assert_eq!(
            can(gov.as_ptr() as usize, 77),
            0,
            "unknown owner closes known persona"
        );
        assert_eq!(
            can(gov.as_ptr() as usize, 78),
            1,
            "unknown owner preserves ordinary Governor"
        );
        assert!(trace.property.dropped.load(Ordering::Acquire) > 0);
        assert_eq!(
            trace.property.complete_rows().unwrap().len(),
            TRACE_CAPACITY
        );
        let original_calls = u32::from_le_bytes(gov[0x78..0x7c].try_into().unwrap());
        let getter_calls = u32::from_le_bytes(owner.0[0x570..0x574].try_into().unwrap());
        assert_eq!(
            original_calls,
            trace.calibration.next.load(Ordering::Acquire)
        );
        println!(
            "production original_count={} getter_original_count={} calibration_unsampled={} property_unsampled={}",
            original_calls,
            getter_calls,
            trace.calibration.dropped.load(Ordering::Acquire),
            trace.property.dropped.load(Ordering::Acquire)
        );
        unsafe { uninstall() }.unwrap();
        assert_eq!(
            unsafe { ptr::read(can_entry.cast::<[u8; 8]>()) },
            can_bytes[..8]
        );
        assert_eq!(
            unsafe { ptr::read(property_entry.cast::<[u8; 8]>()) },
            PROPERTY_PROLOGUE
        );
        assert_eq!(
            trace.property.snapshot_rows().unwrap().len(),
            TRACE_CAPACITY
        );
        assert!(trace.property.dropped.load(Ordering::Acquire) > 0);
        assert_ne!(unsafe { VirtualFree(page.cast(), 0, MEM_RELEASE) }, 0);
    }

    #[test]
    fn delayed_receiver_bind_cannot_replace_a_newer_published_receiver() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        joined_deactivate();
        assert!(joined_activate());
        let session = joined_session(JOINED_RECEIVER_PERMISSION.load());
        joined_bind(0x1000, session, joined_next_ticket().unwrap());
        JOINED_TEST_BIND_RECEIVER.store(0x2000, Ordering::Release);
        JOINED_TEST_BIND_PAUSED.store(false, Ordering::Release);
        JOINED_TEST_BIND_RELEASE.store(false, Ordering::Release);
        let old_ticket = joined_next_ticket().unwrap();
        let mut old = PausedFixtureWorker::new(
            &JOINED_TEST_BIND_RELEASE,
            std::thread::spawn(move || joined_bind(0x2000, session, old_ticket)),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while !JOINED_TEST_BIND_PAUSED.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        if !JOINED_TEST_BIND_PAUSED.load(Ordering::Acquire) {
            JOINED_TEST_BIND_RELEASE.store(true, Ordering::Release);
            old.join().unwrap();
            joined_deactivate();
            panic!("old receiver bind did not pause before its compare-exchange");
        }
        joined_bind(0x3000, session, joined_next_ticket().unwrap());
        let token = unsafe { joined_property_begin(0x3000, ASHINA_ADMISSION_KEY_ID) };
        joined_observe_property(0x3000, ASHINA_ADMISSION_KEY_ID, token, 1, 1, 1, 0);
        let newest = JOINED_RECEIVER_PERMISSION.load();
        assert_eq!(joined_receiver(newest), 0x3000);
        assert!(joined_permitted(newest));
        JOINED_TEST_BIND_RELEASE.store(true, Ordering::Release);
        old.join().unwrap();
        let after = JOINED_RECEIVER_PERMISSION.load();
        JOINED_TEST_BIND_RECEIVER.store(0, Ordering::Release);
        joined_deactivate();
        assert_eq!(
            after, newest,
            "a stale receiver replaced a newer admitted receiver"
        );
    }
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry;
    use windows_sys::Win32::System::Threading::{
        CreateThread, GetExitCodeThread, ResumeThread, WaitForSingleObject, THREAD_CREATE_SUSPENDED,
    };

    pub(super) static HOOK_TEST_LOCK: Mutex<()> = Mutex::new(());

    struct PausedFixtureWorker<T> {
        release: &'static AtomicBool,
        worker: Option<std::thread::JoinHandle<T>>,
    }

    impl<T> PausedFixtureWorker<T> {
        fn new(release: &'static AtomicBool, worker: std::thread::JoinHandle<T>) -> Self {
            Self {
                release,
                worker: Some(worker),
            }
        }

        fn join(&mut self) -> std::thread::Result<T> {
            self.release.store(true, Ordering::Release);
            self.worker.take().unwrap().join()
        }
    }

    impl<T> Drop for PausedFixtureWorker<T> {
        fn drop(&mut self) {
            if let Some(worker) = self.worker.take() {
                self.release.store(true, Ordering::Release);
                let _ = worker.join();
            }
        }
    }

    struct HookFixtureGuard;

    impl Drop for HookFixtureGuard {
        fn drop(&mut self) {
            let _ = unsafe { uninstall() };
        }
    }

    fn wait_for_synthetic_phase(
        progress: &std::sync::mpsc::Receiver<(u32, u32)>,
        expected_phase: u32,
    ) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut observed = 0u32;
        while observed != 0b1111 {
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return false;
            };
            let Ok((owner, phase)) = progress.recv_timeout(remaining) else {
                return false;
            };
            if owner >= 4 || phase != expected_phase || observed & (1 << owner) != 0 {
                return false;
            }
            observed |= 1 << owner;
        }
        true
    }

    struct SyntheticCallers {
        phase: Arc<AtomicU32>,
        workers: Vec<std::thread::JoinHandle<u32>>,
    }

    impl SyntheticCallers {
        fn stop_and_join(&mut self) -> Result<u32, &'static str> {
            self.phase.store(3, Ordering::Release);
            let mut calls = 0;
            let mut failed = false;
            for worker in self.workers.drain(..) {
                match worker.join() {
                    Ok(count) => calls += count,
                    Err(_) => failed = true,
                }
            }
            if failed {
                Err("synthetic caller panicked")
            } else {
                Ok(calls)
            }
        }
    }

    impl Drop for SyntheticCallers {
        fn drop(&mut self) {
            // The trace is declared before this guard and remains alive through
            // worker collection, including while the test body is unwinding.
            let _ = self.stop_and_join();
            let _ = unsafe { uninstall() };
        }
    }

    type PromotionWorker = (u64, u32, u64, usize);

    struct PromotionCallers {
        running: Arc<AtomicBool>,
        workers: Vec<std::thread::JoinHandle<PromotionWorker>>,
    }

    impl PromotionCallers {
        fn stop_and_join(&mut self) -> Result<Vec<PromotionWorker>, &'static str> {
            self.running.store(false, Ordering::Release);
            let mut joined = Vec::with_capacity(self.workers.len());
            let mut failed = false;
            for worker in self.workers.drain(..) {
                match worker.join() {
                    Ok(result) => joined.push(result),
                    Err(_) => failed = true,
                }
            }
            if failed {
                Err("promotion caller panicked")
            } else {
                Ok(joined)
            }
        }
    }

    impl Drop for PromotionCallers {
        fn drop(&mut self) {
            let _ = self.stop_and_join();
            let _ = unsafe { restore_promotion_call() };
        }
    }

    fn spawn_synthetic_callers(
        phase: &Arc<AtomicU32>,
        call: impl Fn(u32) + Send + Sync + 'static,
    ) -> (SyntheticCallers, std::sync::mpsc::Receiver<(u32, u32)>) {
        spawn_synthetic_callers_with_hook(phase, call, |_| {})
    }

    fn spawn_synthetic_callers_with_hook(
        phase: &Arc<AtomicU32>,
        call: impl Fn(u32) + Send + Sync + 'static,
        after_spawn: impl Fn(usize),
    ) -> (SyntheticCallers, std::sync::mpsc::Receiver<(u32, u32)>) {
        let call = Arc::new(call);
        let (send, progress) = std::sync::mpsc::channel();
        // Own each handle as soon as it exists, so a later spawn panic still
        // stops and joins the already running callers before their trace drops.
        let mut callers = SyntheticCallers {
            phase: Arc::clone(phase),
            workers: Vec::new(),
        };
        for owner in 0..4 {
            let phase = Arc::clone(phase);
            let call = Arc::clone(&call);
            let send = send.clone();
            callers.workers.push(std::thread::spawn(move || {
                let mut last_phase = u32::MAX;
                let mut calls = 0;
                while calls < 1_000_000 {
                    let current = phase.load(Ordering::Acquire);
                    if current == 3 {
                        break;
                    }
                    call(owner);
                    calls += 1;
                    if current != last_phase {
                        send.send((owner, current)).unwrap();
                        last_phase = current;
                    }
                    std::thread::yield_now();
                }
                calls
            }));
            after_spawn(callers.workers.len());
        }
        (callers, progress)
    }

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
        let unwind = unsafe { ptr::read(unwind_address as *const [u8; 8]) };
        assert_eq!(unwind[0] & 0x07, 1); // UNWIND_INFO version 1
        assert_eq!(unwind[2], 2); // UWOP_ALLOC_LARGE uses two unwind slots
        assert_eq!(unwind[5] & 0x0f, 1);
        assert_eq!(u16::from_le_bytes([unwind[6], unwind[7]]), 0xd8 / 8);
        assert!(!unsafe {
            RtlLookupFunctionEntry(
                joined_cas128 as *const () as u64,
                &mut image_base,
                null_mut(),
            )
        }
        .is_null());
        let promotion = unsafe {
            RtlLookupFunctionEntry(
                promotion_call_gateway as *const () as u64,
                &mut image_base,
                null_mut(),
            )
        };
        assert!(
            !promotion.is_null(),
            "promotion gateway calls a recorder and changes rsp"
        );
        let promotion_rva = promotion_call_gateway as *const () as u64 - image_base;
        assert!(unsafe { (*promotion).BeginAddress as u64 } <= promotion_rva);
        assert!(promotion_rva < unsafe { (*promotion).EndAddress as u64 });
    }

    #[test]
    fn joined_atomic_word_compares_all_receiver_generation_and_permission_bits() {
        assert_eq!(std::mem::size_of::<AtomicJoinedWord>(), 16);
        assert_eq!(std::mem::align_of::<AtomicJoinedWord>(), 16);
        let word = AtomicJoinedWord::new();
        assert_eq!(word.load(), 0);
        let first = joined_word(u64::MAX, u32::MAX, JOINED_TICKET_MAX, true);
        assert!(word.compare_exchange(0, first).is_ok());
        assert_eq!(word.load(), first);
        assert_eq!(word.compare_exchange(0, 0), Err(first));
        word.store(joined_word(0, 1, 2, false));
        assert_eq!(joined_receiver(word.load()), 0);
        assert_eq!(joined_session(word.load()), 1);
        assert_eq!(joined_ticket(word.load()), 2);
        assert!(!joined_permitted(word.load()));
    }

    #[test]
    fn calibration_preserves_original_result_and_rereads_current_owner() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        let mut image_base = 0u64;
        assert!(!unsafe {
            RtlLookupFunctionEntry(
                calibration_gateway as *const () as u64,
                &mut image_base,
                null_mut(),
            )
        }
        .is_null());
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
        // Pinned five-byte prologue, then increment an owned invocation count.
        // The synthetic original returns the low bit of Governor Index.
        let bytes = [
            0x48, 0x89, 0x5c, 0x24, 0x08, 0xff, 0x41, 0x78, 0x89, 0xd0, 0x83, 0xe0, 0x01, 0xc3,
        ];
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len()) };
        let original: [u8; 8] = bytes[..8].try_into().unwrap();
        let f: extern "system" fn(usize, u32) -> u8 = unsafe { std::mem::transmute(target) };
        let mut governors = [0u64; 16];
        let mut first_player = [0u8; 0x600];
        let mut next_player = [0u8; 0x600];
        first_player[0xd8..0xdc].copy_from_slice(&0u32.to_le_bytes());
        next_player[0xd8..0xdc].copy_from_slice(&1u32.to_le_bytes());
        governors[14] = first_player.as_ptr() as u64; // +0x70
        let mut trace = Box::new(CalibrationTrace::default());
        unsafe { install_calibration_at(target, trace.as_mut(), original) }.unwrap();
        assert_eq!(f(governors.as_ptr() as usize, 4), 0);
        governors[14] = next_player.as_ptr() as u64;
        assert_eq!(f(governors.as_ptr() as usize, 5), 1);
        governors[14] = 1; // Bad owner field cannot become a positive observation.
        assert_eq!(f(governors.as_ptr() as usize, 4), 0);
        unsafe { uninstall() }.unwrap();
        let rows = trace.complete_rows().unwrap();
        assert_eq!(governors[15], 3, "original function ran once per call");
        assert_eq!(
            (
                rows[0].player_id,
                rows[0].governor_index,
                rows[0].original_al
            ),
            (0, 4, 0)
        );
        assert_eq!(
            (
                rows[1].player_id,
                rows[1].governor_index,
                rows[1].original_al
            ),
            (1, 5, 1)
        );
        assert_eq!(rows[0].owner_read_state, 1);
        assert_eq!(rows[1].owner_read_state, 1);
        assert_eq!(rows[2].owner_read_state, 2);
        assert_eq!(
            rows[0].property_receiver,
            first_player.as_ptr() as u64 + 0x528
        );
        assert_eq!(
            rows[1].property_receiver,
            next_player.as_ptr() as u64 + 0x528
        );
        assert_eq!(unsafe { ptr::read(target.cast::<[u8; 8]>()) }, original);
        assert_ne!(unsafe { VirtualFree(page.cast(), 0, MEM_RELEASE) }, 0);
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
        let phase = Arc::new(AtomicU32::new(0));
        let address = target as usize;
        let mut trace = Box::new(PropertyTrace::default());
        let (mut workers, progress) = spawn_synthetic_callers(&phase, move |_| {
            let first: extern "system" fn(usize, u32) -> u32 =
                unsafe { std::mem::transmute(address) };
            let second: extern "system" fn(usize) -> u32 =
                unsafe { std::mem::transmute(address + PROPERTY_FIRST_LEN) };
            assert_eq!(first(argument, 0), expected);
            assert_eq!(second(argument), expected);
        });
        let before_hook = wait_for_synthetic_phase(&progress, 0);
        unsafe { install_property_at(target, trace.as_mut(), PROPERTY_PROLOGUE) }.unwrap();
        phase.store(1, Ordering::Release);
        let during_hook = wait_for_synthetic_phase(&progress, 1);
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
        phase.store(2, Ordering::Release);
        let after_restore = wait_for_synthetic_phase(&progress, 2);
        assert!(workers.stop_and_join().unwrap() > 0);
        assert_eq!(
            unsafe { ptr::read(target.cast::<[u8; 8]>()) },
            PROPERTY_PROLOGUE
        );
        assert_eq!(
            unsafe { ptr::read(slot.cast::<[u8; PROPERTY_PREFIX_LEN]>()) }[0],
            0xe9
        );
        assert_ne!(unsafe { VirtualFree(page.cast(), 0, MEM_RELEASE) }, 0);
        assert!(before_hook, "each caller must execute before installation");
        assert!(
            during_hook,
            "each caller must execute while the hook is installed"
        );
        assert!(after_restore, "each caller must execute after restoration");
    }

    #[test]
    fn property_trampoline_calls_original_once_and_preserves_pointer_or_missing() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        assert_eq!(std::mem::size_of::<PropertyRow>(), 48);
        assert_eq!(std::mem::offset_of!(PropertyTrace, rows), 32);
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
    fn numeric_scope_ignores_large_unrelated_getter_traffic_before_reserving_rows() {
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
        let prefix = unsafe { target.sub(PROPERTY_PREFIX_LEN) };
        unsafe {
            ptr::copy_nonoverlapping(PROPERTY_PREFIX_PAD.as_ptr(), prefix, PROPERTY_PREFIX_LEN)
        };
        let bytes = [0x48, 0x8b, 0x41, 0x40, 0x4c, 0x8d, 0x49, 0x30, 0xc3];
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len()) };
        let mut value = [0u8; 24];
        value[..2].copy_from_slice(&2u16.to_le_bytes());
        value[8..16].copy_from_slice(&(PROPERTY_TEST_VALUE_KEY_ID as u64).to_le_bytes());
        let mut property = [0u64; 9];
        property[8] = value.as_ptr() as u64;
        let f: extern "system" fn(usize, u32) -> usize = unsafe { std::mem::transmute(target) };
        let mut trace = Box::new(PropertyTrace::scoped());
        unsafe { install_property_at(target, trace.as_mut(), PROPERTY_PROLOGUE) }.unwrap();
        for _ in 0..120_000 {
            assert_eq!(
                f(property.as_ptr() as usize, 583_956_078),
                value.as_ptr() as usize
            );
        }
        assert_eq!(
            f(property.as_ptr() as usize, PROPERTY_TEST_VALUE_KEY_ID),
            value.as_ptr() as usize
        );
        assert_eq!(
            f(property.as_ptr() as usize, PROPERTY_TEST_PHASE_KEY_ID),
            value.as_ptr() as usize
        );
        unsafe { uninstall() }.unwrap();
        assert_eq!(trace.filtered.load(Ordering::Acquire), 120_000);
        assert_eq!(trace.dropped.load(Ordering::Acquire), 0);
        assert_eq!(trace.next.load(Ordering::Acquire), 2);
        let rows = trace.complete_rows().unwrap();
        assert_eq!(
            rows.iter().map(|r| r.key_id).collect::<Vec<_>>(),
            [PROPERTY_TEST_VALUE_KEY_ID, PROPERTY_TEST_PHASE_KEY_ID]
        );
        assert!(rows
            .iter()
            .all(|r| r.tag == 2 && r.payload == PROPERTY_TEST_VALUE_KEY_ID as u64));
        assert_eq!(
            unsafe { ptr::read(target.cast::<[u8; 8]>()) },
            PROPERTY_PROLOGUE
        );
        assert_ne!(unsafe { VirtualFree(page.cast(), 0, MEM_RELEASE) }, 0);
    }

    #[test]
    fn admission_scope_records_only_the_fixed_ashina_numeric_key() {
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
        let prefix = unsafe { target.sub(PROPERTY_PREFIX_LEN) };
        unsafe {
            ptr::copy_nonoverlapping(PROPERTY_PREFIX_PAD.as_ptr(), prefix, PROPERTY_PREFIX_LEN)
        };
        let bytes = [0x48, 0x8b, 0x41, 0x40, 0x4c, 0x8d, 0x49, 0x30, 0xc3];
        unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len()) };
        let mut value = [0u8; 24];
        value[..2].copy_from_slice(&1u16.to_le_bytes());
        value[8] = 1;
        let mut property = [0u64; 9];
        property[8] = value.as_ptr() as u64;
        let f: extern "system" fn(usize, u32) -> usize = unsafe { std::mem::transmute(target) };
        let mut trace = Box::new(PropertyTrace::admission_scoped());
        unsafe { install_property_at(target, trace.as_mut(), PROPERTY_PROLOGUE) }.unwrap();
        assert_eq!(
            f(property.as_ptr() as usize, PROPERTY_TEST_VALUE_KEY_ID),
            value.as_ptr() as usize
        );
        assert_eq!(
            f(property.as_ptr() as usize, ASHINA_ADMISSION_KEY_ID),
            value.as_ptr() as usize
        );
        unsafe { uninstall() }.unwrap();
        assert_eq!(trace.filtered.load(Ordering::Acquire), 1);
        let rows = trace.complete_rows().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            (rows[0].key_id, rows[0].tag, rows[0].payload),
            (ASHINA_ADMISSION_KEY_ID, 1, 1)
        );
        assert_ne!(unsafe { VirtualFree(page.cast(), 0, MEM_RELEASE) }, 0);
    }

    #[test]
    fn joined_gate_opens_only_bound_owner_after_native_core_bool_and_restores_both_entries() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        joined_deactivate();
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
        let can_entry = unsafe { page.add(16) };
        let property_entry = unsafe { page.add(128) };
        let prefix = unsafe { property_entry.sub(PROPERTY_PREFIX_LEN) };
        let can_bytes = [
            0x48, 0x89, 0x5c, 0x24, 0x08, 0xff, 0x41, 0x78, 0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3,
        ];
        let property_bytes = [
            0x48, 0x8b, 0x41, 0x40, 0x4c, 0x8d, 0x49, 0x30, 0xff, 0x41, 0x48, 0xc3,
        ];
        unsafe {
            ptr::copy_nonoverlapping(can_bytes.as_ptr(), can_entry, can_bytes.len());
            ptr::copy_nonoverlapping(
                property_bytes.as_ptr(),
                property_entry,
                property_bytes.len(),
            );
            ptr::copy_nonoverlapping(PROPERTY_PREFIX_PAD.as_ptr(), prefix, PROPERTY_PREFIX_LEN);
        }
        #[repr(align(8))]
        struct PlayerBytes([u8; 0x600]);
        let mut owner = PlayerBytes([0u8; 0x600]);
        let mut other = PlayerBytes([0u8; 0x600]);
        other.0[0xd8..0xdc].copy_from_slice(&1u32.to_le_bytes());
        let mut value = [0u8; 24];
        value[..2].copy_from_slice(&1u16.to_le_bytes());
        owner.0[0x568..0x570].copy_from_slice(&(value.as_ptr() as u64).to_le_bytes());
        let mut gov = [0u8; 0x80];
        gov[0x70..0x78].copy_from_slice(&(owner.0.as_ptr() as u64).to_le_bytes());
        let can: extern "system" fn(usize, u32) -> u8 = unsafe { std::mem::transmute(can_entry) };
        let property: extern "system" fn(usize, u32) -> usize =
            unsafe { std::mem::transmute(property_entry) };
        let mut trace = Box::new(JoinedTrace::default());
        unsafe { install_property_at(property_entry, &mut trace.property, PROPERTY_PROLOGUE) }
            .unwrap();
        unsafe {
            install_hook(
                can_entry,
                (&mut trace.calibration as *mut CalibrationTrace).cast(),
                can_bytes[..8].try_into().unwrap(),
                BridgeMode::JoinedAdmission,
            )
        }
        .unwrap();
        assert!(joined_activate());
        assert_eq!(can(gov.as_ptr() as usize, 4), 0);
        assert_eq!(
            property(owner.0.as_ptr() as usize + 0x528, ASHINA_ADMISSION_KEY_ID),
            value.as_ptr() as usize
        );
        assert_eq!(can(gov.as_ptr() as usize, 4), 0);
        value[8] = 1;
        assert_eq!(
            property(owner.0.as_ptr() as usize + 0x528, ASHINA_ADMISSION_KEY_ID),
            value.as_ptr() as usize
        );
        assert_eq!(can(gov.as_ptr() as usize, 4), 1);
        value[8] = 0;
        let _ = property(owner.0.as_ptr() as usize + 0x528, ASHINA_ADMISSION_KEY_ID);
        assert_eq!(can(gov.as_ptr() as usize, 4), 0, "false closes admission");
        value[8] = 1;
        let _ = property(owner.0.as_ptr() as usize + 0x528, ASHINA_ADMISSION_KEY_ID);
        assert_eq!(can(gov.as_ptr() as usize, 4), 1);
        assert_eq!(
            can(gov.as_ptr() as usize, 5),
            1,
            "unrelated index keeps original result"
        );
        gov[0x70..0x78].copy_from_slice(&(other.0.as_ptr() as u64).to_le_bytes());
        assert_eq!(can(gov.as_ptr() as usize, 4), 0);
        other.0[0xd8..0xdc].copy_from_slice(&0u32.to_le_bytes());
        other.0[0x568..0x570].copy_from_slice(&(value.as_ptr() as u64).to_le_bytes());
        assert_eq!(
            can(gov.as_ptr() as usize, 4),
            0,
            "new Core receiver starts closed"
        );
        let _ = property(other.0.as_ptr() as usize + 0x528, ASHINA_ADMISSION_KEY_ID);
        assert_eq!(can(gov.as_ptr() as usize, 4), 1);
        gov[0x70..0x78].copy_from_slice(&(owner.0.as_ptr() as u64).to_le_bytes());
        assert_eq!(
            can(gov.as_ptr() as usize, 4),
            0,
            "A-B-A changes pointer generation"
        );
        let _ = property(owner.0.as_ptr() as usize + 0x528, ASHINA_ADMISSION_KEY_ID);
        assert_eq!(can(gov.as_ptr() as usize, 4), 1);
        value[..2].copy_from_slice(&2u16.to_le_bytes());
        let _ = property(owner.0.as_ptr() as usize + 0x528, ASHINA_ADMISSION_KEY_ID);
        assert_eq!(
            can(gov.as_ptr() as usize, 4),
            0,
            "bad native tag closes admission"
        );
        value[..2].copy_from_slice(&1u16.to_le_bytes());
        owner.0[0x568..0x570].copy_from_slice(&1u64.to_le_bytes());
        assert_eq!(
            property(owner.0.as_ptr() as usize + 0x528, ASHINA_ADMISSION_KEY_ID),
            1
        );
        assert_eq!(
            can(gov.as_ptr() as usize, 4),
            0,
            "unreadable value closes admission"
        );
        owner.0[0x568..0x570].copy_from_slice(&(value.as_ptr() as u64).to_le_bytes());
        let _ = property(owner.0.as_ptr() as usize + 0x528, ASHINA_ADMISSION_KEY_ID);
        assert_eq!(can(gov.as_ptr() as usize, 4), 1);
        let remaining = TRACE_CAPACITY as u32 - trace.property.next.load(Ordering::Acquire);
        for _ in 0..remaining {
            let _ = property(owner.0.as_ptr() as usize + 0x528, ASHINA_ADMISSION_KEY_ID);
        }
        assert_eq!(
            trace.property.next.load(Ordering::Acquire),
            TRACE_CAPACITY as u32
        );
        assert_eq!(can(gov.as_ptr() as usize, 4), 1);
        let _ = property(owner.0.as_ptr() as usize + 0x528, ASHINA_ADMISSION_KEY_ID);
        assert_eq!(
            can(gov.as_ptr() as usize, 4),
            0,
            "capacity closes admission"
        );
        assert_eq!(trace.property.dropped.load(Ordering::Acquire), 1);
        JOINED_NEXT_TICKET.store(JOINED_TICKET_MAX, Ordering::Release);
        let _ = property(owner.0.as_ptr() as usize + 0x528, ASHINA_ADMISSION_KEY_ID);
        assert!(JOINED_TICKET_EXHAUSTED.load(Ordering::Acquire));
        assert!(JOINED_ACTIVE.load(Ordering::Acquire));
        assert_eq!(
            can(gov.as_ptr() as usize, 4),
            0,
            "ticket exhaustion stays closed"
        );
        unsafe { uninstall() }.unwrap();
        assert_eq!(can(gov.as_ptr() as usize, 4), 1);
        assert_eq!(
            unsafe { ptr::read(can_entry.cast::<[u8; 8]>()) },
            can_bytes[..8]
        );
        assert_eq!(
            unsafe { ptr::read(property_entry.cast::<[u8; 8]>()) },
            PROPERTY_PROLOGUE
        );
        assert!(trace
            .calibration
            .complete_rows()
            .unwrap()
            .iter()
            .any(|row| row.player_id == 0
                && row.governor_index == 4
                && row.original_al == 1
                && row.effective_al == 1));
        assert_ne!(unsafe { VirtualFree(page.cast(), 0, MEM_RELEASE) }, 0);
    }

    #[test]
    fn joined_late_true_getter_cannot_reopen_after_newer_false() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        joined_deactivate();
        JOINED_TEST_PAUSED.store(false, Ordering::Release);
        JOINED_TEST_RELEASE.store(false, Ordering::Release);
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
        let can_entry = unsafe { page.add(16) };
        let property_entry = unsafe { page.add(128) };
        let prefix = unsafe { property_entry.sub(PROPERTY_PREFIX_LEN) };
        let can_bytes = [
            0x48, 0x89, 0x5c, 0x24, 0x08, 0xff, 0x41, 0x78, 0xb8, 0x01, 0x00, 0x00, 0x00, 0xc3,
        ];
        let property_bytes = [
            0x48, 0x8b, 0x41, 0x40, 0x4c, 0x8d, 0x49, 0x30, 0xff, 0x41, 0x48, 0xc3,
        ];
        unsafe {
            ptr::copy_nonoverlapping(can_bytes.as_ptr(), can_entry, can_bytes.len());
            ptr::copy_nonoverlapping(
                property_bytes.as_ptr(),
                property_entry,
                property_bytes.len(),
            );
            ptr::copy_nonoverlapping(PROPERTY_PREFIX_PAD.as_ptr(), prefix, PROPERTY_PREFIX_LEN);
        }
        #[repr(align(8))]
        struct PlayerBytes([u8; 0x600]);
        let mut owner = PlayerBytes([0u8; 0x600]);
        let other = PlayerBytes([0u8; 0x600]);
        let mut yes = [0u8; 24];
        yes[..2].copy_from_slice(&1u16.to_le_bytes());
        yes[8] = 1;
        let mut no = [0u8; 24];
        no[..2].copy_from_slice(&1u16.to_le_bytes());
        owner.0[0x568..0x570].copy_from_slice(&(yes.as_ptr() as u64).to_le_bytes());
        let mut gov = [0u8; 0x80];
        gov[0x70..0x78].copy_from_slice(&(owner.0.as_ptr() as u64).to_le_bytes());
        let receiver = owner.0.as_ptr() as usize + 0x528;
        let can: extern "system" fn(usize, u32) -> u8 = unsafe { std::mem::transmute(can_entry) };
        let property: extern "system" fn(usize, u32) -> usize =
            unsafe { std::mem::transmute(property_entry) };
        let mut trace = Box::new(JoinedTrace::default());
        unsafe { install_property_at(property_entry, &mut trace.property, PROPERTY_PROLOGUE) }
            .unwrap();
        let _hook_cleanup = HookFixtureGuard;
        unsafe {
            install_hook(
                can_entry,
                (&mut trace.calibration as *mut CalibrationTrace).cast(),
                can_bytes[..8].try_into().unwrap(),
                BridgeMode::JoinedAdmission,
            )
        }
        .unwrap();
        assert!(joined_activate());
        assert_eq!(can(gov.as_ptr() as usize, 4), 0);
        JOINED_TEST_DELAY_RETURNED.store(yes.as_ptr() as u64, Ordering::Release);
        let mut old = PausedFixtureWorker::new(
            &JOINED_TEST_RELEASE,
            std::thread::spawn(move || property(receiver, ASHINA_ADMISSION_KEY_ID)),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while !JOINED_TEST_PAUSED.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        if !JOINED_TEST_PAUSED.load(Ordering::Acquire) {
            JOINED_TEST_RELEASE.store(true, Ordering::Release);
            old.join().unwrap();
            panic!("old native getter did not pause after returning true");
        }
        owner.0[0x568..0x570].copy_from_slice(&(no.as_ptr() as u64).to_le_bytes());
        assert_eq!(
            property(receiver, ASHINA_ADMISSION_KEY_ID),
            no.as_ptr() as usize
        );
        assert_eq!(can(gov.as_ptr() as usize, 4), 0);
        JOINED_TEST_RELEASE.store(true, Ordering::Release);
        assert_eq!(old.join().unwrap(), yes.as_ptr() as usize);
        let after_late_true = can(gov.as_ptr() as usize, 4);
        owner.0[0x568..0x570].copy_from_slice(&(yes.as_ptr() as u64).to_le_bytes());
        JOINED_TEST_PAUSED.store(false, Ordering::Release);
        JOINED_TEST_RELEASE.store(false, Ordering::Release);
        let mut old_session = PausedFixtureWorker::new(
            &JOINED_TEST_RELEASE,
            std::thread::spawn(move || property(receiver, ASHINA_ADMISSION_KEY_ID)),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while !JOINED_TEST_PAUSED.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        if !JOINED_TEST_PAUSED.load(Ordering::Acquire) {
            JOINED_TEST_RELEASE.store(true, Ordering::Release);
            old_session.join().unwrap();
            panic!("old-session native getter did not pause");
        }
        joined_deactivate();
        assert!(joined_activate());
        assert_eq!(can(gov.as_ptr() as usize, 4), 0);
        JOINED_TEST_RELEASE.store(true, Ordering::Release);
        assert_eq!(old_session.join().unwrap(), yes.as_ptr() as usize);
        let after_restart = can(gov.as_ptr() as usize, 4);
        JOINED_TEST_DELAY_RETURNED.store(0, Ordering::Release);
        assert_eq!(
            property(receiver, ASHINA_ADMISSION_KEY_ID),
            yes.as_ptr() as usize
        );
        let after_fresh_read = can(gov.as_ptr() as usize, 4);
        JOINED_TEST_DELAY_RETURNED.store(yes.as_ptr() as u64, Ordering::Release);
        JOINED_TEST_PAUSED.store(false, Ordering::Release);
        JOINED_TEST_RELEASE.store(false, Ordering::Release);
        let mut old_generation = PausedFixtureWorker::new(
            &JOINED_TEST_RELEASE,
            std::thread::spawn(move || property(receiver, ASHINA_ADMISSION_KEY_ID)),
        );
        let deadline = Instant::now() + Duration::from_secs(5);
        while !JOINED_TEST_PAUSED.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        if !JOINED_TEST_PAUSED.load(Ordering::Acquire) {
            JOINED_TEST_RELEASE.store(true, Ordering::Release);
            old_generation.join().unwrap();
            panic!("old-generation native getter did not pause");
        }
        gov[0x70..0x78].copy_from_slice(&(other.0.as_ptr() as u64).to_le_bytes());
        assert_eq!(can(gov.as_ptr() as usize, 4), 0);
        gov[0x70..0x78].copy_from_slice(&(owner.0.as_ptr() as u64).to_le_bytes());
        assert_eq!(can(gov.as_ptr() as usize, 4), 0);
        JOINED_TEST_RELEASE.store(true, Ordering::Release);
        assert_eq!(old_generation.join().unwrap(), yes.as_ptr() as usize);
        let after_generation_change = can(gov.as_ptr() as usize, 4);
        JOINED_TEST_DELAY_RETURNED.store(0, Ordering::Release);
        assert_eq!(
            property(receiver, ASHINA_ADMISSION_KEY_ID),
            yes.as_ptr() as usize
        );
        let after_generation_read = can(gov.as_ptr() as usize, 4);
        unsafe { uninstall() }.unwrap();
        assert_eq!(
            u32::from_le_bytes(owner.0[0x570..0x574].try_into().unwrap()),
            6
        );
        assert_eq!(u32::from_le_bytes(gov[0x78..0x7c].try_into().unwrap()), 10);
        assert_eq!(trace.property.complete_rows().unwrap().len(), 6);
        assert_eq!(
            after_late_true, 0,
            "old true getter reopened a closed admission"
        );
        assert_eq!(
            after_restart, 0,
            "old session reopened admission after restart"
        );
        assert_eq!(
            after_fresh_read, 1,
            "new session requires a fresh Core read"
        );
        assert_eq!(after_generation_change, 0, "A-B-A reused an old Core read");
        assert_eq!(after_generation_read, 1);
        assert_ne!(unsafe { VirtualFree(page.cast(), 0, MEM_RELEASE) }, 0);
    }

    #[test]
    fn joined_second_hook_failure_restores_the_first_property_entry() {
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
        let can_entry = unsafe { page.add(16) };
        let property_entry = unsafe { page.add(128) };
        let prefix = unsafe { property_entry.sub(PROPERTY_PREFIX_LEN) };
        let bad = [0x90u8; 8];
        unsafe {
            ptr::copy_nonoverlapping(bad.as_ptr(), can_entry, 8);
            ptr::copy_nonoverlapping(PROPERTY_PROLOGUE.as_ptr(), property_entry, 8);
            ptr::copy_nonoverlapping(PROPERTY_PREFIX_PAD.as_ptr(), prefix, PROPERTY_PREFIX_LEN);
        }
        let mut trace = Box::new(JoinedTrace::default());
        unsafe { install_property_at(property_entry, &mut trace.property, PROPERTY_PROLOGUE) }
            .unwrap();
        assert!(unsafe {
            install_hook(
                can_entry,
                (&mut trace.calibration as *mut CalibrationTrace).cast(),
                bad,
                BridgeMode::JoinedAdmission,
            )
        }
        .is_err());
        unsafe { uninstall() }.unwrap();
        assert_eq!(
            unsafe { ptr::read(property_entry.cast::<[u8; 8]>()) },
            PROPERTY_PROLOGUE
        );
        assert_eq!(unsafe { ptr::read(can_entry.cast::<[u8; 8]>()) }, bad);
        assert!(!JOINED_ACTIVE.load(Ordering::Acquire));
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

    unsafe fn synthetic_promotion_call(call: usize, evidence: *mut u8, id: u64) -> usize {
        let result: usize;
        unsafe {
            std::arch::asm!(
                "sub rsp, 0x20",
                "mov rax, 0x1111111111111111",
                "movq xmm0, rax",
                "mov rax, 0x2222222222222222",
                "movq xmm1, rax",
                "mov rax, 0x3333333333333333",
                "movq xmm2, rax",
                "mov rax, 0x4444444444444444",
                "movq xmm3, rax",
                "mov rax, 0x5555555555555555",
                "movq xmm4, rax",
                "mov rax, 0x6666666666666666",
                "movq xmm5, rax",
                "mov rax, 0x7777777777777777",
                "pinsrq xmm0, rax, 1",
                "pinsrq xmm1, rax, 1",
                "pinsrq xmm2, rax, 1",
                "pinsrq xmm3, rax, 1",
                "pinsrq xmm4, rax, 1",
                "pinsrq xmm5, rax, 1",
                "stc",
                "call r11",
                "add rsp, 0x20",
                in("r11") call,
                in("rcx") evidence,
                in("rdx") id,
                in("r8") id + 1,
                in("r9") id + 2,
                lateout("rax") result,
                clobber_abi("win64"),
            );
        }
        result
    }

    #[test]
    fn promotion_operand_switch_preserves_concurrent_original_calls_and_raw_entry() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        let page = unsafe {
            VirtualAlloc(
                null_mut(),
                0x40000,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_EXECUTE_READWRITE,
            )
        }
        .cast::<u8>();
        assert!(!page.is_null());
        let callee = page;
        let call = unsafe { page.add(0x3105f) };
        // The fixed displacement is -0x31064. The synthetic callee writes
        // incoming registers, XMM0..5, flags and its actual stack return.
        let callee_bytes = [
            0x48, 0x89, 0x51, 0x00, 0x4c, 0x89, 0x41, 0x08, 0x4c, 0x89, 0x49, 0x10, 0x48, 0x8b,
            0x04, 0x24, 0x48, 0x89, 0x41, 0x18, 0x0f, 0x11, 0x41, 0x20, 0x0f, 0x11, 0x49, 0x30,
            0x0f, 0x11, 0x51, 0x40, 0x0f, 0x11, 0x59, 0x50, 0x0f, 0x11, 0x61, 0x60, 0x0f, 0x11,
            0x69, 0x70, 0x9c, 0x41, 0x5a, 0x4c, 0x89, 0x91, 0x80, 0x00, 0x00, 0x00, 0xc3,
        ];
        unsafe {
            ptr::copy_nonoverlapping(callee_bytes.as_ptr(), callee, callee_bytes.len());
            ptr::copy_nonoverlapping(PROMOTION_CALL_WINDOW.as_ptr(), call, 16);
            *call.add(16) = 0xc3;
        }
        let expected_return = call as usize + 5;
        assert!(verified_promotion_window(
            call as usize,
            callee as usize,
            &PROMOTION_CALL_WINDOW
        ));
        let mut before = [0u8; 144];
        assert_eq!(
            unsafe { synthetic_promotion_call(call as usize, before.as_mut_ptr(), 10) },
            expected_return
        );
        assert_eq!(
            u64::from_le_bytes(before[24..32].try_into().unwrap()),
            expected_return as u64
        );
        let mut trace = Box::new(PromotionTrace::default());
        let running = Arc::new(AtomicBool::new(true));
        let progress = Arc::new(AtomicU32::new(0));
        let mut callers = PromotionCallers {
            running: Arc::clone(&running),
            workers: Vec::new(),
        };
        for id in 0..4u64 {
            let running = Arc::clone(&running);
            let progress = Arc::clone(&progress);
            let address = call as usize;
            callers.workers.push(std::thread::spawn(move || {
                let mut evidence = [0u8; 144];
                let evidence_address = evidence.as_mut_ptr() as u64;
                let thread_id = unsafe { GetCurrentThreadId() };
                let mut count = 0;
                while running.load(Ordering::Acquire) && count < 500 {
                    let result = unsafe {
                        synthetic_promotion_call(address, evidence.as_mut_ptr(), id + 20)
                    };
                    assert_eq!(result, address + 5);
                    assert_eq!(
                        u64::from_le_bytes(evidence[0..8].try_into().unwrap()),
                        id + 20
                    );
                    assert_eq!(
                        u64::from_le_bytes(evidence[8..16].try_into().unwrap()),
                        id + 21
                    );
                    assert_eq!(
                        u64::from_le_bytes(evidence[16..24].try_into().unwrap()),
                        id + 22
                    );
                    assert_eq!(
                        u64::from_le_bytes(evidence[24..32].try_into().unwrap()),
                        (address + 5) as u64
                    );
                    for (xmm, expected) in [
                        0x1111111111111111u64,
                        0x2222222222222222,
                        0x3333333333333333,
                        0x4444444444444444,
                        0x5555555555555555,
                        0x6666666666666666,
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        assert_eq!(
                            u64::from_le_bytes(
                                evidence[32 + xmm * 16..40 + xmm * 16].try_into().unwrap()
                            ),
                            expected
                        );
                        assert_eq!(
                            u64::from_le_bytes(
                                evidence[40 + xmm * 16..48 + xmm * 16].try_into().unwrap()
                            ),
                            0x7777777777777777
                        );
                    }
                    assert_ne!(
                        u64::from_le_bytes(evidence[128..136].try_into().unwrap()) & 1,
                        0
                    );
                    count += 1;
                    progress.fetch_add(1, Ordering::Release);
                    std::thread::sleep(Duration::from_micros(100));
                }
                (id + 20, thread_id, evidence_address, count)
            }));
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while progress.load(Ordering::Acquire) < 4 && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(progress.load(Ordering::Acquire) >= 4);
        // Workers are already executing the original E8 when the operand
        // changes. Both old and new destinations are kept executable.
        unsafe { install_promotion_call(call, callee, trace.as_mut(), PROMOTION_CALL_WINDOW) }
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while trace.next.load(Ordering::Acquire) < 20 && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(trace.next.load(Ordering::Acquire) >= 20);
        let before_restore = progress.load(Ordering::Acquire);
        assert!(unsafe { restore_promotion_call() }.unwrap());
        let deadline = Instant::now() + Duration::from_secs(2);
        while progress.load(Ordering::Acquire) < before_restore + 4 && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(progress.load(Ordering::Acquire) >= before_restore + 4);
        let workers = callers.stop_and_join().unwrap();
        assert!(workers.iter().map(|worker| worker.3).sum::<usize>() > 0);
        let mut after = [0u8; 144];
        assert_eq!(
            unsafe { synthetic_promotion_call(call as usize, after.as_mut_ptr(), 99) },
            expected_return
        );
        assert_eq!(
            unsafe { ptr::read(call.cast::<[u8; 16]>()) },
            PROMOTION_CALL_WINDOW
        );
        let rows = trace.complete_rows().unwrap();
        assert!(!rows.is_empty());
        for row in rows {
            assert_eq!(row.return_address, expected_return as u64);
            let worker = workers.iter().find(|worker| worker.0 == row.rdx).unwrap();
            assert_eq!(row.thread_id, worker.1);
            assert_eq!(row.rcx, worker.2);
            assert_eq!(row.r8, row.rdx + 1);
            assert_eq!(row.r9, row.rdx + 2);
        }
        drop(callers);

        let mut panic_trace = Box::new(PromotionTrace::default());
        let running = Arc::new(AtomicBool::new(true));
        let progress = Arc::new(AtomicU32::new(0));
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut callers = PromotionCallers {
                running: Arc::clone(&running),
                workers: Vec::new(),
            };
            for id in 0..2 {
                let running = Arc::clone(&running);
                let progress = Arc::clone(&progress);
                let address = call as usize;
                callers.workers.push(std::thread::spawn(move || {
                    let mut evidence = [0u8; 144];
                    let mut count = 0;
                    while running.load(Ordering::Acquire) {
                        assert_eq!(
                            unsafe { synthetic_promotion_call(address, evidence.as_mut_ptr(), id) },
                            address + 5
                        );
                        count += 1;
                        progress.fetch_add(1, Ordering::Release);
                        std::thread::yield_now();
                    }
                    (id, 0, 0, count)
                }));
            }
            let deadline = Instant::now() + Duration::from_secs(5);
            while progress.load(Ordering::Acquire) < 2 && Instant::now() < deadline {
                std::thread::yield_now();
            }
            assert!(progress.load(Ordering::Acquire) >= 2);
            unsafe {
                install_promotion_call(call, callee, panic_trace.as_mut(), PROMOTION_CALL_WINDOW)
            }
            .unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while panic_trace.next.load(Ordering::Acquire) == 0 && Instant::now() < deadline {
                std::thread::yield_now();
            }
            assert!(panic_trace.next.load(Ordering::Acquire) > 0);
            panic!("injected failure while the promotion hook has callers");
        }));
        assert!(panic.is_err());
        assert!(!running.load(Ordering::Acquire));
        assert_eq!(Arc::strong_count(&running), 1);
        assert_eq!(panic_trace.in_flight.load(Ordering::Acquire), 0);
        assert_eq!(
            unsafe { ptr::read(call.cast::<[u8; 16]>()) },
            PROMOTION_CALL_WINDOW
        );
        PROMOTION_TRACE_PTR.store(null_mut(), Ordering::Release);
        PROMOTION_CALLEE_PTR.store(null_mut(), Ordering::Release);
        assert_ne!(unsafe { VirtualFree(page.cast(), 0, MEM_RELEASE) }, 0);
    }

    #[test]
    fn promotion_call_window_and_trace_reject_bad_boundaries_and_partial_rows() {
        assert_eq!(
            promotion_operand(PINNED_PROMOTION_CALL_RVA, PINNED_PROMOTION_CALLEE_RVA),
            Some(0xfffcef9c)
        );
        assert!(promotion_operand(0x30ec0e, 0x2ddbb0).is_none());
        assert!(promotion_operand(0x30ec0f, usize::MAX).is_none());
        assert!(promotion_operand(0x3b, 0x100).is_some()); // final aligned DWORD of a line
        assert!(promotion_operand(0x3f, 0x100).is_some()); // E8 crosses a fetch block; operand does not
        assert!(!verified_promotion_window(
            PINNED_PROMOTION_CALL_RVA,
            PINNED_PROMOTION_CALLEE_RVA + 1,
            &PROMOTION_CALL_WINDOW
        ));
        let mut wrong = PROMOTION_CALL_WINDOW;
        wrong[0] = 0xe9;
        assert!(!verified_promotion_window(
            PINNED_PROMOTION_CALL_RVA,
            PINNED_PROMOTION_CALLEE_RVA,
            &wrong
        ));
        wrong = PROMOTION_CALL_WINDOW;
        wrong[1] ^= 1;
        assert!(!verified_promotion_window(
            PINNED_PROMOTION_CALL_RVA,
            PINNED_PROMOTION_CALLEE_RVA,
            &wrong
        ));
        wrong = PROMOTION_CALL_WINDOW;
        wrong[5] ^= 1;
        assert!(!verified_promotion_window(
            PINNED_PROMOTION_CALL_RVA,
            PINNED_PROMOTION_CALLEE_RVA,
            &wrong
        ));
        let mut trace = Box::new(PromotionTrace::default());
        trace.next.store(1, Ordering::Release);
        assert!(trace.complete_rows().is_err());
        trace.rows[0].sequence.store(1, Ordering::Release);
        assert_eq!(trace.complete_rows().unwrap().len(), 1);
        trace.in_flight.store(1, Ordering::Release);
        assert!(trace.complete_rows().is_err());
        trace.in_flight.store(0, Ordering::Release);
        trace
            .next
            .store(PROMOTION_TRACE_CAPACITY as u32 + 1, Ordering::Release);
        trace.dropped.store(1, Ordering::Release);
        assert!(trace.complete_rows().is_err());
        trace.dropped.store(0, Ordering::Release);
        assert!(trace.complete_rows().is_err());
        trace.next.store(0, Ordering::Release);
        trace.mode = BridgeMode::CanEver as u32;
        assert!(trace.complete_rows().is_err());
        trace.mode = BridgeMode::PromotionCall as u32;
        trace.magic = 0x4a41_4342;
        assert!(trace.complete_rows().is_err());
        trace.magic = PROMOTION_TRACE_MAGIC;
        trace.version += 1;
        assert!(trace.complete_rows().is_err());
        trace.version = PROMOTION_TRACE_VERSION;
        trace.capacity -= 1;
        assert!(trace.complete_rows().is_err());
    }

    #[test]
    fn promotion_recorder_accepts_exact_capacity_then_rejects_the_next_call() {
        let _exclusive = HOOK_TEST_LOCK.lock().unwrap();
        let mut trace = Box::new(PromotionTrace::default());
        let raw = RawPromotionRegisters {
            rcx: 1,
            rdx: 2,
            r8: 3,
            r9: 4,
            return_address: 5,
        };
        PROMOTION_TRACE_PTR.store(trace.as_mut(), Ordering::Release);
        for _ in 0..PROMOTION_TRACE_CAPACITY {
            record_promotion_call(&raw);
        }
        assert_eq!(
            trace.complete_rows().unwrap().len(),
            PROMOTION_TRACE_CAPACITY
        );
        record_promotion_call(&raw);
        assert_eq!(trace.dropped.load(Ordering::Acquire), 1);
        assert!(trace.complete_rows().is_err());
        PROMOTION_TRACE_PTR.store(null_mut(), Ordering::Release);
    }

    #[test]
    fn fixed_primary_gamecore_maps_complete_promotion_call_and_keeps_callee_boundary() {
        if !Path::new(GAMECORE).is_file() {
            return;
        }
        let mut file = File::open(GAMECORE).unwrap();
        assert_eq!(
            pe_file_offset(&mut file, PINNED_PROMOTION_CALL_RVA as u32),
            Some(0x30e00f)
        );
        assert_eq!(
            pe_file_offset(&mut file, PINNED_PROMOTION_CALLEE_RVA as u32),
            Some(0x2dcfb0)
        );
        assert_eq!(
            pinned_gamecore(Path::new(GAMECORE), BridgeMode::PromotionCall),
            Some(PROMOTION_CALL_HEAD)
        );
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
        let phase = Arc::new(AtomicU32::new(1));
        let address = target as usize;
        let (mut callers, progress) = spawn_synthetic_callers(&phase, move |owner| {
            let f: extern "system" fn(usize, u32) -> u32 = unsafe { std::mem::transmute(address) };
            assert_eq!(f(0x1000 + owner as usize, owner), 0x1000);
        });
        let during_hook = wait_for_synthetic_phase(&progress, 1);
        unsafe { uninstall() }.unwrap();
        phase.store(2, Ordering::Release);
        let after_restore = wait_for_synthetic_phase(&progress, 2);
        assert!(callers.stop_and_join().unwrap() > 0);
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
        assert!(
            during_hook,
            "each caller must execute while the hook is installed"
        );
        assert!(after_restore, "each caller must execute after restoration");
    }

    #[test]
    fn synthetic_caller_guard_joins_and_restores_before_trace_drop_on_panic() {
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
        let phase = Arc::new(AtomicU32::new(1));
        let address = target as usize;
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (_callers, progress) = spawn_synthetic_callers(&phase, move |owner| {
                let f: extern "system" fn(usize, u32) -> u32 =
                    unsafe { std::mem::transmute(address) };
                assert_eq!(f(0x1000 + owner as usize, owner), 0x1000);
            });
            assert!(wait_for_synthetic_phase(&progress, 1));
            panic!("injected test-body failure after every worker entered the hook");
        }));
        assert!(panic.is_err());
        assert_eq!(phase.load(Ordering::Acquire), 3);
        assert_eq!(
            Arc::strong_count(&phase),
            1,
            "no worker may retain the phase"
        );
        assert_eq!(trace.in_flight.load(Ordering::Acquire), 0);
        assert_eq!(unsafe { ptr::read(target.cast::<[u8; 8]>()) }, bytes[..8]);

        let mut next_trace = Box::new(SharedTrace::default());
        unsafe { install_at(target, next_trace.as_mut(), bytes[..8].try_into().unwrap()) }.unwrap();
        let phase = Arc::new(AtomicU32::new(1));
        let invoked = Arc::new(AtomicU32::new(0));
        let calls = Arc::clone(&invoked);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = spawn_synthetic_callers_with_hook(
                &phase,
                move |owner| {
                    let f: extern "system" fn(usize, u32) -> u32 =
                        unsafe { std::mem::transmute(address) };
                    assert_eq!(f(0x1000 + owner as usize, owner), 0x1000);
                    calls.fetch_add(1, Ordering::Release);
                },
                |spawned| {
                    if spawned == 2 {
                        let deadline = Instant::now() + Duration::from_secs(5);
                        while invoked.load(Ordering::Acquire) == 0 && Instant::now() < deadline {
                            std::thread::yield_now();
                        }
                        assert!(invoked.load(Ordering::Acquire) > 0);
                        panic!("injected failure partway through worker creation");
                    }
                },
            );
        }));
        assert!(panic.is_err());
        assert_eq!(phase.load(Ordering::Acquire), 3);
        assert_eq!(Arc::strong_count(&phase), 1);
        assert_eq!(next_trace.in_flight.load(Ordering::Acquire), 0);
        assert_eq!(unsafe { ptr::read(target.cast::<[u8; 8]>()) }, bytes[..8]);
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
            BridgeInstall(std::ptr::dangling_mut::<c_void>()),
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
        property.mode = 3;
        assert_eq!(
            BridgeInstall((&mut property as *mut BridgeConfig).cast()),
            BridgeStatus::InvalidInput as u32
        );
        let mut promotion = BridgeConfig::for_promotion_call(1, 1, 1);
        assert_eq!(promotion.mode, 8);
        assert_eq!(
            promotion.trace_size as usize,
            std::mem::size_of::<PromotionTrace>()
        );
        for mode in [6, 7, 9] {
            promotion.mode = mode;
            assert_eq!(
                BridgeInstall((&mut promotion as *mut BridgeConfig).cast()),
                BridgeStatus::InvalidInput as u32
            );
        }
        promotion.mode = 8;
        promotion.trace_size = BridgeMode::CanEver.trace_size() as u32;
        assert_eq!(
            BridgeInstall((&mut promotion as *mut BridgeConfig).cast()),
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
