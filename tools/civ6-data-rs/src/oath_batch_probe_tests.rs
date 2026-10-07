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
local function growth(city)
 local g={}
 function g:GetOverallGrowthModifier() return 1 end
 function g:GetOtherGrowthModifier() return 0 end
 function g:GetHousingGrowthModifier() return 1 end
 function g:GetHappinessGrowthModifier() return 0 end
 function g:GetFoodSurplus() return city:GetYield(5)-4 end
 function g:GetLoyaltyGrowthModifier() return 1 end
 return g
end
for pid=0,21 do
 local list={}
 for n=1,(pid<12 and 3 or 2) do
  local plot={index=1000+pid*10+n,props={}}
  function plot:GetIndex() return self.index end
  function plot:GetOwner() return pid end
  function plot:GetProperty(k) return self.props[k] end
  function plot:SetProperty(k,v) self.props[k]=v end
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

fn rejects(run: &Run, edit: impl FnOnce(&mut Vec<Value>), reason: &str) -> Result<()> {
    let original = records(run)?;
    let mut rows = original.clone();
    edit(&mut rows);
    write_rows(run.dir.path(), &rows)?;
    let error = format!(
        "{:#}",
        verify(&Repository::checked_in(), run.dir.path()).unwrap_err()
    );
    assert!(error.contains(reason), "{reason}: {error}");
    write_rows(run.dir.path(), &original)?;
    Ok(())
}

fn replace(row: &mut Value, from: &str, to: &str) {
    let detail = row["detail"]
        .as_str()
        .unwrap_or_default()
        .replacen(from, to, 1);
    row["detail"] = json!(detail);
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

#[test]
fn verifier_rejects_altered_records_frames_names_and_missing_scenario() -> Result<()> {
    let probe = first()?;
    let run = simulate(&probe, "")?;
    let repo = Repository::checked_in();
    verify(&repo, run.dir.path())?;
    let save = record(&run, &probe, Role::Save, "OK")?;
    let load = record(&run, &probe, Role::Load, "OK")?;
    let core_after = record(&run, &probe, Role::CoreFrame { after: true }, "OK")?;
    let ui_before = record(&run, &probe, Role::UiFrame { after: false }, "OK")?;
    let compare_start = record(&run, &probe, Role::Compare, "START")?;
    rejects(
        &run,
        |rows| {
            replace(
                &mut rows[core_after],
                "13:string:GOLDEN",
                "13:string:GOLDEn",
            )
        },
        "Core ledger, history, flags or yields changed",
    )?;
    rejects(
        &run,
        |rows| replace(&mut rows[ui_before], "number:1", "number:2"),
        "UI yields or growth changed",
    )?;
    rejects(
        &run,
        |rows| replace(&mut rows[core_after], "|END-JAC-OATH-SAVE", ""),
        "frame terminator missing",
    )?;
    rejects(
        &run,
        |rows| replace(&mut rows[core_after], "|JAC_OATH_", "|9JAC_OATH_"),
        "frame missing",
    )?;
    rejects(
        &run,
        |rows| {
            let d = rows[ui_before]["detail"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            let (head, tail) = d.split_once("|batch-1|").unwrap_or_default();
            let (length, body) = tail.split_once(':').unwrap_or_default();
            rows[ui_before]["detail"] = json!(format!(
                "{head}|batch-1|{}:{body}",
                length.parse::<usize>().unwrap_or_default() + 1
            ));
        },
        "incomplete frame return",
    )?;
    rejects(
        &run,
        |rows| rows[save]["detail"] = json!("C:\\Saves\\JAC-Todo134-Batch-1-agent-01.Civ6Save"),
        "recorded save name does not follow",
    )?;
    rejects(
        &run,
        |rows| rows[load]["detail"] = json!("loaded other-save"),
        "load OK record does not name",
    )?;
    rejects(
        &run,
        |rows| {
            let row = rows[compare_start].clone();
            rows.insert(compare_start, row);
        },
        "missing or duplicated driver record",
    )?;
    rejects(
        &run,
        |rows| rows.swap(compare_start, compare_start + 1),
        "driver record order differs",
    )?;
    rejects(
        &run,
        |rows| rows[compare_start]["status"] = json!("FAIL"),
        "failed driver observation retained",
    )?;
    rejects(
        &run,
        |rows| {
            rows.pop();
        },
        "exit 3",
    )?;
    rejects(
        &run,
        |rows| {
            let mut started: Value =
                serde_json::from_str(rows[compare_start]["detail"].as_str().unwrap_or_default())
                    .unwrap_or_default();
            started["code"] = json!("return true");
            rows[compare_start]["detail"] = json!(started.to_string());
        },
        "executed step does not match",
    )?;
    // The scenario and result files are evidence too.
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
        let error = format!("{:#}", verify(&repo, run.dir.path()).unwrap_err());
        assert!(error.contains(reason), "{from}: {error}");
    }
    fs::write(&scenario_path, &original)?;
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
        assert!(verify(&repo, run.dir.path()).is_err(), "{from}");
    }
    fs::write(&result_path, &result)?;
    verify(&repo, run.dir.path())?;
    fs::remove_file(&scenario_path)?;
    let error = format!("{:#}", verify(&repo, run.dir.path()).unwrap_err());
    assert!(
        error.contains("lacks the executed scenario.json"),
        "{error}"
    );
    Ok(())
}

/// The verifier recomputes from the samples, so neither a changed sample nor an
/// optimistic in-game verdict can make an oath pass alone.
#[test]
fn recorded_samples_are_recomputed_and_disagreement_fails_closed() -> Result<()> {
    let probe = first()?;
    let run = simulate(&probe, "")?;
    let repo = Repository::checked_in();
    let measured = record(&run, &probe, Role::Measure(Phase::On(2)), "OK")?;
    let compared = record(&run, &probe, Role::Compare, "OK")?;
    let original = records(&run)?;
    let mut rows = original.clone();
    let detail = rows[measured]["detail"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let mut sample: Value = serde_json::from_str(
        detail
            .strip_prefix("ran Lua in InGame; return=")
            .unwrap_or_default(),
    )?;
    let kind = probe.cases[2].effects[0].0.key();
    let report = verify(&repo, run.dir.path())?;
    let player = report["cases"][2]["player"].as_i64().context("player")?;
    let city = sample["cities"]
        .as_array_mut()
        .context("cities")?
        .iter_mut()
        .find(|c| c["owner"] == player)
        .context("own city")?;
    city[kind] = json!(city[kind].as_f64().unwrap_or_default() + 0.5);
    rows[measured]["detail"] = json!(format!("ran Lua in InGame; return={sample}"));
    write_rows(run.dir.path(), &rows)?;
    let changed = verify(&repo, run.dir.path())?;
    let case = &changed["cases"][2];
    assert_eq!(case["quantity_and_native_save_verified"], false);
    assert!(case["reasons"]
        .as_array()
        .context("reasons")?
        .contains(&json!("lua_rust_disagreement")));
    assert_eq!(changed["verified_count"], probe.cases.len() - 1);
    // An in-game verdict that contradicts its own reasons is not evidence.
    let mut rows = original.clone();
    replace(
        &mut rows[compared],
        "\"quantity_verified\":true,\"reasons\":[]",
        "\"quantity_verified\":false,\"reasons\":[]",
    );
    write_rows(run.dir.path(), &rows)?;
    assert!(format!("{:#}", verify(&repo, run.dir.path()).unwrap_err())
        .contains("contradicts its reasons"));
    // An in-game failure the samples do not show still fails the oath.
    let mut rows = original.clone();
    replace(
        &mut rows[compared],
        "\"quantity_verified\":true,\"reasons\":[]",
        "\"quantity_verified\":false,\"reasons\":[\"leak:1:gold\"]",
    );
    replace(
        &mut rows[compared],
        "\"quantity_verified_count\":13",
        "\"quantity_verified_count\":12",
    );
    write_rows(run.dir.path(), &rows)?;
    let report = verify(&repo, run.dir.path())?;
    assert_eq!(
        report["cases"][0]["quantity_and_native_save_verified"],
        false
    );
    assert_eq!(report["verified_count"], probe.cases.len() - 1);
    Ok(())
}

/// The saved frame must show every batch choice and nothing else, read in Rust.
#[test]
fn saved_frame_rejects_missing_or_foreign_choices() -> Result<()> {
    let probe = first()?;
    let run = simulate(&probe, "")?;
    let human = &probe.cases[0].oath;
    let ai = &probe.cases[1];
    let civs = civs(&run.scenario)?;
    let ai_pid = civs.iter().position(|c| *c == ai.civ).context("house")?;
    let oda = civs
        .iter()
        .position(|c| c == "CIVILIZATION_JAC_ODA")
        .context("Oda")?;
    let oda_oath = "JAC_OATH_ODA_TENKA_FUBU";
    // A self-consistent saved state: the ledger edit is mirrored into every flag.
    let frame_of = |game: &Game, mutation: &str| -> Result<String> {
        game.vm.load(format!("CONTEXT='core'; PROPS.JAC_TODO134_SAVE_PROBE=nil; local l=PROPS.JAC_AGES_LEDGER\n{mutation}\nPROPS.JAC_AGES_LEDGER=l; for _,c in ipairs(CITIES) do JAC_Oaths_WritePlot(l.houses['p'..c.owner],PLOTS[c.plot]) end")).exec()?;
        let text: String = game.vm.load(core_code(human, false)).eval()?;
        Ok(text
            .split_once(':')
            .and_then(|(_, rest)| rest.strip_suffix(END))
            .context("frame")?
            .to_owned())
    };
    let clean = frame_of(&run.game, "")?;
    saved_selections(&clean, &probe.cases)?;
    for (mutation, reason) in [
        (
            format!("l.houses.p{ai_pid}.oaths={{}}"),
            "lacks its saved golden choice",
        ),
        (
            format!("l.houses.p{ai_pid}.age='NORMAL'"),
            "lacks its saved golden choice",
        ),
        (
            format!("l.houses.p{oda}.age='GOLDEN'; l.houses.p{oda}.oaths={{{oda_oath}=true}}"),
            "a house outside the batch has a saved oath",
        ),
    ] {
        let game = simulate(&probe, "")?.game;
        let frame = frame_of(&game, &mutation)?;
        let error = format!("{:#}", saved_selections(&frame, &probe.cases).unwrap_err());
        assert!(error.contains(reason), "{mutation}: {error}");
    }
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
