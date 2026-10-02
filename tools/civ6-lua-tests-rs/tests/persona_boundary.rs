//! Independent engine-boundary diagnostics. Passing here does not establish
//! that the real Civ6 engine executed these scenarios successfully.

use civ6_lua_tests_rs::strict::{ChunkExt, FromLua, FromLuaMulti, TableExt};
use civ6_lua_tests_rs::{exec, new_lua, read_repo};
use mlua::{Function, IntoLuaMulti, Lua, MultiValue, Table, Value};
use regex::Regex;
use rusqlite::{types::ValueRef, Connection};

const MOD: &str = "poc/JAC_Test_PersonaBoundary";
const VIEW: &str = "poc/JAC_Test_PersonaBoundary/UI/JAC_PersonaBoundary_View.lua";
const FIXTURE: &str = "poc/JAC_Test_PersonaBoundary/Scripts/JAC_PersonaBoundary_Fixture.lua";
const LEADER: &str = "LEADER_JAC_ODA_NOBUHIDE";

fn strict_table_call(function: &Function, args: impl IntoLuaMulti) -> mlua::Result<Table> {
    Table::from_results(function.call::<MultiValue>(args)?)
}

struct Boundary(Lua);

impl Boundary {
    fn new() -> Self {
        let lua = new_lua();
        exec(
            &lua,
            include_str!("../fixtures/persona/boundary_engine.lua"),
        );
        exec(&lua, include_str!("../fixtures/persona/boundary_cases.lua"));
        exec(
            &lua,
            &read_repo(format!("{MOD}/Shared/JAC_PersonaBoundaryRules.lua")),
        );
        exec(&lua, &read_repo(VIEW));
        Self(lua)
    }

    fn global<T: FromLua>(&self, name: &str) -> T {
        self.0.globals().strict_get(name).unwrap()
    }

    fn set(&self, name: &str, value: impl mlua::IntoLua) {
        self.0.globals().set(name, value).unwrap();
    }

    fn case(&self, name: &str, args: impl IntoLuaMulti) {
        let cases: Table = self.global("BOUNDARY_CASES");
        let f: Function = cases.strict_get(name).unwrap();
        f.call::<()>(args).unwrap();
    }

    fn call_global(&self, name: &str, args: impl IntoLuaMulti) {
        self.global::<Function>(name).call::<()>(args).unwrap();
    }

    fn event(&self, name: &str) -> mlua::Result<()> {
        let events: Table = self.global("GameEvents");
        let event: Table = events.strict_get(name).unwrap();
        self.global::<Function>("fire").call((event, 0))
    }

    fn b(&self) -> Table {
        self.global::<Table>("ExposedMembers")
            .strict_get("JAC_PersonaBoundary")
            .unwrap()
    }

    fn read(&self) -> Table {
        self.0
            .load("return ExposedMembers.JAC_PersonaBoundary.Read()")
            .strict_eval()
            .unwrap()
    }

    fn check(&self, phase: &str, merit: Option<i64>) -> String {
        self.0
            .load("return ExposedMembers.JAC_PersonaBoundary.Check(...)")
            .strict_call((phase, merit))
            .unwrap()
    }

    fn error_check(&self, phase: &str, merit: Option<i64>, expected: &str) {
        let error = self
            .0
            .load("return ExposedMembers.JAC_PersonaBoundary.Check(...)")
            .call::<MultiValue>((phase, merit))
            .expect_err("boundary check must reject");
        assert!(error.to_string().contains(expected), "{error}");
    }

    fn frame(&self) -> mlua::Result<()> {
        self.global::<Function>("FRAME").call(())
    }

    fn queue(&self, action: &str, owner: i64, city: i64) {
        let params = self.0.create_table().unwrap();
        params.set("owner", owner).unwrap();
        params.set("city", city).unwrap();
        self.b()
            .strict_get::<Function>("Queue")
            .unwrap()
            .call::<()>((action, params))
            .unwrap();
    }

    fn game_property<T: FromLuaMulti>(&self, name: &str) -> T {
        self.0
            .load("return Game:GetProperty(...)")
            .strict_call(name)
            .unwrap()
    }

    fn set_game_property(&self, name: &str, value: impl mlua::IntoLua) {
        let game: Table = self.global("Game");
        let setter: Function = game.strict_get("SetProperty").unwrap();
        setter.call::<()>((game, name, value)).unwrap();
    }

    fn logs(&self) -> Vec<String> {
        self.global::<Table>("LOGS")
            .strict_sequence_values()
            .collect::<mlua::Result<_>>()
            .unwrap()
    }

    fn writes(&self) -> Vec<String> {
        self.global::<Table>("WRITES")
            .strict_sequence_values()
            .collect::<mlua::Result<_>>()
            .unwrap()
    }

    fn fixture(&self) {
        exec(&self.0, &read_repo(FIXTURE));
    }
}

fn field<T: FromLua>(table: &Table, key: &str) -> T {
    table.strict_get(key).unwrap()
}

#[test]
fn test_native_resolution_rejects_property_only_or_wrong_native_instance() {
    let h = Boundary::new();
    h.check("SETTLED", None);
    h.set("APPOINTED", false);
    h.error_check("SETTLED", None, "native governor not appointed");
    h.set("APPOINTED", true);
    h.global::<Table>("GOV").set("type", 77).unwrap();
    h.error_check("SETTLED", None, "native governor type mismatch");
    assert!(h.writes().is_empty());
}

#[test]
fn test_native_yields_effects_and_two_context_markers_are_in_same_record() {
    let h = Boundary::new();
    let s = h.read();
    let city: Table = field::<Table>(&s, "cities").strict_get("2/0").unwrap();
    assert_eq!(
        field::<Table>(&city, "yields")
            .strict_get::<f64>("YIELD_GOLD")
            .unwrap(),
        13.5
    );
    let node: String = h.global("NODE");
    assert_eq!(
        field::<Table>(&city, "effects")
            .strict_get::<i64>(node.as_str())
            .unwrap(),
        1
    );
    assert_eq!(
        field::<Table>(&city, "selectors")
            .strict_get::<i64>(node.as_str())
            .unwrap(),
        1
    );
    let gameplay: Table = field(&s, "gameplay");
    let target: Table = field::<Table>(&gameplay, "cities")
        .strict_get("2/0")
        .unwrap();
    assert_eq!(field::<i64>(&target, "settled"), 1);
    let native: Table = field(&s, "native");
    assert_eq!(
        (
            field::<i64>(&native, "type"),
            field::<i64>(&native, "neutralized"),
            field::<i64>(&s, "merit")
        ),
        (0, 0, 12)
    );
    h.case("merit_disagreement", ());
    h.error_check("SETTLED", None, "merit disagreement");
}

#[test]
fn test_native_success_with_captured_escape_is_still_a_neutralization() {
    let h = Boundary::new();
    h.call_global("stop", 3);
    h.case("mission", (2, 2, 5, 4));
    let s = h.read();
    assert!(field::<bool>(&s, "neutralizationMission"));
    let mission: Table = field::<Table>(&s, "missions").strict_get("1/0").unwrap();
    assert_eq!(
        (
            field::<i64>(&mission, "attacker"),
            field::<i64>(&mission, "escape")
        ),
        (1, 4)
    );
    h.check("NEUTRALIZED", Some(12));
    let events: Table = h.global("Events");
    h.global::<Function>("fire")
        .call::<()>((
            events.strict_get::<Table>("SpyMissionCompleted").unwrap(),
            1,
            0,
        ))
        .unwrap();
    assert!(h
        .logs()
        .iter()
        .any(|line| line.contains("MISSION_EVENT") && line.contains("attacker=1.000000")));
}

#[test]
fn test_failed_wrong_city_or_pre_fixture_mission_cannot_pass_neutralization() {
    let h = Boundary::new();
    h.call_global("stop", 3);
    for (initial, plot, turn) in [(3, 2, 5), (1, 12, 5), (1, 2, -1)] {
        h.case("mission", (initial, plot, turn, 0));
        h.error_check("NEUTRALIZED", None, "mission history was not observed");
    }
    assert!(h
        .logs()
        .iter()
        .any(|line| line.contains("CHECK_NEUTRALIZED")));
}

#[test]
fn test_missing_enemy_history_is_reported_and_cannot_pass_spy_check() {
    let h = Boundary::new();
    h.case("missing_history", ());
    let message: String = field::<Table>(&h.read(), "missions")
        .strict_get("1/unavailable")
        .unwrap();
    assert!(message.contains("unavailable"));
    h.error_check("NEUTRALIZED", None, "mission history was not observed");
    h.call_global("lose", ());
    h.check("LOST", Some(12));
}

#[test]
fn test_recovery_is_unassigned_and_never_an_automatic_return() {
    let h = Boundary::new();
    h.call_global("stop", 0);
    h.check("RECOVERED", Some(12));
    h.global::<Table>("GOV")
        .set("assigned", h.global::<Table>("TARGET"))
        .unwrap();
    h.error_check("RECOVERED", Some(12), "automatic return");
    h.check("ESTABLISHING", Some(12));
    h.global::<Table>("TARGET").set("marker", 1).unwrap();
    h.error_check("ESTABLISHING", None, "settlement marker");
}

#[test]
fn test_stale_settlement_active_skill_or_merit_is_rejected_after_loss() {
    let h = Boundary::new();
    h.call_global("lose", ());
    h.check("LOST", Some(12));
    h.global::<Table>("TARGET").set("marker", 1).unwrap();
    h.error_check("LOST", None, "settlement marker");
    h.global::<Table>("TARGET").set("marker", 0).unwrap();
    h.set("EFFECT_TARGET", true);
    h.error_check("LOST", None, "native city effect");
    h.set("EFFECT_TARGET", false);
    h.set_game_property(&format!("JAC_PERSONA_{LEADER}_MERIT"), 13);
    h.error_check("LOST", Some(12), "merit changed");
}

#[test]
fn test_unknown_native_effect_inventory_is_not_treated_as_zero_effects() {
    let h = Boundary::new();
    h.call_global("stop", 0);
    h.case("inventory_unavailable", ());
    h.error_check("RECOVERED", None, "inventory unavailable");
    h.case("activity_unavailable", ());
    h.error_check("RECOVERED", None, "activity unavailable");
}

#[test]
fn test_orphaned_native_subject_after_transfer_is_recorded_and_rejected() {
    let h = Boundary::new();
    h.case("orphan_effects", ());
    assert_eq!(field::<Table>(&h.read(), "unmatchedEffects").raw_len(), 3);
    h.error_check("LOST", None, "unmatched city subject");
    assert!(h
        .logs()
        .iter()
        .any(|line| line.contains("Old cached city name")));
}

#[test]
fn test_other_players_same_type_modifier_is_not_attributed_to_the_fixture() {
    let h = Boundary::new();
    h.case("other_player_effects", ());
    h.check("SETTLED", None);
    h.call_global("lose", ());
    h.case("foreign_active_only", ());
    h.check("LOST", Some(12));
}

#[test]
fn test_native_state_is_read_again_after_context_reload() {
    let h = Boundary::new();
    h.call_global("stop", 3);
    h.case("mission", (1, 2, 5, 0));
    let before = h.check("NEUTRALIZED", Some(12));
    exec(&h.0, &read_repo(VIEW));
    assert_eq!(h.check("NEUTRALIZED", Some(12)), before);
    h.global::<Table>("GOV").set("neutralized", 2).unwrap();
    assert_ne!(h.check("NEUTRALIZED", Some(12)), before);
    assert!(h.writes().is_empty());
}

#[test]
fn test_native_spy_travel_is_sent_once_from_an_ordinary_frame() {
    let h = Boundary::new();
    h.case("spy", (0, 15));
    h.queue("SPY_TRAVEL", 1, 20);
    assert_eq!(h.global::<Table>("UNIT_REQUESTS").raw_len(), 0);
    h.set("BUSY", true);
    h.frame().unwrap();
    assert_eq!(h.global::<Table>("UNIT_REQUESTS").raw_len(), 0);
    h.set("BUSY", false);
    h.frame().unwrap();
    let requests: Table = h.global("UNIT_REQUESTS");
    let request: Table = requests.strict_get(1).unwrap();
    let params: Table = field(&request, "params");
    assert_eq!(
        (
            field::<i64>(&request, "op"),
            field::<i64>(&params, "x"),
            field::<i64>(&params, "y")
        ),
        (991, 2, 1)
    );
    h.frame().unwrap();
    assert_eq!(requests.raw_len(), 1);
}

#[test]
fn test_foreign_destination_is_selected_by_native_travel_eligibility() {
    let h = Boundary::new();
    h.case("spy", (0, 15));
    let destination = strict_table_call(
        &h.b().strict_get::<Function>("FindSpyDestination").unwrap(),
        (),
    )
    .unwrap();
    assert_eq!(
        (
            field::<i64>(&destination, "owner"),
            field::<i64>(&destination, "city")
        ),
        (1, 20)
    );
    h.set("CAN_START", false);
    let error = h
        .b()
        .strict_get::<Function>("FindSpyDestination")
        .unwrap()
        .call::<MultiValue>(())
        .expect_err("must refuse");
    assert!(
        error
            .to_string()
            .contains("no native-eligible foreign spy destination"),
        "{error}"
    );
    assert_eq!(h.global::<Table>("UNIT_REQUESTS").raw_len(), 0);
}

#[test]
fn test_native_mission_refusal_and_enemy_control_are_not_replayed() {
    let h = Boundary::new();
    h.case("spy", (0, 15));
    let probe =
        strict_table_call(&h.b().strict_get::<Function>("Probe").unwrap(), (1, 20)).unwrap();
    assert_eq!(
        (
            field::<i64>(&probe, "turns"),
            field::<i64>(&probe, "travel"),
            field::<i64>(&probe, "establish")
        ),
        (8, 3, 4)
    );
    let query: Table = h.global("QUERY");
    assert_eq!(
        (field::<bool>(&query, "a"), field::<bool>(&query, "b")),
        (false, true)
    );
    h.queue("SPY_MISSION", 1, 20);
    let error = h.frame().expect_err("target governor absent");
    assert!(
        error.to_string().contains("target governor absent"),
        "{error}"
    );
    h.frame().unwrap();
    assert_eq!(h.global::<Table>("UNIT_REQUESTS").raw_len(), 0);
    h.case("spy", (1, 16));
    let error = h
        .b()
        .strict_get::<Function>("Probe")
        .unwrap()
        .call::<MultiValue>((1, 20))
        .expect_err("enemy spy must refuse");
    assert!(
        error.to_string().contains("no proven control path"),
        "{error}"
    );
}

#[test]
fn test_native_mission_request_is_not_itself_evidence_of_neutralization() {
    let h = Boundary::new();
    h.case("spy", (0, 15));
    h.case("target_governor", ());
    h.queue("SPY_MISSION", 1, 20);
    h.frame().unwrap();
    let requests: Table = h.global("UNIT_REQUESTS");
    let request: Table = requests.strict_get(1).unwrap();
    assert_eq!(field::<i64>(&request, "op"), 990);
    assert!(field::<Option<Table>>(&request, "params").is_none());
    h.frame().unwrap();
    assert_eq!(requests.raw_len(), 1);
    assert_eq!(
        field::<i64>(&field::<Table>(&h.read(), "native"), "neutralized"),
        0
    );
    h.error_check("NEUTRALIZED", None, "neutralization was not observed");
}

#[test]
fn test_missing_or_unidentified_modifier_definition_cannot_report_zero_effects() {
    for kind in ["nil", "empty", "false", "blank"] {
        let h = Boundary::new();
        h.case("bad_definition", kind);
        h.error_check("LOST", Some(12), "definition unavailable");
        assert!(h.writes().is_empty(), "{kind}");
    }
}

#[test]
fn test_loss_rejects_replacement_city_and_missing_or_wrong_transfer_identity() {
    for (kind, reason) in [
        ("city_id", "identity"),
        ("plot", "original plot missing"),
        ("missing", "identity"),
        ("from_id", "identity"),
    ] {
        let h = Boundary::new();
        h.case("bad_transfer", kind);
        h.error_check("LOST", Some(12), reason);
    }
}

#[test]
fn test_transfer_result_id_is_read_from_native_operation_not_inferred_later() {
    let h = Boundary::new();
    h.fixture();
    h.case("changed_transfer_result", ());
    let config: Table = h.game_property("JAC_BOUNDARY_CONFIG");
    let transfer: Table = field(&config, "transfer");
    assert_eq!(
        (
            field::<i64>(&transfer, "fromOwner"),
            field::<i64>(&transfer, "fromCityID"),
            field::<i64>(&transfer, "toOwner"),
            field::<i64>(&transfer, "toCityID")
        ),
        (0, 11, 62, 77)
    );
    h.check("LOST", Some(12));
    h.global::<Table>("TARGET").set("id", 78).unwrap();
    h.error_check("LOST", Some(12), "identity");
}

#[test]
fn test_independent_gameplay_city_identity_missing_or_changed_cannot_pass() {
    for (field_name, value) in [
        ("id", Some(78)),
        ("owner", Some(0)),
        ("id", None),
        ("owner", None),
    ] {
        let h = Boundary::new();
        h.case("gameplay_identity", (field_name, value));
        h.error_check("LOST", Some(12), "city identity disagreement");
    }
}

#[test]
fn test_gameplay_only_city_with_active_or_inactive_marker_is_rejected() {
    for marker in [0, 1] {
        let h = Boundary::new();
        h.case("gameplay_only_city", marker);
        h.error_check("LOST", Some(12), "Gameplay-only city observation");
    }
}

#[test]
fn test_fixture_prepare_uses_civic_completion_and_regular_join_with_noncapital_city() {
    let h = Boundary::new();
    h.fixture();
    h.case("prepare_inputs", 1);
    h.event("JAC_BoundaryPrepare").unwrap();
    let config: Table = h.game_property("JAC_BOUNDARY_CONFIG");
    assert_eq!(
        (
            field::<i64>(&config, "cityID"),
            field::<i64>(&config, "controlID"),
            field::<i64>(&config, "source"),
            field::<i64>(&config, "total"),
            field::<i64>(&config, "spent")
        ),
        (11, 10, 1, 1, 0)
    );
    assert_eq!(h.global::<i64>("JOIN_CALLS"), 1);
    assert_eq!(h.writes(), ["JAC_BOUNDARY_CONFIG"]);
}

#[test]
fn test_fixture_prepare_rejects_incorrect_native_title_grant_before_join() {
    let h = Boundary::new();
    h.fixture();
    h.case("prepare_inputs", 2);
    let error = h
        .event("JAC_BoundaryPrepare")
        .expect_err("two titles must refuse");
    assert!(error.to_string().contains("exactly one title"), "{error}");
    assert_eq!(h.global::<i64>("JOIN_CALLS"), 0);
    assert!(h.writes().is_empty());
}

#[test]
fn test_native_city_transfer_and_spy_creation_do_not_write_persona_state() {
    let h = Boundary::new();
    h.fixture();
    h.event("JAC_BoundaryLoseCity").unwrap();
    assert_eq!(
        (
            field::<i64>(&h.global::<Table>("TRANSFERRED"), "id"),
            field::<i64>(&h.global::<Table>("TARGET"), "owner")
        ),
        (11, 62)
    );
    h.check("LOST", Some(12));
    assert_eq!(h.writes(), ["JAC_BOUNDARY_CONFIG"]);
    h.event("JAC_BoundarySpawnSpy").unwrap();
    assert_eq!(
        field::<i64>(&h.game_property::<Table>("JAC_BOUNDARY_SPY"), "id"),
        15
    );
    assert_eq!(h.writes(), ["JAC_BOUNDARY_CONFIG", "JAC_BOUNDARY_SPY"]);
}

const PERSONA_MOD: &str = "poc/JAC_PoC_LeaderPersona";

fn load_persona_contexts(lua: &Lua) {
    for path in [
        "Shared/JAC_PersonaTreeRules.lua",
        "Scripts/JAC_LeaderPersona_Gameplay.lua",
        "UI/JAC_LeaderPersona_View.lua",
        "UI/JAC_LeaderPersona_Tree.lua",
    ] {
        exec(lua, &read_repo(format!("{PERSONA_MOD}/{path}")));
    }
}

fn inject_persona_tree_data(lua: &Lua) {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch("CREATE TABLE JAC_LeaderPersonas (LeaderType TEXT PRIMARY KEY);")
        .unwrap();
    let sql = read_repo(format!("{PERSONA_MOD}/Data/JAC_LeaderPersona_Trees.sql"));
    let people = sql
        .split_once("INSERT INTO JAC_PersonaTreePeople VALUES")
        .expect("tree people data")
        .1
        .split_once(';')
        .expect("tree people terminator")
        .0;
    let leader_pattern = Regex::new(r"\('(?P<leader>LEADER_JAC_[^']+)'\s*,").unwrap();
    let leaders = leader_pattern
        .captures_iter(people)
        .map(|capture| capture["leader"].to_owned())
        .collect::<Vec<_>>();
    assert_eq!(leaders.len(), 22);
    for leader in leaders {
        connection
            .execute(
                "INSERT INTO JAC_LeaderPersonas (LeaderType) VALUES (?1)",
                [&leader],
            )
            .unwrap();
    }
    let (tree_sql, _) = sql
        .split_once("-- All custom modifiers attach")
        .expect("tree data boundary");
    connection.execute_batch(tree_sql).unwrap();
    let game_info: Table = lua.globals().strict_get("GameInfo").unwrap();
    let collection: Function = lua.globals().strict_get("collection").unwrap();
    for (name, key) in [
        ("JAC_PersonaTreeNodes", "NodeType"),
        ("JAC_PersonaTreePeople", "LeaderType"),
        ("JAC_PersonaCategories", "CategoryType"),
        ("JAC_PersonaCategoryAccess", "AccessType"),
        ("JAC_PersonaRouteTemplates", "RouteType"),
        ("JAC_PersonaTreeParameters", "Name"),
        ("JAC_PersonaMeritRules", "Profile"),
    ] {
        let mut statement = connection
            .prepare(&format!("SELECT rowid, * FROM {name}"))
            .unwrap();
        let columns = statement
            .column_names()
            .iter()
            .map(|column| column.to_string())
            .collect::<Vec<_>>();
        let mut query = statement.query([]).unwrap();
        let lua_rows = lua.create_table().unwrap();
        let mut count = 0;
        while let Some(row) = query.next().unwrap() {
            let item = lua.create_table().unwrap();
            item.set("Index", row.get::<_, i64>(0).unwrap() - 1)
                .unwrap();
            for (index, column) in columns.iter().enumerate().skip(1) {
                let value = match row.get_ref(index).unwrap() {
                    ValueRef::Null => Value::Nil,
                    ValueRef::Integer(number) => Value::Integer(number),
                    ValueRef::Real(number) => Value::Number(number),
                    ValueRef::Text(text) => Value::String(lua.create_string(text).unwrap()),
                    ValueRef::Blob(_) => panic!("unexpected blob in {name}"),
                };
                if !matches!(value, Value::Nil) {
                    item.set(column.as_str(), value).unwrap();
                }
            }
            count += 1;
            lua_rows.set(count, item).unwrap();
        }
        assert!(count > 0, "expected {name} rows");
        game_info
            .set(
                name,
                strict_table_call(&collection, (lua_rows, key)).unwrap(),
            )
            .unwrap();
    }
    let buildings = lua.create_table().unwrap();
    for index in 0..=1 {
        let building = lua.create_table().unwrap();
        building.set("Index", index).unwrap();
        building
            .set("BuildingType", format!("BUILDING_TEST_{index}"))
            .unwrap();
        buildings.set(index + 1, building).unwrap();
    }
    game_info
        .set(
            "Buildings",
            strict_table_call(&collection, (buildings, "BuildingType")).unwrap(),
        )
        .unwrap();
    game_info
        .set(
            "JAC_PersonaDisabledGovernors",
            strict_table_call(&collection, (lua.create_table().unwrap(), "GovernorType")).unwrap(),
        )
        .unwrap();
}

fn merit(lua: &Lua) -> i64 {
    lua.load("return Game:GetProperty('JAC_PERSONA_'..DATE..'_MERIT')")
        .strict_eval()
        .unwrap()
}

#[test]
fn test_real_persona_gameplay_stops_all_four_merit_sources_at_each_boundary() {
    for phase in ["NEUTRALIZED", "RECOVERED", "ESTABLISHING", "LOST"] {
        let lua = new_lua();
        exec(&lua, include_str!("../fixtures/persona/stubs.lua"));
        inject_persona_tree_data(&lua);
        load_persona_contexts(&lua);
        exec(
            &lua,
            include_str!("../fixtures/persona/production_boundary.lua"),
        );
        let governor: Table = lua
            .load("return SETTLED_DATE()")
            .strict_eval()
            .unwrap_or_else(|error| panic!("{phase}: settle failed: {error}"));
        lua.globals()
            .strict_get::<Function>("SET_BOUNDARY_PHASE")
            .unwrap()
            .call::<()>((governor.clone(), phase))
            .unwrap();
        lua.globals()
            .strict_get::<Function>("FIRE_FOUR_MERIT_SOURCES")
            .unwrap()
            .call::<()>(())
            .unwrap_or_else(|error| panic!("{phase}: first merit events: {error}"));
        assert_eq!(merit(&lua), 0, "{phase} before reload");
        lua.globals()
            .strict_get::<Function>("RESET_CONTEXTS")
            .unwrap()
            .call::<()>(())
            .unwrap();
        load_persona_contexts(&lua);
        lua.globals().set("TURN", 2).unwrap();
        lua.globals()
            .strict_get::<Function>("FIRE_FOUR_MERIT_SOURCES")
            .unwrap()
            .call::<()>(())
            .unwrap_or_else(|error| panic!("{phase}: reloaded merit events: {error}"));
        assert_eq!(merit(&lua), 0, "{phase} after reload");
        lua.globals()
            .strict_get::<Function>("FIRE_POSITIVE_CONTROL")
            .unwrap()
            .call::<()>(governor)
            .unwrap_or_else(|error| panic!("{phase}: positive control: {error}"));
        assert_eq!(merit(&lua), 5, "{phase} positive control");
    }
}
