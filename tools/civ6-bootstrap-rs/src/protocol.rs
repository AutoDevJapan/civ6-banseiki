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
/// Public CompositeTrace framing only. Semantic payment/publication checks
/// remain with the existing Mode6 validator; dropped/in-flight rows fail here.
pub fn native_trace(b: &[u8]) -> Result<(), String> {
    const PROPERTY: usize = 24 + 1024 * 72;
    const ASSIGN: usize = PROPERTY + 32 + 4096 * 48;
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
        let u = |p: usize| -> Result<u32, String> {
            Ok(u32::from_le_bytes(
                b.get(p..p + 4)
                    .ok_or("native header range")?
                    .try_into()
                    .unwrap(),
            ))
        };
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
        assert!(native_trace(&b).is_ok());
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
            assert!(native_trace(&bad).is_err());
        }
    }
}
