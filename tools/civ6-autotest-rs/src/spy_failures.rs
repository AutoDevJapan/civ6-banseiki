//! Read two native failures without resetting the fixture or pairing anonymous captives.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Native/UI reads only. Existing fixture resolution also verifies the Governor's
/// real type, the saved unit reference and the original failed native history.
pub const READ: &str = r#"
local b=assert(ExposedMembers.JAC_PersonaBoundary,'boundary observer missing')
local s=b.ReadEnemy()
local c=assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_CONFIG'))
local retry=assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_SPY_RETRY'),'first failure archive missing')
local a=assert(retry.archive)
local e=assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_MISSION'),'second native failure event missing')
local diplomacy=Players[c.attacker]:GetDiplomacy()
local function mission(id)
 local m=assert(diplomacy:GetMission(c.attacker,id),'native mission absent')
 assert(type(m)=='table','native mission absent')
 return {attacker=c.attacker,id=id,operation=m.Operation,plot=m.PlotIndex,turn=m.CompletionTurn,initial=m.InitialResult,escape=m.EscapeResult}
end
local first=mission(a.eventID)
local second=mission(e.id)
local op=assert(GameInfo.UnitOperations['UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR'])
local arrays={map=true,offMap=true,captured=true,activeOffMap=true,capturedPlaceholderGroups=true,offMapIndices=true,captiveIndices=true,failures=true}
local function quote(text)
 return '"'..string.gsub(text,'[%z\1-\31\\"]',function(ch)
  return string.format('\\u%04x',string.byte(ch))
 end)..'"'
end
local function encode(v,array)
 if type(v)=='string' then return quote(v) end
 if type(v)=='number' then assert(v==v and math.abs(v)<math.huge); return string.format('%.17g',v) end
 if type(v)=='boolean' then return v and 'true' or 'false' end
 assert(type(v)=='table','unexpected native observation field')
 local parts={}
 if array then
  for _,item in ipairs(v) do parts[#parts+1]=encode(item,false) end
  return '['..table.concat(parts,',')..']'
 end
 local keys={}; for key in pairs(v) do assert(type(key)=='string'); keys[#keys+1]=key end; table.sort(keys)
 for _,key in ipairs(keys) do parts[#parts+1]=quote(key)..':'..encode(v[key],arrays[key]) end
 return '{'..table.concat(parts,',')..'}'
end
return encode({turn=s.turn,attacker=c.attacker,defender=c.owner,phase=b.EnemyProgress(),current=retry.current,
 firstSpyID=a.spyID,retrySpyID=retry.spyID,attemptArchive=a,
 spy=s.spy,native=s.native,capacity=diplomacy:GetSpyCapacity(),roster=s.spyRoster,
 defaultName=GameInfo.Units['UNIT_SPY'].Name,operationIndex=op.Index,operationHash=op.Hash,
 successUndetected=EspionageResultTypes.SUCCESS_UNDETECTED,successEscape=EspionageResultTypes.SUCCESS_MUST_ESCAPE,
 failureUndetected=EspionageResultTypes.FAIL_UNDETECTED,failureEscape=EspionageResultTypes.FAIL_MUST_ESCAPE,
 killed=EspionageResultTypes.KILLED,capturedResult=EspionageResultTypes.CAPTURED,noResult=EspionageResultTypes.NO_RESULT,
 archive={attacker=c.attacker,id=a.eventID,operation=a.operation,plot=a.plot,turn=a.completionTurn,initial=a.initial,escape=a.escape},
 event={attacker=e.attacker,id=e.id,turn=e.turn},failures={first,second}},false)
"#;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Captive {
    pub captor: i64,
    pub name: String,
    pub level: i64,
    pub name_index: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OffMap {
    pub name: String,
    pub level: i64,
    pub return_turn: i64,
    pub x: i64,
    pub y: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Roster {
    pub map: Vec<serde_json::Value>,
    pub off_map: Vec<OffMap>,
    pub captured: Vec<Captive>,
    pub active_off_map: Vec<OffMap>,
    pub physical_count: usize,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Failure {
    pub attacker: i64,
    pub id: i64,
    pub operation: i64,
    pub plot: i64,
    pub turn: i64,
    pub initial: i64,
    pub escape: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MissionEvent {
    pub attacker: i64,
    pub id: i64,
    pub turn: i64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Spy {
    pub owner: i64,
    pub id: i64,
    pub present: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Governor {
    pub neutralized: i64,
    pub established: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub turn: i64,
    pub attacker: i64,
    pub defender: i64,
    pub phase: String,
    pub current: i64,
    #[serde(rename = "firstSpyID")]
    pub first_spy_id: i64,
    #[serde(rename = "retrySpyID")]
    pub retry_spy_id: i64,
    pub capacity: usize,
    pub spy: Spy,
    pub native: Governor,
    pub roster: Roster,
    pub default_name: String,
    pub operation_index: i64,
    pub operation_hash: i64,
    pub success_undetected: i64,
    pub success_escape: i64,
    pub failure_undetected: i64,
    pub failure_escape: i64,
    pub killed: i64,
    pub captured_result: i64,
    pub no_result: i64,
    pub archive: Failure,
    pub event: MissionEvent,
    pub failures: Vec<Failure>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Next {
    NativeExchangeOrCapacity,
    AwaitNativeReturn,
    VerifyDistinctOperative,
    VerifyProduction,
}

/// This is a plan for prerequisites, never permission to resend the old attempt.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub raw: serde_json::Value,
    pub physical_count: usize,
    pub discounted_sentinels: usize,
    pub captive_identities_resolved: bool,
    pub neutralization_verified: bool,
    pub next: Next,
}

fn problem(message: &str) -> Error {
    Error::scenario(format!("native spy failure diagnosis: {message}"))
}
fn key(name: &str, level: i64, default: &str) -> (String, i64) {
    (if name == default { "" } else { name }.into(), level)
}

pub fn diagnose(raw: &str) -> Result<Report> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| problem(&format!("native JSON observation unavailable: {e}")))?;
    let o: Observation = serde_json::from_value(value.clone())
        .map_err(|e| problem(&format!("native observation incomplete: {e}")))?;
    if o.phase != "FAILED" || o.current != 2 || o.attacker == o.defender {
        return Err(problem("requires the preserved failed second attempt"));
    }
    if o.spy.present
        || o.spy.owner != o.attacker
        || o.spy.id < 0
        || o.first_spy_id < 0
        || o.retry_spy_id != o.spy.id
    {
        return Err(problem(
            "failed saved operative is still present or has changed owner",
        ));
    }
    if o.native.neutralized != 0 || !o.native.established {
        return Err(problem(
            "native Governor does not match the failed settled target",
        ));
    }
    if o.failures.len() != 2 || o.failures[0] != o.archive {
        return Err(problem("the first native failure differs from its archive"));
    }
    let first = &o.failures[0];
    let second = &o.failures[1];
    if first.id < 0
        || second.id <= first.id
        || first.turn < 0
        || second.turn <= first.turn
        || second.turn > o.turn
        || second.id != o.event.id
        || second.turn != o.event.turn
        || o.event.attacker != o.attacker
        || first.plot != second.plot
    {
        return Err(problem(
            "failure event, target or chronological history differs",
        ));
    }
    for failure in &o.failures {
        // Official EspionageSupport.lua:123-188. An escape still awaiting a
        // choice is not a finished failure, and unknown enums prove nothing.
        let completed_failure = if failure.initial == o.failure_undetected {
            failure.escape == o.no_result
        } else if failure.initial == o.failure_escape {
            [o.failure_escape, o.killed, o.captured_result].contains(&failure.escape)
        } else if [o.killed, o.captured_result].contains(&failure.initial) {
            failure.escape == o.no_result
        } else {
            false
        };
        if failure.attacker != o.attacker
            || failure.plot < 0
            || ![o.operation_index, o.operation_hash].contains(&failure.operation)
            || failure.initial == o.success_undetected
            || failure.initial == o.success_escape
            || !completed_failure
        {
            return Err(problem("mission is not a failed native neutralization"));
        }
    }
    if o.roster.map.len() > 256 || o.roster.off_map.len() > 256 || o.roster.captured.len() > 256 {
        return Err(problem("native roster exceeds its diagnostic bound"));
    }
    let mut captives = BTreeMap::<(String, i64), usize>::new();
    for captive in &o.roster.captured {
        if captive.captor < 0 || captive.level < 0 || captive.name_index < -1 {
            return Err(problem("native captive fields unavailable"));
        }
        *captives
            .entry(key(&captive.name, captive.level, &o.default_name))
            .or_default() += 1;
    }
    let mut sentinels = BTreeMap::<(String, i64), usize>::new();
    for record in &o.roster.off_map {
        if record.level < 0 {
            return Err(problem("native off-map rank unavailable"));
        }
        if record.return_turn == -1 && record.x == -9999 && record.y == -9999 {
            *sentinels
                .entry(key(&record.name, record.level, &o.default_name))
                .or_default() += 1;
        }
    }
    let active: Vec<_> = o
        .roster
        .off_map
        .iter()
        .filter(|record| {
            let group = key(&record.name, record.level, &o.default_name);
            !(record.return_turn == -1
                && record.x == -9999
                && record.y == -9999
                && captives
                    .get(&group)
                    .is_some_and(|count| sentinels.get(&group) == Some(count)))
        })
        .cloned()
        .collect();
    let physical = o.roster.map.len() + active.len() + o.roster.captured.len();
    if active != o.roster.active_off_map || physical != o.roster.physical_count {
        return Err(problem("Rust/native roster classification disagrees"));
    }
    if active
        .iter()
        .any(|record| record.return_turn <= o.turn || record.x < 0 || record.y < 0)
    {
        return Err(problem("unresolved off-map sentinel or overdue return"));
    }
    if physical > o.capacity {
        return Err(problem("operative count exceeds native capacity"));
    }
    let next = if !active.is_empty() {
        Next::AwaitNativeReturn
    } else if !o.roster.map.is_empty() {
        Next::VerifyDistinctOperative
    } else if physical >= o.capacity {
        Next::NativeExchangeOrCapacity
    } else {
        Next::VerifyProduction
    };
    Ok(Report {
        raw: value,
        physical_count: physical,
        discounted_sentinels: o.roster.off_map.len() - active.len(),
        captive_identities_resolved: false,
        neutralization_verified: false,
        next,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn observed() -> serde_json::Value {
        let sentinel = json!({"name":"","level":1,"returnTurn":-1,"x":-9999,"y":-9999});
        let captive = json!({"name":"","level":1,"nameIndex":-1,"captor":1});
        let first = json!({"attacker":0,"id":0,"operation":59,"plot":8511,"turn":23,"initial":1,"escape":-1});
        let second = json!({"attacker":0,"id":1,"operation":59,"plot":8511,"turn":45,"initial":1,"escape":-1});
        json!({"turn":45,"attacker":0,"defender":1,"phase":"FAILED","current":2,"capacity":2,"firstSpyID":655368,"retrySpyID":851976,
            "spy":{"owner":0,"id":851976,"present":false},"native":{"neutralized":0,"established":true},
            "defaultName":"LOC_UNIT_SPY_NAME","operationIndex":59,"operationHash":9959,"successUndetected":2,"successEscape":3,
            "failureUndetected":1,"failureEscape":4,"killed":5,"capturedResult":6,"noResult":-1,
            "archive":first,"event":{"attacker":0,"id":1,"turn":45},"failures":[first,second],
            "roster":{"map":[],"offMap":[sentinel,sentinel],"captured":[captive,captive],"activeOffMap":[],"physicalCount":2}})
    }
    #[test]
    fn live160_counts_two_without_pairing_people_or_discarding_native_history() {
        let input = observed();
        let report = diagnose(&input.to_string()).unwrap();
        assert_eq!((report.physical_count, report.discounted_sentinels), (2, 2));
        assert_eq!(report.raw, input);
        assert_eq!(report.next, Next::NativeExchangeOrCapacity);
        assert!(!report.neutralization_verified && !report.captive_identities_resolved);
    }
    #[test]
    fn repeated_failure_is_not_a_native_success_or_a_new_attempt() {
        for field in ["initial", "id", "turn", "operation", "plot", "attacker"] {
            let mut input = observed();
            input["failures"][1][field] = json!(match field {
                "initial" => 2,
                "id" => 0,
                "turn" => 23,
                "operation" => 991,
                "plot" => 1,
                _ => 1,
            });
            assert!(diagnose(&input.to_string()).is_err(), "{field}");
        }
        let mut input = observed();
        input["archive"]["escape"] = json!(4);
        assert!(diagnose(&input.to_string()).is_err());
    }
    #[test]
    fn unknown_initial_and_pending_or_unknown_escape_are_not_completed_failures() {
        for (initial, escape) in [(99, -1), (4, -1), (4, 99), (1, 99), (5, 99)] {
            let mut input = observed();
            input["failures"][1]["initial"] = json!(initial);
            input["failures"][1]["escape"] = json!(escape);
            assert!(diagnose(&input.to_string()).is_err(), "{initial}/{escape}");
        }
        for (initial, escape) in [(4, 4), (4, 5), (4, 6), (5, -1), (6, -1)] {
            let mut input = observed();
            input["failures"][1]["initial"] = json!(initial);
            input["failures"][1]["escape"] = json!(escape);
            assert!(diagnose(&input.to_string()).is_ok(), "{initial}/{escape}");
        }
    }
    #[test]
    fn real_travel_is_a_separate_native_operative() {
        let mut input = observed();
        input["capacity"] = json!(3);
        let travel = json!({"name":"","level":1,"returnTurn":49,"x":134,"y":29});
        input["roster"]["offMap"]
            .as_array_mut()
            .unwrap()
            .push(travel.clone());
        input["roster"]["activeOffMap"] = json!([travel]);
        input["roster"]["physicalCount"] = json!(3);
        assert_eq!(
            diagnose(&input.to_string()).unwrap().next,
            Next::AwaitNativeReturn
        );
    }
    #[test]
    fn ambiguous_sentinel_and_overdue_return_do_not_authorize_production() {
        let mut input = observed();
        input["roster"]["offMap"][0]["level"] = json!(2);
        assert!(diagnose(&input.to_string()).is_err());
        input["roster"]["activeOffMap"] = input["roster"]["offMap"].clone();
        input["roster"]["physicalCount"] = json!(4);
        assert!(diagnose(&input.to_string())
            .unwrap_err()
            .to_string()
            .contains("unresolved"));
    }
    #[test]
    fn present_recycled_unit_or_neutralized_governor_requires_another_diagnostic() {
        let mut input = observed();
        input["spy"]["present"] = json!(true);
        assert!(diagnose(&input.to_string()).is_err());
        let mut input = observed();
        input["native"]["neutralized"] = json!(6);
        assert!(diagnose(&input.to_string()).is_err());
    }
}
