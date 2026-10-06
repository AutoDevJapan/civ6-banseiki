use anyhow::{ensure, Result};
use civ6_data_rs::Repository;

const BEGIN: &str = "-- BEGIN RUST GENERATED TRAINING ORIGIN FIXTURE\n";
const END: &str = "-- END RUST GENERATED TRAINING ORIGIN FIXTURE\n";
const HOOK: &str = r#"local ORIGIN_TEST_KEY="JAC_ASN_DIAGNOSTIC_ORIGIN_"
function R.TestTrainingOrigin(id,cityID,x,y)
  if Game:GetProperty(R.TEST)~=1 then return false,"TEST_MODE_OFF" end
  local player=Players[id]
  local city=player and player:IsAlive() and player:GetCities():FindID(cityID)
  if not city or city:GetOwner()~=id then return false,"CITY_GONE" end
  local key=ORIGIN_TEST_KEY..tostring(id)
  if x==nil and y==nil then Game:SetProperty(key,nil); return true end
  if type(x)~="number" or type(y)~="number" or x~=math.floor(x) or y~=math.floor(y) then
    return false,"TEST_ORIGIN_INVALID"
  end
  local plot=Map.GetPlot(x,y)
  if not plot or plot:IsCity() then return false,"TEST_ORIGIN_INVALID" end
  local production=R.State(id).Production
  if production and production.City~=cityID then return false,"TEST_ORIGIN_WRONG_CITY" end
  Game:SetProperty(key,{City=cityID,X=x,Y=y})
  return true
end
function R.TrainingOrigin(id,city)
  -- Ordinary production ignores even a stale diagnostic property.
  if Game:GetProperty(R.TEST)==1 then
    local origin=Game:GetProperty(ORIGIN_TEST_KEY..tostring(id))
    if origin then
      assert(type(origin)=="table" and origin.City==city:GetID() and
        type(origin.X)=="number" and type(origin.Y)=="number" and
        origin.X==math.floor(origin.X) and origin.Y==math.floor(origin.Y),"invalid diagnostic origin")
      local plot=Map.GetPlot(origin.X,origin.Y)
      assert(plot and not plot:IsCity(),"diagnostic origin no longer valid")
      return origin.X,origin.Y
    end
  end
  return city:GetX(),city:GetY()
end
function R.TestAbortOriginFixture(id)
  if Game:GetProperty(R.TEST)~=1 then return false,"TEST_MODE_OFF" end
  local snapshot=Game:GetProperty("JAC_ASN_BLOCKED_TEST")
  local errors={}
  if snapshot and snapshot.Actor==id then
    for _,row in ipairs(snapshot.Plots or {}) do
      local ok,why=pcall(function()
        local plot=Map.GetPlot(row.X,row.Y)
        assert(plot and not plot:IsCity(),"rollback plot is missing or became a city")
        TerrainBuilder.SetTerrainType(plot,row.Terrain)
        TerrainBuilder.SetFeatureType(plot,row.Feature)
        assert(plot:GetTerrainType()==row.Terrain and plot:GetFeatureType()==row.Feature,"rollback terrain differs")
      end)
      if not ok then errors[#errors+1]=tostring(why) end
    end
  end
  Game:SetProperty(ORIGIN_TEST_KEY..tostring(id),nil)
  Game:SetProperty(R.TEST,nil)
  return #errors==0,table.concat(errors,"; ")
end
"#;

fn generate(source: &str) -> Result<String> {
    let mut source = source.to_string();
    if let Some(start) = source.find(BEGIN) {
        let end = source[start..].find(END).unwrap() + start + END.len();
        source.replace_range(start..end, "");
    }
    let at = source.find("local function Copy(v)").unwrap();
    source.insert_str(at, &format!("{BEGIN}{HOOK}{END}"));
    let original = "        UnitManager.InitUnitValidAdjacentHex(id,R.UnitForPlayer(id),city:GetX(),city:GetY(),1)";
    let replacement = "        local spawnX,spawnY=R.TrainingOrigin(id,city)\n        UnitManager.InitUnitValidAdjacentHex(id,R.UnitForPlayer(id),spawnX,spawnY,1)";
    if source.contains(original) {
        ensure!(source.matches(original).count() == 1);
        source = source.replace(original, replacement);
    } else {
        ensure!(source.matches(replacement).count() == 1);
    }
    Ok(source)
}

fn main() -> Result<()> {
    let repo = Repository::checked_in();
    let path = "poc/JAC_PoC_Assassin/Scripts/JAC_Assassin_Gameplay.lua";
    std::fs::write(repo.path(path), generate(&repo.read(path)?)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mlua::Lua;
    #[test]
    fn generated_source_is_idempotent_and_compiles_in_lua51() -> Result<()> {
        let repo = Repository::checked_in();
        let data = generate(&repo.read("poc/JAC_PoC_Assassin/Scripts/JAC_Assassin_Gameplay.lua")?)?;
        assert_eq!(generate(&data)?, data);
        Lua::new().load(data).into_function()?;
        Ok(())
    }
    #[test]
    fn normal_mode_ignores_origin_and_debug_mode_checks_city_and_map() -> Result<()> {
        let lua = Lua::new();
        lua.load(r#"
local props={}
Game={GetProperty=function(_,k) return props[k] end,SetProperty=function(_,k,v) props[k]=v end}
local city={GetID=function() return 10 end,GetOwner=function() return 0 end,
  GetX=function() return 120 end,GetY=function() return 40 end}
Players={[0]={IsAlive=function() return true end,GetCities=function() return {FindID=function(_,id) if id==10 then return city end end} end}}
Map={GetPlot=function(x,y) if x==30 and y==10 then return {IsCity=function() return false end} end end}
R={TEST='TEST',State=function() return {Production={City=10}} end}
CITY=city
"#).exec()?;
        lua.load(HOOK).exec()?;
        lua.load(
            r#"
assert(not R.TestTrainingOrigin(0,10,30,10))
Game:SetProperty('JAC_ASN_DIAGNOSTIC_ORIGIN_0',{City=10,X=30,Y=10})
local x,y=R.TrainingOrigin(0,CITY); assert(x==120 and y==40)
Game:SetProperty('TEST',1)
assert(not R.TestTrainingOrigin(1,10,30,10))
assert(not R.TestTrainingOrigin(0,11,30,10))
assert(not R.TestTrainingOrigin(0,10,30.5,10))
assert(not R.TestTrainingOrigin(0,10,999,999))
assert(R.TestTrainingOrigin(0,10,30,10))
x,y=R.TrainingOrigin(0,CITY); assert(x==30 and y==10)
Game:SetProperty('JAC_ASN_DIAGNOSTIC_ORIGIN_0',{City=11,X=30,Y=10})
assert(not pcall(R.TrainingOrigin,0,CITY))
assert(R.TestTrainingOrigin(0,10,nil,nil))
x,y=R.TrainingOrigin(0,CITY); assert(x==120 and y==40)
"#,
        )
        .exec()?;
        Ok(())
    }
}
