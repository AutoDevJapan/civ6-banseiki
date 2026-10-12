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

fn ui(lua: &Lua, source: &str) {
    run(lua, &format!("UI_CALL(function() {source} end)"));
}

fn reject(lua: &Lua, source: &str, reason: &str) {
    let error = lua.load(source).exec().expect_err("diagnostic must reject");
    assert!(error.to_string().contains(reason), "{error}");
}

fn reject_ui(lua: &Lua, source: &str, reason: &str) {
    reject(lua, &format!("UI_CALL(function() {source} end)"), reason);
}

fn title_fixture(lua: &Lua) {
    ui(lua, "JAC_SlotsPrepare()");
    run(lua, "FLUSH_ALL()");
    ui(lua, "JAC_SlotsGiveTitles()");
    run(lua, "FLUSH_ALL(); TURN=1");
    ui(lua, "JAC_SlotsAssertAI(true)");
}

fn appoint_both(lua: &Lua) {
    title_fixture(lua);
    ui(
        lua,
        "JAC_SlotsAppoint('GOVERNOR_JAC_TITLE_PROXY'); JAC_SlotsUnassigned('PROXY_UNASSIGNED',1)",
    );
    run(lua, "FLUSH_ALL()");
    ui(
        lua,
        "JAC_SlotsAppoint('GOVERNOR_JAC_NATIVE_SLOT'); JAC_SlotsUnassigned('UNASSIGNED',2)",
    );
    run(lua, "FLUSH_ALL()");
}

fn appoint_both_with_mock_receipts(lua: &Lua) {
    title_fixture(lua);
    ui(lua, "JAC_SlotsAppoint('GOVERNOR_JAC_TITLE_PROXY')");
    run(lua, "Players[0].props.JAC_PROXY_NATIVE_PAID_0=1");
    ui(lua, "JAC_SlotsUnassigned('PROXY_UNASSIGNED',1)");
    run(lua, "FLUSH_ALL()");
    ui(lua, "JAC_SlotsAppoint('GOVERNOR_JAC_NATIVE_SLOT')");
    run(lua, "Players[0].props.JAC_NATIVE_SLOT_PLAYER_RECEIPT=1");
    ui(lua, "JAC_SlotsUnassigned('UNASSIGNED',2)");
    run(lua, "FLUSH_ALL()");
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
        (
            "poc/JAC_Test_NativeSlotOnly",
            "JAC_Test_NativeSlotOnly.modinfo",
            "8c615430-5a09-4431-877a-bfd03bca9e80",
        ),
        (
            "poc/JAC_Test_StaticPersonaGate",
            "JAC_Test_StaticPersonaGate.modinfo",
            "c678a216-58ab-4d40-a614-8f309d36ac86",
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
    // The archived snapshot predates the loader's InsertOrIgnore support.
    // Base/Assets/Gameplay/Data/Agendas.xml declares this official kind;
    // persona.rs applies the same seed when rebuilding the database from assets.
    db.execute_batch("INSERT OR IGNORE INTO Kinds (Kind) VALUES ('KIND_TRAIT');")
        .unwrap();
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
    let isolated = read_repo("poc/JAC_Test_NativeSlotOnly/Data/JAC_NativeSlotOnly.sql");
    for statement in isolated.split(';').filter(|sql| !sql.trim().is_empty()) {
        db.execute_batch(statement)
            .unwrap_or_else(|error| panic!("isolated slot SQL: {statement}: {error}"));
    }
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Governors WHERE GovernorType='GOVERNOR_JAC_TITLE_PROXY' AND TraitType='TRAIT_JAC_TEST_PROXY_DISABLED'"), 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM CivilizationTraits WHERE TraitType='TRAIT_JAC_TEST_PROXY_DISABLED'"), 0);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM LeaderTraits WHERE TraitType='TRAIT_JAC_TEST_PROXY_DISABLED'"
        ),
        0
    );
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Governors WHERE GovernorType='GOVERNOR_JAC_NATIVE_SLOT' AND IdentityPressure=8 AND TransitionStrength=100 AND TraitType IS NULL"), 1);
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM pragma_foreign_key_check"),
        before_fk
    );
}

#[test]
fn test_all_six_diagnostic_lua_sources_execute_in_lua_51() {
    let lua = fixture();
    run(&lua, "assert(type(JAC_TitlePaymentRules.Request)=='function'); assert(type(JAC_SlotsPrepare)=='function'); assert(#GameEvents.JAC_TitleProxyRequest.handlers==1); assert(#GameEvents.JAC_NativeSlotsGiveTitles.handlers==1); assert(#GameEvents.JAC_NativeSlotsSaveObservation.handlers==1)");
    ui(&lua, "assert(JAC_ProxyRequest('LEADER_TEST_1'))");
    run(&lua, "assert(Players[0]:GetProperty('JAC_PROXY_REQUEST_RESULT')==nil); FLUSH_ALL(); assert(Players[0]:GetProperty('JAC_PROXY_REQUEST_RESULT')=='NOT_OWNED'); assert(Game:GetProperty('JAC_PROXY_PAYMENT_QUEUE_0')==nil)");
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
    let ui_source = read_repo("poc/JAC_Test_NativeSlots/UI/JAC_NativeSlots_View.lua");
    assert!(!ui_source.contains("Game:SetProperty"));
}

#[test]
fn test_ui_capture_queues_only_scalars_and_gamecore_saves_actual_receipts() {
    let lua = fixture();
    reject(
        &lua,
        "UI_CALL(function() Game:SetProperty('JAC_SLOTS_PROBE_FORBIDDEN',1) end)",
        "UI Game:SetProperty is unavailable",
    );
    title_fixture(&lua);
    ui(&lua, "JAC_SlotsAppoint('GOVERNOR_JAC_TITLE_PROXY')");
    ui(&lua, "JAC_SlotsUnassigned('PROXY_UNASSIGNED',1)");
    run(&lua, "assert(Game:GetProperty('JAC_SLOTS_PROBE_PROXY_UNASSIGNED')==nil); assert(#SYNC_QUEUE==1); local p=SYNC_QUEUE[1].params; assert(p.OnStart=='JAC_NativeSlotsSaveObservation' and p.Phase=='PROXY_UNASSIGNED' and p.hasProxy==true); for _,v in pairs(p) do assert(type(v)~='table') end; assert(Players[0]:GetProperty('JAC_PROXY_NATIVE_PAID_0')==nil)");
    run(&lua, "Players[0].props.JAC_PROXY_NATIVE_PAID_0=1; CAPITAL.props.JAC_NATIVE_SLOT_CITY_ACTIVE=1; FLUSH_ONE(); local s=Game:GetProperty('JAC_SLOTS_PROBE_PROXY_UNASSIGNED'); assert(s.proxyReceipt==1 and s.slotReceipt==nil and s.hasProxy==true and s.cities[1].receipt==1); assert(Players[0]:GetProperty('JAC_PROXY_NATIVE_PAID_0')==1 and CAPITAL:GetProperty('JAC_NATIVE_SLOT_CITY_ACTIVE')==1)");
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
    reject_ui(
        &lua,
        "JAC_SlotsAssertAI(true)",
        "unauthorized spending; AI=7",
    );
    run(&lua, "Players[7].gs.spent=0; Players[7].gs.native[301]={}");
    reject_ui(
        &lua,
        "JAC_SlotsAssertAI(true)",
        "unauthorized spending; AI=7",
    );
    run(
        &lua,
        "Players[7].gs.native[301]=nil; Players[7].gs.points=2",
    );
    reject_ui(
        &lua,
        "JAC_SlotsAssertAI(true)",
        "unauthorized spending; AI=7",
    );
    run(&lua, "Players[7].gs.points=3; TURN=2");
    reject_ui(
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
    reject_ui(
        &lua,
        "JAC_SlotsAppoint('GOVERNOR_JAC_NATIVE_SLOT')",
        "native appointment denied",
    );
    run(
        &lua,
        "assert(#OPERATIONS==2); assert(OPERATIONS[1].operation=='EXECUTE_SCRIPT' and OPERATIONS[2].operation=='EXECUTE_SCRIPT')",
    );
}

#[test]
fn test_unassigned_receipt_cannot_be_inferred_from_official_spending_or_diagnostic_property() {
    let lua = fixture();
    appoint_both(&lua);
    reject_ui(
        &lua,
        "JAC_SlotsRequireUnassignedReceipt()",
        "UNASSIGNED_RECEIPT_MISSING coreSlot=nil coreProxy=nil",
    );
    run(&lua, "Game:SetProperty('JAC_NATIVE_SLOT_PLAYER_RECEIPT',1); Players[0].props.JAC_NATIVE_SLOT_PLAYER_RECEIPT=1; Players[0].props.JAC_PROXY_NATIVE_PAID_0=1");
    reject_ui(
        &lua,
        "JAC_SlotsRequireUnassignedReceipt()",
        "UNASSIGNED_RECEIPT_MISSING coreSlot=nil coreProxy=nil",
    );
    run(
        &lua,
        "Game:SetProperty('JAC_SLOTS_PROBE_ASSIGNED',{slotReceipt=1})",
    );
    reject_ui(
        &lua,
        "JAC_SlotsRequireUnassignedReceipt()",
        "SLOT_RECEIPT_AFTER_ASSIGNMENT",
    );
}

#[test]
fn test_missing_unassigned_snapshot_is_not_rewritten_by_later_mock_activation() {
    let lua = fixture();
    appoint_both(&lua);
    run(&lua, "Players[0].props.JAC_NATIVE_SLOT_PLAYER_RECEIPT=1; Players[0].props.JAC_PROXY_NATIVE_PAID_0=1");
    ui(&lua, "JAC_SlotsMarkAssignment()");
    run(&lua, "FLUSH_ALL()");
    ui(&lua, "JAC_SlotsAssign(); JAC_SlotsCheckCity('ASSIGNED',0)");
    run(&lua, "assert(Game:GetProperty('JAC_SLOTS_PROBE_ASSIGNED')==nil); FLUSH_ALL(); local original=Game:GetProperty('JAC_SLOTS_PROBE_UNASSIGNED'); local later=Game:GetProperty('JAC_SLOTS_PROBE_ASSIGNED'); assert(original.slotReceipt==nil and original.proxyReceipt==nil); assert(later.slotReceipt==1 and later.proxyReceipt==1)");
    reject_ui(
        &lua,
        "JAC_SlotsRequireUnassignedReceipt()",
        "SLOT_RECEIPT_AFTER_ASSIGNMENT",
    );
    reject_ui(
        &lua,
        "JAC_SlotsCheckReload('UNASSIGNED')",
        "native reload differs: slotReceipt",
    );
}

#[test]
fn test_native_scenarios_wait_for_synchronized_saves_and_keep_receipt_gates() {
    for path in [
        "tools/civ6-autotest/scenarios/leader-persona-native-slots-15km.json",
        "tools/civ6-autotest/scenarios/leader-persona-native-slots-activation-15km.json",
    ] {
        let scenario: serde_json::Value = serde_json::from_str(&read_repo(path)).unwrap();
        let steps = scenario["steps"].as_array().unwrap();
        for (call, property) in [
            ("JAC_SlotsPrepare()", "JAC_SLOTS_PROBE_BEFORE_TITLES"),
            (
                "JAC_SlotsUnassigned('PROXY_UNASSIGNED',1)",
                "JAC_SLOTS_PROBE_PROXY_UNASSIGNED",
            ),
            (
                "JAC_SlotsUnassigned('UNASSIGNED',2)",
                "JAC_SLOTS_PROBE_UNASSIGNED",
            ),
            ("JAC_SlotsMarkAssignment()", "JAC_SLOTS_PROBE_CITY"),
            (
                "JAC_SlotsCheckCity('ASSIGNED',0)",
                "JAC_SLOTS_PROBE_ASSIGNED",
            ),
            (
                "JAC_SlotsCheckCity('TURN_FOUR',4)",
                "JAC_SLOTS_PROBE_TURN_FOUR",
            ),
            ("JAC_SlotsCheckCity('SETTLED',5)", "JAC_SLOTS_PROBE_SETTLED"),
        ] {
            let index = steps
                .iter()
                .position(|step| {
                    step["action"] == "ui_lua" && step["code"].as_str().unwrap().contains(call)
                })
                .unwrap_or_else(|| panic!("{path}: missing {call}"));
            assert_eq!(steps[index + 1]["action"], "wait_for", "{path}: {call}");
            assert!(
                steps[index + 1]["condition"]["code"]
                    .as_str()
                    .unwrap()
                    .contains(property),
                "{path}: {call} has no completion check"
            );
        }
        let codes: Vec<_> = steps
            .iter()
            .filter_map(|step| step["code"].as_str())
            .collect();
        assert!(codes.contains(&"return JAC_SlotsAssertAI(true)"), "{path}");
        assert!(
            codes.contains(&"return JAC_SlotsRequireUnassignedReceipt()")
                || codes
                    .iter()
                    .any(|code| code.contains("JAC_SlotsRequireUnassignedReceipt();")),
            "{path}"
        );
        assert!(
            steps
                .iter()
                .all(|step| !step["code"].as_str().is_some_and(|code| {
                    code.contains("SetProperty('JAC_NATIVE_SLOT_")
                        || code.contains("SetProperty('JAC_PROXY_NATIVE_PAID_")
                })),
            "{path}: scenario fabricates a receipt"
        );
    }
}

#[test]
fn test_city_receipt_requires_real_five_turn_clock_and_reload_equality() {
    let lua = fixture();
    appoint_both_with_mock_receipts(&lua);
    ui(
        &lua,
        "JAC_SlotsRequireUnassignedReceipt(); JAC_SlotsMarkAssignment()",
    );
    run(&lua, "assert(Game:GetProperty('JAC_SLOTS_PROBE_CITY')==nil and Game:GetProperty('JAC_SLOTS_PROBE_ASSIGN_TURN')==nil)");
    reject_ui(
        &lua,
        "JAC_SlotsAssign()",
        "Gameplay assignment marker missing or stale",
    );
    run(
        &lua,
        "local gs=Players[0].gs; local has,get,can=gs.HasGovernor,gs.GetGovernor,gs.CanAssignGovernor; \
         gs.HasGovernor=nil; gs.GetGovernor=nil; gs.CanAssignGovernor=nil; \
         FLUSH_ALL(); assert(Game:GetProperty('JAC_SLOTS_PROBE_CITY')==CAPITAL:GetID()); \
         gs.HasGovernor=has; gs.GetGovernor=get; gs.CanAssignGovernor=can",
    );
    ui(&lua, "JAC_SlotsAssign(); JAC_SlotsCheckCity('ASSIGNED',0)");
    run(
        &lua,
        "FLUSH_ALL(); TURN=5; Players[0].gs.native[301].site=4",
    );
    ui(&lua, "JAC_SlotsCheckCity('TURN_FOUR',4)");
    run(&lua, "FLUSH_ALL()");
    run(
        &lua,
        "TURN=6; Players[0].gs.native[301].site=5; Players[0].gs.native[301].established=true",
    );
    reject_ui(
        &lua,
        "JAC_SlotsCheckCity('SETTLED',5)",
        "city property does not agree with native establishment",
    );
    run(&lua, "assert(Game:GetProperty('JAC_SLOTS_PROBE_SETTLED')==nil); CAPITAL.props.JAC_NATIVE_SLOT_CITY_ACTIVE=1");
    ui(&lua, "JAC_SlotsCheckCity('SETTLED',5)");
    run(&lua, "FLUSH_ALL()");
    ui(&lua, "JAC_SlotsCheckReload('SETTLED')");
    run(&lua, "Players[0].gs.spent=3");
    reject_ui(
        &lua,
        "JAC_SlotsCheckReload('SETTLED')",
        "native reload differs: spent",
    );
}
