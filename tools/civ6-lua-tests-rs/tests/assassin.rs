use civ6_lua_tests_rs::strict::ChunkExt;
use civ6_lua_tests_rs::{new_lua, read_repo, repo_root};
use mlua::{Lua, Value as LuaValue};
use regex::Regex;
use rusqlite::Connection;
use serde_json::Value as Json;

#[path = "assassin_support/mod.rs"]
mod assassin_support;

const MOD: &str = "poc/JAC_PoC_Assassin";
const RULES: &str = "poc/JAC_PoC_Assassin/Scripts/JAC_Assassin_Rules.lua";
const GAMEPLAY: &str = "poc/JAC_PoC_Assassin/Scripts/JAC_Assassin_Gameplay.lua";
const SCENARIO: &str = "tools/civ6-autotest/scenarios/assassin-leader-save-load.json";

#[test]
fn parallel_production_assertion_rejects_extra_results_and_nil_or_string_false() {
    let h = Harness::new();
    h.run("assert(A.StartProduction(0,10))");
    assert_eq!(h.parallel_result().unwrap(), (false, "PRODUCING".into()));
    for result in [
        "false,'PRODUCING',99",
        "false,'PRODUCING',nil",
        "nil,'PRODUCING'",
        "'false','PRODUCING'",
    ] {
        h.run(&format!("A.StartProduction=function() return {result} end"));
        assert!(h.parallel_result().is_err(), "{result}");
    }
    h.run("A.StartProduction=function() return false,'PRODUCING' end");
    assert_eq!(h.parallel_result().unwrap(), (false, "PRODUCING".into()));
}

fn case_segments(name: &str) -> Vec<String> {
    let source = include_str!("../fixtures/assassin/cases.lua");
    let mut active = false;
    let mut current = String::new();
    let mut result = Vec::new();
    for line in source.lines() {
        if let Some(case) = line.strip_prefix("-- CASE ") {
            if active && !current.trim().is_empty() {
                result.push(std::mem::take(&mut current));
            }
            active = case == name;
        } else if line == "-- SEGMENT" {
            if active && !current.trim().is_empty() {
                result.push(std::mem::take(&mut current));
            }
        } else if active {
            current.push_str(line);
            current.push('\n');
        }
    }
    if active && !current.trim().is_empty() {
        result.push(current);
    }
    assert!(!result.is_empty(), "missing Lua fixture for {name}");
    result
}

struct Harness {
    lua: Lua,
}

impl Harness {
    fn new() -> Self {
        let lua = new_lua();
        lua.load(include_str!("../fixtures/assassin/stubs.lua"))
            .exec()
            .unwrap();
        let this = Self { lua };
        this.reload();
        this.run("A=JAC_ASN; U=GRANT(0,0,1,0):GetID(); U2=GRANT(0,0,1,0):GetID()");
        this
    }

    fn run(&self, source: &str) {
        self.lua
            .load(source)
            .exec()
            .unwrap_or_else(|e| panic!("Lua failed: {e}\n{source}"));
    }

    fn reload(&self) {
        self.run(&read_repo(RULES));
        self.lua
            .globals()
            .set(
                "include",
                self.lua.create_function(|_, _: LuaValue| Ok(())).unwrap(),
            )
            .unwrap();
        self.run(&read_repo(GAMEPLAY));
    }

    fn case(&self, name: &str) {
        for segment in case_segments(name) {
            self.run(&segment);
        }
    }

    fn error_contains(&self, source: &str, expected: &str) {
        let error = self
            .lua
            .load(source)
            .exec()
            .expect_err("expected Lua error");
        assert!(
            error.to_string().contains(expected),
            "expected {expected:?}, got {error}"
        );
    }
    fn parallel_result(&self) -> mlua::Result<(bool, String)> {
        self.lua
            .load("return A.StartProduction(0,11)")
            .strict_eval()
    }
}

fn scenario() -> Json {
    assassin_support::build_scenario()
}

fn checked_in_scenario() -> Json {
    serde_json::from_str(&read_repo(SCENARIO)).unwrap()
}

fn scenario_code(needle: &str) -> String {
    let scenario = scenario();
    let matches: Vec<&str> = scenario["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|step| step["code"].as_str())
        .filter(|code| code.contains(needle))
        .collect();
    assert_eq!(matches.len(), 1, "scenario code search: {needle}");
    matches[0].to_owned()
}

fn fixture_helpers() -> String {
    let scenario = scenario();
    let first = scenario["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|step| step["action"] == "ui_lua")
        .unwrap()["code"]
        .as_str()
        .unwrap();
    let start = first.find("local function FixtureReveal").unwrap();
    let end = first.find("local function Same").unwrap();
    first[start..end].to_owned()
}

fn child<'a>(node: roxmltree::Node<'a, 'a>, tag: &str) -> roxmltree::Node<'a, 'a> {
    node.children()
        .find(|child| child.is_element() && child.tag_name().name() == tag)
        .unwrap_or_else(|| panic!("missing {tag} below {}", node.tag_name().name()))
}

fn children<'a>(node: roxmltree::Node<'a, 'a>, tag: &str) -> Vec<roxmltree::Node<'a, 'a>> {
    node.children()
        .filter(|child| child.is_element() && child.tag_name().name() == tag)
        .collect()
}

macro_rules! simple_tests {
    ($($name:ident),* $(,)?) => {
        $(#[test] fn $name() { Harness::new().case(stringify!($name)); })*
    };
}

mod assassin_lua_tests {
    use super::*;

    simple_tests! {
        test_base_100_and_minimum_20_for_all_speeds,
        test_unplaced_spawn_keeps_training_pending_without_starting_nationwide_lock,
        test_every_factor_and_security_direction,
        test_actual_completion_and_exact_lock_boundary,
        test_minimum_is_elapsed_time_even_with_extreme_bonuses,
        test_cancellation_cannot_transfer_elapsed_progress,
        test_cancel_does_not_clear_existing_completion_lock,
        test_lost_city_releases_only_the_production_reservation,
        test_blocked_spawn_lock_begins_at_actual_completion,
        test_production_and_command_locks_are_independent,
        test_training_recalculates_current_factors_and_never_rerolls,
        test_training_can_extend_again_and_finish_after_elapsed_reduction,
        test_test_deadline_hook_is_inert_when_test_mode_is_off,
        test_test_deadline_hook_selects_clocks_and_never_extends_expired_locks,
        test_only_full_stack_reaches_wait_one,
        test_distances_target_periods_and_rounding,
        test_all_six_success_rates_and_corrections,
        test_builder_hero_gp_and_assassin_are_classified_by_real_unit,
        test_names_are_assigned_by_confirmed_country_not_whole_map,
        test_one_active_mission_even_after_wait_expires,
        test_command_wait_cannot_be_bypassed_by_unit_or_country,
        test_outcome_roll_is_at_start_and_not_at_resolution,
        test_percent_threshold_is_not_inclusive,
        test_forced_outcome_only_works_with_explicit_test_flag,
        test_ui_operation_cannot_supply_forced_outcome,
        test_leader_death_splits_all_cities_disbands_units_and_archives_people,
        test_post_death_snapshot_and_branch_are_frozen,
        test_unit_success_and_failure_dispositions,
        test_missing_target_does_not_redirect_or_reroll,
        test_fixed_failure_detection_penalties_survive_target_disappearance,
        test_actor_disappearance_prevents_ghost_success,
        test_eliminated_country_finishes_saved_jobs_and_fixed_penalties,
        test_replacement_unit_with_same_id_and_type_is_not_the_target,
        test_diplomacy_changes_recalculate_training_with_worst_living_major,
        test_relation_penalty_increases_hostility_and_shortens_wait,
        test_notification_exception_cannot_duplicate_production_or_penalties,
        test_visibility_checks_do_not_disclose_or_accept_hidden_targets,
        test_branch_weights_use_security_unrest_strength_relations_and_candidates,
        test_unregistered_great_person_keeps_original_unit_id_when_archived,
        test_absorption_freezes_recipient_transfers_cities_disbands_and_preserves_people,
        test_recipient_formula_combines_strength_friendship_border_distance_religion_culture,
        test_recipient_alliance_is_gameplay_and_optional_closeness_does_not_reward_missing_data,
        test_recipient_omits_pantheon_and_missing_apis_and_prefers_supplied_culture_over_proxy,
        test_recipient_distance_and_border_can_change_choice_and_ties_use_player_id,
        test_favor_vote_weight_outvotes_more_nations_and_is_frozen_at_command,
        test_lost_absorption_recipient_falls_back_without_new_draw,
        test_lost_successor_falls_back_without_selecting_next_candidate,
        test_absorption_people_have_one_identity_and_dead_leaders_never_rejoin,
        test_later_leader_death_updates_people_registry_and_cannot_revive_predecessor,
        test_registered_country_name_survives_kin_succession_and_free_city_archive,
        test_registered_governor_uses_stable_identity,
        test_earlier_pending_save_migrates_plan_without_new_rng,
        test_non_major_person_targets_are_rejected_in_core_without_excluding_units,
    }

    #[test]
    fn test_multiple_cities_reject_parallel_production() {
        let h = Harness::new();
        let first = h
            .lua
            .load("return A.StartProduction(0,10)")
            .python_truthy()
            .unwrap();
        assert!(first);
        let second = h.parallel_result().unwrap();
        assert_eq!(second, (false, "PRODUCING".into()));
        let other = h
            .lua
            .load("return A.StartProduction(1,10)")
            .python_truthy()
            .unwrap();
        assert!(other);
    }

    #[test]
    fn test_dynamic_training_round_trip_keeps_start_speed_and_current_inputs() {
        let h = Harness::new();
        let segments =
            case_segments("test_dynamic_training_round_trip_keeps_start_speed_and_current_inputs");
        assert_eq!(segments.len(), 2);
        h.run(&segments[0]);
        h.reload();
        h.run(&segments[1]);
    }

    #[test]
    fn test_training_relation_survives_reload_and_completes_at_reduced_boundary() {
        let h = Harness::new();
        let segments = case_segments(
            "test_training_relation_survives_reload_and_completes_at_reduced_boundary",
        );
        assert_eq!(segments.len(), 2);
        h.run(&segments[0]);
        h.reload();
        h.run(&segments[1]);
    }

    #[test]
    fn test_test_deadlines_survive_recomputation_reload_and_resolve_on_real_turn() {
        let h = Harness::new();
        let segments = case_segments(
            "test_test_deadlines_survive_recomputation_reload_and_resolve_on_real_turn",
        );
        assert_eq!(segments.len(), 2);
        h.run(&segments[0]);
        h.reload();
        h.run(&segments[1]);
    }

    #[test]
    fn test_eliminated_human_slot_cancels_saved_training_after_reload_without_retransferring() {
        let h = Harness::new();
        let segments = case_segments(
            "test_eliminated_human_slot_cancels_saved_training_after_reload_without_retransferring",
        );
        assert_eq!(segments.len(), 2);
        h.run(&segments[0]);
        h.reload();
        h.run(&segments[1]);
    }

    #[test]
    fn test_equivalent_favor_vote_is_available_without_test_mode_on_japan_map() {
        let h = Harness::new();
        h.case("test_equivalent_favor_vote_is_available_without_test_mode_on_japan_map");
        h.error_contains("A.StartAttempt(0,U,1,'LEADER')", "Congress absent");
    }

    #[test]
    fn test_failure_and_detection_independently_share_penalty_once() {
        let segments = case_segments("test_failure_and_detection_independently_share_penalty_once");
        assert_eq!(segments.len(), 1);
        for (success, detected) in [(false, false), (false, true), (true, true), (true, false)] {
            let h = Harness::new();
            h.run(&format!("FORCE({success},{detected},'FREE_STATE'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(5)"));
            h.run(&segments[0]);
        }
    }

    #[test]
    fn test_actor_loss_cancels_all_four_outcomes_but_always_applies_fixed_penalty_once() {
        let segments = case_segments(
            "test_actor_loss_cancels_all_four_outcomes_but_always_applies_fixed_penalty_once",
        );
        assert_eq!(segments.len(), 1);
        for (success, detected) in [(false, false), (false, true), (true, false), (true, true)] {
            let h = Harness::new();
            let code = segments[0]
                .replace("{str(success).lower()}", &success.to_string())
                .replace("{str(detected).lower()}", &detected.to_string())
                .replace("{{", "{")
                .replace("}}", "}");
            h.run(&code);
        }
    }

    #[test]
    fn test_successor_kin_first_and_single_role_for_gp_and_governor() {
        let segments =
            case_segments("test_successor_kin_first_and_single_role_for_gp_and_governor");
        assert_eq!(segments.len(), 1);
        for kind in ["GREAT_PERSON", "GOVERNOR"] {
            let h = Harness::new();
            h.run(&segments[0].replace("\"\"\"+kind+\"\"\"", kind));
            if kind == "GREAT_PERSON" {
                h.run("assert(DATA[1].Units[GP]==nil)");
            } else {
                h.run("assert(A.Input(Players[1]).Governors['7'].Retired==1); assert(A.ResolveTarget(1,'GOVERNOR',7)==nil)");
            }
        }
    }

    #[test]
    fn test_registered_death_overrides_present_gp_governor_or_person() {
        let segments =
            case_segments("test_registered_death_overrides_present_gp_governor_or_person");
        assert_eq!(segments.len(), 2);
        for kind in ["PERSON", "GREAT_PERSON", "GOVERNOR"] {
            for source in ["people", "single", "list"] {
                let h = Harness::new();
                h.run(&segments[0].replace("\"\"\"+kind+\"\"\"", kind));
                match source {
                    "people" => h.run("Players[1]:SetProperty(A.PEOPLE,{HEIR={Identity='HEIR',Dead=1}})"),
                    "single" => h.run("local i=A.Input(Players[1]); i.Successor.Dead=1; Players[1]:SetProperty(A.INPUT,i)"),
                    "list" => h.run("local i=A.Input(Players[1]); i.Successors={{Identity='HEIR',Dead=1}}; Players[1]:SetProperty(A.INPUT,i)"),
                    _ => unreachable!(),
                }
                h.run(&segments[1]);
            }
        }
    }

    #[test]
    fn test_absorption_never_recreates_dead_registered_gp_under_a_unit_token() {
        let segments =
            case_segments("test_absorption_never_recreates_dead_registered_gp_under_a_unit_token");
        assert_eq!(segments.len(), 2);
        for source in ["people", "single", "list"] {
            let h = Harness::new();
            h.run(&segments[0]);
            match source {
                "people" => h.run("Players[1]:SetProperty(A.PEOPLE,{DEAD_GP={Identity='DEAD_GP',Dead=1}})"),
                "single" => h.run("local i=A.Input(Players[1]); i.Successor.Dead=1; Players[1]:SetProperty(A.INPUT,i)"),
                "list" => h.run("local i=A.Input(Players[1]); i.Successors={{Identity='DEAD_GP',Dead=1}}; Players[1]:SetProperty(A.INPUT,i)"),
                _ => unreachable!(),
            }
            h.run(&segments[1]);
        }
    }

    #[test]
    fn test_persistent_person_binding_does_not_claim_same_id_replacement_gp() {
        let h = Harness::new();
        let segments =
            case_segments("test_persistent_person_binding_does_not_claim_same_id_replacement_gp");
        assert_eq!(segments.len(), 2);
        h.run(&segments[0]);
        h.reload();
        h.run(&segments[1]);
    }

    #[test]
    fn test_congress_ballots_freeze_favor_weight_and_tie_goes_to_free_cities() {
        let segments =
            case_segments("test_congress_ballots_freeze_favor_weight_and_tie_goes_to_free_cities");
        assert_eq!(segments.len(), 1);
        for (votes, outcome) in [
            (
                "{['0']='SUCCESSION',['1']='SUCCESSION',['999']='ABSORPTION'}",
                "SUCCESSION",
            ),
            ("{['0']='FREE_STATE',['1']='SUCCESSION'}", "FREE_STATE"),
            ("{['0']='ABSORPTION',['1']='ABSORPTION'}", "ABSORPTION"),
            ("{}", "FREE_STATE"),
        ] {
            let h = Harness::new();
            h.run(
                &segments[0]
                    .replace("\"\"\"+votes+\"\"\"", votes)
                    .replace("\"\"\"+outcome+\"\"\"", outcome),
            );
        }
    }

    #[test]
    fn test_round_trip_reload_reinitializes_script_without_reroll() {
        let h = Harness::new();
        let segments = case_segments("test_round_trip_reload_reinitializes_script_without_reroll");
        assert_eq!(segments.len(), 3);
        h.run(&segments[0]);
        h.reload();
        h.run(&segments[1]);
        h.reload();
        h.run(&segments[2]);
    }

    #[test]
    fn test_all_post_death_outcomes_round_trip_without_reapplying_assets() {
        let segments =
            case_segments("test_all_post_death_outcomes_round_trip_without_reapplying_assets");
        assert_eq!(segments.len(), 2);
        for branch in ["FREE_STATE", "ABSORPTION", "SUCCESSION", "CONGRESS"] {
            let h = Harness::new();
            h.run(&segments[0].replace("\"\"\"+branch+\"\"\"", branch));
            h.reload();
            h.run(&segments[1]);
            h.reload();
            h.run("A=JAC_ASN; A.Tick(); ADVANCE(1); assert(#TRANSFERS==COUNT and RAND_CALLS==3)");
        }
    }

    #[test]
    fn test_live_fixture_reveals_hidden_target_without_disabling_gameplay_visibility() {
        let h = Harness::new();
        h.case("test_live_fixture_reveals_hidden_target_without_disabling_gameplay_visibility");
        h.run(&format!(
            "local Map={{}}; local PlayersVisibility={{}}; local R=A; {}\n{}",
            fixture_helpers(),
            r#"
          assert(not pcall(function() FixtureReveal(0,2,0) end) and REVEALS==0)
          local ok,why=R.CheckAttempt(0,U,1,'LEADER'); assert(not ok and why=='TARGET_UNKNOWN')
          Game:SetProperty(R.TEST,1); FixtureReveal(0,2,0)
          assert(REVEALS==1 and R.KnownTarget(0,R.ResolveTarget(1,'LEADER')))
          assert(R.PROVISIONAL.Visibility=='REVEALED_CITY_VISIBLE_UNIT')
          assert(R.StartAttempt(0,U,1,'LEADER'))
        "#
        ));
    }

    #[test]
    fn test_live_fixture_reveals_unit_tile_separately_from_known_capital() {
        let h = Harness::new();
        h.case("test_live_fixture_reveals_unit_tile_separately_from_known_capital");
        h.run(&format!("local Map={{}}; local PlayersVisibility={{}}; local R=A; {}\n{}", fixture_helpers(), r#"
          assert(R.KnownTarget(0,R.ResolveTarget(1,'LEADER')))
          local ok,why=R.CheckAttempt(0,U,1,'UNIT',VICTIM)
          assert(not ok and why=='TARGET_UNKNOWN' and R.State(0).Mission==nil)
          assert(not pcall(function() FixtureReveal(0,3,1) end) and REVEALS==0)
          Game:SetProperty(R.TEST,1); FixtureReveal(0,2,0)
          assert(REVEALS==0 and not R.KnownTarget(0,R.ResolveTarget(1,'UNIT',VICTIM)))
          ok,why=R.CheckAttempt(0,U,1,'UNIT',VICTIM)
          assert(not ok and why=='TARGET_UNKNOWN' and R.State(0).Mission==nil)
          FixtureReveal(0,3,1); FixtureReveal(0,3,1)
          assert(REVEALS==1 and R.PROVISIONAL.Visibility=='REVEALED_CITY_VISIBLE_UNIT')
          assert(R.StartAttempt(0,U,1,'UNIT',VICTIM))
          local target=R.State(0).Mission.Target
          assert(target.Kind=='UNIT' and target.Owner==1 and target.ID==VICTIM and target.X==3 and target.Y==1)
        "#));
    }

    #[test]
    fn test_live_fixture_retries_unplaced_unit_without_killing_existing_units() {
        let h = Harness::new();
        h.case("test_live_fixture_retries_unplaced_unit_without_killing_existing_units");
        h.run(&format!(
            "local R=A; {}\n{}",
            fixture_helpers(),
            r#"
          local u=Grant(0,R.UNIT,Players[1]:GetCities():GetCapitalCity())
          assert(GRANT_CALLS==2 and u:GetX()==2 and u:GetY()==0)
          assert(Players[0]:GetUnits():FindID(BAD_GRANT)==nil and #KILLS==1)
          assert(Players[0]:GetUnits():FindID(U) and Players[0]:GetUnits():FindID(U2))
        "#
        ));
    }

    #[test]
    fn test_live_trained_unit_check_rejects_unplaced_missing_or_wrong_type_unit() {
        let h = Harness::new();
        h.run("BAD_TRAINED=GRANT(0,0,-9999,-9999):GetID(); WRONG_TRAINED=GRANT(0,1,2,0):GetID()");
        h.run(&format!(
            "local R=A; {}\n{}",
            fixture_helpers(),
            r#"
          assert(TrainedUnit(0,U):GetID()==U)
          assert(not pcall(function() TrainedUnit(0,BAD_TRAINED) end))
          assert(not pcall(function() TrainedUnit(0,WRONG_TRAINED) end))
          assert(not pcall(function() TrainedUnit(0,-1) end))
        "#
        ));
    }

    #[test]
    fn test_live_training_check_requires_one_new_unit_and_rejects_reused_id() {
        let h = Harness::new();
        h.run(&format!(
            "local R=A; {}\n{}",
            fixture_helpers(),
            r#"
          local before=TrainingUnits(0)
          assert(not pcall(function() NewTrainedUnit(0,before,U) end))
          local fresh=GRANT(0,0,2,0):GetID()
          assert(NewTrainedUnit(0,before,fresh+0.0):GetID()==fresh)
          assert(not pcall(function() NewTrainedUnit(0,before,U) end))
          GRANT(0,0,2,1)
          assert(not pcall(function() NewTrainedUnit(0,before,fresh) end))
        "#
        ));
    }

    #[test]
    fn test_live_parking_keeps_identity_requires_test_mode_and_preserves_other_assassins() {
        let h = Harness::new();
        h.run(&format!("local R=A; {}\n{}", fixture_helpers(), r#"
          assert(not pcall(function() ParkTrainedUnit(0,U,10) end))
          assert(Players[0]:GetUnits():FindID(U):GetX()==1)
          Game:SetProperty(A.TEST,1)
          local guard=GRANT(0,4,0,0):GetID()
          local snapshot=ParkTrainedUnit(0,U,10)
          assert(snapshot.ID==U and snapshot.X==0 and snapshot.Y==0 and snapshot.Damage==0)
          assert(Players[0]:GetUnits():FindID(guard)==nil and Players[0]:GetUnits():FindID(U2))
          local before=#KILLS
          assert(not pcall(function() ParkTrainedUnit(0,U2,10) end))
          assert(#KILLS==before and Players[0]:GetUnits():FindID(U) and Players[0]:GetUnits():FindID(U2))
          assert(not pcall(function() ParkTrainedUnit(0,U2,-1) end))
        "#));
    }

    fn start_live_trained_unit_watch(h: &Harness) -> String {
        h.run(&case_segments("helper_live_watch_setup")[0]);
        h.run(&scenario_code("JAC_ASN_LIVE_WATCH=watch"));
        scenario_code("surviving trained individual changed after native fixture")
    }

    #[test]
    fn test_live_unit_watch_accepts_witnessed_native_death_and_removes_observers() {
        let h = Harness::new();
        let check = start_live_trained_unit_watch(&h);
        let segments = case_segments(
            "test_live_unit_watch_accepts_witnessed_native_death_and_removes_observers",
        );
        assert_eq!(segments.len(), 2);
        h.run(&segments[0]);
        h.run(&check);
        h.run(&segments[1]);
    }

    #[test]
    fn test_live_unit_watch_rejects_unexplained_loss_or_replacement() {
        for change in [
            "DATA[0].Units[U]=nil",
            "UI_HANDLERS.UnitDamageChanged(1,U,100,83); UI_HANDLERS.UnitRemovedFromMap(1,U); DATA[0].Units[U]=nil",
            "UI_HANDLERS.UnitRemovedFromMap(0,U); DATA[0].Units[U]=nil",
            "UI_HANDLERS.UnitDamageChanged(0,U,100,83); DATA[0].Units[U]=nil",
            "UI_HANDLERS.UnitDamageChanged(0,U,99,83); UI_HANDLERS.UnitRemovedFromMap(0,U); DATA[0].Units[U]=nil",
            "DATA[0].Units[U].X=-9999",
            "DATA[0].Units[U].Type=1",
            "UI_HANDLERS.UnitDamageChanged(0,U,100,83); UI_HANDLERS.UnitRemovedFromMap(0,U)",
        ] {
            let h = Harness::new();
            let check = start_live_trained_unit_watch(&h);
            h.run(change);
            h.error_contains(&check, "trained");
            h.run("assert(JAC_ASN_LIVE_WATCH==nil and UI_HANDLERS.UnitDamageChanged==nil and UI_HANDLERS.UnitRemovedFromMap==nil)");
        }
    }

    #[test]
    fn test_live_unit_watch_rejects_survivor_damage_movement_and_loss() {
        for change in [
            "UI_HANDLERS.UnitDamageChanged(0,U2,25,0)",
            "UI_HANDLERS.UnitDamageChanged(0,U2,25,0); UI_HANDLERS.UnitDamageChanged(0,U2,0,25)",
            "DATA[0].Units[U2].Damage=25",
            "DATA[0].Units[U2].X=DATA[0].Units[U2].X+1",
            "DATA[0].Units[U2].Y=DATA[0].Units[U2].Y+1",
            "DATA[0].Units[U2].Type=1",
            "UI_HANDLERS.UnitDamageChanged(0,U2,100,0); UI_HANDLERS.UnitRemovedFromMap(0,U2); DATA[0].Units[U2]=nil",
        ] {
            let h = Harness::new();
            let check = start_live_trained_unit_watch(&h);
            h.run(&format!("UI_HANDLERS.UnitDamageChanged(0,U,100,0); UI_HANDLERS.UnitRemovedFromMap(0,U); DATA[0].Units[U]=nil; {change}"));
            h.error_contains(&check, "surviving trained individual changed");
            h.run("assert(JAC_ASN_LIVE_WATCH==nil and UI_HANDLERS.UnitDamageChanged==nil and UI_HANDLERS.UnitRemovedFromMap==nil)");
        }
    }

    fn load_panel(h: &Harness) {
        h.run(&case_segments("helper_panel_setup")[0]);
        h.run(&read_repo("poc/JAC_PoC_Assassin/UI/JAC_AssassinPanel.lua"));
    }

    #[test]
    fn test_panel_open_close_produce_cancel_refresh_and_inactive_buttons() {
        let h = Harness::new();
        load_panel(&h);
        h.case("test_panel_open_close_produce_cancel_refresh_and_inactive_buttons");
    }

    #[test]
    fn test_panel_lists_visible_non_major_units_without_leader_candidates() {
        let h = Harness::new();
        h.run("DATA[3].Alive=true; V=GRANT(3,4,2,0):GetID(); rebuild()");
        load_panel(&h);
        h.case("test_panel_lists_visible_non_major_units_without_leader_candidates");
    }

    #[test]
    fn test_full_generated_live_sequence_resolves_in_18_real_ticks_and_round_trips() {
        let h = Harness::new();
        h.case("test_full_generated_live_sequence_resolves_in_18_real_ticks_and_round_trips");
        let steps = scenario()["steps"].as_array().unwrap().clone();
        let mut turns = 0;
        for (index, step) in steps.iter().enumerate() {
            h.run(
                "Game.GetLocalPlayer=function() error('gameplay must not read local player') end",
            );
            if step["state_name"] == "JAC_AssassinPanel" {
                h.run("Game.GetLocalPlayer=function() return 0 end");
            }
            let action = step["action"].as_str().unwrap();
            match action {
                "ui_lua" => h.run(step["code"].as_str().unwrap()),
                "end_turn" => {
                    let n = step["turns"].as_i64().unwrap_or(1);
                    turns += n;
                    h.run(&format!("ADVANCE({n})"));
                }
                "wait_for" => {
                    if step["condition"]["state_name"] == "InGame" {
                        h.run("Game.GetLocalPlayer=function() return 0 end");
                    }
                    let code = step["condition"]["code"].as_str().unwrap();
                    let value =
                        h.lua.load(code).python_truthy().unwrap_or_else(|e| {
                            panic!("step {index} wait_for failed: {e}\n{code}")
                        });
                    assert!(value, "step {index}: wait_for was false");
                }
                "save" => {
                    h.lua
                        .globals()
                        .set("SAVE_NAME", step["name"].as_str().unwrap())
                        .unwrap();
                    h.run("LIVE_SAVES[SAVE_NAME]={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}");
                }
                "load" => {
                    h.lua
                        .globals()
                        .set("SAVE_NAME", step["name"].as_str().unwrap())
                        .unwrap();
                    h.run("local s=LIVE_SAVES[SAVE_NAME]; GAME_PROPS,DATA,NOW=clone(s.Game),clone(s.Players),s.Now; rebuild(); EVENTS={}");
                    h.reload();
                }
                "enable_mods" | "configure" | "start_game" | "exit_to_menu" => {}
                other => panic!("step {index}: unexpected action {other}"),
            }
        }
        assert_eq!(turns, 18);
        h.run("assert(NOW==1018 and RAND_CALLS==20 and FREE_CITY_CALLS==2 and Game:GetProperty('JAC_ASN_TEST_MODE')==nil)");
    }

    #[test]
    fn test_live_final_check_rejects_penalty_added_during_load() {
        let h = Harness::new();
        let checks: Vec<String> = checked_in_scenario()["steps"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|step| step["code"].as_str())
            .filter(|code| code.contains("load changed penalties"))
            .map(str::to_owned)
            .collect();
        assert_eq!(checks.len(), 1);
        let check = &checks[0];
        let segments = case_segments("test_live_final_check_rejects_penalty_added_during_load");
        assert_eq!(segments.len(), 2);
        h.run(&segments[0]);
        let prefix = check.split("local function Same").next().unwrap();
        h.run(&format!("{prefix}\n{}", "t.FinalUnits={} for _,id in ipairs({U,U2}) do t.FinalUnits[tostring(id)]=TrainedSnapshot(0,id) end; Game:SetProperty('JAC_ASN_LIVE_TEST',t)"));
        h.run(check);
        let changed_unit_error =
            Regex::new("load changed trained individual|trained unit is not placed").unwrap();
        for change in [
            "DATA[0].Units[U].X=DATA[0].Units[U].X+1",
            "DATA[0].Units[U2].Y=DATA[0].Units[U2].Y+1",
            "DATA[0].Units[U].Damage=83",
            "DATA[0].Units[U2]=nil",
        ] {
            h.run(&format!(
                "local saved=clone(DATA); BAD_LOAD_BACKUP=saved; {change}"
            ));
            let error = h
                .lua
                .load(check)
                .exec()
                .expect_err("changed unit must fail");
            assert!(
                changed_unit_error.is_match(&error.to_string()),
                "unexpected error for {change}: {error}"
            );
            h.run("DATA=BAD_LOAD_BACKUP; rebuild()");
        }
        h.run(&segments[1]);
        h.error_contains(check, "load changed penalties");
    }
}

mod assassin_packaging_tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    fn text_rows() -> HashMap<(String, String), String> {
        let source = read_repo(format!("{MOD}/Text/JAC_Assassin_Text.xml"));
        let doc = roxmltree::Document::parse(&source).unwrap();
        let rows = child(doc.root_element(), "LocalizedText");
        children(rows, "Row")
            .into_iter()
            .map(|row| {
                (
                    (
                        row.attribute("Tag").unwrap().to_owned(),
                        row.attribute("Language").unwrap().to_owned(),
                    ),
                    child(row, "Text").text().unwrap_or("").to_owned(),
                )
            })
            .collect()
    }

    #[test]
    fn test_mod_scope_files_and_order() {
        let source = read_repo(format!("{MOD}/JAC_PoC_Assassin.modinfo"));
        let doc = roxmltree::Document::parse(&source).unwrap();
        let meta = doc.root_element();
        let criteria = children(child(meta, "ActionCriteria"), "Criteria");
        assert_eq!(criteria.len(), 4);
        let map = Regex::new(r"JAC_Japan_(15|20)km(_Settlers)?\.Civ6Map$").unwrap();
        let mut ids = HashSet::new();
        for criterion in criteria {
            ids.insert(criterion.attribute("id").unwrap());
            assert_eq!(
                child(criterion, "RuleSetInUse").text(),
                Some("RULESET_JAC_BANSEIKI")
            );
            let value = child(child(criterion, "ConfigurationValueMatches"), "Value")
                .text()
                .unwrap();
            assert!(map.is_match(value), "{value}");
        }
        for action in child(meta, "InGameActions")
            .children()
            .filter(|n| n.is_element())
        {
            if action.tag_name().name() != "ImportFiles" {
                assert!(ids.contains(action.attribute("criteria").unwrap()));
            }
            if action.tag_name().name() == "UpdateDatabase" {
                let load_order: i64 = child(child(action, "Properties"), "LoadOrder")
                    .text()
                    .unwrap()
                    .parse()
                    .unwrap();
                assert!(load_order > 200);
            }
            for file in children(action, "File") {
                let path = repo_root().join(MOD).join(file.text().unwrap());
                assert!(path.is_file(), "{}", path.display());
            }
        }
    }

    #[test]
    fn test_data_loads_against_official_schema_after_sengoku() {
        let db = civ6_lua_tests_rs::official::sengoku_db();
        let fk_errors = |db: &Connection| -> HashSet<(String, i64, String, i64)> {
            let mut stmt = db.prepare("PRAGMA foreign_key_check").unwrap();
            stmt.query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<rusqlite::Result<HashSet<_>>>()
            .unwrap()
        };
        let existing = fk_errors(&db);
        db.execute_batch(&read_repo(format!("{MOD}/Data/JAC_Assassin.sql")))
            .unwrap();
        let row: (i64, Option<String>, i64, Option<String>, Option<String>) = db
            .query_row(
                "SELECT CanTrain,PurchaseYield,MustPurchase,PrereqTech,PrereqCivic FROM Units WHERE UnitType=?1",
                ["UNIT_JAC_JAPAN_SENGOKU_ASSASSIN"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .unwrap();
        assert_eq!(row, (0, None, 0, None, None));
        let upgrades: i64 = db
            .query_row(
                "SELECT count(*) FROM UnitUpgrades WHERE Unit=?1 OR UpgradeUnit=?1",
                ["UNIT_JAC_JAPAN_SENGOKU_ASSASSIN"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(upgrades, 0);
        assert_eq!(fk_errors(&db).difference(&existing).count(), 0);
    }

    #[test]
    fn test_japan_name_and_no_unapproved_cultural_names() {
        let texts = text_rows();
        for (lang, value) in [
            ("ja_JP", "暗殺忍者"),
            ("en_US", "Assassin Ninja"),
            ("ru_RU", "Ниндзя-убийца"),
        ] {
            assert_eq!(
                texts[&("LOC_JAC_ASN_NINJA_NAME".into(), lang.into())],
                value
            );
        }
        assert_eq!(
            texts[&("LOC_JAC_ASN_COMMON_NAME".into(), "ru_RU".into())],
            "Убийца"
        );
        let all = format!("{texts:?}");
        for name in ["Hashashin", "Cike", "Jagaek"] {
            assert!(!all.contains(name));
        }
    }

    #[test]
    fn test_training_terms_in_all_three_languages() {
        let texts = text_rows();
        for (lang, word) in [
            ("ja_JP", "育成"),
            ("en_US", "training"),
            ("ru_RU", "подготов"),
        ] {
            for tag in ["PRODUCE", "CANCEL", "TRAINING", "PRODUCED"] {
                let key = (format!("LOC_JAC_ASN_{tag}"), lang.to_owned());
                assert!(texts[&key].to_lowercase().contains(word), "{key:?}");
            }
        }
        let all = format!("{texts:?}");
        assert!(!Regex::new(r"募集|[Rr]ecruit|[Нн]абор|подготовкаа")
            .unwrap()
            .is_match(&all));
    }

    #[test]
    fn test_live_scenario_has_three_actual_round_trips_and_boundary_assertions() {
        let scenario = checked_in_scenario();
        let steps = scenario["steps"].as_array().unwrap();
        for action in ["save", "load"] {
            assert_eq!(steps.iter().filter(|s| s["action"] == action).count(), 3);
        }
        let turns: i64 = steps
            .iter()
            .filter(|step| step["action"] == "end_turn")
            .map(|step| step["turns"].as_i64().unwrap_or(1))
            .sum();
        assert_eq!(turns, 18);
        assert!(steps
            .iter()
            .filter(|step| step["action"] == "end_turn")
            .all(|step| step["timeout"].as_i64().unwrap() >= 600));
        let code = steps
            .iter()
            .filter_map(|step| step["code"].as_str())
            .collect::<Vec<_>>()
            .join("\n");
        for needle in [
            "IsHuman()",
            "SuccessRoll",
            "FREE_STATE",
            "ABSORPTION",
            "SUCCESSION",
            "CONGRESS",
            "PRODUCTION_LOCK",
            "MISSION_ACTIVE",
            "COMMAND_LOCK",
            "t.Lock-1",
            "s.LastCompletion+10",
            "t.Start+120",
            "t.Start+110",
            "t.Start+70",
            "t.MinStart+20",
            "ACTOR_GONE",
            "LossGrievances+25",
            "WarReason",
            "PostDeathPlan",
            "TestAdvanceDeadlines",
            "TEST_MODE_OFF",
            "GetFreeCitiesPlayerID",
            "receiver formula",
            "GetDiplomacy()",
            "ChangeFavor",
            "favor did not outweigh majority",
            "Same(s.Mission,t.Frozen)",
        ] {
            assert!(
                code.contains(needle),
                "missing scenario assertion: {needle}"
            );
        }
        assert!(!code.contains("PENDING_"));
        assert!(!Regex::new(r"local u=UnitManager.InitUnitValidAdjacentHex")
            .unwrap()
            .is_match(&code));
        let lua = new_lua();
        for step in steps.iter().filter(|step| step["action"] == "ui_lua") {
            let script = step["code"].as_str().unwrap();
            assert!(script.contains("assert("));
            lua.load(script).into_function().unwrap();
        }
    }

    #[test]
    fn test_live_scenario_matches_generator_exactly() {
        let generated = assassin_support::build_scenario_text();
        let actual = read_repo(SCENARIO).replace("\r\n", "\n");
        assert_eq!(
            actual, generated,
            "Assassin scenario differs from the Rust generator"
        );
    }
}
