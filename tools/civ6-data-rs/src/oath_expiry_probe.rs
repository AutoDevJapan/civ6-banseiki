//! Natural reign-change expiry of a selected oath, generated from a verified
//! quantity + native save run. The scenario loads the recorded save, fast-forwards
//! only engine turns to the turn before the due 改元, reaches the due turn with
//! one human end-turn request and observes the product's own Judge/Reset/
//! WriteProperties. Nothing here writes the ledger, ages, reigns, oaths, flags or
//! history; the only property written is the probe's own judgement snapshot.
use crate::oath_save_probe::{self, core_code, loaded_ready, lua, Evidence, HELPERS};
use crate::{reigns, Repository};
use anyhow::{bail, ensure, Context, Result};
use civ6_autotest_rs::runner::{CORE_PLAYER0_READY, HUMAN_TURN_READY_UI};
use serde_json::{json, Value};
use std::{ops::Range, path::Path};

/// One value of the canonical `ser` frame: `LEN:kind:text` scalars and sorted
/// `{key value ...}` tables. Spans index the source text of each table value.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    Bool(bool),
    Num(f64),
    Str(String),
    Table(Vec<(Frame, Frame, Range<usize>)>),
}

impl Frame {
    pub fn parse(text: &str) -> Result<Frame> {
        let mut at = 0;
        let frame = parse_value(text.as_bytes(), &mut at)?;
        ensure!(at == text.len(), "trailing bytes after the canonical frame");
        Ok(frame)
    }
    fn entry(&self, key: &str) -> Option<&(Frame, Frame, Range<usize>)> {
        match self {
            Frame::Table(entries) => entries
                .iter()
                .find(|(k, _, _)| matches!(k, Frame::Str(s) if s == key)),
            _ => None,
        }
    }
    pub fn get(&self, key: &str) -> Result<&Frame> {
        self.entry(key)
            .map(|(_, v, _)| v)
            .with_context(|| format!("frame field {key} missing"))
    }
    pub fn int(&self) -> Result<i64> {
        match self {
            Frame::Num(n) if n.fract() == 0.0 && n.abs() < 9.0e15 => Ok(*n as i64),
            _ => bail!("integer frame value required"),
        }
    }
    pub fn text(&self) -> Result<&str> {
        match self {
            Frame::Str(s) => Ok(s),
            _ => bail!("string frame value required"),
        }
    }
    pub fn entries(&self) -> Result<&[(Frame, Frame, Range<usize>)]> {
        match self {
            Frame::Table(entries) => Ok(entries),
            _ => bail!("table frame value required"),
        }
    }
}

fn parse_value(s: &[u8], at: &mut usize) -> Result<Frame> {
    if s.get(*at) == Some(&b'{') {
        *at += 1;
        let mut entries: Vec<(Frame, Frame, Range<usize>)> = Vec::new();
        loop {
            match s.get(*at) {
                Some(b'}') => {
                    *at += 1;
                    return Ok(Frame::Table(entries));
                }
                None => bail!("unterminated frame table"),
                _ => {}
            }
            let key = parse_value(s, at)?;
            ensure!(
                !matches!(key, Frame::Table(_)),
                "frame keys must be typed scalars"
            );
            ensure!(
                entries.iter().all(|(k, _, _)| *k != key),
                "duplicate frame key"
            );
            let start = *at;
            let value = parse_value(s, at)?;
            entries.push((key, value, start..*at));
        }
    }
    let digits = s[*at..].iter().take_while(|b| b.is_ascii_digit()).count();
    ensure!(
        digits > 0 && s.get(*at + digits) == Some(&b':'),
        "frame scalar length missing"
    );
    let length: usize = std::str::from_utf8(&s[*at..*at + digits])?.parse()?;
    *at += digits + 1;
    let body = s
        .get(*at..*at + length)
        .context("frame scalar shorter than its length")?;
    *at += length;
    let (kind, text) = std::str::from_utf8(body)?
        .split_once(':')
        .context("frame scalar kind missing")?;
    Ok(match kind {
        "boolean" if text == "true" || text == "false" => Frame::Bool(text == "true"),
        "number" => {
            let n: f64 = text.parse()?;
            ensure!(n.is_finite(), "nonfinite frame number");
            Frame::Num(n)
        }
        "string" => Frame::Str(text.to_owned()),
        _ => bail!("unsupported frame scalar {kind}"),
    })
}

/// Everything the scenario needs, derived from the verified frame and checked-in data.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub oath: String,
    pub saved_name: String,
    pub speed: String,
    pub start: i64,
    pub saved_turn: i64,
    pub reign: i64,
    pub reign_type: String,
    pub old_reign_start: i64,
    pub due: i64,
    /// History entries of the saved ledger, as their canonical text.
    pub history: Vec<(String, String)>,
}

impl Plan {
    pub fn elapsed(&self) -> i64 {
        self.saved_turn - self.start + 1
    }
    pub fn fast_forward(&self) -> i64 {
        self.due - 1 - self.elapsed()
    }
}

fn speed(source: &Value) -> Result<String> {
    let configured: Vec<_> = source["steps"]
        .as_array()
        .context("source steps")?
        .iter()
        .filter(|s| s["action"] == "configure")
        .collect();
    ensure!(
        configured.len() == 1,
        "source needs exactly one configure step"
    );
    Ok(configured[0]["speed"]
        .as_str()
        .context("source game speed")?
        .to_owned())
}

/// The independent due: SSOT reign dates -> era lengths -> engine turns at the
/// speed (Eras_XP1 x CostMultiplier, at least 2 turns), as reigns.rs generates.
pub fn due_of(repo: &Repository, speed: &str, reign: i64) -> Result<(i64, String)> {
    let data = reigns::load(repo)?;
    let list = data["reigns"].as_array().context("reigns")?;
    let dates: Vec<_> = list
        .iter()
        .map(|r| (r["year"].as_i64(), r["month"].as_i64()))
        .collect();
    ensure!(
        dates.windows(2).all(|w| w[0] < w[1]),
        "reigns must be in date order"
    );
    let index = usize::try_from(reign - 1).context("judged reign index")?;
    let turns = reigns::engine_turns(repo, speed)?;
    let due = *turns
        .get(index)
        .context("no reign after the saved period")?;
    let id = list[index]["id"].as_str().context("reign id")?;
    Ok((due, format!("REIGN_{id}")))
}

pub fn plan(repo: &Repository, evidence: &Evidence) -> Result<Plan> {
    let text = &evidence.core_after;
    let frame = Frame::parse(text)?;
    let keys: Vec<_> = frame
        .entries()?
        .iter()
        .map(|(k, _, _)| k.text().map(str::to_owned))
        .collect::<Result<_>>()?;
    ensure!(
        keys.len() == 5
            && ["oath", "turn", "start", "ledger", "cities"]
                .iter()
                .all(|k| keys.iter().any(|x| x == k)),
        "frame lacks the actual oath, turn, start, ledger or cities"
    );
    ensure!(
        frame.get("oath")?.text()? == evidence.oath,
        "frame oath differs"
    );
    let ledger = frame.get("ledger")?;
    let reign = ledger.get("reign")?.int()?;
    let houses = ledger.get("houses")?;
    ensure!(houses.entries()?.len() == 22, "frame needs 22 houses");
    let human = houses.get("p0")?;
    let oaths = human.get("oaths")?.entries()?;
    ensure!(
        reign >= 1
            && human.get("age")?.text()? == "GOLDEN"
            && human.get("oathChosen")?.int()? == reign
            && oaths.len() == 1
            && oaths[0].0 == Frame::Str(evidence.oath.clone())
            && oaths[0].1 == Frame::Bool(true),
        "frame lacks the actual golden selection of the oath"
    );
    let history = ledger
        .get("history")?
        .entries()?
        .iter()
        .map(|(k, _, span)| Ok((k.text()?.to_owned(), text[span.clone()].to_owned())))
        .collect::<Result<Vec<_>>>()?;
    let speed = speed(&evidence.source)?;
    let next = reign + 1;
    let (due, reign_type) = due_of(repo, &speed, next)?;
    let plan = Plan {
        oath: evidence.oath.clone(),
        saved_name: evidence.saved_name.clone(),
        speed,
        start: frame.get("start")?.int()?,
        saved_turn: frame.get("turn")?.int()?,
        reign: next,
        reign_type,
        old_reign_start: ledger.get("reignStart")?.int()?,
        due,
        history,
    };
    ensure!(
        plan.elapsed() >= 1 && plan.fast_forward() >= 0,
        "the saved turn is not before the due turn"
    );
    Ok(plan)
}

fn lua_str(s: &str) -> String {
    let mut out = String::from("'");
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b" _-.:{}|,+=".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("\\{b:03}"));
        }
    }
    out.push('\'');
    out
}

const COMMON: &str = r#"
local oath=__OATH__; local due=__DUE__; local reign=__REIGN__; local oldStart=__OLD_START__
local old=__OLD_HISTORY__
assert(GameConfiguration.GetStartTurn()==__START__,'start turn differs from the verified frame')
local function elapsed() return Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1 end
local rows,ages={},{}
for row in GameInfo.JAC_SengokuOaths() do rows[#rows+1]=row end
assert(#rows==66,'expected 66 loaded oaths')
for row in GameInfo.JAC_SengokuOathAges() do ages[row.OathType]=ages[row.OathType] or {}; ages[row.OathType][row.AgeType]=true end
local function count(t) local n=0; for _ in pairs(t or {}) do n=n+1 end; return n end
local function choices(h)
 local list={}
 for _,row in ipairs(rows) do if row.CivilizationType==h.civ and ages[row.OathType] and ages[row.OathType][h.age] then list[#list+1]=row.OathType end end
 return list
end
-- AssignDefault: every offered oath when heroic, otherwise exactly one offered oath.
local function legalDefault(h)
 local offered=choices(h); local set={}
 for _,o in ipairs(offered) do set[o]=true end
 if #offered==0 then return false end
 for o,v in pairs(h.oaths or {}) do if v~=true or not set[o] then return false end end
 return count(h.oaths)==(h.age=='HEROIC' and #offered or 1)
end
local function strong(h) return h.age=='GOLDEN' or h.age=='HEROIC' end
local function history(l,extra)
 assert(count(l.history)==#old+extra,'judgement history count changed')
 for _,e in ipairs(old) do assert(l.history[e.k]~=nil and ser(l.history[e.k])==e.s,'old judgement history changed') end
end
-- Every real city of every alive major against its house in the ledger.
local function flags(l)
 local majors=PlayerManager.GetAliveMajors(); assert(#majors==22,'expected 22 real houses')
 local cities,human,seen=0,0,{}
 for _,p in ipairs(majors) do
  local id=p:GetID(); local h=assert(l.houses['p'..id],'alive house missing from ledger')
  for _,c in p:GetCities():Members() do
   local plot=assert(Map.GetPlot(c:GetX(),c:GetY()))
   assert(c:GetOwner()==id and plot:GetOwner()==id,'owner/plot mismatch')
   assert(not seen[plot:GetIndex()],'duplicate city plot'); seen[plot:GetIndex()]=true
   for _,row in ipairs(rows) do
    local on=row.CivilizationType==h.civ and strong(h) and h.oaths and h.oaths[row.OathType]==true
    assert(plot:GetProperty(row.OathType..'_ON')==(on and 1 or 0),'oath flag disagrees with the ledger')
   end
   cities=cities+1; if id==0 then human=human+1 end
  end
 end
 assert(human>0,'human has no real city')
 return cities,human
end
local l=assert(Game:GetProperty('JAC_AGES_LEDGER'),'saved age ledger missing')
assert(count(l.houses)==22,'ledger house inventory changed')
assert(Players[0]:IsHuman(),'player 0 must stay human')
local h=assert(l.houses.p0)
"#;

const CALC: &str = r#"
assert(Game.GetCurrentGameTurn()==__SAVED_TURN__,'loaded turn differs from the verified frame')
assert(l.reign==reign-1 and h.oathChosen==reign-1 and h.oaths[oath]==true and count(h.oaths)==1,'loaded selection differs from the verified frame')
history(l,0)
local speed=assert(GameInfo.GameSpeeds[GameConfiguration.GetGameSpeedType()])
assert(speed.GameSpeedType==__SPEED__,'game speed differs from the source scenario')
-- Same derivation as JAC_Sengoku_Ages.lua ERA_START and REIGNS, from the loaded GameInfo.
local eras={}; for row in GameInfo.Eras() do eras[#eras+1]=row end
table.sort(eras,function(a,b) return a.ChronologyIndex<b.ChronologyIndex end)
local starts,turn={},1
for _,era in ipairs(eras) do
 starts[era.Index]=turn
 local xp1=GameInfo.Eras_XP1[era.EraType]
 if xp1==nil or xp1.GameEraMaximumTurns==nil then break end
 turn=turn+math.max(2,math.floor(xp1.GameEraMaximumTurns*speed.CostMultiplier/100))
end
local reigns={}
for row in GameInfo.JAC_SengokuReigns() do reigns[#reigns+1]={type=row.ReignType,date=row.Year*12+row.Month,due=starts[GameInfo.Eras[row.EraType].Index]} end
table.sort(reigns,function(a,b) if a.date~=b.date then return a.date<b.date end return a.type<b.type end)
local r=assert(reigns[reign],'judged reign missing')
assert(r.type==__REIGN_TYPE__ and r.due==due,'product reign due differs from the independent calculation')
assert(elapsed()<due,'due already reached at the saved turn')
return 'JAC_OATH_EXPIRY_CALC|'..oath..'|reign='..reign..'|due='..due..'|elapsed='..elapsed()..'|saved_next_due='..tostring(l.nextDue)
"#;

const PRE: &str = r#"
assert(elapsed()==due-1,'fast-forward did not stop on the turn before the due turn')
assert(l.reign==reign-1,'a judgement ran before the due turn')
history(l,0)
assert(h.age=='GOLDEN' and h.oathReign==reign-1 and h.oathChosen==reign-1 and h.oaths[oath]==true and count(h.oaths)==1,'selected oath changed before the due turn')
local cities,human=flags(l)
return 'JAC_OATH_EXPIRY_PRE|'..oath..'|elapsed='..elapsed()..'|cities='..cities..'|human_cities='..human
"#;

const DUE: &str = r#"
assert(elapsed()==due,'one human request did not reach the due turn')
assert(l.reign==reign and l.reignStart==due,'natural judgement did not advance exactly one reign')
history(l,1)
local r=assert(l.history['r'..reign],'natural judgement record missing')
assert(r.turn==due and r.due==due and r.from==reign-1 and r.length==due-oldStart,'judgement record differs from the natural due turn')
local results=0
for k in pairs(r) do if type(k)=='string' and string.match(k,'^p%d+$') then results=results+1 end end
assert(results==22,'judgement must cover all 22 houses')
local defaults=0
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
 local id=p:GetID(); local house=assert(l.houses['p'..id]); local result=assert(r['p'..id],'house judgement missing')
 assert(result.age==house.age and house.oathReign==reign,'house was not reset by the judgement')
 if id==0 then
  -- The expiry itself: the human chooses again, so nothing is selected yet.
  assert(next(house.oaths or {})==nil and house.oathChosen==nil and house.oathDue==due+1,'human oath survived the reign change')
 elseif #choices(house)>0 then
  assert(house.oathChosen==reign and legalDefault(house),'AI default missing for the new reign'); defaults=defaults+1
 else
  assert(next(house.oaths or {})==nil and house.oathChosen==nil,'AI oath without an offered choice')
 end
end
local cities,human=flags(l)
local key='JAC_TODO134_EXPIRY_JUDGEMENT'
assert(Game:GetProperty(key)==nil,'expiry probe may not replace an existing judgement snapshot')
Game:SetProperty(key,ser(r))
return 'JAC_OATH_EXPIRY_DUE|'..oath..'|reign='..reign..'|human_age='..h.age..'|human_selection=empty|old_flag=0|ai_defaults='..defaults..'|cities='..cities..'|human_cities='..human
"#;

const NEXT: &str = r#"
assert(elapsed()==due+1,'auxiliary request did not reach exactly the next turn')
assert(l.reign==reign,'more than one judgement ran')
history(l,1)
assert(ser(l.history['r'..reign])==Game:GetProperty('JAC_TODO134_EXPIRY_JUDGEMENT'),'the due judgement record changed')
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
 local id=p:GetID(); local house=assert(l.houses['p'..id])
 assert(house.oathReign==reign,'house left the judged reign')
 -- After the deadline the human gets the same deterministic default as the AI; it may legally be the old oath.
 if #choices(house)>0 then assert(house.oathChosen==reign and legalDefault(house),'selection after the deadline is not a legal default')
 else assert(next(house.oaths or {})==nil and house.oathChosen==nil,'oath without an offered choice') end
end
local cities=flags(l)
return 'JAC_OATH_EXPIRY_NEXT|'..oath..'|human_default_legal=true|same_oath_reselected='..tostring(h.oaths[oath]==true)..'|flag='..((strong(h) and h.oaths[oath]) and 1 or 0)..'|cities='..cities
"#;

const END_TURN: &str = r#"
local n=__N__; local expected=__ELAPSED__
JAC_OATH_EXPIRY_REQUESTS=JAC_OATH_EXPIRY_REQUESTS or {}
local requests=JAC_OATH_EXPIRY_REQUESTS
assert(requests[n]==nil,'duplicate human end-turn request')
for i=1,n-1 do assert(requests[i],'earlier human request missing') end
local turn=Game.GetCurrentGameTurn()
assert(turn-GameConfiguration.GetStartTurn()+1==expected and Players[0]:IsTurnActive() and Players[0]:IsHuman(),'human request from the wrong turn')
requests[n]={turn=turn}
-- Official Shift+Enter path: Base/Assets/UI/ActionPanel.lua:1099. One request, no AI hand-over.
UI.RequestAction(ActionTypes.ACTION_ENDTURN,{REASON='UserForced'})
return 'one native human end-turn requested: '..n
"#;

fn probe(plan: &Plan, body: &str) -> String {
    let old = plan
        .history
        .iter()
        .map(|(k, s)| format!("{{k={},s={}}}", lua_str(k), lua_str(s)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{HELPERS}{COMMON}{body}")
        .replace("__OATH__", &lua_str(&plan.oath))
        .replace("__DUE__", &plan.due.to_string())
        .replace("__REIGN_TYPE__", &lua_str(&plan.reign_type))
        .replace("__REIGN__", &plan.reign.to_string())
        .replace("__OLD_START__", &plan.old_reign_start.to_string())
        .replace("__OLD_HISTORY__", &format!("{{{old}}}"))
        .replace("__SAVED_TURN__", &plan.saved_turn.to_string())
        .replace("__START__", &plan.start.to_string())
        .replace("__SPEED__", &lua_str(&plan.speed))
}

fn ready(steps: &mut Vec<Value>) {
    for (context, code) in [
        ("GameCore_Tuner", CORE_PLAYER0_READY),
        ("InGame", HUMAN_TURN_READY_UI),
    ] {
        steps.push(json!({"action":"wait_for","timeout":180,"close_diplomacy":true,"condition":{"type":"lua","state_name":context,"code":code}}));
    }
}

fn request(steps: &mut Vec<Value>, n: u8, elapsed: i64) {
    ready(steps);
    steps.push(lua(
        END_TURN
            .replace("__N__", &n.to_string())
            .replace("__ELAPSED__", &elapsed.to_string()),
        "InGame",
    ));
    steps.push(json!({"action":"wait_for","timeout":180,"close_diplomacy":true,"condition":{"type":"lua","state_name":"InGame","code":format!("local r=JAC_OATH_EXPIRY_REQUESTS and JAC_OATH_EXPIRY_REQUESTS[{n}]; if not r then return false end; local turn=Game.GetCurrentGameTurn(); assert(turn<=r.turn+1,'native request advanced more than one turn'); return turn==r.turn+1 and Players[0]:IsTurnActive()")}}));
    ready(steps);
}

pub fn scenario(evidence: &Evidence, plan: &Plan) -> Result<Value> {
    let source_steps = evidence.source["steps"]
        .as_array()
        .context("source steps")?;
    ensure!(
        source_steps
            .first()
            .is_some_and(|s| s["action"] == "enable_mods"),
        "source scenario must start with its enable_mods step"
    );
    let mut steps = vec![
        source_steps[0].clone(),
        json!({"action":"load","name":plan.saved_name,"exact_name":true,"timeout":600}),
        loaded_ready(),
        // The loaded state must still be the verified frame, byte for byte.
        lua(core_code(&plan.oath, true), "GameCore_Tuner"),
        lua(probe(plan, CALC), "GameCore_Tuner"),
    ];
    if plan.fast_forward() > 0 {
        steps.push(json!({"action":"autoplay","turns":plan.fast_forward(),"fast_forward":true,"timeout":1800}));
    }
    ready(&mut steps);
    steps.push(lua(probe(plan, PRE), "GameCore_Tuner"));
    request(&mut steps, 1, plan.due - 1);
    steps.push(lua(probe(plan, DUE), "GameCore_Tuner"));
    request(&mut steps, 2, plan.due);
    steps.push(lua(probe(plan, NEXT), "GameCore_Tuner"));
    let out = json!({
        "name": format!("誓約 {}：正式保存から自然な改元での失効を有限確認する", plan.oath),
        "judge_profile": "generic",
        "diagnostic_only": true,
        "finite_turns": plan.fast_forward() + 2,
        "test_mode": {"skipped_turns":0,"continuous_performance_valid":false},
        "expiry_probe": {
            "oath": plan.oath,
            "scope": "natural_reign_ledger_and_flags_only",
            "source_save_probe": evidence.source["save_probe"],
            "saved_name": plan.saved_name,
            "speed": plan.speed,
            "start_turn": plan.start,
            "saved_turn": plan.saved_turn,
            "saved_elapsed": plan.elapsed(),
            "judged_reign": plan.reign,
            "reign_type": plan.reign_type,
            "due_elapsed": plan.due,
            "fast_forward_request_turns": plan.fast_forward(),
            "human_end_turn_requests": 2,
            "synthetic_inputs": ["source_fixture_reign_1_golden"],
            "skipped_turns_source": "runner result test_mode.skipped_turns",
            "continuous_performance_valid": false,
            "task_accepted": false,
            "remaining": ["live_run_and_artifact_verification", "native_quantity_before_off_after_across_expiry", "foreign_specialty_queue"]
        },
        "steps": steps,
    });
    civ6_autotest_rs::scenario::validate_scenario(&out)?;
    Ok(out)
}

fn compile(scenario: &Value) -> Result<()> {
    let vm = mlua::Lua::new();
    for step in scenario["steps"].as_array().unwrap() {
        if let Some(code) = step["code"]
            .as_str()
            .or_else(|| step["condition"]["code"].as_str())
        {
            vm.load(code).into_function()?;
        }
    }
    Ok(())
}

pub fn run(repo: &Repository, args: &[String]) -> Result<()> {
    if args == ["--check"] {
        // Schema and Lua 5.1 for every oath, from each source's own speed. The
        // frame values here are placeholders; a real plan comes only from artifacts.
        for entry in oath_save_probe::catalogue(repo)? {
            let source = oath_save_probe::make_probe(repo, &entry)?;
            let speed = speed(&source)?;
            let (due, reign_type) = due_of(repo, &speed, 2)?;
            let evidence = Evidence {
                oath: entry["oath"].as_str().unwrap().to_owned(),
                source,
                saved_name: "JAC-Todo134-Save-check".into(),
                core_after: String::new(),
            };
            let plan = Plan {
                oath: evidence.oath.clone(),
                saved_name: evidence.saved_name.clone(),
                speed,
                start: 1,
                saved_turn: 1,
                reign: 2,
                reign_type,
                old_reign_start: 1,
                due,
                history: vec![],
            };
            compile(&scenario(&evidence, &plan)?)?;
        }
        println!("66 expiry probes: driver schema and Lua 5.1 compile passed; live unverified");
        return Ok(());
    }
    if args.len() == 4 && args[0] == "--from-artifacts" && args[2] == "--output" {
        let evidence = oath_save_probe::verify(repo, Path::new(&args[1]))?;
        let plan = plan(repo, &evidence)?;
        let out = scenario(&evidence, &plan)?;
        compile(&out)?;
        oath_save_probe::write_new(Path::new(&args[3]), &out)?;
        println!(
            "{}: due {} (elapsed {} -> fast-forward {} + 2 human requests); diagnostic only",
            plan.oath,
            plan.due,
            plan.elapsed(),
            plan.fast_forward()
        );
        return Ok(());
    }
    bail!("usage: generate-oath-expiry-probes --check | --from-artifacts SAVE_PROBE_ARTIFACT_DIR --output NEW.json")
}

#[cfg(test)]
#[path = "oath_expiry_probe_tests.rs"]
mod tests;
