//! Pure comparison of caller-supplied growth observations from two endpoints.
//! Endpoint IDs and session/barrier/phase tags are claims supplied by the collector.
//! A matching phase does not establish actual game timing, and a successful
//! comparison proves only equality of these inputs, not live LAN synchronization.
//! Collection uses the fixed read-only query below; save/load and rejoin are
//! still separate operations. Neither labels nor matching records prove a barrier.

use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Read the four GrowthUI records from an InGame context. No game state is written.
/// The caller must hold both endpoints at the intended collection boundary;
/// the before/after checks reject a turn change during this single query only.
pub const COLLECTION_QUERY: &str = include_str!("growth_sync_query.lua");

/// Envelope for the existing hotseat `turn|localPlayer|owner~unitID~...` text.
/// The LAN collector must establish independent endpoints and a real barrier.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub endpoint_id: String,
    pub session_tag: String,
    pub barrier_tag: String,
    /// Caller-defined collection boundary; this library cannot verify its timing.
    pub phase: String,
    pub observation: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct UnitKey {
    pub owner: u8,
    pub unit_id: i64,
}

impl fmt::Display for UnitKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "owner={} unitID={}", self.owner, self.unit_id)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GrowthRecord {
    pub key: UnitKey,
    pub level: u8,
    /// Finite, nonnegative integer-valued Lua number. Comparison uses exact numeric equality.
    pub xp: f64,
    pub personality: String,
    pub trait_name: String,
    /// Original comma list: neither sorting nor normalization is applied.
    pub equipment: String,
    pub artifact: String,
    pub last_xp_turn: i64,
}

/// Parsed data for callers that need to inspect a capture. Deserialization alone
/// is not validation: `compare` always reparses the original checkpoint inputs.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GrowthSnapshot {
    pub world_turn: i64,
    pub local_player: u8,
    /// Ordered by owner + unitID, independent of input row order.
    pub records: Vec<GrowthRecord>,
}

fn invalid(endpoint: &str, field: &str, detail: impl fmt::Display) -> Error {
    Error::scenario(format!(
        "growth checkpoint endpoint {endpoint:?}: {field}: {detail}"
    ))
}

fn integer(endpoint: &str, field: &str, text: &str) -> Result<i64> {
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid(
            endpoint,
            field,
            format!("invalid integer {text:?}"),
        ));
    }
    text.parse()
        .map_err(|_| invalid(endpoint, field, format!("integer out of range {text:?}")))
}

fn label(endpoint: &str, field: &str, value: &str, allow_empty: bool) -> Result<()> {
    if (!allow_empty && value.trim().is_empty()) || value.chars().any(char::is_control) {
        return Err(invalid(
            endpoint,
            field,
            "empty or contains control characters",
        ));
    }
    Ok(())
}

impl Checkpoint {
    /// Validate the envelope and all four records without changing the input.
    pub fn parse(&self) -> Result<GrowthSnapshot> {
        let endpoint = &self.endpoint_id;
        for (field, value) in [
            ("endpoint_id", endpoint),
            ("session_tag", &self.session_tag),
            ("barrier_tag", &self.barrier_tag),
            ("phase", &self.phase),
        ] {
            label(endpoint, field, value, false)?;
        }
        let mut rows = self.observation.split('|');
        let world_turn = integer(endpoint, "worldTurn", rows.next().unwrap_or(""))?;
        if world_turn < 0 {
            return Err(invalid(endpoint, "worldTurn", "must be nonnegative"));
        }
        let local_player = integer(endpoint, "localPlayer", rows.next().unwrap_or(""))?;
        if !(0..=1).contains(&local_player) {
            return Err(invalid(endpoint, "localPlayer", "must be 0 or 1"));
        }
        let mut records = BTreeMap::new();
        let mut counts = [0; 2];
        for (index, row) in rows.enumerate() {
            let fields: Vec<_> = row.split('~').collect();
            let location = format!("record[{}] owner/unitID", index + 1);
            if fields.len() != 9 {
                return Err(invalid(
                    endpoint,
                    &location,
                    format!("expected 9 fields, got {} in {row:?}", fields.len()),
                ));
            }
            let location = format!(
                "record[{}] owner={} unitID={}",
                index + 1,
                fields[0],
                fields[1]
            );
            let owner = integer(endpoint, &format!("{location}.owner"), fields[0])?;
            if !(0..=1).contains(&owner) {
                return Err(invalid(
                    endpoint,
                    &format!("{location}.owner"),
                    "must be 0 or 1",
                ));
            }
            let unit_id = integer(endpoint, &format!("{location}.unitID"), fields[1])?;
            if unit_id < 0 {
                return Err(invalid(
                    endpoint,
                    &format!("{location}.unitID"),
                    "must be nonnegative",
                ));
            }
            let key = UnitKey {
                owner: owner as u8,
                unit_id,
            };
            if records.contains_key(&key) {
                return Err(invalid(
                    endpoint,
                    &key.to_string(),
                    "duplicate owner/unitID key",
                ));
            }
            let level = integer(endpoint, &format!("{key}.level"), fields[2])?;
            if !(1..=30).contains(&level) {
                return Err(invalid(endpoint, &format!("{key}.level"), "must be 1..30"));
            }
            let xp = fields[3]
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite() && *n >= 0.0 && n.fract() == 0.0)
                .ok_or_else(|| {
                    invalid(
                        endpoint,
                        &format!("{key}.XP"),
                        format!("expected finite nonnegative integer, got {:?}", fields[3]),
                    )
                })?;
            for (field, value, allow_empty) in [
                ("personality", fields[4], false),
                ("trait", fields[5], false),
                ("equipment", fields[6], true),
                ("artifact", fields[7], false),
            ] {
                label(endpoint, &format!("{key}.{field}"), value, allow_empty)?;
            }
            if !fields[6].is_empty() && fields[6].split(',').any(|item| item.trim().is_empty()) {
                return Err(invalid(
                    endpoint,
                    &format!("{key}.equipment"),
                    "empty comma-list item",
                ));
            }
            let last_xp_turn = integer(endpoint, &format!("{key}.LastXPTurn"), fields[8])?;
            if last_xp_turn < -1 || last_xp_turn > world_turn {
                return Err(invalid(
                    endpoint,
                    &format!("{key}.LastXPTurn"),
                    format!("must be -1..{world_turn}, got {last_xp_turn}"),
                ));
            }
            counts[owner as usize] += 1;
            records.insert(
                key,
                GrowthRecord {
                    key,
                    level: level as u8,
                    xp,
                    personality: fields[4].into(),
                    trait_name: fields[5].into(),
                    equipment: fields[6].into(),
                    artifact: fields[7].into(),
                    last_xp_turn,
                },
            );
        }
        for (owner, count) in counts.iter().enumerate() {
            if *count != 2 {
                let keys: Vec<_> = records
                    .keys()
                    .filter(|key| key.owner as usize == owner)
                    .collect();
                return Err(invalid(
                    endpoint,
                    &format!("owner={owner}.records"),
                    format!("expected 2, got {count}; observed keys {keys:?}"),
                ));
            }
        }
        Ok(GrowthSnapshot {
            world_turn,
            local_player: local_player as u8,
            records: records.into_values().collect(),
        })
    }
}

/// Compare four growth records at a caller-specified session/barrier/phase.
/// Success describes only supplied data, not verified endpoints or live timing.
/// Different localPlayer values are required, but are not a growth difference.
pub fn compare(left: &Checkpoint, right: &Checkpoint) -> Result<()> {
    let a = left.parse()?;
    let b = right.parse()?;
    if left.endpoint_id == right.endpoint_id {
        return Err(invalid(
            &right.endpoint_id,
            "endpoint_id",
            "must differ between independent endpoints",
        ));
    }
    for (field, l, r) in [
        ("session_tag", &left.session_tag, &right.session_tag),
        ("barrier_tag", &left.barrier_tag, &right.barrier_tag),
        ("phase", &left.phase, &right.phase),
    ] {
        if l != r {
            return Err(invalid(
                &right.endpoint_id,
                field,
                format!("left={l:?}, right={r:?}"),
            ));
        }
    }
    if a.world_turn != b.world_turn {
        return Err(invalid(
            &right.endpoint_id,
            "worldTurn",
            format!("left={}, right={}", a.world_turn, b.world_turn),
        ));
    }
    if a.local_player == b.local_player {
        return Err(invalid(
            &right.endpoint_id,
            "localPlayer",
            "endpoints must operate different players 0/1",
        ));
    }
    let left_keys: Vec<_> = a.records.iter().map(|r| r.key).collect();
    let right_keys: Vec<_> = b.records.iter().map(|r| r.key).collect();
    if left_keys != right_keys {
        let missing: Vec<_> = left_keys
            .iter()
            .filter(|key| !right_keys.contains(key))
            .collect();
        let extra: Vec<_> = right_keys
            .iter()
            .filter(|key| !left_keys.contains(key))
            .collect();
        return Err(invalid(
            &right.endpoint_id,
            "owner/unitID keys",
            format!("missing on right {missing:?}; extra on right {extra:?}"),
        ));
    }
    for (l, r) in a.records.iter().zip(&b.records) {
        for (field, differs, left_value, right_value) in [
            (
                "level",
                l.level != r.level,
                l.level.to_string(),
                r.level.to_string(),
            ),
            ("XP", l.xp != r.xp, l.xp.to_string(), r.xp.to_string()),
            (
                "personality",
                l.personality != r.personality,
                l.personality.clone(),
                r.personality.clone(),
            ),
            (
                "trait",
                l.trait_name != r.trait_name,
                l.trait_name.clone(),
                r.trait_name.clone(),
            ),
            (
                "equipment",
                l.equipment != r.equipment,
                l.equipment.clone(),
                r.equipment.clone(),
            ),
            (
                "artifact",
                l.artifact != r.artifact,
                l.artifact.clone(),
                r.artifact.clone(),
            ),
            (
                "LastXPTurn",
                l.last_xp_turn != r.last_xp_turn,
                l.last_xp_turn.to_string(),
                r.last_xp_turn.to_string(),
            ),
        ] {
            if differs {
                return Err(invalid(
                    &right.endpoint_id,
                    &format!("{}.{}", l.key, field),
                    format!("left={left_value:?}, right={right_value:?}"),
                ));
            }
        }
    }
    Ok(())
}
