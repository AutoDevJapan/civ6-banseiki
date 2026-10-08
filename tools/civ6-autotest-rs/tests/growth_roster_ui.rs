use civ6_autotest_rs::{
    growth_roster::{UI_LUA, UI_XML},
    repo_root,
};
use mlua::Lua;

fn publisher(local: i64) -> Lua {
    let lua = Lua::new();
    lua.globals().set("me", local).unwrap();
    lua.load(r#"
      turn=1; clock=10; network=true; observers={62}; humans={1,62,0}
      props={JAC_POC_GROWTHUI_ACTIVE_TURNS={['0']=1,['1']=1}}; packets={}; hooks={}
      GameConfiguration={IsNetworkMultiplayer=function() return network end,
        GetHumanPlayerIDs=function() return humans end, GetObserverIDs=function() return observers end}
      Game={GetLocalPlayer=function() return me end,GetCurrentGameTurn=function() return turn end,
        GetProperty=function(_,key) return props[key] end,SetProperty=function() error('UI wrote state') end}
      PlayerOperations={EXECUTE_SCRIPT=77}
      ContextPtr={SetHide=function(_,hidden) assert(hidden==true) end}
      UI={GetElapsedTime=function() return clock end,
        RequestPlayerOperation=function(actor,kind,params)
          assert(kind==77);packets[#packets+1]={actor=actor,params=params}
        end}
      Events={LoadScreenClose={Add=function(f) hooks.load=f end},
        LocalPlayerTurnBegin={Add=function(f) hooks.begin=f end},
        GameCoreEventPublishComplete={Add=function(f) hooks.publish=f end}}
    "#).exec().unwrap();
    lua.load(UI_LUA).exec().unwrap();
    lua
}

fn count(lua: &Lua) -> usize {
    lua.load("return #packets").eval().unwrap()
}
fn first(lua: &Lua) -> (i64, String, i64, String) {
    lua.load("local p=packets[1];return p.actor,p.params.Roster,p.params.Turn,p.params.OnStart")
        .eval()
        .unwrap()
}

#[test]
fn selected_human_publishes_sorted_configured_roster_without_observers_or_game_writes() {
    let lua = publisher(0);
    lua.load("hooks.load()").exec().unwrap();
    assert_eq!(first(&lua), (0, "0,1".into(), 1, "JAC_GrowthRoster".into()));
    assert_eq!(count(&lua), 1);
}

#[test]
fn other_peer_or_observer_or_missing_active_actor_never_sends() {
    for local in [1, 62, -1] {
        let lua = publisher(local);
        lua.load("hooks.load(); hooks.begin(); hooks.publish()")
            .exec()
            .unwrap();
        assert_eq!(count(&lua), 0);
    }
    let lua = publisher(0);
    lua.load("props.JAC_POC_GROWTHUI_ACTIVE_TURNS={}; hooks.load()")
        .exec()
        .unwrap();
    assert_eq!(count(&lua), 0);
}

#[test]
fn repeated_publications_are_throttled_until_shared_roster_acknowledges_them() {
    let lua = publisher(0);
    lua.load("hooks.load(); hooks.begin(); hooks.publish()")
        .exec()
        .unwrap();
    assert_eq!(count(&lua), 1);
    lua.load("clock=12; hooks.publish()").exec().unwrap();
    assert_eq!(count(&lua), 2);
    lua.load("props.JAC_POC_GROWTHUI_NETWORK_ROSTER={0,1}; clock=14; hooks.publish()")
        .exec()
        .unwrap();
    assert_eq!(count(&lua), 2);
}

#[test]
fn initialized_reload_and_singleplayer_never_publish_a_new_roster() {
    for setup in ["network=false", "props.JAC_POC_GROWTHUI_INITIALIZED=true"] {
        let lua = publisher(0);
        lua.load(setup).exec().unwrap();
        lua.load("hooks.load(); hooks.begin(); hooks.publish()")
            .exec()
            .unwrap();
        assert_eq!(count(&lua), 0);
    }
}

#[test]
fn shipped_roster_ui_matches_generated_cli_source_and_mod_registration() {
    let source =
        std::fs::read_to_string(repo_root().join("poc/JAC_PoC_GrowthUI/UI/JAC_GrowthRoster.lua"))
            .unwrap();
    assert_eq!(source, UI_LUA);
    let info =
        std::fs::read_to_string(repo_root().join("poc/JAC_PoC_GrowthUI/JAC_PoC_GrowthUI.modinfo"))
            .unwrap();
    let doc = roxmltree::Document::parse(&info).unwrap();
    for area in ["ImportFiles", "Files"] {
        assert_eq!(
            doc.descendants()
                .filter(|node| node.has_tag_name(area))
                .flat_map(|node| node.children())
                .filter(|node| node.has_tag_name("File")
                    && node.text() == Some("UI/JAC_GrowthRoster.lua"))
                .count(),
            1
        );
    }
    assert_eq!(
        std::fs::read_to_string(repo_root().join("poc/JAC_PoC_GrowthUI/UI/JAC_GrowthRoster.xml"))
            .unwrap(),
        UI_XML
    );
    assert_eq!(
        doc.descendants()
            .filter(|node| node.has_tag_name("AddUserInterfaces"))
            .flat_map(|node| node.children())
            .filter(
                |node| node.has_tag_name("File") && node.text() == Some("UI/JAC_GrowthRoster.xml")
            )
            .count(),
        1
    );
    let cli = std::process::Command::new(env!("CARGO_BIN_EXE_civ6-growth-sync"))
        .arg("roster-ui")
        .output()
        .unwrap();
    assert!(cli.status.success());
    assert_eq!(cli.stdout, UI_LUA.as_bytes());
}
