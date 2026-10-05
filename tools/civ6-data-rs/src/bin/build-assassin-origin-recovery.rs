use anyhow::{ensure, Result};
use civ6_data_rs::Repository;
use serde_json::{json, Value};

const SETUP: &str = r#"
Game:SetProperty(R.TEST,1)
local owner=Game.GetLocalPlayer(); assert(Players[owner]:IsHuman(),'human required')
local city=Players[owner]:GetCities():GetCapitalCity(); assert(city,'capital missing')
local cityX,cityY=city:GetX(),city:GetY(); local center=Map.GetPlot(cityX,cityY)
local function EmptyLand(p)
  if not p or p:IsCity() or p:IsWater() or p:IsImpassable() or
    (p:GetOwner()~=-1 and p:GetOwner()~=owner) or p:GetResourceType()~=-1 or
    p:GetImprovementType()~=-1 or p:GetDistrictType()~=-1 then return false end
  for id=0,63 do local player=Players[id]
    if player then for _,u in player:GetUnits():Members() do
      if u:GetX()==p:GetX() and u:GetY()==p:GetY() then return false end
    end end
  end
  return true
end
local origin,plots
for index=0,Map.GetPlotCount()-1 do
  local p=Map.GetPlotByIndex(index)
  local distance=Map.GetPlotDistance(cityX,cityY,p:GetX(),p:GetY())
  if distance>=3 and distance<=18 and EmptyLand(p) then
    local candidate={p}; for _,adjacent in ipairs(Map.GetAdjacentPlots(p:GetX(),p:GetY())) do candidate[#candidate+1]=adjacent end
    local good=#candidate==7
    for _,q in ipairs(candidate) do if not EmptyLand(q) then good=false end end
    if good then origin=p; plots=candidate; break end
  end
end
assert(origin,'no safe empty remote land footprint')
local rows={}
for _,p in ipairs(plots) do rows[#rows+1]={X=p:GetX(),Y=p:GetY(),Terrain=p:GetTerrainType(),
  Feature=p:GetFeatureType(),Barrier=GameInfo.Terrains.TERRAIN_GRASS_MOUNTAIN.Index} end
t={Actor=owner,City=city:GetID(),X=cityX,Y=cityY,Plots=rows,
  CenterTerrain=center:GetTerrainType(),CenterFeature=center:GetFeatureType(),
  FixtureX=origin:GetX(),FixtureY=origin:GetY(),Start=R.Turn(),Initial=R.State(owner),
  Before=Assassins(owner),AllBefore=UnitIDs(owner)}
assert(next(t.Before)==nil,'fresh game already contains assassins')
Game:SetProperty('JAC_ASN_BLOCKED_TEST',t)
assert(R.TestTrainingOrigin(owner,t.City,t.FixtureX,t.FixtureY),'test origin not accepted')
for _,p in ipairs(rows) do local plot=Map.GetPlot(p.X,p.Y)
  TerrainBuilder.SetFeatureType(plot,-1); TerrainBuilder.SetTerrainType(plot,p.Barrier)
  assert(plot:IsImpassable(),'native terrain barrier not established'); IntactCity()
end
local before=UnitIDs(owner); local probes={}
local sx,sy=R.TrainingOrigin(owner,city)
assert(sx==t.FixtureX and sy==t.FixtureY,'fixture origin not used')
UnitManager.InitUnitValidAdjacentHex(owner,R.UnitForPlayer(owner),sx,sy,1)
for _,u in Players[owner]:GetUnits():Members() do if not before[tostring(u:GetID())] then probes[#probes+1]=Row(u) end end
local blocked=#probes==0
for _,row in ipairs(probes) do
  Trace('origin-probe',row)
  local u=Players[owner]:GetUnits():FindID(row.ID)
  assert(u and u:GetID()==row.ID and u:GetOwner()==row.Owner and u:GetType()==row.Type,'origin probe identity changed')
  UnitManager.Kill(u)
  blocked=blocked or (row.Type==GameInfo.Units[R.UnitForPlayer(owner)].Index and row.X<0 and row.Y<0)
end
assert(Same(UnitIDs(owner),before),'origin probe cleanup changed old inventory')
if #probes>1 or not blocked then
  RestoreTerrain(); assert(R.TestTrainingOrigin(owner,t.City,nil,nil))
  error('native API did not reject the remote blocked origin')
end
Blockers(); IntactCity()
print('[JAC_ASN_BLOCKED] qualified origin='..sx..','..sy..' training-city='..cityX..','..cityY..
  ' origin_override=true native_api=true tick_replaced=false')
return 'real native refusal at explicit test origin; training city and original units preserved'
"#;

fn build(repo: &Repository) -> Result<Value> {
    let mut data = repo.json("tools/civ6-autotest/scenarios/assassin-blocked-recovery.json")?;
    let steps = data["steps"].as_array_mut().unwrap();
    let code = steps[4]["code"].as_str().unwrap();
    let prefix = code.split("Game:SetProperty(R.TEST,1)").next().unwrap();
    ensure!(prefix.contains("local function IntactCity()"));
    steps[4]["code"] = json!(format!("{prefix}\n{SETUP}"));
    let last = steps.last_mut().unwrap();
    let final_code = last["code"].as_str().unwrap();
    last["code"]=json!(final_code.replace("Game:SetProperty(R.TEST,nil)",
        "assert(R.TestTrainingOrigin(t.Actor,t.City,nil,nil),'origin cleanup failed')\nGame:SetProperty(R.TEST,nil)\nlocal accepted,reason=R.TestTrainingOrigin(t.Actor,t.City,t.FixtureX,t.FixtureY); assert(not accepted and reason=='TEST_MODE_OFF','origin hook remains active')"));
    data["name"]=json!("Assassin: native blocked-origin retry through real turns, pending save/load and recovery (explicit test origin)");
    Ok(data)
}
fn main() -> Result<()> {
    let path = std::env::args().nth(1).unwrap();
    let data = build(&Repository::checked_in())?;
    std::fs::write(path, serde_json::to_string_pretty(&data)? + "\n")?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_real_turns_and_round_trips_with_explicit_origin() -> Result<()> {
        let data = build(&Repository::checked_in())?;
        let lua = mlua::Lua::new();
        let steps = data["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 32);
        let mut compiled = 0;
        for step in steps {
            if let Some(code) = step["code"]
                .as_str()
                .or_else(|| step["condition"]["code"].as_str())
            {
                lua.load(code).into_function()?;
                compiled += 1;
            }
        }
        assert!(compiled >= 16);
        assert_eq!(
            steps.iter().filter(|s| s["action"] == "end_turn").count(),
            4
        );
        assert_eq!(steps.iter().filter(|s| s["action"] == "save").count(), 2);
        assert_eq!(steps.iter().filter(|s| s["action"] == "load").count(), 2);
        assert!(!SETUP.contains("R.Tick("));
        Ok(())
    }
}
