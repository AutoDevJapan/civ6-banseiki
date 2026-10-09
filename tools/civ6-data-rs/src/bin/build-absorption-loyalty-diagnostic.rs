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
call('human-active-negative',D.Human,'setter',-5)
call('human-active-native-negative',D.Human,'modifier','JAC_ABSORPTION_LOYALTY_LOSS_5')
call('ai-inactive-negative',D.AI,'setter',-5)
call('ai-inactive-native-negative',D.AI,'modifier','JAC_ABSORPTION_LOYALTY_LOSS_5')
-- Separate diagnostic city, deliberately beyond the +100 map stability.
-- This is an experiment, never a production-policy substitute or retry.
call('human-control-prepare',D.Control,'setter',-150)
GameEvents.PlayerTurnStartComplete.Add(function(actor)
 print('JAC_LOYALTY_MATRIX_TURN actor='..tostring(actor)..' active='..tostring(Players[actor]:IsTurnActive()))
 if actor==0 and Players[actor]:IsTurnActive() and not D.Calls['ai-active-negative'] then
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

fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let output = PathBuf::from(args.next().context("absolute output JSON path required")?);
    ensure!(
        output.is_absolute() && args.next().is_none(),
        "usage: build-absorption-loyalty-diagnostic <absolute-new-json-path>"
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
    steps.push(ui("InGame", UI));
    steps.push(ui("JAC_Absorption_Entry", CORE));
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

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
