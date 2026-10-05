//! Explicit process-lifetime owner of the existing three-entry composite.
//! No timer uninstalls it. The single finite observation may expire or stop;
//! its allocation, gateways and authority remain resident until process exit.
//! Close/rebuild exports are control boundaries, NOT an observed reload hook.
use super::*;
use windows_sys::Win32::System::LibraryLoader::{
    GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_PIN,
};

pub const RUNTIME_CONFIG_VERSION: u32 = 0x0324_0001;
static OWNED: AtomicBool = AtomicBool::new(false);
static INSTALLED: AtomicBool = AtomicBool::new(false);
static RECORDING: AtomicBool = AtomicBool::new(false);
static SAMPLE_FLIGHTS: AtomicU32 = AtomicU32::new(0);
static DEADLINE_MS: AtomicU64 = AtomicU64::new(0);
// Reset stamp and invocation generation change in ONE cmpxchg16b. A reset
// paused after its phase CAS cannot advance generation after a newer reset.
static EPOCH: AtomicJoinedWord = AtomicJoinedWord::new();
static PHASE: [AtomicU64; PRODUCTION_OWNER_LIMIT] =
    [const { AtomicU64::new(0) }; PRODUCTION_OWNER_LIMIT];
pub(super) static INSTALL_LOCK: Mutex<()> = Mutex::new(());
// 0 closed, 1 rebuilding, 2 ready, together with session (27) and ticket (31).

fn stamp(session: u32, ticket: u32) -> u64 {
    (u64::from(session) << 31) | u64::from(ticket)
}

fn phase_word(binding: u128, phase: u64) -> u64 {
    stamp(joined_session(binding), joined_ticket(binding)) << 2 | phase
}

fn live_session(session: u32) -> bool {
    PRODUCT_ACTIVE.load(Ordering::Acquire)
        && !PRODUCT_EXHAUSTED.load(Ordering::Acquire)
        && PRODUCT_SESSION.load(Ordering::Acquire) == session
}

pub(super) fn owned() -> bool {
    OWNED.load(Ordering::Acquire)
}

pub(super) fn ready(owner: usize) -> bool {
    !owned()
        || (owner < PRODUCTION_OWNER_LIMIT
            && PHASE[owner].load(Ordering::Acquire) == phase_word(PRODUCT_OWNERS[owner].load(), 2))
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

fn advance() -> bool {
    advance_epoch(None)
}

fn advance_epoch(reset: Option<(usize, u128)>) -> bool {
    loop {
        let current = EPOCH.load();
        let reset_stamp = match reset {
            Some((owner, binding)) => {
                if !live_session(joined_session(binding))
                    || PRODUCT_OWNERS[owner].load() != binding
                    || PHASE[owner].load(Ordering::Acquire) != phase_word(binding, 3)
                {
                    return false;
                }
                joined_receiver(current).max(stamp(joined_session(binding), joined_ticket(binding)))
            }
            None => joined_receiver(current),
        };
        let Some(generation) = joined_session(current).checked_add(1) else {
            product_deactivate();
            for phase in &PHASE {
                phase.store(0, Ordering::Release);
            }
            return false;
        };
        if EPOCH
            .compare_exchange(current, joined_word(reset_stamp, generation, 0, false))
            .is_ok()
        {
            return true;
        }
    }
}

pub(super) fn reset_owner(owner: usize, receiver: u64, session: u32, ticket: u32) -> bool {
    if !owned() || owner >= PRODUCTION_OWNER_LIMIT || !live_session(session) {
        return false;
    }
    let binding = joined_word(receiver, session, ticket, false);
    let next_stamp = stamp(session, ticket);
    // Changing the owner binding first closes the prior catalog immediately:
    // ready() and every copied cell must match this exact session/ticket.
    loop {
        let current = PRODUCT_OWNERS[owner].load();
        if !live_session(session)
            || stamp(joined_session(current), joined_ticket(current)) >= next_stamp
        {
            return false;
        }
        if PRODUCT_OWNERS[owner]
            .compare_exchange(current, binding)
            .is_ok()
        {
            break;
        }
    }
    loop {
        let phase = PHASE[owner].load(Ordering::Acquire);
        if !live_session(session)
            || PRODUCT_OWNERS[owner].load() != binding
            || phase >> 2 >= next_stamp
        {
            return false;
        }
        // Internal phase 3 is closed while the epoch CAS is pending. A
        // publication cannot make this binding ready before its generation.
        if PHASE[owner]
            .compare_exchange(
                phase,
                phase_word(binding, 3),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            #[cfg(test)]
            if RESET_STAGE.load(Ordering::Acquire) == 2 {
                pause_reset();
            }
            if !advance_epoch(Some((owner, binding))) {
                return false;
            }
            return PHASE[owner]
                .compare_exchange(
                    phase_word(binding, 3),
                    phase_word(binding, 1),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok();
        }
    }
}

pub(super) fn publication(owner: usize, binding: u128, valid: bool, committed: bool) {
    if !owned()
        || owner >= PRODUCTION_OWNER_LIMIT
        || !live_session(joined_session(binding))
        || PRODUCT_OWNERS[owner].load() != binding
    {
        return;
    }
    if !valid {
        let phase = PHASE[owner].load(Ordering::Acquire);
        if phase >> 2 == phase_word(binding, 0) >> 2
            && PHASE[owner]
                .compare_exchange(
                    phase,
                    phase_word(binding, 0),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
        {
            advance();
        }
    } else if committed {
        let _ = PHASE[owner].compare_exchange(
            phase_word(binding, 1),
            phase_word(binding, 2),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }
}

/// The producer's typed COMMIT=false starts a transaction; it is not a failed
/// publish. It closes authority while preserving the reset's rebuild fence.
pub(super) fn begin_catalog(owner: usize, binding: u128) {
    if !owned()
        || owner >= PRODUCTION_OWNER_LIMIT
        || !live_session(joined_session(binding))
        || PRODUCT_OWNERS[owner].load() != binding
    {
        return;
    }
    let ready = phase_word(binding, 2);
    if PHASE[owner]
        .compare_exchange(
            ready,
            phase_word(binding, 1),
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_ok()
    {
        advance();
    }
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
    PHASE[owner].load(Ordering::Acquire) as u32 & 3
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
    for phase in &PHASE {
        phase.store(0, Ordering::Release);
    }
    product_deactivate();
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
