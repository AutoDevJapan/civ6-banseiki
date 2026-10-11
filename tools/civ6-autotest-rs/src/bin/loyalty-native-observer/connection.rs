//! One-call connection witness. No game commands or game-memory writes.
//! City/CacheCity are separate Lua types. A matching owner/ID is required,
//! never a matching loyalty number. A data breakpoint records an AFTER-write
//! RIP; it does not identify the beginning of that instruction or its role.

use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;

type Result<T> = std::result::Result<T, String>;

pub const GETTER_MEMBER: u64 = 0x6D8;
pub const CONSTRUCTOR_RVA: u64 = 0x710FE9;
pub const CONSTRUCTOR_FROM_GETTER: u64 = CONSTRUCTOR_RVA - 0x70AF4A;
pub const WRITE4: u64 = 0xD; // DR7: RW=01 (write), LEN=11 (4 bytes).
pub const SAMPLE_LIMIT: u32 = 128;
pub const WRITE_LIMIT: u32 = 32;
pub const GENERATION_LIMIT: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct CityKey {
    pub owner: u16,
    pub city: u32,
}

impl std::str::FromStr for CityKey {
    type Err = String;
    fn from_str(text: &str) -> Result<Self> {
        let (owner, city) = text.split_once(':').ok_or("expected OWNER:CITY")?;
        let key = Self {
            owner: owner.parse().map_err(|_| "invalid owner")?,
            city: city.parse().map_err(|_| "invalid city")?,
        };
        if key.owner >= 64 {
            return Err("owner must be in 0..64".into());
        }
        Ok(key)
    }
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub struct GetterOrigin {
    pub key: CityKey,
    pub cache_city: u64,
    pub member: u64,
    pub current_address: u64,
    pub current_i32: i32,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct PendingOrigin {
    pub origin: GetterOrigin,
    pub tid: u32,
    pub lua_state: u64,
}

pub fn getter_origin(
    mut read: impl FnMut(u64, usize) -> Result<Vec<u8>>,
    member: u64,
) -> Result<GetterOrigin> {
    let cache_city = member
        .checked_sub(GETTER_MEMBER)
        .filter(|&p| p != 0)
        .ok_or("invalid CacheCity member pointer")?;
    let current_address = member
        .checked_add(8)
        .filter(|p| p.is_multiple_of(4))
        .ok_or("invalid aligned current address")?;
    // 0x690CED..0x690D00 converts an actual CacheCity into an object key:
    // owner = u16[CacheCity+0x18], city = u32[CacheCity].
    let city = u32::from_le_bytes(field(&mut read, cache_city, 4)?.try_into().unwrap());
    let owner = u16::from_le_bytes(field(&mut read, cache_city + 0x18, 2)?.try_into().unwrap());
    let current_i32 = i32::from_le_bytes(field(&mut read, current_address, 4)?.try_into().unwrap());
    Ok(GetterOrigin {
        key: CityKey { owner, city },
        cache_city,
        member,
        current_address,
        current_i32,
    })
}

fn field(
    read: &mut impl FnMut(u64, usize) -> Result<Vec<u8>>,
    address: u64,
    length: usize,
) -> Result<Vec<u8>> {
    if address == 0 || !(1..=8).contains(&length) || address.checked_add(length as u64).is_none() {
        return Err("invalid bounded connection field".into());
    }
    let bytes = read(address, length)?;
    if bytes.len() != length {
        return Err("short connection field".into());
    }
    Ok(bytes)
}

fn u64_field(read: &mut impl FnMut(u64, usize) -> Result<Vec<u8>>, address: u64) -> Result<u64> {
    Ok(u64::from_le_bytes(
        field(read, address, 8)?.try_into().unwrap(),
    ))
}

// The helper receives self+0x38 as a std::set-like tree container. Its +8
// field is incremented at 0x3F388. Thus self+0x40=14 is a size field, NOT a
// proven property number. Search only the two exact wrapper addresses.
pub fn registry_fields(
    mut read: impl FnMut(u64, usize) -> Result<Vec<u8>>,
    receiver: u64,
) -> Value {
    let result = (|| -> Result<Value> {
        let current_wrapper = receiver.checked_add(0x98).ok_or("wrapper overflow")?;
        let delta_wrapper = receiver.checked_add(0xB0).ok_or("wrapper overflow")?;
        let current_self = u64_field(&mut read, current_wrapper + 8)?;
        let delta_self = u64_field(&mut read, delta_wrapper + 8)?;
        if current_self == 0 || delta_self == 0 {
            return Err("null wrapper registration context".into());
        }
        let mut contexts = Vec::new();
        for context in BTreeSet::from([current_self, delta_self]) {
            let tree = context.checked_add(0x38).ok_or("tree overflow")?;
            let head = u64_field(&mut read, tree)?;
            let size = u64_field(&mut read, tree + 8)?;
            contexts.push(
                json!({"context":context,"tree":tree,"head":head,"size":size,
                "current_wrapper":tree_contains(&mut read, head, current_wrapper),
                "delta_wrapper":tree_contains(&mut read, head, delta_wrapper)}),
            );
        }
        Ok(
            json!({"current_wrapper":current_wrapper,"delta_wrapper":delta_wrapper,
            "current_self":current_self,"delta_self":delta_self,"contexts":contexts,
            "scope":"two_exact_wrapper_keys_in_bounded_tree_search"}),
        )
    })();
    match result {
        Ok(value) => value,
        Err(error) => json!({"read_error":error}),
    }
}

fn tree_contains(
    read: &mut impl FnMut(u64, usize) -> Result<Vec<u8>>,
    head: u64,
    wanted: u64,
) -> Value {
    let mut visited = BTreeSet::new();
    let result = (|| -> Result<bool> {
        if head == 0 {
            return Err("null tree head".into());
        }
        let mut node = u64_field(read, head.checked_add(8).ok_or("head overflow")?)?;
        loop {
            if node == 0 {
                return Err("null tree node".into());
            }
            if !visited.insert(node) {
                return Err("tree cycle".into());
            }
            if visited.len() > 32 {
                return Err("tree search limit".into());
            }
            if field(read, node.checked_add(0x19).ok_or("node overflow")?, 1)?[0] != 0 {
                return Ok(false);
            }
            let key = u64_field(read, node.checked_add(0x20).ok_or("node overflow")?)?;
            if key == wanted {
                return Ok(true);
            }
            node = u64_field(
                read,
                node.checked_add(if wanted < key { 0 } else { 0x10 })
                    .ok_or("link overflow")?,
            )?;
        }
    })();
    match result {
        Ok(found) => json!({"found":found,"visited":visited.len()}),
        Err(error) => json!({"read_error":error,"visited":visited.len()}),
    }
}

#[derive(Debug, Serialize)]
pub struct Connection {
    pub target: CityKey,
    pub baseline: Option<GetterOrigin>,
    pub latest: Option<GetterOrigin>,
    pub pending_origin: Option<PendingOrigin>,
    pub native_city: Option<u64>,
    pub returned: bool,
    pub getter_samples: u32,
    pub generations: u32,
    pub writes: u32,
    pub audit: Value,
}

impl Connection {
    pub fn completion(&self) -> Value {
        json!({"baseline_captured":self.baseline.is_some(),
            "same_city_setter_captured":self.native_city.is_some(),
            "setter_return_captured":self.returned,
            "capture_complete":self.baseline.is_some() && self.native_city.is_some() && self.returned,
            "scope":"capture_completeness_only_not_product_or_core_update_success"})
    }

    pub fn new(target: CityKey, audit: Value) -> Self {
        Self {
            target,
            baseline: None,
            latest: None,
            pending_origin: None,
            native_city: None,
            returned: false,
            getter_samples: 0,
            generations: 0,
            writes: 0,
            audit,
        }
    }

    pub fn constructor(&mut self, origin: GetterOrigin, tid: u32, lua_state: u64) -> Result<bool> {
        // A queued pre-baseline factory cannot leave the post-return slot
        // waiting for a getter on an old Lua state.
        if self.baseline.is_some() && !self.returned {
            return Ok(false);
        }
        self.getter_samples += 1;
        if self.getter_samples > SAMPLE_LIMIT {
            return Err("connection origin sample limit".into());
        }
        if origin.key != self.target {
            return Ok(false);
        }
        self.pending_origin = Some(PendingOrigin {
            origin,
            tid,
            lua_state,
        });
        Ok(true)
    }

    pub fn getter(&mut self, origin: GetterOrigin, tid: u32, lua_state: u64) -> Result<bool> {
        self.getter_samples += 1;
        if self.getter_samples > SAMPLE_LIMIT {
            return Err("connection getter sample limit".into());
        }
        if origin.key != self.target
            || !self.pending_origin.is_some_and(|pending| {
                pending.origin.member == origin.member
                    && pending.tid == tid
                    && pending.lua_state == lua_state
            })
        {
            return Ok(false);
        }
        if self.latest.is_none_or(|old| old.member != origin.member) {
            self.generations += 1;
            if self.generations > GENERATION_LIMIT {
                return Err("connection cache generation limit".into());
            }
        }
        self.baseline.get_or_insert(origin);
        self.latest = Some(origin);
        self.pending_origin = None;
        Ok(true)
    }

    pub fn setter(&mut self, city: u64, key: CityKey) -> Result<()> {
        if self.baseline.is_none() {
            return Err("same-city getter baseline was not captured before the one setter".into());
        }
        if key != self.target
            || self.native_city.is_some()
            || city == 0
            || !city.is_multiple_of(4)
            || city.checked_add(0x16D8).is_none()
        {
            return Err("connection first setter identity/replay mismatch".into());
        }
        self.native_city = Some(city);
        Ok(())
    }

    pub fn plan(&self, execution: [u64; 4]) -> ([u64; 4], u64) {
        let mut addresses = execution;
        let mut control = 0x55;
        let constructor = execution[2] + CONSTRUCTOR_FROM_GETTER; // inside the checked maximum DLL RVA
        if self.latest.is_none() && self.pending_origin.is_none() {
            addresses[2] = constructor;
        }
        if let Some(origin) = self.latest {
            addresses[2] = origin.current_address;
            control |= WRITE4 << 24;
        }
        if let Some(city) = self.native_city {
            addresses[0] = city + 0x16D8;
            control |= WRITE4 << 16;
        }
        if self.returned {
            addresses[3] = if self.pending_origin.is_some() {
                execution[2]
            } else {
                constructor
            };
        }
        (addresses, control)
    }

    pub fn wrote(&mut self) -> Result<()> {
        self.writes += 1;
        if self.writes > WRITE_LIMIT {
            return Err("connection write sample limit".into());
        }
        Ok(())
    }
}

// Byte witnesses, never names of a nearby export. Relative destinations are
// decoded independently so a copied displacement cannot certify a wrong edge.
pub const WITNESSES: &[(u32, &[u8])] = &[
    (0x713E15, &[0xE8, 0x26, 0x12, 0x91, 0xFF]),
    (0x713E22, &[0x48, 0x8B, 0xD8]),
    (0x713E2D, &[0x48, 0x8D, 0x8B, 0x30, 0x16, 0, 0]),
    (0x713E3D, &[0xE8, 0xCE, 0x13, 0xA0, 0xFF]),
    (0x11521A, &[0x48, 0x8B, 0xD9]),
    (0x11531A, &[0x48, 0x8B, 0x83, 0x90, 0, 0, 0]),
    (0x115332, &[0x8B, 0x88, 0xA8, 0, 0, 0]),
    (0x115338, &[0x0F, 0xB7, 0x80, 0xD8, 0, 0, 0]),
    (0x02506A, &[0x4C, 0x8D, 0x05, 0x77, 0x86, 0x99, 0]),
    (0x710FE1, &[0xE8, 0x0A, 0x3F, 0x91, 0xFF]),
    (0x024F1A, &[0x4C, 0x8D, 0x05, 0xE7, 0x87, 0x99, 0]),
    (0x710FE9, &[0x48, 0x8D, 0x90, 0xD8, 0x06, 0, 0]),
    (0x710FF0, &[0xE8, 0x8B, 0xF9, 0xFF, 0xFF]),
    (0x71098F, &[0x48, 0x8B, 0xF2]),
    (0x710C55, &[0x48, 0x89, 0x70, 0x08]),
    (0x70AF41, &[0xE8, 0x1A, 0xCF, 0xFF, 0xFF]),
    (0x707E8A, &[0x4C, 0x8D, 0x05, 0x97, 0x4E, 0x33, 0]),
    (0x70AF4A, &[0x8B, 0x50, 0x08]),
    (0x690CE8, &[0xE8, 0x03, 0x42, 0x99, 0xFF]),
    (0x690CF2, &[0x0F, 0xB7, 0x48, 0x18, 0x8B, 0]),
    (0x115243, &[0x48, 0x8D, 0x8B, 0x98, 0, 0, 0]),
    (0x11525B, &[0x48, 0x8D, 0x8B, 0xB0, 0, 0, 0]),
    (0x2CA10D, &[0x48, 0x8B, 0x41, 0x08]),
    (0x2CA11A, &[0x4C, 0x8D, 0x58, 0x38]),
    (0x2CA1FB, &[0xE8, 0xF0, 0x50, 0xD7, 0xFF]),
    (0x2CA200, &[0x8B, 0x03, 0x89, 0x07]),
    (0x3F31C, &[0x48, 0x39, 0x41, 0x08]),
    (0x3F388, &[0x48, 0xFF, 0x43, 0x08]),
    (
        0x3F37D,
        &[
            0x48, 0x8B, 0x44, 0x24, 0x50, 0x48, 0x8B, 0x08, 0x49, 0x89, 0x08,
        ],
    ),
    (0x64A763, &[0x8B, 0x07, 0x89, 0x43, 0x10]),
    (0x64A7E4, &[0x8B, 0x02, 0x89, 0x43, 0x10]),
    (
        0x64A7F4,
        &[0x8B, 0x02, 0x48, 0x8B, 0x5C, 0x24, 0x50, 0x89, 0x41, 0x10],
    ),
];

fn rva(bytes: &[u8], address: u32, length: usize) -> Result<&[u8]> {
    fn u16_at(bytes: &[u8], off: usize) -> Result<u16> {
        Ok(u16::from_le_bytes(
            bytes
                .get(off..off + 2)
                .ok_or("short PE")?
                .try_into()
                .unwrap(),
        ))
    }
    fn u32_at(bytes: &[u8], off: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(
            bytes
                .get(off..off + 4)
                .ok_or("short PE")?
                .try_into()
                .unwrap(),
        ))
    }
    if u16_at(bytes, 0)? != 0x5A4D {
        return Err("not PE".into());
    }
    let pe = u32_at(bytes, 0x3C)? as usize;
    if u32_at(bytes, pe)? != 0x4550 || u16_at(bytes, pe + 4)? != 0x8664 {
        return Err("not x64 PE".into());
    }
    let sections = u16_at(bytes, pe + 6)? as usize;
    let opt_size = u16_at(bytes, pe + 20)? as usize;
    let start = pe.checked_add(24 + opt_size).ok_or("PE section overflow")?;
    let mut mapped = None;
    for index in 0..sections {
        let off = start.checked_add(index * 40).ok_or("section overflow")?;
        let va = u32_at(bytes, off + 12)?;
        let size = u32_at(bytes, off + 16)?;
        if let Some(delta) = address.checked_sub(va) {
            if u64::from(delta) + length as u64 <= u64::from(size) {
                let raw = u32_at(bytes, off + 20)? as usize;
                let offset = raw.checked_add(delta as usize).ok_or("raw overflow")?;
                let data = bytes
                    .get(offset..offset.checked_add(length).ok_or("range overflow")?)
                    .ok_or("short mapped PE")?;
                if mapped.replace(data).is_some() {
                    return Err("ambiguous RVA mapping".into());
                }
            }
        }
    }
    mapped.ok_or_else(|| "RVA is outside file-backed sections".into())
}

fn relative(bytes: &[u8], from: u32, prefix: &[u8], target: u32) -> Result<()> {
    let code = rva(bytes, from, prefix.len() + 4)?;
    if !code.starts_with(prefix) {
        return Err("relative instruction mismatch".into());
    }
    let delta = i32::from_le_bytes(code[prefix.len()..].try_into().unwrap());
    if i64::from(from) + (prefix.len() + 4) as i64 + i64::from(delta) != i64::from(target) {
        return Err("relative destination mismatch".into());
    }
    Ok(())
}

pub fn audit(bytes: &[u8], expected_sha: &str) -> Result<Value> {
    use sha2::{Digest, Sha256};
    if format!("{:X}", Sha256::digest(bytes)) != expected_sha {
        return Err("connection DLL SHA256 mismatch".into());
    }
    for &(address, witness) in WITNESSES {
        if rva(bytes, address, witness.len())? != witness {
            return Err(format!("connection byte mismatch at RVA {address:X}"));
        }
    }
    for (from, prefix, target) in [
        (0x713E15, &[0xE8][..], 0x025040),
        (0x713E3D, &[0xE8][..], 0x115210),
        (0x710FE1, &[0xE8][..], 0x024EF0),
        (0x710FF0, &[0xE8][..], 0x710980),
        (0x70AF41, &[0xE8][..], 0x707E60),
        (0x690CE8, &[0xE8][..], 0x024EF0),
        (0x2CA1FB, &[0xE8][..], 0x03F2F0),
        (0x02506A, &[0x4C, 0x8D, 0x05][..], 0x9BD6E8),
        (0x024F1A, &[0x4C, 0x8D, 0x05][..], 0x9BD708),
        (0x707E8A, &[0x4C, 0x8D, 0x05][..], 0xA3CD28),
    ] {
        relative(bytes, from, prefix, target)?;
    }
    for (address, text) in [
        (0x9BD6E8, &b"City\0"[..]),
        (0x9BD708, &b"CacheCity\0"[..]),
        (0xA3CD28, &b"CacheCityCulturalIdentity\0"[..]),
    ] {
        if rva(bytes, address, text.len())? != text {
            return Err("Lua type string mismatch".into());
        }
    }
    Ok(
        json!({"sha256":expected_sha,"witness_count":WITNESSES.len(),
        "getter_source":"CacheCity+0x6D8; GetLoyalty reads member+8",
        "city_key_source":"0x690CF2 CacheCity owner+0x18 and city+0",
        "setter_source":"City+0x1630; current wrapper+0x98, delta wrapper+0xB0",
        "registration":"wrapper+8 -> self+0x38 tree; self+0x40 is the incremented tree size",
        "limitations":["authoritative writer and cache publication route require runtime evidence",
            "notification candidate is not proof of the matching UI event"]}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn mapped_byte_audit_rejects_different_targets_and_unbacked_ranges() {
        let mut bytes = vec![0; 0xA40000];
        bytes[..2].copy_from_slice(&0x5A4Du16.to_le_bytes());
        bytes[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        bytes[0x80..0x84].copy_from_slice(&0x4550u32.to_le_bytes());
        bytes[0x84..0x86].copy_from_slice(&0x8664u16.to_le_bytes());
        bytes[0x86..0x88].copy_from_slice(&1u16.to_le_bytes());
        bytes[0x94..0x96].copy_from_slice(&240u16.to_le_bytes());
        bytes[0x194..0x198].copy_from_slice(&0x1000u32.to_le_bytes());
        bytes[0x198..0x19C].copy_from_slice(&0xA3F000u32.to_le_bytes());
        bytes[0x19C..0x1A0].copy_from_slice(&0x400u32.to_le_bytes());
        for &(address, witness) in WITNESSES {
            let off = address as usize - 0xC00;
            bytes[off..off + witness.len()].copy_from_slice(witness);
        }
        for (address, text) in [
            (0x9BD6E8, &b"City\0"[..]),
            (0x9BD708, &b"CacheCity\0"[..]),
            (0xA3CD28, &b"CacheCityCulturalIdentity\0"[..]),
        ] {
            let off = address as usize - 0xC00;
            bytes[off..off + text.len()].copy_from_slice(text);
        }
        use sha2::{Digest, Sha256};
        let sha = format!("{:X}", Sha256::digest(&bytes));
        audit(&bytes, &sha).unwrap();
        let mut wrong_identity = bytes.clone();
        wrong_identity[0x115338 - 0xC00 + 3] = 0xDC;
        let wrong_sha = format!("{:X}", Sha256::digest(&wrong_identity));
        assert!(audit(&wrong_identity, &wrong_sha).is_err());
        relative(&bytes, 0x2CA1FB, &[0xE8], 0x03F2F0).unwrap(); // signed negative edge
        assert!(relative(&bytes, 0x2CA1FB, &[0xE8], 0x03F2F1).is_err());
        assert!(rva(&bytes, 0xA3FFFF, 2).is_err());
        assert!(rva(&bytes[..0x200], 0x70AF4A, 3).is_err());
        bytes[0x70AF4A - 0xC00 + 2] = 0x10; // a different getter field, with a new valid hash
        let altered_sha = format!("{:X}", Sha256::digest(&bytes));
        assert!(audit(&bytes, &altered_sha).is_err());
    }

    #[test]
    fn getter_origin_uses_city_key_not_equal_values_and_rejects_bad_reads() {
        let mut fields = BTreeMap::from([
            (0x1000, vec![0, 0, 1, 0]),
            (0x1018, vec![3, 0]),
            (0x16E0, 25600_i32.to_le_bytes().to_vec()),
        ]);
        let mut read = |address, len| {
            fields
                .get(&address)
                .filter(|v| v.len() == len)
                .cloned()
                .ok_or("missing".into())
        };
        let origin = getter_origin(&mut read, 0x16D8).unwrap();
        let mut state = Connection::new("3:65536".parse().unwrap(), Value::Null);
        assert_eq!(state.completion()["capture_complete"], false);
        assert!(!state
            .getter(
                GetterOrigin {
                    key: CityKey {
                        owner: 0,
                        city: 65536
                    },
                    ..origin
                },
                7,
                0x99
            )
            .unwrap());
        assert!(state.baseline.is_none());
        assert!(!state.getter(origin, 7, 0x99).unwrap());
        assert!(state.constructor(origin, 7, 0x99).unwrap());
        assert!(!state.getter(origin, 8, 0x99).unwrap());
        assert!(!state.getter(origin, 7, 0x98).unwrap());
        assert!(state.getter(origin, 7, 0x99).unwrap());
        assert!(state.setter(0x2000, origin.key).is_ok());
        assert!(state.setter(0x2000, origin.key).is_err());
        fields.remove(&0x1018);
        assert!(getter_origin(
            |a, _| fields.get(&a).cloned().ok_or("missing".into()),
            0x16D8
        )
        .is_err());
        assert!(getter_origin(|_, _| Ok(vec![]), 0x16D8).is_err());
        assert!(getter_origin(|_, _| Ok(vec![]), GETTER_MEMBER).is_err());
    }

    #[test]
    fn four_slots_keep_return_and_candidate_while_watching_original_call() {
        let execution = [0x100, 0x200, 0x300, 0x400];
        let mut state = Connection::new("3:65536".parse().unwrap(), Value::Null);
        assert_eq!(
            state.plan(execution),
            ([0x100, 0x200, 0x300 + CONSTRUCTOR_FROM_GETTER, 0x400], 0x55)
        );
        let origin = GetterOrigin {
            key: state.target,
            cache_city: 0x1000,
            member: 0x16D8,
            current_address: 0x16E0,
            current_i32: 25600,
        };
        state.constructor(origin, 7, 0x99).unwrap();
        assert_eq!(state.plan(execution), (execution, 0x55));
        state.getter(origin, 7, 0x99).unwrap();
        assert!(!state.constructor(origin, 8, 0x88).unwrap());
        assert!(state.pending_origin.is_none());
        assert_eq!(
            state.plan(execution),
            ([0x100, 0x200, 0x16E0, 0x400], 0xD000055)
        );
        state.setter(0x2000, state.target).unwrap();
        assert_eq!(state.completion()["capture_complete"], false);
        assert_eq!(
            state.plan(execution),
            ([0x36D8, 0x200, 0x16E0, 0x400], 0xD0D0055)
        );
        state.returned = true;
        assert_eq!(state.completion()["capture_complete"], true);
        assert_eq!(
            state.plan(execution).0,
            [0x36D8, 0x200, 0x16E0, 0x300 + CONSTRUCTOR_FROM_GETTER]
        );
        for _ in 0..WRITE_LIMIT {
            state.wrote().unwrap();
        }
        assert!(state.wrote().is_err());
        assert!(Connection::new(state.target, Value::Null)
            .setter(0x2000, state.target)
            .is_err());
    }

    #[test]
    fn registry_search_distinguishes_membership_absence_cycle_and_size() {
        let receiver = 0x1000u64;
        let mut fields = BTreeMap::from([
            (0x10A0, receiver.to_le_bytes().to_vec()),
            (0x10B8, receiver.to_le_bytes().to_vec()),
            (0x1038, 0x2000u64.to_le_bytes().to_vec()),
            (0x1040, 14u64.to_le_bytes().to_vec()),
            (0x2008, 0x3000u64.to_le_bytes().to_vec()),
            (0x3019, vec![0]),
            (0x3020, 0x1098u64.to_le_bytes().to_vec()),
            (0x3010, 0x2000u64.to_le_bytes().to_vec()),
            (0x2019, vec![1]),
        ]);
        let read = |a, _| fields.get(&a).cloned().ok_or("missing".into());
        let value = registry_fields(read, receiver);
        assert_eq!(value["contexts"][0]["size"], 14);
        assert_eq!(value["contexts"][0]["current_wrapper"]["found"], true);
        assert_eq!(value["contexts"][0]["delta_wrapper"]["found"], false);
        fields.insert(0x3010, 0x3000u64.to_le_bytes().to_vec());
        let value = registry_fields(
            |a, _| fields.get(&a).cloned().ok_or("missing".into()),
            receiver,
        );
        assert_eq!(
            value["contexts"][0]["delta_wrapper"]["read_error"],
            "tree cycle"
        );
        assert!(value["contexts"][0]["delta_wrapper"].get("found").is_none());
    }

    #[test]
    fn pinned_file_witnesses_and_wrong_hash_are_checked_without_process_access() {
        let Some(path) = std::env::var_os("CIV6_CONNECTION_AUDIT_DLL") else {
            return;
        };
        let bytes = std::fs::read(path).unwrap();
        audit(&bytes, super::super::policy::MODULE_SHA256).unwrap();
        assert!(audit(&bytes, &"0".repeat(64)).is_err());
    }
}
