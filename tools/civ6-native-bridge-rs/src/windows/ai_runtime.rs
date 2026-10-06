//! AI gateway adapter backed by the actual runtime owner cells and CallFence.
//! No independently writable permit and no constant-true RuntimeLease.
use super::*;
use crate::ai_operation::gateway::{self, LeaseStamp, Registration, RuntimeLease};

struct OwnedLease;
static LEASE: OwnedLease = OwnedLease;
static REGISTRATION: Mutex<Option<Registration>> = Mutex::new(None);

fn arm(binding: u128) -> u64 {
    (u64::from(joined_session(binding)) << 31) | u64::from(joined_ticket(binding))
}

unsafe impl RuntimeLease for OwnedLease {
    fn on_unwind(&self) {
        ai_operation::cancel();
    }
    fn accepts_entry(&self, installed: LeaseStamp, current: LeaseStamp, player: u64) -> bool {
        // A NEW natural call can consume a newly committed catalog in the same
        // live Player. In-flight calls retain their old stamp and fail current.
        // Runtime close restores registrations before object teardown; it never
        // silently reuses this callable for a reloaded/replaced Player.
        current.pid == installed.pid
            && current.created == installed.created
            && current.owner == installed.owner
            && current.generation >= installed.generation
            && current.session >= installed.session
            && self.current(current, player)
    }
    fn before_original(
        &self,
        stamp: LeaseStamp,
        player: u64,
        definition: crate::governor_payment::PaymentDefinition,
    ) {
        ai_operation::before(self, stamp, player, definition)
    }
    fn after_original(
        &self,
        stamp: LeaseStamp,
        player: u64,
        definition: crate::governor_payment::PaymentDefinition,
    ) {
        ai_operation::after(self, stamp, player, definition)
    }
    fn begin(&self, owner: u32, player: u64) -> Option<LeaseStamp> {
        let fence = runtime::player_call_begin(player)?;
        if fence.owner != owner {
            return None;
        }
        let binding = PRODUCT_OWNERS[owner as usize].load();
        let stamp = LeaseStamp {
            pid: unsafe { GetCurrentProcessId() },
            created: current_created_filetime()?,
            owner,
            generation: runtime::generation(),
            session: fence.session,
            begin_ticket: fence.ticket,
            arm_token: arm(binding),
        };
        self.current(stamp, player).then_some(stamp)
    }
    fn current(&self, stamp: LeaseStamp, player: u64) -> bool {
        if !runtime::ai_installed()
            || stamp.owner as usize >= PRODUCTION_OWNER_LIMIT
            || stamp.pid != unsafe { GetCurrentProcessId() }
            || current_created_filetime() != Some(stamp.created)
            || stamp.generation == 0
            || runtime::generation() != stamp.generation
        {
            return false;
        }
        let Some(receiver) = player.checked_add(0x528) else {
            return false;
        };
        let binding = PRODUCT_OWNERS[stamp.owner as usize].load();
        let fence = runtime::CallFence {
            owner: stamp.owner,
            receiver,
            session: stamp.session,
            ticket: stamp.begin_ticket,
            read_state: 1,
        };
        joined_receiver(binding) == receiver
            && joined_session(binding) == stamp.session
            && arm(binding) == stamp.arm_token
            && runtime::player_call_current(fence, player)
            && PRODUCT_OWNERS[stamp.owner as usize].load() == binding
            && runtime::generation() == stamp.generation
    }
}

/// Parent lifecycle calls before object teardown. The gateway's original code
/// remains pinned; a failure must propagate to the owning controller as critical.
pub(super) fn close() -> bool {
    if !ai_operation::close() {
        return false;
    }
    let Ok(_guard) = REGISTRATION.lock() else {
        return false;
    };
    // Failed installation may have published only the gateway recovery handle.
    // Closing must restore that state too before the controller can tear down.
    gateway::close_registered().is_ok()
}

/// Caller supplies only a current AI container found from the native owner.
/// Governor/promotion definitions come from the runtime's committed GameInfo
/// transport, never from untrusted config indices standing in for a catalog.
pub(super) unsafe fn register_owner(
    base: u64,
    owner: u32,
    ai_owner: u64,
    index: u32,
) -> Result<(), gateway::Failure> {
    if !runtime::ai_installed() || owner as usize >= PRODUCTION_OWNER_LIMIT {
        return Err(gateway::Failure::Lease);
    }
    let binding = PRODUCT_OWNERS[owner as usize].load();
    let player = joined_receiver(binding)
        .checked_sub(0x528)
        .ok_or(gateway::Failure::Identity)?;
    let stamp = LEASE.begin(owner, player).ok_or(gateway::Failure::Lease)?;
    let catalog = runtime::ready_catalog(owner as usize, binding).ok_or(gateway::Failure::Lease)?;
    let definition = catalog
        .into_iter()
        .find(|d| d.governor_index == index)
        .ok_or(gateway::Failure::Identity)?;
    if !(2..=3).contains(&definition.cost)
        || product_permission(
            owner as usize,
            index as usize,
            player + 0x528,
            stamp.session,
        ) != Some(true)
    {
        return Err(gateway::Failure::Lease);
    }
    if copy_current::<8>(ai_owner.checked_add(0x90).ok_or(gateway::Failure::Range)?)
        .map(u64::from_le_bytes)
        != Some(player)
    {
        return Err(gateway::Failure::Identity);
    }
    let head = copy_current::<8>(ai_owner.checked_add(0x1b8).ok_or(gateway::Failure::Range)?)
        .map(u64::from_le_bytes)
        .ok_or(gateway::Failure::Memory)?;
    let count = copy_current::<4>(ai_owner + 0x1c8)
        .map(u32::from_le_bytes)
        .ok_or(gateway::Failure::Memory)?;
    if count == 0 || count > 256 {
        return Err(gateway::Failure::Identity);
    }
    let mut found = None;
    for slot in 0..count {
        let at = head
            .checked_add(u64::from(slot) * 0x40 + 0x38)
            .ok_or(gateway::Failure::Range)?;
        let Some(callable) = copy_current::<8>(at).map(u64::from_le_bytes) else {
            return Err(gateway::Failure::Memory);
        };
        if copy_current::<8>(callable).map(u64::from_le_bytes) == base.checked_add(0x9fb4b8)
            && copy_current::<8>(callable.checked_add(0x10).ok_or(gateway::Failure::Range)?)
                .map(u64::from_le_bytes)
                == base.checked_add(0x4a7ad0)
        {
            if found.replace(callable).is_some() {
                return Err(gateway::Failure::Identity);
            }
        }
    }
    let callable = found.ok_or(gateway::Failure::Caller)?;
    let mut guard = REGISTRATION.lock().map_err(|_| gateway::Failure::Patch)?;
    if guard.is_some() || !LEASE.current(stamp, player) {
        return Err(gateway::Failure::Lease);
    }
    *guard = Some(unsafe {
        gateway::register(
            base,
            ai_owner,
            callable,
            definition,
            definition.titles[0].ok_or(gateway::Failure::Identity)?,
            &LEASE,
        )
    }?);
    Ok(())
}

#[cfg(test)]
pub(super) fn lease_for_test() -> &'static dyn RuntimeLease {
    &LEASE
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_registration_rejects_unowned_and_malformed_input_without_game_calls() {
        assert_eq!(
            BridgeRuntimeAiObserve(ptr::null_mut()),
            BridgeStatus::InvalidInput as u32
        );
        let mut config = AiObservationConfig {
            size: std::mem::size_of::<AiObservationConfig>() as u32,
            version: AI_OBSERVATION_VERSION,
            game_pid: unsafe { GetCurrentProcessId() },
            owner: 2,
            created_filetime: current_created_filetime().unwrap(),
            generation: 1,
            governor_index: 77,
            status: 0,
            execution_mode: 0,
        };
        // Test library has no production GameCore owner. Typed config itself
        // cannot turn a current process or a numeric index into permission.
        assert_eq!(
            BridgeRuntimeAiObserve((&mut config as *mut AiObservationConfig).cast()),
            BridgeStatus::InvalidInput as u32
        );
        assert!(REGISTRATION.lock().unwrap().is_none());
    }
}

/// Finite registration of one joined cost2/3 actor under the current owner.
/// Contains no raw address, no permission boolean and no artificial ABI flag.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct AiObservationConfig {
    pub size: u32,
    pub version: u32,
    pub game_pid: u32,
    pub owner: u32,
    pub created_filetime: u64,
    pub generation: u32,
    pub governor_index: u32,
    pub status: u32,
    /// 0 observes; 1 requests one formal title after an unchanged AI callback.
    pub execution_mode: u32,
}
pub const AI_OBSERVATION_VERSION: u32 = 0x0880_0001;

#[repr(C)]
pub struct AiRuntimeState {
    pub size: u32,
    pub version: u32,
    pub game_pid: u32,
    pub owner: u32,
    pub created_filetime: u64,
    pub governor_index: u32,
    pub generation: u32,
    pub ready: u32,
    pub record_present: u32,
    pub record_generation: u32,
    pub spent_before: u32,
    pub spent_after: u32,
    pub title: u32,
    pub dispatched: u32,
    pub verified: u32,
}

/// Read-only receipt. Ready/generation come from real runtime cells; neither is
/// an execution permit. Registration and every callback revalidate the owner.
#[unsafe(no_mangle)]
pub extern "system" fn BridgeRuntimeAiQuery(input: *mut c_void) -> u32 {
    if !committed_range(
        input as usize,
        std::mem::size_of::<AiRuntimeState>(),
        std::mem::align_of::<AiRuntimeState>(),
        true,
    ) {
        return BridgeStatus::InvalidInput as u32;
    }
    std::panic::catch_unwind(|| {
        let config = unsafe { &mut *input.cast::<AiRuntimeState>() };
        if config.size as usize != std::mem::size_of::<AiRuntimeState>()
            || config.version != AI_OBSERVATION_VERSION
            || config.owner >= 64
            || config.governor_index >= 256
            || !runtime::ai_installed()
        {
            return BridgeStatus::InvalidInput as u32;
        }
        if config.game_pid != unsafe { GetCurrentProcessId() }
            || current_created_filetime() != Some(config.created_filetime)
        {
            return BridgeStatus::ProcessIdentity as u32;
        }
        config.generation = runtime::generation();
        config.ready = 0;
        config.record_present = 0;
        config.record_generation = 0;
        config.spent_before = 0;
        config.spent_after = 0;
        config.title = 0;
        config.dispatched = 0;
        config.verified = 0;
        let binding = PRODUCT_OWNERS[config.owner as usize].load();
        if let Some(player) = joined_receiver(binding).checked_sub(0x528) {
            config.ready = u32::from(LEASE.begin(config.owner, player).is_some());
        }
        if let Some(row) = ai_operation::last()
            .filter(|row| row.owner == config.owner && row.index == config.governor_index)
        {
            config.record_present = 1;
            config.record_generation = row.generation;
            config.spent_before = row.before;
            config.spent_after = row.after;
            config.title = row.title;
            config.dispatched = u32::from(row.dispatched);
            config.verified = u32::from(row.verified);
        }
        BridgeStatus::Ok as u32
    })
    .unwrap_or(BridgeStatus::Panic as u32)
}

#[unsafe(no_mangle)]
pub extern "system" fn BridgeRuntimeAiObserve(input: *mut c_void) -> u32 {
    if !committed_range(
        input as usize,
        std::mem::size_of::<AiObservationConfig>(),
        std::mem::align_of::<AiObservationConfig>(),
        true,
    ) {
        return BridgeStatus::InvalidInput as u32;
    }
    std::panic::catch_unwind(|| {
        let Ok(_guard) = runtime::INSTALL_LOCK.lock() else {
            return BridgeStatus::Panic as u32;
        };
        let mut config = unsafe { ptr::read(input.cast::<AiObservationConfig>()) };
        if config.size as usize != std::mem::size_of::<AiObservationConfig>()
            || config.version != AI_OBSERVATION_VERSION
            || config.execution_mode > 1
            || config.owner >= 64
            || config.governor_index >= 256
            || config.generation == 0
            || config.status != 0
            || !runtime::ai_installed()
        {
            return BridgeStatus::InvalidInput as u32;
        }
        if config.game_pid != unsafe { GetCurrentProcessId() }
            || current_created_filetime() != Some(config.created_filetime)
            || config.generation != runtime::generation()
        {
            return BridgeStatus::ProcessIdentity as u32;
        }
        let binding = PRODUCT_OWNERS[config.owner as usize].load();
        let Some(player) = joined_receiver(binding).checked_sub(0x528) else {
            return BridgeStatus::ProcessIdentity as u32;
        };
        if LEASE.begin(config.owner, player).is_none() {
            return BridgeStatus::InvalidInput as u32;
        }
        // Fixed 2F9378/2F95AE use Player+6B0. 401D30 binds AI+90 to Player.
        let Some(ai_owner) = player
            .checked_add(0x6b0)
            .and_then(copy_current::<8>)
            .map(u64::from_le_bytes)
            .filter(|p| *p != 0)
        else {
            return BridgeStatus::InvalidInput as u32;
        };
        let base =
            unsafe { GetModuleHandleW(wide("GameCore_XP2_FinalRelease.dll").as_ptr()) } as u64;
        let result = unsafe { register_owner(base, config.owner, ai_owner, config.governor_index) };
        let mut status = match result {
            Ok(()) => BridgeStatus::Ok,
            Err(gateway::Failure::CriticalRestore) => BridgeStatus::Critical,
            _ => BridgeStatus::HookFailure,
        } as u32;
        // Never transiently enable actions before registration succeeds.
        if status == BridgeStatus::Ok as u32
            && ai_operation::configure(base, config.execution_mode == 1).is_err()
        {
            status = if close() {
                BridgeStatus::GameCoreIdentity
            } else {
                BridgeStatus::Critical
            } as u32;
        }
        if status != BridgeStatus::Ok as u32 {
            ai_operation::cancel();
        }
        config.status = status;
        unsafe { ptr::write(input.cast::<AiObservationConfig>(), config) };
        status
    })
    .unwrap_or(BridgeStatus::Panic as u32)
}
