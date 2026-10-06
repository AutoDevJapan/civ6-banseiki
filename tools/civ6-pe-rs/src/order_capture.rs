//! Fixed, bounded capture capability; no addresses or instruction bytes from input.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

pub const EXE_SHA: &str = "e7450823cc8e00468cff7b9d7b97c63140eae38ae1d774ba4efa437556c42d63";
pub const CORE_SHA: &str = "324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c";
pub const SCHEMA: &str = "todo880-order-v1";

#[path = "order_analysis.rs"]
pub mod analysis;

#[cfg(windows)]
#[path = "order_protocol.rs"]
pub mod protocol;

pub fn source_identity() -> Value {
    let files = [
        (
            "civ6-pe-rs/src/order_analysis.rs",
            include_str!("order_analysis.rs"),
        ),
        (
            "civ6-pe-rs/src/order_step_tests.rs",
            include_str!("order_step_tests.rs"),
        ),
        (
            "civ6-pe-rs/src/order_protocol.rs",
            include_str!("order_protocol.rs"),
        ),
        (
            "civ6-pe-rs/src/order_capture.rs",
            include_str!("order_capture.rs"),
        ),
        (
            "civ6-pe-rs/src/order_windows.rs",
            include_str!("order_windows.rs"),
        ),
        (
            "civ6-pe-rs/src/bin/civ6-order-observer.rs",
            include_str!("bin/civ6-order-observer.rs"),
        ),
        (
            "civ6-native-probe-rs/src/windows.rs",
            include_str!("../../civ6-native-probe-rs/src/windows.rs"),
        ),
        (
            "civ6-native-probe-rs/src/lib.rs",
            include_str!("../../civ6-native-probe-rs/src/lib.rs"),
        ),
        (
            "civ6-native-probe-rs/src/return_correlation.rs",
            include_str!("../../civ6-native-probe-rs/src/return_correlation.rs"),
        ),
        (
            "civ6-native-probe-rs/Cargo.toml",
            include_str!("../../civ6-native-probe-rs/Cargo.toml"),
        ),
    ];
    let mut digest = Sha256::new();
    let rows: Vec<_> = files
        .iter()
        .map(|(path, body)| {
            digest.update(path.as_bytes());
            digest.update([0]);
            digest.update(body.as_bytes());
            json!({"path":path,"sha256":format!("{:x}",Sha256::digest(body.as_bytes()))})
        })
        .collect();
    json!({"compiled_source_sha256":format!("{:x}",digest.finalize()),"source_files":rows,"base_head":"c99055e57ce016d7443470fead5c171ffad3c9e0","old_prefix_clones":0})
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Front,
    Load,
}
impl Phase {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "front" => Ok(Self::Front),
            "load" => Ok(Self::Load),
            _ => Err("phase must be front or load".into()),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Front => "front",
            Self::Load => "load",
        }
    }
    pub fn sites(self) -> &'static [Site; 4] {
        match self {
            Self::Front => &FRONT,
            Self::Load => &LOAD,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Site {
    pub name: &'static str,
    pub rva: u64,
    pub bytes: &'static [u8],
    pub next: Next,
}
#[derive(Clone, Copy, Debug)]
pub enum Next {
    Sequential,
    Rcx,
    Rva(u64),
}
pub const FRONT: [Site; 4] = [
    Site {
        name: "loader_return",
        rva: 0xf8577,
        bytes: &[0x48, 0x89, 0x05, 0x5a, 0xdb, 0x1d, 2],
        next: Next::Sequential,
    },
    Site {
        name: "context_factory_call",
        rva: 0xf845d,
        bytes: &[0xff, 0xd1],
        next: Next::Rcx,
    },
    Site {
        name: "context_factory_return",
        rva: 0xf845f,
        bytes: &[0x48, 0x89, 0x05, 0x5a, 0xdc, 0x1d, 2],
        next: Next::Sequential,
    },
    // lua_newstate call is 0x972bb8 -> 0xdbd848. This is its return, not Gameplay identity.
    Site {
        name: "lua_newstate_return",
        rva: 0x972bbd,
        bytes: &[0x44, 0x8b, 0xc6],
        next: Next::Sequential,
    },
];
pub const LOAD: [Site; 4] = [
    Site {
        name: "context_release",
        rva: 0xf7110,
        bytes: &[0x40, 0x53],
        next: Next::Sequential,
    },
    Site {
        name: "context_replace",
        rva: 0xf7830,
        bytes: &[0x48, 0x89, 0x5c, 0x24, 8],
        next: Next::Sequential,
    },
    Site {
        name: "lua_close_call",
        rva: 0x971b11,
        bytes: &[0xe8, 0x26, 0xbd, 0x44, 0],
        next: Next::Rva(0xdbd83c),
    },
    Site {
        name: "lua_close_deleting_call",
        rva: 0x971cb7,
        bytes: &[0xe8, 0x80, 0xbb, 0x44, 0],
        next: Next::Rva(0xdbd83c),
    },
];
pub fn prefix(phase: Phase, count: usize) -> Result<&'static [Site], String> {
    if !(1..=4).contains(&count) {
        return Err("approved prefix count must be 1..4".into());
    }
    Ok(&phase.sites()[..count])
}
pub fn successor(site: Site, base: u64, rcx: u64) -> Result<u64, String> {
    match site.next {
        Next::Sequential => base
            .checked_add(site.rva)
            .and_then(|a| a.checked_add(site.bytes.len() as u64)),
        Next::Rva(rva) => base.checked_add(rva),
        Next::Rcx => (rcx != 0).then_some(rcx),
    }
    .ok_or("invalid instruction successor".into())
}
pub fn classify_hit(
    address: u64,
    rip: u64,
    dr: [u64; 4],
    dr7: u64,
    dr6: u64,
    sites: &[u64],
) -> Result<Option<usize>, String> {
    if sites.is_empty() || sites.len() > 4 {
        return Err("invalid hardware slot count".into());
    }
    let slot = sites.iter().position(|a| *a == address);
    if slot.is_none() && !sites.contains(&rip) {
        return Ok(None);
    }
    let i = slot.ok_or("exception address and RIP disagree")?;
    if rip != address
        || dr[i] != address
        || dr7 & (3 << (i * 2)) != 1 << (i * 2)
        || dr7 & (15 << (16 + i * 4)) != 0
        || dr6 & 15 != 1 << i
        || dr6 & ((1 << 13) | (1 << 14) | (1 << 15)) != 0
    {
        return Err("owned breakpoint register/RIP mismatch".into());
    }
    Ok(Some(i))
}

pub fn restored_stop(row: &Value, pid: u32, created: u64) -> bool {
    row["type"] == "order_stop"
        && row["schema"] == SCHEMA
        && row["game_pid"].as_u64() == Some(u64::from(pid))
        && row["game_created_filetime"].as_u64() == Some(created)
        && row["terminal_kind"] == "detached"
        && row["debug_registers_restored"] == true
        && row["input_reader_joined"] == true
}
pub fn verify_step(
    expected: u64,
    address: u64,
    rip: u64,
    dr6: u64,
    eflags: u32,
    armed_eflags: u32,
) -> Result<(), String> {
    // Owned Windows x64 MOV fixture: TF is set/read back before Continue,
    // then cleared at first-chance delivery while DR6.BS proves the step.
    if address != expected
        || rip != expected
        || dr6 & 0xe00f != 1 << 14
        || eflags & (1 << 8) != 0
        || armed_eflags & (1 << 8) == 0
    {
        return Err("single original instruction completion is unproven".into());
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Request {
    pub pid: u32,
    pub created: u64,
    pub chain_pid: u32,
    pub chain_created: u64,
    pub controller_pid: u32,
    pub controller_created: u64,
    pub source_dll: PathBuf,
    pub source_dll_sha: String,
    pub output: PathBuf,
    pub seconds: u64,
    pub max_hits: u64,
    pub front_prefix: usize,
    pub load_prefix: usize,
    pub phase: Phase,
}
impl Request {
    pub fn parse(v: &Value) -> Result<Self, String> {
        let obj = v.as_object().ok_or("request object required")?;
        const KEYS: [&str; 15] = [
            "schema",
            "pid",
            "created_filetime",
            "chain_pid",
            "chain_created_filetime",
            "controller_pid",
            "controller_created_filetime",
            "source_dll",
            "source_dll_sha256",
            "output",
            "max_seconds",
            "max_hits",
            "front_prefix",
            "load_prefix",
            "phase",
        ];
        if obj.len() != KEYS.len()
            || obj.keys().any(|k| !KEYS.contains(&k.as_str()))
            || v["schema"] != SCHEMA
        {
            return Err("exact order-v1 request fields required".into());
        }
        let number = |key: &str| {
            v[key]
                .as_u64()
                .filter(|n| *n != 0)
                .ok_or_else(|| format!("positive integer {key} required"))
        };
        let pid = |key: &str| u32::try_from(number(key)?).map_err(|_| format!("{key} exceeds u32"));
        let text = |key: &str| {
            v[key]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| format!("text {key} required"))
        };
        let req = Self {
            pid: pid("pid")?,
            created: number("created_filetime")?,
            chain_pid: pid("chain_pid")?,
            chain_created: number("chain_created_filetime")?,
            controller_pid: pid("controller_pid")?,
            controller_created: number("controller_created_filetime")?,
            source_dll: PathBuf::from(text("source_dll")?),
            source_dll_sha: text("source_dll_sha256")?.into(),
            output: PathBuf::from(text("output")?),
            seconds: number("max_seconds")?,
            max_hits: number("max_hits")?,
            front_prefix: number("front_prefix")? as usize,
            load_prefix: number("load_prefix")? as usize,
            phase: Phase::parse(text("phase")?)?,
        };
        prefix(Phase::Front, req.front_prefix)?;
        prefix(Phase::Load, req.load_prefix)?;
        if req.seconds > 300
            || req.max_hits > 256
            || req.pid == req.chain_pid
            || req.pid == req.controller_pid
            || req.chain_pid == req.controller_pid
            || req.source_dll_sha.len() != 64
            || !req.source_dll_sha.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("invalid capture limits or identity".into());
        }
        Ok(req)
    }
    pub fn count(&self, phase: Phase) -> usize {
        match phase {
            Phase::Front => self.front_prefix,
            Phase::Load => self.load_prefix,
        }
    }
}

fn u32at(b: &[u8], p: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        b.get(p..p.checked_add(4).ok_or("offset overflow")?)
            .ok_or("truncated PE")?
            .try_into()
            .unwrap(),
    ))
}
pub fn disk_sites(bytes: &[u8]) -> Result<Value, String> {
    let hash = format!("{:x}", Sha256::digest(bytes));
    if hash != EXE_SHA {
        return Err("EXE SHA mismatch".into());
    }
    let p = u32at(bytes, 0x3c)? as usize;
    let n = (u32at(bytes, p + 4)? >> 16) as usize;
    let optional = (u32at(bytes, p + 20)? & 0xffff) as usize;
    let table = p + 24 + optional;
    let mut rows = Vec::new();
    for phase in [Phase::Front, Phase::Load] {
        for site in phase.sites() {
            let mut offset = None;
            for i in 0..n {
                let s = table + i * 40;
                let r = u32at(bytes, s + 12)? as u64;
                let size = u32at(bytes, s + 16)? as u64;
                if site.rva >= r && site.rva + site.bytes.len() as u64 <= r + size {
                    if u32at(bytes, s + 36)? & 0x2000_0000 == 0 {
                        return Err("site section not executable".into());
                    }
                    offset = Some(u32at(bytes, s + 20)? as usize + (site.rva - r) as usize);
                    break;
                }
            }
            let o = offset.ok_or("site outside file sections")?;
            if bytes.get(o..o + site.bytes.len()) != Some(site.bytes) {
                return Err(format!("opcode mismatch at 0x{:x}", site.rva));
            }
            rows.push(json!({"phase":phase.name(),"name":site.name,"rva":format!("0x{:x}",site.rva),"instruction_hex":site.bytes.iter().map(|b|format!("{b:02x}")).collect::<String>(),"file_offset":o,"static_admission":"matched","runtime_hits":null,"original_instruction_executions":null}));
        }
    }
    Ok(
        json!({"schema":SCHEMA,"source":source_identity(),"exe_sha256":hash,"gamecore_sha256":CORE_SHA,"static_sites":rows,"calibration_cap":4,"prefix_range":[1,4],"max_hits":256,"max_seconds":300,"phase_switch_cap":8,"live_observation":"unknown","common_boundary_proof":"unknown","startup_before_gameplay":"unknown","console_lease_generation_verified":false,"native_parent_cleanup_verified":false,"native_order_observed":false,"memory_writes":0,"dll_install_calls":0,"live_runs":0}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prefixes_and_wrong_traps_are_refused() {
        assert!(prefix(Phase::Front, 0).is_err());
        assert!(prefix(Phase::Load, 5).is_err());
        assert!(classify_hit(5, 5, [1, 2, 3, 4], 0x55, 1, &[1, 2, 3, 4, 5]).is_err());
        assert_eq!(prefix(Phase::Front, 4).unwrap().len(), 4);
        assert_eq!(
            classify_hit(0x123, 0x123, [0x123, 0, 0, 0], 1, 1, &[0x123]).unwrap(),
            Some(0)
        );
        assert!(classify_hit(0x123, 0x124, [0x123, 0, 0, 0], 1, 1, &[0x123]).is_err());
        assert!(classify_hit(0x123, 0x123, [0x124, 0, 0, 0], 1, 1, &[0x123]).is_err());
        assert!(classify_hit(0x123, 0x123, [0x123, 0, 0, 0], 0x10001, 1, &[0x123]).is_err());
        assert!(classify_hit(0x123, 0x123, [0x123, 0, 0, 0], 1, 0, &[0x123]).is_err());
        assert!(classify_hit(0x123, 0x123, [0x123, 0, 0, 0], 1, 1 << 14, &[0x123]).is_err());
        assert_eq!(
            classify_hit(0x200, 0x200, [0x123, 0, 0, 0], 1, 0, &[0x123]).unwrap(),
            None
        );
    }
    #[test]
    fn one_instruction_requires_the_expected_successor() {
        assert_eq!(
            successor(FRONT[1], 0x140000000, 0x180752d50).unwrap(),
            0x180752d50
        );
        assert_eq!(successor(LOAD[2], 0x140000000, 0).unwrap(), 0x140dbd83c);
        assert!(successor(FRONT[1], 0, 0).is_err());
        assert!(verify_step(10, 11, 11, 1 << 14, 0, 1 << 8).is_err());
        assert!(verify_step(10, 10, 11, 1 << 14, 0, 1 << 8).is_err());
        assert!(verify_step(10, 11, 10, 1 << 14, 0, 1 << 8).is_err());
        assert!(verify_step(10, 10, 10, 0, 0, 1 << 8).is_err());
        assert!(verify_step(10, 10, 10, (1 << 14) | 1, 0, 1 << 8).is_err());
        assert!(verify_step(10, 10, 10, 1 << 14, 0, 0).is_err());
        assert!(verify_step(10, 10, 10, 1 << 14, 1 << 8, 1 << 8).is_err());
        verify_step(10, 10, 10, 0xffff4ff0, 0x202, 0x10302).unwrap();
        assert!(disk_sites(b"bad PE").is_err());
    }
    #[test]
    fn request_rejects_unbounded_prefix_and_forged_identity_fields() {
        let good = json!({"schema":SCHEMA,"pid":1,"created_filetime":100,"chain_pid":2,"chain_created_filetime":90,"controller_pid":3,"controller_created_filetime":95,"source_dll":"D:/bridge.dll","source_dll_sha256":"a".repeat(64),"output":"D:/order.jsonl","max_seconds":30,"max_hits":4,"front_prefix":4,"load_prefix":4,"phase":"front"});
        assert_eq!(Request::parse(&good).unwrap().front_prefix, 4);
        for (field, value) in [
            ("front_prefix", json!(5)),
            ("load_prefix", json!(0)),
            ("max_hits", json!(257)),
            ("max_seconds", json!(301)),
            ("created_filetime", json!(0)),
            ("controller_pid", json!(1)),
            ("phase", json!("automatic")),
            ("source_dll_sha256", json!("secret")),
        ] {
            let mut bad = good.clone();
            bad[field] = value;
            assert!(Request::parse(&bad).is_err(), "accepted {field}");
        }
        let mut bad = good.clone();
        bad["sites"] = json!(["0x123"]);
        assert!(Request::parse(&bad).is_err());
    }
    #[test]
    fn parent_requires_restoration_receipt_for_exact_game() {
        let row = json!({"type":"order_stop","schema":SCHEMA,"game_pid":7,"game_created_filetime":9,"terminal_kind":"detached","debug_registers_restored":true,"input_reader_joined":true});
        assert!(restored_stop(&row, 7, 9));
        assert!(!restored_stop(&row, 7, 10));
        for field in ["debug_registers_restored", "input_reader_joined"] {
            let mut bad = row.clone();
            bad[field] = json!(false);
            assert!(!restored_stop(&bad, 7, 9));
        }
        let mut bad = row;
        bad["terminal_kind"] = json!("attached_unresolved");
        assert!(!restored_stop(&bad, 7, 9));
    }
}
