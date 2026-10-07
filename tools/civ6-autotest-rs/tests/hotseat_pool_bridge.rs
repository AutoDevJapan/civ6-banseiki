use civ6_autotest_rs::hotseat_bridge::{self as hs, Canonical};
use mlua::Lua;
use serde_json::{json, Value};
use std::fs;

const NONCE: &str = "0123456789abcdef0123456789abcdef";
const MOD_ID: &str = "78ea4ad5-d526-4ca0-bd0e-cc4519261719";
const POOL_MOD_ID: &str = "c8fb8042-8e3e-4b35-b02a-781947194700";

fn shipped(name: &str) -> Value {
    let path = civ6_autotest_rs::repo_root()
        .join("tools/civ6-autotest/scenarios")
        .join(name);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn template() -> Value {
    shipped("multiplayer-banseiki-hotseat-growth.json")
}

fn fixed() -> Value {
    shipped("banseiki-pool-bridge.json")
}

fn random() -> Value {
    shipped("banseiki-pool-random-bridge.json")
}

#[test]
fn shipped_pool_bridges_are_the_registered_canonicals() {
    assert_eq!(fixed(), hs::pool_scenario(false));
    assert_eq!(random(), hs::pool_scenario(true));
    assert_eq!(hs::canonical(&fixed()).unwrap(), Canonical::Pool);
    assert_eq!(hs::canonical(&random()).unwrap(), Canonical::PoolRandom);
    assert_eq!(
        hs::canonical(&template()).unwrap(),
        Canonical::BanseikiGrowth
    );
    assert_eq!(
        hs::canonical(&shipped("multiplayer-hotseat-growth.json")).unwrap(),
        Canonical::Growth
    );
    // The direct-FireTuner pool scenario is not a bridge canonical.
    assert!(hs::check_scenario(&shipped("banseiki-pool-hotseat.json")).is_err());
}

#[test]
fn installed_witness_is_identified_by_its_run_id_without_overwriting_another_task() {
    let tmp = tempfile::tempdir().unwrap();
    let bundle = hs::prepare(&fixed(), &tmp.path().join("bundle"), NONCE, MOD_ID).unwrap();
    let prepared: Value = serde_json::from_slice(&fs::read(&bundle.scenario).unwrap()).unwrap();
    let mods = tmp.path().join("Mods");
    let installed = mods.join("JAC_Test_Pool1947_unique");
    fs::create_dir_all(installed.join("UI")).unwrap();
    for file in [
        "JAC_Test_HotseatBridge.modinfo",
        "UI/JAC_HotseatWitness.xml",
        "UI/JAC_HotseatWitness.lua",
        "UI/JAC_HotseatPlan.lua",
    ] {
        fs::copy(
            std::path::Path::new(&bundle.mod_folder).join(file),
            installed.join(file),
        )
        .unwrap();
    }
    hs::verify_installed(&bundle, &prepared, &mods).unwrap();
    let other = mods.join("JAC_Test_HotseatBridge");
    fs::create_dir_all(&other).unwrap();
    fs::write(
        other.join("JAC_Test_HotseatBridge.modinfo"),
        "<Mod id=\"ffffffff-ffff-ffff-ffff-ffffffffffff\"/>",
    )
    .unwrap();
    hs::verify_installed(&bundle, &prepared, &mods).unwrap();
    fs::write(installed.join("UI/JAC_HotseatPlan.lua"), "tampered").unwrap();
    assert!(hs::verify_installed(&bundle, &prepared, &mods).is_err());
}

#[test]
fn only_the_prepared_random_canonical_gets_the_fixed_pregame_helper() {
    for (data, expected) in [
        (shipped("multiplayer-hotseat-growth.json"), false),
        (template(), false),
        (fixed(), false),
        (random(), true),
    ] {
        let output = tempfile::tempdir().unwrap();
        let bundle = hs::prepare(&data, output.path(), NONCE, MOD_ID).unwrap();
        let prepared: Value = serde_json::from_slice(&fs::read(&bundle.scenario).unwrap()).unwrap();
        hs::verify_installed(&bundle, &prepared, output.path()).unwrap();
        for scenario in [&data, &prepared] {
            let helper = hs::pregame_helper(scenario).unwrap();
            assert_eq!(helper, expected.then_some(hs::POOL_RANDOM_PREGAME));
        }
        let ids = prepared["steps"][0]["ids"].as_array().unwrap();
        assert_eq!(ids.last().unwrap(), MOD_ID);
    }
}

#[test]
fn altered_pool_bridges_are_rejected_before_any_helper() {
    let mut cases: Vec<Value> = Vec::new();
    let mut add = |base: Value, edit: &dyn Fn(&mut Value)| {
        let mut data = base;
        edit(&mut data);
        cases.push(data);
    };
    for base in [fixed(), random()] {
        add(base.clone(), &|d| {
            d["steps"][1]["players"][1]["leader"] = json!("LEADER_JAC_POOL_O009")
        });
        add(base.clone(), &|d| {
            d["steps"][1]["players"][0]["civ"] = json!("CIVILIZATION_JAC_TAKEDA")
        });
        add(base.clone(), &|d| {
            d["steps"][0]["ids"].as_array_mut().unwrap().pop();
        });
        add(base.clone(), &|d| {
            d["steps"][4]["code"] = json!("return 'fabricated pool record'")
        });
        add(base.clone(), &|d| {
            let code = d["steps"][20]["code"]
                .as_str()
                .unwrap()
                .replace("|JAC_POOL|", "|JAC_POOL_CHANGED|");
            d["steps"][20]["code"] = code.into();
        });
        add(base.clone(), &|d| {
            d["steps"][1]["seed"] = json!(1);
        });
        add(base.clone(), &|d| {
            d["pool_random_pregame"] = json!("return true");
        });
    }
    add(random(), &|d| {
        d["steps"][1]["players"][2]["human"] = json!(true);
    });
    add(random(), &|d| {
        d["steps"][1]["players"].as_array_mut().unwrap().pop();
    });
    // Fixed-person checks with random players, and random checks with fixed players.
    add(random(), &|d| {
        d["steps"][1]["players"] = fixed()["steps"][1]["players"].clone()
    });
    add(fixed(), &|d| {
        d["steps"][1]["players"] = random()["steps"][1]["players"].clone()
    });
    for data in cases {
        assert!(hs::check_scenario(&data).is_err());
        assert!(hs::pregame_helper(&data).is_err());
        assert!(hs::prepare(&data, tempfile::tempdir().unwrap().path(), NONCE, MOD_ID).is_err());
    }
}

#[test]
fn pool_record_is_appended_and_every_original_growth_check_is_kept() {
    let source = template();
    for data in [fixed(), random()] {
        let steps = data["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 21);
        assert_eq!(
            data["steps"][0]["ids"],
            json!(source["steps"][0]["ids"]
                .as_array()
                .unwrap()
                .iter()
                .cloned()
                .chain([json!(POOL_MOD_ID)])
                .collect::<Vec<_>>())
        );
        for (index, step) in steps.iter().enumerate() {
            let base = &source["steps"][index];
            let mut left = step.clone();
            let mut right = base.clone();
            for value in [&mut left, &mut right] {
                let object = value.as_object_mut().unwrap();
                object.remove("code");
                if index <= 1 {
                    object.remove("ids");
                    object.remove("players");
                }
            }
            assert_eq!(left, right, "step {}", index + 1);
            let code = step["code"].as_str();
            if hs::POOL_RECORD_STEPS.contains(&(index + 1)) {
                let original = base["code"].as_str().unwrap();
                let kept = original
                    .strip_suffix("return table.concat(rows,'|')")
                    .unwrap();
                let code = code.unwrap();
                assert!(code.starts_with(kept), "step {}", index + 1);
                assert!(code.contains("local growth=table.concat(rows,'|')\n"));
                assert!(
                    code.ends_with("\nreturn growth..'|JAC_POOL|'..table.concat(poolParts,';')")
                );
                for check in [
                    "poolRows~=300",
                    "ledgerCount~=300",
                    "'^300:%x+$'",
                    "savedLedger~=ledgerVersion",
                    "person~=row.PersonID or trait~=row.TraitType or role~='LEADER'",
                ] {
                    assert!(code.contains(check), "{check}");
                }
            } else {
                assert_eq!(step["code"], base["code"], "step {}", index + 1);
            }
        }
        assert_eq!(steps[8]["name"], "JAC-Hotseat-Mid-Round");
        assert_eq!(steps[11]["equals_capture"], "MID_ROUND");
        assert_eq!(steps[20]["equals_capture"], "NEXT_PLAYER");
    }
    let players = |data: Value| data["steps"][1]["players"].clone();
    assert_eq!(
        players(fixed()),
        json!([
            {"civ": "CIVILIZATION_JAC_ODA", "leader": "LEADER_JAC_ODA_NOBUHIDE", "human": true},
            {"civ": "CIVILIZATION_JAC_ODA", "leader": "LEADER_JAC_POOL_O008", "human": true}
        ])
    );
    let random_players = players(random());
    assert_eq!(random_players.as_array().unwrap().len(), 4);
    assert_eq!(random_players[0], players(fixed())[0]);
    assert_eq!(random_players[1], players(fixed())[1]);
    assert_eq!(random_players[2]["human"], false);
    assert_eq!(random_players[3]["human"], false);
}

const LEDGER: &str = "300:69277281d50d1b3c";

/// InGame stand-ins for GameInfo, players, and the pool/growth properties.
fn ingame(random: bool, tweak: &str) -> Lua {
    let lua = Lua::new();
    lua.load(format!(
        r#"
        pool={{}}
        for i=1,300 do pool[i]={{PersonID='JAC_POOL_X'..i, LeaderType='LEADER_X'..i, CivilizationType='CIVILIZATION_X', TraitType='TRAIT_X'}} end
        pool[1]={{PersonID='JAC_POOL_H040', LeaderType='LEADER_JAC_ODA_NOBUHIDE', CivilizationType='CIVILIZATION_JAC_ODA', TraitType='TRAIT_LEADER_JAC_POOL_ASTUTE'}}
        pool[2]={{PersonID='JAC_POOL_O008', LeaderType='LEADER_JAC_POOL_O008', CivilizationType='CIVILIZATION_JAC_ODA', TraitType='TRAIT_O008'}}
        ledger={{{{Version='{LEDGER}', Count=300}}}}
        local function rows(t) return function() local i=0; return function() i=i+1; return t[i] end end end
        GameInfo={{JAC_BanseikiPool=function() return rows(pool)() end, JAC_BanseikiPoolLedger=function() return rows(ledger)() end,
          Leaders=setmetatable({{}}, {{__index=function(_, k) if k~='RANDOM' then return {{LeaderType=k}} end end}})}}
        leaders={{[0]='LEADER_JAC_ODA_NOBUHIDE', 'LEADER_JAC_POOL_O008', 'LEADER_OFFICIAL_A', 'LEADER_OFFICIAL_B'}}
        civs={{[0]='CIVILIZATION_JAC_ODA', 'CIVILIZATION_JAC_ODA', 'CIVILIZATION_A', 'CIVILIZATION_B'}}
        props={{[0]={{JAC_POOL_PERSON_ID='JAC_POOL_H040', JAC_POOL_TRAIT='TRAIT_LEADER_JAC_POOL_ASTUTE', JAC_POOL_ROLE='LEADER'}},
          [1]={{JAC_POOL_PERSON_ID='JAC_POOL_O008', JAC_POOL_TRAIT='TRAIT_O008', JAC_POOL_ROLE='LEADER'}}, [2]={{}}, [3]={{}}}}
        gameProps={{JAC_POC_GROWTHUI_INITIALIZED=true, JAC_BANSEIKI_POOL_LEDGER='{LEDGER}'}}
        majors={majors}
        Game={{GetProperty=function(_, k) return gameProps[k] end, GetCurrentGameTurn=function() return 1 end, GetLocalPlayer=function() return 1 end}}
        GameConfiguration={{IsHotseat=function() return true end, GetStartTurn=function() return 0 end}}
        records={{}}
        Players={{}}
        for pid=0,3 do
          records[pid]={{}}
          for uid=1,2 do
            local id=uid+pid*10
            local r={{Level=1, XP=0, Personality='calm', Trait='steady', Equipment={{'spear'}}, Artifact=nil, LastXPTurn=0}}
            records[pid][uid]={{GetID=function() return id end, GetProperty=function(_, k) if k=='JAC_POC_GROWTHUI_WARLORD' then return r end end}}
          end
          Players[pid]={{IsHuman=function() return pid<2 end, GetProperty=function(_, k) return props[pid][k] end,
            GetUnits=function() return {{Members=function() local i=0; return function() i=i+1; if records[pid][i] then return i, records[pid][i] end end end}} end}}
        end
        PlayerConfigurations={{}}
        for pid=0,3 do PlayerConfigurations[pid]={{GetLeaderTypeName=function() return leaders[pid] end, GetCivilizationTypeName=function() return civs[pid] end}} end
        PlayerManager={{GetAliveMajors=function() local t={{}}; for pid=0,majors-1 do t[#t+1]=Players[pid] end; return t end}}
        {tweak}
        "#,
        majors = if random { 4 } else { 2 }
    ))
    .exec()
    .unwrap();
    lua
}

fn run(lua: &Lua, code: &str) -> mlua::Result<String> {
    lua.load("JAC_HOTSEAT_GROWTH_PREVIOUS=nil").exec()?;
    lua.load(code).eval::<String>()
}

fn human_records() -> String {
    format!(
        "{LEDGER};0~LEADER_JAC_ODA_NOBUHIDE~CIVILIZATION_JAC_ODA~JAC_POOL_H040~TRAIT_LEADER_JAC_POOL_ASTUTE~LEADER;1~LEADER_JAC_POOL_O008~CIVILIZATION_JAC_ODA~JAC_POOL_O008~TRAIT_O008~LEADER"
    )
}

#[test]
fn injected_pool_record_extends_the_original_capture_deterministically() {
    let source = template();
    for (data, is_random, record) in [
        (fixed(), false, human_records()),
        (
            random(),
            true,
            format!(
                "{};2~LEADER_OFFICIAL_A~CIVILIZATION_A;3~LEADER_OFFICIAL_B~CIVILIZATION_B",
                human_records()
            ),
        ),
    ] {
        let lua = ingame(is_random, "");
        for step in hs::POOL_RECORD_STEPS {
            let original = run(&lua, source["steps"][step - 1]["code"].as_str().unwrap()).unwrap();
            let code = data["steps"][step - 1]["code"].as_str().unwrap();
            let value = run(&lua, code).unwrap();
            assert_eq!(
                value,
                format!("{original}|JAC_POOL|{record}"),
                "step {step}"
            );
            // Save/load equality compares this exact string: rerunning cannot redraw it.
            assert_eq!(run(&lua, code).unwrap(), value);
        }
    }
}

#[test]
fn injected_pool_record_rejects_wrong_people_ledgers_and_leaders() {
    let cases: &[(bool, &str, &str)] = &[
        (
            false,
            "props[1].JAC_POOL_TRAIT='TRAIT_OTHER'",
            "pool person 1",
        ),
        (false, "props[0].JAC_POOL_ROLE=nil", "pool person 0"),
        (
            false,
            "props[0].JAC_POOL_PERSON_ID='JAC_POOL_O008'",
            "pool person 0",
        ),
        (false, "ledger[1].Count=299", "pool ledger"),
        (false, "pool[300]=nil", "pool ledger 299"),
        (
            false,
            "ledger[2]={Version='300:00', Count=300}",
            "pool ledger",
        ),
        (
            false,
            "ledger[1].Version='299:abc'; gameProps.JAC_BANSEIKI_POOL_LEDGER='299:abc'",
            "pool ledger",
        ),
        (
            false,
            "gameProps.JAC_BANSEIKI_POOL_LEDGER='300:ffff'",
            "saved pool ledger",
        ),
        (
            false,
            "gameProps.JAC_BANSEIKI_POOL_LEDGER=nil",
            "saved pool ledger",
        ),
        (
            false,
            "leaders[1]='LEADER_JAC_ODA_NOBUHIDE'",
            "fixed pool leader 1",
        ),
        (
            false,
            "civs[1]='CIVILIZATION_JAC_TAKEDA'",
            "pool civilization 1",
        ),
        (false, "majors=3", "fixed pool majors"),
        (
            true,
            "leaders[3]='LEADER_OFFICIAL_A'",
            "random pool duplicate leader",
        ),
        (
            true,
            "leaders[1]='LEADER_JAC_ODA_NOBUHIDE'",
            "random pool duplicate leader",
        ),
        (true, "leaders[2]='RANDOM'", "random pool player 2"),
        (true, "leaders[1]='LEADER_OFFICIAL_C'", "pool human 1"),
        (true, "majors=3", "random pool majors"),
        (
            true,
            "Players[2].IsHuman=function() return true end",
            "random pool player 2",
        ),
        // The original growth checks still run first.
        (
            true,
            "records[0][1]:GetProperty('JAC_POC_GROWTHUI_WARLORD').Level=0",
            "invalid growth numbers",
        ),
        (false, "records[1][2]=nil", "expected 2 growth records"),
    ];
    for (is_random, tweak, expected) in cases {
        let data = if *is_random { random() } else { fixed() };
        let lua = ingame(*is_random, tweak);
        for step in hs::POOL_RECORD_STEPS {
            let error = run(&lua, data["steps"][step - 1]["code"].as_str().unwrap())
                .unwrap_err()
                .to_string();
            assert!(error.contains(*expected), "{tweak}: step {step}: {error}");
        }
    }
}

fn pregame(tweak: &str) -> (Lua, mlua::Result<String>) {
    let lua = Lua::new();
    lua.load(format!(
        r#"
        UI={{IsInFrontEnd=function() return true end}}
        GameConfiguration={{IsHotseat=function() return true end, GetValue=function(k) if k=='RULESET' then return 'RULESET_JAC_BANSEIKI' end end}}
        Network={{IsInSession=function() return false end}}
        JAC_POOL_FILTER_INSTALLED=true
        poolRows=300
        DB={{ConfigurationQuery=function(sql)
          assert(sql=='SELECT LeaderType FROM JAC_BanseikiPool')
          local t={{}}; for i=1,poolRows do t[i]={{LeaderType='LEADER_P'..i}} end; return t end}}
        current={{[0]='LEADER_P1', 'LEADER_P2', 'LEADER_OFFICIAL_A', 'LEADER_P3'}}
        candidates={{
          [0]={{{{Value='RANDOM'}}, {{Value='LEADER_P1'}}, {{Value='LEADER_P2', Invalid=true}}, {{Value='LEADER_P3'}}}},
          [1]={{{{Value='RANDOM'}}, {{Value='LEADER_P2'}}, {{Value='LEADER_P1', Invalid=true}}, {{Value='LEADER_P3'}}}},
          [2]={{{{Value='RANDOM'}}, {{Value='LEADER_OFFICIAL_A'}}, {{Value='LEADER_P3'}}}},
          [3]={{{{Value='RANDOM'}}, {{Value='LEADER_P3'}}, {{Value='LEADER_OFFICIAL_B'}}}}}}
        written={{}}; writes=0; refreshes=0
        PlayerConfigurations={{}}
        for pid=0,3 do PlayerConfigurations[pid]={{IsHuman=function() return pid<2 end, GetLeaderTypeName=function() return current[pid] end}} end
        function CreatePlayerParameters(pid, headless)
          assert(headless==true)
          local leader={{ConfigurationGroup='Player', ConfigurationId='LEADER_TYPE_NAME', Values=candidates[pid]}}
          local p={{PlayerId=pid, Parameters={{PlayerLeader=leader}}}}
          function p:Refresh() refreshes=refreshes+1 end
          function p:SetParameterValue(parameter, value) assert(parameter==leader); writes=writes+1; written[pid]=value.Value end
          function p:Config_ReadParameterValues(parameter) assert(parameter.ConfigurationId=='LEADER_TYPE_NAME'); return written[pid] end
          return p
        end
        {tweak}
        "#
    ))
    .exec()
    .unwrap();
    let result = lua.load(hs::POOL_RANDOM_PREGAME).eval::<String>();
    (lua, result)
}

#[test]
fn random_pregame_helper_writes_official_random_for_all_four_after_checks() {
    let (lua, result) = pregame("");
    assert_eq!(result.unwrap(), hs::POOL_RANDOM_PREGAME_RESULT);
    let check: bool = lua
        .load("return writes==4 and refreshes==8 and written[0]=='RANDOM' and written[1]=='RANDOM' and written[2]=='RANDOM' and written[3]=='RANDOM'")
        .eval()
        .unwrap();
    assert!(check);
    for (tweak, expected) in [
        (
            "candidates[3]={{Value='RANDOM'}, {Value='LEADER_P3'}}",
            "pool random candidates 3",
        ),
        (
            "candidates[2]={{Value='LEADER_OFFICIAL_A'}}",
            "pool random candidates 2",
        ),
        ("candidates[1][2].Invalid=true", "pool random candidates 1"),
        ("current[2]='LEADER_OFFICIAL_Z'", "pool random candidates 2"),
        (
            "table.insert(candidates[0], {Value='LEADER_OFFICIAL_A'})",
            "human candidate outside pool 0",
        ),
        ("poolRows=299", "pool count 299"),
        (
            "JAC_POOL_FILTER_INSTALLED=nil",
            "pool frontend hook not loaded",
        ),
        (
            "Network.IsInSession=function() return true end",
            "before HostGame",
        ),
        (
            "PlayerConfigurations[2].IsHuman=function() return true end",
            "pool random slot 2",
        ),
        (
            "GameConfiguration.GetValue=function() return 'RULESET_EXPANSION_2' end",
            "Banseiki ruleset required",
        ),
    ] {
        let (lua, result) = pregame(tweak);
        let error = result.unwrap_err().to_string();
        assert!(error.contains(expected), "{tweak}: {error}");
        // Every candidate check runs before the first write.
        let writes: i64 = lua.load("return writes").eval().unwrap();
        assert_eq!(writes, 0, "{tweak}");
    }
    let (_, result) = pregame("local original=CreatePlayerParameters; function CreatePlayerParameters(pid, headless) local p=original(pid, headless); p.Config_ReadParameterValues=function() return 'LEADER_P1' end; return p end");
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("RANDOM leader not written"));
}
