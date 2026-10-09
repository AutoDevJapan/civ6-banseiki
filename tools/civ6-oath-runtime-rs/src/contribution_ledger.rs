//! Bounded bookkeeping for runtime-admitted military contributions.
//!
//! This is NOT a trace reader. The runtime must independently establish the
//! actual receiver, world/load epoch, city birth generation, owner/full ID,
//! definition and instance binding, and the completed update's attribution.
//! `AdmittedCompletion` describes that caller contract; constructing this type
//! (or a boolean, ticket, double read, AL=1, Active flag or subject list) does
//! not prove host synchronization, reference retention, lifetime or completion.
//! The runtime must retain the host objects and serialize the original update,
//! begin/finish, destruction/transfer/load and Quantity in the same valid host
//! holding interval. Recursive Quantity must query this ledger and be denied
//! while an update is pending. There is deliberately no diagnostic-feed API.
//!
//! Start a RootFresh/Birth boundary only when the caller has proved that the
//! city has no prior named contributions or removal history. On load, clear
//! the prior epoch BEFORE host loading and restore only from the authoritative
//! save's checkpoint, rebound to caller-established actual identities. Missing
//! history is Unknown, not a negative baseline or a zero contribution.
//!
//! Only the required receiver address is retained as an opaque identity value;
//! it is never dereferenced. No array/definition/instance memory pointer is
//! owned/stored. Instance bindings are caller-assigned incarnation numbers,
//! never pointer casts.
//! Checkpoint structs contain no addresses, epochs, tickets or process IDs.
//! They are DTOs for a future official-save codec, NOT evidence of a successful
//! save/restore connection, accepted Quantity, or product acceptance.

use std::sync::atomic::{AtomicU64, Ordering};

const MAX_WORLD_ID_BYTES: usize = 128;
const MAX_DEFINITION_BYTES: usize = 64;
const MAX_BOUND: usize = 65_536;
const MAX_TOTAL_RECORDS: usize = 1_048_576;
pub const CHECKPOINT_VERSION: u32 = 1;
static NEXT_LEDGER_NONCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StableCityIdentity {
    /// Save-domain world identity; never a process ID or an address.
    pub world_id: String,
    /// Caller-established birth generation, not just the native full ID.
    pub birth_generation: u64,
    pub owner: i32,
    pub full_id: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CityIdentity {
    pub stable: StableCityIdentity,
    pub load_epoch: u64,
    /// Actual callee receiver, not a wrapper or a guessed table candidate.
    pub receiver_address: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StableEffectIdentity {
    pub definition_name: String,
    /// Caller-established save-domain instance identity. Must survive load;
    /// a native Instance ID alone need not meet that requirement (ABA).
    pub stable_instance_id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectIdentity {
    pub stable: StableEffectIdentity,
    /// Caller-established incarnation, not a memory pointer or process ID.
    pub instance_binding: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub cities: usize,
    pub effects_per_city: usize,
    pub units_per_city: usize,
    /// Includes inactive rows whose historical residue is still necessary.
    pub records_per_city: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            cities: 128,
            effects_per_city: 16,
            units_per_city: 512,
            records_per_city: 4096,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProvenStartBoundary {
    RootFresh,
    Birth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnknownReason {
    Unavailable,
    Incomplete,
    Unwound,
    ReentrantUpdate,
    IdentityChanged,
    OwnerTransferred,
    DuplicateApply,
    LeadingRemove,
    InvalidCompletion,
    CapacityExceeded,
    ArithmeticOverflow,
    AllocationFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedgerError {
    InvalidLimits,
    InvalidIdentity,
    MissingCity,
    IdentityMismatch,
    EpochMismatch,
    Unknown(UnknownReason),
    Pending,
    ReentrantUpdate,
    DuplicateApply,
    LeadingRemove,
    InstanceBindingMismatch,
    UnitOutOfRange,
    InvalidCompletion,
    StaleTicket,
    CapacityExceeded,
    ArithmeticOverflow,
    AllocationFailed,
    InvalidCheckpoint,
    InvalidRebind,
    RestoreRequiresEmptyLedger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateKind {
    Apply,
    Remove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitDelta {
    pub unit_index: u32,
    /// Signed change attributable ONLY to this completed named update.
    /// Never compute it from diagnostic samples or a negative city baseline.
    pub raw_q8_delta: i32,
}

/// Supplied by the runtime after separately admitting completed host work.
/// `Applied` enumerates every affected unit (empty means proven no-op).
/// `Removed` must enumerate exactly the currently active units, including any
/// zero deltas. Unavailable/incomplete/unwound work cannot publish quantities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdmittedCompletion {
    Applied {
        declared_pct: i32,
        units: Vec<UnitDelta>,
    },
    Removed {
        units: Vec<UnitDelta>,
    },
    Unavailable,
    Incomplete,
    Unwound,
}

/// A ledger-local sequencing token, NOT a host lock or lifetime certificate.
/// Dropping it leaves the city pending and quantities unavailable. After an
/// unwind the runtime should explicitly finish with `Unwound` when possible.
#[derive(Debug)]
pub struct UpdateTicket {
    ledger_nonce: u64,
    serial: u64,
    city: CityIdentity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActiveContribution {
    pub raw_q8: i32,
    pub declared_pct: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuantityInputs {
    pub active_raw_q8: i32,
    pub declared_pct: i32,
    pub removal_residue_q8: i32,
}

/// Original native data, copied unchanged; no city/baseline inference occurs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeQuantityFeed {
    pub owner_q8: i32,
    pub city_q8: i32,
    pub other_rate_q8: i32,
    pub base_q8: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuantityInput {
    pub native: NativeQuantityFeed,
    pub named: QuantityInputs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckpointStatus {
    Known,
    Unknown(UnknownReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckpointUnit {
    pub unit_index: u32,
    pub active: Option<ActiveContribution>,
    pub removal_residue_q8: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckpointEffect {
    pub identity: StableEffectIdentity,
    /// Also records an admitted Apply that affected zero units.
    pub is_active: bool,
    pub units: Vec<CheckpointUnit>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckpointCity {
    pub identity: StableCityIdentity,
    pub unit_count: u32,
    pub status: CheckpointStatus,
    pub effects: Vec<CheckpointEffect>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    pub version: u32,
    pub cities: Vec<CheckpointCity>,
}

/// Actual identities supplied by the restoring runtime, matched by stable
/// identities, never by vector position or by replaying saved addresses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestoreBinding {
    pub city: CityIdentity,
    pub effects: Vec<EffectIdentity>,
}

#[derive(Debug)]
struct Effect {
    identity: EffectIdentity,
    is_active: bool,
    units: Vec<CheckpointUnit>,
}

#[derive(Debug)]
struct PendingUpdate {
    serial: u64,
    effect: EffectIdentity,
    kind: UpdateKind,
}

#[derive(Debug)]
struct City {
    identity: CityIdentity,
    unit_count: u32,
    status: CheckpointStatus,
    pending: Option<PendingUpdate>,
    effects: Vec<Effect>,
}

/// Internally serialized by `&mut self`; host-side holding/synchronization is
/// still the runtime's obligation, including for read-only quantity queries.
#[derive(Debug)]
pub struct ContributionLedger {
    limits: Limits,
    load_epoch: u64,
    nonce: u64,
    next_serial: u64,
    cities: Vec<City>,
}

fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<(), LedgerError> {
    values
        .try_reserve_exact(additional)
        .map_err(|_| LedgerError::AllocationFailed)
}

fn valid_stable_city(city: &StableCityIdentity) -> bool {
    !city.world_id.is_empty()
        && city.world_id.len() <= MAX_WORLD_ID_BYTES
        && !city.world_id.chars().any(char::is_control)
        && city.birth_generation != 0
        && city.owner >= 0
}

fn valid_effect(effect: &EffectIdentity) -> bool {
    let name = &effect.stable.definition_name;
    !name.is_empty()
        && name.len() <= MAX_DEFINITION_BYTES
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && effect.stable.stable_instance_id != 0
        && effect.instance_binding != 0
}

fn make_unknown(city: &mut City, reason: UnknownReason) {
    city.pending = None;
    city.status = CheckpointStatus::Unknown(reason);
}

fn ready(city: &City) -> Result<(), LedgerError> {
    if let CheckpointStatus::Unknown(reason) = city.status {
        return Err(LedgerError::Unknown(reason));
    }
    if city.pending.is_some() {
        return Err(LedgerError::Pending);
    }
    Ok(())
}

fn quantity(city: &City, unit_index: u32) -> Result<QuantityInputs, LedgerError> {
    ready(city)?;
    if unit_index >= city.unit_count {
        return Err(LedgerError::UnitOutOfRange);
    }
    let mut raw = 0i64;
    let mut pct = 0i64;
    let mut residue = 0i64;
    for effect in &city.effects {
        if let Some(unit) = effect.units.iter().find(|u| u.unit_index == unit_index) {
            if let Some(active) = unit.active {
                raw = raw
                    .checked_add(i64::from(active.raw_q8))
                    .ok_or(LedgerError::ArithmeticOverflow)?;
                pct = pct
                    .checked_add(i64::from(active.declared_pct))
                    .ok_or(LedgerError::ArithmeticOverflow)?;
            }
            residue = residue
                .checked_add(i64::from(unit.removal_residue_q8))
                .ok_or(LedgerError::ArithmeticOverflow)?;
        }
    }
    Ok(QuantityInputs {
        active_raw_q8: i32::try_from(raw).map_err(|_| LedgerError::ArithmeticOverflow)?,
        declared_pct: i32::try_from(pct).map_err(|_| LedgerError::ArithmeticOverflow)?,
        removal_residue_q8: i32::try_from(residue).map_err(|_| LedgerError::ArithmeticOverflow)?,
    })
}

impl ContributionLedger {
    pub fn new(load_epoch: u64, limits: Limits) -> Result<Self, LedgerError> {
        let counts = [
            limits.cities,
            limits.effects_per_city,
            limits.units_per_city,
            limits.records_per_city,
        ];
        if counts.iter().any(|n| *n == 0 || *n > MAX_BOUND)
            || limits
                .cities
                .checked_mul(limits.records_per_city)
                .filter(|n| *n <= MAX_TOTAL_RECORDS)
                .is_none()
            || limits
                .cities
                .checked_mul(limits.effects_per_city)
                .filter(|n| *n <= MAX_TOTAL_RECORDS)
                .is_none()
        {
            return Err(LedgerError::InvalidLimits);
        }
        if load_epoch == 0 {
            return Err(LedgerError::EpochMismatch);
        }
        let nonce = NEXT_LEDGER_NONCE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| LedgerError::ArithmeticOverflow)?;
        Ok(Self {
            limits,
            load_epoch,
            nonce,
            next_serial: 1,
            cities: Vec::new(),
        })
    }

    fn validate_city(&self, city: &CityIdentity) -> Result<(), LedgerError> {
        if !valid_stable_city(&city.stable) || city.receiver_address == 0 {
            return Err(LedgerError::InvalidIdentity);
        }
        if city.load_epoch != self.load_epoch {
            return Err(LedgerError::EpochMismatch);
        }
        Ok(())
    }

    fn city_index(&self, identity: &CityIdentity) -> Result<usize, LedgerError> {
        self.validate_city(identity)?;
        let index = self
            .cities
            .iter()
            .position(|c| c.identity.stable == identity.stable)
            .ok_or(LedgerError::MissingCity)?;
        if self.cities[index].identity != *identity {
            return Err(LedgerError::IdentityMismatch);
        }
        Ok(index)
    }

    /// This enum is a caller assertion, not proof. Both boundaries require
    /// proven absence of ALL earlier named active contributions and residues.
    /// Replacing an existing city explicitly revokes its pending ticket.
    pub fn start_proven_boundary(
        &mut self,
        identity: CityIdentity,
        unit_count: u32,
        _boundary: ProvenStartBoundary,
    ) -> Result<(), LedgerError> {
        self.validate_city(&identity)?;
        if unit_count == 0 || unit_count as usize > self.limits.units_per_city {
            return Err(LedgerError::CapacityExceeded);
        }
        let index = self
            .cities
            .iter()
            .position(|c| c.identity.stable == identity.stable);
        if self.cities.iter().any(|c| {
            c.identity.receiver_address == identity.receiver_address
                && c.identity.stable != identity.stable
        }) {
            return Err(LedgerError::IdentityMismatch);
        }
        let city = City {
            identity,
            unit_count,
            status: CheckpointStatus::Known,
            pending: None,
            effects: Vec::new(),
        };
        if let Some(index) = index {
            self.cities[index] = city;
        } else {
            if self.cities.len() >= self.limits.cities {
                return Err(LedgerError::CapacityExceeded);
            }
            reserve(&mut self.cities, 1)?;
            self.cities.push(city);
        }
        Ok(())
    }

    /// Call BEFORE the original update, while retaining/holding the host city.
    /// Any second update in this city is treated as reentry: both are denied
    /// publication until a separately proven boundary or restore recovers it.
    pub fn begin(
        &mut self,
        city: &CityIdentity,
        effect: EffectIdentity,
        kind: UpdateKind,
    ) -> Result<UpdateTicket, LedgerError> {
        let index = self.city_index(city)?;
        if self.cities[index].pending.is_some() {
            make_unknown(&mut self.cities[index], UnknownReason::ReentrantUpdate);
            return Err(LedgerError::ReentrantUpdate);
        }
        ready(&self.cities[index])?;
        if !valid_effect(&effect) {
            make_unknown(&mut self.cities[index], UnknownReason::IdentityChanged);
            return Err(LedgerError::InvalidIdentity);
        }
        let existing = self.cities[index].effects.iter().find(|e| {
            e.identity.stable.stable_instance_id == effect.stable.stable_instance_id
                || e.identity.instance_binding == effect.instance_binding
        });
        if let Some(existing) = existing {
            if existing.identity != effect {
                make_unknown(&mut self.cities[index], UnknownReason::IdentityChanged);
                return Err(LedgerError::InstanceBindingMismatch);
            }
            if kind == UpdateKind::Apply && existing.is_active {
                make_unknown(&mut self.cities[index], UnknownReason::DuplicateApply);
                return Err(LedgerError::DuplicateApply);
            }
        }
        if kind == UpdateKind::Remove && !existing.is_some_and(|e| e.is_active) {
            make_unknown(&mut self.cities[index], UnknownReason::LeadingRemove);
            return Err(LedgerError::LeadingRemove);
        }
        if existing.is_none() && self.cities[index].effects.len() >= self.limits.effects_per_city {
            make_unknown(&mut self.cities[index], UnknownReason::CapacityExceeded);
            return Err(LedgerError::CapacityExceeded);
        }
        let serial = self.next_serial;
        self.next_serial = match serial.checked_add(1) {
            Some(next) => next,
            None => {
                make_unknown(&mut self.cities[index], UnknownReason::ArithmeticOverflow);
                return Err(LedgerError::ArithmeticOverflow);
            }
        };
        self.cities[index].pending = Some(PendingUpdate {
            serial,
            effect,
            kind,
        });
        Ok(UpdateTicket {
            ledger_nonce: self.nonce,
            serial,
            city: city.clone(),
        })
    }

    /// Only a matching pending token may publish. The runtime must re-establish
    /// the current actual identity under its holding interval at completion.
    /// Stale tokens do not poison a replacement city or a newer transaction.
    pub fn finish(
        &mut self,
        ticket: UpdateTicket,
        current_city: &CityIdentity,
        completion: AdmittedCompletion,
    ) -> Result<(), LedgerError> {
        if ticket.ledger_nonce != self.nonce {
            return Err(LedgerError::StaleTicket);
        }
        let index = self
            .city_index(&ticket.city)
            .map_err(|_| LedgerError::StaleTicket)?;
        if self.cities[index]
            .pending
            .as_ref()
            .is_none_or(|p| p.serial != ticket.serial)
        {
            return Err(LedgerError::StaleTicket);
        }
        if *current_city != ticket.city {
            make_unknown(&mut self.cities[index], UnknownReason::IdentityChanged);
            return Err(LedgerError::IdentityMismatch);
        }
        let result = self.complete(index, completion);
        if let Err(error) = result {
            let reason = match error {
                LedgerError::Unknown(reason) => reason,
                LedgerError::CapacityExceeded => UnknownReason::CapacityExceeded,
                LedgerError::ArithmeticOverflow => UnknownReason::ArithmeticOverflow,
                LedgerError::AllocationFailed => UnknownReason::AllocationFailed,
                _ => UnknownReason::InvalidCompletion,
            };
            make_unknown(&mut self.cities[index], reason);
        }
        result
    }

    fn complete(
        &mut self,
        index: usize,
        completion: AdmittedCompletion,
    ) -> Result<(), LedgerError> {
        let city = &self.cities[index];
        let pending = city.pending.as_ref().ok_or(LedgerError::StaleTicket)?;
        let (units, declared_pct) = match completion {
            AdmittedCompletion::Applied {
                declared_pct,
                units,
            } if pending.kind == UpdateKind::Apply && declared_pct > 0 => {
                (units, Some(declared_pct))
            }
            AdmittedCompletion::Removed { units } if pending.kind == UpdateKind::Remove => {
                (units, None)
            }
            AdmittedCompletion::Unavailable => {
                return Err(LedgerError::Unknown(UnknownReason::Unavailable));
            }
            AdmittedCompletion::Incomplete => {
                return Err(LedgerError::Unknown(UnknownReason::Incomplete));
            }
            AdmittedCompletion::Unwound => {
                return Err(LedgerError::Unknown(UnknownReason::Unwound));
            }
            _ => return Err(LedgerError::InvalidCompletion),
        };
        if units.len() > self.limits.units_per_city {
            return Err(LedgerError::CapacityExceeded);
        }
        for (position, unit) in units.iter().enumerate() {
            if unit.unit_index >= city.unit_count {
                return Err(LedgerError::UnitOutOfRange);
            }
            if units[..position]
                .iter()
                .any(|u| u.unit_index == unit.unit_index)
                || (declared_pct.is_some() && unit.raw_q8_delta <= 0)
            {
                return Err(LedgerError::InvalidCompletion);
            }
        }
        let existing_index = city
            .effects
            .iter()
            .position(|e| e.identity == pending.effect);
        let existing = existing_index.map(|i| &city.effects[i]);
        // Stage the complete effect before modifying any published records.
        let mut staged = Effect {
            identity: pending.effect.clone(),
            is_active: declared_pct.is_some(),
            units: Vec::new(),
        };
        let old_len = existing.map_or(0, |e| e.units.len());
        let added = units
            .iter()
            .filter(|u| {
                !existing.is_some_and(|e| e.units.iter().any(|old| old.unit_index == u.unit_index))
            })
            .count();
        let new_len = old_len
            .checked_add(added)
            .ok_or(LedgerError::ArithmeticOverflow)?;
        let old_total = city.effects.iter().try_fold(0usize, |sum, e| {
            sum.checked_add(e.units.len())
                .ok_or(LedgerError::ArithmeticOverflow)
        })?;
        if old_total
            .checked_add(added)
            .ok_or(LedgerError::ArithmeticOverflow)?
            > self.limits.records_per_city
        {
            return Err(LedgerError::CapacityExceeded);
        }
        reserve(&mut staged.units, new_len)?;
        if let Some(existing) = existing {
            staged.units.extend(existing.units.iter().cloned());
        }
        if declared_pct.is_none() {
            let active_count = staged.units.iter().filter(|u| u.active.is_some()).count();
            if units.len() != active_count
                || units.iter().any(|u| {
                    !staged
                        .units
                        .iter()
                        .any(|old| old.unit_index == u.unit_index && old.active.is_some())
                })
            {
                return Err(LedgerError::InvalidCompletion);
            }
        }
        for unit in units {
            let row_index = staged
                .units
                .iter()
                .position(|u| u.unit_index == unit.unit_index);
            if let Some(pct) = declared_pct {
                let active = Some(ActiveContribution {
                    raw_q8: unit.raw_q8_delta,
                    declared_pct: pct,
                });
                if let Some(i) = row_index {
                    staged.units[i].active = active;
                } else {
                    staged.units.push(CheckpointUnit {
                        unit_index: unit.unit_index,
                        active,
                        removal_residue_q8: 0,
                    });
                }
            } else {
                let row = &mut staged.units[row_index.ok_or(LedgerError::InvalidCompletion)?];
                let raw = row.active.ok_or(LedgerError::InvalidCompletion)?.raw_q8;
                // Live68: raw=38, completed removal delta=-39 => residue += -1.
                // Widen first: a representable result must not fail due to an
                // overflowing intermediate i32 cancellation.
                let value = i64::from(row.removal_residue_q8)
                    + i64::from(raw)
                    + i64::from(unit.raw_q8_delta);
                row.removal_residue_q8 =
                    i32::try_from(value).map_err(|_| LedgerError::ArithmeticOverflow)?;
                row.active = None;
            }
        }
        // Ensure aggregation is representable, including other named effects.
        for unit_index in 0..city.unit_count {
            let mut raw = 0i64;
            let mut pct = 0i64;
            let mut residue = 0i64;
            for effect in city
                .effects
                .iter()
                .filter(|e| e.identity != staged.identity)
                .chain(std::iter::once(&staged))
            {
                if let Some(unit) = effect.units.iter().find(|u| u.unit_index == unit_index) {
                    if let Some(active) = unit.active {
                        raw = raw
                            .checked_add(i64::from(active.raw_q8))
                            .ok_or(LedgerError::ArithmeticOverflow)?;
                        pct = pct
                            .checked_add(i64::from(active.declared_pct))
                            .ok_or(LedgerError::ArithmeticOverflow)?;
                    }
                    residue = residue
                        .checked_add(i64::from(unit.removal_residue_q8))
                        .ok_or(LedgerError::ArithmeticOverflow)?;
                }
            }
            for value in [raw, pct, residue] {
                i32::try_from(value).map_err(|_| LedgerError::ArithmeticOverflow)?;
            }
        }
        let city = &mut self.cities[index];
        if let Some(i) = existing_index {
            city.effects[i] = staged;
        } else {
            reserve(&mut city.effects, 1)?;
            city.effects.push(staged);
        }
        city.pending = None;
        Ok(())
    }

    pub fn quantity_inputs(
        &self,
        city: &CityIdentity,
        unit_index: u32,
    ) -> Result<QuantityInputs, LedgerError> {
        quantity(&self.cities[self.city_index(city)?], unit_index)
    }

    pub fn quantity_input(
        &self,
        city: &CityIdentity,
        unit_index: u32,
        native: NativeQuantityFeed,
    ) -> Result<QuantityInput, LedgerError> {
        Ok(QuantityInput {
            native,
            named: self.quantity_inputs(city, unit_index)?,
        })
    }

    pub fn invalidate(
        &mut self,
        city: &CityIdentity,
        reason: UnknownReason,
    ) -> Result<(), LedgerError> {
        let index = self.city_index(city)?;
        make_unknown(&mut self.cities[index], reason);
        Ok(())
    }

    /// Actual transfer must be established by the runtime. Keeping the history
    /// as Unknown permits checkpointing uncertainty but never migrates its
    /// contribution into accepted Quantity for the new owner.
    pub fn owner_transfer(
        &mut self,
        old: &CityIdentity,
        new: CityIdentity,
    ) -> Result<(), LedgerError> {
        let index = self.city_index(old)?;
        self.validate_city(&new)?;
        if old.stable.world_id != new.stable.world_id
            || old.stable.birth_generation != new.stable.birth_generation
            || old.stable.owner == new.stable.owner
            || self.cities.iter().enumerate().any(|(i, c)| {
                i != index
                    && (c.identity.stable == new.stable
                        || c.identity.receiver_address == new.receiver_address)
            })
        {
            return Err(LedgerError::InvalidIdentity);
        }
        self.cities[index].identity = new;
        make_unknown(&mut self.cities[index], UnknownReason::OwnerTransferred);
        Ok(())
    }

    pub fn destroy(&mut self, city: &CityIdentity) -> Result<(), LedgerError> {
        let index = self.city_index(city)?;
        self.cities.remove(index);
        Ok(())
    }

    /// Must be called at the caller-proven start of host load, before any new
    /// Quantity. Absence after clearing is MissingCity, never implicit zero.
    pub fn begin_load(&mut self, new_epoch: u64) -> Result<(), LedgerError> {
        if new_epoch <= self.load_epoch {
            return Err(LedgerError::EpochMismatch);
        }
        self.load_epoch = new_epoch;
        self.cities.clear();
        Ok(())
    }

    /// In-memory DTO only. Pending work persists as Unknown(Incomplete), with
    /// the last completed records retained, never a guessed partial update.
    pub fn snapshot(&self) -> Checkpoint {
        Checkpoint {
            version: CHECKPOINT_VERSION,
            cities: self
                .cities
                .iter()
                .map(|city| CheckpointCity {
                    identity: city.identity.stable.clone(),
                    unit_count: city.unit_count,
                    status: if city.pending.is_some() {
                        CheckpointStatus::Unknown(UnknownReason::Incomplete)
                    } else {
                        city.status
                    },
                    effects: city
                        .effects
                        .iter()
                        .map(|effect| CheckpointEffect {
                            identity: effect.identity.stable.clone(),
                            is_active: effect.is_active,
                            units: effect.units.clone(),
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    /// Caller must have reached the authoritative host restore boundary and
    /// verified checkpoint provenance/completeness against that actual save.
    /// Rebinding numeric values here does not establish host lifetime/identity.
    /// Validation is atomic: invalid DTO/bindings publish no restored city.
    pub fn restore_checkpoint(
        &mut self,
        checkpoint: &Checkpoint,
        bindings: &[RestoreBinding],
    ) -> Result<(), LedgerError> {
        if !self.cities.is_empty() {
            return Err(LedgerError::RestoreRequiresEmptyLedger);
        }
        if checkpoint.version != CHECKPOINT_VERSION {
            return Err(LedgerError::InvalidCheckpoint);
        }
        if checkpoint.cities.len() > self.limits.cities || bindings.len() > self.limits.cities {
            return Err(LedgerError::CapacityExceeded);
        }
        if checkpoint.cities.len() != bindings.len() {
            return Err(LedgerError::InvalidRebind);
        }
        let mut staged = Vec::<City>::new();
        reserve(&mut staged, checkpoint.cities.len())?;
        for saved in &checkpoint.cities {
            if !valid_stable_city(&saved.identity)
                || saved.unit_count == 0
                || staged.iter().any(|c| c.identity.stable == saved.identity)
            {
                return Err(LedgerError::InvalidCheckpoint);
            }
            if saved.unit_count as usize > self.limits.units_per_city
                || saved.effects.len() > self.limits.effects_per_city
            {
                return Err(LedgerError::CapacityExceeded);
            }
            let mut matches = bindings.iter().filter(|b| b.city.stable == saved.identity);
            let binding = matches.next().ok_or(LedgerError::InvalidRebind)?;
            if matches.next().is_some() || binding.effects.len() != saved.effects.len() {
                return Err(LedgerError::InvalidRebind);
            }
            self.validate_city(&binding.city)?;
            if staged
                .iter()
                .any(|c| c.identity.receiver_address == binding.city.receiver_address)
            {
                return Err(LedgerError::InvalidRebind);
            }
            let mut city = City {
                identity: binding.city.clone(),
                unit_count: saved.unit_count,
                status: saved.status,
                pending: None,
                effects: Vec::new(),
            };
            reserve(&mut city.effects, saved.effects.len())?;
            let mut record_count = 0usize;
            for saved_effect in &saved.effects {
                if city.effects.iter().any(|e| {
                    e.identity.stable.stable_instance_id == saved_effect.identity.stable_instance_id
                }) {
                    return Err(LedgerError::InvalidCheckpoint);
                }
                let mut matches = binding
                    .effects
                    .iter()
                    .filter(|e| e.stable == saved_effect.identity);
                let effect_binding = matches.next().ok_or(LedgerError::InvalidRebind)?;
                if matches.next().is_some()
                    || !valid_effect(effect_binding)
                    || city
                        .effects
                        .iter()
                        .any(|e| e.identity.instance_binding == effect_binding.instance_binding)
                {
                    return Err(LedgerError::InvalidRebind);
                }
                record_count = record_count
                    .checked_add(saved_effect.units.len())
                    .ok_or(LedgerError::ArithmeticOverflow)?;
                if record_count > self.limits.records_per_city
                    || saved_effect.units.len() > self.limits.units_per_city
                {
                    return Err(LedgerError::CapacityExceeded);
                }
                for (position, unit) in saved_effect.units.iter().enumerate() {
                    if unit.unit_index >= city.unit_count
                        || saved_effect.units[..position]
                            .iter()
                            .any(|u| u.unit_index == unit.unit_index)
                        || unit
                            .active
                            .is_some_and(|a| a.raw_q8 <= 0 || a.declared_pct <= 0)
                        || (!saved_effect.is_active && unit.active.is_some())
                        || unit.active.is_some_and(|active| {
                            saved_effect.units[..position]
                                .iter()
                                .filter_map(|u| u.active)
                                .any(|prior| prior.declared_pct != active.declared_pct)
                        })
                    {
                        return Err(LedgerError::InvalidCheckpoint);
                    }
                }
                let mut units = Vec::new();
                reserve(&mut units, saved_effect.units.len())?;
                units.extend(saved_effect.units.iter().cloned());
                city.effects.push(Effect {
                    identity: effect_binding.clone(),
                    is_active: saved_effect.is_active,
                    units,
                });
            }
            // Unknown stays Unknown; even structurally sound stored rows do
            // not rehabilitate a pending/unwound/otherwise unproven update.
            if city.status == CheckpointStatus::Known {
                for unit in 0..city.unit_count {
                    quantity(&city, unit)?;
                }
            }
            staged.push(city);
        }
        self.cities = staged;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn city() -> CityIdentity {
        CityIdentity {
            stable: StableCityIdentity {
                world_id: "saved-world-68".into(),
                birth_generation: 7,
                owner: 0,
                full_id: 65_536,
            },
            load_epoch: 1,
            receiver_address: 0x1578c3980,
        }
    }

    fn effect(id: u64) -> EffectIdentity {
        EffectIdentity {
            stable: StableEffectIdentity {
                definition_name: "JAC_OATH_ODA_TENKA_FUBU_E1".into(),
                stable_instance_id: id,
            },
            instance_binding: id + 10_000,
        }
    }

    fn ledger() -> ContributionLedger {
        let mut ledger = ContributionLedger::new(1, Limits::default()).unwrap();
        ledger
            .start_proven_boundary(city(), 132, ProvenStartBoundary::RootFresh)
            .unwrap();
        ledger
    }

    fn applied(raw: i32) -> AdmittedCompletion {
        AdmittedCompletion::Applied {
            declared_pct: 15,
            units: vec![UnitDelta {
                unit_index: 7,
                raw_q8_delta: raw,
            }],
        }
    }

    fn removed(delta: i32) -> AdmittedCompletion {
        AdmittedCompletion::Removed {
            units: vec![UnitDelta {
                unit_index: 7,
                raw_q8_delta: delta,
            }],
        }
    }

    fn apply(ledger: &mut ContributionLedger) {
        let ticket = ledger
            .begin(&city(), effect(464), UpdateKind::Apply)
            .unwrap();
        ledger.finish(ticket, &city(), applied(38)).unwrap();
    }

    fn remove(ledger: &mut ContributionLedger) {
        let ticket = ledger
            .begin(&city(), effect(464), UpdateKind::Remove)
            .unwrap();
        ledger.finish(ticket, &city(), removed(-39)).unwrap();
    }

    fn expected(raw: i32, pct: i32, residue: i32) -> QuantityInputs {
        QuantityInputs {
            active_raw_q8: raw,
            declared_pct: pct,
            removal_residue_q8: residue,
        }
    }

    #[test]
    fn zero_raw_change_cannot_publish_a_positive_rate_or_restore_as_active() {
        let mut zero_ledger = ledger();
        let ticket = zero_ledger
            .begin(&city(), effect(464), UpdateKind::Apply)
            .unwrap();
        assert_eq!(
            zero_ledger.finish(ticket, &city(), applied(0)),
            Err(LedgerError::InvalidCompletion)
        );
        assert!(zero_ledger.quantity_inputs(&city(), 7).is_err());
        let mut source = ledger();
        apply(&mut source);
        let mut saved = source.snapshot();
        saved.cities[0].effects[0].units[0]
            .active
            .as_mut()
            .unwrap()
            .raw_q8 = 0;
        let mut restored = ContributionLedger::new(1, Limits::default()).unwrap();
        assert_eq!(
            restored.restore_checkpoint(
                &saved,
                &[RestoreBinding {
                    city: city(),
                    effects: vec![effect(464)]
                }]
            ),
            Err(LedgerError::InvalidCheckpoint)
        );
        assert!(restored.quantity_inputs(&city(), 7).is_err());
    }

    #[test]
    fn live68_keeps_active_and_signed_removal_residue_separate() {
        let mut ledger = ledger();
        assert_eq!(
            ledger.quantity_inputs(&city(), 7).unwrap(),
            expected(0, 0, 0)
        );
        apply(&mut ledger);
        assert_eq!(
            ledger.quantity_inputs(&city(), 7).unwrap(),
            expected(38, 15, 0)
        );
        remove(&mut ledger);
        assert_eq!(
            ledger.quantity_inputs(&city(), 7).unwrap(),
            expected(0, 0, -1)
        );
        apply(&mut ledger);
        let input = ledger.quantity_inputs(&city(), 7).unwrap();
        assert_eq!(input, expected(38, 15, -1));
        assert_eq!(input.active_raw_q8 + input.removal_residue_q8, 37);
        remove(&mut ledger);
        assert_eq!(
            ledger.quantity_inputs(&city(), 7).unwrap(),
            expected(0, 0, -2)
        );
        assert_eq!(
            ledger.quantity_inputs(&city(), 8).unwrap(),
            expected(0, 0, 0)
        );
    }

    #[test]
    fn unrelated_negative_native_feed_is_copied_unchanged() {
        let mut ledger = ledger();
        apply(&mut ledger);
        remove(&mut ledger);
        apply(&mut ledger);
        let native = NativeQuantityFeed {
            owner_q8: 307,
            city_q8: -64 + 37,
            other_rate_q8: 257,
            base_q8: 4096 * 256,
        };
        let input = ledger.quantity_input(&city(), 7, native).unwrap();
        assert_eq!(input.native, native);
        assert_eq!(input.named, expected(38, 15, -1));
        // A future arithmetic consumer subtracts ONLY the named values.
        assert_eq!(
            input.native.city_q8 - input.named.active_raw_q8 - input.named.removal_residue_q8,
            -64
        );
        remove(&mut ledger);
        let off_native = NativeQuantityFeed {
            city_q8: -66,
            ..native
        };
        let off = ledger.quantity_input(&city(), 7, off_native).unwrap();
        assert_eq!(off.native, off_native);
        assert_eq!(off.native.city_q8 - off.named.removal_residue_q8, -64);
    }

    #[test]
    fn pending_blocks_all_city_units_and_publication_is_atomic() {
        let mut ledger = ledger();
        let ticket = ledger
            .begin(&city(), effect(464), UpdateKind::Apply)
            .unwrap();
        for unit in [7, 8, 131] {
            assert_eq!(
                ledger.quantity_inputs(&city(), unit),
                Err(LedgerError::Pending)
            );
        }
        assert_eq!(
            ledger.snapshot().cities[0].status,
            CheckpointStatus::Unknown(UnknownReason::Incomplete)
        );
        ledger
            .finish(
                ticket,
                &city(),
                AdmittedCompletion::Applied {
                    declared_pct: 15,
                    units: vec![
                        UnitDelta {
                            unit_index: 7,
                            raw_q8_delta: 38,
                        },
                        UnitDelta {
                            unit_index: 8,
                            raw_q8_delta: 38,
                        },
                    ],
                },
            )
            .unwrap();
        for unit in [7, 8] {
            assert_eq!(
                ledger.quantity_inputs(&city(), unit).unwrap(),
                expected(38, 15, 0)
            );
        }
        assert_eq!(
            ledger.quantity_inputs(&city(), 131).unwrap(),
            expected(0, 0, 0)
        );
    }

    #[test]
    fn duplicate_apply_and_leading_or_duplicate_remove_are_errors_and_unknown() {
        let mut duplicate = ledger();
        apply(&mut duplicate);
        assert!(matches!(
            duplicate.begin(&city(), effect(464), UpdateKind::Apply),
            Err(LedgerError::DuplicateApply)
        ));
        assert_eq!(
            duplicate.quantity_inputs(&city(), 7),
            Err(LedgerError::Unknown(UnknownReason::DuplicateApply))
        );
        // The previously completed 38 was not replaced by a guessed zero.
        assert_eq!(
            duplicate.snapshot().cities[0].effects[0].units[0].active,
            Some(ActiveContribution {
                raw_q8: 38,
                declared_pct: 15
            })
        );
        let mut leading = ledger();
        assert!(matches!(
            leading.begin(&city(), effect(464), UpdateKind::Remove),
            Err(LedgerError::LeadingRemove)
        ));
        assert_eq!(
            leading.quantity_inputs(&city(), 7),
            Err(LedgerError::Unknown(UnknownReason::LeadingRemove))
        );
        let mut twice = ledger();
        apply(&mut twice);
        remove(&mut twice);
        assert!(matches!(
            twice.begin(&city(), effect(464), UpdateKind::Remove),
            Err(LedgerError::LeadingRemove)
        ));
    }

    #[test]
    fn reentry_invalidates_outer_token_and_denies_even_unrelated_units() {
        let mut ledger = ledger();
        let outer = ledger
            .begin(&city(), effect(464), UpdateKind::Apply)
            .unwrap();
        assert!(matches!(
            ledger.begin(&city(), effect(465), UpdateKind::Apply),
            Err(LedgerError::ReentrantUpdate)
        ));
        assert_eq!(
            ledger.finish(outer, &city(), applied(38)),
            Err(LedgerError::StaleTicket)
        );
        assert_eq!(
            ledger.quantity_inputs(&city(), 8),
            Err(LedgerError::Unknown(UnknownReason::ReentrantUpdate))
        );
    }

    #[test]
    fn unavailable_incomplete_and_unwound_never_publish_and_survive_checkpoint() {
        for (completion, reason) in [
            (AdmittedCompletion::Unavailable, UnknownReason::Unavailable),
            (AdmittedCompletion::Incomplete, UnknownReason::Incomplete),
            (AdmittedCompletion::Unwound, UnknownReason::Unwound),
        ] {
            let mut ledger = ledger();
            apply(&mut ledger);
            let ticket = ledger
                .begin(&city(), effect(464), UpdateKind::Remove)
                .unwrap();
            assert_eq!(
                ledger.finish(ticket, &city(), completion),
                Err(LedgerError::Unknown(reason))
            );
            assert_eq!(
                ledger.quantity_inputs(&city(), 7),
                Err(LedgerError::Unknown(reason))
            );
            let saved = ledger.snapshot();
            assert_eq!(saved.cities[0].status, CheckpointStatus::Unknown(reason));
            assert_eq!(
                saved.cities[0].effects[0].units[0].active,
                Some(ActiveContribution {
                    raw_q8: 38,
                    declared_pct: 15
                })
            );
            let mut restored = ContributionLedger::new(2, Limits::default()).unwrap();
            let mut actual = city();
            actual.load_epoch = 2;
            actual.receiver_address += 4096;
            let binding = RestoreBinding {
                city: actual.clone(),
                effects: vec![effect(464)],
            };
            restored.restore_checkpoint(&saved, &[binding]).unwrap();
            assert_eq!(
                restored.quantity_inputs(&actual, 7),
                Err(LedgerError::Unknown(reason))
            );
        }
    }

    #[test]
    fn completion_identity_changes_cannot_publish() {
        let mut ledger = ledger();
        let ticket = ledger
            .begin(&city(), effect(464), UpdateKind::Apply)
            .unwrap();
        let mut changed = city();
        changed.receiver_address += 8;
        assert_eq!(
            ledger.finish(ticket, &changed, applied(38)),
            Err(LedgerError::IdentityMismatch)
        );
        assert_eq!(
            ledger.quantity_inputs(&city(), 7),
            Err(LedgerError::Unknown(UnknownReason::IdentityChanged))
        );
        assert_eq!(
            ledger.quantity_inputs(&changed, 7),
            Err(LedgerError::IdentityMismatch)
        );
    }

    #[test]
    fn stale_after_destroy_cannot_affect_new_birth_at_reused_receiver() {
        let mut ledger = ledger();
        let ticket = ledger
            .begin(&city(), effect(464), UpdateKind::Apply)
            .unwrap();
        ledger.destroy(&city()).unwrap();
        assert_eq!(
            ledger.quantity_inputs(&city(), 7),
            Err(LedgerError::MissingCity)
        );
        let mut newborn = city();
        newborn.stable.birth_generation += 1;
        ledger
            .start_proven_boundary(newborn.clone(), 132, ProvenStartBoundary::Birth)
            .unwrap();
        let new_ticket = ledger
            .begin(&newborn, effect(464), UpdateKind::Apply)
            .unwrap();
        assert_eq!(
            ledger.finish(ticket, &newborn, applied(38)),
            Err(LedgerError::StaleTicket)
        );
        assert_eq!(
            ledger.quantity_inputs(&newborn, 7),
            Err(LedgerError::Pending)
        );
        ledger.finish(new_ticket, &newborn, applied(38)).unwrap();
        assert_eq!(
            ledger.quantity_inputs(&newborn, 7).unwrap(),
            expected(38, 15, 0)
        );
    }

    #[test]
    fn owner_transfer_invalidates_ticket_and_new_owner_remains_unknown() {
        let mut ledger = ledger();
        apply(&mut ledger);
        let ticket = ledger
            .begin(&city(), effect(464), UpdateKind::Remove)
            .unwrap();
        let mut transferred = city();
        transferred.stable.owner = 1;
        transferred.stable.full_id = 131_073;
        ledger.owner_transfer(&city(), transferred.clone()).unwrap();
        assert_eq!(
            ledger.finish(ticket, &transferred, removed(-39)),
            Err(LedgerError::StaleTicket)
        );
        assert_eq!(
            ledger.quantity_inputs(&transferred, 7),
            Err(LedgerError::Unknown(UnknownReason::OwnerTransferred))
        );
        assert_eq!(
            ledger.quantity_inputs(&city(), 7),
            Err(LedgerError::MissingCity)
        );
        assert_eq!(ledger.snapshot().cities[0].identity, transferred.stable);
    }

    #[test]
    fn load_invalidates_tickets_even_with_identical_native_address() {
        let mut ledger = ledger();
        let ticket = ledger
            .begin(&city(), effect(464), UpdateKind::Apply)
            .unwrap();
        ledger.begin_load(2).unwrap();
        let mut loaded = city();
        loaded.load_epoch = 2;
        assert_eq!(
            ledger.quantity_inputs(&loaded, 7),
            Err(LedgerError::MissingCity)
        );
        // This fresh boundary is asserted only in the synthetic fixture.
        ledger
            .start_proven_boundary(loaded.clone(), 132, ProvenStartBoundary::RootFresh)
            .unwrap();
        let fresh = ledger
            .begin(&loaded, effect(464), UpdateKind::Apply)
            .unwrap();
        assert_eq!(
            ledger.finish(ticket, &loaded, applied(38)),
            Err(LedgerError::StaleTicket)
        );
        assert_eq!(
            ledger.quantity_inputs(&loaded, 7),
            Err(LedgerError::Pending)
        );
        ledger.finish(fresh, &loaded, applied(38)).unwrap();
    }

    #[test]
    fn checkpoint_rebinds_to_new_receiver_epoch_and_instance_incarnation() {
        let mut old = ledger();
        apply(&mut old);
        remove(&mut old);
        apply(&mut old);
        let saved = old.snapshot();
        let mut restored = ContributionLedger::new(20, Limits::default()).unwrap();
        let mut actual = city();
        actual.load_epoch = 20;
        actual.receiver_address = 0x9876;
        let mut actual_effect = effect(464);
        actual_effect.instance_binding = 55;
        assert_eq!(
            restored.quantity_inputs(&actual, 7),
            Err(LedgerError::MissingCity)
        );
        restored
            .restore_checkpoint(
                &saved,
                &[RestoreBinding {
                    city: actual.clone(),
                    effects: vec![actual_effect.clone()],
                }],
            )
            .unwrap();
        assert_eq!(
            restored.quantity_inputs(&actual, 7).unwrap(),
            expected(38, 15, -1)
        );
        assert_eq!(restored.snapshot(), saved); // no process-local identity leaked
        let ticket = restored
            .begin(&actual, actual_effect, UpdateKind::Remove)
            .unwrap();
        restored.finish(ticket, &actual, removed(-39)).unwrap();
        assert_eq!(
            restored.quantity_inputs(&actual, 7).unwrap(),
            expected(0, 0, -2)
        );
        assert_eq!(
            restored.quantity_inputs(&city(), 7),
            Err(LedgerError::EpochMismatch)
        );
    }

    #[test]
    fn pending_snapshot_restores_unknown_not_old_known_quantity() {
        let mut old = ledger();
        apply(&mut old);
        let abandoned = old.begin(&city(), effect(464), UpdateKind::Remove).unwrap();
        let saved = old.snapshot();
        assert_eq!(old.quantity_inputs(&city(), 7), Err(LedgerError::Pending));
        let mut restored = ContributionLedger::new(1, Limits::default()).unwrap();
        restored
            .restore_checkpoint(
                &saved,
                &[RestoreBinding {
                    city: city(),
                    effects: vec![effect(464)],
                }],
            )
            .unwrap();
        assert_eq!(
            restored.quantity_inputs(&city(), 7),
            Err(LedgerError::Unknown(UnknownReason::Incomplete))
        );
        assert_eq!(
            restored.finish(abandoned, &city(), removed(-39)),
            Err(LedgerError::StaleTicket)
        );
    }

    #[test]
    fn stale_same_identity_after_explicit_boundary_cannot_finish_new_pending() {
        let mut ledger = ledger();
        let old = ledger
            .begin(&city(), effect(464), UpdateKind::Apply)
            .unwrap();
        ledger
            .start_proven_boundary(city(), 132, ProvenStartBoundary::RootFresh)
            .unwrap();
        let current = ledger
            .begin(&city(), effect(464), UpdateKind::Apply)
            .unwrap();
        assert_eq!(
            ledger.finish(old, &city(), applied(38)),
            Err(LedgerError::StaleTicket)
        );
        assert_eq!(
            ledger.quantity_inputs(&city(), 7),
            Err(LedgerError::Pending)
        );
        ledger.finish(current, &city(), applied(38)).unwrap();
    }

    #[test]
    fn changed_instance_or_definition_is_not_a_second_contribution() {
        for changed_name in [false, true] {
            let mut ledger = ledger();
            apply(&mut ledger);
            remove(&mut ledger);
            let mut changed = effect(464);
            if changed_name {
                changed.stable.definition_name = "OTHER_DEFINITION".into();
            } else {
                changed.instance_binding += 1;
            }
            assert!(matches!(
                ledger.begin(&city(), changed, UpdateKind::Apply),
                Err(LedgerError::InstanceBindingMismatch)
            ));
            assert_eq!(
                ledger.quantity_inputs(&city(), 7),
                Err(LedgerError::Unknown(UnknownReason::IdentityChanged))
            );
        }
    }

    #[test]
    fn incomplete_unit_set_remove_is_unknown_and_commits_no_partial_rows() {
        let mut ledger = ledger();
        let ticket = ledger
            .begin(&city(), effect(464), UpdateKind::Apply)
            .unwrap();
        ledger
            .finish(
                ticket,
                &city(),
                AdmittedCompletion::Applied {
                    declared_pct: 15,
                    units: vec![
                        UnitDelta {
                            unit_index: 7,
                            raw_q8_delta: 38,
                        },
                        UnitDelta {
                            unit_index: 8,
                            raw_q8_delta: 38,
                        },
                    ],
                },
            )
            .unwrap();
        let before = ledger.snapshot().cities[0].effects.clone();
        let ticket = ledger
            .begin(&city(), effect(464), UpdateKind::Remove)
            .unwrap();
        assert_eq!(
            ledger.finish(ticket, &city(), removed(-39)),
            Err(LedgerError::InvalidCompletion)
        );
        assert_eq!(ledger.snapshot().cities[0].effects, before);
        assert_eq!(
            ledger.quantity_inputs(&city(), 8),
            Err(LedgerError::Unknown(UnknownReason::InvalidCompletion))
        );
    }

    #[test]
    fn raw_percent_residue_and_ticket_overflow_are_explicit_and_fail_closed() {
        for percent_overflow in [false, true] {
            let mut ledger = ledger();
            let first = ledger.begin(&city(), effect(1), UpdateKind::Apply).unwrap();
            ledger
                .finish(
                    first,
                    &city(),
                    AdmittedCompletion::Applied {
                        declared_pct: if percent_overflow { i32::MAX } else { 15 },
                        units: vec![UnitDelta {
                            unit_index: 7,
                            raw_q8_delta: if percent_overflow { 38 } else { i32::MAX },
                        }],
                    },
                )
                .unwrap();
            let second = ledger.begin(&city(), effect(2), UpdateKind::Apply).unwrap();
            assert_eq!(
                ledger.finish(second, &city(), applied(1)),
                Err(LedgerError::ArithmeticOverflow)
            );
            assert_eq!(ledger.snapshot().cities[0].effects.len(), 1);
            assert_eq!(
                ledger.quantity_inputs(&city(), 7),
                Err(LedgerError::Unknown(UnknownReason::ArithmeticOverflow))
            );
        }
        let mut residue = ledger();
        apply(&mut residue);
        let ticket = residue
            .begin(&city(), effect(464), UpdateKind::Remove)
            .unwrap();
        assert_eq!(
            residue.finish(ticket, &city(), removed(i32::MAX)),
            Err(LedgerError::ArithmeticOverflow)
        );
        let mut serial = ledger();
        serial.next_serial = u64::MAX;
        assert!(matches!(
            serial.begin(&city(), effect(464), UpdateKind::Apply),
            Err(LedgerError::ArithmeticOverflow)
        ));
        assert_eq!(
            serial.quantity_inputs(&city(), 7),
            Err(LedgerError::Unknown(UnknownReason::ArithmeticOverflow))
        );
    }

    #[test]
    fn finite_city_effect_unit_and_historical_record_limits_are_enforced() {
        let limits = Limits {
            cities: 1,
            effects_per_city: 1,
            units_per_city: 132,
            records_per_city: 1,
        };
        let mut bounded = ContributionLedger::new(1, limits).unwrap();
        bounded
            .start_proven_boundary(city(), 132, ProvenStartBoundary::Birth)
            .unwrap();
        let mut another = city();
        another.stable.birth_generation += 1;
        another.receiver_address += 4096;
        assert_eq!(
            bounded.start_proven_boundary(another, 132, ProvenStartBoundary::Birth),
            Err(LedgerError::CapacityExceeded)
        );
        apply(&mut bounded);
        remove(&mut bounded);
        // The inactive unit's residue still consumes capacity.
        let ticket = bounded
            .begin(&city(), effect(464), UpdateKind::Apply)
            .unwrap();
        assert_eq!(
            bounded.finish(
                ticket,
                &city(),
                AdmittedCompletion::Applied {
                    declared_pct: 15,
                    units: vec![UnitDelta {
                        unit_index: 8,
                        raw_q8_delta: 38
                    }],
                }
            ),
            Err(LedgerError::CapacityExceeded)
        );
        assert_eq!(
            bounded.snapshot().cities[0].effects[0].units[0].removal_residue_q8,
            -1
        );
        let mut effect_bound = ContributionLedger::new(1, limits).unwrap();
        effect_bound
            .start_proven_boundary(city(), 132, ProvenStartBoundary::Birth)
            .unwrap();
        apply(&mut effect_bound);
        assert!(matches!(
            effect_bound.begin(&city(), effect(465), UpdateKind::Apply),
            Err(LedgerError::CapacityExceeded)
        ));
        let mut unit_bound = ledger();
        let ticket = unit_bound
            .begin(&city(), effect(464), UpdateKind::Apply)
            .unwrap();
        assert_eq!(
            unit_bound.finish(
                ticket,
                &city(),
                AdmittedCompletion::Applied {
                    declared_pct: 15,
                    units: vec![UnitDelta {
                        unit_index: 132,
                        raw_q8_delta: 38
                    }],
                }
            ),
            Err(LedgerError::UnitOutOfRange)
        );
        assert!(matches!(
            ContributionLedger::new(
                1,
                Limits {
                    cities: usize::MAX,
                    ..limits
                }
            ),
            Err(LedgerError::InvalidLimits)
        ));
    }

    #[test]
    fn checkpoint_validation_requires_exact_rebind_and_is_atomic() {
        let mut old = ledger();
        apply(&mut old);
        let saved = old.snapshot();
        let binding = RestoreBinding {
            city: city(),
            effects: vec![effect(464)],
        };
        let mut target = ContributionLedger::new(1, Limits::default()).unwrap();
        assert_eq!(
            target.restore_checkpoint(&saved, &[]),
            Err(LedgerError::InvalidRebind)
        );
        let mut wrong = binding.clone();
        wrong.city.stable.owner += 1;
        assert_eq!(
            target.restore_checkpoint(&saved, &[wrong]),
            Err(LedgerError::InvalidRebind)
        );
        let mut corrupt = saved.clone();
        let duplicated_unit = corrupt.cities[0].effects[0].units[0].clone();
        corrupt.cities[0].effects[0].units.push(duplicated_unit);
        assert_eq!(
            target.restore_checkpoint(&corrupt, std::slice::from_ref(&binding)),
            Err(LedgerError::InvalidCheckpoint)
        );
        assert_eq!(
            target.quantity_inputs(&city(), 7),
            Err(LedgerError::MissingCity)
        );
        target
            .restore_checkpoint(&saved, std::slice::from_ref(&binding))
            .unwrap();
        assert_eq!(
            target.restore_checkpoint(&saved, &[binding]),
            Err(LedgerError::RestoreRequiresEmptyLedger)
        );
    }

    #[test]
    fn signed_residue_cancellation_is_order_independent_and_not_baseline_inference() {
        let mut source = ledger();
        apply(&mut source);
        remove(&mut source);
        let mut saved = source.snapshot();
        let template = saved.cities[0].effects[0].clone();
        saved.cities[0].effects.clear();
        let mut bindings = Vec::new();
        for (id, residue) in [(1, i32::MAX), (2, 1), (3, -1)] {
            let mut stored = template.clone();
            stored.identity = effect(id).stable;
            stored.units[0].removal_residue_q8 = residue;
            saved.cities[0].effects.push(stored);
            bindings.push(effect(id));
        }
        let mut target = ContributionLedger::new(1, Limits::default()).unwrap();
        target
            .restore_checkpoint(
                &saved,
                &[RestoreBinding {
                    city: city(),
                    effects: bindings,
                }],
            )
            .unwrap();
        assert_eq!(
            target.quantity_inputs(&city(), 7).unwrap(),
            expected(0, 0, i32::MAX)
        );
    }

    #[test]
    fn distinct_cities_can_share_birth_generation_without_sharing_contributions() {
        let mut ledger = ledger();
        apply(&mut ledger);
        let mut second = city();
        second.stable.full_id = 131_073;
        second.receiver_address += 4096;
        ledger
            .start_proven_boundary(second.clone(), 132, ProvenStartBoundary::Birth)
            .unwrap();
        assert_eq!(
            ledger.quantity_inputs(&second, 7).unwrap(),
            expected(0, 0, 0)
        );
        assert_eq!(
            ledger.quantity_inputs(&city(), 7).unwrap(),
            expected(38, 15, 0)
        );
        let saved = ledger.snapshot();
        let mut restored = ContributionLedger::new(1, Limits::default()).unwrap();
        // Bindings deliberately reverse the checkpoint order.
        restored
            .restore_checkpoint(
                &saved,
                &[
                    RestoreBinding {
                        city: second.clone(),
                        effects: vec![],
                    },
                    RestoreBinding {
                        city: city(),
                        effects: vec![effect(464)],
                    },
                ],
            )
            .unwrap();
        assert_eq!(
            restored.quantity_inputs(&second, 7).unwrap(),
            expected(0, 0, 0)
        );
        assert_eq!(
            restored.quantity_inputs(&city(), 7).unwrap(),
            expected(38, 15, 0)
        );
    }

    #[test]
    fn aliasing_instance_bindings_cannot_double_count_or_restore() {
        let mut source = ledger();
        apply(&mut source);
        let mut alias = effect(465);
        alias.instance_binding = effect(464).instance_binding;
        assert!(matches!(
            source.begin(&city(), alias, UpdateKind::Apply),
            Err(LedgerError::InstanceBindingMismatch)
        ));
        let mut source = ledger();
        apply(&mut source);
        let other = source
            .begin(&city(), effect(465), UpdateKind::Apply)
            .unwrap();
        source.finish(other, &city(), applied(38)).unwrap();
        assert_eq!(
            source.quantity_inputs(&city(), 7).unwrap(),
            expected(76, 30, 0)
        );
        let saved = source.snapshot();
        let mut alias = effect(465);
        alias.instance_binding = effect(464).instance_binding;
        let mut target = ContributionLedger::new(1, Limits::default()).unwrap();
        assert_eq!(
            target.restore_checkpoint(
                &saved,
                &[RestoreBinding {
                    city: city(),
                    effects: vec![effect(464), alias],
                }]
            ),
            Err(LedgerError::InvalidRebind)
        );
        assert_eq!(
            target.quantity_inputs(&city(), 7),
            Err(LedgerError::MissingCity)
        );
    }

    #[test]
    fn identifier_and_completion_sizes_are_bounded_before_publication() {
        let mut oversized_city = city();
        oversized_city.stable.world_id = "w".repeat(MAX_WORLD_ID_BYTES + 1);
        let mut source = ledger();
        assert_eq!(
            source.start_proven_boundary(oversized_city, 132, ProvenStartBoundary::Birth),
            Err(LedgerError::InvalidIdentity)
        );
        let mut long_definition = effect(464);
        long_definition.stable.definition_name = "N".repeat(MAX_DEFINITION_BYTES + 1);
        assert!(matches!(
            source.begin(&city(), long_definition, UpdateKind::Apply),
            Err(LedgerError::InvalidIdentity)
        ));
        let mut source = ledger();
        let pending = source
            .begin(&city(), effect(464), UpdateKind::Apply)
            .unwrap();
        assert_eq!(
            source.finish(
                pending,
                &city(),
                AdmittedCompletion::Applied {
                    declared_pct: 15,
                    units: vec![
                        UnitDelta {
                            unit_index: 7,
                            raw_q8_delta: 38
                        };
                        513
                    ],
                }
            ),
            Err(LedgerError::CapacityExceeded)
        );
        assert_eq!(source.snapshot().cities[0].effects.len(), 0);
        assert_eq!(
            source.quantity_inputs(&city(), 7),
            Err(LedgerError::Unknown(UnknownReason::CapacityExceeded))
        );
    }

    #[test]
    fn negative_historical_residue_overflow_preserves_previous_completed_rows() {
        let mut source = ledger();
        apply(&mut source);
        let mut saved = source.snapshot();
        saved.cities[0].effects[0].units[0].removal_residue_q8 = i32::MIN;
        let mut target = ContributionLedger::new(1, Limits::default()).unwrap();
        target
            .restore_checkpoint(
                &saved,
                &[RestoreBinding {
                    city: city(),
                    effects: vec![effect(464)],
                }],
            )
            .unwrap();
        let pending = target
            .begin(&city(), effect(464), UpdateKind::Remove)
            .unwrap();
        assert_eq!(
            target.finish(pending, &city(), removed(-39)),
            Err(LedgerError::ArithmeticOverflow)
        );
        let after = target.snapshot();
        assert_eq!(after.cities[0].effects, saved.cities[0].effects);
        assert_eq!(
            after.cities[0].status,
            CheckpointStatus::Unknown(UnknownReason::ArithmeticOverflow)
        );
    }
}
