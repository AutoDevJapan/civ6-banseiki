//! Paid-assignment consumer, isolated from appointment permission.
//!
//! This is the bounded B1 component, not a new installed hook mode. A future
//! composite controller must connect these methods to the same natural Core
//! getter and four-argument CanAssign invocation. Mode 7 remains observational.
//! Only a synchronized, authenticated producer may publish payment evidence;
//! these properties do not establish that producer's GameInfo provenance.
use super::*;

pub const PAID_MANIFEST_BASE: u32 = 904_000;
pub const PAID_ALLOW_BASE: u32 = 905_000;
pub const PAID_CLOSE_BASE: u32 = 906_000;
pub const PAID_RESET_BASE: u32 = 907_000;

static MANIFEST: [AtomicJoinedWord; PRODUCT_CELL_COUNT] =
    [const { AtomicJoinedWord::new() }; PRODUCT_CELL_COUNT];
static PAYMENT: [AtomicJoinedWord; PRODUCT_CELL_COUNT] =
    [const { AtomicJoinedWord::new() }; PRODUCT_CELL_COUNT];
static CLOSURE: [AtomicJoinedWord; PRODUCT_CELL_COUNT] =
    [const { AtomicJoinedWord::new() }; PRODUCT_CELL_COUNT];
static ACK: [AtomicJoinedWord; PRODUCT_CELL_COUNT] =
    [const { AtomicJoinedWord::new() }; PRODUCT_CELL_COUNT];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    Manifest(usize),
    Payment(usize),
    Close(usize),
    Reset(usize),
}

fn key(value: u32) -> Option<Key> {
    for (base, kind) in [
        (PAID_MANIFEST_BASE, Key::Manifest as fn(usize) -> Key),
        (PAID_ALLOW_BASE, Key::Payment),
        (PAID_CLOSE_BASE, Key::Close),
    ] {
        if (base..base + PRODUCTION_INDEX_LIMIT as u32).contains(&value) {
            return Some(kind((value - base) as usize));
        }
    }
    (PAID_RESET_BASE..PAID_RESET_BASE + PRODUCTION_OWNER_LIMIT as u32)
        .contains(&value)
        .then(|| Key::Reset((value - PAID_RESET_BASE) as usize))
}

pub(super) fn is_key(value: u32) -> bool {
    key(value).is_some()
}

fn live(session: u32) -> bool {
    PRODUCT_ACTIVE.load(Ordering::Acquire)
        && !PRODUCT_EXHAUSTED.load(Ordering::Acquire)
        && PRODUCT_SESSION.load(Ordering::Acquire) == session
}

fn known(index: usize, session: u32) -> bool {
    index < PRODUCTION_INDEX_LIMIT
        && (product_index_known(index, session)
            || (0..PRODUCTION_OWNER_LIMIT).any(|owner| {
                let offset = owner * PRODUCTION_INDEX_LIMIT + index;
                [MANIFEST[offset].load(), CLOSURE[offset].load()]
                    .into_iter()
                    .any(|word| joined_session(word) == session && joined_receiver(word) != 0)
            }))
}

/// Finite consumer of copied, natural Core property returns. It uses the
/// appointment consumer's owner bindings, session and entry-ticket allocator,
/// but has independent manifest/payment/closure/ACK cells. Appointment allow
/// can never become payment allow. There is no install/exported DLL entry here.
pub struct PaidAssignmentConsumer;

/// One getter invocation. Fields stay private so a return cannot accidentally
/// be applied to a different owner, key, receiver or entry ticket.
pub struct PaidAssignmentRead {
    receiver: u64,
    property_key: u32,
    token: u64,
}

impl PaidAssignmentRead {
    pub(super) fn from_token(receiver: u64, property_key: u32, token: u64) -> Self {
        Self {
            receiver,
            property_key,
            token,
        }
    }
}

struct InvocationMemory;
impl crate::governor_payment::CurrentMemory for InvocationMemory {
    fn copy<const N: usize>(&self, address: u64) -> Option<[u8; N]> {
        copy_current(address)
    }
}

impl PaidAssignmentConsumer {
    /// Allocate once before the original natural getter. A close/reset takes
    /// effect now even if that getter later returns a stale or malformed value.
    /// None means rejected receiver, unknown key or inactive/exhausted session.
    pub fn begin(receiver: u64, property_key: u32) -> Option<PaidAssignmentRead> {
        let token = Self::begin_token(receiver, property_key);
        (token != 0).then_some(PaidAssignmentRead {
            receiver,
            property_key,
            token,
        })
    }

    pub(super) fn begin_token(receiver: u64, property_key: u32) -> u64 {
        let Some(kind) = key(property_key) else {
            return 0;
        };
        if let Key::Reset(owner) = kind {
            // One binding threshold closes both admission and assignment. The
            // owner-specific natural reset path closes before receiver cleanup.
            return product_property_begin(receiver, PRODUCTION_RESET_KEY + owner as u32);
        }
        if receiver == 0 || receiver & 7 != 0 {
            return 0;
        }
        let session = PRODUCT_SESSION.load(Ordering::Acquire);
        if !live(session) {
            return 0;
        }
        let mut found = None;
        for (owner, cell) in PRODUCT_OWNERS.iter().enumerate() {
            let word = cell.load();
            if joined_session(word) == session && joined_receiver(word) == receiver {
                if found.is_some() {
                    return 0;
                }
                found = Some(owner);
            }
        }
        let Some(owner) = found else {
            return 0;
        };
        let Some(ticket) = product_next_ticket() else {
            return 0;
        };
        let binding = PRODUCT_OWNERS[owner].load();
        if !live(session)
            || joined_session(binding) != session
            || joined_receiver(binding) != receiver
            || joined_ticket(binding) >= ticket
        {
            return 0;
        }
        if let Key::Close(index) = kind {
            product_publish_word(
                &CLOSURE[owner * PRODUCTION_INDEX_LIMIT + index],
                receiver,
                session,
                ticket,
                false,
            );
        }
        (u64::from(session) << 37) | ((owner as u64) << 31) | u64::from(ticket)
    }

    /// Consume the one copied 24-byte property result, never borrowed storage.
    /// `state=1, tag=1, flags=0, payload=0/1` is the only accepted bool encoding.
    /// This method does not read a getter or execute Lua.
    pub fn observe(read: PaidAssignmentRead, state: u32, tag: u32, payload: u64, flags: u32) {
        let PaidAssignmentRead {
            receiver,
            property_key,
            token,
        } = read;
        let (index, manifest) = match key(property_key) {
            Some(Key::Manifest(index)) => (index, true),
            Some(Key::Payment(index)) => (index, false),
            _ => return,
        };
        let session = (token >> 37) as u32;
        let owner = ((token >> 31) & 63) as usize;
        let ticket = token as u32 & JOINED_TICKET_MAX;
        if token == 0 || ticket == 0 || !live(session) {
            return;
        }
        let binding = PRODUCT_OWNERS[owner].load();
        if joined_session(binding) != session
            || joined_receiver(binding) != receiver
            || joined_ticket(binding) >= ticket
        {
            return;
        }
        let offset = owner * PRODUCTION_INDEX_LIMIT + index;
        let closure = CLOSURE[offset].load();
        if joined_session(closure) != session
            || joined_receiver(closure) != receiver
            || joined_ticket(closure) <= joined_ticket(binding)
            || joined_ticket(closure) >= ticket
        {
            return;
        }
        let typed = state == 1 && tag == 1 && flags == 0 && payload <= 1;
        if !manifest && typed && payload == 0 {
            product_publish_word(&ACK[offset], receiver, session, ticket, true);
        }
        let ack = ACK[offset].load();
        let acknowledged = joined_session(ack) == session
            && joined_receiver(ack) == receiver
            && joined_ticket(ack) > joined_ticket(closure)
            && joined_permitted(ack);
        product_publish_word(
            if manifest {
                &MANIFEST[offset]
            } else {
                &PAYMENT[offset]
            },
            receiver,
            session,
            ticket,
            typed && payload == 1 && acknowledged,
        );
    }

    /// Read using scalar identity and receiver copied in the current invocation.
    /// None identifies an Index absent from both current persona manifests;
    /// the composite gate must finish catalog initialization before installing.
    /// Some(false) closes a known persona; Some(true) only limits an original
    /// true result, and is not permission to call the assignment setter.
    pub fn permission(owner: usize, index: usize, receiver: u64, session: u32) -> Option<bool> {
        if composite::ENABLED.load(Ordering::Acquire) {
            let Some(catalog) = composite::ready_catalog(owner, receiver, session) else {
                return Some(false);
            };
            if !catalog
                .iter()
                .any(|definition| definition.governor_index as usize == index)
            {
                return None;
            }
        }
        let current_session = PRODUCT_SESSION.load(Ordering::Acquire);
        if !known(index, current_session) {
            return if composite::ENABLED.load(Ordering::Acquire) {
                Some(false)
            } else {
                None
            };
        }
        if !live(session) || owner >= PRODUCTION_OWNER_LIMIT || receiver == 0 {
            return Some(false);
        }
        let binding = PRODUCT_OWNERS[owner].load();
        if joined_session(binding) != session || joined_receiver(binding) != receiver {
            return Some(false);
        }
        for (other, cell) in PRODUCT_OWNERS.iter().enumerate() {
            if other != owner {
                let word = cell.load();
                if joined_session(word) == session && joined_receiver(word) == receiver {
                    return Some(false);
                }
            }
        }
        let offset = owner * PRODUCTION_INDEX_LIMIT + index;
        let closure = CLOSURE[offset].load();
        let ack = ACK[offset].load();
        let manifest = MANIFEST[offset].load();
        let payment = PAYMENT[offset].load();
        let words = [closure, ack, manifest, payment];
        let identity = words
            .iter()
            .all(|word| joined_session(*word) == session && joined_receiver(*word) == receiver);
        let allowed = identity
            && (!composite::ENABLED.load(Ordering::Acquire)
                || composite::payment_after_catalog(owner, joined_ticket(payment)))
            && joined_ticket(closure) > joined_ticket(binding)
            && joined_ticket(ack) > joined_ticket(closure)
            && joined_ticket(manifest) > joined_ticket(ack)
            && joined_ticket(payment) > joined_ticket(manifest)
            && [ack, manifest, payment]
                .iter()
                .all(|word| joined_permitted(*word));
        Some(
            allowed
                && live(session)
                && PRODUCT_OWNERS[owner].load() == binding
                && CLOSURE[offset].load() == closure
                && ACK[offset].load() == ack
                && MANIFEST[offset].load() == manifest
                && PAYMENT[offset].load() == payment,
        )
    }

    /// Keep all of original RAX except clear AL for a denied known persona.
    /// An original false is never turned true, even after full payment.
    pub fn restrict(original: u64, owner: usize, index: usize, receiver: u64, session: u32) -> u64 {
        if owner >= PRODUCTION_OWNER_LIMIT
            || index >= PRODUCTION_INDEX_LIMIT
            || receiver == 0
            || receiver & 7 != 0
            || session == 0
            || Self::permission(owner, index, receiver, session) == Some(false)
        {
            original & !0xff
        } else {
            original
        }
    }

    /// Adapter for the *current* four-argument invocation after the original
    /// validation returns. Only its two input objects are read. Addresses never
    /// leave the call and no getter is invoked. This is not an installed hook.
    pub fn restrict_current(original: u64, governors: u64, governor: u64) -> u64 {
        let identity = (|| {
            let person = copy_current::<8>(governor)?;
            let index = u32::from_le_bytes(person[4..].try_into().ok()?) as usize;
            let owner_bytes = copy_current::<4>(
                governor.checked_add(crate::governor_payment::CORE_GOVERNOR_OWNER_OFFSET)?,
            )?;
            let actual_owner = u32::from_le_bytes(owner_bytes);
            let player = u64::from_le_bytes(copy_current::<8>(governors.checked_add(0x70)?)?);
            let owner = u32::from_le_bytes(copy_current::<4>(player.checked_add(0xd8)?)?);
            let receiver = player.checked_add(0x528)?;
            if actual_owner != owner
                || owner as usize >= PRODUCTION_OWNER_LIMIT
                || index >= PRODUCTION_INDEX_LIMIT
                || copy_current::<8>(governor)? != person
                || copy_current::<4>(
                    governor.checked_add(crate::governor_payment::CORE_GOVERNOR_OWNER_OFFSET)?,
                )? != owner_bytes
            {
                return None;
            }
            Some((owner as usize, index, receiver))
        })();
        let Some((owner, index, receiver)) = identity else {
            return original & !0xff;
        };
        let session = PRODUCT_SESSION.load(Ordering::Acquire);
        let definition = if composite::ENABLED.load(Ordering::Acquire) {
            let Some(catalog) = composite::ready_catalog(owner, receiver, session) else {
                return original & !0xff;
            };
            match catalog
                .into_iter()
                .find(|definition| definition.governor_index as usize == index)
            {
                None => return original,
                Some(definition) => Some(definition),
            }
        } else {
            None
        };
        match Self::permission(owner, index, receiver, session) {
            // The composite gate first completes the persona catalog. A
            // classified ordinary Governor need not have a persona-compatible
            // promotion vector; Core's original result must survive that case.
            None => return original,
            Some(false) => return original & !0xff,
            Some(true) => {}
        }
        let Ok(person) =
            crate::governor_payment::read_current(&InvocationMemory, governors, governor)
        else {
            return original & !0xff;
        };
        if person.owner as usize != owner || person.governor_index as usize != index {
            return original & !0xff;
        }
        if let Some(definition) = definition {
            if person.diagnostic_paid(owner as u32, definition) != Ok(definition.cost)
                || !composite::ready_catalog(owner, receiver, session)
                    .is_some_and(|catalog| catalog.into_iter().any(|current| current == definition))
            {
                return original & !0xff;
            }
        }
        Self::restrict(original, owner, index, receiver, session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start(owner: usize, receiver: u64) -> u32 {
        product_deactivate();
        assert!(product_activate());
        let token = PaidAssignmentConsumer::begin(receiver, PAID_RESET_BASE + owner as u32);
        assert!(token.is_some());
        PRODUCT_SESSION.load(Ordering::Acquire)
    }

    fn publish(receiver: u64, key: u32, payload: u64) {
        let token = PaidAssignmentConsumer::begin(receiver, key);
        assert!(token.is_some());
        PaidAssignmentConsumer::observe(token.unwrap(), 1, 1, payload, 0);
    }

    fn closed(receiver: u64, index: u32) {
        publish(receiver, PAID_CLOSE_BASE + index, 1);
        publish(receiver, PAID_ALLOW_BASE + index, 0);
        publish(receiver, PAID_MANIFEST_BASE + index, 1);
    }

    #[test]
    fn appointment_and_other_person_payment_never_open_assignment() {
        let _guard = super::super::tests::HOOK_TEST_LOCK.lock().unwrap();
        let session = start(2, 0x1000);
        let admission = |key, payload| {
            let token = product_property_begin(0x1000, key);
            product_observe_property(0x1000, key, token, 1, 1, payload, 0);
        };
        admission(PRODUCTION_CLOSE_BASE + 77, 1);
        admission(PRODUCTION_ALLOW_BASE + 77, 0);
        admission(PRODUCTION_MANIFEST_BASE + 77, 1);
        admission(PRODUCTION_ALLOW_BASE + 77, 1);
        assert_eq!(product_permission(2, 77, 0x1000, session), Some(true));
        assert_eq!(
            PaidAssignmentConsumer::permission(2, 77, 0x1000, session),
            Some(false)
        );
        closed(0x1000, 78);
        publish(0x1000, PAID_ALLOW_BASE + 78, 1);
        assert_eq!(
            PaidAssignmentConsumer::permission(2, 78, 0x1000, session),
            Some(true)
        );
        assert_eq!(
            PaidAssignmentConsumer::permission(2, 77, 0x1000, session),
            Some(false)
        );
        assert_eq!(
            PaidAssignmentConsumer::restrict(0xaabbccdd01, 2, 77, 0x1000, session),
            0xaabbccdd00
        );
        assert_eq!(
            PaidAssignmentConsumer::restrict(0xaabbccdd00, 2, 78, 0x1000, session),
            0xaabbccdd00
        );
        assert_eq!(
            PaidAssignmentConsumer::restrict(0xaabbccdd01, 2, 79, 0x1000, session),
            0xaabbccdd01
        );
        product_deactivate();
    }

    #[test]
    fn old_true_requires_a_new_typed_closed_ack_and_manifest() {
        let _guard = super::super::tests::HOOK_TEST_LOCK.lock().unwrap();
        let session = start(2, 0x1000);
        closed(0x1000, 77);
        publish(0x1000, PAID_ALLOW_BASE + 77, 1);
        let late = PaidAssignmentConsumer::begin(0x1000, PAID_ALLOW_BASE + 77);
        let close = PaidAssignmentConsumer::begin(0x1000, PAID_CLOSE_BASE + 77);
        assert!(close.is_some());
        assert_eq!(
            PaidAssignmentConsumer::permission(2, 77, 0x1000, session),
            Some(false)
        );
        PaidAssignmentConsumer::observe(late.unwrap(), 1, 1, 1, 0);
        publish(0x1000, PAID_ALLOW_BASE + 77, 1);
        publish(0x1000, PAID_MANIFEST_BASE + 77, 1);
        assert_eq!(
            PaidAssignmentConsumer::permission(2, 77, 0x1000, session),
            Some(false)
        );
        // A numeric zero is not the required bool false ACK.
        let zero = PaidAssignmentConsumer::begin(0x1000, PAID_ALLOW_BASE + 77);
        PaidAssignmentConsumer::observe(zero.unwrap(), 1, 2, 0, 0);
        publish(0x1000, PAID_MANIFEST_BASE + 77, 1);
        publish(0x1000, PAID_ALLOW_BASE + 77, 1);
        assert_eq!(
            PaidAssignmentConsumer::permission(2, 77, 0x1000, session),
            Some(false)
        );
        publish(0x1000, PAID_ALLOW_BASE + 77, 0);
        publish(0x1000, PAID_MANIFEST_BASE + 77, 1);
        publish(0x1000, PAID_ALLOW_BASE + 77, 1);
        assert_eq!(
            PaidAssignmentConsumer::permission(2, 77, 0x1000, session),
            Some(true)
        );
        product_deactivate();
    }

    #[test]
    fn malformed_values_reset_rebind_and_cancel_close_the_same_person() {
        let _guard = super::super::tests::HOOK_TEST_LOCK.lock().unwrap();
        let session = start(2, 0x1000);
        for (state, tag, payload, flags) in [
            (0, 1, 1, 0),
            (2, 1, 1, 0),
            (1, 2, 1, 0),
            (1, 1, 2, 0),
            (1, 1, 1, 1),
        ] {
            closed(0x1000, 77);
            publish(0x1000, PAID_ALLOW_BASE + 77, 1);
            let token = PaidAssignmentConsumer::begin(0x1000, PAID_ALLOW_BASE + 77);
            PaidAssignmentConsumer::observe(token.unwrap(), state, tag, payload, flags);
            assert_eq!(
                PaidAssignmentConsumer::permission(2, 77, 0x1000, session),
                Some(false)
            );
        }
        closed(0x1000, 77);
        publish(0x1000, PAID_ALLOW_BASE + 77, 1);
        let late = PaidAssignmentConsumer::begin(0x1000, PAID_ALLOW_BASE + 77);
        assert_ne!(product_property_begin(0x1000, PRODUCTION_RESET_KEY + 2), 0);
        PaidAssignmentConsumer::observe(late.unwrap(), 1, 1, 1, 0);
        assert_eq!(
            PaidAssignmentConsumer::permission(2, 77, 0x1000, session),
            Some(false)
        );
        closed(0x1000, 77);
        publish(0x1000, PAID_ALLOW_BASE + 77, 1);
        assert!(PaidAssignmentConsumer::begin(0x1000, PAID_RESET_BASE + 3).is_some());
        assert_eq!(
            PaidAssignmentConsumer::permission(2, 77, 0x1000, session),
            Some(false)
        );
        assert_eq!(
            PaidAssignmentConsumer::permission(3, 77, 0x1000, session),
            Some(false)
        );
        closed(0x1000, 77);
        publish(0x1000, PAID_ALLOW_BASE + 77, 1);
        assert_eq!(
            PaidAssignmentConsumer::permission(3, 77, 0x1000, session),
            Some(true)
        );
        assert_eq!(
            PaidAssignmentConsumer::permission(64, 77, 0x1000, session),
            Some(false)
        );
        assert_eq!(
            PaidAssignmentConsumer::permission(3, 77, 0x2000, session),
            Some(false)
        );
        assert!(PaidAssignmentConsumer::begin(0x2000, PAID_ALLOW_BASE + 77).is_none());
        assert!(PaidAssignmentConsumer::begin(0x1001, PAID_ALLOW_BASE + 77).is_none());
        product_deactivate();
        assert_eq!(
            PaidAssignmentConsumer::permission(3, 77, 0x1000, session),
            Some(false)
        );
    }

    #[test]
    fn an_old_session_cannot_republish_after_reload_or_ticket_exhaustion() {
        let _guard = super::super::tests::HOOK_TEST_LOCK.lock().unwrap();
        let old = start(2, 0x1000);
        closed(0x1000, 77);
        let late = PaidAssignmentConsumer::begin(0x1000, PAID_ALLOW_BASE + 77);
        let session = start(2, 0x1000);
        closed(0x1000, 77);
        PaidAssignmentConsumer::observe(late.unwrap(), 1, 1, 1, 0);
        assert_eq!(
            PaidAssignmentConsumer::permission(2, 77, 0x1000, session),
            Some(false)
        );
        publish(0x1000, PAID_ALLOW_BASE + 77, 1);
        assert_eq!(
            PaidAssignmentConsumer::permission(2, 77, 0x1000, old),
            Some(false)
        );
        assert_eq!(
            PaidAssignmentConsumer::permission(2, 77, 0x1000, session),
            Some(true)
        );
        PRODUCT_NEXT_TICKET.store(JOINED_TICKET_MAX, Ordering::Release);
        assert!(PaidAssignmentConsumer::begin(0x1000, PAID_ALLOW_BASE + 77).is_none());
        assert_eq!(
            PaidAssignmentConsumer::permission(2, 77, 0x1000, session),
            Some(false)
        );
        product_deactivate();
    }

    #[test]
    fn namespace_boundaries_and_false_manifest_do_not_reuse_payment() {
        let _guard = super::super::tests::HOOK_TEST_LOCK.lock().unwrap();
        let session = start(63, 0x1000);
        for ordinary_key in [
            PRODUCTION_MANIFEST_BASE,
            PRODUCTION_ALLOW_BASE,
            PRODUCTION_RESET_KEY,
            PRODUCTION_CLOSE_BASE,
            PAID_ALLOW_BASE + 256,
            PAID_RESET_BASE + 64,
        ] {
            assert!(PaidAssignmentConsumer::begin(0x1000, ordinary_key).is_none());
        }
        for index in [0, 255] {
            closed(0x1000, index);
            publish(0x1000, PAID_ALLOW_BASE + index, 1);
            assert_eq!(
                PaidAssignmentConsumer::permission(63, index as usize, 0x1000, session),
                Some(true)
            );
            publish(0x1000, PAID_MANIFEST_BASE + index, 0);
            assert_eq!(
                PaidAssignmentConsumer::permission(63, index as usize, 0x1000, session),
                Some(false)
            );
            publish(0x1000, PAID_MANIFEST_BASE + index, 1);
            assert_eq!(
                PaidAssignmentConsumer::permission(63, index as usize, 0x1000, session),
                Some(false),
                "old payment predates the new manifest"
            );
        }
        for (owner, index, receiver, generation) in [
            (64, 79, 0x1000, session),
            (63, 256, 0x1000, session),
            (63, 79, 0, session),
            (63, 79, 0x1001, session),
            (63, 79, 0x1000, 0),
        ] {
            assert_eq!(
                PaidAssignmentConsumer::restrict(0xaabb01, owner, index, receiver, generation),
                0xaabb00
            );
        }
        product_deactivate();
    }

    #[test]
    fn reset_closes_paid_before_concurrent_receiver_cleanup() {
        let _guard = super::super::tests::HOOK_TEST_LOCK.lock().unwrap();
        let session = start(2, 0x1000);
        closed(0x1000, 77);
        publish(0x1000, PAID_ALLOW_BASE + 77, 1);
        PRODUCT_TEST_RESET_PAUSED.store(false, Ordering::Release);
        PRODUCT_TEST_RESET_RELEASE.store(false, Ordering::Release);
        PRODUCT_TEST_RESET_PAUSE.store(true, Ordering::Release);
        let worker =
            std::thread::spawn(|| PaidAssignmentConsumer::begin(0x2000, PAID_RESET_BASE + 2));
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !PRODUCT_TEST_RESET_PAUSED.load(Ordering::Acquire)
            && std::time::Instant::now() < deadline
        {
            std::thread::yield_now();
        }
        let paused = PRODUCT_TEST_RESET_PAUSED.load(Ordering::Acquire);
        let permission = PaidAssignmentConsumer::permission(2, 77, 0x1000, session);
        // Release and join before asserting, including the timeout path.
        PRODUCT_TEST_RESET_RELEASE.store(true, Ordering::Release);
        let reset = worker.join();
        PRODUCT_TEST_RESET_PAUSE.store(false, Ordering::Release);
        product_deactivate();
        assert!(paused);
        assert!(reset.unwrap().is_some());
        assert_eq!(permission, Some(false));
    }
}
