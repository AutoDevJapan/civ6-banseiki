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
 local instance=rawget(identity,'__instance')
 d.BaselineInstance=instance
 local mt=instance and getmetatable(instance)
 local custom=type(mt)=='table' and rawget(mt,'__tostring') or nil
 print('JAC_LOYALTY_NATIVE_INSTANCE kind='..type(instance)..' text='..tostring(instance)..' mtKind='..type(mt)..' customFormatterKind='..type(custom))
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

fn native_probe_steps(handoff: &std::path::Path) -> Vec<Value> {
    vec![
        ui("InGame", NATIVE_UI),
        ui("JAC_Absorption_Entry", NATIVE_CORE),
        json!({"action":"native_observer_ready","timeout":240,"handoff":handoff}),
        ui("InGame", NATIVE_REQUEST_UI),
        json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":"local d=ExposedMembers.JAC_LoyaltyNativeProbe;return d and d.Issued and d.Publications>=2"}}),
        ui("InGame", "local d=assert(ExposedMembers.JAC_LoyaltyNativeProbe);assert(Game.GetCurrentGameTurn()==d.Turn,'native probe turn changed');local c=assert(CityManager.GetCityAt(d.X,d.Y));assert(c:GetOwner()==d.Owner and c:GetID()==d.ID,'native probe city replaced');d.After=c:GetCulturalIdentity():GetLoyalty();print('JAC_LOYALTY_NATIVE_RESULT owner='..d.Owner..' city='..d.ID..' before='..d.Before..' after='..d.After..' cityEvents='..d.Events..' publications='..d.Publications);return 'raw comparison recorded, not a product acceptance'"),
        json!({"action":"native_observer_finish","handoff":handoff,"timeout":120}),
    ]
}

fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let output = PathBuf::from(args.next().context("absolute output JSON path required")?);
    let option = args.next();
    let operation_mode = option
        .as_deref()
        .is_some_and(|v| v == "--operation-context");
    let native_mode = option.as_deref().is_some_and(|v| v == "--native-observer");
    let handoff = if native_mode {
        Some(PathBuf::from(
            args.next()
                .context("absolute native observer handoff path required")?,
        ))
    } else {
        None
    };
    ensure!(
        output.is_absolute() && (option.is_none() || operation_mode || native_mode) && args.next().is_none()
            && handoff.as_ref().is_none_or(|p| p.is_absolute()),
        "usage: build-absorption-loyalty-diagnostic <absolute-new-json-path> [--operation-context|--native-observer <absolute-new-handoff-path>]"
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
