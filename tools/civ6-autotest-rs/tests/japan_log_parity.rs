use civ6_autotest_rs::judge;
use std::collections::{BTreeMap, BTreeSet};
fn base() -> BTreeMap<String, String> {
    BTreeMap::from([("Lua.log".into(),"".into()),("Database.log".into(),"".into()),("Modding.log".into(),"403be889-1455-4de7-967b-ad54c0ff3431\nApplying Component - JAC_JapanMapScale_Config (UpdateDatabase)\nApplying Component - JAC_JapanMapScale_InGameText (UpdateText)\n".into())])
}
fn failures(logs: &BTreeMap<String, String>) -> BTreeSet<String> {
    judge::judge(logs, "japan_map", None, false)
        .into_iter()
        .filter(|c| !c.passed)
        .map(|c| c.name)
        .collect()
}

#[test]
fn test_map_log_profile_detects_mod_errors() {
    let logs = base();
    assert!(failures(&logs).is_empty());
    let mut broken = logs.clone();
    broken.insert(
        "Database.log".into(),
        "ERROR JAC_JapanMapScale_InGameText invalid tag\n".into(),
    );
    assert!(failures(&broken).contains("Japan map: Database.log"));
    for(extra,passes)in[("LocalizedText - Loading Text/CivRoyaleScenario_Translations_ConfigText_China.xml\nWarning: LocalizedText - Failed loading XML.\nUISettingHandler - Loading JAC_PoC_CustomCivs.dep\n",true),("LocalizedText - Loading Text/JAC_JapanMapScale_Text.xml\nWarning: LocalizedText - Failed loading XML.\n",false),("Warning: Unable to load Text/JAC_JapanMapScale_Text.xml\nWarning: LocalizedText - Failed loading XML.\n",false),("Warning: Unable to load ../../../DLC/Australia/Text/Australia_Translations_ConfigText_China.xml\nWarning: LocalizedText - Failed loading XML.\n",true)]{let mut modified=logs.clone();modified.get_mut("Modding.log").unwrap().push_str(extra);assert_eq!(!failures(&modified).contains("Japan map: Modding.log"),passes,"{extra}");}
    let mut broken = logs.clone();
    broken
        .get_mut("Modding.log")
        .unwrap()
        .push_str("ERROR JAC_JapanMapScale_Config failed\n");
    assert!(failures(&broken).contains("Japan map: Modding.log"));
}

#[test]
fn test_following_file_diagnostic_belongs_to_error() {
    let mut logs = base();
    logs.insert("Modding.log".into(),"403be889-1455-4de7-967b-ad54c0ff3431\nApplying Component - JAC_JapanMapScale_InGameText (UpdateText)\n".into());
    let diagnostic="[3963954.476] [Localization] ERROR: UNIQUE constraint failed: LocalizedText.Language, LocalizedText.Tag\n[3963954.476] [Localization]: While executing - 'insert into BaseGameText('Tag', 'Text') values (?, ?);'\n[3963954.476] [Localization]: In XMLSerializer while updating table BaseGameText from file Data/JAC_Japan_MapRules.xml.\n";
    for log_name in ["Database.log", "Modding.log"] {
        let mut modified = logs.clone();
        modified.get_mut(log_name).unwrap().push_str(diagnostic);
        assert!(failures(&modified).contains(&format!("Japan map: {log_name}")));
    }
    let mut unrelated = logs.clone();
    unrelated.insert(
        "Database.log".into(),
        diagnostic.replace(
            "Data/JAC_Japan_MapRules.xml",
            "CurrentClickouts/Text/ClickoutText_en_US.xml",
        ),
    );
    assert!(failures(&unrelated).is_empty());
    let mut modified = logs;
    modified.get_mut("Modding.log").unwrap().push_str("[3964923.674] LocalizedText - Loading Text/JAC_JapanMapScale_Text.xml\n[3964923.674] Warning: Unable to load Text/JAC_JapanMapScale_Text.xml\n[3964923.674] Warning: LocalizedText - Failed loading XML.\n[3964923.675] Successfully released save point.\n");
    assert!(failures(&modified).contains("Japan map: Modding.log"));
}
