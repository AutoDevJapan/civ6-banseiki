//! One formal extra-title operation, ONLY on a validated natural AI callback.
//! All pointers below are the fixed native caller's exact ABIs, not Lua APIs.
use super::*;
use crate::ai_operation::gateway::{LeaseStamp, RuntimeLease};
use crate::governor_payment::{read_current, CurrentMemory, PaymentDefinition, PromotionSnapshot};
use std::{cell::RefCell, sync::OnceLock};

#[repr(C, align(8))]
struct Dictionary([u64; 8]); // 9A1800 + 9A4F60 initialize exactly through +38.
#[repr(C)]
struct Operation {
    kind: u32,
    value_type: u32,
    parameters: Dictionary,
} // AI 4A7E3F..54
#[repr(C, align(8))]
struct Wrapper([u64; 13]); // 38990: copied Operation at +20, dictionary at +28.
const _: () = assert!(
    std::mem::size_of::<Dictionary>() == 0x40
        && std::mem::size_of::<Operation>() == 0x48
        && std::mem::size_of::<Wrapper>() == 0x68
);

struct Api {
    dictionary_vtable: u64,
    parameter: unsafe extern "system-unwind" fn(*mut Dictionary, u32) -> u64,
    skip_ai: unsafe extern "system-unwind" fn(u64) -> u8,
    governors: unsafe extern "system-unwind" fn(u64) -> u64,
    governor: unsafe extern "system-unwind" fn(u64, u32) -> u64,
    spent: unsafe extern "system-unwind" fn(u64) -> u32,
    construct: unsafe extern "system-unwind" fn(*mut Dictionary) -> *mut Dictionary,
    destroy: unsafe extern "system-unwind" fn(*mut Dictionary),
    integer: unsafe extern "system-unwind" fn(*mut Dictionary, u32, u32),
    operation: unsafe extern "system-unwind" fn(u32, u32, *mut Dictionary, *mut Operation) -> u8,
    filter: unsafe extern "system-unwind" fn(u32, *mut Operation, u8, u64) -> u8,
    wrap: unsafe extern "system-unwind" fn(*mut Wrapper, u32, *mut Operation) -> *mut Wrapper,
    execute: unsafe extern "system-unwind" fn(*mut Wrapper),
}
static API: OnceLock<Api> = OnceLock::new();
static ENABLED: AtomicBool = AtomicBool::new(false);
static ACTION_THREAD: AtomicU32 = AtomicU32::new(0);
static NATIVE_UNWINDS: AtomicU32 = AtomicU32::new(0);
thread_local! {static BEFORE:RefCell<Option<Prepared>>=const{RefCell::new(None)};}
thread_local! {static CLEANUP:RefCell<[(u64,u64);3]>=const{RefCell::new([(0,0);3])};}

unsafe extern "system" fn native_cleanup(
    record: *const windows_sys::Win32::System::Diagnostics::Debug::EXCEPTION_RECORD,
    _frame: usize,
    _context: *mut windows_sys::Win32::System::Diagnostics::Debug::CONTEXT,
    _dispatcher: *mut c_void,
) -> i32 {
    if !record.is_null() && unsafe { (*record).ExceptionFlags } & (2 | 4 | 0x20 | 0x40) != 0 {
        NATIVE_UNWINDS.fetch_add(1, Ordering::AcqRel);
        cancel();
        let pending = CLEANUP.with(|cell| std::mem::replace(&mut *cell.borrow_mut(), [(0, 0); 3]));
        for (pointer, destructor) in pending.into_iter().rev() {
            if pointer != 0 {
                let destroy: unsafe extern "system-unwind" fn(*mut Dictionary) =
                    unsafe { std::mem::transmute(destructor as usize) };
                unsafe {
                    destroy(pointer as *mut Dictionary);
                }
            }
        }
        ACTION_THREAD.store(0, Ordering::Release);
    }
    1
}
// Native call boundary is BELOW the Rust caller: its termination handler runs
// while all constructor outputs on the caller's stack are still alive.
core::arch::global_asm!(
    ".globl ai_native_call4", ".seh_proc ai_native_call4", "ai_native_call4:",
    "sub rsp,0x28", ".seh_stackalloc 0x28", ".seh_handler {cleanup}, @unwind", ".seh_endprologue",
    "mov r11,rcx","mov rcx,rdx","mov rdx,r8","mov r8,r9","mov r9,[rsp+0x50]",
    // The return PC must NOT be in an epilogue; Win64 otherwise skips the
    // termination handler while unwinding an exception from the callee.
    "call r11","mov [rsp+0x20],rax","mov rax,[rsp+0x20]","add rsp,0x28","ret",".seh_endproc",cleanup=sym native_cleanup
);
unsafe extern "system-unwind" {
    fn ai_native_call4(function: usize, a: u64, b: u64, c: u64, d: u64) -> u64;
}
unsafe fn invoke(function: usize, args: [u64; 4]) -> u64 {
    unsafe { ai_native_call4(function, args[0], args[1], args[2], args[3]) }
}

#[derive(Clone, Debug)]
pub struct StepRecord {
    pub owner: u32,
    pub index: u32,
    pub generation: u32,
    pub before: u32,
    pub after: u32,
    pub title: u32,
    pub dispatched: bool,
    pub verified: bool,
}
static LAST: Mutex<Option<StepRecord>> = Mutex::new(None);
pub(super) fn cancel() {
    ENABLED.store(false, Ordering::Release);
}
pub(super) fn close() -> bool {
    cancel();
    if ACTION_THREAD.load(Ordering::Acquire) == unsafe { GetCurrentThreadId() } {
        return false;
    }
    // Foreign SEH cannot leak a Rust MutexGuard. The native boundary explicitly
    // releases this in-flight marker after cleanup, even without Rust Drop.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while ACTION_THREAD.load(Ordering::Acquire) != 0 {
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    true
}
struct ActionGuard;
impl ActionGuard {
    fn enter() -> Option<Self> {
        if !ENABLED.load(Ordering::Acquire) {
            return None;
        }
        let thread = unsafe { GetCurrentThreadId() };
        ACTION_THREAD
            .compare_exchange(0, thread, Ordering::AcqRel, Ordering::Acquire)
            .ok()?;
        if !ENABLED.load(Ordering::Acquire) {
            ACTION_THREAD.store(0, Ordering::Release);
            return None;
        }
        Some(Self)
    }
}
impl Drop for ActionGuard {
    fn drop(&mut self) {
        ACTION_THREAD.store(0, Ordering::Release);
    }
}

struct DictionaryGuard<'a> {
    dictionary: *mut Dictionary,
    api: &'a Api,
}
impl<'a> DictionaryGuard<'a> {
    fn new(dictionary: *mut Dictionary, api: &'a Api) -> Self {
        CLEANUP.with(|cell| {
            let mut list = cell.borrow_mut();
            let slot = list
                .iter_mut()
                .find(|(p, _)| *p == 0)
                .expect("three fixed native dictionaries");
            *slot = (dictionary as u64, api.destroy as usize as u64);
        });
        Self { dictionary, api }
    }
}
impl Drop for DictionaryGuard<'_> {
    fn drop(&mut self) {
        let present = CLEANUP.with(|cell| {
            let mut list = cell.borrow_mut();
            if let Some(slot) = list.iter_mut().find(|(p, _)| *p == self.dictionary as u64) {
                *slot = (0, 0);
                true
            } else {
                false
            }
        });
        if present {
            unsafe {
                invoke(self.api.destroy as usize, [self.dictionary as u64, 0, 0, 0]);
            }
        }
    }
}
pub(super) fn last() -> Option<StepRecord> {
    LAST.lock().ok()?.clone()
}

/// Called once under runtime install ownership, before enabling any callback.
pub(super) fn configure(base: u64, enabled: bool) -> Result<(), ()> {
    if !enabled {
        ENABLED.store(false, Ordering::Release);
        return Ok(());
    }
    if base == 0 {
        return Err(());
    }
    if API.get().is_none() {
        let image = std::fs::read(GAMECORE).map_err(|_| ())?;
        crate::ai_operation::pinned::audit(&image).map_err(|_| ())?;
        let entries = [
            0x2f9820u64,
            0x2de920,
            0x2de960,
            0x2dea90,
            0x9a4f60,
            0x9a4fe0,
            0x9a5d50,
            0x9a64f0,
            0x30cbe0,
            0x30c100,
            0x38990,
            0x38c80,
        ];
        for rva in entries {
            let addr = base.checked_add(rva).filter(|p| *p != 0).ok_or(())?;
            if copy_current::<8>(addr).as_ref().map(|v| v.as_slice())
                != image.get((rva - 0xc00) as usize..(rva - 0xc00 + 8) as usize)
            {
                return Err(());
            }
        }
        macro_rules! function {
            ($rva:expr,$ty:ty) => {
                unsafe { std::mem::transmute::<usize, $ty>((base + $rva) as usize) }
            };
        }
        let api = Api {
            dictionary_vtable: base + 0xabf918,
            parameter: function!(
                0x9a5d50,
                unsafe extern "system-unwind" fn(*mut Dictionary, u32) -> u64
            ),
            skip_ai: function!(0x2f9820, unsafe extern "system-unwind" fn(u64) -> u8),
            governors: function!(0x2de920, unsafe extern "system-unwind" fn(u64) -> u64),
            governor: function!(0x2de960, unsafe extern "system-unwind" fn(u64, u32) -> u64),
            spent: function!(0x2dea90, unsafe extern "system-unwind" fn(u64) -> u32),
            construct: function!(
                0x9a4f60,
                unsafe extern "system-unwind" fn(*mut Dictionary) -> *mut Dictionary
            ),
            destroy: function!(0x9a4fe0, unsafe extern "system-unwind" fn(*mut Dictionary)),
            integer: function!(
                0x9a64f0,
                unsafe extern "system-unwind" fn(*mut Dictionary, u32, u32)
            ),
            operation: function!(
                0x30cbe0,
                unsafe extern "system-unwind" fn(u32, u32, *mut Dictionary, *mut Operation) -> u8
            ),
            filter: function!(
                0x30c100,
                unsafe extern "system-unwind" fn(u32, *mut Operation, u8, u64) -> u8
            ),
            wrap: function!(
                0x38990,
                unsafe extern "system-unwind" fn(*mut Wrapper, u32, *mut Operation) -> *mut Wrapper
            ),
            execute: function!(0x38c80, unsafe extern "system-unwind" fn(*mut Wrapper)),
        };
        API.set(api).map_err(|_| ())?;
    }
    ENABLED.store(true, Ordering::Release);
    Ok(())
}

struct Memory;
impl CurrentMemory for Memory {
    fn copy<const N: usize>(&self, address: u64) -> Option<[u8; N]> {
        copy_current(address)
    }
}
#[derive(Clone, PartialEq, Eq)]
struct Account {
    earned: u32,
    spent: u32,
    paid: [u8; 22],
    native: [Option<PromotionSnapshot>; 22],
    definitions: [PaymentDefinition; 22],
}
struct Prepared {
    stamp: LeaseStamp,
    player: u64,
    definition: PaymentDefinition,
    account: Account,
    slot: usize,
}

impl Api {
    unsafe fn parameter_matches(
        &self,
        dictionary: *mut Dictionary,
        key: u32,
        expected: u32,
    ) -> bool {
        if unsafe { (*dictionary).0[0] } != self.dictionary_vtable {
            return false;
        }
        let address = unsafe {
            invoke(
                self.parameter as usize,
                [dictionary as u64, u64::from(key), 0, 0],
            )
        };
        let Some(value) = copy_current::<24>(address) else {
            return false;
        };
        u16::from_le_bytes(value[..2].try_into().unwrap()) == 2
            && u32::from_le_bytes(value[20..24].try_into().unwrap()) & (1 << 29) == 0
            && u32::from_le_bytes(value[8..12].try_into().unwrap()) == expected
    }
    fn account(
        &self,
        stamp: LeaseStamp,
        player: u64,
        definition: PaymentDefinition,
    ) -> Option<(Account, usize)> {
        let binding = PRODUCT_OWNERS[stamp.owner as usize].load();
        let definitions = runtime::ready_catalog(stamp.owner as usize, binding)?;
        let slot = definitions.iter().position(|d| *d == definition)?;
        let governors = unsafe { invoke(self.governors as usize, [player, 0, 0, 0]) };
        if governors == 0
            || copy_current::<8>(governors.checked_add(0x70)?).map(u64::from_le_bytes)
                != Some(player)
        {
            return None;
        }
        let earned = u32::from_le_bytes(copy_current::<4>(governors.checked_add(0x118)?)?);
        let spent = unsafe { invoke(self.spent as usize, [governors, 0, 0, 0]) as u32 };
        let mut result = Account {
            earned,
            spent,
            paid: [0; 22],
            native: [None; 22],
            definitions,
        };
        let mut attributed = 0u32;
        let mut remaining = 0u32;
        for (i, d) in definitions.iter().enumerate() {
            let current = unsafe {
                invoke(
                    self.governor as usize,
                    [governors, u64::from(d.governor_index), 0, 0],
                )
            };
            if current != 0 {
                let snapshot = read_current(&Memory, governors, current).ok()?;
                let paid = snapshot.diagnostic_paid(stamp.owner, *d).ok()?;
                result.native[i] = Some(snapshot);
                result.paid[i] = paid;
                attributed += u32::from(paid);
            }
            if product_permission(
                stamp.owner as usize,
                d.governor_index as usize,
                player + 0x528,
                stamp.session,
            ) == Some(true)
            {
                remaining += u32::from(d.cost - result.paid[i]);
            }
        }
        if spent != attributed
            || earned.checked_sub(spent)? < remaining
            || product_permission(
                stamp.owner as usize,
                definition.governor_index as usize,
                player + 0x528,
                stamp.session,
            ) != Some(true)
        {
            return None;
        }
        Some((result, slot))
    }

    /// Matches native 4A7E80..4A7F34 including constructors/copies/destructors.
    /// Never calls mutation 2DF340, edits flags, or skips the original filter.
    unsafe fn promote(
        &self,
        owner: u32,
        index: u32,
        title: u32,
        still_current: impl Fn() -> bool,
    ) -> bool {
        let mut input = Dictionary([0; 8]);
        let mut op = Operation {
            kind: u32::MAX,
            value_type: 2,
            parameters: Dictionary([0; 8]),
        };
        unsafe {
            invoke(
                self.construct as usize,
                [&mut input as *mut Dictionary as u64, 0, 0, 0],
            );
        }
        let _input = DictionaryGuard::new(&mut input, self);
        unsafe {
            invoke(
                self.construct as usize,
                [&mut op.parameters as *mut Dictionary as u64, 0, 0, 0],
            );
        }
        let _op = DictionaryGuard::new(&mut op.parameters, self);
        unsafe {
            invoke(
                self.integer as usize,
                [
                    &mut input as *mut Dictionary as u64,
                    0xaf5e1625,
                    u64::from(index),
                    0,
                ],
            );
            invoke(
                self.integer as usize,
                [
                    &mut input as *mut Dictionary as u64,
                    0x6d4bd4ea,
                    u64::from(title),
                    0,
                ],
            );
        }
        let made = unsafe {
            invoke(
                self.operation as usize,
                [
                    u64::from(owner),
                    0x7d3da99d,
                    &mut input as *mut Dictionary as u64,
                    &mut op as *mut Operation as u64,
                ],
            ) as u8
        };
        let allowed = made != 0
            && op.value_type == 2
            && op.kind == 0x7d3da99d
            && unsafe { self.parameter_matches(&mut op.parameters, 0xaf5e1625, index) }
            && unsafe { self.parameter_matches(&mut op.parameters, 0x6d4bd4ea, title) }
            && still_current()
            && unsafe {
                invoke(
                    self.filter as usize,
                    [u64::from(owner), &mut op as *mut Operation as u64, 0, 0],
                ) as u8
            } != 0;
        let dispatched = if allowed && still_current() {
            let mut wrapper = Wrapper([0; 13]);
            unsafe {
                invoke(
                    self.wrap as usize,
                    [
                        &mut wrapper as *mut Wrapper as u64,
                        u64::from(owner),
                        &mut op as *mut Operation as u64,
                        0,
                    ],
                );
            }
            let dictionary = unsafe {
                (&mut wrapper as *mut Wrapper)
                    .cast::<u8>()
                    .add(0x28)
                    .cast::<Dictionary>()
            };
            let _wrapper = DictionaryGuard::new(dictionary, self);
            let current = still_current()
                && wrapper.0[3] as u32 == owner
                && wrapper.0[4] as u32 == 0x7d3da99d
                && (wrapper.0[4] >> 32) == 2
                && unsafe { self.parameter_matches(dictionary, 0xaf5e1625, index) }
                && unsafe { self.parameter_matches(dictionary, 0x6d4bd4ea, title) };
            if current {
                unsafe {
                    invoke(
                        self.execute as usize,
                        [&mut wrapper as *mut Wrapper as u64, 0, 0, 0],
                    );
                }
            }
            current
        } else {
            false
        };
        dispatched
    }
}

pub(super) fn before(
    lease: &impl RuntimeLease,
    stamp: LeaseStamp,
    player: u64,
    definition: PaymentDefinition,
) {
    BEFORE.with(|cell| *cell.borrow_mut() = None);
    let Some(_guard) = ActionGuard::enter() else {
        return;
    };
    if !ENABLED.load(Ordering::Acquire)
        || !lease.current(stamp, player)
        || !(2..=3).contains(&definition.cost)
    {
        return;
    }
    let Some(api) = API.get() else { return };
    // This is the exact native AI callback's own precondition (4A7AF7).
    if unsafe { invoke(api.skip_ai as usize, [player, 0, 0, 0]) as u8 } != 0 {
        return;
    }
    let Some((account, slot)) = api.account(stamp, player, definition) else {
        return;
    };
    let paid = account.paid[slot];
    if paid == 0 || paid >= definition.cost || !lease.current(stamp, player) {
        return;
    }
    BEFORE.with(|cell| {
        *cell.borrow_mut() = Some(Prepared {
            stamp,
            player,
            definition,
            account,
            slot,
        })
    });
}
pub(super) fn after(
    lease: &impl RuntimeLease,
    stamp: LeaseStamp,
    player: u64,
    definition: PaymentDefinition,
) {
    let Some(before) = BEFORE.with(|cell| cell.borrow_mut().take()) else {
        return;
    };
    let Some(_guard) = ActionGuard::enter() else {
        return;
    };
    if before.stamp != stamp
        || before.player != player
        || before.definition != definition
        || !ENABLED.load(Ordering::Acquire)
        || !lease.current(stamp, player)
    {
        return;
    }
    let Some(api) = API.get() else { return };
    let Some((now, slot)) = api.account(stamp, player, definition) else {
        return;
    };
    // Native AI already changed any observed account: never add a second OP.
    if now != before.account || slot != before.slot {
        return;
    }
    let Some(title) = definition.titles[now.paid[slot] as usize - 1] else {
        return;
    };
    let dispatched = unsafe {
        api.promote(stamp.owner, definition.governor_index, title, || {
            ENABLED.load(Ordering::Acquire) && lease.current(stamp, player)
        })
    };
    let post = api.account(stamp, player, definition);
    let mut expected = now.clone();
    expected.spent += 1;
    expected.paid[slot] += 1;
    if let Some(own) = expected.native[slot].as_mut() {
        own.owned[title as usize / 64] |= 1 << (title % 64);
    }
    let verified = dispatched
        && lease.current(stamp, player)
        && post
            .as_ref()
            .is_some_and(|(after, position)| *position == slot && *after == expected);
    if let Ok(mut record) = LAST.lock() {
        *record = Some(StepRecord {
            owner: stamp.owner,
            index: definition.governor_index,
            generation: stamp.generation,
            before: now.spent,
            after: post.as_ref().map_or(now.spent, |(a, _)| a.spent),
            title,
            dispatched,
            verified,
        });
    }
    // An unconfirmed dispatch is not retried. A new native proof is required.
    if dispatched && !verified {
        ENABLED.store(false, Ordering::Release);
    }
}

#[cfg(test)]
#[path = "ai_operation_tests.rs"]
mod tests;
