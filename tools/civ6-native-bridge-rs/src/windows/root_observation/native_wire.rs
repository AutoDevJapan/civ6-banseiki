//! A typed Snapshot invocation carries copied wire values into its load.
//! Header turn/sequence are observations, never a World nonce or OS queue cut.
use super::*;
use std::cell::RefCell;
#[path = "native_wire_fixed.rs"]
mod fixed;
struct Fragment {
    begin: u32,
    end: u32,
    unwind: u32,
    hex: &'static str,
}
struct FixedFrame {
    begin: u32,
    end: u32,
    sha: &'static str,
    #[cfg(test)]
    specimen: &'static str,
    parts: &'static [Fragment],
}

static EXE: AtomicU64 = AtomicU64::new(0);
static NEXT: AtomicU64 = AtomicU64::new(0);
static HOOKS: Mutex<Vec<OwnedHook>> = Mutex::new(Vec::new());
#[unsafe(no_mangle)]
static NATIVE_WIRE_CONTINUE: AtomicU64 = AtomicU64::new(0);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Snapshot {
    serial: u64,
    packet: Option<sessions::PacketReceipt>,
    pub session: Option<(
        sessions::SessionLease,
        Option<sessions::SessionWorldReceipt>,
    )>,
    // Decoded native fields. +24 is resolved locally, not a wire World nonce.
    header: [u32; 7],
    owner: u64,
    payload: u64,
    length: u32,
    metadata: u32,
}
impl Snapshot {
    pub(super) fn bootstrap_target(
        self,
        owner: u64,
        session: u64,
    ) -> Option<sessions::BootstrapTarget> {
        if self.owner != owner || !self.matches_session(None) {
            return None;
        }
        sessions::bootstrap_load_target(owner, session, self.packet)
    }
    pub fn matches_session(
        self,
        source: Option<(
            sessions::SessionLease,
            Option<sessions::SessionWorldReceipt>,
        )>,
    ) -> bool {
        self.serial != 0
            && self.header[0] == 0x4e8
            && self.session == source
            && self.packet.is_none_or(|packet| {
                packet.serial != 0
                    && packet.transport != 0
                    && source.is_none_or(|(lease, _)| lease == packet.lease)
            })
    }
}
struct Call {
    serial: u64,
    rsp: u64,
    snapshot: Option<Snapshot>,
}
thread_local! { static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) }; }
#[track_caller]
fn fault() -> ! {
    census::fault(
        census::module(module_path!()),
        std::panic::Location::caller().line(),
    );
    diagnostic(140);
    FAILED.store(true, Ordering::Release);
    if let Some(frames) = FRAMES.get() {
        frames.mark_unresolved();
    }
    std::process::abort()
}
fn value(pointer: u64) -> Option<u64> {
    copy_current::<8>(pointer).map(u64::from_le_bytes)
}
fn word(pointer: u64) -> Option<u32> {
    copy_current::<4>(pointer).map(u32::from_le_bytes)
}
fn capture(message: u64, session: u64, serial: u64) -> Option<Snapshot> {
    let exe = EXE.load(Ordering::Acquire);
    if value(message)? != exe + 0x10417e8 || word(message + 0x18)? != 0x4e8 {
        return None;
    }
    let owner = value(session.checked_add(0x18)?)?;
    if value(owner)? != exe + 0xfd35c8 || value(owner.checked_add(8)?)? != session {
        return None;
    }
    let header = [0x18, 0x1c, 0x20, 0x24, 0x28, 0x2c, 0x30]
        .map(|offset| word(message + offset))
        .into_iter()
        .collect::<Option<Vec<_>>>()?;
    let start = value(message + 0x40)?;
    let end = value(message + 0x48)?;
    let length = word(message + 0x58)?;
    if start == 0 || end <= start || length == 0 || u64::from(length) > end - start {
        return None;
    }
    let packet = sessions::current_packet();
    let issued_world = sessions::admitted_source_world(session)
        .filter(|world| packet.is_none_or(|packet| packet.world == Some(*world)));
    let relation = sessions::snapshot_owner(owner, session)
        .filter(|(_, previous)| previous.is_some() && *previous == issued_world);
    Some(Snapshot {
        serial,
        packet,
        session: relation,
        header: header.try_into().ok()?,
        owner,
        payload: start,
        length,
        metadata: word(message + 0x38)?,
    })
}
/// Only the original80F291 virtual call may pass this invocation's values.
/// No saved native pointer is dereferenced after its Snapshot frame returns.
pub(super) fn request(raw: &Saved) -> Option<Snapshot> {
    if raw.invocation.caller != EXE.load(Ordering::Acquire) + 0x80f297 {
        return None;
    }
    CALLS.with(|calls| {
        let calls = calls.borrow();
        let snapshot = calls.last()?.snapshot?;
        if snapshot.owner != raw.gpr[0]
            || snapshot.payload != raw.gpr[1]
            || snapshot.length != raw.gpr[2] as u32
            || snapshot.metadata != raw.gpr[3] as u32
        {
            fault();
        }
        Some(snapshot)
    })
}
extern "system" fn begin(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    if std::panic::catch_unwind(|| {
        let raw = unsafe { &mut *raw };
        let serial = NEXT
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |id| id.checked_add(1))
            .unwrap_or_else(|_| fault())
            + 1;
        let snapshot = capture(raw.gpr[0], raw.gpr[1], serial);
        CALLS.with(|calls| {
            calls.borrow_mut().push(Call {
                serial,
                rsp: raw.invocation.caller_rsp,
                snapshot,
            })
        });
        raw.invocation.token = serial;
        raw.invocation.cookie = COOKIE;
    })
    .is_err()
    {
        fault();
    }
    unsafe { SetLastError(error) };
}
fn finish(raw: &mut Saved, exceptional: bool) {
    if exceptional {
        census::terminal(census::module(module_path!()), raw.invocation.kind);
    }
    if raw.invocation.cookie != COOKIE {
        return;
    }
    raw.invocation.cookie = 0;
    let call = CALLS
        .with(|calls| calls.borrow_mut().pop())
        .unwrap_or_else(|| fault());
    if call.serial != raw.invocation.token || call.rsp != raw.invocation.caller_rsp {
        fault();
    }
    if exceptional && call.snapshot.is_some() {
        FAILED.store(true, Ordering::Release);
        if let Some(frames) = FRAMES.get() {
            frames.mark_unresolved();
        }
    }
}
extern "system" fn end(raw: *mut Saved) {
    let error = unsafe { GetLastError() };
    if std::panic::catch_unwind(|| finish(unsafe { &mut *raw }, false)).is_err() {
        fault();
    }
    unsafe { SetLastError(error) };
}
unsafe extern "system" fn unwind(
    record: *const EXCEPTION_RECORD,
    frame: usize,
    _: *mut CONTEXT,
    _: *mut c_void,
) -> i32 {
    let error = unsafe { GetLastError() };
    if !record.is_null()
        && unsafe { (*record).ExceptionFlags } & (2 | 4 | 0x20 | 0x40) != 0
        && std::panic::catch_unwind(|| {
            finish(unsafe { &mut *((frame + 0x20) as *mut Saved) }, true)
        })
        .is_err()
    {
        fault();
    }
    unsafe { SetLastError(error) };
    1
}
global_asm!(
    ".globl native_wire_snapshot_original",
    ".seh_proc native_wire_snapshot_original",
    "native_wire_snapshot_original:",
    "mov [rsp+0x10],rsi",
    ".seh_savereg rsi,0x10",
    ".seh_endprologue",
    "mov [rsp+8],rax",
    "mov rax,[rip+NATIVE_WIRE_CONTINUE]",
    "mov [rsp+0x18],rax",
    "mov rax,[rsp+8]",
    "jmp qword ptr [rsp+0x18]",
    ".seh_endproc",
);
unsafe extern "system" {
    fn native_wire_snapshot_original();
    fn native_wire_snapshot_gateway();
}
gateway!(
    "native_wire_snapshot_gateway",
    "140",
    native_wire_snapshot_original,
    begin,
    end,
    unwind
);
const ORIGINAL: [u8; 8] = [0x48, 0x89, 0x74, 0x24, 0x10, 0x57, 0x48, 0x83];
pub(super) unsafe fn prepare_host(module: usize) -> Result<(), &'static str> {
    let _pin = super::super::pin(GAME_EXE, civ6_bootstrap_rs::EXE_SHA)?;
    unsafe {
        fixed::verify(module)?;
        install_at(module + 0x80f230)
    }
}
unsafe fn install_at(address: usize) -> Result<(), &'static str> {
    let mut hooks = HOOKS.lock().map_err(|_| "wire hook owner")?;
    if !hooks.is_empty() || copy_current::<8>(address as u64) != Some(ORIGINAL) {
        return Err("wire entry differs or owned");
    }
    let target = address as *mut u8;
    let mut info: MEMORY_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    if !address.is_multiple_of(8)
        || unsafe { VirtualQuery(target.cast(), &mut info, std::mem::size_of_val(&info)) } == 0
        || info.State != MEM_COMMIT
        || info.Protect & 0xff != PAGE_EXECUTE_READ
        || info.Protect & PAGE_GUARD != 0
    {
        return Err("wire entry protection");
    }
    let relay = unsafe { executable_near(address, address, 5) }.ok_or("wire relay")?;
    let mut unpublished = Unpublished(relay);
    unsafe {
        ptr::copy_nonoverlapping([0xff, 0x25, 0, 0, 0, 0].as_ptr(), relay, 6);
        ptr::copy_nonoverlapping(
            (native_wire_snapshot_gateway as *const () as u64)
                .to_le_bytes()
                .as_ptr(),
            relay.add(6),
            8,
        );
    }
    let mut old = 0;
    if unsafe { VirtualProtect(relay.cast(), CODE_PAGE, PAGE_EXECUTE_READ, &mut old) } == 0
        || unsafe { FlushInstructionCache(GetCurrentProcess(), relay.cast(), 14) } == 0
    {
        return Err("wire relay publication");
    }
    let mut patched = ORIGINAL;
    patched[..5].copy_from_slice(&rel32_jump(address, relay as usize).ok_or("wire relay range")?);
    NATIVE_WIRE_CONTINUE.store((address + 5) as u64, Ordering::Release);
    hooks.push(OwnedHook {
        hook: Hook {
            target,
            original: ORIGINAL,
            patched,
            prefix: None,
            _code: relay,
        },
        protection: info.Protect,
    });
    unpublished.0 = null_mut();
    EXE.store((address - 0x80f230) as u64, Ordering::Release);
    let result = unsafe {
        exchange_entry(
            target,
            u64::from_le_bytes(ORIGINAL),
            u64::from_le_bytes(patched),
        )
    }?;
    if !result.exchanged || !result.protection_restored || !result.cache_flushed {
        return Err("critical: wire publication");
    }
    Ok(())
}
pub(super) unsafe fn restore() -> Result<(), &'static str> {
    let mut hooks = HOOKS.lock().map_err(|_| "wire hook owner")?;
    hooks.retain(|owned| {
        let h = &owned.hook;
        let current = copy_current::<8>(h.target as u64);
        let restored = current == Some(h.original)
            || (current == Some(h.patched)
                && unsafe {
                    exchange_entry(
                        h.target,
                        u64::from_le_bytes(h.patched),
                        u64::from_le_bytes(h.original),
                    )
                }
                .is_ok_and(|r| r.exchanged && r.protection_restored && r.cache_flushed)
                && copy_current::<8>(h.target as u64) == Some(h.original));
        let mut old = 0;
        !(restored
            && unsafe { VirtualProtect(h.target.cast(), 8, owned.protection, &mut old) } != 0
            && unsafe { FlushInstructionCache(GetCurrentProcess(), h.target.cast(), 8) } != 0)
    });
    if hooks.is_empty() {
        Ok(())
    } else {
        Err("critical: wire restoration")
    }
}
#[cfg(test)]
#[path = "native_wire_tests.rs"]
mod tests;
