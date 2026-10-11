//! A real engine friendship and natural ledger intake, with a labelled age fixture.
use anyhow::{bail, Result};
use civ6_data_rs::Repository;
use serde_json::{json, Value};
use std::{fs, path::PathBuf};

const FIXTURE: &str = r#"
local l=Game:GetProperty('JAC_AGES_LEDGER')
local turn=Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1
if turn~=2 or not l or l.reign~=0 or l.houses.p0.civ~='CIVILIZATION_JAC_OUCHI' then error('fresh Ouchi fixture required') end
local a,b=0,1 local key='0-1' local da,db=Players[a]:GetDiplomacy(),Players[b]:GetDiplomacy()
if Players[a]:GetTeam()==Players[b]:GetTeam() or da:HasAllied(b) or da:HasDeclaredFriendship(b) or db:HasDeclaredFriendship(a) then error('pair must be unrelated different teams') end
if l.friends[key]~='none' then error('natural initial relation baseline missing') end
if type(Game:GetProperty('JAC_1534_ALLIANCE_OUCHI-TAKEDA'))=='number' then error('starting alliance fixture forbidden') end
-- Only the age/choice window is fabricated; the engine friendship and intake are real.
l.reign=1 l.reignStart=turn
l.history.r1={turn=turn,length=1}
local h=l.houses.p0
h.age='NORMAL' h.oaths={} h.oathReign=1 h.oathChosen=nil h.oathDue=turn+10
Game:SetProperty('JAC_AGES_LEDGER',l)
return 'TEST_FIXTURE normal Ouchi at turn2, actual unrelated pair0-1; not natural era balance'
"#;

const CHOOSE: &str = r#"
if not JAC_ShowOaths(true) then error('normal Ouchi choice unavailable') end
if not JAC_ChooseOath('JAC_OATH_OUCHI_CERTIFIED_ROUTES') then error('relation oath request failed') end
return 'normal relation oath requested through existing player operation'
"#;

const ESTABLISH: &str = r#"
local l=Game:GetProperty('JAC_AGES_LEDGER') local h=l.houses.p0
if h.oathChosen~=1 or not h.oaths.JAC_OATH_OUCHI_CERTIFIED_ROUTES then error('relation oath not chosen') end
if l.keys['friend:0:1:1'] or l.friends['0-1']~='none' then error('friendship already scored') end
local a,b=Players[0]:GetDiplomacy(),Players[1]:GetDiplomacy()
if a:HasDeclaredFriendship(1) or b:HasDeclaredFriendship(0) then error('friendship unexpectedly formed before fixture') end
Game:SetProperty('JAC_TODO134_FRIEND_BASE',{a=h.parts.ally or 0,b=l.houses.p1.parts.ally or 0,oath=h.parts.oath or 0,turn=Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1})
a:SetHasMet(1) b:SetHasMet(0)
a:SetHasDeclaredFriendship(1,true) b:SetHasDeclaredFriendship(0,true)
if a:HasDeclaredFriendship(1)~=true or b:HasDeclaredFriendship(0)~=true then error('engine did not establish bilateral friendship') end
if a:HasAllied(1) then error('friendship must not be alliance') end
return 'ENGINE_FRIENDSHIP_ESTABLISHED pair0-1 bilateral=true alliance=false'
"#;

const JUDGE: &str = r#"
local l=Game:GetProperty('JAC_AGES_LEDGER') local base=Game:GetProperty('JAC_TODO134_FRIEND_BASE')
if not l or not base or l.reign~=1 then error('friendship baseline/reign missing') end
local a,b=l.houses.p0,l.houses.p1
local da=(a.parts.ally or 0)-base.a local db=(b.parts.ally or 0)-base.b local dq=(a.parts.oath or 0)-base.oath
if l.friends['0-1']~='on' or l.keys['friend:0:1:1']~=true then error('natural ledger intake did not admit friendship') end
if da~=1 or db~=1 or dq~=1 then error('friendship deltas must be base1/base1/oath1, got '..da..'/'..db..'/'..dq) end
local forward,reverse=Players[0]:GetDiplomacy(),Players[1]:GetDiplomacy()
if forward:HasDeclaredFriendship(1)~=true or reverse:HasDeclaredFriendship(0)~=true or forward:HasAllied(1) or reverse:HasAllied(0) then error('engine bilateral friendship changed') end
return 'FRIENDSHIP_POINTS_PASSED base0=1 base1=1 oath0=1 key=once'
"#;

const SETUP: &str = "BuildHeadlessGameSetup(); g_GameParameters:FullRefresh(); if PlayerConfigurations[0]:GetCivilizationTypeName()~='CIVILIZATION_JAC_OUCHI' or PlayerConfigurations[1]:GetCivilizationTypeName()~='CIVILIZATION_JAC_TAKEDA' then error('setup changed Ouchi/Takeda slots') end; HideGameSetup(); return 'Ouchi setup built'";

fn judge_after(delta: u32) -> Value {
    lua("GameCore_Tuner", &format!("local base=Game:GetProperty('JAC_TODO134_FRIEND_BASE'); local turn=Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1; if not base or turn~=base.turn+{delta} then error('unexpected friendship turn '..turn) end\n{JUDGE}"))
}

fn after_load() -> Value {
    lua("GameCore_Tuner", &format!("local saved=Game:GetProperty('JAC_TODO134_FRIEND_SAVE_TURN'); local turn=Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1; if saved==nil or turn~=saved then error('friendship load advanced the turn') end\n{JUDGE}"))
}

fn lua(state: &str, code: &str) -> Value {
    json!({"action":"ui_lua","state_name":state,"code":code,"timeout":60})
}
fn scenario(repo: &Repository) -> Result<Value> {
    let base = repo.json("tools/civ6-autotest/scenarios/sengoku-oaths-oda.json")?;
    let mut configure = base["steps"][1].clone();
    let players = configure["players"].as_array_mut().unwrap();
    let other = players
        .iter()
        .position(|p| p["civ"] == "CIVILIZATION_JAC_OUCHI")
        .ok_or_else(|| anyhow::anyhow!("Ouchi missing"))?;
    players.swap(0, other);
    for (i, p) in players.iter_mut().enumerate() {
        p["human"] = json!(i == 0);
    }
    if players[1]["civ"] != "CIVILIZATION_JAC_TAKEDA" {
        bail!("peer1 must be Takeda");
    }
    let start = base["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["action"] == "start_game")
        .ok_or_else(|| anyhow::anyhow!("template has no start_game"))?
        .clone();
    let active = json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":"return Players[0]:IsTurnActive()"}});
    Ok(
        json!({"name":"新しい友好宣言：実成立後の台帳1点と誓約1点、保存復元と次手番の重複防止（年齢だけ試験設定）","judge_profile":"generic","steps":[base["steps"][0].clone(),configure,lua("HostGame",SETUP),start,json!({"action":"autoplay","turns":1,"timeout":900}),lua("GameCore_Tuner",FIXTURE),active.clone(),lua("DedicationPopup",CHOOSE),json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"GameCore_Tuner","code":"local l=Game:GetProperty('JAC_AGES_LEDGER'); return l and l.houses.p0.oathChosen==1"}}),lua("GameCore_Tuner",ESTABLISH),json!({"action":"end_turn","timeout":900}),judge_after(1),lua("GameCore_Tuner","local turn=Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1; Game:SetProperty('JAC_TODO134_FRIEND_SAVE_TURN',turn); return 'friendship save turn='..turn"),json!({"action":"save","name":"JAC-Todo134-Friendship","timeout":180}),json!({"action":"exit_to_menu","timeout":180}),json!({"action":"load","name":"JAC-Todo134-Friendship","timeout":600}),after_load(),active,json!({"action":"end_turn","timeout":900}),judge_after(2)]}),
    )
}
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        bail!("usage: build-oaths-friendship <output-directory>");
    }
    let out = PathBuf::from(&args[0]);
    fs::create_dir_all(&out)?;
    let scene = scenario(&Repository::checked_in())?;
    fs::write(
        out.join("friendship.json"),
        format!("{}\n", serde_json::to_string_pretty(&scene)?),
    )?;
    println!(
        "{} {} steps",
        out.join("friendship.json").display(),
        scene["steps"].as_array().unwrap().len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn game_is_started_before_any_gameplay_or_turn_action() -> Result<()> {
        let scene = scenario(&Repository::checked_in())?;
        let steps = scene["steps"].as_array().unwrap();
        let mut started = false;
        for step in steps {
            if step["action"] == "start_game" {
                assert!(!started);
                started = true;
            }
            if step["action"] == "autoplay"
                || step["action"] == "end_turn"
                || step["state_name"] == "GameCore_Tuner"
            {
                assert!(started, "gameplay before start_game");
            }
        }
        assert!(started);
        let setup = steps
            .iter()
            .find(|step| step["state_name"] == "HostGame")
            .expect("emitted HostGame setup");
        let setup_code = setup["code"].as_str().unwrap();
        let vm = mlua::Lua::new();
        vm.load("BuildHeadlessGameSetup=function()end; HideGameSetup=function()end; g_GameParameters={FullRefresh=function()end}; civ0='CIVILIZATION_JAC_OUCHI'; PlayerConfigurations={[0]={GetCivilizationTypeName=function()return civ0 end},[1]={GetCivilizationTypeName=function()return 'CIVILIZATION_JAC_TAKEDA' end}}").exec()?;
        let _: String = vm.load(setup_code).eval()?;
        vm.load("civ0='CIVILIZATION_JAC_ODA'").exec()?;
        assert!(vm.load(setup_code).eval::<String>().is_err());
        Ok(())
    }
    #[test]
    fn generated_lua_compiles_on_lua51() -> Result<()> {
        let vm = mlua::Lua::new();
        for step in scenario(&Repository::checked_in())?["steps"]
            .as_array()
            .unwrap()
        {
            for code in [
                step.get("code"),
                step.get("condition").and_then(|v| v.get("code")),
            ]
            .into_iter()
            .flatten()
            {
                vm.load(code.as_str().unwrap()).into_function()?;
            }
        }
        Ok(())
    }
    #[test]
    fn judge_rejects_missing_oath_reward_and_double_scoring() -> Result<()> {
        let vm = mlua::Lua::new();
        vm.load(r#"ledger={reign=1,friends={['0-1']='on'},keys={['friend:0:1:1']=true},houses={p0={parts={ally=4,oath=6}},p1={parts={ally=8}}}}; base={a=3,b=7,oath=5}; Game={GetProperty=function(_,key) if key=='JAC_AGES_LEDGER' then return ledger else return base end end}; Players={[0]={GetDiplomacy=function() return {HasDeclaredFriendship=function() return true end,HasAllied=function() return false end} end}}; Players[1]=Players[0]"#).exec()?;
        let _: String = vm.load(JUDGE).eval()?;
        vm.load("ledger.houses.p0.parts.oath=5").exec()?;
        assert!(vm.load(JUDGE).eval::<String>().is_err());
        vm.load("ledger.houses.p0.parts.oath=7").exec()?;
        assert!(vm.load(JUDGE).eval::<String>().is_err());
        vm.load("ledger.houses.p0.parts.oath=6; ledger.houses.p1.parts.ally=9")
            .exec()?;
        assert!(vm.load(JUDGE).eval::<String>().is_err());
        vm.load("ledger.houses.p1.parts.ally=8;Players[1]={GetDiplomacy=function()return {HasDeclaredFriendship=function()return false end,HasAllied=function()return false end}end}").exec()?;
        let error = vm.load(JUDGE).eval::<String>().unwrap_err();
        assert!(error.to_string().contains("bilateral friendship changed"));
        Ok(())
    }
    #[test]
    fn turn_judges_reject_an_advanced_load_and_skipped_next_turn() -> Result<()> {
        let vm = mlua::Lua::new();
        vm.load(r#"turn=42; ledger={reign=1,friends={['0-1']='on'},keys={['friend:0:1:1']=true},houses={p0={parts={ally=4,oath=6}},p1={parts={ally=8}}}}; base={a=3,b=7,oath=5,turn=41}; savedTurn=42; Game={GetCurrentGameTurn=function()return turn end,GetProperty=function(_,key)if key=='JAC_AGES_LEDGER' then return ledger elseif key=='JAC_TODO134_FRIEND_SAVE_TURN' then return savedTurn else return base end end}; GameConfiguration={GetStartTurn=function()return 1 end}; Players={[0]={GetDiplomacy=function()return {HasDeclaredFriendship=function()return true end,HasAllied=function()return false end}end}}; Players[1]=Players[0]"#).exec()?;
        let load = after_load();
        let load_code = load["code"].as_str().unwrap();
        let _: String = vm.load(load_code).eval()?;
        vm.load("turn=43").exec()?;
        assert!(vm.load(load_code).eval::<String>().is_err());
        let next = judge_after(2);
        let next_code = next["code"].as_str().unwrap();
        let _: String = vm.load(next_code).eval()?;
        vm.load("turn=44").exec()?;
        assert!(vm.load(next_code).eval::<String>().is_err());
        Ok(())
    }
}
