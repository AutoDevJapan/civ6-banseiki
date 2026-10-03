//! New spy acceptance cases live apart from the fixed 119-method PR86 parity inventory.
//! These execute the shipped Lua under mlua's Lua 5.1 engine, not the Civ6 runtime.

use civ6_lua_tests_rs::strict::{FromLua, FromLuaMulti, TableExt};
use civ6_lua_tests_rs::{exec, new_lua, read_repo};
use mlua::{Function, IntoLuaMulti, Lua, MultiValue, Table};

struct SpyHarness(Lua);

fn table_call(function: &Function, args: impl IntoLuaMulti) -> mlua::Result<Table> {
    Table::from_results(function.call::<MultiValue>(args)?)
}

fn optional_table_call(
    function: &Function,
    args: impl IntoLuaMulti,
) -> mlua::Result<Option<Table>> {
    Option::<Table>::from_results(function.call::<MultiValue>(args)?)
}

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
        exec(
            &lua,
            &read_repo("poc/JAC_Test_PersonaBoundary/Scripts/JAC_PersonaBoundary_Fixture.lua"),
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

    fn call(&self, name: &str, args: impl IntoLuaMulti) {
        self.global::<Function>(name).call::<()>(args).unwrap();
    }

    fn event(&self, name: &str) -> mlua::Result<()> {
        let event: Table = self.global::<Table>("GameEvents").strict_get(name).unwrap();
        self.global::<Function>("fire").call((event, 0))
    }

    fn city_queue_event(&self) {
        let event: Table = self
            .global::<Table>("Events")
            .strict_get("CityProductionQueueChanged")
            .unwrap();
        self.global::<Function>("fire")
            .call::<()>((event, 0, 10))
            .unwrap();
    }

    fn barrier(&self) {
        self.call("ACK_BARRIER", ());
    }

    fn adoption(&self) {
        self.call("ACK_ADOPTION", ());
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
    assert_eq!(h.action(), "BARRIER_SENT_SPY_TRAVEL");
    h.barrier();
    h.frame().unwrap();
    assert_eq!(h.action(), "ACCEPTED_SPY_TRAVEL");
    let spy: Table = self_spy(&h);
    assert_eq!(spy.strict_get::<bool>("present").unwrap(), false);
    let read = h.b().strict_get::<Function>("Read").unwrap();
    let roster: Table = table_call(&read, ())
        .unwrap()
        .strict_get("spyRoster")
        .unwrap();
    let off_map: Table = roster.strict_get("offMap").unwrap();
    assert_eq!(off_map.raw_len(), 1);
    let record: Table = off_map.strict_get(1).unwrap();
    assert_eq!(record.strict_get::<i64>("returnTurn").unwrap(), 8);
    assert_eq!(h.requests(), 1);
    let writes: Table = h.global("WRITES");
    assert_eq!(writes.raw_len(), 2);
    assert_eq!(writes.strict_get::<String>(1).unwrap(), "JAC_BOUNDARY_SPY");
    assert_eq!(
        writes.strict_get::<String>(2).unwrap(),
        "JAC_BOUNDARY_SPY_BARRIER"
    );
}

fn self_spy(h: &SpyHarness) -> Table {
    let read = h.b().strict_get::<Function>("Read").unwrap();
    let state = table_call(&read, ()).unwrap();
    state.strict_get("spy").unwrap()
}

#[test]
fn travel_requires_matching_native_roster_destination_and_future_return() {
    for (effect, accepted) in [
        ("arrive", true),
        ("different_spy_offmap", false),
        ("different_level_offmap", false),
        ("wrong_destination_offmap", false),
        ("expired_offmap", false),
        ("multi_offmap", false),
        ("wrong_arrive", false),
        ("none", false),
    ] {
        let h = SpyHarness::new();
        h.case("spy_home", (0, 15));
        h.set("REQUEST_EFFECT", effect);
        h.queue("SPY_TRAVEL");
        h.frame().unwrap();
        h.frame().unwrap();
        h.barrier();
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
        if effect == "arrive" {
            assert_eq!(self_spy(&h).strict_get::<i64>("endTurn").unwrap(), 0);
        }
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
    for effect in ["mission", "mission_hash"] {
        let h = SpyHarness::new();
        h.case("spy", (0, 15));
        h.case("target_governor", ());
        h.set("REQUEST_EFFECT", effect);
        h.queue("SPY_MISSION");
        h.frame().unwrap();
        h.frame().unwrap();
        h.barrier();
        h.frame().unwrap();
        assert_eq!(h.action(), "ACCEPTED_SPY_MISSION", "{effect}");
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
}

#[test]
fn travel_waits_for_the_synchronized_barrier_before_reading_off_map_state() {
    let h = SpyHarness::new();
    h.case("spy_home", (0, 15));
    h.set("REQUEST_EFFECT", "travel");
    h.queue("SPY_TRAVEL");
    h.frame().unwrap();
    h.frame().unwrap();
    assert_eq!(h.action(), "BARRIER_SENT_SPY_TRAVEL");
    h.frame().unwrap();
    assert_eq!(h.action(), "BARRIER_SENT_SPY_TRAVEL");
    assert_eq!(h.requests(), 1);
    h.barrier();
    h.frame().unwrap();
    assert_eq!(h.action(), "ACCEPTED_SPY_TRAVEL");
}

#[test]
fn immediate_arrival_with_new_unit_id_adopts_only_the_diagnostic_reference() {
    let h = SpyHarness::new();
    h.case("spy_home", (0, 15));
    h.set("REQUEST_EFFECT", "arrive_new_id");
    h.queue("SPY_TRAVEL");
    h.frame().unwrap();
    h.frame().unwrap();
    h.barrier();
    h.frame().unwrap();
    assert_eq!(h.action(), "ADOPT_PENDING_SPY_TRAVEL");
    let game: Table = h.global("Game");
    let get = game.strict_get::<Function>("GetProperty").unwrap();
    let old = table_call(&get, (game.clone(), "JAC_BOUNDARY_SPY")).unwrap();
    assert_eq!(old.strict_get::<i64>("id").unwrap(), 15);
    h.frame().unwrap();
    h.adoption();
    h.frame().unwrap();
    assert_eq!(h.action(), "ACCEPTED_SPY_TRAVEL");
    assert_eq!(self_spy(&h).strict_get::<i64>("id").unwrap(), 16);
    assert_eq!(h.requests(), 1);
}

#[test]
fn off_map_arrival_with_new_id_can_be_bound_after_native_observation() {
    let h = SpyHarness::new();
    h.case("spy_home", (0, 15));
    h.set("REQUEST_EFFECT", "travel");
    h.queue("SPY_TRAVEL");
    h.frame().unwrap();
    h.frame().unwrap();
    h.barrier();
    h.frame().unwrap();
    assert_eq!(h.action(), "ACCEPTED_SPY_TRAVEL");
    h.call("ARRIVE_OFFMAP", (16,));
    h.queue("ADOPT_SPY");
    h.frame().unwrap();
    h.frame().unwrap();
    h.adoption();
    h.frame().unwrap();
    assert_eq!(h.action(), "ACCEPTED_ADOPT_SPY");
    assert_eq!(self_spy(&h).strict_get::<i64>("id").unwrap(), 16);
}

#[test]
fn travel_refuses_ambiguous_native_spies_before_request() {
    let h = SpyHarness::new();
    h.case("spy_home", (0, 15));
    h.call("makeSpy", (0, 16));
    h.queue("SPY_TRAVEL");
    assert!(h.frame().is_err());
    assert_eq!(h.requests(), 0);

    let h = SpyHarness::new();
    h.case("spy_home", (0, 15));
    let record = h.0.create_table().unwrap();
    record.set("Name", "Existing Spy").unwrap();
    record.set("Level", 1).unwrap();
    record.set("ReturnTurn", 10).unwrap();
    record.set("XLocation", 2).unwrap();
    record.set("YLocation", 1).unwrap();
    h.global::<Table>("OFF_MAP").set(1, record).unwrap();
    h.queue("SPY_TRAVEL");
    assert!(h.frame().is_err());
    assert_eq!(h.requests(), 0);
}

#[test]
fn operation_started_event_alone_cannot_accept_an_unchanged_roster() {
    let h = SpyHarness::new();
    h.case("spy_home", (0, 15));
    h.queue("SPY_TRAVEL");
    h.frame().unwrap();
    h.call("START_NATIVE", (0, 15, 991));
    h.frame().unwrap();
    h.barrier();
    h.frame().unwrap();
    assert_eq!(h.action(), "REJECTED_SPY_TRAVEL");
    assert_eq!(h.requests(), 1);
}

fn prepare_spy_build(h: &SpyHarness) {
    h.event("JAC_BoundarySpawnSpy").unwrap();
    let ready = h
        .b()
        .strict_get::<Function>("SpyPrerequisitesReady")
        .unwrap();
    table_call(&ready, ()).unwrap();
    h.queue("BUILD_SPY");
    h.frame().unwrap();
    h.frame().unwrap();
    assert_eq!(h.action(), "READY_BUILD_SPY");
    assert_eq!(h.global::<Table>("BUILD_REQUESTS").raw_len(), 1);
}

#[test]
fn native_civic_build_and_one_turn_progress_create_exactly_one_spy() {
    let h = SpyHarness::new();
    prepare_spy_build(&h);
    h.queue("PROGRESS_SPY");
    h.frame().unwrap();
    h.event("JAC_BoundaryProgressSpy").unwrap();
    assert_eq!(h.global::<i64>("PROGRESS_CALLS"), 1);
    h.call("ADVANCE_TURN", ());
    h.queue("BIND_SPY");
    h.frame().unwrap();
    h.event("JAC_BoundaryBindSpy").unwrap();
    assert_eq!(self_spy(&h).strict_get::<i64>("id").unwrap(), 15);
    assert_eq!(h.global::<Table>("UNIT_REQUESTS").raw_len(), 0);
    let request: Table = h.global::<Table>("BUILD_REQUESTS").strict_get(1).unwrap();
    let selected: Table = request.strict_get("selected").unwrap();
    assert_eq!(selected.strict_get::<i64>("id").unwrap(), 10);
}

#[test]
fn spy_capacity_and_native_build_refusal_prevent_progress() {
    for flag in ["SPY_CAPACITY", "BUILD_CAN_START"] {
        let h = SpyHarness::new();
        h.event("JAC_BoundarySpawnSpy").unwrap();
        if flag == "SPY_CAPACITY" {
            h.set(flag, 0_i64);
        } else {
            h.set(flag, false);
        }
        h.queue("BUILD_SPY");
        assert!(h.frame().is_err(), "{flag}");
        assert_eq!(h.global::<i64>("PROGRESS_CALLS"), 0);
        assert_eq!(h.global::<Table>("BUILD_REQUESTS").raw_len(), 0);
    }
}

#[test]
fn spy_preparation_grants_the_civic_before_requiring_its_native_effect() {
    let h = SpyHarness::new();
    h.event("JAC_BoundarySpawnSpy").unwrap();
    assert!(h.global::<bool>("CIVIC_COMPLETED"));
    assert_eq!(h.global::<i64>("CIVIC_GRANT_CALLS"), 1);
    h.queue("BUILD_SPY");
    h.frame().unwrap();
    assert_eq!(h.global::<Table>("BUILD_REQUESTS").raw_len(), 1);
}

#[test]
fn gameplay_progress_does_not_require_the_ui_only_queue_getter() {
    let h = SpyHarness::new();
    h.event("JAC_BoundarySpawnSpy").unwrap();
    h.queue("BUILD_SPY");
    h.frame().unwrap();
    h.city_queue_event();
    h.frame().unwrap();
    assert_eq!(h.action(), "READY_BUILD_SPY");
    let city: Table = h.global("CONTROL");
    let queue: Table = city
        .strict_get::<Function>("GetBuildQueue")
        .unwrap()
        .call(city.clone())
        .unwrap();
    queue.set("GetAt", mlua::Value::Nil).unwrap();
    h.event("JAC_BoundaryProgressSpy").unwrap();
    assert_eq!(h.global::<i64>("PROGRESS_CALLS"), 1);
    assert_eq!(
        h.global::<Table>("UNITS")
            .strict_get::<Table>(0)
            .unwrap()
            .raw_len(),
        0
    );
}

#[test]
fn wrong_spy_civic_or_unaccepted_city_queue_cannot_start_production() {
    let h = SpyHarness::new();
    let units: Table = h.global::<Table>("GameInfo").strict_get("Units").unwrap();
    units
        .strict_get::<Table>("UNIT_SPY")
        .unwrap()
        .set("PrereqCivic", "CIVIC_WRONG")
        .unwrap();
    assert!(h.event("JAC_BoundarySpawnSpy").is_err());
    assert_eq!(h.global::<i64>("PROGRESS_CALLS"), 0);

    let h = SpyHarness::new();
    h.event("JAC_BoundarySpawnSpy").unwrap();
    h.set("BUILD_ACCEPTED", false);
    h.queue("BUILD_SPY");
    h.frame().unwrap();
    h.city_queue_event();
    h.frame().unwrap();
    assert_eq!(h.action(), "REJECTED_BUILD_SPY");
    assert_eq!(h.global::<i64>("PROGRESS_CALLS"), 0);
}

#[test]
fn wrong_native_mission_type_is_rejected_even_with_future_end_turn() {
    let h = SpyHarness::new();
    h.case("spy", (0, 15));
    h.case("target_governor", ());
    h.set("REQUEST_EFFECT", "mission_wrong");
    h.queue("SPY_MISSION");
    h.frame().unwrap();
    h.frame().unwrap();
    h.barrier();
    h.frame().unwrap();
    assert_eq!(h.action(), "REJECTED_SPY_MISSION");
}

#[test]
fn missing_or_multiple_produced_spies_never_bind_a_diagnostic_id() {
    for duplicate in [false, true] {
        let h = SpyHarness::new();
        prepare_spy_build(&h);
        h.queue("PROGRESS_SPY");
        h.frame().unwrap();
        h.event("JAC_BoundaryProgressSpy").unwrap();
        if !duplicate {
            h.set("PRODUCTION_COMPLETES", false);
        }
        h.call("ADVANCE_TURN", ());
        if duplicate {
            h.call("makeSpy", (0, 16));
        }
        assert!(h.event("JAC_BoundaryBindSpy").is_err());
        let game: Table = h.global("Game");
        let get = game.strict_get::<Function>("GetProperty").unwrap();
        let spy = optional_table_call(&get, (game.clone(), "JAC_BOUNDARY_SPY")).unwrap();
        assert!(spy.is_none());
    }
}
