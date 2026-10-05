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
local target
for _,city in p:GetCities():Members() do
 if city:GetID()~=cap:GetID() and (not target or city:GetID()<target:GetID()) then target=city end
end
assert(target,'donor needs a second city for official spread')
assert(target:GetReligion():GetMajorityReligion()~=r.Index,'target already converted')
local before={};for _,u in p:GetUnits():Members() do before[u:GetID()]=true end
UnitManager.InitUnitValidAdjacentHex(from,'UNIT_MISSIONARY',target:GetX(),target:GetY(),1)
local priest
for _,u in p:GetUnits():Members() do if not before[u:GetID()] then assert(not priest,'ambiguous new unit');priest=u end end
assert(priest and GameInfo.Units[priest:GetType()].UnitType=='UNIT_MISSIONARY','missionary creation failed')
priest:GetReligion():SetReligionType(r.ReligionType)
assert(priest:GetReligion():GetReligionType()==r.Index,'core religion initialization failed')
-- The native religion setter changes gameplay state without publishing it to UI.
-- Publish the initialized unit through a real placement on another valid plot.
local destination
for dir=0,5 do
 local q=Map.GetAdjacentPlot(target:GetX(),target:GetY(),dir)
 if q and q:GetOwner()==from and not q:IsWater() and not q:IsMountain() and q:GetDistrictType()<0
  and (q:GetX()~=priest:GetX() or q:GetY()~=priest:GetY()) then destination=q;break end
end
assert(destination,'no second valid target-adjacent plot for native publication')
UnitManager.PlaceUnit(priest,destination:GetX(),destination:GetY())
assert(priest:GetX()==destination:GetX() and priest:GetY()==destination:GetY(),'native placement rejected destination')
print('[JAC_SPENT_SPREAD_192] created xy='..priest:GetX()..','..priest:GetY()..' target='..target:GetX()..','..target:GetY()..' core_religion='..priest:GetReligion():GetReligionType()..' expected_religion='..r.Index)
assert(priest:GetX()>=0 and priest:GetY()>=0 and Map.GetPlotDistance(priest:GetX(),priest:GetY(),target:GetX(),target:GetY())<=1,'missionary not on a valid target-adjacent plot')
local e={From=from,To=to,UnitID=priest:GetID(),Religion=r.Index,TargetOwner=target:GetOwner(),TargetID=target:GetID(),X=target:GetX(),Y=target:GetY(),StartTurn=Game.GetCurrentGameTurn()}
Game:SetProperty('JAC_ABSORPTION_SPENT_SPREAD_192',e)
print('[JAC_SPENT_SPREAD_192] fixture owner='..from..' unit='..e.UnitID..' gameplay_request='..type(UnitManager.RequestOperation)..' gameplay_can='..type(UnitManager.CanStartOperation))
return 'AI missionary fixture created without changing spread charges'"#;

const FIXTURE_READY: &str = r#"local e=Game:GetProperty('JAC_ABSORPTION_SPENT_SPREAD_192');if not e then return false end
local u=Players[e.From]:GetUnits():FindID(e.UnitID)
local observed='owner='..e.From..' expected_id='..e.UnitID..' expected_religion='..e.Religion
if u then observed=observed..' id='..u:GetID()..' religion='..u:GetReligionType()..' charges='..u:GetSpreadCharges()..' xy='..u:GetX()..','..u:GetY() else observed=observed..' unit=missing' end
if JAC192_READY_LAST~=observed then print('[JAC_SPENT_SPREAD_192] UI fixture '..observed);JAC192_READY_LAST=observed end
return u and u:GetID()==e.UnitID and u:GetReligionType()==e.Religion and u:GetSpreadCharges()==3"#;

const ARM: &str = r#"local e=assert(Game:GetProperty('JAC_ABSORPTION_SPENT_SPREAD_192'))
local u=assert(Players[e.From]:GetUnits():FindID(e.UnitID));assert(u:GetID()==e.UnitID and u:GetOwner()==e.From,'fixture identity mismatch')
assert(u:GetSpreadCharges()==3 and u:GetReligionType()==e.Religion,'fixture native state mismatch')
JAC192_SPREAD={Owner=e.From,ID=e.UnitID,Before=u:GetSpreadCharges(),Turns={},Operations={},Events={}}
local function charges(owner,id,newCharges,oldCharges)
 if owner==e.From and id==e.UnitID then
  local unit=Players[owner]:GetUnits():FindID(id)
  JAC192_SPREAD.Events[#JAC192_SPREAD.Events+1]={Turn=Game.GetCurrentGameTurn(),Charges=unit and unit:GetSpreadCharges() or -1,Old=oldCharges,New=newCharges}
  print('[JAC_SPENT_SPREAD_192] charge turn='..Game.GetCurrentGameTurn()..' old='..tostring(oldCharges)..' new='..tostring(newCharges)..' getter='..tostring(unit and unit:GetSpreadCharges()))
 end
end
local function operation(owner,id,kind)
 if owner~=e.From or id~=e.UnitID then return end
 print('[JAC_SPENT_SPREAD_192] operation kind='..tostring(kind)..' expected='..tostring(UnitOperationTypes.SPREAD_RELIGION)..' turn='..Game.GetCurrentGameTurn())
 if kind==UnitOperationTypes.SPREAD_RELIGION then
  JAC192_SPREAD.Operations[#JAC192_SPREAD.Operations+1]={Turn=Game.GetCurrentGameTurn(),Kind=kind,Active=Players[owner]:IsTurnActive()}
  print('[JAC_SPENT_SPREAD_192] native spread owner='..owner..' unit='..id..' turn='..Game.GetCurrentGameTurn())
 end
end
local function command(owner,id,kind)
 if owner==e.From and id==e.UnitID then print('[JAC_SPENT_SPREAD_192] command kind='..tostring(kind)..' expected='..tostring(UnitOperationTypes.SPREAD_RELIGION)..' turn='..Game.GetCurrentGameTurn()) end
end
local function activated(owner)
 if owner~=e.From then return end
 local turn=Game.GetCurrentGameTurn()
 if JAC192_SPREAD.Turns[turn] then return end
 local unit=assert(Players[owner]:GetUnits():FindID(e.UnitID),'donor absent at AI turn start')
 local record={Before=unit:GetSpreadCharges(),EventCount=#JAC192_SPREAD.Events}
 JAC192_SPREAD.Turns[turn]=record
 if JAC192_SPREAD.FirstTurn==nil then
  assert(record.Before==3 and record.EventCount==0,'donor spent charges before AI turn')
  JAC192_SPREAD.FirstTurn=turn
 end
 local called,can,reasons=pcall(UnitManager.CanStartOperation,unit,UnitOperationTypes.SPREAD_RELIGION,nil,false,true)
 print('[JAC_SPENT_SPREAD_192] AI turn='..turn..' charges='..record.Before..' xy='..unit:GetX()..','..unit:GetY()..' opcode='..tostring(UnitOperationTypes.SPREAD_RELIGION)..' can_call='..tostring(called)..' can='..tostring(can)..' reasons='..tostring(reasons))
end
Events.PlayerTurnActivated.Add(activated)
Events.UnitChargesChanged.Add(charges)
Events.UnitOperationStarted.Add(operation)
if Events.UnitCommandStarted then Events.UnitCommandStarted.Add(command) end
JAC192_STOP=function() Events.PlayerTurnActivated.Remove(activated);Events.UnitChargesChanged.Remove(charges);Events.UnitOperationStarted.Remove(operation);if Events.UnitCommandStarted then Events.UnitCommandStarted.Remove(command) end end
return 'observing donor AI native spread operation and charge consumption'"#;
const VERIFY: &str = r#"local e=assert(Game:GetProperty('JAC_ABSORPTION_SPENT_SPREAD_192'));local o=assert(JAC192_SPREAD)
if JAC192_STOP then JAC192_STOP();JAC192_STOP=nil end
local u=assert(Players[e.From]:GetUnits():FindID(e.UnitID),'spent donor disappeared')
assert(u:GetID()==e.UnitID and u:GetOwner()==e.From and u:GetType()==GameInfo.Units.UNIT_MISSIONARY.Index,'donor identity changed')
local n=u:GetSpreadCharges()
print('[JAC_SPENT_SPREAD_192] observed before='..o.Before..' after='..n..' operations='..#o.Operations..' events='..#o.Events)
local first=o.FirstTurn and o.Turns[o.FirstTurn]
assert(o.Before==3 and first and first.Before==3 and first.EventCount==0,'native donor AI turn not observed')
assert(n==2,'native AI spread did not consume native charge exactly once')
assert(#o.Operations==1,'expected one official native spread operation')
local op=o.Operations[1]
local turn=o.Turns[op.Turn]
assert(op.Kind==UnitOperationTypes.SPREAD_RELIGION and op.Active==true and turn and turn.Before==3 and turn.EventCount==0,'native operation not on donor AI turn')
assert(#o.Events==1,'expected one native charge change event')
local changed=o.Events[1]
assert(changed.Old==3 and changed.New==2 and changed.Charges==2 and changed.Turn==op.Turn,'native charge event not associated with spread operation')
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
    steps.push(json!({"action":"wait_for","timeout":45,"condition":{"type":"lua","state_name":"InGame","code":FIXTURE_READY}}));
    steps.push(json!({"action":"ui_lua","state_name":"InGame","code":ARM}));
    steps.push(json!({"action":"end_turn","turns":2,"timeout":900}));
    steps.push(json!({"action":"ui_lua","state_name":"InGame","code":VERIFY}));
    steps.push(json!({"action":"save","name":"jac-spent-spread-192"}));
    base["name"] = json!("AI native spread operation diagnostic for ToDo 192");
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
    probe["name"] = json!(
        "Spent AI missionary absorption must block and preserve donors, including saved reload"
    );
    probe
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../civ6-autotest/scenarios");
    let base = serde_json::from_slice(&fs::read(root.join("absorption-priest-diagnostic.json"))?)?;
    let scenario = generate(base);
    civ6_autotest_rs::scenario::validate_scenario(&scenario)?;
    let output = root.join("absorption-spent-spread-192.json");
    fs::write(
        &output,
        format!("{}\n", serde_json::to_string_pretty(&scenario)?),
    )?;
    println!("{}", output.display());
    let verification = full(scenario);
    civ6_autotest_rs::scenario::validate_scenario(&verification)?;
    fs::write(
        root.join("absorption-spent-spread-block-192.json"),
        format!("{}\n", serde_json::to_string_pretty(&verification)?),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_embedded_commands_compile_as_lua51() {
        let lua = mlua::Lua::new();
        for code in [
            SETUP,
            FIXTURE_READY,
            ARM,
            VERIFY,
            APPLY,
            RESUME,
            BLOCKED,
            READY,
        ] {
            lua.load(code).into_function().unwrap();
        }
    }

    #[test]
    fn shipped_scenarios_match_the_generator() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../civ6-autotest/scenarios");
        let read = |name: &str| -> Value {
            serde_json::from_slice(&fs::read(root.join(name)).unwrap()).unwrap()
        };
        let probe = generate(read("absorption-priest-diagnostic.json"));
        assert_eq!(probe, read("absorption-spent-spread-192.json"));
        assert_eq!(full(probe), read("absorption-spent-spread-block-192.json"));
    }

    #[test]
    fn native_consumption_without_matching_operation_is_rejected() {
        let lua = mlua::Lua::new();
        lua.load(
            r#"
            CHARGES=3
            local u={GetID=function() return 42 end,GetOwner=function() return 3 end,
              GetType=function() return 7 end,GetSpreadCharges=function() return CHARGES end,
              GetReligionType=function() return 13 end}
            Game={GetProperty=function() return {From=3,UnitID=42} end}
            Players={[3]={GetUnits=function() return {FindID=function() return u end} end}}
            GameInfo={Units={UNIT_MISSIONARY={Index=7}}}
            UnitOperationTypes={SPREAD_RELIGION=99}
            JAC192_SPREAD={Before=3,FirstTurn=1,Turns={[1]={Before=3,EventCount=0}},Operations={},Events={}}
            ExposedMembers={JAC_Absorption={Canonical=function() return 'verified' end}}
        "#,
        )
        .exec()
        .unwrap();
        let error = lua.load(VERIFY).eval::<String>().unwrap_err();
        assert!(error.to_string().contains("did not consume native charge"));
        lua.globals().set("CHARGES", 2).unwrap();
        assert!(lua.load(VERIFY).eval::<String>().is_err());
        lua.load("JAC192_SPREAD.Events={{Old=3,New=2,Charges=2,Turn=1}};JAC192_SPREAD.Operations={{Kind=99,Active=true,Turn=1}}")
            .exec()
            .unwrap();
        assert_eq!(lua.load(VERIFY).eval::<String>().unwrap(), "verified");
        lua.load("JAC192_SPREAD.Operations[1].Kind=98")
            .exec()
            .unwrap();
        assert!(lua.load(VERIFY).eval::<String>().is_err());
        lua.load("JAC192_SPREAD.Operations[1].Kind=99")
            .exec()
            .unwrap();
        lua.globals().set("CHARGES", 0).unwrap();
        let error = lua.load(VERIFY).eval::<String>().unwrap_err();
        assert!(error.to_string().contains("did not consume native charge"));
    }

    #[test]
    fn native_event_handlers_use_official_arguments_and_filter_identity() {
        for (events, expected) in [
            ("CHARGES=2;H.charges(4,42,2,3);H.charges(3,43,2,3);H.operation(4,42,99);H.operation(3,43,99);H.operation(3,42,98);H.charges(3,42,2,3);H.operation(3,42,99);TURN=2;H.activated(3)", None),
            ("TURN=2;H.activated(3);CHARGES=2;H.charges(3,42,2,3);H.operation(3,42,99)", None),
            ("CHARGES=2;H.operation(3,42,99)", Some("expected one native charge change event")),
            ("CHARGES=2;H.charges(3,42,2,3);H.operation(3,42,98)", Some("expected one official native spread operation")),
            ("CHARGES=2;TURN=2;H.charges(3,42,2,3);H.operation(3,42,99)", Some("native operation not on donor AI turn")),
        ] {
            let lua = mlua::Lua::new();
            lua.load(r#"
                CHARGES=3;TURN=1;H={}
                local function event(name) return {Add=function(f) H[name]=f end,Remove=function(f) if H[name]==f then H[name]=nil end end} end
                Events={PlayerTurnActivated=event('activated'),UnitChargesChanged=event('charges'),UnitOperationStarted=event('operation')}
                local u={GetID=function() return 42 end,GetOwner=function() return 3 end,GetType=function() return 7 end,
                  GetSpreadCharges=function() return CHARGES end,GetReligionType=function() return 13 end,
                  GetX=function() return 1 end,GetY=function() return 1 end}
                Game={GetProperty=function() return {From=3,UnitID=42,Religion=13} end,GetCurrentGameTurn=function() return TURN end}
                Players={[3]={IsTurnActive=function() return true end,GetUnits=function() return {FindID=function(_,id) if id==42 then return u end end} end}}
                GameInfo={Units={UNIT_MISSIONARY={Index=7}}};UnitOperationTypes={SPREAD_RELIGION=99}
                UnitManager={CanStartOperation=function() return true end}
                ExposedMembers={JAC_Absorption={Canonical=function() return 'verified' end}}
            "#).exec().unwrap();
            lua.load(ARM).exec().unwrap();
            lua.load("H.activated(3)").exec().unwrap();
            lua.load(events).exec().unwrap();
            match expected {
                None => assert_eq!(lua.load(VERIFY).eval::<String>().unwrap(), "verified"),
                Some(message) => assert!(lua.load(VERIFY).eval::<String>().unwrap_err().to_string().contains(message)),
            }
        }
    }
}
