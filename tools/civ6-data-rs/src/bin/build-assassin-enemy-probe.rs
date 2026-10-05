use anyhow::{bail, Context, Result};
use civ6_data_rs::{assassin, Repository};
use serde_json::{json, Value};

// This qualifies a fixture. A successful probe run is NOT acceptance of
// training retry, save/load or recovery. No turn is advanced here.
const PROBE: &str = r#"
local R=ExposedMembers.JAC_Assassin; assert(R,'assassin gameplay absent')
local actor=Game.GetLocalPlayer(); assert(Players[actor]:IsHuman(),'human required')
local city=Players[actor]:GetCities():GetCapitalCity(); assert(city,'capital missing')
local cityID,x,y=city:GetID(),city:GetX(),city:GetY()
local initial=R.State(actor); assert(not initial.Production and not initial.Mission,'fresh idle game required')
local function Same(a,b)
  if type(a)~=type(b) then return false end
  if type(a)~='table' then return a==b end
  for k,v in pairs(a) do if not Same(v,b[k]) then return false end end
  for k,v in pairs(b) do if not Same(v,a[k]) then return false end end
  return true
end
local function Row(u)
  return {ID=u:GetID(),Owner=u:GetOwner(),Type=u:GetType(),X=u:GetX(),Y=u:GetY(),Damage=u:GetDamage()}
end
local function Rows(owner)
  local rows={}; for _,u in Players[owner]:GetUnits():Members() do rows[tostring(u:GetID())]=Row(u) end
  return rows
end
local function Cities(owner)
  local rows={}; for _,c in Players[owner]:GetCities():Members() do
    rows[tostring(c:GetID())]={ID=c:GetID(),Owner=c:GetOwner(),X=c:GetX(),Y=c:GetY()}
  end; return rows
end
local before=Rows(actor); local cities=Cities(actor)
local enemy
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
  local other=p:GetID(); local capital=p:GetCities():GetCapitalCity()
  if other~=actor and not p:IsHuman() and capital and
    Map.GetPlotDistance(x,y,capital:GetX(),capital:GetY())>=12 then enemy=other; break end
end
assert(enemy,'distant alive AI major absent')
local enemyBefore=Rows(enemy); local enemyCities=Cities(enemy)
local kind=R.UnitForPlayer(enemy); local definition=GameInfo.Units[kind]
assert(definition and (definition.CanCapture==false or definition.CanCapture==0),
  'fixture unit must not be able to capture the city')
assert(WarTypes and WarTypes.FORMAL_WAR~=nil,'formal war enum absent')
local plots={Map.GetPlot(x,y)}
for _,p in ipairs(Map.GetAdjacentPlots(x,y)) do plots[#plots+1]=p end
assert(#plots==7,'complete radius-one footprint required')
local terrain={}
for _,p in ipairs(plots) do
  assert(p and not p:IsWater() and not p:IsImpassable(),'probe requires passable land footprint')
  terrain[#terrain+1]={X=p:GetX(),Y=p:GetY(),Terrain=p:GetTerrainType(),Feature=p:GetFeatureType()}
end
local function CityAndTerrain()
  assert(Same(Cities(actor),cities) and Same(Cities(enemy),enemyCities),'native city inventory changed')
  for _,p in ipairs(terrain) do local plot=Map.GetPlot(p.X,p.Y)
    assert(plot:GetTerrainType()==p.Terrain and plot:GetFeatureType()==p.Feature,'native terrain changed')
  end
end
local fixtures,probes={},{}
local function NewUnits(owner,old,record)
  local rows={}
  for _,u in Players[owner]:GetUnits():Members() do if not old[tostring(u:GetID())] then
    local row=Row(u); row.Marker='JAC_ASN_OCCUPANCY_'..owner..'_'..row.ID
    u:SetProperty('JAC_ASN_OCCUPANCY_MARKER',row.Marker)
    record[#record+1]=row; rows[#rows+1]=row
  end end
  return rows
end
local function Remove(record)
  local failures={}
  for _,row in ipairs(record) do
    local ok,why=pcall(function()
      local function Matches(u)
        return u and u:GetID()==row.ID and u:GetOwner()==row.Owner and u:GetType()==row.Type and
          u:GetProperty('JAC_ASN_OCCUPANCY_MARKER')==row.Marker
      end
      local units=Players[row.Owner]:GetUnits(); local u=units:FindID(row.ID)
      if not Matches(u) then
        u=nil
        for _,candidate in units:Members() do if Matches(candidate) then
          assert(not u,'probe cleanup has ambiguous identity'); u=candidate
        end end
      end
      assert(Matches(u),'probe cleanup identity mismatch')
      UnitManager.Kill(u)
    end)
    if not ok then failures[#failures+1]=tostring(why) end
  end
  return #failures==0,table.concat(failures,'; ')
end
local qualified=false
Game:SetProperty(R.TEST,1)
local ok,why=xpcall(function()
  if not Players[enemy]:GetDiplomacy():IsAtWarWith(actor) then
    Players[enemy]:GetDiplomacy():DeclareWarOn(actor,WarTypes.FORMAL_WAR,true)
  end
  assert(Players[enemy]:GetDiplomacy():IsAtWarWith(actor),'native war not established')
  for _,p in ipairs(plots) do
    local old=Rows(enemy)
    UnitManager.InitUnit(enemy,kind,p:GetX(),p:GetY())
    local created=NewUnits(enemy,old,fixtures)
    assert(#created==1 and created[1].Owner==enemy and created[1].Type==definition.Index and
      created[1].X==p:GetX() and created[1].Y==p:GetY(),'hostile fixture not placed exactly once')
    assert(Same(Rows(actor),before),'fixture changed an existing actor unit')
    CityAndTerrain()
    print('[JAC_ASN_OCCUPANCY] enemy id='..created[1].ID..' owner='..enemy..
      ' type='..created[1].Type..' xy='..created[1].X..','..created[1].Y)
  end
  local old=Rows(actor); local want=R.UnitForPlayer(actor)
  UnitManager.InitUnitValidAdjacentHex(actor,want,x,y,1)
  local created=NewUnits(actor,old,probes)
  assert(#created<=1,'native probe created multiple units')
  for id,row in pairs(before) do assert(Same(Rows(actor)[id],row),'native probe changed an existing unit') end
  CityAndTerrain()
  if #created==0 then qualified=true else
    local row=created[1]
    assert(row.Owner==actor and row.Type==GameInfo.Units[want].Index,'native probe returned another kind')
    qualified=row.X<0 or row.Y<0
    print('[JAC_ASN_OCCUPANCY] native id='..row.ID..' owner='..row.Owner..' type='..row.Type..
      ' xy='..row.X..','..row.Y..' damage='..row.Damage)
  end
end,function(err) return tostring(err) end)
-- Remove only identities created by this probe. Even a failed qualification
-- must never remove a pre-existing unit via a reused FindID reference.
local probesClean,probeWhy=Remove(probes)
local fixturesClean,fixtureWhy=Remove(fixtures)
Game:SetProperty(R.TEST,nil)
assert(probesClean and fixturesClean,'probe cleanup failed: '..probeWhy..'; '..fixtureWhy)
assert(Same(Rows(actor),before) and Same(Rows(enemy),enemyBefore),'probe left changed unit inventories')
CityAndTerrain()
assert(Same(R.State(actor),initial),'probe changed native training state')
assert(ok,'probe rejected: '..tostring(why))
print('[JAC_ASN_OCCUPANCY] qualified='..tostring(qualified)..' city='..cityID..' xy='..x..','..y..
  ' actor='..actor..' enemy='..enemy..' cities_and_units_preserved=true terrain_preserved=true'..
  ' war_restoration_attempted=false recovery_acceptance=false')
return 'qualification='..tostring(qualified)..'; city and existing units preserved; training/save/recovery not tested'
"#;

fn build(repo: &Repository) -> Result<Value> {
    let base = repo.json("tools/civ6-autotest/scenarios/assassin-leader-save-load.json")?;
    let mut steps = base["steps"].as_array().context("base steps absent")?[..3].to_vec();
    for id in [
        "5c1e7a3d-9b2f-4e61-8d47-2a6f0c9e1b85",
        "61320778-2e75-4c11-9009-94101b519aba",
        "9b0b9a39-bd72-4ff1-9062-05eeac09b693",
        "d8532710-f853-4c53-8853-271000000001",
        "f6398d3b-707c-4d0a-8b19-e68333efa200",
    ] {
        steps[0]["disable_ids"]
            .as_array_mut()
            .unwrap()
            .push(json!(id));
    }
    steps.push(assassin::active());
    steps.push(assassin::ui(PROBE, "GameCore_Tuner"));
    steps.push(json!({"action":"exit_to_menu","timeout":60}));
    Ok(
        json!({"name":"Assassin: hostile occupancy qualification only; no recovery acceptance",
        "judge_profile":"japan_map","steps":steps}),
    )
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 {
        bail!("provide the diagnostic JSON output path (outside shipped scenarios)");
    }
    let data = build(&Repository::checked_in())?;
    std::fs::write(&args[1], serde_json::to_string_pretty(&data)? + "\n")?;
    println!("{}: qualification-only, no recovery acceptance", args[1]);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;

    #[test]
    fn diagnostic_uses_native_calls_without_terrain_or_training_replacement() -> Result<()> {
        let data = build(&Repository::checked_in())?;
        assert_eq!(data["steps"].as_array().unwrap().len(), 6);
        let lua = Lua::new();
        lua.load(PROBE).into_function()?;
        assert!(PROBE.contains("UnitManager.InitUnitValidAdjacentHex(actor,want,x,y,1)"));
        for forbidden in [
            "TerrainBuilder.",
            "R.Tick(",
            "R.StartProduction(",
            "TestAdvanceDeadlines",
        ] {
            assert!(!PROBE.contains(forbidden));
        }
        Ok(())
    }

    #[test]
    fn qualification_distinguishes_native_placement_and_preserves_existing_identities() -> Result<()>
    {
        let repo = Repository::checked_in();
        for mode in [
            "blocked",
            "unplaced",
            "placed",
            "wrong_type",
            "alias",
            "corrupt_marker",
        ] {
            let lua = Lua::new();
            lua.load(repo.read("tools/civ6-lua-tests-rs/fixtures/assassin/stubs.lua")?)
                .exec()?;
            lua.load(format!("MODE='{mode}'")).exec()?;
            lua.load(r#"
Game.GetLocalPlayer=function() return 0 end
for _,c in pairs(DATA[1].Cities) do c.X=30 end
for _,d in pairs(GameInfo.Units) do d.CanCapture=0 end
local function Plot(x,y) return {GetX=function() return x end,GetY=function() return y end,
  IsWater=function() return false end,IsImpassable=function() return false end,
  GetTerrainType=function() return 1 end,GetFeatureType=function() return -1 end} end
Map.GetPlot=Plot
Map.GetAdjacentPlots=function() local rows={}; for x=1,6 do rows[#rows+1]=Plot(x,0) end; return rows end
WarTypes={FORMAL_WAR=1}
Players[1].GetDiplomacy=function() return {DeclareWarOn=function() DATA[1].Wars[0]=true end,
  IsAtWarWith=function() return DATA[1].Wars[0]==true end} end
ExposedMembers={JAC_Assassin={TEST='JAC_ASN_TEST_MODE',State=function() return {} end,
  UnitForPlayer=function() return 'UNIT_JAC_JAPAN_SENGOKU_ASSASSIN' end}}
OLD_ACTOR=GRANT(0,4,0,0):GetID(); OLD_ENEMY=GRANT(1,4,30,0):GetID()
local originalMake=makeUnit
if MODE=='corrupt_marker' then makeUnit=function(d)
  local u=originalMake(d)
  if d.Owner==0 and d.ID~=OLD_ACTOR then
    u.GetProperty=function(_,k) if k=='JAC_ASN_OCCUPANCY_MARKER' then return 'changed' end return d.Properties[k] end
  end
  return u
end end
UnitManager.InitUnit=function(owner,kind,x,y) GRANT(owner,GameInfo.Units[kind].Index,x,y) end
UnitManager.InitUnitValidAdjacentHex=function(owner,kind,x,y,radius)
  assert(owner==0 and kind=='UNIT_JAC_JAPAN_SENGOKU_ASSASSIN' and x==0 and y==0 and radius==1)
  if MODE~='blocked' then GRANT(owner,MODE=='wrong_type' and 4 or 0,
    MODE=='unplaced' and -9999 or 0,MODE=='unplaced' and -9999 or 0) end
end
local collection=Players[0].GetUnits
if MODE=='alias' then Players[0].GetUnits=function()
  local c=collection(); c.FindID=function() return makeUnit(DATA[0].Units[OLD_ACTOR]) end; return c
end end
function CheckOriginals()
  assert(DATA[0].Units[OLD_ACTOR] and DATA[1].Units[OLD_ENEMY])
  assert(Players[0]:GetCities():GetCount()==2 and Players[1]:GetCities():GetCount()==2)
  assert(Game:GetProperty('JAC_ASN_TEST_MODE')==nil)
end
function CheckInventories()
  assert(Players[0]:GetUnits():GetCount()==1 and Players[1]:GetUnits():GetCount()==1)
end
function CheckUnauthenticatedProbe()
  assert(Players[0]:GetUnits():GetCount()==2 and Players[1]:GetUnits():GetCount()==1)
  assert(#KILLS==7)
end
"#).exec()?;
            let outcome = lua.load(PROBE).eval::<String>();
            if matches!(mode, "wrong_type" | "corrupt_marker") {
                assert!(outcome.is_err(), "{mode}");
            } else {
                let outcome = outcome?;
                assert!(outcome.contains(if matches!(mode, "placed" | "alias") {
                    "qualification=false"
                } else {
                    "qualification=true"
                }));
            }
            lua.load("CheckOriginals()").exec()?;
            if mode == "corrupt_marker" {
                lua.load("CheckUnauthenticatedProbe()").exec()?;
            } else {
                lua.load("CheckInventories()").exec()?;
            }
        }
        Ok(())
    }
}
