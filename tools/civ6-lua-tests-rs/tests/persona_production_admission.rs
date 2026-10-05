//! Normal Gameplay publication is tested separately from the private B gate.
use civ6_lua_tests_rs::{new_lua, read_repo};
use mlua::{Function, Table};

const GAMEPLAY: &str = "poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua";
const RULES: &str = "poc/JAC_PoC_LeaderPersona/Shared/JAC_PersonaTreeRules.lua";

fn run_case(name: &str) {
    let lua = new_lua();
    for (source, label) in [
        (
            include_str!("../fixtures/persona/stubs.lua").to_string(),
            "stubs",
        ),
        (
            include_str!("../fixtures/persona/admission_engine.lua").to_string(),
            "private engine",
        ),
        (
            include_str!("../fixtures/persona/production_admission_engine.lua").to_string(),
            "extended engine",
        ),
        (read_repo(RULES), "rules"),
        (read_repo(GAMEPLAY), "Gameplay"),
        (
            include_str!("../fixtures/persona/production_admission_cases.lua").to_string(),
            "cases",
        ),
    ] {
        lua.load(&source).set_name(label).exec().unwrap();
    }
    lua.globals()
        .set("GAMEPLAY_SOURCE", read_repo(GAMEPLAY))
        .unwrap();
    let cases: Table = lua.globals().get("PRODUCTION_ADMISSION_CASES").unwrap();
    let case: Function = cases.get(name).unwrap();
    case.call::<()>(())
        .unwrap_or_else(|error| panic!("{name}: {error}"));
}

#[test]
fn ai_owner_and_sparse_gameinfo_indices() {
    run_case("ai_owner_and_sparse_gameinfo_indices");
}

#[test]
fn cost_two_reserves_all_titles_and_rejects_wrong_owner() {
    run_case("cost_two_reserves_all_titles_and_rejects_wrong_owner");
}

#[test]
fn official_payments_and_core_only_final_state() {
    run_case("official_payments_and_core_only_final_state");
}

#[test]
fn cost_three_needs_two_ordered_official_promotions() {
    run_case("cost_three_needs_two_ordered_official_promotions");
}

#[test]
fn partial_native_payment_reserves_only_each_persons_unpaid_titles() {
    run_case("partial_native_payment_reserves_only_each_persons_unpaid_titles");
}

#[test]
fn unchanged_turns_do_not_republish_numeric_keys() {
    run_case("unchanged_turns_do_not_republish_numeric_keys");
}
#[test]
fn stale_true_during_closure_aborts_owner_publication() {
    run_case("stale_true_during_closure_aborts_owner_publication");
}
#[test]
fn owner_reuse_and_reload_reclose_saved_true() {
    run_case("owner_reuse_and_reload_reclose_saved_true");
}
#[test]
fn missing_core_has_governor_stays_closed_and_diagnostic_reports_it() {
    run_case("missing_core_has_governor_stays_closed_and_diagnostic_reports_it");
}
#[test]
fn failed_reset_and_false_ack_never_publish_manifest() {
    run_case("failed_reset_and_false_ack_never_publish_manifest");
}
#[test]
fn owner_operation_publishes_only_changed_cells() {
    run_case("owner_operation_publishes_only_changed_cells");
}
#[test]
fn twenty_two_owners_initialize_once_and_keep_300_turns_bounded() {
    run_case("twenty_two_owners_initialize_once_and_keep_300_turns_bounded");
}
#[test]
fn reload_reset_failure_still_closes_every_owner_before_aborting() {
    run_case("reload_reset_failure_still_closes_every_owner_before_aborting");
}
#[test]
fn legacy_schema_does_not_overwrite_a_foreign_close_key() {
    run_case("legacy_schema_does_not_overwrite_a_foreign_close_key");
}
#[test]
fn legacy_schema_claims_empty_close_range_before_republishing() {
    run_case("legacy_schema_claims_empty_close_range_before_republishing");
}
