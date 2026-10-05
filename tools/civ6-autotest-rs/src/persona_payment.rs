//! Cross-check bounded Core promotion snapshots with an independent UI read.
//! This is diagnostic evidence only, never a product permission or AI test.
use crate::{Error, Result};
use civ6_native_bridge_rs::governor_payment::{
    PaymentDefinition, PromotionSnapshot, PROMOTION_WORDS,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MARKER: &str = "PERSONA_PAYMENT1595 ";

/// Runs inside the existing assignment observation, before the real assignment.
/// Every index and owned flag comes from this game's normal UI bindings.
pub const READ_COST_ONE: &str = r#"
local leader='LEADER_JAC_ASHINA_MORIKIYO'
local row=assert(GameInfo.JAC_LeaderPersonas[leader])
local d=assert(GameInfo.Governors[row.GovernorType])
local base=assert(GameInfo.GovernorPromotions[row.BasePromotionType])
local g=Players[0]:GetGovernors()
local gov=assert(g:GetGovernor(d.Hash))
local cities={}; for _,city in Players[0]:GetCities():Members() do cities[#cities+1]=city end
table.sort(cities,function(a,b) return a:GetID()<b:GetID() end)
local city=assert(cities[1])
local before=assert(Game:GetProperty('JAC_TEST853_ADMISSION_RECEIPT'))
assert(before.owner==0 and before.leader==leader and row.EvolutionCost==1)
local owned={}; local count=0
for promotion in GameInfo.GovernorPromotions() do
 assert(type(promotion.Index)=='number' and promotion.Index>=0 and promotion.Index%1==0)
 count=math.max(count,promotion.Index+1)
 if gov:HasPromotion(promotion.Hash) then owned[#owned+1]=promotion.Index end
end
table.sort(owned)
local can=g:CanAssignGovernor(d.Hash,city)
assert(type(can)=='boolean')
return 'PERSONA_PAYMENT1595 {"owner":0,"governor":'..d.Index..',"cost":1,"base":'..base.Index..
 ',"titles":[],"count":'..count..',"owned":['..table.concat(owned,',')..
 '],"paid":1,"beforeSpent":'..before.spent..',"spent":'..g:GetGovernorPointsSpent()..
 ',"city":'..city:GetID()..',"x":'..city:GetX()..',"y":'..city:GetY()..
 ',"canAssign":'..tostring(can)..'}'
"#;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Receipt {
    pub owner: u32,
    pub governor: u32,
    pub cost: u8,
    pub base: u32,
    pub titles: Vec<u32>,
    pub count: u32,
    pub owned: Vec<u32>,
    pub paid: u8,
    pub before_spent: u32,
    pub spent: u32,
    pub city: u32,
    pub x: u32,
    pub y: u32,
    pub can_assign: bool,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub owner: u32,
    pub governor: u32,
    pub cost: u8,
    pub paid: u8,
    pub matching_rows: usize,
    pub ui_calls: usize,
    pub core_update_calls: usize,
    pub fully_paid: bool,
    pub original_can_assign: bool,
    pub payment_authority: bool,
    pub scope: &'static str,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub created_filetime: u64,
}
#[derive(Debug)]
pub struct Evidence {
    pub receipt: Receipt,
    pub process: ProcessIdentity,
    pub artifact: String,
    pub artifact_path: std::path::PathBuf,
}

/// Bind the independent UI result to one observed process and a completed run.
pub fn evidence_from_steps(text: &str) -> Result<Evidence> {
    let mut process = None;
    let mut artifact = None;
    let mut artifact_path = None;
    let mut opened = false;
    let mut closed = false;
    let mut exited = false;
    let mut judged = false;
    let mut found = None;
    for line in text.lines() {
        require(!judged, "step records follow the final judge")?;
        let row: Value = serde_json::from_str(line)?;
        require(
            !matches!(row["status"].as_str(), Some("FAIL" | "ERROR")),
            "failed scenario cannot supply payment evidence",
        )?;
        if row["status"] != "OK" {
            continue;
        }
        let action = row["action"].as_str().unwrap_or("");
        let detail = row["detail"].as_str().unwrap_or("");
        if action.ends_with(" start_native_assign_calibration") {
            require(
                !opened && !closed && process.is_none(),
                "multiple assignment observations",
            )?;
            let (ready, raw) = detail
                .split_once("; process=")
                .ok_or_else(|| Error::input("native process identity absent from start receipt"))?;
            let name = ready
                .strip_prefix(
                    "native assign-calibration original-return telemetry ready; artifact ",
                )
                .filter(|s| s.ends_with(".jsonl") && !s.contains('/') && !s.contains('\\'))
                .ok_or_else(|| Error::input("assignment artifact name missing"))?;
            let identity: ProcessIdentity = serde_json::from_str(raw)?;
            require(
                identity.pid > 0 && identity.created_filetime > 0,
                "invalid observed process identity",
            )?;
            artifact = Some(name.to_owned());
            process = Some(identity);
            opened = true;
        } else if action.ends_with(" ui_lua")
            && detail.starts_with(&format!("ran Lua in InGame; return={MARKER}"))
        {
            require(
                opened && !closed && found.is_none(),
                "payment receipt outside its single observation",
            )?;
            let raw = detail
                .strip_prefix(&format!("ran Lua in InGame; return={MARKER}"))
                .unwrap();
            let (receipt, native_process) = raw
                .split_once("; native_process=")
                .ok_or_else(|| Error::input("UI read lacks its current pinned process"))?;
            let ui_identity: ProcessIdentity = serde_json::from_str(native_process)?;
            require(
                process.as_ref() == Some(&ui_identity),
                "UI receipt is from a different game process",
            )?;
            found = Some(serde_json::from_str(receipt)?);
        } else if action.ends_with(" stop_native_assign_calibration") {
            require(
                opened && !closed && found.is_some(),
                "assignment stopped without a payment receipt",
            )?;
            let name = detail
                .strip_prefix("native entry restored; artifact ")
                .unwrap_or("")
                .replace('\\', "/");
            require(
                name.rsplit('/').next() == artifact.as_deref(),
                "stop receipt belongs to a different artifact",
            )?;
            artifact_path = Some(std::path::PathBuf::from(name));
            closed = true;
        } else if action.ends_with(" exit_to_menu") {
            require(
                closed && !exited && detail == "main menu reached",
                "scenario did not finish after restoration",
            )?;
            exited = true;
        } else if action == "judge" {
            require(
                closed && exited,
                "judge preceded complete payment observation",
            )?;
            judged = true;
        } else if opened && !closed && action.ends_with(" restart_game") {
            return Err(Error::input("game restarted inside payment observation"));
        }
    }
    require(judged, "completed scenario judge missing")?;
    Ok(Evidence {
        receipt: found.unwrap(),
        process: process.unwrap(),
        artifact: artifact.unwrap(),
        artifact_path: artifact_path.unwrap(),
    })
}

fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::input(message))
    }
}
fn number(row: &Value, name: &str) -> Result<u64> {
    row[name]
        .as_u64()
        .ok_or_else(|| Error::input(format!("missing integer {name}")))
}
fn pointer(row: &Value, name: &str) -> Result<u64> {
    let raw = row[name]
        .as_str()
        .and_then(|s| s.strip_prefix("0x"))
        .filter(|s| s.len() == 16)
        .ok_or_else(|| Error::input(format!("invalid pointer {name}")))?;
    u64::from_str_radix(raw, 16).map_err(|_| Error::input(format!("invalid pointer {name}")))
}
fn ordered(indices: &[u32], count: u32) -> bool {
    count > 0
        && count <= 4096
        && indices.iter().all(|i| *i < count)
        && indices.windows(2).all(|pair| pair[0] < pair[1])
}

/// Requires the entire restored, lossless trace, and compares every observed
/// call for this person, including failed reads. No last-good-row selection.
pub fn verify(trace: &str, evidence: &Evidence) -> Result<Report> {
    let receipt = &evidence.receipt;
    require(
        receipt.owner < 64
            && receipt.governor < 256
            && (1..=3).contains(&receipt.cost)
            && (1..=receipt.cost).contains(&receipt.paid)
            && receipt.titles.len() == usize::from(receipt.cost - 1)
            && ordered(&receipt.owned, receipt.count),
        "invalid independent payment receipt",
    )?;
    require(
        receipt.before_spent.checked_add(u32::from(receipt.paid)) == Some(receipt.spent),
        "official spending delta disagrees with expected payment",
    )?;
    let definition = PaymentDefinition {
        governor_index: receipt.governor,
        cost: receipt.cost,
        base: receipt.base,
        titles: [
            receipt.titles.first().copied(),
            receipt.titles.get(1).copied(),
        ],
    };
    let mut identity = false;
    let mut installed = false;
    let mut restored = false;
    let mut stopped = false;
    let mut entries = 0usize;
    let mut matching = 0usize;
    let mut ui = 0usize;
    let mut core = 0usize;
    let mut target_identity = None;
    for line in trace.lines() {
        require(!stopped, "trace contains data after terminal record")?;
        let row: Value = serde_json::from_str(line)?;
        match row["kind"].as_str() {
            Some("identity") => {
                require(
                    !identity
                        && !installed
                        && entries == 0
                        && row["mode"] == "assign-calibration"
                        && row["gamecore_sha256"] == civ6_native_bridge_rs::PINNED_GAMECORE_SHA256
                        && row["entry_rva"] == 0x2dd880
                        && number(&row, "pid")? == u64::from(evidence.process.pid)
                        && number(&row, "created_filetime")? == evidence.process.created_filetime,
                    "wrong native identity",
                )?;
                identity = true;
            }
            Some("install") => {
                require(
                    identity && !installed && !restored && row["status"] == 0,
                    "invalid install",
                )?;
                installed = true;
            }
            Some("uninstall") => {
                require(
                    installed
                        && !restored
                        && entries == 0
                        && row["status"] == 0
                        && row["entry_restored"] == true
                        && row["prefix_retained_until_exit"] == false,
                    "entry not restored",
                )?;
                restored = true;
            }
            Some("assign_calibration") => {
                require(
                    restored
                        && number(&row, "sequence")? == entries as u64 + 1
                        && number(&row, "thread_id")? > 0
                        && row["payment_authority"] == false,
                    "invalid assignment sample",
                )?;
                let original = number(&row, "original_al")?;
                require(
                    original <= 1
                        && number(&row, "effective_al")? == original
                        && pointer(&row, "original_rax")? & 255 == original,
                    "original result changed",
                )?;
                entries += 1;
                // A failed identity read can erase the target's scalar owner
                // and index. Reject the entire small observation rather than
                // trying to reconstruct that person from stale pointers.
                require(
                    row["read_flags"] == 7 && row["promotion_snapshot_status"] == 0,
                    "observation contains an unreadable identity or promotion sample",
                )?;
                if row["player_id"] != receipt.owner || row["governor_index"] != receipt.governor {
                    continue;
                }
                require(
                    row["read_flags"] == 7
                        && row["promotion_snapshot_status"] == 0
                        && row["promotion_owner"] == receipt.owner
                        && row["promotion_index"] == receipt.governor
                        && row["promotion_count"] == receipt.count,
                    "Core person's promotion read failed",
                )?;
                let indices: Vec<u32> =
                    serde_json::from_value(row["owned_promotion_indices"].clone())?;
                require(
                    ordered(&indices, receipt.count) && indices == receipt.owned,
                    "UI and Core promotion flags differ",
                )?;
                let mut owned = [0; PROMOTION_WORDS];
                for index in &indices {
                    owned[*index as usize / 64] |= 1 << (*index % 64);
                }
                let snapshot = PromotionSnapshot {
                    owner: receipt.owner,
                    governor_index: receipt.governor,
                    count: receipt.count,
                    owned,
                };
                require(
                    snapshot.diagnostic_paid(receipt.owner, definition).ok() == Some(receipt.paid),
                    "Core person's payment differs",
                )?;
                require(
                    row["city_a8"] == receipt.city
                        && row["city_c0"] == receipt.x
                        && row["city_c4"] == receipt.y
                        && row["city_d8"] == receipt.owner
                        && original == u64::from(receipt.can_assign),
                    "city or assignment verdict differs",
                )?;
                let pointers = [
                    pointer(&row, "governors_this")?,
                    pointer(&row, "governor_pointer")?,
                    pointer(&row, "city_pointer")?,
                    pointer(&row, "player_address")?,
                ];
                require(
                    pointers.iter().all(|p| *p != 0)
                        && target_identity.is_none_or(|old| old == pointers),
                    "native identity changed within observation",
                )?;
                target_identity = Some(pointers);
                matching += 1;
                match number(&row, "return_site_rva")? {
                    0xba3e2 => ui += 1,
                    0x2dd375 => core += 1,
                    _ => {}
                }
            }
            Some("stop") => {
                require(
                    restored
                        && row["classification"] == "assign_calibration"
                        && row["result"] == "ok"
                        && row["rows"] == entries
                        && row["observed_calls"] == entries
                        && row["dropped"] == 0
                        && row["in_flight"] == 0
                        && row["snapshot_stable"] == true
                        && row["quiescence_proven"] == false
                        && row["counter_scope"] == "snapshot_observation_not_complete_call_history",
                    "native observation incomplete",
                )?;
                stopped = true;
            }
            _ => return Err(Error::input("unknown native observation record")),
        }
    }
    require(
        stopped && matching > 0 && ui > 0 && core > 0,
        "matching UI and Core assignment samples missing",
    )?;
    Ok(Report {
        owner: receipt.owner,
        governor: receipt.governor,
        cost: receipt.cost,
        paid: receipt.paid,
        matching_rows: matching,
        ui_calls: ui,
        core_update_calls: core,
        fully_paid: receipt.paid == receipt.cost,
        original_can_assign: receipt.can_assign,
        payment_authority: false,
        scope: "bounded_read_only_diagnostic_not_product_or_AI_acceptance",
    })
}
