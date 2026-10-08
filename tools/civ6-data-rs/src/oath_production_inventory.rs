//! Preserve every original city while explicitly auditing new foreign cities.
//! Natural AI founding cannot supply a missing baseline for the subject player.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

pub const VERSION: &str = "original56-plus-new-foreign-v1";
const CAPTURE: &str = r#"
assert(not Game:GetProperty('JAC_TODO134_ORIGINAL_CITIES'),'original inventory may be captured only once')
local majors=PlayerManager.GetAliveMajors();assert(#majors==22,'original houses changed')
local cities={};local n=0
for _,p in ipairs(majors) do for _,c in p:GetCities():Members() do
 local plot=assert(Map.GetPlot(c:GetX(),c:GetY()));local key=tostring(plot:GetIndex())
 assert(c:GetOwner()==p:GetID() and plot:GetOwner()==p:GetID() and not cities[key],'original city identity/owner invalid')
 cities[key]={id=c:GetID(),owner=p:GetID(),plot=plot:GetIndex()};n=n+1
end end
assert(n==56,'capture requires exactly the original 56 cities')
Game:SetProperty('JAC_TODO134_ORIGINAL_CITIES',{turn=Game.GetCurrentGameTurn(),cities=cities})
return 'all original 56 city identities captured before production turns'
"#;

const INVENTORY_GUARD: &str = r#"
local original=assert(Game:GetProperty('JAC_TODO134_ORIGINAL_CITIES'),'original inventory missing')
assert(original.turn<=Game.GetCurrentGameTurn(),'original inventory from a future turn')
local original_count=0
for key,old in pairs(original.cities) do
 local current=assert(cities[key],'an original city disappeared')
 assert(current.id==old.id and current.owner==old.owner and current.plot:GetIndex()==old.plot,'original city identity or owner changed')
 original_count=original_count+1
end
assert(original_count==56,'original inventory incomplete')
for key,current in pairs(cities) do
 if not original.cities[key] then assert(current.owner~=0,'new subject city has no numeric baseline') end
end
assert(count>=56,'actual city inventory lost original cities')
"#;

const FINAL_MATCH: &str = "local old=assert(original[plot:GetIndex()],'a new city appeared during production diagnostics')\n assert(old.owner==p:GetID() and old.id==city:GetID(),'original city owner or identity changed during production diagnostics')";
const FINAL_GUARD: &str = r#"local old=original[plot:GetIndex()]
 if old then
  assert(old.owner==p:GetID() and old.id==city:GetID(),'original city owner or identity changed during production diagnostics')
  assert(not original_seen[plot:GetIndex()],'original city counted twice');original_seen[plot:GetIndex()]=true
 else
  assert(p:GetID()~=0,'new subject city has no numeric baseline')
  new_foreign=new_foreign+1
 end"#;
const FINAL_COUNT: &str = r#"local capture=assert(Game:GetProperty('JAC_TODO134_ORIGINAL_CITIES'))
local original_count=0
for key,old in pairs(capture.cities) do
 local baseline=assert(original[old.plot],'original city absent from numeric baseline')
 assert(baseline.id==old.id and baseline.owner==old.owner and original_seen[old.plot],'original city missing or changed')
 original_count=original_count+1
end
assert(original_count==56 and count==56+new_foreign,'actual/original/new-city inventory mismatch')
all.new_foreign_city_count=new_foreign"#;

pub fn decorate(mut probe: Value) -> Result<Value> {
    ensure!(
        probe["native_exact_removal_candidate"] == true
            && probe["finite_turns"].as_u64().is_some_and(|n| n > 0),
        "expanded inventory requires a native production/save probe"
    );
    ensure!(
        probe.get("production_inventory_scope").is_none(),
        "inventory scope already set"
    );
    let steps = probe["steps"]
        .as_array_mut()
        .context("production probe steps")?;
    ensure!(
        steps[3]["action"] == "start_game" && steps[4]["action"] == "wait_for",
        "production startup boundary changed"
    );
    let (mut saves, mut finals) = (0, 0);
    for step in steps.iter_mut() {
        let Some(original) = step["code"].as_str() else {
            continue;
        };
        let mut code = original.to_owned();
        const SAVE_COUNT: &str = "assert(count==56,'expected original 56 real cities')";
        if code.contains(SAVE_COUNT) {
            ensure!(
                code.matches(SAVE_COUNT).count() == 1,
                "save inventory boundary changed"
            );
            code = code.replace(SAVE_COUNT, INVENTORY_GUARD);
            saves += 1;
        }
        if code.contains(FINAL_MATCH) {
            code = code.replace(
                "local original={}; local count=0",
                "local original={}; local count=0;local original_seen={};local new_foreign=0",
            );
            code = code.replace(FINAL_MATCH, FINAL_GUARD);
            let count =
                "assert(count==56,'original city inventory changed during production diagnostics')";
            ensure!(
                code.contains(count),
                "final inventory count boundary changed"
            );
            code = code.replace(count, FINAL_COUNT);
            const REPORT: &str =
                "\"diagnostic_restore_observed\":true,\"production_results_complete\":";
            ensure!(
                code.matches(REPORT).count() == 1,
                "final inventory report boundary changed"
            );
            code=code.replace(REPORT,"\"diagnostic_restore_observed\":true,\"original_city_count\":56,\"actual_city_count\":'..count..',\"new_foreign_city_count\":'..new_foreign..',\"production_results_complete\":");
            finals += 1;
        }
        if code != original {
            step["code"] = json!(code);
        }
    }
    ensure!(
        saves == 4 && finals == 1,
        "every Core/UI save frame and production restore must audit the expanded inventory"
    );
    steps.insert(
        5,
        json!({"action":"ui_lua","state_name":"GameCore_Tuner","code":CAPTURE,"timeout":30}),
    );
    probe["production_inventory_scope"] = json!(VERSION);
    civ6_autotest_rs::scenario::validate_scenario(&probe)?;
    Ok(probe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_executes_original_identity_and_new_foreign_flag_checks() -> Result<()> {
        let repo = crate::Repository::checked_in();
        let entry = crate::oath_save_probe::catalogue(&repo)?
            .into_iter()
            .find(|e| e["oath"] == "JAC_OATH_MIYOSHI_IIMORI")
            .unwrap();
        let probe = decorate(crate::oath_save_probe::make_native_probe(&repo, &entry)?)?;
        let code = probe["steps"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|s| s["code"].as_str())
            .find(|c| c.contains("all.new_foreign_city_count=new_foreign"))
            .context("final comparison")?;
        let fixture = r#"
oath='JAC_OATH_MIYOSHI_IIMORI';records={};capture={turn=1,cities={}}
JAC_OATH_AUDIT={quantities_verified=true,yield_growth_observed=true,baseline={own={},foreign={}},production={}}
for i=0,55 do
 local r={id=i,plot=i,owner=i%22};records[i]=r;capture.cities[tostring(i)]={id=i,plot=i,owner=r.owner}
 local group=r.owner==0 and JAC_OATH_AUDIT.baseline.own or JAC_OATH_AUDIT.baseline.foreign
 group[#group+1]={id=i,plot=i,owner=r.owner}
end
for _,target in ipairs({'BUILDING_WALLS','BUILDING_CASTLE','BUILDING_STAR_FORT','BUILDING_MONUMENT'}) do
 JAC_OATH_AUDIT.production[target]={quantity_verified=true,expected_percent=target=='BUILDING_MONUMENT' and 0 or 20,sample_count=3,off_first={},on={},off_last={}}
end
ledger={reign=1,houses={p0={civ='CIVILIZATION_JAC_MIYOSHI',oathChosen=1,oaths={[oath]=true}}}}
Game={GetProperty=function(_,key)return key=='JAC_AGES_LEDGER' and ledger or capture end,GetCurrentGameTurn=function()return 17 end}
local function city(r)return {GetOwner=function()return r.owner end,GetID=function()return r.id end,GetX=function()return r.plot end,GetY=function()return 0 end}end
local majors={};for id=0,21 do local owner=id
 majors[#majors+1]={GetID=function()return owner end,GetCities=function()return {Members=function()
 local index=-1;return function()repeat index=index+1 until index>100 or (records[index] and records[index].owner==owner)
 if index<=100 then return index,city(records[index])end end end}end}
end
PlayerManager={GetAliveMajors=function()return majors end}
Map={GetPlot=function(x)local r=assert(records[x]);return {GetIndex=function()return r.plot end,GetOwner=function()return r.owner end,
 GetProperty=function(_,key)if key==oath..'_ON' then return r.wrong_flag or (r.owner==0 and 1 or 0) end;return 0 end}end}
GameInfo={JAC_SengokuOaths=function()local i=0;return function()i=i+1;if i<=66 then return {OathType=i==1 and oath or ('OTHER_'..i)}end end end}
"#;
        for (change, count) in [("", 56), ("records[57]={id=57,plot=57,owner=1}", 57)] {
            let vm = mlua::Lua::new();
            vm.load(fixture).exec()?;
            vm.load(change).exec()?;
            let raw = vm.load(code).eval::<String>()?;
            let result: Value = serde_json::from_str(&raw)?;
            assert_eq!(result["accepted"], true);
            assert_eq!(result["original_city_count"], 56);
            assert_eq!(result["actual_city_count"], count);
            assert_eq!(result["new_foreign_city_count"], count - 56);
        }
        for change in [
            "records[2]=nil",
            "records[2].id=99",
            "records[2].owner=3",
            "records[57]={id=57,plot=57,owner=0}",
            "records[57]={id=57,plot=57,owner=1,wrong_flag=1}",
            "records[57]={id=57,plot=2,owner=2}",
        ] {
            let vm = mlua::Lua::new();
            vm.load(fixture).exec()?;
            vm.load(change).exec()?;
            assert!(vm.load(code).eval::<String>().is_err(), "accepted {change}");
        }
        Ok(())
    }

    #[test]
    fn original_cities_cannot_be_lost_or_changed_and_new_subject_cities_are_rejected() -> Result<()>
    {
        let original = (0..56)
            .map(|i| {
                format!(
                    "['{i}']={{id={i},owner={},plot={i}}}",
                    if i == 0 { 0 } else { 1 }
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let fixture = format!(
            r#"
original={{turn=1,cities={{{original}}}}};cities={{}};count=56
for key,old in pairs(original.cities) do local index=old.plot;cities[key]={{id=old.id,owner=old.owner,plot={{GetIndex=function()return index end}}}} end
Game={{GetProperty=function()return original end,GetCurrentGameTurn=function()return 9 end}}
"#
        );
        for change in [
            "",
            "cities['57']={id=57,owner=1,plot={GetIndex=function()return 57 end}};count=57",
        ] {
            let vm = mlua::Lua::new();
            vm.load(&fixture).exec()?;
            vm.load(change).exec()?;
            vm.load(INVENTORY_GUARD).exec()?;
        }
        for change in [
            "cities['0']=nil;count=55",
            "cities['2'].id=99",
            "cities['2'].owner=3",
            "cities['57']={id=57,owner=0,plot={GetIndex=function()return 57 end}};count=57",
            "original.cities['2']=nil",
        ] {
            let vm = mlua::Lua::new();
            vm.load(&fixture).exec()?;
            vm.load(change).exec()?;
            assert!(
                vm.load(INVENTORY_GUARD).exec().is_err(),
                "accepted {change}"
            );
        }
        Ok(())
    }

    #[test]
    fn expanded_production_saves_keep_every_actual_city_and_every_original_assertion() -> Result<()>
    {
        let repo = crate::Repository::checked_in();
        let entries = crate::oath_save_probe::catalogue(&repo)?;
        for oath in ["JAC_OATH_MORI_SETO_FLEETS", "JAC_OATH_MIYOSHI_IIMORI"] {
            let entry = entries.iter().find(|e| e["oath"] == oath).unwrap();
            let base = if oath == "JAC_OATH_MORI_SETO_FLEETS" {
                crate::oath_naval_port::make_probe(&repo, entry)?
            } else {
                crate::oath_save_probe::make_native_probe(&repo, entry)?
            };
            let result = decorate(base.clone())?;
            assert_eq!(result["finite_turns"], base["finite_turns"]);
            assert_eq!(result["production_inventory_scope"], VERSION);
            let vm = mlua::Lua::new();
            for step in result["steps"].as_array().unwrap() {
                if let Some(code) = step["code"]
                    .as_str()
                    .or_else(|| step["condition"]["code"].as_str())
                {
                    vm.load(code).into_function()?;
                }
            }
        }
        Ok(())
    }
}
