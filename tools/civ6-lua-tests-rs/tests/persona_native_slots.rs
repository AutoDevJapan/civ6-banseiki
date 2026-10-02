//! Test-only PR86 candidates. Lua fixtures exercise the diagnostic boundary;
//! they are not evidence that native Civ6 has published any receipt.

use civ6_lua_tests_rs::{new_lua, official, read_repo, repo_root};
use mlua::Lua;
use roxmltree::Document;
use rusqlite::Connection;
use std::collections::BTreeSet;

const TITLE: &str = "poc/JAC_Test_TitleProxy";
const SLOTS: &str = "poc/JAC_Test_NativeSlots";
const SOURCES: [&str; 6] = [
    "poc/JAC_Test_TitleProxy/Shared/JAC_TitlePaymentRules.lua",
    "poc/JAC_Test_TitleProxy/Scripts/JAC_TitleProxy_Probe.lua",
    "poc/JAC_Test_TitleProxy/UI/JAC_TitleProxy_View.lua",
    "poc/JAC_Test_TitleProxy/UI/JAC_TitleProxy_GovernorPanel.lua",
    "poc/JAC_Test_NativeSlots/Scripts/JAC_NativeSlots_GameCore.lua",
    "poc/JAC_Test_NativeSlots/UI/JAC_NativeSlots_View.lua",
];

fn fixture() -> Lua {
    let lua = new_lua();
    lua.load(include_str!("../fixtures/persona/native_slots.lua"))
        .set_name("native_slots.lua")
        .exec()
        .unwrap();
    for path in SOURCES {
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

fn reject(lua: &Lua, source: &str, reason: &str) {
    let error = lua.load(source).exec().expect_err("diagnostic must reject");
    assert!(error.to_string().contains(reason), "{error}");
}

fn title_fixture(lua: &Lua) {
    run(
        lua,
        "JAC_SlotsPrepare(); JAC_SlotsGiveTitles(); TURN=1; JAC_SlotsAssertAI(true)",
    );
}

fn appoint_both(lua: &Lua) {
    title_fixture(lua);
    run(
        lua,
        "JAC_SlotsAppoint('GOVERNOR_JAC_TITLE_PROXY'); JAC_SlotsUnassigned('PROXY_UNASSIGNED',1)",
    );
    run(
        lua,
        "JAC_SlotsAppoint('GOVERNOR_JAC_NATIVE_SLOT'); JAC_SlotsUnassigned('UNASSIGNED',2)",
    );
}

fn count(db: &Connection, query: &str) -> i64 {
    db.query_row(query, [], |row| row.get(0)).unwrap()
}

#[test]
fn test_diagnostic_mods_are_separate_map_scoped_packages() {
    for (root, manifest, id) in [
        (
            TITLE,
            "JAC_Test_TitleProxy.modinfo",
            "5d4393c0-c02a-48bf-83d2-7b6839d4e5cb",
        ),
        (
            SLOTS,
            "JAC_Test_NativeSlots.modinfo",
            "a1faab65-5371-4e25-9a46-7522075f865d",
        ),
    ] {
        let xml = read_repo(format!("{root}/{manifest}"));
        let document = Document::parse(&xml).unwrap();
        let mod_node = document.root_element();
        assert_eq!(mod_node.attribute("id"), Some(id));
        let criteria = mod_node
            .descendants()
            .find(|node| node.has_tag_name("ActionCriteria"))
            .unwrap();
        assert!(criteria
            .descendants()
            .any(|node| node.has_tag_name("RuleSetInUse")
                && node.text() == Some("RULESET_JAC_BANSEIKI")));
        assert!(criteria.descendants().any(|node| node.has_tag_name("Value")
            && node
                .text()
                .is_some_and(|text| text.contains("JAC_Japan_15km.Civ6Map"))));
        let files = mod_node
            .descendants()
            .find(|node| node.has_tag_name("Files"))
            .unwrap();
        let listed: BTreeSet<_> = files
            .children()
            .filter(|node| node.has_tag_name("File"))
            .map(|node| node.text().unwrap())
            .collect();
        assert!(!listed.is_empty());
        for file in listed {
            assert!(repo_root().join(root).join(file).is_file(), "{root}/{file}");
        }
        assert!(
            mod_node
                .descendants()
                .any(|node| node.has_tag_name("Dependency")
                    && node.attribute("id") == Some("91cf7b65-54bb-4f69-a452-19a65527cde2"))
                || mod_node.descendants().any(|node| node.has_tag_name("Mod")
                    && node.attribute("id") == Some("91cf7b65-54bb-4f69-a452-19a65527cde2"))
        );
    }
    let native = read_repo(format!("{SLOTS}/JAC_Test_NativeSlots.modinfo"));
    assert!(native.contains("5d4393c0-c02a-48bf-83d2-7b6839d4e5cb"));
}

#[test]
fn test_official_sql_keeps_production_personas_until_diagnostic_mods_are_loaded() {
    let db = official::official_db();
    db.execute_batch(&read_repo(
        "poc/JAC_PoC_CustomCivs/Data/JAC_CustomCivs_Gameplay.sql",
    ))
    .unwrap();
    db.execute_batch(&read_repo(
        "poc/JAC_PoC_CustomCivs/Data/JAC_Japan1534_Gameplay.sql",
    ))
    .unwrap();
    official::run_sengoku(&db);
    db.execute_batch(&read_repo(
        "poc/JAC_PoC_LeaderPersona/Data/JAC_LeaderPersona.sql",
    ))
    .unwrap();
    db.execute_batch(&read_repo(
        "poc/JAC_PoC_LeaderPersona/Data/JAC_LeaderPersona_Trees.sql",
    ))
    .unwrap();
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Governors WHERE GovernorType IN (SELECT GovernorType FROM JAC_LeaderPersonas)"), 22);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Types WHERE Kind='KIND_GOVERNOR' AND Type IN (SELECT GovernorType FROM JAC_LeaderPersonas)"), 22);
    let before_fk = count(&db, "SELECT COUNT(*) FROM pragma_foreign_key_check");
    db.execute_batch(&read_repo(format!("{TITLE}/Data/JAC_TitleProxy_Probe.sql")))
        .unwrap();
    db.execute_batch(&read_repo(format!(
        "{SLOTS}/Data/JAC_NativeSlots_Probe.sql"
    )))
    .unwrap();
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Governors WHERE GovernorType IN (SELECT GovernorType FROM JAC_LeaderPersonas)"), 0);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Types WHERE Kind='KIND_GOVERNOR' AND Type IN (SELECT GovernorType FROM JAC_LeaderPersonas)"), 0);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Governors WHERE GovernorType IN ('GOVERNOR_JAC_TITLE_PROXY','GOVERNOR_JAC_NATIVE_SLOT')"), 2);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM GovernorsCannotAssign WHERE GovernorType='GOVERNOR_JAC_TITLE_PROXY' AND CannotAssign=1"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM GovernorsCannotAssign WHERE GovernorType='GOVERNOR_JAC_NATIVE_SLOT'"), 0);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Governors WHERE GovernorType='GOVERNOR_JAC_NATIVE_SLOT' AND IdentityPressure=8 AND TransitionStrength=100 AND AssignCityState=0 AND TraitType IS NULL"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM GovernorPromotionModifiers WHERE GovernorPromotionType='GOVERNOR_PROMOTION_JAC_NATIVE_SLOT_BASE'"), 2);
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM JAC_TitleProxyReceipts"),
        66
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Modifiers WHERE ModifierId IN ('JAC_NATIVE_SLOT_PLAYER_RECEIPT','JAC_NATIVE_SLOT_CITY_ACTIVE')"), 2);
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM pragma_foreign_key_check"),
        before_fk
    );
}

#[test]
fn test_all_six_diagnostic_lua_sources_execute_in_lua_51() {
    let lua = fixture();
    run(&lua, "assert(type(JAC_TitlePaymentRules.Request)=='function'); assert(type(JAC_SlotsPrepare)=='function'); assert(#GameEvents.JAC_TitleProxyRequest.handlers==1); assert(#GameEvents.JAC_NativeSlotsGiveTitles.handlers==1)");
    run(&lua, "assert(JAC_ProxyRequest('LEADER_TEST_1')); assert(Players[0]:GetProperty('JAC_PROXY_REQUEST_RESULT')=='NOT_OWNED'); assert(Game:GetProperty('JAC_PROXY_PAYMENT_QUEUE_0')==nil)");
}

#[test]
fn test_diagnostic_lua_never_publishes_engine_receipts() {
    let scripts = SOURCES.map(read_repo).join("\n");
    for forbidden in [
        "SetProperty('JAC_NATIVE_SLOT_PLAYER_RECEIPT'",
        "SetProperty('JAC_NATIVE_SLOT_CITY_ACTIVE'",
        "SetProperty('JAC_PROXY_NATIVE_PAID_",
    ] {
        assert!(!scripts.contains(forbidden), "receipt writer: {forbidden}");
    }
    assert!(scripts.contains("player:GetProperty('JAC_NATIVE_SLOT_PLAYER_RECEIPT')"));
    assert!(scripts.contains("city:GetProperty('JAC_NATIVE_SLOT_CITY_ACTIVE')"));
    assert!(scripts.contains("player:GetProperty('JAC_PROXY_NATIVE_PAID_"));
}

#[test]
fn test_first_real_turn_requires_three_titles_and_no_ai_spending_or_appointment() {
    let lua = fixture();
    title_fixture(&lua);
    run(
        &lua,
        "for id=1,21 do assert(Players[id].gs.points==3 and Players[id].gs.spent==0) end",
    );
    run(&lua, "Players[7].gs.spent=1");
    reject(
        &lua,
        "JAC_SlotsAssertAI(true)",
        "unauthorized spending; AI=7",
    );
    run(&lua, "Players[7].gs.spent=0; Players[7].gs.native[301]={}");
    reject(
        &lua,
        "JAC_SlotsAssertAI(true)",
        "unauthorized spending; AI=7",
    );
    run(
        &lua,
        "Players[7].gs.native[301]=nil; Players[7].gs.points=2",
    );
    reject(
        &lua,
        "JAC_SlotsAssertAI(true)",
        "unauthorized spending; AI=7",
    );
    run(&lua, "Players[7].gs.points=3; TURN=2");
    reject(
        &lua,
        "JAC_SlotsAssertAI(true)",
        "first native AI turn was not exactly one turn",
    );
}

#[test]
fn test_type_specific_appointment_getter_is_independent_of_persona_ui() {
    let lua = fixture();
    title_fixture(&lua);
    run(&lua, "Players[0].gs.CanAppoint=function() return true end; Players[0].gs.CanAppointGovernor=function(_,hash) assert(hash==301); return false end");
    reject(
        &lua,
        "JAC_SlotsAppoint('GOVERNOR_JAC_NATIVE_SLOT')",
        "native appointment denied",
    );
    run(
        &lua,
        "assert(#OPERATIONS==1); assert(OPERATIONS[1].operation=='EXECUTE_SCRIPT')",
    );
}

#[test]
fn test_unassigned_receipt_cannot_be_inferred_from_official_spending_or_diagnostic_property() {
    let lua = fixture();
    appoint_both(&lua);
    reject(
        &lua,
        "JAC_SlotsRequireUnassignedReceipt()",
        "UNASSIGNED_RECEIPT_MISSING coreSlot=nil coreProxy=nil",
    );
    run(&lua, "Game:SetProperty('JAC_NATIVE_SLOT_PLAYER_RECEIPT',1); Players[0].props.JAC_NATIVE_SLOT_PLAYER_RECEIPT=1; Players[0].props.JAC_PROXY_NATIVE_PAID_0=1");
    reject(
        &lua,
        "JAC_SlotsRequireUnassignedReceipt()",
        "UNASSIGNED_RECEIPT_MISSING coreSlot=nil coreProxy=nil",
    );
    run(
        &lua,
        "Game:SetProperty('JAC_SLOTS_PROBE_ASSIGNED',{slotReceipt=1})",
    );
    reject(
        &lua,
        "JAC_SlotsRequireUnassignedReceipt()",
        "SLOT_RECEIPT_AFTER_ASSIGNMENT",
    );
}

#[test]
fn test_city_receipt_requires_real_five_turn_clock_and_reload_equality() {
    let lua = fixture();
    appoint_both(&lua);
    run(&lua, "Players[0].props.JAC_NATIVE_SLOT_PLAYER_RECEIPT=1; Players[0].props.JAC_PROXY_NATIVE_PAID_0=1; JAC_SlotsObserve('UNASSIGNED'); JAC_SlotsRequireUnassignedReceipt(); JAC_SlotsAssign(); JAC_SlotsCheckCity('ASSIGNED',0)");
    run(
        &lua,
        "TURN=5; Players[0].gs.native[301].site=4; JAC_SlotsCheckCity('TURN_FOUR',4)",
    );
    run(
        &lua,
        "TURN=6; Players[0].gs.native[301].site=5; Players[0].gs.native[301].established=true",
    );
    reject(
        &lua,
        "JAC_SlotsCheckCity('SETTLED',5)",
        "city property does not agree with native establishment",
    );
    run(&lua, "CAPITAL.props.JAC_NATIVE_SLOT_CITY_ACTIVE=1; JAC_SlotsCheckCity('SETTLED',5); JAC_SlotsCheckReload('SETTLED')");
    run(&lua, "Players[0].gs.spent=3");
    reject(
        &lua,
        "JAC_SlotsCheckReload('SETTLED')",
        "native reload differs: spent",
    );
}
