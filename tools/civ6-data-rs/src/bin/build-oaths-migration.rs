//! Finite acceptance using actual pre-oath saves; never manufacture a legacy ledger.
use anyhow::{bail, Context, Result};
use civ6_data_rs::Repository;
use serde_json::{json, Value};
use std::{fs, path::PathBuf};

const OLD_NAME: &str = "JAC-SengokuAges-02-ages-owner-l-05-543484304b0c";
const PRE_NAME: &str = "JAC-SengokuAges-01-ages-owner-l-05-543484304b0c";
const LEGACY: &str = r#"
local l=Game:GetProperty('JAC_AGES_LEDGER')
if type(l)~='table' or l.reign~=1 then error('actual legacy reign1 required') end
local turn=Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1
if turn~=67 or l.history.r1.turn~=66 then error('legacy save provenance/turn mismatch') end
local n=0
for k,h in pairs(l.houses) do
 n=n+1
 for _,field in ipairs({'oaths','oathReign','oathChosen','oathDue','oathKnowledge'}) do
  if h[field]~=nil then error('save already has oath field '..k..'.'..field) end
 end
 if (h.parts.oath or 0)~=0 then error('save has oath points') end
end
if n~=22 or not Players[0]:IsAlive() or not Players[0]:IsHuman() then error('legacy 22 houses/human missing') end
return 'LEGACY_ACCEPTED turn=67 reign=1 houses=22 oathFields=absent'
"#;

const SNAPSHOT: &str = r#"
local l=Game:GetProperty('JAC_AGES_LEDGER')
local function ser(v)
 if type(v)~='table' then return type(v)..':'..tostring(v) end
 local keys={} for k in pairs(v) do keys[#keys+1]=k end table.sort(keys)
 local out={} for _,k in ipairs(keys) do out[#out+1]=tostring(k)..'='..ser(v[k]) end
 return '{'..table.concat(out,',')..'}'
end
Game:SetProperty('JAC_TODO134_OLD_HISTORY',ser(l.history))
Game:SetProperty('JAC_TODO134_OLD_TURN',Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1)
return 'legacy history snapshot captured without modifying the ledger'
"#;

const MIGRATED: &str = r#"
local l=Game:GetProperty('JAC_AGES_LEDGER')
local turn=Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1
if turn~=68 or l.reign~=1 then error('one natural turn must migrate the same reign') end
local humans,ai=0,0
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
 local id=p:GetID() local h=l.houses['p'..id]
 if h.oathReign~=1 or type(h.oaths)~='table' or type(h.oathKnowledge)~='table' then error('missing migration for '..id) end
 if (h.parts.oath or 0)~=0 then error('retroactive oath reward '..id) end
 local selected=0
 for oath,on in pairs(h.oaths) do
  if on then
   local row=GameInfo.JAC_SengokuOaths[oath]
   if not row or row.CivilizationType~=h.civ then error('foreign migrated oath '..id) end
   selected=selected+1
  end
 end
 if p:IsHuman() then
  humans=humans+1
  if h.oathChosen~=nil or selected~=0 or h.oathDue~=69 then error('human must get a new choice window') end
 else
  ai=ai+1
  if h.oathChosen~=1 or selected~=(h.age=='HEROIC' and 3 or 1) then error('AI migrated choice count '..id) end
 end
end
if humans~=1 or ai==0 then error('missing human/AI migration coverage') end
return 'MIGRATED turn=68 human='..humans..' AI='..ai..' retroactiveOathPoints=0'
"#;

const CHOOSE: &str = r#"
if not JAC_ShowOaths(true) then error('migrated oath popup unavailable') end
local rows=JAC_GetOfferedOaths()
if #rows~=3 then error('migrated human must have exactly three own choices') end
local h=Game:GetProperty('JAC_AGES_LEDGER').houses.p0
if h.age=='HEROIC' then
 for _,row in ipairs(rows) do if not JAC_SelectOath(row.type) then error('heroic selection failed') end end
 if not JAC_ConfirmOaths() then error('heroic confirmation failed') end
elseif not JAC_ChooseOath(rows[1].type) then error('migrated confirmation failed') end
return 'migrated human chose through the existing popup and player operation'
"#;

const OATH_SNAPSHOT: &str = r#"
local l=Game:GetProperty('JAC_AGES_LEDGER') local h=l.houses.p0
if h.oathChosen~=l.reign then error('choice not received by gameplay') end
local function ser(v)
 if type(v)~='table' then return type(v)..':'..tostring(v) end
 local keys={} for k in pairs(v) do keys[#keys+1]=k end table.sort(keys)
 local out={} for _,k in ipairs(keys) do out[#out+1]=tostring(k)..'='..ser(v[k]) end
 return '{'..table.concat(out,',')..'}'
end
if ser(l.history)~=Game:GetProperty('JAC_TODO134_OLD_HISTORY') then error('migration changed legacy history') end
local out={ser(h.oaths),tostring(h.oathReign),tostring(h.oathChosen),tostring(h.oathDue),ser(h.oathKnowledge)}
local cities=0
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
 local owner=l.houses['p'..p:GetID()]
 for _,city in p:GetCities():Members() do
  cities=cities+1
  local plot=Map.GetPlot(city:GetX(),city:GetY())
  for row in GameInfo.JAC_SengokuOaths() do
   local expected=(owner.civ==row.CivilizationType and (owner.age=='GOLDEN' or owner.age=='HEROIC') and owner.oaths[row.OathType]) and 1 or 0
   if plot:GetProperty(row.OathType..'_ON')~=expected then error('migration city flag mismatch '..row.OathType) end
  end
 end
end
if cities==0 then error('no migrated city flags checked') end
local now=table.concat(out,'|')
local saved=Game:GetProperty('JAC_TODO134_MIGRATED_SNAPSHOT')
if saved and saved~=now then error('migration state changed across save/load') end
Game:SetProperty('JAC_TODO134_MIGRATED_SNAPSHOT',now)
if Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1~=68 then error('load advanced the turn') end
return 'MIGRATION_PERSISTED same history/oath state and all 66 flags in '..cities..' cities'
"#;

const CAPABILITIES: &str = r#"
local l=Game:GetProperty('JAC_AGES_LEDGER') local out={}
local p=Players[0] local dip=p:GetDiplomacy()
for _,name in ipairs({'HasDeclaredFriendship','SetHasDeclaredFriendship','SetHasMet','HasAllied'}) do out[#out+1]=name..'='..type(dip[name]) end
local city=p:GetCities():GetCapitalCity()
if not city then error('no capital') end
local q=city:GetBuildQueue() local g=city:GetGrowth()
for _,name in ipairs({'GetUnitProduction','GetBuildingProduction','GetProductionYield','GetUnitProgress','GetBuildingProgress','GetTurnsLeft'}) do out[#out+1]='queue.'..name..'='..type(q[name]) end
out[#out+1]='growth.GetOverallGrowthModifier='..type(g.GetOverallGrowthModifier)
out[#out+1]='reign='..tostring(l.reign)
return 'TODO134_CAPABILITIES '..table.concat(out,' ')
"#;

fn lua(state: &str, code: &str) -> Value {
    json!({"action":"ui_lua","state_name":state,"code":code,"timeout":60})
}

fn scenarios(repo: &Repository) -> Result<Vec<(&'static str, Value)>> {
    let base = repo.json("tools/civ6-autotest/scenarios/sengoku-oaths-oda.json")?;
    let enabled = base["steps"][0].clone();
    let load = |name| json!({"action":"load","name":name,"exact_name":true,"timeout":600});
    let active = json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":"return Players[0]:IsTurnActive()"}});
    let probe = json!({"name":"誓約導入前の実保存：現在の台帳とAPIを読み取る","judge_profile":"generic","steps":[enabled.clone(),load(OLD_NAME),lua("GameCore_Tuner",LEGACY),lua("GameCore_Tuner",CAPABILITIES)]});
    let migration = json!({"name":"誓約導入前の実保存：自然な次手番で人間とAIを移行し、旧履歴と保存復元を照合する","judge_profile":"generic","steps":[enabled.clone(),load(OLD_NAME),lua("GameCore_Tuner",LEGACY),lua("GameCore_Tuner",SNAPSHOT),active.clone(),json!({"action":"end_turn","timeout":900}),lua("GameCore_Tuner",MIGRATED),active.clone(),lua("DedicationPopup",CHOOSE),json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"GameCore_Tuner","code":"local l=Game:GetProperty('JAC_AGES_LEDGER'); return l and l.houses.p0.oathChosen==l.reign"}}),lua("GameCore_Tuner",OATH_SNAPSHOT),json!({"action":"save","name":"JAC-Todo134-Migrated","timeout":180}),json!({"action":"exit_to_menu","timeout":180}),json!({"action":"load","name":"JAC-Todo134-Migrated","timeout":600}),lua("GameCore_Tuner",OATH_SNAPSHOT)]});
    let pre = json!({"name":"誓約導入前の天文保存：改元前には候補と効果を持たない","judge_profile":"generic","steps":[enabled,load(PRE_NAME),lua("GameCore_Tuner",r#"local l=Game:GetProperty('JAC_AGES_LEDGER'); if not l or l.reign~=0 then error('actual pre-reign legacy save required') end; local t=Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1; if t~=34 then error('unexpected legacy Tenbun turn '..t) end; for _,h in pairs(l.houses) do if h.oathChosen~=nil or next(h.oaths or {})~=nil then error('oath before first reign') end end; return 'legacy Tenbun turn34 has no oath'"#),active,json!({"action":"end_turn","timeout":900}),lua("GameCore_Tuner",r#"local l=Game:GetProperty('JAC_AGES_LEDGER'); local t=Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1; if t~=35 or l.reign~=0 then error('pre-reign migration advanced unexpectedly') end; local n=0; for _,p in ipairs(PlayerManager.GetAliveMajors()) do local h=l.houses['p'..p:GetID()]; if h.oathChosen~=nil or next(h.oaths or {})~=nil or (h.parts.oath or 0)~=0 then error('pre-reign oath/reward') end; for _,c in p:GetCities():Members() do n=n+1; local plot=Map.GetPlot(c:GetX(),c:GetY()); for r in GameInfo.JAC_SengokuOaths() do if plot:GetProperty(r.OathType..'_ON')~=0 then error('pre-reign strong effect') end end end end; if n==0 then error('no cities checked') end; return 'LEGACY_TENBUN_PASSED turn35 flags=0 cities='..n"#)]});
    Ok(vec![
        ("legacy-capabilities.json", probe),
        ("legacy-migration.json", migration),
        ("legacy-tenbun.json", pre),
    ])
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        bail!("usage: build-oaths-migration <output-directory>");
    }
    let out = PathBuf::from(&args[0]);
    fs::create_dir_all(&out)?;
    for (name, scene) in scenarios(&Repository::checked_in())? {
        fs::write(
            out.join(name),
            format!("{}\n", serde_json::to_string_pretty(&scene)?),
        )
        .with_context(|| name)?;
        println!(
            "{} {} steps",
            out.join(name).display(),
            scene["steps"].as_array().unwrap().len()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_emitted_lua_chunk_compiles_on_lua51() -> Result<()> {
        let vm = mlua::Lua::new();
        for (_, scene) in scenarios(&Repository::checked_in())? {
            for step in scene["steps"].as_array().unwrap() {
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
        }
        Ok(())
    }
    fn legacy_vm() -> Result<mlua::Lua> {
        let vm = mlua::Lua::new();
        vm.load(r#"ledger={reign=1,history={r1={turn=66}},houses={}}; for i=0,21 do ledger.houses['p'..i]={parts={}} end; Game={GetProperty=function() return ledger end,GetCurrentGameTurn=function() return 67 end}; GameConfiguration={GetStartTurn=function() return 1 end}; Players={[0]={IsAlive=function() return true end,IsHuman=function() return true end}}"#).exec()?;
        Ok(vm)
    }
    #[test]
    fn legacy_probe_rejects_current_or_incomplete_saves() -> Result<()> {
        let vm = legacy_vm()?;
        let _: String = vm.load(LEGACY).eval()?;
        vm.load("ledger.houses.p0.oaths={}").exec()?;
        assert!(vm.load(LEGACY).eval::<String>().is_err());
        let vm = legacy_vm()?;
        vm.load("ledger.houses.p21=nil").exec()?;
        assert!(vm.load(LEGACY).eval::<String>().is_err());
        let vm = legacy_vm()?;
        vm.load("ledger.history.r1.turn=65").exec()?;
        assert!(vm.load(LEGACY).eval::<String>().is_err());
        Ok(())
    }
}
