//! ToDo #172: native country elimination with pending and resolved save files.
use crate::{apply_outputs, Outputs, Repository};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::PathBuf;

const PREFIX: &str = r#"local R=ExposedMembers.JAC_Assassin; assert(R,'Assassin gameplay absent')
local t=Game:GetProperty('JAC_ASN_ELIM_TEST')
local function Copy(v)
  if type(v)~='table' then return v end
  local c={}; for k,x in pairs(v) do c[k]=Copy(x) end; return c
end
local function Same(a,b)
  if type(a)~=type(b) then return false end
  if type(a)~='table' then return a==b end
  for k,v in pairs(a) do if not Same(v,b[k]) then return false end end
  for k,v in pairs(b) do if not Same(v,a[k]) then return false end end
  return true
end
local function NativeDead(id)
  assert(not Players[id]:IsAlive(),'native player still alive: '..id)
  assert(Players[id]:GetCities():GetCount()==0,'eliminated country retained cities')
  assert(Players[id]:GetUnits():GetCount()==0,'eliminated country retained units')
  for _,p in ipairs(PlayerManager.GetAliveMajors()) do
    assert(p:GetID()~=id,'eliminated country remains in alive-major enumeration')
  end
end
local function Cities(rows)
  for _,row in ipairs(rows) do
    local city=CityManager.GetCityAt(row.X,row.Y)
    assert(city and city:GetOwner()==row.Owner,'saved city ownership changed')
  end
end
local function NoSuccess()
  assert(Same(Game:GetProperty(R.NATIONS),t.Nations),'country history changed')
  assert(Same(Players[t.Target]:GetProperty(R.NATION),t.Nation),'leader death was applied')
  Cities(t.Assets)
end
local function Denouncements()
  local rows={}
  for _,p in ipairs(PlayerManager.GetAliveMajors()) do
    local id=p:GetID(); local r={}
    for _,job in ipairs(t.Jobs) do
      r[tostring(job.Actor)]=p:GetProperty('JAC_ASN_DENOUNCE_'..job.Actor)
    end
    rows[tostring(id)]=r
  end
  return rows
end
local function Pending()
  NoSuccess()
  assert(Same(Players[t.Target]:GetProperty('JAC_ASN_DIPLOMACY_V1'),t.Ledger),'penalty applied before deadline')
  for _,job in ipairs(t.Jobs) do
    assert(Same(R.State(job.Actor),job.Pending),'pending state changed across save/load')
    if t.Mode=='actor' then NativeDead(job.Actor) else assert(Players[job.Actor]:IsAlive(),'actor eliminated') end
    local found=false
    for _,id in ipairs(Game:GetProperty(R.ACTORS) or {}) do if id==job.Actor then found=true end end
    assert(found,'saved work owner absent from actor registry')
  end
  if t.Mode=='target' then NativeDead(t.Target) end
  assert(R.Turn()==t.PendingTurn,'pending load advanced the turn')
end
local function Resolved()
  NoSuccess()
  local expected=Copy(t.Ledger) or {Grievances={},Friendship={},WarReason={}}
  expected.Applied=expected.Applied or {}
  local expectedDenouncements=Copy(t.BeforeDenouncements)
  for _,job in ipairs(t.Jobs) do
    local s=R.State(job.Actor); local m=s.LastMission
    assert(s.Mission==nil,'mission still pending')
    assert(s.CommandUnlock==job.Pending.CommandUnlock,'nationwide wait changed')
    local want=Copy(job.Pending.Mission)
    want.Result=t.Mode=='actor' and 'ACTOR_GONE' or 'TARGET_GONE'
    want.Cancelled=t.Mode=='actor'; want.Resolved=t.ResolvedTurn
    assert(Same(m,want),'fixed rolls, result or resolution changed')
    if t.Mode=='actor' then
      NativeDead(job.Actor)
      assert(s.Production==nil,'eliminated actor retained training')
      assert(s.LastCompletion==job.Pending.LastCompletion,'eliminated actor completed training')
      assert(s.ProductionUnlock==job.Pending.ProductionUnlock,'eliminated actor changed training lock')
    else
      assert(Players[job.Actor]:IsAlive(),'actor eliminated')
      local u=Players[job.Actor]:GetUnits():FindID(job.Unit)
      if not want.Success then
        local sameIdentity
        for _,candidate in Players[job.Actor]:GetUnits():Members() do
          if candidate:GetProperty(R.TOKEN)==want.ActorIdentity then sameIdentity=candidate end
        end
        if u and u:GetProperty(R.TOKEN)~=want.ActorIdentity then
          print('[JAC_ASN_ELIM] unit_id_identity_mismatch actor='..job.Actor..' unit='..job.Unit..
            ' expected_identity='..tostring(want.ActorIdentity)..
            ' observed_identity='..tostring(u:GetProperty(R.TOKEN))..
            ' original_identity_present='..tostring(sameIdentity~=nil))
        end
        assert(not sameIdentity,'failed assassin identity remains')
      else
        assert(u and u:GetProperty(R.TOKEN)==want.ActorIdentity,'surviving actor changed identity')
        assert(u:GetID()==job.Unit and u:GetOwner()==job.Actor,'surviving actor was replaced')
      end
    end
    if t.Mode=='actor' or not want.Success or want.Detected then
      local key=tostring(job.Actor)
      expected.Grievances[key]=(expected.Grievances[key] or 0)+want.PenaltyGrievances
      expected.Friendship[key]=(expected.Friendship[key] or 0)-want.PenaltyFriendship
      expected.Applied[want.ID]=1
      if t.Mode=='target' and want.Detected then expected.WarReason[key]=want.Due end
    end
    if t.Mode=='target' and want.Detected then
      for id,row in pairs(expectedDenouncements) do
        if tonumber(id)~=job.Actor and tonumber(id)~=t.Target then row[tostring(job.Actor)]=want.Due end
      end
    end
    print('[JAC_ASN_ELIM] mode='..t.Mode..' actor='..job.Actor..' alive='..tostring(Players[job.Actor]:IsAlive())..
      ' mission='..m.ID..' result='..m.Result..' success='..tostring(m.Success)..' detected='..tostring(m.Detected)..
      ' resolved='..m.Resolved..' wait='..s.CommandUnlock)
  end
  if t.Mode=='target' then NativeDead(t.Target) end
  assert(Same(Players[t.Target]:GetProperty('JAC_ASN_DIPLOMACY_V1'),expected),'missing or duplicate penalty')
  assert(Same(Denouncements(),expectedDenouncements),'missing or unexpected denunciation')
  if t.Final then
    for _,job in ipairs(t.Jobs) do assert(Same(R.State(job.Actor),t.Final[tostring(job.Actor)]),'resolved state changed') end
    assert(Same(Denouncements(),t.AfterDenouncements),'denunciation changed across save/load')
  end
end
"#;

const SETUP: &str = r#"
local function PlaceForMission(owner,u,city)
  assert(Game:GetProperty(R.TEST)==1,'mission placement requires test mode')
  local id=u:GetID(); local occupied={}
  for _,p in ipairs(PlayerManager.GetAlive()) do
    for _,other in p:GetUnits():Members() do
      if other:GetOwner()~=owner or other:GetID()~=id then
        occupied[other:GetX()..','..other:GetY()]=true
      end
    end
  end
  local cx,cy=city:GetX(),city:GetY()
  for radius=1,6 do
    for y=cy-radius,cy+radius do for x=cx-radius,cx+radius do
      local plot=Map.GetPlot(x,y)
      if plot and not occupied[x..','..y] and not plot:IsWater() and not plot:IsImpassable()
        and not plot:IsCity() and Map.GetPlotDistance(x,y,cx,cy)<=radius then
        -- Official AustraliaScenario.lua:854. Keep the native unit ID and
        -- create at its own city first; remote foreign-country spawning fails.
        UnitManager.PlaceUnit(u,x,y)
        if u:GetX()==x and u:GetY()==y then
          assert(u:GetOwner()==owner and u:GetID()==id,'placement replaced the actor')
          FixtureReveal(owner,x,y)
          print('[JAC_ASN_ELIM_PLACE] actor='..owner..' unit='..id..' xy='..x..','..y)
          return
        end
      end
    end end
  end
  error('fixture mission placement failed: owner='..owner..' unit='..id)
end
Game:SetProperty(R.TEST,1)
local houses={}
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
  houses[PlayerConfigurations[p:GetID()]:GetCivilizationTypeName()]=p:GetID()
end
t={Mode=MODE,Jobs={},Assets={},PendingTurn=R.Turn()}
t.Target=assert(houses.CIVILIZATION_JAC_MATSUDAIRA,'target absent')
local targetCity=assert(Players[t.Target]:GetCities():GetCapitalCity(),'target capital absent')
t.Nations=Game:GetProperty(R.NATIONS); t.Nation=Players[t.Target]:GetProperty(R.NATION)
t.Ledger=Players[t.Target]:GetProperty('JAC_ASN_DIPLOMACY_V1')
local names={'CIVILIZATION_JAC_DATE','CIVILIZATION_JAC_ASHINA','CIVILIZATION_JAC_HOJO','CIVILIZATION_JAC_YAMANOUCHI_UESUGI'}
for i,name in ipairs(names) do
  local a=assert(houses[name],'actor absent: '..name); assert(not Players[a]:IsHuman(),'actor must be AI')
  local city=assert(Players[a]:GetCities():GetCapitalCity(),'actor capital absent')
  local u=Grant(a,R.UnitForPlayer(a),city)
  PlaceForMission(a,u,targetCity)
  FixtureReveal(a,targetCity:GetX(),targetCity:GetY())
  Players[a]:SetProperty('JAC_ASN_FORCE_NEXT',{Success=i>2 and 1 or 0,Detected=i%2==0 and 1 or 0,Branch='FREE_STATE'})
  assert(R.StartAttempt(a,u:GetID(),t.Target,'LEADER'))
  if MODE=='actor' then assert(R.StartProduction(a,city:GetID())) end
  assert(R.TestAdvanceDeadlines(a,{Mission=true}))
  local state=R.State(a)
  assert(state.Mission.Success==(i>2),'forced success combination not applied')
  assert(state.Mission.Detected==(i%2==0),'forced detection combination not applied')
  assert(Players[a]:GetProperty('JAC_ASN_FORCE_NEXT')==nil,'forced result not consumed')
  assert(state.Mission.Due==R.Turn()+1,'mission hook did not shorten deadline')
  t.Jobs[#t.Jobs+1]={Actor=a,Unit=u:GetID(),Pending=state,ParkX=city:GetX(),ParkY=city:GetY()}
  if MODE=='target' then ParkTrainedUnit(a,u:GetID(),city:GetID()) end
end
local ids={}
if MODE=='actor' then for _,job in ipairs(t.Jobs) do ids[#ids+1]=job.Actor end else ids={t.Target} end
t.Dead=ids
-- Official AustraliaScenario.lua and the existing free-state branch: clear units
-- before transferring the last city. Never overwrite Player:IsAlive or saved jobs.
for _,id in ipairs(ids) do
  local unitIDs={}; for _,u in Players[id]:GetUnits():Members() do unitIDs[#unitIDs+1]=u:GetID() end
  for _,uid in ipairs(unitIDs) do UnitManager.Kill(assert(Players[id]:GetUnits():FindID(uid))) end
  local cityIDs={}; for _,c in Players[id]:GetCities():Members() do cityIDs[#cityIDs+1]=c:GetID() end
  assert(#cityIDs>0,'elimination requires actual cities')
  for _,cid in ipairs(cityIDs) do
    local c=assert(Players[id]:GetCities():FindID(cid)); local x,y=c:GetX(),c:GetY()
    CityManager.TransferCityToFreeCities(c)
    t.Assets[#t.Assets+1]={X=x,Y=y,Owner=PlayerManager.GetFreeCitiesPlayerID()}
  end
  NativeDead(id)
end
if MODE=='actor' then
  for _,c in Players[t.Target]:GetCities():Members() do t.Assets[#t.Assets+1]={X=c:GetX(),Y=c:GetY(),Owner=t.Target} end
end
t.BeforeDenouncements=Denouncements()
Game:SetProperty('JAC_ASN_ELIM_TEST',t)
Pending()
return 'four fixed result combinations pending after native '..MODE..' elimination'
"#;

fn check(steps: &mut Vec<Value>, code: &str) {
    steps.push(super::ui(&format!("{PREFIX}\n{code}"), "GameCore_Tuner"));
}

fn round_trip(steps: &mut Vec<Value>, name: &str) {
    super::round_trip(steps, name);
}

pub fn build_scenario(repo: &Repository, actor: bool) -> Result<Value> {
    let base = super::build_scenario(repo)?;
    let source = base["steps"].as_array().context("base steps absent")?;
    let mode = if actor { "actor" } else { "target" };
    let mut steps = source[..4].to_vec();
    check(
        &mut steps,
        &format!("{}\nlocal MODE='{mode}'\n{SETUP}", super::GRANT),
    );
    round_trip(&mut steps, &format!("JAC-Assassin-{mode}-Pending"));
    check(&mut steps, "Pending(); R.Tick(); Pending(); return 'pending eliminated-country properties survive same-turn reload'");
    super::advance(&mut steps, 1);
    check(
        &mut steps,
        r#"
      t.ResolvedTurn=R.Turn(); assert(t.ResolvedTurn==t.PendingTurn+1,'resolution overshot the deadline')
      Resolved(); t.Final={}
      for _,job in ipairs(t.Jobs) do t.Final[tostring(job.Actor)]=R.State(job.Actor) end
      t.AfterDenouncements=Denouncements(); Game:SetProperty('JAC_ASN_ELIM_TEST',t)
      R.Tick(); Resolved(); return 'fixed results and exact penalty deltas resolved once'
    "#,
    );
    round_trip(&mut steps, &format!("JAC-Assassin-{mode}-Resolved"));
    check(&mut steps, "Resolved(); R.Tick(); Resolved(); return 'resolved results and penalty ledger survive first reload'");
    super::advance(&mut steps, 1);
    check(
        &mut steps,
        "Resolved(); return 'next native turn does not repeat resolution or city transfers'",
    );
    round_trip(&mut steps, &format!("JAC-Assassin-{mode}-Stable"));
    check(&mut steps, "Resolved(); R.Tick(); Resolved(); Game:SetProperty(R.TEST,nil); return 'second resolved reload remains identical; test mode cleared'");
    Ok(
        json!({"name":format!("Assassin: native {mode} country elimination, four fixed combinations, pending and two resolved save/load cycles"),"judge_profile":"japan_map","steps":steps}),
    )
}

pub fn outputs(repo: &Repository) -> Result<Outputs> {
    let mut result = Outputs::new();
    for actor in [true, false] {
        let mode = if actor { "actor" } else { "target" };
        result.insert(
            PathBuf::from(format!(
                "tools/civ6-autotest/scenarios/assassin-{mode}-eliminated-save-load.json"
            )),
            serde_json::to_string_pretty(&build_scenario(repo, actor)?)? + "\n",
        );
    }
    Ok(result)
}

pub fn cli(repo: &Repository, check: bool) -> Result<()> {
    apply_outputs(repo, outputs(repo)?, check)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_scenarios_and_all_lua51_chunks() -> Result<()> {
        let repo = Repository::checked_in();
        for (path, content) in outputs(&repo)? {
            assert_eq!(repo.read(&path)?, content, "{}", path.display());
            let scenario: Value = serde_json::from_str(&content)?;
            let lua = mlua::Lua::new();
            let mut chunks = 0;
            for step in scenario["steps"].as_array().unwrap() {
                if let Some(code) = step["code"].as_str().or(step["condition"]["code"].as_str()) {
                    lua.load(code).into_function()?;
                    chunks += 1;
                }
            }
            assert_eq!(chunks, 12);
            assert_eq!(scenario["steps"].as_array().unwrap().len(), 26);
        }
        Ok(())
    }

    fn harness(repo: &Repository) -> Result<mlua::Lua> {
        let lua = mlua::Lua::new();
        lua.load(repo.read("tools/civ6-lua-tests-rs/fixtures/assassin/stubs.lua")?)
            .exec()?;
        lua.load(r#"
          MAX_MAJOR=5
          local names={[0]='CIVILIZATION_JAC_ODA',[1]='CIVILIZATION_JAC_DATE',
            [2]='CIVILIZATION_JAC_ASHINA',[3]='CIVILIZATION_JAC_HOJO',
            [4]='CIVILIZATION_JAC_YAMANOUCHI_UESUGI',[5]='CIVILIZATION_JAC_MATSUDAIRA'}
          DATA={}
          for id=0,6 do
            DATA[id]={Properties={},Units={},Cities={},Capital=10,Alive=id~=6,
              Wars={},Allies={},Favor=0,Military=0,Religion=-1,Civilization=names[id]}
            if id~=6 then
              DATA[id].Cities[10]={ID=10,Owner=id,X=id*10,Y=0,Properties={}}
            end
            PlayersVisibility[id]={IsRevealed=function() return true end,IsVisible=function() return true end}
          end
          rebuild()
          Map.GetPlot=function(x,y)
            if x<0 or y<0 then return nil end
            return {IsWater=function() return false end,IsImpassable=function() return false end,
              IsCity=function() return CityManager.GetCityAt(x,y)~=nil end}
          end
          local spawn=UnitManager.InitUnitValidAdjacentHex
          UnitManager.InitUnitValidAdjacentHex=function(id,kind,x,y,radius)
            local city=DATA[id].Cities[DATA[id].Capital]
            assert(city and city.X==x and city.Y==y,'fixture spawn must use an owned anchor')
            spawn(id,kind,x,y,radius)
          end
          include=function() end
        "#).exec()?;
        reload(&lua, repo)?;
        lua.load("ExposedMembers.JAC_Assassin.Tick()").exec()?;
        Ok(lua)
    }

    fn reload(lua: &mlua::Lua, repo: &Repository) -> Result<()> {
        lua.load(repo.read("poc/JAC_PoC_Assassin/Scripts/JAC_Assassin_Rules.lua")?)
            .exec()?;
        lua.load(repo.read("poc/JAC_PoC_Assassin/Scripts/JAC_Assassin_Gameplay.lua")?)
            .exec()?;
        Ok(())
    }

    #[test]
    fn generated_checks_execute_with_recreated_gameplay_contexts() -> Result<()> {
        let repo = Repository::checked_in();
        for actor in [true, false] {
            let lua = harness(&repo)?;
            let scenario = build_scenario(&repo, actor)?;
            let mut ended_turns = 0;
            for (index, step) in scenario["steps"].as_array().unwrap().iter().enumerate() {
                match step["action"].as_str().unwrap() {
                    "ui_lua" => lua
                        .load(step["code"].as_str().unwrap())
                        .exec()
                        .with_context(|| format!("actor={actor} step={index}"))?,
                    "end_turn" => {
                        lua.load("ADVANCE(1)").exec()?;
                        ended_turns += 1;
                        if !actor && ended_turns == 2 {
                            lua.load(
                                r#"
                              local t=Game:GetProperty('JAC_ASN_ELIM_TEST')
                              for _,job in ipairs(t.Jobs) do
                                if not job.Pending.Mission.Success then
                                  local units=DATA[job.Actor].Units
                                  assert(units[job.Unit]==nil,'failed assassin survived before UnitID reuse')
                                  units[job.Unit]={
                                    ID=job.Unit,Owner=job.Actor,
                                    Type=GameInfo.Units[JAC_ASN.UnitForPlayer(job.Actor)].Index,
                                    X=job.Actor,Y=1,
                                    Properties={[JAC_ASN.TOKEN]='replacement:'..job.Actor}
                                  }
                                end
                              end
                            "#,
                            )
                            .exec()?;
                        }
                    }
                    "load" => reload(&lua, &repo)?,
                    _ => {}
                }
            }
            if !actor {
                lua.load(
                    r#"
                  local t=Game:GetProperty('JAC_ASN_ELIM_TEST')
                  local job
                  for _,candidate in ipairs(t.Jobs) do
                    if not candidate.Pending.Mission.Success then job=candidate; break end
                  end
                  assert(job,'target scenario lacks a failed mission')
                  local units=DATA[job.Actor].Units
                  local data=assert(units[job.Unit])
                  local replacement='replacement:'..job.Actor
                  assert(data.Properties[JAC_ASN.TOKEN]==replacement,'UnitID reuse fixture missing')
                  data.Properties[JAC_ASN.TOKEN]=job.Pending.Mission.ActorIdentity
                "#,
                )
                .exec()?;
                let same_id_error = lua
                    .load(format!("{PREFIX}\nResolved()"))
                    .exec()
                    .unwrap_err();
                assert!(
                    same_id_error
                        .to_string()
                        .contains("failed assassin identity remains"),
                    "{same_id_error}"
                );
                lua.load(
                    r#"
                  local t=Game:GetProperty('JAC_ASN_ELIM_TEST')
                  local job
                  for _,candidate in ipairs(t.Jobs) do
                    if not candidate.Pending.Mission.Success then job=candidate; break end
                  end
                  local units=DATA[job.Actor].Units
                  local data=assert(units[job.Unit])
                  local old_id=job.Unit; local new_id=NEXT_UNIT; NEXT_UNIT=NEXT_UNIT+1
                  units[old_id]=nil; data.ID=new_id; units[new_id]=data
                "#,
                )
                .exec()?;
                let different_id_error = lua
                    .load(format!("{PREFIX}\nResolved()"))
                    .exec()
                    .unwrap_err();
                assert!(
                    different_id_error
                        .to_string()
                        .contains("failed assassin identity remains"),
                    "{different_id_error}"
                );
                lua.load(
                    r#"
                  local t=Game:GetProperty('JAC_ASN_ELIM_TEST')
                  local job
                  for _,candidate in ipairs(t.Jobs) do
                    if not candidate.Pending.Mission.Success then job=candidate; break end
                  end
                  local units=DATA[job.Actor].Units
                  local data=assert(units[NEXT_UNIT-1])
                  units[NEXT_UNIT-1]=nil; data.ID=job.Unit; units[job.Unit]=data
                  data.Properties[JAC_ASN.TOKEN]='replacement:'..job.Actor
                "#,
                )
                .exec()?;
            }
            // Reusing the exact generated checker rejects corruption introduced by a load.
            lua.load(
                r#"
              local t=Game:GetProperty('JAC_ASN_ELIM_TEST')
              local ledger=Players[t.Target]:GetProperty('JAC_ASN_DIPLOMACY_V1')
              GOOD_LEDGER=clone(ledger)
              ledger.Grievances[tostring(t.Jobs[1].Actor)]=999
              Players[t.Target]:SetProperty('JAC_ASN_DIPLOMACY_V1',ledger)
            "#,
            )
            .exec()?;
            let err = lua
                .load(format!("{PREFIX}\nResolved()"))
                .exec()
                .unwrap_err();
            assert!(
                err.to_string().contains("missing or duplicate penalty"),
                "{err}"
            );
            lua.load(format!(
                r#"{PREFIX}
              Players[t.Target]:SetProperty('JAC_ASN_DIPLOMACY_V1',GOOD_LEDGER)
              Players[0]:SetProperty('JAC_ASN_DENOUNCE_'..t.Jobs[2].Actor,999)
            "#
            ))
            .exec()?;
            let err = lua
                .load(format!("{PREFIX}\nResolved()"))
                .exec()
                .unwrap_err();
            assert!(
                err.to_string()
                    .contains("missing or unexpected denunciation"),
                "{err}"
            );
        }
        Ok(())
    }
}
