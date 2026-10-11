//! Pointer-free checkpoint transport for an official Game string property.
//! This codec is not a save hook, host lease, or contribution authority.
//! Its checksum detects accidental corruption; the restoring runtime still
//! verifies the actual world, lifecycle boundary and current identity bindings.
use crate::contribution_ledger::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const PROPERTY_KEY: &str = "JAC_OATH_NATIVE_CONTRIBUTIONS_V1";
const PREFIX: &str = "JAC-OATH-LEDGER-1:";
const MAX_BYTES: usize = 4 * 1024 * 1024;
/// Maximum ASCII property length, including the format prefix and hex body.
/// Native storage must allow this many bytes plus its terminating NUL.
pub const MAX_PROPERTY_BYTES: usize = PREFIX.len() + MAX_BYTES * 2;
const MAGIC: &[u8; 4] = b"JOAL";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveError {
    Bounds,
    Corrupt,
    Unsupported,
    Identity,
    PropertyUnavailable,
    WriteNotConfirmed,
}
type Result<T> = std::result::Result<T, SaveError>;

/// Implemented by the runtime's official Game property accessor, inside the
/// native save/load holding interval. A file/sidecar is not an implementation.
/// The interface itself does not prove that interval or native attribution.
pub trait OfficialGameProperty {
    fn read(&mut self, key: &str) -> Result<Option<String>>;
    fn write(&mut self, key: &str, value: &str) -> Result<()>;
}

pub fn write_official(
    property: &mut impl OfficialGameProperty,
    checkpoint: &Checkpoint,
    limits: Limits,
) -> Result<()> {
    let value = encode(checkpoint, limits)?;
    property.write(PROPERTY_KEY, &value)?;
    if property.read(PROPERTY_KEY)?.as_deref() != Some(value.as_str()) {
        return Err(SaveError::WriteNotConfirmed);
    }
    Ok(())
}

/// Missing state remains missing. It is never replaced with an empty or
/// known-zero checkpoint, including when the original array is negative.
pub fn read_official(
    property: &mut impl OfficialGameProperty,
    limits: Limits,
    actual_world_id: &str,
) -> Result<Checkpoint> {
    let value = property
        .read(PROPERTY_KEY)?
        .ok_or(SaveError::PropertyUnavailable)?;
    decode(&value, limits, actual_world_id)
}

fn reason_code(reason: UnknownReason) -> u8 {
    match reason {
        UnknownReason::Unavailable => 1,
        UnknownReason::Incomplete => 2,
        UnknownReason::Unwound => 3,
        UnknownReason::ReentrantUpdate => 4,
        UnknownReason::IdentityChanged => 5,
        UnknownReason::OwnerTransferred => 6,
        UnknownReason::DuplicateApply => 7,
        UnknownReason::LeadingRemove => 8,
        UnknownReason::InvalidCompletion => 9,
        UnknownReason::CapacityExceeded => 10,
        UnknownReason::ArithmeticOverflow => 11,
        UnknownReason::AllocationFailed => 12,
    }
}
fn reason(code: u8) -> Result<UnknownReason> {
    Ok(match code {
        1 => UnknownReason::Unavailable,
        2 => UnknownReason::Incomplete,
        3 => UnknownReason::Unwound,
        4 => UnknownReason::ReentrantUpdate,
        5 => UnknownReason::IdentityChanged,
        6 => UnknownReason::OwnerTransferred,
        7 => UnknownReason::DuplicateApply,
        8 => UnknownReason::LeadingRemove,
        9 => UnknownReason::InvalidCompletion,
        10 => UnknownReason::CapacityExceeded,
        11 => UnknownReason::ArithmeticOverflow,
        12 => UnknownReason::AllocationFailed,
        _ => return Err(SaveError::Unsupported),
    })
}

fn validate(checkpoint: &Checkpoint, limits: Limits, expected_world: Option<&str>) -> Result<()> {
    if checkpoint.version != CHECKPOINT_VERSION {
        return Err(SaveError::Unsupported);
    }
    if checkpoint.cities.len() > limits.cities {
        return Err(SaveError::Bounds);
    }
    let mut identities = BTreeSet::new();
    for city in &checkpoint.cities {
        let id = &city.identity;
        if id.world_id.is_empty()
            || id.world_id.len() > 128
            || id.world_id.chars().any(char::is_control)
            || id.birth_generation == 0
            || id.owner < 0
            || expected_world.is_some_and(|world| world != id.world_id)
            || !identities.insert((&id.world_id, id.birth_generation, id.owner, id.full_id))
        {
            return Err(SaveError::Identity);
        }
        if city.unit_count == 0
            || city.unit_count as usize > limits.units_per_city
            || city.effects.len() > limits.effects_per_city
        {
            return Err(SaveError::Bounds);
        }
        let mut effects = BTreeSet::new();
        let mut record_count = 0usize;
        for effect in &city.effects {
            let id = &effect.identity;
            if id.definition_name.is_empty()
                || id.definition_name.len() > 64
                || !id
                    .definition_name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                || id.stable_instance_id == 0
                || !effects.insert(id.stable_instance_id)
            {
                return Err(SaveError::Identity);
            }
            record_count = record_count
                .checked_add(effect.units.len())
                .ok_or(SaveError::Bounds)?;
            if record_count > limits.records_per_city {
                return Err(SaveError::Bounds);
            }
            let mut units = BTreeSet::new();
            for unit in &effect.units {
                if unit.unit_index >= city.unit_count || !units.insert(unit.unit_index) {
                    return Err(SaveError::Bounds);
                }
                if let Some(active) = unit.active {
                    if !effect.is_active || active.raw_q8 <= 0 || active.declared_pct <= 0 {
                        return Err(SaveError::Corrupt);
                    }
                }
            }
        }
    }
    Ok(())
}

struct Writer(Vec<u8>);
impl Writer {
    fn put(&mut self, bytes: &[u8]) -> Result<()> {
        let next = self
            .0
            .len()
            .checked_add(bytes.len())
            .ok_or(SaveError::Bounds)?;
        if next > MAX_BYTES - 32 {
            return Err(SaveError::Bounds);
        }
        self.0
            .try_reserve(bytes.len())
            .map_err(|_| SaveError::Bounds)?;
        self.0.extend_from_slice(bytes);
        Ok(())
    }
    fn string(&mut self, text: &str) -> Result<()> {
        let size = u16::try_from(text.len()).map_err(|_| SaveError::Bounds)?;
        self.put(&size.to_le_bytes())?;
        self.put(text.as_bytes())
    }
    fn count(&mut self, count: usize) -> Result<()> {
        self.put(
            &u32::try_from(count)
                .map_err(|_| SaveError::Bounds)?
                .to_le_bytes(),
        )
    }
}

/// Returns an ASCII value suitable for Game:SetProperty(PROPERTY_KEY, value).
/// No file, process, Lua API, or game function is touched here.
pub fn encode(checkpoint: &Checkpoint, limits: Limits) -> Result<String> {
    validate(checkpoint, limits, None)?;
    let mut out = Writer(Vec::new());
    out.put(MAGIC)?;
    out.put(&checkpoint.version.to_le_bytes())?;
    out.count(checkpoint.cities.len())?;
    for city in &checkpoint.cities {
        out.string(&city.identity.world_id)?;
        out.put(&city.identity.birth_generation.to_le_bytes())?;
        out.put(&city.identity.owner.to_le_bytes())?;
        out.put(&city.identity.full_id.to_le_bytes())?;
        out.put(&city.unit_count.to_le_bytes())?;
        out.put(&[match city.status {
            CheckpointStatus::Known => 0,
            CheckpointStatus::Unknown(why) => reason_code(why),
        }])?;
        out.count(city.effects.len())?;
        for effect in &city.effects {
            out.string(&effect.identity.definition_name)?;
            out.put(&effect.identity.stable_instance_id.to_le_bytes())?;
            out.put(&[u8::from(effect.is_active)])?;
            out.count(effect.units.len())?;
            for unit in &effect.units {
                out.put(&unit.unit_index.to_le_bytes())?;
                out.put(&[u8::from(unit.active.is_some())])?;
                if let Some(active) = unit.active {
                    out.put(&active.raw_q8.to_le_bytes())?;
                    out.put(&active.declared_pct.to_le_bytes())?;
                }
                out.put(&unit.removal_residue_q8.to_le_bytes())?;
            }
        }
    }
    let digest = Sha256::digest(&out.0);
    out.0.extend_from_slice(&digest);
    let mut text = String::new();
    text.try_reserve_exact(PREFIX.len() + out.0.len() * 2)
        .map_err(|_| SaveError::Bounds)?;
    text.push_str(PREFIX);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in out.0 {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 15) as usize] as char);
    }
    Ok(text)
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}
impl Reader<'_> {
    fn read<const N: usize>(&mut self) -> Result<[u8; N]> {
        let next = self.cursor.checked_add(N).ok_or(SaveError::Bounds)?;
        let value = self
            .bytes
            .get(self.cursor..next)
            .ok_or(SaveError::Corrupt)?;
        self.cursor = next;
        Ok(value.try_into().unwrap())
    }
    fn count(&mut self, bound: usize) -> Result<usize> {
        let count = u32::from_le_bytes(self.read()?) as usize;
        if count > bound {
            return Err(SaveError::Bounds);
        }
        Ok(count)
    }
    fn string(&mut self, bound: usize) -> Result<String> {
        let count = u16::from_le_bytes(self.read()?) as usize;
        if count == 0 || count > bound {
            return Err(SaveError::Bounds);
        }
        let next = self.cursor.checked_add(count).ok_or(SaveError::Bounds)?;
        let text = std::str::from_utf8(
            self.bytes
                .get(self.cursor..next)
                .ok_or(SaveError::Corrupt)?,
        )
        .map_err(|_| SaveError::Corrupt)?
        .to_owned();
        self.cursor = next;
        Ok(text)
    }
    fn flag(&mut self) -> Result<bool> {
        match self.read::<1>()?[0] {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(SaveError::Corrupt),
        }
    }
}

/// Decodes only into pointer-free DTOs. Current native addresses and instance
/// incarnations must come from the restoring runtime, never from this value.
pub fn decode(text: &str, limits: Limits, expected_world: &str) -> Result<Checkpoint> {
    if text.len() > MAX_PROPERTY_BYTES {
        return Err(SaveError::Bounds);
    }
    let body = text.strip_prefix(PREFIX).ok_or(SaveError::Unsupported)?;
    if !body.len().is_multiple_of(2) || body.len() < (12 + 32) * 2 {
        return Err(SaveError::Corrupt);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(body.len() / 2)
        .map_err(|_| SaveError::Bounds)?;
    let nibble = |b: u8| match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        _ => Err(SaveError::Corrupt),
    };
    for pair in body.as_bytes().chunks_exact(2) {
        bytes.push(nibble(pair[0])? * 16 + nibble(pair[1])?);
    }
    let split = bytes.len() - 32;
    if Sha256::digest(&bytes[..split])[..] != bytes[split..] {
        return Err(SaveError::Corrupt);
    }
    let mut input = Reader {
        bytes: &bytes[..split],
        cursor: 0,
    };
    if input.read::<4>()? != *MAGIC {
        return Err(SaveError::Unsupported);
    }
    let version = u32::from_le_bytes(input.read()?);
    if version != CHECKPOINT_VERSION {
        return Err(SaveError::Unsupported);
    }
    let count = input.count(limits.cities)?;
    let mut cities = Vec::new();
    for _ in 0..count {
        let identity = StableCityIdentity {
            world_id: input.string(128)?,
            birth_generation: u64::from_le_bytes(input.read()?),
            owner: i32::from_le_bytes(input.read()?),
            full_id: u32::from_le_bytes(input.read()?),
        };
        let unit_count = u32::from_le_bytes(input.read()?);
        let status = match input.read::<1>()?[0] {
            0 => CheckpointStatus::Known,
            code => CheckpointStatus::Unknown(reason(code)?),
        };
        let count = input.count(limits.effects_per_city)?;
        let mut effects = Vec::new();
        let mut records = 0usize;
        for _ in 0..count {
            let identity = StableEffectIdentity {
                definition_name: input.string(64)?,
                stable_instance_id: u64::from_le_bytes(input.read()?),
            };
            let is_active = input.flag()?;
            let count = input.count(
                limits
                    .records_per_city
                    .checked_sub(records)
                    .ok_or(SaveError::Bounds)?,
            )?;
            records += count;
            let mut units = Vec::new();
            for _ in 0..count {
                let unit_index = u32::from_le_bytes(input.read()?);
                let active = if input.flag()? {
                    Some(ActiveContribution {
                        raw_q8: i32::from_le_bytes(input.read()?),
                        declared_pct: i32::from_le_bytes(input.read()?),
                    })
                } else {
                    None
                };
                units.push(CheckpointUnit {
                    unit_index,
                    active,
                    removal_residue_q8: i32::from_le_bytes(input.read()?),
                });
            }
            effects.push(CheckpointEffect {
                identity,
                is_active,
                units,
            });
        }
        cities.push(CheckpointCity {
            identity,
            unit_count,
            status,
            effects,
        });
    }
    if input.cursor != input.bytes.len() {
        return Err(SaveError::Corrupt);
    }
    let checkpoint = Checkpoint { version, cities };
    validate(&checkpoint, limits, Some(expected_world))?;
    Ok(checkpoint)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn checkpoint() -> Checkpoint {
        Checkpoint {
            version: CHECKPOINT_VERSION,
            cities: vec![CheckpointCity {
                identity: StableCityIdentity {
                    world_id: "world-é-1".into(),
                    birth_generation: 3,
                    owner: 0,
                    full_id: 65536,
                },
                unit_count: 132,
                status: CheckpointStatus::Known,
                effects: vec![CheckpointEffect {
                    identity: StableEffectIdentity {
                        definition_name: "JAC_OATH_ODA_TENKA_FUBU_E1".into(),
                        stable_instance_id: 464,
                    },
                    is_active: true,
                    units: vec![CheckpointUnit {
                        unit_index: 7,
                        active: Some(ActiveContribution {
                            raw_q8: 38,
                            declared_pct: 15,
                        }),
                        removal_residue_q8: -1,
                    }],
                }],
            }],
        }
    }
    #[test]
    fn pointer_free_ascii_property_preserves_active_and_off_residue() {
        let mut source = checkpoint();
        for active in [true, false] {
            source.cities[0].effects[0].is_active = active;
            source.cities[0].effects[0].units[0].active = active.then_some(ActiveContribution {
                raw_q8: 38,
                declared_pct: 15,
            });
            let text = encode(&source, Limits::default()).unwrap();
            assert!(text.is_ascii());
            assert_eq!(
                decode(&text, Limits::default(), "world-é-1").unwrap(),
                source
            );
            assert_eq!(
                decode(&text, Limits::default(), "another-world"),
                Err(SaveError::Identity)
            );
        }
        source.cities[0].status = CheckpointStatus::Unknown(UnknownReason::Incomplete);
        let text = encode(&source, Limits::default()).unwrap();
        assert_eq!(
            decode(&text, Limits::default(), "world-é-1").unwrap(),
            source
        );
    }
    #[test]
    fn missing_truncated_corrupt_and_over_budget_values_are_refused() {
        let text = encode(&checkpoint(), Limits::default()).unwrap();
        for length in [0, 16, text.len() - 1, text.len() - 2] {
            assert!(decode(&text[..length], Limits::default(), "world-é-1").is_err());
        }
        let mut corrupt = text.into_bytes();
        let last = corrupt.len() - 1;
        corrupt[last] = if corrupt[last] == b'0' { b'1' } else { b'0' };
        assert_eq!(
            decode(
                std::str::from_utf8(&corrupt).unwrap(),
                Limits::default(),
                "world-é-1"
            ),
            Err(SaveError::Corrupt)
        );
        let mut source = checkpoint();
        let duplicate = source.cities[0].effects[0].units[0].clone();
        source.cities[0].effects[0].units.push(duplicate);
        assert!(encode(&source, Limits::default()).is_err());
        let text = encode(&checkpoint(), Limits::default()).unwrap();
        assert_eq!(
            decode(
                &text,
                Limits {
                    records_per_city: 0,
                    ..Limits::default()
                },
                "world-é-1"
            ),
            Err(SaveError::Bounds)
        );
    }
    #[test]
    fn official_property_requires_readback_and_preserves_missing_world_and_unknown_state() {
        struct Property {
            value: Option<String>,
            discard: bool,
        }
        impl OfficialGameProperty for Property {
            fn read(&mut self, key: &str) -> Result<Option<String>> {
                assert_eq!(key, PROPERTY_KEY);
                Ok(self.value.clone())
            }
            fn write(&mut self, key: &str, value: &str) -> Result<()> {
                assert_eq!(key, PROPERTY_KEY);
                if !self.discard {
                    self.value = Some(value.to_owned());
                }
                Ok(())
            }
        }
        let mut property = Property {
            value: None,
            discard: false,
        };
        assert_eq!(
            read_official(&mut property, Limits::default(), "world-é-1"),
            Err(SaveError::PropertyUnavailable)
        );
        let mut source = checkpoint();
        source.cities[0].status = CheckpointStatus::Unknown(UnknownReason::Unwound);
        write_official(&mut property, &source, Limits::default()).unwrap();
        assert_eq!(
            read_official(&mut property, Limits::default(), "world-é-1").unwrap(),
            source
        );
        assert_eq!(
            read_official(&mut property, Limits::default(), "other-world"),
            Err(SaveError::Identity)
        );
        property.discard = true;
        source.cities[0].status = CheckpointStatus::Known;
        assert_eq!(
            write_official(&mut property, &source, Limits::default()),
            Err(SaveError::WriteNotConfirmed)
        );
    }
    #[test]
    fn same_birth_generation_in_different_cities_roundtrips_but_duplicate_identity_does_not() {
        let mut source = checkpoint();
        let mut second = source.cities[0].clone();
        second.identity.full_id = 131073;
        source.cities.push(second);
        let text = encode(&source, Limits::default()).unwrap();
        assert_eq!(
            decode(&text, Limits::default(), "world-é-1").unwrap(),
            source
        );
        source.cities[1].identity = source.cities[0].identity.clone();
        assert_eq!(encode(&source, Limits::default()), Err(SaveError::Identity));
    }
}
