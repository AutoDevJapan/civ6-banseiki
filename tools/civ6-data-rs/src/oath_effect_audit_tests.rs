//! Execute the generated assertions against independent numeric observations.
use super::*;
use mlua::{Lua, Value as LuaValue};

#[test]
fn reporting_step_supports_live_binary_yields_without_inventing_subinteger_progress() -> Result<()>
{
    let vm = Lua::new();
    let observed: f64 = vm
        .load(format!(
            "{LUA_HELPERS}\nreturn reported_step({{7.8984375,15.796875,7.9765625,8.984375}})"
        ))
        .eval()?;
    assert!((observed - (1.0 / 128.0 + 0.00002)).abs() < 1e-10);
    let integral: f64 = vm
        .load(format!(
            "{LUA_HELPERS}\nreturn reported_step({{0,7,17,26}})"
        ))
        .eval()?;
    assert!(integral >= 1.0, "integer progress must remain coarse");
    let noise: f64 = vm
        .load(format!(
            "{LUA_HELPERS}\nreturn reported_step({{100,110.0000000001,120.0000000002}})"
        ))
        .eval()?;
    assert!(
        noise >= 10.0,
        "floating noise must not imply a fine reporting step"
    );
    let same: LuaValue = vm
        .load(format!("{LUA_HELPERS}\nreturn reported_step({{7,7,7}})"))
        .eval()?;
    assert!(matches!(same, LuaValue::Nil));
    Ok(())
}

fn decode(lua: &Lua, code: &str) -> Result<Value> {
    let LuaValue::String(result) = lua.load(code).eval::<LuaValue>()? else {
        bail!("generated observation must return a JSON string");
    };
    Ok(serde_json::from_str(result.to_str()?.as_ref())?)
}

fn generated_code(case: &Case, needle: &str) -> Result<String> {
    let repo = Repository::checked_in();
    let scenario = selected_scenario(&repo.json(BASE)?, case)?;
    scenario["steps"]
        .as_array()
        .context("steps")?
        .iter()
        .filter_map(|step| step["code"].as_str())
        .find(|code| code.contains(needle))
        .map(str::to_owned)
        .context("generated assertion missing")
}

fn oda() -> Result<Case> {
    cases(&Repository::checked_in())?
        .into_iter()
        .find(|c| c.oath == "JAC_OATH_ODA_TENKA_FUBU")
        .context("catalogue lacks Tenka Fubu")
}

const PRODUCTION_OBSERVATIONS: &str = r#"
JAC_OATH_AUDIT={current={Target='UNIT_SPEARMAN',City=3},production={}}
function audit_rows(off,on,last,first_yield,on_yield,last_yield)
 local turn=0; local progress=0
 local function phase(delta,yield)
  local samples={}
  for i=1,1 do
   turn=turn+1; local before=progress; progress=progress+delta
   samples[i]={city=3,plot=22,turn=turn,before=before,after=progress,delta=delta,city_yield=yield,city_yield_after=yield,cost=1000}
  end
  return {samples=samples}
 end
 local first=phase(off,first_yield); local active=phase(on,on_yield); local final=phase(last,last_yield)
 return {off_first=first,on=active,off_last=final}
end
JAC_OATH_AUDIT.production.UNIT_SPEARMAN=audit_rows(10.01,13.9035,11.02,10.01,12.09,11.02)
"#;

#[test]
fn actual_progress_verifies_fifteen_percent_separately_from_city_yield() -> Result<()> {
    let code = generated_code(&oda()?, "local target=\"UNIT_SPEARMAN\"; local amount=15")?;
    let lua = Lua::new();
    lua.load(PRODUCTION_OBSERVATIONS).exec()?;
    let result = decode(&lua, &code)?;
    assert_eq!(result["quantity_verified"], true);
    assert_eq!(result["expected_percent"], 15);
    assert_eq!(result["expected_on"], 1.15);
    assert_eq!(result["on"], 1.15);
    assert_eq!(result["restored"], true);
    for delta in [12.0, 12.12, 13.2, 13.68, 15.0] {
        lua.load(PRODUCTION_OBSERVATIONS).exec()?;
        lua.load(format!(
            "JAC_OATH_AUDIT.production.UNIT_SPEARMAN=audit_rows(10.01,{delta},11.02,10.01,12.09,11.02)"
        ))
        .exec()?;
        assert!(
            decode(&lua, &code).is_err(),
            "accepted wrong on-progress {delta}"
        );
    }
    Ok(())
}

#[test]
fn other_production_modifiers_and_insufficient_resolution_do_not_prove_percentage() -> Result<()> {
    let code = generated_code(&oda()?, "local target=\"UNIT_SPEARMAN\"; local amount=15")?;
    let lua = Lua::new();
    lua.load(PRODUCTION_OBSERVATIONS).exec()?;
    lua.load("JAC_OATH_AUDIT.production.UNIT_SPEARMAN=audit_rows(15,19.8,15,10,12,10)")
        .exec()?;
    let result = decode(&lua, &code)?;
    assert_eq!(result["quantity_verified"], false);
    assert_eq!(result["unmodified_off_basis_observed"], false);
    assert_eq!(result["expected_on"], Value::Null);
    lua.load(PRODUCTION_OBSERVATIONS).exec()?;
    lua.load("JAC_OATH_AUDIT.production.UNIT_SPEARMAN=audit_rows(0.1,0.115,0.1,0.1,0.1,0.1)")
        .exec()?;
    let result = decode(&lua, &code)?;
    assert_eq!(result["quantity_verified"], false);
    assert_eq!(result["integer_percentage_resolved"], false);
    Ok(())
}

#[test]
fn non_target_builds_wrong_city_and_nonconsecutive_turns_are_rejected() -> Result<()> {
    let case = oda()?;
    let subject = generated_code(&case, "local target=\"UNIT_SPEARMAN\"; local amount=15")?;
    let control = generated_code(&case, "local target=\"UNIT_BUILDER\"; local amount=0")?;
    let lua = Lua::new();
    lua.load(PRODUCTION_OBSERVATIONS).exec()?;
    lua.load("JAC_OATH_AUDIT.current.Target='UNIT_BUILDER'; JAC_OATH_AUDIT.production.UNIT_BUILDER=audit_rows(10.01,12.09,11.02,10.01,12.09,11.02)").exec()?;
    assert_eq!(decode(&lua, &control)?["quantity_verified"], true);
    lua.load(
        "JAC_OATH_AUDIT.production.UNIT_BUILDER=audit_rows(10.01,13.9035,11.02,10.01,12.09,11.02)",
    )
    .exec()?;
    assert!(
        decode(&lua, &control).is_err(),
        "specialty leaked to a builder"
    );
    for mutation in [
        "r.on.samples[1].city=4",
        "r.off_last.samples[1].plot=99",
        "r.on.samples[1].turn=3",
        "r.off_last.samples[1].delta=9",
    ] {
        lua.load(PRODUCTION_OBSERVATIONS).exec()?;
        lua.load(format!(
            "local r=JAC_OATH_AUDIT.production.UNIT_SPEARMAN; {mutation}"
        ))
        .exec()?;
        assert!(decode(&lua, &subject).is_err(), "accepted {mutation}");
    }
    Ok(())
}

#[test]
fn generated_native_end_turn_is_forced_and_is_never_requested_twice() -> Result<()> {
    let code = generated_code(&oda()?, "single native human end-turn requested")?;
    let lua = Lua::new();
    lua.load(PRODUCTION_OBSERVATIONS).exec()?;
    lua.load(r#"
local calls=0
Game={GetCurrentGameTurn=function() return 1 end}
Players={[0]={IsTurnActive=function() return true end}}
ActionTypes={ACTION_ENDTURN=9}
UI={RequestAction=function(action,params) assert(action==9 and params.REASON=='UserForced'); calls=calls+1 end}
native_request_count=function() return calls end
"#).exec()?;
    lua.load(&code).exec()?;
    assert!(lua.load(&code).exec().is_err());
    let value = lua
        .load("return native_request_count()")
        .eval::<LuaValue>()?;
    match value {
        LuaValue::Number(calls) => assert_eq!(calls, 1.0),
        LuaValue::Integer(calls) => assert_eq!(calls, 1),
        _ => bail!("numeric native request count required"),
    }
    Ok(())
}

#[test]
fn single_turn_samples_distinguish_adjacent_percentages_and_reject_low_resolution() -> Result<()> {
    let military = generated_code(&oda()?, "local target=\"UNIT_SPEARMAN\"; local amount=15")?;
    let mut wall_case = growth_case();
    wall_case.effects = vec![
        Effect {
            kind: "walls".into(),
            amount: 20,
        },
        Effect {
            kind: "growth".into(),
            amount: 10,
        },
    ];
    let wall = generated_code(
        &wall_case,
        "local target=\"BUILDING_WALLS\"; local amount=20",
    )?;
    let lua = Lua::new();
    for (target, code, correct, adjacent) in [
        ("UNIT_SPEARMAN", military, 7.038, 6.9768),
        ("BUILDING_WALLS", wall, 7.344, 7.2828),
    ] {
        lua.load(PRODUCTION_OBSERVATIONS).exec()?;
        lua.load(format!("JAC_OATH_AUDIT.current.Target='{target}'; JAC_OATH_AUDIT.production['{target}']=audit_rows(5.01,{correct},7.03,5.01,6.12,7.03)")).exec()?;
        let result = decode(&lua, &code)?;
        assert_eq!(result["quantity_verified"], true);
        assert_eq!(result["integer_percentage_resolved"], true);
        lua.load(format!(
            "JAC_OATH_AUDIT.production['{target}']=audit_rows(5.01,{adjacent},7.03,5.01,6.12,7.03)"
        ))
        .exec()?;
        assert!(
            decode(&lua, &code).is_err(),
            "accepted adjacent amount for {target}"
        );
        lua.load(format!(
            "JAC_OATH_AUDIT.production['{target}']=audit_rows(20,23,20,20,20,20)"
        ))
        .exec()?;
        let unresolved = decode(&lua, &code)?;
        assert_eq!(unresolved["quantity_verified"], false);
        assert_eq!(unresolved["integer_percentage_resolved"], false);
    }
    Ok(())
}

#[test]
fn production_changes_inside_a_turn_never_prove_a_specialty_percentage() -> Result<()> {
    let code = generated_code(&oda()?, "local target=\"UNIT_SPEARMAN\"; local amount=15")?;
    let lua = Lua::new();
    lua.load(PRODUCTION_OBSERVATIONS).exec()?;
    lua.load("JAC_OATH_AUDIT.production.UNIT_SPEARMAN.on.samples[1].city_yield_after=13")
        .exec()?;
    let result = decode(&lua, &code)?;
    assert_eq!(result["quantity_verified"], false);
    assert_eq!(result["city_production_stable_inside_turns"], false);
    assert_eq!(result["expected_on"], Value::Null);
    Ok(())
}

const CITY_OBSERVATIONS: &str = r#"
local oath='JAC_OATH_ODA_TENKA_FUBU'
GameInfo={JAC_SengokuOaths=function()
 local i=0; return function() i=i+1; if i<=66 then return {OathType=i==1 and oath or ('OTHER_'..i)} end end
end}
Map={GetPlotByIndex=function() return {GetOwner=function() return 0 end,GetProperty=function(_,name) return name==oath..'_ON' and 1 or 0 end} end}
local function city(owner,on)
 return {id=owner+1,plot=owner+10,owner=owner,y={production=10,gold=3,science=on and 2.1 or 2,culture=3,faith=1,food=10},
 growth=on and 0.55 or 0.5,other=on and 0.1 or 0,housing=0.5,happiness=0,surplus=2,occupied=false,loyalty=1}
end
JAC_OATH_AUDIT={}
for _,phase in ipairs({'baseline','selected','off','restored'}) do
 JAC_OATH_AUDIT[phase]={own={city(0,phase=='selected' or phase=='restored')},foreign={city(1,false)}}
end
"#;

fn growth_case() -> Case {
    Case {
        house: "ODA".into(),
        civ: "CIVILIZATION_JAC_ODA".into(),
        oath: "JAC_OATH_ODA_TENKA_FUBU".into(),
        effects: vec![
            Effect {
                kind: "growth".into(),
                amount: 10,
            },
            Effect {
                kind: "science".into(),
                amount: 5,
            },
        ],
    }
}

#[test]
fn growth_uses_housing_formula_and_raw_yield_quantity_remains_unaccepted() -> Result<()> {
    let code = generated_code(&growth_case(), "local spec=")?;
    let lua = Lua::new();
    lua.load(CITY_OBSERVATIONS).exec()?;
    let result = decode(&lua, &code)?;
    assert_eq!(result["results"][0]["expected_delta"], 0.05);
    assert_eq!(result["results"][0]["observed_delta"], 0.05);
    assert_eq!(result["results"][0]["quantity_verified"], true);
    assert_eq!(result["results"][1]["expected_delta"], Value::Null);
    assert_eq!(result["results"][1]["expected_percent"], 5);
    assert_eq!(result["quantity_verified"], false);
    for mutation in [
        "a.selected.own[1].other=0.01",
        "a.selected.own[1].growth=0.6",
        "a.off.own[1].growth=0.55",
        "a.selected.foreign[1].y.science=3",
        "a.restored.foreign[1].plot=99",
        "a.selected.own[1].y.gold=4",
    ] {
        lua.load(CITY_OBSERVATIONS).exec()?;
        lua.load(format!("local a=JAC_OATH_AUDIT; {mutation}"))
            .exec()?;
        assert!(decode(&lua, &code).is_err(), "accepted {mutation}");
    }
    Ok(())
}

#[test]
fn native_reference_proves_yield_percentage_and_rejects_wrong_reference_or_leakage() -> Result<()> {
    let code = generated_code(&growth_case(), "local spec=")?;
    let lua = Lua::new();
    let reference = r#"
local function copy(value)
 if type(value)~='table' then return value end
 local result={}; for k,v in pairs(value) do result[k]=copy(v) end; return result
end
local a=JAC_OATH_AUDIT
a.selected.own[1].y.science=2.4; a.restored.own[1].y.science=2.4
a.reference=copy(a.baseline); a.reference_off=copy(a.baseline)
a.reference.own[1].y.science=a.baseline.own[1].y.science+8
a.precision_one=copy(a.baseline); a.precision_two=copy(a.baseline); a.precision_off=copy(a.baseline)
a.precision_one.own[1].y.science=a.baseline.own[1].y.science+0.08; a.precision_two.own[1].y.science=a.baseline.own[1].y.science+1.09
"#;
    lua.load(CITY_OBSERVATIONS).exec()?;
    lua.load(reference).exec()?;
    let result = decode(&lua, &code)?;
    assert_eq!(result["quantity_verified"], true);
    assert_eq!(result["results"][1]["expected_delta"], 0.4);
    assert_eq!(
        result["results"][1]["reason"],
        "native_100_percent_reference_delta"
    );
    for mutation in [
        "a.reference.own[1].y.science=a.baseline.own[1].y.science",
        "a.reference.own[1].y.science=a.baseline.own[1].y.science+9",
        "a.reference_off.own[1].y.science=99",
        "a.reference.own[1].id=99",
        "a.reference.own[1].owner=99",
        "a.reference.foreign={}",
        "a.reference_off=nil",
        "a.reference.foreign[1].y.gold=99",
        "a.reference_off.foreign[1].owner=0",
    ] {
        lua.load(CITY_OBSERVATIONS).exec()?;
        lua.load(reference).exec()?;
        lua.load(format!("local a=JAC_OATH_AUDIT; {mutation}"))
            .exec()?;
        assert!(decode(&lua, &code).is_err(), "accepted {mutation}");
    }
    lua.load(CITY_OBSERVATIONS).exec()?;
    lua.load(reference).exec()?;
    lua.load("local a=JAC_OATH_AUDIT; a.reference.own[1].y.science=a.baseline.own[1].y.science+2; a.selected.own[1].y.science=2.1; a.restored.own[1].y.science=2.1").exec()?;
    assert_eq!(decode(&lua, &code)?["quantity_verified"], false);
    Ok(())
}

#[test]
fn reference_mod_has_only_explicit_scoped_inputs_and_all_valid_modifier_definitions() -> Result<()>
{
    let files = meter_outputs();
    let sql = &files[&PathBuf::from(format!("{METER_DIR}/Data/Meter.sql"))];
    let db = rusqlite::Connection::open_in_memory()?;
    db.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE Requirements(RequirementId TEXT PRIMARY KEY,RequirementType TEXT NOT NULL); CREATE TABLE RequirementArguments(RequirementId TEXT REFERENCES Requirements,Name TEXT,Value TEXT,PRIMARY KEY(RequirementId,Name)); CREATE TABLE RequirementSets(RequirementSetId TEXT PRIMARY KEY,RequirementSetType TEXT); CREATE TABLE RequirementSetRequirements(RequirementSetId TEXT REFERENCES RequirementSets,RequirementId TEXT REFERENCES Requirements,PRIMARY KEY(RequirementSetId,RequirementId)); CREATE TABLE Modifiers(ModifierId TEXT PRIMARY KEY,ModifierType TEXT NOT NULL,SubjectRequirementSetId TEXT REFERENCES RequirementSets); CREATE TABLE ModifierArguments(ModifierId TEXT REFERENCES Modifiers,Name TEXT,Value TEXT,PRIMARY KEY(ModifierId,Name));")?;
    db.execute_batch(sql)?;
    let count: i64 = db.query_row("SELECT count(*) FROM Modifiers", [], |r| r.get(0))?;
    assert_eq!(count, 27);
    for (id, kind, amount) in [
        (
            "JAC_TODO134_HEADROOM_PRODUCTION",
            "MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_CHANGE",
            "4096",
        ),
        (
            "JAC_TODO134_HEADROOM_GROWTH",
            "MODIFIER_PLAYER_CITIES_ADJUST_CITY_GROWTH",
            "-1000",
        ),
    ] {
        assert_eq!(
            db.query_row(
                "SELECT ModifierType FROM Modifiers WHERE ModifierId=?",
                [id],
                |r| r.get::<_, String>(0)
            )?,
            kind
        );
        assert_eq!(
            db.query_row(
                "SELECT Value FROM ModifierArguments WHERE ModifierId=? AND Name='Amount'",
                [id],
                |r| r.get::<_, String>(0)
            )?,
            amount
        );
        assert_eq!(db.query_row("SELECT Value FROM RequirementArguments WHERE RequirementId='JAC_TODO134_HEADROOM_REQ' AND Name='PropertyName'",[],|r|r.get::<_,String>(0))?,"JAC_TODO134_HEADROOM_ON");
        assert_eq!(
            db.query_row(
                "SELECT SubjectRequirementSetId FROM Modifiers WHERE ModifierId=?",
                [id],
                |r| r.get::<_, String>(0)
            )?,
            "JAC_TODO134_HEADROOM_SET"
        );
    }
    for kind in ["PRODUCTION", "GOLD", "SCIENCE", "CULTURE", "FAITH", "FOOD"] {
        for (mode, amount, suffix) in [
            ("BASE", "6", "CHANGE"),
            ("CAL", "100", "MODIFIER"),
            ("FINE", "1", "MODIFIER"),
            ("SHIFT", "1", "CHANGE"),
        ] {
            let id = format!("JAC_TODO134_{mode}_{kind}");
            let modifier: String = db.query_row(
                "SELECT ModifierType FROM Modifiers WHERE ModifierId=?",
                [&id],
                |r| r.get(0),
            )?;
            assert_eq!(
                modifier,
                format!("MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_{suffix}")
            );
            let value: String = db.query_row(
                "SELECT Value FROM ModifierArguments WHERE ModifierId=? AND Name='Amount'",
                [&id],
                |r| r.get(0),
            )?;
            assert_eq!(value, amount);
            let yield_type: String = db.query_row(
                "SELECT Value FROM ModifierArguments WHERE ModifierId=? AND Name='YieldType'",
                [&id],
                |r| r.get(0),
            )?;
            assert_eq!(yield_type, format!("YIELD_{kind}"));
        }
    }
    assert!(
        !sql.contains("TraitModifiers"),
        "diagnostic must not attach automatically"
    );
    let meta = &files[&PathBuf::from(format!("{METER_DIR}/JAC_PoC_OathMeter.modinfo"))];
    let document = roxmltree::Document::parse(meta)?;
    assert_eq!(document.root_element().attribute("id"), Some(METER_ID));
    assert!(document
        .descendants()
        .any(|n| n.has_tag_name("RuleSetInUse") && n.text() == Some("RULESET_JAC_BANSEIKI")));
    assert!(document
        .descendants()
        .any(|n| n.has_tag_name("UpdateDatabase")
            && n.attribute("criteria") == Some("JAC_OathMeter_Banseiki")));
    assert!(!document
        .descendants()
        .any(|n| n.has_tag_name("AddGameplayScripts")));
    let criteria = document
        .descendants()
        .find(|n| n.has_tag_name("Criteria") && n.attribute("id") == Some("JAC_OathMeter_Headroom"))
        .context("headroom criteria")?;
    assert!(criteria
        .descendants()
        .any(|n| n.has_tag_name("RuleSetInUse") && n.text() == Some("RULESET_JAC_BANSEIKI")));
    assert!(criteria.descendants().any(
        |n| n.has_tag_name("ConfigurationId") && n.text() == Some("JAC_TODO134_QUEUE_HEADROOM")
    ));
    let headroom = &files[&PathBuf::from(format!("{METER_DIR}/Data/Headroom.sql"))];
    let config = &files[&PathBuf::from(format!("{METER_DIR}/Config/Headroom.xml"))];
    let config = roxmltree::Document::parse(config)?;
    let row = config
        .descendants()
        .find(|n| n.attribute("ConfigurationId") == Some("JAC_TODO134_QUEUE_HEADROOM"))
        .context("headroom config row")?;
    assert_eq!(row.attribute("DefaultValue"), Some("0"));
    assert_eq!(row.attribute("Key2"), Some("RULESET_JAC_BANSEIKI"));
    assert_eq!(row.attribute("Visible"), Some("0"));
    for mode in [
        "SupportsLANMultiplayer",
        "SupportsInternetMultiplayer",
        "SupportsHotSeat",
        "SupportsPlayByCloud",
    ] {
        assert_eq!(row.attribute(mode), Some("0"));
    }
    db.execute_batch("CREATE TABLE Units(UnitType TEXT PRIMARY KEY,Cost INTEGER); CREATE TABLE Buildings(BuildingType TEXT PRIMARY KEY,Cost INTEGER); INSERT INTO Units VALUES('UNIT_SPEARMAN',65),('UNIT_BUILDER',50),('UNIT_GALLEY',65),('UNIT_WARRIOR',40); INSERT INTO Buildings VALUES('BUILDING_WALLS',80),('BUILDING_CASTLE',100),('BUILDING_STAR_FORT',300),('BUILDING_MONUMENT',60);")?;
    db.execute_batch(headroom)?;
    for unit in ["UNIT_SPEARMAN", "UNIT_BUILDER", "UNIT_GALLEY"] {
        assert_eq!(
            db.query_row("SELECT Cost FROM Units WHERE UnitType=?", [unit], |r| r
                .get::<_, i64>(0))?,
            32768
        );
    }
    for building in ["BUILDING_WALLS", "BUILDING_CASTLE", "BUILDING_STAR_FORT"] {
        assert_eq!(
            db.query_row(
                "SELECT Cost FROM Buildings WHERE BuildingType=?",
                [building],
                |r| r.get::<_, i64>(0)
            )?,
            32768
        );
    }
    assert_eq!(
        db.query_row(
            "SELECT Cost FROM Units WHERE UnitType='UNIT_SPEARMAN'",
            [],
            |r| r.get::<_, i64>(0)
        )?,
        32768
    );
    assert_eq!(
        db.query_row(
            "SELECT Cost FROM Units WHERE UnitType='UNIT_WARRIOR'",
            [],
            |r| r.get::<_, i64>(0)
        )?,
        40
    );
    assert_eq!(
        db.query_row(
            "SELECT Cost FROM Buildings WHERE BuildingType='BUILDING_MONUMENT'",
            [],
            |r| r.get::<_, i64>(0)
        )?,
        60
    );
    Ok(())
}

const FINAL_NATIVE_FIXTURE: &str = r#"
local oath='JAC_OATH_ODA_TENKA_FUBU'
local ledger={reign=1,houses={p0={civ='CIVILIZATION_JAC_ODA',oathChosen=1,oaths={[oath]=true}}}}
Game={GetProperty=function(_,key) assert(key=='JAC_AGES_LEDGER'); return ledger end}
GameInfo={JAC_SengokuOaths=function()
 local i=0; return function() i=i+1; if i<=66 then return {OathType=i==1 and oath or ('OTHER_'..i)} end end
end}
local plots={}; local majors={}; local own={}; local foreign={}
Players={}
for owner=0,21 do
 local playerID=owner; local cities={}
 local n=playerID>=1 and playerID<=12 and 3 or 2
 for index=1,n do
  local cityID=index; local plotID=playerID*100+cityID
  local city={GetID=function() return cityID end,GetOwner=function() return playerID end,
   GetX=function() return plotID end,GetY=function() return 0 end}
  plots[plotID]={GetIndex=function() return plotID end,GetOwner=function() return playerID end,
   GetProperty=function(_,name) return playerID==0 and name==oath..'_ON' and 1 or 0 end}
  cities[#cities+1]=city
  local records=playerID==0 and own or foreign
  records[#records+1]={id=cityID,plot=plotID,owner=playerID}
 end
 local collection={Members=function() return ipairs(cities) end}
 local player={GetID=function() return playerID end,GetCities=function() return collection end}
 Players[playerID]=player; majors[#majors+1]=player
end
PlayerManager={GetAliveMajors=function() return majors end}
Map={GetPlot=function(x,y) assert(y==0); return plots[x] end}
JAC_OATH_AUDIT=JAC_OATH_AUDIT or {}
JAC_OATH_AUDIT.baseline={own=own,foreign=foreign}
JAC_OATH_AUDIT.yield_growth_observed=true
"#;

#[test]
fn final_generated_report_and_gate_require_true_quantity_and_every_production_result() -> Result<()>
{
    let final_code = generated_code(&oda()?, "saved UI choice not restored after diagnostics")?;
    let lua = Lua::new();
    for quantity in ["true", "false", "nil"] {
        lua.load("JAC_OATH_AUDIT=nil").exec()?;
        lua.load(FINAL_NATIVE_FIXTURE).exec()?;
        lua.load(format!(r#"
JAC_OATH_AUDIT.quantities_verified={quantity}
local function proof(amount) return {{quantity_verified=true,expected_percent=amount,sample_count=3,off_first={{}},on={{}},off_last={{}}}} end
JAC_OATH_AUDIT.production={{UNIT_SPEARMAN=proof(15),UNIT_BUILDER=proof(0)}}
"#)).exec()?;
        let result = decode(&lua, &final_code)?;
        assert_eq!(result["accepted"], quantity == "true");
        assert_eq!(result["production_results_complete"], true);
        if quantity == "true" {
            lua.load(ACCEPTANCE_GATE).exec()?;
        } else {
            assert!(lua.load(ACCEPTANCE_GATE).exec().is_err());
        }
    }
    for mutation in [
        "a.production.UNIT_BUILDER=nil",
        "a.production.UNIT_BUILDER.quantity_verified=false",
        "a.production.UNIT_BUILDER.quantity_verified=nil",
        "a.production.UNIT_SPEARMAN.expected_percent=1",
        "a.production.UNIT_SPEARMAN.off_last=nil",
        "a.production.UNIT_SPEARMAN.sample_count=nil",
        "a.production.UNIT_SPEARMAN.sample_count=9",
        "a.production.UNIT_SPEARMAN.sample_count=2",
        "a.production={}; a.production_quantities_verified=true",
    ] {
        lua.load("JAC_OATH_AUDIT=nil").exec()?;
        lua.load(FINAL_NATIVE_FIXTURE).exec()?;
        lua.load("JAC_OATH_AUDIT.quantities_verified=true; local function p(amount) return {quantity_verified=true,expected_percent=amount,sample_count=3,off_first={},on={},off_last={}} end; JAC_OATH_AUDIT.production={UNIT_SPEARMAN=p(15),UNIT_BUILDER=p(0)}").exec()?;
        lua.load(format!("local a=JAC_OATH_AUDIT; {mutation}"))
            .exec()?;
        let result = decode(&lua, &final_code)?;
        assert_eq!(result["accepted"], false, "accepted {mutation}");
        assert_eq!(result["production_results_complete"], false);
        assert!(lua.load(ACCEPTANCE_GATE).exec().is_err());
    }
    Ok(())
}

#[test]
fn low_effective_yield_is_not_mistaken_for_zero_raw_yield() -> Result<()> {
    let case = growth_case();
    let compare = generated_code(&case, "local spec=")?;
    let final_code = generated_code(&case, "saved UI choice not restored after diagnostics")?;
    let lua = Lua::new();
    lua.load(CITY_OBSERVATIONS).exec()?;
    lua.load("local a=JAC_OATH_AUDIT; a.baseline.own[1].y.science=0.02; a.off.own[1].y.science=0.02; a.selected.own[1].y.science=0.12; a.restored.own[1].y.science=0.12").exec()?;
    let observed = decode(&lua, &compare)?;
    assert_eq!(observed["results"][1]["observed_delta"], 0.1);
    assert_eq!(observed["quantity_verified"], false);
    // Keep the actual comparison result while adding the native final-state fixture.
    lua.load(FINAL_NATIVE_FIXTURE).exec()?;
    let result = decode(&lua, &final_code)?;
    assert_eq!(result["accepted"], false);
    assert!(lua.load(ACCEPTANCE_GATE).exec().is_err());
    Ok(())
}

#[test]
fn zero_yield_no_growth_or_wrong_oath_flag_never_proves_effect() -> Result<()> {
    let code = generated_code(&growth_case(), "local spec=")?;
    let lua = Lua::new();
    for mutation in [
        "for _,p in ipairs({'baseline','selected','off','restored'}) do a[p].own[1].y.science=0 end",
        "for _,p in ipairs({'baseline','selected','off','restored'}) do a[p].own[1].housing=0; a[p].own[1].growth=0 end",
        "Map.GetPlotByIndex=function() return {GetOwner=function() return 0 end,GetProperty=function() return 1 end} end",
    ] {
        lua.load(CITY_OBSERVATIONS).exec()?;
        lua.load(format!("local a=JAC_OATH_AUDIT; {mutation}")).exec()?;
        assert!(decode(&lua,&code).is_err(),"accepted {mutation}");
    }
    Ok(())
}

#[test]
fn capability_diagnosis_calls_apis_and_preserves_json_without_certifying_candidates() -> Result<()>
{
    let base = Repository::checked_in().json(BASE)?;
    let scenario = capability_scenario(&base)?;
    let code = scenario["steps"]
        .as_array()
        .context("steps")?
        .last()
        .context("UI diagnosis")?["code"]
        .as_str()
        .context("code")?;
    let lua = Lua::new();
    lua.load(r#"
local c={GetID=function() return 1 end,GetYield=function() return 10 end,GetBaseYield=function() return 8 end,
 GetYieldModifier=function() error('unknown "modifier"\nAPI') end,
 GetGrowth=function() return {GetOverallGrowthModifier=function() return 1 end} end,
 GetBuildQueue=function() return setmetatable({GetUnitProgress=function() return '0' end,GetUnitCost=function() return 0/0 end},{__index=function(_,key) error('missing member '..key) end}) end}
Players={[0]={GetCities=function() return {GetCapitalCity=function() return c end} end}}
GameInfo={Yields=setmetatable({},{__index=function() return {Index=0} end}),Units={UNIT_SPEARMAN={Index=1}},Buildings={BUILDING_WALLS={Index=2}}}
"#).exec()?;
    let result = decode(&lua, code)?;
    assert_eq!(result["accepted"], false);
    let probes = result["probes"].as_array().context("probes")?;
    assert_eq!(probes.len(), 31);
    let find = |name: &str| {
        probes
            .iter()
            .find(|p| p["method"] == name)
            .expect("named probe")
    };
    assert_eq!(find("GetBaseYield")["finite_numeric"], true);
    assert_eq!(
        find("GetBaseYield")["quantity_contract"],
        "unknown_do_not_accept"
    );
    assert_eq!(find("GetYieldModifier")["call_ok"], false);
    assert!(find("GetYieldModifier")["error"]
        .as_str()
        .context("escaped error")?
        .contains("\nAPI"));
    assert_eq!(find("GetUnitProgress")["return_type"], "string");
    assert_eq!(find("GetUnitProgress")["finite_numeric"], false);
    assert_eq!(find("GetUnitCost")["value"], Value::Null);
    assert_eq!(find("GetBuildingCost")["exists"], false);
    assert!(find("GetBuildingCost")["error"]
        .as_str()
        .context("lookup diagnostic")?
        .contains("missing member"));
    Ok(())
}

#[test]
fn checked_in_scenarios_and_inventory_match_the_generator() -> Result<()> {
    let repo = Repository::checked_in();
    for (path, expected) in outputs(&repo)? {
        assert_eq!(repo.read(&path)?, expected, "stale {}", path.display());
    }
    Ok(())
}

fn copied_source() -> Result<(tempfile::TempDir, Repository)> {
    let directory = tempfile::tempdir()?;
    let copied = Repository::new(directory.path());
    let source = Repository::checked_in();
    for relative in [BASE, "documents/poc/data/sengoku-oaths.json"] {
        let path = copied.path(relative);
        std::fs::create_dir_all(path.parent().context("fixture parent")?)?;
        std::fs::write(path, source.read(relative)?)?;
    }
    Ok((directory, copied))
}

#[test]
fn incomplete_catalogues_duplicate_metrics_and_invalid_map_rosters_are_rejected() -> Result<()> {
    for mutation in 0..4 {
        let (_directory, repo) = copied_source()?;
        let mut catalogue = repo.json("documents/poc/data/sengoku-oaths.json")?;
        match mutation {
            0 => {
                catalogue["houses"][0]["oaths"]
                    .as_array_mut()
                    .context("oaths")?
                    .pop();
            }
            1 => {
                catalogue["houses"][0]["oaths"][0]["effects"][1][0] = json!("military");
            }
            2 => {
                catalogue["houses"][0]["oaths"][0]["effects"][0][0] = json!("unknown");
            }
            _ => {
                catalogue["houses"][0]["oaths"][0]["effects"][0][1] = json!(0);
            }
        }
        std::fs::write(
            repo.path("documents/poc/data/sengoku-oaths.json"),
            serde_json::to_vec(&catalogue)?,
        )?;
        assert!(
            outputs(&repo).is_err(),
            "accepted malformed catalogue case {mutation}"
        );
    }
    let (_directory, repo) = copied_source()?;
    let mut base = repo.json(BASE)?;
    base["steps"][1]["players"][1] = base["steps"][1]["players"][0].clone();
    std::fs::write(repo.path(BASE), serde_json::to_vec(&base)?)?;
    assert!(
        outputs(&repo).is_err(),
        "accepted a duplicated and missing real house"
    );
    for field in ["ruleset", "map_file", "map_size"] {
        let (_directory, repo) = copied_source()?;
        let mut base = repo.json(BASE)?;
        base["steps"][1][field] = json!("UNRELATED");
        std::fs::write(repo.path(BASE), serde_json::to_vec(&base)?)?;
        assert!(outputs(&repo).is_err(), "accepted unrelated {field}");
    }
    Ok(())
}

#[test]
fn check_rejects_stale_missing_and_extra_scenarios_without_writing_them() -> Result<()> {
    let (_directory, repo) = copied_source()?;
    cli(&repo, &[])?;
    cli(&repo, &["--check".into()])?;
    let path = repo.path(format!("{DIR}/jac_oath_oda_tenka_fubu.json"));
    std::fs::write(&path, "{\"stale\":true}\n")?;
    assert!(cli(&repo, &["--check".into()]).is_err());
    assert_eq!(std::fs::read_to_string(&path)?, "{\"stale\":true}\n");
    std::fs::remove_file(&path)?;
    assert!(cli(&repo, &["--check".into()]).is_err());
    assert!(!path.exists());
    cli(&repo, &[])?;
    let extra = repo.path(format!("{DIR}/retired.json"));
    std::fs::write(&extra, "{}")?;
    assert!(cli(&repo, &["--check".into()]).is_err());
    assert_eq!(std::fs::read_to_string(extra)?, "{}");
    Ok(())
}

#[test]
fn every_product_lua_file_compiles_with_lua51() -> Result<()> {
    let repo = Repository::checked_in();
    let lua = Lua::new();
    let mut files = BTreeSet::new();
    for entry in walkdir::WalkDir::new(repo.path("poc")) {
        let entry = entry?;
        if entry.file_type().is_file()
            && entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("lua"))
        {
            let path = entry.path().strip_prefix(&repo.root)?;
            lua.load(product_lua_for_vm(path, repo.read(path)?)?)
                .set_name(path.to_string_lossy().as_ref())
                .into_function()
                .with_context(|| format!("Lua 5.1 compile {}", path.display()))?;
            files.insert(path.to_path_buf());
        }
    }
    assert!(files.contains(&PathBuf::from(
        "poc/JAC_PoC_SengokuTrees/Scripts/JAC_Sengoku_Oaths.lua"
    )));
    assert!(
        files.len() > 50,
        "product Lua inventory unexpectedly shrank"
    );
    eprintln!("compiled {} product Lua files with Lua 5.1; the existing AdvisorItem Havok declaration was validated and adapted", files.len());
    Ok(())
}

#[derive(Debug)]
struct CodeToken {
    text: String,
    start: usize,
    end: usize,
    literal: bool,
}

fn long_lua_end(source: &str, start: usize) -> Result<Option<usize>> {
    let bytes = source.as_bytes();
    if bytes.get(start) != Some(&b'[') {
        return Ok(None);
    }
    let mut cursor = start + 1;
    while bytes.get(cursor) == Some(&b'=') {
        cursor += 1;
    }
    if bytes.get(cursor) != Some(&b'[') {
        return Ok(None);
    }
    let close = format!("]{}]", "=".repeat(cursor - start - 1));
    let suffix = &source[cursor + 1..];
    let end = suffix
        .find(&close)
        .context("unterminated Lua long string/comment")?;
    Ok(Some(cursor + 1 + end + close.len()))
}

// Only tokens in actual code can introduce Havok declarations/constructions.
// Keep comments and strings verbatim; both Lua long-bracket levels and escaped
// quoted strings must be skipped before looking for the two native keywords.
fn lua_code_tokens(source: &str) -> Result<Vec<CodeToken>> {
    let bytes = source.as_bytes();
    let mut cursor = 0;
    let mut tokens = Vec::new();
    while cursor < bytes.len() {
        if bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        if bytes.get(cursor..cursor + 2) == Some(b"--") {
            if let Some(end) = long_lua_end(source, cursor + 2)? {
                cursor = end;
            } else {
                cursor += 2;
                while cursor < bytes.len() && bytes[cursor] != b'\n' {
                    cursor += 1;
                }
            }
            continue;
        }
        if bytes[cursor] == b'\'' || bytes[cursor] == b'"' {
            let start = cursor;
            let quote = bytes[cursor];
            cursor += 1;
            let mut closed = false;
            while cursor < bytes.len() {
                if bytes[cursor] == b'\\' {
                    cursor += 2;
                } else if bytes[cursor] == quote {
                    cursor += 1;
                    closed = true;
                    break;
                } else {
                    cursor += 1;
                }
            }
            if !closed {
                bail!("unterminated quoted Lua string");
            }
            tokens.push(CodeToken {
                text: source[start..cursor].to_owned(),
                start,
                end: cursor,
                literal: true,
            });
            continue;
        }
        if let Some(end) = long_lua_end(source, cursor)? {
            tokens.push(CodeToken {
                text: source[cursor..end].to_owned(),
                start: cursor,
                end,
                literal: true,
            });
            cursor = end;
            continue;
        }
        let start = cursor;
        if bytes[cursor].is_ascii_alphabetic() || bytes[cursor] == b'_' {
            cursor += 1;
            while cursor < bytes.len()
                && (bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'_')
            {
                cursor += 1;
            }
        } else {
            cursor += source[cursor..]
                .chars()
                .next()
                .context("code character")?
                .len_utf8();
        }
        tokens.push(CodeToken {
            text: source[start..cursor].to_owned(),
            start,
            end: cursor,
            literal: false,
        });
    }
    Ok(tokens)
}

// Adapt only the existing native AdvisorItem. A token parser validates the
// complete field grammar, so commented declarations and extra terminators can
// never be discarded as harmless type annotations.
fn product_lua_for_vm(path: &std::path::Path, source: String) -> Result<String> {
    if path
        != std::path::Path::new("poc/JAC_PoC_HistoricalEvents/UI/JAC_HistoricalEventsAdvisor.lua")
    {
        return Ok(source);
    }
    let tokens = lua_code_tokens(&source)?;
    let definitions = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.literal && t.text == "hstructure")
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    if definitions.len() != 1
        || definitions[0] != 0
        || tokens.get(1).map(|t| t.text.as_str()) != Some("AdvisorItem")
    {
        bail!("expected the one official AdvisorItem declaration");
    }
    let mut actual = BTreeSet::new();
    let mut cursor = 2;
    while tokens.get(cursor).map(|t| t.text.as_str()) != Some("end") {
        let field = tokens
            .get(cursor..cursor + 4)
            .context("incomplete AdvisorItem field")?;
        if field.iter().any(|t| t.literal)
            || field[1].text != ":"
            || field[3].text != ";"
            || !["string", "number", "ifunction", "boolean", "table"]
                .contains(&field[2].text.as_str())
        {
            bail!("invalid Havok AdvisorItem field grammar");
        }
        if !actual.insert((field[0].text.clone(), field[2].text.clone())) {
            bail!("duplicate AdvisorItem field");
        }
        cursor += 4;
    }
    let declaration_end = tokens[cursor].end;
    if tokens.get(cursor + 1).map(|t| t.text.as_str()) == Some("end") {
        bail!("extra AdvisorItem terminator");
    }
    let expected: BTreeSet<_> = [
        ("Message", "string"),
        ("MessageAudio", "string"),
        ("Image", "string"),
        ("OptionsNum", "number"),
        ("Button1Text", "string"),
        ("Button2Text", "string"),
        ("Button1Func", "ifunction"),
        ("Button2Func", "ifunction"),
        ("CalloutHeader", "string"),
        ("CalloutBody", "string"),
        ("PlotCallback", "ifunction"),
        ("ShowPortrait", "boolean"),
        ("UITriggers", "table"),
    ]
    .into_iter()
    .map(|(a, b)| (a.to_owned(), b.to_owned()))
    .collect();
    if actual != expected {
        bail!("AdvisorItem fields do not match the official constructor contract");
    }
    let constructions = tokens
        .iter()
        .enumerate()
        .filter(|(_, t)| !t.literal && t.text == "hmake")
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    if constructions.len() != 1 {
        bail!("expected one AdvisorItem construction");
    }
    let index = constructions[0];
    if tokens.get(index + 1).map(|t| t.text.as_str()) != Some("AdvisorItem")
        || tokens.get(index + 2).map(|t| t.text.as_str()) != Some("{")
    {
        bail!("invalid AdvisorItem construction");
    }
    let mut edits = vec![
        (tokens[0].start, declaration_end),
        (tokens[index].start, tokens[index + 1].end),
    ];
    edits.sort_by_key(|(start, _)| std::cmp::Reverse(*start));
    let mut adapted = source;
    for (start, end) in edits {
        let blanks = adapted.as_bytes()[start..end]
            .iter()
            .map(|b| if *b == b'\n' || *b == b'\r' { *b } else { b' ' })
            .collect::<Vec<_>>();
        adapted.replace_range(start..end, std::str::from_utf8(&blanks)?);
    }
    Ok(adapted)
}

#[test]
fn havok_adapter_validates_fields_and_does_not_hide_unknown_syntax() -> Result<()> {
    let repo = Repository::checked_in();
    let path =
        std::path::Path::new("poc/JAC_PoC_HistoricalEvents/UI/JAC_HistoricalEventsAdvisor.lua");
    let source = repo.read(path)?;
    assert!(product_lua_for_vm(
        path,
        source.replace("hstructure AdvisorItem", "hstructure OtherItem")
    )
    .is_err());
    for literal in ["\"junk\"", "'junk'", "[=[junk]=]"] {
        assert!(product_lua_for_vm(
            path,
            source.replace("hmake AdvisorItem", &format!("hmake {literal} AdvisorItem"))
        )
        .is_err());
        assert!(product_lua_for_vm(
            path,
            source.replace(
                "hstructure AdvisorItem",
                &format!("hstructure {literal} AdvisorItem")
            )
        )
        .is_err());
    }
    assert!(product_lua_for_vm(path, source.replace(": ifunction;", ": unknown;")).is_err());
    assert!(product_lua_for_vm(
        path,
        source.replace(
            "hstructure AdvisorItem\n",
            "hstructure AdvisorItem\n error('hidden executable')\n"
        )
    )
    .is_err());
    let declaration_end = source.find("\nend\n").context("declaration end")? + 5;
    let commented = format!(
        "--[=[{}]=]\n{}",
        &source[..declaration_end],
        &source[declaration_end..]
    );
    assert!(product_lua_for_vm(path, commented).is_err());
    assert!(product_lua_for_vm(path, source.replacen("\nend\n", "\n  end\nend\n", 1)).is_err());
    let prefix = "--[==[ hstructure AdvisorItem end hmake AdvisorItem {} ]==]\n";
    let suffix="\nlocal quoted = \"hmake AdvisorItem {}\"\nlocal long = [=[hstructure AdvisorItem end]=]\n";
    let preserved = product_lua_for_vm(path, format!("{prefix}{source}{suffix}"))?;
    assert!(preserved.starts_with(prefix) && preserved.ends_with(suffix));
    assert!(Lua::new().load(preserved).into_function().is_ok());
    let adapter = product_lua_for_vm(path, source)?;
    assert!(Lua::new().load(adapter.clone()).into_function().is_ok());
    assert!(Lua::new()
        .load(format!("{adapter}\nlocal broken ="))
        .into_function()
        .is_err());
    Ok(())
}
