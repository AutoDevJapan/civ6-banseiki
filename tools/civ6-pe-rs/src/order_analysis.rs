//! Offline interpretation of capture-only receipts, not authorization to patch.
use crate::order_capture::{Phase, CORE_SHA, EXE_SHA, SCHEMA};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    io::{BufRead, Read},
};

const LIMIT: usize = 20_000;
const BYTE_LIMIT: u64 = 16 * 1024 * 1024;
fn number(row: &Value, key: &str) -> Result<u64, String> {
    row[key]
        .as_u64()
        .ok_or_else(|| format!("missing integer {key}"))
}
fn positive(row: &Value, key: &str) -> Result<u64, String> {
    let n = number(row, key)?;
    if n == 0 {
        return Err(format!("zero {key}"));
    }
    Ok(n)
}
fn sha(row: &Value, key: &str) -> Result<String, String> {
    let s = row[key].as_str().ok_or_else(|| format!("missing {key}"))?;
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("invalid {key}"));
    }
    Ok(s.to_ascii_lowercase())
}

#[derive(Clone)]
struct Hit {
    seq: u64,
    tid: u64,
    window: u64,
    site: String,
    rsp: u64,
    pointer: Option<u64>,
}
pub fn analyze(reader: impl BufRead) -> Result<Value, String> {
    let mut bytes = Vec::new();
    reader
        .take(BYTE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > BYTE_LIMIT {
        return Err("capture byte limit exceeded".into());
    }
    let mut rows = Vec::new();
    for line in std::io::Cursor::new(bytes).lines() {
        if rows.len() >= LIMIT {
            return Err("capture row limit exceeded".into());
        }
        let line = line.map_err(|e| e.to_string())?;
        if line.len() > 1_048_576 {
            return Err("capture row too long".into());
        }
        let v: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        if !v.is_object() {
            return Err("capture rows must be JSON objects".into());
        }
        rows.push(v);
    }
    let first = rows.first().ok_or("empty capture")?;
    if first["type"] != "order_identity" || first["schema"] != SCHEMA {
        return Err("native order identity missing; static snapshot is not a capture".into());
    }
    let pid = positive(first, "game_pid")?;
    let created = positive(first, "game_created_filetime")?;
    let pid32 = u32::try_from(pid).map_err(|_| "PID exceeds Windows DWORD")?;
    let chain = positive(first, "chain_pid")?;
    let controller = positive(first, "controller_pid")?;
    positive(first, "chain_created_filetime")?;
    positive(first, "controller_created_filetime")?;
    if chain == pid || controller == pid || chain == controller {
        return Err("capture ownership identities overlap".into());
    }
    let compiled_source = sha(&first["static_proof"]["source"], "compiled_source_sha256")?;
    if first["static_proof"]["exe_sha256"] != EXE_SHA
        || first["static_proof"]["gamecore_sha256"] != CORE_SHA
    {
        return Err("capture PE identity differs".into());
    }
    let source = sha(first, "source_dll_sha256")?;
    let exe_base = positive(first, "exe_base")?;
    let mut phase = Phase::parse(
        first["capture_phase"]
            .as_str()
            .ok_or("capture phase was not recorded; capture with current observer")?,
    )?;
    let prefixes = [
        number(first, "front_prefix")? as usize,
        number(first, "load_prefix")? as usize,
    ];
    for (p, n) in [(Phase::Front, prefixes[0]), (Phase::Load, prefixes[1])] {
        crate::order_capture::prefix(p, n)?;
    }
    let mut window = 0;
    let mut last_seq = 0;
    let mut last_ms = 0;
    let mut pending = BTreeMap::<u64, (Value, u64)>::new();
    let mut hits = Vec::<Hit>::new();
    let mut counts = BTreeMap::<String, u64>::new();
    let mut armed = BTreeMap::<String, u64>::new();
    let prefix_len = |p| {
        if p == Phase::Front {
            prefixes[0]
        } else {
            prefixes[1]
        }
    };
    for site in crate::order_capture::prefix(phase, prefix_len(phase))? {
        *armed.entry(site.name.into()).or_default() += 1;
    }
    let mut terminal = false;
    let mut next_capture = 1;
    let mut module_changes = Vec::new();
    for row in &rows {
        let seq = positive(row, "sequence")?;
        let ms = number(row, "elapsed_ms")?;
        if seq != last_seq + 1 || ms < last_ms {
            return Err("missing/non-monotonic capture sequence/time".into());
        }
        last_seq = seq;
        last_ms = ms;
        if terminal {
            return Err("rows after terminal receipt".into());
        }
        match row["type"].as_str().ok_or("row type missing")? {
            "order_identity" => {
                if seq != 1 {
                    return Err("duplicate or displaced identity".into());
                }
            }
            "phase_gap" => {
                if !pending.is_empty() {
                    return Err("phase changed with pending instruction".into());
                }
                if row["old"] != phase.name() {
                    return Err("phase history mismatch".into());
                }
                phase = Phase::parse(row["new"].as_str().ok_or("new phase missing")?)?;
                window += 1;
                for site in crate::order_capture::prefix(phase, prefix_len(phase))? {
                    *armed.entry(site.name.into()).or_default() += 1;
                }
            }
            "instruction_before" => {
                let id = positive(row, "capture_seq")?;
                let tid = positive(row, "tid")?;
                if id != next_capture {
                    return Err("capture ID is missing or repeated".into());
                }
                next_capture += 1;
                if row["phase"] != phase.name() {
                    return Err("instruction outside declared phase".into());
                }
                let n = if phase == Phase::Front {
                    prefixes[0]
                } else {
                    prefixes[1]
                };
                let site = crate::order_capture::prefix(phase, n)?
                    .iter()
                    .find(|s| row["site"] == s.name)
                    .ok_or("site not armed in this prefix")?;
                if number(row, "rva")? != site.rva
                    || number(row, "rip")?
                        != exe_base.checked_add(site.rva).ok_or("RIP overflow")?
                {
                    return Err("site/RVA/RIP differs".into());
                }
                if row["original_instruction_executions"] != Value::Null {
                    return Err("before row cannot claim instruction completion".into());
                }
                if pending.insert(tid, (row.clone(), window)).is_some() {
                    return Err("second pending instruction on one thread".into());
                }
            }
            "instruction_completed" => {
                let tid = positive(row, "tid")?;
                let (before, w) = pending
                    .remove(&tid)
                    .ok_or("completion without before row")?;
                for key in [
                    "capture_seq",
                    "site",
                    "phase",
                    "rva",
                    "rax",
                    "rbx",
                    "rcx",
                    "rdx",
                    "r8",
                    "r9",
                    "rsp",
                    "rip",
                    "pointer_candidate",
                ] {
                    if row[key] != before[key] {
                        return Err(format!("completion changed captured {key}"));
                    }
                }
                let site = phase
                    .sites()
                    .iter()
                    .find(|s| row["site"] == s.name)
                    .ok_or("unknown completion site")?;
                let next =
                    crate::order_capture::successor(*site, exe_base, number(&before, "rcx")?)?;
                if number(row, "completion_rip")? != next
                    || row["original_instruction_executions"] != 1
                {
                    return Err("original instruction completion differs".into());
                }
                *counts.entry(site.name.into()).or_default() += 1;
                hits.push(Hit {
                    seq: number(&before, "sequence")?,
                    tid,
                    window: w,
                    site: site.name.into(),
                    rsp: number(&before, "rsp")?,
                    pointer: before["pointer_candidate"].as_u64(),
                });
            }
            "module_load" | "module_unload" => module_changes
                .push(json!({"sequence":seq,"kind":row["type"],"base":positive(row,"base")?})),
            "module_attach_snapshot" | "debug_exception" => {}
            "order_stop" => {
                if !crate::order_capture::restored_stop(row, pid32, created)
                    || row["result"] != "ok"
                    || number(row, "pending_instruction_completions")? != 0
                    || !pending.is_empty()
                {
                    return Err("capture did not end with confirmed restoration".into());
                }
                if row["trace_write_error"].is_string() {
                    return Err("trace has an output failure".into());
                }
                let reported = row["sites"]
                    .as_array()
                    .ok_or("terminal site counts missing")?;
                if reported.len() != 8 {
                    return Err("terminal site inventory incomplete".into());
                }
                for s in crate::order_capture::FRONT
                    .iter()
                    .chain(crate::order_capture::LOAD.iter())
                {
                    let found: Vec<_> = reported.iter().filter(|r| r["site"] == s.name).collect();
                    if found.len() != 1
                        || number(found[0], "completed")?
                            != counts.get(s.name).copied().unwrap_or(0)
                        || number(found[0], "armed_windows")?
                            != armed.get(s.name).copied().unwrap_or(0)
                    {
                        return Err("terminal site counts disagree".into());
                    }
                }
                terminal = true;
            }
            _ => return Err("unaccepted/critical capture row; no positive order proof".into()),
        }
    }
    if !terminal {
        return Err("truncated capture without order_stop".into());
    }
    // Pair a factory invocation only within one phase window, thread and stack
    // level. UI Lua and Gameplay Lua are deliberately not equated.
    let mut pairs = Vec::new();
    let mut calls = BTreeMap::new();
    for h in &hits {
        let key = (h.window, h.tid, h.rsp);
        if h.site == "context_factory_call" {
            calls.insert(key, h.seq);
        }
        if h.site == "context_factory_return" {
            if let Some(call) = calls.remove(&key) {
                pairs.push(json!({"thread_id":h.tid,"window":h.window,"call_sequence":call,"return_sequence":h.seq,"context_candidate":h.pointer}));
            }
        }
    }
    Ok(
        json!({"schema":"todo880-order-analysis-v1","input_integrity":"consistent","game_pid":pid,"game_created_filetime":created,"source_dll_sha256":source,"compiled_source_sha256":compiled_source,"completed_instructions":hits.len(),"site_counts":counts,"factory_pairs":pairs,"module_changes":module_changes,"phase_windows":window+1,"initial_context":first["initial_context"],"initial_core_base":first["initial_core_base"],"first_factory_since_process_start":"unknown","gameplay_lua_identity":"unknown","common_load_boundary_proven":false,"runtime_activation_allowed":false,"runtime_calls_executed":0,"next_runtime_sequence":["BridgeRuntimeInstall before first Gameplay initialization","BridgeRuntimeClose before old objects/Lua destruction","BridgeRuntimeRebuild after destruction and before new Gameplay RESET","existing runtime COMMIT CAS publishes eligibility"],"missing_evidence":["native startup entry before Gameplay, including already-loaded front end","one pre-destruction boundary covering Load/Continue/QuickLoad/LeaveGame","Gameplay state identity and new RESET boundary","runtime installation/rebuild and COMMIT receipts"],"evidence_scope":"supplied capture structure; no live replay or runtime activation"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<Value> {
        let mut rows = vec![
            json!({"type":"order_identity","schema":SCHEMA,"game_pid":7,"game_created_filetime":99,"chain_pid":8,"chain_created_filetime":80,"controller_pid":9,"controller_created_filetime":90,"source_dll_sha256":"a".repeat(64),"exe_base":0x140000000u64,"static_proof":{"exe_sha256":EXE_SHA,"gamecore_sha256":CORE_SHA,"source":{"compiled_source_sha256":"b".repeat(64)}},"capture_phase":"front","front_prefix":4,"load_prefix":4,"initial_context":0,"initial_core_base":0x180000000u64}),
        ];
        for (id, name, rva, rip, rax, rcx) in [
            (
                1,
                "context_factory_call",
                0xf845du64,
                0x180752d50u64,
                0u64,
                0x180752d50u64,
            ),
            (
                2,
                "context_factory_return",
                0xf845f,
                0x1400f8466,
                0x12340000,
                0,
            ),
        ] {
            let row = json!({"type":"instruction_before","capture_seq":id,"tid":21,"phase":"front","site":name,"rva":rva,"rip":0x140000000u64+rva,"rsp":0x99990000u64,"rax":rax,"rbx":0,"rcx":rcx,"rdx":0,"r8":0,"r9":0,"pointer_candidate":if id==2{json!(rax)}else{Value::Null},"original_instruction_executions":null});
            rows.push(row.clone());
            let mut after = row;
            after["type"] = json!("instruction_completed");
            after["original_instruction_executions"] = json!(1);
            after["completion_rip"] = json!(rip);
            rows.push(after);
        }
        let counts:Vec<_>=crate::order_capture::FRONT.iter().chain(crate::order_capture::LOAD.iter()).map(|s|json!({"site":s.name,"armed_windows":u64::from(crate::order_capture::FRONT.iter().any(|f| f.name == s.name)),"completed":if s.name.starts_with("context_factory"){1}else{0}})).collect();
        rows.push(json!({"type":"order_stop","schema":SCHEMA,"game_pid":7,"game_created_filetime":99,"chain_pid":8,"chain_created_filetime":80,"controller_pid":9,"controller_created_filetime":90,"result":"ok","terminal_kind":"detached","debug_registers_restored":true,"input_reader_joined":true,"pending_instruction_completions":0,"sites":counts}));
        for (i, row) in rows.iter_mut().enumerate() {
            row["sequence"] = json!(i + 1);
            row["elapsed_ms"] = json!(i);
        }
        rows
    }
    fn read(rows: &[Value]) -> Result<Value, String> {
        let text = rows
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        analyze(std::io::Cursor::new(text))
    }
    #[test]
    fn structural_fixture_pairs_calls_without_claiming_runtime_activation() {
        let v = read(&fixture()).unwrap();
        assert_eq!(
            v["factory_pairs"],
            json!([{"thread_id":21,"window":0,"call_sequence":2,"return_sequence":4,"context_candidate":0x12340000u64}])
        );
        assert_eq!(v["runtime_activation_allowed"], false);
        assert_eq!(v["common_load_boundary_proven"], false);
        assert_eq!(v["gameplay_lua_identity"], "unknown");
    }
    #[test]
    fn missing_completion_and_truncation_are_rejected() {
        let mut rows = fixture();
        rows.remove(2);
        assert!(read(&rows).is_err());
        let mut rows = fixture();
        rows.pop();
        assert!(read(&rows).is_err());
    }
    #[test]
    fn mismatched_identity_and_successor_cannot_be_order_evidence() {
        for (index, key, value) in [
            (5, "game_created_filetime", json!(100)),
            (2, "completion_rip", json!(0x180752d51u64)),
            (2, "rcx", json!(0)),
            (1, "phase", json!("load")),
        ] {
            let mut rows = fixture();
            rows[index][key] = value;
            assert!(read(&rows).is_err(), "accepted {key}");
        }
    }
    #[test]
    fn phase_gap_never_pairs_old_factory_with_new_return() {
        let mut rows = fixture();
        let gap = json!({"type":"phase_gap","old":"front","new":"front"});
        rows.insert(3, gap);
        for site in rows.last_mut().unwrap()["sites"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .take(4)
        {
            site["armed_windows"] = json!(2);
        }
        for (i, r) in rows.iter_mut().enumerate() {
            r["sequence"] = json!(i + 1);
            r["elapsed_ms"] = json!(i);
        }
        assert_eq!(read(&rows).unwrap()["factory_pairs"], json!([]));
    }
    #[test]
    fn snapshot_booleans_are_not_live_receipts() {
        assert!(analyze(std::io::Cursor::new(
            json!({"schema":SCHEMA,"live_runs":0,"runtime_activation_allowed":true}).to_string()
        ))
        .is_err());
    }
    #[test]
    fn omitted_or_forged_coverage_is_rejected() {
        for value in [Value::Null, json!(0), json!(2)] {
            let mut rows = fixture();
            rows.last_mut().unwrap()["sites"][0]["armed_windows"] = value.clone();
            assert_eq!(
                read(&rows).unwrap_err(),
                if value.is_null() {
                    "missing integer armed_windows"
                } else {
                    "terminal site counts disagree"
                }
            );
        }
    }
    #[test]
    fn input_allocation_is_bounded_before_parsing() {
        let source = std::io::repeat(b' ');
        let mut source = std::io::BufReader::new(source);
        assert_eq!(
            analyze(&mut source).unwrap_err(),
            "capture byte limit exceeded"
        );
    }
}
