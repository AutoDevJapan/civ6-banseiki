use anyhow::Result;
use civ6_data_rs::{assassin, Repository};
use serde_json::{json, Value};

const OUTPUT: &str = "tools/civ6-autotest/scenarios/assassin-blocked-recovery.json";
const PREFIX: &str = r#"
local R=ExposedMembers.JAC_Assassin; assert(R,'assassin gameplay absent')
local t=Game:GetProperty('JAC_ASN_BLOCKED_TEST')
local function Same(a,b)
  if type(a)~=type(b) then return false end
  if type(a)~='table' then return a==b end
  for k,v in pairs(a) do if not Same(v,b[k]) then return false end end
  for k,v in pairs(b) do if not Same(v,a[k]) then return false end end
  return true
end
local function Row(u)
  assert(u and u:GetID()>=0,'unit missing')
  return {ID=u:GetID(),Owner=u:GetOwner(),Type=u:GetType(),X=u:GetX(),Y=u:GetY(),Damage=u:GetDamage()}
end
local function Trace(label,row)
  print('[JAC_ASN_BLOCKED] '..label..' turn='..Game.GetCurrentGameTurn()..
    ' id='..row.ID..' owner='..row.Owner..' type='..row.Type..' xy='..row.X..','..row.Y..' damage='..row.Damage)
end
local function Assassins(owner)
  local ids={}; for _,u in Players[owner]:GetUnits():Members() do
    if R.IsAssassin(u) then ids[tostring(u:GetID())]=Row(u) end
  end; return ids
end
local function UnitIDs(owner)
  local ids={}; for _,u in Players[owner]:GetUnits():Members() do ids[tostring(u:GetID())]=true end
  return ids
end
local function Blockers()
  for _,p in ipairs(t.Plots) do
    local plot=Map.GetPlot(p.X,p.Y); assert(plot,'fixture plot missing')
    if p.ID then
      local u=Players[t.Actor]:GetUnits():FindID(p.ID)
      assert(u and u:GetID()==p.ID and u:GetOwner()==t.Actor and u:GetX()==p.X and u:GetY()==p.Y,
        'blocking unit moved or disappeared')
      assert(GameInfo.Units[u:GetType()].FormationClass=='FORMATION_CLASS_LAND_COMBAT','wrong blocker formation')
    else assert(plot:IsWater() or plot:IsImpassable(),'spawnable plot left unblocked') end
  end
end
local function Pending()
  local s=R.State(t.Actor)
  print('[JAC_ASN_BLOCKED] inspect turn='..R.Turn()..' production='..tostring(s.Production~=nil)..
    ' completion='..tostring(s.LastCompletion)..' produced='..tostring(s.LastProducedUnit))
  for _,u in Players[t.Actor]:GetUnits():Members() do
    if not t.AllBefore[tostring(u:GetID())] then Trace('unexpected-spawn',Row(u)) end
  end
  Blockers()
  assert(R.Turn()>=t.Due and Game:GetProperty('JAC_ASN_TICK')==R.Turn(),'completion tick not reached')
  assert(s.Production and s.Production.City==t.City and s.Production.Start==t.Start and s.Production.Due<=R.Turn(),
    'blocked training disappeared or restarted')
  assert(s.LastCompletion==t.Initial.LastCompletion and s.ProductionUnlock==t.Initial.ProductionUnlock and
    s.LastProducedUnit==t.Initial.LastProducedUnit,'blocked spawn started completion or lock')
  assert(Same(Assassins(t.Actor),t.Before),'blocked spawn leaked an assassin')
  assert(Same(UnitIDs(t.Actor),t.AllBefore),'blocked spawn changed the complete unit inventory')
  print('[JAC_ASN_BLOCKED] pending turn='..R.Turn()..' due='..s.Production.Due..' completion='..
    tostring(s.LastCompletion)..' unlock='..tostring(s.ProductionUnlock))
  return s
end
"#;

fn check(steps: &mut Vec<Value>, code: &str, state: &str) {
    steps.push(assassin::ui(&format!("{PREFIX}\n{code}"), state));
}
fn advance(steps: &mut Vec<Value>) {
    steps.push(json!({"action":"end_turn","turns":1,"timeout":600}));
    steps.push(assassin::active());
}
fn round_trip(steps: &mut Vec<Value>, name: &str) {
    steps.extend([
        json!({"action":"save","name":name,"timeout":60}),
        json!({"action":"exit_to_menu","timeout":60}),
        json!({"action":"load","name":name,"timeout":300}),
        assassin::active(),
    ]);
}
fn build(repo: &Repository) -> Result<Value> {
    let base = repo.json("tools/civ6-autotest/scenarios/assassin-leader-save-load.json")?;
    let mut steps = base["steps"].as_array().unwrap()[..3].to_vec();
    steps.push(assassin::active());
    check(
        &mut steps,
        r#"
Game:SetProperty(R.TEST,1)
local owner=Game.GetLocalPlayer(); assert(Players[owner]:IsHuman(),'human required')
local city=Players[owner]:GetCities():GetCapitalCity(); assert(city,'capital missing')
local plots={Map.GetPlot(city:GetX(),city:GetY())}
for _,p in ipairs(Map.GetAdjacentPlots(city:GetX(),city:GetY())) do plots[#plots+1]=p end
assert(#plots==7,'complete radius-one footprint required')
local rows={}; local occupied={}
local function Occupy(p)
  local row={X=p:GetX(),Y=p:GetY()}
  if not p:IsWater() and not p:IsImpassable() then
    local blocker
    for _,u in Players[owner]:GetUnits():Members() do
      if u:GetX()==row.X and u:GetY()==row.Y and GameInfo.Units[u:GetType()].FormationClass=='FORMATION_CLASS_LAND_COMBAT' then
        assert(not blocker,'stacked combat blockers'); blocker=u
      end
    end
    if not blocker then
      local had={}; for _,u in Players[owner]:GetUnits():Members() do had[u:GetID()]=true end
      UnitManager.InitUnit(owner,'UNIT_WARRIOR',row.X,row.Y)
      for _,u in Players[owner]:GetUnits():Members() do if not had[u:GetID()] then
        assert(not blocker and u:GetType()==GameInfo.Units.UNIT_WARRIOR.Index,'ambiguous blocker creation'); blocker=u
      end end
    end
    assert(blocker and blocker:GetX()==row.X and blocker:GetY()==row.Y,'blocker not on requested plot')
    row.ID=blocker:GetID(); Trace('blocker',Row(blocker))
  end
  rows[#rows+1]=row; occupied[row.X..','..row.Y]=row
end
for _,p in ipairs(plots) do Occupy(p) end
-- The native API can search past the first ring. Discover its real placement
-- candidates instead of treating the fifth argument as a maximum radius.
local blocked=false; local kind=R.UnitForPlayer(owner)
for attempt=1,128 do
  local before=UnitIDs(owner); local probe; local count=0
  UnitManager.InitUnitValidAdjacentHex(owner,kind,city:GetX(),city:GetY(),1)
  for _,u in Players[owner]:GetUnits():Members() do if not before[tostring(u:GetID())] then
    count=count+1; probe=u
  end end
  assert(count<=1,'native placement probe created multiple units')
  if not probe then
    assert(Same(UnitIDs(owner),before),'failed native probe changed existing inventory')
    blocked=true; print('[JAC_ASN_BLOCKED] native-blocked attempts='..attempt..' plots='..#rows); break
  end
  local row=Row(probe); Trace('native-probe',row)
  assert(row.Type==GameInfo.Units[kind].Index,'native placement probe has wrong type')
  UnitManager.Kill(probe)
  assert(Same(UnitIDs(owner),before),'native probe cleanup changed existing inventory')
  if row.X<0 or row.Y<0 then
    blocked=true; print('[JAC_ASN_BLOCKED] native-blocked-unplaced attempts='..attempt..' plots='..#rows); break
  end
  local existing=occupied[row.X..','..row.Y]
  assert(not existing or not existing.ID,'native placement stacked onto an occupied combat plot')
  local plot=Map.GetPlot(row.X,row.Y)
  assert(plot and not plot:IsWater() and not plot:IsImpassable(),'native probe selected an invalid land plot')
  assert(not existing,'native probe selected an impassable footprint plot')
  Occupy(plot)
end
assert(blocked,'native placement did not become blocked within 128 probes')
t={Actor=owner,City=city:GetID(),X=city:GetX(),Y=city:GetY(),Plots=rows,
  Start=R.Turn(),Initial=R.State(owner),Before=Assassins(owner),AllBefore=UnitIDs(owner)}
assert(next(t.Before)==nil,'fresh game already contains assassins')
Game:SetProperty('JAC_ASN_BLOCKED_TEST',t); Blockers()
return 'real combat units block the native API after discovering its actual placement candidates'
"#,
        "GameCore_Tuner",
    );
    check(&mut steps,"assert(JAC_AssassinProduce(t.City),'training request not sent'); return 'training requested through UI'","JAC_AssassinPanel");
    steps.push(json!({"action":"wait_for","timeout":30,"condition":{"type":"lua","state_name":"GameCore_Tuner",
        "code":format!("{PREFIX} return R.State(t.Actor).Production~=nil")}}));
    check(
        &mut steps,
        r#"
local s=R.State(t.Actor); assert(s.Production.City==t.City and s.Production.Start==t.Start,'wrong training')
assert(s.Production.Turns>=20,'natural training below minimum')
local ok,due=R.TestAdvanceDeadlines(t.Actor,{Training=true}); assert(ok,'deadline hook refused')
t.Due=due; Game:SetProperty('JAC_ASN_BLOCKED_TEST',t)
return 'only the test deadline is shortened; native spawn and tick are unchanged'
"#,
        "GameCore_Tuner",
    );
    advance(&mut steps);
    check(&mut steps,"t.Pending=Pending(); Game:SetProperty('JAC_ASN_BLOCKED_TEST',t); return 'blocked spawn leaves training pending and lock unchanged'","GameCore_Tuner");
    round_trip(&mut steps, "JAC_ASSASSIN_BLOCKED_PENDING");
    check(&mut steps,"assert(Same(Pending(),t.Pending),'pending state changed on load'); return 'saved pending training and real blockers survive load'","GameCore_Tuner");
    advance(&mut steps);
    check(
        &mut steps,
        r#"
Pending()
for _,p in ipairs(t.Plots) do if p.ID then
  local u=Players[t.Actor]:GetUnits():FindID(p.ID); assert(u and u:GetID()==p.ID,'blocker identity changed')
  UnitManager.Kill(u)
end end
t.Released=R.Turn(); t.AllReleased=UnitIDs(t.Actor); Game:SetProperty('JAC_ASN_BLOCKED_TEST',t)
return 'second blocked real tick verified; fixture blockers removed without touching training'
"#,
        "GameCore_Tuner",
    );
    advance(&mut steps);
    check(
        &mut steps,
        r#"
local s=R.State(t.Actor); local count=0; local row
for _,u in Players[t.Actor]:GetUnits():Members() do if R.IsAssassin(u) then
  count=count+1; row=Row(u); assert(not t.Before[tostring(row.ID)],'reused production id')
end end
assert(count==1 and row.ID==s.LastProducedUnit and row.X>=0 and row.Y>=0,'recovery did not create exactly one placed assassin')
assert(row.Type==GameInfo.Units[R.UnitForPlayer(t.Actor)].Index,'recovered unit has the wrong civilization-specific type')
local newCount=0; local all=UnitIDs(t.Actor)
for id in pairs(all) do if not t.AllReleased[id] then newCount=newCount+1; assert(tonumber(id)==row.ID,'unexpected new unit during recovery') end end
for id in pairs(t.AllReleased) do assert(all[id],'unrelated unit disappeared during recovery') end
assert(newCount==1,'native recovery did not create exactly one new unit of any type')
assert(Map.GetPlotDistance(t.X,t.Y,row.X,row.Y)<=1,'spawn beyond native radius one')
assert(s.Production==nil and s.LastCompletion==R.Turn() and R.Turn()==t.Released+1,'wrong actual completion turn')
assert(s.ProductionUnlock==s.LastCompletion+R.Scale(R.PRODUCTION_LOCK),'lock did not start at actual completion')
t.Complete=s; t.Unit=row; Trace('gameplay-before-save',row); Game:SetProperty('JAC_ASN_BLOCKED_TEST',t)
local ok,why=R.StartProduction(t.Actor,t.City); assert(not ok and why=='PRODUCTION_LOCK','nationwide lock not enforced')
return 'one native placed assassin; full lock starts only at actual recovery'
"#,
        "GameCore_Tuner",
    );
    check(
        &mut steps,
        r#"
local u=Players[t.Actor]:GetUnits():FindID(t.Unit.ID); local row=Row(u)
assert(Same(row,t.Unit),'independent UI coordinates differ from gameplay')
Trace('ui-before-save',row); return 'native UI independently matches production id, type, position and damage'
"#,
        "InGame",
    );
    round_trip(&mut steps, "JAC_ASSASSIN_BLOCKED_RECOVERED");
    check(
        &mut steps,
        r#"
assert(Same(R.State(t.Actor),t.Complete),'completion state changed after load')
local row=Row(Players[t.Actor]:GetUnits():FindID(t.Unit.ID)); assert(Same(row,t.Unit),'gameplay unit changed after load')
Trace('gameplay-after-load',row); return 'completed state and same native unit restored unchanged'
"#,
        "GameCore_Tuner",
    );
    check(
        &mut steps,
        r#"
local row=Row(Players[t.Actor]:GetUnits():FindID(t.Unit.ID)); assert(Same(row,t.Unit),'independent UI unit changed after load')
Trace('ui-after-load',row); return 'independent UI restores the same id, owner, type, coordinates and damage'
"#,
        "InGame",
    );
    advance(&mut steps);
    check(
        &mut steps,
        r#"
assert(Same(R.State(t.Actor),t.Complete),'next turn duplicated completion or extended lock')
local ids=Assassins(t.Actor); local count=0; for id in pairs(ids) do count=count+1; assert(tonumber(id)==t.Unit.ID,'duplicate assassin') end
assert(count==1,'recovered assassin lost'); Trace('next-turn',ids[tostring(t.Unit.ID)])
Game:SetProperty(R.TEST,nil)
local ok,why=R.TestAdvanceDeadlines(t.Actor); assert(not ok and why=='TEST_MODE_OFF','test hook remains active')
return 'next real turn preserves one completion and original lock; test hook disabled'
"#,
        "GameCore_Tuner",
    );
    Ok(
        json!({"name":"Assassin: real blocked spawn, pending save/load, native recovery and independent unit coordinates","judge_profile":"japan_map","steps":steps}),
    )
}

fn main() -> Result<()> {
    let repo = Repository::checked_in();
    let data = build(&repo)?;
    std::fs::write(
        repo.path(OUTPUT),
        serde_json::to_string_pretty(&data)? + "\n",
    )?;
    println!(
        "{}: {} steps",
        OUTPUT,
        data["steps"].as_array().unwrap().len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;
    #[test]
    fn checked_in_scenario_matches_generator() -> Result<()> {
        let repo = Repository::checked_in();
        assert_eq!(build(&repo)?, repo.json(OUTPUT)?);
        Ok(())
    }
    #[test]
    fn all_generated_lua_compiles_in_lua51() -> Result<()> {
        let data = build(&Repository::checked_in())?;
        let lua = Lua::new();
        let mut count = 0;
        for step in data["steps"].as_array().unwrap() {
            if let Some(code) = step["code"]
                .as_str()
                .or_else(|| step["condition"]["code"].as_str())
            {
                lua.load(code).into_function()?;
                count += 1;
            }
        }
        assert!(count >= 16);
        Ok(())
    }
    #[test]
    fn native_footprint_discovery_handles_fallback_and_rejects_unsafe_probes() -> Result<()> {
        let repo = Repository::checked_in();
        let data = build(&repo)?;
        let setup = data["steps"][4]["code"].as_str().unwrap();
        let fixture = r#"
Game.GetLocalPlayer=function() return 0 end
local function Plot(x,y)
  return {GetX=function() return x end,GetY=function() return y end,
    IsWater=function() return false end,IsImpassable=function() return false end}
end
Map.GetPlot=Plot
Map.GetAdjacentPlots=function() local rows={}; for x=1,6 do rows[#rows+1]=Plot(x,0) end; return rows end
for _,row in pairs(GameInfo.Units) do
  if row.Combat>0 then row.FormationClass='FORMATION_CLASS_LAND_COMBAT' end
end
ExposedMembers={JAC_Assassin={TEST='JAC_ASN_TEST_MODE',State=function() return {} end,
  Turn=function() return NOW end,UnitForPlayer=function() return 'UNIT_JAC_JAPAN_SENGOKU_ASSASSIN' end,
  IsAssassin=function(u) return u:GetType()==0 or u:GetType()==5 end}}
local unrelated=GRANT(0,1,1000,0):GetID()
UnitManager.InitUnit=function(owner,kind,x,y) GRANT(owner,GameInfo.Units[kind].Index,x,y) end
NATIVE_CALLS=0
UnitManager.InitUnitValidAdjacentHex=function(owner,kind,x,y,arg)
  assert(owner==0 and kind=='UNIT_JAC_JAPAN_SENGOKU_ASSASSIN' and x==0 and y==0 and arg==1)
  NATIVE_CALLS=NATIVE_CALLS+1
  if PROBE_MODE=='unbounded' then GRANT(owner,0,6+NATIVE_CALLS,0)
  elseif PROBE_MODE=='stacked' then GRANT(owner,0,0,0)
  elseif NATIVE_CALLS==1 then GRANT(owner,PROBE_MODE=='wrong_type' and 4 or 0,7,0)
  elseif PROBE_MODE=='unplaced' then GRANT(owner,0,-9999,-9999) end
end
local kill=UnitManager.Kill
UnitManager.Kill=function(u)
  kill(u)
  if PROBE_MODE=='corrupt_cleanup' then DATA[0].Units[unrelated]=nil end
end
function CheckFootprint()
  local t=Game:GetProperty('JAC_ASN_BLOCKED_TEST')
  assert(NATIVE_CALLS==2 and #t.Plots==8)
  assert(Players[0]:GetUnits():GetCount()==9 and Players[0]:GetUnits():FindID(unrelated))
  for _,u in Players[0]:GetUnits():Members() do assert(u:GetType()~=0 and u:GetType()~=5) end
end
"#;
        for mode in [
            "fallback",
            "unplaced",
            "stacked",
            "unbounded",
            "wrong_type",
            "corrupt_cleanup",
        ] {
            let lua = Lua::new();
            lua.load(repo.read("tools/civ6-lua-tests-rs/fixtures/assassin/stubs.lua")?)
                .exec()?;
            lua.load(format!("PROBE_MODE='{mode}'\n{fixture}")).exec()?;
            let result = lua.load(setup).exec();
            let safe = matches!(mode, "fallback" | "unplaced");
            assert_eq!(result.is_ok(), safe, "{mode}: {result:?}");
            if safe {
                lua.load("CheckFootprint()").exec()?;
            }
        }
        Ok(())
    }
    #[test]
    fn pending_check_rejects_premature_completion_and_leaked_units() -> Result<()> {
        let setup = r#"
local s={Production={City=4,Start=1,Due=2}}
local wrong=false
ExposedMembers={JAC_Assassin={State=function() return s end,Turn=function() return 2 end,IsAssassin=function(u) return u~=nil and not wrong end}}
Game={GetProperty=function(_,k) if k=='JAC_ASN_TICK' then return 2 end return
  {Actor=0,City=4,Start=1,Due=2,Initial={},Before={},AllBefore={},Plots={}} end,GetCurrentGameTurn=function() return 2 end}
local leak=false
Players={[0]={GetUnits=function() return {Members=function() local done=false; return function()
  if leak and not done then done=true; return 1,{GetID=function() return 99 end,GetOwner=function() return 0 end,
    GetType=function() return 116 end,GetX=function() return -9999 end,GetY=function() return -9999 end,GetDamage=function() return 0 end} end
end end} end}}
function Complete() s.Production=nil; s.LastCompletion=2; s.ProductionUnlock=12 end
function Leak() leak=true end
function LeakWrong() leak=true; wrong=true end
"#;
        for mutation in ["", "Complete()", "Leak()", "LeakWrong()"] {
            let lua = Lua::new();
            lua.load(setup).exec()?;
            lua.load(mutation).exec()?;
            let result = lua.load(format!("{PREFIX} Pending()")).exec();
            assert_eq!(result.is_ok(), mutation.is_empty(), "{mutation}");
        }
        Ok(())
    }
}
