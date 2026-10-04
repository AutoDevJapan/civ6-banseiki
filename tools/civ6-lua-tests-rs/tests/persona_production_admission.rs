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
