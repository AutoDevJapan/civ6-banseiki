//! New spy acceptance cases live apart from the fixed 119-method PR86 parity inventory.
//! These execute the shipped Lua under mlua's Lua 5.1 engine, not the Civ6 runtime.

use civ6_lua_tests_rs::strict::{FromLua, TableExt};
use civ6_lua_tests_rs::{exec, new_lua, read_repo};
use mlua::{Function, IntoLuaMulti, Lua, MultiValue, Table};

struct SpyHarness(Lua);

impl SpyHarness {
    fn new() -> Self {
        let lua = new_lua();
        exec(
            &lua,
            include_str!("../fixtures/persona/boundary_engine.lua"),
        );
        exec(&lua, include_str!("../fixtures/persona/boundary_cases.lua"));
        exec(
            &lua,
            &read_repo("poc/JAC_Test_PersonaBoundary/Shared/JAC_PersonaBoundaryRules.lua"),
        );
        exec(
            &lua,
            &read_repo("poc/JAC_Test_PersonaBoundary/UI/JAC_PersonaBoundary_View.lua"),
        );
        Self(lua)
    }

    fn global<T: FromLua>(&self, name: &str) -> T {
        self.0.globals().strict_get(name).unwrap()
    }

    fn b(&self) -> Table {
        self.global::<Table>("ExposedMembers")
            .strict_get("JAC_PersonaBoundary")
            .unwrap()
    }

    fn case(&self, name: &str, args: impl IntoLuaMulti) {
        self.global::<Table>("BOUNDARY_CASES")
            .strict_get::<Function>(name)
            .unwrap()
            .call::<()>(args)
            .unwrap();
    }

    fn set(&self, name: &str, value: impl mlua::IntoLua) {
        self.0.globals().set(name, value).unwrap();
    }

    fn queue(&self, action: &str) {
        let params = self.0.create_table().unwrap();
        params.set("owner", 1).unwrap();
        params.set("city", 20).unwrap();
        self.b()
            .strict_get::<Function>("Queue")
            .unwrap()
            .call::<()>((action, params))
            .unwrap();
    }

    fn frame(&self) -> mlua::Result<()> {
        self.global::<Function>("FRAME").call(())
    }

    fn action(&self) -> String {
        self.b().strict_get("lastAction").unwrap()
    }

    fn requests(&self) -> usize {
        self.global::<Table>("UNIT_REQUESTS").raw_len()
    }
}

#[test]
fn accepted_travel_is_observed_on_first_nonbusy_frame() {
    let h = SpyHarness::new();
    h.case("spy_home", (0, 15));
    h.set("REQUEST_EFFECT", "travel");
    h.queue("SPY_TRAVEL");
    h.frame().unwrap();
    assert_eq!(h.action(), "DISPATCHED_SPY_TRAVEL");
    h.set("BUSY", true);
    h.frame().unwrap();
    assert_eq!(h.action(), "DISPATCHED_SPY_TRAVEL");
    h.set("BUSY", false);
    h.frame().unwrap();
    assert_eq!(h.action(), "ACCEPTED_SPY_TRAVEL");
    let spy: Table = self_spy(&h);
    assert_eq!(spy.strict_get::<i64>("operation").unwrap(), 1);
    assert_eq!(spy.strict_get::<i64>("endTurn").unwrap(), 8);
    assert_eq!(h.requests(), 1);
    let writes: Table = h.global("WRITES");
    assert_eq!(writes.raw_len(), 1);
    assert_eq!(writes.strict_get::<String>(1).unwrap(), "JAC_BOUNDARY_SPY");
}

fn self_spy(h: &SpyHarness) -> Table {
    let state: Table = h
        .b()
        .strict_get::<Function>("Read")
        .unwrap()
        .call(())
        .unwrap();
    state.strict_get("spy").unwrap()
}

#[test]
fn travel_requires_real_destination_or_active_native_assignment() {
    for (effect, accepted) in [
        ("arrive", true),
        ("wrong_arrive", false),
        ("empty_arrive", false),
    ] {
        let h = SpyHarness::new();
        h.case("spy_home", (0, 15));
        h.set("REQUEST_EFFECT", effect);
        h.queue("SPY_TRAVEL");
        h.frame().unwrap();
        h.frame().unwrap();
        assert_eq!(
            h.action(),
            if accepted {
                "ACCEPTED_SPY_TRAVEL"
            } else {
                "REJECTED_SPY_TRAVEL"
            },
            "{effect}"
        );
        assert_eq!(h.requests(), 1);
    }
}

#[test]
fn spy_selection_mode_and_readiness_are_checked_before_dispatch() {
    for flag in ["SELECT_FAIL", "MODE_FAIL", "SPY_READY"] {
        let h = SpyHarness::new();
        h.case("spy_home", (0, 15));
        h.set(flag, flag != "SPY_READY");
        h.queue("SPY_TRAVEL");
        assert!(
            h.frame().is_err(),
            "{flag} must refuse before native request"
        );
        assert_eq!(h.requests(), 0);
        assert_eq!(h.action(), "ERROR_SPY_TRAVEL");
    }
}

#[test]
fn travel_chooser_refuses_a_spy_outside_its_home_city() {
    let h = SpyHarness::new();
    h.case("spy", (0, 15));
    h.queue("SPY_TRAVEL");
    let error = h
        .frame()
        .expect_err("official travel chooser needs a home city");
    assert!(error.to_string().contains("home city"), "{error}");
    assert_eq!(h.requests(), 0);
}

#[test]
fn mission_assignment_does_not_claim_governor_neutralization() {
    let h = SpyHarness::new();
    h.case("spy", (0, 15));
    h.case("target_governor", ());
    h.set("REQUEST_EFFECT", "mission");
    h.queue("SPY_MISSION");
    h.frame().unwrap();
    h.frame().unwrap();
    assert_eq!(h.action(), "ACCEPTED_SPY_MISSION");
    let error = h
        .b()
        .strict_get::<Function>("Check")
        .unwrap()
        .call::<MultiValue>(("NEUTRALIZED", Option::<i64>::None))
        .expect_err("request acceptance is not neutralization");
    assert!(error
        .to_string()
        .contains("neutralization was not observed"));
    assert_eq!(h.requests(), 1);
}
