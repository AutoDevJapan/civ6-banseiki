//! Extend a numeric oath scenario with a real save/reload. UI observations are
//! compared from immutable driver returns, never retained in a UI Lua cache.
use anyhow::{bail, ensure, Context, Result};
use civ6_data_rs::Repository;
use serde_json::{json, Value};
use std::{collections::BTreeSet, fs, io::Write, path::Path};

const INVENTORY: &str = "documents/poc/data/oath-effect-scenarios.json";
const KEY: &str = "JAC_TODO134_SAVE_PROBE";
const HELPERS: &str = r#"
local function ser(v)
 local kind=type(v)
 if kind=='table' then
  local entries={}
  for k,value in pairs(v) do entries[#entries+1]=ser(k)..ser(value) end
  table.sort(entries); return '{'..table.concat(entries)..'}'
 end
 assert(kind=='nil' or kind=='boolean' or kind=='string' or kind=='number','unsupported snapshot value')
 if kind=='number' then assert(v==v and v~=math.huge and v~=-math.huge,'nonfinite snapshot value') end
 local text=kind=='number' and string.format('%.17g',v) or tostring(v)
 local s=kind..':'..text; return #s..':'..s
end
local function number(v)
 assert(type(v)=='number' and v==v and v~=math.huge and v~=-math.huge,'numeric getter unavailable')
 return v
end
local function inventory()
 local majors=PlayerManager.GetAliveMajors(); assert(#majors==22,'expected 22 real houses')
 local cities={}; local houses={}
 for _,p in ipairs(majors) do
  local id=p:GetID(); assert(not houses[id],'duplicate real house'); houses[id]=true
  for _,c in p:GetCities():Members() do
   local plot=assert(Map.GetPlot(c:GetX(),c:GetY()))
   assert(c:GetOwner()==id and plot:GetOwner()==id,'owner/plot mismatch')
   local key=tostring(plot:GetIndex()); assert(not cities[key],'duplicate city plot')
   cities[key]={object=c,plot=plot,id=c:GetID(),owner=id}
  end
 end
 local count=0; for _ in pairs(cities) do count=count+1 end
 assert(count==56,'expected original 56 real cities')
 return cities
end
"#;

fn core_code(oath: &str, verify: bool) -> String {
    format!(
        r#"{HELPERS}
local oath={oath:?}; local key='{KEY}'
local l=assert(Game:GetProperty('JAC_AGES_LEDGER'))
local human=assert(l.houses.p0)
assert(l.reign==1 and human.oathChosen==1 and human.oaths[oath]==true,'actual selected oath missing')
local houseCount=0; for _ in pairs(l.houses) do houseCount=houseCount+1 end
assert(houseCount==22,'saved house inventory changed')
local catalogue={{}}; for row in GameInfo.JAC_SengokuOaths() do catalogue[#catalogue+1]=row end
assert(#catalogue==66,'expected 66 loaded oaths')
local cities={{}}
for plotKey,r in pairs(inventory()) do
 local h=assert(l.houses['p'..r.owner]); local c=r.object; local plot=r.plot
 local row={{id=r.id,owner=r.owner,flags={{}},yields={{}}}}
 for _,entry in ipairs(catalogue) do
  local flag=plot:GetProperty(entry.OathType..'_ON')
  local expected=h.civ==entry.CivilizationType and (h.age=='GOLDEN' or h.age=='HEROIC') and h.oaths and h.oaths[entry.OathType]==true
  assert(flag==(expected and 1 or 0),'saved oath flag disagrees with actual house')
  row.flags[entry.OathType]=flag
 end
 for _,kind in ipairs({{'PRODUCTION','GOLD','SCIENCE','CULTURE','FAITH','FOOD'}}) do
  row.yields[kind]=number(c:GetYield(GameInfo.Yields['YIELD_'..kind].Index))
  assert((plot:GetProperty('JAC_TODO134_BASE_'..kind) or 0)==0,'diagnostic input still active')
 end
 for _,property in ipairs({{'JAC_TODO134_CAL_ON','JAC_TODO134_FINE_ON','JAC_TODO134_SHIFT_ON','JAC_TODO134_SHIFT_TWO_ON','JAC_TODO134_HEADROOM_ON'}}) do
  assert((plot:GetProperty(property) or 0)==0,'diagnostic modifier still active')
 end
 cities[plotKey]=row
end
local now=ser({{oath=oath,turn=Game.GetCurrentGameTurn(),ledger=l,cities=cities}})
if {verify} then
 assert(type(Game:GetProperty(key))=='string' and Game:GetProperty(key)==now,'ledger, history, flags or real yields changed across native save/load')
 return 'JAC_OATH_SAVE_CORE_AFTER|'..oath..'|'..#now..':'..now..'|END-JAC-OATH-SAVE'
end
assert(Game:GetProperty(key)==nil,'save probe may not replace an existing before snapshot')
Game:SetProperty(key,now)
return 'JAC_OATH_SAVE_CORE_BEFORE|'..oath..'|'..#now..':'..now..'|END-JAC-OATH-SAVE'
"#,
        verify = if verify { "true" } else { "false" }
    )
}

fn ui_code(oath: &str, targets: &[String], after: bool) -> String {
    let targets = targets
        .iter()
        .map(|s| format!("{s:?}"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{HELPERS}
local oath={oath:?}; local cities={{}}
if {before} then assert(JAC_OATH_AUDIT and JAC_OATH_AUDIT.accepted==true,'numeric acceptance must precede native save') end
assert(Players[0]:IsTurnActive() and Players[0]:IsTurnActiveComplete(),'human start not complete')
assert(UI.CanEndTurn() and not UI.HasSentTurnComplete() and not UI.IsProcessingMessages(),'human UI not ready')
for plotKey,r in pairs(inventory()) do
 local c=r.object; local g=assert(c:GetGrowth()); local q=assert(c:GetBuildQueue())
 local row={{id=r.id,owner=r.owner,yields={{}},growth={{}},queue={{hash=q:GetCurrentProductionTypeHash(),targets={{}}}}}}
 for _,kind in ipairs({{'PRODUCTION','GOLD','SCIENCE','CULTURE','FAITH','FOOD'}}) do
  row.yields[kind]=number(c:GetYield(GameInfo.Yields['YIELD_'..kind].Index))
 end
 for _,method in ipairs({{'GetOverallGrowthModifier','GetOtherGrowthModifier','GetHousingGrowthModifier','GetHappinessGrowthModifier','GetFoodSurplus','GetLoyaltyGrowthModifier'}}) do
  row.growth[method]=number(g[method](g))
 end
 for _,target in ipairs({{{targets}}}) do
  local unit=GameInfo.Units[target]; local building=GameInfo.Buildings[target]
  assert(unit or building,'queue target definition missing')
  row.queue.targets[target]=unit and {{progress=number(q:GetUnitProgress(unit.Index)),cost=number(q:GetUnitCost(unit.Index))}} or {{progress=number(q:GetBuildingProgress(building.Index)),cost=number(q:GetBuildingCost(building.Index))}}
 end
 cities[plotKey]=row
end
local signature=ser({{turn=Game.GetCurrentGameTurn(),cities=cities}})
return 'JAC_OATH_SAVE_UI_{phase}|'..oath..'|'..#signature..':'..signature..'|END-JAC-OATH-SAVE'
"#,
        phase = if after { "AFTER" } else { "BEFORE" },
        before = if after { "false" } else { "true" }
    )
}

fn lua(code: String, state: &str) -> Value {
    json!({"action":"ui_lua","state_name":state,"code":code,"timeout":30})
}

fn suffix(oath: &str, targets: &[String]) -> Vec<Value> {
    let name = format!("JAC-Todo134-Save-{oath}");
    vec![
        lua(core_code(oath, false), "GameCore_Tuner"),
        lua(ui_code(oath, targets, false), "InGame"),
        json!({"action":"save","name":name,"timeout":180}),
        json!({"action":"exit_to_menu","timeout":180}),
        json!({"action":"load","name":name,"timeout":600}),
        json!({"action":"wait_for","timeout":60,"close_diplomacy":true,"condition":{"type":"lua","state_name":"InGame","code":"return Players[0]:IsTurnActive() and Players[0]:IsTurnActiveComplete() and UI.CanEndTurn() and not UI.HasSentTurnComplete() and not UI.IsProcessingMessages()"}}),
        lua(core_code(oath, true), "GameCore_Tuner"),
        lua(ui_code(oath, targets, true), "InGame"),
    ]
}

fn make_probe(repo: &Repository, entry: &Value) -> Result<Value> {
    let oath = entry["oath"].as_str().context("catalogue oath")?;
    ensure!(
        oath.starts_with("JAC_OATH_") && oath.bytes().all(|b| b.is_ascii_uppercase() || b == b'_'),
        "invalid oath key"
    );
    let targets = entry["production_targets"]
        .as_array()
        .context("production targets")?
        .iter()
        .map(|v| {
            v["target"]
                .as_str()
                .map(str::to_owned)
                .context("target key")
        })
        .collect::<Result<Vec<_>>>()?;
    let mut source = repo.json(entry["scenario"].as_str().context("source scenario")?)?;
    ensure!(
        source["diagnostic_only"] != true,
        "a calibration is not an oath quantity acceptance"
    );
    let steps = source["steps"].as_array_mut().context("source steps")?;
    let gate = steps.last().context("numeric acceptance gate")?;
    ensure!(
        gate["state_name"] == "InGame"
            && gate["code"]
                .as_str()
                .is_some_and(|s| s.contains("JAC_OATH_AUDIT.accepted==true")),
        "source lacks numeric acceptance gate"
    );
    steps.extend(suffix(oath, &targets));
    source["name"] = json!(format!(
        "誓約 {oath}：数量確認後に正式保存・再読込を照合する"
    ));
    source["diagnostic_only"] = json!(true);
    source["save_probe"] = json!({"oath":oath,"scope":"numeric_and_native_save_only","source_scenario":entry["scenario"],"requires_offline_ui_comparison":true,"task_accepted":false,"remaining":["natural_age_transition","foreign_specialty_queue"]});
    civ6_autotest_rs::scenario::validate_scenario(&source)?;
    Ok(source)
}

fn catalogue(repo: &Repository) -> Result<Vec<Value>> {
    let data = repo.json(INVENTORY)?;
    let entries = data["scenarios"]
        .as_array()
        .context("oath inventory")?
        .clone();
    let names: BTreeSet<_> = entries.iter().filter_map(|e| e["oath"].as_str()).collect();
    ensure!(
        entries.len() == 66 && names.len() == 66,
        "expected all 66 distinct oaths"
    );
    Ok(entries)
}

fn verify_artifacts(repo: &Repository, path: &Path) -> Result<Value> {
    let read = |name: &str| -> Result<Value> {
        Ok(serde_json::from_str(&fs::read_to_string(path.join(name))?)?)
    };
    let scenario = read("scenario.json")?;
    let oath = scenario["save_probe"]["oath"]
        .as_str()
        .context("artifact oath identity")?;
    let entry = catalogue(repo)?
        .into_iter()
        .find(|e| e["oath"] == oath)
        .context("unknown oath artifact")?;
    let expected = make_probe(repo, &entry)?;
    let actual_steps = scenario["steps"].as_array().context("artifact steps")?;
    let expected_steps = expected["steps"].as_array().unwrap();
    ensure!(
        scenario["diagnostic_only"] == true && scenario["save_probe"] == expected["save_probe"],
        "artifact scope changed"
    );
    // Shipping can prepend lifecycle steps. It may not change the actual probe.
    ensure!(
        actual_steps.len() >= expected_steps.len(),
        "artifact source steps missing"
    );
    let prefix = &actual_steps[..actual_steps.len() - expected_steps.len()];
    ensure!(
        prefix.is_empty() || prefix == [json!({"action":"restart_game","timeout":420})],
        "unknown lifecycle prefix"
    );
    let mut normalized = actual_steps[actual_steps.len() - expected_steps.len()..].to_vec();
    let mut saved_name = None;
    for (actual, expected) in normalized.iter_mut().zip(expected_steps) {
        if expected["action"] == "save" || expected["action"] == "load" {
            let base = expected["name"].as_str().unwrap();
            let name = actual["name"].as_str().context("artifact save/load name")?;
            ensure!(
                name == base || name.starts_with(&format!("{base}-")),
                "wrong artifact save name"
            );
            if expected["action"] == "save" {
                saved_name = Some(name.to_owned());
            } else {
                ensure!(
                    saved_name.as_deref() == Some(name),
                    "save/load names differ"
                );
            }
            actual["name"] = expected["name"].clone();
        }
    }
    ensure!(
        normalized == *expected_steps,
        "artifact does not contain the generated source scenario"
    );
    let result = read("result.json")?;
    ensure!(
        result["diagnostic_only"] == true
            && result["observation_completed"] == true
            && result["passed"] == false
            && result["acceptance_passed"] == false
            && result["failure"].is_null()
            && result["completed_steps"].as_u64() == Some(actual_steps.len() as u64),
        "driver did not complete the native save probe"
    );
    let checks = result["checks"].as_array().context("driver checks")?;
    ensure!(
        !checks.is_empty() && checks.iter().all(|c| c["passed"] == true),
        "driver log checks failed or are absent"
    );
    let records: Vec<Value> = fs::read_to_string(path.join("steps.jsonl"))?
        .lines()
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    ensure!(
        !records.iter().any(|r| r["status"] == "FAIL"),
        "failed driver observation retained"
    );
    let mut values = Vec::new();
    let mut last_completion = None;
    for (offset, step) in actual_steps.iter().enumerate() {
        let index = offset + 1;
        let action = format!(
            "{index}/{} {}",
            actual_steps.len(),
            step["action"].as_str().unwrap()
        );
        let starts: Vec<_> = records
            .iter()
            .enumerate()
            .filter(|(_, r)| r["action"] == action && r["status"] == "START")
            .collect();
        ensure!(starts.len() == 1, "missing or duplicated start: {action}");
        let started: Value = serde_json::from_str(
            starts[0].1["detail"]
                .as_str()
                .context("step start detail")?,
        )?;
        ensure!(
            &started == step,
            "executed step does not match source: {action}"
        );
        let matches: Vec<_> = records
            .iter()
            .enumerate()
            .filter(|(_, r)| r["action"] == action && r["status"] == "OK")
            .collect();
        ensure!(
            matches.len() == 1,
            "missing or duplicated completion: {action}"
        );
        ensure!(
            starts[0].0 < matches[0].0 && last_completion.is_none_or(|prior| prior < starts[0].0),
            "step start/completion order differs: {action}"
        );
        last_completion = Some(matches[0].0);
        values.push((
            matches[0].0,
            matches[0].1["detail"].as_str().context("driver return")?,
        ));
    }
    let values = &values[values.len() - 8..];
    for (before, after, context) in [(0, 6, "CORE"), (1, 7, "UI")] {
        let extract = |detail: &str, phase: &str| -> Result<String> {
            let marker = format!("return=JAC_OATH_SAVE_{context}_{phase}|{oath}|");
            let signature = detail
                .strip_prefix(&format!(
                    "ran Lua in {}; ",
                    if context == "CORE" {
                        "GameCore_Tuner"
                    } else {
                        "InGame"
                    }
                ))
                .context("wrong Lua context")?
                .strip_prefix(&marker)
                .context("wrong observation marker")?;
            let (length, body) = signature
                .split_once(':')
                .context("snapshot length missing")?;
            let body = body
                .strip_suffix("|END-JAC-OATH-SAVE")
                .context("snapshot terminator missing")?;
            ensure!(
                length.parse::<usize>()? == body.len()
                    && body.starts_with('{')
                    && body.ends_with('}'),
                "incomplete snapshot return"
            );
            Ok(body.to_owned())
        };
        ensure!(
            extract(values[before].1, "BEFORE")? == extract(values[after].1, "AFTER")?,
            "{context} real quantities or ledger changed across save/load"
        );
    }
    Ok(
        json!({"oath":oath,"numeric_and_native_save_verified":true,"house_count":22,"city_count":56,"flags_per_city":66,"task_accepted":false,"remaining":["natural_age_transition","foreign_specialty_queue"]}),
    )
}

fn run(repo: &Repository, args: &[String]) -> Result<()> {
    if args == ["--check"] {
        let vm = mlua::Lua::new();
        for entry in catalogue(repo)? {
            let probe = make_probe(repo, &entry)?;
            for step in probe["steps"].as_array().unwrap() {
                if let Some(code) = step["code"]
                    .as_str()
                    .or_else(|| step["condition"]["code"].as_str())
                {
                    vm.load(code).into_function()?;
                }
            }
        }
        println!("66 save probes: driver schema and Lua 5.1 compile passed; live unverified");
        return Ok(());
    }
    if args.len() == 2 && args[0] == "--verify" {
        println!(
            "{}",
            serde_json::to_string_pretty(&verify_artifacts(repo, Path::new(&args[1]))?)?
        );
        return Ok(());
    }
    if args.len() == 4 && args[0] == "--oath" && args[2] == "--output" {
        let entry = catalogue(repo)?
            .into_iter()
            .find(|e| e["oath"] == args[1])
            .context("unknown oath")?;
        let probe = make_probe(repo, &entry)?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&args[3])
            .context("create new scenario; existing evidence is never overwritten")?;
        writeln!(output, "{}", serde_json::to_string_pretty(&probe)?)?;
        return Ok(());
    }
    bail!("usage: generate-oath-save-probes --check | --oath JAC_OATH_... --output NEW.json | --verify ARTIFACT_DIR")
}

fn main() {
    if let Err(error) = run(
        &Repository::checked_in(),
        &std::env::args().skip(1).collect::<Vec<_>>(),
    ) {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_66_keep_the_numeric_gate_and_compile_in_lua51() -> Result<()> {
        let repo = Repository::checked_in();
        run(&repo, &["--check".into()])?;
        for entry in catalogue(&repo)? {
            let source = repo.json(entry["scenario"].as_str().unwrap())?;
            let probe = make_probe(&repo, &entry)?;
            let original = source["steps"].as_array().unwrap();
            assert_eq!(
                &probe["steps"].as_array().unwrap()[..original.len()],
                original
            );
            assert_eq!(probe["finite_turns"], source["finite_turns"]);
            assert_eq!(probe["diagnostic_only"], true);
            assert_eq!(probe["save_probe"]["task_accepted"], false);
        }
        Ok(())
    }

    fn vm() -> Result<mlua::Lua> {
        let vm = mlua::Lua::new();
        vm.load(r#"
props={}; flags={}; plots={}; ledger={reign=1,history={r1={turn=66}},houses={}}
JAC_OATH_AUDIT={accepted=true}; Game={GetCurrentGameTurn=function() return 16611 end,GetProperty=function(self,k) if k=='JAC_AGES_LEDGER' then return ledger end return props[k] end,SetProperty=function(self,k,v) props[k]=v end}
GameInfo={Yields={}}; for i,k in ipairs({'PRODUCTION','GOLD','SCIENCE','CULTURE','FAITH','FOOD'}) do GameInfo.Yields['YIELD_'..k]={Index=i} end
catalogue={}; for i=1,66 do catalogue[i]={OathType=i==1 and 'JAC_OATH_ODA_TENKA_FUBU' or 'OATH_'..i,CivilizationType=i==1 and 'ODA' or 'OTHER'} end
GameInfo.JAC_SengokuOaths=function() local i=0; return function() i=i+1; return catalogue[i] end end
players={}; for id=0,21 do
 ledger.houses['p'..id]={civ=id==0 and 'ODA' or 'CIV_'..id,age='GOLDEN',oathChosen=1,oaths=id==0 and {JAC_OATH_ODA_TENKA_FUBU=true} or {}}
 local list={}; for n=1,(id<12 and 3 or 2) do
  local index=id*10+n; local f={}; for _,row in ipairs(catalogue) do f[row.OathType..'_ON']=(id==0 and row.OathType=='JAC_OATH_ODA_TENKA_FUBU') and 1 or 0 end
  flags[index]=f; plots[index]={GetIndex=function() return index end,GetOwner=function() return id end,GetProperty=function(self,k) return flags[index][k] end}
  list[#list+1]={GetOwner=function() return id end,GetID=function() return index end,GetX=function() return index end,GetY=function() return 0 end,GetYield=function(self,kind) return 10+kind end}
 end
 local collection={Members=function() local i=0; return function() i=i+1; if list[i] then return i,list[i] end end end}
 players[#players+1]={GetID=function() return id end,GetCities=function() return collection end}
end
PlayerManager={GetAliveMajors=function() return players end}; Map={GetPlot=function(x,y) return plots[x] end}
"#).exec()?;
        Ok(vm)
    }

    #[test]
    fn actual_core_snapshot_rejects_history_foreign_flag_and_quantity_changes() -> Result<()> {
        let oath = "JAC_OATH_ODA_TENKA_FUBU";
        let before = core_code(oath, false);
        let after = core_code(oath, true);
        for mutation in [
            "ledger.history.r1.turn=65",
            "flags[211].OATH_2_ON=1",
            "plots[1].GetOwner=function() return 1 end",
            "local c=select(2,players[1]:GetCities():Members()()); c.GetYield=function() return 999 end",
            "ledger.houses.p21=nil",
            "Game.GetCurrentGameTurn=function() return 16612 end",
        ] {
            let vm = vm()?;
            let saved: String = vm.load(&before).eval()?;
            assert!(saved.starts_with("JAC_OATH_SAVE_CORE_BEFORE|"));
            vm.load(&after).eval::<String>()?;
            vm.load(mutation).exec()?;
            assert!(vm.load(&after).eval::<String>().is_err(), "{mutation}");
        }
        let vm = vm()?;
        // The Core VM cannot see the UI-only numeric audit cache.
        vm.load("JAC_OATH_AUDIT=nil").exec()?;
        assert!(vm.load(&after).eval::<String>().is_err());
        vm.load(&before).eval::<String>()?;
        vm.load(&after).eval::<String>()?;
        Ok(())
    }

    #[test]
    fn ui_snapshot_needs_actual_finite_growth_and_queue_values_after_reload() -> Result<()> {
        let vm = vm()?;
        vm.load(r#"
Players={[0]={IsTurnActive=function() return true end,IsTurnActiveComplete=function() return true end}}
UI={CanEndTurn=function() return true end,HasSentTurnComplete=function() return false end,IsProcessingMessages=function() return false end}
GameInfo.Units={UNIT_SPEARMAN={Index=1}}; GameInfo.Buildings={}
growth={}; for _,name in ipairs({'GetOverallGrowthModifier','GetOtherGrowthModifier','GetHousingGrowthModifier','GetHappinessGrowthModifier','GetFoodSurplus','GetLoyaltyGrowthModifier'}) do growth[name]=function() return 1 end end
queue={GetCurrentProductionTypeHash=function() return 123 end,GetUnitProgress=function() return 300 end,GetUnitCost=function() return 32768 end}
for _,p in ipairs(players) do for _,c in p:GetCities():Members() do c.GetGrowth=function() return growth end; c.GetBuildQueue=function() return queue end end end
"#).exec()?;
        let targets = vec!["UNIT_SPEARMAN".into()];
        let before = ui_code("JAC_OATH_ODA_TENKA_FUBU", &targets, false);
        let after = ui_code("JAC_OATH_ODA_TENKA_FUBU", &targets, true);
        let first: String = vm.load(&before).eval()?;
        // Simulate a new UI state: the diagnostic cache is gone, Game properties remain.
        vm.load("JAC_OATH_AUDIT=nil").exec()?;
        let second: String = vm.load(&after).eval()?;
        assert_eq!(first.replace("_BEFORE|", "_AFTER|"), second);
        assert!(vm.load(&before).eval::<String>().is_err());
        vm.load("queue.GetUnitProgress=function() return 301 end")
            .exec()?;
        assert_ne!(second, vm.load(&after).eval::<String>()?);
        vm.load("growth.GetOtherGrowthModifier=function() return 0/0 end")
            .exec()?;
        assert!(vm.load(&after).eval::<String>().is_err());
        vm.load("growth.GetOtherGrowthModifier=nil").exec()?;
        assert!(vm.load(&after).eval::<String>().is_err());
        Ok(())
    }

    fn artifacts() -> Result<(tempfile::TempDir, Vec<Value>)> {
        let repo = Repository::checked_in();
        let entry = catalogue(&repo)?.into_iter().next().unwrap();
        let mut scenario = make_probe(&repo, &entry)?;
        let steps = scenario["steps"].as_array_mut().unwrap();
        steps.insert(0, json!({"action":"restart_game","timeout":420}));
        for s in steps
            .iter_mut()
            .filter(|s| s["action"] == "save" || s["action"] == "load")
        {
            s["name"] = json!(format!("{}-agent-01", s["name"].as_str().unwrap()));
        }
        let oath = entry["oath"].as_str().unwrap();
        let count = steps.len();
        let mut rows = Vec::new();
        for (offset, s) in steps.iter().enumerate() {
            let tail = if offset >= count - 8 {
                Some(offset - (count - 8))
            } else {
                None
            };
            let detail = match tail {
                Some(0) => format!(
                    "ran Lua in GameCore_Tuner; return=JAC_OATH_SAVE_CORE_BEFORE|{oath}|6:{{core}}|END-JAC-OATH-SAVE"
                ),
                Some(1) => {
                    format!("ran Lua in InGame; return=JAC_OATH_SAVE_UI_BEFORE|{oath}|4:{{ui}}|END-JAC-OATH-SAVE")
                }
                Some(6) => format!(
                    "ran Lua in GameCore_Tuner; return=JAC_OATH_SAVE_CORE_AFTER|{oath}|6:{{core}}|END-JAC-OATH-SAVE"
                ),
                Some(7) => {
                    format!("ran Lua in InGame; return=JAC_OATH_SAVE_UI_AFTER|{oath}|4:{{ui}}|END-JAC-OATH-SAVE")
                }
                _ => "operation completed".into(),
            };
            let action = format!("{}/{} {}", offset + 1, count, s["action"].as_str().unwrap());
            rows.push(json!({"action":action,"status":"START","detail":serde_json::to_string(s)?}));
            rows.push(json!({"action":action,"status":"OK","detail":detail}));
        }
        let temp = tempfile::tempdir()?;
        fs::write(
            temp.path().join("scenario.json"),
            serde_json::to_string(&scenario)?,
        )?;
        fs::write(
            temp.path().join("result.json"),
            serde_json::to_string(
                &json!({"diagnostic_only":true,"observation_completed":true,"passed":false,"acceptance_passed":false,"failure":null,"completed_steps":count,"checks":[{"passed":true}]}),
            )?,
        )?;
        write_rows(temp.path(), &rows)?;
        Ok((temp, rows))
    }

    fn write_rows(path: &Path, rows: &[Value]) -> Result<()> {
        let text = rows
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()?
            .join("\n");
        fs::write(path.join("steps.jsonl"), text)?;
        Ok(())
    }

    #[test]
    fn verifier_compares_immutable_ui_returns_and_native_operation_order() -> Result<()> {
        let repo = Repository::checked_in();
        let (temp, original) = artifacts()?;
        assert_eq!(
            verify_artifacts(&repo, temp.path())?["task_accepted"],
            false
        );
        let count = original.len();
        for mutation in 0..10 {
            let mut rows = original.clone();
            match mutation {
                0 => {
                    rows[count - 1]["detail"] = json!(rows[count - 1]["detail"]
                        .as_str()
                        .unwrap()
                        .replace("{ui}", "{different}"))
                }
                1 => {
                    rows.remove(count - 6);
                }
                2 => {
                    rows.push(rows[count - 1].clone());
                }
                3 => rows.swap(count - 6, count - 4),
                4 => rows[count - 4]["status"] = json!("FAIL"),
                5 => {
                    rows.remove(count - 17);
                }
                6 | 7 => {
                    let index = if mutation == 6 {
                        count - 18
                    } else {
                        count - 12
                    };
                    let mut started: Value =
                        serde_json::from_str(rows[index]["detail"].as_str().unwrap())?;
                    if mutation == 6 {
                        started["code"] = json!("return true");
                    } else {
                        started["name"] = json!("wrong-save");
                    }
                    rows[index]["detail"] = json!(serde_json::to_string(&started)?);
                }
                8 => {
                    for index in [count - 13, count - 1] {
                        rows[index]["detail"] = json!(rows[index]["detail"]
                            .as_str()
                            .unwrap()
                            .replace("{ui}", "{u}"));
                    }
                }
                _ => {
                    rows[count - 1]["detail"] = json!(rows[count - 1]["detail"]
                        .as_str()
                        .unwrap()
                        .replace("|END-JAC-OATH-SAVE", ""));
                }
            }
            write_rows(temp.path(), &rows)?;
            assert!(
                verify_artifacts(&repo, temp.path()).is_err(),
                "mutation {mutation}"
            );
        }
        Ok(())
    }

    #[test]
    fn verifier_rejects_mutated_probe_and_load_name() -> Result<()> {
        let repo = Repository::checked_in();
        for mutation in 0..3 {
            let (temp, _) = artifacts()?;
            let mut scenario: Value =
                serde_json::from_str(&fs::read_to_string(temp.path().join("scenario.json"))?)?;
            let count = scenario["steps"].as_array().unwrap().len();
            match mutation {
                0 => scenario["steps"][count - 4]["name"] = json!("other-save"),
                1 => scenario["steps"][count - 2]["code"] = json!("return true"),
                _ => scenario["diagnostic_only"] = json!(false),
            }
            fs::write(
                temp.path().join("scenario.json"),
                serde_json::to_string(&scenario)?,
            )?;
            assert!(verify_artifacts(&repo, temp.path()).is_err());
        }
        Ok(())
    }

    #[test]
    fn observation_is_not_acceptance_and_incomplete_results_cannot_verify() -> Result<()> {
        let repo = Repository::checked_in();
        for (key, value) in [
            ("passed", json!(true)),
            ("acceptance_passed", json!(true)),
            ("observation_completed", json!(false)),
            ("diagnostic_only", json!(false)),
            ("checks", json!([])),
            ("failure", json!("incomplete")),
            ("completed_steps", json!(0)),
        ] {
            let (temp, _) = artifacts()?;
            let mut result: Value =
                serde_json::from_str(&fs::read_to_string(temp.path().join("result.json"))?)?;
            result[key] = value;
            fs::write(
                temp.path().join("result.json"),
                serde_json::to_string(&result)?,
            )?;
            assert!(verify_artifacts(&repo, temp.path()).is_err(), "{key}");
        }
        Ok(())
    }
}
