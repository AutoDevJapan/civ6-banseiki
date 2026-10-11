//! Extend a numeric oath scenario with a real save/reload. UI observations are
//! compared from immutable driver returns, never retained in a UI Lua cache.
//! The verified source evidence is shared with the natural expiry probe.
use crate::Repository;
use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};
use std::{collections::BTreeSet, fs, io::Write, path::Path};

const INVENTORY: &str = "documents/poc/data/oath-effect-scenarios.json";
const KEY: &str = "JAC_TODO134_SAVE_PROBE";
pub(crate) const HELPERS: &str = r#"
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
/// Ready condition after a native load, shared with the expiry probe.
pub(crate) const LOADED_READY: &str = "return Players[0]:IsTurnActive() and Players[0]:IsTurnActiveComplete() and UI.CanEndTurn() and not UI.HasSentTurnComplete() and not UI.IsProcessingMessages()";

pub(crate) fn core_code(oath: &str, verify: bool) -> String {
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
-- start: the actual GameConfiguration start turn, so a frame gives the game-relative turn.
local now=ser({{oath=oath,turn=Game.GetCurrentGameTurn(),start=number(GameConfiguration.GetStartTurn()),ledger=l,cities=cities}})
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

pub(crate) fn lua(code: String, state: &str) -> Value {
    json!({"action":"ui_lua","state_name":state,"code":code,"timeout":30})
}

pub(crate) fn loaded_ready() -> Value {
    json!({"action":"wait_for","timeout":60,"close_diplomacy":true,"condition":{"type":"lua","state_name":"InGame","code":LOADED_READY}})
}

fn suffix(oath: &str, targets: &[String]) -> Vec<Value> {
    let name = format!("JAC-Todo134-Save-{oath}");
    vec![
        lua(core_code(oath, false), "GameCore_Tuner"),
        lua(ui_code(oath, targets, false), "InGame"),
        json!({"action":"save","name":name,"timeout":180}),
        json!({"action":"exit_to_menu","timeout":180}),
        json!({"action":"load","name":name,"timeout":600}),
        loaded_ready(),
        lua(core_code(oath, true), "GameCore_Tuner"),
        lua(ui_code(oath, targets, true), "InGame"),
    ]
}

pub fn make_probe(repo: &Repository, entry: &Value) -> Result<Value> {
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

const GROWTH_FOCUS_RESYNC: &str = r#"
-- Official CityPanel.lua:447-479 command parameters; resend existing values.
-- This is an unverified cache refresh attempt, not an assertion of refresh.
local core=assert(Game:GetProperty('JAC_TODO134_GROWTH_CORE'),'growth Core snapshot missing')
local player=Game.GetLocalPlayer()
assert(player==0 and core.turn==Game.GetCurrentGameTurn(),'focus resync owner or turn changed')
local phase=assert(core.phase)
assert(phase=='selected' or phase=='off' or phase=='restored','unexpected resync phase')
local all=assert(JAC_OATH_AUDIT)
all.focus_resync=all.focus_resync or {}
assert(not all.focus_resync[phase],'focus resync must not be sent twice')
local rows={}; local seen={}
local yield=assert(GameInfo.Yields['YIELD_FOOD']).Index
for _,row in ipairs(core.snapshot.own) do
 assert(row.owner==player and not seen[row.id],'focus resync foreign or duplicate city')
 local city=assert(Players[player]:GetCities():FindID(row.id),'focus resync city missing')
 local plot=assert(Map.GetPlot(city:GetX(),city:GetY()))
 assert(city:GetOwner()==player and plot:GetIndex()==row.plot,'focus resync city identity changed')
 local citizens=assert(city:GetCitizens())
 local favored=citizens:IsFavoredYield(yield); local ignored=citizens:IsDisfavoredYield(yield)
 assert(type(favored)=='boolean' and type(ignored)=='boolean' and not (favored and ignored),'invalid citizen preferences')
 rows[#rows+1]={id=row.id,plot=row.plot,owner=row.owner,favored=favored,ignored=ignored}
 seen[row.id]=true
end
assert(#rows>0,'focus resync city inventory empty')
-- Record before dispatch: a partial send cannot be silently replayed.
all.focus_resync[phase]={turn=core.turn,rows=rows,sent=0}
for _,row in ipairs(rows) do
 local city=assert(Players[player]:GetCities():FindID(row.id))
 for _,item in ipairs({{flag=0,value=row.favored},{flag=1,value=row.ignored}}) do
  local parameters={}
  parameters[CityCommandTypes.PARAM_FLAGS]=item.flag
  parameters[CityCommandTypes.PARAM_YIELD_TYPE]=yield
  parameters[CityCommandTypes.PARAM_DATA0]=item.value and 1 or 0
  CityManager.RequestCommand(city,CityCommandTypes.SET_FOCUS,parameters)
  all.focus_resync[phase].sent=all.focus_resync[phase].sent+1
 end
end
return '{"phase":"'..phase..'","existing_focus_commands":'..all.focus_resync[phase].sent..',"refresh_proven":false}'
"#;

const GROWTH_FOCUS_UNCHANGED: &str = r#"
local attempt=assert(JAC_OATH_AUDIT and JAC_OATH_AUDIT.focus_resync and JAC_OATH_AUDIT.focus_resync[core.phase],'focus resync not recorded')
assert(attempt.turn==Game.GetCurrentGameTurn() and attempt.sent==2*#attempt.rows,'focus resync incomplete or turn changed')
local yield=assert(GameInfo.Yields['YIELD_FOOD']).Index
for _,row in ipairs(attempt.rows) do
 local city=assert(Players[row.owner]:GetCities():FindID(row.id),'focus resync city disappeared')
 local plot=assert(Map.GetPlot(city:GetX(),city:GetY()))
 assert(city:GetOwner()==row.owner and plot:GetIndex()==row.plot,'focus resync city identity changed')
 local citizens=assert(city:GetCitizens())
 assert(citizens:IsFavoredYield(yield)==row.favored and citizens:IsDisfavoredYield(yield)==row.ignored,'focus resync changed citizen preferences')
end
"#;

/// A production or yield-and-growth diagnostic, with an owned-process inverse-removal
/// candidate before any reference or oath is attached. Normal probes stay intact.
pub fn make_native_probe(repo: &Repository, entry: &Value) -> Result<Value> {
    make_native_probe_with_basis(repo, entry, NativeProductionBasis::Q8)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NativeProductionBasis {
    // Preserve immutable probes produced before the naval/building correction.
    Legacy,
    Q8,
}

fn production_rate_to_q8(code: &str, targets: &[Value]) -> Result<String> {
    const BINDING: &str = "local native_rate=";
    ensure!(
        code.matches(BINDING).count() == 1,
        "native rate binding must occur exactly once"
    );
    let amount = targets
        .iter()
        .find_map(|target| {
            let name = target["target"].as_str()?;
            let amount = target["expected_percent"].as_i64()?;
            let prefix = format!(
                "local target={}; local amount={amount}; {BINDING}",
                serde_json::to_string(name).ok()?
            );
            code.contains(&prefix).then_some(amount)
        })
        .context("native comparison does not match a catalogue production target")?;
    let start = code.find(BINDING).unwrap() + BINDING.len();
    let end = start
        + code[start..]
            .find('\n')
            .context("native rate line boundary missing")?;
    ensure!(
        code[start..end].trim() == format!("{amount}/100"),
        "unexpected legacy production rate expression"
    );
    let mut result = code.to_owned();
    result.replace_range(
        start..end,
        &format!(
            "{}/256",
            crate::oath_effect_audit::native_percentage_raw(amount)
        ),
    );
    Ok(result)
}

fn make_native_probe_with_basis(
    repo: &Repository,
    entry: &Value,
    basis: NativeProductionBasis,
) -> Result<Value> {
    ensure!(
        catalogue(repo)?.iter().any(|known| known == entry),
        "native diagnostic needs an unchanged catalogue entry"
    );
    let effects = entry["effects"].as_array().context("catalogue effects")?;
    let targets = entry["production_targets"]
        .as_array()
        .context("catalogue production targets")?;
    let has_growth = effects.iter().any(|effect| effect["kind"] == "growth");
    let growth = has_growth && targets.is_empty();
    let production = !has_growth
        && effects.iter().any(|effect| {
            ["military", "naval", "walls"]
                .iter()
                .any(|kind| effect["kind"] == *kind)
        });
    ensure!(
        production || growth,
        "the native candidate requires production without growth, or yield-and-growth without production"
    );
    let naval_buildings = effects
        .iter()
        .any(|effect| effect["kind"] == "naval" || effect["kind"] == "walls");
    let mut probe = make_probe(repo, entry)?;
    let steps = probe["steps"].as_array_mut().context("probe steps")?;
    // Q8 observations belong only to the explicit memory diagnostic. Ordinary
    // numeric acceptance retains the catalogue percentage and its old bounds.
    let mut conversions = 0;
    let mut rate_conversions = 0;
    for step in steps.iter_mut() {
        if let Some(code) = step["code"].as_str() {
            if code.contains("local native_diagnostic=false") {
                conversions += 1;
                let mut converted = code.replace(
                    "local native_diagnostic=false",
                    "local native_diagnostic=true",
                );
                if naval_buildings
                    && basis == NativeProductionBasis::Q8
                    && converted.contains("local native_rate=")
                {
                    // Only generated production comparisons have this binding.
                    // The normal catalogue and its decimal acceptance stay intact.
                    converted = production_rate_to_q8(&converted, targets)?;
                    rate_conversions += 1;
                }
                step["code"] = json!(converted);
            }
        }
    }
    ensure!(
        conversions == 1 + targets.len(),
        "native diagnostic comparison boundaries changed"
    );
    ensure!(
        rate_conversions
            == if naval_buildings && basis == NativeProductionBasis::Q8 {
                targets.len()
            } else {
                0
            },
        "native production rate comparison count changed"
    );
    if growth {
        let original = std::mem::take(steps);
        let mut refreshes = 0;
        for mut step in original {
            if let Some(code) = step["condition"]["code"].as_str() {
                if code.contains("local core=Game:GetProperty('JAC_TODO134_GROWTH_CORE')") {
                    refreshes += 1;
                    let guarded = code.replace(
                        "assert(core.turn==Game.GetCurrentGameTurn(),'growth observation turn changed')",
                        &format!("assert(core.turn==Game.GetCurrentGameTurn(),'growth observation turn changed')\n{GROWTH_FOCUS_UNCHANGED}"),
                    );
                    ensure!(guarded != code, "growth phase guard anchor missing");
                    // Acknowledge the current phase in the UI before dispatch.
                    // A Core property may still be in transit after its setter.
                    steps.push(step.clone());
                    steps.push(
                        json!({"action":"ui_lua","state_name":"InGame","code":GROWTH_FOCUS_RESYNC}),
                    );
                    step["condition"]["code"] = json!(guarded);
                }
            }
            steps.push(step);
        }
        ensure!(refreshes == 3, "growth resync phase boundaries changed");
    }
    ensure!(
        steps.len() > 5 && steps[3]["action"] == "start_game" && steps[4]["action"] == "wait_for",
        "native candidate startup boundary changed"
    );
    let action = if naval_buildings {
        json!({"action":"oath_exact_removal","timeout":30,"scope":"naval-buildings-v1"})
    } else {
        json!({"action":"oath_exact_removal","timeout":30})
    };
    steps.insert(5, action);
    probe["native_exact_removal_candidate"] = json!(true);
    if naval_buildings && basis == NativeProductionBasis::Q8 {
        probe["native_production_basis"] = json!("q8-v2");
    }
    civ6_autotest_rs::scenario::validate_scenario(&probe)?;
    Ok(probe)
}

pub fn catalogue(repo: &Repository) -> Result<Vec<Value>> {
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

/// A completed quantity + native save run, read only from its recorded driver evidence.
pub struct Evidence {
    pub oath: String,
    /// The generated save probe the artifact executed (lifecycle prefix removed).
    pub source: Value,
    /// The file name the native save actually wrote, from its OK record.
    pub saved_name: String,
    /// Canonical Core frame read back after the native load.
    pub core_after: String,
}

/// The runner writes `NAME.Civ6Save`; NAME is the step name, or with exact_name
/// absent its first 67 bytes plus a 12-digit hash of the output directory.
pub(crate) fn recorded_save_name(detail: &str, step: &Value) -> Result<String> {
    let file = detail.rsplit(['/', '\\']).next().unwrap_or_default();
    let name = file
        .strip_suffix(".Civ6Save")
        .context("save OK record lacks the native save path")?;
    let base = step["name"].as_str().context("save step name")?;
    let runner_suffixed = name
        .strip_prefix(&base[..base.len().min(67)])
        .and_then(|rest| rest.strip_prefix('-'))
        .is_some_and(|hash| {
            hash.len() == 12
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        });
    ensure!(
        if step["exact_name"] == true {
            name == base
        } else {
            runner_suffixed
        },
        "recorded save name does not follow the executed save step"
    );
    ensure!(
        !name.is_empty()
            && name.len() <= 80
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b)),
        "unsafe recorded save name"
    );
    Ok(name.to_owned())
}

pub fn verify(repo: &Repository, path: &Path) -> Result<Evidence> {
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
    let native = scenario.get("native_exact_removal_candidate").is_some();
    ensure!(
        !native || scenario["native_exact_removal_candidate"] == true,
        "invalid native candidate marker"
    );
    let basis = match scenario.get("native_production_basis") {
        None => NativeProductionBasis::Legacy,
        Some(value) if native && value == "q8-v2" => NativeProductionBasis::Q8,
        _ => bail!("unknown native production observation basis"),
    };
    let port = match scenario.get("naval_port_fixture") {
        None => false,
        Some(value)
            if native
                && basis == NativeProductionBasis::Q8
                && value == crate::oath_naval_port::VERSION =>
        {
            true
        }
        _ => bail!("unknown or incompatible naval port fixture"),
    };
    let mut expected = if port {
        crate::oath_naval_port::make_probe(repo, &entry)?
    } else if native {
        make_native_probe_with_basis(repo, &entry, basis)?
    } else {
        make_probe(repo, &entry)?
    };
    if let Some(scope) = scenario.get("production_inventory_scope") {
        ensure!(
            native && scope == crate::oath_production_inventory::VERSION,
            "unknown production inventory scope"
        );
        expected = crate::oath_production_inventory::decorate(expected)?;
    }
    if native {
        let action = expected["steps"]
            .as_array()
            .context("expected native steps")?
            .iter()
            .find(|step| step["action"] == "oath_exact_removal")
            .context("expected native scope")?;
        let scope = civ6_autotest_rs::oath_exact_removal::Scope::from_step(action)?;
        let proof = read(
            if scope == civ6_autotest_rs::oath_exact_removal::Scope::Legacy {
                "oath-exact-removal.json"
            } else {
                "oath-exact-removal-naval-buildings-v1.json"
            },
        )?;
        verify_native_proof(&proof, scope)?;
    }
    let actual_steps = scenario["steps"].as_array().context("artifact steps")?;
    let expected_steps = expected["steps"].as_array().unwrap();
    ensure!(
        scenario["diagnostic_only"] == true
            && scenario["save_probe"] == expected["save_probe"]
            && scenario.get("native_production_basis") == expected.get("native_production_basis")
            && scenario.get("naval_port_fixture") == expected.get("naval_port_fixture")
            && scenario.get("production_inventory_scope")
                == expected.get("production_inventory_scope"),
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
    let tail = &actual_steps[actual_steps.len() - 8..];
    let saved_name = recorded_save_name(values[2].1, &tail[2])?;
    ensure!(
        values[4].1 == format!("loaded {saved_name}"),
        "load OK record does not name the recorded save"
    );
    let mut bodies = Vec::new();
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
        let after = extract(values[after].1, "AFTER")?;
        ensure!(
            extract(values[before].1, "BEFORE")? == after,
            "{context} real quantities or ledger changed across save/load"
        );
        bodies.push(after);
    }
    Ok(Evidence {
        oath: oath.to_owned(),
        source: expected,
        saved_name,
        core_after: bodies.swap_remove(0),
    })
}

pub fn verify_artifacts(repo: &Repository, path: &Path) -> Result<Value> {
    let evidence = verify(repo, path)?;
    let native = evidence.source["native_exact_removal_candidate"] == true;
    let mut remaining = vec!["natural_age_transition", "foreign_specialty_queue"];
    if native {
        remaining.extend(["nominal_percentage_quantity", "normal_product_integration"]);
    }
    let mut report = json!({"oath":evidence.oath,"numeric_and_native_save_verified":!native,
        "native_q8_inverse_and_save_diagnostic_verified":native,
        "saved_name":evidence.saved_name,"house_count":22,"city_count":56,
        "flags_per_city":66,"task_accepted":false,"remaining":remaining});
    append_production_inventory_counts(&mut report, &evidence)?;
    Ok(report)
}

fn append_production_inventory_counts(report: &mut Value, evidence: &Evidence) -> Result<()> {
    if evidence.source["production_inventory_scope"] == crate::oath_production_inventory::VERSION {
        let frame = crate::oath_expiry_probe::Frame::parse(&evidence.core_after)?;
        let count = frame.get("cities")?.entries()?.len();
        ensure!(count >= 56, "expanded saved frame lost original cities");
        report["city_count"] = json!(count);
        report["original_city_count"] = json!(56);
        report["new_foreign_city_count"] = json!(count - 56);
    }
    Ok(())
}

fn verify_native_proof(
    proof: &Value,
    scope: civ6_autotest_rs::oath_exact_removal::Scope,
) -> Result<()> {
    for (key, value) in civ6_autotest_rs::oath_exact_removal::plan_for_scope(scope)
        .as_object()
        .context("native plan")?
    {
        ensure!(
            &proof[key] == value,
            "native proof differs from the reviewed instruction plan: {key}"
        );
    }
    ensure!(
        (proof["status"] == "applied" && proof["writes"] == 4)
            || (proof["status"] == "already_patched" && proof["writes"] == 0),
        "native candidate did not complete"
    );
    ensure!(
        proof["original_protections_restored"] == true
            && proof["pid"].as_u64().is_some_and(|id| id > 0)
            && proof["creation_filetime"]
                .as_u64()
                .is_some_and(|time| time > 0),
        "native process or protection evidence missing"
    );
    Ok(())
}

pub fn run(repo: &Repository, args: &[String]) -> Result<()> {
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
        write_new(Path::new(&args[3]), &make_probe(repo, &entry)?)?;
        return Ok(());
    }
    if args.len() == 5
        && args[0] == "--native-exact-removal"
        && args[1] == "--oath"
        && args[3] == "--output"
    {
        let entry = catalogue(repo)?
            .into_iter()
            .find(|entry| entry["oath"] == args[2])
            .context("unknown oath")?;
        write_new(Path::new(&args[4]), &make_native_probe(repo, &entry)?)?;
        return Ok(());
    }
    if args.len() == 6
        && args[0] == "--native-exact-removal"
        && args[1] == "--naval-harbor"
        && args[2] == "--oath"
        && args[4] == "--output"
    {
        let entry = catalogue(repo)?
            .into_iter()
            .find(|entry| entry["oath"] == args[3])
            .context("unknown oath")?;
        write_new(
            Path::new(&args[5]),
            &crate::oath_naval_port::make_probe(repo, &entry)?,
        )?;
        return Ok(());
    }
    if args.len() == 5
        && args[0] == "--startup-diagnostic"
        && args[1] == "--oath"
        && args[3] == "--output"
    {
        let entry = catalogue(repo)?
            .into_iter()
            .find(|e| e["oath"] == args[2])
            .context("unknown oath")?;
        write_new(
            Path::new(&args[4]),
            &crate::oath_naval_port::make_startup_probe(repo, &entry)?,
        )?;
        return Ok(());
    }
    let dynamic_native = args.len() == 6
        && args[0] == "--native-exact-removal"
        && args[1] == "--dynamic-foreign-inventory"
        && args[2] == "--oath"
        && args[4] == "--output";
    let dynamic_port = args.len() == 7
        && args[0] == "--native-exact-removal"
        && args[1] == "--naval-harbor"
        && args[2] == "--dynamic-foreign-inventory"
        && args[3] == "--oath"
        && args[5] == "--output";
    if dynamic_native || dynamic_port {
        let name = if dynamic_port { &args[4] } else { &args[3] };
        let entry = catalogue(repo)?
            .into_iter()
            .find(|e| e["oath"] == *name)
            .context("unknown production oath")?;
        let source = if dynamic_port {
            crate::oath_naval_port::make_probe(repo, &entry)?
        } else {
            make_native_probe(repo, &entry)?
        };
        let output = if dynamic_port { &args[6] } else { &args[5] };
        write_new(
            Path::new(output),
            &crate::oath_production_inventory::decorate(source)?,
        )?;
        return Ok(());
    }
    bail!("usage: generate-oath-save-probes --check | [--native-exact-removal [--naval-harbor] [--dynamic-foreign-inventory] | --startup-diagnostic] --oath JAC_OATH_... --output NEW.json | --verify ARTIFACT_DIR; options must appear in the shown order")
}

pub(crate) fn write_new(path: &Path, scenario: &Value) -> Result<()> {
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .context("create new scenario; existing evidence is never overwritten")?;
    writeln!(output, "{}", serde_json::to_string_pretty(scenario)?)?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn saved_inventory_report_counts_canonical_frames_without_relabeling_legacy_evidence(
    ) -> Result<()> {
        let vm = mlua::Lua::new();
        for count in [55, 56, 57] {
            let core_after=vm.load(format!("{HELPERS}\nlocal cities={{}};for i=1,{count} do cities[tostring(i)]={{id=i,owner=i==1 and 0 or 1}} end;return ser({{cities=cities}})")).eval::<String>()?;
            let mut evidence = Evidence {
                oath: "test".into(),
                source: json!({"production_inventory_scope":crate::oath_production_inventory::VERSION}),
                saved_name: "test".into(),
                core_after,
            };
            let mut report = json!({"city_count":56});
            if count < 56 {
                assert!(append_production_inventory_counts(&mut report, &evidence).is_err());
            } else {
                append_production_inventory_counts(&mut report, &evidence)?;
                assert_eq!(report["city_count"], count);
                assert_eq!(report["original_city_count"], 56);
                assert_eq!(report["new_foreign_city_count"], count - 56);
            }
            evidence.source = json!({});
            let mut legacy = json!({"city_count":56});
            append_production_inventory_counts(&mut legacy, &evidence)?;
            assert_eq!(legacy, json!({"city_count":56}));
        }
        Ok(())
    }

    #[test]
    fn production_rate_conversion_checks_bindings_and_tolerates_line_endings() -> Result<()> {
        let targets = vec![json!({"target":"BUILDING_WALLS","expected_percent":20})];
        for ending in ["\n", "\r\n", "  \r\n"] {
            let code = format!("local target=\"BUILDING_WALLS\"; local amount=20; local native_rate=20/100{ending}local native_diagnostic=true\n");
            assert!(production_rate_to_q8(&code, &targets)?.contains("local native_rate=51/256\n"));
            for invalid in [
                code.replace("20/100", "20/1000"),
                code.replace("local amount=20", "local amount=15"),
                code.replace("BUILDING_WALLS", "BUILDING_UNKNOWN"),
                format!("{code}local native_rate=20/100\n"),
                code.replace("local native_rate=", "local other_rate="),
            ] {
                assert!(production_rate_to_q8(&invalid, &targets).is_err());
            }
        }
        Ok(())
    }

    #[test]
    fn production_probes_bind_their_scope_and_preserve_all_quantity_and_save_steps() -> Result<()> {
        let repo = Repository::checked_in();
        let mut seen = BTreeSet::new();
        for entry in catalogue(&repo)? {
            let effects = entry["effects"].as_array().unwrap();
            let kind = effects.iter().find_map(|effect| {
                effect["kind"]
                    .as_str()
                    .filter(|kind| ["military", "naval", "walls"].contains(kind))
            });
            let Some(kind) = kind else { continue };
            if effects.iter().any(|effect| effect["kind"] == "growth") {
                assert!(make_native_probe(&repo, &entry).is_err());
                continue;
            }
            seen.insert(kind.to_owned());
            let plain = make_probe(&repo, &entry)?;
            let mut native =
                make_native_probe_with_basis(&repo, &entry, NativeProductionBasis::Legacy)?;
            let steps = native["steps"].as_array_mut().unwrap();
            let action = steps.remove(5);
            assert_eq!(action["action"], "oath_exact_removal");
            if kind == "military" {
                assert!(action.get("scope").is_none());
            } else {
                assert_eq!(action["scope"], "naval-buildings-v1");
            }
            for step in steps {
                if let Some(code) = step["code"].as_str() {
                    mlua::Lua::new().load(code).into_function()?;
                    step["code"] = json!(code.replace(
                        "local native_diagnostic=true",
                        "local native_diagnostic=false"
                    ));
                }
            }
            native
                .as_object_mut()
                .unwrap()
                .remove("native_exact_removal_candidate");
            assert_eq!(native, plain, "{}", entry["oath"]);
        }
        assert_eq!(
            seen,
            BTreeSet::from(["military".into(), "naval".into(), "walls".into()])
        );
        Ok(())
    }

    #[test]
    fn native_proofs_reject_other_scopes_and_incomplete_application() -> Result<()> {
        use civ6_autotest_rs::oath_exact_removal::{plan_for_scope, Scope};
        for scope in [Scope::Legacy, Scope::NavalBuildingsV1] {
            let mut proof = plan_for_scope(scope);
            proof["status"] = json!("applied");
            proof["writes"] = json!(4);
            proof["original_protections_restored"] = json!(true);
            proof["pid"] = json!(42);
            proof["creation_filetime"] = json!(123);
            verify_native_proof(&proof, scope)?;
            let other = if scope == Scope::Legacy {
                Scope::NavalBuildingsV1
            } else {
                Scope::Legacy
            };
            assert!(verify_native_proof(&proof, other).is_err());
            for (key, value) in [
                ("writes", json!(3)),
                ("status", json!("failed")),
                ("original_protections_restored", json!(false)),
                ("pid", json!(0)),
                ("creation_filetime", json!(0)),
            ] {
                let mut incomplete = proof.clone();
                incomplete[key] = value;
                assert!(verify_native_proof(&incomplete, scope).is_err(), "{key}");
            }
        }
        Ok(())
    }

    #[test]
    fn native_candidate_keeps_all_numeric_and_save_steps_and_refuses_other_oaths() -> Result<()> {
        let repo = Repository::checked_in();
        let entries = catalogue(&repo)?;
        let entry = entries
            .iter()
            .find(|entry| entry["oath"] == "JAC_OATH_ODA_TENKA_FUBU")
            .context("military oath")?;
        let plain = make_probe(&repo, entry)?;
        let mut native = make_native_probe(&repo, entry)?;
        let steps = native["steps"].as_array_mut().unwrap();
        assert_eq!(
            steps.remove(5),
            json!({"action":"oath_exact_removal","timeout":30})
        );
        let mut conversions = 0;
        for step in steps {
            if let Some(code) = step["code"].as_str() {
                if code.contains("local native_diagnostic=true") {
                    conversions += 1;
                    step["code"] = json!(code.replace(
                        "local native_diagnostic=true",
                        "local native_diagnostic=false"
                    ));
                }
            }
        }
        assert_eq!(conversions, 3);
        native
            .as_object_mut()
            .unwrap()
            .remove("native_exact_removal_candidate");
        assert_eq!(native, plain);
        Ok(())
    }

    #[test]
    fn growth_focus_diagnostic_keeps_plain_probe_and_all_three_numeric_waits() -> Result<()> {
        let repo = Repository::checked_in();
        let growth_oaths = [
            "JAC_OATH_ODA_TSUSHIMA",
            "JAC_OATH_TAKEDA_RYUO",
            "JAC_OATH_ASAKURA_ICHIJODANI",
            "JAC_OATH_ROKKAKU_ISHIDERA",
            "JAC_OATH_HONGANJI_TEMPLE_TOWN",
            "JAC_OATH_MORI_OTA_TOWN",
            "JAC_OATH_OTOMO_FUNAI_HEALING",
            "JAC_OATH_AINU_RIVER_MOUTH",
        ];
        let mut covered = BTreeSet::new();
        for entry in catalogue(&repo)? {
            let oath = entry["oath"].as_str().context("oath id")?;
            if !growth_oaths.contains(&oath) {
                if oath != "JAC_OATH_ODA_TENKA_FUBU" {
                    let effects = entry["effects"].as_array().unwrap();
                    if effects.iter().any(|e| e["kind"] == "growth")
                        || entry["production_targets"].as_array().unwrap().is_empty()
                    {
                        assert!(make_native_probe(&repo, &entry).is_err(), "{oath}");
                    }
                }
                continue;
            }
            covered.insert(oath.to_owned());
            let plain = make_probe(&repo, &entry)?;
            let native = make_native_probe(&repo, &entry)?;
            let steps = native["steps"].as_array().context("native steps")?;
            let focus_positions: Vec<_> = steps
                .iter()
                .enumerate()
                .filter(|(_, step)| step["code"] == GROWTH_FOCUS_RESYNC)
                .map(|(i, _)| i)
                .collect();
            assert_eq!(focus_positions.len(), 3);
            assert_eq!(steps.len(), plain["steps"].as_array().unwrap().len() + 7);
            for position in focus_positions {
                assert_eq!(steps[position - 1]["action"], "wait_for");
                assert!(steps[position - 1]["condition"]["code"]
                    .as_str()
                    .unwrap()
                    .contains("Core growth inventory incomplete"));
                assert_eq!(steps[position - 2]["state_name"], "GameCore_Tuner");
                assert_eq!(steps[position + 1]["action"], "wait_for");
                assert!(steps[position + 1]["condition"]["code"]
                    .as_str()
                    .unwrap()
                    .contains(GROWTH_FOCUS_UNCHANGED));
                assert!(steps[position + 2]["condition"]["code"]
                    .as_str()
                    .unwrap()
                    .contains("previous growth observation missing"));
            }
            assert_eq!(steps.iter().filter(|s| s["action"] == "save").count(), 1);
            assert_eq!(steps.iter().filter(|s| s["action"] == "load").count(), 1);
            assert_eq!(
                steps
                    .iter()
                    .filter(|s| s["action"] == "oath_exact_removal")
                    .count(),
                1
            );
            for step in steps {
                if let Some(code) = step["code"]
                    .as_str()
                    .or_else(|| step["condition"]["code"].as_str())
                {
                    mlua::Lua::new().load(code).into_function()?;
                }
            }
            assert!(!plain.to_string().contains("focus_resync"));
            let mut changed = entry.clone();
            changed["effects"][1]["expected_percent"] = json!(999);
            assert!(make_native_probe(&repo, &changed).is_err());
        }
        assert_eq!(covered.len(), growth_oaths.len());
        Ok(())
    }

    #[test]
    fn focus_resync_replays_only_existing_preferences_once_and_rejects_side_effects() -> Result<()>
    {
        let lua = mlua::Lua::new();
        let fixture = r#"
turn=5; sent={}; preferences={{favored=true,ignored=false},{favored=false,ignored=true}}
core={phase='selected',turn=turn,snapshot={own={{id=1,owner=0,plot=10},{id=2,owner=0,plot=20}}}}
Game={GetProperty=function() return core end,GetCurrentGameTurn=function() return turn end,GetLocalPlayer=function() return 0 end}
GameInfo={Yields={YIELD_FOOD={Index=3}}}
CityCommandTypes={PARAM_FLAGS='flags',PARAM_YIELD_TYPE='yield',PARAM_DATA0='value',SET_FOCUS=8}
local cities={}
for id=1,2 do local n=id; cities[n]={GetOwner=function() return 0 end,GetX=function() return 10*n end,GetY=function() return 0 end,
 GetCitizens=function() return {IsFavoredYield=function(_,yield) assert(yield==3); return preferences[n].favored end,IsDisfavoredYield=function(_,yield) assert(yield==3); return preferences[n].ignored end} end} end
Players={[0]={GetCities=function() return {FindID=function(_,id) return cities[id] end} end}}
Map={GetPlot=function(x) return {GetIndex=function() return x end} end}
CityManager={RequestCommand=function(city,kind,p) assert(kind==8); local id=city:GetX()/10; assert(p.yield==3); local field=p.flags==0 and 'favored' or 'ignored'; assert(p.value==(preferences[id][field] and 1 or 0)); sent[#sent+1]={id=id,flag=p.flags,value=p.value} end}
JAC_OATH_AUDIT={}
"#;
        lua.load(fixture).exec()?;
        lua.load(GROWTH_FOCUS_RESYNC).exec()?;
        assert_eq!(lua.load("return #sent").eval::<i64>()?, 4);
        lua.load(GROWTH_FOCUS_UNCHANGED).exec()?;
        assert!(lua.load(GROWTH_FOCUS_RESYNC).exec().is_err());
        assert_eq!(lua.load("return #sent").eval::<i64>()?, 4);
        for mutation in [
            "preferences[1].favored=false",
            "turn=6",
            "JAC_OATH_AUDIT.focus_resync.selected.sent=3",
        ] {
            lua.load(fixture).exec()?;
            lua.load(GROWTH_FOCUS_RESYNC).exec()?;
            lua.load(mutation).exec()?;
            assert!(
                lua.load(GROWTH_FOCUS_UNCHANGED).exec().is_err(),
                "accepted {mutation}"
            );
        }
        for mutation in [
            "core.snapshot.own[2].owner=1",
            "core.snapshot.own[2]=core.snapshot.own[1]",
            "preferences[1].favored=1",
            "preferences[1].ignored=true",
            "core.turn=4",
        ] {
            lua.load(fixture).exec()?;
            lua.load(mutation).exec()?;
            assert!(
                lua.load(GROWTH_FOCUS_RESYNC).exec().is_err(),
                "accepted {mutation}"
            );
            assert_eq!(lua.load("return #sent").eval::<i64>()?, 0);
        }
        Ok(())
    }

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
GameConfiguration={GetStartTurn=function() return 16603 end}
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
            "GameConfiguration.GetStartTurn=function() return 16604 end",
        ] {
            let vm = vm()?;
            let saved: String = vm.load(&before).eval()?;
            assert!(saved.starts_with("JAC_OATH_SAVE_CORE_BEFORE|"));
            assert!(saved.contains("12:string:start12:number:16603"));
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
        // A frame without the actual start turn is never produced.
        let missing = self::vm()?;
        missing
            .load("GameConfiguration.GetStartTurn=function() return nil end")
            .exec()?;
        assert!(missing.load(&before).eval::<String>().is_err());
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

    /// Shared with the expiry tests: a complete save run whose Core frames are `core`.
    pub(crate) fn artifacts_with(core: &str) -> Result<(tempfile::TempDir, Vec<Value>)> {
        let repo = Repository::checked_in();
        let entry = catalogue(&repo)?.into_iter().next().unwrap();
        artifacts_for(&entry, false, core)
    }

    fn artifacts_for(
        entry: &Value,
        native: bool,
        core: &str,
    ) -> Result<(tempfile::TempDir, Vec<Value>)> {
        artifacts_for_basis(entry, native, core, NativeProductionBasis::Q8)
    }

    fn artifacts_for_basis(
        entry: &Value,
        native: bool,
        core: &str,
        basis: NativeProductionBasis,
    ) -> Result<(tempfile::TempDir, Vec<Value>)> {
        let repo = Repository::checked_in();
        let mut scenario = if native {
            make_native_probe_with_basis(&repo, entry, basis)?
        } else {
            make_probe(&repo, entry)?
        };
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
        let saved = format!("JAC-Todo134-Save-{oath}-agent-01-0123456789ab");
        let mut rows = Vec::new();
        for (offset, s) in steps.iter().enumerate() {
            let tail = if offset >= count - 8 {
                Some(offset - (count - 8))
            } else {
                None
            };
            let core_frame = format!("{}:{core}", core.len());
            let detail = match tail {
                Some(0) => format!(
                    "ran Lua in GameCore_Tuner; return=JAC_OATH_SAVE_CORE_BEFORE|{oath}|{core_frame}|END-JAC-OATH-SAVE"
                ),
                Some(1) => {
                    format!("ran Lua in InGame; return=JAC_OATH_SAVE_UI_BEFORE|{oath}|4:{{ui}}|END-JAC-OATH-SAVE")
                }
                Some(2) => format!("C:\\Users\\agent\\Saves\\single\\{saved}.Civ6Save"),
                Some(4) => format!("loaded {saved}"),
                Some(6) => format!(
                    "ran Lua in GameCore_Tuner; return=JAC_OATH_SAVE_CORE_AFTER|{oath}|{core_frame}|END-JAC-OATH-SAVE"
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

    fn artifacts() -> Result<(tempfile::TempDir, Vec<Value>)> {
        artifacts_with("{core}")
    }

    #[test]
    fn production_verifier_reads_its_own_proof_file_and_rejects_legacy_evidence() -> Result<()> {
        use civ6_autotest_rs::oath_exact_removal::{plan_for_scope, Scope};
        let repo = Repository::checked_in();
        for oath in ["JAC_OATH_MORI_SETO_FLEETS", "JAC_OATH_MIYOSHI_IIMORI"] {
            let entry = catalogue(&repo)?
                .into_iter()
                .find(|e| e["oath"] == oath)
                .context("production test catalogue row")?;
            let (temp, _) = artifacts_for(&entry, true, "{core}")?;
            let mut proof = plan_for_scope(Scope::NavalBuildingsV1);
            proof["status"] = json!("applied");
            proof["writes"] = json!(4);
            proof["original_protections_restored"] = json!(true);
            proof["pid"] = json!(42);
            proof["creation_filetime"] = json!(123);
            let target = temp
                .path()
                .join("oath-exact-removal-naval-buildings-v1.json");
            fs::write(
                temp.path().join("oath-exact-removal.json"),
                serde_json::to_vec(&proof)?,
            )?;
            assert!(verify_artifacts(&repo, temp.path()).is_err());
            fs::write(&target, serde_json::to_vec(&proof)?)?;
            let result = verify_artifacts(&repo, temp.path())?;
            assert_eq!(
                result["native_q8_inverse_and_save_diagnostic_verified"],
                true
            );
            assert_eq!(result["numeric_and_native_save_verified"], false);
            assert_eq!(result["task_accepted"], false);
            fs::write(&target, serde_json::to_vec(&plan_for_scope(Scope::Legacy))?)?;
            assert!(verify_artifacts(&repo, temp.path()).is_err());
        }
        Ok(())
    }

    #[test]
    fn production_basis_versions_bind_source_and_preserve_old_failures() -> Result<()> {
        use civ6_autotest_rs::oath_exact_removal::{plan_for_scope, Scope};
        let repo = Repository::checked_in();
        for oath in ["JAC_OATH_MORI_SETO_FLEETS", "JAC_OATH_MIYOSHI_IIMORI"] {
            let entry = catalogue(&repo)?
                .into_iter()
                .find(|e| e["oath"] == oath)
                .unwrap();
            for basis in [NativeProductionBasis::Legacy, NativeProductionBasis::Q8] {
                let (temp, _) = artifacts_for_basis(&entry, true, "{core}", basis)?;
                let mut proof = plan_for_scope(Scope::NavalBuildingsV1);
                proof["status"] = json!("applied");
                proof["writes"] = json!(4);
                proof["original_protections_restored"] = json!(true);
                proof["pid"] = json!(42);
                proof["creation_filetime"] = json!(123);
                fs::write(
                    temp.path()
                        .join("oath-exact-removal-naval-buildings-v1.json"),
                    serde_json::to_vec(&proof)?,
                )?;
                assert_eq!(
                    verify_artifacts(&repo, temp.path())?["task_accepted"],
                    false
                );
                let path = temp.path().join("scenario.json");
                let original = fs::read(&path)?;
                let mut scenario: Value = serde_json::from_slice(&original)?;
                // A marker cannot relabel old decimal Lua as Q8, or vice versa.
                if basis == NativeProductionBasis::Legacy {
                    scenario["native_production_basis"] = json!("q8-v2");
                } else {
                    scenario
                        .as_object_mut()
                        .unwrap()
                        .remove("native_production_basis");
                }
                fs::write(&path, serde_json::to_vec(&scenario)?)?;
                assert!(verify_artifacts(&repo, temp.path()).is_err());
                scenario["native_production_basis"] = json!("unknown");
                fs::write(&path, serde_json::to_vec(&scenario)?)?;
                assert!(verify_artifacts(&repo, temp.path()).is_err());
                fs::write(path, original)?;
                let result_path = temp.path().join("result.json");
                let mut result: Value = serde_json::from_slice(&fs::read(&result_path)?)?;
                result["failure"] = json!("old actual rate failed nominal acceptance");
                fs::write(result_path, serde_json::to_vec(&result)?)?;
                assert!(verify_artifacts(&repo, temp.path()).is_err());
            }
        }
        Ok(())
    }

    pub(crate) fn write_rows(path: &Path, rows: &[Value]) -> Result<()> {
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
        let verified = verify_artifacts(&repo, temp.path())?;
        assert_eq!(verified["task_accepted"], false);
        assert_eq!(
            verified["saved_name"],
            "JAC-Todo134-Save-JAC_OATH_ODA_TENKA_FUBU-agent-01-0123456789ab"
        );
        let count = original.len();
        for mutation in 0..15 {
            let mut rows = original.clone();
            match mutation {
                0 => {
                    rows[count - 1]["detail"] = json!(rows[count - 1]["detail"]
                        .as_str()
                        .unwrap()
                        .replace("{ui}", "{uI}"))
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
                9 => {
                    rows[count - 1]["detail"] = json!(rows[count - 1]["detail"]
                        .as_str()
                        .unwrap()
                        .replace("|END-JAC-OATH-SAVE", ""));
                }
                // The save OK record is the only source of the actual file name.
                10 => rows[count - 11]["detail"] = json!("operation completed"),
                11 => {
                    rows[count - 11]["detail"] =
                        json!("C:\\Saves\\JAC-Todo134-Save-JAC_OATH_ODA_TENKA_FUBU-agent-01.Civ6Save")
                }
                12 => {
                    rows[count - 11]["detail"] = json!(
                        "C:\\Saves\\JAC-Todo134-Save-JAC_OATH_ODA_TENKA_FUBU-agent-01-0123456789AB.Civ6Save"
                    )
                }
                13 => rows[count - 7]["detail"] = json!("loaded other-save"),
                _ => {
                    rows[count - 3]["detail"] = json!(rows[count - 3]["detail"]
                        .as_str()
                        .unwrap()
                        .replace("{core}", "{corE}"));
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
    fn recorded_name_follows_runner_suffix_or_exact_name() -> Result<()> {
        let step = json!({"action":"save","name":"base-ship"});
        assert_eq!(
            recorded_save_name("D:/s/base-ship-00aa11bb22cc.Civ6Save", &step)?,
            "base-ship-00aa11bb22cc"
        );
        assert!(recorded_save_name("D:/s/base-ship.Civ6Save", &step).is_err());
        let exact = json!({"action":"save","name":"base-ship","exact_name":true});
        assert_eq!(
            recorded_save_name("D:/s/base-ship.Civ6Save", &exact)?,
            "base-ship"
        );
        assert!(recorded_save_name("D:/s/base-ship-00aa11bb22cc.Civ6Save", &exact).is_err());
        let long = "x".repeat(70);
        let step = json!({"action":"save","name":long});
        assert_eq!(
            recorded_save_name(
                &format!("D:/s/{}-00aa11bb22cc.Civ6Save", &long[..67]),
                &step
            )?
            .len(),
            80
        );
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
