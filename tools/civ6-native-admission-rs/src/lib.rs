//! Native persona admission policy for the pinned GameCore build. This crate
//! contains no hook and never assumes that Lua values are C++ value bytes.
//! A runtime adapter must prove the property ABI before feeding a Snapshot.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const PERSONA_COUNT: usize = 22;
pub const OWNER_POINTER_OFFSET: u64 = 0x70;
pub const PLAYER_ID_OFFSET: u64 = 0xd8;
pub const PLAYER_PROPERTIES_OFFSET: u64 = 0x528;

/// The observed owner fields only. The property interface value layout is
/// deliberately absent: its vtable/getter and lifetime are not yet proven.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnerFields {
    pub player_address: u64,
    pub player_id: u32,
    pub property_interface_address: u64,
}

pub trait MemoryRead {
    fn u64_at(&self, address: u64) -> Option<u64>;
    fn u32_at(&self, address: u64) -> Option<u32>;
}

/// Re-read every call. A pointer from an old game or an earlier save is never
/// retained by the admission policy.
pub fn owner_fields(memory: &impl MemoryRead, governors: u64) -> Option<OwnerFields> {
    let player_address = memory.u64_at(governors.checked_add(OWNER_POINTER_OFFSET)?)?;
    if player_address == 0 {
        return None;
    }
    Some(OwnerFields {
        player_address,
        player_id: memory.u32_at(player_address.checked_add(PLAYER_ID_OFFSET)?)?,
        property_interface_address: player_address.checked_add(PLAYER_PROPERTIES_OFFSET)?,
    })
}

/// Correlates the Player Lua wrapper with one native property getter call on
/// the same thread. It expires the player pointer at the next getter/return.
#[derive(Default, Debug)]
pub struct PropertyCapture {
    player: Option<u64>,
    key: Option<u32>,
}
impl PropertyCapture {
    pub fn player_getter(&mut self, player: u64) {
        self.player = (player != 0).then_some(player);
        self.key = None;
    }
    pub fn before_value(&mut self, property_interface: u64, key: u32) -> bool {
        let matched = self
            .player
            .take()
            .and_then(|p| p.checked_add(PLAYER_PROPERTIES_OFFSET))
            == Some(property_interface);
        self.key = matched.then_some(key);
        matched
    }
    pub fn after_value(&mut self) -> Option<u32> {
        self.key.take()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernorDefinition {
    pub leader: String,
    pub governor: String,
    /// The Index from this game's GameInfo.Governors, never a Hash or enum slot.
    pub index: u32,
}

#[derive(Clone, Debug)]
pub struct Mapping {
    by_index: BTreeMap<u32, String>,
}

impl Mapping {
    pub fn from_gameinfo(rows: Vec<GovernorDefinition>) -> Result<Self, String> {
        if rows.len() != PERSONA_COUNT {
            return Err(format!(
                "expected {PERSONA_COUNT} GameInfo persona governors"
            ));
        }
        let mut by_index = BTreeMap::new();
        let mut leaders = BTreeSet::new();
        let mut governors = BTreeSet::new();
        for row in rows {
            if !row.leader.starts_with("LEADER_JAC_")
                || !row.governor.starts_with("GOVERNOR_JAC_PERSONA_")
                || row.leader.trim_start_matches("LEADER_JAC_")
                    != row.governor.trim_start_matches("GOVERNOR_JAC_PERSONA_")
                || !leaders.insert(row.leader.clone())
                || !governors.insert(row.governor)
                || by_index.insert(row.index, row.leader).is_some()
            {
                return Err("invalid or duplicate GameInfo persona mapping".into());
            }
        }
        Ok(Self { by_index })
    }

    pub fn leader(&self, governor_index: u32) -> Option<&str> {
        self.by_index.get(&governor_index).map(String::as_str)
    }

    fn complete(&self, snapshot: &Snapshot) -> bool {
        snapshot.personas.len() == PERSONA_COUNT
            && self
                .by_index
                .values()
                .all(|leader| snapshot.personas.contains_key(leader))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Stage {
    GreatPerson,
    Evolving,
    Governor,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Persona {
    pub owner: Option<u32>,
    pub stage: Stage,
    pub cost: u8,
    /// These observations must come from native units/governors, not a caller's
    /// operation request or an independently editable title counter.
    pub person_present: bool,
    pub governor_present: bool,
    pub base_promotion: bool,
    /// Native APPOINT is one title; ordered native payment promotions add 0..2.
    pub paid: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Titles {
    pub earned: u32,
    pub spent: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub personas: BTreeMap<String, Persona>,
    pub titles: BTreeMap<u32, Titles>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Let the original GameCore result stand for every unrelated governor.
    Original,
    Deny,
    Admit,
}

fn well_formed(person: &Persona) -> bool {
    (1..=3).contains(&person.cost)
        && person.paid <= person.cost
        && person.governor_present == (person.paid >= 1)
        && (!person.governor_present || person.base_promotion)
        && (person.stage != Stage::GreatPerson || person.paid == 0)
        && (person.stage != Stage::Governor
            || (!person.person_present && person.paid == person.cost))
}

fn balance(owner: u32, snapshot: &Snapshot) -> Option<(u32, bool)> {
    let Some(titles) = snapshot.titles.get(&owner) else {
        return None;
    };
    let Some(available) = titles.earned.checked_sub(titles.spent) else {
        return None;
    };
    let mut native_paid = 0u32;
    for person in snapshot
        .personas
        .values()
        .filter(|p| p.owner == Some(owner))
    {
        if !well_formed(person) {
            return None;
        }
        native_paid = native_paid.checked_add(u32::from(person.paid))?;
    }
    (titles.spent >= native_paid).then_some((available, titles.spent == native_paid))
}

fn secured(owner: u32, snapshot: &Snapshot) -> bool {
    let Some((available, exact_native_accounting)) = balance(owner, snapshot) else {
        return false;
    };
    if !exact_native_accounting {
        return false;
    }
    let mut outstanding = 0u32;
    for person in snapshot.personas.values() {
        if person.owner == Some(owner) && person.stage == Stage::Evolving {
            if !well_formed(person) || !person.person_present {
                return false;
            }
            let Some(total) = outstanding.checked_add(u32::from(person.cost - person.paid)) else {
                return false;
            };
            outstanding = total;
        }
    }
    available >= outstanding
}

fn person_for<'a>(
    mapping: &Mapping,
    snapshot: &'a Snapshot,
    index: u32,
) -> Option<Option<&'a Persona>> {
    mapping
        .leader(index)
        .map(|leader| snapshot.personas.get(leader))
}

/// The positive result is only valid once a runtime adapter has independently
/// established that this snapshot is from the synchronized GameCore state.
pub fn can_ever(mapping: &Mapping, snapshot: &Snapshot, owner: u32, index: u32) -> Verdict {
    let Some(person) = person_for(mapping, snapshot, index) else {
        return Verdict::Original;
    };
    if !mapping.complete(snapshot) {
        return Verdict::Deny;
    }
    let Some(person) = person else {
        return Verdict::Deny;
    };
    if person.owner != Some(owner) || !well_formed(person) {
        return Verdict::Deny;
    }
    match person.stage {
        Stage::GreatPerson => Verdict::Deny,
        Stage::Evolving if person.person_present && secured(owner, snapshot) => Verdict::Admit,
        Stage::Governor if person.governor_present && balance(owner, snapshot).is_some() => {
            Verdict::Admit
        }
        _ => Verdict::Deny,
    }
}

pub fn can_assign(mapping: &Mapping, snapshot: &Snapshot, owner: u32, index: u32) -> Verdict {
    let Some(person) = person_for(mapping, snapshot, index) else {
        return Verdict::Original;
    };
    if !mapping.complete(snapshot) {
        return Verdict::Deny;
    }
    let Some(person) = person else {
        return Verdict::Deny;
    };
    if person.owner == Some(owner)
        && well_formed(person)
        && person.stage == Stage::Governor
        && person.governor_present
        && balance(owner, snapshot).is_some()
    {
        // Assignment has additional native city and turn checks. Never turn
        // a native rejection into acceptance at that boundary.
        Verdict::Original
    } else {
        Verdict::Deny
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping() -> Mapping {
        Mapping::from_gameinfo(
            (0..22)
                .map(|i| GovernorDefinition {
                    leader: format!("LEADER_JAC_PERSON_{i}"),
                    governor: format!("GOVERNOR_JAC_PERSONA_PERSON_{i}"),
                    index: 300 + i,
                })
                .collect(),
        )
        .unwrap()
    }

    fn snapshot(cost: u8, earned: u32) -> Snapshot {
        let mut personas = BTreeMap::new();
        for i in 0..22 {
            personas.insert(
                format!("LEADER_JAC_PERSON_{i}"),
                Persona {
                    owner: None,
                    stage: Stage::GreatPerson,
                    cost: 1,
                    person_present: false,
                    governor_present: false,
                    base_promotion: false,
                    paid: 0,
                },
            );
        }
        personas.insert(
            "LEADER_JAC_PERSON_3".into(),
            Persona {
                owner: Some(2),
                stage: Stage::Evolving,
                cost,
                person_present: true,
                governor_present: false,
                base_promotion: false,
                paid: 0,
            },
        );
        Snapshot {
            personas,
            titles: BTreeMap::from([(2, Titles { earned, spent: 0 })]),
        }
    }

    #[test]
    fn mapping_uses_real_indices_and_rejects_partial_or_duplicate_tables() {
        let m = mapping();
        assert_eq!(m.leader(303), Some("LEADER_JAC_PERSON_3"));
        assert_eq!(m.leader(3), None);
        let rows = (0..21)
            .map(|i| GovernorDefinition {
                leader: format!("LEADER_JAC_PERSON_{i}"),
                governor: format!("GOVERNOR_JAC_PERSONA_PERSON_{i}"),
                index: 300 + i,
            })
            .collect();
        assert!(Mapping::from_gameinfo(rows).is_err());
    }

    #[test]
    fn wrong_owner_unjoined_and_unrelated_governor() {
        let m = mapping();
        let mut s = snapshot(2, 2);
        assert_eq!(can_ever(&m, &s, 1, 303), Verdict::Deny);
        assert_eq!(can_ever(&m, &s, 2, 304), Verdict::Deny);
        assert_eq!(can_ever(&m, &s, 2, 12), Verdict::Original);
        s.personas.get_mut("LEADER_JAC_PERSON_3").unwrap().stage = Stage::GreatPerson;
        assert_eq!(can_ever(&m, &s, 2, 303), Verdict::Deny);
    }

    #[test]
    fn full_reservation_and_exact_official_payment_are_required() {
        let m = mapping();
        let mut s = snapshot(3, 2);
        assert_eq!(can_ever(&m, &s, 2, 303), Verdict::Deny);
        s.titles.get_mut(&2).unwrap().earned = 3;
        assert_eq!(can_ever(&m, &s, 2, 303), Verdict::Admit);
        assert_eq!(can_assign(&m, &s, 2, 303), Verdict::Deny);
        let p = s.personas.get_mut("LEADER_JAC_PERSON_3").unwrap();
        p.governor_present = true;
        p.base_promotion = true;
        p.paid = 1;
        s.titles.get_mut(&2).unwrap().spent = 1;
        assert_eq!(can_ever(&m, &s, 2, 303), Verdict::Admit);
        assert_eq!(can_assign(&m, &s, 2, 303), Verdict::Deny);
        s.titles.get_mut(&2).unwrap().spent = 2;
        assert_eq!(can_ever(&m, &s, 2, 303), Verdict::Deny);
        s.titles.get_mut(&2).unwrap().earned = 4;
        let p = s.personas.get_mut("LEADER_JAC_PERSON_3").unwrap();
        p.paid = 3;
        p.person_present = false;
        p.stage = Stage::Governor;
        s.titles.get_mut(&2).unwrap().spent = 3;
        assert_eq!(can_ever(&m, &s, 2, 303), Verdict::Admit);
        assert_eq!(can_assign(&m, &s, 2, 303), Verdict::Original);
        s.titles.get_mut(&2).unwrap().spent = 2;
        assert_eq!(can_ever(&m, &s, 2, 303), Verdict::Deny);
        assert_eq!(can_assign(&m, &s, 2, 303), Verdict::Deny);
    }

    #[test]
    fn multiple_reservations_cannot_spend_one_title_twice() {
        let m = mapping();
        let mut s = snapshot(2, 3);
        s.personas.insert(
            "LEADER_JAC_PERSON_4".into(),
            Persona {
                owner: Some(2),
                stage: Stage::Evolving,
                cost: 2,
                person_present: true,
                governor_present: false,
                base_promotion: false,
                paid: 0,
            },
        );
        assert_eq!(can_ever(&m, &s, 2, 303), Verdict::Deny);
        s.titles.get_mut(&2).unwrap().earned = 4;
        assert_eq!(can_ever(&m, &s, 2, 303), Verdict::Admit);
    }

    #[test]
    fn prior_and_unaccounted_spending_cannot_mask_a_pending_payment() {
        let m = mapping();
        let mut s = snapshot(2, 5);
        let past = s.personas.get_mut("LEADER_JAC_PERSON_4").unwrap();
        past.owner = Some(2);
        past.stage = Stage::Governor;
        past.cost = 2;
        past.governor_present = true;
        past.base_promotion = true;
        past.paid = 2;
        s.titles.get_mut(&2).unwrap().spent = 2;
        assert_eq!(can_ever(&m, &s, 2, 303), Verdict::Admit);
        let current = s.personas.get_mut("LEADER_JAC_PERSON_3").unwrap();
        current.governor_present = true;
        current.base_promotion = true;
        current.paid = 1;
        // The native cache can get ahead of official Spent; past payments
        // cannot be counted a second time toward this appointment.
        assert_eq!(can_ever(&m, &s, 2, 303), Verdict::Deny);
        s.titles.get_mut(&2).unwrap().spent = 3;
        assert_eq!(can_ever(&m, &s, 2, 303), Verdict::Admit);
        s.titles.get_mut(&2).unwrap().spent = 4; // another title user
        assert_eq!(can_ever(&m, &s, 2, 303), Verdict::Deny);
    }

    #[test]
    fn unavailable_snapshot_and_save_load_roundtrip_fail_closed() {
        let m = mapping();
        assert_eq!(can_ever(&m, &Snapshot::default(), 2, 303), Verdict::Deny);
        let before = snapshot(2, 2);
        let mut incomplete = before.clone();
        incomplete.personas.remove("LEADER_JAC_PERSON_4");
        assert_eq!(can_ever(&m, &incomplete, 2, 303), Verdict::Deny);
        let bytes = serde_json::to_vec(&before).unwrap();
        let after: Snapshot = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(can_ever(&m, &after, 2, 303), Verdict::Admit);
        let mut missing = after;
        missing
            .personas
            .get_mut("LEADER_JAC_PERSON_3")
            .unwrap()
            .person_present = false;
        assert_eq!(can_ever(&m, &missing, 2, 303), Verdict::Deny);
    }

    #[test]
    fn synthetic_owner_abi_rereads_after_player_lifetime_change() {
        struct Memory(BTreeMap<u64, u64>);
        impl MemoryRead for Memory {
            fn u64_at(&self, address: u64) -> Option<u64> {
                self.0.get(&address).copied()
            }
            fn u32_at(&self, address: u64) -> Option<u32> {
                self.0.get(&address).and_then(|n| u32::try_from(*n).ok())
            }
        }
        let mut mem = Memory(BTreeMap::from([(0x1070, 0x2000), (0x20d8, 2)]));
        assert_eq!(
            owner_fields(&mem, 0x1000),
            Some(OwnerFields {
                player_address: 0x2000,
                player_id: 2,
                property_interface_address: 0x2528,
            })
        );
        mem.0.remove(&0x20d8);
        mem.0.insert(0x1070, 0x3000);
        mem.0.insert(0x30d8, 7);
        assert_eq!(owner_fields(&mem, 0x1000).unwrap().player_id, 7);
        mem.0.remove(&0x30d8);
        assert_eq!(owner_fields(&mem, 0x1000), None);
    }

    #[test]
    fn synthetic_property_getter_never_reuses_an_old_player_or_value() {
        let mut capture = PropertyCapture::default();
        capture.player_getter(0x2000);
        assert!(!capture.before_value(0x3528, 8));
        assert_eq!(capture.after_value(), None);
        capture.player_getter(0x3000);
        assert!(capture.before_value(0x3528, 12));
        assert_eq!(capture.after_value(), Some(12));
        assert_eq!(capture.after_value(), None);
        assert!(!capture.before_value(0x3528, 12));
        capture.player_getter(0);
        assert!(!capture.before_value(0x528, 12));
    }
}
