//! Independent acceptance of the fixed normal-start unjoined-AI probe.
//! This does not supply joined admission, formal AI payment or the 113 suite.
use crate::{persona_full, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

const CONTRACT: &str = include_str!("persona_unjoined_fixture.json");
const CONTRACT_HASH: &str = "fc87c760531f945cf96bf7d3d1dfee4508144ec0c8aa13c39d9d2b350d8288ef";

#[derive(Debug, Serialize)]
pub struct Report {
    pub normal_unjoined_rejection: bool,
    pub original_steps: usize,
    pub recorded_steps: usize,
    pub preparation_restart: bool,
    pub ai_owners: usize,
    pub persona_definitions: usize,
    pub denied_pairs_per_audit: usize,
    pub native_audits: usize,
    pub actual_turns: usize,
    pub turns: [u64; 3],
    pub matched_save_load_snapshots: usize,
    pub physical_save_name: String,
    pub physical_save_bytes: u64,
    pub physical_save_sha256: String,
    pub contract_sha256: String,
    pub steps_sha256: String,
    pub result_sha256: String,
    pub product_acceptance: bool,
    pub scope: &'static str,
}

fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(message.into())
    }
}
fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn snapshot(value: &str, budget: bool) -> Result<u64> {
    let fields: Vec<_> = value.split('|').collect();
    require(
        fields.len() == 5
            && fields[1] == "ai=21"
            && fields[2] == "persons=22"
            && fields[3] == "negative=462",
        "incomplete native AI matrix",
    )?;
    let turn = fields[0]
        .strip_prefix("turn=")
        .ok_or("missing native turn")?
        .parse()?;
    let rows = fields[4]
        .strip_prefix("titles=")
        .ok_or("missing native titles")?;
    let mut owners = BTreeSet::new();
    for row in rows.split(',') {
        let parts: Vec<_> = row.split(':').collect();
        require(parts.len() == 3, "invalid native title row")?;
        let id: u32 = parts[0].parse()?;
        let total: u32 = parts[1].parse()?;
        let spent: u32 = parts[2].parse()?;
        require(
            (1..=21).contains(&id) && owners.insert(id) && total >= spent,
            "native owner/title balance differs",
        )?;
        require(
            spent == 0 && total == u32::from(budget),
            "native AI title balance differs from fixed fresh probe",
        )?;
    }
    require(owners.len() == 21, "native title coverage differs")?;
    Ok(turn)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SaveProof {
    name: String,
    bytes: u64,
    sha256: String,
}

/// The save's expected size/hash must come from the host's preserved source
/// record, separately from the artifact and its replaceable save-proof.json.
pub fn verify_directory(
    directory: &Path,
    source_bytes: u64,
    source_sha256: &str,
) -> Result<Report> {
    require(
        source_bytes > 0
            && source_sha256.len() == 64
            && source_sha256.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid source save commitment",
    )?;
    require(
        hash(&CONTRACT.replace("\r\n", "\n")) == CONTRACT_HASH,
        "fixed normal AI contract changed",
    )?;
    let transcript = std::fs::read_to_string(directory.join("steps.jsonl"))?;
    let result_text = std::fs::read_to_string(directory.join("result.json"))?;
    let result: Value = serde_json::from_str(&result_text)?;
    require(
        result["passed"] == true
            && result.get("failure") == Some(&Value::Null)
            && result.get("test_mode") == Some(&Value::Null),
        "failed, incomplete or fast-forwarded probe",
    )?;
    let count = result["completed_steps"]
        .as_u64()
        .ok_or("completed step count missing")?;
    let run = directory
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or("artifact run ID missing")?;
    let (expected, restart, _) =
        persona_full::prepared_contract(CONTRACT, &transcript, count, Some(run))?;
    require(result["scenario"] == expected["name"], "wrong scenario")?;
    let checks = result["checks"].as_array().ok_or("missing judge checks")?;
    let mut names = BTreeSet::new();
    for check in checks {
        let name = check["name"].as_str().ok_or("judge name missing")?;
        require(
            check["passed"] == true && names.insert(name),
            "failed or duplicate judge",
        )?;
    }
    require(
        names == persona_full::CHECKS.into_iter().collect(),
        "strict judge coverage differs",
    )?;
    let logs: BTreeMap<_, _> = ["Lua.log", "Database.log", "Modding.log"]
        .into_iter()
        .map(|name| {
            std::fs::read_to_string(directory.join(name)).map(|text| (name.to_owned(), text))
        })
        .collect::<std::io::Result<_>>()?;
    for check in civ6_autotest_rs::judge::judge(&logs, "generic", None, true) {
        require(check.passed, "raw log judge failed")?;
    }
    let rows: Vec<Value> = transcript
        .lines()
        .filter(|l| !l.is_empty())
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    let steps = expected["steps"]
        .as_array()
        .ok_or("contract steps missing")?;
    require(
        rows.len() == steps.len() * 2 + 2,
        "missing, duplicate or mixed-run rows",
    )?;
    require(
        rows[0]["action"] == "start"
            && rows[0]["status"] == "OK"
            && rows[0]["detail"] == expected["name"],
        "incorrect run start",
    )?;
    let last = rows.last().ok_or("empty run")?;
    require(
        last["action"] == "judge"
            && last["status"] == "OK"
            && last["detail"] == "6/6 checks passed",
        "incorrect final judge",
    )?;
    let mut returns = BTreeMap::new();
    let mut save_name = None;
    for (index, step) in steps.iter().enumerate() {
        let action = step["action"].as_str().ok_or("missing action")?;
        let label = format!("{}/{} {action}", index + 1, steps.len());
        let started = &rows[1 + index * 2];
        let done = &rows[2 + index * 2];
        require(
            started["action"] == label
                && started["status"] == "START"
                && done["action"] == label
                && done["status"] == "OK",
            "step order or status differs",
        )?;
        let recorded: Value =
            serde_json::from_str(started["detail"].as_str().ok_or("missing START body")?)?;
        require(
            &recorded == step,
            "executed step differs from fixed contract",
        )?;
        let detail = done["detail"]
            .as_str()
            .ok_or("missing raw operation result")?;
        if action == "ui_lua" {
            let value = persona_full::lua_return(
                detail,
                step["state_name"].as_str().ok_or("missing Lua state")?,
            )?;
            // Indices always refer to the original 21-step contract.
            returns.insert(index - usize::from(restart), value.to_owned());
        } else if action == "save" {
            let name = detail
                .rsplit(['/', '\\'])
                .next()
                .ok_or("save result missing")?;
            let name = name
                .strip_suffix(".Civ6Save")
                .ok_or("physical save extension differs")?;
            let logical = step["name"].as_str().ok_or("logical save name missing")?;
            let suffix = name
                .strip_prefix(&format!("{logical}-"))
                .ok_or("physical save namespace differs")?;
            require(
                suffix.len() == 12 && suffix.bytes().all(|b| b.is_ascii_hexdigit()),
                "physical save nonce differs",
            )?;
            save_name = Some(name.to_owned());
        } else if action == "load" {
            require(
                detail.strip_prefix("loaded ") == save_name.as_deref(),
                "load did not use the same physical save",
            )?;
        }
    }
    let get = |index| {
        returns
            .get(&index)
            .map(String::as_str)
            .ok_or_else(|| format!("missing Lua return at original step {}", index + 1))
    };
    let audits = [
        snapshot(get(4)?, false)?,
        snapshot(get(6)?, true)?,
        snapshot(get(10)?, true)?,
        snapshot(get(15)?, true)?,
        snapshot(get(19)?, true)?,
    ];
    let t0 = audits[0];
    require(audits[1] == t0, "budget preparation advanced the turn")?;
    require(
        get(7)?
            .strip_prefix("actual turn baseline=")
            .ok_or("actual turn baseline missing")?
            .parse::<u64>()?
            == t0,
        "wrong first turn baseline",
    )?;
    let t1 = audits[2];
    require(
        t0.checked_add(1) == Some(t1) && get(9)? == "one actual turn observed",
        "first actual turn missing",
    )?;
    require(
        get(10)? == get(15)? && audits[3] == t1,
        "save/load native snapshot differs",
    )?;
    require(
        get(16)?
            .strip_prefix("restored actual turn baseline=")
            .ok_or("restored turn baseline missing")?
            .parse::<u64>()?
            == t1,
        "wrong restored turn baseline",
    )?;
    let t2 = audits[4];
    require(
        t1.checked_add(1) == Some(t2) && get(18)? == "one actual turn observed",
        "second actual turn missing",
    )?;
    let physical_save_name = save_name.ok_or("physical save missing")?;
    let proof_text = std::fs::read_to_string(directory.join("save-proof.json"))?;
    require(
        proof_text.trim_start().starts_with('{'),
        "save proof must be an object",
    )?;
    let proof: SaveProof = serde_json::from_str(&proof_text)?;
    require(
        proof.bytes == source_bytes && proof.sha256.eq_ignore_ascii_case(source_sha256),
        "artifact save differs from preserved source commitment",
    )?;
    require(
        proof.name == format!("{physical_save_name}.Civ6Save") && proof.bytes > 0,
        "physical save proof identity differs",
    )?;
    let physical = std::fs::read(directory.join(&proof.name))?;
    let physical_hash = format!("{:x}", Sha256::digest(&physical));
    require(
        physical.len() as u64 == proof.bytes && physical_hash == proof.sha256,
        "physical save bytes/hash differ",
    )?;
    Ok(Report {
        normal_unjoined_rejection: true,
        original_steps: 21,
        recorded_steps: steps.len(),
        preparation_restart: restart,
        ai_owners: 21,
        persona_definitions: 22,
        denied_pairs_per_audit: 462,
        native_audits: audits.len(),
        actual_turns: 2,
        turns: [t0, t1, t2],
        matched_save_load_snapshots: 1,
        physical_save_name,
        physical_save_bytes: proof.bytes,
        physical_save_sha256: physical_hash,
        contract_sha256: CONTRACT_HASH.into(),
        steps_sha256: hash(&transcript),
        result_sha256: hash(&result_text),
        product_acceptance: false,
        scope: "Normal unjoined rejection only; joined admission, AI payment, original113, MP and neutralization/recovery remain separate",
    })
}
