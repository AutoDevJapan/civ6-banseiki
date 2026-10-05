//! Generate the joint PoC acceptance scenario from the retained native absorption test.
use serde_json::{json, Value};
use std::{fs, path::PathBuf};

const SETUP: &str = r#"
local RP=assert(ExposedMembers.JAC_ReligionPersistence,'religion persistence not loaded')
local A=assert(JAC_AbsorptionGameplay)
local e=assert(Game:GetProperty('JAC_ABSORPTION_LIVE_EXPECT_V1'))
assert(Game:GetProperty(RP.PROPERTY_KEY)==nil,'joint test requires a fresh ledger')
local id='JAC_RP_PLACEHOLDER_01'
RP.Found(id,e.From,'origin',5,1,'PLAYER_'..e.From)
RP.BindEngineReligion(id,e.Religion)
RP.PreserveAsset(id,'joint_scripture','SCRIPTURE','CAVE',e.From)
RP.PreserveAsset(id,'joint_record','RECORD','PLAYER_'..e.From,e.From)
RP.PreserveAsset(id,'hero_relic','RELIC','HERO_PLAYER_'..e.From,e.From)
RP.Found('JAC_RP_PLACEHOLDER_02',Game.GetLocalPlayer(),'third_party',3,1,'PLAYER_'..Game.GetLocalPlayer())
RP.PreserveAsset('JAC_RP_PLACEHOLDER_02','third_book','SCRIPTURE','RUINS',Game.GetLocalPlayer())
-- Seed an already completed RP receipt for the real native Hero created by
-- the retained absorption fixture. This tests migration, not RP recruitment.
local hero=assert(e.SourceRoster['live:hero']).Unit.ID
local u=assert(Players[e.From]:GetUnits():FindID(hero))
assert(u:IsHero(),'receipt fixture must refer to a native Hero')
local state=RP.Export()
state.heroes[tostring(e.From)]={religion=id,discovery='joint_hero',unit=hero,
 heroClass=u:GetHeroClassType(),heroUnit=GameInfo.Units[u:GetType()].UnitType,
 rewardFaith=0,verification='APPLIED'}
state.religions[id].discoveries.joint_hero={asset='hero_relic',kind='RELIC',actor='FIXTURE_HERO',
 job='KABUKIMONO',site='RUINS',player=e.From,foundTurn=Game.GetCurrentGameTurn(),
 outcome='HERO',effects='APPLIED',resolvedTurn=Game.GetCurrentGameTurn()}
state.religions[id].materialUses.hero_relic={outcome='HERO',effects='APPLIED',discovery='joint_hero',turn=Game.GetCurrentGameTurn()}
Game:SetProperty(RP.PROPERTY_KEY,state)
assert(RP.Inspect(id).followers>0 and not RP.Inspect(id).founderDefeated,'live believers not established')
Game:SetProperty('JAC_JOINT_BEFORE_V1',{Third=A.Rules.Copy(RP.Inspect('JAC_RP_PLACEHOLDER_02')),
 Hero=A.Rules.Copy(state.heroes[tostring(e.From)]),Discovery=A.Rules.Copy(state.religions[id].discoveries.joint_hero),
 Materials=A.Rules.Copy(state.religions[id].materialUses),Followers=RP.NativeFollowers(id)})
return 'joint RP ledger seeded against real native religion and Hero'
"#;

const CHECK: &str = r#"
local RP=assert(ExposedMembers.JAC_ReligionPersistence)
local A=assert(JAC_AbsorptionGameplay)
local e=assert(Game:GetProperty('JAC_ABSORPTION_LIVE_EXPECT_V1'))
local before=assert(Game:GetProperty('JAC_JOINT_BEFORE_V1'))
local tx=assert(A.GetState().Transactions['japan-live-absorption'])
assert(tx.Status=='COMPLETE' and tx.Adapters['religion-persistence']~=nil,'religion persistence adapter was not part of absorption')
local s=RP.Inspect('JAC_RP_PLACEHOLDER_01')
assert(s.founder==e.From and s.founderDefeated and s.engineReligion==e.Religion,'founder provenance/native binding changed')
assert(s.followers==before.Followers and s.followers>0 and not s.lost and not s.canRevive,'defeat removed believers or caused false loss')
assert(s.communities.origin.holder=='PLAYER_'..e.To,'community protection not transferred')
assert(s.assets.joint_scripture.owner==e.To and s.assets.joint_scripture.holder=='CAVE','scripture owner/site not preserved')
assert(s.assets.joint_record.owner==e.To,'record owner not transferred')
assert(A.Canonical(s.materialUses)==A.Canonical(before.Materials),'historical effects changed')
assert(A.Canonical(s.discoveries.joint_hero)==A.Canonical(before.Discovery),'historical recipient changed')
assert(A.Canonical(RP.Inspect('JAC_RP_PLACEHOLDER_02'))==A.Canonical(before.Third),'third-party religion changed')
local h=assert(RP.Export().heroes[tostring(e.To)],'destination RP Hero record missing')
assert(RP.Export().heroes[tostring(e.From)]==nil,'source Hero record retained')
local mapped=assert(tx.UnitMap[tostring(e.From)..':'..before.Hero.unit])
assert(h.unit==mapped.ID and h.heroClass==before.Hero.heroClass and h.verification=='APPLIED','Hero identity/receipt not rebound')
local expectedHero=A.Rules.Copy(before.Hero);expectedHero.unit=mapped.ID
assert(A.Canonical(h)==A.Canonical(expectedHero),'Hero receipt fields changed during transfer')
local u=assert(Players[e.To]:GetUnits():FindID(h.unit))
assert(u:IsHero() and u:GetHeroClassType()==h.heroClass,'rebound record does not name the native Hero')
local saved=RP.Snapshot()
A.Apply(assert(Game:GetProperty('JAC_ABSORPTION_LIVE_PLAN_V1')))
assert(RP.Snapshot()==saved,'completed absorption replay wrote the RP ledger')
return saved
"#;

const REVIVE: &str = r#"
local RP=assert(ExposedMembers.JAC_ReligionPersistence)
local e=assert(Game:GetProperty('JAC_ABSORPTION_LIVE_EXPECT_V1'))
local id='JAC_RP_PLACEHOLDER_01'
for _,p in ipairs(PlayerManager.GetAlive()) do
 for _,c in p:GetCities():Members() do
  c:GetReligion():RemovePressureOneReligion(e.Religion,8388607,-1)
  c:GetReligion():RemovePressureOneReligion(e.Religion,1,-1)
 end
end
local lost=RP.Inspect(id)
assert(RP.NativeFollowers(id)==0 and lost.lost and lost.canRevive,'zero native citizens must permit revival from transferred scripture')
local p=Players[e.To]
p:GetReligion():ChangeFaithBalance(200-p:GetReligion():GetFaithBalance())
local city=p:GetCities():GetCapitalCity()
local f=p:GetReligion():GetFaithBalance()
RP.Revive(id,'joint_revival','joint_scripture','joint_reborn',e.To,city:GetID())
local s=RP.Inspect(id)
assert(RP.NativeFollowers(id)==1 and s.followers==1 and not s.lost and not s.canRevive,'revival failed to create an actual citizen')
assert(s.revivalCount==1 and s.revivals.joint_revival.effects=='APPLIED','revival not recorded once')
assert(p:GetReligion():GetFaithBalance()==f-RP.Rules().REVIVAL_FAITH_COST,'revival did not charge the rule cost once')
Game:SetProperty('JAC_JOINT_REVIVED_FAITH_V1',p:GetReligion():GetFaithBalance())
local saved=RP.Snapshot()
assert(not pcall(RP.Revive,id,'joint_revival','joint_scripture','joint_reborn',e.To,city:GetID()),'duplicate revival accepted')
assert(saved==RP.Snapshot() and p:GetReligion():GetFaithBalance()==Game:GetProperty('JAC_JOINT_REVIVED_FAITH_V1'),'duplicate revival changed saved state or faith')
return saved
"#;

const AFTER_REVIVE: &str = r#"
local RP=assert(ExposedMembers.JAC_ReligionPersistence)
local e=assert(Game:GetProperty('JAC_ABSORPTION_LIVE_EXPECT_V1'))
local s=RP.Inspect('JAC_RP_PLACEHOLDER_01')
assert(RP.NativeFollowers('JAC_RP_PLACEHOLDER_01')==1 and not s.lost and s.revivalCount==1,'native revival not restored')
assert(s.revivals.joint_revival.effects=='APPLIED' and s.assets.joint_scripture.owner==e.To,'revival receipt/material owner not restored')
assert(Players[e.To]:GetReligion():GetFaithBalance()==Game:GetProperty('JAC_JOINT_REVIVED_FAITH_V1'),'reload replayed the revival cost')
local before=assert(Game:GetProperty('JAC_JOINT_BEFORE_V1'))
assert(JAC_AbsorptionGameplay.Canonical(RP.Inspect('JAC_RP_PLACEHOLDER_02'))==JAC_AbsorptionGameplay.Canonical(before.Third),'third party changed during revival')
return RP.Snapshot()
"#;

fn core(code: &str) -> Value {
    json!({"action":"ui_lua","state_name":"JAC_Absorption_Entry","code":code})
}

fn generate(mut source: Value) -> Value {
    let original = source["steps"].as_array().unwrap();
    assert!(original[14]["code"]
        .as_str()
        .unwrap()
        .contains("local function ApplyAndWait"));
    assert_eq!(original[82]["action"], "save");
    assert_eq!(original[88]["action"], "ui_lua");
    let mut steps = original[..89].to_vec();
    steps[0]["ids"]
        .as_array_mut()
        .unwrap()
        .push(json!("9b0b9a39-bd72-4ff1-9062-05eeac09b693"));
    steps[0]["disable_ids"]
        .as_array_mut()
        .unwrap()
        .retain(|x| x != "9b0b9a39-bd72-4ff1-9062-05eeac09b693");
    let mut result = Vec::new();
    for (i, step) in steps.into_iter().enumerate() {
        if i == 14 {
            result.push(core(SETUP));
        }
        if i == 82 {
            let mut check = core(CHECK);
            check["capture_as"] = json!("JOINT_TRANSFERRED");
            result.push(check);
        }
        result.push(step);
    }
    let mut check = core(CHECK);
    check["equals_capture"] = json!("JOINT_TRANSFERRED");
    result.push(check);
    let mut revive = core(REVIVE);
    revive["capture_as"] = json!("JOINT_REVIVED");
    result.push(revive);
    result.push(json!({"action":"save","name":"JAC-JOINT-REVIVED"}));
    result.push(json!({"action":"exit_to_menu"}));
    result.push(json!({"action":"load","name":"JAC-JOINT-REVIVED","timeout":240}));
    result.push(json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":"return ExposedMembers.JAC_ReligionPersistence~=nil and ExposedMembers.JAC_Absorption~=nil"}}));
    let mut restored = core(AFTER_REVIVE);
    restored["equals_capture"] = json!("JOINT_REVIVED");
    result.push(restored);
    source["steps"] = json!(result);
    source["name"] = json!("Joint absorption and religion persistence: native citizens, Hero receipt, scripture ownership, reload and native revival");
    source["test_mode"] = json!({"continuous_performance_valid":false,"skipped_turns":0});
    source
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("tools/civ6-autotest/scenarios");
    let source = serde_json::from_slice(&fs::read(dir.join("absorption-japan.json"))?)?;
    fs::write(
        dir.join("absorption-religion-persistence.json"),
        format!("{}\n", serde_json::to_string_pretty(&generate(source))?),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_native_acceptance_and_joint_chunks_compile_in_lua51() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source: Value = serde_json::from_slice(
            &fs::read(root.join("tools/civ6-autotest/scenarios/absorption-japan.json")).unwrap(),
        )
        .unwrap();
        let result = generate(source.clone());
        let deployed: Value = serde_json::from_slice(
            &fs::read(
                root.join("tools/civ6-autotest/scenarios/absorption-religion-persistence.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(result == deployed, "regenerate the deployed joint scenario");
        civ6_autotest_rs::scenario::validate_scenario(&result).unwrap();
        let steps = result["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 98);
        for step in steps {
            if let Some(code) = step["code"]
                .as_str()
                .or_else(|| step["condition"]["code"].as_str())
            {
                mlua::Lua::new().load(code).into_function().unwrap();
            }
        }
        for i in 1..89 {
            assert!(
                steps.contains(&source["steps"][i]),
                "original step {i} dropped"
            );
        }
    }
}
