//! An explicit, map-bound diagnostic fixture for Mori's inland capital.
//! It stages a legal harbor before any numeric baseline, without creating a city
//! or changing plot ownership directly. Ordinary and historical probes stay intact.
use crate::{oath_save_probe, Repository};
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};

pub const VERSION: &str = "mori-owned-harbor-v1";

const PREPARE: &str = r#"
assert(not Game:GetProperty('JAC_TODO134_PORT_FIXTURE'),'port fixture may run only once')
local optin=GameConfiguration.GetValue('JAC_TODO134_QUEUE_HEADROOM')
assert(optin==1 or optin==true,'explicit diagnostic cost fixture required')
local p=assert(Players[0]); assert(p:IsHuman(),'port fixture requires human player zero')
local l=assert(Game:GetProperty('JAC_AGES_LEDGER'))
assert(l.houses.p0.civ=='CIVILIZATION_JAC_MORI','port fixture is only for Mori')
local majors=PlayerManager.GetAliveMajors(); assert(#majors==22,'original houses changed')
local n=0; for _,owner in ipairs(majors) do for _ in owner:GetCities():Members() do n=n+1 end end
assert(n==56,'port fixture may not create a city')
local c=assert(p:GetCities():GetCapitalCity()); local center=assert(Map.GetPlot(c:GetX(),c:GetY()))
assert(c:GetOwner()==0 and center:GetOwner()==0 and center:GetIndex()==12135,'expected original Mori capital')
local port=assert(Map.GetPlotByIndex(11522))
assert(port:GetX()==98 and port:GetY()==56 and port:GetOwner()==-1,'original unowned port site changed')
assert(GameInfo.Terrains[port:GetTerrainType()].TerrainType=='TERRAIN_COAST','port site is not coast')
local harbor=assert(GameInfo.Districts.DISTRICT_HARBOR)
local ship=assert(GameInfo.Units.UNIT_GALLEY)
assert(harbor.PrereqTech=='TECH_JAC_SENGOKU_HARBOR_PILOTS' and not harbor.PrereqCivic,'loaded harbor prerequisite does not match this fixture')
assert(ship.PrereqTech=='TECH_JAC_SENGOKU_COASTAL_PILOTAGE' and not ship.PrereqCivic,'loaded ship prerequisite does not match this fixture')
for _,row in ipairs({harbor,ship}) do
 if row.PrereqTech then p:GetTechs():SetTech(assert(GameInfo.Technologies[row.PrereqTech]).Index,true) end
 if row.PrereqCivic then p:GetCulture():SetCivic(assert(GameInfo.Civics[row.PrereqCivic]).Index,true) end
end
-- Debug/City.ltp's official population button and Debug/Player.ltp's treasury button.
assert(c:GetPopulation()==3,'original diagnostic population changed')
c:ChangePopulation(1); assert(c:GetPopulation()==4,'population staging did not complete')
local treasury=assert(p:GetTreasury()); local gold=treasury:GetGoldBalance()
assert(type(gold)=='number' and gold==gold and gold>=0,'finite treasury required')
if gold<1000 then treasury:ChangeGoldBalance(1000) end
assert(treasury:GetGoldBalance()==gold+(gold<1000 and 1000 or 0),'diagnostic funding changed unexpectedly')
Game:SetProperty('JAC_TODO134_PORT_FIXTURE',{city=c:GetID(),center=12135,port=11522,turn=Game.GetCurrentGameTurn(),population=4,gold_before=gold})
return 'Mori diagnostic population and loaded harbor/galley prerequisites staged before baseline'
"#;

const UI_COMMON: &str = r#"
local m=assert(Game:GetProperty('JAC_TODO134_PORT_FIXTURE'),'port fixture not delivered')
assert(m.turn==Game.GetCurrentGameTurn(),'port fixture turn changed')
local p=assert(Players[0]); local c=assert(p:GetCities():FindID(m.city),'fixture city missing')
local center=assert(Map.GetPlot(c:GetX(),c:GetY())); local port=assert(Map.GetPlotByIndex(m.port))
assert(c:GetOwner()==0 and center:GetOwner()==0 and center:GetIndex()==m.center,'fixture city ownership changed')
assert(c:GetPopulation()==m.population,'fixture population changed')
local harbor=assert(GameInfo.Districts.DISTRICT_HARBOR)
local function purchased()
 for _,index in pairs(Map.GetCityPlots():GetPurchasedPlots(c)) do if index==m.port then return true end end
 return false
end
local function placement()
 local params={}
 params[CityOperationTypes.PARAM_DISTRICT_TYPE]=harbor.Hash
 params[CityOperationTypes.PARAM_X]=port:GetX(); params[CityOperationTypes.PARAM_Y]=port:GetY()
 params[CityOperationTypes.PARAM_INSERT_MODE]=CityOperationTypes.VALUE_REPLACE_AT
 params[CityOperationTypes.PARAM_QUEUE_DESTINATION_LOCATION]=0
 return params
end
"#;

const PURCHASE: &str = r#"
assert(not JAC_TODO134_PORT_UI,'do not repeat a port purchase request')
assert(Game.GetLocalPlayer()==0 and p:IsTurnActive(),'actual human turn required')
local districts=assert(c:GetDistricts())
assert(districts:GetNumAllowedDistrictsRequiringPopulation()>districts:GetNumZonedDistrictsRequiringPopulation(),'no legal harbor population slot')
assert(port:GetOwner()==-1 and not purchased(),'port site already owned')
assert(port:CanHaveDistrict(harbor.Index,0,c:GetID()),'port site fails official district placement')
local mode=UI.GetInterfaceMode()
UI.SelectCity(c); assert(UI.GetHeadSelectedCity():GetID()==c:GetID(),'fixture city not selected')
-- CityPanel.lua: OnTogglePurchaseTile and PlotInfo.lua: OnClickPurchasePlot.
UI.SetInterfaceMode(InterfaceModeTypes.CITY_MANAGEMENT)
local params={}
params[CityCommandTypes.PARAM_PLOT_PURCHASE]=UI.GetInterfaceModeParameter(CityCommandTypes.PARAM_PLOT_PURCHASE)
local result=assert(CityManager.GetCommandTargets(c,CityCommandTypes.PURCHASE,params),'purchase targets unavailable')
local offered=false; for _,index in pairs(assert(result[CityCommandResults.PLOTS],'purchase plot list unavailable')) do if index==m.port then offered=true end end
assert(offered,'port site not in actual purchase targets')
local cost=c:GetGold():GetPlotPurchaseCost(m.port)
assert(type(cost)=='number' and cost==cost and cost>=0 and p:GetTreasury():GetGoldBalance()>=cost,'port purchase not affordable')
params[CityCommandTypes.PARAM_X]=port:GetX(); params[CityCommandTypes.PARAM_Y]=port:GetY()
assert(CityManager.CanStartCommand(c,CityCommandTypes.PURCHASE,params),'official port purchase denied')
JAC_TODO134_PORT_UI={city=m.city,port=m.port,turn=m.turn,mode=mode,sent=true}
CityManager.RequestCommand(c,CityCommandTypes.PURCHASE,params)
return 'one official port-site purchase requested'
"#;

const OWNED: &str = r#"
local sent=assert(JAC_TODO134_PORT_UI,'purchase request missing')
assert(sent.sent and sent.city==m.city and sent.port==m.port and sent.turn==m.turn,'port request identity changed')
return port:GetOwner()==0 and purchased()
"#;

const PERMIT: &str = r#"
assert(port:GetOwner()==0 and purchased(),'port must belong to the actual city')
assert(port:CanHaveDistrict(harbor.Index,0,c:GetID()),'owned port site fails official placement')
local allowed,results=c:GetBuildQueue():CanProduce(harbor.Hash,false,true)
local reasons={}
if results and results[CityCommandResults.FAILURE_REASONS] then
 for _,reason in pairs(results[CityCommandResults.FAILURE_REASONS]) do reasons[#reasons+1]=Locale.Lookup(reason) end
end
assert(allowed,'harbor production denied: '..table.concat(reasons,'; '))
local can=CityManager.CanStartOperation(c,CityOperationTypes.BUILD,placement(),true)
assert(can,'official harbor placement operation denied; no district created')
return 'actual city, owned plot, population slot and official harbor build permission verified'
"#;

const COMPLETE: &str = r#"
local m=assert(Game:GetProperty('JAC_TODO134_PORT_FIXTURE'))
assert(m.turn==Game.GetCurrentGameTurn() and not m.completed,'port completion may run only once in the fixture turn')
local c=assert(Players[0]:GetCities():FindID(m.city)); local port=assert(Map.GetPlotByIndex(m.port))
assert(c:GetOwner()==0 and c:GetPopulation()==m.population and port:GetOwner()==0,'port fixture ownership or population changed')
local belongs=false; for _,index in pairs(Map.GetCityPlots():GetPurchasedPlots(c)) do if index==m.port then belongs=true end end
assert(belongs,'purchased plot belongs to a different city')
local harbor=assert(GameInfo.Districts.DISTRICT_HARBOR)
assert(not c:GetDistricts():HasDistrict(harbor.Index),'port fixture may not overwrite a district')
-- TunerCityPanel.lua: g_DistrictPlacement.Place, with placement construction 100.
c:GetBuildQueue():CreateIncompleteDistrict(harbor.Index,m.port,100)
assert(c:GetDistricts():HasDistrict(harbor.Index,true),'official Tuner harbor completion failed')
m.completed=true; Game:SetProperty('JAC_TODO134_PORT_FIXTURE',m)
return 'official Tuner harbor completion at 100 verified'
"#;

const READY: &str = r#"
if not m.completed or port:GetOwner()~=0 or not purchased() then return false end
local districts=c:GetDistricts()
if not districts:HasDistrict(harbor.Index,true) then return false end
local matching=false
for _,district in districts:Members() do
 if district:GetType()==harbor.Index and district:GetX()==port:GetX() and district:GetY()==port:GetY() then matching=true end
end
assert(matching and not districts:IsPillaged(harbor.Index,m.port),'completed harbor identity or usability changed')
local ship=assert(GameInfo.Units.UNIT_GALLEY); local params={}
params[CityOperationTypes.PARAM_UNIT_TYPE]=ship.Hash
params[CityOperationTypes.PARAM_INSERT_MODE]=CityOperationTypes.VALUE_REPLACE_AT
params[CityOperationTypes.PARAM_QUEUE_DESTINATION_LOCATION]=0
assert(CityManager.CanStartOperation(c,CityOperationTypes.BUILD,params,true),'completed harbor did not enable official galley production')
local sent=assert(JAC_TODO134_PORT_UI)
if not sent.restored then UI.SetInterfaceMode(sent.mode); sent.restored=true end
return true
"#;

fn ui(code: &str) -> Value {
    json!({"action":"ui_lua","state_name":"InGame","code":format!("{UI_COMMON}\n{code}"),"timeout":30})
}

fn wait(code: &str) -> Value {
    json!({"action":"wait_for","timeout":30,"condition":{"type":"lua","state_name":"InGame","code":format!("{UI_COMMON}\n{code}")}})
}

pub fn make_probe(repo: &Repository, entry: &Value) -> Result<Value> {
    ensure!(
        entry["oath"] == "JAC_OATH_MORI_SETO_FLEETS",
        "owned harbor fixture is bound to Mori's original Japan-map capital"
    );
    let mut probe = oath_save_probe::make_native_probe(repo, entry)?;
    let steps = probe["steps"].as_array_mut().context("native steps")?;
    ensure!(
        steps[5]["action"] == "oath_exact_removal",
        "native boundary changed"
    );
    let mut requests = 0;
    const COAST_GATE: &str = " and (def.Domain~='DOMAIN_SEA' or plot:IsCoastalLand())";
    for step in steps.iter_mut() {
        if let Some(code) = step["code"].as_str() {
            if code.contains(COAST_GATE) {
                ensure!(
                    code.matches(COAST_GATE).count() == 1,
                    "production request coastal gate changed"
                );
                // The official BUILD decision includes the complete, usable harbor.
                step["code"] = json!(code.replace(COAST_GATE, ""));
                requests += 1;
            }
        }
    }
    ensure!(
        requests == 2,
        "galley and non-target land control requests must both remain"
    );
    steps.splice(
        6..6,
        [
            json!({"action":"ui_lua","state_name":"GameCore_Tuner","code":PREPARE,"timeout":30}),
            json!({"action":"wait_for","timeout":30,"condition":{"type":"lua","state_name":"InGame","code":"local m=Game:GetProperty('JAC_TODO134_PORT_FIXTURE'); if not m then return false end; assert(m.turn==Game.GetCurrentGameTurn() and m.center==12135 and m.port==11522 and m.population==4,'port setup identity changed during delivery'); return true"}}),
            ui(PURCHASE),
            wait(OWNED),
            ui(PERMIT),
            json!({"action":"ui_lua","state_name":"GameCore_Tuner","code":COMPLETE,"timeout":30}),
            wait(READY),
        ],
    );
    probe["naval_port_fixture"] = json!(VERSION);
    civ6_autotest_rs::scenario::validate_scenario(&probe)?;
    Ok(probe)
}

#[cfg(test)]
mod tests {
    use super::*;

    const UI_STATE: &str = r#"
turn=0; owner=-1; member=false; population=4; balance=1000; can=true; permitted=true; pillaged=false; built=false
requests=0; completions=0; mode=42; mode_changes=0
m={city=65536,center=12135,port=11522,turn=0,population=4}
Game={GetProperty=function()return m end,GetCurrentGameTurn=function()return turn end,GetLocalPlayer=function()return 0 end,SetProperty=function(_,_,value)m=value end}
GameInfo={Districts={DISTRICT_HARBOR={Index=7,Hash=77}},Units={UNIT_GALLEY={Hash=88}}}
local district={GetType=function()return 7 end,GetX=function()return 98 end,GetY=function()return 56 end}
districts={GetNumAllowedDistrictsRequiringPopulation=function()return 2 end,GetNumZonedDistrictsRequiringPopulation=function()return 1 end,
 HasDistrict=function()return built end,IsPillaged=function()return pillaged end,Members=function()local done=false; return function()if not done and built then done=true;return 1,district end end end}
queue={CanProduce=function()return permitted,{} end,CreateIncompleteDistrict=function(_,kind,index,percent)assert(kind==7 and index==11522 and percent==100);completions=completions+1;built=true end}
city={GetID=function()return 65536 end,GetOwner=function()return 0 end,GetX=function()return 99 end,GetY=function()return 59 end,
 GetPopulation=function()return population end,GetDistricts=function()return districts end,GetBuildQueue=function()return queue end,
 GetGold=function()return {GetPlotPurchaseCost=function()return 150 end} end}
Players={[0]={IsTurnActive=function()return true end,GetCities=function()return {FindID=function(_,id)if id==65536 then return city end end}end,
 GetTreasury=function()return {GetGoldBalance=function()return balance end}end}}
port={GetOwner=function()return owner end,GetX=function()return 98 end,GetY=function()return 56 end,CanHaveDistrict=function()return permitted end}
Map={GetPlotByIndex=function(id)assert(id==11522);return port end,GetPlot=function()return {GetOwner=function()return 0 end,GetIndex=function()return 12135 end}end,
 GetCityPlots=function()return {GetPurchasedPlots=function(_,c)assert(c==city);return member and {11522} or {} end}end}
UI={GetInterfaceMode=function()return mode end,SelectCity=function(c)assert(c==city)end,GetHeadSelectedCity=function()return city end,
 SetInterfaceMode=function(value)mode=value;mode_changes=mode_changes+1 end,GetInterfaceModeParameter=function()return nil end}
InterfaceModeTypes={CITY_MANAGEMENT=5}; CityCommandTypes={PARAM_PLOT_PURCHASE=1,PARAM_X=2,PARAM_Y=3,PURCHASE=4}
CityCommandResults={PLOTS=1,FAILURE_REASONS=2};CityOperationTypes={PARAM_DISTRICT_TYPE=1,PARAM_X=2,PARAM_Y=3,PARAM_INSERT_MODE=4,VALUE_REPLACE_AT=5,PARAM_QUEUE_DESTINATION_LOCATION=6,PARAM_UNIT_TYPE=7,BUILD=8}
CityManager={GetCommandTargets=function()return {[1]={11522}}end,CanStartCommand=function()return can end,
 RequestCommand=function(_,_,params)assert(params[2]==98 and params[3]==56);requests=requests+1 end,
 CanStartOperation=function()return can end}
Locale={Lookup=function(v)return tostring(v)end}
"#;

    const CORE_SETUP: &str = r#"
population=3;gold_changes=0;tech_changes=0;fixture=nil
local ledger={houses={p0={civ='CIVILIZATION_JAC_MORI'}}}
Game.GetProperty=function(_,name)if name=='JAC_AGES_LEDGER' then return ledger else return fixture end end
Game.SetProperty=function(_,_,value)fixture=value end
GameConfiguration={GetValue=function()return 1 end}
Players[0].IsHuman=function()return true end
city.ChangePopulation=function(_,delta)assert(delta==1);population=population+delta end
local cities=Players[0]:GetCities();cities.GetCapitalCity=function()return city end
cities.Members=function()local i=0;return function()i=i+1;if i==1 then return i,city end end end
Players[0].GetCities=function()return cities end
local treasury={GetGoldBalance=function()return balance end,ChangeGoldBalance=function(_,delta)assert(delta==1000);balance=balance+delta;gold_changes=gold_changes+1 end}
Players[0].GetTreasury=function()return treasury end
Players[0].GetTechs=function()return {SetTech=function(_,index,value)assert((index==31 or index==32) and value);tech_changes=tech_changes+1 end}end
GameInfo.Districts.DISTRICT_HARBOR.PrereqTech='TECH_JAC_SENGOKU_HARBOR_PILOTS'
GameInfo.Units.UNIT_GALLEY.PrereqTech='TECH_JAC_SENGOKU_COASTAL_PILOTAGE'
GameInfo.Technologies={TECH_JAC_SENGOKU_HARBOR_PILOTS={Index=31},TECH_JAC_SENGOKU_COASTAL_PILOTAGE={Index=32}}
GameInfo.Terrains={[1]={TerrainType='TERRAIN_COAST'}};port.GetTerrainType=function()return 1 end
PlayerManager={GetAliveMajors=function()local list={Players[0]};for i=1,21 do local count=i<=13 and 3 or 2;list[#list+1]={GetCities=function()return {Members=function()local j=0;return function()j=j+1;if j<=count then return j,{} end end end}end}end;return list end}
"#;

    #[test]
    fn loaded_prerequisites_are_checked_before_population_and_diagnostic_funding() -> Result<()> {
        for balance in [0, 1000] {
            let vm = mlua::Lua::new();
            vm.load(UI_STATE).exec()?;
            vm.load(CORE_SETUP).exec()?;
            vm.load(format!("balance={balance}")).exec()?;
            vm.load(PREPARE).eval::<String>()?;
            assert_eq!(vm.load("return population").eval::<i64>()?, 4);
            assert_eq!(vm.load("return tech_changes").eval::<i64>()?, 2);
            assert_eq!(vm.load("return balance").eval::<i64>()?, 1000);
            assert_eq!(
                vm.load("return gold_changes").eval::<i64>()?,
                i64::from(balance == 0)
            );
            assert!(vm.load(PREPARE).eval::<String>().is_err());
            assert_eq!(vm.load("return population").eval::<i64>()?, 4);
        }
        for change in [
            "GameInfo.Districts.DISTRICT_HARBOR.PrereqTech='TECH_CELESTIAL_NAVIGATION'",
            "GameInfo.Units.UNIT_GALLEY.PrereqTech='TECH_SAILING'",
            "GameInfo.Districts.DISTRICT_HARBOR.PrereqCivic='WRONG'",
        ] {
            let vm = mlua::Lua::new();
            vm.load(UI_STATE).exec()?;
            vm.load(CORE_SETUP).exec()?;
            vm.load(change).exec()?;
            assert!(
                vm.load(PREPARE).eval::<String>().is_err(),
                "accepted {change}"
            );
            assert_eq!(vm.load("return population").eval::<i64>()?, 3);
            assert_eq!(vm.load("return tech_changes").eval::<i64>()?, 0);
            assert_eq!(vm.load("return gold_changes").eval::<i64>()?, 0);
        }
        Ok(())
    }

    #[test]
    fn purchase_is_single_and_ownership_must_arrive_before_legal_completion() -> Result<()> {
        let vm = mlua::Lua::new();
        vm.load(UI_STATE).exec()?;
        vm.load(format!("{UI_COMMON}\n{PURCHASE}"))
            .eval::<String>()?;
        assert_eq!(vm.load("return requests").eval::<i64>()?, 1);
        assert!(vm
            .load(format!("{UI_COMMON}\n{PURCHASE}"))
            .eval::<String>()
            .is_err());
        assert!(!vm.load(format!("{UI_COMMON}\n{OWNED}")).eval::<bool>()?);
        vm.load("owner=0").exec()?;
        assert!(!vm.load(format!("{UI_COMMON}\n{OWNED}")).eval::<bool>()?);
        assert!(vm.load(COMPLETE).eval::<String>().is_err());
        assert_eq!(vm.load("return completions").eval::<i64>()?, 0);
        vm.load("member=true").exec()?;
        assert!(vm.load(format!("{UI_COMMON}\n{OWNED}")).eval::<bool>()?);
        vm.load("can=false").exec()?;
        assert!(vm
            .load(format!("{UI_COMMON}\n{PERMIT}"))
            .eval::<String>()
            .is_err());
        vm.load("can=true").exec()?;
        vm.load(format!("{UI_COMMON}\n{PERMIT}")).eval::<String>()?;
        vm.load(COMPLETE).eval::<String>()?;
        assert_eq!(vm.load("return completions").eval::<i64>()?, 1);
        assert!(vm.load(COMPLETE).eval::<String>().is_err());
        vm.load("pillaged=true").exec()?;
        assert!(vm
            .load(format!("{UI_COMMON}\n{READY}"))
            .eval::<bool>()
            .is_err());
        vm.load("pillaged=false;can=false").exec()?;
        assert!(vm
            .load(format!("{UI_COMMON}\n{READY}"))
            .eval::<bool>()
            .is_err());
        vm.load("can=true").exec()?;
        assert!(vm.load(format!("{UI_COMMON}\n{READY}")).eval::<bool>()?);
        assert_eq!(vm.load("return mode").eval::<i64>()?, 42);
        assert!(vm.load(format!("{UI_COMMON}\n{READY}")).eval::<bool>()?);
        assert_eq!(vm.load("return mode_changes").eval::<i64>()?, 2);
        vm.load("turn=1").exec()?;
        assert!(vm
            .load(format!("{UI_COMMON}\n{READY}"))
            .eval::<bool>()
            .is_err());
        Ok(())
    }

    #[test]
    fn denied_or_unaffordable_purchase_never_sends_an_operation() -> Result<()> {
        for change in [
            "balance=149",
            "can=false",
            "permitted=false",
            "owner=1",
            "population=3",
            "m.city=123",
            "turn=1",
            "CityManager.GetCommandTargets=function()return {[1]={123}}end",
        ] {
            let vm = mlua::Lua::new();
            vm.load(UI_STATE).exec()?;
            vm.load(change).exec()?;
            assert!(
                vm.load(format!("{UI_COMMON}\n{PURCHASE}"))
                    .eval::<String>()
                    .is_err(),
                "accepted {change}"
            );
            assert_eq!(vm.load("return requests").eval::<i64>()?, 0);
            assert_eq!(vm.load("return completions").eval::<i64>()?, 0);
        }
        Ok(())
    }

    #[test]
    fn fixture_precedes_baseline_and_preserves_original_targets_and_turns() -> Result<()> {
        let repo = Repository::checked_in();
        let entries = oath_save_probe::catalogue(&repo)?;
        let entry = entries
            .iter()
            .find(|e| e["oath"] == "JAC_OATH_MORI_SETO_FLEETS")
            .unwrap();
        let plain = oath_save_probe::make_native_probe(&repo, entry)?;
        let port = make_probe(&repo, entry)?;
        assert_eq!(port["finite_turns"], plain["finite_turns"]);
        assert_eq!(port["naval_port_fixture"], VERSION);
        let mut restored = port.clone();
        restored
            .as_object_mut()
            .unwrap()
            .remove("naval_port_fixture");
        let steps = restored["steps"].as_array_mut().unwrap();
        steps.drain(6..13);
        for step in steps {
            if let Some(code) = step["code"].as_str() {
                if code.contains("local can=CityManager.CanStartOperation(city,CityOperationTypes.BUILD,params,true)") {
                    step["code"]=json!(code.replace("if can and city:GetOwner()==0 and plot:GetOwner()==0 then","if can and city:GetOwner()==0 and plot:GetOwner()==0 and (def.Domain~='DOMAIN_SEA' or plot:IsCoastalLand()) then"));
                }
            }
        }
        assert_eq!(restored, plain);
        let vm = mlua::Lua::new();
        for step in port["steps"].as_array().unwrap() {
            if let Some(code) = step["code"]
                .as_str()
                .or_else(|| step["condition"]["code"].as_str())
            {
                vm.load(code).into_function()?;
            }
        }
        for other in entries.iter().filter(|e| e["oath"] != entry["oath"]) {
            assert!(make_probe(&repo, other).is_err());
        }
        Ok(())
    }
}
