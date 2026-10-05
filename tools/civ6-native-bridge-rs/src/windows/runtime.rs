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
static GENERATION: AtomicU32 = AtomicU32::new(1);
static PHASE: [AtomicU64; PRODUCTION_OWNER_LIMIT] =
    [const { AtomicU64::new(0) }; PRODUCTION_OWNER_LIMIT];
pub(super) static INSTALL_LOCK: Mutex<()> = Mutex::new(());
// 0 closed, 1 rebuilding, 2 ready. Ready requires a complete natural catalog.

pub(super) fn owned() -> bool {
    OWNED.load(Ordering::Acquire)
}

pub(super) fn ready(owner: usize) -> bool {
    !owned()
        || (owner < PRODUCTION_OWNER_LIMIT
            && PHASE[owner].load(Ordering::Acquire)
                == (u64::from(joined_ticket(PRODUCT_OWNERS[owner].load())) << 2 | 2))
}

pub(super) fn generation() -> u32 {
    if owned() {
        GENERATION.load(Ordering::Acquire)
    } else {
        0
    }
}

pub(super) fn current(generation: u32) -> bool {
    !owned() || (generation != 0 && generation == self::generation())
}

fn advance() -> bool {
    if GENERATION
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| n.checked_add(1))
        .is_err()
    {
        product_deactivate();
        for phase in &PHASE {
            phase.store(0, Ordering::Release);
        }
        return false;
    }
    true
}

pub(super) fn reset_owner(owner: usize, ticket: u32) {
    if owned() && owner < PRODUCTION_OWNER_LIMIT {
        PHASE[owner].store(u64::from(ticket) << 2 | 1, Ordering::Release);
        advance();
    }
}

pub(super) fn publication(owner: usize, binding_ticket: u32, valid: bool, committed: bool) {
    if !owned() || owner >= PRODUCTION_OWNER_LIMIT {
        return;
    }
    if !valid {
        let phase = PHASE[owner].load(Ordering::Acquire);
        if phase >> 2 == u64::from(binding_ticket)
            && PHASE[owner]
                .compare_exchange(
                    phase,
                    u64::from(binding_ticket) << 2,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
        {
            advance();
        }
    } else if committed {
        let _ = PHASE[owner].compare_exchange(
            u64::from(binding_ticket) << 2 | 1,
            u64::from(binding_ticket) << 2 | 2,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }
}

/// The producer's typed COMMIT=false starts a transaction; it is not a failed
/// publish. It closes authority while preserving the reset's rebuild fence.
pub(super) fn begin_catalog(owner: usize, binding_ticket: u32) {
    if !owned() || owner >= PRODUCTION_OWNER_LIMIT {
        return;
    }
    let ready = u64::from(binding_ticket) << 2 | 2;
    if PHASE[owner]
        .compare_exchange(
            ready,
            u64::from(binding_ticket) << 2 | 1,
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
    trace
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
