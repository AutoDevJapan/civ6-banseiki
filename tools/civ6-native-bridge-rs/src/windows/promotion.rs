//! Restriction of natural CanPromote results, owned by the ordinary runtime.
//! No AI actor, operation constructor, getter, Lua VM or mutation is invoked.
//! Only the two pinned aligned E8 operands are changed, under mode8's AMD rule.
use super::*;
use crate::governor_payment::{self, CurrentMemory, PromotionSubject};

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
        copy_current(address)
    }
}

// Entirely invocation-local. No persistent Core pointer, TLS, trace writer or
// in-flight counter exists here, so SEH needs only unwind the assembly frame.
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Invocation {
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
const _: () = assert!(std::mem::size_of::<Invocation>() == 0x90);

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
    let person = copy_current::<8>(governor)?;
    let owner = u32::from_le_bytes(copy_current::<4>(
        governor.checked_add(governor_payment::CORE_GOVERNOR_OWNER_OFFSET)?,
    )?);
    Some((person, owner))
}

extern "system" fn begin(out: *mut Invocation, governors: u64, governor: u64, requested: u32) {
    // Initialize before any fallible work. A Rust panic closes this invocation.
    let mut invocation = Invocation {
        governors,
        governor,
        requested,
        ..Invocation::default()
    };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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
    }));
    unsafe { ptr::write(out, invocation) };
}

fn restrict_current(original: u64, invocation: Invocation) -> u64 {
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

extern "system" fn end(invocation: *const Invocation, original: u64) -> u64 {
    #[cfg(test)]
    if tests::END_PAUSE.swap(false, Ordering::AcqRel) {
        tests::END_PAUSED.store(true, Ordering::Release);
        while !tests::END_RELEASE.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
    }
    std::panic::catch_unwind(|| restrict_current(original, unsafe { ptr::read(invocation) }))
        .unwrap_or(original & !0xff)
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
    "sub rsp,0x1c0", ".seh_stackalloc 0x1c0", ".seh_endprologue",
    "mov [rsp+0x20],rcx", "mov [rsp+0x28],rdx", "mov [rsp+0x30],r8", "mov [rsp+0x38],r9",
    "mov [rsp+0x40],rax", "mov [rsp+0x48],r10", "mov [rsp+0x50],r11",
    "mov rax,[rsp+0x1c0]", "mov [rsp+0x58],rax",
    "movdqu [rsp+0x60],xmm0", "movdqu [rsp+0x70],xmm1", "movdqu [rsp+0x80],xmm2",
    "movdqu [rsp+0x90],xmm3", "movdqu [rsp+0xa0],xmm4", "movdqu [rsp+0xb0],xmm5",
    "mov r9d,r8d", "mov r8,rdx", "mov rdx,rcx", "lea rcx,[rsp+0xc0]", "call {begin}",
    "mov rcx,[rsp+0x20]", "mov rdx,[rsp+0x28]", "mov r8,[rsp+0x30]", "mov r9,[rsp+0x38]",
    "mov rax,[rsp+0x40]", "mov r10,[rsp+0x48]", "mov r11,[rsp+0x50]",
    "movdqu xmm0,[rsp+0x60]", "movdqu xmm1,[rsp+0x70]", "movdqu xmm2,[rsp+0x80]",
    "movdqu xmm3,[rsp+0x90]", "movdqu xmm4,[rsp+0xa0]", "movdqu xmm5,[rsp+0xb0]",
    "push qword ptr [rsp+0x58]", "popfq",
    "call qword ptr [rip+RUNTIME_PROMOTION_TARGET]",
    ".globl runtime_promotion_return", "runtime_promotion_return:",
    "pushfq", "pop qword ptr [rsp+0x58]",
    "mov [rsp+0x20],rcx", "mov [rsp+0x28],rdx", "mov [rsp+0x30],r8", "mov [rsp+0x38],r9",
    "mov [rsp+0x40],rax", "mov [rsp+0x48],r10", "mov [rsp+0x50],r11",
    "movdqu [rsp+0x60],xmm0", "movdqu [rsp+0x70],xmm1", "movdqu [rsp+0x80],xmm2",
    "movdqu [rsp+0x90],xmm3", "movdqu [rsp+0xa0],xmm4", "movdqu [rsp+0xb0],xmm5",
    "mov rdx,rax", "lea rcx,[rsp+0xc0]", "call {end}", "mov [rsp+0x40],rax",
    "movdqu xmm0,[rsp+0x60]", "movdqu xmm1,[rsp+0x70]", "movdqu xmm2,[rsp+0x80]",
    "movdqu xmm3,[rsp+0x90]", "movdqu xmm4,[rsp+0xa0]", "movdqu xmm5,[rsp+0xb0]",
    "mov rcx,[rsp+0x20]", "mov rdx,[rsp+0x28]", "mov r8,[rsp+0x30]", "mov r9,[rsp+0x38]",
    "mov rax,[rsp+0x40]", "mov r10,[rsp+0x48]", "mov r11,[rsp+0x50]",
    "push qword ptr [rsp+0x58]", "popfq", "lea rsp,[rsp+0x1c8]", "ret", ".seh_endproc",
    begin = sym begin, end = sym end,
);
unsafe extern "system" {
    pub(super) fn runtime_promotion_gateway();
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
#[path = "promotion_tests.rs"]
mod tests;
