//! Every generated batch step runs in Lua 5.1 against a mocked Civ VI: separate
//! globals per Lua context, a UI yield cache that follows Core only on ticks, the
//! product's own JAC_Sengoku_Oaths.lua choice and flag functions, and a native
//! save/load copy. The artifacts use the runner's record format, so the verifier
//! reads them exactly as it reads a live run.
use super::*;
use crate::oath_save_probe::tests::write_rows;
use mlua::{Lua, Table, Value as LuaValue};

const ORACLE: &str = "poc/JAC_PoC_SengokuTrees/Scripts/JAC_Sengoku_Oaths.lua";

/// The oath rows and ages as oaths.rs writes them to SQL, and each oath's effects.
fn data_chunk(repo: &Repository) -> Result<String> {
    let (mut rows, mut ages, mut effects) = (String::new(), String::new(), String::new());
    for house in oaths::catalogue(repo)? {
        let id = house["id"].as_str().context("house id")?;
        for (position, oath) in house["oaths"]
            .as_array()
            .context("oaths")?
            .iter()
            .enumerate()
        {
            let key = format!("JAC_OATH_{id}_{}", oath["id"].as_str().context("oath id")?);
            let q = |s: &str| lua_string(s);
            rows += &format!(
                "{{OathType={},CivilizationType={},Position={},Name={},GoldenDescription={},QuestType={},AIFocus={}}},",
                q(&key),
                q(&format!("CIVILIZATION_JAC_{id}")),
                position + 1,
                q(&format!("LOC_{key}_NAME")),
                q(&format!("LOC_{key}_GOLDEN")),
                q(oath["quest"].as_str().context("quest")?),
                q(oath["focus"].as_str().context("focus")?)
            );
            for age in ["NORMAL", "DARK", "GOLDEN", "HEROIC"] {
                ages += &format!("{{OathType={},AgeType={}}},", q(&key), q(age));
            }
            let pairs: Vec<_> = oath["effects"]
                .as_array()
                .context("effects")?
                .iter()
                .map(|pair| format!("{}={}", pair[0].as_str().unwrap_or_default(), pair[1]))
                .collect();
            effects += &format!("[{}]={{{}}},", q(&key), pairs.join(","));
        }
    }
    Ok(format!(
        "OATH_ROWS={{{rows}}}\nAGE_ROWS={{{ages}}}\nOATH_EFFECTS={{{effects}}}\n"
    ))
}

const MOCK: &str = r#"
print=function() end
local function copy(v) if type(v)~='table' then return v end local t={} for k,x in pairs(v) do t[k]=copy(x) end return t end
local KINDS={'PRODUCTION','GOLD','SCIENCE','CULTURE','FAITH','FOOD'}
PROPS={}; ATTACHED={}; CONTEXT='core'; UI_CACHE={}; STUCK={}; TICKS=0
local function rows(list) return function() local i=0; return function() i=i+1; return list[i] end end end
GameInfo={Yields={},JAC_SengokuOaths=setmetatable({},{__call=rows(OATH_ROWS)}),JAC_SengokuOathAges=setmetatable({},{__call=rows(AGE_ROWS)}),
 Technologies=setmetatable({},{__call=rows({})}),Civics=setmetatable({},{__call=rows({})})}
for i,k in ipairs(KINDS) do GameInfo.Yields['YIELD_'..k]={Index=i-1} end
for _,r in ipairs(OATH_ROWS) do GameInfo.JAC_SengokuOaths[r.OathType]=r end
Locale={Lookup=function(s) return s end}
GameConfiguration={GetStartTurn=function() return 1 end}
-- Only string-keyed tables are stored, like every live-proven saved property.
local function keyed(v) if type(v)=='table' then for k,x in pairs(v) do assert(type(k)=='string','Game property table with a non-string key'); keyed(x) end end end
CoreGame={GetCurrentGameTurn=function() return 1 end,GetLocalPlayer=function() return 0 end,
 GetProperty=function(_,k) return copy(PROPS[k]) end,SetProperty=function(_,k,v) keyed(v); PROPS[k]=copy(v) end}
-- UI contexts read Game properties but cannot write them (live 18).
UIGame={GetCurrentGameTurn=CoreGame.GetCurrentGameTurn,GetLocalPlayer=CoreGame.GetLocalPlayer,GetProperty=CoreGame.GetProperty}
Game=CoreGame
UI={CanEndTurn=function() return true end,HasSentTurnComplete=function() return false end,IsProcessingMessages=function() return false end}
PLOTS={}; CITIES={}; Players={}; PlayerConfigurations={}; MAJORS={}
local function flag(plot,key) return (plot.props[key] or 0)>=1 end
local function anywhere(key) for _,plot in pairs(PLOTS) do if flag(plot,key) then return true end end return false end
local function other_oath(oath) for _,r in ipairs(OATH_ROWS) do if r.OathType~=oath and anywhere(r.OathType..'_ON') then return true end end return false end
-- Additive percentages on a base, quantized to 1/256 like the live GetYield.
function CORE_YIELD(city,k)
 local plot=PLOTS[city.plot]; local kind=KINDS[k]; local mine=ATTACHED[city.owner] or {}
 local base=1+((city.plot*7+k*13)%9)+0.25*((city.plot+k)%4)+(KNOB.corrupted==city.plot and 1 or 0)
 if mine['JAC_TODO134_BASE_'..kind] and flag(plot,'JAC_TODO134_BASE_'..kind) then base=base+6 end
 if mine['JAC_TODO134_SHIFT_'..kind] and flag(plot,'JAC_TODO134_SHIFT_ON') then base=base+1 end
 local pct=0
 if (kind=='PRODUCTION' or kind=='GOLD') and flag(plot,'JAC_AGE_GOLDEN') then pct=pct+10 end
 if city.owner~=0 and (kind=='SCIENCE' or kind=='CULTURE') then pct=pct+20 end
 if mine['JAC_TODO134_CAL_'..kind] and flag(plot,'JAC_TODO134_CAL_ON') then pct=pct+100 end
 if KNOB.meter_leak==city.plot and anywhere('JAC_TODO134_CAL_ON') then pct=pct+100 end
 if mine['JAC_TODO134_FINE_'..kind] and flag(plot,'JAC_TODO134_FINE_ON') then pct=pct+1 end
 if mine['JAC_TODO134_FINE_TWO_'..kind] and flag(plot,'JAC_TODO134_FINE_TWO_ON') then pct=pct+2 end
 for _,r in ipairs(OATH_ROWS) do
  local oath=r.OathType; local amount=OATH_EFFECTS[oath][string.lower(kind)]
  if amount then
   local on=flag(plot,oath..'_ON')
   if KNOB.sticky==oath and on then STUCK[city.plot]=true end
   if KNOB.sticky==oath and STUCK[city.plot] then on=true end
   if KNOB.leak==oath and city.plot==KNOB.victim and anywhere(oath..'_ON') then on=true end
   if on then
    if KNOB.wrong==oath then amount=amount+1 end
    if KNOB.joint==oath and other_oath(oath) then amount=amount*2 end
    pct=pct+amount
   end
  end
 end
 return math.floor(base*(100+pct)/100*256+0.5)/256
end
function REFRESH() for _,c in ipairs(CITIES) do local row={}; for k=1,6 do row[k]=CORE_YIELD(c,k) end; UI_CACHE[c.plot]=row end end
-- The UI cache follows Core on ticks only; a lagging UI misses every other tick.
function TICK() TICKS=TICKS+1; if KNOB.frozen or (KNOB.lag and TICKS%2==1) then return end; REFRESH() end
-- Growth as CityPanelOverview.lua:661-686 reads it: bonuses apply only while the
-- city grows; in a deficit the overall modifier carries no bonus. A side-effect
-- knob lets an oath's flag move one growth value as a faulty product would.
local function growth(city)
 local plot=PLOTS[city.plot]; local g={}
 local function side(key) return KNOB.side and KNOB.side.key==key and flag(plot,KNOB.side.oath..'_ON') end
 local function housing() return (city.plot%2==0) and 0.5 or 1 end
 local function happiness() return ((city.plot%3==0) and -15 or 10)+(side('happiness') and 5 or 0) end
 function g:GetFoodSurplus() if KNOB.fixed_surplus~=nil then return KNOB.fixed_surplus end; return city:GetYield(5)-((KNOB.consume or {})[city.plot] or 4) end
 function g:GetOtherGrowthModifier() return 0 end
 function g:GetHousingGrowthModifier() return housing() end
 function g:GetHappinessGrowthModifier() return happiness() end
 function g:GetLoyaltyGrowthModifier() return 1 end
 function g:GetOverallGrowthModifier()
  local overall=g:GetFoodSurplus()>0 and housing()*math.max(1+happiness()/100,0) or 1
  return overall+(side('growth') and 0.05 or 0)
 end
 return g
end
for pid=0,21 do
 local list={}
 for n=1,(pid<12 and 3 or 2) do
  local plot={index=1000+pid*10+n,props={}}
  function plot:GetIndex() return self.index end
  function plot:GetOwner() return pid end
  function plot:GetProperty(k) return self.props[k] end
  function plot:SetProperty(k,v)
   if k=='JAC_TODO134_FINE_TWO_ON' and KNOB.extra_flag_stick==self.index and self.props[k]==1 and v==0 then return end
   self.props[k]=v
   if k=='JAC_TODO134_FINE_TWO_ON' and v==1 and KNOB.extra_flag_leak then PLOTS[KNOB.extra_flag_leak].props[k]=1 end
  end
  PLOTS[plot.index]=plot
  local city={id=65536*n+pid,owner=pid,plot=plot.index}
  function city:GetID() return self.id end
  function city:GetOwner() return self.owner end
  function city:GetX() return self.plot end
  function city:GetY() return 0 end
  function city:GetYield(i) if CONTEXT=='ui' then return UI_CACHE[self.plot][i+1] end return CORE_YIELD(self,i+1) end
  function city:GetGrowth() return growth(self) end
  function city:IsOccupied() return false end
  function city:GetBuildQueue() return {GetCurrentProductionTypeHash=function() return 0 end} end
  list[#list+1]=city; CITIES[#CITIES+1]=city
 end
 local cities={}
 function cities:Members() local i=0; return function() i=i+1; if list[i] then return i,list[i] end end end
 function cities:GetCount() return #list end
 function cities:GetCapitalCity() return list[1] end
 local p={id=pid}
 function p:GetID() return self.id end
 function p:IsHuman() return self.id==0 end
 function p:IsTurnActive() return true end
 function p:IsTurnActiveComplete() return true end
 function p:GetCities() return cities end
 function p:AttachModifierByID(id) ATTACHED[self.id]=ATTACHED[self.id] or {}; ATTACHED[self.id][id]=true end
 function p:GetTechs() return {HasTech=function() return false end} end
 function p:GetCulture() return {HasCivic=function() return false end} end
 Players[pid]=p; MAJORS[#MAJORS+1]=p
 PlayerConfigurations[pid]={GetCivilizationTypeName=function() return CIVS[pid+1] end}
end
PlayerManager={GetAliveMajors=function() return MAJORS end}
Map={GetPlot=function(x) return PLOTS[x] end,GetPlotByIndex=function(i) return PLOTS[i] end}
-- WriteProperties of JAC_Sengoku_Ages.lua with the product's own JAC_Oaths_WritePlot.
function WRITE_PROPERTIES(l)
 for _,c in ipairs(CITIES) do
  local house=l.houses['p'..c.owner]
  JAC_Oaths_WritePlot(house,PLOTS[c.plot])
  PLOTS[c.plot]:SetProperty('JAC_AGE_GOLDEN',(house.age=='GOLDEN' or house.age=='HEROIC') and 1 or 0)
 end
end
function START_GAME()
 local l={version=1,reign=0,reignStart=1,houses={},cities={},allies={},friends={},keys={},history={}}
 for pid=0,21 do l.houses['p'..pid]={civ=CIVS[pid+1],score=0,parts={},golden=0,dark=0,age='NONE',research=0} end
 WRITE_PROPERTIES(l); PROPS.JAC_AGES_LEDGER=copy(l); REFRESH()
end
-- DedicationPopup entry points; confirming runs the product's gameplay choice handler.
function JAC_ShowOaths() local l=PROPS.JAC_AGES_LEDGER; local h=l.houses.p0; return h.oathReign==l.reign and h.oathChosen~=l.reign end
function JAC_GetOfferedOaths()
 local out={}
 for _,row in ipairs(JAC_Oaths_Choices(PROPS.JAC_AGES_LEDGER.houses.p0)) do out[#out+1]={type=row.OathType,name=row.Name,description=row.GoldenDescription} end
 return out
end
function JAC_ChooseOath(oath)
 local l=CoreGame:GetProperty('JAC_AGES_LEDGER')
 if not JAC_Oaths_Choose(l,0,{Reign=l.reign,Oath1=oath}) then return false end
 WRITE_PROPERTIES(l); CoreGame:SetProperty('JAC_AGES_LEDGER',l)
 if KNOB.rewrite then PLOTS[KNOB.rewrite.plot]:SetProperty(KNOB.rewrite.oath..'_ON',1) end
 return true
end
function SAVE()
 local plots={}; for index,plot in pairs(PLOTS) do plots[index]=copy(plot.props) end
 SAVED={props=copy(PROPS),plots=plots,attached=copy(ATTACHED)}
end
function LOAD()
 PROPS=copy(SAVED.props); ATTACHED=copy(SAVED.attached)
 for index,plot in pairs(PLOTS) do plot.props=copy(SAVED.plots[index]) end
 KNOB.corrupted=KNOB.corrupt
 REFRESH()
end
"#;

struct Game {
    vm: Lua,
    envs: BTreeMap<String, Table>,
}

impl Game {
    fn new(repo: &Repository, civs: &[String], knobs: &str) -> Result<Self> {
        let vm = Lua::new();
        vm.load(data_chunk(repo)?).exec()?;
        let civs: Vec<_> = civs.iter().map(|c| lua_string(c)).collect();
        vm.load(format!("CIVS={{{}}}; KNOB={{}}\n{knobs}", civs.join(",")))
            .exec()?;
        vm.load(MOCK).exec()?;
        vm.load(repo.read(ORACLE)?).set_name(ORACLE).exec()?;
        Ok(Game {
            vm,
            envs: BTreeMap::new(),
        })
    }

    /// One global table per Lua context; UI contexts get the read-only Game.
    fn run(&mut self, state: &str, code: &str) -> Result<String> {
        let globals = self.vm.globals();
        let core = state == "GameCore_Tuner";
        if !self.envs.contains_key(state) {
            let env = self.vm.create_table()?;
            let meta = self.vm.create_table()?;
            meta.set("__index", globals.clone())?;
            env.set_metatable(Some(meta))?;
            if !core {
                env.set("Game", globals.get::<Table>("UIGame")?)?;
            }
            self.envs.insert(state.to_owned(), env);
        }
        globals.set("CONTEXT", if core { "core" } else { "ui" })?;
        if !core {
            self.vm.load("TICK()").exec()?;
        }
        let value: LuaValue = self
            .vm
            .load(code)
            .set_environment(self.envs[state].clone())
            .eval()?;
        Ok(globals
            .get::<mlua::Function>("tostring")?
            .call::<String>(value)?)
    }

    fn step(&mut self, step: &Value) -> Result<String> {
        let name = || {
            let name = step["name"].as_str().unwrap_or_default();
            format!("{}-0123456789ab", &name[..name.len().min(67)])
        };
        Ok(match step["action"].as_str().context("action")? {
            "restart_game" => "Civ VI relaunched via Steam; main menu visible and ready".into(),
            "enable_mods" | "configure" => "operation completed".into(),
            "start_game" => {
                self.vm.load("START_GAME()").exec()?;
                "single-player game reached InGame".into()
            }
            "ui_lua" if step["state_name"] == "HostGame" => {
                "ran Lua in HostGame; return=audit setup built".into()
            }
            "ui_lua" => {
                let state = step["state_name"].as_str().context("state")?;
                let value = self.run(state, step["code"].as_str().context("code")?)?;
                format!("ran Lua in {state}; return={value}")
            }
            "wait_for" => {
                let condition = &step["condition"];
                let state = condition["state_name"].as_str().context("state")?;
                let code = condition["code"].as_str().context("code")?;
                let mut met = false;
                for _ in 0..4 {
                    if self.run(state, code)? == "true" {
                        met = true;
                        break;
                    }
                }
                ensure!(met, "Timed out waiting for {condition}");
                format!("condition met: {condition}")
            }
            "save" => {
                self.vm.load("SAVE()").exec()?;
                format!("C:\\Users\\agent\\Saves\\Single\\{}.Civ6Save", name())
            }
            "exit_to_menu" => {
                self.envs.clear();
                "main menu reached".into()
            }
            "load" => {
                self.vm.load("LOAD()").exec()?;
                self.envs.clear();
                format!("loaded {}", name())
            }
            other => bail!("unexpected action {other}"),
        })
    }
}

/// Shipping prepends a restart and suffixes the save name, as live_chain does.
fn shipped(probe: &Probe) -> Result<Value> {
    let mut scenario = probe.scenario()?;
    let steps = scenario["steps"].as_array_mut().context("steps")?;
    steps.insert(0, json!({"action":"restart_game","timeout":420}));
    for step in steps
        .iter_mut()
        .filter(|s| s["action"] == "save" || s["action"] == "load")
    {
        step["name"] = json!(format!(
            "{}-agent-01",
            step["name"].as_str().unwrap_or_default()
        ));
    }
    Ok(scenario)
}

fn civs(scenario: &Value) -> Result<Vec<String>> {
    scenario["steps"]
        .as_array()
        .context("steps")?
        .iter()
        .find(|s| s["action"] == "configure")
        .context("configure")?["players"]
        .as_array()
        .context("players")?
        .iter()
        .map(|p| p["civ"].as_str().map(str::to_owned).context("civ"))
        .collect()
}

struct Run {
    dir: tempfile::TempDir,
    scenario: Value,
    failure: Option<String>,
    game: Game,
}

/// Executes every step like the runner and writes scenario/result/steps files.
fn simulate(probe: &Probe, knobs: &str) -> Result<Run> {
    let repo = Repository::checked_in();
    let scenario = shipped(probe)?;
    let mut game = Game::new(&repo, &civs(&scenario)?, knobs)?;
    let steps = scenario["steps"].as_array().context("steps")?;
    let mut rows = vec![json!({"action":"start","status":"OK","detail":scenario["name"]})];
    let (mut failure, mut completed) = (None, 0);
    for (i, step) in steps.iter().enumerate() {
        let action = format!(
            "{}/{} {}",
            i + 1,
            steps.len(),
            step["action"].as_str().unwrap_or_default()
        );
        rows.push(json!({"action":action,"status":"START","detail":serde_json::to_string(step)?}));
        match game.step(step) {
            Ok(detail) => {
                rows.push(json!({"action":action,"status":"OK","detail":detail}));
                completed += 1;
            }
            Err(error) => {
                rows.push(json!({"action":action,"status":"FAIL","detail":format!("{error:#}")}));
                failure = Some(format!("{error:#}"));
                break;
            }
        }
    }
    if failure.is_none() {
        rows.push(json!({"action":"judge","status":"OK","detail":"5/5 checks passed"}));
        rows.push(json!({"action":"diagnostic","status":"OBSERVED","detail":"observation complete; acceptance not evaluated; exit 3"}));
    }
    let dir = tempfile::tempdir()?;
    fs::write(
        dir.path().join("scenario.json"),
        serde_json::to_string_pretty(&scenario)?,
    )?;
    let result = json!({"scenario":scenario["name"],"passed":false,"completed_steps":completed,"failure":failure,
        "checks":[{"name":"Generic: JAC Runtime Error","passed":true}],"diagnostic_only":true,"observation_completed":failure.is_none(),"acceptance_passed":false});
    fs::write(
        dir.path().join("result.json"),
        serde_json::to_string(&result)?,
    )?;
    write_rows(dir.path(), &rows)?;
    Ok(Run {
        dir,
        scenario,
        failure,
        game,
    })
}

fn first() -> Result<Probe> {
    Probe::new(&Repository::checked_in(), 1)
}

/// First mocked city plot of a house in the probe's configured player order.
fn first_plot(probe: &Probe, civ: &str) -> Result<i64> {
    let pid = civs(&probe.scenario()?)?
        .iter()
        .position(|c| c == civ)
        .context("house")?;
    Ok(1001 + 10 * pid as i64)
}

/// Oda has no yield-only oath, so its cities are controls in every batch.
fn control_plot(probe: &Probe) -> Result<i64> {
    first_plot(probe, "CIVILIZATION_JAC_ODA")
}

/// The mocked plot of a mocked city id (id 65536*n+player, plot 1000+10*player+n).
fn plot_of_city(city: i64) -> i64 {
    1000 + 10 * (city % 65536) + city / 65536
}

fn records(run: &Run) -> Result<Vec<Value>> {
    fs::read_to_string(run.dir.path().join("steps.jsonl"))?
        .lines()
        .map(|line| Ok(serde_json::from_str(line)?))
        .collect()
}

/// Index of the record for a generated role (shifted by the shipped restart).
fn record(run: &Run, probe: &Probe, role: Role, status: &str) -> Result<usize> {
    let steps = run.scenario["steps"].as_array().context("steps")?;
    let at = probe.position(role)? + 1;
    let label = format!(
        "{}/{} {}",
        at + 1,
        steps.len(),
        steps[at]["action"].as_str().unwrap_or_default()
    );
    records(run)?
        .iter()
        .position(|r| r["action"] == label.as_str() && r["status"] == status)
        .context("record")
}

/// Writes edited records, expects the verifier to reject them for `reason`, restores.
fn rejects(
    run: &Run,
    edit: impl FnOnce(&mut Vec<Value>) -> Result<()>,
    reason: &str,
) -> Result<()> {
    let original = records(run)?;
    let mut rows = original.clone();
    edit(&mut rows)?;
    write_rows(run.dir.path(), &rows)?;
    let outcome = verify(&Repository::checked_in(), run.dir.path());
    write_rows(run.dir.path(), &original)?;
    let error = format!(
        "{:#}",
        outcome.err().context(format!("accepted: {reason}"))?
    );
    assert!(error.contains(reason), "{reason}: {error}");
    Ok(())
}

fn replace(row: &mut Value, from: &str, to: &str) {
    let detail = row["detail"]
        .as_str()
        .unwrap_or_default()
        .replacen(from, to, 1);
    row["detail"] = json!(detail);
}

/// Rewrites the body of both save frames of one side alike, keeping each frame's
/// marker, length and terminator consistent, so only content checks can reject.
fn edit_frames(
    rows: &mut [Value],
    run: &Run,
    probe: &Probe,
    core: bool,
    edit: &dyn Fn(&str) -> String,
) -> Result<()> {
    for after in [false, true] {
        let role = if core {
            Role::CoreFrame { after }
        } else {
            Role::UiFrame { after }
        };
        let at = record(run, probe, role, "OK")?;
        let detail = rows[at]["detail"].as_str().context("frame")?.to_owned();
        let (head, tail) = detail.split_at(detail.find(":{").context("frame body")?);
        let prefix = &head[..head.rfind('|').context("frame marker")? + 1];
        let body = edit(tail[1..].strip_suffix(END).context("frame end")?);
        rows[at]["detail"] = json!(format!("{prefix}{}:{body}{END}", body.len()));
    }
    Ok(())
}

/// Edits the JSON object a driver record returned, keeping its context head.
fn edit_return(row: &mut Value, edit: impl FnOnce(&mut Value)) -> Result<()> {
    let detail = row["detail"].as_str().context("detail")?.to_owned();
    let (head, body) = detail.split_once("; return=").context("return")?;
    let mut value: Value = serde_json::from_str(body)?;
    edit(&mut value);
    row["detail"] = json!(format!("{head}; return={value}"));
    Ok(())
}

/// The scalar after `marker` changes from a number to a numeric string.
fn retype_after(body: &str, marker: &str) -> String {
    let start = body.find(marker).expect("marker") + marker.len();
    let at = start + body[start..].find(":number:").expect("number");
    format!("{}:string:{}", &body[..at], &body[at + 8..])
}

/// The first digit of the number after `marker` changes, keeping its length.
fn bump_after(body: &str, marker: &str) -> String {
    let start = body.find(marker).expect("marker") + marker.len();
    let at = start + body[start..].find(":number:").expect("number") + 8;
    let digit = if body.as_bytes()[at] == b'9' {
        '8'
    } else {
        (body.as_bytes()[at] + 1) as char
    };
    format!("{}{digit}{}", &body[..at], &body[at + 1..])
}

fn reasons(case: &Value) -> Vec<String> {
    case["reasons"]
        .as_array()
        .map(|r| {
            r.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn batches_cover_each_yield_only_oath_once_with_distinct_houses() -> Result<()> {
    let repo = Repository::checked_in();
    let all = eligible(&repo)?;
    let mut seen = BTreeSet::new();
    let mut sizes = Vec::new();
    for index in 1..=BATCHES {
        let cases = batch(&repo, index)?;
        let houses: BTreeSet<_> = cases.iter().map(|c| c.house.as_str()).collect();
        assert_eq!(houses.len(), cases.len());
        assert!(cases.iter().all(|c| seen.insert(c.oath.clone())));
        sizes.push(cases.len());
    }
    assert_eq!(sizes, [13, 13, 12]);
    assert_eq!(seen.len(), all.len());
    // Independent source: the audit inventory of all 66. Only yield-only oaths batch.
    let inventory = crate::oath_save_probe::catalogue(&repo)?;
    assert_eq!(inventory.len(), 66);
    for entry in &inventory {
        let kinds: Vec<_> = entry["effects"]
            .as_array()
            .context("effects")?
            .iter()
            .filter_map(|e| e["kind"].as_str())
            .collect();
        let yield_only = kinds.len() == 2 && kinds.iter().all(|k| Yield::parse(k).is_some());
        assert_eq!(
            seen.contains(entry["oath"].as_str().unwrap_or_default()),
            yield_only,
            "{}",
            entry["oath"]
        );
    }
    for case in &all {
        let entry = inventory
            .iter()
            .find(|e| e["oath"] == case.oath.as_str())
            .context("inventory")?;
        assert_eq!(entry["civilization"], case.civ.as_str());
        for ((kind, amount), effect) in case
            .effects
            .iter()
            .zip(entry["effects"].as_array().context("effects")?)
        {
            assert_eq!(
                (effect["kind"].as_str(), effect["expected_percent"].as_i64()),
                (Some(kind.key()), Some(*amount))
            );
        }
    }
    assert!(batch(&repo, 0).is_err() && batch(&repo, 4).is_err());
    Ok(())
}

#[test]
fn generated_batches_are_one_launch_one_save_and_compile_in_lua51() -> Result<()> {
    let repo = Repository::checked_in();
    run(&repo, &["--check".into()])?;
    for index in 1..=BATCHES {
        let probe = Probe::new(&repo, index)?;
        let scenario = probe.scenario()?;
        assert_eq!(scenario["diagnostic_only"], true);
        assert_eq!(scenario["finite_turns"], 0);
        assert_eq!(scenario["batch_probe"]["task_accepted"], false);
        let steps = scenario["steps"].as_array().context("steps")?;
        for (action, count) in [
            ("start_game", 1),
            ("save", 1),
            ("exit_to_menu", 1),
            ("load", 1),
            ("autoplay", 0),
            ("end_turn", 0),
        ] {
            assert_eq!(
                steps.iter().filter(|s| s["action"] == action).count(),
                count,
                "{action}"
            );
        }
        let text = scenario.to_string();
        assert!(!text.contains("ACTION_ENDTURN"));
        for placeholder in [
            "__BATCH__",
            "__CASES__",
            "__PHASE__",
            "__ON__",
            "__FINE__",
            "__SHIFT__",
            "__CAL__",
            "__BASE__",
            "__TARGETS__",
            "__FLAG__",
            "__ORDER__",
            "__OATH__",
            "__CIV__",
            "__BEFORE__",
            "__COUNT__",
        ] {
            assert!(!text.contains(placeholder), "{placeholder}");
        }
        let players = steps[1]["players"].as_array().context("players")?;
        assert_eq!(players[0]["civ"], probe.cases[0].civ.as_str());
        assert_eq!(players.iter().filter(|p| p["human"] == true).count(), 1);
        // Every phase: Core snapshot, UI wait, UI measurement, in that order.
        for phase in schedule(probe.cases.len()) {
            let core = probe.position(Role::Snapshot(phase))?;
            assert_eq!(probe.position(Role::Synced(phase))?, core + 1);
            assert_eq!(probe.position(Role::Measure(phase))?, core + 2);
        }
        let compare = probe.position(Role::Compare)?;
        assert!(probe.position(Role::Measure(Phase::AllOn))? < compare);
        assert!(probe.position(Role::Popup)? < probe.position(Role::LedgerChoice)?);
        // UI frames only after the UI shows the cleared Core state, before and after the reload.
        let saved = probe.position(Role::Synced(Phase::Saved))?;
        assert!(compare < saved && saved + 1 == probe.position(Role::CoreFrame { after: false })?);
        assert_eq!(probe.position(Role::UiFrame { after: false })?, saved + 2);
        let loaded = probe.position(Role::Synced(Phase::Loaded))?;
        assert_eq!(probe.position(Role::CoreFrame { after: true })?, loaded - 2);
        assert_eq!(probe.position(Role::UiFrame { after: true })?, loaded + 1);
        assert!(probe.position(Role::Load)? < loaded);
    }
    Ok(())
}

#[test]
fn reported_step_matches_the_audit_lua() -> Result<()> {
    let vm = Lua::new();
    for values in [
        vec![7.8984375, 15.796875, 7.9765625, 8.984375],
        vec![0.0, 7.0, 17.0, 26.0],
        vec![100.0, 110.0000000001, 120.0000000002],
        vec![7.0, 7.0, 7.0],
        vec![1.0, f64::NAN, 2.0],
        vec![1.0, 2.0],
        vec![12.21875, 24.4296875, 12.3359375, 13.4375],
        vec![3.1, 3.2, 3.3, 4.7],
    ] {
        let list: Vec<_> = values
            .iter()
            .map(|v| {
                if v.is_nan() {
                    "0/0".to_owned()
                } else {
                    format!("{v:?}")
                }
            })
            .collect();
        let lua: Option<f64> = vm
            .load(format!(
                "{LUA_HELPERS}\nreturn reported_step({{{}}})",
                list.join(",")
            ))
            .eval()?;
        assert_eq!(
            lua.map(f64::to_bits),
            reported_step(&values).map(f64::to_bits),
            "{values:?}"
        );
    }
    Ok(())
}

#[test]
fn independent_extra_observation_can_break_the_live_coarse_lattice() -> Result<()> {
    // Original live38 city 196610 culture observations, in their recorded order.
    let off = 6.6015625;
    let reference = 14.09765625;
    let one = 6.67578125;
    let shifted = 7.56640625;
    let old = reported_step(&[off, reference, one, shifted]).context("old step")?;
    assert_eq!(old, 19.0 / 256.0 + 0.00002);
    // A possible *new observation*, 37 native ticks above OFF. This is a
    // diagnostic fixture, not a claim about what the next game run will return.
    let extra = off + 37.0 / 256.0;
    let refined = reported_step(&[off, reference, one, shifted, extra]).context("new step")?;
    assert_eq!(refined, 1.0 / 256.0 + 0.00002);
    let hundred = reference - off;
    let tolerance = 2.0 * refined * 1.05 + 0.00004;
    assert!(2.0 * old * 1.05 + 0.00004 >= hundred * 0.005);
    assert!(tolerance < hundred * 0.005);
    assert!(((6.9765625 - off) - hundred * 0.05).abs() <= tolerance);
    assert!(((6.9765625 - off) - hundred * 0.06).abs() > tolerance);
    let vm = Lua::new();
    let lua: f64 = vm
        .load(format!(
            "{LUA_HELPERS}\nreturn reported_step({{{off},{reference},{one},{shifted},{extra}}})"
        ))
        .eval()?;
    assert_eq!(lua.to_bits(), refined.to_bits());
    Ok(())
}

#[test]
fn refined_batch_uses_all_six_new_samples_and_keeps_case_failures() -> Result<()> {
    let repo = Repository::checked_in();
    let probe = Probe::with_resolution(&repo, 2, true)?;
    let phases = schedule_for(probe.cases.len(), true);
    assert_eq!(phases[3], Phase::PrecisionFineTwo);
    assert_eq!(phases.len(), schedule(probe.cases.len()).len() + 1);
    let scenario = probe.scenario()?;
    assert_eq!(scenario["batch_probe"]["refine_resolution"], true);
    assert_eq!(scenario["batch_probe"]["task_accepted"], false);
    for step in scenario["steps"].as_array().context("steps")? {
        if let Some(code) = step["code"]
            .as_str()
            .or_else(|| step["condition"]["code"].as_str())
        {
            Lua::new().load(code).into_function()?;
        }
    }
    let attach = scenario["steps"]
        .as_array()
        .context("steps")?
        .iter()
        .filter_map(|s| s["code"].as_str())
        .find(|code| code.contains(ATTACH_FINE_TWO))
        .context("refined attachment")?;
    assert!(attach
        .contains("for _,kind in ipairs(KINDS) do p:AttachModifierByID('JAC_TODO134_FINE_TWO_'"));
    let run = simulate(&probe, "")?;
    assert_eq!(run.failure, None);
    let report = verify(&repo, run.dir.path())?;
    assert_eq!(report["verified_count"], probe.cases.len());
    assert_eq!(report["task_accepted"], false);
    assert_eq!(report["accepted_all_66"], false);
    let wrong = simulate(
        &probe,
        &format!("KNOB.wrong={}", lua_string(&probe.cases[2].oath)),
    )?;
    assert_eq!(wrong.failure, None);
    let wrong_report = verify(&repo, wrong.dir.path())?;
    assert!(reasons(&wrong_report["cases"][2])
        .iter()
        .any(|reason| reason.starts_with("quantity:")));
    assert_eq!(
        wrong_report["cases"][2]["quantity_and_native_save_verified"],
        false
    );
    Ok(())
}

#[test]
fn refined_metadata_phase_code_and_observation_are_required() -> Result<()> {
    let repo = Repository::checked_in();
    let probe = Probe::with_resolution(&repo, 2, true)?;
    let run = simulate(&probe, "")?;
    assert_eq!(run.failure, None);
    verify(&repo, run.dir.path())?;
    let path = run.dir.path().join("scenario.json");
    for edit in 0..4 {
        let mut scenario = run.scenario.clone();
        match edit {
            0 => {
                scenario["batch_probe"]
                    .as_object_mut()
                    .context("metadata")?
                    .remove("refine_resolution");
            }
            1 => scenario["batch_probe"]["refine_resolution"] = json!(false),
            2 => scenario["batch_probe"]["phases"][3] = json!("precision_off"),
            _ => {
                let at = probe.position(Role::Measure(Phase::PrecisionFineTwo))? + 1;
                let code = scenario["steps"][at]["code"].as_str().context("code")?;
                ensure!(code.contains("fine_two=1"), "refined state missing");
                scenario["steps"][at]["code"] = json!(code.replacen("fine_two=1", "fine_two=0", 1));
            }
        }
        fs::write(&path, serde_json::to_string(&scenario)?)?;
        assert!(
            verify(&repo, run.dir.path()).is_err(),
            "scenario mutation {edit}"
        );
    }
    fs::write(&path, serde_json::to_string(&run.scenario)?)?;
    let at = record(&run, &probe, Role::Measure(Phase::PrecisionFineTwo), "OK")?;
    rejects(
        &run,
        |rows| {
            rows.remove(at);
            Ok(())
        },
        "driver record",
    )?;
    rejects(
        &run,
        |rows| {
            edit_return(&mut rows[at], |value| {
                value["cities"][0]["culture"] = json!(9999.0);
            })
        },
        "Core and UI observations differ",
    )?;
    verify(&repo, run.dir.path())?;
    Ok(())
}

#[test]
fn refined_meter_rejects_control_leaks_and_failed_restoration() -> Result<()> {
    let repo = Repository::checked_in();
    let probe = Probe::with_resolution(&repo, 2, true)?;
    let control = control_plot(&probe)?;
    let subject = first_plot(&probe, &probe.cases[0].civ)?;
    for (knob, plot) in [("extra_flag_leak", control), ("extra_flag_stick", subject)] {
        let run = simulate(&probe, &format!("KNOB.{knob}={plot}"))?;
        let failure = run.failure.context("refined meter fault was accepted")?;
        assert!(
            failure.contains("diagnostic meter property leaked or not restored"),
            "{failure}"
        );
        assert!(verify(&repo, run.dir.path()).is_err());
    }
    Ok(())
}

#[test]
fn every_batch_verifies_each_oath_from_one_run_and_one_native_save() -> Result<()> {
    let repo = Repository::checked_in();
    for index in 1..=BATCHES {
        let probe = Probe::new(&repo, index)?;
        let run = simulate(&probe, "")?;
        assert_eq!(run.failure, None);
        let report = verify(&repo, run.dir.path())?;
        assert_eq!(report["verified_count"], probe.cases.len(), "{report:#}");
        assert_eq!(report["native_save_frames_equal"], true);
        assert_eq!(report["task_accepted"], false);
        assert_eq!(report["accepted_all_66"], false);
        assert_eq!(
            report["saved_name"],
            format!("JAC-Todo134-Batch-{index}-agent-01-0123456789ab").as_str()
        );
        let cases = report["cases"].as_array().context("cases")?;
        assert_eq!(cases[0]["selection_path"], "real_ui_selection");
        assert_eq!(cases[0]["player"], 0);
        for case in &cases[1..] {
            assert_eq!(case["selection_path"], "ledger_fixture_selection");
            assert!(case["remaining"]
                .as_array()
                .context("remaining")?
                .contains(&json!("official_ui_selection")));
        }
        for case in cases {
            assert_eq!(case["quantity_and_native_save_verified"], true);
            let results = case["results"].as_array().context("results")?;
            assert!(
                !results.is_empty()
                    && results.iter().all(|r| r["quantity_verified"] == true
                        && r["reason"] == "native_100_percent_reference_delta")
            );
        }
        let remaining = report["remaining"].as_array().context("remaining")?;
        assert!(remaining.contains(&json!("natural_age_transition")));
        assert!(remaining.contains(&json!("official_ui_selection_for_ledger_fixture_houses")));
    }
    Ok(())
}

#[test]
fn a_lagging_ui_is_waited_for_and_a_frozen_ui_fails_the_run() -> Result<()> {
    let probe = first()?;
    let lagging = simulate(&probe, "KNOB.lag=true")?;
    assert_eq!(lagging.failure, None);
    assert_eq!(
        verify(&Repository::checked_in(), lagging.dir.path())?["verified_count"],
        probe.cases.len()
    );
    let frozen = simulate(&probe, "KNOB.frozen=true")?;
    assert!(frozen
        .failure
        .as_deref()
        .is_some_and(|f| f.contains("Timed out waiting")));
    assert!(verify(&Repository::checked_in(), frozen.dir.path()).is_err());
    Ok(())
}

/// A fault in one oath fails that oath only, with the same reasons in Lua and Rust.
#[test]
fn case_faults_fail_only_their_oath_with_matching_lua_and_rust_reasons() -> Result<()> {
    let probe = first()?;
    let target = &probe.cases[2].oath;
    let control = control_plot(&probe)?;
    for (knob, expected) in [
        (format!("KNOB.wrong='{target}'"), vec!["quantity:"]),
        (format!("KNOB.sticky='{target}'"), vec!["restore:"]),
        (
            format!("KNOB.leak='{target}'; KNOB.victim={control}"),
            vec!["leak:", "simultaneous:"],
        ),
        (format!("KNOB.joint='{target}'"), vec!["simultaneous:"]),
    ] {
        let run = simulate(&probe, &knob)?;
        assert_eq!(run.failure, None, "{knob}");
        let report = verify(&Repository::checked_in(), run.dir.path())?;
        assert_eq!(
            report["verified_count"],
            probe.cases.len() - 1,
            "{knob}: {report:#}"
        );
        for (i, case) in report["cases"]
            .as_array()
            .context("cases")?
            .iter()
            .enumerate()
        {
            assert_eq!(case["reasons"], case["in_game_reasons"], "{knob}");
            let reasons: Vec<_> = case["reasons"]
                .as_array()
                .context("reasons")?
                .iter()
                .filter_map(Value::as_str)
                .collect();
            if i == 2 {
                assert_eq!(case["quantity_and_native_save_verified"], false);
                for prefix in &expected {
                    assert!(
                        reasons.iter().any(|r| r.starts_with(prefix)),
                        "{knob}: {reasons:?}"
                    );
                }
                assert!(case["remaining"]
                    .as_array()
                    .context("remaining")?
                    .contains(&json!("quantity_and_native_save")));
                assert!(report["remaining"]
                    .as_array()
                    .context("remaining")?
                    .contains(&json!(format!("quantity_and_native_save:{target}"))));
            } else {
                assert!(reasons.is_empty(), "{knob}: case {i} {reasons:?}");
            }
        }
    }
    Ok(())
}

#[test]
fn calibration_ui_flag_ledger_and_save_faults_fail_the_whole_run() -> Result<()> {
    let probe = first()?;
    let control = control_plot(&probe)?;
    let illegal = &probe.cases[1].oath;
    let first_ai_plot = first_plot(&probe, &probe.cases[1].civ)?;
    for (knob, reason) in [
        (format!("KNOB.meter_leak={control}"), "diagnostic meter leaked to non-batch city"),
        ("KNOB.corrupt=1001".to_owned(), "ledger, history, flags or real yields changed"),
        (format!("for i=#AGE_ROWS,1,-1 do if AGE_ROWS[i].OathType=='{illegal}' and AGE_ROWS[i].AgeType=='GOLDEN' then table.remove(AGE_ROWS,i) end end"), "not a legal golden choice"),
        (format!("KNOB.rewrite={{plot={first_ai_plot},oath='{illegal}'}}"), "Core oath flags differ from phase on_1"),
    ] {
        let run = simulate(&probe, &knob)?;
        assert!(run.failure.as_deref().is_some_and(|f| f.contains(reason)), "{knob}: {:?}", run.failure);
        assert!(verify(&Repository::checked_in(), run.dir.path()).is_err(), "{knob}");
    }
    Ok(())
}

/// A growth side effect fails its oath. A food oath may move overall growth only
/// across the deficit boundary, where the official modifier stops applying.
#[test]
fn growth_side_effects_fail_their_oath_and_food_may_cross_the_deficit() -> Result<()> {
    let repo = Repository::checked_in();
    let probe = first()?;
    let food = |case: usize| {
        probe.cases[case]
            .effects
            .iter()
            .any(|(k, _)| *k == Yield::Food)
    };
    let (plain, fed, crossing) = (3, 2, 11);
    assert!(!food(plain) && food(fed) && food(crossing));
    for (target, key, expected) in [
        (plain, "happiness", vec![":happiness", ":growth"]),
        (plain, "growth", vec![":growth"]),
        (fed, "growth", vec![":growth"]),
    ] {
        let oath = &probe.cases[target].oath;
        let run = simulate(&probe, &format!("KNOB.side={{oath='{oath}',key='{key}'}}"))?;
        assert_eq!(run.failure, None, "{key}");
        let report = verify(&repo, run.dir.path())?;
        assert_eq!(
            report["verified_count"],
            probe.cases.len() - 1,
            "{key}: {report:#}"
        );
        let case = &report["cases"][target];
        assert_eq!(case["reasons"], case["in_game_reasons"], "{key}");
        let found = reasons(case);
        for suffix in expected {
            assert!(
                found
                    .iter()
                    .any(|r| r.starts_with("non_target:") && r.ends_with(suffix)),
                "{key} {suffix}: {found:?}"
            );
        }
    }
    // Consumption between the OFF and ON food of one city: OFF starves, ON grows.
    let clean = verify(&repo, simulate(&probe, "")?.dir.path())?;
    let line = clean["cases"][crossing]["results"]
        .as_array()
        .context("results")?
        .iter()
        .find(|r| r["kind"] == "food")
        .context("food line")?
        .clone();
    let city = line["city"].as_i64().context("city")?;
    let consume = (line["off"].as_f64().context("off")? + line["on"].as_f64().context("on")?) / 2.0;
    let run = simulate(
        &probe,
        &format!("KNOB.consume={{[{}]={consume:?}}}", plot_of_city(city)),
    )?;
    assert_eq!(run.failure, None);
    let report = verify(&repo, run.dir.path())?;
    assert_eq!(report["verified_count"], probe.cases.len(), "{report:#}");
    // The case really crossed: overall growth moved with the food surplus sign.
    let rows = records(&run)?;
    let sample = |phase: Phase| -> Result<Value> {
        let detail = rows[record(&run, &probe, Role::Measure(phase), "OK")?]["detail"]
            .as_str()
            .context("measure")?
            .to_owned();
        let data: Value = serde_json::from_str(detail.split_once("return=").context("return")?.1)?;
        data["cities"]
            .as_array()
            .context("cities")?
            .iter()
            .find(|c| c["city"] == city)
            .cloned()
            .context("city")
    };
    let (prev, on) = (
        sample(Phase::Off(crossing - 1))?,
        sample(Phase::On(crossing))?,
    );
    assert!(prev["food_surplus"].as_f64() < Some(0.0) && on["food_surplus"].as_f64() > Some(0.0));
    assert_ne!(prev["growth"], on["growth"]);
    Ok(())
}

#[test]
fn food_oaths_reject_growth_side_effects_when_both_samples_are_nonpositive() -> Result<()> {
    let repo = Repository::checked_in();
    let probe = first()?;
    let target = 2;
    assert!(probe.cases[target]
        .effects
        .iter()
        .any(|(kind, _)| *kind == Yield::Food));
    let clean = verify(&repo, simulate(&probe, "")?.dir.path())?;
    let food_rows: Vec<_> = clean["cases"][target]["results"]
        .as_array()
        .context("food results")?
        .iter()
        .filter(|row| row["kind"] == "food")
        .collect();
    let oath = &probe.cases[target].oath;
    for mode in ["negative", "on_zero", "both_zero"] {
        let knobs = if mode == "both_zero" {
            "KNOB.fixed_surplus=0;".to_owned()
        } else {
            let consume = food_rows
                .iter()
                .map(|row| {
                    Ok(format!(
                        "[{}]={:.17}",
                        plot_of_city(row["city"].as_i64().context("city")?),
                        row["on"].as_f64().context("on food")?
                            + if mode == "negative" { 10.0 } else { 0.0 }
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            format!("KNOB.consume={{{}}};", consume.join(","))
        };
        let run = simulate(
            &probe,
            &format!("{knobs} KNOB.side={{oath='{oath}',key='growth'}}"),
        )?;
        assert_eq!(run.failure, None, "{mode}");
        let report = verify(&repo, run.dir.path())?;
        assert_eq!(
            report["verified_count"],
            probe.cases.len() - 1,
            "{mode}: {report:#}"
        );
        assert!(
            reasons(&report["cases"][target])
                .iter()
                .any(|reason| reason.starts_with("non_target:") && reason.ends_with(":growth")),
            "{mode}"
        );
    }
    Ok(())
}

#[test]
fn saved_control_civilization_must_match_its_configured_player_slot() -> Result<()> {
    let probe = first()?;
    assert!(probe
        .cases
        .iter()
        .all(|case| case.civ != "CIVILIZATION_JAC_ODA"));
    let run = simulate(&probe, "")?;
    rejects(
        &run,
        |rows| {
            edit_frames(rows, &run, &probe, true, &|body| {
                body.replace("CIVILIZATION_JAC_ODA", "CIVILIZATION_JAC_ODZ")
            })
        },
        "saved civilizations differ from the configured 22 player slots",
    )?;
    Ok(())
}

/// Every numbered record maps to exactly one generated operation; only the start
/// record, a relaunch before the first step, UI-recovery notes inside a step, the
/// judge and the diagnostic exit 3 may surround them.
#[test]
fn run_log_admits_only_the_generated_operations() -> Result<()> {
    let probe = first()?;
    let run = simulate(&probe, "")?;
    let repo = Repository::checked_in();
    verify(&repo, run.dir.path())?;
    let total = run.scenario["steps"].as_array().context("steps")?.len();
    let save = record(&run, &probe, Role::Save, "OK")?;
    let compare = record(&run, &probe, Role::Compare, "START")?;
    let unexpected = "unexpected, missing or duplicated driver record";
    rejects(
        &run,
        |rows| {
            for status in ["OK", "START"] {
                let label = format!("{}/{} load", total + 1, total + 1);
                rows.insert(
                    save + 1,
                    json!({"action":label,"status":status,"detail":"loaded other"}),
                );
            }
            Ok(())
        },
        unexpected,
    )?;
    rejects(
        &run,
        |rows| {
            let (start, ok) = (rows[save - 1].clone(), rows[save].clone());
            rows.insert(save + 1, ok);
            rows.insert(save + 1, start);
            Ok(())
        },
        unexpected,
    )?;
    rejects(
        &run,
        |rows| {
            rows.insert(
                compare,
                json!({"action":"hotseat_staging","status":"OK","detail":"x"}),
            );
            Ok(())
        },
        unexpected,
    )?;
    rejects(
        &run,
        |rows| {
            rows.insert(
                compare + 1,
                json!({"action":"ingame_unblock","status":"UNKNOWN","detail":"x"}),
            );
            Ok(())
        },
        unexpected,
    )?;
    rejects(
        &run,
        |rows| {
            rows.push(json!({"action":"judge","status":"OK","detail":"5/5 checks passed"}));
            Ok(())
        },
        "after the diagnostic observation",
    )?;
    rejects(
        &run,
        |rows| {
            rows[0]["detail"] = json!("another scenario");
            Ok(())
        },
        "start record of this scenario",
    )?;
    rejects(
        &run,
        |rows| {
            let at = rows
                .iter()
                .position(|r| r["action"] == "judge")
                .context("judge")?;
            rows.remove(at);
            Ok(())
        },
        "judge record",
    )?;
    rejects(
        &run,
        |rows| {
            rows.pop();
            Ok(())
        },
        "exit 3",
    )?;
    rejects(
        &run,
        |rows| {
            rows.swap(compare, compare + 1);
            Ok(())
        },
        unexpected,
    )?;
    rejects(
        &run,
        |rows| {
            rows[compare]["status"] = json!("FAIL");
            Ok(())
        },
        "failed driver observation retained",
    )?;
    rejects(
        &run,
        |rows| {
            let mut started: Value =
                serde_json::from_str(rows[compare]["detail"].as_str().context("detail")?)?;
            started["code"] = json!("return true");
            rows[compare]["detail"] = json!(started.to_string());
            Ok(())
        },
        "executed step does not match",
    )?;
    // A relaunch before the first step and a UI-recovery note inside a step are no operations.
    let original = records(&run)?;
    let mut rows = original.clone();
    rows.insert(compare + 1, json!({"action":"ingame_unblock","status":"OK","detail":"closed X after an InGame timeout; retry 1/3"}));
    rows.insert(
        1,
        json!({"action":"launch","status":"OK","detail":"Civ VI relaunched"}),
    );
    rows.insert(
        1,
        json!({"action":"launch","status":"START","detail":"Civ VI is not running"}),
    );
    write_rows(run.dir.path(), &rows)?;
    let admitted = verify(&repo, run.dir.path());
    write_rows(run.dir.path(), &original)?;
    assert_eq!(admitted?["verified_count"], probe.cases.len());
    Ok(())
}

/// Both save frames are read field by field: exact keys and types, the 66
/// catalogue flags with exactly the chosen flag on, the ledger with 22 houses and
/// its history, the same cities and yields on both sides and in the clean-state
/// Core snapshots, and one turn throughout.
#[test]
fn save_frames_are_typed_complete_and_match_each_other_and_the_clean_state() -> Result<()> {
    let probe = first()?;
    let run = simulate(&probe, "")?;
    let repo = Repository::checked_in();
    verify(&repo, run.dir.path())?;
    let chosen = probe.cases[3].oath.clone();
    let unknown = format!("{}X", &chosen[..chosen.len() - 1]);
    let other_city = "the UI frame shows other cities or yields than the Core frame";
    let selected_reason = format!("the selected flag of {chosen} is not saved");
    type Edit = Box<dyn Fn(&str) -> String>;
    let swap =
        |from: String, to: String| -> Edit { Box::new(move |b: &str| b.replacen(&from, &to, 1)) };
    let cases: Vec<(bool, bool, Edit, &str)> = vec![
        // (core side, UI side, edit, reason)
        (
            true,
            false,
            swap(
                format!("{chosen}8:number:1"),
                format!("{unknown}8:number:0"),
            ),
            "saved flags differ from the 66 catalogue oaths",
        ),
        (
            true,
            false,
            swap(
                "JAC_OATH_ODA_AZUCHI8:number:0".into(),
                "JAC_OATH_ODA_AZUCHX8:number:0".into(),
            ),
            "saved flags differ from the 66 catalogue oaths",
        ),
        (
            true,
            false,
            swap(format!("{chosen}8:number:1"), format!("{chosen}8:number:0")),
            selected_reason.as_str(),
        ),
        (
            true,
            false,
            swap(
                "JAC_OATH_ODA_AZUCHI8:number:0".into(),
                "JAC_OATH_ODA_AZUCHI8:number:1".into(),
            ),
            "outside its house's choice",
        ),
        (
            true,
            false,
            Box::new(|b: &str| retype_after(b, "JAC_OATH_ODA_AZUCHI")),
            "saved oath flag must be a number",
        ),
        (
            true,
            false,
            swap("14:string:history".into(), "14:string:histXry".into()),
            "ledger lacks history",
        ),
        (
            true,
            false,
            swap("12:string:start".into(), "12:string:stArt".into()),
            "Core frame needs exactly the keys",
        ),
        (
            true,
            false,
            swap(
                "11:string:turn8:number:1".into(),
                "11:string:turn8:number:2".into(),
            ),
            "disagree on the turn",
        ),
        (
            true,
            false,
            Box::new(|b: &str| retype_after(b, "17:string:PRODUCTION")),
            "Core yields must be a number",
        ),
        (
            true,
            false,
            swap("12:string:FAITH".into(), "12:string:FAITX".into()),
            "Core yields needs exactly the keys",
        ),
        (
            true,
            false,
            swap("10:string:p21".into(), "10:string:q21".into()),
            "house key must be p",
        ),
        (
            true,
            false,
            swap("11:string:1001".into(), "11:string:1009".into()),
            other_city,
        ),
        (
            true,
            false,
            Box::new(|b: &str| bump_after(b, "9:string:id")),
            other_city,
        ),
        (
            false,
            true,
            swap(
                "31:string:GetOverallGrowthModifier".into(),
                "31:string:GetOverallGrowthModifiex".into(),
            ),
            "UI growth needs exactly the keys",
        ),
        (
            false,
            true,
            Box::new(|b: &str| retype_after(b, "11:string:hash")),
            "UI queue hash must be a number",
        ),
        (
            false,
            true,
            swap(
                "11:string:turn8:number:1".into(),
                "11:string:turn8:number:2".into(),
            ),
            "disagree on the turn",
        ),
        (
            false,
            true,
            Box::new(|b: &str| bump_after(b, "9:string:id")),
            other_city,
        ),
        (
            false,
            true,
            Box::new(|b: &str| bump_after(b, "17:string:PRODUCTION")),
            other_city,
        ),
        (
            false,
            true,
            swap("12:string:FAITH".into(), "12:string:FAITX".into()),
            "UI yields needs exactly the keys",
        ),
        // The same gap or the same other city on both sides.
        (
            true,
            true,
            swap("12:string:FAITH".into(), "12:string:FAITX".into()),
            "Core yields needs exactly the keys",
        ),
        (
            true,
            true,
            swap("11:string:1001".into(), "11:string:1009".into()),
            "the save frames differ from the observed clean state",
        ),
    ];
    for (core, ui, edit, reason) in &cases {
        rejects(
            &run,
            |rows| {
                if *core {
                    edit_frames(rows, &run, &probe, true, edit.as_ref())?;
                }
                if *ui {
                    edit_frames(rows, &run, &probe, false, edit.as_ref())?;
                }
                Ok(())
            },
            reason,
        )?;
    }
    // The clean-state and per-phase Core snapshots are evidence too.
    let saved = record(&run, &probe, Role::Snapshot(Phase::Saved), "OK")?;
    let loaded = record(&run, &probe, Role::Snapshot(Phase::Loaded), "OK")?;
    let on = record(&run, &probe, Role::Snapshot(Phase::On(2)), "OK")?;
    let measure = record(&run, &probe, Role::Measure(Phase::On(1)), "OK")?;
    type Change = fn(&mut Value);
    let snapshot_cases: Vec<(usize, Change, &str)> = vec![
        (
            saved,
            |v| {
                v["cities"][0]["production"] =
                    json!(v["cities"][0]["production"].as_f64().unwrap_or_default() + 1.0)
            },
            "the save frames differ from the observed clean state",
        ),
        (
            loaded,
            |v| {
                v["cities"][0]["city"] =
                    json!(v["cities"][0]["city"].as_i64().unwrap_or_default() + 1)
            },
            "the save frames differ from the observed clean state",
        ),
        (saved, |v| v["turn"] = json!(2), "disagree on the turn"),
        (
            on,
            |v| {
                v["cities"][0]["production"] =
                    json!(v["cities"][0]["production"].as_f64().unwrap_or_default() + 1.0)
            },
            "Core and UI observations differ in on_3",
        ),
        (
            on,
            |v| v["cities"][0]["food"] = json!("6"),
            "Core snapshot food must be a finite number",
        ),
        (
            on,
            |v| v["cities"][0]["extra"] = json!(1),
            "Core snapshot city needs exactly the fields",
        ),
        (
            measure,
            |v| v["cities"][0]["gold"] = json!("1.5"),
            "measured gold must be a finite number",
        ),
        (
            measure,
            |v| v["cities"][0]["note"] = json!(true),
            "measured city needs exactly the fields",
        ),
    ];
    for (at, change, reason) in snapshot_cases {
        rejects(&run, |rows| edit_return(&mut rows[at], change), reason)?;
    }
    Ok(())
}

/// The verifier recomputes from the samples, so neither a changed sample nor an
/// optimistic in-game verdict or quantity line can make an oath pass alone.
#[test]
fn recorded_samples_and_lines_are_recomputed_and_disagreement_fails_closed() -> Result<()> {
    let probe = first()?;
    let run = simulate(&probe, "")?;
    let repo = Repository::checked_in();
    let clean = verify(&repo, run.dir.path())?;
    let compared = record(&run, &probe, Role::Compare, "OK")?;
    let original = records(&run)?;
    let check = |rows: &[Value], case: usize| -> Result<()> {
        write_rows(run.dir.path(), rows)?;
        let report = verify(&repo, run.dir.path());
        write_rows(run.dir.path(), &original)?;
        let report = report?;
        assert_eq!(
            report["cases"][case]["quantity_and_native_save_verified"],
            false
        );
        assert!(reasons(&report["cases"][case]).contains(&"lua_rust_disagreement".to_owned()));
        assert_eq!(report["verified_count"], probe.cases.len() - 1);
        Ok(())
    };
    // Core and UI agree on a changed sample the in-game comparison never saw.
    let kind = probe.cases[2].effects[0].0.key();
    let player = clean["cases"][2]["player"].as_i64().context("player")?;
    let mut rows = original.clone();
    for role in [Role::Measure(Phase::On(2)), Role::Snapshot(Phase::On(2))] {
        edit_return(&mut rows[record(&run, &probe, role, "OK")?], |data| {
            if let Some(city) = data["cities"]
                .as_array_mut()
                .and_then(|c| c.iter_mut().find(|c| c["owner"] == player))
            {
                city[kind] = json!(city[kind].as_f64().unwrap_or_default() + 0.5);
            }
        })?;
    }
    check(&rows, 2)?;
    // Quantity lines must equal the Rust recomputation field by field.
    type Change = fn(&mut Value);
    let lines: [Change; 6] = [
        |case| case["results"] = json!([]),
        |case| {
            let t = case["results"][0]["tolerance"].as_f64().unwrap_or_default();
            case["results"][0]["tolerance"] = json!(t * 2.0)
        },
        |case| case["results"][0]["quantity_verified"] = json!(false),
        |case| case["results"][0]["note"] = json!("extra"),
        |case| case["results"][0]["off"] = json!("1"),
        |case| {
            let first = case["results"][0].clone();
            case["results"]
                .as_array_mut()
                .into_iter()
                .for_each(|r| r.push(first.clone()))
        },
    ];
    for change in lines {
        let mut rows = original.clone();
        edit_return(&mut rows[compared], |data| change(&mut data["cases"][0]))?;
        check(&rows, 0)?;
    }
    // An in-game failure the samples do not show still fails the oath.
    let mut rows = original.clone();
    edit_return(&mut rows[compared], |data| {
        data["cases"][0]["quantity_verified"] = json!(false);
        data["cases"][0]["reasons"] = json!(["leak:1:gold"]);
        data["quantity_verified_count"] = json!(probe.cases.len() - 1);
    })?;
    check(&rows, 0)?;
    // A verdict that contradicts its own reasons is not evidence at all.
    rejects(
        &run,
        |rows| {
            edit_return(&mut rows[compared], |data| {
                data["cases"][0]["quantity_verified"] = json!(false)
            })
        },
        "contradicts its reasons",
    )?;
    // The report carries the Rust lines, equal to the in-game ones here.
    let line = &clean["cases"][0]["results"][0];
    assert_eq!(line["reason"], "native_100_percent_reference_delta");
    assert!(DETAIL_KEYS.iter().all(|k| line.get(*k).is_some()));
    Ok(())
}

/// A saved ledger that is internally consistent but not the batch is rejected.
#[test]
fn saved_ledger_rejects_missing_or_foreign_choices() -> Result<()> {
    let probe = first()?;
    let human = &probe.cases[0].oath;
    let civs = civs(&probe.scenario()?)?;
    let ai = civs
        .iter()
        .position(|c| *c == probe.cases[1].civ)
        .context("house")?;
    let oda = civs
        .iter()
        .position(|c| c == "CIVILIZATION_JAC_ODA")
        .context("Oda")?;
    for (mutation, reason) in [
        (
            format!("l.houses.p{ai}.oathChosen=2"),
            "lacks its saved golden choice",
        ),
        (
            format!("l.houses.p{ai}.oathReign=2"),
            "lacks its saved golden choice",
        ),
        (
            format!("l.houses.p{ai}.age='HEROIC'"),
            "lacks its saved golden choice",
        ),
        (
            format!("l.houses.p{oda}.oaths={{JAC_OATH_ODA_TENKA_FUBU=true}}"),
            "a house outside the batch has a saved oath",
        ),
        (
            format!("l.houses.p{oda}.oathChosen=1"),
            "a house outside the batch has a saved oath",
        ),
    ] {
        let run = simulate(&probe, "")?;
        // The same self-consistent Core frame before and after: flags follow the ledger.
        run.game
            .vm
            .load(format!("CONTEXT='core'; PROPS.JAC_TODO134_SAVE_PROBE=nil; local l=PROPS.JAC_AGES_LEDGER\n{mutation}\nPROPS.JAC_AGES_LEDGER=l; for _,c in ipairs(CITIES) do JAC_Oaths_WritePlot(l.houses['p'..c.owner],PLOTS[c.plot]) end"))
            .exec()?;
        let text: String = run.game.vm.load(core_code(human, false)).eval()?;
        let body = text
            .split_once(':')
            .and_then(|(_, rest)| rest.strip_suffix(END))
            .context("frame")?
            .to_owned();
        rejects(
            &run,
            |rows| edit_frames(rows, &run, &probe, true, &|_| body.clone()),
            reason,
        )?;
    }
    Ok(())
}

#[test]
fn verifier_rejects_altered_names_scenario_result_and_missing_scenario() -> Result<()> {
    let probe = first()?;
    let run = simulate(&probe, "")?;
    let repo = Repository::checked_in();
    verify(&repo, run.dir.path())?;
    let save = record(&run, &probe, Role::Save, "OK")?;
    let load = record(&run, &probe, Role::Load, "OK")?;
    let core_after = record(&run, &probe, Role::CoreFrame { after: true }, "OK")?;
    let ui_before = record(&run, &probe, Role::UiFrame { after: false }, "OK")?;
    rejects(
        &run,
        |rows| {
            replace(
                &mut rows[core_after],
                "13:string:GOLDEN",
                "13:string:GOLDEn",
            );
            Ok(())
        },
        "Core ledger, history, flags or yields changed",
    )?;
    rejects(
        &run,
        |rows| {
            replace(&mut rows[ui_before], "number:1", "number:2");
            Ok(())
        },
        "UI yields or growth changed",
    )?;
    rejects(
        &run,
        |rows| {
            replace(&mut rows[core_after], "|END-JAC-OATH-SAVE", "");
            Ok(())
        },
        "frame terminator missing",
    )?;
    rejects(
        &run,
        |rows| {
            replace(&mut rows[core_after], "|JAC_OATH_", "|9JAC_OATH_");
            Ok(())
        },
        "frame missing",
    )?;
    rejects(
        &run,
        |rows| {
            replace(&mut rows[ui_before], ":{", "0:{");
            Ok(())
        },
        "incomplete frame return",
    )?;
    rejects(
        &run,
        |rows| {
            rows[save]["detail"] = json!("C:\\Saves\\JAC-Todo134-Batch-1-agent-01.Civ6Save");
            Ok(())
        },
        "recorded save name does not follow",
    )?;
    rejects(
        &run,
        |rows| {
            rows[load]["detail"] = json!("loaded other-save");
            Ok(())
        },
        "load OK record does not name",
    )?;
    let scenario_path = run.dir.path().join("scenario.json");
    let original = fs::read_to_string(&scenario_path)?;
    for (from, to, reason) in [
        (
            "\"diagnostic_only\": true",
            "\"diagnostic_only\": false",
            "artifact scope changed",
        ),
        (
            "\"task_accepted\": false",
            "\"task_accepted\": true",
            "artifact scope changed",
        ),
        (
            "JAC-Todo134-Batch-1-agent-01\"",
            "JAC-Todo134-Batch-1-agent-02\"",
            "save/load names differ",
        ),
        (
            "B.compared=true",
            "B.compared=false",
            "artifact does not contain the generated batch scenario",
        ),
    ] {
        fs::write(&scenario_path, original.replacen(from, to, 1))?;
        let outcome = verify(&repo, run.dir.path());
        fs::write(&scenario_path, &original)?;
        let error = format!("{:#}", outcome.err().context(from)?);
        assert!(error.contains(reason), "{from}: {error}");
    }
    let result_path = run.dir.path().join("result.json");
    let result = fs::read_to_string(&result_path)?;
    for (from, to) in [
        ("\"passed\":false", "\"passed\":true"),
        ("\"acceptance_passed\":false", "\"acceptance_passed\":true"),
        (
            "\"observation_completed\":true",
            "\"observation_completed\":false",
        ),
    ] {
        fs::write(&result_path, result.replacen(from, to, 1))?;
        let outcome = verify(&repo, run.dir.path());
        fs::write(&result_path, &result)?;
        assert!(outcome.is_err(), "{from}");
    }
    verify(&repo, run.dir.path())?;
    fs::remove_file(&scenario_path)?;
    let error = format!("{:#}", verify(&repo, run.dir.path()).unwrap_err());
    assert!(
        error.contains("lacks the executed scenario.json"),
        "{error}"
    );
    Ok(())
}

#[test]
fn cli_writes_new_files_only_and_rejects_bad_arguments() -> Result<()> {
    let repo = Repository::checked_in();
    let dir = tempfile::tempdir()?;
    let path = |name: &str| dir.path().join(name).to_string_lossy().into_owned();
    run(&repo, &["--output".into(), path("one.json")])?;
    let written: Value = serde_json::from_str(&fs::read_to_string(path("one.json"))?)?;
    assert_eq!(written, first()?.scenario()?);
    assert!(run(&repo, &["--output".into(), path("one.json")]).is_err());
    run(
        &repo,
        &[
            "--batch".into(),
            "3".into(),
            "--output".into(),
            path("three.json"),
        ],
    )?;
    let third: Value = serde_json::from_str(&fs::read_to_string(path("three.json"))?)?;
    assert_eq!(third["batch_probe"]["batch"], 3);
    run(
        &repo,
        &[
            "--refine-resolution".into(),
            "--batch".into(),
            "2".into(),
            "--output".into(),
            path("refined.json"),
        ],
    )?;
    let refined: Value = serde_json::from_str(&fs::read_to_string(path("refined.json"))?)?;
    assert_eq!(refined, Probe::with_resolution(&repo, 2, true)?.scenario()?);
    assert_eq!(refined["batch_probe"]["refine_resolution"], true);
    assert!(run(
        &repo,
        &[
            "--batch".into(),
            "4".into(),
            "--output".into(),
            path("four.json")
        ]
    )
    .is_err());
    assert!(run(&repo, &["--verify".into(), path("")]).is_err());
    assert!(run(&repo, &[]).is_err());
    assert!(run(&repo, &["--check".into(), "--output".into()]).is_err());
    Ok(())
}
