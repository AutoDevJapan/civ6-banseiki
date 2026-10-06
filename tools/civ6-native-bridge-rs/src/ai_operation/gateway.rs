//! Optional in-process registration. No exported entry, numeric property permit,
//! Lua, dispatcher invocation, process injector, or automatic startup.
//! RuntimeLease must be implemented INSIDE the owning runtime with its existing
//! lease/ARM and atomic CallFence; this module cannot manufacture that authority.
use super::{
    capture::{Capture, Context},
    protocol::Epoch,
};
use civ6_native_bridge_rs::governor_payment::{
    CurrentMemory, PaymentDefinition, PromotionSnapshot, read_current,
};
use std::{
    cell::Cell,
    ptr,
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
};
use windows_sys::Win32::{
    Foundation::FILETIME,
    System::{
        Diagnostics::Debug::{FlushInstructionCache, ReadProcessMemory},
        LibraryLoader::{
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN,
            GetModuleHandleExW,
        },
        Memory::{
            MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, MEMORY_BASIC_INFORMATION, PAGE_EXECUTE_READ,
            PAGE_EXECUTE_READWRITE, PAGE_GUARD, PAGE_NOACCESS, PAGE_READWRITE, VirtualAlloc,
            VirtualFree, VirtualProtect, VirtualQuery,
        },
        Threading::{GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId, GetProcessTimes},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeaseStamp {
    pub pid: u32,
    pub created: u64,
    pub owner: u32,
    pub generation: u32,
    pub session: u32,
    pub begin_ticket: u32,
    pub arm_token: u64,
}
fn same_registration(now: LeaseStamp, installed: LeaseStamp) -> bool {
    now.pid == installed.pid
        && now.created == installed.created
        && now.owner == installed.owner
        && now.generation == installed.generation
        && now.session == installed.session
        && now.arm_token == installed.arm_token
        && now.begin_ticket >= installed.begin_ticket
}
/// # Safety
/// Implement only against the real parent-owned lease/PRODARM and call fence.
/// A boolean constant, fixture value, or numeric property is not an adapter.
/// `current` must revalidate atomic state and receiver binding, including load.
pub unsafe trait RuntimeLease: Sync {
    fn begin(&self, owner: u32, player: u64) -> Option<LeaseStamp>;
    fn current(&self, stamp: LeaseStamp, player: u64) -> bool;
    fn accepts_entry(&self, installed: LeaseStamp, current: LeaseStamp, _player: u64) -> bool {
        same_registration(current, installed)
    }
    /// Only the natural stage-2 callback invokes these; never a property hook.
    fn before_original(&self, _stamp: LeaseStamp, _player: u64, _definition: PaymentDefinition) {}
    fn after_original(&self, _stamp: LeaseStamp, _player: u64, _definition: PaymentDefinition) {}
    fn on_unwind(&self) {}
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Registers {
    pub rcx: u64,
    pub rdx: u64,
    pub r8: u64,
    pub r9: u64,
    pub rax: u64,
    pub r10: u64,
    pub r11: u64,
    pub flags: u64,
    pub xmm: [[u8; 16]; 6],
    pub caller_rsp: u64,
    pub return_address: u64,
    pub site: u64,
    pub ticket: u64,
}
const _: () = assert!(std::mem::size_of::<Registers>() == 0xc0);
// Assembly frame: outgoing caller bytes [0,60), snapshot [60,120), spare [120,1b0), saved flags at1b0.
#[derive(Clone, Copy, Debug)]
pub struct Observation {
    pub sequence: u32,
    pub thread: u32,
    pub after: bool,
    pub raw: Registers,
    pub accepted: bool,
    pub owner: Option<u32>,
    pub generation: u32,
    pub own: Option<PromotionSnapshot>,
    pub capture_entry: bool,
}
const CAPACITY: usize = 128;
static ROWS: Mutex<Vec<Observation>> = Mutex::new(Vec::new());
static NEXT: AtomicU32 = AtomicU32::new(1);
static DROPPED: AtomicU32 = AtomicU32::new(0);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static BINDING: OnceLock<Binding> = OnceLock::new();
static UNWOUND: AtomicU32 = AtomicU32::new(0);
#[unsafe(no_mangle)]
static AI_GATE_FLIGHTS: AtomicU64 = AtomicU64::new(0);
#[unsafe(no_mangle)]
static AI_GATE_UNTRACKED: AtomicBool = AtomicBool::new(false);
thread_local! {static THREAD_FLIGHTS:Cell<u32>=const{Cell::new(0)};}

/// Call only after restoring the registration, before releasing owner objects.
/// A tail-forwarded unreadable frame prevents a quiescence claim permanently.
pub fn drain() -> bool {
    if THREAD_FLIGHTS.with(Cell::get) != 0 {
        return false;
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while AI_GATE_FLIGHTS.load(Ordering::Acquire) != 0 {
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    !AI_GATE_UNTRACKED.load(Ordering::Acquire)
}
thread_local! {static INVOCATION:Cell<Option<Invocation>>=const {Cell::new(None)};}
thread_local! {static DEPTH:Cell<u32>=const {Cell::new(0)};}
thread_local! {static CAPTURE:std::cell::RefCell<Option<Capture>>=const {std::cell::RefCell::new(None)};}
#[derive(Clone, Copy)]
struct Invocation {
    stamp: LeaseStamp,
    player: u64,
    receiver: u64,
    thread: u32,
}
struct Binding {
    base: u64,
    callback_return: u64,
    receiver: u64,
    player: u64,
    owner: u32,
    definition: PaymentDefinition,
    lease: &'static dyn RuntimeLease,
    installed: LeaseStamp,
    promotion: u32,
}

pub struct Memory;
impl CurrentMemory for Memory {
    fn copy<const N: usize>(&self, address: u64) -> Option<[u8; N]> {
        if address == 0 || address.checked_add(N as u64).is_none() {
            return None;
        }
        let mut bytes = [0; N];
        let mut copied = 0;
        let ok = unsafe {
            ReadProcessMemory(
                GetCurrentProcess(),
                address as *const _,
                bytes.as_mut_ptr().cast(),
                N,
                &mut copied,
            )
        };
        (ok != 0 && copied == N).then_some(bytes)
    }
}
fn u64_at(address: u64) -> Option<u64> {
    Some(u64::from_le_bytes(Memory.copy(address)?))
}
fn u32_at(address: u64) -> Option<u32> {
    Some(u32::from_le_bytes(Memory.copy(address)?))
}
fn identity(stamp: LeaseStamp) -> bool {
    let mut c = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut e = c;
    let mut k = c;
    let mut u = c;
    (unsafe {
        GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) != 0
            && GetCurrentProcessId() == stamp.pid
    }) && stamp.created == (u64::from(c.dwHighDateTime) << 32 | u64::from(c.dwLowDateTime))
        && stamp.owner < 64
        && stamp.generation > 0
        && stamp.session > 0
        && stamp.begin_ticket > 0
        && stamp.arm_token != 0
}

/// SEH termination handler: unwinding through any observed original call
/// invalidates the whole per-thread capture. No native call or allocation.
unsafe extern "system" fn unwind_observer(
    record: *const windows_sys::Win32::System::Diagnostics::Debug::EXCEPTION_RECORD,
    _frame: usize,
    _context: *mut windows_sys::Win32::System::Diagnostics::Debug::CONTEXT,
    _dispatcher: *mut std::ffi::c_void,
) -> i32 {
    if !record.is_null() && unsafe { (*record).ExceptionFlags } & (2 | 4 | 0x20 | 0x40) != 0 {
        AI_GATE_FLIGHTS.fetch_sub(1, Ordering::AcqRel);
        let _ = THREAD_FLIGHTS.try_with(|c| c.set(c.get().saturating_sub(1)));
        if let Some(binding) = BINDING.get() {
            binding.lease.on_unwind();
        }
        let _ = INVOCATION.try_with(|c| c.set(None));
        let _ = DEPTH.try_with(|d| d.set(0));
        let _ = CAPTURE.try_with(|c| {
            if let Ok(mut capture) = c.try_borrow_mut() {
                *capture = None;
            }
        });
        UNWOUND.fetch_add(1, Ordering::Relaxed);
    }
    1 // ExceptionContinueSearch, preserves the original unwind target.
}

/// Records copied entry/exit registers, never changes them. A missing lease
/// suppresses observation while the immutable original target still runs once.
extern "system" fn observe(raw: *mut Registers, after: u32) {
    let _ = std::panic::catch_unwind(|| {
        let raw = unsafe { &mut *raw };
        if after == 0 && AI_GATE_FLIGHTS.load(Ordering::Acquire) != 0 {
            THREAD_FLIGHTS.with(|c| c.set(c.get().saturating_add(1)))
        } else {
            THREAD_FLIGHTS.with(|c| c.set(c.get().saturating_sub(1)))
        }
        if after == 0 {
            raw.ticket = 0;
            // Bounded readable source is the actual caller home/argument span.
            // Never speculatively load 0x60 bytes across an inaccessible page.
            let Some(source) = raw
                .caller_rsp
                .checked_add(8)
                .and_then(|p| Memory.copy::<0x60>(p))
            else {
                return;
            };
            // Retain all 0x60 bytes outside this recorder's shadow area. The
            // gateway copies them AFTER this recorder returns, including home.
            unsafe {
                ptr::copy_nonoverlapping(
                    source.as_ptr(),
                    (raw as *mut Registers).cast::<u8>().add(0xc0),
                    0x60,
                )
            };
            raw.ticket = 1; // stack copied; before any optional observation policy
        }
        let Some(binding) = BINDING.get() else { return };
        let thread = unsafe { GetCurrentThreadId() };
        if after == 0 {
            raw.ticket = match NEXT
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
            {
                Ok(n) => u64::from(n),
                Err(_) => {
                    ACTIVE.store(false, Ordering::Release);
                    u64::from(u32::MAX)
                }
            };
        }
        let mut invocation = INVOCATION.with(Cell::get);
        if raw.site == 0 && after == 0 {
            DEPTH.with(|d| d.set(d.get().saturating_add(1)));
            // Nested callback or non-natural entry cannot replace a live context.
            if invocation.is_none()
                && DEPTH.with(Cell::get) == 1
                && ACTIVE.load(Ordering::Acquire)
                && raw.return_address == binding.callback_return
                && raw.rcx == binding.receiver
                && raw.rdx as u32 == 2
                && u64_at(raw.rcx + 0x98) == Some(binding.player)
                && u32_at(binding.player + 0xd8) == Some(binding.owner)
            {
                invocation = binding
                    .lease
                    .begin(binding.owner, binding.player)
                    .filter(|s| {
                        identity(*s)
                            && s.owner == binding.owner
                            && binding
                                .lease
                                .accepts_entry(binding.installed, *s, binding.player)
                    })
                    .map(|stamp| Invocation {
                        stamp,
                        player: binding.player,
                        receiver: raw.rcx,
                        thread,
                    });
                INVOCATION.with(|cell| cell.set(invocation));
                if let Some(i) = invocation {
                    let context = Context {
                        epoch: Epoch {
                            pid: i.stamp.pid,
                            created: i.stamp.created,
                            generation: u64::from(i.stamp.generation),
                        },
                        thread,
                        module_base: binding.base,
                    };
                    let observed = civ6_native_bridge_rs::PromotionObservation {
                        sequence: raw.ticket as u32,
                        thread_id: thread,
                        rcx: raw.rcx,
                        rdx: raw.rdx,
                        r8: raw.r8,
                        r9: raw.r9,
                        return_address: raw.return_address,
                    };
                    let entry = Capture::begin(
                        context,
                        binding.base + 0x4a7ad0,
                        observed,
                        binding.definition,
                        binding.promotion,
                        &Memory,
                    )
                    .ok();
                    CAPTURE.with(|c| *c.borrow_mut() = entry);
                }
            }
        }
        let accepted = ACTIVE.load(Ordering::Acquire)
            && DEPTH.with(Cell::get) == 1
            && invocation.is_some_and(|i| {
                i.thread == thread
                    && identity(i.stamp)
                    && binding.lease.current(i.stamp, i.player)
                    && u64_at(i.receiver + 0x98) == Some(i.player)
                    && u32_at(i.player + 0xd8) == Some(i.stamp.owner)
            });
        let mut own = None;
        let mut accepted = accepted;
        if after == 0 && raw.site > 0 {
            let expected = match raw.site {
                1 => 0x4a7d6c,
                2 => 0x30ec14,
                3 => 0x2ddbd0,
                _ => 0,
            };
            accepted &= expected != 0
                && raw.return_address == binding.base + expected
                && u64_at(raw.rcx.checked_add(0x70).unwrap_or(0)) == Some(binding.player);
        }
        if raw.site == 1 || raw.site == 2 {
            if after == 0 && accepted {
                own = read_current(&Memory, raw.rcx, raw.rdx).ok();
                accepted = own.is_some_and(|p| {
                    p.owner == binding.owner
                        && p.governor_index == binding.definition.governor_index
                        && p.diagnostic_paid(binding.owner, binding.definition).is_ok()
                });
            }
        }
        if after == 0 && accepted {
            raw.ticket |= 1u64 << 63;
        }
        if after != 0 {
            accepted &= raw.ticket & (1u64 << 63) != 0;
        }
        let row = Observation {
            sequence: raw.ticket as u32,
            thread,
            after: after != 0,
            raw: *raw,
            accepted,
            owner: invocation.map(|i| i.stamp.owner),
            generation: invocation.map_or(0, |i| i.stamp.generation),
            own,
            capture_entry: CAPTURE.with(|c| c.borrow().is_some()),
        };
        if let Ok(mut rows) = ROWS.try_lock() {
            if rows.len() < CAPACITY {
                rows.push(row)
            } else {
                DROPPED.fetch_add(1, Ordering::Relaxed);
            }
        } else {
            DROPPED.fetch_add(1, Ordering::Relaxed);
        }
        if raw.site == 0 && after != 0 {
            DEPTH.with(|d| {
                if d.get() == 1 {
                    INVOCATION.with(|cell| cell.set(None));
                    CAPTURE.with(|c| *c.borrow_mut() = None);
                }
                d.set(d.get().saturating_sub(1));
            });
        }
    });
}

// Engine exceptions are allowed through this frame and are handled by the
// gateway's SEH boundary; never route them into observe's catch_unwind.
extern "system-unwind" fn native_action(raw: *const Registers, after: u32) {
    let Some(binding) = BINDING.get() else { return };
    let raw = unsafe { &*raw };
    if raw.site != 0
        || raw.ticket & (1u64 << 63) == 0
        || !ACTIVE.load(Ordering::Acquire)
        || DEPTH.with(Cell::get) != 1
    {
        return;
    }
    let Some(i) = INVOCATION.with(Cell::get) else {
        return;
    };
    if !binding.lease.current(i.stamp, i.player) {
        return;
    }
    if after == 0 {
        binding
            .lease
            .before_original(i.stamp, i.player, binding.definition)
    } else {
        binding
            .lease
            .after_original(i.stamp, i.player, binding.definition)
    }
}

// Every target is published once BEFORE registration and retained permanently.
// Removing a registration never points a delayed invocation at another target.
#[unsafe(no_mangle)]
static AI_GATE_TARGETS: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
macro_rules! gateway {
    ($name:literal,$site:literal,$offset:literal)=>{core::arch::global_asm!(concat!(
        ".globl ",$name,"\n.seh_proc ",$name,"\n",$name,":\n",
        "pushfq\n.seh_stackalloc 8\nsub rsp,0x1b0\n.seh_stackalloc 0x1b0\n.seh_handler {unwind}, @unwind\n.seh_endprologue\nlock inc qword ptr [rip+AI_GATE_FLIGHTS]\n",
        "mov [rsp+0x60],rcx\nmov [rsp+0x68],rdx\nmov [rsp+0x70],r8\nmov [rsp+0x78],r9\nmov [rsp+0x80],rax\nmov [rsp+0x88],r10\nmov [rsp+0x90],r11\n",
        "mov rax,[rsp+0x1b0]\nmov [rsp+0x98],rax\n",
        "movdqu [rsp+0xa0],xmm0\nmovdqu [rsp+0xb0],xmm1\nmovdqu [rsp+0xc0],xmm2\nmovdqu [rsp+0xd0],xmm3\nmovdqu [rsp+0xe0],xmm4\nmovdqu [rsp+0xf0],xmm5\n",
        "lea rax,[rsp+0x1b8]\nmov [rsp+0x100],rax\nmov rax,[rax]\nmov [rsp+0x108],rax\nmov qword ptr [rsp+0x110],",$site,"\n",
        "lea rcx,[rsp+0x60]\nxor edx,edx\ncall {record}\nlea rcx,[rsp+0x60]\nxor edx,edx\ncall {action}\n",
        // Exactly 0x60 bytes from the TRUE caller's home/stack arguments, not
        // the gateway's saved registers. All reads occur within the copied span.
        "cmp qword ptr [rsp+0x118],0\nje 8f\n",
        "movdqu xmm0,[rsp+0x120]\nmovdqu [rsp],xmm0\nmovdqu xmm0,[rsp+0x130]\nmovdqu [rsp+0x10],xmm0\n",
        "movdqu xmm0,[rsp+0x140]\nmovdqu [rsp+0x20],xmm0\nmovdqu xmm0,[rsp+0x150]\nmovdqu [rsp+0x30],xmm0\n",
        "movdqu xmm0,[rsp+0x160]\nmovdqu [rsp+0x40],xmm0\nmovdqu xmm0,[rsp+0x170]\nmovdqu [rsp+0x50],xmm0\n",
        "movdqu xmm0,[rsp+0xa0]\nmovdqu xmm1,[rsp+0xb0]\nmovdqu xmm2,[rsp+0xc0]\nmovdqu xmm3,[rsp+0xd0]\nmovdqu xmm4,[rsp+0xe0]\nmovdqu xmm5,[rsp+0xf0]\n",
        "mov rcx,[rsp+0x60]\nmov rdx,[rsp+0x68]\nmov r8,[rsp+0x70]\nmov r9,[rsp+0x78]\nmov rax,[rsp+0x80]\nmov r10,[rsp+0x88]\nmov r11,[rsp+0x90]\npush qword ptr [rsp+0x98]\npopfq\n",
        "call qword ptr [rip+AI_GATE_TARGETS+",$offset,"]\n",
        "pushfq\npop qword ptr [rsp+0x98]\nmov [rsp+0x60],rcx\nmov [rsp+0x68],rdx\nmov [rsp+0x70],r8\nmov [rsp+0x78],r9\nmov [rsp+0x80],rax\nmov [rsp+0x88],r10\nmov [rsp+0x90],r11\n",
        "movdqu [rsp+0xa0],xmm0\nmovdqu [rsp+0xb0],xmm1\nmovdqu [rsp+0xc0],xmm2\nmovdqu [rsp+0xd0],xmm3\nmovdqu [rsp+0xe0],xmm4\nmovdqu [rsp+0xf0],xmm5\n",
        "lea rcx,[rsp+0x60]\nmov edx,1\ncall {action}\nlea rcx,[rsp+0x60]\nmov edx,1\ncall {record}\n",
        "movdqu xmm0,[rsp+0xa0]\nmovdqu xmm1,[rsp+0xb0]\nmovdqu xmm2,[rsp+0xc0]\nmovdqu xmm3,[rsp+0xd0]\nmovdqu xmm4,[rsp+0xe0]\nmovdqu xmm5,[rsp+0xf0]\n",
        "mov rcx,[rsp+0x60]\nmov rdx,[rsp+0x68]\nmov r8,[rsp+0x70]\nmov r9,[rsp+0x78]\nmov rax,[rsp+0x80]\nmov r10,[rsp+0x88]\nmov r11,[rsp+0x90]\n",
        "lock dec qword ptr [rip+AI_GATE_FLIGHTS]\npush qword ptr [rsp+0x98]\npopfq\nlea rsp,[rsp+0x1b8]\nret\n",
        // Failed frame read: restore and tail-jump with the real original stack.
        "8:\nmov byte ptr [rip+AI_GATE_UNTRACKED],1\nlock dec qword ptr [rip+AI_GATE_FLIGHTS]\nmovdqu xmm0,[rsp+0xa0]\nmovdqu xmm1,[rsp+0xb0]\nmovdqu xmm2,[rsp+0xc0]\nmovdqu xmm3,[rsp+0xd0]\nmovdqu xmm4,[rsp+0xe0]\nmovdqu xmm5,[rsp+0xf0]\n",
        "mov rcx,[rsp+0x60]\nmov rdx,[rsp+0x68]\nmov r8,[rsp+0x70]\nmov r9,[rsp+0x78]\nmov rax,[rsp+0x80]\nmov r10,[rsp+0x88]\nmov r11,[rsp+0x90]\npush qword ptr [rsp+0x98]\npopfq\nlea rsp,[rsp+0x1b8]\njmp qword ptr [rip+AI_GATE_TARGETS+",$offset,"]\n.seh_endproc\n"
    ),record=sym observe,action=sym native_action,unwind=sym unwind_observer);};
}
gateway!("ai_callback_gateway", "0", "0");
gateway!("ai_checker_gateway", "1", "8");
gateway!("official_checker_gateway", "2", "16");
gateway!("spent_gateway", "3", "24");
unsafe extern "system" {
    fn ai_callback_gateway();
    fn ai_checker_gateway();
    fn official_checker_gateway();
    fn spent_gateway();
}
fn gateways() -> [usize; 4] {
    [
        ai_callback_gateway as *const () as usize,
        ai_checker_gateway as *const () as usize,
        official_checker_gateway as *const () as usize,
        spent_gateway as *const () as usize,
    ]
}

#[derive(Debug, PartialEq, Eq)]
pub enum Failure {
    Identity,
    Lease,
    Caller,
    Memory,
    AlreadyUsed,
    Range,
    Alignment,
    Patch,
    UnsupportedCpu,
    CriticalRestore,
}
pub fn operand(call: usize, target: usize) -> Result<u32, Failure> {
    let op = call.checked_add(1).ok_or(Failure::Range)?;
    if op % 4 != 0 || op % 64 > 60 || op % 4096 > 4092 {
        return Err(Failure::Alignment);
    }
    let next = call.checked_add(5).ok_or(Failure::Range)?;
    Ok(i32::try_from(target as i128 - next as i128).map_err(|_| Failure::Range)? as u32)
}
#[derive(Clone)]
struct Patch {
    address: usize,
    old: u64,
    new: u64,
    wide: bool,
}
#[path = "patch_state.rs"]
mod patch_state;
pub use patch_state::Registration;
pub fn recover_failed_registration() -> Result<(), Failure> {
    patch_state::recover_failed_registration()
}
/// Close both a published registration and an install failure retained only by
/// recovery. Absence of a public handle does not prove that no patch is live.
pub fn close_registered() -> Result<(), Failure> {
    let guard = INSTALL.lock().map_err(|_| Failure::Patch)?;
    ACTIVE.store(false, Ordering::Release);
    if let Some(registration) = RECOVERY.get() {
        registration.close_locked(&guard, &SystemApi)?;
    }
    drop(guard);
    if drain() {
        Ok(())
    } else {
        Err(Failure::CriticalRestore)
    }
}
use patch_state::{INSTALL, RECOVERY, SystemApi};

unsafe fn relay(call: usize, gateway: usize) -> Result<usize, Failure> {
    for step in 1..=0x4000usize {
        for addr in [
            (call & !0xffff).checked_add(step * 0x10000),
            (call & !0xffff).checked_sub(step * 0x10000),
        ]
        .into_iter()
        .flatten()
        {
            if operand(call, addr).is_err() {
                continue;
            }
            let page = unsafe {
                VirtualAlloc(
                    addr as *const _,
                    4096,
                    MEM_COMMIT | MEM_RESERVE,
                    PAGE_READWRITE,
                )
            } as *mut u8;
            if page.is_null() {
                continue;
            }
            unsafe {
                ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), page, 6);
                ptr::copy_nonoverlapping((gateway as u64).to_le_bytes().as_ptr(), page.add(6), 8);
            }
            let mut old = 0;
            if unsafe { VirtualProtect(page.cast(), 4096, PAGE_EXECUTE_READ, &mut old) } == 0
                || unsafe { FlushInstructionCache(GetCurrentProcess(), page.cast(), 14) } == 0
            {
                unsafe { VirtualFree(page.cast(), 0, MEM_RELEASE) };
                return Err(Failure::Patch);
            }
            return Ok(page as usize);
        }
    }
    Err(Failure::Range)
}

/// # Safety
/// Called by the REAL owning runtime under its install lock, with loaded and
/// SHA-verified pinned GameCore; callable is a current member of AI+1B8 vector.
/// Caller keeps that collection alive through close and old-call drainage.
/// There is no CLI/export that can bypass the required RuntimeLease adapter.
pub unsafe fn register(
    base: u64,
    ai_owner: u64,
    callable: u64,
    definition: PaymentDefinition,
    promotion: u32,
    lease: &'static dyn RuntimeLease,
) -> Result<Registration, Failure> {
    let _lock = INSTALL.lock().map_err(|_| Failure::Patch)?;
    if BINDING.get().is_some() {
        return Err(Failure::AlreadyUsed);
    }
    if !definition.titles.contains(&Some(promotion)) || promotion >= 4096 {
        return Err(Failure::Identity);
    }
    if base == 0
        || ai_owner == 0
        || callable == 0
        || callable % 8 != 0
        || base.checked_add(0x9fb4d0).is_none()
        || ai_owner.checked_add(0x1cc).is_none()
        || callable.checked_add(0x18).is_none()
    {
        return Err(Failure::Identity);
    }
    // Verify the installed primary file, plus loaded witness bytes. Address
    // parameters alone never authorize execution of an arbitrary target.
    let image=std::fs::read("C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll").map_err(|_|Failure::Identity)?;
    super::pinned::audit(&image).map_err(|_| Failure::Identity)?;
    for rva in [0x401534u64, 0x404170, 0x4a7ad0, 0x2ddbb0, 0x2dea90] {
        if Memory.copy::<16>(base + rva).as_ref().map(|v| v.as_slice())
            != image.get((rva - 0xc00) as usize..(rva - 0xc00) as usize + 16)
        {
            return Err(Failure::Identity);
        }
    }
    let mut core = ptr::null_mut();
    if unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            base as *const u16,
            &mut core,
        )
    } == 0
        || core as u64 != base
    {
        return Err(Failure::Identity);
    }
    if !civ6_native_bridge_rs::supports_atomic_promotion_fetch() {
        return Err(Failure::UnsupportedCpu);
    }
    let start =
        u64_at(ai_owner.checked_add(0x1b8).ok_or(Failure::Range)?).ok_or(Failure::Memory)?;
    let count = u32_at(ai_owner + 0x1c8).ok_or(Failure::Memory)?;
    if count == 0 || count > 256 || start == 0 {
        return Err(Failure::Caller);
    }
    let mut found = 0;
    for i in 0..count {
        if u64_at(
            start
                .checked_add(u64::from(i) * 0x40 + 0x38)
                .ok_or(Failure::Range)?,
        ) == Some(callable)
        {
            found += 1
        }
    }
    if found != 1
        || u64_at(callable) != base.checked_add(0x9fb4b8)
        || u64_at(callable + 0x10) != base.checked_add(0x4a7ad0)
    {
        return Err(Failure::Caller);
    }
    let receiver = u64_at(callable + 8).ok_or(Failure::Memory)?;
    let player =
        u64_at(receiver.checked_add(0x98).ok_or(Failure::Range)?).ok_or(Failure::Memory)?;
    let owner = u32_at(player.checked_add(0xd8).ok_or(Failure::Range)?).ok_or(Failure::Memory)?;
    if u64_at(ai_owner + 0x90) != Some(player) {
        return Err(Failure::Identity);
    }
    let stamp = lease
        .begin(owner, player)
        .filter(|s| identity(*s) && s.owner == owner && lease.current(*s, player))
        .ok_or(Failure::Lease)?;
    let mut module = ptr::null_mut();
    if unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
            ai_callback_gateway as *const () as *const u16,
            &mut module,
        )
    } == 0
    {
        return Err(Failure::Identity);
    }
    let sites = [
        (0x4a7d67u64, 0x2ddbb0u64),
        (0x30ec0f, 0x2ddbb0),
        (0x2ddbcb, 0x2dea90),
    ];
    let mut patches = vec![Patch {
        address: (callable + 0x10) as usize,
        old: base + 0x4a7ad0,
        new: gateways()[0] as u64,
        wide: true,
    }];
    for (i, (site, target)) in sites.into_iter().enumerate() {
        let call = base.checked_add(site).ok_or(Failure::Range)? as usize;
        let target = base.checked_add(target).ok_or(Failure::Range)? as usize;
        let original = Memory.copy::<5>(call as u64).ok_or(Failure::Memory)?;
        let displacement = operand(call, target)?;
        if original[0] != 0xe8
            || u32::from_le_bytes(original[1..].try_into().unwrap()) != displacement
        {
            return Err(Failure::Caller);
        }
        let near = unsafe { relay(call, gateways()[i + 1]) }?;
        patches.push(Patch {
            address: call + 1,
            old: u64::from(displacement),
            new: u64::from(operand(call, near)?),
            wide: false,
        });
        AI_GATE_TARGETS[i + 1].store(target as u64, Ordering::Release);
    }
    AI_GATE_TARGETS[0].store(base + 0x4a7ad0, Ordering::Release);
    ROWS.lock()
        .map_err(|_| Failure::Patch)?
        .reserve_exact(CAPACITY);
    BINDING
        .set(Binding {
            base,
            callback_return: base + 0x401537,
            receiver,
            player,
            owner,
            definition,
            lease,
            installed: stamp,
            promotion,
        })
        .map_err(|_| Failure::AlreadyUsed)?;
    if !lease.current(stamp, player) {
        return Err(Failure::Lease);
    }
    let registration = Registration::prepare_locked(patches, &_lock)?;
    RECOVERY
        .set(registration.clone())
        .map_err(|_| Failure::AlreadyUsed)?;
    if registration.install_locked(&_lock, &SystemApi).is_err() {
        return if registration.close_locked(&_lock, &SystemApi).is_err() {
            Err(Failure::CriticalRestore)
        } else {
            Err(Failure::Patch)
        };
    }
    if !lease.current(stamp, player) {
        return if registration.close_locked(&_lock, &SystemApi).is_err() {
            Err(Failure::CriticalRestore)
        } else {
            Err(Failure::Lease)
        };
    }
    ACTIVE.store(true, Ordering::Release);
    Ok(registration)
}
pub fn observations() -> (Vec<Observation>, u32) {
    (
        ROWS.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        DROPPED.load(Ordering::Acquire),
    )
}

#[cfg(test)]
#[path = "gateway_tests.rs"]
mod tests;
