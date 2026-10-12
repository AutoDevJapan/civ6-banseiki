//! PR86 diagnostic only. These Lua 5.1 executions do not constitute live Civ6 evidence.
use civ6_lua_tests_rs::{new_lua, official, read_repo, repo_root};
use mlua::Lua;
use roxmltree::Document;
use rusqlite::Connection;

const MOD: &str = "poc/JAC_Test_StaticPersonaGate";
const SQL: &str = "poc/JAC_Test_StaticPersonaGate/Data/JAC_StaticPersonaGate.sql";
const GAMEPLAY: &str = "poc/JAC_Test_StaticPersonaGate/Scripts/JAC_StaticPersonaGate_Gameplay.lua";
const VIEW: &str = "poc/JAC_Test_StaticPersonaGate/UI/JAC_StaticPersonaGate_View.lua";

fn count(db: &Connection, query: &str) -> i64 {
    db.query_row(query, [], |row| row.get(0)).unwrap()
}

fn lua() -> Lua {
    let lua = new_lua();
    lua.load(include_str!("../fixtures/persona/static_gate.lua"))
        .exec()
        .unwrap();
    for path in [GAMEPLAY, VIEW] {
        lua.load(read_repo(path))
            .set_name(path)
            .exec()
            .unwrap_or_else(|error| panic!("{path}: {error}"));
    }
    lua
}

fn run(lua: &Lua, source: &str) {
    lua.load(source).exec().unwrap();
}

fn ui(lua: &Lua, source: &str) {
    run(lua, &format!("UI_CALL(function() {source} end)"));
}

fn reject(lua: &Lua, source: &str, fragment: &str) {
    let error = lua.load(source).exec().expect_err("must fail closed");
    assert!(error.to_string().contains(fragment), "{error}");
}

fn ai_finished(lua: &Lua) {
    ui(lua, "assert(JAC_StaticGate.AssertClosed(false))");
    ui(lua, "JAC_StaticGate.Queue('AI_TITLES')");
    run(
        lua,
        "BUSY=true; FRAME(); assert(#REQUESTS==0); BUSY=false; FRAME(); FLUSH(); TURN=1",
    );
    ui(lua, "assert(JAC_StaticGate.AssertAI())");
}

fn requested(lua: &Lua) {
    ai_finished(lua);
    ui(lua, "JAC_StaticGate.Queue('PREPARE_HUMAN')");
    run(lua, "FRAME(); FLUSH(); assert(JOIN_CALLS==1)");
    ui(lua, "assert(JAC_StaticGate.Ready()==0)");
    ui(lua, "JAC_StaticGate.Queue('APPOINT')");
    run(lua, "FRAME(); assert(#REQUESTS==1 and REQUESTS[1].kind=='APPOINT_GOVERNOR'); assert(REQUESTS[1].params.PARAM_GOVERNOR_TYPE==204)");
}

#[test]
fn test_test_only_package_is_map_scoped_and_wires_each_source() {
    let xml = read_repo(format!("{MOD}/JAC_Test_StaticPersonaGate.modinfo"));
    let document = Document::parse(&xml).unwrap();
    let root = document.root_element();
    assert_eq!(
        root.attribute("id"),
        Some("c678a216-58ab-4d40-a614-8f309d36ac86")
    );
    assert!(document.descendants().any(|node| node.has_tag_name("Mod")
        && node.attribute("id") == Some("91cf7b65-54bb-4f69-a452-19a65527cde2")));
    let criteria = document
        .descendants()
        .find(|node| node.has_tag_name("ActionCriteria"))
        .unwrap();
    assert!(criteria.descendants().any(
        |node| node.has_tag_name("RuleSetInUse") && node.text() == Some("RULESET_JAC_BANSEIKI")
    ));
    assert!(criteria.descendants().any(|node| node.has_tag_name("Value")
        && node
            .text()
            .is_some_and(|text| text.contains("JAC_Japan_15km.Civ6Map"))));
    let files: Vec<_> = document
        .descendants()
        .find(|node| node.has_tag_name("Files"))
        .unwrap()
        .children()
        .filter(|node| node.has_tag_name("File"))
        .map(|node| node.text().unwrap())
        .collect();
    assert_eq!(files.len(), 4);
    for file in files {
        assert!(repo_root().join(MOD).join(file).is_file(), "{file}");
        assert!(document.descendants().any(|node| node.has_tag_name("File")
            && node.text() == Some(file)
            && node
                .ancestors()
                .any(|parent| parent.has_tag_name("InGameActions"))));
    }
}

#[test]
fn test_official_schema_closes_exactly_22_production_personas_without_granting_trait() {
    let db = official::official_db();
    // The archived official snapshot predates the InsertOrIgnore in Agendas.xml.
    db.execute_batch("INSERT OR IGNORE INTO Kinds (Kind) VALUES ('KIND_TRAIT');")
        .unwrap();
    for sql in [
        "poc/JAC_PoC_CustomCivs/Data/JAC_CustomCivs_Gameplay.sql",
        "poc/JAC_PoC_CustomCivs/Data/JAC_Japan1534_Gameplay.sql",
    ] {
        db.execute_batch(&read_repo(sql)).unwrap();
    }
    official::run_sengoku(&db);
    for sql in [
        "poc/JAC_PoC_LeaderPersona/Data/JAC_LeaderPersona.sql",
        "poc/JAC_PoC_LeaderPersona/Data/JAC_LeaderPersona_Trees.sql",
    ] {
        db.execute_batch(&read_repo(sql)).unwrap();
    }
    assert_eq!(count(&db, "SELECT COUNT(*) FROM JAC_LeaderPersonas"), 22);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Governors WHERE GovernorType IN (SELECT GovernorType FROM JAC_LeaderPersonas) AND TraitType='TRAIT_JAC_PERSONA_ADMISSION_CLOSED'"), 22);
    let before_fk = count(&db, "SELECT COUNT(*) FROM pragma_foreign_key_check");
    db.execute_batch(&read_repo(SQL)).unwrap();
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Types WHERE Type='TRAIT_JAC_TEST_ALL_PERSONAS_CLOSED' AND Kind='KIND_TRAIT'"), 1);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM Traits WHERE TraitType='TRAIT_JAC_TEST_ALL_PERSONAS_CLOSED'"
        ),
        1
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Governors WHERE GovernorType IN (SELECT GovernorType FROM JAC_LeaderPersonas) AND TraitType='TRAIT_JAC_TEST_ALL_PERSONAS_CLOSED'"), 22);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Governors WHERE GovernorType NOT IN (SELECT GovernorType FROM JAC_LeaderPersonas) AND TraitType='TRAIT_JAC_TEST_ALL_PERSONAS_CLOSED'"), 0);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM CivilizationTraits WHERE TraitType='TRAIT_JAC_TEST_ALL_PERSONAS_CLOSED'"), 0);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM LeaderTraits WHERE TraitType='TRAIT_JAC_TEST_ALL_PERSONAS_CLOSED'"), 0);
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM pragma_foreign_key_check"),
        before_fk
    );
}

#[test]
fn test_visible_frame_grants_real_civic_budgets_and_checks_first_ai_turn() {
    let lua = lua();
    ai_finished(&lua);
    run(&lua, "for i=1,21 do assert(Players[i].points==3 and Players[i].spent==0) end; assert(JAC_StaticGate.aiChecked)");
}

#[test]
fn test_484_native_getters_reject_a_single_open_ai_candidate() {
    let lua = lua();
    run(&lua, "EVER[7]={[104]=true,[204]=true}");
    reject(
        &lua,
        "UI_CALL(function() JAC_StaticGate.AssertClosed(false) end)",
        "native eligibility open: 7/LEADER_JAC_ODA_NOBUHIDE",
    );
}

#[test]
fn test_joined_person_one_visible_frame_appointment_and_native_accounting() {
    let lua = lua();
    requested(&lua);
    run(&lua, "FLUSH(); assert(Players[0].spent==1)");
    ui(&lua, "JAC_StaticGate.Queue('BARRIER')");
    run(&lua, "FRAME(); FLUSH()");
    ui(
        &lua,
        "assert(string.find(JAC_StaticGate.Outcome(),'DIAGNOSTIC_ACCEPTED'))",
    );
    run(&lua, "assert(Players[0].points==1 and JOIN_CALLS==1 and Game:GetProperty('JAC_PERSONA_LEADER_JAC_ODA_NOBUHIDE_STATE')=='GREAT_PERSON')");
}

#[test]
fn test_refused_native_request_stays_failed_and_cannot_be_replayed() {
    let lua = lua();
    requested(&lua);
    run(&lua, "ACCEPT=false; FLUSH()");
    ui(&lua, "JAC_StaticGate.Queue('BARRIER')");
    run(&lua, "FRAME(); FLUSH()");
    reject(
        &lua,
        "UI_CALL(function() JAC_StaticGate.Outcome() end)",
        "REQUEST_REJECTED_OR_UNOBSERVED",
    );
    reject(
        &lua,
        "UI_CALL(function() JAC_StaticGate.Queue('APPOINT') end)",
        "already queued",
    );
    run(&lua, "assert(Players[0].spent==0 and JOIN_CALLS==1)");
}

#[test]
fn test_unknown_barrier_cannot_count_as_native_acceptance() {
    let lua = lua();
    requested(&lua);
    run(&lua, "FLUSH()");
    reject(
        &lua,
        "UI_CALL(function() JAC_StaticGate.Outcome() end)",
        "RESPONSE_UNKNOWN",
    );
    reject(
        &lua,
        "UI_CALL(function() JAC_StaticGate.Queue('APPOINT') end)",
        "already queued",
    );
}

#[test]
fn test_diagnostic_sources_do_not_forge_persona_accounting_or_spy_state() {
    let source = [read_repo(GAMEPLAY), read_repo(VIEW)].join("\n");
    for forbidden in [
        "SetProperty('JAC_PERSONA_",
        "SetGovernorPoints",
        "SetGovernorPointsSpent",
        "SetProperty('JAC_BOUNDARY_SPY",
        "CanEverAppointGovernor =",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden diagnostic write: {forbidden}"
        );
    }
    assert!(source.contains("ExposedMembers.JAC_LeaderPersona.Join(owner, source)"));
    assert!(source.contains("PlayerOperations.APPOINT_GOVERNOR"));
}
