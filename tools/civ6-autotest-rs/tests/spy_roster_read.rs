//! Modeled context tests for raw collection, never proof of a native outcome.
#[allow(dead_code)]
#[path = "../examples/group-roster-read-62.rs"]
mod host;
#[path = "../src/spy_roster_read.rs"]
mod observer;
use mlua::Lua;
use serde_json::Value;

const NATIVE: &str = r#"
_G=nil; WRITES=0; SENDS=0; STEPS=0;
Game={GetCurrentGameTurn=function() return 62 end};
PROPERTIES={JAC_BOUNDARY_THIRD_SPY_ARCHIVE={histories={{id=0,turn=23},{id=1,turn=45}}},
 JAC_BOUNDARY_THIRD_MISSION_ACK={observed={mission={id=2,turn=62}}}};
function Game:GetProperty(k) return PROPERTIES[k] end
function Game:SetProperty() WRITES=WRITES+1; error('observer wrote') end
GameInfo={Units={[18]={Index=18,UnitType='UNIT_SPY',Spy=true},
 [19]={Index=19,UnitType='UNIT_WARRIOR',Spy=false}}};
GameInfo.Units.UNIT_SPY=GameInfo.Units[18];
local function unit(id,t,x,y)
 local u={};
 function u:GetID() return id end; function u:GetOwner() return 0 end
 function u:GetType() return t end; function u:GetName() return '' end
 function u:GetX() return x end; function u:GetY() return y end
 function u:GetPlotId() return -1 end
 return u
end
UNITS={unit(4,19,140,40),unit(1048586,18,-9999,-9999)};
local units={}; function units:Members()
 local i=0; return function()
  i=i+1; if ENUM_ERROR and i==2 then error('mid-enumeration failure') end
  if UNITS[i] then return i,UNITS[i] end
 end
end
DIP={}; function DIP:GetSpyCapacity() return 3 end
function DIP:GetNumSpiesOffMap() return 3 end
function DIP:GetNthOffMapSpy(owner,i)
 return {Name='',Level=1,ReturnTurn=-1,XLocation=-9999,YLocation=-9999}
end
function DIP:GetNumSpiesCaptured() return 3 end
function DIP:GetNthCapturedSpy(owner,i)
 return {OwningPlayer=0,Name='',Level=1,NameIndex=-1}
end
function DIP:GetMission(owner,i)
 return {Operation=59,PlotIndex=8511,CompletionTurn=({23,45,62})[i+1],
  InitialResult=1,EscapeResult=-1}
end
Players={}; for i=0,1 do
 Players[i]={GetUnits=function() return units end,GetDiplomacy=function() return DIP end}
end
ExposedMembers={JAC_PersonaBoundary={ReadEnemy=function(...)
 assert(select('#',...)==0,'public dot-call received self');
 return {turn=62,attacker=0,owner=1,spyRoster={map={},captured={
  {name='',level=1,nameIndex=-1},{name='',level=1,nameIndex=-1},{name='',level=1,nameIndex=-1}},
  offMap={{name='',level=1,x=-9999,y=-9999,returnTurn=-1},
   {name='',level=1,x=-9999,y=-9999,returnTurn=-1},
   {name='',level=1,x=-9999,y=-9999,returnTurn=-1}},physicalCount=3}}
end}};
DealManager={SendWorkingDeal=function() SENDS=SENDS+1; error('observer sent') end};
"#;

fn read(setup: &str, ui: bool) -> (Lua, Value) {
    let lua = Lua::new();
    assert_eq!(
        lua.load("return _VERSION").eval::<String>().unwrap(),
        "Lua 5.1"
    );
    lua.load(NATIVE).exec().unwrap();
    lua.load(setup).exec().unwrap();
    let script = if ui {
        observer::ui_read()
    } else {
        observer::core_read()
    };
    let raw: String = lua.load(script).eval().unwrap();
    let value: Value = serde_json::from_str(&raw).unwrap();
    lua.load("assert(WRITES==0 and SENDS==0 and STEPS==0); assert(PROPERTIES.JAC_BOUNDARY_THIRD_SPY_ARCHIVE.histories[1].turn==23 and PROPERTIES.JAC_BOUNDARY_THIRD_SPY_ARCHIVE.histories[2].turn==45 and PROPERTIES.JAC_BOUNDARY_THIRD_MISSION_ACK.observed.mission.id==2)").exec().unwrap();
    (lua, value)
}

#[test]
fn records_all_units_and_spy_field_types_without_assuming_map_or_captive_identity() {
    let (_, v) = read("", false);
    assert_eq!(v["unitCount"], 2);
    assert_eq!(
        v["units"]["1"]["definition"]["Spy"],
        serde_json::json!({"kind":"boolean","value":false})
    );
    assert_eq!(v["units"]["2"]["GetID"]["value"], 1048586);
    assert_eq!(v["units"]["2"]["GetX"]["value"], -9999);
    assert_eq!(v["units"]["2"]["level"]["status"], "UNAVAILABLE");
    assert_eq!(v["diplomacy"]["captured"].as_object().unwrap().len(), 3);
    assert_eq!(v["diplomacy"]["offMap"].as_object().unwrap().len(), 3);
    assert!(v.get("accepted").is_none());
}

#[test]
fn preserves_numeric_zero_and_unknown_spy_definitions_separately() {
    for (setup, kind, value) in [
        (
            "GameInfo.Units[19].Spy=0",
            "number",
            Some(serde_json::json!(0)),
        ),
        ("GameInfo.Units[19].Spy=nil", "nil", None),
        (
            "GameInfo.Units[19].Spy='0'",
            "string",
            Some(serde_json::json!("0")),
        ),
    ] {
        let (_, v) = read(setup, false);
        assert_eq!(v["units"]["1"]["definition"]["Spy"]["kind"], kind);
        assert_eq!(
            v["units"]["1"]["definition"]["Spy"].get("value"),
            value.as_ref()
        );
    }
}

#[test]
fn keeps_partial_rows_and_native_errors_instead_of_losing_evidence() {
    let (_, v) = read(
        "ENUM_ERROR=true; DIP.GetSpyCapacity=function() error('native capacity failure') end",
        false,
    );
    assert_eq!(v["enumeration"]["status"], "ERROR");
    assert_eq!(v["units"].as_object().unwrap().len(), 1);
    assert_eq!(v["diplomacy"]["capacity"]["status"], "ERROR");
    assert_eq!(
        v["diplomacy"]["histories"]["2"]["fields"]["CompletionTurn"]["value"],
        62
    );
}

#[test]
fn preserves_fractions_and_json_string_controls_exactly() {
    let (_,v)=read("UNITS[1].GetX=function() return 140.125 end; UNITS[1].GetName=function() return '改行\\n引用\"と\\\\' end",false);
    assert_eq!(v["units"]["1"]["GetX"]["value"], 140.125);
    assert_eq!(v["units"]["1"]["GetName"]["value"], "改行\n引用\"と\\");
}

#[test]
fn reads_ui_roster_in_its_own_context_without_publishing_or_pairing() {
    let (_, v) = read("", true);
    assert_eq!(v["context"], "InGame");
    assert_eq!(v["boundary"]["read"]["status"], "OK");
    assert_eq!(
        v["boundary"]["roster"]["captured"]["entries"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(v["boundary"]["roster"]["physicalCount"], 3);
    let (_,v)=read("ExposedMembers.JAC_PersonaBoundary.ReadEnemy=function() error('UI observation failure') end",true);
    assert_eq!(v["boundary"]["read"]["status"], "ERROR");
    assert_eq!(v["unitCount"], 2);
}

#[test]
fn host_has_only_pinned_load_two_reads_and_exit() {
    let v = host::scenario();
    let steps = v["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 5);
    assert_eq!(
        steps[0]["name"],
        "JAC-Enemy-Neutralization-ad65ffec7ebf491c8c2057b227df2304-0000000000000010"
    );
    assert_eq!(steps[2]["state_name"], "InGame");
    assert_eq!(steps[3]["state_name"], "GameCore_Tuner");
    assert_eq!(steps[4]["action"], "exit_to_menu");
    for step in &steps[2..4] {
        let code = step["code"].as_str().unwrap();
        for forbidden in [
            "SetProperty",
            "SendWorkingDeal",
            "RequestOperation",
            "JAC_GroupExchange548=",
            "end_turn",
        ] {
            assert!(!code.contains(forbidden), "observer contains {forbidden}");
        }
    }
}

#[test]
fn independent_rust_counts_lua_numeric_zero_without_treating_it_as_a_spy() {
    let (_, ui) = read("", true);
    let (_, core) = read("GameInfo.Units[19].Spy=0", false);
    let report = observer::inspect_pair(&ui, &core).unwrap();
    assert_eq!(report["contexts"]["InGame"]["legacyLuaTruthyCount"], 1);
    assert_eq!(
        report["contexts"]["GameCore_Tuner"]["legacyLuaTruthyCount"],
        2
    );
    assert_eq!(
        report["contexts"]["GameCore_Tuner"]["knownSpyTypeRows"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        report["contexts"]["GameCore_Tuner"]["knownSpyTypeRows"][0]["raw"]["GetID"]["value"],
        1048586
    );
    assert_eq!(report["exchangeAuthorized"], false);
    assert_eq!(report["coreRaw"], core);
}

#[test]
fn rust_refuses_fractional_counts_partial_enumeration_and_wrong_turns() {
    let (_, ui) = read("", true);
    let (_, core) = read("", false);
    for (field, value) in [
        ("unitCount", serde_json::json!(2.1)),
        ("enumeration", serde_json::json!({"status":"ERROR"})),
        ("turn", serde_json::json!({"value":45})),
    ] {
        let mut changed = core.clone();
        changed[field] = value;
        assert!(observer::inspect_pair(&ui, &changed).is_err());
    }
}

fn complete_records() -> Vec<Value> {
    let (_, ui) = read("", true);
    let (_, core) = read("", false);
    let scene = host::scenario();
    let mut records =
        vec![serde_json::json!({"action":"start","status":"OK","detail":scene["name"]})];
    for (index, step) in scene["steps"].as_array().unwrap().iter().enumerate() {
        let action = format!("{}/5 {}", index + 1, step["action"].as_str().unwrap());
        let detail = match step["action"].as_str().unwrap() {
            "load" => format!("loaded {}", step["name"].as_str().unwrap()),
            "wait_for" => format!("condition met: {}", step["condition"]),
            "ui_lua" => {
                let context = step["state_name"].as_str().unwrap();
                let raw = if context == "InGame" { &ui } else { &core };
                format!("ran Lua in {context}; return={raw}")
            }
            "exit_to_menu" => "exited to menu".into(),
            _ => unreachable!(),
        };
        records
            .push(serde_json::json!({"action":action,"status":"START","detail":step.to_string()}));
        records.push(serde_json::json!({"action":action,"status":"OK","detail":detail}));
    }
    records.push(serde_json::json!({"action":"judge","status":"OK","detail":"6/6 checks passed"}));
    records
}

#[test]
fn transcript_requires_exact_script_provenance_and_unique_returns() {
    let records = complete_records();
    let text = records
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let (ui, core) = host::observations(&text).unwrap();
    assert!(observer::inspect_pair(&ui, &core).is_ok());
    assert!(host::observations(&format!("{text}\n{}", records[7])).is_err());
    assert!(host::observations(
        &text.replace("local CONTEXT='GameCore_Tuner'", "local CONTEXT='Other'")
    )
    .is_err());
    for changed in [
        records[2..].to_vec(),
        [records[0..2].to_vec(), records[4..].to_vec()].concat(),
        [
            records[0..4].to_vec(),
            records[6..8].to_vec(),
            records[4..6].to_vec(),
            records[8..].to_vec(),
        ]
        .concat(),
    ] {
        assert!(host::observations(
            &changed
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
        .is_err());
    }
    assert!(host::observations(&text.replace(
        "ad65ffec7ebf491c8c2057b227df2304",
        "e35efb41263041e0a763128ee8a4c165"
    ))
    .is_err());
}

#[test]
fn mixed_numeric_and_string_keys_keep_both_values_and_key_types() {
    let (_, v) = read(
        "PROPERTIES.JAC_BOUNDARY_THIRD_SPY_ARCHIVE.mixed={[1]='number-key',['1']='string-key'}",
        false,
    );
    let mixed = &v["properties"]["JAC_BOUNDARY_THIRD_SPY_ARCHIVE"]["value"]["mixed"];
    assert_eq!(mixed["kind"], "lua_table_entries");
    assert_eq!(
        mixed["entries"][0]["key"],
        serde_json::json!({"kind":"number","value":1})
    );
    assert_eq!(mixed["entries"][0]["value"], "number-key");
    assert_eq!(
        mixed["entries"][1]["key"],
        serde_json::json!({"kind":"string","value":"1"})
    );
    assert_eq!(mixed["entries"][1]["value"], "string-key");
}

#[test]
fn partial_ui_read_is_explicit_in_the_independent_report() {
    let (_, ui) = read(
        "ExposedMembers.JAC_PersonaBoundary.ReadEnemy=function() error('UI failed') end",
        true,
    );
    let (_, core) = read("", false);
    let report = observer::inspect_pair(&ui, &core).unwrap();
    assert_eq!(report["captureState"], "PARTIAL_UI_READ_FAILURE");
    assert_eq!(report["partial"], true);
    assert_eq!(report["exchangeAuthorized"], false);
    assert_eq!(report["uiRaw"], ui);
}

#[test]
fn real_runner_startup_records_are_paired_once_before_the_pinned_load() {
    let records = complete_records();
    let text = |rows: &[Value]| {
        rows.iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    };
    for (action, detail) in [
        ("launch", "Civ VI is not running"),
        ("auto_restart", "Civ6 is not running"),
        (
            "auto_restart",
            "mod file newer than Civ6 process: owned/modinfo (mtime=1.000, started=0.000)",
        ),
        (
            "1/6 restart_game",
            "{\"action\":\"restart_game\",\"timeout\":420}",
        ),
    ] {
        let start = serde_json::json!({"action":action,"status":"START","detail":detail});
        let ok = serde_json::json!({"action":action,"status":"OK","detail":"Civ VI relaunched via Steam; main menu visible and ready"});
        assert!(host::observations(&text(
            &[
                records[0..1].to_vec(),
                vec![start.clone(), ok.clone()],
                records[1..].to_vec()
            ]
            .concat()
        ))
        .is_ok());
        for prefix in [
            vec![start.clone()],
            vec![ok.clone()],
            vec![start.clone(), ok.clone(), start.clone(), ok.clone()],
            vec![
                start.clone(),
                serde_json::json!({"action":action,"status":"FAIL","detail":"failed"}),
            ],
        ] {
            assert!(host::observations(&text(
                &[records[0..1].to_vec(), prefix, records[1..].to_vec()].concat()
            ))
            .is_err());
        }
        assert!(host::observations(&text(
            &[
                records[0..2].to_vec(),
                vec![start, ok],
                records[2..].to_vec()
            ]
            .concat()
        ))
        .is_err());
    }
    let skipped = serde_json::json!({"action":"auto_restart","status":"SKIP","detail":"Civ VI process is newer than all scenario mod files"});
    assert!(host::observations(&text(
        &[
            records[0..1].to_vec(),
            vec![skipped.clone()],
            records[1..].to_vec()
        ]
        .concat()
    ))
    .is_ok());
    assert!(host::observations(&text(
        &[
            records[0..1].to_vec(),
            vec![skipped.clone(), skipped],
            records[1..].to_vec()
        ]
        .concat()
    ))
    .is_err());
    assert!(host::observations(&text(
        &[
            vec![
                serde_json::json!({"action":"unknown_startup","status":"START","detail":"not JSON"})
            ],
            records
        ]
        .concat()
    ))
    .is_err());
}

#[test]
fn numeric_only_tables_preserve_key_types_and_fractional_keys() {
    let (_, v) = read(
        "PROPERTIES.JAC_BOUNDARY_THIRD_SPY_ARCHIVE.fractions={[1.25]='quarter',[2.5]='half'}",
        false,
    );
    let rows = &v["properties"]["JAC_BOUNDARY_THIRD_SPY_ARCHIVE"]["value"]["fractions"];
    assert_eq!(
        rows["entries"][0]["key"],
        serde_json::json!({"kind":"number","value":1.25})
    );
    assert_eq!(
        rows["entries"][1]["key"],
        serde_json::json!({"kind":"number","value":2.5})
    );
    let histories = &v["properties"]["JAC_BOUNDARY_THIRD_SPY_ARCHIVE"]["value"]["histories"];
    assert_eq!(
        histories["entries"][0]["key"],
        serde_json::json!({"kind":"number","value":1})
    );
    assert_eq!(histories["entries"][0]["value"]["turn"], 23);
}

#[test]
fn header_and_positive_final_judge_are_required_not_an_exit_fragment() {
    let records = complete_records();
    let text = |r: &[Value]| {
        r.iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert!(host::observations(&text(&records)).is_ok());
    assert!(host::observations(&text(&records[1..])).is_err());
    assert!(host::observations(&text(&records[..records.len() - 1])).is_err());
    for detail in ["0/0 checks passed", "5/6 checks passed", "bad"] {
        let mut changed = records.clone();
        changed.last_mut().unwrap()["detail"] = detail.into();
        assert!(host::observations(&text(&changed)).is_err());
    }
    let mut changed = records.clone();
    changed[0]["detail"] = "different scenario".into();
    assert!(host::observations(&text(&changed)).is_err());
    for extra in [
        serde_json::json!({"action":"unknown","status":"SKIP","detail":"unknown"}),
        serde_json::json!({"action":"launch","status":"OK","detail":"Civ VI relaunched via Steam"}),
        records.last().unwrap().clone(),
    ] {
        let mut changed = records.clone();
        changed.push(extra.clone());
        assert!(host::observations(&text(&changed)).is_err());
        let mut changed = records.clone();
        changed.insert(1, extra);
        assert!(host::observations(&text(&changed)).is_err());
    }
}
