//! Diagnostic entry at the pinned effect vtable's actual callee boundary.
//! RDX is the receiver delivered by the City adapter, not a shared_ptr.
//! This records raw arguments and completion only. There is no quantity write,
//! City lifetime authority, public export or product bootstrap in this module.
use crate::native::*;
use crate::oath_record::{OathCall, OathEffectFamily};

const CAPACITY: usize = 1024;
const COUNTED: u64 = 1 << 63;
const MASK: u64 = 0xffff_ffff;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecipientRecord {
    pub state: u32,
    pub slot: u32,
    pub thread: u32,
    pub effect: u64,
    pub actual_receiver_argument: u64,
    pub raw_r8: u64,
    pub caller: u64,
    pub caller_rbp: u64,
    pub caller_rdi: u64,
    pub original_rax: Option<u64>,
}
#[derive(Default)]
struct Row {
    state: AtomicU32,
    slot: AtomicU32,
    thread: AtomicU32,
    effect: AtomicU64,
    city: AtomicU64,
    r8: AtomicU64,
    caller: AtomicU64,
    rbp: AtomicU64,
    rdi: AtomicU64,
    rax: AtomicU64,
}
pub struct RecipientTrace {
    stopped: AtomicU32,
    flight: AtomicU32,
    next: AtomicU32,
    dropped: AtomicU32,
    completed: AtomicU32,
    unwound: AtomicU32,
    rows: [Row; CAPACITY],
}
impl RecipientTrace {
    pub fn boxed() -> Box<Self> {
        let mut trace = Box::<Self>::new_uninit();
        unsafe {
            let p = trace.as_mut_ptr();
            ptr::addr_of_mut!((*p).stopped).write(AtomicU32::new(1));
            ptr::addr_of_mut!((*p).flight).write(AtomicU32::new(0));
            ptr::addr_of_mut!((*p).next).write(AtomicU32::new(0));
            ptr::addr_of_mut!((*p).dropped).write(AtomicU32::new(0));
            ptr::addr_of_mut!((*p).completed).write(AtomicU32::new(0));
            ptr::addr_of_mut!((*p).unwound).write(AtomicU32::new(0));
            let rows = ptr::addr_of_mut!((*p).rows).cast::<Row>();
            for i in 0..CAPACITY {
                rows.add(i).write(Row::default());
            }
            trace.assume_init()
        }
    }
    pub fn start(&self) -> Result<(), &'static str> {
        self.stopped
            .compare_exchange(1, 0, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| ())
            .map_err(|_| "recipient observation cannot restart")
    }
    pub fn stop(&self) {
        self.stopped.store(2, Ordering::Release);
    }
    /// Flight is acquired before reading aggregate counts. Stopped + zero
    /// flight prevents recording callbacks from changing these counts later.
    pub fn counts(&self) -> (bool, u32, u32, u32, u32) {
        let stopped = self.stopped.load(Ordering::Acquire) == 2;
        let flight = self.flight.load(Ordering::Acquire);
        (
            stopped && flight == 0,
            flight,
            self.completed.load(Ordering::Acquire),
            self.unwound.load(Ordering::Acquire),
            self.dropped.load(Ordering::Acquire),
        )
    }
    pub fn records(&self) -> Vec<RecipientRecord> {
        let n = self.next.load(Ordering::Acquire).min(CAPACITY as u32);
        (0..n)
            .map(|i| {
                let r = &self.rows[i as usize];
                let state = r.state.load(Ordering::Acquire);
                RecipientRecord {
                    state,
                    slot: r.slot.load(Ordering::Relaxed),
                    thread: r.thread.load(Ordering::Relaxed),
                    effect: r.effect.load(Ordering::Relaxed),
                    actual_receiver_argument: r.city.load(Ordering::Relaxed),
                    raw_r8: r.r8.load(Ordering::Relaxed),
                    caller: r.caller.load(Ordering::Relaxed),
                    caller_rbp: r.rbp.load(Ordering::Relaxed),
                    caller_rdi: r.rdi.load(Ordering::Relaxed),
                    original_rax: (state == 2).then(|| r.rax.load(Ordering::Relaxed)),
                }
            })
            .collect()
    }
}
static TRACE: AtomicPtr<RecipientTrace> = AtomicPtr::new(null_mut());
static ORIGINALS: [AtomicU64; 8] = [const { AtomicU64::new(0) }; 8];
struct Binding {
    address: usize,
    original: u64,
    gateway: u64,
    published: bool,
    settled: bool,
    protection: u32,
}
static BINDINGS: Mutex<[Option<Binding>; 8]> = Mutex::new([const { None }; 8]);

unsafe extern "system-unwind" fn begin(
    effect: u64,
    city: u64,
    r8: u64,
    slot: u64,
    caller: u64,
    token: *mut u64,
    rbp: u64,
    rdi: u64,
) {
    let Some(trace) = (unsafe { TRACE.load(Ordering::Acquire).as_ref() }) else {
        return;
    };
    trace.flight.fetch_add(1, Ordering::AcqRel);
    unsafe { ptr::write_volatile(token, COUNTED) };
    if trace.stopped.load(Ordering::Acquire) != 0 {
        return;
    }
    let Ok(index) = trace
        .next
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n < CAPACITY as u32).then_some(n + 1)
        })
    else {
        trace.dropped.fetch_add(1, Ordering::Relaxed);
        return;
    };
    unsafe { ptr::write_volatile(token, COUNTED | u64::from(index + 1)) };
    let row = &trace.rows[index as usize];
    row.slot.store(slot as u32, Ordering::Relaxed);
    row.thread
        .store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
    row.effect.store(effect, Ordering::Relaxed);
    row.city.store(city, Ordering::Relaxed);
    row.r8.store(r8, Ordering::Relaxed);
    row.caller.store(caller, Ordering::Relaxed);
    row.rbp.store(rbp, Ordering::Relaxed);
    row.rdi.store(rdi, Ordering::Relaxed);
    row.state.store(1, Ordering::Release);
}
unsafe fn settle(token: *mut u64, rax: Option<u64>) {
    let claim = unsafe { ptr::read_volatile(token) };
    unsafe { ptr::write_volatile(token, 0) };
    if claim & COUNTED == 0 {
        return;
    }
    let Some(trace) = (unsafe { TRACE.load(Ordering::Acquire).as_ref() }) else {
        return;
    };
    let index = claim & MASK;
    if index != 0 {
        let row = &trace.rows[index as usize - 1];
        if let Some(rax) = rax {
            row.rax.store(rax, Ordering::Relaxed);
            row.state.store(2, Ordering::Release);
            trace.completed.fetch_add(1, Ordering::Relaxed);
        } else {
            row.state.store(3, Ordering::Release);
            trace.unwound.fetch_add(1, Ordering::Relaxed);
        }
    }
    trace.flight.fetch_sub(1, Ordering::Release);
}
unsafe extern "system-unwind" fn end(token: *mut u64, rax: u64) {
    unsafe { settle(token, Some(rax)) };
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

// The eight callee gateways preserve the same three-argument Win64 ABI.
// The original target comes from a one-way published cell, never the vtable
// being replaced, so the original executes once even during uninstallation.
macro_rules! gateway {
    ($name:literal,$index:literal) => {global_asm!(
        concat!(".globl ",$name),concat!(".seh_proc ",$name),concat!($name,":"),
        ".seh_handler {unwind}, @unwind","sub rsp,0xf8",".seh_stackalloc 0xf8",
        "mov qword ptr [rsp+0x68],0",".seh_endprologue",
        "mov [rsp+0xe0],rcx","mov [rsp+0xe8],rdx","mov [rsp+0x40],r8","mov [rsp+0x48],r9",
        "mov [rsp+0x50],rax","mov [rsp+0x58],r10","mov [rsp+0x60],r11",
        "movdqu [rsp+0x80],xmm0","movdqu [rsp+0x90],xmm1","movdqu [rsp+0xa0],xmm2",
        "movdqu [rsp+0xb0],xmm3","movdqu [rsp+0xc0],xmm4","movdqu [rsp+0xd0],xmm5",
        concat!("mov r9d,",$index),"mov [rsp+0x30],rbp","mov [rsp+0x38],rdi",
        "mov rax,[rsp+0xf8]","mov [rsp+0x20],rax","lea rax,[rsp+0x68]","mov [rsp+0x28],rax","call {begin}",
        "mov rcx,[rsp+0xe0]","mov rdx,[rsp+0xe8]","mov r8,[rsp+0x40]","mov r9,[rsp+0x48]",
        "mov rax,[rsp+0x50]","mov r10,[rsp+0x58]","mov r11,[rsp+0x60]",
        "movdqu xmm0,[rsp+0x80]","movdqu xmm1,[rsp+0x90]","movdqu xmm2,[rsp+0xa0]",
        "movdqu xmm3,[rsp+0xb0]","movdqu xmm4,[rsp+0xc0]","movdqu xmm5,[rsp+0xd0]",
        concat!("call qword ptr [rip+{originals}+",$index,"*8]"),
        "mov [rsp+0x70],rax","movdqu [rsp+0x80],xmm0","lea rcx,[rsp+0x68]","mov rdx,rax","call {end}",
        "mov rax,[rsp+0x70]","movdqu xmm0,[rsp+0x80]","add rsp,0xf8","ret",".seh_endproc",
        begin=sym begin,end=sym end,unwind=sym unwind,originals=sym ORIGINALS
    );};
}
gateway!("recipient_military_apply", "0");
gateway!("recipient_military_remove", "1");
gateway!("recipient_naval_apply", "2");
gateway!("recipient_naval_remove", "3");
gateway!("recipient_build_apply", "4");
gateway!("recipient_build_remove", "5");
gateway!("recipient_growth_apply", "6");
gateway!("recipient_growth_remove", "7");
unsafe extern "system" {
    fn recipient_military_apply();
    fn recipient_military_remove();
    fn recipient_naval_apply();
    fn recipient_naval_remove();
    fn recipient_build_apply();
    fn recipient_build_remove();
    fn recipient_growth_apply();
    fn recipient_growth_remove();
}
fn gateway_address(index: usize) -> u64 {
    [
        recipient_military_apply as *const (),
        recipient_military_remove as *const (),
        recipient_naval_apply as *const (),
        recipient_naval_remove as *const (),
        recipient_build_apply as *const (),
        recipient_build_remove as *const (),
        recipient_growth_apply as *const (),
        recipient_growth_remove as *const (),
    ][index] as u64
}

/// # Safety
/// The caller owns the writable eight-byte slot and original three-argument
/// callee, pins both images, and never releases the trace or slot while a
/// gateway could run. This internal entry is for synthetic validation only;
/// it is not reachable from runtime exports or normal product startup.
#[allow(dead_code)]
pub(crate) unsafe fn install_at(
    index: usize,
    address: usize,
    original: u64,
    trace: &'static RecipientTrace,
) -> Result<(), &'static str> {
    if index >= 8 || original == 0 || !committed_range(address, 8, 8, false) {
        return Err("invalid recipient slot");
    }
    let mut bindings = BINDINGS.lock().map_err(|_| "recipient bindings poisoned")?;
    if bindings[index].is_some() {
        return Err("recipient binding frozen; restart process");
    }
    let trace_ptr = trace as *const RecipientTrace as *mut RecipientTrace;
    let current = TRACE.load(Ordering::Acquire);
    if !current.is_null() && current != trace_ptr {
        return Err("recipient trace frozen");
    }
    let slot = unsafe { &*(address as *const AtomicU64) };
    if slot.load(Ordering::Acquire) != original {
        return Err("recipient original slot differs");
    }
    let mut protection = 0;
    if unsafe { VirtualProtect(address as *const c_void, 8, PAGE_READWRITE, &mut protection) } == 0
    {
        return Err("recipient protect failed");
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
        return Err("recipient publication lost; binding retained");
    }
    if !restored {
        return Err("recipient protection unresolved; binding retained");
    }
    Ok(())
}
#[allow(dead_code)]
pub(crate) unsafe fn uninstall() -> Result<(), &'static str> {
    let mut bindings = BINDINGS.lock().map_err(|_| "recipient bindings poisoned")?;
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
        Err("recipient uninstallation unresolved; bindings retained")
    } else {
        Ok(())
    }
}

pub fn pinned_slot(index: usize, base: u64) -> Option<(u64, u64)> {
    let table = *([0xa76a30u64, 0xa6cdb8, 0xa90328, 0xa9e0f0].get(index / 2)?);
    let call = if index.is_multiple_of(2) {
        OathCall::Apply
    } else {
        OathCall::Remove
    };
    let family = OathEffectFamily::from_vtable_rva(table)?;
    Some((
        base.checked_add(table + call.vtable_slot())?,
        base.checked_add(family.callee_rva(call))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    static CALLS: AtomicU32 = AtomicU32::new(0);
    unsafe extern "system-unwind" fn original(effect: u64, city: u64, r8: u64) -> u64 {
        CALLS.fetch_add(1, Ordering::SeqCst);
        effect ^ city ^ r8
    }
    unsafe extern "system-unwind" fn throwing(_: u64, _: u64, _: u64) -> u64 {
        panic!("synthetic callee unwind");
    }
    #[test]
    fn recipient_gateway_child() {
        let Some(case) = std::env::var("CC_RECIPIENT_CHILD").ok() else {
            return;
        };
        let trace = Box::leak(RecipientTrace::boxed());
        let index = std::env::var("CC_RECIPIENT_INDEX")
            .unwrap()
            .parse::<usize>()
            .unwrap();
        let target = if case == "unwind" {
            throwing as *const () as u64
        } else {
            original as *const () as u64
        };
        let slot = Box::leak(Box::new(AtomicU64::new(target)));
        unsafe { install_at(index, slot as *const _ as usize, target, trace).unwrap() };
        trace.start().unwrap();
        let f: unsafe extern "system-unwind" fn(u64, u64, u64) -> u64 =
            unsafe { std::mem::transmute(slot.load(Ordering::Acquire)) };
        if case == "unwind" {
            assert!(std::panic::catch_unwind(|| unsafe { f(7, 0x12340000, 91) }).is_err());
        } else {
            assert_eq!(unsafe { f(7, 0x12340000, 91) }, 7 ^ 0x12340000 ^ 91);
        }
        trace.stop();
        let records = trace.records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].actual_receiver_argument, 0x12340000);
        assert_eq!(records[0].raw_r8, 91);
        assert_eq!(records[0].slot, index as u32);
        assert_eq!(
            trace.counts(),
            if case == "unwind" {
                (true, 0, 0, 1, 0)
            } else {
                (true, 0, 1, 0, 0)
            }
        );
        unsafe { uninstall().unwrap() };
        assert_eq!(slot.load(Ordering::Acquire), target);
        assert!(unsafe { install_at(index, slot as *const _ as usize, target, trace) }.is_err());
        if case != "unwind" {
            assert_eq!(CALLS.load(Ordering::Acquire), 1);
        }
    }
    #[test]
    fn recipient_gateway_completion_and_unwind_run_in_owned_children() {
        for (index, case) in (0..8).map(|i| (i, "normal")).chain([(0, "unwind")]) {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "recipient_trace::tests::recipient_gateway_child",
                    "--nocapture",
                ])
                .env("CC_RECIPIENT_CHILD", case)
                .env("CC_RECIPIENT_INDEX", index.to_string())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    #[test]
    fn each_pinned_slot_keeps_its_original_family_and_operation() {
        for i in 0..8 {
            let (slot, target) = pinned_slot(i, 0x180000000).unwrap();
            assert!(slot > 0x180000000 && target > 0x180000000);
        }
        assert!(pinned_slot(8, 0).is_none());
        assert!(pinned_slot(0, u64::MAX).is_none());
    }
}
