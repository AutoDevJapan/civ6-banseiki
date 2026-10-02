//! Exercise the production Rust generator against the checked-in scenario.

use civ6_data_rs::{assassin, Repository};
use civ6_lua_tests_rs::repo_root;

pub fn build_scenario() -> serde_json::Value {
    assassin::build_scenario(&Repository::new(repo_root())).unwrap()
}

pub fn build_scenario_text() -> String {
    serde_json::to_string_pretty(&build_scenario()).unwrap() + "\n"
}
