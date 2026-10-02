use civ6_lua_tests_rs::{exec, new_lua, read_repo};
use mlua::{FromLuaMulti, Lua, Table};

struct Relations(Lua);

impl Relations {
    fn new() -> Self {
        let lua = new_lua();
        exec(&lua, include_str!("../fixtures/relations/stubs.lua"));
        exec(
            &lua,
            &read_repo("poc/JAC_PoC_JapanMapScale/Scripts/JAC_Japan_1534Relations.lua"),
        );
        exec(&lua, "FIRE('NewGameInitialized'); API=ExposedMembers.JAC_Japan1534Relations; HOJO_IMAGAWA=PAIR('HOJO','IMAGAWA'); SHIMAZU_RYUKYU=PAIR('SHIMAZU','RYUKYU'); MADE=COUNT('SetHasAllied')");
        let s = Self(lua);
        assert!(s.eval::<bool>("ALLIED[HOJO_IMAGAWA]"));
        assert!(s.eval::<bool>("API.IsStartAllianceActive('IMAGAWA','HOJO')"));
        s
    }
    fn run(&self, script: &str) {
        exec(&self.0, script);
    }
    fn eval<T: FromLuaMulti>(&self, expression: &str) -> T {
        self.0.load(format!("return {expression}")).eval().unwrap()
    }
    fn turn(&self, number: i64) {
        self.run(&format!("TURN={number}; FIRE('OnGameTurnStarted', TURN)"));
    }
    fn lapse(&self) {
        self.run("ALLIED[HOJO_IMAGAWA]=nil; ALLIED[SHIMAZU_RYUKYU]=nil");
    }
    fn log(&self) -> Vec<String> {
        self.eval::<Table>("LOG")
            .sequence_values()
            .collect::<mlua::Result<_>>()
            .unwrap()
    }
    fn assert_no_remake(&self) {
        assert_eq!(
            self.eval::<i64>("COUNT('SetHasAllied')"),
            self.eval::<i64>("MADE")
        );
    }
}

#[test]
fn test_lapsed_start_alliances_are_not_re_made_and_turn_false() {
    let s = Relations::new();
    s.turn(31);
    assert_eq!(
        s.eval::<i64>("Game:GetProperty('JAC_1534_ALLIANCE_HOJO-IMAGAWA')"),
        1
    );
    s.lapse();
    s.turn(32);
    assert!(s.eval::<Option<bool>>("ALLIED[HOJO_IMAGAWA]").is_none());
    assert!(s.eval::<Option<bool>>("ALLIED[SHIMAZU_RYUKYU]").is_none());
    s.assert_no_remake();
    assert_eq!(
        s.eval::<Option<bool>>("Game:GetProperty('JAC_1534_ALLIANCE_HOJO-IMAGAWA')"),
        Some(false)
    );
    assert_eq!(
        s.eval::<Option<bool>>("Game:GetProperty('JAC_1534_ALLIANCE_RYUKYU-SHIMAZU')"),
        Some(false)
    );
    assert!(s.log().contains(
        &"[JAC_1534_RELATIONS] start alliance lapsed R02 HOJO-IMAGAWA on turn 32".into()
    ));
    assert!(s
        .eval::<Option<i64>>("Game:GetProperty('JAC_1534_ALLIANCE_LAPSES')")
        .is_none());
}

#[test]
fn test_an_alliance_the_pair_makes_later_is_not_a_start_alliance() {
    let s = Relations::new();
    s.lapse();
    s.turn(32);
    s.run("ALLIED[HOJO_IMAGAWA]=true");
    s.turn(40);
    assert!(!s.eval::<bool>("API.IsStartAllianceActive('HOJO','IMAGAWA')"));
    assert_eq!(
        s.eval::<Option<bool>>("Game:GetProperty('JAC_1534_ALLIANCE_HOJO-IMAGAWA')"),
        Some(false)
    );
    assert!(!s.eval::<bool>("API.EndStartAlliance('HOJO','IMAGAWA')"));
    s.run("ALLIED[HOJO_IMAGAWA]=nil");
    s.turn(71);
    s.assert_no_remake();
}

#[test]
fn test_end_marks_the_pair_ended_and_the_alliance_lapses_at_the_limit() {
    let s = Relations::new();
    s.turn(5);
    assert!(s.eval::<bool>("API.EndStartAlliance('HOJO','IMAGAWA')"));
    assert!(!s.eval::<bool>("API.IsStartAllianceActive('IMAGAWA','HOJO')"));
    assert!(!s.eval::<bool>("API.EndStartAlliance('IMAGAWA','HOJO')"));
    assert!(s.eval::<bool>("ALLIED[HOJO_IMAGAWA]"));
    assert!(s
        .log()
        .contains(&"[JAC_1534_RELATIONS] ended start alliance HOJO-IMAGAWA".into()));
    assert!(s.eval::<bool>("API.IsStartAllianceActive('SHIMAZU','RYUKYU')"));
    s.lapse();
    s.turn(32);
    assert_eq!(
        s.eval::<Option<bool>>("Game:GetProperty('JAC_1534_ALLIANCE_HOJO-IMAGAWA')"),
        Some(false)
    );
    assert!(!s.log().contains(
        &"[JAC_1534_RELATIONS] start alliance lapsed R02 HOJO-IMAGAWA on turn 32".into()
    ));
    s.assert_no_remake();
    assert_eq!(s.eval::<i64>("COUNT('DeclareWarOn')"), 0);
    assert_eq!(s.eval::<i64>("COUNT('MakePeaceWith')"), 0);
}

#[test]
fn test_a_war_started_elsewhere_ends_the_alliance_without_a_second_war_or_peace() {
    let s = Relations::new();
    s.run("WAR[HOJO_IMAGAWA]=true");
    s.turn(5);
    assert!(!s.eval::<bool>("API.IsStartAllianceActive('HOJO','IMAGAWA')"));
    s.turn(6);
    assert_eq!(s.eval::<i64>("COUNT('DeclareWarOn')"), 0);
    assert_eq!(s.eval::<i64>("COUNT('MakePeaceWith')"), 0);
    assert!(s.eval::<bool>("WAR[HOJO_IMAGAWA]"));
}
