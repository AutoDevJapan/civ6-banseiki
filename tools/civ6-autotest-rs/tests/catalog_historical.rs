#[allow(dead_code)]
#[path = "fixtures/catalog-scenario-cba.rs"]
mod catalog;
// Lua 5.1 archived engine model tests, not live/native qualification evidence.
// Historical private qualification uses its fixed bd252 Gameplay source.
// Normal startup/qualification and private-flag rejection load shipped
// Gameplay separately. Engine getters use an independent ledger/unit/title
// oracle; they do not read the producer's admission property.
use civ6_autotest_rs::{payment_scenario::READ_CLOSED_ADMISSION, repo_root};
use mlua::Lua;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const LEGACY_GAMEPLAY: &str = include_str!("fixtures/catalog-gameplay-bd252.lua");

fn template() -> Value {
    serde_json::from_str(catalog::NORMAL).unwrap()
}
fn run(lua: &Lua, code: &str) -> String {
    lua.load(code).eval().unwrap()
}

fn model() -> Lua {
    model_with_startup("", false).unwrap()
}

fn model_with_startup(engine_fault: &str, normal: bool) -> mlua::Result<Lua> {
    let lua = Lua::new();
    let configuration = template();
    let players = configuration["steps"][1]["players"].as_array().unwrap();
    let list = |field: &str, prefix: &str| {
        players
            .iter()
            .map(|p| {
                format!(
                    "'{}'",
                    p[field].as_str().unwrap().strip_prefix(prefix).unwrap()
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    let stub = std::fs::read_to_string(
        repo_root().join("tools/civ6-lua-tests-rs/fixtures/persona/stubs.lua"),
    )
    .unwrap()
    .replace(
        "local names = {'ODA_NOBUHIDE','DATE_TANEMUNE','HOJO_UJITSUNA'}",
        &format!("local names = {{{}}}", list("leader", "LEADER_JAC_")),
    )
    .replace(
        "local civs = {'ODA','DATE','HOJO'}",
        &format!("local civs = {{{}}}", list("civ", "CIVILIZATION_JAC_")),
    );
    lua.load(stub).exec().unwrap();
    lua.load("GameConfiguration=GameConfiguration or {}; GameConfiguration.GetStartTurn=function() return 0 end").exec().unwrap();
    lua.load(r#"
IDS={}; DELAY=false; UNKNOWN=false; WRITES={}; CIVIC_COMPLETIONS=0; UI_EVER_CALLS={}
RAW={}; print=function(value) RAW[#RAW+1]=tostring(value) end
for id=0,21 do
 IDS[#IDS+1]=id
 local row=ROWS[id+1]; row.EvolutionCost=id%3+1
 GOVDEFS[id+1].Index=17+id*7
 local player=Players[id]; player.human=id==0
 local completed={}
 player.GetCulture=function() return {
  HasCivic=function(_,index) return completed[index]==true end,
  GetCultureCost=function() return 100 end,
  SetCulturalProgress=function(_,index,amount)
   assert(amount==100 and not completed[index]); completed[index]=true
   CIVIC_COMPLETIONS=CIVIC_COMPLETIONS+1
   if not DELAY then player.points=player.points+1 end
  end
 } end
 local original=player.GetGovernors
 player.GetGovernors=function()
  local g=original()
  local function allowed(hash)
   assert(hash>=1000,'Hash required')
   if UNKNOWN then return nil end
   local selected
   for n,d in ipairs(GOVDEFS) do if d.Hash==hash then selected=ROWS[n] end end
   assert(selected)
   local prefix='JAC_PERSONA_'..selected.LeaderType..'_'
   local owner=Game:GetProperty(prefix..'OWNER')
   if owner~=id or Game:GetProperty(prefix..'SOURCE')==id or Game:GetProperty(prefix..'STATE')~='EVOLVING' then return false end
   local unit=NATIVE_UNITS[id][Game:GetProperty(prefix..'UNIT')]
   return unit~=nil and player.points-player.spent>=selected.EvolutionCost
  end
  g.CanEverAppointGovernor=function(_,hash)
   local value=allowed(hash); UI_EVER_CALLS[#UI_EVER_CALLS+1]={owner=id,hash=hash,value=value}; return value
  end
  g.CanAppointGovernor=function(_,hash) return allowed(hash) end
  if CORE_CONTEXT then
   g.CanEverAppointGovernor=nil; g.CanAppointGovernor=nil; g.HasGovernor=nil
  end
  return g
 end
 local write=player.SetProperty
 player.SetProperty=function(self,k,v) WRITES[#WRITES+1]={owner=id,key=k,value=v}; write(self,k,v) end
end
PlayerManager.GetAliveMajorIDs=function() return IDS end
GameInfo.Governors=collection(GOVDEFS,'GovernorType')
local civicRows,official={},{}
for n=1,15 do civicRows[n]={CivicType='CIVIC_TEST_'..n}; official[n]={CivicType='CIVIC_TEST_'..n,Index=n} end
GameInfo.JAC_PersonaTitleCivics=collection(civicRows,'CivicType')
GameInfo.Civics=collection(official,'CivicType')
-- Normal Gameplay now validates the complete official promotion catalog and
-- captures a turn-zero, all-closed baseline. The archived three-person mock's
-- dense promotion indices collide when expanded to 22; keep the same native
-- hashes and independent eligibility oracle, but supply valid engine indices
-- and explicit membership rather than fabricating a saved receipt.
local sets={}
for index,promotion in ipairs(PROMOS) do
 promotion.Index=512+index; promotion.BaseAbility=0
 for _,row in ipairs(ROWS) do
  if promotion.GovernorPromotionType==row.BasePromotionType or
     promotion.GovernorPromotionType:sub(1,#row.BasePromotionType+7)==row.BasePromotionType..'_TITLE_' then
   if promotion.GovernorPromotionType==row.BasePromotionType then promotion.BaseAbility=1 end
   sets[#sets+1]={GovernorType=row.GovernorType,GovernorPromotion=promotion.GovernorPromotionType}
  end
 end
end
GameInfo.GovernorPromotions=collection(PROMOS,'GovernorPromotionType')
GameInfo.GovernorPromotionSets=setmetatable({},{__call=function() local i=0; return function() i=i+1; return sets[i] end end})
TURN=0
"#).exec().unwrap();
    lua.load(engine_fault).exec()?;
    for path in [
        "poc/JAC_PoC_LeaderPersona/Shared/JAC_PersonaTreeRules.lua",
        "poc/JAC_PoC_LeaderPersona/UI/JAC_LeaderPersona_View.lua",
        "poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua",
    ] {
        let script = if !normal && path.ends_with("JAC_LeaderPersona_Gameplay.lua") {
            LEGACY_GAMEPLAY.to_owned()
        } else {
            std::fs::read_to_string(repo_root().join(path)).unwrap()
        };
        lua.load(script).set_name(path).exec()?;
    }
    // Only shipped initialization can create the normal acquisition baseline.
    // Resume the archived scenario at turn one after it has been acknowledged.
    if normal {
        lua.load("assert(type(Game:GetProperty('JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1'))=='table','normal catalog startup failed')").exec()?;
    } else {
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(LEGACY_GAMEPLAY.replace("\r\n", "\n"))
            ),
            "45d9966849284ba9cfd64e1217b337582dcbbe8dfd059bee1097b7015daa58be",
            "historical Gameplay must stay bound to bd252"
        );
        lua.load("assert(Game:GetProperty('JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1')==nil,'historical source mixed with normal authority')").exec()?;
    }
    lua.load("TURN=1").exec()?;
    // A call to the shipped UI reader runs in its own UI context. The Core
    // fixture intentionally has no CanEver/CanAppoint bindings.
    lua.load(
        r#"
local uiRead=ExposedMembers.JAC_LeaderPersonaNativeRead
ExposedMembers.JAC_LeaderPersonaNativeRead=function(...)
 local prior=CORE_CONTEXT; CORE_CONTEXT=false
 local value=uiRead(...); CORE_CONTEXT=prior
 return value
end
"#,
    )
    .exec()
    .unwrap();
    Ok(lua)
}

#[test]
fn invalid_initial_engine_catalog_cannot_create_normal_receipts() {
    for engine_fault in [
        "TURN=1",
        "GameInfo.GovernorPromotionSets=nil",
        "PROMOS[2].Index=PROMOS[1].Index; GameInfo.GovernorPromotions=collection(PROMOS,'GovernorPromotionType')",
        "PROMOS[1].BaseAbility=0",
        "PlayerConfigurations[21].civ='WRONG'",
        "Players[21].spent=1",
    ] {
        assert!(
            model_with_startup(engine_fault, true).is_err(),
            "invalid engine startup was accepted: {engine_fault}"
        );
    }
}

#[test]
fn current_normal_all_22_owners_keep_independent_qualification_without_private_flags() {
    let lua = model_with_startup("", true).unwrap();
    // The original title-grant operation is preserved. This model exercise
    // omits its two historical UI pause flags and does not claim to execute
    // the legacy zero-operation live scenario on current Gameplay.
    let titles = catalog::prepare_code()
        .replace("Game:SetProperty('JAC_TEST853_ADMISSION_ACTIVE',1)", "")
        .replace(
            "Game:SetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT',1)",
            "",
        );
    run(&lua, &titles);
    for source in 0..22 {
        let row = template()["steps"][1]["players"][source]["leader"]
            .as_str()
            .unwrap()
            .to_owned();
        let owner = (source + 1) % 22;
        lua.load(format!(
            "local api=ExposedMembers.JAC_LeaderPersona; assert(api.Join({owner},{source})=='JOINED'); assert(api.PrepareEvolution({owner},'{row}')=='EVOLVING')"
        )).exec().unwrap();
    }
    let core = run(&lua, &catalog::matrix_code(true));
    assert!(core.contains("MATRIX identities=22"));
    // Run the same complete matrix through the independent UI getter oracle.
    let ui = catalog::matrix_code(true).replacen("local core=true", "local core=false", 1);
    let ui_receipt = run(&lua, &ui);
    assert_eq!(core, ui_receipt, "Core/UI 22-person receipts differ");
    lua.load("assert(#UI_EVER_CALLS==484 and SPAWNS==22 and DESTROYS==0 and #REQUESTS==0); assert(Game:GetProperty('JAC_TEST853_ADMISSION_ACTIVE')==nil and Game:GetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT')==nil)").exec().unwrap();
}

#[test]
fn current_shipped_publication_rejects_the_historical_private_pause_contract() {
    for flag in [
        "JAC_TEST853_ADMISSION_ACTIVE",
        "JAC_TEST853_ADMISSION_MANUAL_APPOINT",
    ] {
        for value in ["1", "false"] {
            let lua = model_with_startup("", true).unwrap();
            let titles = catalog::prepare_code()
                .replace("Game:SetProperty('JAC_TEST853_ADMISSION_ACTIVE',1)", "")
                .replace(
                    "Game:SetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT',1)",
                    "",
                );
            run(&lua, &titles);
            lua.load(format!(r#"
local api=ExposedMembers.JAC_LeaderPersona; local leader='LEADER_JAC_HOJO_UJITSUNA'
assert(api.Join(1,0)=='JOINED' and api.PrepareEvolution(1,leader)=='EVOLVING')
assert(Players[1]:GetProperty(901017)==true)
local get=Players[1].GetProperty; local closes={{}}; local before=#WRITES
Players[1].GetProperty=function(self,key)
 if key==902001 or key==907001 then closes[key]=(closes[key] or 0)+1 end
 return get(self,key)
end
Game:SetProperty('{flag}',{value}); api.Reconcile()
assert((closes[902001] or 0)>0 and (closes[907001] or 0)>0,'both native close requests required')
for index=before+1,#WRITES do local write=WRITES[index]
 assert(not (write.owner==1 and (write.key==901017 or write.key==905017) and write.value==true),'private flag republished eligibility')
end
assert(SPAWNS==1 and DESTROYS==0 and Players[1].spent==0 and #REQUESTS==0)
"#)).exec().unwrap();
        }
    }
}

#[test]
fn generated_normal_setup_mode_and_zero_operation_contract_validate() {
    let data = catalog::build(&template()).unwrap();
    civ6_autotest_rs::scenario::validate_scenario(&data).unwrap();
    let steps = data["steps"].as_array().unwrap();
    assert_eq!(steps[2], template()["steps"][1]);
    assert_eq!(
        steps[2]["players"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["human"] == true)
            .count(),
        1
    );
    let mut joins = 0;
    let mut evolves = 0;
    let lua = Lua::new();
    let start = steps
        .iter()
        .position(|s| s["action"] == "start_native_production_admission")
        .unwrap();
    assert_eq!(steps[start + 1]["code"], READ_CLOSED_ADMISSION);
    assert_eq!(steps[start]["max_seconds"], 300);
    for step in steps {
        assert!(!matches!(
            step["action"].as_str(),
            Some("end_turn" | "autoplay" | "save" | "load")
        ));
        if let Some(code) = step["code"].as_str() {
            lua.load(code).into_function().unwrap();
            joins += code.matches("api.Join(entry.owner,entry.source)").count();
            evolves += code
                .matches("api.PrepareEvolution(entry.owner,entry.row.LeaderType)")
                .count();
            for forbidden in [
                "RequestPlayerOperation",
                "CompleteEvolution(",
                "ChangeGovernorPoints",
                "SetGovernorPoints",
                "853304",
                "CreatePerson(",
                "Reconcile(",
            ] {
                assert!(!code.contains(forbidden), "{forbidden}");
            }
            // Generator writes only the two existing UI pause flags.
            for line in code.lines().filter(|l| l.contains(":SetProperty(")) {
                assert!(
                    line == "Game:SetProperty('JAC_TEST853_ADMISSION_ACTIVE',1)"
                        || line == "Game:SetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT',1)"
                );
            }
        }
    }
    assert_eq!((joins, evolves), (22, 22));
}

#[test]
fn historical_all_22_cyclic_owners_costs_and_receipts_execute_with_bd252_gameplay() {
    let lua = model();
    let data = catalog::build(&template()).unwrap();
    let mut captures = BTreeMap::new();
    for step in data["steps"].as_array().unwrap() {
        if let Some(code) = step["code"].as_str() {
            if step["capture_as"] == "CATALOG880_FINAL" {
                // Scheduling evidence only, never actual native acceptance.
                lua.load("assert(#UI_EVER_CALLS==22); local seen={}; for _,v in ipairs(UI_EVER_CALLS) do assert(v.value==true and not seen[v.hash]); seen[v.hash]=true end").exec().unwrap();
            }
            lua.globals()
                .set("CORE_CONTEXT", step["state_name"] == "GameCore_Tuner")
                .unwrap();
            let value = run(&lua, code);
            if let Some(key) = step["equals_capture"].as_str() {
                assert_eq!(captures.get(key), Some(&value), "{key}");
            }
            if let Some(key) = step["capture_as"].as_str() {
                captures.insert(key.to_string(), value);
            }
            // Exercise the shipped automatic UI callback after every adapter.
            lua.load(
                "CORE_CONTEXT=false; FIREUI('GameCoreEventPublishComplete'); assert(#REQUESTS==0)",
            )
            .exec()
            .unwrap();
        }
    }
    assert!(captures["CATALOG880_CLOSED"].contains("MATRIX identities=22"));
    assert!(captures["CATALOG880_FINAL"].contains("MATRIX identities=22"));
    assert!(captures["CATALOG880_20_EVOLVE"].contains("21:0:164:LEADER_JAC_AINU_TARIKONA:1:"));
    lua.load(
        r#"
assert(SPAWNS==22 and DESTROYS==0 and TURN==1 and #REQUESTS==0)
local raw=table.concat(RAW,'\n')
assert(raw:find('GAMEPLAY_PROJECTION MATRIX positive=22 negative=462',1,true))
assert(raw:find('UI_NATIVE_QUALIFICATION MATRIX positive=22 negative=462',1,true))
assert(raw:find('GAMEPLAY_PROJECTION MATRIX positive=0 negative=484',1,true))
assert(not raw:find('UI_NATIVE_QUALIFICATION MATRIX positive=0 negative=484',1,true))
local costs={}
for source=0,21 do
 local row=ROWS[source+1]; local owner=(source+1)%22; costs[row.EvolutionCost]=true
 local prefix='JAC_PERSONA_'..row.LeaderType..'_'
 assert(Game:GetProperty(prefix..'OWNER')==owner and Game:GetProperty(prefix..'SOURCE')==source)
 assert(Game:GetProperty(prefix..'EVOLUTION_COST')==row.EvolutionCost)
 assert(Game:GetProperty(prefix..'EVOLUTION_SURCHARGE')==row.EvolutionCost-1)
 for id=0,21 do
  assert(Players[id].spent==0)
  -- Independently check shipped normal publication; getter oracle never reads this.
  assert(Players[id]:GetProperty(901000+GOVDEFS[source+1].Index)==(id==owner))
 end
end
assert(costs[1] and costs[2] and costs[3])
"#,
    )
    .exec()
    .unwrap();
}

#[test]
fn absent_saved_properties_may_return_zero_lua_values() {
    let lua = model();
    lua.load(
        r#"
local get=Game.GetProperty
Game.GetProperty=function(self,key)
 local value=get(self,key)
 if value==nil then return end
 return value
end
"#,
    )
    .exec()
    .unwrap();
    run(&lua, &catalog::prepare_code());
    let baseline = run(&lua, &catalog::matrix_code(false));
    assert!(baseline.contains(":nil:nil:nil:"));
    run(&lua, &catalog::pair_code(0, false, true).unwrap());
    let joined = run(&lua, &catalog::pair_code(0, false, false).unwrap());
    assert!(joined.contains(":GREAT_PERSON:") && joined.contains(":nil:"));
    run(&lua, &catalog::pair_code(0, true, true).unwrap());
    let evolving = run(&lua, &catalog::pair_code(0, true, false).unwrap());
    assert!(evolving.contains(":EVOLVING:"));
}

#[test]
fn historical_join_rejects_same_owner_and_world_duplicate_for_every_person() {
    let lua = model();
    run(&lua, &catalog::prepare_code());
    for source in 0..22 {
        lua.load(format!(
            "assert(ExposedMembers.JAC_LeaderPersona.Join({source},{source})=='INVALID')"
        ))
        .exec()
        .unwrap();
        run(&lua, &catalog::pair_code(source, false, true).unwrap());
        lua.load(format!(
            "assert(ExposedMembers.JAC_LeaderPersona.Join({}, {})=='OTHER_OWNER')",
            (source + 2) % 22,
            source
        ))
        .exec()
        .unwrap();
    }
    lua.load("assert(SPAWNS==22 and #REQUESTS==0)")
        .exec()
        .unwrap();
    assert!(catalog::pair_code(22, false, true).is_err());
    let changed = catalog::pair_code(0, false, true)
        .unwrap()
        .replace("owner=(source+1)%22", "owner=source");
    assert!(model().load(changed).eval::<String>().is_err());
}

#[test]
fn malformed_catalog_identity_missing_capital_and_unknown_projection_reject() {
    for mutation in [
        "ROWS[22]=nil",
        "GOVDEFS[22].Index=GOVDEFS[1].Index",
        "GOVDEFS[22].Hash=GOVDEFS[1].Hash",
        "ROWS[1].EvolutionCost=0",
        "ROWS[1].EvolutionCost=4",
        "ROWS[1].EvolutionCost=1.5",
        "for _,row in ipairs(ROWS) do row.EvolutionCost=1 end",
        "ROWS[1].CivilizationType='WRONG'",
        "PlayerConfigurations[21].leader='WRONG'",
        "Players[1].human=true",
        "IDS[22]=IDS[21]",
        "CITIES[21]={}",
        "Players[21]:SetProperty(901164,nil)",
    ] {
        let lua = model();
        lua.load(mutation).exec().unwrap();
        assert!(
            lua.load(catalog::matrix_code(false))
                .eval::<String>()
                .is_err(),
            "{mutation}"
        );
    }
    let mut bad = template();
    bad["steps"][1]["players"][1]["human"] = true.into();
    assert!(catalog::build(&bad).is_err());
}

#[test]
fn delayed_civics_and_unknown_mutation_returns_never_become_success() {
    let lua = model();
    lua.load("DELAY=true").exec().unwrap();
    assert!(lua.load(catalog::prepare_code()).eval::<String>().is_err());
    lua.load("assert(SPAWNS==0 and #REQUESTS==0)")
        .exec()
        .unwrap();
    for evolving in [false, true] {
        let lua = model();
        run(&lua, &catalog::prepare_code());
        if evolving {
            run(&lua, &catalog::pair_code(0, false, true).unwrap());
        }
        let method = if evolving { "PrepareEvolution" } else { "Join" };
        lua.load(format!("local original=ExposedMembers.JAC_LeaderPersona.{method}; CALLS=0; ExposedMembers.JAC_LeaderPersona.{method}=function(...) CALLS=CALLS+1; original(...); return nil end")).exec().unwrap();
        assert!(lua
            .load(catalog::pair_code(0, evolving, true).unwrap())
            .eval::<String>()
            .is_err());
        assert_eq!(lua.globals().get::<u32>("CALLS").unwrap(), 1);
    }
}

#[test]
fn game_cost_owner_source_and_boolean_result_tampering_rejects() {
    for mutation in [
        "Game:SetProperty('JAC_PERSONA_LEADER_JAC_HOJO_UJITSUNA_OWNER',2)",
        "Game:SetProperty('JAC_PERSONA_LEADER_JAC_HOJO_UJITSUNA_SOURCE',1)",
        "Game:SetProperty('JAC_PERSONA_LEADER_JAC_HOJO_UJITSUNA_EVOLUTION_COST',3)",
        "Game:SetProperty('JAC_PERSONA_LEADER_JAC_HOJO_UJITSUNA_EVOLUTION_SURCHARGE',1)",
        "Players[1].points=0",
        "Players[1].spent=1",
        "UNKNOWN=true",
        "local g=Players[1].GetGovernors; Players[1].GetGovernors=function() local v=g(); v.CanEverAppointGovernor=function() return 1 end; return v end",
    ] {
        let lua = model();
        run(&lua, &catalog::prepare_code());
        run(&lua, &catalog::pair_code(0, false, true).unwrap());
        run(&lua, &catalog::pair_code(0, true, true).unwrap());
        lua.load(mutation).exec().unwrap();
        assert!(
            lua.load(catalog::pair_code(0, true, false).unwrap())
                .eval::<String>()
                .is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn every_cost_requires_all_official_titles_before_the_single_reservation() {
    for source in 0..3 {
        let lua = model();
        run(&lua, &catalog::prepare_code());
        run(&lua, &catalog::pair_code(source, false, true).unwrap());
        lua.load(format!("Players[{}].points={source}", source + 1))
            .exec()
            .unwrap();
        assert!(lua
            .load(catalog::pair_code(source, true, true).unwrap())
            .eval::<String>()
            .is_err());
        lua.load(format!(
            "assert(Players[{}]:GetProperty('JAC_PERSONA_RESULT')=='NO_TITLE'); assert(SPAWNS==1 and DESTROYS==0 and #REQUESTS==0)", source+1
        )).exec().unwrap();
    }
}

#[test]
fn batch_stops_on_ambiguous_join_and_does_not_run_remaining_people() {
    let lua = model();
    run(&lua, &catalog::prepare_code());
    lua.load(
        r#"
local join=ExposedMembers.JAC_LeaderPersona.Join
CALLS=0
ExposedMembers.JAC_LeaderPersona.Join=function(owner,source)
 CALLS=CALLS+1; local result=join(owner,source)
 if source==1 then return nil end
 return result
end
"#,
    )
    .exec()
    .unwrap();
    assert!(lua
        .load(catalog::batch_code(0, 4, false, true).unwrap())
        .eval::<String>()
        .is_err());
    lua.load("assert(CALLS==2 and SPAWNS==2 and #REQUESTS==0)")
        .exec()
        .unwrap();
    for (first, end) in [(0, 0), (0, 5), (21, 23)] {
        assert!(catalog::batch_code(first, end, false, true).is_err());
    }
}
