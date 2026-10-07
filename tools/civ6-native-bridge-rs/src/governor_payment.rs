//! Copies the pinned Core Governor's own promotion flags during a natural
//! CanAssign invocation. This is a read-only snapshot, never a persisted permit.
//! No Core getter is called and no game memory is changed.

pub const PROMOTION_LIMIT: usize = 4096;
pub const PROMOTION_WORDS: usize = PROMOTION_LIMIT / 64;
const GOVERNOR_LIMIT: usize = 256;
pub(crate) const CORE_GOVERNOR_OWNER_OFFSET: u64 = 0x18;

pub trait CurrentMemory {
    fn copy<const N: usize>(&self, address: u64) -> Option<[u8; N]>;
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadFailure {
    Unreadable = 1,
    InvalidLayout = 2,
    IdentityMismatch = 3,
    NotMember = 4,
    Changed = 5,
    InvalidFlag = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromotionSnapshot {
    pub owner: u32,
    pub governor_index: u32,
    pub count: u32,
    pub owned: [u64; PROMOTION_WORDS],
}

/// Indices must be supplied by a validated GameInfo persona definition. They
/// identify which copied Core bytes to compare; they are not payment evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaymentDefinition {
    pub governor_index: u32,
    pub cost: u8,
    pub base: u32,
    pub titles: [Option<u32>; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentFailure {
    Definition,
    DifferentOwner,
    DifferentPerson,
    UnknownPromotion,
    OutOfOrder,
}

impl PromotionSnapshot {
    pub fn has(&self, index: u32) -> Option<bool> {
        (index < self.count && index < PROMOTION_LIMIT as u32)
            .then(|| self.owned[index as usize / 64] & (1 << (index % 64)) != 0)
    }

    /// Diagnostic interpretation of caller-supplied GameInfo indices, not an
    /// authenticated definition or a payment permit. Production must first
    /// validate metadata against that game's synchronized GameInfo catalog.
    /// Aggregate Spent and another owner's/person's flags are never consulted.
    pub fn diagnostic_paid(
        &self,
        owner: u32,
        definition: PaymentDefinition,
    ) -> Result<u8, PaymentFailure> {
        if owner >= 64 || self.owner != owner {
            return Err(PaymentFailure::DifferentOwner);
        }
        if !(1..=3).contains(&definition.cost)
            || definition.governor_index >= GOVERNOR_LIMIT as u32
            || definition.titles[0].is_some() != (definition.cost >= 2)
            || definition.titles[1].is_some() != (definition.cost == 3)
            || definition
                .titles
                .iter()
                .flatten()
                .any(|index| *index == definition.base)
            || (definition.cost == 3 && definition.titles[0] == definition.titles[1])
        {
            return Err(PaymentFailure::Definition);
        }
        if self.governor_index != definition.governor_index {
            return Err(PaymentFailure::DifferentPerson);
        }
        let base = self
            .has(definition.base)
            .ok_or(PaymentFailure::UnknownPromotion)?;
        let mut paid = u8::from(base);
        let mut missing = !base;
        for index in definition.titles.into_iter().flatten() {
            let owned = self.has(index).ok_or(PaymentFailure::UnknownPromotion)?;
            if owned && missing {
                return Err(PaymentFailure::OutOfOrder);
            }
            if owned {
                paid += 1;
            } else {
                missing = true;
            }
        }
        Ok(paid)
    }
}

/// Who an official PROMOTE checker result belongs to, as already identified by
/// the caller from its ready catalog. `Standard` is only valid for an Index the
/// caller found absent from that catalog; unknown or unready catalogs must be
/// passed as a `Persona` without a snapshot so that they close.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromotionSubject {
    Standard,
    Persona {
        definition: PaymentDefinition,
        /// Expected owner from the current invocation (Governors+0x70 -> +0xd8).
        owner: u32,
        /// None when the person's own flags could not be read consistently.
        snapshot: Option<PromotionSnapshot>,
        /// Synchronized join/evolution admission for this owner and person.
        joined: bool,
    },
}

/// Pure promotion policy. It never turns an original false into true and
/// keeps every RAX bit except AL. A persona may only take the next unpaid
/// evolution TITLE of its saved cost; base, extra TITLEs, skills (bought via
/// property, never PROMOTE) and anything after full payment are denied.
/// Partial payment is allowed here, unlike the paid-assignment gate.
/// Not connected to any runtime call site yet.
pub fn restrict_promotion(
    original: u64,
    subject: PromotionSubject,
    requested: u32,
    call_current: bool,
) -> u64 {
    if original & 0xff == 0 {
        return original;
    }
    let PromotionSubject::Persona {
        definition,
        owner,
        snapshot,
        joined,
    } = subject
    else {
        return original;
    };
    let next = snapshot
        .filter(|_| joined && call_current)
        .and_then(|s| s.diagnostic_paid(owner, definition).ok())
        .filter(|paid| (1..definition.cost).contains(paid))
        .and_then(|paid| definition.titles[usize::from(paid) - 1]);
    if next == Some(requested) {
        original
    } else {
        original & !0xff
    }
}

fn copy<const N: usize>(memory: &impl CurrentMemory, address: u64) -> Result<[u8; N], ReadFailure> {
    if address == 0 || address.checked_add(N as u64).is_none() {
        return Err(ReadFailure::Unreadable);
    }
    memory.copy(address).ok_or(ReadFailure::Unreadable)
}

fn field<const N: usize>(
    memory: &impl CurrentMemory,
    base: u64,
    offset: u64,
) -> Result<[u8; N], ReadFailure> {
    copy(
        memory,
        base.checked_add(offset).ok_or(ReadFailure::Unreadable)?,
    )
}

fn member(memory: &impl CurrentMemory, head: u64, governor: u64) -> Result<(), ReadFailure> {
    let mut node = head;
    let mut visited = [0u64; GOVERNOR_LIMIT];
    for step in 0..GOVERNOR_LIMIT {
        if node == 0 {
            return Err(ReadFailure::NotMember);
        }
        // A repeated node is rejected by the finite traversal even if a corrupt
        // chain would otherwise eventually point at the requested Governor.
        if visited[..step].contains(&node) {
            return Err(ReadFailure::InvalidLayout);
        }
        visited[step] = node;
        let entry = copy::<24>(memory, node)?;
        if u64::from_le_bytes(entry[..8].try_into().unwrap()) == governor {
            return Ok(());
        }
        let next = u64::from_le_bytes(entry[16..24].try_into().unwrap());
        if next == node {
            return Err(ReadFailure::InvalidLayout);
        }
        node = next;
    }
    Err(ReadFailure::InvalidLayout)
}

fn flags(
    memory: &impl CurrentMemory,
    address: u64,
    count: usize,
) -> Result<[u8; PROMOTION_LIMIT], ReadFailure> {
    let mut bytes = [0; PROMOTION_LIMIT];
    let bulk = count / 256 * 256;
    for offset in (0..bulk).step_by(256) {
        bytes[offset..offset + 256].copy_from_slice(&field::<256>(memory, address, offset as u64)?);
    }
    let complete = count / 8 * 8;
    for offset in (bulk..complete).step_by(8) {
        bytes[offset..offset + 8].copy_from_slice(&field::<8>(memory, address, offset as u64)?);
    }
    for (offset, byte) in bytes.iter_mut().enumerate().take(count).skip(complete) {
        *byte = field::<1>(memory, address, offset as u64)?[0];
    }
    Ok(bytes)
}

/// `governors` and `governor` must be the two current CanAssign arguments.
/// Result contains scalar identities and copied flags, never a reusable pointer.
pub fn read_current(
    memory: &impl CurrentMemory,
    governors: u64,
    governor: u64,
) -> Result<PromotionSnapshot, ReadFailure> {
    let actor = field::<8>(memory, governors, 0x70)?;
    let player = u64::from_le_bytes(actor);
    let owner_bytes = field::<4>(memory, player, 0xd8)?;
    let owner = u32::from_le_bytes(owner_bytes);
    if owner >= 64 {
        return Err(ReadFailure::IdentityMismatch);
    }
    let head_bytes = field::<8>(memory, governors, 0x58)?;
    let head = u64::from_le_bytes(head_bytes);
    member(memory, head, governor)?;
    let identity = copy::<8>(memory, governor)?;
    // Pinned APPOINT 2dd069 writes an instance identifier at +0;
    // 2dd071..2dd07c copies Player+d8 to the Core Governor owner at +18.
    // UI CacheGovernor getters have a different layout.
    let governor_owner = field::<4>(memory, governor, CORE_GOVERNOR_OWNER_OFFSET)?;
    let actual_owner = u32::from_le_bytes(governor_owner);
    let index = u32::from_le_bytes(identity[4..].try_into().unwrap());
    if actual_owner != owner || index >= GOVERNOR_LIMIT as u32 {
        return Err(ReadFailure::IdentityMismatch);
    }
    // Pinned 259980 reads flags at +68. 2598f0 counts nonzero bytes up to
    // length +78. 259990 allocates capacity +70, then publishes length +78.
    let header = field::<20>(memory, governor, 0x68)?;
    let flag_address = u64::from_le_bytes(header[..8].try_into().unwrap());
    let capacity = u32::from_le_bytes(header[8..12].try_into().unwrap());
    let count = u32::from_le_bytes(header[16..20].try_into().unwrap());
    if count == 0 || count > capacity || capacity > PROMOTION_LIMIT as u32 || flag_address == 0 {
        return Err(ReadFailure::InvalidLayout);
    }
    let bytes = flags(memory, flag_address, count as usize)?;
    let mut owned = [0u64; PROMOTION_WORDS];
    for index in 0..count as usize {
        let value = bytes[index];
        if value > 1 {
            return Err(ReadFailure::InvalidFlag);
        }
        if value == 1 {
            owned[index / 64] |= 1 << (index % 64);
        }
    }
    // Repeat all selected bytes and identities. This detects torn observations;
    // it does not assert global quiescence or turn a snapshot into authority.
    if flags(memory, flag_address, count as usize)? != bytes {
        return Err(ReadFailure::Changed);
    }
    if field::<8>(memory, governors, 0x70)? != actor
        || field::<4>(memory, player, 0xd8)? != owner_bytes
        || field::<8>(memory, governors, 0x58)? != head_bytes
        || copy::<8>(memory, governor)? != identity
        || field::<4>(memory, governor, CORE_GOVERNOR_OWNER_OFFSET)? != governor_owner
        || field::<20>(memory, governor, 0x68)? != header
    {
        return Err(ReadFailure::Changed);
    }
    member(memory, head, governor)?;
    Ok(PromotionSnapshot {
        owner,
        governor_index: index,
        count,
        owned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Memory {
        bytes: [u8; 32768],
        mutate: Cell<Option<u64>>,
        reads: Cell<usize>,
    }
    impl CurrentMemory for Memory {
        fn copy<const N: usize>(&self, address: u64) -> Option<[u8; N]> {
            let count = self.reads.get();
            self.reads.set(count + 1);
            let start = usize::try_from(address).ok()?;
            let mut value: [u8; N] = self
                .bytes
                .get(start..start.checked_add(N)?)?
                .try_into()
                .ok()?;
            if self.mutate.get() == Some(address) && count > 12 {
                value[0] ^= 1;
            }
            Some(value)
        }
    }
    fn fixture() -> Memory {
        let mut m = Memory {
            bytes: [0; 32768],
            mutate: Cell::new(None),
            reads: Cell::new(0),
        };
        put(&mut m, 0x1070, &0x2000u64.to_le_bytes());
        put(&mut m, 0x20d8, &7u32.to_le_bytes());
        put(&mut m, 0x1058, &0x3000u64.to_le_bytes());
        put(&mut m, 0x3000, &0x3800u64.to_le_bytes());
        put(&mut m, 0x3800, &0x10001u32.to_le_bytes());
        put(&mut m, 0x3804, &22u32.to_le_bytes());
        put(&mut m, 0x3818, &7u32.to_le_bytes());
        put(&mut m, 0x3868, &0x4000u64.to_le_bytes());
        put(&mut m, 0x3870, &4096u32.to_le_bytes());
        put(&mut m, 0x3878, &4096u32.to_le_bytes());
        for i in [0, 63, 64, 1000, 4095] {
            m.bytes[0x4000 + i] = 1;
        }
        m
    }
    fn put(m: &mut Memory, address: usize, bytes: &[u8]) {
        m.bytes[address..address + bytes.len()].copy_from_slice(bytes);
    }

    #[test]
    fn copies_this_governors_own_flags_at_word_and_capacity_boundaries() {
        let m = fixture();
        let before = m.bytes;
        let s = read_current(&m, 0x1000, 0x3800).unwrap();
        assert_eq!((s.owner, s.governor_index, s.count), (7, 22, 4096));
        for i in [0, 63, 64, 1000, 4095] {
            assert_eq!(s.has(i), Some(true));
        }
        for i in [1, 62, 65, 999, 4094] {
            assert_eq!(s.has(i), Some(false));
        }
        assert_eq!(s.has(4096), None);
        assert_eq!(m.bytes, before);
        assert!(
            m.reads.get() <= 64,
            "4096 flags require bounded bulk reads, not one call per byte"
        );
    }
    #[test]
    fn rejects_wrong_owner_nonmember_and_invalid_lengths_without_getters() {
        let mut m = fixture();
        put(&mut m, 0x3818, &8u32.to_le_bytes());
        assert_eq!(
            read_current(&m, 0x1000, 0x3800),
            Err(ReadFailure::IdentityMismatch)
        );
        let m = fixture();
        assert_eq!(
            read_current(&m, 0x1000, 0x3900),
            Err(ReadFailure::NotMember)
        );
        for (offset, value) in [(0x3870, 4097u32), (0x3878, 4097), (0x3878, 0)] {
            let mut m = fixture();
            put(&mut m, offset, &value.to_le_bytes());
            assert_eq!(
                read_current(&m, 0x1000, 0x3800),
                Err(ReadFailure::InvalidLayout)
            );
        }
    }
    #[test]
    fn rejects_unreadable_relocated_and_non_boolean_flags() {
        let mut m = fixture();
        m.bytes[0x4001] = 2;
        assert_eq!(
            read_current(&m, 0x1000, 0x3800),
            Err(ReadFailure::InvalidFlag)
        );
        let mut m = fixture();
        put(&mut m, 0x3868, &0xfffffffffffffff0u64.to_le_bytes());
        assert_eq!(
            read_current(&m, 0x1000, 0x3800),
            Err(ReadFailure::Unreadable)
        );
        let m = fixture();
        m.mutate.set(Some(0x4000));
        assert_eq!(read_current(&m, 0x1000, 0x3800), Err(ReadFailure::Changed));
        let m = fixture();
        m.mutate.set(Some(0x3868));
        assert_eq!(read_current(&m, 0x1000, 0x3800), Err(ReadFailure::Changed));
    }
    #[test]
    fn core_instance_identifier_is_not_owner_and_owner_changes_are_rejected() {
        let mut m = fixture();
        assert_eq!(read_current(&m, 0x1000, 0x3800).unwrap().owner, 7);
        // A misleading instance identifier must not authorize another owner.
        put(&mut m, 0x3800, &7u32.to_le_bytes());
        put(&mut m, 0x3818, &13u32.to_le_bytes());
        assert_eq!(
            read_current(&m, 0x1000, 0x3800),
            Err(ReadFailure::IdentityMismatch)
        );
        for changed in [0x3800, 0x3818] {
            let m = fixture();
            m.mutate.set(Some(changed));
            assert_eq!(read_current(&m, 0x1000, 0x3800), Err(ReadFailure::Changed));
        }
    }

    #[test]
    fn rejects_cyclic_membership_with_a_finite_bound() {
        let mut m = fixture();
        put(&mut m, 0x3000, &0x3900u64.to_le_bytes());
        put(&mut m, 0x3010, &0x3020u64.to_le_bytes());
        put(&mut m, 0x3030, &0x3000u64.to_le_bytes());
        assert_eq!(
            read_current(&m, 0x1000, 0x3800),
            Err(ReadFailure::InvalidLayout)
        );
        assert!(m.reads.get() < 270);
    }

    #[test]
    fn membership_reads_head_middle_and_tail_through_the_pinned_next_link() {
        for position in 0..3 {
            let mut m = fixture();
            for node in 0..3 {
                let address = 0x3000 + node * 0x20;
                let person = if node == position {
                    0x3800u64
                } else {
                    0x3900u64
                };
                let next = if node == 2 {
                    0u64
                } else {
                    (address + 0x20) as u64
                };
                put(&mut m, address, &person.to_le_bytes());
                put(&mut m, address + 0x10, &next.to_le_bytes());
            }
            let snapshot = read_current(&m, 0x1000, 0x3800).unwrap();
            assert_eq!((snapshot.owner, snapshot.governor_index), (7, 22));
            put(&mut m, 0x20d8, &13u32.to_le_bytes());
            assert_eq!(
                read_current(&m, 0x1000, 0x3800),
                Err(ReadFailure::IdentityMismatch)
            );
        }
    }

    #[test]
    fn payment_uses_only_matching_person_and_ordered_official_title_flags() {
        let mut m = fixture();
        m.bytes[0x4000 + 63] = 0;
        m.bytes[0x4000 + 64] = 0;
        let definition = PaymentDefinition {
            governor_index: 22,
            cost: 3,
            base: 0,
            titles: [Some(63), Some(64)],
        };
        let snapshot = read_current(&m, 0x1000, 0x3800).unwrap();
        assert_eq!(snapshot.diagnostic_paid(7, definition), Ok(1));
        m.bytes[0x4000 + 63] = 1;
        assert_eq!(
            read_current(&m, 0x1000, 0x3800)
                .unwrap()
                .diagnostic_paid(7, definition),
            Ok(2)
        );
        m.bytes[0x4000 + 64] = 1;
        let snapshot = read_current(&m, 0x1000, 0x3800).unwrap();
        assert_eq!(snapshot.diagnostic_paid(7, definition), Ok(3));
        assert_eq!(
            snapshot.diagnostic_paid(13, definition),
            Err(PaymentFailure::DifferentOwner)
        );
        assert_eq!(
            snapshot.diagnostic_paid(64, definition),
            Err(PaymentFailure::DifferentOwner)
        );
        assert_eq!(
            snapshot.diagnostic_paid(
                7,
                PaymentDefinition {
                    governor_index: 23,
                    ..definition
                }
            ),
            Err(PaymentFailure::DifferentPerson)
        );
        assert_eq!(
            snapshot.diagnostic_paid(
                7,
                PaymentDefinition {
                    cost: 2,
                    titles: [Some(63), None],
                    ..definition
                }
            ),
            Ok(2)
        );
        assert_eq!(
            snapshot.diagnostic_paid(
                7,
                PaymentDefinition {
                    cost: 1,
                    titles: [None, None],
                    ..definition
                }
            ),
            Ok(1)
        );
        m.bytes[0x4000 + 63] = 0;
        assert_eq!(
            read_current(&m, 0x1000, 0x3800)
                .unwrap()
                .diagnostic_paid(7, definition),
            Err(PaymentFailure::OutOfOrder)
        );
        m.bytes[0x4000] = 0;
        assert_eq!(
            read_current(&m, 0x1000, 0x3800)
                .unwrap()
                .diagnostic_paid(7, definition),
            Err(PaymentFailure::OutOfOrder)
        );
    }

    fn snapshot_with(owner: u32, person: u32, owned: &[u32]) -> PromotionSnapshot {
        let mut words = [0u64; PROMOTION_WORDS];
        for index in owned {
            words[*index as usize / 64] |= 1 << (index % 64);
        }
        PromotionSnapshot {
            owner,
            governor_index: person,
            count: 64,
            owned: words,
        }
    }

    #[test]
    fn promotion_policy_allows_only_the_next_unpaid_title_exhaustively() {
        // base 10, TITLE_1 11, TITLE_2 12. Indices 13/14 model TITLE_1/2 rows
        // kept for cost1 saved-price compatibility; 20 is a property skill.
        let sequence = [10u32, 11, 12];
        let requests = [10u32, 11, 12, 13, 14, 20, 4095];
        let sentinel = 0xaabb_ccdd_1122_3301u64;
        let mut allowed_cases = 0;
        for cost in 1..=3u8 {
            let definition = PaymentDefinition {
                governor_index: 22,
                cost,
                base: 10,
                titles: [(cost >= 2).then_some(11), (cost == 3).then_some(12)],
            };
            let needed = &sequence[..cost as usize];
            for mask in 0..32u32 {
                let owned: Vec<u32> = [10, 11, 12, 13, 14]
                    .into_iter()
                    .enumerate()
                    .filter(|(bit, _)| mask & (1 << bit) != 0)
                    .map(|(_, index)| index)
                    .collect();
                // Independent model: the needed flags must form a prefix.
                let held: Vec<bool> = needed.iter().map(|i| owned.contains(i)).collect();
                let paid = held.iter().take_while(|h| **h).count();
                let ordered = held[paid..].iter().all(|h| !h);
                for (snapshot_owner, person) in [(7, 22), (8, 22), (7, 23)] {
                    let snapshot = snapshot_with(snapshot_owner, person, &owned);
                    for requested in requests {
                        for (joined, current, readable) in [
                            (true, true, true),
                            (false, true, true),
                            (true, false, true),
                            (true, true, false),
                        ] {
                            let subject = PromotionSubject::Persona {
                                definition,
                                owner: 7,
                                snapshot: readable.then_some(snapshot),
                                joined,
                            };
                            let allowed = joined
                                && current
                                && readable
                                && (snapshot_owner, person) == (7, 22)
                                && ordered
                                && paid >= 1
                                && paid < cost as usize
                                && requested == sequence[paid];
                            allowed_cases += usize::from(allowed);
                            let expected = if allowed { sentinel } else { sentinel & !0xff };
                            assert_eq!(
                                restrict_promotion(sentinel, subject, requested, current),
                                expected,
                                "cost {cost} owned {owned:?} owner {snapshot_owner} person {person} \
                                 request {requested} joined {joined} current {current} readable {readable}"
                            );
                            // An original false is never opened, all upper bits kept.
                            assert_eq!(
                                restrict_promotion(sentinel & !0xff, subject, requested, current),
                                sentinel & !0xff
                            );
                        }
                    }
                }
            }
        }
        // cost2: base only -> TITLE_1. cost3: base -> TITLE_1, base+T1 -> TITLE_2.
        // Flags outside the definition are free: cost2 has 12/13/14 (8),
        // cost3 has 13/14 for each of its two steps (4 + 4).
        assert_eq!(allowed_cases, 8 + 4 + 4);
    }

    #[test]
    fn promotion_policy_keeps_standard_governors_and_rejects_bad_definitions() {
        let sentinel = 0x1234_5678_9abc_de01u64;
        for requested in [0, 10, 11, 4095] {
            for current in [true, false] {
                assert_eq!(
                    restrict_promotion(sentinel, PromotionSubject::Standard, requested, current),
                    sentinel
                );
                assert_eq!(
                    restrict_promotion(
                        sentinel & !0xff,
                        PromotionSubject::Standard,
                        requested,
                        current
                    ),
                    sentinel & !0xff
                );
            }
        }
        let snapshot = snapshot_with(7, 22, &[10]);
        for definition in [
            // cost2 missing TITLE_1, duplicate title, title equal to base, cost0.
            PaymentDefinition {
                governor_index: 22,
                cost: 2,
                base: 10,
                titles: [None, None],
            },
            PaymentDefinition {
                governor_index: 22,
                cost: 3,
                base: 10,
                titles: [Some(11), Some(11)],
            },
            PaymentDefinition {
                governor_index: 22,
                cost: 2,
                base: 10,
                titles: [Some(10), None],
            },
            PaymentDefinition {
                governor_index: 22,
                cost: 0,
                base: 10,
                titles: [None, None],
            },
            // Title outside the copied flag range is unknown, not payable.
            PaymentDefinition {
                governor_index: 22,
                cost: 2,
                base: 10,
                titles: [Some(64), None],
            },
        ] {
            for requested in [10, 11, 64] {
                let subject = PromotionSubject::Persona {
                    definition,
                    owner: 7,
                    snapshot: Some(snapshot),
                    joined: true,
                };
                assert_eq!(
                    restrict_promotion(sentinel, subject, requested, true),
                    sentinel & !0xff
                );
            }
        }
        // A non-boolean nonzero AL is still treated as an original true and
        // only AL is cleared on denial.
        let subject = PromotionSubject::Persona {
            definition: PaymentDefinition {
                governor_index: 22,
                cost: 1,
                base: 10,
                titles: [None, None],
            },
            owner: 7,
            snapshot: Some(snapshot),
            joined: true,
        };
        assert_eq!(
            restrict_promotion(0xffff_ff80, subject, 13, true),
            0xffff_ff00
        );
        // An out-of-range owner never matches even if the snapshot says so.
        let foreign = PromotionSubject::Persona {
            definition: PaymentDefinition {
                governor_index: 22,
                cost: 2,
                base: 10,
                titles: [Some(11), None],
            },
            owner: 64,
            snapshot: Some(snapshot_with(64, 22, &[10])),
            joined: true,
        };
        assert_eq!(
            restrict_promotion(sentinel, foreign, 11, true),
            sentinel & !0xff
        );
    }

    #[test]
    fn missing_or_duplicate_payment_indices_cannot_become_full_payment() {
        let snapshot = read_current(&fixture(), 0x1000, 0x3800).unwrap();
        let definition = PaymentDefinition {
            governor_index: 22,
            cost: 3,
            base: 0,
            titles: [Some(63), Some(64)],
        };
        for titles in [
            [None, None],
            [Some(63), None],
            [Some(0), Some(64)],
            [Some(63), Some(63)],
        ] {
            assert_eq!(
                snapshot.diagnostic_paid(
                    7,
                    PaymentDefinition {
                        titles,
                        ..definition
                    }
                ),
                Err(PaymentFailure::Definition)
            );
        }
        assert_eq!(
            snapshot.diagnostic_paid(
                7,
                PaymentDefinition {
                    cost: 0,
                    ..definition
                }
            ),
            Err(PaymentFailure::Definition)
        );
        assert_eq!(
            snapshot.diagnostic_paid(
                7,
                PaymentDefinition {
                    base: 4096,
                    ..definition
                }
            ),
            Err(PaymentFailure::UnknownPromotion)
        );
        assert_eq!(
            snapshot.diagnostic_paid(
                7,
                PaymentDefinition {
                    titles: [Some(63), Some(4096)],
                    ..definition
                }
            ),
            Err(PaymentFailure::UnknownPromotion)
        );
    }
}
