//! Opt-in finite diagnosis; it does not change the production loyalty policy.
use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::{fs, path::PathBuf};

const UI: &str = r#"
JAC_LOYALTY_MATRIX={Publications=0,Events=0}
ExposedMembers.JAC_LoyaltyMatrixRead=function(x,y)
 local c=assert(CityManager.GetCityAt(x,y),'diagnostic city missing')
 local identity=c:GetCulturalIdentity()
 return {Owner=c:GetOwner(),ID=c:GetID(),Current=identity:GetLoyalty(),Rate=identity:GetLoyaltyPerTurn(),
  Turn=Game.GetCurrentGameTurn(),Local=Game.GetLocalPlayer(),Active=Players[c:GetOwner()]:IsTurnActive()}
end
Events.GameCoreEventPublishComplete.Add(function() JAC_LOYALTY_MATRIX.Publications=JAC_LOYALTY_MATRIX.Publications+1 end)
Events.CityLoyaltyChanged.Add(function(owner,id)
 JAC_LOYALTY_MATRIX.Events=JAC_LOYALTY_MATRIX.Events+1
 print('JAC_LOYALTY_MATRIX_EVENT owner='..tostring(owner)..' id='..tostring(id))
end)
return 'native getter, rate and city-event observer registered'
"#;

const CORE: &str = r#"
local operationMode=false
local D={Calls={},Rows={},Turn=Game.GetCurrentGameTurn()}
local function record(label,c)
 local r=ExposedMembers.JAC_LoyaltyMatrixRead(c:GetX(),c:GetY())
 assert(r.Owner==c:GetOwner() and r.ID==c:GetID(),'diagnostic identity mismatch')
 local active={};for _,p in ipairs(PlayerManager.GetAliveMajors()) do
  if p:IsTurnActive() then active[#active+1]=tostring(p:GetID()) end
 end
 r.Label=label;r.ActiveOwners=table.concat(active,',');D.Rows[#D.Rows+1]=r
 print('JAC_LOYALTY_MATRIX label='..label..' owner='..r.Owner..' id='..r.ID..' turn='..r.Turn..
  ' current='..r.Current..' rate='..r.Rate..' active='..tostring(Players[r.Owner]:IsTurnActive())..' activeOwners='..r.ActiveOwners)
 return r
end
local function city(row)
 local c=assert(CityManager.GetCityAt(row.X,row.Y))
 assert(c:GetOwner()==row.Owner and c:GetID()==row.ID,'diagnostic city replaced')
 return c
end
local function call(label,row,kind,amount)
 assert(not D.Calls[label],'diagnostic mutation replay')
 local c=city(row);record(label..':before',c);D.Calls[label]=true
 if kind=='setter' then c:ChangeLoyalty(amount) else c:AttachModifierByID(amount) end
 record(label..':issued',c)
end
local function row(c) return {Owner=c:GetOwner(),ID=c:GetID(),X=c:GetX(),Y=c:GetY()} end
local human=ExposedMembers.JAC_LoyaltyMatrixRead(Players[3]:GetCities():GetCapitalCity():GetX(),Players[3]:GetCities():GetCapitalCity():GetY()).Local
assert(human==3 and Players[human]:IsTurnActive(),'expected actual Ashina human turn')
local hc=assert(Players[human]:GetCities():GetCapitalCity())
local hp;for _,c in Players[human]:GetCities():Members() do if c:GetID()~=hc:GetID() then hp=c;break end end
assert(hp,'human noncapital missing')
local ac=assert(Players[0]:GetCities():GetCapitalCity())
local ap;for _,c in Players[0]:GetCities():Members() do if c:GetID()~=ac:GetID() then ap=c;break end end
assert(ap and not Players[0]:IsTurnActive(),'expected inactive AI with noncapital')
D.Human=row(hp);D.Control=row(hc);D.AI=row(ap);D.AIActive=row(ap)
if not operationMode then
call('human-active-negative',D.Human,'setter',-5)
call('human-active-native-negative',D.Human,'modifier','JAC_ABSORPTION_LOYALTY_LOSS_5')
call('ai-inactive-negative',D.AI,'setter',-5)
call('ai-inactive-native-negative',D.AI,'modifier','JAC_ABSORPTION_LOYALTY_LOSS_5')
-- Separate diagnostic city, deliberately beyond the +100 map stability.
-- This is an experiment, never a production-policy substitute or retry.
call('human-control-prepare',D.Control,'setter',-150)
end
if operationMode then
 local cases={
  ['human-operation-setter']={D.Human,'setter',-5},
  ['human-operation-modifier']={D.Human,'modifier','JAC_ABSORPTION_LOYALTY_LOSS_5'},
  ['ai-operation-setter']={D.AI,'setter',-5},
  ['ai-operation-modifier']={D.AI,'modifier','JAC_ABSORPTION_LOYALTY_LOSS_5'},
  ['human-operation-control']={D.Control,'setter',-150}}
 GameEvents.JAC_LoyaltyMatrix_Operation.Add(function(actor,params)
  assert(actor==human and Players[actor]:IsTurnActive(),'wrong diagnostic operation actor')
  local s=assert(cases[params.Label],'unknown diagnostic operation')
  call(params.Label,s[1],s[2],s[3])
  D.LastOperation=params.Label;Game:SetProperty('JAC_LOYALTY_MATRIX_OPERATIONS',D)
 end)
 ExposedMembers.JAC_LoyaltyMatrixLastOperation=function() return D.LastOperation end
end
GameEvents.PlayerTurnStartComplete.Add(function(actor)
 print('JAC_LOYALTY_MATRIX_TURN actor='..tostring(actor)..' active='..tostring(Players[actor]:IsTurnActive()))
 if not operationMode and actor==0 and Players[actor]:IsTurnActive() and not D.Calls['ai-active-negative'] then
  call('ai-active-negative',D.AIActive,'setter',-5)
  call('ai-active-native-negative',D.AIActive,'modifier','JAC_ABSORPTION_LOYALTY_LOSS_5')
 end
end)
ExposedMembers.JAC_LoyaltyMatrixSnapshot=function(label)
 for name,r in pairs({Human=D.Human,Control=D.Control,AI=D.AI,AIActive=D.AIActive}) do record(label..':'..name,city(r)) end
 return D
end
ExposedMembers.JAC_LoyaltyMatrixPositive=function()
 local c=city(D.Control);local r=record('positive-control-baseline',c)
 if not (r.Current>0 and r.Current<100) then
  D.PositiveEligible=false;print('JAC_LOYALTY_MATRIX_POSITIVE_UNAVAILABLE current='..r.Current)
  return 'UNAVAILABLE: no actual below-cap positive control; no positive mutation issued'
 end
 D.PositiveEligible=true
 assert(Players[r.Owner]:IsTurnActive(),'positive control requires owner active')
 -- Record the real below-cap baseline; do not assume that a capped + value works.
 call('human-positive-setter',D.Control,'setter',5)
 call('human-positive-native',D.Control,'modifier','PROJECT_COMPLETION_LOYALTY')
 return 'positive setter and stock +20 effect issued once from actual below-cap baseline'
end
return ExposedMembers.JAC_LoyaltyMatrixSnapshot('initial') and 'finite loyalty matrix armed'
"#;

fn ui(state: &str, code: &str) -> Value {
    json!({"action":"ui_lua","state_name":state,"code":code})
}

const NATIVE_UI: &str = r#"
assert(not ExposedMembers.JAC_LoyaltyNativeProbe,'native probe already armed')
local actor=Game.GetLocalPlayer()
assert(actor==3 and Players[actor]:IsTurnActive(),'expected actual Ashina human turn')
local capital=assert(Players[actor]:GetCities():GetCapitalCity())
local target
for _,c in Players[actor]:GetCities():Members() do
 if c:GetID()~=capital:GetID() and (not target or c:GetID()<target:GetID()) then target=c end
end
assert(target,'human noncapital missing')
local d={Owner=actor,ID=target:GetID(),X=target:GetX(),Y=target:GetY(),Turn=Game.GetCurrentGameTurn(),Events=0,Publications=0}
ExposedMembers.JAC_LoyaltyNativeProbe=d
d.ReadCurrent=function(label)
 local c=assert(CityManager.GetCityAt(d.X,d.Y),'native getter city missing')
 assert(c:GetOwner()==d.Owner and c:GetID()==d.ID,'native getter city replaced')
 assert(Game.GetCurrentGameTurn()==d.Turn,'native getter turn changed')
 print('JAC_LOYALTY_NATIVE_GETTER_CALL label='..label..' owner='..d.Owner..' city='..d.ID)
 local current=c:GetCulturalIdentity():GetLoyalty()
 print('JAC_LOYALTY_NATIVE_GETTER_RETURN label='..label..' current='..tostring(current))
 return current
end
Events.CityLoyaltyChanged.Add(function(owner,id)
 if owner==d.Owner and id==d.ID then d.Events=d.Events+1 end
end)
Events.GameCoreEventPublishComplete.Add(function() d.Publications=d.Publications+1 end)
local identity=target:GetCulturalIdentity()
d.BaselineIdentity=identity
d.Before=identity:GetLoyalty()
assert(type(d.Before)=='number' and d.Before>=0 and d.Before<=100,'invalid native probe baseline')
print('JAC_LOYALTY_NATIVE_IDENTITY kind='..type(identity)..' text='..tostring(identity))
if type(identity)=='table' then
 local instance=identity.__instance
 d.BaselineInstance=instance
 local mt=instance and type(getmetatable)=='function' and getmetatable(instance) or nil
 local custom=type(mt)=='table' and mt.__tostring or nil
 print('JAC_LOYALTY_NATIVE_INSTANCE kind='..type(instance)..' text='..tostring(instance)..' access=field getmetatableKind='..type(getmetatable)..' mtKind='..type(mt)..' customFormatterKind='..type(custom))
end
print('JAC_LOYALTY_NATIVE_EVENT_HASH '..tostring(DB.MakeHash('CityLoyaltyChanged')))
print('JAC_LOYALTY_NATIVE_ARMED owner='..d.Owner..' city='..d.ID..' x='..d.X..' y='..d.Y..' turn='..d.Turn..' before='..d.Before)
return 'single setter probe armed; waiting for verified native observer'
"#;

const NATIVE_CORE: &str = r#"
local d=assert(ExposedMembers.JAC_LoyaltyNativeProbe)
if d.HandlerRegistered then return 'existing native probe handler retained' end
GameEvents.JAC_LoyaltyNativeProbe_Operation.Add(function(actor,params)
 assert(actor==d.Owner and params.Tag=='native-probe' and Players[actor]:IsTurnActive(),'wrong native probe actor')
 assert(d.ObserverReady,'native observer is not ready')
 assert(not d.MutationIssued,'native probe mutation replay')
 assert(Game.GetCurrentGameTurn()==d.Turn,'native probe turn changed')
 local c=assert(CityManager.GetCityAt(d.X,d.Y),'native probe city missing')
 assert(c:GetOwner()==d.Owner and c:GetID()==d.ID,'native probe city replaced')
 d.MutationIssued=true
 print('JAC_LOYALTY_NATIVE_CALL owner='..d.Owner..' city='..d.ID..' x='..d.X..' y='..d.Y)
 c:ChangeLoyalty(-5)
 d.Issued=true
 print('JAC_LOYALTY_NATIVE_RETURN')
end)
d.HandlerRegistered=true
return 'one synchronized setter registered; mutation not issued yet'
"#;

// Keep CacheCity reads in their UI context. The Gameplay operation only
// resolves its City and issues the one mutation; it never calls a UI closure.
const NATIVE_REQUEST_UI: &str = r#"
local d=assert(ExposedMembers.JAC_LoyaltyNativeProbe)
assert(d.ObserverReady and not d.MutationIssued,'native probe is not ready or already issued')
d.ImmediateBefore=d.ReadCurrent('before-setter')
d.Publications=0
UI.RequestPlayerOperation(d.Owner,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_LoyaltyNativeProbe_Operation',Tag='native-probe'})
return 'one native setter requested'
"#;

const SINGLE_REQUEST_UI: &str = r#"
local d=assert(ExposedMembers.JAC_LoyaltyNativeProbe)
assert(not d.UIRequestIssued and not d.MutationIssued,'single UI request already issued')
d.ImmediateBefore=d.ReadCurrent('before-setter')
assert(type(d.ImmediateBefore)=='number' and d.ImmediateBefore==d.Before,'UI baseline changed')
d.Publications=0
d.UIRequestIssued=true
UI.RequestPlayerOperation(d.Owner,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_LoyaltyNativeProbe_Operation',Tag='native-probe'})
return 'one synchronized setter requested; no native observer'
"#;

fn native_probe_steps(handoff: &std::path::Path) -> Vec<Value> {
    loyalty_probe_steps(Some(handoff))
}

fn publication_probe_steps() -> Vec<Value> {
    let mut steps = loyalty_probe_steps(None);
    let request = format!("local d=assert(ExposedMembers.JAC_LoyaltyNativeProbe)\nassert(not d.PublicationSenderRegistered,'publication sender already registered')\nEvents.GameCoreEventPublishComplete.Add(function()\n if d.UIRequestIssued then return end\n print('JAC_LOYALTY_REQUEST_CONTEXT publish_complete')\n{SINGLE_REQUEST_UI}\nend)\nd.PublicationSenderRegistered=true\nreturn 'one setter request registered for next publish complete; no native observer'");
    steps[2] = ui("InGame", &request);
    steps
}

fn ai_probe_steps() -> Vec<Value> {
    let mut steps = loyalty_probe_steps(None);
    let arm = steps[0]["code"]
        .as_str()
        .unwrap()
        .replace(
            "local capital=assert(Players[actor]:GetCities():GetCapitalCity())",
            "local subject=0\nlocal capital=assert(Players[subject]:GetCities():GetCapitalCity())",
        )
        .replace(
            "Players[actor]:GetCities():Members()",
            "Players[subject]:GetCities():Members()",
        )
        .replace(
            "local d={Owner=actor,",
            "local d={Actor=actor,Owner=subject,",
        );
    steps[0] = ui("InGame", &arm);
    let core = steps[1]["code"]
        .as_str()
        .unwrap()
        .replace("actor==d.Owner", "actor==(d.Actor or d.Owner)");
    steps[1] = ui("JAC_Absorption_Entry", &core);
    steps[2] = ui(
        "InGame",
        &SINGLE_REQUEST_UI.replace(
            "UI.RequestPlayerOperation(d.Owner,",
            "UI.RequestPlayerOperation(d.Actor or d.Owner,",
        ),
    );
    steps
}

fn capital_probe_steps() -> Vec<Value> {
    let mut steps = ai_probe_steps();
    let selection = "local target\nfor _,c in Players[subject]:GetCities():Members() do\n if c:GetID()~=capital:GetID() and (not target or c:GetID()<target:GetID()) then target=c end\nend\nassert(target,'human noncapital missing')";
    let arm = steps[0]["code"].as_str().unwrap();
    assert_eq!(
        arm.matches(selection).count(),
        1,
        "capital selection template changed"
    );
    steps[0] = ui("InGame", &arm.replace(selection, "local target=capital"));
    steps
}

fn cross_context_probe_steps() -> Vec<Value> {
    let mut steps = ai_probe_steps();
    let core = steps[1]["code"].as_str().unwrap().replace(
        "c:ChangeLoyalty(-5)",
        "d.CrossContextBefore=d.ReadCurrent('cross-context-before-setter')\n c:ChangeLoyalty(-5)",
    );
    steps[1] = ui("JAC_Absorption_Entry", &core);
    steps
}

// This comparison changes one donor capital's ownership before the ordinary
// UI -> synchronized Core setter. Core never invokes the UI getter closure.
const TRANSFER_CORE: &str = r#"
local d=assert(ExposedMembers.JAC_LoyaltyNativeProbe)
assert(not d.HandlerRegistered,'transfer handlers already registered')
local function validate(actor,params,tag,owner,id)
 assert(actor==d.Actor and Players[actor]:IsTurnActive(),'wrong transfer probe actor')
 assert(params.Tag==tag and params.Owner==owner and params.ID==id and params.X==d.X and params.Y==d.Y and params.Turn==d.Turn,'transfer probe request identity mismatch')
 assert(Game.GetCurrentGameTurn()==d.Turn,'transfer probe turn changed')
 local c=assert(CityManager.GetCityAt(d.X,d.Y),'transfer probe city missing')
 assert(c:GetOwner()==owner and c:GetID()==id and c:GetX()==d.X and c:GetY()==d.Y,'transfer probe city replaced')
 return c
end
GameEvents.JAC_LoyaltyTransferProbe_Operation.Add(function(actor,params)
 assert(d.TransferRequestIssued and not d.TransferIntent and not d.MutationIssued,'transfer probe replay or unrequested transfer')
 local c=validate(actor,params,d.Tag,d.OriginalOwner,d.OriginalID)
 assert(d.OriginalOwner==1 and d.Recipient==0 and not Players[d.Recipient]:IsHuman(),'transfer recipient changed')
 local cities=Players[d.OriginalOwner]:GetCities()
 local capital=assert(cities:GetCapitalCity(),'donor capital missing')
 assert(cities:GetCount()>1 and capital:GetID()==d.OriginalID,'donor no longer has capital and another city')
 d.TransferIntent={Owner=c:GetOwner(),ID=c:GetID(),X=c:GetX(),Y=c:GetY(),Turn=d.Turn,Actor=actor,Recipient=d.Recipient,Tag=params.Tag}
 d.TransferCalls=1
 print('JAC_LOYALTY_TRANSFER_INTENT actor='..actor..' active='..tostring(Players[actor]:IsTurnActive())..' owner='..c:GetOwner()..' city='..c:GetID()..' x='..d.X..' y='..d.Y..' turn='..d.Turn..' tag='..params.Tag)
 CityManager.TransferCity(c,d.Recipient)
 local moved=assert(CityManager.GetCityAt(d.X,d.Y),'transferred Core city missing')
 assert(moved:GetOwner()==d.Recipient and moved:GetX()==d.X and moved:GetY()==d.Y,'transferred Core identity mismatch')
 assert(Game.GetCurrentGameTurn()==d.Turn and Players[actor]:IsTurnActive() and cities:GetCount()>0,'transfer changed turn or eliminated donor')
 d.CoreTransferredID=moved:GetID()
 d.TransferComplete=true
 print('JAC_LOYALTY_TRANSFER_RETURN actor='..actor..' oldOwner='..d.OriginalOwner..' oldCity='..d.OriginalID..' newOwner='..moved:GetOwner()..' newCity='..d.CoreTransferredID..' x='..d.X..' y='..d.Y..' turn='..d.Turn)
end)
GameEvents.JAC_LoyaltyNativeProbe_Operation.Add(function(actor,params)
 assert(d.TransferComplete and d.TransferUIReady and d.TransferCalls==1,'transfer not published to UI')
 assert(d.UIRequestIssued and type(d.ImmediateBefore)=='number' and d.ImmediateBefore==d.Before,'fresh UI baseline request is not ready')
 assert(not d.MutationIssued,'transfer setter replay')
 assert(d.Owner==d.Recipient and d.ID==d.CoreTransferredID,'transferred setter key mismatch')
 local c=validate(actor,params,'native-probe',d.Recipient,d.CoreTransferredID)
 d.MutationIssued=true;d.SetterCalls=1
 print('JAC_LOYALTY_NATIVE_CALL actor='..actor..' active='..tostring(Players[actor]:IsTurnActive())..' owner='..c:GetOwner()..' city='..c:GetID()..' x='..d.X..' y='..d.Y..' turn='..d.Turn)
 c:ChangeLoyalty(-5)
 d.Issued=true
 print('JAC_LOYALTY_NATIVE_RETURN')
end)
d.HandlerRegistered=true
return 'one transfer and one post-publication setter registered; neither issued'
"#;

const TRANSFER_REQUEST_UI: &str = r#"
local d=assert(ExposedMembers.JAC_LoyaltyNativeProbe)
assert(d.HandlerRegistered and not d.TransferRequestIssued and not d.TransferIntent and not d.UIRequestIssued,'transfer request replay')
assert(Game.GetLocalPlayer()==d.Actor and Players[d.Actor]:IsTurnActive(),'transfer sender changed')
assert(Game.GetCurrentGameTurn()==d.Turn and d.Owner==d.OriginalOwner and d.ID==d.OriginalID,'original transfer key changed')
d.TransferBefore=d.ReadCurrent('before-transfer')
assert(type(d.TransferBefore)=='number' and d.TransferBefore==d.OriginalBefore,'original UI baseline changed')
d.TransferRequestIssued=true
UI.RequestPlayerOperation(d.Actor,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_LoyaltyTransferProbe_Operation',Tag=d.Tag,Owner=d.OriginalOwner,ID=d.OriginalID,X=d.X,Y=d.Y,Turn=d.Turn})
return 'one transfer requested; unknown response never retried'
"#;

const TRANSFER_SETTER_UI: &str = r#"
local d=assert(ExposedMembers.JAC_LoyaltyNativeProbe)
assert(d.TransferComplete and d.TransferUIReady and d.TransferCalls==1,'transfer UI publication absent')
assert(not d.UIRequestIssued and not d.MutationIssued,'transfer setter request replay')
assert(Game.GetLocalPlayer()==d.Actor and Players[d.Actor]:IsTurnActive(),'setter sender changed')
d.ImmediateBefore=d.ReadCurrent('before-setter')
assert(type(d.ImmediateBefore)=='number' and d.ImmediateBefore>=0 and d.ImmediateBefore<=100,'invalid fresh UI baseline')
d.Before=d.ImmediateBefore
d.Events=0;d.Publications=0
d.UIRequestIssued=true
UI.RequestPlayerOperation(d.Actor,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_LoyaltyNativeProbe_Operation',Tag='native-probe',Owner=d.Owner,ID=d.ID,X=d.X,Y=d.Y,Turn=d.Turn})
return 'one setter requested after transferred CacheCity publication and fresh UI getter'
"#;

fn transfer_probe_steps() -> Vec<Value> {
    let selection = "local target\nfor _,c in Players[actor]:GetCities():Members() do\n if c:GetID()~=capital:GetID() and (not target or c:GetID()<target:GetID()) then target=c end\nend\nassert(target,'human noncapital missing')";
    let arm = NATIVE_UI
        .replace("local capital=assert(Players[actor]:GetCities():GetCapitalCity())",
            "assert(Players[actor]:IsHuman() and not Players[0]:IsHuman(),'expected human sender and AI recipient')\nlocal donor=1\nassert(Players[donor]:GetCities():GetCount()>1,'donor needs another city')\nlocal capital=assert(Players[donor]:GetCities():GetCapitalCity())")
        .replace(selection, "local target=capital\nassert(target:GetOwner()==donor,'wrong donor capital')")
        .replace("local d={Owner=actor,", "local d={Actor=actor,Owner=donor,Recipient=0,Tag='transfer-probe',TransferCalls=0,SetterCalls=0,TransferPublications=0,")
        .replace("ExposedMembers.JAC_LoyaltyNativeProbe=d",
            "d.OriginalOwner=d.Owner;d.OriginalID=d.ID\nExposedMembers.JAC_LoyaltyNativeProbe=d")
        .replace("c:GetOwner()==d.Owner and c:GetID()==d.ID,'native getter city replaced'",
            "c:GetOwner()==d.Owner and c:GetID()==d.ID and c:GetX()==d.X and c:GetY()==d.Y,'native getter city replaced'")
        .replace("if owner==d.Owner and id==d.ID then d.Events=d.Events+1 end",
            "if d.UIRequestIssued and d.MutationIssued and owner==d.Owner and id==d.ID then d.Events=d.Events+1 end")
        .replace("Events.GameCoreEventPublishComplete.Add(function() d.Publications=d.Publications+1 end)", r#"Events.GameCoreEventPublishComplete.Add(function()
 if d.UIRequestIssued then
  if d.Issued then d.Publications=d.Publications+1 end
  return
 end
 if not d.TransferComplete or d.TransferUIReady then return end
 assert(Game.GetCurrentGameTurn()==d.Turn and Game.GetLocalPlayer()==d.Actor and Players[d.Actor]:IsTurnActive(),'transfer UI turn or sender changed')
 local c=assert(CityManager.GetCityAt(d.X,d.Y),'transferred UI city missing')
 assert(c:GetOwner()==d.Recipient and c:GetID()==d.CoreTransferredID and c:GetX()==d.X and c:GetY()==d.Y,'transferred CacheCity identity mismatch')
 d.Owner=c:GetOwner();d.ID=c:GetID()
 local identity=c:GetCulturalIdentity()
 d.TransferIdentity=identity
 print('JAC_LOYALTY_TRANSFER_IDENTITY kind='..type(identity)..' text='..tostring(identity))
 if type(identity)=='table' then
  local instance=identity.__instance
  d.TransferInstance=instance
  local mt=instance and type(getmetatable)=='function' and getmetatable(instance) or nil
  local custom=type(mt)=='table' and mt.__tostring or nil
  print('JAC_LOYALTY_TRANSFER_INSTANCE kind='..type(instance)..' text='..tostring(instance)..' access=field getmetatableKind='..type(getmetatable)..' mtKind='..type(mt)..' customFormatterKind='..type(custom))
 end
 d.TransferAfter=d.ReadCurrent('after-transfer-publication')
 assert(type(d.TransferAfter)=='number' and d.TransferAfter>=0 and d.TransferAfter<=100,'invalid transfer UI value')
 d.TransferPublications=d.TransferPublications+1
 d.TransferUIReady=true
 print('JAC_LOYALTY_TRANSFER_UI actor='..d.Actor..' oldOwner='..d.OriginalOwner..' oldCity='..d.OriginalID..' newOwner='..d.Owner..' newCity='..d.ID..' x='..d.X..' y='..d.Y..' turn='..d.Turn..' before='..d.TransferBefore..' after='..d.TransferAfter..' publications='..d.TransferPublications)
end)"#)
        .replace("d.Before=identity:GetLoyalty()", "d.Before=identity:GetLoyalty()\nd.OriginalBefore=d.Before")
        .replace("single setter probe armed; waiting for verified native observer",
            "donor capital transfer probe armed; no native observer");
    vec![
        ui("InGame", &arm),
        ui("JAC_Absorption_Entry", TRANSFER_CORE),
        ui("InGame", TRANSFER_REQUEST_UI),
        json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":"local d=ExposedMembers.JAC_LoyaltyNativeProbe;return d and d.TransferComplete and d.TransferUIReady and d.TransferPublications>=1"}}),
        ui("InGame", TRANSFER_SETTER_UI),
        json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":"local d=ExposedMembers.JAC_LoyaltyNativeProbe;return d and d.Issued and d.Publications>=2"}}),
        ui("InGame", "local d=assert(ExposedMembers.JAC_LoyaltyNativeProbe);assert(d.TransferCalls==1 and d.SetterCalls==1 and d.TransferUIReady and d.Issued and d.Publications>=2,'finite transfer comparison incomplete');d.After=d.ReadCurrent('after-setter-publication');print('JAC_LOYALTY_TRANSFER_RESULT actor='..d.Actor..' active='..tostring(Players[d.Actor]:IsTurnActive())..' oldOwner='..d.OriginalOwner..' oldCity='..d.OriginalID..' owner='..d.Owner..' city='..d.ID..' x='..d.X..' y='..d.Y..' turn='..d.Turn..' transferBefore='..d.TransferBefore..' transferAfter='..d.TransferAfter..' before='..d.Before..' after='..d.After..' cityEvents='..d.Events..' publications='..d.Publications..' transferPublications='..d.TransferPublications..' transfers='..d.TransferCalls..' setters='..d.SetterCalls);return 'raw transfer comparison recorded; not a product acceptance or root cause'"),
    ]
}

fn transfer_direct_core_probe_steps() -> Vec<Value> {
    let mut steps = transfer_probe_steps();
    let core = steps[1]["code"].as_str().unwrap();
    let setter_open = "GameEvents.JAC_LoyaltyNativeProbe_Operation.Add(function(actor,params)";
    let setter_close = "end)\nd.HandlerRegistered=true";
    assert_eq!(core.matches(setter_open).count(), 1);
    assert_eq!(core.matches(setter_close).count(), 1);
    let core = core.replace(setter_open, "local function directDiagnosticSetter(actor,params)")
        .replace(setter_close, "end\nd.DirectDiagnosticSetter=directDiagnosticSetter\nGameEvents.JAC_LoyaltyNativeProbe_Operation.Add(directDiagnosticSetter)\nd.HandlerRegistered=true");
    steps[1] = ui("JAC_Absorption_Entry", &core);
    let request = "UI.RequestPlayerOperation(d.Actor,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_LoyaltyNativeProbe_Operation',Tag='native-probe',Owner=d.Owner,ID=d.ID,X=d.X,Y=d.Y,Turn=d.Turn})";
    let prepare = TRANSFER_SETTER_UI.replace(request, "d.DirectDiagnosticParams={Tag='native-probe',Owner=d.Owner,ID=d.ID,X=d.X,Y=d.Y,Turn=d.Turn}")
        .replace("one setter requested after transferred CacheCity publication and fresh UI getter", "direct Core comparison armed from fresh UI baseline; setter not issued");
    assert!(!prepare.contains("UI.RequestPlayerOperation"));
    steps[4] = ui("InGame", &prepare);
    steps.insert(5, ui("JAC_Absorption_Entry", "local d=assert(ExposedMembers.JAC_LoyaltyNativeProbe);assert(d.TransferUIReady and d.UIRequestIssued and d.DirectDiagnosticParams and not d.DirectDiagnosticInvocation,'direct Core diagnostic not armed or already invoked');d.DirectDiagnosticInvocation=true;d.DirectDiagnosticSetter(d.Actor,d.DirectDiagnosticParams);return 'one direct Core diagnostic call; no native observer or synchronized setter request'"));
    steps
}

fn transfer_reload_probe_steps() -> Vec<Value> {
    let mut steps = vec![
        ui("JAC_Absorption_Entry", "local c=assert(Players[1]:GetCities():GetCapitalCity());Game:SetProperty('JAC_LOYALTY_RELOAD_DIAGNOSTIC_V1',{Turn=Game.GetCurrentGameTurn(),Owner=c:GetOwner(),ID=c:GetID(),X=c:GetX(),Y=c:GetY()});return 'save diagnostic baseline before any transfer or setter'"),
        json!({"action":"save","name":"JAC-loyalty-transfer-reload-diagnostic"}),
        json!({"action":"exit_to_menu","timeout":90}),
        json!({"action":"load","name":"JAC-loyalty-transfer-reload-diagnostic","timeout":600}),
        json!({"action":"wait_for","timeout":600,"condition":{"type":"lua","state_name":"InGame","code":"return ExposedMembers.JAC_AbsorptionProductNativeRead~=nil and ExposedMembers.JAC_HeroReadAPI and type(ExposedMembers.JAC_HeroReadAPI.ReadPlayer)=='function' and Game.GetLocalPlayer()==3 and Players[3]:IsTurnActive()"}}),
        ui("JAC_Absorption_Entry", "local s=assert(Game:GetProperty('JAC_LOYALTY_RELOAD_DIAGNOSTIC_V1'),'saved diagnostic baseline absent');local c=assert(Players[1]:GetCities():GetCapitalCity());assert(Game.GetCurrentGameTurn()==s.Turn and c:GetOwner()==s.Owner and c:GetID()==s.ID and c:GetX()==s.X and c:GetY()==s.Y,'reload changed baseline turn or donor identity');return 'physical reload preserved turn and donor; transfer and setter still unissued'"),
    ];
    steps.extend(transfer_probe_steps());
    steps
}

fn loyalty_probe_steps(handoff: Option<&std::path::Path>) -> Vec<Value> {
    let core = if handoff.is_some() {
        NATIVE_CORE.to_owned()
    } else {
        NATIVE_CORE.replace("assert(d.ObserverReady,'native observer is not ready')",
            "assert(d.UIRequestIssued and type(d.ImmediateBefore)=='number' and d.ImmediateBefore==d.Before,'UI baseline request is not ready')")
    };
    let arm = if handoff.is_some() {
        NATIVE_UI.to_owned()
    } else {
        NATIVE_UI.replace(
            "single setter probe armed; waiting for verified native observer",
            "single UI setter probe armed; no native observer requested",
        )
    };
    let mut steps = vec![ui("InGame", &arm), ui("JAC_Absorption_Entry", &core)];
    if let Some(handoff) = handoff {
        steps.push(json!({"action":"native_observer_ready","timeout":240,"handoff":handoff}));
    }
    steps.extend([
        ui("InGame", if handoff.is_some() { NATIVE_REQUEST_UI } else { SINGLE_REQUEST_UI }),
        json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":"local d=ExposedMembers.JAC_LoyaltyNativeProbe;return d and d.Issued and d.Publications>=2"}}),
        ui("InGame", "local d=assert(ExposedMembers.JAC_LoyaltyNativeProbe);assert(Game.GetCurrentGameTurn()==d.Turn,'native probe turn changed');local c=assert(CityManager.GetCityAt(d.X,d.Y));assert(c:GetOwner()==d.Owner and c:GetID()==d.ID,'native probe city replaced');d.After=c:GetCulturalIdentity():GetLoyalty();print('JAC_LOYALTY_NATIVE_RESULT owner='..d.Owner..' city='..d.ID..' before='..d.Before..' after='..d.After..' cityEvents='..d.Events..' publications='..d.Publications);return 'raw comparison recorded, not a product acceptance'"),
    ]);
    if let Some(handoff) = handoff {
        steps.push(json!({"action":"native_observer_finish","handoff":handoff,"timeout":120}));
    }
    steps
}

fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let output = PathBuf::from(args.next().context("absolute output JSON path required")?);
    let option = args.next();
    let operation_mode = option
        .as_deref()
        .is_some_and(|v| v == "--operation-context");
    let native_mode = option.as_deref().is_some_and(|v| v == "--native-observer");
    let single_mode = option.as_deref().is_some_and(|v| v == "--single-setter-ui");
    let publish_mode = option
        .as_deref()
        .is_some_and(|v| v == "--single-setter-publish-ui");
    let ai_mode = option
        .as_deref()
        .is_some_and(|v| v == "--single-setter-ai-ui");
    let cross_mode = option
        .as_deref()
        .is_some_and(|v| v == "--single-setter-ai-cross-ui");
    let capital_mode = option
        .as_deref()
        .is_some_and(|v| v == "--single-setter-ai-capital-ui");
    let transfer_mode = option
        .as_deref()
        .is_some_and(|v| v == "--single-setter-transfer-ui");
    let direct_core_mode = option
        .as_deref()
        .is_some_and(|v| v == "--single-setter-transfer-direct-core");
    let reload_mode = option
        .as_deref()
        .is_some_and(|v| v == "--single-setter-transfer-reload-ui");
    let handoff = if native_mode {
        Some(PathBuf::from(
            args.next()
                .context("absolute native observer handoff path required")?,
        ))
    } else {
        None
    };
    ensure!(
        output.is_absolute() && (option.is_none() || operation_mode || native_mode || single_mode || publish_mode || ai_mode || cross_mode || capital_mode || transfer_mode || direct_core_mode || reload_mode) && args.next().is_none()
            && handoff.as_ref().is_none_or(|p| p.is_absolute()),
        "usage: build-absorption-loyalty-diagnostic <absolute-new-json-path> [--operation-context|--native-observer <absolute-new-handoff-path>|--single-setter-ui|--single-setter-publish-ui|--single-setter-ai-ui|--single-setter-ai-cross-ui|--single-setter-ai-capital-ui|--single-setter-transfer-ui|--single-setter-transfer-direct-core|--single-setter-transfer-reload-ui]"
    );
    ensure!(
        !output.exists(),
        "output already exists; preserve original experiments"
    );
    let base: Value = serde_json::from_str(include_str!(
        "../../../civ6-autotest/scenarios/absorption-product-small-ai.json"
    ))?;
    let setup = base["steps"]
        .as_array()
        .context("base scene steps missing")?;
    ensure!(setup.len() >= 8, "base scene setup is incomplete");
    let mut steps = setup[..8].to_vec();
    if native_mode {
        steps.extend(native_probe_steps(handoff.as_ref().unwrap()));
    } else if single_mode {
        steps.extend(loyalty_probe_steps(None));
    } else if publish_mode {
        steps.extend(publication_probe_steps());
    } else if ai_mode {
        steps.extend(ai_probe_steps());
    } else if cross_mode {
        steps.extend(cross_context_probe_steps());
    } else if capital_mode {
        steps.extend(capital_probe_steps());
    } else if transfer_mode {
        steps.extend(transfer_probe_steps());
    } else if direct_core_mode {
        steps.extend(transfer_direct_core_probe_steps());
    } else if reload_mode {
        steps.extend(transfer_reload_probe_steps());
    } else {
        steps.push(ui("InGame", UI));
        let core = CORE.replace(
            "local operationMode=false",
            &format!("local operationMode={operation_mode}"),
        );
        steps.push(ui("JAC_Absorption_Entry", &core));
        if operation_mode {
            for label in [
                "human-operation-setter",
                "human-operation-modifier",
                "ai-operation-setter",
                "ai-operation-modifier",
                "human-operation-control",
            ] {
                steps.push(ui("InGame", &format!("JAC_LOYALTY_MATRIX.Publications=0;UI.RequestPlayerOperation(Game.GetLocalPlayer(),PlayerOperations.EXECUTE_SCRIPT,{{OnStart='JAC_LoyaltyMatrix_Operation',Label='{label}'}});return 'one synchronized native operation requested'")));
                steps.push(json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":format!("return ExposedMembers.JAC_LoyaltyMatrixLastOperation()=='{label}' and JAC_LOYALTY_MATRIX.Publications>=2")}}));
                steps.push(ui("JAC_Absorption_Entry", &format!("ExposedMembers.JAC_LoyaltyMatrixSnapshot('{label}:after-operation');return 'actual values after synchronized operation'")));
            }
            steps.push(ui("InGame", "print('JAC_LOYALTY_MATRIX_EVENT_TOTAL '..JAC_LOYALTY_MATRIX.Events);return JAC_LOYALTY_MATRIX.Events"));
        } else {
            steps.push(json!({"action":"end_turn","turns":1,"timeout":600}));
            steps.push(ui("JAC_Absorption_Entry", "ExposedMembers.JAC_LoyaltyMatrixSnapshot('after-real-turn');return 'native values recorded after actual turn'"));
            steps.push(ui(
                "InGame",
                "JAC_LOYALTY_MATRIX.Publications=0;return true",
            ));
            steps.push(ui(
                "JAC_Absorption_Entry",
                "return ExposedMembers.JAC_LoyaltyMatrixPositive()",
            ));
            steps.push(json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":"return JAC_LOYALTY_MATRIX.Publications>=2"}}));
            steps.push(ui("JAC_Absorption_Entry", "ExposedMembers.JAC_LoyaltyMatrixSnapshot('positive-after-publication');return 'positive actual values recorded'"));
            steps.push(json!({"action":"end_turn","turns":1,"timeout":600}));
            steps.push(ui("JAC_Absorption_Entry", "local d=ExposedMembers.JAC_LoyaltyMatrixSnapshot('positive-after-real-turn');print('JAC_LOYALTY_MATRIX_AI_ACTIVE_CALL '..tostring(d.Calls['ai-active-negative']));return 'finite comparison complete'"));
            steps.push(ui("InGame", "print('JAC_LOYALTY_MATRIX_EVENT_TOTAL '..JAC_LOYALTY_MATRIX.Events);return JAC_LOYALTY_MATRIX.Events"));
        }
    }
    let scene = json!({"name":"Finite native loyalty current/rate/owner-turn diagnosis","judge_profile":"generic","loadscreen_autostart":true,"steps":steps});
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?;
    file.write_all(serde_json::to_string_pretty(&scene)?.as_bytes())?;
    println!(
        "Generated {} finite steps: {}",
        scene["steps"].as_array().unwrap().len(),
        output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reload_comparison_keeps_original_transfer_and_rejects_changed_saved_identity() -> Result<()>
    {
        let steps = transfer_reload_probe_steps();
        assert_eq!(&steps[6..], transfer_probe_steps().as_slice());
        assert_eq!(steps[1]["action"], "save");
        assert_eq!(steps[2]["action"], "exit_to_menu");
        assert_eq!(steps[3]["action"], "load");
        assert_eq!(steps[1]["name"], steps[3]["name"]);
        let lua = transfer_fixture()?;
        lua.load("Domain='Core';Saved={};Game.SetProperty=function(_,k,v) Saved[k]=v end;Game.GetProperty=function(_,k) return Saved[k] end").exec()?;
        lua.load(steps[0]["code"].as_str().unwrap()).exec()?;
        lua.load(steps[5]["code"].as_str().unwrap()).exec()?;
        for change in ["Turn=2", "CoreID=12", "CoreOwner=0", "CoreX=7", "CoreY=8"] {
            lua.load("Turn=1;CoreID=11;CoreOwner=1;CoreX=5;CoreY=6")
                .exec()?;
            lua.load(change).exec()?;
            assert!(lua.load(steps[5]["code"].as_str().unwrap()).exec().is_err());
        }
        lua.load("assert(Transfers==0 and Setters==0 and #Requests==0)")
            .exec()?;
        for step in &steps {
            if let Some(code) = step["code"]
                .as_str()
                .or_else(|| step["condition"]["code"].as_str())
            {
                lua.load(format!("return function()\n{code}\nend"))
                    .into_function()?;
            }
        }
        Ok(())
    }

    fn transfer_fixture() -> Result<mlua::Lua> {
        transfer_fixture_for_steps(transfer_probe_steps())
    }

    fn transfer_fixture_for_steps(steps: Vec<Value>) -> Result<mlua::Lua> {
        let lua = mlua::Lua::new();
        lua.globals()
            .set("FixtureInstance", lua.create_any_userdata(())?)?;
        lua.load(r#"
Domain='UI';Turn=1;Local=3;Active=true;DonorCount=2
CoreOwner=1;CoreID=11;CoreX=5;CoreY=6
CacheOwner=1;CacheID=11;CacheX=5;CacheY=6;Current=100
Transfers=0;Setters=0;Reads=0;Requests={};Order={};Lines={}
local handlers={};local publications={};local cityEvents={}
local identity={__instance=FixtureInstance,GetLoyalty=function() assert(Domain=='UI');Reads=Reads+1;return Current end}
OriginalIdentity=identity
local cache={GetOwner=function() return CacheOwner end,GetID=function() return CacheID end,
 GetX=function() return CacheX end,GetY=function() return CacheY end,
 GetCulturalIdentity=function() assert(Domain=='UI');return identity end}
local core={GetOwner=function() return CoreOwner end,GetID=function() return CoreID end,
 GetX=function() return CoreX end,GetY=function() return CoreY end,
 GetCulturalIdentity=function() error('Core must never read UI getter') end,
 ChangeLoyalty=function(_,v)
  assert(Domain=='Core' and v==-5 and Transfers==1 and ExposedMembers.JAC_LoyaltyNativeProbe.TransferUIReady)
  Setters=Setters+1;Order[#Order+1]='setter';Current=Current+v
  if Reentrant then assert(not pcall(InvokeSetter)) end
 end}
local cities={GetCapitalCity=function() return Domain=='UI' and cache or core end,
 GetCount=function() return DonorCount end,Members=function() error('capital must use GetCapitalCity') end}
Players={[1]={GetCities=function() return cities end,IsHuman=function() return false end},
 [0]={IsHuman=function() return false end},
 [3]={IsHuman=function() return true end,IsTurnActive=function() return Active end}}
Game={GetLocalPlayer=function() return Local end,GetCurrentGameTurn=function() return Turn end}
CityManager={GetCityAt=function(x,y) assert(x==5 and y==6);return Domain=='UI' and cache or core end,
 TransferCity=function(c,owner)
  assert(Domain=='Core' and c==core and owner==0)
  local d=ExposedMembers.JAC_LoyaltyNativeProbe
  assert(d.TransferIntent and d.TransferCalls==1 and not d.UIRequestIssued)
  Transfers=Transfers+1;Order[#Order+1]='transfer'
  if Reentrant then assert(not pcall(InvokeTransfer)) end
  CoreOwner=0;CoreID=22;DonorCount=1
  if BadTransferOwner then CoreOwner=2 end
  if BadTransferCoords then CoreX=7 end
  if UnknownTransfer then error('transfer effect happened; return unknown') end
 end}
ExposedMembers={};DB={MakeHash=function() return 19 end};rawget=nil
print=function(line) Lines[#Lines+1]=line end
Events={CityLoyaltyChanged={Add=function(f) cityEvents[#cityEvents+1]=f end},
 GameCoreEventPublishComplete={Add=function(f) publications[#publications+1]=f end}}
GameEvents={JAC_LoyaltyTransferProbe_Operation={Add=function(f) handlers.transfer=f end},
 JAC_LoyaltyNativeProbe_Operation={Add=function(f) handlers.setter=f end}}
PlayerOperations={EXECUTE_SCRIPT=19}
TransferParams=function() return {Tag='transfer-probe',Owner=1,ID=11,X=5,Y=6,Turn=1} end
SetterParams=function() return {Tag='native-probe',Owner=0,ID=22,X=5,Y=6,Turn=1} end
InvokeTransfer=function(actor,params) handlers.transfer(actor or 3,params or TransferParams()) end
InvokeSetter=function(actor,params) handlers.setter(actor or 3,params or SetterParams()) end
Dispatch=function(actor,params)
 local old=Domain;Domain='Core'
 local f=params.OnStart=='JAC_LoyaltyTransferProbe_Operation' and InvokeTransfer or InvokeSetter
 local ok,err=pcall(f,actor,params);Domain=old;assert(ok,err)
end
UI={RequestPlayerOperation=function(actor,op,params)
 assert(Domain=='UI' and actor==3 and op==19)
 Requests[#Requests+1]=params
 local d=ExposedMembers.JAC_LoyaltyNativeProbe
 if params.OnStart=='JAC_LoyaltyNativeProbe_Operation' then
  assert(d.TransferUIReady and d.ImmediateBefore==Current and d.Events==0 and d.Publications==0)
 end
 if Reentrant then assert(not pcall(RepeatRequest,params.OnStart)) end
 if not DelayDispatch then Dispatch(actor,params) end
 if UnknownRequest==params.OnStart then error('request sent; response unknown') end
end}
FireCity=function(owner,id) assert(Domain=='UI');for _,f in ipairs(cityEvents) do f(owner,id) end end
Fire=function() assert(Domain=='UI');for _,f in ipairs(publications) do f() end end
PublishTransfer=function()
 assert(Domain=='UI');CacheOwner=CoreOwner;CacheID=CoreID;CacheX=CoreX;CacheY=CoreY
 identity={__instance=FixtureInstance,GetLoyalty=OriginalIdentity.GetLoyalty}
 FireCity(CacheOwner,CacheID);Fire()
end
"#).exec()?;
        lua.load(steps[0]["code"].as_str().unwrap()).exec()?;
        lua.load("Domain='Core'").exec()?;
        lua.load(steps[1]["code"].as_str().unwrap()).exec()?;
        lua.load("Domain='UI'").exec()?;
        let repeat = lua.create_function(|lua, name: String| {
            lua.load(if name == "JAC_LoyaltyTransferProbe_Operation" {
                TRANSFER_REQUEST_UI
            } else {
                TRANSFER_SETTER_UI
            })
            .exec()
        })?;
        lua.globals().set("RepeatRequest", repeat)?;
        Ok(lua)
    }

    #[test]
    fn transfer_direct_core_comparison_does_not_send_a_setter_operation_or_replay() -> Result<()> {
        let steps = transfer_direct_core_probe_steps();
        let lua = transfer_fixture_for_steps(steps.clone())?;
        assert!(lua.load(steps[4]["code"].as_str().unwrap()).exec().is_err());
        lua.load(TRANSFER_REQUEST_UI).exec()?;
        lua.load("PublishTransfer();Reentrant=true").exec()?;
        lua.load(steps[4]["code"].as_str().unwrap()).exec()?;
        lua.load("assert(Transfers==1 and Setters==0 and #Requests==1);Domain='Core'")
            .exec()?;
        lua.load(steps[5]["code"].as_str().unwrap()).exec()?;
        assert!(lua.load(steps[5]["code"].as_str().unwrap()).exec().is_err());
        lua.load("assert(Setters==1 and #Requests==1 and Current==95);Domain='UI';FireCity(0,22);Fire();Fire();assert(ExposedMembers.JAC_LoyaltyNativeProbe.Events==1 and ExposedMembers.JAC_LoyaltyNativeProbe.Publications==2)").exec()?;
        lua.load(steps[7]["code"].as_str().unwrap()).exec()?;
        Ok(())
    }

    #[test]
    fn transfer_direct_core_unknown_outcome_and_wrong_key_are_never_reissued() -> Result<()> {
        for wrong_key in [false, true] {
            let steps = transfer_direct_core_probe_steps();
            let lua = transfer_fixture_for_steps(steps.clone())?;
            lua.load(TRANSFER_REQUEST_UI).exec()?;
            lua.load("PublishTransfer()").exec()?;
            lua.load(steps[4]["code"].as_str().unwrap()).exec()?;
            if wrong_key {
                lua.load("ExposedMembers.JAC_LoyaltyNativeProbe.DirectDiagnosticParams.ID=999")
                    .exec()?;
            } else {
                lua.load("Domain='Core';local c=CityManager.GetCityAt(5,6);local old=c.ChangeLoyalty;c.ChangeLoyalty=function(self,v) old(self,v);error('effect happened; result unknown') end").exec()?;
            }
            lua.load("Domain='Core'").exec()?;
            assert!(lua.load(steps[5]["code"].as_str().unwrap()).exec().is_err());
            assert!(lua.load(steps[5]["code"].as_str().unwrap()).exec().is_err());
            lua.load(if wrong_key {
                "assert(Setters==0 and Transfers==1 and #Requests==1)"
            } else {
                "assert(Setters==1 and Transfers==1 and #Requests==1)"
            })
            .exec()?;
        }
        Ok(())
    }

    #[test]
    fn transfer_probe_executes_core_callbacks_after_ui_barrier_once() -> Result<()> {
        let lua = transfer_fixture()?;
        lua.load("Reentrant=true;local d=ExposedMembers.JAC_LoyaltyNativeProbe;assert(d.Actor==3 and d.Owner==1 and d.ID==11 and d.BaselineIdentity==OriginalIdentity and type(d.BaselineInstance)=='userdata');assert(Transfers==0 and Setters==0 and #Requests==0);FireCity(1,11);Fire();assert(d.Events==0 and d.Publications==0)").exec()?;
        assert!(lua.load(TRANSFER_SETTER_UI).exec().is_err());
        lua.load(TRANSFER_REQUEST_UI).exec()?;
        lua.load("local d=ExposedMembers.JAC_LoyaltyNativeProbe;assert(Transfers==1 and Setters==0 and #Requests==1 and d.CoreTransferredID==22 and d.Owner==1 and d.ID==11);Domain='Core';assert(not pcall(InvokeSetter));Domain='UI'").exec()?;
        assert!(lua.load(TRANSFER_SETTER_UI).exec().is_err());
        // Core completion alone must not accept the old CacheCity at publication.
        lua.load("assert(not pcall(Fire));assert(#Requests==1 and Setters==0);PublishTransfer();local d=ExposedMembers.JAC_LoyaltyNativeProbe;assert(d.TransferUIReady and d.Owner==0 and d.ID==22 and d.TransferAfter==100 and d.Events==0 and d.Publications==0 and d.BaselineIdentity==OriginalIdentity)").exec()?;
        // A fresh getter after publication is the setter baseline, not donor's 100.
        lua.load("local d=ExposedMembers.JAC_LoyaltyNativeProbe;assert(d.TransferIdentity~=d.BaselineIdentity and type(d.TransferInstance)=='userdata');Domain='Core';assert(not pcall(InvokeSetter));Domain='UI';assert(Setters==0 and #Requests==1 and d.ImmediateBefore==nil);FireCity(0,22);Fire();assert(d.Events==0 and d.Publications==0);Current=99").exec()?;
        lua.load(TRANSFER_SETTER_UI).exec()?;
        lua.load("local d=ExposedMembers.JAC_LoyaltyNativeProbe;assert(d.Before==99 and d.ImmediateBefore==99 and Current==94 and d.OriginalBefore==100);FireCity(1,11);FireCity(0,999);FireCity(0,22);Fire();Fire();assert(d.Events==1 and d.Publications==2 and d.TransferPublications==1);assert(Transfers==1 and Setters==1 and #Requests==2 and Order[1]=='transfer' and Order[2]=='setter' and #Order==2);assert(d.ObserverReady==nil);Domain='Core';assert(not pcall(InvokeTransfer));assert(not pcall(InvokeSetter));Domain='UI'").exec()?;
        let steps = transfer_probe_steps();
        assert!(lua
            .load(steps[5]["condition"]["code"].as_str().unwrap())
            .eval::<bool>()?);
        lua.load(steps[6]["code"].as_str().unwrap()).exec()?;
        assert!(lua.load(TRANSFER_REQUEST_UI).exec().is_err());
        assert!(lua.load(TRANSFER_SETTER_UI).exec().is_err());
        lua.load("assert(Transfers==1 and Setters==1 and #Requests==2)")
            .exec()?;
        Ok(())
    }

    #[test]
    fn transfer_probe_unknown_transfer_and_requests_cannot_replay() -> Result<()> {
        for (fault, setter) in [
            ("UnknownTransfer=true", false),
            ("UnknownRequest='JAC_LoyaltyTransferProbe_Operation'", false),
            (
                "DelayDispatch=true;UnknownRequest='JAC_LoyaltyTransferProbe_Operation'",
                false,
            ),
            ("UnknownRequest='JAC_LoyaltyNativeProbe_Operation'", true),
            (
                "DelayDispatch=true;UnknownRequest='JAC_LoyaltyNativeProbe_Operation'",
                true,
            ),
        ] {
            let lua = transfer_fixture()?;
            if setter {
                lua.load(TRANSFER_REQUEST_UI).exec()?;
                lua.load("PublishTransfer()").exec()?;
                lua.load(fault).exec()?;
                assert!(lua.load(TRANSFER_SETTER_UI).exec().is_err());
                assert!(lua.load(TRANSFER_SETTER_UI).exec().is_err());
                lua.load("assert(#Requests==2 and Transfers==1);if DelayDispatch then assert(Setters==0);Dispatch(3,Requests[2]) end;assert(Setters==1);Domain='Core';assert(not pcall(InvokeSetter));Domain='UI'").exec()?;
            } else {
                lua.load(fault).exec()?;
                assert!(lua.load(TRANSFER_REQUEST_UI).exec().is_err());
                assert!(lua.load(TRANSFER_REQUEST_UI).exec().is_err());
                lua.load("assert(#Requests==1 and Setters==0);if DelayDispatch then assert(Transfers==0);Dispatch(3,Requests[1]) end;assert(Transfers==1);Domain='Core';assert(not pcall(InvokeTransfer));Domain='UI'").exec()?;
                if fault == "UnknownTransfer=true" {
                    assert!(lua.load(TRANSFER_SETTER_UI).exec().is_err());
                    lua.load("Fire();assert(not ExposedMembers.JAC_LoyaltyNativeProbe.TransferComplete and #Requests==1 and Setters==0)").exec()?;
                }
            }
        }
        Ok(())
    }

    #[test]
    fn transfer_probe_rejects_wrong_original_and_setter_callback_identity() -> Result<()> {
        for setter in [false, true] {
            for fault in [
                "CoreOwner=2",
                "CoreID=99",
                "CoreX=7",
                "CoreY=7",
                "Turn=2",
                "Active=false",
                "params.Tag='wrong'",
                "params.Owner=2",
                "params.ID=99",
                "params.X=7",
                "params.Y=7",
                "params.Turn=2",
                "actor=0",
            ] {
                let lua = transfer_fixture()?;
                if setter {
                    lua.load(TRANSFER_REQUEST_UI).exec()?;
                    lua.load("PublishTransfer()").exec()?;
                    lua.load("DelayDispatch=true").exec()?;
                    lua.load(TRANSFER_SETTER_UI).exec()?;
                } else {
                    lua.load("DelayDispatch=true").exec()?;
                    lua.load(TRANSFER_REQUEST_UI).exec()?;
                }
                lua.load(format!("local params=Requests[#Requests];local actor=3;{fault};Domain='Core';assert(not pcall({},actor,params));Domain='UI';assert(Transfers=={} and Setters==0)",
                    if setter { "InvokeSetter" } else { "InvokeTransfer" }, usize::from(setter))).exec()?;
            }
        }
        Ok(())
    }

    #[test]
    fn transfer_probe_rejects_bad_reacquisition_donor_and_ui_keys() -> Result<()> {
        for fault in [
            "BadTransferOwner=true",
            "BadTransferCoords=true",
            "DonorCount=1",
        ] {
            let lua = transfer_fixture()?;
            lua.load(fault).exec()?;
            assert!(lua.load(TRANSFER_REQUEST_UI).exec().is_err());
            assert!(lua.load(TRANSFER_REQUEST_UI).exec().is_err());
            assert!(lua.load(TRANSFER_SETTER_UI).exec().is_err());
            lua.load("assert(Setters==0 and #Requests==1);Fire();assert(not ExposedMembers.JAC_LoyaltyNativeProbe.TransferUIReady)").exec()?;
        }
        for fault in [
            "CacheOwner=1",
            "CacheID=11",
            "CacheX=7",
            "CacheY=7",
            "Turn=2",
            "Local=0",
            "Active=false",
        ] {
            let lua = transfer_fixture()?;
            lua.load(TRANSFER_REQUEST_UI).exec()?;
            lua.load(format!("CacheOwner=0;CacheID=22;{fault};assert(not pcall(Fire));assert(not ExposedMembers.JAC_LoyaltyNativeProbe.TransferUIReady and Setters==0 and #Requests==1)")).exec()?;
            assert!(lua.load(TRANSFER_SETTER_UI).exec().is_err());
        }
        // The getter closure follows the new key and rejects a stale donor city.
        for fault in [
            "CacheOwner=1",
            "CacheID=11",
            "CacheX=7",
            "CacheY=7",
            "Turn=2",
            "Local=0",
            "Active=false",
        ] {
            let lua = transfer_fixture()?;
            lua.load(TRANSFER_REQUEST_UI).exec()?;
            lua.load("PublishTransfer()").exec()?;
            lua.load(fault).exec()?;
            assert!(lua.load(TRANSFER_SETTER_UI).exec().is_err());
            lua.load("assert(Setters==0 and #Requests==1)").exec()?;
        }
        Ok(())
    }

    #[test]
    fn transfer_probe_generated_scene_is_finite_and_lua51_compiles() -> Result<()> {
        let lua = mlua::Lua::new();
        assert_eq!(lua.load("return _VERSION").eval::<String>()?, "Lua 5.1");
        let steps = transfer_probe_steps();
        assert_eq!(steps.len(), 7);
        for step in &steps {
            assert!(matches!(
                step["action"].as_str(),
                Some("ui_lua" | "wait_for")
            ));
            let code = step["code"]
                .as_str()
                .or_else(|| step["condition"]["code"].as_str())
                .unwrap();
            lua.load(format!("return function()\n{code}\nend"))
                .into_function()?;
            for forbidden in [
                "ObserverReady=",
                "rawget",
                "AttachModifierByID",
                "UnitManager",
                "ReadProcessMemory",
            ] {
                assert!(!code.contains(forbidden));
            }
            if step["action"] == "wait_for" {
                assert_eq!(step["timeout"], 60);
            }
        }
        assert_eq!(
            steps[1]["code"]
                .as_str()
                .unwrap()
                .matches("CityManager.TransferCity(")
                .count(),
            1
        );
        assert_eq!(
            steps[1]["code"]
                .as_str()
                .unwrap()
                .matches("c:ChangeLoyalty(-5)")
                .count(),
            1
        );
        assert!(!steps[1]["code"].as_str().unwrap().contains("ReadCurrent("));
        let base: Value = serde_json::from_str(include_str!(
            "../../../civ6-autotest/scenarios/absorption-product-small-ai.json"
        ))?;
        // Check the actual unchanged setup chunks too, without running a game.
        for step in &base["steps"].as_array().unwrap()[..8] {
            if let Some(code) = step["code"]
                .as_str()
                .or_else(|| step["condition"]["code"].as_str())
            {
                lua.load(format!("return function()\n{code}\nend"))
                    .into_function()?;
            }
        }
        Ok(())
    }
    #[test]
    fn capital_probe_selects_engine_capital_without_searching_noncapitals() -> Result<()> {
        let lua = mlua::Lua::new();
        lua.load(r#"
rawget=nil
local identity={__instance={},GetLoyalty=function() return 100 end}
local capital={GetOwner=function() return 0 end,GetID=function() return 9 end,
 GetX=function() return 12 end,GetY=function() return 13 end,
 GetCulturalIdentity=function() return identity end}
local cities={GetCapitalCity=function() return capital end,Members=function() error('must select native capital') end}
Players={[0]={GetCities=function() return cities end},[3]={IsTurnActive=function() return true end}}
Game={GetLocalPlayer=function() return 3 end,GetCurrentGameTurn=function() return 1 end}
ExposedMembers={};DB={MakeHash=function() return 19 end}
Events={CityLoyaltyChanged={Add=function() end},GameCoreEventPublishComplete={Add=function() end}}
"#).exec()?;
        let steps = capital_probe_steps();
        let baseline = ai_probe_steps();
        for index in 1..5 {
            assert_eq!(steps[index], baseline[index]);
        }
        lua.load(steps[0]["code"].as_str().unwrap()).exec()?;
        lua.load("local d=ExposedMembers.JAC_LoyaltyNativeProbe;assert(d.Actor==3 and d.Owner==0 and d.ID==9 and d.X==12 and d.Y==13 and d.Before==100 and d.ObserverReady==nil and not d.MutationIssued)").exec()?;
        Ok(())
    }

    #[test]
    fn cross_context_comparison_changes_only_one_pre_setter_getter_call() -> Result<()> {
        let baseline = ai_probe_steps();
        let comparison = cross_context_probe_steps();
        for index in [0, 2, 3, 4] {
            assert_eq!(comparison[index], baseline[index]);
        }
        let extra = "d.CrossContextBefore=d.ReadCurrent('cross-context-before-setter')\n ";
        let code = comparison[1]["code"].as_str().unwrap();
        assert_eq!(code.matches(extra).count(), 1);
        assert_eq!(
            code.replace(extra, ""),
            baseline[1]["code"].as_str().unwrap()
        );
        assert_eq!(code.matches("c:ChangeLoyalty(-5)").count(), 1);
        let lua = mlua::Lua::new();
        lua.load(r#"
local handler;Reads=0;Calls=0
ExposedMembers={JAC_LoyaltyNativeProbe={Actor=3,Owner=0,ID=8,X=5,Y=6,Turn=1,Before=100,
 UIRequestIssued=true,ImmediateBefore=100,ReadCurrent=function(label)
 assert(label=='cross-context-before-setter');Reads=Reads+1;return 100 end}}
Players={[3]={IsTurnActive=function() return true end}}
Game={GetCurrentGameTurn=function() return 1 end}
CityManager={GetCityAt=function() return {GetOwner=function() return 0 end,GetID=function() return 8 end,
 ChangeLoyalty=function(_,v) assert(v==-5 and Reads==1);Calls=Calls+1 end} end}
GameEvents={JAC_LoyaltyNativeProbe_Operation={Add=function(f) handler=f end}}
Invoke=function() handler(3,{Tag='native-probe'}) end
"#).exec()?;
        lua.load(code).exec()?;
        lua.load("Invoke();assert(Reads==1 and Calls==1);assert(not pcall(Invoke));assert(Reads==1 and Calls==1)").exec()?;
        Ok(())
    }

    #[test]
    fn ai_probe_separates_active_sender_from_inactive_city_owner() -> Result<()> {
        let lua = mlua::Lua::new();
        lua.load(r#"
Domain='UI';Calls=0;Requests=0;rawget=nil;local handler
local identity={__instance={},GetLoyalty=function() assert(Domain=='UI');return 100 end}
local capital={GetID=function() return 1 end}
local target={GetOwner=function() return 0 end,GetID=function() return 8 end,
 GetX=function() return 5 end,GetY=function() return 6 end,
 GetCulturalIdentity=function() assert(Domain=='UI');return identity end,
 ChangeLoyalty=function(_,v) assert(Domain=='Core' and v==-5);Calls=Calls+1 end}
local cities={GetCapitalCity=function() return capital end,Members=function() return ipairs({capital,target}) end}
Players={[0]={GetCities=function() return cities end,IsTurnActive=function() return false end},
 [3]={IsTurnActive=function() return true end,GetCities=function() error('wrong human target') end}}
Game={GetLocalPlayer=function() return 3 end,GetCurrentGameTurn=function() return 1 end}
CityManager={GetCityAt=function(x,y) assert(x==5 and y==6);return target end}
ExposedMembers={};DB={MakeHash=function() return 19 end}
Events={CityLoyaltyChanged={Add=function() end},GameCoreEventPublishComplete={Add=function() end}}
GameEvents={JAC_LoyaltyNativeProbe_Operation={Add=function(f) handler=f end}}
PlayerOperations={EXECUTE_SCRIPT=19}
UI={RequestPlayerOperation=function(actor,op,params)
 assert(actor==3 and op==19);Requests=Requests+1;Domain='Core';handler(actor,params);Domain='UI'
end}
"#).exec()?;
        let steps = ai_probe_steps();
        lua.load(steps[0]["code"].as_str().unwrap()).exec()?;
        lua.load(steps[1]["code"].as_str().unwrap()).exec()?;
        lua.load(steps[2]["code"].as_str().unwrap()).exec()?;
        lua.load("local d=ExposedMembers.JAC_LoyaltyNativeProbe;assert(d.Owner==0 and d.Actor==3 and d.ID==8 and Calls==1 and Requests==1 and d.ObserverReady==nil)").exec()?;
        assert!(lua.load(steps[2]["code"].as_str().unwrap()).exec().is_err());
        lua.load("assert(Calls==1 and Requests==1)").exec()?;
        Ok(())
    }

    #[test]
    fn publication_sender_issues_once_on_event_and_never_retries_unknown_send() -> Result<()> {
        let lua = mlua::Lua::new();
        lua.load(
            r#"
local handlers={};Reads=0;Requests=0
Events={GameCoreEventPublishComplete={Add=function(f) handlers[#handlers+1]=f end}}
Fire=function() for _,f in ipairs(handlers) do f() end end
ExposedMembers={JAC_LoyaltyNativeProbe={Owner=3,Before=100,Publications=9,
 ReadCurrent=function() Reads=Reads+1;return 100 end}}
PlayerOperations={EXECUTE_SCRIPT=19}
UI={RequestPlayerOperation=function(owner,op,params)
 assert(owner==3 and op==19 and params.Tag=='native-probe')
 Requests=Requests+1;Fire();error('request sent but response unknown')
end}
"#,
        )
        .exec()?;
        let steps = publication_probe_steps();
        assert_eq!(steps.len(), 5);
        lua.load(steps[2]["code"].as_str().unwrap()).exec()?;
        lua.load("assert(Requests==0 and Reads==0);assert(not pcall(Fire));Fire();assert(Requests==1 and Reads==1);assert(ExposedMembers.JAC_LoyaltyNativeProbe.ObserverReady==nil)").exec()?;
        Ok(())
    }

    #[test]
    fn single_ui_operation_reads_in_ui_and_mutates_once_without_observer() -> Result<()> {
        let lua = mlua::Lua::new();
        lua.load(
            r#"
Domain='UI';Reads=0;Calls=0;Requests=0;local handler
local d={Owner=3,ID=8,X=5,Y=6,Turn=1,Before=100,Publications=9,
 ReadCurrent=function() assert(Domain=='UI');Reads=Reads+1;return 100 end}
ExposedMembers={JAC_LoyaltyNativeProbe=d}
Players={[3]={IsTurnActive=function() return true end}}
Game={GetCurrentGameTurn=function() return 1 end}
CityManager={GetCityAt=function(x,y)
 assert(Domain=='Core' and x==5 and y==6)
 return {GetOwner=function() return 3 end,GetID=function() return 8 end,
 ChangeLoyalty=function(_,v) assert(v==-5 and Domain=='Core');Calls=Calls+1 end}
end}
GameEvents={JAC_LoyaltyNativeProbe_Operation={Add=function(f) handler=f end}}
PlayerOperations={EXECUTE_SCRIPT=19}
UI={RequestPlayerOperation=function(owner,op,params)
 assert(owner==3 and op==19 and d.UIRequestIssued and d.ImmediateBefore==100 and d.Publications==0)
 Requests=Requests+1;Domain='Core';handler(owner,params);Domain='UI'
end}
Invoke=function() handler(3,{Tag='native-probe'}) end
"#,
        )
        .exec()?;
        let steps = loyalty_probe_steps(None);
        assert_eq!(steps.len(), 5);
        for step in &steps {
            assert!(!step["action"]
                .as_str()
                .unwrap()
                .starts_with("native_observer"));
            if let Some(code) = step["code"]
                .as_str()
                .or_else(|| step["condition"]["code"].as_str())
            {
                lua.load(format!("return function()\n{code}\nend"))
                    .into_function()?;
            }
        }
        lua.load(steps[1]["code"].as_str().unwrap()).exec()?;
        lua.load("Domain='Core';assert(not pcall(Invoke));assert(Calls==0);Domain='UI'")
            .exec()?;
        lua.load(SINGLE_REQUEST_UI).exec()?;
        lua.load("assert(Calls==1 and Reads==1 and Requests==1 and ExposedMembers.JAC_LoyaltyNativeProbe.Issued);assert(ExposedMembers.JAC_LoyaltyNativeProbe.ObserverReady==nil)").exec()?;
        assert!(lua.load(SINGLE_REQUEST_UI).exec().is_err());
        lua.load("assert(Calls==1 and Reads==1 and Requests==1)")
            .exec()?;
        Ok(())
    }

    #[test]
    fn native_ui_identity_metadata_without_rawget_does_not_abort_or_mutate() -> Result<()> {
        let lua = mlua::Lua::new();
        lua.load(
            r#"
rawget=nil
local instance=setmetatable({}, {__tostring=function() return 'metadata fixture' end})
local identity={__instance=instance,GetLoyalty=function() return 100 end}
local capital={GetID=function() return 1 end}
local city={GetID=function() return 8 end,GetX=function() return 5 end,
 GetY=function() return 6 end,GetOwner=function() return 3 end,
 GetCulturalIdentity=function() return identity end,
 ChangeLoyalty=function() error('metadata must not mutate') end}
local cities={GetCapitalCity=function() return capital end,
 Members=function() return ipairs({capital,city}) end}
Players={[3]={IsTurnActive=function() return true end,GetCities=function() return cities end}}
Game={GetLocalPlayer=function() return 3 end,GetCurrentGameTurn=function() return 1 end}
ExposedMembers={}
Events={CityLoyaltyChanged={Add=function() end},GameCoreEventPublishComplete={Add=function() end}}
DB={MakeHash=function() return 19 end}
Lines={};print=function(line) Lines[#Lines+1]=line end
ExpectedIdentity=identity;ExpectedInstance=instance
"#,
        )
        .exec()?;
        lua.load(NATIVE_UI).exec()?;
        lua.load("local d=ExposedMembers.JAC_LoyaltyNativeProbe;assert(d.Before==100 and d.BaselineIdentity==ExpectedIdentity and d.BaselineInstance==ExpectedInstance);assert(not d.MutationIssued);local found=false;for _,line in ipairs(Lines) do if line:find('JAC_LOYALTY_NATIVE_INSTANCE',1,true) then found=true end end;assert(found)").exec()?;
        Ok(())
    }

    #[test]
    fn native_probe_arms_without_mutation_and_rejects_replay_and_replaced_city() -> Result<()> {
        let lua = mlua::Lua::new();
        lua.load(
            r#"
local handler
local city={GetOwner=function() return 3 end,GetID=function() return 8 end,
 ChangeLoyalty=function(_,amount) assert(amount==-5);Calls=Calls+1 end}
Calls=0
AddCount=0
Reads=0
ExposedMembers={JAC_LoyaltyNativeProbe={Owner=3,ID=8,X=5,Y=6,Turn=1,
 ReadCurrent=function(label) Reads=Reads+1;error('UI closure must not run in Gameplay') end}}
Players={[3]={IsTurnActive=function() return true end}}
Game={GetCurrentGameTurn=function() return 1 end}
CityManager={GetCityAt=function(x,y) assert(x==5 and y==6);return city end}
GameEvents={JAC_LoyaltyNativeProbe_Operation={Add=function(f) AddCount=AddCount+1;handler=f end}}
Invoke=function() handler(3,{Tag='native-probe'}) end
Replace=function() city.GetID=function() return 9 end end
"#,
        )
        .exec()?;
        lua.load(NATIVE_CORE).exec()?;
        lua.load(NATIVE_CORE).exec()?;
        lua.load("assert(AddCount==1 and Calls==0);assert(not pcall(Invoke));assert(Calls==0);ExposedMembers.JAC_LoyaltyNativeProbe.ObserverReady=true;Replace();assert(not pcall(Invoke));assert(Calls==0)")
            .exec()?;
        // Use a fresh binding for the successful once-only case.
        lua.load("CityManager.GetCityAt=function() return {GetOwner=function() return 3 end,GetID=function() return 8 end,ChangeLoyalty=function(_,v) assert(v==-5);Calls=Calls+1 end} end").exec()?;
        lua.load("Invoke();assert(Calls==1 and Reads==0 and ExposedMembers.JAC_LoyaltyNativeProbe.Issued);assert(not pcall(Invoke));assert(Calls==1 and Reads==0)").exec()?;
        Ok(())
    }

    #[test]
    fn native_ui_request_requires_its_baseline_before_the_one_operation() -> Result<()> {
        let lua = mlua::Lua::new();
        lua.load(r#"
Reads=0;Requests=0
local d={Owner=3,ObserverReady=true,Publications=7,
 ReadCurrent=function() error('cache not ready') end}
ExposedMembers={JAC_LoyaltyNativeProbe=d}
PlayerOperations={EXECUTE_SCRIPT=19}
UI={RequestPlayerOperation=function(owner,op,params)
 assert(owner==3 and op==19 and params.OnStart=='JAC_LoyaltyNativeProbe_Operation' and params.Tag=='native-probe')
 assert(Reads==1 and d.ImmediateBefore==100 and d.Publications==0)
 Requests=Requests+1
end}
"#).exec()?;
        assert!(lua.load(NATIVE_REQUEST_UI).exec().is_err());
        lua.load("assert(Requests==0);ExposedMembers.JAC_LoyaltyNativeProbe.ReadCurrent=function() Reads=Reads+1;return 100 end").exec()?;
        lua.load(NATIVE_REQUEST_UI).exec()?;
        lua.load("assert(Requests==1 and Reads==1);ExposedMembers.JAC_LoyaltyNativeProbe.MutationIssued=true").exec()?;
        assert!(lua.load(NATIVE_REQUEST_UI).exec().is_err());
        lua.load("assert(Requests==1 and Reads==1)").exec()?;
        Ok(())
    }

    #[test]
    fn native_probe_has_no_modifier_or_turn_advance_and_compiles_every_lua_chunk() -> Result<()> {
        let lua = mlua::Lua::new();
        let handoff = std::env::temp_dir().join("native-probe-handoff.json");
        let steps = native_probe_steps(&handoff);
        assert_eq!(steps.len(), 7);
        assert_eq!(steps[2]["action"], "native_observer_ready");
        assert_eq!(steps[2]["handoff"], json!(handoff));
        assert_eq!(steps[2]["timeout"], 240);
        assert_eq!(steps[6]["action"], "native_observer_finish");
        assert_eq!(steps[6]["handoff"], json!(handoff));
        for step in &steps {
            assert_ne!(step["action"], "end_turn");
            assert_ne!(step["action"], "autoplay");
            let code = step["code"]
                .as_str()
                .or_else(|| step["condition"]["code"].as_str());
            if let Some(code) = code {
                assert!(!code.contains("AttachModifierByID"));
                lua.load(format!("return function()\n{code}\nend"))
                    .into_function()?;
            }
        }
        Ok(())
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
