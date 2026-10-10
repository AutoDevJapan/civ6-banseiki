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
    let handoff = if native_mode {
        Some(PathBuf::from(
            args.next()
                .context("absolute native observer handoff path required")?,
        ))
    } else {
        None
    };
    ensure!(
        output.is_absolute() && (option.is_none() || operation_mode || native_mode || single_mode || publish_mode || ai_mode) && args.next().is_none()
            && handoff.as_ref().is_none_or(|p| p.is_absolute()),
        "usage: build-absorption-loyalty-diagnostic <absolute-new-json-path> [--operation-context|--native-observer <absolute-new-handoff-path>|--single-setter-ui|--single-setter-publish-ui|--single-setter-ai-ui]"
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
