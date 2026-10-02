use civ6_lua_tests_rs::strict::{assert_boolean_identity, ChunkExt, Data, FromLuaMulti, TableExt};
use civ6_lua_tests_rs::{exec, exec_args, json_to_lua, new_lua, read_repo, repo_root};
use mlua::{Lua, Table};
use regex::Regex;
use roxmltree::{Document, Node};
use serde_json::{json, Value as Json};
use std::collections::{BTreeMap, BTreeSet};

const MOD: &str = "poc/JAC_PoC_CourtArbitration/";
const FIXTURE: &str = "tools/civ6-lua-tests-rs/fixtures/court/";

mod fidelity_regressions {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    #[test]
    fn shogunate_end_comparison_rejects_numeric_strings_and_fractional_numbers() {
        let h = Court::new();
        h.eq("JAC_ARB.ShogunateEndTurn()", 120_i64);
        for result in ["'120'", "120.5", "true", "nil", "120,99"] {
            h.run(&format!(
                "JAC_ARB.ShogunateEndTurn=function() return {result} end"
            ));
            assert!(
                catch_unwind(AssertUnwindSafe(
                    || h.eq("JAC_ARB.ShogunateEndTurn()", 120_i64)
                ))
                .is_err(),
                "{result}"
            );
        }
    }

    #[test]
    fn overlay_identity_check_rejects_nil_instead_of_false() {
        let h = Court::scenario();
        h.run(&fixture("overlay_setup.lua"));
        h.run(&mod_source("UI/JAC_CourtArbitrationPanel.lua"));
        h.identity("ContextPtr:IsHidden()", false);
        h.run("ContextPtr.IsHidden=function() return nil end");
        assert!(catch_unwind(AssertUnwindSafe(
            || h.identity("ContextPtr:IsHidden()", false)
        ))
        .is_err());
        h.run("ContextPtr.IsHidden=function() return false end");
        h.identity("ContextPtr:IsHidden()", false);
    }

    #[test]
    fn all_four_cost_and_penalty_comparisons_reject_extra_results_and_wrong_kinds() {
        for (function, power, expected) in [
            ("Costs", 1, (100_i64, 5_i64)),
            ("Costs", 3, (34, 2)),
            ("RefusalPenalty", 1, (15, 10)),
            ("RefusalPenalty", 3, (45, 30)),
        ] {
            let h = Court::new();
            let expression = format!("JAC_ARB.{function}({power})");
            h.eq(&expression, expected);
            for result in [
                format!("{},{},99", expected.0, expected.1),
                format!("{},{},nil", expected.0, expected.1),
                format!("'{}',{}", expected.0, expected.1),
            ] {
                h.run(&format!(
                    "JAC_ARB.{function}=function() return {result} end"
                ));
                assert!(
                    catch_unwind(AssertUnwindSafe(|| h.eq(&expression, expected))).is_err(),
                    "{expression}: {result}"
                );
            }
        }
    }
}

fn mod_source(path: &str) -> String {
    read_repo(format!("{MOD}{path}"))
}

fn fixture(path: &str) -> String {
    read_repo(format!("{FIXTURE}{path}"))
}

fn child<'a>(node: Node<'a, 'a>, name: &str) -> Node<'a, 'a> {
    node.children()
        .find(|n| n.is_element() && n.has_tag_name(name))
        .unwrap_or_else(|| panic!("missing {name}"))
}

fn text_at<'a>(node: Node<'a, 'a>, names: &[&str]) -> &'a str {
    let mut at = node;
    for name in names {
        at = child(at, name);
    }
    at.text().unwrap().trim()
}

fn calendar() -> Json {
    let xml = read_repo("poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Calendar.xml");
    let doc = Document::parse(&xml).unwrap();
    let root = doc.root_element();
    let rows = child(root, "GameSpeed_Turns")
        .children()
        .filter(|n| n.has_tag_name("Row"))
        .map(|row| {
            json!({
                "GameSpeedType": text_at(row, &["GameSpeedType"]),
                "TurnsPerIncrement": text_at(row, &["TurnsPerIncrement"]).parse::<i64>().unwrap(),
                "MonthIncrement": text_at(row, &["MonthIncrement"]).parse::<i64>().unwrap(),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "StartYear": text_at(root, &["StartEras", "Update", "Set", "Year"]).parse::<i64>().unwrap(),
        "GameSpeed_Turns": rows,
    })
}

struct Court {
    lua: Lua,
    turn: i64,
}

impl Court {
    fn new() -> Self {
        let mut court = Self::with_civs(&[
            "CIVILIZATION_JAC_OUCHI",
            "CIVILIZATION_JAC_OTOMO",
            "CIVILIZATION_JAC_ASHIKAGA",
            "CIVILIZATION_JAC_MORI",
        ]);
        court.run("JAC_ARB.AI_EXHAUSTION_THRESHOLD=9999; JAC_ARB.AI_SHOGUN_THRESHOLD=9999; SET_WAR(0,1,true)");
        court.next(1);
        court
    }

    fn with_civs(civs: &[&str]) -> Self {
        let lua = new_lua();
        let civs_table = lua.create_table().unwrap();
        for (index, civ) in civs.iter().enumerate() {
            civs_table.set(index + 1, *civ).unwrap();
        }
        let rules_path = repo_root()
            .join(format!("{MOD}Scripts/JAC_CourtArbitration_Rules.lua"))
            .to_string_lossy()
            .replace('\\', "/");
        exec_args(
            &lua,
            &fixture("stubs.lua"),
            (
                json_to_lua(&lua, &calendar()),
                civs_table,
                16603,
                rules_path,
            ),
        );
        exec(&lua, &mod_source("Scripts/JAC_CourtArbitration.lua"));
        Self { lua, turn: 1 }
    }

    fn ai() -> Self {
        let court = Self::new();
        court.run("JAC_ARB.AI_EXHAUSTION_THRESHOLD=6; JAC_ARB.AI_SHOGUN_THRESHOLD=3");
        court
    }

    fn scenario() -> Self {
        let mut court = Self::with_civs(&[
            "CIVILIZATION_JAC_OUCHI",
            "CIVILIZATION_JAC_OTOMO",
            "CIVILIZATION_JAC_ASHIKAGA",
            "CIVILIZATION_JAC_MORI",
            "CIVILIZATION_JAC_ODA",
            "CIVILIZATION_JAC_TAKEDA",
        ]);
        court.run("MIN_WAR=10; SET_WAR(0,1,true); Players[0].cities=4; Players[2].human=true");
        court.next(1);
        court
    }

    fn run(&self, code: &str) {
        self.lua
            .load(code)
            .exec()
            .unwrap_or_else(|error| panic!("{code}: {error}"));
    }

    fn strict_eval<T: FromLuaMulti>(&self, expression: &str) -> T {
        self.lua
            .load(format!("return {expression}"))
            .strict_eval()
            .unwrap_or_else(|error| panic!("{expression}: {error}"))
    }

    fn eq(&self, expression: &str, expected: impl Into<Data>) {
        let actual = self
            .lua
            .load(format!("return {expression}"))
            .python_eval()
            .unwrap();
        let expected = expected.into();
        assert!(
            actual.python_eq(&expected),
            "{expression}: actual {actual:?}, expected {expected:?}"
        );
    }
    fn identity(&self, expression: &str, expected: bool) {
        assert_boolean_identity(&self.strict_eval::<Data>(expression), expected);
    }
    fn truth(&self, expression: &str, expected: bool) {
        assert_eq!(
            self.lua
                .load(format!("return {expression}"))
                .python_truthy()
                .unwrap(),
            expected,
            "{expression}"
        );
    }

    fn next(&mut self, turn: i64) {
        self.turn = turn;
        self.run(&format!("START_TURN({turn})"));
    }

    fn advance(&mut self) {
        self.next(self.turn + 1);
    }

    fn logs(&self) -> Vec<String> {
        self.lua
            .globals()
            .strict_get::<Table>("LOG")
            .unwrap()
            .strict_values::<String>()
            .collect::<mlua::Result<Vec<_>>>()
            .unwrap()
    }

    fn log_has(&self, line: &str) {
        assert!(
            self.logs().iter().any(|entry| entry == line),
            "missing {line:?} in {:?}",
            self.logs()
        );
    }

    fn log_contains(&self, fragment: &str) {
        assert!(
            self.logs().iter().any(|entry| entry.contains(fragment)),
            "missing {fragment:?} in {:?}",
            self.logs()
        );
    }

    fn log_lacks(&self, fragment: &str) {
        assert!(
            self.logs().iter().all(|entry| !entry.contains(fragment)),
            "unexpected {fragment:?} in {:?}",
            self.logs()
        );
    }

    fn exhaust(&self, player: i64) {
        self.run(&format!(
            "Players[{player}].cities=1; Players[{player}].units=1"
        ));
    }

    fn refuse_on_turn_2(&mut self) {
        self.run("Players[1].cities=4; Players[2].human=true; REQUEST(0,0,1)");
        self.next(2);
    }
}

mod scope_and_text_tests {
    use super::*;

    #[test]
    fn test_every_action_loads_only_on_the_four_banseiki_japan_maps() {
        let xml = mod_source("JAC_PoC_CourtArbitration.modinfo");
        let doc = Document::parse(&xml).unwrap();
        let root = doc.root_element();
        let criteria = child(root, "ActionCriteria")
            .children()
            .filter(|n| n.has_tag_name("Criteria"))
            .map(|n| (n.attribute("id").unwrap(), n))
            .collect::<BTreeMap<_, _>>();
        let prefix = "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/";
        let expected = [
            "JAC_Japan_20km.Civ6Map",
            "JAC_Japan_15km.Civ6Map",
            "JAC_Japan_20km_Settlers.Civ6Map",
            "JAC_Japan_15km_Settlers.Civ6Map",
        ]
        .map(|name| format!("{prefix}{name}"))
        .into_iter()
        .collect::<BTreeSet<_>>();
        let actions = child(root, "InGameActions");
        for kind in [
            "UpdateDatabase",
            "UpdateText",
            "AddGameplayScripts",
            "AddUserInterfaces",
        ] {
            let mut selected = BTreeSet::new();
            for action in actions.children().filter(|n| n.has_tag_name(kind)) {
                let criterion = criteria[action.attribute("criteria").unwrap()];
                assert_eq!(
                    text_at(criterion, &["RuleSetInUse"]),
                    "RULESET_JAC_BANSEIKI"
                );
                assert_eq!(
                    text_at(criterion, &["ConfigurationValueMatches", "ConfigurationId"]),
                    "MAP_SCRIPT"
                );
                selected.insert(
                    text_at(criterion, &["ConfigurationValueMatches", "Value"]).to_string(),
                );
            }
            assert_eq!(selected, expected, "{kind}");
        }
        for file in child(root, "Files")
            .children()
            .filter(|n| n.has_tag_name("File"))
        {
            let path = file.text().unwrap();
            assert!(repo_root().join(MOD).join(path).is_file(), "{path}");
        }
    }

    #[test]
    fn test_every_text_key_used_by_the_lua_exists_in_three_languages() {
        let xml = mod_source("Text/JAC_CourtArbitration_Text.xml");
        let doc = Document::parse(&xml).unwrap();
        let mut languages: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for row in child(doc.root_element(), "LocalizedText")
            .children()
            .filter(|n| n.has_tag_name("Row"))
        {
            languages
                .entry(row.attribute("Tag").unwrap())
                .or_default()
                .insert(row.attribute("Language").unwrap());
        }
        let expected = BTreeSet::from(["ja_JP", "en_US", "ru_RU"]);
        for (tag, found) in &languages {
            assert_eq!(*found, expected, "{tag}");
        }
        let sources = [
            "UI/JAC_CourtArbitrationPanel.lua",
            "UI/JAC_CourtArbitrationPanel.xml",
            "Scripts/JAC_CourtArbitration.lua",
        ]
        .map(mod_source)
        .join("");
        let direct = Regex::new("\"(LOC_JAC_ARB_[A-Z_]+)\"").unwrap();
        let mut used = direct
            .captures_iter(&sources)
            .map(|m| m[1].to_string())
            .collect::<BTreeSet<_>>();
        used.remove("LOC_JAC_ARB_REASON_");
        let rules = mod_source("Scripts/JAC_CourtArbitration_Rules.lua");
        let reasons = Regex::new("return false, \"([A-Z_]+)\"").unwrap();
        used.extend(
            reasons
                .captures_iter(&rules)
                .map(|m| format!("LOC_JAC_ARB_REASON_{}", &m[1])),
        );
        let missing = used
            .into_iter()
            .filter(|key| !languages.contains_key(key.as_str()))
            .collect::<Vec<_>>();
        assert!(missing.is_empty(), "missing localization keys: {missing:?}");
    }

    #[test]
    fn test_the_modifiers_read_the_flags_the_script_writes() {
        let sql = mod_source("Data/JAC_CourtArbitration_Legitimacy.sql");
        let rules = mod_source("Scripts/JAC_CourtArbitration_Rules.lua");
        let flags = Regex::new("R\\.(FLAG_[A-Z]+) = \"([A-Z_]+)\"")
            .unwrap()
            .captures_iter(&rules)
            .map(|c| (c[1].to_string(), c[2].to_string()))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            flags.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            BTreeSet::from(["FLAG_HIGH", "FLAG_LOW", "FLAG_SUBJUGATION"])
        );
        for name in flags.values() {
            assert!(sql.contains(&format!("'PropertyName', '{name}'")), "{name}");
        }
        let rows = Regex::new("\\('(JAC_ARB_[A-Z_]+)', '(MODIFIER_[A-Z_]+)'")
            .unwrap()
            .captures_iter(&sql)
            .map(|c| (c[1].to_string(), c[2].to_string()))
            .collect::<BTreeMap<_, _>>();
        assert!(!sql.contains("IDENTITY"));
        let yields = ["HIGH", "LOW"]
            .into_iter()
            .flat_map(|level| {
                ["GOLD", "CULTURE"].map(move |yield_name| format!("JAC_ARB_{level}_{yield_name}"))
            })
            .collect::<Vec<_>>();
        for modifier in &yields {
            assert_eq!(
                rows[modifier],
                "MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_MODIFIER"
            );
            let parts = modifier.split('_').collect::<Vec<_>>();
            let (level, yield_name) = (parts[2], parts[3]);
            assert!(sql.contains(&format!("'MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_MODIFIER', NULL, 'JAC_ARB_CITY_LEGITIMACY_{level}')")));
            assert!(sql.contains(&format!(
                "('{modifier}', 'YieldType', 'YIELD_{yield_name}')"
            )));
            assert!(sql.contains(&format!(
                "('{modifier}', 'Amount', '{}')",
                if level == "HIGH" { "10" } else { "-10" }
            )));
        }
        assert_eq!(
            rows["JAC_ARB_WAR_WEARINESS"],
            "MODIFIER_PLAYER_ADJUST_WAR_WEARINESS"
        );
        assert!(sql.contains("('JAC_ARB_WAR_WEARINESS', 'MODIFIER_PLAYER_ADJUST_WAR_WEARINESS', 'JAC_ARB_CITY_SUBJUGATION_ANCHOR', NULL)"));
        for modifier in yields
            .iter()
            .map(String::as_str)
            .chain(["JAC_ARB_ATTACH_WAR_WEARINESS"])
        {
            assert!(sql.contains(&format!("('TRAIT_LEADER_MAJOR_CIV', '{modifier}')")));
        }
        let gameplay = mod_source("Scripts/JAC_CourtArbitration.lua");
        assert!(!Regex::new("GetFavor|ChangeFavor")
            .unwrap()
            .is_match(&(rules + &gameplay)));
    }

    #[test]
    fn test_ui_script_compiles() {
        let lua = new_lua();
        lua.load(mod_source("UI/JAC_CourtArbitrationPanel.lua"))
            .into_function()
            .unwrap();
    }
}

mod arbitration_turn_tests {
    use super::*;

    #[test]
    fn test_muromachi_end_is_1573_month_7_on_the_sengoku_calendar() {
        let h = Court::new();
        h.eq("JAC_ARB.ShogunateEndTurn()", 120_i64);
        h.truth("JAC_ARB.IsEmpoweredShogun(2,119)", true);
        h.truth("JAC_ARB.IsEmpoweredShogun(2,120)", false);
        h.truth("JAC_ARB.IsEmpoweredShogun(0,1)", false);
        h.log_has("[JAC_ARB] loaded shogunate_end_turn=120");
    }

    #[test]
    fn test_party_request_charges_the_cost_and_an_even_ai_accepts_next_turn() {
        let mut h = Court::new();
        h.eq("JAC_ARB.Legitimacy(0)", 53_i64);
        h.run("REQUEST(0,0,1)");
        h.eq("Players[0].gold, JAC_ARB.Legitimacy(0)", (400_i64, 48_i64));
        h.eq("Game:GetProperty('JAC_ARB_PENDING').Requester, Game:GetProperty('JAC_ARB_PENDING').A, Game:GetProperty('JAC_ARB_PENDING').B, Game:GetProperty('JAC_ARB_PENDING').Power", (0_i64, 0_i64, 1_i64, 1_i64));
        h.eq(
            "NOTES[1][1], NOTES[1][2], #NOTES",
            (1_i64, "LOC_JAC_ARB_NOTIFY_REQUEST_NAME".to_string(), 1_i64),
        );
        h.run("REQUEST(1,1,0)");
        h.eq("Players[1].gold", 500_i64);
        h.log_has("[JAC_ARB] rejected request player=1 pair=1-0 reason=ONE_PER_TURN");
        h.advance();
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", false);
        h.eq("Game:GetProperty('JAC_ARB_TRUCE_0_1')", 12_i64);
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
        h.log_has("[JAC_ARB] decision player=1 accept=true by=ai");
        h.eq(
            "JAC_ARB.Legitimacy(1), JAC_ARB.Legitimacy(0)",
            (61_i64, 51_i64),
        );
    }

    #[test]
    fn test_every_execute_script_is_recorded_at_entry_even_when_rejected() {
        let h = Court::new();
        h.eq("Game:GetProperty('JAC_ARB_LAST_EVENT')", None::<String>);
        h.run("REQUEST(3,0,1)");
        h.eq(
            "Game:GetProperty('JAC_ARB_LAST_EVENT')",
            "request player=3 turn=1".to_string(),
        );
        h.run("RESPOND(1,1)");
        h.eq(
            "Game:GetProperty('JAC_ARB_LAST_EVENT')",
            "respond player=1 turn=1".to_string(),
        );
    }

    #[test]
    fn test_the_other_party_may_be_the_requester() {
        let h = Court::new();
        h.run("REQUEST(1,0,1)");
        h.eq(
            "Game:GetProperty('JAC_ARB_PENDING').A, Game:GetProperty('JAC_ARB_PENDING').B",
            (1_i64, 0_i64),
        );
    }

    #[test]
    fn test_ashikaga_calls_a_ceasefire_between_others_with_triple_power_until_1573() {
        let mut h = Court::new();
        h.run("REQUEST(2,0,1)");
        h.eq("Game:GetProperty('JAC_ARB_PENDING').Requester, Game:GetProperty('JAC_ARB_PENDING').A, Game:GetProperty('JAC_ARB_PENDING').B, Game:GetProperty('JAC_ARB_PENDING').Power", (2_i64, 0_i64, 1_i64, 3_i64));
        h.eq(
            "#NOTES, math.min(NOTES[1][1],NOTES[2][1]), math.max(NOTES[1][1],NOTES[2][1])",
            (2_i64, 0_i64, 1_i64),
        );
        h.eq("Players[2].gold, JAC_ARB.Legitimacy(2)", (466_i64, 71_i64));
        h.advance();
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", false);
        h.eq("Game:GetProperty('JAC_ARB_TRUCE_0_1')", 32_i64);
    }

    #[test]
    fn test_refusing_a_shogun_ruling_costs_three_times_as_much() {
        let mut h = Court::new();
        h.run("Players[1].cities=4; REQUEST(2,0,1)");
        h.advance();
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", true);
        h.eq("JAC_ARB.Legitimacy(1)", 11_i64);
        h.eq("Players[1]:GetProperty('JAC_ARB_OUTLAW_UNTIL')", 32_i64);
        h.log_has(
            "[JAC_ARB] refused player=1 pair=0-1 turn=2 legitimacy=11 outlaw_until=32 power=3",
        );
        h.eq("JAC_ARB.Legitimacy(0)", 61_i64);
    }

    #[test]
    fn test_the_shogun_can_afford_a_ruling_at_a_third_of_the_cost() {
        let h = Court::new();
        h.run("Players[2].gold=34; Players[2]:SetProperty('JAC_LEGITIMACY',30); REQUEST(2,0,1)");
        h.eq(
            "Players[2].gold, JAC_ARB.Legitimacy(2), Game:GetProperty('JAC_ARB_PENDING').Power",
            (0_i64, 28_i64, 3_i64),
        );
        h.eq("JAC_ARB.Costs(1)", (100_i64, 5_i64));
        h.eq("JAC_ARB.Costs(3)", (34_i64, 2_i64));
        h.eq("JAC_ARB.RefusalPenalty(1)", (15_i64, 10_i64));
        h.eq("JAC_ARB.RefusalPenalty(3)", (45_i64, 30_i64));
    }

    #[test]
    fn test_after_1573_ashikaga_survives_but_cannot_call_rulings_for_others() {
        let mut h = Court::new();
        h.next(120);
        h.run("REQUEST(2,0,1)");
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
        h.log_has("[JAC_ARB] rejected request player=2 pair=0-1 reason=NOT_PARTY");
        h.eq("Players[2].gold", 500_i64);
        h.run("SET_WAR(2,3,true); REQUEST(2,2,3)");
        h.eq("Game:GetProperty('JAC_ARB_PENDING').Power", 1_i64);
        h.eq("Players[2].gold", 400_i64);
        h.advance();
        h.eq("Game:GetProperty('JAC_ARB_TRUCE_2_3')", 131_i64);
    }

    #[test]
    fn test_non_party_non_shogun_and_peace_pairs_are_rejected() {
        let h = Court::new();
        h.run("REQUEST(3,0,1); REQUEST(0,0,3)");
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
        h.log_has("[JAC_ARB] rejected request player=3 pair=0-1 reason=NOT_PARTY");
        h.log_has("[JAC_ARB] rejected request player=0 pair=0-3 reason=NOT_AT_WAR");
    }

    #[test]
    fn test_short_gold_or_legitimacy_below_30_rejects_without_charging() {
        let h = Court::new();
        h.run("Players[0].gold=99; REQUEST(0,0,1)");
        h.log_has("[JAC_ARB] rejected request player=0 pair=0-1 reason=GOLD");
        h.run("Players[0].gold=500; Players[0]:SetProperty('JAC_LEGITIMACY',29); REQUEST(0,0,1)");
        h.log_has("[JAC_ARB] rejected request player=0 pair=0-1 reason=LEGITIMACY");
        h.eq("Players[0].gold, JAC_ARB.Legitimacy(0)", (500_i64, 29_i64));
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
    }

    #[test]
    fn test_legitimacy_starts_at_50_or_70_and_earns_one_to_three_per_turn() {
        let mut h = Court::new();
        h.eq("JAC_ARB.Legitimacy(0), JAC_ARB.Legitimacy(1), JAC_ARB.Legitimacy(2), JAC_ARB.Legitimacy(3)", (53_i64, 53_i64, 73_i64, 53_i64));
        h.eq("Players[0]:GetProperty('JAC_ARB_HOME')", 1_i64);
        h.run("SET_WAR(0,2,true)");
        h.eq(
            "JAC_ARB.LegitimacyIncome(0,2), JAC_ARB.LegitimacyIncome(2,2)",
            (2_i64, 3_i64),
        );
        h.run("PLOT_OWNER[1]=1");
        h.eq("JAC_ARB.LegitimacyIncome(0,2)", 1_i64);
        h.eq(
            "JAC_ARB.LegitimacyIncome(3,120), JAC_ARB.LegitimacyIncome(2,120)",
            (2_i64, 2_i64),
        );
        h.advance();
        h.eq("JAC_ARB.Legitimacy(0)", 54_i64);
        h.log_has("[JAC_ARB] legitimacy player=0 +1 = 54 turn=2");
        h.run("Players[3]:SetProperty('JAC_LEGITIMACY',99)");
        h.advance();
        h.eq("JAC_ARB.Legitimacy(3)", 100_i64);
        h.run("Players[1]:SetProperty('JAC_LEGITIMACY',1); Players[1].cities=4; REQUEST(2,0,1)");
        h.advance();
        h.eq("JAC_ARB.Legitimacy(1)", 0_i64);
    }

    #[test]
    fn test_high_and_low_legitimacy_flag_every_city() {
        let mut h = Court::new();
        let key_hi = "JAC_ARB_LEGITIMACY_HIGH";
        let key_lo = "JAC_ARB_LEGITIMACY_LOW";
        for (xs, key, expected) in [
            ([201, 202], key_hi, (1_i64, 1_i64)),
            ([201, 202], key_lo, (0, 0)),
            ([1, 2], key_hi, (0, 0)),
            ([1, 2], key_lo, (0, 0)),
        ] {
            h.eq(
                &format!("FLAG({},'{key}'), FLAG({},'{key}')", xs[0], xs[1]),
                expected,
            );
        }
        h.run("Players[0]:SetProperty('JAC_LEGITIMACY',20)");
        h.advance();
        h.eq(
            "FLAG(1,'JAC_ARB_LEGITIMACY_LOW'), FLAG(2,'JAC_ARB_LEGITIMACY_LOW')",
            (1_i64, 1_i64),
        );
        h.run("Players[0]:SetProperty('JAC_LEGITIMACY',27)");
        h.advance();
        h.eq(
            "FLAG(1,'JAC_ARB_LEGITIMACY_LOW'), FLAG(2,'JAC_ARB_LEGITIMACY_LOW')",
            (0_i64, 0_i64),
        );
    }

    #[test]
    fn test_an_ai_below_30_legitimacy_refuses_when_merely_ahead() {
        let mut h = Court::new();
        h.run("Players[1].cities=3; Players[1]:SetProperty('JAC_LEGITIMACY',20); REQUEST(0,0,1)");
        h.advance();
        h.log_has("[JAC_ARB] decision player=1 accept=false by=ai");
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", true);
    }

    #[test]
    fn test_a_clearly_winning_ai_refuses_and_is_penalized() {
        let mut h = Court::new();
        h.run("Players[1].cities=4; REQUEST(0,0,1)");
        h.advance();
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", true);
        h.eq(
            "JAC_ARB.Legitimacy(1), Players[1]:GetProperty('JAC_ARB_OUTLAW_UNTIL')",
            (41_i64, 12_i64),
        );
        h.eq(
            "Players[0]:GetProperty('JAC_ARB_OUTLAW_UNTIL')",
            None::<i64>,
        );
        h.eq("Game:GetProperty('JAC_ARB_TRUCE_0_1')", None::<i64>);
        h.run("Players[1].cities=3; REQUEST(0,0,1)");
        h.advance();
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", false);
    }

    #[test]
    fn test_human_respondent_answers_or_is_counted_as_refusing() {
        let mut h = Court::new();
        h.run("Players[1].human=true; REQUEST(0,0,1)");
        h.advance();
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", true);
        h.log_has("[JAC_ARB] decision player=1 accept=false by=no_answer");
        h.run("REQUEST(0,0,1); RESPOND(3,1); RESPOND(0,1); RESPOND(1,1); RESPOND(1,0)");
        assert_eq!(
            h.logs()
                .iter()
                .filter(|line| *line == "[JAC_ARB] ignored answer player=1")
                .count(),
            1
        );
        h.advance();
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", false);
        h.log_has("[JAC_ARB] decision player=1 accept=true by=answer");
    }

    #[test]
    fn test_war_during_the_truce_is_undone_and_the_truce_ends_on_time() {
        let mut h = Court::new();
        h.run("REQUEST(0,0,1)");
        h.next(2);
        h.next(3);
        h.run("SET_WAR(0,1,true)");
        h.advance();
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", false);
        h.log_has("[JAC_ARB] truce reimposed pair=0-1 turn=4 until=12 ok=true");
        h.next(13);
        h.eq("Game:GetProperty('JAC_ARB_TRUCE_0_1')", None::<i64>);
        h.run("SET_WAR(0,1,true)");
        h.advance();
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", true);
    }

    #[test]
    fn test_forced_peace_ends_a_war_inside_the_minimum_war_length_at_once() {
        let mut h = Court::new();
        h.run("MIN_WAR=10; SET_WAR(0,1,true); REQUEST(2,0,1)");
        h.advance();
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", false);
        h.eq(
            "Game:GetProperty('JAC_ARB_TRUCE_0_1'), Game:GetProperty('JAC_ARB_PEACE_TURN_0_1')",
            (32_i64, 2_i64),
        );
        h.eq("Game:GetProperty('JAC_ARB_AWAIT_0_1')", None::<i64>);
        h.log_has("[JAC_ARB] accepted pair=0-1 turn=2 truce_turns=30 power=3 peace=true");
        h.next(12);
        h.run("SET_WAR(0,1,true)");
        h.advance();
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", false);
        h.eq(
            "Game:GetProperty('JAC_ARB_PEACE_TURN_0_1'), Game:GetProperty('JAC_ARB_TRUCE_0_1')",
            (13_i64, 32_i64),
        );
        h.log_has("[JAC_ARB] truce reimposed pair=0-1 turn=13 until=32 ok=true");
        h.log_lacks("SAFETY NET");
    }

    #[test]
    fn test_a_war_on_the_turn_of_the_scripts_peace_is_undone_one_turn_later() {
        let mut h = Court::new();
        h.run("MIN_WAR=10; SET_WAR(0,1,true); REQUEST(2,0,1)");
        h.advance();
        h.run("SET_WAR(0,1,true)");
        h.advance();
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", true);
        h.eq(
            "Game:GetProperty('JAC_ARB_PEACE_TURN_0_1'), Game:GetProperty('JAC_ARB_TRUCE_0_1')",
            (2_i64, 32_i64),
        );
        h.log_has("[JAC_ARB] truce breach deferred pair=0-1 turn=3 peace_turn=2");
        h.advance();
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", false);
        h.eq(
            "Game:GetProperty('JAC_ARB_PEACE_TURN_0_1'), Game:GetProperty('JAC_ARB_TRUCE_0_1')",
            (4_i64, 32_i64),
        );
        h.log_has("[JAC_ARB] truce reimposed pair=0-1 turn=4 until=32 ok=true");
        h.log_lacks("SAFETY NET");
    }

    #[test]
    fn test_safety_net_defers_peace_when_the_force_flag_does_nothing() {
        let mut h = Court::new();
        h.run("MIN_WAR=10; FORCE_IGNORED=true; SET_WAR(0,1,true); REQUEST(2,0,1)");
        h.advance();
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", true);
        h.eq("Game:GetProperty('JAC_ARB_TRUCE_0_1')", None::<i64>);
        h.eq("Game:GetProperty('JAC_ARB_AWAIT_0_1')", 30_i64);
        h.log_has("[JAC_ARB] SAFETY NET accepted pair=0-1 turn=2 truce_turns=30 power=3 peace=deferred reason=min_war");
        h.eq("(function() for _,n in pairs(NOTES) do if n[3]=='LOC_JAC_ARB_NOTIFY_ACCEPTED_DEFERRED_SUMMARY' then return true end end return false end)()", true);
        h.eq(
            "select(2,JAC_ARB.CheckRequest(0,0,1))",
            "AWAITING_PEACE".to_string(),
        );
        h.next(10);
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", true);
        h.log_has("[JAC_ARB] SAFETY NET peace deferred pair=0-1 turn=10 reason=min_war");
        h.next(11);
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", false);
        h.eq("Game:GetProperty('JAC_ARB_AWAIT_0_1')", None::<i64>);
        h.eq(
            "Game:GetProperty('JAC_ARB_TRUCE_0_1'), Game:GetProperty('JAC_ARB_PEACE_TURN_0_1')",
            (41_i64, 11_i64),
        );
        h.log_has("[JAC_ARB] truce started pair=0-1 turn=11 until=41");
    }

    #[test]
    fn test_a_pair_that_made_peace_on_its_own_while_waiting_starts_the_truce() {
        let mut h = Court::new();
        h.run("MIN_WAR=10; FORCE_IGNORED=true; SET_WAR(0,1,true); REQUEST(0,0,1)");
        h.advance();
        h.run("SET_WAR(0,1,false)");
        h.advance();
        h.eq("Game:GetProperty('JAC_ARB_TRUCE_0_1')", 13_i64);
        h.eq("Game:GetProperty('JAC_ARB_PEACE_TURN_0_1')", None::<i64>);
    }

    #[test]
    fn test_safety_net_undoes_a_truce_breach_when_allowed_even_after_the_truce() {
        let mut h = Court::new();
        h.run("MIN_WAR=10; FORCE_IGNORED=true; Game:SetProperty('JAC_ARB_TRUCE_2_3',5); SET_WAR(2,3,true)");
        h.advance();
        h.log_has(
            "[JAC_ARB] SAFETY NET truce reimposed pair=2-3 turn=2 until=5 ok=false reason=min_war",
        );
        h.next(10);
        h.truth("Players[2]:GetDiplomacy():IsAtWarWith(3)", true);
        h.eq("Game:GetProperty('JAC_ARB_TRUCE_2_3')", 5_i64);
        h.next(11);
        h.truth("Players[2]:GetDiplomacy():IsAtWarWith(3)", false);
        h.eq("Game:GetProperty('JAC_ARB_TRUCE_2_3')", None::<i64>);
        h.log_has("[JAC_ARB] truce ended pair=2-3 turn=11");
    }

    #[test]
    fn test_make_peace_probe_tries_every_variant_and_never_stops() {
        let h = Court::new();
        let scenario: Json = serde_json::from_str(&read_repo(
            "tools/civ6-autotest/scenarios/probe-make-peace.json",
        ))
        .unwrap();
        let steps = scenario["steps"].as_array().unwrap();
        let turns = steps
            .iter()
            .filter(|s| s["action"] == "autoplay")
            .map(|s| s["turns"].as_i64().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(turns, [1, 9, 1]);
        let probes = steps
            .iter()
            .filter(|s| s["action"] == "ui_lua")
            .collect::<Vec<_>>();
        assert_eq!(probes.len(), 4);
        assert!(probes.iter().all(|s| s["state_name"] == "GameCore_Tuner"));
        let first = probes[0]["code"].as_str().unwrap();
        assert!(first.contains("DIPLOMACY_WAR_MIN_TURNS"));
        assert!(first.contains("GetMinPeaceDuration()"));
        for variant in [
            "Try('F1','a',false)",
            "Try('F2','a',true)",
            "Try('F3','b',false)",
            "Try('R01','b',true)",
        ] {
            assert!(first.contains(variant), "{variant}");
        }
        for probe in probes {
            let code = probe["code"].as_str().unwrap();
            assert!(!code.contains("error("));
            let report: String = h.lua.load(code).strict_eval().unwrap();
            assert!(report.starts_with("turn="), "{report}");
        }
    }

    #[test]
    fn test_a_request_is_void_if_the_war_ended_before_the_ruling() {
        let mut h = Court::new();
        h.run("REQUEST(0,0,1); SET_WAR(0,1,false)");
        h.advance();
        h.log_has("[JAC_ARB] void request pair=0-1 turn=2 (no longer at war)");
        h.eq("Game:GetProperty('JAC_ARB_TRUCE_0_1')", None::<i64>);
    }
}

mod subjugation_order_tests {
    use super::*;

    #[test]
    fn test_the_refuser_is_the_target_and_an_ai_at_war_with_it_joins_at_once() {
        let mut h = Court::new();
        h.refuse_on_turn_2();
        h.eq("Players[1]:GetProperty('JAC_ARB_OUTLAW_UNTIL')", 12_i64);
        h.log_has("[JAC_ARB] joined player=0 target=1 turn=2 until=12 by=ai");
        h.truth("JAC_ARB.HasJoined(0,1,2)", true);
        h.truth("JAC_ARB.HasJoined(3,1,2)", false);
        h.truth("JAC_ARB.HasJoined(2,1,2)", false);
        h.eq("(function() local n=0; for _,v in pairs(NOTES) do if v[2]=='LOC_JAC_ARB_NOTIFY_REFUSED_NAME' then n=n+1 end end return n end)()", 4_i64);
        h.run("NEAR['1_3']=true");
        h.advance();
        h.truth("JAC_ARB.HasJoined(3,1,3)", true);
        h.log_has("[JAC_ARB] joined player=3 target=1 turn=3 until=12 by=ai");
    }

    #[test]
    fn test_a_human_joins_through_execute_script_once() {
        let mut h = Court::new();
        h.refuse_on_turn_2();
        h.run("JOIN(2,1)");
        h.truth("JAC_ARB.HasJoined(2,1,2)", true);
        h.eq(
            "Game:GetProperty('JAC_ARB_LAST_EVENT')",
            "join player=2 turn=2".to_string(),
        );
        h.run("JOIN(2,1); JOIN(1,1); JOIN(2,3)");
        h.log_has("[JAC_ARB] rejected join player=2 target=1 reason=ALREADY");
        h.log_has("[JAC_ARB] rejected join player=1 target=1 reason=INVALID");
        h.log_has("[JAC_ARB] rejected join player=2 target=3 reason=NO_ORDER");
    }

    #[test]
    fn test_war_weariness_flag_is_on_the_participants_anchor_city_only_during_the_order() {
        let mut h = Court::new();
        h.refuse_on_turn_2();
        h.eq(
            "FLAG(1,'JAC_ARB_SUBJUGATION_ANCHOR'), FLAG(2,'JAC_ARB_SUBJUGATION_ANCHOR')",
            (1_i64, 0_i64),
        );
        h.eq(
            "FLAG(201,'JAC_ARB_SUBJUGATION_ANCHOR'), FLAG(202,'JAC_ARB_SUBJUGATION_ANCHOR')",
            (0_i64, 0_i64),
        );
        h.run("JOIN(2,1)");
        h.eq(
            "FLAG(201,'JAC_ARB_SUBJUGATION_ANCHOR'), FLAG(202,'JAC_ARB_SUBJUGATION_ANCHOR')",
            (1_i64, 0_i64),
        );
        h.eq("FLAG(101,'JAC_ARB_SUBJUGATION_ANCHOR'), FLAG(102,'JAC_ARB_SUBJUGATION_ANCHOR'), FLAG(103,'JAC_ARB_SUBJUGATION_ANCHOR'), FLAG(104,'JAC_ARB_SUBJUGATION_ANCHOR')", (0_i64, 0_i64, 0_i64, 0_i64));
        h.next(12);
        h.eq("FLAG(1,'JAC_ARB_SUBJUGATION_ANCHOR')", 1_i64);
        h.next(13);
        h.eq(
            "FLAG(1,'JAC_ARB_SUBJUGATION_ANCHOR'), FLAG(201,'JAC_ARB_SUBJUGATION_ANCHOR')",
            (0_i64, 0_i64),
        );
        h.eq("Players[1]:GetProperty('JAC_ARB_OUTLAW_UNTIL'), Players[0]:GetProperty('JAC_ARB_JOINED_1'), Players[2]:GetProperty('JAC_ARB_JOINED_1')", (None::<i64>, None::<i64>, None::<i64>));
        h.log_has("[JAC_ARB] subjugation ended target=1 turn=13");
        h.eq("(function() local a,n={},0; for _,v in pairs(NOTES) do if v[2]=='LOC_JAC_ARB_NOTIFY_ORDER_ENDED_NAME' then a[v[1]]=true; n=n+1 end end return n==3 and a[0]==true and a[1]==true and a[2]==true and a[3]==nil end)()", true);
        h.run("JOIN(2,1)");
        h.log_has("[JAC_ARB] rejected join player=2 target=1 reason=NO_ORDER");
    }

    #[test]
    fn test_a_participant_that_takes_a_target_city_is_rewarded() {
        let mut h = Court::new();
        h.refuse_on_turn_2();
        let (ouchi_before, otomo_before): (i64, i64) =
            h.strict_eval("JAC_ARB.Legitimacy(0), JAC_ARB.Legitimacy(1)");
        h.run("CONQUER(0,1,104)");
        h.eq(
            "Players[0].gold, JAC_ARB.Legitimacy(0), JAC_ARB.Legitimacy(1)",
            (600_i64, ouchi_before + 10, otomo_before - 10),
        );
        h.log_has(&format!("[JAC_ARB] subjugation capture player=0 target=1 city=7 turn=2 gold=+200 legitimacy={} target_legitimacy={}", ouchi_before + 10, otomo_before - 10));
        h.eq("(function() local n=0; for _,v in pairs(NOTES) do if v[2]=='LOC_JAC_ARB_NOTIFY_CAPTURE_NAME' then n=n+1 end end return n end)()", 4_i64);
        let mori_gold: i64 = h.strict_eval("Players[3].gold");
        h.run("CONQUER(3,1,103)");
        h.eq(
            "Players[3].gold, JAC_ARB.Legitimacy(1)",
            (mori_gold, otomo_before - 10),
        );
        h.next(13);
        let gold: i64 = h.strict_eval("Players[0].gold");
        h.run("CONQUER(0,1,102)");
        h.eq("Players[0].gold", gold);
    }

    #[test]
    fn test_refusing_a_shogun_ruling_orders_30_turns_and_a_new_refusal_never_shortens_it() {
        let mut h = Court::new();
        h.run("Players[1].cities=4; REQUEST(2,0,1)");
        h.advance();
        h.eq("Players[1]:GetProperty('JAC_ARB_OUTLAW_UNTIL')", 32_i64);
        h.run("Players[1]:SetProperty('JAC_LEGITIMACY',50); REQUEST(0,0,1)");
        h.advance();
        h.eq("Players[1]:GetProperty('JAC_ARB_OUTLAW_UNTIL')", 32_i64);
    }
}

mod arbitration_ai_tests {
    use super::*;

    #[test]
    fn test_weariness_coverage_is_logged_without_a_request() {
        let mut h = Court::ai();
        h.run("Players[0].weariness=nil; Players[3].human=true");
        h.advance();
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
        h.log_has("[JAC_ARB] ai weariness player=0 turn=2 total=0 observed=0 cities=2");
        h.log_has("[JAC_ARB] ai weariness player=1 turn=2 total=0 observed=2 cities=2");
        h.log_has("[JAC_ARB] ai weariness roster turn=2 cities=0:2,1:2,2:2");
        h.log_lacks("[JAC_ARB] ai weariness player=3 turn=2 ");
    }

    #[test]
    fn test_exhausted_party_requests_pays_and_waits_until_the_following_turn() {
        let mut h = Court::ai();
        h.exhaust(0);
        h.advance();
        h.eq("(function() local p=Game:GetProperty('JAC_ARB_PENDING'); return p.Requester,p.A,p.B,p.Turn,p.Power,p.Source end)()", (0_i64, 0_i64, 1_i64, 2_i64, 1_i64, "ai_party".to_string()));
        h.eq("Players[0].gold, JAC_ARB.Legitimacy(0)", (400_i64, 51_i64));
        h.eq("(function() local c=Players[0]:GetProperty('JAC_ARB_LAST_COST'); return c.GoldBefore,c.GoldAfter,c.LegitimacyBefore,c.LegitimacyAfter end)()", (500_i64, 400_i64, 56_i64, 51_i64));
        h.eq("Game:GetProperty('JAC_ARB_AI_PAIR_REQUEST_0_1'), Players[0]:GetProperty('JAC_ARB_AI_LAST_REQUEST')", (2_i64, 2_i64));
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", true);
        h.eq("Game:GetProperty('JAC_ARB_LAST_EVENT')", None::<String>);
        h.log_has("[JAC_ARB] ai request player=0 pair=0-1 turn=2 by=ai_party score=6 duration=1 weariness=0 weariness_cities=1 army_loss=3 city_loss=1 low_legitimacy=0");
        h.advance();
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", true);
        h.eq(
            "JAC_ARB.Legitimacy(1), Players[1]:GetProperty('JAC_ARB_OUTLAW_UNTIL')",
            (44_i64, 13_i64),
        );
    }

    #[test]
    fn test_duration_alone_can_prompt_a_party_after_1573_with_normal_cost_and_power() {
        let mut h = Court::ai();
        h.next(120);
        h.eq("(function() local p=Game:GetProperty('JAC_ARB_PENDING'); return p.Requester,p.Source,p.Power end)()", (0_i64, "ai_party".to_string(), 1_i64));
        h.next(121);
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", false);
        h.eq("Game:GetProperty('JAC_ARB_TRUCE_0_1')", 131_i64);
    }

    #[test]
    fn test_weariness_and_low_legitimacy_are_independent_score_components() {
        let mut h = Court::ai();
        h.run("Players[0].weariness=2; Players[0].units=3; Players[0]:SetProperty('JAC_LEGITIMACY',35)");
        h.advance();
        h.eq(
            "Game:GetProperty('JAC_ARB_PENDING').Requester, JAC_ARB.Legitimacy(0)",
            (0_i64, 33_i64),
        );
        h.log_contains("score=6 duration=1 weariness=4 weariness_cities=2 army_loss=1 city_loss=0 low_legitimacy=2");
    }

    #[test]
    fn test_unavailable_weariness_api_uses_duration_and_keeps_coverage_visible() {
        let mut h = Court::ai();
        h.run("Players[0].weariness=nil; Players[2].human=true");
        h.next(24);
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
        h.next(25);
        h.eq("Game:GetProperty('JAC_ARB_PENDING').Requester", 0_i64);
        h.log_contains("duration=24 weariness=0 weariness_cities=0");
    }

    #[test]
    fn test_unexhausted_and_human_parties_do_not_request() {
        let mut h = Court::ai();
        h.next(2);
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
        h.run("Players[0].human=true");
        h.exhaust(0);
        h.next(3);
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
        h.eq("Players[0].gold", 500_i64);
    }

    #[test]
    fn test_insufficient_gold_or_legitimacy_does_not_charge_or_start_an_interval() {
        let mut h = Court::ai();
        h.exhaust(0);
        h.run("Players[0].gold=99");
        h.next(2);
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
        h.eq("Players[0].gold", 99_i64);
        h.eq(
            "Players[0]:GetProperty('JAC_ARB_AI_LAST_REQUEST')",
            None::<i64>,
        );
        h.run("Players[0].gold=500; Players[0]:SetProperty('JAC_LEGITIMACY',20)");
        h.next(3);
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
        h.eq("Players[0].gold", 500_i64);
        h.log_contains("gold=1 legitimacy=0 interval=0");
        h.log_contains("gold=0 legitimacy=1 interval=0");
    }

    #[test]
    fn test_more_exhausted_unaffordable_candidate_does_not_starve_another_party() {
        let mut h = Court::ai();
        h.exhaust(0);
        h.run("Players[0].gold=0; Players[1].weariness=3; Players[1].units=1");
        h.next(2);
        h.eq("Game:GetProperty('JAC_ARB_PENDING').Requester", 1_i64);
        h.eq("Players[0].gold, Players[1].gold", (0_i64, 400_i64));
    }

    #[test]
    fn test_shogun_proposes_for_an_ally_and_both_parties_answer() {
        let mut h = Court::ai();
        h.run("ALLIED['0_2']=true");
        h.next(2);
        h.eq("(function() local p=Game:GetProperty('JAC_ARB_PENDING'); return p.Requester,p.A,p.B,p.Power,p.Source end)()", (2_i64, 0_i64, 1_i64, 3_i64, "ai_shogun".to_string()));
        h.eq("Players[2].gold, JAC_ARB.Legitimacy(2)", (466_i64, 74_i64));
        h.next(3);
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", false);
        h.eq("Game:GetProperty('JAC_ARB_TRUCE_0_1')", 33_i64);
        h.eq(
            "JAC_ARB.Legitimacy(0), JAC_ARB.Legitimacy(1)",
            (64_i64, 64_i64),
        );
    }

    #[test]
    fn test_shogun_proposes_for_the_capital_region_and_for_long_wars() {
        let mut h = Court::ai();
        h.run("NEAR['0_2']=true");
        h.next(2);
        h.eq(
            "Game:GetProperty('JAC_ARB_PENDING').Source",
            "ai_shogun".to_string(),
        );
        h.log_contains("capital=true");
        let mut h = Court::ai();
        h.next(12);
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
        h.next(13);
        h.eq(
            "Game:GetProperty('JAC_ARB_PENDING').Source",
            "ai_shogun".to_string(),
        );
        h.log_contains("long_war=true");
    }

    #[test]
    fn test_shogun_stops_at_the_exact_calendar_cutoff_but_may_request_its_own_war() {
        let mut h = Court::ai();
        h.run("JAC_ARB.AI_EXHAUSTION_THRESHOLD=9999; ALLIED['0_2']=true");
        h.next(119);
        h.eq(
            "Game:GetProperty('JAC_ARB_PENDING').Source",
            "ai_shogun".to_string(),
        );
        h.next(120);
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", false);
        h.eq("Game:GetProperty('JAC_ARB_TRUCE_0_1')", 150_i64);
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
        h.run("SET_WAR(0,3,true)");
        h.next(131);
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
        h.run("SET_WAR(2,3,true)");
        h.next(132);
        h.run("JAC_ARB.AI_EXHAUSTION_THRESHOLD=6");
        h.exhaust(2);
        h.next(133);
        h.eq("(function() local p=Game:GetProperty('JAC_ARB_PENDING'); return p.Requester,p.Power,p.Source end)()", (2_i64, 1_i64, "ai_party".to_string()));
        h.eq("Players[2].gold", 366_i64);
    }

    #[test]
    fn test_human_shogun_never_automatically_proposes() {
        let mut h = Court::ai();
        h.run("Players[2].human=true; ALLIED['0_2']=true; NEAR['0_2']=true");
        h.next(13);
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
    }

    #[test]
    fn test_party_and_pair_intervals_survive_lua_reload_and_prevent_alternating_requesters() {
        let mut h = Court::ai();
        h.exhaust(0);
        h.next(2);
        h.next(3);
        h.run("Players[1].units=1; Players[1].weariness=3");
        h.run(&mod_source("Scripts/JAC_CourtArbitration_AI.lua"));
        h.next(11);
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
        h.eq("Players[0].gold, Players[1].gold", (400_i64, 500_i64));
        h.next(12);
        h.eq("Game:GetProperty('JAC_ARB_PENDING') ~= nil", true);
        h.eq("Game:GetProperty('JAC_ARB_AI_PAIR_REQUEST_0_1')", 12_i64);
    }

    #[test]
    fn test_requester_interval_applies_across_different_wars() {
        let mut h = Court::ai();
        h.exhaust(0);
        h.next(2);
        h.next(3);
        h.run("SET_WAR(0,3,true)");
        h.next(4);
        h.run("Players[0].weariness=3; Players[0]:SetProperty('JAC_LEGITIMACY',35)");
        h.next(11);
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
        h.eq("Players[0].gold", 400_i64);
    }

    #[test]
    fn test_shogun_interval_applies_to_other_pairs() {
        let mut h = Court::ai();
        h.run("ALLIED['0_2']=true; ALLIED['2_3']=true");
        h.next(2);
        h.next(3);
        h.run("SET_WAR(1,3,true)");
        h.next(11);
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
        h.next(12);
        h.eq("Game:GetProperty('JAC_ARB_PENDING').Requester, Game:GetProperty('JAC_ARB_PENDING').Source", (2_i64, "ai_shogun".to_string()));
    }

    #[test]
    fn test_existing_pending_and_one_per_turn_limit_take_priority_over_ai() {
        let mut h = Court::ai();
        h.run("REQUEST(2,0,1)");
        h.exhaust(0);
        h.run("START_TURN(1)");
        h.eq(
            "Game:GetProperty('JAC_ARB_PENDING').Requester, Players[0].gold",
            (2_i64, 500_i64),
        );
        h.next(2);
        h.truth("Players[0]:GetDiplomacy():IsAtWarWith(1)", true);
        h.eq("(function() local p=Game:GetProperty('JAC_ARB_PENDING'); return p.Requester,p.Turn,p.Source end)()", (0_i64, 2_i64, "ai_party".to_string()));
        h.run("START_TURN(2)");
        h.eq("Game:GetProperty('JAC_ARB_PENDING').Turn", 2_i64);
        assert_eq!(
            h.logs()
                .iter()
                .filter(|line| line.starts_with("[JAC_ARB] request "))
                .count(),
            2
        );
    }

    #[test]
    fn test_peace_removes_observations_and_new_wars_start_without_old_losses() {
        let mut h = Court::ai();
        h.run("SET_WAR(0,1,false)");
        h.next(2);
        h.eq("Game:GetProperty('JAC_ARB_AI_WARS')['p0_1']", None::<i64>);
        h.exhaust(0);
        h.run("SET_WAR(0,1,true)");
        h.next(3);
        h.eq("(function() local w=Game:GetProperty('JAC_ARB_AI_WARS')['p0_1']; return w.Since,w.CitiesA,w.ArmyA end)()", (3_i64, 1_i64, 1_i64));
        h.eq("Game:GetProperty('JAC_ARB_PENDING')", None::<i64>);
    }
}

mod live_scenario_on_stubs_tests {
    use super::*;

    #[test]
    fn test_scenario_steps_pass_on_the_stubs() {
        let mut h = Court::scenario();
        h.run("function JAC_RequestArbitration(a,b) REQUEST(2,a,b); return true end; function JAC_JoinSubjugation(target) JOIN(2,target); return true end");
        let scenario: Json = serde_json::from_str(&read_repo(
            "tools/civ6-autotest/scenarios/court-arbitration.json",
        ))
        .unwrap();
        let mut results = Vec::new();
        for step in scenario["steps"].as_array().unwrap() {
            match step["action"].as_str().unwrap() {
                "autoplay" => h.next(h.turn + step["turns"].as_i64().unwrap()),
                "ui_lua"
                    if !["InGame", "DiplomacyActionView", "DiplomacyDealView"]
                        .contains(&step["state_name"].as_str().unwrap())
                        && !step["code"].as_str().unwrap().contains("Controls.") =>
                {
                    let code = step["code"].as_str().unwrap();
                    results.push(
                        h.lua
                            .load(code)
                            .strict_eval::<String>()
                            .unwrap_or_else(|error| panic!("{code}: {error}")),
                    );
                }
                "wait_for" if step["condition"]["state_name"] == "GameCore_Tuner" => {
                    let code = step["condition"]["code"].as_str().unwrap();
                    assert!(h
                        .lua
                        .load(code)
                        .strict_eval::<bool>()
                        .unwrap_or_else(|error| panic!("{code}: {error}")));
                }
                _ => {}
            }
        }
        assert!(results[0].contains("legitimacy ashikaga=73 (+3/turn) ouchi=53 otomo=53"));
        assert!(results[0]
            .contains("accepted-path pair CIVILIZATION_JAC_MORI 3 - CIVILIZATION_JAC_ODA 4"));
        assert!(results[2].contains("accepted ruling: 3-4 made peace on turn 2"));
        assert!(results[2].contains("legitimacy 3=61 4=61 (+5 for accepting)"));
        assert_eq!(results[4], "refused by Ouchi (legitimacy 56 -> 14: -45 with this turn income; order to subdue until 33 = 3+10x3); joined by 1; accepted pair 3-4 still at peace");
        let mut expected_yields = Vec::new();
        for (player, yield_value, count) in [(2, "11.0", 2), (0, "9.0", 4)] {
            for _ in 0..count {
                for yield_name in ["GOLD", "CULTURE"] {
                    expected_yields.push(format!("p{player} {yield_name} {yield_value}/10.0"));
                }
            }
        }
        assert_eq!(
            results[5],
            format!(
                "legitimacy yield modifiers live (flag on/off, high +10% / low -10%): {}",
                expected_yields.join(", ")
            )
        );
        assert_eq!(results[6], "join sent via EXECUTE_SCRIPT");
        assert_eq!(
            results[7],
            "forced war inside the truce between 3 and 4 on turn 13"
        );
        assert_eq!(results[8], "order to subdue Ouchi shortened to end after turn 14 (test only); truce breach 3-4 forced on turn 13 undone on turn 14, truce still until 32");
        assert_eq!(results[9], "order to subdue Ouchi ended on turn 15; war-weariness anchor flags of Ashikaga and Otomo off");
        assert_eq!(results.len(), 10);
        h.log_has("[JAC_ARB] joined player=2 target=0 turn=3 until=33 by=request");
        h.log_lacks("deferred");
        h.log_lacks("SAFETY NET");
    }

    #[test]
    fn test_human_snapshots_preserve_missing_properties_and_boolean_answers() {
        let h = Court::scenario();
        h.run(&fixture("snapshot_setup.lua"));
        let scenario: Json = serde_json::from_str(&read_repo(
            "tools/civ6-autotest/scenarios/court-arbitration-human.json",
        ))
        .unwrap();
        let snapshots = scenario["steps"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|step| step.get("capture_as").is_some() || step.get("equals_capture").is_some())
            .collect::<Vec<_>>();
        assert_eq!(snapshots.len(), 12);
        let prefix = "accept|1|65|45|500|500|1|nil|nil|nil|nil|nil|nil|true|true|";
        let cities = "0:1:nil:nil:nil,0:2:nil:nil:nil,2:201:nil:nil:nil,2:202:nil:nil:nil";
        for answer in [Some(false), Some(true), None] {
            let pending = if let Some(answer) = answer {
                h.run(&format!("Game:SetProperty(ExposedMembers.JAC_CourtArbitration.PENDING_KEY, {{Requester=0,A=0,B=2,Turn=1,Power=1,AnswerB={answer}}})"));
                format!("Requester=0|A=0|B=2|Turn=1|Power=1|AnswerA=nil|AnswerB={answer}")
            } else {
                h.run("Game:SetProperty(ExposedMembers.JAC_CourtArbitration.PENDING_KEY,nil)");
                "no pending".to_string()
            };
            for step in &snapshots {
                let code = step["code"].as_str().unwrap();
                let result = h
                    .lua
                    .load(code)
                    .strict_eval::<String>()
                    .unwrap_or_else(|error| panic!("{code}: {error}"));
                assert_eq!(
                    result,
                    format!("{prefix}{pending}|{cities}"),
                    "answer={answer:?}, capture={:?}",
                    step.get("capture_as")
                        .or_else(|| step.get("equals_capture"))
                );
            }
        }
    }

    #[test]
    fn test_overlay_buttons_can_open_and_close_from_an_initially_hidden_context() {
        for (source, toggle, panel, close) in [
            (
                format!("{MOD}UI/JAC_CourtArbitrationPanel.lua"),
                "OpenButton",
                "Panel",
                "CloseButton",
            ),
            (
                "poc/JAC_PoC_SengokuTrees/UI/JAC_UnificationPact.lua".to_string(),
                "PactToggle",
                "PactPanel",
                "PactToggle",
            ),
        ] {
            let h = Court::scenario();
            h.run(&fixture("overlay_setup.lua"));
            h.run(&read_repo(&source));
            h.identity("ContextPtr:IsHidden()", false);
            h.run(&format!("Controls.{toggle}:DoLeftClick()"));
            h.identity(&format!("Controls.{panel}:IsHidden()"), false);
            if panel == "Panel" {
                h.eq(
                    "#Controls.Rows.instances, Controls.Rows.instances[1].Root:GetSizeY()",
                    (1_i64, 76_i64),
                );
            }
            h.run(&format!("Controls.{close}:DoLeftClick()"));
            h.identity(&format!("Controls.{panel}:IsHidden()"), true);
        }
    }
}
