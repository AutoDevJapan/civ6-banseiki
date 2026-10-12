//! Restriction of natural CanPromote results, owned by the ordinary runtime.
//! No AI actor, operation constructor, getter, Lua VM or mutation is invoked.
//! Only the two pinned aligned E8 operands are changed, under mode8's AMD rule.
use super::*;
use crate::governor_payment::{self, CurrentMemory, PromotionSubject};
use windows_sys::Win32::Foundation::{GetLastError, SetLastError};
use windows_sys::Win32::System::Diagnostics::Debug::{CONTEXT, EXCEPTION_RECORD};

pub(super) const AI_CALL_RVA: usize = 0x4a_7d67;
pub(super) const AI_WINDOW: [u8; 16] = [
    0xe8, 0x44, 0x5e, 0xe3, 0xff, 0x84, 0xc0, 0x74, 0x79, 0x48, 0x8b, 0x03, 0x48, 0x63, 0x48, 0x14,
];
const OFFICIAL_ARGS: [u8; 9] = [0x44, 0x8b, 0xc7, 0x48, 0x8b, 0xd0, 0x48, 0x8b, 0xce];
const AI_ARGS: [u8; 14] = [
    0x45, 0x8b, 0xc4, 0x48, 0x8b, 0xd0, 0x4c, 0x8b, 0x64, 0x24, 0x20, 0x49, 0x8b, 0xcc,
];
const CALLEE_BOUNDARY: [u8; 13] = [
    0xe9, 0xd0, 0xfc, 0xff, 0xff, 0x40, 0x53, 0x56, 0x57, 0x48, 0x83, 0xec, 0x30,
];

struct Memory;
impl CurrentMemory for Memory {
    fn copy<const N: usize>(&self, address: u64) -> Option<[u8; N]> {
        #[cfg(test)]
        tests::READ_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
        copy_current(address)
    }
}

// The first word is the sole generation lease, owned by this native frame.
// Numeric Core receivers and copied admission words never outlive the call.
#[repr(C)]
#[derive(Default)]
struct NativeInvocation {
    cookie: AtomicU64,
    governors: u64,
    governor: u64,
    requested: u32,
    readable: u32,
    identity: [u8; 8],
    owner: u32,
    reserved: u32,
    fence: runtime::CallFence,
    admission: [u128; 5],
}
const _: () = {
    assert!(std::mem::offset_of!(NativeInvocation, cookie) == 0);
    assert!(std::mem::size_of::<NativeInvocation>() == 0xa0);
};

struct LastError(u32);
impl Drop for LastError {
    fn drop(&mut self) {
        unsafe { SetLastError(self.0) };
    }
}

fn admission(fence: runtime::CallFence, index: usize) -> Option<[u128; 5]> {
    let owner = fence.owner as usize;
    if owner >= PRODUCTION_OWNER_LIMIT
        || index >= PRODUCTION_INDEX_LIMIT
        || product_permission(owner, index, fence.receiver, fence.session) != Some(true)
    {
        return None;
    }
    let offset = owner * PRODUCTION_INDEX_LIMIT + index;
    let cells = [
        &PRODUCT_OWNERS[owner],
        &PRODUCT_CLOSURE[offset],
        &PRODUCT_CLOSED_ACK[offset],
        product_cell(owner, index, true),
        product_cell(owner, index, false),
    ];
    let words = cells.map(AtomicJoinedWord::load);
    if words.iter().any(|w| {
        joined_receiver(*w) != fence.receiver
            || joined_session(*w) != fence.session
            || joined_ticket(*w) > fence.ticket
    }) || words
        .windows(2)
        .any(|w| joined_ticket(w[0]) >= joined_ticket(w[1]))
        || words[2..].iter().any(|w| !joined_permitted(*w))
        || cells
            .iter()
            .zip(words)
            .any(|(cell, word)| cell.load() != word)
        || product_permission(owner, index, fence.receiver, fence.session) != Some(true)
    {
        return None;
    }
    Some(words)
}

fn identity(governor: u64) -> Option<([u8; 8], u32)> {
    #[cfg(test)]
    tests::READ_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
    let person = copy_current::<8>(governor)?;
    let owner = u32::from_le_bytes(copy_current::<4>(
        governor.checked_add(governor_payment::CORE_GOVERNOR_OWNER_OFFSET)?,
    )?);
    Some((person, owner))
}

extern "system" fn begin(
    out: *mut NativeInvocation,
    governors: u64,
    governor: u64,
    requested: u32,
) {
    let _error = LastError(unsafe { GetLastError() });
    // Gateway zeroes the first word even if unwind occurs before this helper.
    unsafe { ptr::write(out, NativeInvocation::default()) };
    let invocation = unsafe { &mut *out };
    invocation
        .cookie
        .store(runtime::invocation_start(), Ordering::Release);
    if !runtime::invocation_admitted(invocation.cookie.load(Ordering::Acquire)) {
        return; // Original still runs once; no metadata/ticket/receiver reads.
    }
    invocation.governors = governors;
    invocation.governor = governor;
    invocation.requested = requested;
    let status = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        #[cfg(test)]
        tests::READ_ATTEMPTS.fetch_add(1, Ordering::Relaxed);
        invocation.fence = runtime::call_begin(governors);
        if let Some((person, owner)) = identity(governor) {
            invocation.identity = person;
            invocation.owner = owner;
            invocation.readable = 1;
            invocation.admission = admission(
                invocation.fence,
                u32::from_le_bytes(person[4..].try_into().unwrap()) as usize,
            )
            .unwrap_or([0; 5]);
        }
        #[cfg(test)]
        if tests::BEGIN_PANIC.swap(false, Ordering::AcqRel) {
            panic!("promotion entry helper panic fixture");
        }
    }));
    if status.is_err() {
        invocation.readable = 0;
        invocation.admission = [0; 5];
    }
}

fn restrict_current(original: u64, invocation: &NativeInvocation) -> u64 {
    if !runtime::invocation_admitted(invocation.cookie.load(Ordering::Acquire)) {
        return original & !0xff;
    }
    if original & 0xff == 0 {
        return original;
    }
    let closed = original & !0xff;
    let fence = invocation.fence;
    let current = || {
        runtime::owned()
            && composite::ENABLED.load(Ordering::Acquire)
            && invocation.readable == 1
            && invocation.owner == fence.owner
            && runtime::call_current(fence, invocation.governors)
            && identity(invocation.governor) == Some((invocation.identity, invocation.owner))
    };
    if !current() {
        return closed;
    }
    let owner = fence.owner as usize;
    let index = u32::from_le_bytes(invocation.identity[4..].try_into().unwrap());
    if index >= PRODUCTION_INDEX_LIMIT as u32 {
        return closed;
    }
    let catalog = || composite::ready_catalog(owner, fence.receiver, fence.session);
    let Some(definitions) = catalog() else {
        return closed;
    };
    let Some(definition) = definitions.into_iter().find(|d| d.governor_index == index) else {
        // Absence from a complete current catalog is the ONLY Standard proof.
        // Ordinary Governors need not have a persona-compatible flag vector.
        return if current() && catalog() == Some(definitions) {
            governor_payment::restrict_promotion(
                original,
                PromotionSubject::Standard,
                invocation.requested,
                true,
            )
        } else {
            closed
        };
    };
    // This is the synchronized production AdmissionAllowed verdict: OWNER,
    // STATE, person presence, saved Cost and all-person outstanding reservation.
    // PAID_ALLOW is deliberately not consulted: payment may still be partial.
    let admitted = || admission(fence, index as usize) == Some(invocation.admission);
    if !admitted() {
        return closed;
    }
    let Ok(snapshot) =
        governor_payment::read_current(&Memory, invocation.governors, invocation.governor)
    else {
        return closed;
    };
    let result = governor_payment::restrict_promotion(
        original,
        PromotionSubject::Persona {
            definition,
            owner: fence.owner,
            snapshot: Some(&snapshot),
            joined: true,
        },
        invocation.requested,
        current(),
    );
    if current() && admitted() && catalog() == Some(definitions) {
        result
    } else {
        closed
    }
}

extern "system" fn end(invocation: *const NativeInvocation, original: u64) -> u64 {
    let _error = LastError(unsafe { GetLastError() });
    #[cfg(test)]
    if tests::END_PAUSE.swap(false, Ordering::AcqRel) {
        tests::END_PAUSED.store(true, Ordering::Release);
        while !tests::END_RELEASE.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
    }
    let result = std::panic::catch_unwind(|| {
        #[cfg(test)]
        if tests::END_PANIC.swap(false, Ordering::AcqRel) {
            panic!("promotion exit helper panic fixture");
        }
        restrict_current(original, unsafe { &*invocation })
    })
    .unwrap_or(original & !0xff);
    unsafe { runtime::invocation_finish(ptr::addr_of!((*invocation).cookie)) };
    result
}

unsafe extern "system" fn unwind(
    record: *const EXCEPTION_RECORD,
    frame: usize,
    _context: *mut CONTEXT,
    _dispatcher: *mut c_void,
) -> i32 {
    let _error = LastError(unsafe { GetLastError() });
    unsafe { runtime::invocation_unwind(record, (frame + 0xc0) as *const AtomicU64) };
    1 // ExceptionContinueSearch: never manufacture a normal native return.
}

#[unsafe(no_mangle)]
static RUNTIME_PROMOTION_TARGET: AtomicPtr<u8> = AtomicPtr::new(null_mut());

// Fixed callee's COMPLETE body consumes RCX/RDX/R8D only; no stack inputs or
// R9 input (pr86-native-admission.md:559-565). No guessed stack span is copied.
// Original receives a correctly aligned Win64 frame/home area and is called
// exactly once. Preserve entry/return volatile GPRs, XMM0..5 and RFLAGS. Only
// AL may be cleared. Nonvolatile GPR/SIMD are preserved by the Win64 ABI.
// Caller RSP, home/stack contents and original return address remain intact.
global_asm!(
    ".globl runtime_promotion_gateway", ".seh_proc runtime_promotion_gateway",
    "runtime_promotion_gateway:",
    "pushfq", ".seh_stackalloc 8",
    "sub rsp,0x1c0", ".seh_stackalloc 0x1c0",
    "mov qword ptr [rsp+0xc0],0", ".seh_handler {unwind}, @unwind", ".seh_endprologue",
    "mov [rsp+0x20],rcx", "mov [rsp+0x28],rdx", "mov [rsp+0x30],r8", "mov [rsp+0x38],r9",
    "mov [rsp+0x40],rax", "mov [rsp+0x48],r10", "mov [rsp+0x50],r11",
    "movdqu [rsp+0x60],xmm0", "movdqu [rsp+0x70],xmm1", "movdqu [rsp+0x80],xmm2",
    "movdqu [rsp+0x90],xmm3", "movdqu [rsp+0xa0],xmm4", "movdqu [rsp+0xb0],xmm5",
    "mov r9d,r8d", "mov r8,rdx", "mov rdx,rcx", "lea rcx,[rsp+0xc0]", "call {begin}",
    "mov rcx,[rsp+0x20]", "mov rdx,[rsp+0x28]", "mov r8,[rsp+0x30]", "mov r9,[rsp+0x38]",
    "mov rax,[rsp+0x40]", "mov r10,[rsp+0x48]", "mov r11,[rsp+0x50]",
    "movdqu xmm0,[rsp+0x60]", "movdqu xmm1,[rsp+0x70]", "movdqu xmm2,[rsp+0x80]",
    "movdqu xmm3,[rsp+0x90]", "movdqu xmm4,[rsp+0xa0]", "movdqu xmm5,[rsp+0xb0]",
    "call {restore_flags}",
    "call qword ptr [rip+RUNTIME_PROMOTION_TARGET]",
    ".globl runtime_promotion_return", "runtime_promotion_return:",
    "call {save_flags}",
    "mov [rsp+0x20],rcx", "mov [rsp+0x28],rdx", "mov [rsp+0x30],r8", "mov [rsp+0x38],r9",
    "mov [rsp+0x40],rax", "mov [rsp+0x48],r10", "mov [rsp+0x50],r11",
    "movdqu [rsp+0x60],xmm0", "movdqu [rsp+0x70],xmm1", "movdqu [rsp+0x80],xmm2",
    "movdqu [rsp+0x90],xmm3", "movdqu [rsp+0xa0],xmm4", "movdqu [rsp+0xb0],xmm5",
    "mov rdx,rax", "lea rcx,[rsp+0xc0]", "call {end}", "mov [rsp+0x40],rax",
    "movdqu xmm0,[rsp+0x60]", "movdqu xmm1,[rsp+0x70]", "movdqu xmm2,[rsp+0x80]",
    "movdqu xmm3,[rsp+0x90]", "movdqu xmm4,[rsp+0xa0]", "movdqu xmm5,[rsp+0xb0]",
    "mov rcx,[rsp+0x20]", "mov rdx,[rsp+0x28]", "mov r8,[rsp+0x30]", "mov r9,[rsp+0x38]",
    "mov rax,[rsp+0x40]", "mov r10,[rsp+0x48]", "mov r11,[rsp+0x50]",
    "call {restore_flags}", "lea rsp,[rsp+0x1c8]", "ret", ".seh_endproc",
    begin = sym begin, end = sym end, unwind = sym unwind,
    restore_flags = sym root_flags_restore, save_flags = sym root_flags_save,
);
unsafe extern "system" {
    pub(super) fn runtime_promotion_gateway();
    fn root_flags_restore();
    fn root_flags_save();
}

static USED: AtomicBool = AtomicBool::new(false);
/// No E8 operand can reach this allocation until ownership is transferred to
/// a Slot. Failed RX protection/cache/range validation releases only this
/// unpublished page; any potentially fetched relay stays owned for process life.
struct UnpublishedRelay(*mut u8);
impl Drop for UnpublishedRelay {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { VirtualFree(self.0.cast(), 0, MEM_RELEASE) };
        }
    }
}
struct Slot {
    call: *mut u8,
    original: [u8; 16],
    operand: u32,
    protection: u32,
    _relay: *mut u8,
}
unsafe impl Send for Slot {}
static SLOTS: Mutex<Vec<Slot>> = Mutex::new(Vec::new());

pub(super) fn used() -> bool {
    USED.load(Ordering::Acquire)
}

fn window_valid(call: usize, callee: usize, bytes: [u8; 16], expected: [u8; 16]) -> bool {
    bytes == expected
        && bytes[0] == 0xe8
        && promotion_operand(call, callee)
            == Some(u32::from_le_bytes(bytes[1..5].try_into().unwrap()))
}

/// Validate the fixed SHA through the existing mode8 validator, PE .text
/// mapping, both complete caller windows/argument witnesses and callee boundary.
/// Installation compares the same witnesses against the LOADED pinned module.
pub(super) fn verify(base: usize) -> bool {
    if pinned_gamecore(Path::new(GAMECORE), BridgeMode::PromotionCall).is_none() {
        return false;
    }
    let Ok(mut file) = File::open(GAMECORE) else {
        return false;
    };
    let witnesses: &[(usize, &[u8])] = &[
        (PINNED_PROMOTION_CALL_RVA, &PROMOTION_CALL_WINDOW),
        (AI_CALL_RVA, &AI_WINDOW),
        (
            PINNED_PROMOTION_CALL_RVA - OFFICIAL_ARGS.len(),
            &OFFICIAL_ARGS,
        ),
        (AI_CALL_RVA - AI_ARGS.len(), &AI_ARGS),
        (PINNED_PROMOTION_CALLEE_RVA - 5, &CALLEE_BOUNDARY),
    ];
    witnesses.iter().all(|(rva, expected)| {
        let Some(offset) = pe_file_offset(&mut file, *rva as u32) else {
            return false;
        };
        let mut disk = vec![0; expected.len()];
        let Some(address) = base.checked_add(*rva) else {
            return false;
        };
        if file.seek(SeekFrom::Start(offset)).is_err()
            || file.read_exact(&mut disk).is_err()
            || disk != *expected
        {
            return false;
        }
        expected
            .iter()
            .enumerate()
            .all(|(i, byte)| copy_current::<1>((address + i) as u64) == Some([*byte]))
    })
}

pub(super) unsafe fn install(base: usize) -> Result<(), &'static str> {
    if !runtime::owned() || !supports_atomic_promotion_fetch() || !verify(base) {
        return Err("runtime promotion fixed image or AMD precondition differs");
    }
    let sites = [
        (
            unsafe { (base as *mut u8).add(PINNED_PROMOTION_CALL_RVA) },
            PROMOTION_CALL_WINDOW,
        ),
        (unsafe { (base as *mut u8).add(AI_CALL_RVA) }, AI_WINDOW),
    ];
    unsafe { install_sites(sites, (base + PINNED_PROMOTION_CALLEE_RVA) as *mut u8) }
}

/// Caller has pinned both code images and verified the fixed primary image.
/// Also used with process-owned synthetic sites, never a game/controller ABI.
unsafe fn install_sites(
    sites: [(*mut u8, [u8; 16]); 2],
    callee: *mut u8,
) -> Result<(), &'static str> {
    if !supports_atomic_promotion_fetch() {
        return Err("runtime promotion requires AMD atomic operand fetch");
    }
    let mut slots = SLOTS
        .lock()
        .map_err(|_| "runtime promotion lock poisoned")?;
    if USED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
        || !slots.is_empty()
    {
        return Err("runtime promotion lifetime already consumed");
    }
    if RUNTIME_PROMOTION_TARGET
        .compare_exchange(null_mut(), callee, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err("runtime promotion target already published");
    }
    let result = (|| {
        for (call, expected) in sites {
            let Some(original) = copy_current::<16>(call as u64) else {
                return Err("runtime promotion caller unreadable");
            };
            if !window_valid(call as usize, callee as usize, original, expected) {
                return Err("runtime promotion complete call window differs");
            }
            let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
            if unsafe { VirtualQuery(call.cast(), &mut info, std::mem::size_of_val(&info)) } == 0 {
                return Err("runtime promotion protection unreadable");
            }
            let relay = unsafe { promotion_relay_near(call as usize) }
                .ok_or("runtime promotion relay unavailable")?;
            let mut unpublished = UnpublishedRelay(relay);
            let gateway = runtime_promotion_gateway as *const () as u64;
            unsafe {
                ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), relay, 6);
                ptr::copy_nonoverlapping(gateway.to_le_bytes().as_ptr(), relay.add(6), 8);
            }
            let mut old = 0;
            if unsafe { VirtualProtect(relay.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut old) } == 0
                || unsafe { FlushInstructionCache(GetCurrentProcess(), relay.cast(), 14) } == 0
            {
                return Err("runtime promotion relay publication failed");
            }
            let operand = promotion_operand(call as usize, relay as usize)
                .ok_or("runtime promotion relay range")?;
            // Retain BEFORE the exchange: even a protection/flush failure after
            // successful CAS must have an owner capable of restoring it.
            slots.push(Slot {
                call,
                original,
                operand,
                protection: info.Protect,
                _relay: relay,
            });
            unpublished.0 = null_mut();
            let original_operand = u32::from_le_bytes(original[1..5].try_into().unwrap());
            let outcome = unsafe { exchange_promotion_operand(call, original_operand, operand) }?;
            let mut patched = original;
            patched[1..5].copy_from_slice(&operand.to_le_bytes());
            if !outcome.exchanged
                || !outcome.protection_restored
                || !outcome.cache_flushed
                || copy_current::<16>(call as u64) != Some(patched)
            {
                return Err("runtime promotion operand publication unverified");
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        return if unsafe { restore_slots(&mut slots) }.is_ok() {
            Err(error)
        } else {
            Err("critical: runtime promotion rollback failed")
        };
    }
    Ok(())
}

unsafe fn restore_slots(slots: &mut Vec<Slot>) -> Result<bool, &'static str> {
    let had = !slots.is_empty();
    let mut failed = false;
    for i in (0..slots.len()).rev() {
        let slot = &slots[i];
        let original_operand = u32::from_le_bytes(slot.original[1..5].try_into().unwrap());
        let current = copy_current::<4>(unsafe { slot.call.add(1) } as u64).map(u32::from_le_bytes);
        let exchanged = if current == Some(slot.operand) {
            unsafe { exchange_promotion_operand(slot.call, slot.operand, original_operand) }
                .is_ok_and(|r| r.exchanged)
        } else {
            current == Some(original_operand)
        };
        // Explicit saved protection also repairs an install whose first restore
        // of VirtualProtect failed. A second CAS's previous RWX is not authority.
        let mut old = 0;
        let protected =
            unsafe { VirtualProtect(slot.call.add(1).cast(), 4, slot.protection, &mut old) } != 0;
        let flushed =
            unsafe { FlushInstructionCache(GetCurrentProcess(), slot.call.cast(), 5) } != 0;
        if exchanged
            && protected
            && flushed
            && copy_current::<16>(slot.call as u64) == Some(slot.original)
        {
            slots.remove(i);
        } else {
            failed = true;
        }
    }
    if failed {
        Err("critical: runtime promotion restoration unverified")
    } else {
        Ok(had)
    }
}

pub(super) unsafe fn restore() -> Result<bool, &'static str> {
    let mut slots = SLOTS
        .lock()
        .map_err(|_| "critical: runtime promotion lock poisoned")?;
    unsafe { restore_slots(&mut slots) }
}

#[cfg(test)]
mod tests {
    include!("promotion_tests.rs");
    pub(super) static BEGIN_PANIC: AtomicBool = AtomicBool::new(false);
    pub(super) static END_PANIC: AtomicBool = AtomicBool::new(false);
    pub(super) static READ_ATTEMPTS: AtomicU64 = AtomicU64::new(0);
    static NATIVE_ORIGINALS: AtomicU64 = AtomicU64::new(0);
    static ORIGINAL_ERROR: AtomicU32 = AtomicU32::new(0);
    static REBUILD_STATUS: AtomicU32 = AtomicU32::new(u32::MAX);

    unsafe fn invocation_calls(target: *mut u8) -> [Call; 2] {
        let page = unsafe { promotion_relay_near(target as usize | 3) }.unwrap();
        let (sites, calls) = unsafe { sites(page, target) };
        unsafe { install_sites(sites, target) }.unwrap();
        calls
    }

    extern "system" fn closed_original(_: u64, _: u64, _: u32, result: u64) -> u64 {
        NATIVE_ORIGINALS.fetch_add(1, Ordering::Relaxed);
        ORIGINAL_ERROR.store(unsafe { GetLastError() }, Ordering::Relaxed);
        REBUILD_STATUS.store(runtime::BridgeRuntimeRebuild(null_mut()), Ordering::Release);
        unsafe { SetLastError(0x11223344) };
        result
    }

    #[test]
    fn promotion_invocation_closed_before_reads_stays_rejected_across_original_rebuild() {
        if !child("promotion_invocation_closed_before_reads_stays_rejected_across_original_rebuild")
        {
            return;
        }
        runtime::allocate_trace().unwrap();
        assert!(runtime::start_observation(MAX_ACTIVE_SECONDS));
        let calls = unsafe { invocation_calls(closed_original as *const () as *mut u8) };
        for call in calls {
            assert_eq!(runtime::BridgeRuntimeClose(null_mut()), 0);
            READ_ATTEMPTS.store(0, Ordering::Release);
            let before = NATIVE_ORIGINALS.load(Ordering::Acquire);
            unsafe { SetLastError(0x55667788) };
            let result = unsafe { call(1, 3, 8, ORIGINAL) };
            let error = unsafe { GetLastError() };
            assert_eq!(result, ORIGINAL & !255);
            assert_eq!(NATIVE_ORIGINALS.load(Ordering::Acquire), before + 1);
            assert_eq!(REBUILD_STATUS.load(Ordering::Acquire), 0);
            assert_eq!(ORIGINAL_ERROR.load(Ordering::Acquire), 0x55667788);
            assert_eq!(error, 0x11223344);
            assert_eq!(
                READ_ATTEMPTS.load(Ordering::Acquire),
                0,
                "refused cookie must not acquire metadata or read native identity after Rebuild"
            );
        }
        assert_eq!(runtime::BridgeRuntimeClose(null_mut()), 0);
        unsafe { restore() }.unwrap();
    }

    #[test]
    fn promotion_invocation_normal_return_and_exit_panic_settle_after_observation_expiry() {
        if !child(
            "promotion_invocation_normal_return_and_exit_panic_settle_after_observation_expiry",
        ) {
            return;
        }
        let mut fixture = Fixture::new();
        fixture.catalog();
        fixture.select(200, &[]);
        let target = unsafe { fixture._page.add(1024) };
        let code = [0xff, 0x41, 0x78, 0x4c, 0x89, 0xc8, 0xc3];
        unsafe { ptr::copy_nonoverlapping(code.as_ptr(), target, code.len()) };
        let (sites, calls) = unsafe { sites(fixture._page, target) };
        unsafe { install_sites(sites, target) }.unwrap();
        runtime::expire_observation();
        assert_eq!(runtime::BridgeRuntimeStopObservation(null_mut()), 0);
        for call in calls {
            let before = fixture.governors[0x78 / 8];
            let ordinary = unsafe { call(fixture.this(), fixture.person(), 8, ORIGINAL) };
            END_PANIC.store(true, Ordering::Release);
            let panic = unsafe { call(fixture.this(), fixture.person(), 8, ORIGINAL) };
            let resumed = unsafe { call(fixture.this(), fixture.person(), 8, ORIGINAL) };
            BEGIN_PANIC.store(true, Ordering::Release);
            let entry_panic = unsafe { call(fixture.this(), fixture.person(), 8, ORIGINAL) };
            assert_eq!(
                (ordinary, panic, resumed),
                (ORIGINAL, ORIGINAL & !255, ORIGINAL)
            );
            assert_eq!(entry_panic, ORIGINAL & !255);
            assert_eq!(
                fixture.governors[0x78 / 8],
                before + 4,
                "original once per call including helper panic"
            );
        }
        assert_eq!(
            runtime::BridgeRuntimeClose(null_mut()),
            0,
            "no held invocation remains"
        );
        unsafe { restore() }.unwrap();
    }

    #[test]
    fn promotion_invocation_raw_unwind_flags_settle_only_the_owned_cookie_once() {
        if !child("promotion_invocation_raw_unwind_flags_settle_only_the_owned_cookie_once") {
            return;
        }
        runtime::allocate_trace().unwrap();
        assert!(runtime::start_observation(MAX_ACTIVE_SECONDS));
        #[repr(align(16))]
        struct Frame([u8; 0x1c8]);
        let mut frame = Frame([0; 0x1c8]);
        let native = unsafe { frame.0.as_mut_ptr().add(0xc0).cast::<NativeInvocation>() };
        begin(native, 0, 0, 0);
        let held = unsafe { (*native).cookie.load(Ordering::Acquire) };
        assert!(held >= 2);
        let mut record: EXCEPTION_RECORD = unsafe { std::mem::zeroed() };
        assert_eq!(
            unsafe { unwind(&record, frame.0.as_ptr() as usize, null_mut(), null_mut()) },
            1
        );
        assert_eq!(
            unsafe { (*native).cookie.load(Ordering::Acquire) },
            held,
            "exception search is not invocation completion"
        );
        record.ExceptionFlags = 2;
        unsafe { SetLastError(0xabc123) };
        let disposition =
            unsafe { unwind(&record, frame.0.as_ptr() as usize, null_mut(), null_mut()) };
        let error = unsafe { GetLastError() };
        assert_eq!(disposition, 1);
        assert_eq!(error, 0xabc123);
        assert_eq!(unsafe { (*native).cookie.load(Ordering::Acquire) }, 0);
        assert_eq!(runtime::BridgeRuntimeClose(null_mut()), 0);
        assert_eq!(runtime::BridgeRuntimeRebuild(null_mut()), 0);
        let newer = AtomicU64::new(runtime::invocation_start());
        assert!(newer.load(Ordering::Acquire) > held);
        unsafe { unwind(&record, frame.0.as_ptr() as usize, null_mut(), null_mut()) };
        assert_eq!(end(native, ORIGINAL), ORIGINAL & !255);
        unsafe { runtime::invocation_finish(&newer) };
        assert_eq!(
            runtime::BridgeRuntimeClose(null_mut()),
            0,
            "repeated unwind/normal finish must not settle a newer generation"
        );
    }

    global_asm!(
        ".globl promotion_runtime_original_unwinds", ".seh_proc promotion_runtime_original_unwinds",
        "promotion_runtime_original_unwinds:", "sub rsp,0x28", ".seh_stackalloc 0x28", ".seh_endprologue",
        "lock inc qword ptr [rip+{count}]", "call {original}", "add rsp,0x28", "ret", ".seh_endproc",
        count = sym NATIVE_ORIGINALS, original = sym promotion_original_unwinds,
    );
    unsafe extern "system" {
        fn promotion_runtime_original_unwinds();
        fn root_flags_restore_pushed();
        fn root_flags_restore_pop();
        fn root_flags_restore_ret();
        fn root_flags_save_pushed();
        fn root_flags_save_pop();
        fn root_flags_save_ret();
    }

    #[test]
    fn promotion_invocation_flags_helpers_unwind_to_exact_gateway_cookie() {
        if !child("promotion_invocation_flags_helpers_unwind_to_exact_gateway_cookie") {
            return;
        }
        runtime::allocate_trace().unwrap();
        assert!(runtime::start_observation(MAX_ACTIVE_SECONDS));
        #[repr(align(16))]
        struct Stack([u64; 96]);
        for (pc, pushed) in [
            (root_flags_restore_pushed as *const () as u64, true),
            (root_flags_restore_pop as *const () as u64, true),
            (root_flags_restore_ret as *const () as u64, false),
            (root_flags_save_pushed as *const () as u64, true),
            (root_flags_save_pop as *const () as u64, true),
            (root_flags_save_ret as *const () as u64, false),
        ] {
            let mut stack = Stack([0; 96]);
            let gateway_sp = unsafe { stack.0.as_mut_ptr().add(4) } as u64;
            let native = (gateway_sp + 0xc0) as *mut NativeInvocation;
            begin(native, 0, 0, 0);
            assert!(unsafe { (*native).cookie.load(Ordering::Acquire) } >= 2);
            stack.0[3] = runtime_promotion_return as *const () as u64;
            stack.0[2] = 0x202; // helper's temporary flags, never a return RIP
            stack.0[4 + 0x1c8 / 8] = 0x11223344;
            let mut context: CONTEXT = unsafe { std::mem::zeroed() };
            context.Rsp = gateway_sp - if pushed { 16 } else { 8 };
            context.Rip = pc;
            let (mut base, mut data, mut establisher) = (0, null_mut(), 0);
            let helper = unsafe { RtlLookupFunctionEntry(pc, &mut base, null_mut()) };
            assert!(!helper.is_null());
            unsafe {
                RtlVirtualUnwind(
                    0,
                    base,
                    pc,
                    helper,
                    &mut context,
                    &mut data,
                    &mut establisher,
                    null_mut(),
                )
            };
            assert_eq!(context.Rsp, gateway_sp);
            assert_eq!(context.Rip, runtime_promotion_return as *const () as u64);
            let gateway = unsafe { RtlLookupFunctionEntry(context.Rip, &mut base, null_mut()) };
            assert!(!gateway.is_null());
            let handler = unsafe {
                RtlVirtualUnwind(
                    2,
                    base,
                    context.Rip,
                    gateway,
                    &mut context,
                    &mut data,
                    &mut establisher,
                    null_mut(),
                )
            };
            assert_eq!(handler, unwind as *const () as usize);
            assert_eq!(establisher, gateway_sp);
            assert_eq!(context.Rip, 0x11223344);
            assert_eq!(context.Rsp, gateway_sp + 0x1d0);
            let mut record: EXCEPTION_RECORD = unsafe { std::mem::zeroed() };
            record.ExceptionFlags = 2;
            unsafe { unwind(&record, establisher as usize, &mut context, null_mut()) };
            assert_eq!(unsafe { (*native).cookie.load(Ordering::Acquire) }, 0);
        }
        assert_eq!(runtime::BridgeRuntimeClose(null_mut()), 0);
    }

    #[test]
    fn promotion_invocation_actual_seh_returns_to_native_caller_and_drains() {
        if !child("promotion_invocation_actual_seh_returns_to_native_caller_and_drains") {
            return;
        }
        runtime::allocate_trace().unwrap();
        assert!(runtime::start_observation(MAX_ACTIVE_SECONDS));
        let calls =
            unsafe { invocation_calls(promotion_runtime_original_unwinds as *const () as *mut u8) };
        let mut state = [0u64; 2];
        for call in calls {
            let before = NATIVE_ORIGINALS.load(Ordering::Acquire);
            let landed = unsafe { promotion_unwind_driver(state.as_mut_ptr(), call as usize) };
            assert_eq!(landed, 1);
            assert_eq!(NATIVE_ORIGINALS.load(Ordering::Acquire), before + 1);
            assert_eq!(
                runtime::BridgeRuntimeClose(null_mut()),
                0,
                "actual native unwind must release frame lease"
            );
            assert_eq!(runtime::BridgeRuntimeRebuild(null_mut()), 0);
        }
        assert_eq!(runtime::BridgeRuntimeClose(null_mut()), 0);
        unsafe { restore() }.unwrap();
    }
}
