//! Finite diagnostic vtable observation of context root-setting and main teardown.
//! No reference acquisition, getter, quantity or save operation is added.
//! Direct callers bypassing these slots are outside this observation's coverage.
use crate::lifecycle_transport::{MappedTrace, SharedLifecycleConfig, MAPPING_SIZE};
use crate::native::*;
use crate::runtime::{OathRuntimeConfig, OathStatus};
use std::cell::Cell;

pub const CAPACITY: usize = 512;
const COUNTED: u64 = 1 << 63;
thread_local! { static DEPTH: Cell<u32> = const { Cell::new(0) }; }

/// Wire states: 0 unpublished, 1 entered, 2 normal return, 3 unwound.
/// root_or_this is RDX for context slot60, RCX for main slot30.
#[repr(C)]
#[derive(Default)]
pub struct Row {
    pub state: AtomicU32,
    pub kind: AtomicU32,
    pub thread: AtomicU32,
    pub depth: AtomicU32,
    pub root_or_this: AtomicU64,
    pub rcx: AtomicU64,
    pub rdx: AtomicU64,
    pub caller: AtomicU64,
    pub rax: AtomicU64,
    pub completion_order: AtomicU32,
    pub reserved: u32,
}
/// Process-lifetime wire storage. Counters are point-in-time observations,
/// never a certificate permitting DLL/code/data retirement.
#[repr(C)]
pub struct LifecycleTrace {
    pub magic: u32,
    pub version: u32,
    pub next: AtomicU32,
    pub dropped: AtomicU32,
    pub flight: AtomicU32,
    pub completed: AtomicU32,
    pub unwound: AtomicU32,
    pub stopped: AtomicU32,
    pub deadline_ms: AtomicU64,
    pub expired: AtomicU32,
    pub detached: AtomicU32,
    pub rows: [Row; CAPACITY],
}
impl LifecycleTrace {
    pub(crate) fn new() -> Self {
        Self {
            magic: 0x4c43_3134,
            version: 1,
            next: AtomicU32::new(0),
            dropped: AtomicU32::new(0),
            flight: AtomicU32::new(0),
            completed: AtomicU32::new(0),
            unwound: AtomicU32::new(0),
            stopped: AtomicU32::new(1),
            deadline_ms: AtomicU64::new(0),
            expired: AtomicU32::new(0),
            detached: AtomicU32::new(0),
            rows: std::array::from_fn(|_| Row::default()),
        }
    }
    #[cfg(test)]
    fn boxed() -> Box<Self> {
        Box::new(Self::new())
    }
    #[cfg(test)]
    fn start(&self, seconds: u32) -> Result<(), OathStatus> {
        self.start_until(seconds, 0)
    }
    fn start_until(&self, seconds: u32, absolute: u64) -> Result<(), OathStatus> {
        let current = now();
        let relative = current.saturating_add(u64::from(seconds) * 1000);
        let due = if absolute == 0 {
            relative
        } else {
            relative.min(absolute)
        };
        if due <= current {
            return Err(OathStatus::WrongState);
        }
        self.deadline_ms.store(due, Ordering::Release);
        self.stopped
            .compare_exchange(1, 0, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|_| OathStatus::WrongState)
    }
    fn stop(&self) {
        self.stopped.store(2, Ordering::Release);
    }
    fn poll(&self) {
        let due = self.deadline_ms.load(Ordering::Acquire);
        if due != 0 && now() >= due {
            self.expired.store(1, Ordering::Release);
            self.stop();
        }
    }
    pub(crate) fn snapshot(&self) -> Vec<u8> {
        let mut bytes = vec![0; std::mem::size_of::<Self>()];
        macro_rules! field {
            ($value:expr, $offset:expr) => {{
                let value = $value.to_le_bytes();
                bytes[$offset..$offset + value.len()].copy_from_slice(&value);
            }};
        }
        // These fields are immutable after the parent's sole initialization.
        field!(self.magic, std::mem::offset_of!(Self, magic));
        field!(self.version, std::mem::offset_of!(Self, version));
        macro_rules! header {
            ($($name:ident),*) => {$(
                field!(self.$name.load(Ordering::Acquire), std::mem::offset_of!(Self, $name));
            )*};
        }
        header!(
            next,
            dropped,
            flight,
            completed,
            unwound,
            stopped,
            deadline_ms,
            expired,
            detached
        );
        for (i, row) in self.rows.iter().enumerate() {
            let base = std::mem::offset_of!(Self, rows) + i * std::mem::size_of::<Row>();
            macro_rules! row_fields {
                ($($name:ident),*) => {$(
                    field!(row.$name.load(Ordering::Acquire), base + std::mem::offset_of!(Row, $name));
                )*};
            }
            // Acquire the publication state before reading payload. An entered
            // row may complete during this snapshot; its state stays conservative.
            row_fields!(
                state,
                kind,
                thread,
                depth,
                root_or_this,
                rcx,
                rdx,
                caller,
                rax,
                completion_order
            );
            field!(row.reserved, base + std::mem::offset_of!(Row, reserved));
        }
        bytes
    }
    pub(crate) fn initialized(&self) -> bool {
        self.snapshot() == Self::new().snapshot()
    }
    pub fn wire_layout() -> (usize, usize, usize) {
        (
            std::mem::offset_of!(Self, rows),
            std::mem::size_of::<Row>(),
            std::mem::size_of::<Self>(),
        )
    }
}
fn now() -> u64 {
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }
}
static TRACE: AtomicPtr<LifecycleTrace> = AtomicPtr::new(null_mut());
static ORIGINALS: [AtomicU64; 2] = [const { AtomicU64::new(0) }; 2];
fn trace() -> Option<&'static LifecycleTrace> {
    unsafe { TRACE.load(Ordering::Acquire).as_ref() }
}

unsafe extern "system-unwind" fn begin(
    rcx: u64,
    rdx: u64,
    _r8: u64,
    kind: u64,
    caller: u64,
    token: *mut u64,
) {
    let Some(t) = trace() else {
        return;
    };
    t.flight.fetch_add(1, Ordering::AcqRel);
    unsafe {
        ptr::write_volatile(token, COUNTED);
    }
    let depth = DEPTH.with(|d| {
        let n = d.get() + 1;
        d.set(n);
        n
    });
    stage(1);
    t.poll();
    if t.stopped.load(Ordering::Acquire) != 0 {
        return;
    }
    let Ok(i) = t
        .next
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n < CAPACITY as u32).then_some(n + 1)
        })
    else {
        t.dropped.fetch_add(1, Ordering::Relaxed);
        return;
    };
    unsafe {
        ptr::write_volatile(token, COUNTED | u64::from(i + 1));
    }
    stage(2);
    let r = &t.rows[i as usize];
    r.kind.store(kind as u32, Ordering::Relaxed);
    r.thread
        .store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
    r.depth.store(depth, Ordering::Relaxed);
    r.root_or_this
        .store(if kind == 0 { rdx } else { rcx }, Ordering::Relaxed);
    r.rcx.store(rcx, Ordering::Relaxed);
    r.rdx.store(rdx, Ordering::Relaxed);
    r.caller.store(caller, Ordering::Relaxed);
    r.state.store(1, Ordering::Release);
    stage(3);
}
unsafe fn settle(token: *mut u64, result: Option<u64>) {
    let claim = unsafe { ptr::read_volatile(token) };
    unsafe { ptr::write_volatile(token, 0) };
    if claim == 0 {
        return;
    }
    let Some(t) = trace() else {
        return;
    };
    DEPTH.with(|d| d.set(d.get() - 1));
    if result.is_none() {
        t.unwound.fetch_add(1, Ordering::Relaxed);
    }
    let i = (claim & !COUNTED) as usize;
    if i > 0 && i <= CAPACITY {
        let r = &t.rows[i - 1];
        if let Some(rax) = result {
            r.rax.store(rax, Ordering::Relaxed);
            r.completion_order.store(
                t.completed.fetch_add(1, Ordering::AcqRel) + 1,
                Ordering::Relaxed,
            );
            r.state.store(2, Ordering::Release);
        } else {
            r.state.store(3, Ordering::Release);
        }
    }
    t.flight.fetch_sub(1, Ordering::Release);
}
unsafe extern "system-unwind" fn end(token: *mut u64, rax: u64) {
    stage(4);
    unsafe { settle(token, Some(rax)) };
}
#[cfg(test)]
static INJECT: AtomicU32 = AtomicU32::new(0);
#[inline(always)]
fn stage(_stage: u32) {
    #[cfg(test)]
    if INJECT
        .compare_exchange(_stage, 0, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        panic!("synthetic lifecycle callback stage {_stage}");
    }
}
unsafe extern "system" fn unwind(
    exception: *mut c_void,
    frame: u64,
    _context: *mut c_void,
    _dispatcher: *mut c_void,
) -> i32 {
    let flags = unsafe { ptr::read_unaligned((exception as *const u8).add(4).cast::<u32>()) };
    if flags & 0x66 != 0 {
        unsafe { settle((frame + 0x68) as *mut u64, None) };
    }
    1
}

// Actual slots have register-only arguments. No copied instruction or dynamic
// trampoline is used. Original native function/unwind information stays intact.
macro_rules! gateway {
    ($name:literal,$index:literal) => {global_asm!(
        concat!(".globl ",$name),concat!(".seh_proc ",$name),concat!($name,":"),
        ".seh_handler {unwind}, @unwind","sub rsp,0xf8",".seh_stackalloc 0xf8",
        "mov qword ptr [rsp+0x68],0",".seh_endprologue",
        "mov [rsp+0xe0],rcx","mov [rsp+0xe8],rdx","mov [rsp+0x40],r8","mov [rsp+0x48],r9",
        "mov [rsp+0x50],rax","mov [rsp+0x58],r10","mov [rsp+0x60],r11",
        "movdqu [rsp+0x80],xmm0","movdqu [rsp+0x90],xmm1","movdqu [rsp+0xa0],xmm2",
        "movdqu [rsp+0xb0],xmm3","movdqu [rsp+0xc0],xmm4","movdqu [rsp+0xd0],xmm5",
        concat!("mov r9d,",$index),"mov rax,[rsp+0xf8]","mov [rsp+0x20],rax",
        "lea rax,[rsp+0x68]","mov [rsp+0x28],rax","call {begin}",
        "mov rcx,[rsp+0xe0]","mov rdx,[rsp+0xe8]","mov r8,[rsp+0x40]","mov r9,[rsp+0x48]",
        "mov rax,[rsp+0x50]","mov r10,[rsp+0x58]","mov r11,[rsp+0x60]",
        "movdqu xmm0,[rsp+0x80]","movdqu xmm1,[rsp+0x90]","movdqu xmm2,[rsp+0xa0]",
        "movdqu xmm3,[rsp+0xb0]","movdqu xmm4,[rsp+0xc0]","movdqu xmm5,[rsp+0xd0]",
        concat!("call qword ptr [rip+{originals}+",$index,"*8]"),
        "mov [rsp+0x70],rax","movdqu [rsp+0x80],xmm0","mov [rsp+0xe8],rdx",
        "lea rcx,[rsp+0x68]","mov rdx,rax","call {end}",
        "mov rax,[rsp+0x70]","mov rdx,[rsp+0xe8]","movdqu xmm0,[rsp+0x80]",
        "add rsp,0xf8","ret",".seh_endproc",
        begin=sym begin,end=sym end,unwind=sym unwind,originals=sym ORIGINALS
    );};
}
gateway!("lifecycle_root_gateway", "0");
gateway!("lifecycle_teardown_gateway", "1");
unsafe extern "system" {
    fn lifecycle_root_gateway();
    fn lifecycle_teardown_gateway();
}
fn gateway_address(index: usize) -> u64 {
    [
        lifecycle_root_gateway as *const (),
        lifecycle_teardown_gateway as *const (),
    ][index] as u64
}

struct Binding {
    address: usize,
    original: u64,
    gateway: u64,
    published: bool,
    settled: bool,
    protection: u32,
}
static BINDINGS: Mutex<[Option<Binding>; 2]> = Mutex::new([const { None }; 2]);

static CONTROL: Mutex<bool> = Mutex::new(false);
static SECONDS: AtomicU32 = AtomicU32::new(0);
// Set only by the cold callback, before Install/Start, never by an export.
static COLD_DEADLINE: AtomicU64 = AtomicU64::new(0);
pub(crate) fn bound_cold_start(deadline: u64) -> Result<(), OathStatus> {
    COLD_DEADLINE
        .compare_exchange(0, deadline, Ordering::AcqRel, Ordering::Acquire)
        .map(|_| ())
        .map_err(|_| OathStatus::WrongState)
}
/// Uses exactly the existing fixed-process/image identity contract, followed
/// by lifecycle code and table ownership checks. flags must remain zero.
#[unsafe(no_mangle)]
pub extern "system" fn Civ6OathLifecycleInstall(input: *mut c_void) -> u32 {
    let status = std::panic::catch_unwind(|| {
        if !committed_range(input as usize, 48, 8, true) {
            return OathStatus::InvalidInput;
        }
        let mut c = unsafe { ptr::read(input.cast::<OathRuntimeConfig>()) };
        if crate::runtime::validate(&c).is_err() || c.flags != 0 {
            return OathStatus::InvalidInput;
        }
        if let Err(status) = install(&mut c, None, Civ6OathLifecycleInstall as *const () as usize) {
            return status;
        }
        unsafe { ptr::write(input.cast::<OathRuntimeConfig>(), c) };
        OathStatus::Ok
    })
    .unwrap_or(OathStatus::Panic);
    status as u32
}
/// Shared transport entry; never called from DllMain. Validates the same
/// fixed process/image/slot contract as the legacy entry before publication.
/// Does not initialize or reset the parent's mapping, or close the duplicated
/// target handle. On success (and attempted publication failure) its view is
/// retained until process exit; the parent can read its own view after exit.
#[unsafe(no_mangle)]
pub extern "system" fn Civ6OathLifecycleInstallShared(input: *mut c_void) -> u32 {
    install_shared_verified(input, crate::identity_windows::verify_lifecycle)
}
// Sealed test injection; the public export always uses the current-game gate.
pub(crate) fn install_shared_verified(
    input: *mut c_void,
    verify: impl FnOnce(&OathRuntimeConfig) -> Result<[(usize, u64); 2], OathStatus>
        + std::panic::UnwindSafe,
) -> u32 {
    std::panic::catch_unwind(|| {
        if !committed_range(
            input as usize,
            std::mem::size_of::<SharedLifecycleConfig>(),
            8,
            true,
        ) {
            return OathStatus::InvalidInput;
        }
        let mut c = unsafe { ptr::read(input.cast::<SharedLifecycleConfig>()) };
        if crate::runtime::validate(&c.config).is_err()
            || c.config.flags != 0
            || c.mapping_size != MAPPING_SIZE
            || c.reserved != 0
            || c.mapping_handle == 0
            || c.mapping_handle == u64::MAX
        {
            return OathStatus::InvalidInput;
        }
        // The transport contract requires sole initialization before sharing,
        // immutable header/reserved fields, and exclusively atomic publication.
        let view = match unsafe { MappedTrace::map(c.mapping_handle as _, c.mapping_size) } {
            Ok(view) if view.trace().initialized() => view,
            _ => return OathStatus::InvalidInput,
        };
        if let Err(status) = install_verified(
            &mut c.config,
            Some(view),
            Civ6OathLifecycleInstallShared as *const () as usize,
            verify,
        ) {
            return status;
        }
        unsafe { ptr::write(input.cast::<SharedLifecycleConfig>(), c) };
        OathStatus::Ok
    })
    .unwrap_or(OathStatus::Panic) as u32
}

fn install(
    c: &mut OathRuntimeConfig,
    shared: Option<MappedTrace>,
    entry: usize,
) -> Result<(), OathStatus> {
    install_verified(c, shared, entry, crate::identity_windows::verify_lifecycle)
}
fn install_verified(
    c: &mut OathRuntimeConfig,
    shared: Option<MappedTrace>,
    entry: usize,
    verify: impl FnOnce(&OathRuntimeConfig) -> Result<[(usize, u64); 2], OathStatus>,
) -> Result<(), OathStatus> {
    let mut used = CONTROL.lock().map_err(|_| OathStatus::Panic)?;
    if *used {
        return Err(OathStatus::WrongState);
    }
    let slots = verify(c)?;
    if crate::runtime::pin_module(entry).is_none() {
        return Err(OathStatus::HookFailure);
    }
    *used = true;
    let t = match shared {
        Some(view) => view.retain(),
        None => Box::leak(Box::new(LifecycleTrace::new())),
    };
    for (i, (address, original)) in slots.into_iter().enumerate() {
        if unsafe { install_at(i, address, original, t) }.is_err() {
            t.stop();
            return Err(if unsafe { uninstall() }.is_ok() {
                OathStatus::HookFailure
            } else {
                OathStatus::Critical
            });
        }
    }
    c.out_trace_address = t as *const _ as u64;
    c.trace_size = std::mem::size_of_val(t) as u32;
    // Install is inactive; start controls the finite deadline.
    SECONDS.store(c.max_seconds, Ordering::Release);
    Ok(())
}
#[unsafe(no_mangle)]
pub extern "system" fn Civ6OathLifecycleStart() -> u32 {
    let Ok(_guard) = CONTROL.lock() else {
        return OathStatus::Panic as u32;
    };
    let Some(t) = trace() else {
        return OathStatus::WrongState as u32;
    };
    if t.stopped.load(Ordering::Acquire) != 1 {
        return OathStatus::WrongState as u32;
    }
    t.start_until(
        SECONDS.load(Ordering::Acquire),
        COLD_DEADLINE.load(Ordering::Acquire),
    )
    .map_or_else(|e| e as u32, |_| 0)
}
#[unsafe(no_mangle)]
pub extern "system" fn Civ6OathLifecycleStop() -> u32 {
    let Ok(_guard) = CONTROL.lock() else {
        return OathStatus::Panic as u32;
    };
    let Some(t) = trace() else {
        return OathStatus::WrongState as u32;
    };
    t.stop();
    if unsafe { uninstall() }.is_ok() {
        0
    } else {
        OathStatus::Critical as u32
    }
}
/// Finite poll/restore; trace address/size was returned by Install. Parent
/// copies the repr(C) trace via ReadProcessMemory. No permanent drain promised.
#[unsafe(no_mangle)]
pub extern "system" fn Civ6OathLifecyclePoll() -> u32 {
    let Ok(_guard) = CONTROL.lock() else {
        return OathStatus::Panic as u32;
    };
    let Some(t) = trace() else {
        return OathStatus::WrongState as u32;
    };
    t.poll();
    if t.stopped.load(Ordering::Acquire) == 2 && unsafe { uninstall() }.is_err() {
        OathStatus::Critical as u32
    } else {
        0
    }
}

#[cfg(test)]
#[path = "lifecycle_trace_tests.rs"]
mod tests;
/// # Safety
/// Crate-internal only: the sealed identity gate must prove both image/table
/// ownership and the original register-only ABI, and pin modules first.
/// Storage and targets must remain valid through process termination, even
/// when publication or restoration fails. Synthetic tests own their slots.
pub(crate) unsafe fn install_at(
    index: usize,
    address: usize,
    original: u64,
    trace: &'static LifecycleTrace,
) -> Result<(), &'static str> {
    if index >= 2 || original == 0 || !committed_range(address, 8, 8, false) {
        return Err("invalid lifecycle slot");
    }
    let mut bindings = BINDINGS.lock().map_err(|_| "lifecycle bindings poisoned")?;
    if bindings[index].is_some() {
        return Err("lifecycle binding frozen; restart process");
    }
    let trace_ptr = trace as *const LifecycleTrace as *mut LifecycleTrace;
    let current = TRACE.load(Ordering::Acquire);
    if !current.is_null() && current != trace_ptr {
        return Err("lifecycle trace frozen");
    }
    let slot = unsafe { &*(address as *const AtomicU64) };
    if slot.load(Ordering::Acquire) != original {
        return Err("lifecycle original slot differs");
    }
    let mut protection = 0;
    if unsafe { VirtualProtect(address as *const c_void, 8, PAGE_READWRITE, &mut protection) } == 0
    {
        return Err("lifecycle protect failed");
    }
    let gateway = gateway_address(index);
    ORIGINALS[index].store(original, Ordering::Release);
    TRACE.store(trace_ptr, Ordering::Release);
    bindings[index] = Some(Binding {
        address,
        original,
        gateway,
        published: true,
        settled: false,
        protection,
    });
    let exchanged = slot
        .compare_exchange(original, gateway, Ordering::AcqRel, Ordering::Acquire)
        .is_ok();
    let mut ignored = 0;
    let restored =
        unsafe { VirtualProtect(address as *const c_void, 8, protection, &mut ignored) } != 0;
    // An attempted CAS is conservatively frozen. No target or trace can be
    // rebound even if this invocation lost a race to a foreign writer.
    bindings[index].as_mut().unwrap().settled = restored;
    if !exchanged {
        return Err("lifecycle publication lost; binding retained");
    }
    if !restored {
        return Err("lifecycle protection unresolved; binding retained");
    }
    Ok(())
}
pub(crate) unsafe fn uninstall() -> Result<(), &'static str> {
    let mut bindings = BINDINGS.lock().map_err(|_| "lifecycle bindings poisoned")?;
    let mut failed = false;
    for binding in bindings.iter_mut().flatten() {
        if !binding.published {
            continue;
        }
        let mut protection = 0;
        if unsafe {
            VirtualProtect(
                binding.address as *const c_void,
                8,
                PAGE_READWRITE,
                &mut protection,
            )
        } == 0
        {
            failed = true;
            continue;
        }
        let slot = unsafe { &*(binding.address as *const AtomicU64) };
        let exchanged = slot
            .compare_exchange(
                binding.gateway,
                binding.original,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
            || slot.load(Ordering::Acquire) == binding.original;
        let mut ignored = 0;
        let restored = unsafe {
            VirtualProtect(
                binding.address as *const c_void,
                8,
                binding.protection,
                &mut ignored,
            )
        } != 0;
        binding.settled = exchanged && restored;
        if !binding.settled {
            failed = true;
        }
    }
    if failed {
        Err("lifecycle uninstallation unresolved; bindings retained")
    } else {
        if let Some(trace) = unsafe { TRACE.load(Ordering::Acquire).as_ref() } {
            trace.detached.store(1, Ordering::Release);
        }
        Ok(())
    }
}
