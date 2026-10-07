//! Explicit process-lifetime owner of the existing three-entry composite.
//! No timer uninstalls it. The single finite observation may expire or stop;
//! its allocation, gateways and authority remain resident until process exit.
//! Close/rebuild exports are control boundaries, NOT an observed reload hook.
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
static OWNED: AtomicBool = AtomicBool::new(false);
static INSTALLED: AtomicBool = AtomicBool::new(false);
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
    if !owned() {
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
    if fence.read_state != 1
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

pub(super) fn start_observation(seconds: u32) {
    let now = unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() };
    DEADLINE_MS.store(now + u64::from(seconds) * 1000, Ordering::Release);
    SAMPLE_GATE.initialize_before_publish();
    // Publish stoppability only after opening the observation. Otherwise a
    // concurrent stop could succeed against the initial closed word and then
    // be undone by this store of zero.
    INSTALLED.store(true, Ordering::Release);
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
    std::panic::catch_unwind(|| {
        let Ok(_guard) = INSTALL_LOCK.lock() else {
            return BridgeStatus::Panic as u32;
        };
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
        config.version = EXPERIMENTAL_PAID_CONFIG_VERSION;
        config.trace_address = trace as u64;
        config.trace_size = std::mem::size_of::<CompositeTrace>() as u32;
        let status = guarded_install(config);
        if !matches!(status, BridgeStatus::Ok) {
            // Storage stays alive for any already-fetched relay. No retry may
            // repoint it. guarded_install/composite restore the installed set.
            product_deactivate();
            return status as u32;
        }
        start_observation(config.max_seconds);
        config.version = RUNTIME_CONFIG_VERSION;
        unsafe {
            ptr::write(input.cast::<BridgeConfig>(), config);
        }
        BridgeStatus::Ok as u32
    })
    .unwrap_or_else(|_| {
        product_deactivate();
        BridgeStatus::Panic as u32
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
    product_deactivate();
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
    if !product_activate() {
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
