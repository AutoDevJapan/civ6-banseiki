use serde_json::{json, Value};
use std::{fs, path::PathBuf};

const SETUP: &str = r#"local from,to
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
 local name=PlayerConfigurations[p:GetID()]:GetCivilizationTypeName()
 if name=='CIVILIZATION_JAC_ASHINA' then from=p:GetID() elseif name=='CIVILIZATION_JAC_DATE' then to=p:GetID() end
end
assert(from and to and not Players[from]:IsHuman() and not Players[to]:IsHuman(),'need two AI houses')
local p=Players[from];local cap=assert(p:GetCities():GetCapitalCity())
local r=assert(GameInfo.Religions.RELIGION_CUSTOM_1)
assert(p:GetReligion():GetReligionTypeCreated()<0,'fixture religion already founded')
p:GetReligion():SetHolyCity(cap);Game.GetReligion():FoundReligion(from,r.Index)
for _,kind in ipairs({'BELIEF_DIVINE_INSPIRATION','BELIEF_CATHEDRAL'}) do Game.GetReligion():AddBelief(from,GameInfo.Beliefs[kind].Index) end
cap:GetReligion():SetAllCityToReligion(r.Index)
local target=assert(Players[Game.GetLocalPlayer()]:GetCities():GetCapitalCity())
assert(target:GetReligion():GetMajorityReligion()~=r.Index,'target already converted')
local before={};for _,u in p:GetUnits():Members() do before[u:GetID()]=true end
UnitManager.InitUnitValidAdjacentHex(from,'UNIT_MISSIONARY',target:GetX(),target:GetY(),8)
local priest
for _,u in p:GetUnits():Members() do if not before[u:GetID()] then assert(not priest,'ambiguous new unit');priest=u end end
assert(priest and GameInfo.Units[priest:GetType()].UnitType=='UNIT_MISSIONARY','missionary creation failed')
priest:GetReligion():SetReligionType(r.ReligionType)
UnitManager.PlaceUnit(priest,target:GetX(),target:GetY())
assert(priest:GetX()==target:GetX() and priest:GetY()==target:GetY(),'missionary target placement failed')
local e={From=from,To=to,UnitID=priest:GetID(),Religion=r.Index,TargetOwner=target:GetOwner(),TargetID=target:GetID(),X=target:GetX(),Y=target:GetY(),StartTurn=Game.GetCurrentGameTurn()}
Game:SetProperty('JAC_ABSORPTION_SPENT_SPREAD_192',e)
print('[JAC_SPENT_SPREAD_192] fixture owner='..from..' unit='..e.UnitID..' gameplay_request='..type(UnitManager.RequestOperation)..' gameplay_can='..type(UnitManager.CanStartOperation))
return 'AI missionary fixture created without changing spread charges'"#;

const ARM: &str = r#"local e=assert(Game:GetProperty('JAC_ABSORPTION_SPENT_SPREAD_192'))
local u=assert(Players[e.From]:GetUnits():FindID(e.UnitID));assert(u:GetID()==e.UnitID and u:GetOwner()==e.From,'fixture identity mismatch')
assert(u:GetSpreadCharges()==3 and u:GetReligionType()==e.Religion,'fixture native state mismatch')
JAC192_SPREAD={Owner=e.From,ID=e.UnitID,Before=u:GetSpreadCharges(),Requests=0,Events={}}
local function charges(owner,id)
 if owner==e.From and id==e.UnitID then
  local unit=Players[owner]:GetUnits():FindID(id)
  JAC192_SPREAD.Events[#JAC192_SPREAD.Events+1]={Turn=Game.GetCurrentGameTurn(),Charges=unit and unit:GetSpreadCharges() or -1}
 end
end
Events.UnitChargesChanged.Add(charges)
local function send(owner)
 if owner~=e.From or JAC192_SPREAD.Requests~=0 then return end
 Events.PlayerTurnActivated.Remove(send)
 local unit=assert(Players[owner]:GetUnits():FindID(e.UnitID));assert(unit:GetID()==e.UnitID,'AI unit identity reused')
 local op=assert(UnitOperationTypes.SPREAD_RELIGION,'official spread operation absent')
 local ok,reasons=UnitManager.CanStartOperation(unit,op,nil,false,true)
 JAC192_SPREAD.CanStart=ok;JAC192_SPREAD.Active=Players[owner]:IsTurnActive();JAC192_SPREAD.Turn=Game.GetCurrentGameTurn()
 print('[JAC_SPENT_SPREAD_192] official_can='..tostring(ok)..' active='..tostring(JAC192_SPREAD.Active)..' local='..Game.GetLocalPlayer()..' owner='..owner)
 if not ok then JAC192_SPREAD.Error='official AI spread unavailable';return end
 JAC192_SPREAD.Requests=1
 local accepted,result=pcall(UnitManager.RequestOperation,unit,op)
 JAC192_SPREAD.CallOK=accepted;JAC192_SPREAD.Result=tostring(result)
 print('[JAC_SPENT_SPREAD_192] official_request='..tostring(accepted)..' result='..tostring(result))
end
Events.PlayerTurnActivated.Add(send)
return 'one official spread request armed for donor AI turn'"#;

const VERIFY: &str = r#"local e=assert(Game:GetProperty('JAC_ABSORPTION_SPENT_SPREAD_192'));local o=assert(JAC192_SPREAD)
local u=assert(Players[e.From]:GetUnits():FindID(e.UnitID),'spent donor disappeared')
assert(u:GetID()==e.UnitID and u:GetOwner()==e.From and u:GetType()==GameInfo.Units.UNIT_MISSIONARY.Index,'donor identity changed')
local n=u:GetSpreadCharges()
print('[JAC_SPENT_SPREAD_192] observed before='..o.Before..' after='..n..' requests='..o.Requests..' can='..tostring(o.CanStart)..' call='..tostring(o.CallOK)..' result='..tostring(o.Result))
assert(o.Requests==1 and o.CanStart and o.CallOK,'official AI request not sent')
assert(n>0 and n<o.Before,'official AI spread did not consume native charge')
return ExposedMembers.JAC_Absorption.Canonical({Fixture=e,Observation=o,Charges=n,Owner=u:GetOwner(),ID=u:GetID(),Religion=u:GetReligionType()})"#;

const APPLY: &str = r#"local A=assert(JAC_AbsorptionGameplay);local e=assert(Game:GetProperty('JAC_ABSORPTION_SPENT_SPREAD_192'))
local u=assert(UnitManager.GetUnit(e.From,e.UnitID));assert(u:GetID()==e.UnitID and u:GetOwner()==e.From,'donor identity mismatch')
local source=A.SnapshotUnit(u);assert(source.SpreadCharges>0 and source.SpreadCharges<3,'actual spent charges required')
e.SpentCharges=source.SpreadCharges;e.SourceUnits={};e.SourceCities={}
local plan={ID='spent-spread-192',From=e.From,To=e.To,Mode='MERGER',EntryConditionsMet=true,RelationsResolved=true,Leader='RECRUIT',Units={},Assets={}}
for _,unit in Players[e.From]:GetUnits():Members() do
 e.SourceUnits[#e.SourceUnits+1]={ID=unit:GetID(),Type=unit:GetType()}
 plan.Units[A.Rules.UnitKey(e.From,unit:GetID())]='INCORPORATE'
end
for _,c in Players[e.From]:GetCities():Members() do e.SourceCities[#e.SourceCities+1]={ID=c:GetID(),X=c:GetX(),Y=c:GetY()} end
Game:SetProperty('JAC_ABSORPTION_SPENT_SPREAD_192',e)
local ok,tx=pcall(A.Apply,plan)
if not ok then tx=assert(A.GetState().Transactions[plan.ID]);assert(tx.Status=='BLOCKED','unexpected absorption failure '..tostring(tx.Error)) end
return tx.Status"#;

const RESUME: &str = r#"local A=assert(JAC_AbsorptionGameplay);local tx=assert(A.GetState().Transactions['spent-spread-192'])
if tx.Status=='BLOCKED' then return 'BLOCKED' end
assert(tx.Status=='WAITING_UI','absorption unexpectedly advanced to '..tostring(tx.Status))
local ok,result=pcall(A.Resume,tx.ID)
tx=assert(A.GetState().Transactions[tx.ID])
if not ok then assert(tx.Status=='BLOCKED','unexpected absorption failure '..tostring(result)) end
return tx.Status"#;

const BLOCKED: &str = r#"local A=assert(JAC_AbsorptionGameplay);local e=assert(Game:GetProperty('JAC_ABSORPTION_SPENT_SPREAD_192'))
local tx=assert(A.GetState().Transactions['spent-spread-192'])
assert(tx.Status=='BLOCKED' and tostring(tx.Error):find('spent spread charges',1,true),'expected native spread mismatch block; '..tostring(tx.Status)..' '..tostring(tx.Error))
local old=assert(UnitManager.GetUnit(e.From,e.UnitID),'spent original unit was deleted')
assert(old:GetID()==e.UnitID and old:GetOwner()==e.From and old:GetType()==GameInfo.Units.UNIT_MISSIONARY.Index,'spent original identity changed')
assert(A.SnapshotUnit(old).SpreadCharges==e.SpentCharges,'spent original charges changed')
for _,saved in ipairs(e.SourceUnits) do local unit=assert(UnitManager.GetUnit(e.From,saved.ID),'another donor deleted');assert(unit:GetID()==saved.ID and unit:GetOwner()==e.From and unit:GetType()==saved.Type,'another donor changed') end
for _,saved in ipairs(e.SourceCities) do local c=assert(CityManager.GetCityAt(saved.X,saved.Y));assert(c:GetID()==saved.ID and c:GetOwner()==e.From,'donor city transferred before mismatch') end
local snap
for _,s in ipairs(tx.Units) do if s.ID==e.UnitID then assert(not snap,'duplicate donor snapshot');snap=s end end
assert(snap and snap.SpreadCharges==e.SpentCharges,'snapshot did not retain actual spent charges')
local candidate=assert(A.FindUnit(e.To,snap.Token),'staged candidate was lost')
local n=A.SnapshotUnit(candidate).SpreadCharges
assert(n==3 and n~=snap.SpreadCharges,'native candidate mismatch not reproduced')
print('[JAC_SPENT_SPREAD_192] blocked preserved owner='..e.From..' original='..e.UnitID..' original_charges='..e.SpentCharges..' staged='..candidate:GetID()..' staged_charges='..n..' reason='..tx.Error)
return A.Canonical({Status=tx.Status,Error=tx.Error,Original={Owner=old:GetOwner(),ID=old:GetID(),Charges=e.SpentCharges},Candidate={Owner=candidate:GetOwner(),ID=candidate:GetID(),Charges=n},SourceUnits=e.SourceUnits,SourceCities=e.SourceCities})"#;

const READY: &str = r#"local tx=assert(ExposedMembers.JAC_Absorption.GetState().Transactions['spent-spread-192'])
if tx.Status=='BLOCKED' then return true end
local reads=assert(ExposedMembers.JAC_HeroReadAPI)
if tx.AwaitingPreparation then
 if tx.SourceReadyToken and not reads.GetObservation(tx.SourceReadyToken) then return false end
 for _,r in pairs(tx.SourceSpreadReads or {}) do if reads.GetUnitObservation(r.Token)==nil then return false end end
end
for _,snap in ipairs(tx.Units or {}) do for _,r in pairs(snap.SpreadReads or {}) do if r.AwaitingUI and reads.GetUnitObservation(r.Token)==nil then return false end end end
return true"#;

fn generate(mut base: Value) -> Value {
    let mut steps: Vec<Value> = base["steps"].as_array().unwrap()[..5].to_vec();
    steps.push(json!({"action":"ui_lua","state_name":"JAC_Absorption_Entry","code":SETUP}));
    steps.push(json!({"action":"wait_for","timeout":45,"condition":{"type":"lua","state_name":"InGame","code":"local e=Game:GetProperty('JAC_ABSORPTION_SPENT_SPREAD_192');if not e then return false end;local u=Players[e.From]:GetUnits():FindID(e.UnitID);return u and u:GetID()==e.UnitID and u:GetReligionType()==e.Religion and u:GetSpreadCharges()==3"}}));
    steps.push(json!({"action":"ui_lua","state_name":"InGame","code":ARM}));
    steps.push(json!({"action":"end_turn","turns":1,"timeout":600}));
    steps.push(json!({"action":"ui_lua","state_name":"InGame","code":VERIFY}));
    steps.push(json!({"action":"save","name":"jac-spent-spread-192"}));
    base["name"] = json!("AI official spread operation diagnostic for ToDo 192");
    base["steps"] = json!(steps);
    base
}

fn full(mut probe: Value) -> Value {
    let steps = probe["steps"].as_array_mut().unwrap();
    steps.push(json!({"action":"ui_lua","state_name":"JAC_Absorption_Entry","code":APPLY}));
    for _ in 0..8 {
        steps.push(json!({"action":"wait_for","timeout":45,"condition":{"type":"lua","state_name":"InGame","code":READY}}));
        steps.push(json!({"action":"ui_lua","state_name":"JAC_Absorption_Entry","code":RESUME}));
    }
    steps.push(json!({"action":"ui_lua","state_name":"JAC_Absorption_Entry","code":BLOCKED,"capture":"SPENT_SPREAD_BLOCKED"}));
    steps.push(json!({"action":"save","name":"jac-spent-spread-blocked-192"}));
    steps.push(json!({"action":"exit_to_menu"}));
    steps.push(json!({"action":"load","name":"jac-spent-spread-blocked-192","timeout":600}));
    steps.push(json!({"action":"wait_for","timeout":120,"condition":{"type":"lua","state_name":"InGame","code":"return Game.GetLocalPlayer()==1 and Players[1]:IsTurnActive() and ExposedMembers.JAC_Absorption~=nil and ExposedMembers.JAC_HeroReadAPI~=nil"}}));
    steps.push(json!({"action":"ui_lua","state_name":"JAC_Absorption_Entry","code":BLOCKED,"equals_capture":"SPENT_SPREAD_BLOCKED"}));
    probe["name"] = json!("Spent AI missionary absorption must block and preserve donors, including saved reload");
    probe
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../civ6-autotest/scenarios");
    let base = serde_json::from_slice(&fs::read(root.join("absorption-priest-diagnostic.json"))?)?;
    let scenario = generate(base);
    civ6_autotest_rs::scenario::validate_scenario(&scenario)?;
    let output = root.join("absorption-spent-spread-192.json");
    fs::write(&output, format!("{}\n", serde_json::to_string_pretty(&scenario)?))?;
    println!("{}", output.display());
    let verification = full(scenario);
    civ6_autotest_rs::scenario::validate_scenario(&verification)?;
    fs::write(root.join("absorption-spent-spread-block-192.json"), format!("{}\n", serde_json::to_string_pretty(&verification)?))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_embedded_commands_compile_as_lua51() {
        let lua = mlua::Lua::new();
        for code in [SETUP, ARM, VERIFY, APPLY, RESUME, BLOCKED, READY] {
            lua.load(code).into_function().unwrap();
        }
    }

    #[test]
    fn accepted_request_without_actual_consumption_is_rejected() {
        let lua = mlua::Lua::new();
        lua.load(r#"
            CHARGES=3
            local u={GetID=function() return 42 end,GetOwner=function() return 3 end,
              GetType=function() return 7 end,GetSpreadCharges=function() return CHARGES end,
              GetReligionType=function() return 13 end}
            Game={GetProperty=function() return {From=3,UnitID=42} end}
            Players={[3]={GetUnits=function() return {FindID=function() return u end} end}}
            GameInfo={Units={UNIT_MISSIONARY={Index=7}}}
            JAC192_SPREAD={Requests=1,CanStart=true,CallOK=true,Before=3}
            ExposedMembers={JAC_Absorption={Canonical=function() return 'verified' end}}
        "#).exec().unwrap();
        let error = lua.load(VERIFY).eval::<String>().unwrap_err();
        assert!(error.to_string().contains("did not consume native charge"));
        lua.globals().set("CHARGES", 2).unwrap();
        assert_eq!(lua.load(VERIFY).eval::<String>().unwrap(), "verified");
        lua.globals().set("CHARGES", 0).unwrap();
        assert!(lua.load(VERIFY).eval::<String>().is_err());
    }
}
