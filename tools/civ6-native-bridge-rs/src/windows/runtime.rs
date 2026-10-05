//! Explicit process-lifetime owner of the existing three-entry composite.
//! No timer uninstalls it. The single finite observation may expire or stop;
//! its allocation, gateways and authority remain resident until process exit.
//! Close/rebuild exports are control boundaries, NOT an observed reload hook.
use super::*;
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN,
};

pub const RUNTIME_CONFIG_VERSION: u32 = 0x0324_0001;
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
static RECORDING: AtomicBool = AtomicBool::new(false);
static SAMPLE_FLIGHTS: AtomicU32 = AtomicU32::new(0);
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
pub(super) fn current(generation: u32) -> bool {
    !owned() || (generation != 0 && generation == self::generation())
}

#[derive(Clone, Copy)]
struct Fence {
    owner: usize,
    binding: u128,
    ticket: u32,
    commit: Option<u128>,
}
impl Fence {
    fn live(self) -> bool {
        self.owner < PRODUCTION_OWNER_LIMIT
            && live_session(joined_session(self.binding))
            && PRODUCT_OWNERS[self.owner].load() == self.binding
            && self
                .commit
                .is_none_or(|commit| composite::commit_word(self.owner) == commit)
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

// Reset and every accepted COMMIT use this same monotonic transition. Checking
// only a successful cell write is insufficient: the callback may be paused
// after acceptance. The latest entry stamp remains in the state even when its
// phase repeats, so no ready -> rebuilding -> ready ABA can authorize old work.
fn transition(fence: Fence, requested: u32) -> bool {
    loop {
        let state = STATE[fence.owner].load();
        let incoming = binding_stamp(fence.binding);
        if !fence.live()
            || state & BIND_MASK > incoming
            || (state & BIND_MASK == incoming && state_ticket(state) >= fence.ticket)
        {
            return false;
        }
        let begin = if fence.commit.is_none() {
            0
        } else if requested == 1 {
            fence.ticket
        } else if state & BIND_MASK == incoming {
            state_begin(state)
        } else {
            0
        };
        let phase = if requested == 2
            && (begin == 0 || state & BIND_MASK != incoming || !matches!(state_phase(state), 1 | 2))
        {
            0
        } else {
            requested
        };
        let pending = state_word(
            fence.binding,
            fence.ticket,
            begin,
            if phase == 2 { 2 } else { 3 },
        );
        if STATE[fence.owner].compare_exchange(state, pending).is_err() {
            continue;
        }
        #[cfg(test)]
        if fence.commit.is_none() && RESET_STAGE.load(Ordering::Acquire) == 2 {
            pause_reset();
        }
        #[cfg(test)]
        if fence.commit.is_some() {
            composite::pause_commit(3);
        }
        if phase == 2 {
            return true;
        }
        if !advance_epoch(Some((fence, pending))) {
            return false;
        }
        return STATE[fence.owner]
            .compare_exchange(
                pending,
                state_word(fence.binding, fence.ticket, begin, phase),
            )
            .is_ok();
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
            commit: None,
        },
        1,
    )
}

pub(super) fn observe_commit(owner: usize, binding: u128, commit: u128, phase: u32) {
    if owned() {
        transition(
            Fence {
                owner,
                binding,
                ticket: joined_ticket(commit),
                commit: Some(commit),
            },
            phase,
        );
    }
}

pub(super) fn catalog_begin(owner: usize, binding: u128) -> Option<u32> {
    if !owned() {
        return Some(joined_ticket(binding));
    }
    if owner >= PRODUCTION_OWNER_LIMIT
        || !live_session(joined_session(binding))
        || PRODUCT_OWNERS[owner].load() != binding
    {
        return None;
    }
    let state = STATE[owner].load();
    (state & BIND_MASK == binding_stamp(binding)
        && matches!(state_phase(state), 1 | 2)
        && state_begin(state) > joined_ticket(binding))
    .then_some(state_begin(state))
}

pub(super) fn ready(owner: usize) -> bool {
    if !owned() {
        return true;
    }
    if owner >= PRODUCTION_OWNER_LIMIT {
        return false;
    }
    let state = STATE[owner].load();
    let binding = PRODUCT_OWNERS[owner].load();
    let commit = composite::commit_word(owner);
    state_phase(state) == 2
        && state & BIND_MASK == binding_stamp(binding)
        && state_ticket(state) == joined_ticket(commit)
        && joined_permitted(commit)
        && joined_session(commit) == joined_session(binding)
        && joined_receiver(commit) == joined_receiver(binding)
        && live_session(joined_session(binding))
        && STATE[owner].load() == state
        && PRODUCT_OWNERS[owner].load() == binding
        && composite::commit_word(owner) == commit
}

/// Acquisition/recheck makes observation stop drain only sampled callbacks.
/// No hook waits, allocates or takes a lock. Rows are never reset or redirected.
pub(super) fn sample_begin() -> bool {
    if !owned() {
        return true;
    }
    let now = unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() };
    if now >= DEADLINE_MS.load(Ordering::Acquire) {
        RECORDING.store(false, Ordering::Release);
    }
    if !RECORDING.load(Ordering::Acquire) {
        return false;
    }
    SAMPLE_FLIGHTS.fetch_add(1, Ordering::AcqRel);
    if !RECORDING.load(Ordering::Acquire) {
        SAMPLE_FLIGHTS.fetch_sub(1, Ordering::Release);
        return false;
    }
    true
}

pub(super) fn sample_end() {
    if owned() {
        SAMPLE_FLIGHTS.fetch_sub(1, Ordering::Release);
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
    RECORDING.store(false, Ordering::Release);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while SAMPLE_FLIGHTS.load(Ordering::Acquire) != 0 {
        if std::time::Instant::now() >= deadline {
            return BridgeStatus::Critical;
        }
        std::thread::yield_now();
    }
    BridgeStatus::Ok
}

pub(super) fn allocate_trace() -> *mut CompositeTrace {
    let trace = Box::into_raw(CompositeTrace::boxed());
    OWNED.store(true, Ordering::Release);
    advance();
    trace
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
    INSTALLED.store(true, Ordering::Release);
    let now = unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() };
    DEADLINE_MS.store(now + u64::from(seconds) * 1000, Ordering::Release);
    RECORDING.store(true, Ordering::Release);
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
        let trace = allocate_trace();
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
