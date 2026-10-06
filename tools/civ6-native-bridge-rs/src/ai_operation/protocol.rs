//! Pure, finite replay candidate. Values here are fixture inputs, NOT proof of
//! synchronized provenance. No constructor creates a live execution permit.
//! A future adapter must bind them to the natural AI callback documented in
//! pr86-native-admission.md. Never run this from a property getter.
use civ6_native_bridge_rs::governor_payment::{PaymentDefinition, PromotionSnapshot};
use std::collections::BTreeSet;

pub const DATE: &str = "LEADER_JAC_DATE_TANEMUNE";
pub const UESUGI: &str = "LEADER_JAC_YAMANOUCHI_UESUGI_NORIMASA";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Epoch {
    pub pid: u32,
    pub created: u64,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    GreatPerson,
    Evolving,
    Governor,
}

pub struct Person<'a> {
    pub leader: &'a str,
    pub governor_type: &'a str,
    pub base_type: &'a str,
    pub title_types: [Option<&'a str>; 2],
    pub definition: PaymentDefinition,
    pub owner: Option<u32>,
    pub stage: Stage,
    pub great_person_present: bool,
    pub native: Option<PromotionSnapshot>,
}

pub struct Frame<'a> {
    pub epoch: Epoch,
    pub sequence: u64,
    pub owner: u32,
    pub earned: u32,
    pub spent: u32,
    /// All 22 synchronized catalog rows and ledger/native observations. Omission
    /// cannot silently free another person's reservation.
    pub people: &'a [Person<'a>],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    pub epoch: Epoch,
    pub owner: u32,
    pub governor_index: u32,
    pub promotion_index: u32,
    pub previous_paid: u8,
    pub spent_before: u32,
    pub sequence: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// A simulation result only; there is deliberately no send/execute method.
    Propose(Candidate),
    AwaitResult,
    ObservedOnePayment,
    NoAdditionalTitle,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Failure {
    Epoch,
    Order,
    Catalog,
    Identity,
    Payment,
    Reservation,
    AmbiguousResult,
    Exhausted,
}

#[derive(Clone, Copy)]
struct Pending {
    candidate: Candidate,
    definition: PaymentDefinition,
    earned: u32,
    // A different person's promotion must not stand in for this operation.
    paid: [u8; 22],
    native: [Option<PromotionSnapshot>; 22],
    owners: [Option<u32>; 22],
}

/// One finite owner replay, maximum two extra titles. Restore by constructing a
/// fresh replay only AFTER a real load barrier drains old operations. This type
/// has no ability to establish that barrier; runtime integration remains absent.
pub struct Replay {
    epoch: Epoch,
    owner: u32,
    last_sequence: u64,
    proposals: u8,
    pending: Option<Pending>,
    poisoned: bool,
    catalog: Option<Vec<(String, PaymentDefinition)>>,
    last_paid: [u8; 22],
    last_spent: u32,
}

impl Replay {
    pub fn new(epoch: Epoch, owner: u32) -> Result<Self, Failure> {
        if epoch.pid == 0 || epoch.created == 0 || epoch.generation == 0 || owner >= 64 {
            return Err(Failure::Epoch);
        }
        Ok(Self {
            epoch,
            owner,
            last_sequence: 0,
            proposals: 0,
            pending: None,
            poisoned: false,
            catalog: None,
            last_paid: [0; 22],
            last_spent: 0,
        })
    }

    pub fn observe(&mut self, frame: &Frame<'_>) -> Result<Decision, Failure> {
        if self.poisoned {
            return Err(Failure::AmbiguousResult);
        }
        let result = self.observe_inner(frame);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn observe_inner(&mut self, f: &Frame<'_>) -> Result<Decision, Failure> {
        if f.epoch != self.epoch || f.owner != self.owner {
            return Err(Failure::Epoch);
        }
        if f.sequence <= self.last_sequence {
            return Err(Failure::Order);
        }
        self.last_sequence = f.sequence;
        let paid = validate(f)?;
        if f.spent < self.last_spent || paid.iter().zip(self.last_paid).any(|(now, old)| *now < old)
        {
            return Err(Failure::AmbiguousResult);
        }
        self.last_paid = paid;
        self.last_spent = f.spent;
        let catalog: Vec<_> = f
            .people
            .iter()
            .map(|p| (p.leader.to_owned(), p.definition))
            .collect();
        if self.catalog.as_ref().is_some_and(|old| *old != catalog) {
            return Err(Failure::Catalog);
        }
        self.catalog = Some(catalog);
        let native = std::array::from_fn(|i| f.people[i].native);
        let owners = std::array::from_fn(|i| f.people[i].owner);
        if let Some(pending) = self.pending {
            let c = pending.candidate;
            let slot = f
                .people
                .iter()
                .position(|p| p.definition.governor_index == c.governor_index)
                .ok_or(Failure::Identity)?;
            let person = &f.people[slot];
            if person.owner != Some(f.owner)
                || person.definition != pending.definition
                || f.earned != pending.earned
                || person.stage != Stage::Evolving
                || owners != pending.owners
            {
                return Err(Failure::AmbiguousResult);
            }
            if paid == pending.paid && f.spent == c.spent_before && native == pending.native {
                return Ok(Decision::AwaitResult);
            }
            let mut expected = pending.paid;
            expected[slot] = c.previous_paid + 1;
            let mut expected_native = pending.native;
            let own = expected_native[slot].as_mut().ok_or(Failure::Identity)?;
            own.owned[c.promotion_index as usize / 64] |= 1 << (c.promotion_index % 64);
            if paid != expected
                || native != expected_native
                || f.spent != c.spent_before.checked_add(1).ok_or(Failure::Payment)?
            {
                return Err(Failure::AmbiguousResult);
            }
            self.pending = None;
            // Never issue a second proposal within the same result observation.
            return Ok(Decision::ObservedOnePayment);
        }
        for (slot, p) in f.people.iter().enumerate() {
            if p.owner != Some(f.owner) || p.stage != Stage::Evolving {
                continue;
            }
            let cost = match p.leader {
                DATE => 2,
                UESUGI => 3,
                _ => continue,
            };
            if p.definition.cost != cost {
                return Err(Failure::Catalog);
            }
            let already = paid[slot];
            if already == 0 || already == cost {
                continue;
            }
            if self.proposals >= 2 {
                return Err(Failure::Exhausted);
            }
            let c = Candidate {
                epoch: f.epoch,
                owner: f.owner,
                governor_index: p.definition.governor_index,
                promotion_index: p.definition.titles[usize::from(already - 1)]
                    .ok_or(Failure::Catalog)?,
                previous_paid: already,
                spent_before: f.spent,
                sequence: f.sequence,
            };
            self.proposals += 1;
            self.pending = Some(Pending {
                candidate: c,
                definition: p.definition,
                earned: f.earned,
                paid,
                native,
                owners,
            });
            return Ok(Decision::Propose(c));
        }
        Ok(Decision::NoAdditionalTitle)
    }
}

fn validate(f: &Frame<'_>) -> Result<[u8; 22], Failure> {
    if f.people.len() != 22 {
        return Err(Failure::Catalog);
    }
    let sql = include_str!("../../../../poc/JAC_PoC_LeaderPersona/Data/JAC_LeaderPersona.sql");
    let names: BTreeSet<_> = sql
        .split("INSERT INTO JAC_PersonaDesigns VALUES")
        .nth(1)
        .ok_or(Failure::Catalog)?
        .split(';')
        .next()
        .ok_or(Failure::Catalog)?
        .lines()
        .filter_map(|line| line.trim().strip_prefix("('"))
        .filter_map(|line| line.split('\'').next())
        .collect();
    if names.len() != 22 || f.people.iter().map(|p| p.leader).collect::<BTreeSet<_>>() != names {
        return Err(Failure::Catalog);
    }
    let mut indices = BTreeSet::new();
    let mut promotions = BTreeSet::new();
    let mut prior_leader = None;
    let mut paid = [0; 22];
    let mut total = 0u32;
    let mut remaining = 0u32;
    for (slot, p) in f.people.iter().enumerate() {
        let suffix = p
            .leader
            .strip_prefix("LEADER_JAC_")
            .filter(|s| !s.is_empty())
            .ok_or(Failure::Catalog)?;
        let def = p.definition;
        if prior_leader.is_some_and(|prior| prior >= p.leader)
            || p.governor_type != format!("GOVERNOR_JAC_PERSONA_{suffix}")
            || p.base_type != format!("GOVERNOR_PROMOTION_JAC_PERSONA_{suffix}_BASE")
            || def.governor_index >= 256
            || !(1..=3).contains(&def.cost)
            || !indices.insert(def.governor_index)
        {
            return Err(Failure::Catalog);
        }
        prior_leader = Some(p.leader);
        for step in 0..2 {
            let needed = usize::from(def.cost) > step + 1;
            if def.titles[step].is_some() != needed || p.title_types[step].is_some() != needed {
                return Err(Failure::Catalog);
            }
            if needed
                && p.title_types[step]
                    != Some(format!("{}_TITLE_{}", p.base_type, step + 1).as_str())
            {
                return Err(Failure::Catalog);
            }
        }
        for index in std::iter::once(def.base).chain(def.titles.into_iter().flatten()) {
            if index >= 4096 || !promotions.insert(index) {
                return Err(Failure::Catalog);
            }
        }
        if p.owner.is_some_and(|id| id >= 64) {
            return Err(Failure::Identity);
        }
        if let Some(native) = &p.native {
            if native.count == 0
                || native.count > 4096
                || (native.count..4096)
                    .any(|ix| native.owned[ix as usize / 64] & (1 << (ix % 64)) != 0)
            {
                return Err(Failure::Payment);
            }
            let owner = p.owner.ok_or(Failure::Identity)?;
            paid[slot] = native
                .diagnostic_paid(owner, def)
                .map_err(|_| Failure::Payment)?;
            if paid[slot] == 0 {
                return Err(Failure::Payment);
            }
        }
        match p.stage {
            Stage::GreatPerson if paid[slot] != 0 => return Err(Failure::Payment),
            Stage::Evolving if p.owner.is_none() || !p.great_person_present => {
                return Err(Failure::Identity)
            }
            Stage::Governor
                if p.owner.is_none() || p.great_person_present || paid[slot] != def.cost =>
            {
                return Err(Failure::Payment)
            }
            _ => {}
        }
        if p.owner == Some(f.owner) {
            total += u32::from(paid[slot]);
            if p.stage == Stage::Evolving {
                remaining += u32::from(def.cost - paid[slot]);
            }
        }
    }
    // Same conservative equation as native-admission::secured: aggregate Spent
    // must be fully attributed before decreasing any persona reservation.
    if f.spent != total || f.earned.checked_sub(f.spent).ok_or(Failure::Reservation)? < remaining {
        return Err(Failure::Reservation);
    }
    Ok(paid)
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
