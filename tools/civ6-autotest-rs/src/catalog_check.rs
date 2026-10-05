//! Offline verifier for the finite normal-catalog22 capture, never permission authority.
//! The two rings have independent sequence numbers: no cross-ring chronology is invented.
//! A retained prefix can prove observed positives, never full-history negatives.
use crate::{persona_payment::ProcessIdentity, Error, Result};
use civ6_native_bridge_rs::{paid_catalog::COUNT, PINNED_GAMECORE_SHA256, TRACE_CAPACITY};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

mod steps;
pub use steps::{evidence_from_steps, CatalogEntry, Evidence, Matrix};

// These RVAs are Windows-only exports upstream. Keep the offline wire contract
// portable; the Windows test checks them against the implementation constants.
const CAN_EVER_RVA: u64 = 0x2d_ddd0;
const PROPERTY_RVA: u64 = 0x99_f5c0;
const CALIBRATION_LIMIT: usize = 1024;
const COUNTER_SCOPE: &str = "bounded_prefix_samples_not_complete_call_history";

pub(super) fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::input(message))
    }
}
pub(super) fn number(v: &Value, name: &str) -> Result<u64> {
    v[name]
        .as_u64()
        .ok_or_else(|| Error::input(format!("missing unsigned integer: {name}")))
}
fn pointer(v: &Value, name: &str) -> Result<u64> {
    let s = v[name]
        .as_str()
        .and_then(|s| s.strip_prefix("0x"))
        .filter(|s| s.len() == 16 && s.bytes().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(|| Error::input(format!("invalid 64-bit hex: {name}")))?;
    u64::from_str_radix(s, 16).map_err(|_| Error::input("invalid hex"))
}
fn shape(v: &Value, fields: &[&str]) -> Result<()> {
    let obj = v
        .as_object()
        .ok_or_else(|| Error::input("JSON row must be an object"))?;
    require(
        obj.len() == fields.len() && fields.iter().all(|s| obj.contains_key(*s)),
        "raw row fields differ from frozen controller format",
    )
}

/// serde_json::Value normally accepts repeated object keys. Evidence must not.
pub(super) fn json(text: &str) -> Result<Value> {
    struct Unique(Value);
    impl<'de> serde::Deserialize<'de> for Unique {
        fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = Unique;
                fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str("JSON with unique keys")
                }
                fn visit_bool<E: serde::de::Error>(
                    self,
                    v: bool,
                ) -> std::result::Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_i64<E: serde::de::Error>(self, v: i64) -> std::result::Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_u64<E: serde::de::Error>(self, v: u64) -> std::result::Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_f64<E: serde::de::Error>(self, v: f64) -> std::result::Result<Unique, E> {
                    serde_json::Number::from_f64(v)
                        .map(|v| Unique(Value::Number(v)))
                        .ok_or_else(|| E::custom("nonfinite JSON"))
                }
                fn visit_str<E: serde::de::Error>(self, v: &str) -> std::result::Result<Unique, E> {
                    Ok(Unique(v.into()))
                }
                fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<Unique, E> {
                    Ok(Unique(Value::Null))
                }
                fn visit_seq<A: serde::de::SeqAccess<'de>>(
                    self,
                    mut a: A,
                ) -> std::result::Result<Unique, A::Error> {
                    let mut out = Vec::new();
                    while let Some(Unique(v)) = a.next_element()? {
                        out.push(v);
                    }
                    Ok(Unique(Value::Array(out)))
                }
                fn visit_map<A: serde::de::MapAccess<'de>>(
                    self,
                    mut a: A,
                ) -> std::result::Result<Unique, A::Error> {
                    let mut out = serde_json::Map::new();
                    while let Some((k, Unique(v))) = a.next_entry::<String, Unique>()? {
                        if out.insert(k, v).is_some() {
                            return Err(serde::de::Error::custom("duplicate JSON key"));
                        }
                    }
                    Ok(Unique(Value::Object(out)))
                }
            }
            d.deserialize_any(Visitor)
        }
    }
    Ok(serde_json::from_str::<Unique>(text)?.0)
}

#[derive(Debug, Serialize)]
pub struct PairCount {
    pub source: u32,
    pub subject_owner: u32,
    pub observed_owner: u32,
    pub governor_index: u32,
    pub rows: usize,
    pub original_zero_effective_one: usize,
    pub original_one_effective_one: usize,
    pub effective_zero: usize,
    pub calibration_threads: BTreeSet<u64>,
    pub core_getter_threads: BTreeSet<u64>,
    pub core_allow_true_rows: usize,
}
#[derive(Debug, Serialize)]
pub struct Report {
    pub capture_valid: bool,
    pub qualification_positive22: bool,
    pub product_acceptance: bool,
    pub payment_authority: bool,
    pub scope: &'static str,
    pub baseline_unmeasured: Vec<&'static str>,
    pub process: ProcessIdentity,
    pub gamecore_sha256: &'static str,
    pub trace_sha256: String,
    pub steps_sha256: String,
    pub scenario_contract_sha256: String,
    pub catalog: Vec<CatalogEntry>,
    pub pairs: Vec<PairCount>,
    pub calibration_rows: usize,
    pub property_rows: usize,
    pub unrelated_calibration_rows: usize,
    pub observed_calls: u64,
    pub dropped: u64,
    pub filtered_calls: u64,
    pub getter_calls_seen: u64,
    pub calibration_sample_limit: usize,
    pub property_sample_limit: usize,
    pub full_history_authority: bool,
    pub cross_ring_chronology_measured: bool,
    pub gameplay_closed484: bool,
    pub ui_native_qualification: Matrix,
    pub gameplay_projection: Matrix,
    pub missing_positive_sources: Vec<u32>,
    pub qualification_errors: Vec<String>,
}
pub(super) fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

/// Reads full original files; binds start/stop filename and canonical directory.
/// Moving a trace away from its recorded restoration path is deliberately refused.
pub fn check_files(trace: &Path, steps: &Path) -> Result<Report> {
    let trace = trace.canonicalize()?;
    let steps = steps.canonicalize()?;
    require(
        trace.parent() == steps.parent()
            && steps.file_name().and_then(|s| s.to_str()) == Some("steps.jsonl"),
        "trace and steps must be in the same artifact directory",
    )?;
    let evidence = evidence_from_steps(&std::fs::read_to_string(steps)?)?;
    require(
        trace.file_name().and_then(|s| s.to_str()) == Some(evidence.artifact.as_str()),
        "trace filename differs from start receipt",
    )?;
    require(
        evidence.artifact_path.canonicalize()? == trace,
        "trace belongs to a different restoration artifact",
    )?;
    verify(&std::fs::read_to_string(trace)?, &evidence)
}

/// Public text API for unit models. Only check_files also verifies physical paths.
pub fn verify(trace: &str, evidence: &Evidence) -> Result<Report> {
    let rows: Vec<Value> = trace.lines().map(json).collect::<Result<_>>()?;
    require(rows.len() >= 5, "missing capture rows")?;
    let identity = &rows[0];
    shape(
        identity,
        &[
            "kind",
            "pid",
            "created_filetime",
            "gamecore_sha256",
            "mode",
            "entry_rva",
            "entry_bytes",
            "bridge_sha256",
            "duration_limit_seconds",
            "paid_assignment_experimental",
            "can_ever_rva",
            "property_rva",
            "property_entry_bytes",
            "prefix_bytes",
            "scope_key_ranges",
            "reset_key_range",
        ],
    )?;
    require(
        identity["kind"] == "identity"
            && number(identity, "pid")? == u64::from(evidence.process.pid)
            && number(identity, "created_filetime")? == evidence.process.created_filetime
            && identity["gamecore_sha256"] == PINNED_GAMECORE_SHA256
            && identity["mode"] == "production-admission"
            && number(identity, "entry_rva")? == CAN_EVER_RVA
            && number(identity, "can_ever_rva")? == CAN_EVER_RVA
            && number(identity, "property_rva")? == PROPERTY_RVA
            && identity["paid_assignment_experimental"] == false
            && identity["duration_limit_seconds"] == 300
            && identity["entry_bytes"] == "[48, 89, 5c, 24, 08, 48, 89, 6c]"
            && identity["property_entry_bytes"] == "[48, 8b, 41, 40, 4c, 8d, 49, 30]"
            && identity["prefix_bytes"] == "[cc, cc, cc, cc, cc]"
            && identity["scope_key_ranges"]
                == serde_json::json!([[900000, 900255], [901000, 901255], [903000, 903255]])
            && identity["reset_key_range"] == serde_json::json!([902000, 902063]),
        "capture source, process, mode or native entry identity mismatch",
    )?;
    require(
        identity["bridge_sha256"]
            .as_str()
            .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())),
        "bridge SHA missing",
    )?;
    shape(&rows[1], &["kind", "status"])?;
    require(
        rows[1]["kind"] == "install" && rows[1]["status"] == 0,
        "single successful install required",
    )?;
    shape(
        &rows[2],
        &[
            "kind",
            "status",
            "entry_restored",
            "assignment_restored",
            "prefix_retained_until_exit",
        ],
    )?;
    require(
        rows[2]["kind"] == "uninstall"
            && rows[2]["status"] == 0
            && rows[2]["entry_restored"] == true
            && rows[2]["assignment_restored"] == false
            && rows[2]["prefix_retained_until_exit"] == true,
        "complete entry restoration and resident prefix required",
    )?;

    let mut report = Report {
        capture_valid: true, qualification_positive22: false, product_acceptance: false, payment_authority: false,
        scope: "finite normal catalog qualification; raw retained prefixes, not product or payment acceptance",
        baseline_unmeasured: vec!["initial UI closed484", "per-join UI negatives", "complete native call history", "AI-initiated payment", "startup/load", "unpaid placement", "multiplayer", "113-step suite", "neutralization and recovery"],
        process: evidence.process.clone(), gamecore_sha256: PINNED_GAMECORE_SHA256,
        trace_sha256: digest(trace), steps_sha256: evidence.steps_sha256.clone(), scenario_contract_sha256: evidence.scenario_contract_sha256.clone(),
        catalog: evidence.catalog.clone(), pairs: Vec::new(), calibration_rows: 0, property_rows: 0, unrelated_calibration_rows: 0,
        observed_calls: 0, dropped: 0, filtered_calls: 0, getter_calls_seen: 0,
        calibration_sample_limit: CALIBRATION_LIMIT, property_sample_limit: TRACE_CAPACITY,
        full_history_authority: false, cross_ring_chronology_measured: false, gameplay_closed484: true,
        ui_native_qualification: evidence.ui.clone(), gameplay_projection: evidence.gameplay.clone(),
        missing_positive_sources: Vec::new(), qualification_errors: Vec::new(),
    };
    for e in &evidence.catalog {
        for owner in 0..COUNT as u32 {
            report.pairs.push(PairCount {
                source: e.source,
                subject_owner: e.owner,
                observed_owner: owner,
                governor_index: e.index,
                rows: 0,
                original_zero_effective_one: 0,
                original_one_effective_one: 0,
                effective_zero: 0,
                calibration_threads: BTreeSet::new(),
                core_getter_threads: BTreeSet::new(),
                core_allow_true_rows: 0,
            });
        }
    }
    let mut owners = BTreeMap::new(); // owner -> (player address, Governors receiver, property receiver)
    let mut receivers = BTreeMap::new();
    let mut in_properties = false;
    let mut properties = Vec::new();
    for row in &rows[3..rows.len() - 1] {
        match row["kind"].as_str() {
            Some("calibration") => {
                require(
                    !in_properties,
                    "calibration rows must precede the property ring",
                )?;
                shape(
                    row,
                    &[
                        "kind",
                        "sequence",
                        "governor_index",
                        "governors_this",
                        "return_site_rva",
                        "thread_id",
                        "owner_read_state",
                        "player_address",
                        "player_id",
                        "property_receiver",
                        "original_al",
                        "effective_al",
                    ],
                )?;
                report.calibration_rows += 1;
                require(
                    number(row, "sequence")? == report.calibration_rows as u64
                        && report.calibration_rows <= CALIBRATION_LIMIT,
                    "calibration prefix is not contiguous from sequence one",
                )?;
                let owner = number(row, "player_id")?;
                let index = number(row, "governor_index")?;
                let thread = number(row, "thread_id")?;
                let player = pointer(row, "player_address")?;
                let gov = pointer(row, "governors_this")?;
                let receiver = pointer(row, "property_receiver")?;
                let original = number(row, "original_al")?;
                let effective = number(row, "effective_al")?;
                require(
                    owner < COUNT as u64
                        && index < 256
                        && thread > 0
                        && thread <= u32::MAX as u64
                        && row["owner_read_state"] == 1
                        && original <= 1
                        && effective <= 1
                        && player > 0
                        && gov > 0
                        && player.checked_add(0x528) == Some(receiver)
                        && number(row, "return_site_rva")? > 0,
                    "invalid current native owner/index/AL/receiver/thread",
                )?;
                if let Some(old) = owners.insert(owner, (player, gov, receiver)) {
                    require(
                        old == (player, gov, receiver),
                        "native owner receiver changed",
                    )?;
                }
                if let Some(old) = receivers.insert(receiver, owner) {
                    require(old == owner, "native receiver aliases another owner")?;
                }
                if let Some(e) = evidence
                    .catalog
                    .iter()
                    .find(|e| u64::from(e.index) == index)
                {
                    let p = &mut report.pairs[e.source as usize * COUNT + owner as usize];
                    p.rows += 1;
                    p.calibration_threads.insert(thread);
                    match (original, effective) {
                        (0, 1) => p.original_zero_effective_one += 1,
                        (1, 1) => p.original_one_effective_one += 1,
                        _ => p.effective_zero += 1,
                    }
                    if effective == 1 && owner != u64::from(e.owner) {
                        report.qualification_errors.push(format!(
                            "source {} positive for foreign owner {owner}",
                            e.source
                        ));
                    }
                } else {
                    report.unrelated_calibration_rows += 1;
                    require(
                        original == effective,
                        "noncatalog result unexpectedly overridden",
                    )?;
                }
            }
            Some("property_get") => {
                in_properties = true;
                let mut fields = vec![
                    "kind",
                    "sequence",
                    "property",
                    "key_id",
                    "returned",
                    "thread_id",
                    "read_state",
                    "tag",
                    "payload8",
                    "flags14",
                ];
                let has_denial = row.get("denial_checked").is_some();
                if has_denial {
                    fields.extend(["denial_owner", "denial_index", "denial_checked"]);
                }
                shape(row, &fields)?;
                report.property_rows += 1;
                require(
                    number(row, "sequence")? == report.property_rows as u64
                        && report.property_rows <= TRACE_CAPACITY,
                    "property prefix is not contiguous from sequence one",
                )?;
                let receiver = pointer(row, "property")?;
                let thread = number(row, "thread_id")?;
                let key = number(row, "key_id")?;
                let payload = pointer(row, "payload8")?;
                require(
                    receiver > 0
                        && pointer(row, "returned")? > 0
                        && thread > 0
                        && thread <= u32::MAX as u64
                        && row["read_state"] == 1
                        && row["tag"] == 1
                        && row["flags14"] == 0
                        && payload <= 1,
                    "property value is not a successfully read canonical boolean",
                )?;
                if has_denial {
                    require(
                        row["denial_checked"].is_boolean()
                            && number(row, "denial_owner")? < 64
                            && number(row, "denial_index")? < 256,
                        "invalid optional denial metadata",
                    )?;
                    if row["denial_checked"] == true {
                        require(
                            (900000..=900255).contains(&key) || (901000..=901255).contains(&key),
                            "denial metadata on non-cell key",
                        )?;
                        require(
                            number(row, "denial_index")? == key % 1000
                                && receivers
                                    .get(&receiver)
                                    .is_none_or(|o| *o == number(row, "denial_owner").unwrap()),
                            "denial metadata subject mismatch",
                        )?;
                    }
                }
                require(
                    (900000..=900255).contains(&key)
                        || (901000..=901255).contains(&key)
                        || (902000..=902063).contains(&key)
                        || (903000..=903255).contains(&key),
                    "foreign property key",
                )?;
                properties.push((receiver, thread, key, payload == 1));
            }
            _ => {
                return Err(Error::input(
                    "unknown/duplicate/lifecycle row inside native rings",
                ))
            }
        }
    }
    let stop = rows.last().unwrap();
    shape(
        stop,
        &[
            "kind",
            "rows",
            "dropped",
            "observed_calls",
            "filtered_calls",
            "getter_calls_seen",
            "counter_scope",
            "calibration_sample_limit",
            "property_sample_limit",
            "in_flight",
            "snapshot_stable",
            "quiescence_proven",
            "classification",
            "prefix_retained_until_exit",
            "result",
        ],
    )?;
    require(
        stop["kind"] == "stop"
            && stop["result"] == "ok"
            && stop["classification"] == "production_admission"
            && stop["prefix_retained_until_exit"] == true
            && stop["in_flight"] == 0
            && stop["snapshot_stable"] == true
            && stop["quiescence_proven"] == false
            && stop["counter_scope"] == COUNTER_SCOPE
            && number(stop, "calibration_sample_limit")? == CALIBRATION_LIMIT as u64
            && number(stop, "property_sample_limit")? == TRACE_CAPACITY as u64,
        "missing/partial stop or false full-history assertion",
    )?;
    report.dropped = number(stop, "dropped")?;
    report.observed_calls = number(stop, "observed_calls")?;
    report.filtered_calls = number(stop, "filtered_calls")?;
    report.getter_calls_seen = number(stop, "getter_calls_seen")?;
    let retained = (report.calibration_rows + report.property_rows) as u64;
    require(
        number(stop, "rows")? == retained
            && retained > 0
            && retained.checked_add(report.dropped) == Some(report.observed_calls)
            && report.observed_calls.checked_add(report.filtered_calls)
                == Some(report.getter_calls_seen)
            && (report.dropped == 0
                || report.calibration_rows == CALIBRATION_LIMIT
                || report.property_rows == TRACE_CAPACITY),
        "raw rows, observed/dropped/filtered counters or saturated prefix disagree",
    )?;

    // Replay only the retained Core getter prefix. Reset binds the receiver to
    // its real owner; per-cell close(false), manifest(true), allow(true) then
    // supplies a bool receipt. A UI matrix cannot fill a missing raw row.
    let mut bound = BTreeMap::new(); // receiver -> (owner, Core thread)
    let mut cells = BTreeMap::new(); // (receiver,index) -> (close read, false ack, manifest)
    for (receiver, thread, key, value) in properties {
        if (902000..=902063).contains(&key) {
            let owner = key - 902000;
            require(owner < COUNT as u64 && value, "invalid owner reset")?;
            if let Some(real) = receivers.get(&receiver) {
                require(*real == owner, "reset belongs to another calibrated owner")?;
            }
            if let Some(old) = bound.insert(receiver, (owner, thread)) {
                require(old == (owner, thread), "Core owner/thread binding changed")?;
            }
            cells.retain(|(r, _), _| *r != receiver);
            continue;
        }
        let Some(&(owner, core_thread)) = bound.get(&receiver) else {
            continue;
        };
        require(
            core_thread == thread,
            "Core getter thread differs from reset thread",
        )?;
        let base = key / 1000 * 1000;
        let index = key - base;
        let cell = cells
            .entry((receiver, index))
            .or_insert((false, false, false));
        match base {
            903000 => {
                *cell = (true, false, false);
            }
            901000 if !value => {
                cell.1 = cell.0;
                cell.2 = false;
            }
            900000 => {
                cell.2 = value && cell.1;
            }
            901000 if value => {
                if let Some(e) = evidence
                    .catalog
                    .iter()
                    .find(|e| u64::from(e.index) == index)
                {
                    if owner != u64::from(e.owner) {
                        report.qualification_errors.push(format!(
                            "source {} Core allow for foreign owner {owner}",
                            e.source
                        ));
                    }
                    if cell.0 && cell.1 && cell.2 && receivers.get(&receiver) == Some(&owner) {
                        let p = &mut report.pairs[e.source as usize * COUNT + owner as usize];
                        p.core_allow_true_rows += 1;
                        p.core_getter_threads.insert(thread);
                    }
                }
            }
            _ => (),
        }
    }
    for e in &evidence.catalog {
        let p = &report.pairs[e.source as usize * COUNT + e.owner as usize];
        if p.original_zero_effective_one + p.original_one_effective_one == 0
            || p.core_allow_true_rows == 0
        {
            report.missing_positive_sources.push(e.source);
        }
    }
    report.qualification_positive22 =
        report.missing_positive_sources.is_empty() && report.qualification_errors.is_empty();
    Ok(report)
}

#[cfg(all(test, windows))]
mod wire_constants {
    #[test]
    fn portable_contract_matches_native_exports() {
        assert_eq!(
            super::CAN_EVER_RVA as usize,
            civ6_native_bridge_rs::PINNED_CAN_EVER_RVA
        );
        assert_eq!(
            super::PROPERTY_RVA as usize,
            civ6_native_bridge_rs::PINNED_PROPERTY_GET_RVA
        );
        assert_eq!(
            super::CALIBRATION_LIMIT,
            civ6_native_bridge_rs::CALIBRATION_CAPACITY
        );
    }
}
