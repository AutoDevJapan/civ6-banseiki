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
            &read_repo("poc/JAC_Test_PersonaBoundary/Scripts/JAC_PersonaBoundary_Fixture.lua"),
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

    fn event(&self, name: &str) -> mlua::Result<()> {
        let events: Table = self.global("GameEvents");
        let event: Table = events.strict_get(name).unwrap();
        self.global::<Function>("fire").call((event, 0))
    }

    fn production(&self) -> Table {
        self.b()
            .strict_get::<Function>("SpyProduction")
            .unwrap()
            .call(())
            .unwrap()
    }
}

#[test]
fn official_build_is_accepted_before_progress_and_registers_only_new_native_spy() {
    let h = SpyHarness::new();
    h.case("no_persona_config", ());
    assert_eq!(h.production().strict_get::<i64>("capacity").unwrap(), 0);
    h.b()
        .strict_get::<Function>("Queue")
        .unwrap()
        .call::<()>(("SPY_UNLOCK", Option::<Table>::None))
        .unwrap();
    h.frame().unwrap();
    let requests: Table = h.global("REQUESTS");
    assert_eq!(requests.raw_len(), 1);
    let unlock: Table = requests.strict_get(1).unwrap();
    let params: Table = unlock.strict_get("params").unwrap();
    assert_eq!(
        params.strict_get::<String>("OnStart").unwrap(),
        "JAC_BoundaryUnlockSpy"
    );
    h.event("JAC_BoundaryUnlockSpy").unwrap();
    let unlocked = h.production();
    assert!(unlocked.strict_get::<bool>("civic").unwrap());
    assert_eq!(unlocked.strict_get::<i64>("capacity").unwrap(), 1);
    assert_eq!(unlocked.strict_get::<Table>("spies").unwrap().raw_len(), 0);

    h.b()
        .strict_get::<Function>("Queue")
        .unwrap()
        .call::<()>(("SPY_BUILD", Option::<Table>::None))
        .unwrap();
    h.frame().unwrap();
    assert_eq!(h.action(), "DISPATCHED_SPY_BUILD");
    let city_requests: Table = h.global("CITY_REQUESTS");
    assert_eq!(city_requests.raw_len(), 1);
    let build: Table = city_requests.strict_get(1).unwrap();
    assert_eq!(build.strict_get::<i64>("op").unwrap(), 3);
    let params: Table = build.strict_get("params").unwrap();
    assert_eq!(params.strict_get::<i64>("unit").unwrap(), 555);
    assert_eq!(params.strict_get::<String>("insert").unwrap(), "replace");
    assert_eq!(params.strict_get::<i64>("location").unwrap(), 0);
    h.set("BUSY", true);
    h.frame().unwrap();
    assert_eq!(h.action(), "DISPATCHED_SPY_BUILD");
    h.set("BUSY", false);
    h.frame().unwrap();
    assert_eq!(h.action(), "ACCEPTED_SPY_BUILD");
    assert_eq!(h.production().strict_get::<i64>("building").unwrap(), 555);

    h.b()
        .strict_get::<Function>("Queue")
        .unwrap()
        .call::<()>(("SPY_FINISH", Option::<Table>::None))
        .unwrap();
    h.frame().unwrap();
    assert_eq!(requests.raw_len(), 2);
    let finish: Table = requests.strict_get(2).unwrap();
    let params: Table = finish.strict_get("params").unwrap();
    assert_eq!(
        params.strict_get::<String>("OnStart").unwrap(),
        "JAC_BoundaryFinishSpy"
    );
    h.event("JAC_BoundaryFinishSpy").unwrap();
    let logs: Vec<String> = h
        .global::<Table>("LOGS")
        .sequence_values()
        .collect::<mlua::Result<_>>()
        .unwrap();
    assert!(logs.iter().any(|line| line.contains("SPY_PRODUCED")
        && line.contains("sourceCity=10")
        && line.contains("id=15")));
    let roster: Table = h.production().strict_get("spies").unwrap();
    assert_eq!(roster.raw_len(), 1);
    let spy: Table = roster.strict_get(1).unwrap();
    assert_eq!(spy.strict_get::<i64>("id").unwrap(), 15);
    assert_eq!(spy.strict_get::<i64>("cityID").unwrap(), 10);
    assert_eq!(spy.strict_get::<i64>("operation").unwrap(), -1);
    assert_eq!(spy.strict_get::<i64>("endTurn").unwrap(), 0);
    let game: Table = h.global("Game");
    let info: Table = game
        .strict_get::<Function>("GetProperty")
        .unwrap()
        .call((game, "JAC_BOUNDARY_SPY"))
        .unwrap();
    assert_eq!(info.strict_get::<i64>("id").unwrap(), 15);
    let writes: Table = h.global("WRITES");
    assert_eq!(writes.raw_len(), 1);
    assert_eq!(writes.strict_get::<String>(1).unwrap(), "JAC_BOUNDARY_SPY");
    h.set("REQUEST_EFFECT", "travel");
    h.queue("SPY_TRAVEL");
    h.frame().unwrap();
    h.frame().unwrap();
    assert_eq!(h.action(), "ACCEPTED_SPY_TRAVEL");
    assert_eq!(h.requests(), 1);
}

#[test]
fn production_refusal_and_unobserved_completion_cannot_be_reported_as_a_spy() {
    let h = SpyHarness::new();
    h.b()
        .strict_get::<Function>("Queue")
        .unwrap()
        .call::<()>(("SPY_BUILD", Option::<Table>::None))
        .unwrap();
    let error = h.frame().expect_err("no capacity must refuse before BUILD");
    assert!(error.to_string().contains("capacity unavailable"));
    assert_eq!(h.global::<Table>("CITY_REQUESTS").raw_len(), 0);

    let h = SpyHarness::new();
    h.event("JAC_BoundaryUnlockSpy").unwrap();
    h.set("CAN_PRODUCE", false);
    h.b()
        .strict_get::<Function>("Queue")
        .unwrap()
        .call::<()>(("SPY_BUILD", Option::<Table>::None))
        .unwrap();
    let error = h
        .frame()
        .expect_err("native production refusal must stop BUILD");
    assert!(error.to_string().contains("production refused"));
    assert_eq!(h.global::<Table>("CITY_REQUESTS").raw_len(), 0);

    let h = SpyHarness::new();
    h.event("JAC_BoundaryUnlockSpy").unwrap();
    h.set("BUILD_ACCEPT", false);
    h.b()
        .strict_get::<Function>("Queue")
        .unwrap()
        .call::<()>(("SPY_BUILD", Option::<Table>::None))
        .unwrap();
    h.frame().unwrap();
    h.frame().unwrap();
    assert_eq!(h.action(), "REJECTED_SPY_BUILD");
    assert_eq!(h.global::<Table>("CITY_REQUESTS").raw_len(), 1);
    let error = h
        .b()
        .strict_get::<Function>("Queue")
        .unwrap()
        .call::<()>(("SPY_BUILD", Option::<Table>::None))
        .expect_err("unknown native result must not replay");
    assert!(error.to_string().contains("cannot be replayed"));

    let h = SpyHarness::new();
    h.event("JAC_BoundaryUnlockSpy").unwrap();
    h.global::<Table>("BUILD_QUEUE")
        .set("building", 555)
        .unwrap();
    h.case("production_without_unit", ());
    let error = h
        .event("JAC_BoundaryFinishSpy")
        .expect_err("progress without a new roster member must refuse");
    assert!(error.to_string().contains("did not add exactly one spy"));
    assert_eq!(
        h.production()
            .strict_get::<Table>("spies")
            .unwrap()
            .raw_len(),
        0
    );
    assert_eq!(h.global::<Table>("WRITES").raw_len(), 0);
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
