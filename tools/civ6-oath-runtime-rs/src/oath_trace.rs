//! Observer for the common Oath Apply/Remove wrappers (pinned GameCore 324c51e9…).
//! It records, per call, the entry registers (Effect* RCX, subject shared_ptr*
//! RDX, instance id R8D, caller), a bounded copy of the effect vtable and of
//! the subject shared_ptr, and - after the original returned - the full RAX.
//! The original is called exactly once with every register as received.
//!
//! This module is independent of the product payment authority: nothing here
//! reads or writes persona reservation, payment, rate or AI state, enters Lua,
//! or changes a quantity. Not wired to any guarded Install or BridgeMode yet;
//! the synthetic installer below exists to prove the Win64 ABI contract.
//!
//! Hot patch: both wrappers start with "push rbx; sub rsp,30h; movups
//! xmm0,[rip+disp]" and are preceded by INT3 padding. Only "push rbx" (two
//! bytes) is replaced by a short jump into a five byte jump placed in that
//! padding, so no RIP-relative byte is ever moved and a thread already past
//! the first instruction keeps executing unchanged bytes.
//!
//! Ownership. The gateway frame owns one token slot (frame+0x68) that is zeroed
//! inside the unwind prologue. oath_begin publishes the in-flight claim in that
//! slot the instruction after taking it; oath_end consumes (zeroes) it before
//! settling; the unwind handler settles whatever the slot still holds and
//! zeroes it. Every claim is therefore settled exactly once, at any stage.
//!
//! Install is one state machine whose receipt is stored before the first byte
//! of game memory is written (see Stage). Code, unwind tables and the trace stay
//! resident for the process lifetime once anything could have reached them.
use crate::native::*;
use crate::oath_record::*;
use windows_sys::Win32::System::Diagnostics::Debug::{RtlAddFunctionTable, RtlLookupFunctionEntry};

pub const OATH_MAGIC: u32 = 0x4a41_434f; // JACO
/// 2: rows carry the caller's raw RBP and RDI (stack arguments 7 and 8).
pub const OATH_VERSION: u32 = 2;
const COUNTED: u64 = 1 << 63;
const SLOT_MASK: u64 = 0xffff_ffff;
const CALL_SHIFT: u32 = 48;

#[repr(C)]
#[derive(Default)]
pub struct OathRow {
    /// 0 reserved, 1 begun, 2 completed, 3 unwound (see OathState).
    state: AtomicU32,
    call: AtomicU32,
    thread_id: AtomicU32,
    instance_id: AtomicU32,
    copy_flags: AtomicU32,
    completion_order: AtomicU32,
    raw_r8: AtomicU64,
    effect: AtomicU64,
    subject: AtomicU64,
    caller: AtomicU64,
    caller_rbp: AtomicU64,
    caller_rdi: AtomicU64,
    effect_vtable: AtomicU64,
    subject_object: AtomicU64,
    subject_control: AtomicU64,
    original_rax: AtomicU64,
}

/// Finite observation buffer. Never wraps; calls beyond capacity only count.
#[repr(C)]
pub struct OathTrace {
    magic: u32,
    version: u32,
    next: AtomicU32,
    dropped: AtomicU32,
    in_flight: AtomicU32,
    completed: AtomicU32,
    stopped: AtomicU32,
    reserved: u32,
    rows: [OathRow; OATH_CAPACITY],
}

impl OathTrace {
    pub fn boxed() -> Box<Self> {
        let mut trace = Box::<Self>::new_uninit();
        unsafe {
            Self::initialize_at(trace.as_mut_ptr());
            trace.assume_init()
        }
    }

    /// # Safety
    /// `target` is aligned writable storage not yet shared with a gateway.
    pub(super) unsafe fn initialize_at(target: *mut Self) {
        unsafe {
            ptr::addr_of_mut!((*target).magic).write(OATH_MAGIC);
            ptr::addr_of_mut!((*target).version).write(OATH_VERSION);
            ptr::addr_of_mut!((*target).next).write(AtomicU32::new(0));
            ptr::addr_of_mut!((*target).dropped).write(AtomicU32::new(0));
            ptr::addr_of_mut!((*target).in_flight).write(AtomicU32::new(0));
            ptr::addr_of_mut!((*target).completed).write(AtomicU32::new(0));
            ptr::addr_of_mut!((*target).stopped).write(AtomicU32::new(0));
            ptr::addr_of_mut!((*target).reserved).write(0);
            let rows = ptr::addr_of_mut!((*target).rows).cast::<OathRow>();
            for n in 0..OATH_CAPACITY {
                rows.add(n).write(OathRow::default());
            }
        }
    }

    /// Stops recording. Gateways, relay, trampoline and this buffer stay
    /// resident; later calls only run the original.
    pub fn stop(&self) {
        self.stopped.store(1, Ordering::Release);
    }
    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire) != 0
    }
    /// Calls that entered the gateway while recording and have not returned
    /// or unwound.
    pub fn in_flight(&self) -> u32 {
        self.in_flight.load(Ordering::Acquire)
    }
    /// Calls that found the buffer full. They ran the original unobserved.
    pub fn dropped(&self) -> u32 {
        self.dropped.load(Ordering::Acquire)
    }

    pub fn records(&self) -> Result<Vec<OathRecord>, &'static str> {
        if self.magic != OATH_MAGIC || self.version != OATH_VERSION {
            return Err("oath mapping version mismatch");
        }
        let next = (self.next.load(Ordering::Acquire) as usize).min(OATH_CAPACITY);
        let mut out = Vec::with_capacity(next);
        for (slot, row) in self.rows[..next].iter().enumerate() {
            let state = OathState::from_u32(row.state.load(Ordering::Acquire))
                .ok_or("oath row reserved but not yet published")?;
            let call = OathCall::from_u32(row.call.load(Ordering::Relaxed))
                .ok_or("oath row has an unknown call")?;
            out.push(OathRecord {
                sequence: slot as u32 + 1,
                call,
                state,
                thread_id: row.thread_id.load(Ordering::Relaxed),
                instance_id: row.instance_id.load(Ordering::Relaxed),
                raw_r8: row.raw_r8.load(Ordering::Relaxed),
                effect: row.effect.load(Ordering::Relaxed),
                subject: row.subject.load(Ordering::Relaxed),
                caller: row.caller.load(Ordering::Relaxed),
                caller_rbp: row.caller_rbp.load(Ordering::Relaxed),
                caller_rdi: row.caller_rdi.load(Ordering::Relaxed),
                effect_vtable: row.effect_vtable.load(Ordering::Relaxed),
                subject_object: row.subject_object.load(Ordering::Relaxed),
                subject_control: row.subject_control.load(Ordering::Relaxed),
                copy_flags: row.copy_flags.load(Ordering::Relaxed),
                original_rax: row.original_rax.load(Ordering::Relaxed),
                completion_order: row.completion_order.load(Ordering::Relaxed),
            });
        }
        Ok(out)
    }
}

// Process-lifetime singleton: set by the first install and cleared only when
// that install is rolled back before anything could reach a gateway.
#[unsafe(no_mangle)]
static OATH_TRACE_PTR: AtomicPtr<OathTrace> = AtomicPtr::new(null_mut());
#[unsafe(no_mangle)]
static OATH_APPLY_TRAMPOLINE: AtomicPtr<u8> = AtomicPtr::new(null_mut());
#[unsafe(no_mangle)]
static OATH_REMOVE_TRAMPOLINE: AtomicPtr<u8> = AtomicPtr::new(null_mut());

fn oath_trace() -> Option<&'static OathTrace> {
    unsafe { OATH_TRACE_PTR.load(Ordering::Acquire).as_ref() }
}

#[cfg(test)]
pub(super) static OATH_STAGE: AtomicU32 = AtomicU32::new(0);
pub(super) const STAGE_BEGIN_COUNTED: u32 = 1;
pub(super) const STAGE_BEGIN_RESERVED: u32 = 2;
pub(super) const STAGE_BEGIN_PUBLISHED: u32 = 3;
pub(super) const STAGE_END_ENTERED: u32 = 4;
/// Test-only exception injection at a named callback stage.
#[inline(always)]
fn stage_point(_stage: u32) {
    #[cfg(test)]
    if OATH_STAGE
        .compare_exchange(_stage, 0, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        panic!("injected failure at oath callback stage {_stage}");
    }
}

/// Takes the in-flight claim and publishes it in the frame slot at once.
/// Slot value: 0 nothing claimed, else COUNTED | call<<48 | (row+1, 0 if the
/// buffer was full). An exception from here on is settled by the frame handler.
unsafe extern "system-unwind" fn oath_begin(
    effect: u64,
    subject: u64,
    raw_r8: u64,
    call: u64,
    caller: u64,
    token_slot: *mut u64,
    caller_rbp: u64,
    caller_rdi: u64,
) {
    let Some(trace) = oath_trace() else {
        return;
    };
    // A stopped trace costs one load. Otherwise count first and re-check, so
    // a call that passed the check before stop() is counted until it returns
    // or unwinds; only a call racing the stop itself is transient.
    if trace.stopped.load(Ordering::Acquire) != 0 {
        return;
    }
    trace.in_flight.fetch_add(1, Ordering::AcqRel);
    let claim = COUNTED | (call << CALL_SHIFT);
    unsafe { ptr::write_volatile(token_slot, claim) };
    stage_point(STAGE_BEGIN_COUNTED);
    if trace.stopped.load(Ordering::Acquire) != 0 {
        unsafe { ptr::write_volatile(token_slot, 0) };
        trace.in_flight.fetch_sub(1, Ordering::Release);
        return;
    }
    let Ok(slot) = trace
        .next
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n < OATH_CAPACITY as u32).then_some(n + 1)
        })
    else {
        trace.dropped.fetch_add(1, Ordering::Relaxed);
        return;
    };
    unsafe { ptr::write_volatile(token_slot, claim | (u64::from(slot) + 1)) };
    stage_point(STAGE_BEGIN_RESERVED);
    let row = &trace.rows[slot as usize];
    let mut flags = 0;
    if let Some(vtable) = copy_current::<8>(effect).map(u64::from_le_bytes) {
        row.effect_vtable.store(vtable, Ordering::Relaxed);
        flags |= COPY_EFFECT_VTABLE;
    }
    if let Some(raw) = copy_current::<16>(subject) {
        let object = u64::from_le_bytes(raw[..8].try_into().unwrap());
        row.subject_object.store(object, Ordering::Relaxed);
        row.subject_control.store(
            u64::from_le_bytes(raw[8..].try_into().unwrap()),
            Ordering::Relaxed,
        );
        flags |= COPY_SUBJECT_POINTER;
        if object != 0 {
            flags |= COPY_SUBJECT_OBJECT_PRESENT;
        }
    }
    row.call.store(call as u32, Ordering::Relaxed);
    row.thread_id
        .store(unsafe { GetCurrentThreadId() }, Ordering::Relaxed);
    row.instance_id.store(raw_r8 as u32, Ordering::Relaxed);
    row.raw_r8.store(raw_r8, Ordering::Relaxed);
    row.effect.store(effect, Ordering::Relaxed);
    row.subject.store(subject, Ordering::Relaxed);
    row.caller.store(caller, Ordering::Relaxed);
    // Raw observations of the caller's nonvolatile registers, never authority.
    row.caller_rbp.store(caller_rbp, Ordering::Relaxed);
    row.caller_rdi.store(caller_rdi, Ordering::Relaxed);
    row.copy_flags.store(flags, Ordering::Relaxed);
    row.state.store(1, Ordering::Release);
    stage_point(STAGE_BEGIN_PUBLISHED);
}

/// Post-return record. It consumes the frame claim first, then only stores
/// what the original returned; there is no fallible step between the two.
unsafe extern "system-unwind" fn oath_end(token_slot: *mut u64, original_rax: u64) {
    stage_point(STAGE_END_ENTERED);
    let token = unsafe { ptr::read_volatile(token_slot) };
    unsafe { ptr::write_volatile(token_slot, 0) };
    if token == 0 {
        return;
    }
    let Some(trace) = oath_trace() else {
        return;
    };
    let slot = token & SLOT_MASK;
    if slot > 0 && slot <= OATH_CAPACITY as u64 {
        let row = &trace.rows[slot as usize - 1];
        row.original_rax.store(original_rax, Ordering::Relaxed);
        row.completion_order.store(
            trace.completed.fetch_add(1, Ordering::AcqRel) + 1,
            Ordering::Relaxed,
        );
        row.state.store(2, Ordering::Release);
    }
    trace.in_flight.fetch_sub(1, Ordering::Release);
}

fn oath_unwound(token: u64) {
    let Some(trace) = oath_trace() else {
        return;
    };
    let slot = token & SLOT_MASK;
    if slot > 0 && slot <= OATH_CAPACITY as u64 {
        let row = &trace.rows[slot as usize - 1];
        // A row reserved but not yet published is closed here too.
        row.call
            .store(((token >> CALL_SHIFT) & 0xff) as u32, Ordering::Relaxed);
        if row
            .state
            .compare_exchange(1, 3, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            let _ = row
                .state
                .compare_exchange(0, 3, Ordering::AcqRel, Ordering::Acquire);
        }
    }
    trace.in_flight.fetch_sub(1, Ordering::Release);
}

/// Language handler of the gateway frames (UNW_FLAG_UHANDLER only). It settles
/// the claim still held in the frame slot, if any, and zeroes the slot.
unsafe extern "system" fn oath_unwind_handler(
    record: *const u8,
    frame: u64,
    _context: *mut u8,
    _dispatcher: *mut u8,
) -> i32 {
    const EXCEPTION_UNWINDING_OR_EXIT: u32 = 2 | 4;
    let flags = unsafe { ptr::read(record.add(4).cast::<u32>()) };
    if flags & EXCEPTION_UNWINDING_OR_EXIT != 0 {
        let token_slot = (frame + 0x68) as *mut u64;
        let token = unsafe { ptr::read_volatile(token_slot) };
        if token != 0 {
            unsafe { ptr::write_volatile(token_slot, 0) };
            oath_unwound(token);
        }
    }
    1 // ExceptionContinueSearch
}

// Frame (0xf8, RSP 16-byte aligned at every call):
//   00 shadow  20 arg5 (caller)  28 arg6 (&token)  30 arg7 (RBP)  38 arg8 (RDI)
//   40 r8 48 r9 50 rax 58 r10 60 r11  e0 rcx  e8 rdx
//   68 token (zeroed inside the unwind prologue)  70 original RAX
//   78 resume address (original entry+2) for the trampoline's jmp [rsp+88h]
//   80 xmm0..xmm5 on entry, later the returned xmm0  f8 caller's return address
// Nonvolatile registers and XMM6+ are never written; RBP and RDI are only
// passed by value to the begin callback. The original receives every
// argument/scratch register as it came in and is called exactly once.
macro_rules! oath_gateway {
    ($name:literal, $trampoline:path, $kind:literal) => {
        global_asm!(
            concat!(".globl ", $name),
            concat!(".seh_proc ", $name),
            concat!($name, ":"),
            ".seh_handler {handler}, @unwind",
            "sub rsp, 0xf8",
            ".seh_stackalloc 0xf8",
            "mov qword ptr [rsp+0x68], 0",
            ".seh_endprologue",
            "mov [rsp+0xe0], rcx",
            "mov [rsp+0xe8], rdx",
            "mov [rsp+0x40], r8",
            "mov [rsp+0x48], r9",
            "mov [rsp+0x50], rax",
            "mov [rsp+0x58], r10",
            "mov [rsp+0x60], r11",
            "movdqu [rsp+0x80], xmm0",
            "movdqu [rsp+0x90], xmm1",
            "movdqu [rsp+0xa0], xmm2",
            "movdqu [rsp+0xb0], xmm3",
            "movdqu [rsp+0xc0], xmm4",
            "movdqu [rsp+0xd0], xmm5",
            concat!("mov r9d, ", $kind),
            "mov [rsp+0x30], rbp",
            "mov [rsp+0x38], rdi",
            "mov rax, [rsp+0xf8]",
            "mov [rsp+0x20], rax",
            "lea rax, [rsp+0x68]",
            "mov [rsp+0x28], rax",
            "call {begin}",
            "mov rax, [rip+{trampoline}]",
            "mov rax, [rax+{delta}]",
            "mov [rsp+0x78], rax",
            "mov rcx, [rsp+0xe0]",
            "mov rdx, [rsp+0xe8]",
            "mov r8, [rsp+0x40]",
            "mov r9, [rsp+0x48]",
            "mov rax, [rsp+0x50]",
            "mov r10, [rsp+0x58]",
            "mov r11, [rsp+0x60]",
            "movdqu xmm0, [rsp+0x80]",
            "movdqu xmm1, [rsp+0x90]",
            "movdqu xmm2, [rsp+0xa0]",
            "movdqu xmm3, [rsp+0xb0]",
            "movdqu xmm4, [rsp+0xc0]",
            "movdqu xmm5, [rsp+0xd0]",
            "call qword ptr [rip+{trampoline}]",
            "mov [rsp+0x70], rax",
            "movdqu [rsp+0x80], xmm0",
            "lea rcx, [rsp+0x68]",
            "mov rdx, rax",
            "call {end}",
            "mov rax, [rsp+0x70]",
            "movdqu xmm0, [rsp+0x80]",
            "add rsp, 0xf8",
            "ret",
            ".seh_endproc",
            begin = sym oath_begin,
            end = sym oath_end,
            handler = sym oath_unwind_handler,
            trampoline = sym $trampoline,
            delta = const TRAMPOLINE_RESUME_DELTA,
        );
    };
}
oath_gateway!("oath_apply_gateway", OATH_APPLY_TRAMPOLINE, "1");
oath_gateway!("oath_remove_gateway", OATH_REMOVE_TRAMPOLINE, "2");
unsafe extern "system" {
    pub(super) fn oath_apply_gateway();
    pub(super) fn oath_remove_gateway();
}

// ---- install state machine -------------------------------------------------

use crate::oath_install_state::{trace_needed, Bytes, Cas, Confirm, Ledger, Reach, Region, Write};

/// Pointers of one installed call type plus the portable ledger. The ledger
/// (see oath_install_state) alone decides what may be undone or released.
pub(super) struct Receipt {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) code: *mut u8,
    target: *mut u8,
    slot: *mut u8,
    original: [u8; 8],
    entry_patched: [u8; 8],
    prefix_jump: [u8; OATH_PREFIX_PAD.len()],
    /// Protection of the page before the first write; every restore returns to it.
    page_protection: u32,
    prior_trampoline: *mut u8,
    pub(super) ledger: Ledger,
    /// A frozen, settled receipt stays as a tombstone; this reports it once.
    reported: bool,
}
unsafe impl Send for Receipt {}

static OATH_HOOKS: Mutex<[Option<Receipt>; 2]> = Mutex::new([None, None]);

fn trampoline_cell(call: usize) -> &'static AtomicPtr<u8> {
    if call == 0 {
        &OATH_APPLY_TRAMPOLINE
    } else {
        &OATH_REMOVE_TRAMPOLINE
    }
}

// Fault sites, armed one bit at a time by tests; always false otherwise.
const SITE_NONE: u32 = 0;
const SITE_TABLE_ADD: u32 = 1;
const SITE_CODE_PROTECT: u32 = 2;
const SITE_PREFIX_ENABLE: u32 = 3;
const SITE_PREFIX_RESTORE: u32 = 4;
const SITE_PREFIX_FLUSH: u32 = 5;
const SITE_ENTRY_ENABLE: u32 = 6;
const SITE_ENTRY_RACE: u32 = 7;
const SITE_ENTRY_RESTORE: u32 = 8;
const SITE_ENTRY_FLUSH: u32 = 9;
#[cfg(test)]
const SITE_PREFIX_RACE: u32 = 10;
const SITE_UNINSTALL_ENABLE: u32 = 13;
const SITE_UNINSTALL_RESTORE: u32 = 14;
const SITE_UNINSTALL_FLUSH: u32 = 15;
const SITE_REPAIR_PREFIX_RESTORE: u32 = 16;
const SITE_REPAIR_PREFIX_FLUSH: u32 = 17;
const SITE_REPAIR_ENTRY_RESTORE: u32 = 18;
const SITE_REPAIR_ENTRY_FLUSH: u32 = 19;

#[cfg(test)]
pub(super) static OATH_FAULTS: AtomicU64 = AtomicU64::new(0);
/// Sites that keep failing until the test clears the mask.
#[cfg(test)]
pub(super) static OATH_FAULTS_PERSIST: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
fn fault(site: u32) -> bool {
    let bit = 1u64 << site;
    site != SITE_NONE
        && (OATH_FAULTS_PERSIST.load(Ordering::SeqCst) & bit != 0
            || OATH_FAULTS.fetch_and(!bit, Ordering::SeqCst) & bit != 0)
}
#[cfg(not(test))]
fn fault(_site: u32) -> bool {
    false
}

unsafe fn protect(site: u32, address: *mut u8, len: usize, new: u32) -> Option<u32> {
    if fault(site) {
        return None;
    }
    let mut old = 0u32;
    (unsafe { VirtualProtect(address.cast(), len, new, &mut old) } != 0).then_some(old)
}

unsafe fn flush(site: u32, address: *mut u8, len: usize) -> bool {
    !fault(site) && unsafe { FlushInstructionCache(GetCurrentProcess(), address.cast(), len) } != 0
}

fn page_protection_of(address: *mut u8) -> Option<u32> {
    let mut info = unsafe { std::mem::zeroed::<MEMORY_BASIC_INFORMATION>() };
    let size = std::mem::size_of::<MEMORY_BASIC_INFORMATION>();
    (unsafe { VirtualQuery(address.cast(), &mut info, size) } == size).then_some(info.Protect)
}

/// Puts the page protection back and proves it by asking the OS afterwards.
unsafe fn protect_back(site: u32, address: *mut u8, len: usize, restore_to: u32) -> bool {
    unsafe { protect(site, address, len, restore_to) }.is_some()
        && page_protection_of(address) == Some(restore_to)
}

/// Re-verifies a region's protection (repairing it if needed) and flushes it.
unsafe fn confirm_region(
    sites: [u32; 2],
    address: *mut u8,
    len: usize,
    restore_to: u32,
) -> Confirm {
    Confirm {
        protection: page_protection_of(address) == Some(restore_to)
            || unsafe { protect_back(sites[0], address, len, restore_to) },
        flush: unsafe { flush(sites[1], address, len) },
    }
}

/// Replaces the five prefix bytes only if they are still the expected ones.
/// The ledger records the attempt before and the outcome after.
unsafe fn write_prefix(
    ledger: &mut Ledger,
    sites: [u32; 3],
    restore_to: u32,
    at: *mut u8,
    expected: [u8; 5],
    new: [u8; 5],
    target: Bytes,
) -> Write {
    let previous = ledger.prefix_begin();
    let writable = unsafe { protect(sites[0], at, 5, PAGE_EXECUTE_READWRITE) }.is_some();
    // Test only: another writer changes the padding after the checks.
    #[cfg(test)]
    if writable && fault(SITE_PREFIX_RACE) {
        unsafe { ptr::write(at, 0x90) };
    }
    let result = if !writable {
        Write::Refused
    } else if unsafe { ptr::read(at.cast::<[u8; 5]>()) } != expected {
        let restored = unsafe { protect_back(sites[1], at, 5, restore_to) };
        Write::Mismatch { restored }
    } else {
        // From the first copied byte the prefix may be ours: latch the reach.
        ledger.prefix_copy_begin();
        unsafe { ptr::copy_nonoverlapping(new.as_ptr(), at, 5) };
        let restored = unsafe { protect_back(sites[1], at, 5, restore_to) };
        let flushed = unsafe { flush(sites[2], at, 5) };
        let verified = unsafe { ptr::read(at.cast::<[u8; 5]>()) } == new;
        Write::Written {
            restored,
            flushed,
            verified,
        }
    };
    ledger.prefix_end(previous, target, result);
    result
}

/// Two byte compare-exchange on the entry. The ledger learns that the entry
/// may reach the prefix before the exchange runs, never after.
unsafe fn exchange_entry2(
    ledger: &mut Ledger,
    sites: [u32; 4],
    restore_to: u32,
    entry: *mut u8,
    expected: [u8; 2],
    replacement: [u8; 2],
    target: Bytes,
) -> Cas {
    if unsafe { protect(sites[0], entry, 2, PAGE_EXECUTE_READWRITE) }.is_none() {
        return Cas::Untouched;
    }
    let previous = ledger.entry_begin();
    #[cfg(test)]
    if fault(sites[1]) {
        unsafe { ptr::write(entry.cast::<[u8; 2]>(), [0x90, 0x90]) };
    }
    let exchanged = unsafe { &*entry.cast::<AtomicU16>() }
        .compare_exchange(
            u16::from_le_bytes(expected),
            u16::from_le_bytes(replacement),
            Ordering::SeqCst,
            Ordering::SeqCst,
        )
        .is_ok();
    let restored = unsafe { protect_back(sites[2], entry, 2, restore_to) };
    let result = if exchanged {
        let flushed = unsafe { flush(sites[3], entry, 2) };
        let verified = unsafe { ptr::read(entry.cast::<[u8; 2]>()) } == replacement;
        Cas::Done {
            restored,
            flushed,
            verified,
        }
    } else {
        Cas::Lost { restored }
    };
    ledger.entry_end(previous, target, result);
    result
}

// Relay, trampoline and their unwind data share one resident page.
const UNWIND_TABLE_OFFSET: usize = 0x80;
const RELAY_UNWIND_OFFSET: usize = 0xa0;
const TRAMPOLINE_UNWIND_OFFSET: usize = 0xa8;
const RELAY_LEN: usize = 14;
// push rbx (copied), then jmp qword ptr [rsp+88h]. The target (original
// entry+2) is stored by the gateway in its frame slot 78h, which sits at
// rsp+88h once the call and the push are on the stack. Windows reads a jump
// through RIP-relative memory or to an outside rel32 as a tail-call epilogue
// and unwinds it as a bare RET, losing the pushed RBX; a stack-relative jump
// is not an epilogue, so every RIP in the trampoline unwinds correctly.
const TRAMPOLINE_LEN: usize = OATH_RELOCATED_LEN + 7;
/// The resume address is stored next to the trampoline; the gateway reads it
/// through the trampoline pointer, so both always belong together.
const TRAMPOLINE_RESUME_DELTA: usize = 0x10;

/// Writes relay, trampoline and unwind data. The trampoline's one pushed
/// register (rbx, offset 2) is described, so RIP at the trampoline or at its
/// jump back unwinds to the real return address with the right RBX/RSP; the
/// relay is a described leaf.
unsafe fn write_code_page(
    code: *mut u8,
    gateway: usize,
    first: [u8; OATH_RELOCATED_LEN],
    back_to: usize,
) {
    let words: [u32; 6] = [
        RELAY_OFFSET as u32,
        (RELAY_OFFSET + RELAY_LEN) as u32,
        RELAY_UNWIND_OFFSET as u32,
        TRAMPOLINE_OFFSET as u32,
        (TRAMPOLINE_OFFSET + TRAMPOLINE_LEN) as u32,
        TRAMPOLINE_UNWIND_OFFSET as u32,
    ];
    unsafe {
        ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), code.add(RELAY_OFFSET), 6);
        ptr::copy_nonoverlapping(
            (gateway as u64).to_le_bytes().as_ptr(),
            code.add(RELAY_OFFSET + 6),
            8,
        );
        ptr::copy_nonoverlapping(first.as_ptr(), code.add(TRAMPOLINE_OFFSET), first.len());
        ptr::copy_nonoverlapping(
            [0xff, 0xa4, 0x24, 0x88, 0, 0, 0].as_ptr(),
            code.add(TRAMPOLINE_OFFSET + first.len()),
            7,
        );
        ptr::copy_nonoverlapping(
            (back_to as u64).to_le_bytes().as_ptr(),
            code.add(TRAMPOLINE_OFFSET + TRAMPOLINE_RESUME_DELTA),
            8,
        );
        for (n, word) in words.iter().enumerate() {
            ptr::copy_nonoverlapping(
                word.to_le_bytes().as_ptr(),
                code.add(UNWIND_TABLE_OFFSET + 4 * n),
                4,
            );
        }
        // UNWIND_INFO: version 1, no flags. Leaf relay: no prolog, no codes.
        ptr::copy_nonoverlapping([1u8, 0, 0, 0].as_ptr(), code.add(RELAY_UNWIND_OFFSET), 4);
        // Trampoline: prolog of 2 bytes, UWOP_PUSH_NONVOL rbx at offset 2.
        ptr::copy_nonoverlapping(
            [
                1u8,
                OATH_RELOCATED_LEN as u8,
                1,
                0,
                OATH_RELOCATED_LEN as u8,
                0x30,
                0,
                0,
            ]
            .as_ptr(),
            code.add(TRAMPOLINE_UNWIND_OFFSET),
            8,
        );
    }
}

fn release_reservation(hooks: &mut [Option<Receipt>; 2], index: usize) {
    if let Some(receipt) = hooks[index].take() {
        trampoline_cell(index).store(receipt.prior_trampoline, Ordering::Release);
    }
    // The trace is shared by both call types: it goes back with the last
    // binding, whichever install claimed it. A frozen receipt is a binding.
    if !trace_needed(hooks.iter().flatten().map(|receipt| &receipt.ledger)) {
        OATH_TRACE_PTR.store(null_mut(), Ordering::Release);
    }
}

/// Returns the reservations only when the ledger proves nothing could have
/// reached the prefix and every byte, protection and flush is original.
fn conclude(hooks: &mut [Option<Receipt>; 2], index: usize) -> bool {
    let released = hooks[index].as_mut().is_some_and(|r| r.ledger.release());
    if released {
        release_reservation(hooks, index);
    }
    released
}

/// Reads both regions and tells the ledger what is really there.
unsafe fn observe(receipt: &mut Receipt) {
    let prefix = match unsafe { ptr::read(receipt.slot.cast::<[u8; 5]>()) } {
        bytes if bytes == OATH_PREFIX_PAD => Bytes::Original,
        bytes if bytes == receipt.prefix_jump => Bytes::Ours,
        _ => Bytes::Foreign,
    };
    let entry = match unsafe { ptr::read(receipt.target.cast::<[u8; 8]>()) } {
        bytes if bytes == receipt.original => Bytes::Original,
        bytes if bytes == receipt.entry_patched => Bytes::Ours,
        _ => Bytes::Foreign,
    };
    receipt.ledger.observe(prefix, entry);
}

/// The single recovery routine for install failures and uninstall: observe,
/// restore our entry, put the padding back while the ledger allows it, then
/// re-verify protection and flush of both regions. Each step asks the ledger.
unsafe fn settle(receipt: &mut Receipt) {
    unsafe { observe(receipt) };
    if receipt.ledger.may_restore_entry() {
        let first: [u8; OATH_RELOCATED_LEN] = receipt.original[..OATH_RELOCATED_LEN]
            .try_into()
            .unwrap_or_default();
        unsafe {
            exchange_entry2(
                &mut receipt.ledger,
                [
                    SITE_UNINSTALL_ENABLE,
                    SITE_NONE,
                    SITE_UNINSTALL_RESTORE,
                    SITE_UNINSTALL_FLUSH,
                ],
                receipt.page_protection,
                receipt.target,
                PROPERTY_SHORT_JUMP,
                first,
                Bytes::Original,
            );
            observe(receipt);
        }
    }
    if !receipt.ledger.prefix.confirm.complete() {
        let confirm = unsafe {
            confirm_region(
                [SITE_REPAIR_PREFIX_RESTORE, SITE_REPAIR_PREFIX_FLUSH],
                receipt.slot,
                OATH_PREFIX_PAD.len(),
                receipt.page_protection,
            )
        };
        receipt.ledger.prefix_repaired(confirm);
    }
    if !receipt.ledger.entry.confirm.complete() {
        let confirm = unsafe {
            confirm_region(
                [SITE_REPAIR_ENTRY_RESTORE, SITE_REPAIR_ENTRY_FLUSH],
                receipt.target,
                OATH_RELOCATED_LEN,
                receipt.page_protection,
            )
        };
        receipt.ledger.entry_repaired(confirm);
    }
}

/// Synthetic-target installer; not reachable from any BridgeMode. Nothing is
/// written to game memory until the receipt (and its ledger) is stored. While
/// the ledger says nothing could have reached the prefix, every failure
/// returns to the starting bytes, protection and flush and frees the
/// reservations; once it says otherwise everything stays for the process.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) unsafe fn install_oath_at(
    target: *mut u8,
    call: OathCall,
    trace: &'static OathTrace,
    expected: [u8; 8],
) -> Result<(), &'static str> {
    if target.is_null()
        || !(target as usize).is_multiple_of(2)
        || (target as usize & (CODE_PAGE - 1)) < OATH_PREFIX_PAD.len()
    {
        return Err("unaligned or missing oath hook input");
    }
    if trace.magic != OATH_MAGIC || trace.version != OATH_VERSION {
        return Err("oath mapping version mismatch");
    }
    let mut hooks = OATH_HOOKS.lock().map_err(|_| "bridge lock poisoned")?;
    let index = call as usize - 1;
    if let Some(existing) = hooks[index].as_ref() {
        return Err(if existing.ledger.frozen() {
            "oath wrapper is frozen to its first target; restart the process"
        } else {
            "oath wrapper already installed or unresolved"
        });
    }
    let trace_ptr = trace as *const OathTrace as *mut OathTrace;
    let current = OATH_TRACE_PTR.load(Ordering::Acquire);
    if !current.is_null() && current != trace_ptr {
        return Err("oath trace is a process-lifetime singleton");
    }
    let slot = unsafe { target.sub(OATH_PREFIX_PAD.len()) };
    if !committed_range(slot as usize, OATH_PREFIX_PAD.len() + 8, 1, false) {
        return Err("oath entry is not fully readable");
    }
    let original = unsafe { ptr::read(target.cast::<[u8; 8]>()) };
    let prefix = unsafe { ptr::read(slot.cast::<[u8; OATH_PREFIX_PAD.len()]>()) };
    if original != expected || !hotpatch_site_matches(prefix, original) {
        return Err(if prefix[0] == 0xe9 {
            "oath prefix already published; restart the process"
        } else {
            "pinned oath entry or five-byte prefix differs"
        });
    }
    // The jump is unwound as a leaf, so no function may describe the padding.
    let mut image_base = 0u64;
    if !unsafe { RtlLookupFunctionEntry(slot as u64, &mut image_base, null_mut()) }.is_null() {
        return Err("oath prefix lies inside a described function");
    }
    let page_protection = page_protection_of(slot).ok_or("cannot read oath page protection")?;

    let code =
        unsafe { executable_near(slot as usize) }.ok_or("no rel32 relay near oath prefix")?;
    let relay = unsafe { code.add(RELAY_OFFSET) };
    let trampoline = unsafe { code.add(TRAMPOLINE_OFFSET) };
    let prefix_jump =
        rel32_jump(slot as usize, relay as usize).ok_or("oath prefix relay out of range")?;
    let gateway = match call {
        OathCall::Apply => oath_apply_gateway as *const () as usize,
        OathCall::Remove => oath_remove_gateway as *const () as usize,
    };
    let first: [u8; OATH_RELOCATED_LEN] = original[..OATH_RELOCATED_LEN]
        .try_into()
        .map_err(|_| "oath entry width")?;
    unsafe { write_code_page(code, gateway, first, target as usize + OATH_RELOCATED_LEN) };
    // Nothing can reach the page yet, so a failure here may free it.
    if unsafe { protect(SITE_CODE_PROTECT, code, CODE_PAGE, PAGE_EXECUTE_READ) }.is_none()
        || unsafe { FlushInstructionCache(GetCurrentProcess(), code.cast(), CODE_PAGE) } == 0
    {
        unsafe { VirtualFree(code.cast(), 0, MEM_RELEASE) };
        return Err("cannot publish oath relay and trampoline");
    }
    let table = unsafe { code.add(UNWIND_TABLE_OFFSET) };
    if fault(SITE_TABLE_ADD) || !unsafe { RtlAddFunctionTable(table.cast(), 2, code as u64) } {
        unsafe { VirtualFree(code.cast(), 0, MEM_RELEASE) };
        return Err("cannot register oath unwind tables");
    }
    // The page and its table stay for the process lifetime from here on; the
    // reservations below are returned only by a verified Never release.
    let _ =
        OATH_TRACE_PTR.compare_exchange(null_mut(), trace_ptr, Ordering::AcqRel, Ordering::Acquire);
    let prior_trampoline = trampoline_cell(index).swap(trampoline, Ordering::AcqRel);
    let mut ledger = Ledger::new();
    ledger.bind();
    let mut patched = original;
    patched[..OATH_RELOCATED_LEN].copy_from_slice(&PROPERTY_SHORT_JUMP);
    hooks[index] = Some(Receipt {
        code,
        target,
        slot,
        original,
        entry_patched: patched,
        prefix_jump,
        page_protection,
        prior_trampoline,
        ledger,
        reported: false,
    });

    // The padding is never executed on a valid path, so it goes first. Its
    // first copied byte latches the reach: from then on a writer of the entry
    // could branch in, so the padding is never put back.
    {
        let receipt = hooks[index].as_mut().unwrap();
        unsafe {
            write_prefix(
                &mut receipt.ledger,
                [SITE_PREFIX_ENABLE, SITE_PREFIX_RESTORE, SITE_PREFIX_FLUSH],
                page_protection,
                slot,
                OATH_PREFIX_PAD,
                prefix_jump,
                Bytes::Ours,
            )
        };
        let published = Region {
            bytes: Bytes::Ours,
            confirm: Confirm::OK,
        };
        if receipt.ledger.prefix != published {
            unsafe { settle(receipt) };
            let latched = receipt.ledger.reach() == Reach::Possibly;
            return Err(if conclude(&mut hooks, index) {
                "oath prefix write refused; nothing was written"
            } else if latched {
                "critical: oath prefix publication unverified; binding frozen"
            } else {
                "critical: oath prefix write refused; reservations kept until confirmed"
            });
        }
    }

    let receipt = hooks[index].as_mut().unwrap();
    let result = unsafe {
        exchange_entry2(
            &mut receipt.ledger,
            [
                SITE_ENTRY_ENABLE,
                SITE_ENTRY_RACE,
                SITE_ENTRY_RESTORE,
                SITE_ENTRY_FLUSH,
            ],
            page_protection,
            target,
            first,
            PROPERTY_SHORT_JUMP,
            Bytes::Ours,
        )
    };
    match result {
        // The prefix is already ours and reachable: only repair, never recycle.
        Cas::Untouched => {
            unsafe { settle(receipt) };
            Err("critical: oath entry write refused; binding frozen")
        }
        // A foreign entry may already branch to the prefix: keep everything.
        Cas::Lost { .. } => Err("critical: oath entry changed after prefix publication"),
        Cas::Done { .. } => {
            let published = Region {
                bytes: Bytes::Ours,
                confirm: Confirm::OK,
            };
            if receipt.ledger.entry == published {
                Ok(())
            } else {
                Err("critical: oath entry publication unverified")
            }
        }
    }
}

/// Settles every receipt. A Never-reach install is undone completely and its
/// reservations returned; a reachable one is restored (entry) and kept as a
/// frozen tombstone, so its target, trampoline and trace never change. A
/// receipt with any byte, protection or flush unconfirmed is kept and the
/// call fails.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) unsafe fn uninstall_oath() -> Result<bool, &'static str> {
    let mut hooks = OATH_HOOKS.lock().map_err(|_| "bridge lock poisoned")?;
    let mut changed = false;
    let mut failure = None;
    for index in 0..2 {
        let Some(receipt) = hooks[index].as_mut() else {
            continue;
        };
        unsafe { settle(receipt) };
        if conclude(&mut hooks, index) {
            changed = true;
            continue;
        }
        let receipt = hooks[index].as_mut().unwrap();
        if receipt.ledger.frozen()
            && receipt.ledger.settled()
            && receipt.ledger.prefix.confirm.complete()
        {
            if !receipt.reported {
                receipt.reported = true;
                changed = true;
            }
        } else {
            failure = Some("oath install is not settled; receipt kept");
        }
    }
    failure.map_or(Ok(changed), Err)
}

#[cfg(test)]
#[path = "oath_trace_tests.rs"]
mod tests;
