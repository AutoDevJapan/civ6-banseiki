//! The generated probes run against the real JAC_Sengoku_Ages.lua and
//! JAC_Sengoku_Oaths.lua in Lua 5.1, with the checked-in oath and reign data.
//! Only the source quantity fixture (reign 1 / GOLDEN) is synthetic; every later
//! judgement comes from the product's own OnGameTurnStarted.
use super::*;
use crate::oath_save_probe::tests::{artifacts_with, write_rows};
use std::fs;

const OATH: &str = "JAC_OATH_ODA_TENKA_FUBU";
const MOD: &str = "poc/JAC_PoC_SengokuTrees";

/// Checked-in oath catalogue/ages (from the product SQL), eras and reigns as Lua rows.
fn data_chunk(repo: &Repository, no_dark_civ: &str) -> Result<String> {
    let sql = repo.read(format!("{MOD}/Data/JAC_Sengoku_Oaths.sql"))?;
    let db = rusqlite::Connection::open_in_memory()?;
    for statement in sql.split(';') {
        let statement: String = statement
            .lines()
            .filter(|line| !line.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n");
        let head = statement.trim_start();
        if ["JAC_SengokuOaths ", "JAC_SengokuOathAges "]
            .iter()
            .any(|t| {
                head.starts_with(&format!("CREATE TABLE {t}"))
                    || head.starts_with(&format!("INSERT INTO {t}"))
            })
        {
            db.execute_batch(&statement)?;
        }
    }
    let mut out = String::from("OATH_ROWS={");
    let mut query = db.prepare("SELECT OathType, CivilizationType, Position, QuestType, AIFocus FROM JAC_SengokuOaths ORDER BY rowid")?;
    let mut rows = query.query([])?;
    while let Some(r) = rows.next()? {
        out += &format!(
            "{{OathType={},CivilizationType={},Position={},QuestType={},AIFocus={}}},",
            lua_str(&r.get::<_, String>(0)?),
            lua_str(&r.get::<_, String>(1)?),
            r.get::<_, i64>(2)?,
            lua_str(&r.get::<_, String>(3)?),
            lua_str(&r.get::<_, String>(4)?)
        );
    }
    out += "}\nAGE_ROWS={";
    let mut query = db.prepare("SELECT a.OathType, a.AgeType FROM JAC_SengokuOathAges a JOIN JAC_SengokuOaths o USING (OathType) WHERE NOT (o.CivilizationType=?1 AND a.AgeType='DARK') ORDER BY a.rowid")?;
    let mut rows = query.query([no_dark_civ])?;
    while let Some(r) = rows.next()? {
        out += &format!(
            "{{OathType={},AgeType={}}},",
            lua_str(&r.get::<_, String>(0)?),
            lua_str(&r.get::<_, String>(1)?)
        );
    }
    out += "}\nERA_ROWS={";
    let eras = reigns::eras(repo)?;
    let mut xp1 = String::from("XP1_ROWS={");
    for (index, era) in eras.iter().enumerate() {
        let era_type = lua_str(era["EraType"].as_str().unwrap());
        out += &format!(
            "{{EraType={era_type},Index={index},ChronologyIndex={}}},",
            era["ChronologyIndex"]
        );
        xp1 += &match era["length"].as_i64() {
            Some(n) => format!("{{EraType={era_type},GameEraMaximumTurns={n}}},"),
            None => format!("{{EraType={era_type}}},"),
        };
    }
    out += "}\n";
    out += &xp1;
    out += "}\nREIGN_ROWS={";
    let data = reigns::load(repo)?;
    for (index, reign) in data["reigns"].as_array().unwrap().iter().enumerate() {
        let id = reign["id"].as_str().unwrap();
        out += &format!(
            "{{ReignType={},Year={},Month={},EraType={},Notification={}{}}},",
            lua_str(&format!("REIGN_{id}")),
            reign["year"],
            reign["month"],
            lua_str(eras[index + 1]["EraType"].as_str().unwrap()),
            lua_str(&format!("LOC_JAC_REIGN_{id}_NOTIFICATION")),
            reign["forced_age"]
                .as_str()
                .map(|f| format!(",ForcedAge={}", lua_str(f)))
                .unwrap_or_default()
        );
    }
    out += "}\n";
    Ok(out)
}

const ENGINE: &str = r#"
print=function() end
START=16603; turn=START; props={}; handlers={}
local function copy(v) if type(v)~='table' then return v end local t={} for k,x in pairs(v) do t[k]=copy(x) end return t end
local function info(rows,key)
 local t=setmetatable({},{__call=function() local i=0; return function() i=i+1; return rows[i] end end})
 if key then for _,r in ipairs(rows) do t[r[key]]=r end end
 return t
end
TECHS={}; for i=1,120 do TECHS[i]={Index=i} end
GameInfo={JAC_SengokuOaths=info(OATH_ROWS,'OathType'),JAC_SengokuOathAges=info(AGE_ROWS),Eras=info(ERA_ROWS,'EraType'),
 Eras_XP1=info(XP1_ROWS,'EraType'),JAC_SengokuReigns=info(REIGN_ROWS,'ReignType'),
 GameSpeeds=info({{GameSpeedType='GAMESPEED_STANDARD',CostMultiplier=100}},'GameSpeedType'),
 JAC_SengokuDarkAgePolicies=info({{PolicyType='JAC_AGE_POLICY_TEST',Priority=1,Announcement='LOC_TEST'}},'PolicyType'),
 JAC_SengokuDarkAgePolicyDefaults=info({}),JAC_SengokuAgeEventScores=info({}),Technologies=info(TECHS),Civics=info({}),Yields={}}
for i,k in ipairs({'PRODUCTION','GOLD','SCIENCE','CULTURE','FAITH','FOOD'}) do GameInfo.Yields['YIELD_'..k]={Index=i} end
GameConfiguration={GetStartTurn=function() return START end,GetGameSpeedType=function() return 'GAMESPEED_STANDARD' end}
-- The engine's own era starts (independent of the product's ERA_START table).
ERA_SEQ={}
do local t=1; local sorted={}; for _,r in ipairs(ERA_ROWS) do sorted[#sorted+1]=r end
 table.sort(sorted,function(a,b) return a.ChronologyIndex<b.ChronologyIndex end)
 for _,r in ipairs(sorted) do ERA_SEQ[#ERA_SEQ+1]={index=r.Index,start=t}; local x=GameInfo.Eras_XP1[r.EraType]; if not x.GameEraMaximumTurns then break end; t=t+math.max(2,math.floor(x.GameEraMaximumTurns)) end end
local function engineEra() local e=turn-START+1; local current; for _,x in ipairs(ERA_SEQ) do if x.start<=e then current=x.index end end; return current end
Game={GetCurrentGameTurn=function() return turn end,GetLocalPlayer=function() return 0 end,
 GetProperty=function(_,k) return copy(props[k]) end,SetProperty=function(_,k,v) props[k]=copy(v) end,
 GetEras=function() return {GetCurrentEra=function() return engineEra() end} end}
CIVS={}; do local seen={}; for _,r in ipairs(OATH_ROWS) do if not seen[r.CivilizationType] then seen[r.CivilizationType]=true; CIVS[#CIVS+1]=r.CivilizationType end end end
assert(#CIVS==22)
-- Research for houses 0-2 grows with the game-relative turn, including skipped engine turns.
local function knowledge(id) if id<=2 then return math.min(#TECHS,turn-START+1) end return 0 end
plots={}; Players={}; PlayerConfigurations={}; MAJORS={}
for id=0,21 do
 local list={}
 for n=1,(id<12 and 3 or 2) do
  local index=id*10+n; local props={}
  plots[index]={GetIndex=function() return index end,GetOwner=function() return id end,GetProperty=function(_,k) return props[k] end,SetProperty=function(_,k,v) props[k]=v end,IsCoastalLand=function() return false end}
  list[#list+1]={GetOwner=function() return id end,GetID=function() return index end,GetX=function() return index end,GetY=function() return 0 end,GetYield=function(_,kind) return 10+kind end}
 end
 local cities={Members=function() local i=0; return function() i=i+1; if list[i] then return i,list[i] end end end,GetCount=function() return #list end,GetCapitalCity=function() return list[1] end}
 local p={GetID=function() return id end,GetCities=function() return cities end,IsHuman=function() return id==0 end,IsTurnActive=function() return true end,IsTurnActiveComplete=function() return true end,
  GetTechs=function() return {HasTech=function(_,i) return i<=knowledge(id) end} end,GetCulture=function() return {HasCivic=function() return false end} end,
  GetDiplomacy=function() return {HasAllied=function() return false end,IsAtWarWith=function() return false end} end,GetTeam=function() return id end,
  GetTreasury=function() return {GetGoldBalance=function() return 100 end} end,GetProperty=function() return nil end}
 Players[id]=p; MAJORS[#MAJORS+1]=p
 PlayerConfigurations[id]={GetCivilizationTypeName=function() return CIVS[id+1] end}
end
PlayerManager={GetAliveMajors=function() return MAJORS end,GetAlive=function() return MAJORS end}
Map={GetPlot=function(x,y) return plots[x] end,GetPlotByIndex=function(i) return plots[i] end}
NotificationManager={SendNotification=function() end}; NotificationTypes={USER_DEFINED_2=2,USER_DEFINED_3=3}
ParameterTypes={MESSAGE='m',SUMMARY='s'}; NotificationParameterTypes={CanUserDismiss='c'}
GameEvents=setmetatable({},{__index=function(_,name) return {Add=function(f) handlers[name]=f end} end})
include=function() assert(loadstring(OATHS_SRC))() end
UI={CanEndTurn=function() return true end,HasSentTurnComplete=function() return false end,IsProcessingMessages=function() return false end,
 RequestAction=function() turn=turn+1; handlers.OnGameTurnStarted() end}
ActionTypes={ACTION_ENDTURN=1}
function advance(n) for _=1,n do turn=turn+1; handlers.OnGameTurnStarted() end end
-- civ6-autotest-native fast_forward: jump engine turns to two before the target or era boundary, then autoplay.
function fastForward(n)
 local target=turn+n
 while turn<target do
  local e=turn-START+1; local boundary
  for _,x in ipairs(ERA_SEQ) do if x.start>e then boundary=START+x.start-1; break end end
  local limit=boundary and math.min(target-2,boundary-2) or target-2
  if limit>turn then turn=limit end
  for _=1,math.min(2,target-turn) do turn=turn+1; handlers.OnGameTurnStarted() end
 end
end
"#;

/// The source quantity run: its one synthetic age fixture, the real choice event and 8 turns.
const SOURCE_RUN: &str = r#"
handlers.OnGameTurnStarted()
local l=Game:GetProperty('JAC_AGES_LEDGER'); local h=l.houses.p0
assert(l.reign==0)
l.reign=1; h.age='GOLDEN'; h.oaths={}; h.oathReign=1; h.oathChosen=nil; h.oathDue=turn+20
-- Test coverage only: a dark AI can become HEROIC at the natural judgement.
l.houses.p2.age='DARK'
Game:SetProperty('JAC_AGES_LEDGER',l)
handlers.JAC_SengokuOathsChoose(0,{Reign=1,Oath1=__OATH__})
advance(8)
"#;

fn game(repo: &Repository) -> Result<mlua::Lua> {
    let vm = mlua::Lua::new();
    vm.load(data_chunk(repo, "CIVILIZATION_JAC_SHIMAZU")?)
        .exec()?;
    vm.globals().set(
        "OATHS_SRC",
        repo.read(format!("{MOD}/Scripts/JAC_Sengoku_Oaths.lua"))?,
    )?;
    vm.load(ENGINE).exec()?;
    vm.load(repo.read(format!("{MOD}/Scripts/JAC_Sengoku_Ages.lua"))?)
        .exec()?;
    vm.load(SOURCE_RUN.replace("__OATH__", &lua_str(OATH)))
        .exec()?;
    Ok(vm)
}

struct Setup {
    vm: mlua::Lua,
    evidence: Evidence,
    plan: Plan,
    scenario: Value,
    _artifacts: tempfile::TempDir,
}

fn setup() -> Result<Setup> {
    setup_with_history(false)
}

fn setup_with_history(with_history: bool) -> Result<Setup> {
    let repo = Repository::checked_in();
    let vm = game(&repo)?;
    if with_history {
        vm.load("turn=START+66; local l=Game:GetProperty('JAC_AGES_LEDGER'); l.history.r1={turn=66,from=0,nested={note='旧:記録',value=7}}; Game:SetProperty('JAC_AGES_LEDGER',l)").exec()?;
    }
    let before: String = vm.load(core_code(OATH, false)).eval()?;
    let body = before
        .split_once(':')
        .and_then(|(_, rest)| rest.strip_suffix("|END-JAC-OATH-SAVE"))
        .context("core frame")?
        .to_owned();
    let (artifacts, _) = artifacts_with(&body)?;
    let evidence = oath_save_probe::verify(&repo, artifacts.path())?;
    let plan = plan(&repo, &evidence)?;
    let scenario = scenario(&evidence, &plan)?;
    compile(&scenario)?;
    Ok(Setup {
        vm,
        evidence,
        plan,
        scenario,
        _artifacts: artifacts,
    })
}

/// Execute the scenario in the VM; `mutation` runs before the first step containing `at`.
fn execute(s: &Setup, at: &str, mutation: &str) -> Result<Vec<String>> {
    let mut returns = Vec::new();
    let mut pending = !at.is_empty();
    for step in s.scenario["steps"].as_array().unwrap() {
        if pending && step.to_string().contains(at) {
            s.vm.load(mutation)
                .exec()
                .expect("mutation itself must run");
            pending = false;
        }
        match step["action"].as_str().unwrap() {
            "enable_mods" | "load" => {}
            "autoplay" => {
                s.vm.load(format!("fastForward({})", step["turns"]))
                    .exec()?
            }
            "wait_for" => ensure!(
                s.vm.load(step["condition"]["code"].as_str().unwrap())
                    .eval::<bool>()?,
                "condition not met"
            ),
            "ui_lua" => returns.push(s.vm.load(step["code"].as_str().unwrap()).eval()?),
            other => bail!("unexpected action {other}"),
        }
    }
    assert!(!pending, "mutation point {at} absent");
    Ok(returns)
}

/// The probe must fail for the stated reason, not for an unrelated one.
fn rejects(at: &str, mutation: &str, reason: &str) -> Result<()> {
    let s = setup()?;
    let error = format!("{:#}", execute(&s, at, mutation).unwrap_err());
    assert!(error.contains(reason), "{at} {mutation}: {error}");
    Ok(())
}

const H: &str = "local l=Game:GetProperty('JAC_AGES_LEDGER'); local h=l.houses.p0;";
const SAVE: &str = " Game:SetProperty('JAC_AGES_LEDGER',l)";

#[test]
fn a_self_consistent_replaced_save_cannot_replace_the_original_evidence() -> Result<()> {
    let s = setup()?;
    s.vm.load("local l=Game:GetProperty('JAC_AGES_LEDGER'); l.replacement='same name, different state'; Game:SetProperty('JAC_AGES_LEDGER',l); props.JAC_TODO134_SAVE_PROBE=nil").exec()?;
    s.vm.load(core_code(OATH, false)).eval::<String>()?;
    // The substituted save and its internal snapshot agree, but not the original artifact.
    s.vm.load(core_code(OATH, true)).eval::<String>()?;
    let error = format!("{:#}", execute(&s, "", "").unwrap_err());
    assert!(
        error.contains("loaded save differs from original artifact Core record"),
        "{error}"
    );
    Ok(())
}

#[test]
fn existing_nonempty_history_survives_and_changes_are_rejected() -> Result<()> {
    let s = setup_with_history(true)?;
    assert_eq!(s.plan.history.len(), 1);
    execute(&s, "", "")?;
    let s = setup_with_history(true)?;
    let error=format!("{:#}",execute(&s,"JAC_OATH_EXPIRY_DUE|","local l=Game:GetProperty('JAC_AGES_LEDGER'); l.history.r1.nested.value=8; Game:SetProperty('JAC_AGES_LEDGER',l)").unwrap_err());
    assert!(error.contains("old judgement history changed"), "{error}");
    Ok(())
}

#[test]
fn oversized_lengths_return_errors_without_panicking() {
    for text in [
        format!("{}:string:x", usize::MAX),
        format!("{{{}:string:x}}", usize::MAX),
    ] {
        assert!(Frame::parse(&text).is_err());
    }
}

#[test]
fn natural_due_expires_the_human_oath_and_a_legal_default_follows() -> Result<()> {
    let s = setup()?;
    let p = &s.plan;
    assert_eq!(
        (
            p.reign,
            p.reign_type.as_str(),
            p.due,
            p.elapsed(),
            p.fast_forward()
        ),
        (2, "REIGN_EIROKU", 73, 9, 63)
    );
    assert_eq!(
        (p.start, p.saved_turn, p.old_reign_start),
        (16603, 16611, 1)
    );
    assert!(p.history.is_empty());
    assert_eq!(
        p.saved_name,
        "JAC-Todo134-Save-JAC_OATH_ODA_TENKA_FUBU-agent-01-0123456789ab"
    );
    let probe = &s.scenario["expiry_probe"];
    assert_eq!(
        (&probe["task_accepted"], &s.scenario["diagnostic_only"]),
        (&json!(false), &json!(true))
    );
    assert_eq!(
        s.scenario["test_mode"],
        json!({"skipped_turns":0,"continuous_performance_valid":false})
    );
    let steps = s.scenario["steps"].as_array().unwrap();
    assert_eq!(steps[0], s.evidence.source["steps"][0]);
    assert_eq!(
        steps[1],
        json!({"action":"load","name":p.saved_name,"exact_name":true,"timeout":600})
    );
    let requests = steps
        .iter()
        .filter(|s| s.to_string().contains("UI.RequestAction"))
        .count();
    let forwards: Vec<_> = steps.iter().filter(|s| s["action"] == "autoplay").collect();
    assert_eq!(requests, 2);
    assert_eq!(
        forwards,
        [&json!({"action":"autoplay","turns":63,"fast_forward":true,"timeout":1800})]
    );
    let returns = execute(&s, "", "")?;
    assert!(returns[1].contains("|due=73|elapsed=9|"), "{}", returns[1]);
    assert!(returns[2].starts_with("JAC_OATH_EXPIRY_PRE|"));
    let due = returns
        .iter()
        .find(|r| r.starts_with("JAC_OATH_EXPIRY_DUE|"))
        .unwrap();
    // 21 AI houses, minus the dark SHIMAZU house whose dark age offers no oath here.
    assert!(
        due.contains(
            "|human_age=GOLDEN|human_selection=empty|old_flag=0|ai_defaults=20|cities=56|"
        ),
        "{due}"
    );
    let next = returns.last().unwrap();
    assert!(
        next.contains("|same_oath_reselected=true|flag=1|"),
        "{next}"
    );
    // The product judged once, naturally: GOLDEN human, HEROIC AI with all three oaths.
    s.vm.load(
        r#"local l=Game:GetProperty('JAC_AGES_LEDGER')
assert(l.reign==2 and l.history.r2 and l.history.r2.turn==73 and l.houses.p2.age=='HEROIC')
local n=0; for _ in pairs(l.houses.p2.oaths) do n=n+1 end; assert(n==3)
local shimazu=0
for _,h in pairs(l.houses) do if h.civ=='CIVILIZATION_JAC_SHIMAZU' then shimazu=shimazu+1; assert(h.age=='DARK' and h.oathChosen==nil) end end
assert(shimazu==1)"#,
    )
    .exec()?;
    Ok(())
}

#[test]
fn residue_rewritten_history_extra_judgement_and_illegal_choices_are_rejected() -> Result<()> {
    let due = "JAC_OATH_EXPIRY_DUE|";
    let next = "JAC_OATH_EXPIRY_NEXT|";
    let human_plot = "plots[1]:SetProperty('JAC_OATH_ODA_TENKA_FUBU_ON',1)";
    let ai_default = "AI default missing for the new reign";
    for (at, mutation, reason) in [
        (due, format!("{H} l.history.r2.length=1;{SAVE}"), "judgement record differs from the natural due turn"),
        (due, format!("{H} l.history.r1={{turn=1}};{SAVE}"), "judgement history count changed"),
        (due, format!("{H} h.oathChosen=2; h.oaths={{JAC_OATH_ODA_TENKA_FUBU=true}};{SAVE}"), "human oath survived the reign change"),
        (due, format!("{H} l.houses.p1.oathChosen=nil;{SAVE}"), ai_default),
        (due, format!("{H} l.houses.p2.oaths={{JAC_OATH_DATE_X=true}};{SAVE}"), ai_default),
        (due, human_plot.to_owned(), "oath flag disagrees with the ledger"),
        (due, "Players[0].IsHuman=function() return false end".to_owned(), "player 0 must stay human"),
        (due, format!("{H} l.reign=3;{SAVE}"), "natural judgement did not advance exactly one reign"),
        (next, format!("{H} h.oaths={{JAC_OATH_TAKEDA_RYUO=true}};{SAVE}"), "selection after the deadline is not a legal default"),
        (next, format!("{H} l.history.r3={{turn=74}};{SAVE}"), "judgement history count changed"),
        (next, format!("{H} l.history.r2.from=0;{SAVE}"), "the due judgement record changed"),
        (next, "props.JAC_TODO134_EXPIRY_JUDGEMENT='{}'".to_owned(), "the due judgement record changed"),
        // A judgement on the turn before the due turn is not the natural one.
        ("JAC_OATH_EXPIRY_PRE|", format!("{H} l.reign=2;{SAVE}"), "a judgement ran before the due turn"),
        ("JAC_OATH_EXPIRY_PRE|", "advance(1)".to_owned(), "fast-forward did not stop on the turn before the due turn"),
        // Product due and the independent calculation must agree.
        ("JAC_OATH_EXPIRY_CALC|", "GameInfo.Eras_XP1[ERA_ROWS[2].EraType].GameEraMaximumTurns=GameInfo.Eras_XP1[ERA_ROWS[2].EraType].GameEraMaximumTurns+1".to_owned(), "product reign due differs from the independent calculation"),
        ("JAC_OATH_EXPIRY_CALC|", "GameInfo.GameSpeeds.GAMESPEED_STANDARD.GameSpeedType='GAMESPEED_QUICK'".to_owned(), "game speed differs from the source scenario"),
        ("JAC_OATH_SAVE_CORE_AFTER|", format!("{H} h.oaths={{}};{SAVE}"), "actual selected oath missing"),
    ] {
        rejects(at, &mutation, reason)?;
    }
    Ok(())
}

#[test]
fn one_request_per_turn_and_exactly_one_turn_advance() -> Result<()> {
    let s = setup()?;
    // A second request for the same observation is refused before any engine call.
    let first = s.scenario["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s.to_string().contains("UI.RequestAction"))
        .unwrap()["code"]
        .as_str()
        .unwrap()
        .to_owned();
    s.vm.load("fastForward(63)").exec()?;
    s.vm.load(first.as_str()).eval::<String>()?;
    s.vm.load("turn=turn-1").exec()?;
    let error = s.vm.load(first.as_str()).eval::<String>().unwrap_err();
    assert!(error
        .to_string()
        .contains("duplicate human end-turn request"));
    // An extra engine turn after the request fails the exact +1 wait.
    rejects(
        "native request advanced more than one turn",
        "advance(1)",
        "native request advanced more than one turn",
    )?;
    rejects(
        "local n=2",
        "JAC_OATH_EXPIRY_REQUESTS[2]={turn=0}",
        "duplicate human end-turn request",
    )?;
    Ok(())
}

#[test]
fn frames_are_read_by_length_and_typed_key_only() -> Result<()> {
    let s = setup()?;
    let good = &s.evidence.core_after;
    let frame = Frame::parse(good)?;
    assert_eq!(frame.get("start")?.int()?, 16603);
    let nested = "{8:string:a{8:string:b8:number:1}}";
    let parsed = Frame::parse(nested)?;
    let (_, _, span) = &parsed.entries()?[0];
    assert_eq!(&nested[span.clone()], "{8:string:b8:number:1}");
    for broken in [
        "",
        "{",
        "{8:string:a}",
        "{8:string:a8:number:1",
        "{9:string:a8:number:1}",
        "{8:string:a8:number:1}x",
        "{8:string:a5:nil:x}",
        "{8:string:a10:number:inf}",
        "{8:string:a8:number:18:string:a8:number:2}",
        "{{}8:number:1}",
        "{8:string:a11:boolean:yes}",
        "{8:string:a-1:number:1}",
    ] {
        assert!(Frame::parse(broken).is_err(), "{broken}");
    }
    let repo = Repository::checked_in();
    for (from, to) in [
        // An old frame without the actual start turn is not completed by guess.
        ("12:string:start12:number:16603", ""),
        (
            "12:string:start12:number:16603",
            "12:string:start12:number:16700",
        ),
        (
            "17:string:oathChosen8:number:1",
            "17:string:oathChosen8:number:2",
        ),
        (
            "10:string:age13:string:GOLDEN10:string:civ",
            "10:string:age13:string:NORMAL10:string:civ",
        ),
    ] {
        assert!(good.contains(from), "{from}");
        let evidence = Evidence {
            core_after: good.replacen(from, to, 1),
            ..clone(&s.evidence)
        };
        assert!(plan(&repo, &evidence).is_err(), "{from} -> {to}");
    }
    Ok(())
}

fn clone(e: &Evidence) -> Evidence {
    Evidence {
        oath: e.oath.clone(),
        source: e.source.clone(),
        saved_name: e.saved_name.clone(),
        core_after: e.core_after.clone(),
    }
}

#[test]
fn due_is_independent_of_the_stale_saved_next_due() -> Result<()> {
    let repo = Repository::checked_in();
    assert_eq!(
        due_of(&repo, "GAMESPEED_STANDARD", 1)?,
        (66, "REIGN_KOJI".into())
    );
    assert_eq!(
        due_of(&repo, "GAMESPEED_STANDARD", 2)?,
        (73, "REIGN_EIROKU".into())
    );
    let quick = reigns::engine_turns(&repo, "GAMESPEED_QUICK")?[1];
    assert_eq!(due_of(&repo, "GAMESPEED_QUICK", 2)?.0, quick);
    assert!(due_of(&repo, "GAMESPEED_STANDARD", 0).is_err());
    // A save already at the due turn has no natural judgement ahead of it.
    let s = setup()?;
    let mut late = clone(&s.evidence);
    late.core_after = late.core_after.replacen(
        "11:string:turn12:number:16611",
        "11:string:turn12:number:16675",
        1,
    );
    assert!(plan(&repo, &late).is_err());
    late.core_after = late.core_after.replacen(
        "11:string:turn12:number:16675",
        "11:string:turn12:number:16674",
        1,
    );
    let edge = plan(&repo, &late)?;
    assert_eq!(edge.fast_forward(), 0);
    let edge_scenario = scenario(&late, &edge)?;
    assert!(!edge_scenario["steps"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["action"] == "autoplay"));
    Ok(())
}

#[test]
fn cli_requires_verified_source_and_a_new_output() -> Result<()> {
    let repo = Repository::checked_in();
    run(&repo, &["--check".into()])?;
    let s = setup()?;
    let (dir, rows) = artifacts_with(&s.evidence.core_after)?;
    let out = dir.path().join("expiry.json");
    let args = |out: &Path| {
        vec![
            "--from-artifacts".to_owned(),
            dir.path().display().to_string(),
            "--output".into(),
            out.display().to_string(),
        ]
    };
    run(&repo, &args(&out))?;
    let written: Value = serde_json::from_str(&fs::read_to_string(&out)?)?;
    assert_eq!(written, s.scenario);
    assert!(
        run(&repo, &args(&out)).is_err(),
        "existing output overwritten"
    );
    // A failed source save run is never a source.
    let mut broken = rows.clone();
    let last = broken.len() - 1;
    broken[last]["status"] = json!("FAIL");
    write_rows(dir.path(), &broken)?;
    let other = dir.path().join("other.json");
    assert!(run(&repo, &args(&other)).is_err());
    assert!(!other.exists());
    assert!(run(&repo, &["--from-artifacts".into()]).is_err());
    Ok(())
}
