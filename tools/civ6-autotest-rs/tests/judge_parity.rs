use civ6_autotest_rs::judge::{
    judge, log_marker_replaced, read_since, slice_log_bytes, snapshot_logs, LOG_NAMES,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};

const MODDING:&str="dfa0717e-4d0b-4370-b693-27fc6758571b\nApplying Component - JAC_PoC_Bundle0_Gameplay (UpdateDatabase)\nApplying Component - JAC_PoC_Bundle0_Scripts (AddGameplayScripts)\n";
const LUA:&str="JAC_PoC_Bundle0: [JAC_POC] loaded version=1\nJAC_PoC_Bundle0: [JAC_POC] no saved state (new game)\nJAC_PoC_Bundle0: [JAC_POC] turn=1 firstTurn=1 turnsSeen=1\nJAC_PoC_Bundle0: [JAC_POC] turn=2 firstTurn=1 turnsSeen=2\nJAC_PoC_Bundle0: [JAC_POC] turn=3 firstTurn=1 turnsSeen=3\nJAC_PoC_Bundle0: [JAC_POC] turn=4 firstTurn=1 turnsSeen=4\nJAC_PoC_Bundle0: [JAC_POC] loaded version=1\nJAC_PoC_Bundle0: [JAC_POC] restored firstTurn=1 lastTurn=4 turnsSeen=4\nJAC_PoC_Bundle0: [JAC_POC] turn=5 firstTurn=1 turnsSeen=5\n";
const DATABASE:&str="[3897835.363] [Localization] ERROR: UNIQUE constraint failed: LocalizedText\n[3897835.363] from file CurrentClickouts/Text/ClickoutText_en_US.xml\n";
fn logs(lua: &str, db: &str, modding: &str) -> BTreeMap<String, String> {
    [
        ("Lua.log", lua),
        ("Database.log", db),
        ("Modding.log", modding),
    ]
    .into_iter()
    .map(|(k, v)| (k.into(), v.into()))
    .collect()
}
fn sample() -> BTreeMap<String, String> {
    logs(LUA, DATABASE, MODDING)
}
fn failed(
    logs: &BTreeMap<String, String>,
    profile: &str,
    units: Option<usize>,
) -> BTreeSet<String> {
    judge(logs, profile, units, false)
        .into_iter()
        .filter(|c| !c.passed)
        .map(|c| c.name)
        .collect()
}
fn growth_line(kind: &str, unit: i64, level: i64, xp: i64, turn: Option<i64>) -> String {
    let (personality, trait_id, equipment, artifact) = if unit == 1 {
        (
            "JAC_PERSONALITY_BRAVE",
            "JAC_TRAIT_VALIANT",
            "JAC_ITEM_POC_BLADE,JAC_ITEM_POC_CLOAK",
            "JAC_ARTIFACT_POC_RELIC",
        )
    } else {
        (
            "JAC_PERSONALITY_CALM",
            "JAC_TRAIT_FIERCE",
            "JAC_ITEM_POC_BLADE",
            "none",
        )
    };
    format!("JAC_PoC_GrowthUI_Gameplay: [JAC_POC] {kind}unit={unit} level={level} xp={xp} personality={personality} trait={trait_id} equipment={equipment} artifact={artifact} player=0{}",turn.map_or(String::new(),|n|format!(" turn={n}")))
}
fn growth_logs(units: i64) -> BTreeMap<String, String> {
    let mut lines = vec![
        "PartialScreenHooks: [JAC_POC] notebooks button attached parent=ButtonStack".into(),
        "JAC_PoC_GrowthUI_Gameplay: [JAC_POC] no saved state (new game)".into(),
    ];
    for unit in 1..=units {
        lines.push(growth_line("assigned ", unit, 1, 0, None));
    }
    lines.push(format!(
        "JAC_PoC_GrowthUI_Gameplay: [JAC_POC] initialized count={units}"
    ));
    let states = [(1, 1, 20), (2, 2, 10), (3, 2, 30), (4, 2, 50), (5, 2, 70)];
    for (turn, level, xp) in states {
        for unit in 1..=units {
            lines.push(growth_line("", unit, level, xp, Some(turn)));
        }
        if turn == 1 || turn == 5 {
            lines.push("JAC_Notebooks: [JAC_POC] notebooks path lookup found=true".into());
            lines.push("JAC_Notebooks: [JAC_POC] notebooks opened".into());
            lines.push(format!(
                "JAC_Notebooks: [JAC_POC] character journal shown count={units}"
            ));
        }
    }
    for unit in 1..=units {
        lines.push(growth_line("restored ", unit, 2, 70, None));
    }
    lines.push(format!(
        "JAC_PoC_GrowthUI_Gameplay: [JAC_POC] restored count={units}"
    ));
    lines.push("JAC_Notebooks: [JAC_POC] notebooks opened".into());
    lines.push(format!(
        "JAC_Notebooks: [JAC_POC] character journal shown count={units}"
    ));
    for unit in 1..=units {
        lines.push(growth_line("", unit, 3, 5, Some(6)));
    }
    let modding="7696e937-2044-4f3c-8e13-4e27632e2710\nApplying Component - JAC_GrowthUI_Text (UpdateText)\nApplying Component - JAC_GrowthUI_Gameplay (AddGameplayScripts)\nApplying Component - JAC_GrowthUI_InGameUI (AddUserInterfaces)\nApplying Component - JAC_GrowthUI_PartialScreenHooks (ReplaceUIScript)\nApplying Component - JAC_GrowthUI_UIFiles (ImportFiles)\n";
    logs(&(lines.join("\n") + "\n"), "", modding)
}

#[test]
fn bundle0_success_and_unrelated_database_error() {
    let checks = judge(&sample(), "bundle0", None, false);
    assert!(checks.len() >= 9);
    assert!(checks.iter().all(|c| c.passed), "{checks:?}");
}
#[test]
fn modding_errors_fail_every_profile_without_reusing_a_later_mod_load() {
    for source in [
        "Mods/JAC_PoC_Bundle0/Data.xml",
        "dfa0717e-4d0b-4370-b693-27fc6758571b",
        "7696e937-2044-4f3c-8e13-4e27632e2710",
    ] {
        for error in [
            format!("ERROR: failed {source}\n"),
            format!("Loading {source}\nERROR: invalid XML\n"),
            format!("ERROR: invalid XML\nfrom file {source}\n"),
        ] {
            let mut input = sample();
            input.get_mut("Modding.log").unwrap().push_str(&error);
            for profile in ["bundle0", "growth", "japan_map", "generic"] {
                let checks = judge(&input, profile, Some(2), false);
                let check = checks
                    .iter()
                    .find(|c| c.name == "Generic: JAC Modding.log errors")
                    .unwrap();
                assert!(!check.passed, "{profile}: {error}");
                assert_eq!(check.detail, "1 JAC-related modding errors");
            }
        }
    }
    let mut input = sample();
    input.get_mut("Modding.log").unwrap().push_str("Loading OfficialDLC.xml\nERROR: official DLC warning\nLoading Mods/JAC_PoC_Bundle0/Data.xml\n");
    assert!(judge(&input, "bundle0", None, false)
        .iter()
        .all(|c| c.passed));
}

#[test]
fn database_errors_fail_the_generic_profile_too() {
    let input = logs(
        "",
        "ERROR: invalid XML\nfrom file Mods/JAC_PoC_Bundle0/Data.xml\n",
        "",
    );
    assert_eq!(
        failed(&input, "generic", None),
        BTreeSet::from(["Generic: JAC Database.log errors".into()])
    );
}

#[test]
fn every_repository_mod_id_is_owned_in_both_logs_without_requiring_its_path() {
    fn collect(dir: &std::path::Path, ids: &mut BTreeSet<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                collect(&path, ids);
            } else if path.extension().is_some_and(|v| v == "modinfo")
                && entry.file_name().to_string_lossy().starts_with("JAC_")
            {
                let xml = fs::read_to_string(&path).unwrap();
                let doc = roxmltree::Document::parse(xml.trim_start_matches('\u{feff}')).unwrap();
                ids.insert(
                    doc.root_element()
                        .attribute("id")
                        .unwrap()
                        .to_ascii_uppercase(),
                );
            }
        }
    }
    let mut ids = BTreeSet::new();
    collect(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../poc"),
        &mut ids,
    );
    assert!(ids.contains("BD7A82C5-6B38-4B69-A7F5-8DC9AF534249"));
    assert!(ids.contains("61320778-2E75-4C11-9009-94101B519ABA"));
    assert_eq!(
        ids.len(),
        14,
        "update the owned MOD inventory when adding a MOD"
    );
    for id in ids {
        let input = logs(
            "",
            &format!("ERROR: invalid schema for {id}\n"),
            &format!("ERROR: could not load {id}\n"),
        );
        assert_eq!(
            failed(&input, "generic", None),
            BTreeSet::from([
                "Generic: JAC Modding.log errors".into(),
                "Generic: JAC Database.log errors".into()
            ]),
            "MOD {id}"
        );
    }
    assert!(failed(
        &logs(
            "",
            "ERROR: official mod aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa\n",
            "ERROR: official mod aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa\n"
        ),
        "generic",
        None
    )
    .is_empty());
}
#[test]
fn restored_state_mismatch_fails() {
    let mut l = sample();
    l.get_mut("Lua.log")
        .unwrap()
        .clone_from(&LUA.replace("lastTurn=4 turnsSeen=4", "lastTurn=3 turnsSeen=4"));
    assert!(failed(&l, "bundle0", None).contains("Bundle0: restored values"));
}
#[test]
fn missing_action_fails() {
    let mut l = sample();
    l.get_mut("Modding.log")
        .unwrap()
        .clone_from(&MODDING.replace(
            "Applying Component - JAC_PoC_Bundle0_Scripts",
            "Skipped - JAC_PoC_Bundle0_Scripts",
        ));
    assert!(failed(&l, "bundle0", None).contains("Bundle0: script action"));
}
#[test]
fn generic_errors_include_stack_and_api_error() {
    let mut l = sample();
    l.get_mut("Lua.log").unwrap().push_str("JAC_CustomCivs: [JAC_POC] result=api_error error=function expected\nRuntime Error: bad argument\n  [C]: pcall\n  Mods/JAC_PoC_CustomCivs/Scripts.lua:4\n");
    assert_eq!(
        failed(&l, "generic", None),
        ["Generic: [JAC_POC] errors", "Generic: JAC Runtime Error"]
            .into_iter()
            .map(str::to_string)
            .collect()
    );
}
#[test]
fn missing_jac_icon_or_texture_fails() {
    let mut l = sample();
    let lua = l.get_mut("Lua.log").unwrap();
    lua.push_str("DiplomacyRibbon: Could not find icon in CivilizationIcon.UpdateIcon: icon=\"ICON_CIVILIZATION_CHINA\"\n");
    assert!(failed(&l, "generic", None).is_empty());
    l.get_mut("Lua.log").unwrap().push_str("DiplomacyRibbon: Could not find icon in CivilizationIcon.UpdateIcon: icon=\"ICON_CIVILIZATION_JAC_AINU\"\n");
    assert_eq!(
        failed(&l, "generic", None),
        ["Generic: JAC missing texture".into()]
            .into_iter()
            .collect()
    );
}
#[test]
fn missing_jac_texture_fails() {
    let mut l = sample();
    l.get_mut("Lua.log").unwrap().push_str("LoadScreen: Failed to load background image texture: LEADER_JAC_MORI_MOTONARI_BACKGROUND\nLoadScreen: We are lacking a texture for LEADER_JAC_MORI_MOTONARI_NEUTRAL\nLoadScreen: We are lacking a texture for LEADER_HOJO_NEUTRAL\n");
    let checks = judge(&l, "generic", None, false);
    let texture = checks
        .iter()
        .find(|c| c.name == "Generic: JAC missing texture")
        .unwrap();
    assert!(!texture.passed);
    assert_eq!(texture.detail, "2 matching lines");
}
#[test]
fn error_none_is_not_a_failure() {
    let mut l = sample();
    l.get_mut("Lua.log")
        .unwrap()
        .push_str("JAC: [JAC_POC] result=unavailable error=none\n");
    assert!(failed(&l, "generic", None).is_empty());
}
#[test]
fn database_error_with_mod_context_fails() {
    let mut l = sample();
    l.get_mut("Database.log").unwrap().push_str(
        "[100] ERROR: constraint failed\n[100] from file Mods/JAC_PoC_Bundle0/Data/a.sql\n",
    );
    assert!(failed(&l, "bundle0", None).contains("Bundle0: database errors"));
}
#[test]
fn growth_success_and_restored_mismatch() {
    let l = growth_logs(1);
    let checks = judge(&l, "growth", Some(1), false);
    assert!(checks.len() >= 12);
    assert!(checks.iter().all(|c| c.passed), "{checks:?}");
    let mut bad = l.clone();
    let value = bad["Lua.log"].replace(
        "restored unit=1 level=2 xp=70",
        "restored unit=1 level=2 xp=50",
    );
    bad.insert("Lua.log".into(), value);
    assert!(failed(&bad, "growth", Some(1)).contains("Growth: restored rows and journal"));
}
#[test]
fn growth_expected_two_units_in_assignment_restore_and_journal() {
    let l = growth_logs(2);
    assert!(
        failed(&l, "growth", Some(2)).is_empty(),
        "{:?}",
        judge(&l, "growth", Some(2), false)
    );
    for (needle, expected) in [
        ("assigned unit=2 ", "Growth: new-game assignment"),
        ("restored unit=2 ", "Growth: restored rows and journal"),
    ] {
        let mut bad = l.clone();
        let line = bad["Lua.log"]
            .lines()
            .find(|v| v.contains(needle))
            .unwrap()
            .to_string()
            + "\n";
        bad.insert("Lua.log".into(), bad["Lua.log"].replacen(&line, "", 1));
        assert!(failed(&bad, "growth", Some(2)).contains(expected));
    }
    let mut bad = l.clone();
    bad.insert(
        "Lua.log".into(),
        bad["Lua.log"].replace(
            "character journal shown count=2",
            "character journal shown count=1",
        ),
    );
    assert!(failed(&bad, "growth", Some(2)).contains("Growth: restored rows and journal"));
}
#[test]
fn growth_ignores_unrelated_game_error_and_catches_target_ui_error() {
    let mut l = growth_logs(1);
    l.get_mut("Lua.log").unwrap().push_str("BaseGamePanel: UI.DataError: unrelated game panel\nRuntime Error: base game failure\n  Assets/UI/BaseGamePanel.lua:21\nBaseGamePanel: Cannot determine if partial screen for BaseGamePanel\nJAC_Notebooks: [JAC_POC] notebooks opened\n");
    assert!(failed(&l, "growth", Some(1)).is_empty());
    l.get_mut("Lua.log")
        .unwrap()
        .push_str("UI.DataError: Cannot determine if partial screen for JAC_Notebooks\n");
    assert_eq!(
        failed(&l, "growth", Some(1)),
        ["Growth: partial-screen errors".into()]
            .into_iter()
            .collect()
    );
    l.get_mut("Lua.log")
        .unwrap()
        .push_str("Runtime Error: JAC UI failure\n  Mods/JAC_PoC_GrowthUI/JAC_Notebooks.lua:4\n");
    assert_eq!(
        failed(&l, "growth", Some(1)),
        [
            "Growth: partial-screen errors".into(),
            "Generic: JAC Runtime Error".into()
        ]
        .into_iter()
        .collect()
    );
}
#[test]
fn japan_map_fails_on_congress_dependent_ui_error() {
    let mut l=logs("Runtime Error: unrelated\n  BaseGamePanel.lua:21\n","","403be889-1455-4de7-967b-ad54c0ff3431\nApplying Component - JAC_JapanMapScale_InGameText (UpdateText)\n");
    assert!(failed(&l, "japan_map", None).is_empty());
    for text in [
        "Runtime Error: attempt to index a nil value\n  ActionPanel_Expansion2.lua:44\n",
        "UI.DataError: WorldCongressPopup invalid stage\n",
        "Runtime Error: attempt to index a nil value\n  GovernorPanel.lua:723\n",
        "Runtime Error: attempt to index a nil value\n  CityPanel_Expansion2.lua:11\n",
    ] {
        l.insert("Lua.log".into(), text.into());
        assert!(failed(&l, "japan_map", None).contains("Japan map: GS Congress paths"));
    }
}
#[test]
fn baseline_excludes_old_lines_and_handles_truncate() {
    let old = b"old error\n";
    let mark =
        json!({"offset":old.len(),"inode":7,"prefix_sha256":format!("{:x}",Sha256::digest(old))});
    assert_eq!(
        slice_log_bytes(b"old error\nnew line\n", &mark, 7),
        "new line\n"
    );
    assert_eq!(slice_log_bytes(b"fresh\n", &mark, 7), "fresh\n");
    assert_eq!(slice_log_bytes(b"new file\n", &mark, 8), "new file\n");
}
#[test]
fn baseline_detects_same_inode_rewrite_past_old_offset() {
    let dir = tempfile::tempdir().unwrap();
    for name in LOG_NAMES {
        fs::write(dir.path().join(name), b"").unwrap();
    }
    let lua = dir.path().join("Lua.log");
    let original = [vec![b'P'; 256], vec![b'M'; 256], vec![b'T'; 256]].concat();
    fs::write(&lua, &original).unwrap();
    let baseline = snapshot_logs(dir.path()).unwrap();
    use std::io::Write;
    fs::OpenOptions::new()
        .append(true)
        .open(&lua)
        .unwrap()
        .write_all(b"appended\n")
        .unwrap();
    assert_eq!(
        read_since(dir.path(), &baseline).unwrap()["Lua.log"],
        "appended\n"
    );
    let replacement = [
        vec![b'P'; 256],
        vec![b'R'; 256],
        vec![b'U'; 256],
        b"fresh\n".to_vec(),
    ]
    .concat();
    fs::write(&lua, &replacement).unwrap();
    assert_eq!(
        read_since(dir.path(), &baseline).unwrap()["Lua.log"],
        String::from_utf8(replacement).unwrap()
    );
}
#[test]
fn marker_detects_inode_and_tail_changes() {
    let mark = json!({"offset":4,"inode":9,"prefix_length":2,"prefix_sha256":format!("{:x}",Sha256::digest(b"ab")),"tail_length":2,"tail_sha256":format!("{:x}",Sha256::digest(b"cd"))});
    assert!(!log_marker_replaced(b"abcdmore", &mark, 9));
    assert!(log_marker_replaced(b"abXYmore", &mark, 9));
    assert!(log_marker_replaced(b"abcdmore", &mark, 10));
}
#[test]
fn strict_errors_and_invalid_input_fail_closed() {
    let mut l = sample();
    l.get_mut("Lua.log")
        .unwrap()
        .push_str("UI.DataError: official UI\n");
    assert!(judge(&l, "generic", None, true)
        .iter()
        .any(|c| c.name == "Scenario: all Lua errors" && !c.passed));
    assert!(judge(&l, "growth", None, false).iter().any(|c| !c.passed));
    assert!(judge(&l, "missing", None, false).iter().any(|c| !c.passed));
}

#[test]
fn japan_map_attributes_errors_to_loaded_mod_and_ignores_following_loads() {
    let base="403be889-1455-4de7-967b-ad54c0ff3431\nApplying Component - JAC_JapanMapScale_InGameText (UpdateText)\n";
    let clean = logs(
        "",
        "",
        &format!(
            "{base}ERROR: official DLC warning\nLoading Mods/JAC_JapanMapScale/gameplay.xml\n"
        ),
    );
    assert!(failed(&clean, "japan_map", None).is_empty());
    let owned = logs(
        "",
        "",
        &format!("{base}Loading Mods/JAC_JapanMapScale/gameplay.xml\nERROR: invalid XML\n"),
    );
    assert!(failed(&owned, "japan_map", None).contains("Japan map: Modding.log"));
    let id_source = logs(
        "",
        "",
        &format!(
            "{base}Loading {id}\nERROR: invalid XML\n",
            id = "403be889-1455-4de7-967b-ad54c0ff3431"
        ),
    );
    assert!(failed(&id_source, "japan_map", None).contains("Japan map: Modding.log"));
}

#[test]
fn missing_required_log_and_bom_are_explicit() {
    let dir = tempfile::tempdir().unwrap();
    for name in LOG_NAMES {
        fs::write(dir.path().join(name), b"").unwrap();
    }
    let baseline = snapshot_logs(dir.path()).unwrap();
    fs::remove_file(dir.path().join("Database.log")).unwrap();
    assert!(read_since(dir.path(), &baseline)
        .unwrap_err()
        .to_string()
        .contains("Required Civ6 log is missing"));
    let mark = json!({"offset":0,"inode":null,"prefix_length":0,"prefix_sha256":null});
    assert_eq!(
        slice_log_bytes(b"\xef\xbb\xbfnew\xef\xbf\xbd", &mark, 1),
        "new�"
    );
}

#[test]
fn malformed_huge_turn_counters_fail_closed() {
    let mut l = sample();
    l.insert(
        "Lua.log".into(),
        LUA.replace(
            "turnsSeen=1",
            "turnsSeen=999999999999999999999999999999999999999999",
        ),
    );
    assert!(failed(&l, "bundle0", None).contains("Bundle0: turns progress"));
}

#[test]
fn shipped_scenario_inventory_and_json_are_intact() {
    let folder = civ6_autotest_rs::repo_root().join("tools/civ6-autotest/scenarios");
    let expected:Vec<_>="assassin-leader-save-load.json banseiki-ruleset.json bundle0-load-save.json clans-notebooks.json clans-house-names.json clans-both-ai.json custom-clans.json leader-portraits.json growth-notebooks.json sengoku-trees.json sengoku-starting-knowledge.json sengoku-calendar.json unification-victory.json unification-registry-15km.json unification-pact.json sengoku-house-names-oda.json sengoku-house-names-takeda.json sengoku-alliances.json ages-probe.json sengoku-ages.json sengoku-dark-policy.json sengoku-ages-screens.json historical-events-15km.json religion-persistence.json religion-pressure-probe.json religion-hero-delay.json house-crests.json japan-map-20km.json japan-map-15km.json japan-map-20km-takeda.json japan-map-15km-takeda.json japan-1534-relations.json probe-ally-vision.json probe-alliance-type.json probe-alliance-expiry.json probe-end-alliance.json japan-1534-relations-settlers.json japan-1534-feud-ui.json japan-resources.json japan-no-world-congress.json japan-city-names.json japan-house-city-names.json japan-map-setup-screen.json japan-map-15km-settlers.json japan-map-15km-200turn.json japan-15km-200turn-full.json japan-15km-200turn-tuning.json japan-prosperous-15km.json court-arbitration.json court-arbitration-human.json court-arbitration-ai.json probe-make-peace.json probe-era-timing-standard.json probe-era-timing-quick.json sengoku-era-reigns.json sengoku-oaths-oda.json japan-15km-20turn-full-metrics.json japan-15km-300turn-full-metrics.json sengoku-era-reigns-epic.json sengoku-era-reigns-marathon.json sengoku-era-reigns-beyond-nine.json great-people-zero-musicians.json resources-calendar-epic.json".split_whitespace().map(str::to_string).collect();
    let expected_names: Vec<_> = expected
        .into_iter()
        .chain(
            [
                "assassin-blocked-recovery.json",
                "absorption-japan.json",
                "absorption-japan-recall-continuation.json",
                "absorption-japan-owner-turn-diagnostic.json",
                "absorption-japan-owner-turn-isolated.json",
                "absorption-hero-recall.json",
                "absorption-priest-diagnostic.json",
                "absorption-religion-persistence.json",
                "absorption-spent-spread-192.json",
                "absorption-spent-spread-block-192.json",
                "absorption-faith-precision.json",
                "absorption-hero-window-diagnostic.json",
                "absorption-japan-native.json",
                "absorption-context-probe.json",
                "absorption-carrier-boundary-load.json",
                "absorption-flow.json",
                "absorption-native-custody.json",
                "absorption-native-deal.json",
                "absorption-native-product-probe.json",
                "absorption-native-product-resume.json",
                "absorption-native-product-finish.json",
                "absorption-native-scatter-inventory.json",
                "absorption-native-scatter-carrier.json",
                "absorption-native-scatter-flow-start.json",
                "absorption-native-scatter-physical-probe.json",
                "absorption-native-bridge-probe.json",
                "absorption-native-ai-same-owner-move.json",
                "absorption-native-ai-carrier-return.json",
                "absorption-native-carrier-side-effects.json",
                "absorption-native-cross-owner-freecity.json",
                "absorption-native-freecity-recover-original.json",
                "absorption-native-reverse-carrier-recovery.json",
                "absorption-native-scatter-reward-site.json",
                "absorption-native-scatter-recovery.json",
                "absorption-native-acquired-palace-product.json",
                "absorption-native-acquired-carrier-diff.json",
                "absorption-native-acquired-carrier-resume.json",
                "absorption-native-acquired-result-final.json",
                "absorption-native-work-probe.json",
                "absorption-native-artwork-locks.json",
                "absorption-native-artwork-aged.json",
                "absorption-native-artifact-full.json",
                "absorption-native-production-core-probe.json",
                "absorption-native-excavation-resume.json",
                "absorption-native-type-art-prepare.json",
                "absorption-native-type-artifact-prepare.json",
                "absorption-native-type-city-owner-transfer.json",
                "absorption-native-type-return.json",
                "absorption-native-type-scatter.json",
                "absorption-native-type-run-art-city-fresh.json",
                "absorption-native-type-run-art-return-fresh.json",
                "absorption-native-type-run-art-scatter-fresh.json",
                "absorption-native-type-run-artifact-city.json",
                "absorption-native-type-run-artifact-return.json",
                "absorption-native-type-run-artifact-scatter.json",
                "absorption-return-boundary.json",
                "assassin-actor-eliminated-save-load.json",
                "assassin-target-eliminated-save-load.json",
                "hotseat-bridge-live-hold.json",
                "multiplayer-banseiki-hotseat-growth.json",
                "multiplayer-hotseat-growth.json",
                "multiplayer-japan-15km-settlers-hotseat-growth.json",
                "mod-selection-rollback.json",
                "banseiki-pool-hotseat.json",
                "banseiki-pool-single-player.json",
                "banseiki-pool-bridge.json",
                "banseiki-pool-random-bridge.json",
                "reliability-seed.json",
                "reliability-cycle.json",
            ]
            .into_iter()
            .map(str::to_string),
        )
        .collect();
    let expected: BTreeSet<_> = expected_names.iter().cloned().collect();
    assert_eq!(
        expected.len(),
        expected_names.len(),
        "scenario inventory must not repeat a filename"
    );
    let actual: BTreeSet<_> = fs::read_dir(&folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".json"))
        .collect();
    assert_eq!(actual, expected);
    for name in &actual {
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(folder.join(name)).unwrap()).unwrap();
        assert!(value.is_object(), "{name}");
        civ6_autotest_rs::scenario::validate_scenario(&value).unwrap();
        let lua = mlua::Lua::new();
        for (index, step) in value["steps"].as_array().unwrap().iter().enumerate() {
            for code in [step["code"].as_str(), step["condition"]["code"].as_str()]
                .into_iter()
                .flatten()
            {
                lua.load(code)
                    .set_name(format!("{name}: step {}", index + 1))
                    .into_function()
                    .unwrap();
            }
        }
    }
    let growth: serde_json::Value =
        serde_json::from_slice(&fs::read(folder.join("growth-notebooks.json")).unwrap()).unwrap();
    assert_eq!(growth["expected_units"], 2);
}
