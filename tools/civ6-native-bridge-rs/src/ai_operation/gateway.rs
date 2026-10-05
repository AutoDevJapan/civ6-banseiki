//! Optional in-process registration. No exported entry, numeric property permit,
//! Lua, dispatcher invocation, process injector, or automatic startup.
//! RuntimeLease must be implemented INSIDE the owning runtime with its existing
//! lease/ARM and atomic CallFence; this module cannot manufacture that authority.
use civ6_native_bridge_rs::governor_payment::{
    read_current, CurrentMemory, PaymentDefinition, PromotionSnapshot,
};
use std::{
    cell::Cell,
    ffi::c_void,
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        Mutex, OnceLock,
    },
};
use windows_sys::Win32::{
    Foundation::FILETIME,
    System::{
        Diagnostics::Debug::{FlushInstructionCache, ReadProcessMemory},
        LibraryLoader::{
            GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            GET_MODULE_HANDLE_EX_FLAG_PIN,
        },
        Memory::{
            VirtualAlloc, VirtualFree, VirtualProtect, VirtualQuery, MEMORY_BASIC_INFORMATION,
            MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE,
            PAGE_GUARD, PAGE_NOACCESS, PAGE_READWRITE,
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
/// # Safety
/// Implement only against the real parent-owned lease/PRODARM and call fence.
/// A boolean constant, fixture value, or numeric property is not an adapter.
/// `current` must revalidate atomic state and receiver binding, including load.
pub unsafe trait RuntimeLease: Sync {
    fn begin(&self, owner: u32, player: u64) -> Option<LeaseStamp>;
    fn current(&self, stamp: LeaseStamp, player: u64) -> bool;
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
}
const CAPACITY: usize = 128;
static ROWS: Mutex<Vec<Observation>> = Mutex::new(Vec::new());
static NEXT: AtomicU32 = AtomicU32::new(1);
static DROPPED: AtomicU32 = AtomicU32::new(0);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static BINDING: OnceLock<Binding> = OnceLock::new();
thread_local! {static INVOCATION:Cell<Option<Invocation>>=const {Cell::new(None)};}
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

/// Records copied entry/exit registers, never changes them. A missing lease
/// suppresses observation while the immutable original target still runs once.
extern "system" fn observe(raw: *mut Registers, after: u32) {
    let _ = std::panic::catch_unwind(|| {
        let Some(binding) = BINDING.get() else { return };
        let raw = unsafe { &mut *raw };
        let thread = unsafe { GetCurrentThreadId() };
        if after == 0 {
            raw.ticket = u64::from(NEXT.fetch_add(1, Ordering::AcqRel));
        }
        let mut invocation = INVOCATION.with(Cell::get);
        if raw.site == 0 && after == 0 {
            // Nested callback or non-natural entry cannot replace a live context.
            if invocation.is_none()
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
                    .filter(|s| identity(*s) && s.owner == binding.owner)
                    .map(|stamp| Invocation {
                        stamp,
                        player: binding.player,
                        receiver: raw.rcx,
                        thread,
                    });
                INVOCATION.with(|cell| cell.set(invocation));
            }
        }
        let accepted = ACTIVE.load(Ordering::Acquire)
            && invocation.is_some_and(|i| {
                i.thread == thread
                    && identity(i.stamp)
                    && binding.lease.current(i.stamp, i.player)
                    && u64_at(i.receiver + 0x98) == Some(i.player)
                    && u32_at(i.player + 0xd8) == Some(i.stamp.owner)
            });
        let mut own = None;
        let mut accepted = accepted;
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
        let row = Observation {
            sequence: raw.ticket as u32,
            thread,
            after: after != 0,
            raw: *raw,
            accepted,
            owner: invocation.map(|i| i.stamp.owner),
            generation: invocation.map_or(0, |i| i.stamp.generation),
            own,
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
            INVOCATION.with(|cell| cell.set(None));
        }
    });
}

// Every target is published once BEFORE registration and retained permanently.
// Removing a registration never points a delayed invocation at another target.
#[unsafe(no_mangle)]
static AI_GATE_TARGETS: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
macro_rules! gateway {
    ($name:literal,$site:literal,$offset:literal)=>{core::arch::global_asm!(concat!(
        ".globl ",$name,"\n.seh_proc ",$name,"\n",$name,":\n",
        "pushfq\n.seh_stackalloc 8\nsub rsp,0x1b0\n.seh_stackalloc 0x1b0\n.seh_endprologue\n",
        "mov [rsp+0x60],rcx\nmov [rsp+0x68],rdx\nmov [rsp+0x70],r8\nmov [rsp+0x78],r9\nmov [rsp+0x80],rax\nmov [rsp+0x88],r10\nmov [rsp+0x90],r11\n",
        "mov rax,[rsp+0x1b0]\nmov [rsp+0x98],rax\n",
        "movdqu [rsp+0xa0],xmm0\nmovdqu [rsp+0xb0],xmm1\nmovdqu [rsp+0xc0],xmm2\nmovdqu [rsp+0xd0],xmm3\nmovdqu [rsp+0xe0],xmm4\nmovdqu [rsp+0xf0],xmm5\n",
        "lea rax,[rsp+0x1b8]\nmov [rsp+0x100],rax\nmov rax,[rax]\nmov [rsp+0x108],rax\nmov qword ptr [rsp+0x110],",$site,"\n",
        "lea rcx,[rsp+0x60]\nxor edx,edx\ncall {record}\n",
        // Exactly 0x60 bytes from the TRUE caller's home/stack arguments, not
        // the gateway's saved registers. All reads occur within the copied span.
        "movdqu xmm0,[rsp+0x1c0]\nmovdqu [rsp],xmm0\nmovdqu xmm0,[rsp+0x1d0]\nmovdqu [rsp+0x10],xmm0\n",
        "movdqu xmm0,[rsp+0x1e0]\nmovdqu [rsp+0x20],xmm0\nmovdqu xmm0,[rsp+0x1f0]\nmovdqu [rsp+0x30],xmm0\n",
        "movdqu xmm0,[rsp+0x200]\nmovdqu [rsp+0x40],xmm0\nmovdqu xmm0,[rsp+0x210]\nmovdqu [rsp+0x50],xmm0\n",
        "movdqu xmm0,[rsp+0xa0]\nmovdqu xmm1,[rsp+0xb0]\nmovdqu xmm2,[rsp+0xc0]\nmovdqu xmm3,[rsp+0xd0]\nmovdqu xmm4,[rsp+0xe0]\nmovdqu xmm5,[rsp+0xf0]\n",
        "mov rcx,[rsp+0x60]\nmov rdx,[rsp+0x68]\nmov r8,[rsp+0x70]\nmov r9,[rsp+0x78]\nmov rax,[rsp+0x80]\nmov r10,[rsp+0x88]\nmov r11,[rsp+0x90]\npush qword ptr [rsp+0x98]\npopfq\n",
        "call qword ptr [rip+AI_GATE_TARGETS+",$offset,"]\n",
        "pushfq\npop qword ptr [rsp+0x98]\nmov [rsp+0x60],rcx\nmov [rsp+0x68],rdx\nmov [rsp+0x70],r8\nmov [rsp+0x78],r9\nmov [rsp+0x80],rax\nmov [rsp+0x88],r10\nmov [rsp+0x90],r11\n",
        "movdqu [rsp+0xa0],xmm0\nmovdqu [rsp+0xb0],xmm1\nmovdqu [rsp+0xc0],xmm2\nmovdqu [rsp+0xd0],xmm3\nmovdqu [rsp+0xe0],xmm4\nmovdqu [rsp+0xf0],xmm5\n",
        "lea rcx,[rsp+0x60]\nmov edx,1\ncall {record}\n",
        "movdqu xmm0,[rsp+0xa0]\nmovdqu xmm1,[rsp+0xb0]\nmovdqu xmm2,[rsp+0xc0]\nmovdqu xmm3,[rsp+0xd0]\nmovdqu xmm4,[rsp+0xe0]\nmovdqu xmm5,[rsp+0xf0]\n",
        "mov rcx,[rsp+0x60]\nmov rdx,[rsp+0x68]\nmov r8,[rsp+0x70]\nmov r9,[rsp+0x78]\nmov rax,[rsp+0x80]\nmov r10,[rsp+0x88]\nmov r11,[rsp+0x90]\n",
        "push qword ptr [rsp+0x98]\npopfq\nlea rsp,[rsp+0x1b8]\nret\n.seh_endproc\n"
    ),record=sym observe);};
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
}
pub fn operand(call: usize, target: usize) -> Result<u32, Failure> {
    let op = call.checked_add(1).ok_or(Failure::Range)?;
    if op % 4 != 0 || op % 64 > 60 || op % 4096 > 4092 {
        return Err(Failure::Alignment);
    }
    let next = call.checked_add(5).ok_or(Failure::Range)?;
    Ok(i32::try_from(target as i128 - next as i128).map_err(|_| Failure::Range)? as u32)
}
struct Patch {
    address: usize,
    old: u64,
    new: u64,
    wide: bool,
}
pub struct Registration {
    patches: Vec<Patch>,
}
unsafe impl Send for Registration {}
static INSTALL: Mutex<()> = Mutex::new(());
fn writable(address: usize, size: usize) -> bool {
    let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    (unsafe { VirtualQuery(address as *const _, &mut info, std::mem::size_of_val(&info)) != 0 })
        && info.State == MEM_COMMIT
        && info.Protect & (PAGE_GUARD | PAGE_NOACCESS) == 0
        && address
            .checked_add(size)
            .is_some_and(|end| end <= info.BaseAddress as usize + info.RegionSize)
}
unsafe fn exchange(p: &Patch, restore: bool) -> Result<(), Failure> {
    let size = if p.wide { 8 } else { 4 };
    if !writable(p.address, size) {
        return Err(Failure::Memory);
    }
    let mut previous = 0;
    if unsafe {
        VirtualProtect(
            p.address as *const _,
            size,
            PAGE_EXECUTE_READWRITE,
            &mut previous,
        )
    } == 0
    {
        return Err(Failure::Patch);
    }
    let (old, new) = if restore {
        (p.new, p.old)
    } else {
        (p.old, p.new)
    };
    let success = if p.wide {
        unsafe { &*(p.address as *const AtomicU64) }
            .compare_exchange(old, new, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    } else {
        unsafe { &*(p.address as *const AtomicU32) }
            .compare_exchange(old as u32, new as u32, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    };
    let mut discard = 0;
    let protection =
        unsafe { VirtualProtect(p.address as *const _, size, previous, &mut discard) } != 0;
    let flush =
        unsafe { FlushInstructionCache(GetCurrentProcess(), p.address as *const _, size) } != 0;
    if success && protection && flush {
        Ok(())
    } else {
        Err(Failure::Patch)
    }
}
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
    lease: &'static dyn RuntimeLease,
) -> Result<Registration, Failure> {
    let _lock = INSTALL.lock().map_err(|_| Failure::Patch)?;
    if BINDING.get().is_some() {
        return Err(Failure::AlreadyUsed);
    }
    if base == 0 || ai_owner == 0 || callable == 0 || callable % 8 != 0 {
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
        })
        .map_err(|_| Failure::AlreadyUsed)?;
    if !lease.current(stamp, player) {
        return Err(Failure::Lease);
    }
    let registration = Registration { patches };
    for patch in &registration.patches {
        if unsafe { exchange(patch, false) }.is_err() {
            let _ = registration.close();
            return Err(Failure::Patch);
        }
    }
    ACTIVE.store(true, Ordering::Release);
    Ok(registration)
}
impl Registration {
    pub fn close(&self) -> Result<(), Failure> {
        ACTIVE.store(false, Ordering::Release);
        let mut result = Ok(());
        for p in self.patches.iter().rev() {
            let current = if p.wide {
                u64_at(p.address as u64)
            } else {
                u32_at(p.address as u64).map(u64::from)
            };
            if current == Some(p.old) {
                continue;
            }
            if unsafe { exchange(p, true) }.is_err() {
                result = Err(Failure::Patch)
            }
        }
        result
    }
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
