use anyhow::Result;
use civ6_data_rs::{assassin, Repository};
use serde_json::{json, Value};

#[path = "../assassin_blocked_origin.rs"]
mod origin;

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
  local plot=Map.GetPlot and Map.GetPlot(row.X,row.Y)
  if plot and Units and Units.GetUnitsInPlot then
    local present=false; local count=0
    for _,u in ipairs(Units.GetUnitsInPlot(plot)) do
      count=count+1; if u:GetOwner()==row.Owner and u:GetID()==row.ID then present=true end
    end
    print('[JAC_ASN_BLOCKED] plot-membership label='..label..' present='..tostring(present)..' count='..count)
  end
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
local function Placed(row)
  local plot=Map.GetPlot(row.X,row.Y)
  assert(plot and Units and Units.GetUnitsInPlot,'native plot membership unavailable')
  local count=0
  for _,u in ipairs(Units.GetUnitsInPlot(plot)) do
    if u:GetOwner()==row.Owner and u:GetID()==row.ID then
      count=count+1; assert(Same(Row(u),row),'plot member differs from saved unit')
    end
  end
  assert(count==1,'native unit is not present exactly once in its plot')
end
local function AssetFree(plot)
  assert(not plot:IsNaturalWonder(),'fixture would modify a natural wonder')
  assert(plot:GetResourceType()==-1 and plot:GetImprovementType()==-1 and
    plot:GetDistrictType()==-1 and plot:GetRouteType()==-1,'fixture would modify a plot with assets')
end
local function NationwideLock()
  local before=R.State(t.Actor); local count=0
  assert(before.ProductionUnlock==before.LastCompletion+10,'standard-speed lock must last ten turns')
  for _,city in Players[t.Actor]:GetCities():Members() do
    count=count+1
    local ok,why=R.StartProduction(t.Actor,city:GetID())
    assert(not ok and why=='PRODUCTION_LOCK','another city bypassed the nationwide lock')
    assert(Same(R.State(t.Actor),before),'rejected training changed completion state')
  end
  assert(count>=2,'nationwide lock requires another city')
end
local function IntactCity()
  local city=Players[t.Actor]:GetCities():FindID(t.City)
  assert(city and city:GetX()==t.X and city:GetY()==t.Y,'training city lost or replaced')
  local plot=Map.GetPlot(t.X,t.Y)
  assert(plot and plot:GetTerrainType()==t.CenterTerrain and plot:GetFeatureType()==t.CenterFeature,
    'training city center terrain or feature changed')
end
local function Blockers()
  IntactCity()
  for _,p in ipairs(t.Plots) do
    local plot=Map.GetPlot(p.X,p.Y); assert(plot,'fixture plot missing')
    AssetFree(plot)
    assert(plot:IsImpassable() and plot:GetTerrainType()==p.Barrier,'fixture terrain barrier changed')
  end
end
local function RestoredTerrain()
  IntactCity()
  for _,p in ipairs(t.Plots) do
    local plot=Map.GetPlot(p.X,p.Y)
    assert(plot and plot:GetTerrainType()==p.Terrain and plot:GetFeatureType()==p.Feature,'restored terrain or feature differs')
    AssetFree(plot)
  end
end
local function RestoreTerrain()
  for _,p in ipairs(t.Plots) do
    local plot=Map.GetPlot(p.X,p.Y)
    TerrainBuilder.SetTerrainType(plot,p.Terrain)
    TerrainBuilder.SetFeatureType(plot,p.Feature)
    assert(plot:GetTerrainType()==p.Terrain and plot:GetFeatureType()==p.Feature,'fixture terrain did not restore')
  end
  RestoredTerrain()
end
local function Pending()
  local s=R.State(t.Actor)
  local city=Players[t.Actor]:GetCities():FindID(t.City)
  assert(city and city:GetX()==t.X and city:GetY()==t.Y,'blocked training city lost or replaced')
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
fn build_base(repo: &Repository) -> Result<Value> {
    let base = repo.json("tools/civ6-autotest/scenarios/assassin-leader-save-load.json")?;
    let mut steps = base["steps"].as_array().unwrap()[..3].to_vec();
    let disabled = steps[0]["disable_ids"].as_array_mut().unwrap();
    for id in [
        "5c1e7a3d-9b2f-4e61-8d47-2a6f0c9e1b85",
        "61320778-2e75-4c11-9009-94101b519aba",
        "9b0b9a39-bd72-4ff1-9062-05eeac09b693",
        "d8532710-f853-4c53-8853-271000000001",
        "f6398d3b-707c-4d0a-8b19-e68333efa200",
    ] {
        disabled.push(json!(id));
    }
    steps.push(assassin::active());
    check(
        &mut steps,
        r#"
Game:SetProperty(R.TEST,1)
local owner=Game.GetLocalPlayer(); assert(Players[owner]:IsHuman(),'human required')
local city=Players[owner]:GetCities():GetCapitalCity(); assert(city,'capital missing')
local cityID,cityX,cityY=city:GetID(),city:GetX(),city:GetY()
local function CheckCity(label)
  local found=Players[owner]:GetCities():FindID(cityID)
  print('[JAC_ASN_BLOCKED] city-check label='..label..' expected='..cityID..' xy='..cityX..','..cityY..
    ' found='..tostring(found~=nil)..' count='..Players[owner]:GetCities():GetCount())
  for _,c in Players[owner]:GetCities():Members() do
    print('[JAC_ASN_BLOCKED] city-member id='..c:GetID()..' xy='..c:GetX()..','..c:GetY()..' owner='..c:GetOwner())
  end
  assert(found and found:GetX()==cityX and found:GetY()==cityY,'fixture lost training city at '..label)
end
CheckCity('before-fixture')
assert(next(Assassins(owner))==nil,'fresh game already contains assassins')
local center=Map.GetPlot(cityX,cityY); local plots={}
for _,p in ipairs(Map.GetAdjacentPlots(cityX,cityY)) do plots[#plots+1]=p end
assert(#plots==6,'complete adjacent ring required')
local rows={}; local occupied={}
local function SafePlot(p)
  assert(not p:IsCity(),'fixture would modify another city')
  assert(not p:IsWater(),'fixture would modify a water plot')
  assert(p:GetOwner()<0 or p:GetOwner()==owner,'fixture would modify foreign territory')
  AssetFree(p)
  -- Civ6 has 64 player slots, including the free cities and barbarians.
  for id=0,63 do local player=Players[id]
    local units=player and player:GetUnits()
    if units then for _,u in units:Members() do
      assert(u:GetX()~=p:GetX() or u:GetY()~=p:GetY(),'fixture would modify an occupied plot')
    end end
  end
end
-- Reject unsafe initial conditions before altering even the first plot.
for _,p in ipairs(plots) do if not p:IsWater() then SafePlot(p) end end
t={Actor=owner,City=cityID,X=cityX,Y=cityY,Plots=rows,
  CenterTerrain=center:GetTerrainType(),CenterFeature=center:GetFeatureType(),
  Start=R.Turn(),Initial=R.State(owner),Before=Assassins(owner),AllBefore=UnitIDs(owner)}
local function Occupy(p)
  SafePlot(p)
  local row={X=p:GetX(),Y=p:GetY(),Terrain=p:GetTerrainType(),Feature=p:GetFeatureType(),
    Barrier=GameInfo.Terrains.TERRAIN_GRASS_MOUNTAIN.Index}
  -- Record before either setter so partial mutation can also be restored.
  rows[#rows+1]=row; occupied[row.X..','..row.Y]=row
  TerrainBuilder.SetFeatureType(p,-1); CheckCity('after-clear-feature-'..row.X..','..row.Y)
  TerrainBuilder.SetTerrainType(p,row.Barrier); CheckCity('after-terrain-'..row.X..','..row.Y)
  assert(p:IsImpassable(),'terrain setter did not create an impassable plot')
  CheckCity('after-barrier')
  print('[JAC_ASN_BLOCKED] terrain-blocker xy='..row.X..','..row.Y..' original='..row.Terrain..' barrier='..row.Barrier)
end
-- Treat all fixture mutation and native discovery as one transaction.
-- Every failure restores earlier barriers, including unsafe later candidates,
-- invalid native placement and exhaustion of the bounded candidate search.
local prepared,why=xpcall(function()
for _,p in ipairs(plots) do if not p:IsWater() then Occupy(p) end end
-- The native API can search past the first ring. Discover its real placement
-- candidates instead of treating the fifth argument as a maximum radius.
local blocked=false; local kind=R.UnitForPlayer(owner)
for attempt=1,128 do
  local before=UnitIDs(owner); local probe; local count=0
  UnitManager.InitUnitValidAdjacentHex(owner,kind,cityX,cityY,1)
  for _,u in Players[owner]:GetUnits():Members() do if not before[tostring(u:GetID())] then
    count=count+1; probe=u
  end end
  assert(count<=1,'native placement probe created multiple units')
  if not probe then
    assert(Same(UnitIDs(owner),before),'failed native probe changed existing inventory')
    blocked=true; print('[JAC_ASN_BLOCKED] native-blocked attempts='..attempt..' plots='..#rows); break
  end
  local row=Row(probe); Trace('native-probe',row)
  UnitManager.Kill(probe)
  assert(Same(UnitIDs(owner),before),'native probe cleanup changed existing inventory')
  assert(row.Type==GameInfo.Units[kind].Index,'native placement probe has wrong type')
  if row.X<0 or row.Y<0 then
    blocked=true; print('[JAC_ASN_BLOCKED] native-blocked-unplaced attempts='..attempt..' plots='..#rows); break
  end
  if row.X==cityX and row.Y==cityY then
    CheckCity('native-city-center')
    RestoreTerrain()
    assert(Same(UnitIDs(owner),t.AllBefore),'city-center probe changed existing inventory')
    print('[JAC_ASN_BLOCKED] intact-city-fallback terrain-restored=true original-units-preserved=true')
    error('native placement uses the intact training city; no blocked condition was created')
  end
  local existing=occupied[row.X..','..row.Y]
  assert(not existing,'native placement selected an impassable terrain barrier')
  local plot=Map.GetPlot(row.X,row.Y)
  assert(plot,'native probe selected an invalid plot')
  if plot:IsWater() then
    RestoreTerrain()
    assert(Same(UnitIDs(owner),t.AllBefore),'water probe changed existing inventory')
    error('native placement uses unchanged water; no blocked condition was created')
  end
  assert(not plot:IsImpassable(),'native probe selected an invalid land plot')
  Occupy(plot)
end
assert(blocked,'native placement did not become blocked within 128 probes')
assert(Same(UnitIDs(owner),t.AllBefore),'fixture changed existing unit inventory')
assert(next(t.Before)==nil,'fresh game already contains assassins')
Game:SetProperty('JAC_ASN_BLOCKED_TEST',t); Blockers()
end,function(err) return tostring(err) end)
if not prepared then
  RestoreTerrain()
  assert(Same(UnitIDs(owner),t.AllBefore),'failed fixture rollback changed existing inventory')
  error(why)
end
return 'real impassable terrain blocks the native API after discovering its actual placement candidates'
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
RestoreTerrain()
t.Released=R.Turn(); t.AllReleased=UnitIDs(t.Actor); Game:SetProperty('JAC_ASN_BLOCKED_TEST',t)
return 'second blocked real tick verified; original terrain restored without touching training'
"#,
        "GameCore_Tuner",
    );
    advance(&mut steps);
    check(
        &mut steps,
        r#"
local s=R.State(t.Actor); local count=0; local row
RestoredTerrain()
for _,u in Players[t.Actor]:GetUnits():Members() do if R.IsAssassin(u) then
  count=count+1; row=Row(u); assert(not t.Before[tostring(row.ID)],'reused production id')
end end
assert(count==1 and row.ID==s.LastProducedUnit and row.X>=0 and row.Y>=0,'recovery did not create exactly one placed assassin')
assert(row.Type==GameInfo.Units[R.UnitForPlayer(t.Actor)].Index,'recovered unit has the wrong civilization-specific type')
local newCount=0; local all=UnitIDs(t.Actor)
for id in pairs(all) do if not t.AllReleased[id] then newCount=newCount+1; assert(tonumber(id)==row.ID,'unexpected new unit during recovery') end end
for id in pairs(t.AllReleased) do assert(all[id],'unrelated unit disappeared during recovery') end
assert(newCount==1,'native recovery did not create exactly one new unit of any type')
assert(Map.GetPlot(row.X,row.Y),'recovered unit coordinates are outside the map')
print('[JAC_ASN_BLOCKED] recovery-distance='..Map.GetPlotDistance(t.X,t.Y,row.X,row.Y))
assert(s.Production==nil and s.LastCompletion==R.Turn() and R.Turn()==t.Released+1,'wrong actual completion turn')
assert(s.ProductionUnlock==s.LastCompletion+R.Scale(R.PRODUCTION_LOCK),'lock did not start at actual completion')
t.Complete=s; t.Unit=row; Trace('gameplay-before-save',row); Game:SetProperty('JAC_ASN_BLOCKED_TEST',t)
Placed(row); NationwideLock()
return 'one native placed assassin; full lock starts only at actual recovery'
"#,
        "GameCore_Tuner",
    );
    check(
        &mut steps,
        r#"
local u=Players[t.Actor]:GetUnits():FindID(t.Unit.ID); local row=Row(u)
RestoredTerrain()
assert(Same(row,t.Unit),'independent UI coordinates differ from gameplay')
Placed(row)
Trace('ui-before-save',row); return 'native UI independently matches production id, type, position and damage'
"#,
        "InGame",
    );
    round_trip(&mut steps, "JAC_ASSASSIN_BLOCKED_RECOVERED");
    check(
        &mut steps,
        r#"
assert(Same(R.State(t.Actor),t.Complete),'completion state changed after load')
RestoredTerrain()
local row=Row(Players[t.Actor]:GetUnits():FindID(t.Unit.ID)); assert(Same(row,t.Unit),'gameplay unit changed after load')
Placed(row); NationwideLock()
Trace('gameplay-after-load',row); return 'completed state and same native unit restored unchanged'
"#,
        "GameCore_Tuner",
    );
    check(
        &mut steps,
        r#"
local row=Row(Players[t.Actor]:GetUnits():FindID(t.Unit.ID)); assert(Same(row,t.Unit),'independent UI unit changed after load')
Placed(row)
RestoredTerrain()
Trace('ui-after-load',row); return 'independent UI restores the same id, owner, type, coordinates and damage'
"#,
        "InGame",
    );
    advance(&mut steps);
    check(
        &mut steps,
        r#"
assert(Same(R.State(t.Actor),t.Complete),'next turn duplicated completion or extended lock')
RestoredTerrain()
local ids=Assassins(t.Actor); local count=0; for id in pairs(ids) do count=count+1; assert(tonumber(id)==t.Unit.ID,'duplicate assassin') end
assert(count==1,'recovered assassin lost'); Trace('next-turn',ids[tostring(t.Unit.ID)])
Placed(ids[tostring(t.Unit.ID)]); NationwideLock()
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

fn build(repo: &Repository) -> Result<Value> {
    origin::transform(build_base(repo)?)
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
    fn remote_fixture_preserves_city_and_rejects_assets_before_mutation() -> Result<()> {
        let data = build(&Repository::checked_in())?;
        let setup = data["steps"][4]["code"].as_str().unwrap();
        for mode in [
            "safe",
            "resource",
            "improvement",
            "district",
            "route",
            "wonder",
            "water",
            "foreign",
            "occupied",
            "one_city",
        ] {
            let lua = Lua::new();
            lua.load(format!(r#"
local mode='{mode}'; local props={{}}; local terrain={{}}; local feature={{}}
MUTATIONS=0; PROBES=0
Game={{GetLocalPlayer=function() return 0 end,GetCurrentGameTurn=function() return 1 end,
  GetProperty=function(_,k) return props[k] end,SetProperty=function(_,k,v) props[k]=v end}}
local plots={{}}
for x=0,36 do local px=x; plots[x]={{GetX=function() return px end,GetY=function() return 0 end,
  IsCity=function() return px==0 end,IsWater=function() return mode=='water' and px==31 end,
  IsImpassable=function() return terrain[px]==9 end,GetOwner=function() return mode=='foreign' and px==31 and 1 or -1 end,
  GetTerrainType=function() return terrain[px] or 1 end,GetFeatureType=function() return feature[px] or -1 end,
  IsNaturalWonder=function() return mode=='wonder' and px==31 end,
  GetResourceType=function() return mode=='resource' and px==31 and 2 or -1 end,
  GetImprovementType=function() return mode=='improvement' and px==31 and 2 or -1 end,
  GetDistrictType=function() return mode=='district' and px==31 and 2 or -1 end,
  GetRouteType=function() return mode=='route' and px==31 and 2 or -1 end}} end
local city={{GetID=function() return 10 end,GetOwner=function() return 0 end,GetX=function() return 0 end,GetY=function() return 0 end}}
local cities={{GetCapitalCity=function() return city end,FindID=function(_,id) if id==10 then return city end end,
  GetCount=function() return mode=='one_city' and 1 or 2 end,
  Members=function() local count=0; return function() count=count+1; if count<=2 then return count,city end end end}}
local blocker={{GetX=function() return 31 end,GetY=function() return 0 end,GetID=function() return 7 end}}
local units={{Members=function() local done=false; return function()
  if mode=='occupied' and not done then done=true; return 7,blocker end end end}}
Players={{[0]={{IsHuman=function() return true end,GetCities=function() return cities end,GetUnits=function() return units end}}}}
Map={{GetPlotCount=function() return 1 end,GetPlotByIndex=function() return plots[30] end,
  GetPlot=function(x) return plots[x] end,GetPlotDistance=function() return 5 end,
  GetAdjacentPlots=function() local rows={{}}; for x=31,36 do rows[#rows+1]=plots[x] end; return rows end}}
GameInfo={{Terrains={{TERRAIN_GRASS_MOUNTAIN={{Index=9}}}}}}
TerrainBuilder={{SetTerrainType=function(p,v) MUTATIONS=MUTATIONS+1; terrain[p:GetX()]=v end,
  SetFeatureType=function(p,v) MUTATIONS=MUTATIONS+1; feature[p:GetX()]=v end}}
UnitManager={{InitUnitValidAdjacentHex=function() PROBES=PROBES+1 end}}
local ox,oy
ExposedMembers={{JAC_Assassin={{TEST='TEST',State=function() return {{}} end,Turn=function() return 1 end,
  IsAssassin=function() return false end,UnitForPlayer=function() return 'ASSASSIN' end,
  TestTrainingOrigin=function(_,_,x,y) ox,oy=x,y; return true end,
  TrainingOrigin=function() return ox,oy end,TestAbortOriginFixture=function() return true end}}}}
function Verify(safe)
  assert(plots[0]:IsCity() and plots[0]:GetTerrainType()==1 and plots[0]:GetFeatureType()==-1)
  if safe then assert(MUTATIONS==14 and PROBES==1 and ox==30 and oy==0)
  else assert(MUTATIONS==0 and PROBES==0) end
end
"#)).exec()?;
            let result = lua.load(setup).exec();
            assert_eq!(result.is_ok(), mode == "safe", "{mode}: {result:?}");
            lua.load(format!("Verify({})", mode == "safe")).exec()?;
        }
        Ok(())
    }
    #[test]
    fn native_terrain_discovery_handles_fallback_and_rejects_unsafe_probes() -> Result<()> {
        let repo = Repository::checked_in();
        let data = build_base(&repo)?;
        let setup = data["steps"][4]["code"].as_str().unwrap();
        let fixture = r#"
Game.GetLocalPlayer=function() return 0 end
local terrain={['2,0']=4}; local features={['0,0']=3}
if PROBE_MODE=='natural_wonder' then features['2,0']=99 end
GameInfo.Terrains={TERRAIN_GRASS_MOUNTAIN={Index=9}}
local function Plot(x,y)
  local key=x..','..y
  return {GetX=function() return x end,GetY=function() return y end,
    GetOwner=function() return PROBE_MODE=='foreign_territory' and x==2 and 1 or 0 end,
    GetTerrainType=function() return terrain[key] or 1 end,
    GetFeatureType=function() return features[key] or -1 end,
    GetResourceType=function() return PROBE_MODE=='resource' and x==2 and 1 or -1 end,
    GetImprovementType=function() return PROBE_MODE=='improvement' and x==2 and 1 or -1 end,
    GetDistrictType=function() return PROBE_MODE=='district' and x==2 and 1 or -1 end,
    GetRouteType=function() return PROBE_MODE=='route' and x==2 and 1 or -1 end,
    IsNaturalWonder=function() return (PROBE_MODE=='natural_wonder' and x==2) or (PROBE_MODE=='unsafe_candidate' and x==7) end,
    IsCity=function() return (x==0 and y==0) or (PROBE_MODE=='other_city' and x==1) end,
    IsWater=function() return (PROBE_MODE=='water' or PROBE_MODE=='water_candidate') and x==2 end,
    IsImpassable=function() return terrain[key]==9 end}
end
TerrainBuilder={SetTerrainType=function(p,t)
  terrain[p:GetX()..','..p:GetY()]=t
  if p:IsCity() then DATA[0].Cities[10]=nil end
end,SetFeatureType=function(p,f)
  features[p:GetX()..','..p:GetY()]=f
  if PROBE_MODE=='lost_city' then DATA[0].Cities[10]=nil end
end}
Map.GetPlot=Plot
Map.GetAdjacentPlots=function() local rows={}; for x=1,6 do rows[#rows+1]=Plot(x,0) end; return rows end
for _,row in pairs(GameInfo.Units) do
  if row.Combat>0 then row.FormationClass='FORMATION_CLASS_LAND_COMBAT' end
end
ExposedMembers={JAC_Assassin={TEST='JAC_ASN_TEST_MODE',State=function() return {} end,
  Turn=function() return NOW end,UnitForPlayer=function() return 'UNIT_JAC_JAPAN_SENGOKU_ASSASSIN' end,
  IsAssassin=function(u) return u:GetType()==0 or u:GetType()==5 end}}
local unrelated=GRANT(0,1,1000,0):GetID()
local garrison=GRANT(0,4,0,0):GetID()
local otherCityUnit
if PROBE_MODE=='other_city' then otherCityUnit=GRANT(0,4,1,0):GetID() end
local occupiedOwner,occupiedUnit
if PROBE_MODE=='occupied' or PROBE_MODE=='foreign_occupied' then
  occupiedOwner=PROBE_MODE=='occupied' and 0 or 1
  occupiedUnit=GRANT(occupiedOwner,4,2,0):GetID()
end
Players[60]={GetUnits=function() return nil end}
local initialUnits={}; for id,data in pairs(DATA) do initialUnits[id]=clone(data.Units) end
INIT_CALLS=0
UnitManager.InitUnit=function(owner,kind,x,y) INIT_CALLS=INIT_CALLS+1; GRANT(owner,GameInfo.Units[kind].Index,x,y) end
NATIVE_CALLS=0
UnitManager.InitUnitValidAdjacentHex=function(owner,kind,x,y,arg)
  assert(owner==0 and kind=='UNIT_JAC_JAPAN_SENGOKU_ASSASSIN' and x==0 and y==0 and arg==1)
  NATIVE_CALLS=NATIVE_CALLS+1
  if PROBE_MODE=='unbounded' then GRANT(owner,0,6+NATIVE_CALLS,0)
  elseif PROBE_MODE=='stacked' then GRANT(owner,0,0,0)
  elseif PROBE_MODE=='water_candidate' then GRANT(owner,0,2,0)
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
  assert(NATIVE_CALLS==2 and #t.Plots==(PROBE_MODE=='water' and 6 or 7))
  assert(Players[0]:GetUnits():GetCount()==2 and Players[0]:GetUnits():FindID(unrelated))
  assert(Players[0]:GetUnits():FindID(garrison))
  for _,p in ipairs(t.Plots) do assert(Map.GetPlot(p.X,p.Y):IsImpassable()) end
  if PROBE_MODE=='water' then
    assert(Map.GetPlot(2,0):GetTerrainType()==4 and Map.GetPlot(2,0):GetFeatureType()==-1)
  end
  for _,u in Players[0]:GetUnits():Members() do assert(u:GetType()~=0 and u:GetType()~=5) end
end
function CheckRestored()
  local t=Game:GetProperty('JAC_ASN_BLOCKED_TEST')
  for _,p in ipairs(t.Plots) do local plot=Map.GetPlot(p.X,p.Y)
    assert(plot:GetTerrainType()==(p.X==2 and 4 or 1))
    assert(plot:GetFeatureType()==(p.X==0 and 3 or -1))
  end
  assert(Players[0]:GetCities():FindID(10))
  assert(Players[0]:GetUnits():GetCount()==2 and Players[0]:GetUnits():FindID(unrelated))
  assert(Players[0]:GetUnits():FindID(garrison))
  assert(Map.GetPlot(0,0):GetTerrainType()==1 and Map.GetPlot(0,0):GetFeatureType()==3)
end
function CheckCenterFallbackUnchanged()
  assert(Players[0]:GetCities():FindID(10) and Players[0]:GetUnits():GetCount()==2)
  assert(Players[0]:GetUnits():FindID(garrison) and Players[0]:GetUnits():FindID(unrelated))
  for x=0,6 do local plot=Map.GetPlot(x,0)
    assert(plot:GetTerrainType()==(x==2 and 4 or 1))
    assert(plot:GetFeatureType()==(x==0 and 3 or -1))
  end
end
function CheckPreflightUnchanged()
  assert(NATIVE_CALLS==0 and INIT_CALLS==0 and Players[0]:GetUnits():FindID(garrison) and Players[0]:GetUnits():FindID(unrelated))
  for x=1,6 do assert(Map.GetPlot(x,0):GetTerrainType()==(x==2 and 4 or 1)) end
  for x=0,7 do assert(Map.GetPlot(x,0):GetFeatureType()==(x==0 and 3 or (PROBE_MODE=='natural_wonder' and x==2 and 99 or -1))) end
  for owner,expected in pairs(initialUnits) do
    local expectedCount,actualCount=0,0
    for id,row in pairs(expected) do
      expectedCount=expectedCount+1
      local actual=DATA[owner].Units[id]; assert(actual,'preflight removed an existing unit')
      assert(actual.ID==row.ID and actual.Owner==row.Owner and actual.Type==row.Type and
        actual.X==row.X and actual.Y==row.Y and actual.Damage==row.Damage,'preflight changed an existing unit')
    end
    for _ in pairs(DATA[owner].Units) do actualCount=actualCount+1 end
    assert(actualCount==expectedCount,'preflight changed the complete unit inventory')
  end
  if occupiedUnit then
    local u=Players[occupiedOwner]:GetUnits():FindID(occupiedUnit)
    assert(u and u:GetX()==2 and u:GetY()==0 and u:GetType()==4 and u:GetDamage()==0)
  end
end
function CheckUnsafeCandidateRestored()
  assert(NATIVE_CALLS==1 and Players[0]:GetUnits():GetCount()==2)
  assert(Players[0]:GetUnits():FindID(garrison) and Players[0]:GetUnits():FindID(unrelated))
  assert(Players[0]:GetCities():FindID(10))
  for x=0,7 do local plot=Map.GetPlot(x,0)
    assert(plot:GetTerrainType()==(x==2 and 4 or 1))
    assert(plot:GetFeatureType()==(x==0 and 3 or -1))
  end
end
function CorruptCenter(field)
  if field=='terrain' then terrain['0,0']=4 else features['0,0']=-1 end
end
function CheckOtherCityUnchanged()
  assert(Map.GetPlot(1,0):GetTerrainType()==1 and Map.GetPlot(1,0):GetFeatureType()==-1)
  local u=Players[0]:GetUnits():FindID(otherCityUnit)
  assert(u and u:GetOwner()==0 and u:GetType()==4 and u:GetX()==1 and u:GetY()==0 and u:GetDamage()==0)
end
"#;
        for mode in [
            "fallback",
            "unplaced",
            "stacked",
            "unbounded",
            "wrong_type",
            "corrupt_cleanup",
            "lost_city",
            "other_city",
            "occupied",
            "foreign_occupied",
            "foreign_territory",
            "water",
            "water_candidate",
            "resource",
            "improvement",
            "district",
            "route",
            "natural_wonder",
            "unsafe_candidate",
        ] {
            let lua = Lua::new();
            lua.load(repo.read("tools/civ6-lua-tests-rs/fixtures/assassin/stubs.lua")?)
                .exec()?;
            lua.load(format!("PROBE_MODE='{mode}'\n{fixture}")).exec()?;
            let result = lua.load(setup).exec();
            let safe = matches!(mode, "fallback" | "unplaced" | "water");
            assert_eq!(result.is_ok(), safe, "{mode}: {result:?}");
            if mode == "other_city" {
                assert!(result
                    .as_ref()
                    .unwrap_err()
                    .to_string()
                    .contains("fixture would modify another city"));
                lua.load("CheckOtherCityUnchanged()").exec()?;
            }
            if mode == "stacked" {
                assert!(result
                    .as_ref()
                    .unwrap_err()
                    .to_string()
                    .contains("native placement uses the intact training city"));
                lua.load("CheckCenterFallbackUnchanged()").exec()?;
            }
            if matches!(mode, "occupied" | "foreign_occupied") {
                assert!(result
                    .as_ref()
                    .unwrap_err()
                    .to_string()
                    .contains("fixture would modify an occupied plot"));
                lua.load("CheckPreflightUnchanged()").exec()?;
            }
            if mode == "foreign_territory" {
                assert!(result
                    .as_ref()
                    .unwrap_err()
                    .to_string()
                    .contains("fixture would modify foreign territory"));
                lua.load("CheckPreflightUnchanged()").exec()?;
            }
            if matches!(mode, "resource" | "improvement" | "district" | "route") {
                assert!(result
                    .as_ref()
                    .unwrap_err()
                    .to_string()
                    .contains("fixture would modify a plot with assets"));
                lua.load("CheckPreflightUnchanged()").exec()?;
            }
            if mode == "natural_wonder" {
                assert!(result
                    .as_ref()
                    .unwrap_err()
                    .to_string()
                    .contains("fixture would modify a natural wonder"));
                lua.load("CheckPreflightUnchanged()").exec()?;
            }
            if mode == "unsafe_candidate" {
                assert!(result
                    .as_ref()
                    .unwrap_err()
                    .to_string()
                    .contains("fixture would modify a natural wonder"));
                lua.load("CheckUnsafeCandidateRestored()").exec()?;
            }
            if mode == "water_candidate" {
                assert!(result
                    .as_ref()
                    .unwrap_err()
                    .to_string()
                    .contains("native placement uses unchanged water"));
                lua.load("CheckCenterFallbackUnchanged()").exec()?;
            }
            if safe {
                lua.load("CheckFootprint()").exec()?;
                lua.load(format!("{PREFIX} RestoreTerrain()")).exec()?;
                lua.load("CheckRestored()").exec()?;
                for mutation in [
                    "TerrainBuilder.SetTerrainType(Map.GetPlot(1,0),4)",
                    "TerrainBuilder.SetFeatureType(Map.GetPlot(1,0),99)",
                    "DATA[0].Cities[10]=nil",
                ] {
                    lua.load(format!("{PREFIX} RestoreTerrain()")).exec()?;
                    lua.load(mutation).exec()?;
                    assert!(lua
                        .load(format!("{PREFIX} RestoredTerrain()"))
                        .exec()
                        .is_err());
                }
            }
        }
        for field in ["terrain", "feature"] {
            let lua = Lua::new();
            lua.load(repo.read("tools/civ6-lua-tests-rs/fixtures/assassin/stubs.lua")?)
                .exec()?;
            lua.load(format!("PROBE_MODE='fallback'\n{fixture}"))
                .exec()?;
            lua.load(setup).exec()?;
            lua.load(format!(
                "{PREFIX} RestoreTerrain(); CorruptCenter('{field}')"
            ))
            .exec()?;
            assert!(lua
                .load(format!("{PREFIX} RestoredTerrain()"))
                .exec()
                .is_err());
        }
        Ok(())
    }
    #[test]
    fn recovery_accepts_native_positions_beyond_first_ring_and_rejects_off_map_rows() -> Result<()>
    {
        let data = build(&Repository::checked_in())?;
        let recovery = data["steps"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|step| step["code"].as_str())
            .find(|code| code.contains("recovery did not create exactly one placed assassin"))
            .unwrap();
        for (x, mode) in [
            (5, "valid"),
            (999999, "valid"),
            (5, "missing_member"),
            (5, "wrong_member"),
            (5, "duplicate_member"),
            (5, "missing_api"),
            (5, "one_city"),
            (5, "other_city_bypass"),
            (5, "mutated_rejection"),
            (5, "wrong_lock"),
        ] {
            let lua = Lua::new();
            lua.load(format!(r#"
local unit={{GetID=function() return 90 end,GetOwner=function() return 0 end,GetType=function() return 0 end,
  GetX=function() return {x} end,GetY=function() return 0 end,GetDamage=function() return 0 end}}
local s={{LastProducedUnit=90,LastCompletion=2,ProductionUnlock=12}}
local mode='{mode}'
if mode=='wrong_lock' then s.ProductionUnlock=11 end
local t={{Actor=0,City=10,X=0,Y=0,CenterTerrain=1,CenterFeature=3,Plots={{}},Before={{}},AllReleased={{}},Released=1}}
Game={{GetProperty=function() return t end,SetProperty=function() end,GetCurrentGameTurn=function() return 2 end}}
Players={{[0]={{GetCities=function() return {{FindID=function() return {{GetX=function() return 0 end,GetY=function() return 0 end}} end,
  Members=function() local id=9; local last=mode=='one_city' and 10 or 11
    return function() id=id+1; if id<=last then return id,{{GetID=function() return id end}} end end end}} end,
  GetUnits=function() return {{Members=function() local once=false; return function()
    if not once then once=true; return 90,unit end end end}} end}}}}
Map={{GetPlot=function(px) if px==999999 then return nil end
  return {{GetTerrainType=function() return 1 end,GetFeatureType=function() return 3 end}} end,
  GetPlotDistance=function() return {x} end}}
Units={{GetUnitsInPlot=function()
  if mode=='missing_member' then return {{}} end
  if mode=='wrong_member' then return {{{{GetID=function() return 90 end,GetOwner=function() return 1 end}}}} end
  if mode=='duplicate_member' then return {{unit,unit}} end
  return {{unit}}
end}}
if mode=='missing_api' then Units=nil end
GameInfo={{Units={{ASSASSIN={{Index=0}}}}}}
ExposedMembers={{JAC_Assassin={{State=function() local copy={{}}; for k,v in pairs(s) do copy[k]=v end; return copy end,IsAssassin=function() return true end,
  UnitForPlayer=function() return 'ASSASSIN' end,Turn=function() return 2 end,PRODUCTION_LOCK=10,
  Scale=function(v) return v end,StartProduction=function(actor,city)
    assert(actor==0 and (city==10 or city==11))
    if city==11 and mode=='other_city_bypass' then return true end
    if city==11 and mode=='mutated_rejection' then s.ProductionUnlock=13 end
    return false,'PRODUCTION_LOCK' end}}}}
"#)).exec()?;
            let result = lua.load(recovery).exec();
            assert_eq!(
                result.is_ok(),
                x == 5 && mode == "valid",
                "{x}/{mode}: {result:?}"
            );
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
  {Actor=0,City=4,X=0,Y=0,CenterTerrain=1,CenterFeature=-1,Start=1,Due=2,Initial={},Before={},AllBefore={},Plots={}} end,GetCurrentGameTurn=function() return 2 end}
Map={GetPlot=function() return {GetTerrainType=function() return 1 end,GetFeatureType=function() return -1 end} end}
local leak=false
Players={[0]={GetCities=function() return {FindID=function(_,id)
  assert(id==4); return {GetX=function() return 0 end,GetY=function() return 0 end}
end} end,GetUnits=function() return {Members=function() local done=false; return function()
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
            assert_eq!(
                result.is_ok(),
                mutation.is_empty(),
                "{mutation}: {result:?}"
            );
        }
        Ok(())
    }
}
