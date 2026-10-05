use civ6_lua_tests_rs::{
    assassin_panel::{Counts, PanelWorld, PANEL},
    read_repo,
};
use mlua::{Function, Table};

fn zero() -> Counts {
    Counts {
        state_reads: 0,
        candidate_resolutions: 0,
        unit_classifications: 0,
        layouts: 0,
    }
}

#[test]
fn hidden_panel_skips_all_update_sources_and_close_paths_with_5000_units() {
    let world = PanelWorld::new(&read_repo(PANEL), 5000);
    assert_eq!(world.counts.snapshot(), zero(), "initially hidden");
    assert!(world.rows().is_empty());
    for event in [
        "GameCoreEventPublishComplete",
        "UnitSelectionChanged",
        "LocalPlayerTurnBegin",
        "LoadScreenClose",
    ] {
        for _ in 0..300 {
            world.event(event).call::<()>(()).unwrap();
        }
    }
    assert_eq!(world.counts.snapshot(), zero());
    world.click("OpenButton");
    assert!(world.counts.snapshot().candidate_resolutions >= 5000);
    let shown = world.rows();
    assert!(shown.len() >= 5000);
    world.counts.reset();
    world.click("OpenButton"); // launcher also closes the panel
    world
        .event("GameCoreEventPublishComplete")
        .call::<()>(())
        .unwrap();
    assert_eq!(world.counts.snapshot(), zero());
    assert_eq!(world.rows(), shown);
    world.click("OpenButton");
    world.counts.reset();
    world.click("CloseButton");
    world.event("UnitSelectionChanged").call::<()>(()).unwrap();
    assert_eq!(world.counts.snapshot(), zero());
}

#[test]
fn reopen_reads_new_targets_and_training_after_300_hidden_notifications() {
    let world = PanelWorld::new(&read_repo(PANEL), 100);
    world.click("OpenButton");
    let initial = world.rows();
    world.click("CloseButton");
    let grant: Function = world.lua.globals().get("GRANT").unwrap();
    let _: Table = grant.call((3, 4, 2, 0)).unwrap();
    world
        .lua
        .load("Players[0]:SetProperty(A.STATE,{Production={City=10,Due=NOW+20}})")
        .exec()
        .unwrap();
    let hidden = world.sample(300);
    assert_eq!(hidden.counts, zero());
    assert_eq!(world.rows(), initial);
    world.click("OpenButton");
    let reopened = world.rows();
    assert_eq!(reopened[0].1, "LOC_JAC_ASN_CANCEL");
    assert_eq!(
        reopened
            .iter()
            .filter(|row| row.1 == "LOC_JAC_ASN_ATTEMPT")
            .count(),
        initial
            .iter()
            .filter(|row| row.1 == "LOC_JAC_ASN_ATTEMPT")
            .count()
            + 1
    );
    world
        .lua
        .load("UI_ROWS[1].Action.Callback()")
        .exec()
        .unwrap();
    let calls: Table = world.lua.globals().get("UI_CALLS").unwrap();
    let params: Table = calls.get::<Table>(1).unwrap().get("Params").unwrap();
    assert_eq!(params.get::<String>("Action").unwrap(), "CANCEL");
}

#[test]
fn hidden_launcher_tracks_missing_and_dead_local_player_without_building_rows() {
    let world = PanelWorld::new(&read_repo(PANEL), 1000);
    let controls: Table = world.lua.globals().get("Controls").unwrap();
    let launcher: Table = controls.get("OpenButton").unwrap();
    assert!(!launcher.get::<bool>("Hidden").unwrap());
    world.lua.load("DATA[0].Alive=false").exec().unwrap();
    world
        .event("GameCoreEventPublishComplete")
        .call::<()>(())
        .unwrap();
    assert!(launcher.get::<bool>("Hidden").unwrap());
    world
        .lua
        .load("Game.GetLocalPlayer=function() return -1 end")
        .exec()
        .unwrap();
    world.event("LocalPlayerTurnBegin").call::<()>(()).unwrap();
    assert!(launcher.get::<bool>("Hidden").unwrap());
    world
        .lua
        .load("DATA[0].Alive=true; Game.GetLocalPlayer=function() return 0 end")
        .exec()
        .unwrap();
    world.event("LoadScreenClose").call::<()>(()).unwrap();
    assert!(!launcher.get::<bool>("Hidden").unwrap());
    assert_eq!(world.counts.snapshot(), zero());
}

#[test]
fn visible_rows_and_work_match_unconditional_refresh_and_follow_visibility_changes() {
    let source = read_repo(PANEL);
    let legacy = source.replace(
        "if not player or Controls.Panel:IsHidden() then return end",
        "if not player then return end",
    );
    assert_ne!(source, legacy);
    let current = PanelWorld::new(&source, 1000);
    let baseline = PanelWorld::new(&legacy, 1000);
    for world in [&current, &baseline] {
        world.click("OpenButton");
        world.counts.reset();
    }
    assert_eq!(current.rows(), baseline.rows());
    for event in [
        "GameCoreEventPublishComplete",
        "UnitSelectionChanged",
        "LocalPlayerTurnBegin",
        "LoadScreenClose",
    ] {
        for world in [&current, &baseline] {
            world.event(event).call::<()>(()).unwrap();
        }
    }
    assert_eq!(current.counts.snapshot(), baseline.counts.snapshot());
    for world in [&current, &baseline] {
        world
            .lua
            .load("HIDDEN_UNIT=true; Players[0].IsTurnActive=function() return false end")
            .exec()
            .unwrap();
        world.event("UnitSelectionChanged").call::<()>(()).unwrap();
    }
    assert_eq!(current.rows(), baseline.rows());
    assert!(current.rows().iter().all(|row| row.2));
    assert!(current
        .rows()
        .iter()
        .all(|row| row.1 != "LOC_JAC_ASN_ATTEMPT" || !row.0.contains("UNIT_4")));
}
