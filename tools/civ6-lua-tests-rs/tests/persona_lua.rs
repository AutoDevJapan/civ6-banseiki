//! Behavioral port of tests/test_leader_persona_lua.py at d801f97.
//! Each case executes the current worktree's MOD sources in vendored Lua 5.1.

use civ6_lua_tests_rs::official;
use civ6_lua_tests_rs::strict::{ChunkExt, FromLuaMulti, TableExt};
use civ6_lua_tests_rs::{json_to_lua, new_lua, query_json, read_repo};
use mlua::{Function, Lua, MultiValue, Table, Value};
use serde_json::Value as Json;
use std::sync::LazyLock;

const MOD: &str = "poc/JAC_PoC_LeaderPersona";
const SOURCES: [&str; 4] = [
    "Shared/JAC_PersonaTreeRules.lua",
    "Scripts/JAC_LeaderPersona_Gameplay.lua",
    "UI/JAC_LeaderPersona_View.lua",
    "UI/JAC_LeaderPersona_Tree.lua",
];

struct Rows {
    tables: Vec<(&'static str, &'static str, Json)>,
    leaders: Json,
}

static ROWS: LazyLock<Rows> = LazyLock::new(|| {
    let db = official::official_db();
    // The archived official snapshot predates Agendas.xml InsertOrIgnore.
    db.execute_batch("INSERT OR IGNORE INTO Kinds (Kind) VALUES ('KIND_TRAIT');")
        .unwrap();
    for path in [
        "poc/JAC_PoC_CustomCivs/Data/JAC_CustomCivs_Gameplay.sql",
        "poc/JAC_PoC_CustomCivs/Data/JAC_Japan1534_Gameplay.sql",
    ] {
        db.execute_batch(&read_repo(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"));
    }
    official::run_sengoku(&db);
    for path in [
        "poc/JAC_PoC_LeaderPersona/Data/JAC_LeaderPersona.sql",
        "poc/JAC_PoC_LeaderPersona/Data/JAC_LeaderPersona_Trees.sql",
    ] {
        db.execute_batch(&read_repo(path))
            .unwrap_or_else(|error| panic!("{path}: {error}"));
    }
    let tables = [
        ("JAC_PersonaTreeNodes", "NodeType"),
        ("JAC_PersonaTreePeople", "LeaderType"),
        ("JAC_PersonaDisabledGovernors", "GovernorType"),
        ("JAC_PersonaCategories", "CategoryType"),
        ("JAC_PersonaCategoryAccess", "AccessType"),
        ("JAC_PersonaRouteTemplates", "RouteType"),
        ("JAC_PersonaTreeParameters", "Name"),
        ("JAC_PersonaMeritRules", "Profile"),
        ("Buildings", "BuildingType"),
    ]
    .into_iter()
    .map(|(name, key)| {
        let rows = query_json(&db, &format!("SELECT rowid-1 AS \"Index\", * FROM {name}"));
        (name, key, Json::Array(rows))
    })
    .collect();
    let leaders = Json::Array(query_json(
        &db,
        "SELECT rowid-1 AS \"Index\", * FROM JAC_LeaderPersonas WHERE LeaderType IN ('LEADER_JAC_ODA_NOBUHIDE','LEADER_JAC_DATE_TANEMUNE','LEADER_JAC_HOJO_UJITSUNA')",
    ));
    assert_eq!(leaders.as_array().unwrap().len(), 3);
    Rows { tables, leaders }
});

fn load_contexts(lua: &Lua) -> mlua::Result<()> {
    for path in SOURCES {
        lua.load(read_repo(format!("{MOD}/{path}")))
            .set_name(path)
            .exec()?;
    }
    Ok(())
}

fn initialize(lua: &Lua) -> mlua::Result<()> {
    lua.load(include_str!("../fixtures/persona/stubs.lua"))
        .set_name("persona/stubs.lua")
        .exec()?;
    let globals = lua.globals();
    let game_info: Table = globals.strict_get("GameInfo")?;
    let collection: Function = globals.strict_get("collection")?;

    // Preserve the old native mock's three players and native Index/Hash values,
    // while sourcing their persona metadata from the real SQL rows.
    let stub_rows: Table = globals.strict_get("ROWS")?;
    let real_rows = json_to_lua(lua, &ROWS.leaders);
    let Value::Table(real_rows) = real_rows else {
        unreachable!()
    };
    for result in real_rows.strict_sequence_values::<Table>() {
        let row = result?;
        let leader: String = row.strict_get("LeaderType")?;
        let target = stub_rows
            .strict_sequence_values::<Table>()
            .collect::<mlua::Result<Vec<_>>>()?
            .into_iter()
            .find(|stub| stub.strict_get::<String>("LeaderType").unwrap() == leader)
            .expect("SQL persona row absent from native fixture");
        for pair in row.strict_pairs::<String, Value>() {
            let (key, value) = pair?;
            target.set(key, value)?;
        }
    }
    let personas = Table::from_results(collection.call::<MultiValue>((stub_rows, "LeaderType"))?)?;
    game_info.set("JAC_LeaderPersonas", personas)?;

    for (name, key, rows) in &ROWS.tables {
        let Value::Table(table) = json_to_lua(lua, rows) else {
            unreachable!()
        };
        let mapped = Table::from_results(collection.call::<MultiValue>((table, *key))?)?;
        game_info.set(*name, mapped)?;
    }
    load_contexts(lua)
}

fn scenario(lua: &Lua) -> mlua::Result<()> {
    let source = read_repo("tools/civ6-autotest/scenarios/leader-persona-15km.json");
    let scenario: Json = serde_json::from_str(&source).unwrap();
    let steps = scenario["steps"].as_array().unwrap();
    let snapshots: Vec<Json> = steps
        .iter()
        .filter(|step| step.get("capture_as").is_some() || step.get("equals_capture").is_some())
        .map(|step| step["code"].clone())
        .collect();
    let reopen = steps
        .iter()
        .filter_map(|step| step["code"].as_str())
        .find(|code| code.contains("old purchase result replayed on reopen"))
        .expect("reopen assertion absent from current scenario");
    let globals = lua.globals();
    globals.set(
        "SCENARIO_SNAPSHOTS",
        json_to_lua(lua, &Json::Array(snapshots)),
    )?;
    globals.set("SCENARIO_REOPEN_CODE", reopen)?;
    Ok(())
}

fn run_case(name: &str) {
    let lua = new_lua();
    let globals = lua.globals();
    globals
        .set(
            "RESET_TEST",
            lua.create_function(|lua, ()| initialize(lua)).unwrap(),
        )
        .unwrap();
    globals
        .set(
            "LOAD_CONTEXTS",
            lua.create_function(|lua, ()| load_contexts(lua)).unwrap(),
        )
        .unwrap();
    globals
        .set(
            "LOAD_MOD",
            lua.create_function(|lua, name: String| {
                let path = match name.as_str() {
                    "governor_panel" => "UI/JAC_LeaderPersona_GovernorPanel.lua",
                    "unit_panel" => "UI/JAC_LeaderPersona_UnitPanel.lua",
                    "governor_details" => "UI/JAC_LeaderPersona_GovernorDetails.lua",
                    _ => {
                        return Err(mlua::Error::RuntimeError(format!(
                            "unknown MOD context: {name}"
                        )))
                    }
                };
                lua.load(read_repo(format!("{MOD}/{path}")))
                    .set_name(path)
                    .exec()
            })
            .unwrap(),
        )
        .unwrap();
    initialize(&lua).unwrap();
    scenario(&lua).unwrap();
    lua.load(include_str!("../fixtures/persona/cases.lua"))
        .set_name("persona/cases.lua")
        .exec()
        .unwrap();
    let cases: Table = globals.strict_get("CASES").unwrap();
    let test: Function = cases
        .strict_get(name)
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    test.call::<()>(())
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    let assertions: i64 = lua.load("return ASSERTIONS").strict_eval().unwrap();
    assert!(assertions > 0, "{name} made no behavioral assertion");
}

macro_rules! persona_tests {
    ($($name:ident),+ $(,)?) => {$(
        #[test]
        fn $name() { run_case(stringify!($name)); }
    )+};
}

persona_tests!(
    test_all_live_save_snapshots_accept_properties_with_no_return_value,
    test_gameplay_without_ui_governor_methods_uses_fresh_native_reads,
    test_missing_native_reader_never_creates_or_removes_a_person,
    test_non_great_person_unit_type_never_reads_an_individual_identity,
    test_purchase_is_only_available_through_the_gameplay_event,
    test_governor_panel_preserves_foreign_governors_but_blocks_official_and_unjoined_personas,
    test_only_established_unneutralized_city_governance_earns_once_across_reload,
    test_personal_profiles_pay_different_amounts_and_never_share_balances,
    test_building_deeds_reject_repairs_wrong_city_duplicates_and_enforce_cap,
    test_nearby_victories_pay_personal_merit_and_ignore_nonlethal_or_distant_battles,
    test_unproven_building_and_victory_notifications_survive_reload_without_late_rewards,
    test_same_turn_assignment_establishment_or_recovery_cannot_make_old_construction_eligible,
    test_ai_turn_dispatch_uses_committed_state_when_ui_reader_is_missing,
    test_unproven_deeds_cannot_pay_again_on_later_turns,
    test_deferred_events_preserve_ai_turn_before_later_construction,
    test_historical_pending_deed_is_kept_without_inventing_eligibility,
    test_turn_scan_records_alliances_without_inventing_formation_state,
    test_eligible_alliance_formation_pays_eight_once_across_update_reload_and_renewal,
    test_preexisting_alliance_does_not_pay_on_relationship_updates,
    test_alliance_cap_records_two_new_partners_without_next_turn_payment,
    test_absent_gamecore_alliance_event_keeps_turn_scan_unproven,
    test_transfer_does_not_revalue_old_city_deed_and_new_city_can_earn,
    test_ai_six_point_boundary_is_independent_of_ui_reader,
    test_zero_markers_are_ineligible_without_history_but_duplicate_markers_are_unknown,
    test_alliance_formed_before_assignment_establishment_or_recovery_never_pays_late,
    test_same_turn_assignment_establishment_and_recovery_never_revalue_unknown_alliance,
    test_skill_purchase_requires_own_person_prerequisite_merit_and_confirmation,
    test_malformed_gameplay_requests_do_not_reach_native_lookups_or_change_state,
    test_irreversible_category_and_exclusive_sibling_locks_survive_load,
    test_all_six_shared_promotions_and_three_signature_nodes_can_be_acquired,
    test_owned_node_flags_follow_native_assignment_and_new_city_centres,
    test_ai_uses_personality_role_preference_and_only_one_purchase_each_turn,
    test_tree_confirmation_lists_losses_and_cancel_changes_nothing,
    test_tree_spends_only_via_gameplay_request_and_rechecks_queued_duplicates,
    test_queued_conflicting_confirmations_cannot_buy_sibling_or_other_category,
    test_tree_rechecks_stale_balance_at_confirmation_and_respects_inactive_turn,
    test_tree_only_shows_matching_new_purchase_receipts_and_never_replays_old_results,
    test_live_reopen_assertion_accepts_empty_native_label_and_rejects_old_receipt,
    test_keyboard_can_review_cancel_commit_switch_categories_and_cycle_governors,
    test_promotion_titles_fixture_is_removed_and_official_candidates_are_hidden,
    test_void_creation_keeps_named_individual_and_metadata,
    test_synchronized_join_validates_source_and_replays_only_once,
    test_designed_personality_trait_and_cost_are_join_defaults,
    test_fixed_trait_cannot_be_replaced_at_join,
    test_partial_titles_refuse_costlier_date_but_suffice_for_oda,
    test_pending_surcharge_survives_reload_and_spending_without_double_charge,
    test_two_pending_evolutions_cannot_share_the_native_appointment_title,
    test_negative_point_change_is_not_required_for_native_payment,
    test_rejected_second_native_payment_waits_and_resumes_without_double_charge,
    test_upper_cost_consumes_two_extra_titles_and_one_native_once,
    test_joined_cost_is_fixed_even_if_balance_data_changes,
    test_native_individual_identity_is_resolved_through_gameinfo,
    test_repeated_join_and_other_owner_do_not_spawn_or_replace_metadata,
    test_invalid_requests_never_create_a_person,
    test_no_city_failed_spawn_and_retry,
    test_evolution_requires_ownership_person_and_native_title,
    test_native_type_rejection_keeps_person_and_titles_without_reserving_evolution,
    test_pending_evolution_rechecks_native_type_before_requesting_appointment,
    test_native_reader_uses_requested_owner_and_target_type_independently_of_pool,
    test_missing_type_getter_is_unavailable_and_never_uses_pool_as_eligibility,
    test_pending_appointment_keeps_unit_until_native_success,
    test_delayed_removal_does_not_mark_evolution_complete,
    test_native_candidate_object_without_appointment_does_not_remove_the_person,
    test_reload_contexts_preserves_unit_and_reconciles_only_pending,
    test_ui_uses_native_operation_indices_and_engine_requester,
    test_ui_rejects_inactive_turn_and_unowned_governor,
    test_official_governor_panel_hides_unjoined_candidates_and_keeps_native_buttons,
    test_unit_panel_uses_the_leader_portrait_and_preserves_other_units,
    test_governor_details_preserve_native_refresh_then_set_the_leader_portrait,
);

#[test]
fn ai_admission_reserves_full_official_cost_before_any_native_spend() {
    run_case("test_ai_admission_reserves_full_official_cost_before_any_native_spend");
}

#[test]
fn native_receipt_ahead_of_official_spent_keeps_full_reservation() {
    run_case("test_native_receipt_ahead_of_official_spent_keeps_full_reservation");
}

#[test]
fn prior_persona_payment_cannot_mask_unpaid_current_evolution() {
    run_case("test_prior_persona_payment_cannot_mask_unpaid_current_evolution");
}

#[test]
fn lost_unappointed_person_releases_only_its_unpaid_reservation_after_next_turn() {
    run_case("test_lost_unappointed_person_releases_only_its_unpaid_reservation_after_next_turn");
}

#[test]
fn private_manual_admission_keeps_the_fixture_in_control_of_native_requests() {
    for (active, manual, paused) in [(1, 1, true), (0, 1, false), (1, 0, false)] {
        let lua = new_lua();
        initialize(&lua).unwrap();
        lua.globals().set("TEST_ACTIVE", active).unwrap();
        lua.globals().set("TEST_MANUAL", manual).unwrap();
        lua.globals().set("TEST_PAUSED", paused).unwrap();
        lua.load(
            r#"
            local leader='LEADER_JAC_DATE_TANEMUNE'
            local api=ExposedMembers.JAC_LeaderPersona
            Game:SetProperty('JAC_TEST853_ADMISSION_ACTIVE',TEST_ACTIVE)
            Game:SetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT',TEST_MANUAL)
            assert(api.Join(0,1)=='JOINED')
            Players[0].points=api.EvolutionCost(leader)
            assert(api.PrepareEvolution(0,leader)=='EVOLVING')
            FIREUI('GameCoreEventPublishComplete')
            if TEST_PAUSED then
                assert(#REQUESTS==0,'private manual fixture lost ownership of APPOINT')
                assert(Players[0].spent==0)
                assert(Game:GetProperty('JAC_PERSONA_'..leader..'_STATE')=='EVOLVING')
                Game:SetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT',nil)
                FIREUI('GameCoreEventPublishComplete')
            end
            assert(#REQUESTS==1 and REQUESTS[1].op==PlayerOperations.APPOINT_GOVERNOR,
                   'normal automatic appointment must remain available')
            FIREUI('GameCoreEventPublishComplete')
            assert(#REQUESTS==1,'automatic APPOINT was duplicated')
            DRAIN()
            assert(Game:GetProperty('JAC_PERSONA_'..leader..'_STATE')=='GOVERNOR')
            assert(Players[0].spent==api.EvolutionCost(leader) and DESTROYS==1)
            "#,
        )
        .set_name("manual-admission-operation-ownership")
        .exec()
        .unwrap();
    }
}
