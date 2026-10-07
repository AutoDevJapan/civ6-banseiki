//! Portable DTO/trace framing, not game or lifecycle proof.
use serde_json::{json, Value};
use std::path::PathBuf;
pub const HEADER: usize = 72;
pub const ROW: usize = 104;
pub const CAPACITY: usize = 128;
pub const TRACE_BYTES: usize = HEADER + ROW * CAPACITY;
pub struct Request {
    pub pid: u32,
    pub created: u64,
    pub seconds: u32,
    pub runtime: PathBuf,
    pub runtime_sha: String,
    pub bootstrap: PathBuf,
    pub bootstrap_sha: String,
    pub output: PathBuf,
}
pub fn digest(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("SHA256 must have 64 hex characters".into());
    }
    let mut b = [0; 32];
    for (i, v) in b.iter_mut().enumerate() {
        *v = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).map_err(|_| "SHA256")?;
    }
    Ok(b)
}
impl Request {
    pub fn parse(v: &Value) -> Result<Self, String> {
        let map = v.as_object().ok_or("request object required")?;
        let fields = [
            "schema",
            "pid",
            "created_filetime",
            "max_seconds",
            "runtime_dll",
            "runtime_sha256",
            "bootstrap_dll",
            "bootstrap_sha256",
            "output",
        ];
        if map.len() != fields.len()
            || map.keys().any(|k| !fields.contains(&k.as_str()))
            || v["schema"] != "todo880-bootstrap-controller-v2"
        {
            return Err("bootstrap request schema/fields".into());
        }
        let num = |k: &str| {
            v[k].as_u64()
                .filter(|n| *n > 0)
                .ok_or_else(|| format!("invalid {k}"))
        };
        let file = |k: &str| -> Result<PathBuf, String> {
            let p = PathBuf::from(v[k].as_str().ok_or(k)?);
            if !p.is_absolute() {
                return Err(format!("{k} must be absolute"));
            }
            Ok(p)
        };
        let sha = |k: &str| -> Result<String, String> {
            let s = v[k].as_str().ok_or(k)?;
            digest(s)?;
            Ok(s.to_ascii_lowercase())
        };
        let seconds = u32::try_from(num("max_seconds")?).map_err(|_| "seconds overflow")?;
        if seconds > 300 {
            return Err("max_seconds must be 1..300".into());
        }
        Ok(Self {
            pid: u32::try_from(num("pid")?).map_err(|_| "PID overflow")?,
            created: num("created_filetime")?,
            seconds,
            runtime: file("runtime_dll")?,
            runtime_sha: sha("runtime_sha256")?,
            bootstrap: file("bootstrap_dll")?,
            bootstrap_sha: sha("bootstrap_sha256")?,
            output: file("output")?,
        })
    }
}
pub fn word(b: &[u8], offset: usize) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        b.get(offset..offset + 8)
            .ok_or("trace range")?
            .try_into()
            .unwrap(),
    ))
}
pub fn trace(b: &[u8], pid: u32, created: u64) -> Result<Value, String> {
    if b.len() != TRACE_BYTES
        || word(b, 0)? != 2
        || word(b, 8)? != CAPACITY as u64
        || word(b, 16)? != pid as u64
        || word(b, 24)? != created
        || word(b, 64)? != 0
    {
        return Err("trace version/identity/capacity/drop mismatch".into());
    }
    let count = usize::try_from(word(b, 56)?).map_err(|_| "trace count")?;
    if count == 0 || count > CAPACITY {
        return Err("trace empty or exhausted".into());
    }
    let fields = [
        "sequence",
        "kind",
        "tid",
        "receiver",
        "old",
        "incoming",
        "caller",
        "generation",
        "status",
        "calibration_next",
        "calibration_in_flight",
        "property_next",
        "property_in_flight",
    ];
    let mut rows = Vec::new();
    let mut prepared = false;
    let mut installed = false;
    for i in 0..count {
        let p = HEADER + i * ROW;
        if word(b, p)? != i as u64 + 1 {
            return Err("uncommitted/out-of-order trace row".into());
        }
        let kind = word(b, p + 8)?;
        if kind > 15 || kind == 14 {
            return Err("unknown trace kind".into());
        }
        if kind == 11 {
            prepared = true;
        }
        if [1, 3, 7].contains(&kind) && word(b, p + 64)? != 0 {
            return Err("runtime API failure".into());
        }
        if kind == 1 {
            installed = true;
        }
        if installed && (9..13).any(|j| word(b, p + j * 8).ok() == Some(u64::MAX)) {
            return Err("runtime checkpoint unknown".into());
        }
        let mut row = json!({});
        for (j, name) in fields.iter().enumerate() {
            row[*name] = json!(word(b, p + j * 8)?);
        }
        rows.push(row);
    }
    if !prepared || !installed || word(b, 32)? == 0 || word(b, 40)? == 0 || word(b, 48)? == 0 {
        return Err("prepared-only is not an installed observation".into());
    }
    Ok(
        json!({"schema":"todo880-bootstrap-trace-v2","pid":pid,"created_filetime":created,"core":word(b,32)?,"runtime_trace":word(b,40)?,"runtime_trace_size":word(b,48)?,"rows":rows,"frame_valid":true,"boundary_order_proven":false,"product_acceptance":false}),
    )
}
/// Offset of the bounded PropertyTrace inside the public CompositeTrace.
pub const COMPOSITE_PROPERTY: usize = 24 + 1024 * 72;
const PROPERTY_PREFIX_BYTES: usize = 32 + 4096 * 48;
/// Prefix PropertyTrace version meaning "rows are in the runtime property log".
/// Version 3 parsers reject it, so an empty prefix never passes alone.
pub const PROPERTY_EXTERNAL_VERSION: u32 = 4;
/// Runtime-owned raw property log, allocated once at runtime installation.
pub const PROPERTY_LOG_MAGIC: u32 = 0x4a41_434c; // JACL
pub const PROPERTY_LOG_VERSION: u32 = 1;
pub const PROPERTY_LOG_HEADER: usize = 80;
pub const PROPERTY_LOG_STRIDE: usize = 48;
/// Finite diagnostic budget: 2^20 rows (48 MiB), 8.8x the 119,451 accepted
/// property calls of live run 175. Overflow is counted and fails validation.
pub const PROPERTY_LOG_CAPACITY: usize = 1 << 20;
pub const PROPERTY_LOG_BYTES: usize =
    PROPERTY_LOG_HEADER + PROPERTY_LOG_CAPACITY * PROPERTY_LOG_STRIDE;
// Header: magic u32, version u32, then u64 capacity, stride, total bytes,
// PID, creation FILETIME, next, dropped, in_flight, filtered.
pub const PROPERTY_LOG_NEXT: usize = 48;
pub const PROPERTY_LOG_DROPPED: usize = 56;
pub const PROPERTY_LOG_IN_FLIGHT: usize = 64;
pub const PROPERTY_LOG_FILTERED: usize = 72;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PropertyLog {
    pub rows: u64,
    pub filtered: u64,
}

fn u32_at(b: &[u8], offset: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        b.get(offset..offset + 4)
            .ok_or("trace range")?
            .try_into()
            .unwrap(),
    ))
}

/// Complete runtime property log: exact identity and capacity, every reserved
/// row published, the untouched tail still zero, no drop and no writer left.
pub fn property_log(b: &[u8], pid: u32, created: u64) -> Result<PropertyLog, String> {
    if b.len() != PROPERTY_LOG_BYTES
        || u32_at(b, 0)? != PROPERTY_LOG_MAGIC
        || u32_at(b, 4)? != PROPERTY_LOG_VERSION
        || word(b, 8)? != PROPERTY_LOG_CAPACITY as u64
        || word(b, 16)? != PROPERTY_LOG_STRIDE as u64
        || word(b, 24)? != PROPERTY_LOG_BYTES as u64
    {
        return Err("property log version/capacity/stride/size mismatch".into());
    }
    if word(b, 32)? != u64::from(pid) || word(b, 40)? != created {
        return Err("property log PID/FILETIME differs".into());
    }
    let next = word(b, PROPERTY_LOG_NEXT)?;
    let dropped = word(b, PROPERTY_LOG_DROPPED)?;
    let in_flight = word(b, PROPERTY_LOG_IN_FLIGHT)?;
    if dropped != 0 || in_flight != 0 || next > PROPERTY_LOG_CAPACITY as u64 {
        return Err(format!(
            "property log incomplete: next={next} dropped={dropped} in_flight={in_flight}"
        ));
    }
    let end = PROPERTY_LOG_HEADER + next as usize * PROPERTY_LOG_STRIDE;
    if (PROPERTY_LOG_HEADER..end)
        .step_by(PROPERTY_LOG_STRIDE)
        .any(|row| b[row..row + 4] != 1u32.to_le_bytes())
    {
        return Err("property log row unpublished".into());
    }
    if b[end..].iter().any(|byte| *byte != 0) {
        return Err("property log has bytes beyond its published rows".into());
    }
    Ok(PropertyLog {
        rows: next,
        filtered: word(b, PROPERTY_LOG_FILTERED)?,
    })
}

/// Runtime-owned property observation: the prefix must carry the external
/// mark and stay empty, and the separately captured log must be complete.
pub fn runtime_property(
    composite: &[u8],
    log: &[u8],
    pid: u32,
    created: u64,
) -> Result<PropertyLog, String> {
    let prefix = composite
        .get(COMPOSITE_PROPERTY..COMPOSITE_PROPERTY + PROPERTY_PREFIX_BYTES)
        .ok_or("native trace truncated")?;
    if u32_at(prefix, 0)? != 0x4a414350
        || u32_at(prefix, 4)? != PROPERTY_EXTERNAL_VERSION
        || u32_at(prefix, 24)? != 6
        || u32_at(prefix, 28)? != PROPERTY_LOG_MAGIC
    {
        return Err("property prefix lacks the runtime log mark".into());
    }
    if prefix[8..24].iter().chain(&prefix[32..]).any(|b| *b != 0) {
        return Err("runtime property prefix was written".into());
    }
    property_log(log, pid, created)
}

/// Public CompositeTrace framing only. Semantic payment/publication checks
/// remain with the existing Mode6 validator; dropped/in-flight rows fail here.
/// A version 3 prefix keeps its bounded rows; a runtime prefix requires `log`.
pub fn native_trace(b: &[u8], log: Option<&[u8]>, pid: u32, created: u64) -> Result<(), String> {
    const PROPERTY: usize = COMPOSITE_PROPERTY;
    const ASSIGN: usize = PROPERTY + PROPERTY_PREFIX_BYTES;
    let external = u32_at(b, PROPERTY + 4).ok() == Some(PROPERTY_EXTERNAL_VERSION);
    if external {
        runtime_property(b, log.ok_or("runtime property log missing")?, pid, created)?;
    } else if log.is_some() {
        return Err("property log supplied for a bounded prefix".into());
    }
    let tail = b
        .len()
        .checked_sub(ASSIGN + 24 + 1024 * 8)
        .ok_or("native trace truncated")?;
    if !tail.is_multiple_of(1024) {
        return Err("assignment stride differs".into());
    }
    let stride = tail / 1024;
    if !(96..=8192).contains(&stride) || !stride.is_multiple_of(8) {
        return Err("assignment row layout unknown".into());
    }
    for (offset, magic, version, capacity, header, row_size) in [
        (0, 0x4a414341u32, 4u32, 1024usize, 24, 72),
        (PROPERTY, 0x4a414350, 3, 4096, 32, 48),
        (ASSIGN, 0x4a414353, 2, 1024, 24, stride),
    ] {
        if external && offset == PROPERTY {
            continue;
        }
        let u = |p: usize| u32_at(b, p);
        if u(offset)? != magic
            || u(offset + 4)? != version
            || u(offset + 12)? != 0
            || u(offset + 16)? != 0
        {
            return Err(format!(
                "native trace header at {offset:#x}: magic={:#x} version={} dropped={} in_flight={}",
                u(offset)?, u(offset + 4)?, u(offset + 12)?, u(offset + 16)?
            ));
        }
        let count = u(offset + 8)? as usize;
        if count > capacity {
            return Err("native trace exhausted".into());
        }
        for i in 0..count {
            if u(offset + header + i * row_size)? != 1 {
                return Err("native row uncommitted".into());
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_request_requires_both_explicit_dll_hashes() {
        let p = std::env::temp_dir();
        let v = json!({"schema":"todo880-bootstrap-controller-v2","pid":1,"created_filetime":2,"max_seconds":30,"runtime_dll":p.join("r.dll"),"runtime_sha256":"a".repeat(64),"bootstrap_dll":p.join("b.dll"),"bootstrap_sha256":"b".repeat(64),"output":p.join("proof.jsonl")});
        assert!(Request::parse(&v).is_ok());
        for field in ["runtime_sha256", "bootstrap_sha256", "bootstrap_dll"] {
            let mut bad = v.clone();
            bad.as_object_mut().unwrap().remove(field);
            assert!(Request::parse(&bad).is_err());
        }
    }
    #[test]
    fn trace_unknown_dropped_or_partial_is_not_success() {
        let mut b = vec![0; TRACE_BYTES];
        let put = |b: &mut [u8], o: usize, n: u64| b[o..o + 8].copy_from_slice(&n.to_le_bytes());
        for (o, n) in [
            (0, 2),
            (8, 128),
            (16, 1),
            (24, 2),
            (32, 3),
            (40, 4),
            (48, 8),
            (56, 2),
            (HEADER, 1),
            (HEADER + 8, 11),
            (HEADER + ROW, 2),
            (HEADER + ROW + 8, 1),
        ] {
            put(&mut b, o, n);
        }
        assert!(trace(&b, 1, 2).is_ok());
        for (o, n) in [
            (64, 1),
            (HEADER + ROW, 0),
            (HEADER + ROW + 72, u64::MAX),
            (HEADER + ROW + 64, 1),
            (HEADER + ROW + 8, 12),
        ] {
            let mut bad = b.clone();
            put(&mut bad, o, n);
            assert!(trace(&bad, 1, 2).is_err());
        }
    }
    #[test]
    fn native_trace_dropped_inflight_or_uncommitted_is_rejected() {
        const PROPERTY: usize = 24 + 1024 * 72;
        const ASSIGN: usize = PROPERTY + 32 + 4096 * 48;
        let mut b = vec![0; ASSIGN + 24 + 1024 * 96 + 1024 * 8];
        let put = |b: &mut [u8], o: usize, n: u32| b[o..o + 4].copy_from_slice(&n.to_le_bytes());
        for (o, m, v) in [
            (0, 0x4a414341, 4),
            (PROPERTY, 0x4a414350, 3),
            (ASSIGN, 0x4a414353, 2),
        ] {
            put(&mut b, o, m);
            put(&mut b, o + 4, v);
        }
        assert!(native_trace(&b, None, 1, 2).is_ok());
        for offset in [
            12,
            16,
            PROPERTY + 12,
            PROPERTY + 16,
            ASSIGN + 12,
            ASSIGN + 16,
            8,
        ] {
            let mut bad = b.clone();
            put(&mut bad, offset, 1);
            assert!(native_trace(&bad, None, 1, 2).is_err());
        }
        // A bounded version 3 prefix never borrows a runtime log.
        assert!(native_trace(&b, Some(&[]), 1, 2).is_err());
    }

    fn runtime_pair() -> (Vec<u8>, Vec<u8>) {
        const ASSIGN: usize = COMPOSITE_PROPERTY + PROPERTY_PREFIX_BYTES;
        let mut b = vec![0; ASSIGN + 24 + 1024 * 96 + 1024 * 8];
        let put = |b: &mut [u8], o: usize, n: u32| b[o..o + 4].copy_from_slice(&n.to_le_bytes());
        for (o, m, v) in [
            (0, 0x4a414341, 4),
            (COMPOSITE_PROPERTY, 0x4a414350, PROPERTY_EXTERNAL_VERSION),
            (ASSIGN, 0x4a414353, 2),
        ] {
            put(&mut b, o, m);
            put(&mut b, o + 4, v);
        }
        put(&mut b, COMPOSITE_PROPERTY + 24, 6);
        put(&mut b, COMPOSITE_PROPERTY + 28, PROPERTY_LOG_MAGIC);
        let mut log = vec![0; PROPERTY_LOG_BYTES];
        let words = [
            PROPERTY_LOG_CAPACITY as u64,
            PROPERTY_LOG_STRIDE as u64,
            PROPERTY_LOG_BYTES as u64,
            1,
            2,
            5000,
            0,
            0,
            7,
        ];
        put(&mut log, 0, PROPERTY_LOG_MAGIC);
        put(&mut log, 4, PROPERTY_LOG_VERSION);
        for (i, w) in words.into_iter().enumerate() {
            log[8 + i * 8..16 + i * 8].copy_from_slice(&w.to_le_bytes());
        }
        for row in 0..5000 {
            let p = PROPERTY_LOG_HEADER + row * PROPERTY_LOG_STRIDE;
            put(&mut log, p, 1);
            put(&mut log, p + 4, 905_256 + row as u32);
        }
        (b, log)
    }

    #[test]
    fn runtime_prefix_requires_complete_matching_property_log() {
        let (b, log) = runtime_pair();
        assert_eq!(
            runtime_property(&b, &log, 1, 2),
            Ok(PropertyLog {
                rows: 5000,
                filtered: 7
            })
        );
        assert!(native_trace(&b, Some(&log), 1, 2).is_ok());
        assert!(native_trace(&b, None, 1, 2).is_err(), "empty prefix alone");
        assert!(native_trace(&b, Some(&log), 9, 2).is_err(), "other PID");
        assert!(
            native_trace(&b, Some(&log), 1, 3).is_err(),
            "other FILETIME"
        );
        assert!(native_trace(&b, Some(&log[..log.len() - 1]), 1, 2).is_err());
        let word = |o: usize, n: u64| {
            let mut bad = log.clone();
            bad[o..o + 8].copy_from_slice(&n.to_le_bytes());
            bad
        };
        for bad in [
            word(8, 4096),
            word(16, 40),
            word(PROPERTY_LOG_NEXT, 5001),
            word(PROPERTY_LOG_NEXT, PROPERTY_LOG_CAPACITY as u64 + 1),
            word(PROPERTY_LOG_DROPPED, 1),
            word(PROPERTY_LOG_IN_FLIGHT, 1),
            word(PROPERTY_LOG_HEADER + 4999 * PROPERTY_LOG_STRIDE, 0),
            word(PROPERTY_LOG_HEADER + 5000 * PROPERTY_LOG_STRIDE + 40, 1),
        ] {
            assert!(native_trace(&b, Some(&bad), 1, 2).is_err());
        }
        // Accepted calls are counted rows plus drops; overflow never passes.
        let mut full = word(PROPERTY_LOG_NEXT, PROPERTY_LOG_CAPACITY as u64);
        for row in 5000..PROPERTY_LOG_CAPACITY {
            full[PROPERTY_LOG_HEADER + row * PROPERTY_LOG_STRIDE] = 1;
        }
        assert_eq!(property_log(&full, 1, 2).unwrap().rows, 1 << 20);
        full[PROPERTY_LOG_DROPPED] = 1;
        assert!(property_log(&full, 1, 2).unwrap_err().contains("dropped=1"));
        for (offset, value) in [
            (COMPOSITE_PROPERTY + 4, 3),
            (COMPOSITE_PROPERTY + 8, 1),
            (COMPOSITE_PROPERTY + 20, 1),
            (COMPOSITE_PROPERTY + 24, 5),
            (COMPOSITE_PROPERTY + 28, 0),
            (COMPOSITE_PROPERTY + 32, 1),
        ] {
            let mut bad = b.clone();
            bad[offset..offset + 4].copy_from_slice(&(value as u32).to_le_bytes());
            assert!(native_trace(&bad, Some(&log), 1, 2).is_err());
        }
    }
}
