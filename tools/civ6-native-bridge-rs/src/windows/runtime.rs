//! Explicit process-lifetime owner of appointment, property, assignment and
//! the two pinned natural CanPromote callers.
//! No timer uninstalls it. The single finite observation may expire or stop;
//! its allocation, gateways and authority remain resident until process exit.
//! Close/rebuild exports are control boundaries, NOT an observed reload hook.
//! Pinned synchronous native frames are tracked separately from bridge calls;
//! neither count certifies pending publication or all native dispatch domains.
use super::*;
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN,
};

pub const RUNTIME_CONFIG_VERSION: u32 = 0x0324_0002;
// Relocated data exports identify the exact gateway for each entry. Read
// their values in the loaded DLL, not a DONT_RESOLVE_DLL_REFERENCES image.
#[unsafe(no_mangle)]
pub static BRIDGE_RUNTIME_APPOINTMENT_GATEWAY: unsafe extern "system" fn() = calibration_gateway;
#[unsafe(no_mangle)]
pub static BRIDGE_RUNTIME_PROPERTY_GATEWAY: unsafe extern "system" fn() = property_gateway;
#[unsafe(no_mangle)]
pub static BRIDGE_RUNTIME_ASSIGNMENT_GATEWAY: unsafe extern "system" fn() =
    assignment::assign_calibration_gateway;
#[unsafe(no_mangle)]
pub static BRIDGE_RUNTIME_PROMOTION_GATEWAY: unsafe extern "system" fn() =
    promotion::runtime_promotion_gateway;
static OWNED: AtomicBool = AtomicBool::new(false);
static INSTALLED: AtomicBool = AtomicBool::new(false);
static INVOCATIONS: crate::invocation_gate::InvocationGate =
    crate::invocation_gate::InvocationGate::new();
static INVOCATION_FAILURE: AtomicBool = AtomicBool::new(false);
const INVOCATION_REFUSED: u64 = 1 << 63;
static NEXT_INVOCATION: AtomicU64 = AtomicU64::new(1);
struct PendingInvocation {
    cookie: u64,
    generation: u32,
    native_hold: Option<u64>,
}
thread_local! {
    static PENDING_INVOCATIONS: std::cell::RefCell<Vec<PendingInvocation>> = const { std::cell::RefCell::new(Vec::new()) };
}

// 0 is a refused runtime call; 1 is the existing non-runtime diagnostic path.
// Higher cookies hold a single generation until normal return or SEH cleanup.
pub(super) fn invocation_start() -> u64 {
    if !owned() {
        return 1;
    }
    if !INSTALLED.load(Ordering::Acquire) {
        return 0;
    }
    if root_observation::native_frames::unproven_input_active() {
        return 0;
    }
    let Some(generation) = INVOCATIONS.enter() else {
        return 0;
    };
    let Ok(native_hold) = root_observation::native_frames::enter_bridge() else {
        let _ = INVOCATIONS.leave(generation);
        return 0;
    };
    let cookie = NEXT_INVOCATION.fetch_update(Ordering::AcqRel, Ordering::Acquire, |id| {
        id.checked_add(1).filter(|next| *next < INVOCATION_REFUSED)
    });
    let Ok(previous) = cookie else {
        let _ = root_observation::native_frames::leave_bridge(native_hold);
        let _ = INVOCATIONS.leave(generation);
        INVOCATION_FAILURE.store(true, Ordering::Release);
        return 0;
    };
    let cookie = previous + 1;
    PENDING_INVOCATIONS.with(|pending| {
        pending.borrow_mut().push(PendingInvocation {
            cookie,
            generation,
            native_hold,
        })
    });
    cookie
}
pub(super) fn invocation_admitted(cookie: u64) -> bool {
    cookie != 0
        && cookie & INVOCATION_REFUSED == 0
        && (!owned()
            || (!root_observation::native_frames::unproven_input_active()
                && PENDING_INVOCATIONS
                    .with(|pending| pending.borrow().iter().any(|call| call.cookie == cookie))))
}
pub(super) unsafe fn invocation_refuse(cookie: *const AtomicU64) {
    if let Some(cookie) = unsafe { cookie.as_ref() } {
        if cookie.load(Ordering::Acquire) != 0 {
            cookie.fetch_or(INVOCATION_REFUSED, Ordering::AcqRel);
        }
    }
}
#[cfg(test)]
pub(super) fn fixture_invocation_runtime() {
    OWNED.store(true, Ordering::Release);
    assert!(product_activate());
    assert!(start_observation(30));
}
#[cfg(test)]
pub(super) fn fixture_invocation_closed() -> bool {
    INVOCATIONS.closed_for_test()
}
#[cfg(test)]
pub(super) fn fixture_product_inactive() -> bool {
    !PRODUCT_ACTIVE.load(Ordering::Acquire)
}
pub(super) fn inactive_and_drained() -> bool {
    OWNED.load(Ordering::Acquire)
        && INSTALLED.load(Ordering::Acquire)
        && !PRODUCT_ACTIVE.load(Ordering::Acquire)
        && INVOCATIONS.drained()
        && !INVOCATION_FAILURE.load(Ordering::Acquire)
}
#[cfg(test)]
pub(super) fn fixture_pending_invocations() -> u32 {
    INVOCATIONS.pending_for_test()
}
pub(super) unsafe fn invocation_finish(cookie: *const AtomicU64) {
    let Some(cookie) = (unsafe { cookie.as_ref() }) else {
        return;
    };
    let held = cookie.swap(0, Ordering::AcqRel) & !INVOCATION_REFUSED;
    if held <= 1 {
        return;
    }
    let pending = PENDING_INVOCATIONS.with(|pending| {
        let mut pending = pending.borrow_mut();
        let index = pending.iter().position(|call| call.cookie == held)?;
        Some(pending.remove(index))
    });
    let success = pending.is_some_and(|call| {
        let native = root_observation::native_frames::leave_bridge(call.native_hold);
        let bridge = INVOCATIONS.leave(call.generation);
        native && bridge
    });
    if !success {
        INVOCATION_FAILURE.store(true, Ordering::Release);
        INVOCATIONS.close();
        product_deactivate();
    }
}
pub(super) unsafe fn invocation_unwind(
    record: *const windows_sys::Win32::System::Diagnostics::Debug::EXCEPTION_RECORD,
    cookie: *const AtomicU64,
) {
    if !record.is_null() && unsafe { (*record).ExceptionFlags } & (2 | 4 | 0x20 | 0x40) != 0 {
        unsafe { invocation_finish(cookie) };
    }
}
// One atomic word orders admission against closure. A successful stop cannot
// miss a writer between a separate enabled check and counter increment.
static SAMPLE_GATE: civ6_bootstrap_rs::observation::ObservationGate =
    civ6_bootstrap_rs::observation::ObservationGate::new(false);
static DEADLINE_MS: AtomicU64 = AtomicU64::new(0);
// Reset stamp and invocation generation change in ONE cmpxchg16b. A reset
// paused after its phase CAS cannot advance generation after a newer reset.
static EPOCH: AtomicJoinedWord = AtomicJoinedWord::new();
// One CAS value carries binding(session27/ticket31), latest transition ticket31,
// transaction-start ticket31 and phase2. Owner is the array index; the exact
// receiver is revalidated against PRODUCT_OWNERS before every CAS.
static STATE: [AtomicJoinedWord; PRODUCTION_OWNER_LIMIT] =
    [const { AtomicJoinedWord::new() }; PRODUCTION_OWNER_LIMIT];
pub(super) static INSTALL_LOCK: Mutex<()> = Mutex::new(());
#[cfg(test)]
pub(super) static RESTORE_HOOK_REACHED: AtomicBool = AtomicBool::new(false);
const BIND_MASK: u128 = (1u128 << 58) - 1;
const TICKET_MASK: u128 = (1u128 << 31) - 1;

fn stamp(session: u32, ticket: u32) -> u64 {
    (u64::from(session) << 31) | u64::from(ticket)
}
fn binding_stamp(binding: u128) -> u128 {
    u128::from(stamp(joined_session(binding), joined_ticket(binding)))
}
fn state_word(binding: u128, ticket: u32, begin: u32, phase: u32) -> u128 {
    binding_stamp(binding)
        | (u128::from(ticket) << 58)
        | (u128::from(begin) << 89)
        | (u128::from(phase) << 120)
}
fn state_ticket(state: u128) -> u32 {
    ((state >> 58) & TICKET_MASK) as u32
}
fn state_begin(state: u128) -> u32 {
    ((state >> 89) & TICKET_MASK) as u32
}
fn state_phase(state: u128) -> u32 {
    ((state >> 120) & 3) as u32
}
fn live_session(session: u32) -> bool {
    PRODUCT_ACTIVE.load(Ordering::Acquire)
        && !PRODUCT_EXHAUSTED.load(Ordering::Acquire)
        && PRODUCT_SESSION.load(Ordering::Acquire) == session
}
pub(super) fn owned() -> bool {
    OWNED.load(Ordering::Acquire)
}
pub(super) fn generation() -> u32 {
    if owned() {
        joined_session(EPOCH.load())
    } else {
        0
    }
}
/// Invocation-local fence, independent of telemetry and the delayed epoch
/// callback. Numeric receiver identity is used only during this invocation.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub(super) struct CallFence {
    pub session: u32,
    pub ticket: u32,
    pub owner: u32,
    pub read_state: u32,
    pub receiver: u64,
}
const _: () = assert!(std::mem::size_of::<CallFence>() == 24);

fn call_owner(governors: u64) -> Option<(u32, u64)> {
    let player = u64::from_le_bytes(copy_current::<8>(governors.checked_add(0x70)?)?);
    if player == 0 {
        return None;
    }
    let owner = u32::from_le_bytes(copy_current::<4>(player.checked_add(0xd8)?)?);
    let receiver = player.checked_add(0x528)?;
    (owner < PRODUCTION_OWNER_LIMIT as u32 && receiver & 7 == 0).then_some((owner, receiver))
}

pub(super) fn call_begin(governors: u64) -> CallFence {
    if !owned() || root_observation::native_frames::unproven_input_active() {
        return CallFence::default();
    }
    let session = PRODUCT_SESSION.load(Ordering::Acquire);
    let ticket = if live_session(session) {
        product_next_ticket().unwrap_or(0)
    } else {
        0
    };
    let Some((owner, receiver)) = call_owner(governors) else {
        return CallFence {
            session,
            ticket,
            ..CallFence::default()
        };
    };
    CallFence {
        session,
        ticket,
        owner,
        read_state: 1,
        receiver,
    }
}

pub(super) fn call_current(fence: CallFence, governors: u64) -> bool {
    if !owned() {
        return true;
    }
    if root_observation::native_frames::unproven_input_active()
        || fence.read_state != 1
        || fence.ticket == 0
        || !live_session(fence.session)
        || call_owner(governors) != Some((fence.owner, fence.receiver))
    {
        return false;
    }
    let owner = fence.owner as usize;
    let binding = PRODUCT_OWNERS[owner].load();
    let snapshot = STATE[owner].load();
    state_phase(snapshot) == 2
        && snapshot & BIND_MASK == binding_stamp(binding)
        && joined_session(binding) == fence.session
        && joined_receiver(binding) == fence.receiver
        && joined_ticket(binding) <= fence.ticket
        && state_begin(snapshot) <= fence.ticket
        && state_ticket(snapshot) <= fence.ticket
        && STATE[owner].load() == snapshot
        && PRODUCT_OWNERS[owner].load() == binding
        && live_session(fence.session)
}

#[derive(Clone, Copy)]
struct Fence {
    owner: usize,
    binding: u128,
    ticket: u32,
}
impl Fence {
    fn live(self) -> bool {
        self.owner < PRODUCTION_OWNER_LIMIT
            && live_session(joined_session(self.binding))
            && PRODUCT_OWNERS[self.owner].load() == self.binding
    }
}
fn advance() -> bool {
    advance_epoch(None)
}
fn advance_epoch(fence: Option<(Fence, u128)>) -> bool {
    loop {
        let current = EPOCH.load();
        let latest_stamp = if let Some((fence, expected)) = fence {
            if !fence.live() || STATE[fence.owner].load() != expected {
                return false;
            }
            joined_receiver(current).max(stamp(joined_session(fence.binding), fence.ticket))
        } else {
            joined_receiver(current)
        };
        let Some(generation) = joined_session(current).checked_add(1) else {
            product_deactivate();
            for state in &STATE {
                state.store(0);
            }
            return false;
        };
        if EPOCH
            .compare_exchange(current, joined_word(latest_stamp, generation, 0, false))
            .is_ok()
        {
            return true;
        }
    }
}

// STATE is the sole runtime transaction authority. Commit acceptance, begin,
// latest ticket and phase change in the SAME CAS. Legacy COMMIT is not read.
#[derive(Clone, Copy)]
enum Update {
    Reset,
    False,
    Invalid,
    True,
}
fn transition(fence: Fence, update: Update) -> bool {
    loop {
        let state = STATE[fence.owner].load();
        let incoming = binding_stamp(fence.binding);
        if !fence.live()
            || state & BIND_MASK > incoming
            || (state & BIND_MASK == incoming && state_ticket(state) >= fence.ticket)
        {
            return false;
        }
        let (begin, phase) = match update {
            Update::Reset => (0, 3),
            Update::False => (fence.ticket, 1),
            Update::Invalid => (0, 0),
            Update::True => {
                let begin = state_begin(state);
                let valid = state & BIND_MASK == incoming
                    && begin > joined_ticket(fence.binding)
                    && state_phase(state) == 1
                    && composite::copy_catalog(fence.owner, fence.binding, begin, fence.ticket)
                        .is_some();
                if valid {
                    (begin, 2)
                } else {
                    (0, 0)
                }
            }
        };
        // Catalog copying is bounded and uses no saved pointer. CAS below
        // rechecks the exact transaction snapshot used for that entire copy.
        if !fence.live() {
            return false;
        }
        let accepted = state_word(fence.binding, fence.ticket, begin, phase);
        if STATE[fence.owner]
            .compare_exchange(state, accepted)
            .is_err()
        {
            continue;
        }
        #[cfg(test)]
        if matches!(update, Update::Reset) && RESET_STAGE.load(Ordering::Acquire) == 2 {
            pause_reset();
        }
        #[cfg(test)]
        if !matches!(update, Update::Reset) {
            composite::pause_commit(2);
            composite::pause_commit(3);
        }
        if phase == 2 {
            return true;
        }
        if !advance_epoch(Some((fence, accepted))) {
            return false;
        }
        if matches!(update, Update::Reset) {
            return STATE[fence.owner]
                .compare_exchange(accepted, state_word(fence.binding, fence.ticket, 0, 1))
                .is_ok();
        }
        return true;
    }
}

pub(super) fn reset_owner(owner: usize, receiver: u64, session: u32, ticket: u32) -> bool {
    if !owned() || owner >= PRODUCTION_OWNER_LIMIT || !live_session(session) {
        return false;
    }
    let binding = joined_word(receiver, session, ticket, false);
    loop {
        let current = PRODUCT_OWNERS[owner].load();
        if !live_session(session) || binding_stamp(current) >= binding_stamp(binding) {
            return false;
        }
        if PRODUCT_OWNERS[owner]
            .compare_exchange(current, binding)
            .is_ok()
        {
            break;
        }
    }
    transition(
        Fence {
            owner,
            binding,
            ticket,
        },
        Update::Reset,
    )
}

pub(super) fn accept_commit(
    owner: usize,
    binding: u128,
    ticket: u32,
    permitted: bool,
    invalid: bool,
) {
    transition(
        Fence {
            owner,
            binding,
            ticket,
        },
        if invalid {
            Update::Invalid
        } else if permitted {
            Update::True
        } else {
            Update::False
        },
    );
}

pub(super) fn ready_catalog(
    owner: usize,
    binding: u128,
) -> Option<[crate::governor_payment::PaymentDefinition; crate::paid_catalog::COUNT]> {
    if owner >= PRODUCTION_OWNER_LIMIT
        || !live_session(joined_session(binding))
        || PRODUCT_OWNERS[owner].load() != binding
    {
        return None;
    }
    let snapshot = STATE[owner].load();
    if snapshot & BIND_MASK != binding_stamp(binding) || state_phase(snapshot) != 2 {
        return None;
    }
    let catalog = composite::copy_catalog(
        owner,
        binding,
        state_begin(snapshot),
        state_ticket(snapshot),
    )?;
    (STATE[owner].load() == snapshot
        && live_session(joined_session(binding))
        && PRODUCT_OWNERS[owner].load() == binding)
        .then_some(catalog)
}

pub(super) fn payment_after_catalog(owner: usize, ticket: u32) -> bool {
    let binding = PRODUCT_OWNERS[owner].load();
    let snapshot = STATE[owner].load();
    state_phase(snapshot) == 2
        && snapshot & BIND_MASK == binding_stamp(binding)
        && state_ticket(snapshot) < ticket
        && live_session(joined_session(binding))
        && PRODUCT_OWNERS[owner].load() == binding
        && STATE[owner].load() == snapshot
}

/// The closed bit and active writer count share a single linearization point.
/// No hook waits, allocates or takes a lock. Rows are never reset or redirected.
pub(super) fn sample_begin() -> bool {
    if !owned() {
        return true;
    }
    // A callback entering during installation belongs to the unpublished
    // phase. It may finish after initialization, but may not expire or enter
    // that later observation using the old, zero deadline.
    let installed = INSTALLED.load(Ordering::Acquire);
    #[cfg(test)]
    if SAMPLE_PUBLISH_PAUSE.swap(false, Ordering::AcqRel) {
        SAMPLE_PUBLISH_PAUSED.store(true, Ordering::Release);
        while !SAMPLE_PUBLISH_RELEASE.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
    }
    if !installed {
        return false;
    }
    let now = unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() };
    if now >= DEADLINE_MS.load(Ordering::Acquire) {
        SAMPLE_GATE.close();
    }
    SAMPLE_GATE.try_acquire()
}

#[cfg(test)]
pub(super) static SAMPLE_PUBLISH_PAUSE: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
pub(super) static SAMPLE_PUBLISH_PAUSED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
pub(super) static SAMPLE_PUBLISH_RELEASE: AtomicBool = AtomicBool::new(false);

pub(super) fn sample_end() {
    if owned() {
        SAMPLE_GATE.release();
    }
}

pub(super) fn next_slot(next: &AtomicU32, capacity: usize) -> usize {
    if !owned() {
        return next.fetch_add(1, Ordering::AcqRel) as usize;
    }
    next.fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
        (n < capacity as u32).then_some(n + 1)
    })
    .map_or(capacity, |n| n as usize)
}

fn stop_observation() -> BridgeStatus {
    if !INSTALLED.load(Ordering::Acquire) {
        return BridgeStatus::InvalidInput;
    }
    SAMPLE_GATE.close();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while !SAMPLE_GATE.drained() {
        if std::time::Instant::now() >= deadline {
            return BridgeStatus::Critical;
        }
        std::thread::yield_now();
    }
    BridgeStatus::Ok
}

/// The complete raw property log is committed and touched before any hook
/// exists. Its failure refuses installation; there is no 4096-row fallback.
pub(super) fn allocate_trace() -> Option<*mut CompositeTrace> {
    property_log::publish()?;
    let trace = Box::into_raw(CompositeTrace::boxed());
    OWNED.store(true, Ordering::Release);
    advance();
    Some(trace)
}

#[cfg(test)]
pub(super) static RESET_PAUSE: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
pub(super) static RESET_STAGE: AtomicU32 = AtomicU32::new(1);
#[cfg(test)]
pub(super) static RESET_PAUSED: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
pub(super) static RESET_RELEASE: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
pub(super) fn pause_reset_after_ticket() {
    if RESET_STAGE.load(Ordering::Acquire) == 1 {
        pause_reset();
    }
}

#[cfg(test)]
fn pause_reset() {
    if RESET_PAUSE
        .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        RESET_PAUSED.store(true, Ordering::Release);
        while !RESET_RELEASE.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
    }
}

pub(super) fn start_observation(seconds: u32) -> bool {
    let now = unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() };
    DEADLINE_MS.store(now + u64::from(seconds) * 1000, Ordering::Release);
    SAMPLE_GATE.initialize_before_publish();
    // Publish stoppability only after opening the observation. Otherwise a
    // concurrent stop could succeed against the initial closed word and then
    // be undone by this store of zero.
    INSTALLED.store(true, Ordering::Release);
    if INVOCATIONS.open_next().is_none() {
        INSTALLED.store(false, Ordering::Release);
        product_deactivate();
        return false;
    }
    true
}

#[cfg(test)]
pub(super) fn expire_observation() {
    DEADLINE_MS.store(0, Ordering::Release);
}

#[cfg(test)]
pub(super) fn phase(owner: usize) -> u32 {
    state_phase(STATE[owner].load())
}

#[cfg(test)]
pub(super) fn state_for_test(owner: usize) -> u128 {
    STATE[owner].load()
}

#[cfg(test)]
pub(super) fn state_for_authority(owner: usize) -> u128 {
    STATE[owner].load()
}

/// Cold Host preparation precedes Core/Context creation. It captures native
/// session birth; it does not activate gameplay policy or create World proof.
#[unsafe(no_mangle)]
pub extern "system" fn BridgeRuntimePrepareHost(input: *mut c_void) -> u32 {
    if !committed_range(
        input as usize,
        std::mem::size_of::<BridgeConfig>(),
        std::mem::align_of::<BridgeConfig>(),
        false,
    ) {
        return BridgeStatus::InvalidInput as u32;
    }
    let config = unsafe { ptr::read(input.cast::<BridgeConfig>()) };
    if config.version != RUNTIME_CONFIG_VERSION
        || config.size as usize != std::mem::size_of::<BridgeConfig>()
        || config.mode != BridgeMode::ProductionAdmission as u32
        || config.trace_address != 0
        || config.trace_size != 0
        || !(1..=MAX_ACTIVE_SECONDS).contains(&config.max_seconds)
        || config.game_pid != unsafe { GetCurrentProcessId() }
        || current_created_filetime() != Some(config.created_filetime)
        || !module_path(null_mut()).is_some_and(|path| same_file(&path, GAME_EXE))
        || !unsafe { GetModuleHandleW(wide("GameCore_XP2_FinalRelease.dll").as_ptr()) }.is_null()
    {
        return BridgeStatus::InvalidInput as u32;
    }
    let Ok(_guard) = INSTALL_LOCK.lock() else {
        return BridgeStatus::Panic as u32;
    };
    if owned() {
        return BridgeStatus::HookFailure as u32;
    }
    guard_host_prepare(|| unsafe {
        root_observation::native_frames::prepare_host(GetModuleHandleW(null_mut()) as usize)
    })
}

// The install lock remains held across both preparation and rollback. Early
// Host hooks can already be published even though Core installation has not
// set OWNED. Restore every owner; a failed compare retains its relay/record.
fn guard_host_prepare(
    prepare: impl FnOnce() -> Result<(), &'static str> + std::panic::UnwindSafe,
) -> u32 {
    guard_host_prepare_for_cpu(supports_atomic_promotion_fetch(), prepare)
}

fn guard_host_prepare_for_cpu(
    cpu_gate: bool,
    prepare: impl FnOnce() -> Result<(), &'static str> + std::panic::UnwindSafe,
) -> u32 {
    // Host gateways use the same asynchronous eight-byte publication contract
    // as Core installation. Refuse before the first Host entry can change.
    if !cpu_gate {
        return BridgeStatus::InvalidInput as u32;
    }
    if root_observation::native_frames::host_prepared() {
        // A repeated request does not own the earlier successful transaction.
        return BridgeStatus::HookFailure as u32;
    }
    match std::panic::catch_unwind(prepare) {
        Ok(Ok(())) => BridgeStatus::Ok as u32,
        _ => {
            product_deactivate();
            let _restored =
                std::panic::catch_unwind(|| unsafe { root_observation::native_frames::restore() });
            // Preparation still failed. No rollback result may grant policy
            // authority, and unrecovered owners remain available to cleanup.
            BridgeStatus::Critical as u32
        }
    }
}

/// Config is also a receipt: success returns the game-owned trace address.
/// Neither an external mapping nor a saved GameCore pointer becomes authority.
#[unsafe(no_mangle)]
pub extern "system" fn BridgeRuntimeInstall(input: *mut c_void) -> u32 {
    if !committed_range(
        input as usize,
        std::mem::size_of::<BridgeConfig>(),
        std::mem::align_of::<BridgeConfig>(),
        true,
    ) {
        return BridgeStatus::InvalidInput as u32;
    }
    guard_runtime_install(|| {
        let mut config = unsafe { ptr::read(input.cast::<BridgeConfig>()) };
        if owned()
            || config.version != RUNTIME_CONFIG_VERSION
            || config.size as usize != std::mem::size_of::<BridgeConfig>()
            || config.mode != BridgeMode::ProductionAdmission as u32
            || config.trace_address != 0
            || config.trace_size != 0
            || !(1..=MAX_ACTIVE_SECONDS).contains(&config.max_seconds)
            || !supports_atomic_promotion_fetch()
        {
            return BridgeStatus::InvalidInput as u32;
        }
        // Refuse an existing bounded owner before changing global policy or
        // trace ownership. Its ordinary restoration contract still applies.
        if PROPERTY_USED.load(Ordering::Acquire)
            || ASSIGN_USED.load(Ordering::Acquire)
            || PROMOTION_USED.load(Ordering::Acquire)
            || promotion::used()
            || HOOK.lock().map_or(true, |hook| hook.is_some())
            || JOINED_PROPERTY_HOOK
                .lock()
                .map_or(true, |hook| hook.is_some())
            || composite::ASSIGNMENT_HOOK
                .lock()
                .map_or(true, |hook| hook.is_some())
            || PROMOTION_HOOK.lock().map_or(true, |hook| hook.is_some())
        {
            return BridgeStatus::HookFailure as u32;
        }
        if unsafe { GetCurrentProcessId() } != config.game_pid
            || current_created_filetime() != Some(config.created_filetime)
            || !module_path(null_mut()).is_some_and(|path| same_file(&path, GAME_EXE))
        {
            return BridgeStatus::ProcessIdentity as u32;
        }
        // Relay trampolines return into this exact image. Pin its verified
        // loader module as well as the bridge; ordinary FreeLibrary cannot
        // unmap it while a gateway or an already-fetched jump is executing.
        let gamecore = unsafe { GetModuleHandleW(wide("GameCore_XP2_FinalRelease.dll").as_ptr()) };
        if gamecore.is_null()
            || !module_path(gamecore).is_some_and(|path| same_file(&path, GAMECORE))
            || pinned_gamecore(Path::new(GAMECORE), BridgeMode::ProductionAdmission).is_none()
            || !promotion::verify(gamecore as usize)
        {
            return BridgeStatus::GameCoreIdentity as u32;
        }
        let mut pinned_core = null_mut();
        if unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                gamecore.cast(),
                &mut pinned_core,
            )
        } == 0
            || pinned_core != gamecore
        {
            return BridgeStatus::GameCoreIdentity as u32;
        }
        // Pin before publishing any executable relay. A fetched jump may live
        // beyond restoration even when installation fails halfway through.
        let mut module = null_mut();
        if unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                BridgeRuntimeInstall as *const () as *const u16,
                &mut module,
            )
        } == 0
        {
            return BridgeStatus::HookFailure as u32;
        }
        let Some(trace) = allocate_trace() else {
            return BridgeStatus::HookFailure as u32;
        };
        // Restrict both promotion callers before publishing the property/
        // appointment/assignment composite or enabling its session. While the
        // catalog is absent, all original true promotion results stay closed.
        if let Err(reason) = unsafe { promotion::install(gamecore as usize) } {
            product_deactivate();
            return if reason.starts_with("critical:") {
                BridgeStatus::Critical
            } else {
                BridgeStatus::HookFailure
            } as u32;
        }
        if unsafe { root_observation::native_frames::install(gamecore as usize) }.is_err() {
            product_deactivate();
            let restored = unsafe { root_observation::native_frames::restore() }.is_ok();
            let promotion_restored = unsafe { promotion::restore() }.is_ok();
            return if restored && promotion_restored {
                BridgeStatus::HookFailure
            } else {
                BridgeStatus::Critical
            } as u32;
        }
        if !root_observation::native_frames::bind_host(gamecore as usize) {
            product_deactivate();
            return BridgeStatus::Critical as u32;
        }
        config.version = EXPERIMENTAL_PAID_CONFIG_VERSION;
        config.trace_address = trace as u64;
        config.trace_size = std::mem::size_of::<CompositeTrace>() as u32;
        let status = guarded_install(config);
        if !matches!(status, BridgeStatus::Ok) {
            // Storage stays alive for any already-fetched relay. No retry may
            // repoint it. guarded_install/composite restore the installed set.
            product_deactivate();
            let native_restored = unsafe { root_observation::native_frames::restore() }.is_ok();
            if unsafe { promotion::restore() }.is_err() || !native_restored {
                return BridgeStatus::Critical as u32;
            }
            return status as u32;
        }
        if !start_observation(config.max_seconds) {
            return BridgeStatus::Critical as u32;
        }
        config.version = RUNTIME_CONFIG_VERSION;
        unsafe {
            ptr::write(input.cast::<BridgeConfig>(), config);
        }
        BridgeStatus::Ok as u32
    })
}

/// Retain exclusion across both the install unwind and its cleanup. Dropping
/// this guard inside catch_unwind would poison the recovery exports' mutex.
fn guard_runtime_install(action: impl FnOnce() -> u32 + std::panic::UnwindSafe) -> u32 {
    let Ok(_guard) = INSTALL_LOCK.lock() else {
        return BridgeStatus::Panic as u32;
    };
    std::panic::catch_unwind(action).unwrap_or_else(|_| {
        // Never let a partially published installation authorize Rebuild.
        // OWNED, trace storage and failed restoration handles stay retained.
        INSTALLED.store(false, Ordering::Release);
        product_deactivate();
        match std::panic::catch_unwind(|| unsafe { uninstall() }) {
            Ok(Ok(())) | Ok(Err("bridge not installed")) => {
                (if unsafe { root_observation::native_frames::restore() }.is_ok() {
                    BridgeStatus::Panic
                } else {
                    BridgeStatus::Critical
                }) as u32
            }
            _ => BridgeStatus::Critical as u32,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn BridgeRuntimeStopObservation(_: *mut c_void) -> u32 {
    stop_observation() as u32
}

/// Must be called BEFORE any object teardown. No real reload hook is claimed.
#[unsafe(no_mangle)]
pub extern "system" fn BridgeRuntimeClose(_: *mut c_void) -> u32 {
    let Ok(_guard) = INSTALL_LOCK.lock() else {
        return BridgeStatus::Panic as u32;
    };
    close()
}

fn close() -> u32 {
    if !INSTALLED.load(Ordering::Acquire) {
        return BridgeStatus::InvalidInput as u32;
    }
    INVOCATIONS.close();
    product_deactivate();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    if !root_observation::native_frames::request_close() {
        return BridgeStatus::Critical as u32;
    }
    while !INVOCATIONS.drained() {
        if std::time::Instant::now() >= deadline {
            return BridgeStatus::Critical as u32;
        }
        std::thread::yield_now();
    }
    if INVOCATION_FAILURE.load(Ordering::Acquire) {
        return BridgeStatus::Critical as u32;
    }
    for state in &STATE {
        state.store(0);
    }
    if advance() {
        BridgeStatus::Ok as u32
    } else {
        BridgeStatus::Critical as u32
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn BridgeRuntimeRebuild(_: *mut c_void) -> u32 {
    if !INSTALLED.load(Ordering::Acquire) {
        return BridgeStatus::InvalidInput as u32;
    }
    let Ok(_guard) = INSTALL_LOCK.lock() else {
        return BridgeStatus::Panic as u32;
    };
    if close() != BridgeStatus::Ok as u32 {
        return BridgeStatus::Critical as u32;
    }
    if !root_observation::native_frames::reopen() {
        // A missing all-domain certificate is not itself a failed native
        // drain. Never reopen product grants on this path. The observer may
        // advance only after its exact World and every retained use ended.
        if root_observation::native_frames::resume_unqualified_observation() {
            return BridgeStatus::BoundaryUnproven as u32;
        }
        return BridgeStatus::Critical as u32;
    }
    if !product_activate() {
        return BridgeStatus::Critical as u32;
    }
    if INVOCATIONS.open_next().is_none() {
        product_deactivate();
        return BridgeStatus::Critical as u32;
    }
    // Each owner enters rebuilding on its own natural reset getter. No
    // controller metadata supplies identity or a publication fence.
    BridgeStatus::Ok as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_prepare_uses_the_actual_cpu_fact_before_entering_the_installer() {
        const ENV: &str = "CIV6_RUNTIME_CPU_WIRING_CHILD";
        if std::env::var_os(ENV).is_none() {
            use std::os::windows::process::CommandExt;
            let output = test_child_output::run(
                std::process::Command::new(std::env::current_exe().unwrap())
                    .creation_flags(0x08000000)
                    .env(ENV, "1")
                    .args(["--exact", "windows::runtime::tests::host_prepare_uses_the_actual_cpu_fact_before_entering_the_installer", "--nocapture"]),
                std::time::Duration::from_secs(20),
            ).unwrap();
            assert!(
                output.status.success(),
                "{} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        assert!(!root_observation::native_frames::host_prepared());
        let calls = std::cell::Cell::new(0);
        let supported = supports_atomic_promotion_fetch();
        let result = guard_host_prepare(std::panic::AssertUnwindSafe(|| {
            calls.set(calls.get() + 1);
            Ok(())
        }));
        assert_eq!(
            result,
            if supported {
                BridgeStatus::Ok
            } else {
                BridgeStatus::InvalidInput
            } as u32
        );
        assert_eq!(calls.get(), usize::from(supported));
        assert!(!root_observation::native_frames::host_prepared());
    }

    #[test]
    fn unsupported_host_cpu_never_enters_native_preparation() {
        let calls = std::cell::Cell::new(0);
        let before = root_observation::native_frames::host_prepared();
        let result = guard_host_prepare_for_cpu(
            false,
            std::panic::AssertUnwindSafe(|| {
                calls.set(calls.get() + 1);
                Ok(())
            }),
        );
        assert_eq!(result, BridgeStatus::InvalidInput as u32);
        assert_eq!(
            calls.get(),
            0,
            "unsupported CPU entered the native installer"
        );
        assert_eq!(root_observation::native_frames::host_prepared(), before);
        let result = guard_host_prepare_for_cpu(
            true,
            std::panic::AssertUnwindSafe(|| {
                calls.set(calls.get() + 1);
                Ok(())
            }),
        );
        assert_eq!(result, BridgeStatus::Ok as u32);
        assert_eq!(
            calls.get(),
            1,
            "supported CPU did not reach native preparation"
        );
    }

    #[test]
    fn install_panic_keeps_recovery_lock_usable_and_rebuild_closed() {
        const CHILD: &str = "JAC_RUNTIME_INSTALL_PANIC_CHILD";
        if std::env::var_os(CHILD).is_none() {
            use std::io::Read;
            use std::os::windows::process::CommandExt;
            struct OwnedChild(std::process::Child);
            impl Drop for OwnedChild {
                fn drop(&mut self) {
                    if self.0.try_wait().ok().flatten().is_none() {
                        let _ = self.0.kill();
                    }
                    let _ = self.0.wait();
                }
            }
            let mut child = OwnedChild(std::process::Command::new(std::env::current_exe().unwrap())
                .creation_flags(0x08000000)
                .env(CHILD, "1")
                .args([
                    "--exact",
                    "windows::runtime::tests::install_panic_keeps_recovery_lock_usable_and_rebuild_closed",
                    "--nocapture",
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap());
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
            let (status, timed_out) = loop {
                if let Some(status) = child.0.try_wait().unwrap() {
                    break (status, false);
                }
                if std::time::Instant::now() >= deadline {
                    child.0.kill().unwrap();
                    break (child.0.wait().unwrap(), true);
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            };
            // The finite fixture writes only its two panic diagnostics. Drain
            // both pipes after exit/reap before deciding the parent result.
            let mut stdout = Vec::new();
            let mut stderr = Vec::new();
            child
                .0
                .stdout
                .take()
                .unwrap()
                .read_to_end(&mut stdout)
                .unwrap();
            child
                .0
                .stderr
                .take()
                .unwrap()
                .read_to_end(&mut stderr)
                .unwrap();
            assert!(
                !timed_out && status.success(),
                "timeout={timed_out}: {} {}",
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
            return;
        }
        // Process-local fixture state: no game, allocation or native patch.
        // Exercise the same exclusion and panic boundary as the public export.
        static HELD: AtomicBool = AtomicBool::new(false);
        INSTALLED.store(true, Ordering::Release);
        assert!(product_activate());
        assert_eq!(
            guard_runtime_install(|| {
                HELD.store(
                    matches!(
                        INSTALL_LOCK.try_lock(),
                        Err(std::sync::TryLockError::WouldBlock)
                    ),
                    Ordering::Release,
                );
                panic!("runtime install panic regression")
            }),
            BridgeStatus::Panic as u32
        );
        assert!(HELD.load(Ordering::Acquire));
        drop(
            INSTALL_LOCK
                .try_lock()
                .expect("recovery lock must not be poisoned"),
        );
        assert!(!INSTALLED.load(Ordering::Acquire));
        assert!(!PRODUCT_ACTIVE.load(Ordering::Acquire));
        assert_eq!(
            BridgeRuntimeClose(null_mut()),
            BridgeStatus::InvalidInput as u32
        );
        assert_eq!(
            BridgeRuntimeRebuild(null_mut()),
            BridgeStatus::InvalidInput as u32
        );
        let before = EPOCH.load();
        assert_eq!(guard_runtime_install(|| 77), 77);
        assert_eq!(EPOCH.load(), before);

        // Two owned synthetic entry pages exercise the real restoration path.
        // Neither page is ever called. The first compare/exchange is made to
        // fail; the later entry must still be restored, retaining only failure.
        let page = || unsafe {
            VirtualAlloc(
                null_mut(),
                CODE_PAGE,
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            )
            .cast::<u8>()
        };
        let failed = page();
        let restored = page();
        assert!(!failed.is_null() && !restored.is_null());
        let original = [0x90; 8];
        let patched = [0xcc; 8];
        let changed = [0x11; 8];
        unsafe {
            ptr::write(failed.cast::<[u8; 8]>(), changed);
            ptr::write(restored.cast::<[u8; 8]>(), patched);
        }
        let hook = |target| Hook {
            target,
            original,
            patched,
            prefix: None,
            _code: null_mut(), // no relay is published or executed by this fixture
        };
        *composite::ASSIGNMENT_HOOK.lock().unwrap() = Some(hook(failed));
        let mut blocked_restore = HOOK.lock().unwrap();
        *blocked_restore = Some(hook(restored));
        OWNED.store(true, Ordering::Release);
        INSTALLED.store(true, Ordering::Release);
        assert!(product_activate());
        RESTORE_HOOK_REACHED.store(false, Ordering::Release);
        let (send, receive) = std::sync::mpsc::channel();
        let cleanup = std::thread::spawn(move || {
            guard_runtime_install(|| {
                send.send(()).unwrap();
                panic!("partial publication panic regression")
            })
        });
        receive
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while (INSTALLED.load(Ordering::Acquire)
            || PRODUCT_ACTIVE.load(Ordering::Acquire)
            || !RESTORE_HOOK_REACHED.load(Ordering::Acquire))
            && std::time::Instant::now() < deadline
        {
            std::thread::yield_now();
        }
        let exclusion = matches!(
            INSTALL_LOCK.try_lock(),
            Err(std::sync::TryLockError::WouldBlock)
        );
        let closed = !INSTALLED.load(Ordering::Acquire)
            && !PRODUCT_ACTIVE.load(Ordering::Acquire)
            && RESTORE_HOOK_REACHED.load(Ordering::Acquire);
        // Release before assertions/join, including when a regression occurred.
        drop(blocked_restore);
        assert_eq!(cleanup.join().unwrap(), BridgeStatus::Critical as u32);
        assert!(exclusion, "cleanup must retain the installation exclusion");
        assert!(
            closed,
            "permission and Rebuild close before restoration can block"
        );
        assert!(OWNED.load(Ordering::Acquire));
        assert_eq!(
            composite::ASSIGNMENT_HOOK
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .target,
            failed
        );
        assert!(
            HOOK.lock().unwrap().is_none(),
            "later restoration must still run"
        );
        assert_eq!(unsafe { ptr::read(failed.cast::<[u8; 8]>()) }, changed);
        assert_eq!(unsafe { ptr::read(restored.cast::<[u8; 8]>()) }, original);
        drop(
            INSTALL_LOCK
                .try_lock()
                .expect("failed restoration must not poison recovery"),
        );
        assert_eq!(
            BridgeRuntimeRebuild(null_mut()),
            BridgeStatus::InvalidInput as u32
        );
        // Restore the fixture's expected operand and use the retained handle.
        unsafe { ptr::write(failed.cast::<[u8; 8]>(), patched) };
        assert_eq!(unsafe { uninstall() }, Ok(()));
        assert!(composite::ASSIGNMENT_HOOK.lock().unwrap().is_none());
        assert_eq!(unsafe { ptr::read(failed.cast::<[u8; 8]>()) }, original);
        assert_ne!(unsafe { VirtualFree(failed.cast(), 0, MEM_RELEASE) }, 0);
        assert_ne!(unsafe { VirtualFree(restored.cast(), 0, MEM_RELEASE) }, 0);
    }

    #[test]
    fn partial_host_prepare_failure_restores_published_entries_and_allows_retry() {
        const ENV: &str = "CIV6_RUNTIME_PARTIAL_HOST_CHILD";
        if std::env::var_os(ENV).is_none() {
            use std::os::windows::process::CommandExt;
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .creation_flags(0x08000000).env(ENV, "1")
                .args(["--exact", "windows::runtime::tests::partial_host_prepare_failure_restores_published_entries_and_allows_retry", "--nocapture"])
                .spawn().unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    assert!(status.success());
                    return;
                }
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("Host preparation child timeout");
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        use root_observation::native_frames as native;
        let sites = native::fixture_host_job_sites();
        let size = 0xe40000;
        let base =
            unsafe { VirtualAlloc(null_mut(), size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) }
                as usize;
        assert_ne!(base, 0);
        for (rva, bytes, _) in sites {
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), (base + rva) as *mut u8, 8);
            }
        }
        let bad = (base + sites[3].0) as *mut u8;
        unsafe {
            *bad = 0xcc;
        }
        let mut old = 0;
        assert_ne!(
            unsafe { VirtualProtect(base as _, size, PAGE_EXECUTE_READ, &mut old) },
            0
        );
        // This fixture exercises owned byte publication/rollback only; it never executes the patched native entries.
        // Its supplied supported-CPU fact does not assert that this runner supports native instruction fetching.
        let _lock = INSTALL_LOCK.lock().unwrap();
        let published = std::cell::Cell::new(None);
        assert_eq!(
            guard_host_prepare_for_cpu(
                true,
                std::panic::AssertUnwindSafe(|| {
                    let result = unsafe { native::fixture_host_jobs(base) };
                    published.set(Some((
                        result,
                        sites
                            .iter()
                            .take(3)
                            .all(|(rva, _, _)| (unsafe { *((base + rva) as *const u8) }) == 0xe9),
                    )));
                    result
                })
            ),
            BridgeStatus::Critical as u32
        );
        assert!(
            matches!(published.get(), Some((Err(_), true))),
            "partial publication was not observed"
        );
        for (rva, bytes, _) in sites.iter().take(3) {
            assert_eq!(unsafe { ptr::read((base + rva) as *const [u8; 8]) }, *bytes);
        }
        assert!(!PRODUCT_ACTIVE.load(Ordering::Acquire));
        assert_ne!(
            unsafe { VirtualProtect(bad.cast(), 8, PAGE_READWRITE, &mut old) },
            0
        );
        unsafe {
            ptr::copy_nonoverlapping(sites[3].1.as_ptr(), bad, 8);
        }
        assert_ne!(unsafe { VirtualProtect(bad.cast(), 8, old, &mut old) }, 0);
        assert_eq!(
            guard_host_prepare_for_cpu(true, || unsafe { native::fixture_host_jobs(base) }),
            BridgeStatus::Ok as u32
        );
        unsafe { native::restore() }.unwrap();
        for (rva, bytes, _) in sites {
            assert_eq!(unsafe { ptr::read((base + rva) as *const [u8; 8]) }, bytes);
        }
        let retained = (base + sites[0].0) as *mut u8;
        let panic_reached = AtomicBool::new(false);
        assert_eq!(
            guard_host_prepare_for_cpu(
                true,
                std::panic::AssertUnwindSafe(|| {
                    unsafe { native::fixture_host_jobs(base) }?;
                    unsafe {
                        VirtualProtect(retained.cast(), 8, PAGE_READWRITE, &mut old);
                        ptr::write(retained.cast::<[u8; 8]>(), [0xcc; 8]);
                        VirtualProtect(retained.cast(), 8, old, &mut old);
                    }
                    panic_reached.store(true, Ordering::Release);
                    panic!("owned Host preparation panic")
                })
            ),
            BridgeStatus::Critical as u32
        );
        assert!(panic_reached.load(Ordering::Acquire));
        assert_eq!(unsafe { ptr::read(retained.cast::<[u8; 8]>()) }, [0xcc; 8]);
        for (rva, bytes, _) in sites.iter().skip(1) {
            assert_eq!(unsafe { ptr::read((base + rva) as *const [u8; 8]) }, *bytes);
        }
        assert_eq!(
            guard_host_prepare_for_cpu(true, || unsafe { native::fixture_host_jobs(base) }),
            BridgeStatus::Critical as u32
        );
        assert_ne!(
            unsafe { VirtualProtect(retained.cast(), 8, PAGE_READWRITE, &mut old) },
            0
        );
        unsafe {
            ptr::copy_nonoverlapping(sites[0].1.as_ptr(), retained, 8);
        }
        assert_ne!(
            unsafe { VirtualProtect(retained.cast(), 8, old, &mut old) },
            0
        );
        unsafe { native::restore() }.unwrap();
        assert_eq!(
            guard_host_prepare_for_cpu(true, || unsafe { native::fixture_host_jobs(base) }),
            BridgeStatus::Ok as u32
        );
        unsafe { native::restore() }.unwrap();
        assert_ne!(unsafe { VirtualFree(base as _, 0, MEM_RELEASE) }, 0);
    }

    #[test]
    fn runtime_export_rejects_invalid_storage_and_config_without_policy_changes() {
        let _guard = super::super::tests::HOOK_TEST_LOCK.lock().unwrap();
        let before = (
            owned(),
            PRODUCT_ACTIVE.load(Ordering::Acquire),
            PRODUCT_SESSION.load(Ordering::Acquire),
            EPOCH.load(),
        );
        for input in [
            null_mut(),
            std::ptr::dangling_mut(),
            usize::MAX as *mut c_void,
        ] {
            assert_eq!(
                BridgeRuntimeInstall(input),
                BridgeStatus::InvalidInput as u32
            );
        }
        let mut invalid = BridgeConfig::for_production(0, 0, 0);
        // This export accepts only its explicit version and game-owned trace.
        assert_eq!(
            BridgeRuntimeInstall((&mut invalid as *mut BridgeConfig).cast()),
            BridgeStatus::InvalidInput as u32
        );
        invalid.version = RUNTIME_CONFIG_VERSION;
        invalid.trace_size = 0;
        invalid.max_seconds = 0;
        assert_eq!(
            BridgeRuntimeInstall((&mut invalid as *mut BridgeConfig).cast()),
            BridgeStatus::InvalidInput as u32
        );
        assert_eq!(
            (
                owned(),
                PRODUCT_ACTIVE.load(Ordering::Acquire),
                PRODUCT_SESSION.load(Ordering::Acquire),
                EPOCH.load()
            ),
            before
        );
    }
}
