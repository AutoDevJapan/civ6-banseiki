use civ6_lua_tests_rs::{exec, exec_args, new_lua, read_repo};
use mlua::{FromLuaMulti, Lua, Table};

const CASTLE_SET: &str = "JAC_UNIFICATION_CITY_IS_CASTLE";
const REBUILT: &str = "BUILDING_JAC_REBUILT_ON_CASTLE";
const UNIFIED: &str = "BUILDING_JAC_PACT_UNIFIED";
const PACT_LEAD_PREFIX: &str = "JAC_UNIFICATION_PACT_LEAD_";
const PACT_RESULT: &str = "JAC_UNIFICATION_PACT_RESULT";
const ODA: i64 = 0;
const DATE: i64 = 1;
const ASHINA: i64 = 2;
const SHIMAZU: i64 = 3;

struct Pact(Lua);
impl Pact {
    fn new(no_castles: bool) -> Self {
        let lua = new_lua();
        exec_args(
            &lua,
            include_str!("../fixtures/unification/stubs.lua"),
            (CASTLE_SET, REBUILT, UNIFIED),
        );
        if no_castles {
            exec(&lua, "SET_NO_CASTLES()");
        }
        exec(&lua, "CITY(0,6,5,true); CITY(0,6,14); CITY(1,25,5,true); DATE_CASTLE=CITY(1,25,14); CITY(2,16,10,true); CITY(3,3,3,true)");
        for name in [
            "JAC_Sengoku_RebuiltCastles.lua",
            "JAC_Sengoku_UnificationPact.lua",
        ] {
            exec(
                &lua,
                &read_repo(format!("poc/JAC_PoC_SengokuTrees/Scripts/{name}")),
            );
        }
        Self(lua)
    }
    fn run(&self, script: &str) {
        exec(&self.0, script);
    }
    fn eval<T: FromLuaMulti>(&self, expression: &str) -> T {
        self.0.load(format!("return {expression}")).eval().unwrap()
    }
    fn request(&self, event: &str, player: i64, target: Option<i64>) {
        let args = self.0.create_table().unwrap();
        args.set("Target", target).unwrap();
        self.0
            .globals()
            .get::<mlua::Function>("FIRE")
            .unwrap()
            .call::<()>((format!("JAC_UnificationPact{event}"), player, args))
            .unwrap();
    }
    fn lead(&self, player: i64) -> Option<i64> {
        self.eval(&format!("Game:GetProperty('{PACT_LEAD_PREFIX}{player}')"))
    }
    fn result(&self, player: i64) -> Option<String> {
        self.eval(&format!("Players[{player}]:GetProperty('{PACT_RESULT}')"))
    }
    fn assert_result(&self, player: i64, expected: &str) {
        assert_eq!(self.result(player).as_deref(), Some(expected));
    }
    fn marked(&self) -> Vec<(i64, i64)> {
        self.eval::<Table>("BUILT")
            .sequence_values::<Table>()
            .map(|row| {
                let row = row.unwrap();
                (row.get(1).unwrap(), row.get(2).unwrap())
            })
            .collect()
    }
    fn unified(&self) -> i64 {
        self.eval(&format!("GameInfo.Buildings['{UNIFIED}'].Index"))
    }
    fn assert_marker(&self, plot: i64) {
        assert_eq!(self.marked(), vec![(self.unified(), plot)]);
    }
    fn ally(&self, a: i64, b: i64) {
        self.run(&format!("ALLIED['{a}-{b}']=true; ALLIED['{b}-{a}']=true"));
    }
    fn fire(&self, event: &str, args: &str) {
        self.run(&format!("FIRE('{event}', {args})"));
    }
    fn log(&self) -> Vec<String> {
        self.eval::<Table>("LOG")
            .sequence_values()
            .collect::<mlua::Result<_>>()
            .unwrap()
    }
    fn pact_of_oda_date_shimazu(&self) {
        self.ally(ODA, DATE);
        self.request("Propose", ODA, Some(DATE));
        self.request("Propose", ODA, Some(SHIMAZU));
        self.request("Accept", SHIMAZU, Some(ODA));
        assert_eq!(
            (self.lead(ODA), self.lead(DATE), self.lead(SHIMAZU)),
            (Some(ODA), Some(ODA), Some(ODA))
        );
    }
    fn pact_of_oda_date_ashina(&self) {
        self.run("ALLIED['0-1']=true; ALLIED['0-2']=true; DATE_CASTLE.owner=2");
        self.request("Propose", ODA, Some(DATE));
        self.request("Propose", ODA, Some(ASHINA));
        assert_eq!((self.lead(DATE), self.lead(ASHINA)), (Some(ODA), Some(ODA)));
        self.assert_marker(5 * 32 + 6);
        self.run("BUILT[1]=nil");
    }
    fn eliminate_oda(&self, capital: i64, other: i64) {
        self.run(&format!("CITIES[5*32+6].owner={capital}; CITIES[5*32+6].capital=false; CITIES[14*32+6].owner={other}; Players[0].dead=true"));
        self.fire("CityConquered", &format!("{capital},0,0,6,5"));
    }
}

#[test]
fn test_ai_joins_only_when_allied_and_the_lead_capital_is_marked_when_the_pact_holds_all() {
    let s = Pact::new(false);
    s.request("Propose", ODA, Some(DATE));
    s.assert_result(ODA, "NOT_ALLIED");
    assert_eq!(s.lead(DATE), None);
    s.run("DATE_CASTLE.owner=2; ALLIED['0-1']=true");
    s.request("Propose", ODA, Some(DATE));
    s.assert_result(ODA, "JOINED");
    assert_eq!((s.lead(ODA), s.lead(DATE)), (Some(ODA), Some(ODA)));
    assert_eq!(s.marked(), vec![]);
    s.run("DATE_CASTLE.owner=1");
    s.fire("CityBuilt", "1,0,25,14");
    s.assert_marker(5 * 32 + 6);
    s.fire("OnGameTurnStarted", "16604");
    assert_eq!(s.marked().len(), 1);
}
#[test]
fn test_human_target_must_accept_the_proposer() {
    let s = Pact::new(false);
    s.request("Propose", ODA, Some(SHIMAZU));
    s.assert_result(ODA, "PROPOSED");
    assert_eq!(s.lead(SHIMAZU), None);
    s.request("Accept", SHIMAZU, Some(DATE));
    s.assert_result(SHIMAZU, "INVALID");
    s.request("Accept", DATE, Some(ODA));
    s.assert_result(DATE, "INVALID");
    assert_eq!(s.lead(DATE), None);
    s.request("Accept", SHIMAZU, Some(ODA));
    assert_eq!((s.lead(ODA), s.lead(SHIMAZU)), (Some(ODA), Some(ODA)));
}
#[test]
fn test_one_pact_per_player_and_no_pact_within_a_team() {
    let s = Pact::new(false);
    s.run("ALLIED['0-1']=true; ALLIED['2-1']=true; ALLIED['1-2']=true; DATE_CASTLE.owner=2");
    s.request("Propose", ODA, Some(DATE));
    s.request("Propose", ASHINA, Some(DATE));
    s.assert_result(ASHINA, "INVALID");
    s.request("Propose", DATE, Some(ASHINA));
    s.assert_result(DATE, "INVALID");
    assert_eq!(s.lead(ASHINA), None);
    s.run("ALLIED['0-2']=true");
    s.request("Propose", ODA, Some(ASHINA));
    assert_eq!(s.lead(ASHINA), Some(ODA));
    assert_eq!(s.marked().len(), 1);
    s.run("Players[3].team=0");
    s.request("Propose", ODA, Some(SHIMAZU));
    s.assert_result(ODA, "INVALID");
    s.request("Propose", ODA, None);
    s.request("Propose", ODA, Some(ODA));
    s.assert_result(ODA, "INVALID");
}
#[test]
fn test_rebuilt_city_on_a_razed_castle_does_not_block_the_pact() {
    let s = Pact::new(false);
    s.run("ALLIED['0-1']=true; CITIES[14*32+25]=nil; TURN=16604");
    s.fire("OnGameTurnStarted", "16604");
    s.run("CITY(2,25,14)");
    s.fire("CityBuilt", "2,0,25,14");
    s.request("Propose", ODA, Some(DATE));
    assert_eq!(s.marked().last(), Some(&(s.unified(), 5 * 32 + 6)));
}
#[test]
fn test_eliminated_member_drops_out_and_the_rest_of_the_pact_can_still_win() {
    let s = Pact::new(false);
    s.run("DATE_CASTLE.owner=2");
    s.pact_of_oda_date_shimazu();
    s.run("CITIES[5*32+25].owner=0; CITIES[5*32+25].capital=false; Players[1].dead=true");
    s.fire("CityConquered", "0,1,0,25,5");
    assert_eq!((s.lead(ODA), s.lead(SHIMAZU)), (Some(ODA), Some(ODA)));
    assert_eq!(s.marked(), vec![]);
    s.run("DATE_CASTLE.owner=0");
    s.fire("CityConquered", "0,2,0,25,14");
    s.assert_marker(5 * 32 + 6);
}
#[test]
fn test_a_pact_left_with_one_house_is_dissolved() {
    let s = Pact::new(false);
    s.ally(ODA, DATE);
    s.request("Propose", ODA, Some(DATE));
    s.run("Players[1].dead=true");
    s.fire("OnGameTurnStarted", "16604");
    assert_eq!(s.lead(ODA), None);
    s.assert_result(ODA, "DISSOLVED");
}
#[test]
fn test_a_member_leaves_at_once_and_the_rest_stay() {
    let s = Pact::new(false);
    s.pact_of_oda_date_shimazu();
    s.fire("JAC_UnificationPactLeave", "3,{Target=3}");
    assert_eq!(
        (s.lead(ODA), s.lead(DATE), s.lead(SHIMAZU)),
        (Some(ODA), Some(ODA), None)
    );
    s.assert_result(SHIMAZU, "LEFT");
    s.fire("JAC_UnificationPactLeave", "3,nil");
    s.assert_result(SHIMAZU, "INVALID");
    s.request("Accept", SHIMAZU, Some(ODA));
    assert_eq!(s.lead(SHIMAZU), None);
    s.request("Propose", ODA, Some(SHIMAZU));
    s.request("Accept", SHIMAZU, Some(ODA));
    assert_eq!(s.lead(SHIMAZU), Some(ODA));
}
#[test]
fn test_the_lead_leaving_passes_the_pact_and_a_pact_of_two_dissolves() {
    let s = Pact::new(false);
    s.pact_of_oda_date_shimazu();
    s.fire("JAC_UnificationPactLeave", "0,nil");
    assert_eq!(
        (s.lead(ODA), s.lead(DATE), s.lead(SHIMAZU)),
        (None, Some(DATE), Some(DATE))
    );
    s.fire("JAC_UnificationPactLeave", "3,nil");
    assert_eq!(s.lead(DATE), None);
    s.assert_result(DATE, "DISSOLVED");
    s.assert_result(SHIMAZU, "LEFT");
}
#[test]
fn test_members_at_war_leave_and_the_lead_stays() {
    let s = Pact::new(false);
    s.pact_of_oda_date_shimazu();
    s.run("WAR['0-1']=true");
    s.fire("OnGameTurnStarted", "16604");
    assert_eq!(
        (s.lead(ODA), s.lead(DATE), s.lead(SHIMAZU)),
        (Some(ODA), None, Some(ODA))
    );
    s.assert_result(DATE, "AT_WAR");
    assert!(s
        .log()
        .contains(&"JAC_Sengoku_UnificationPact: 1 left the pact of 0 (at war with 0)".into()));
}
#[test]
fn test_two_warring_members_both_leave() {
    let s = Pact::new(false);
    s.pact_of_oda_date_shimazu();
    s.run("WAR['1-3']=true");
    s.fire("OnGameTurnStarted", "16604");
    assert_eq!(
        (s.lead(ODA), s.lead(DATE), s.lead(SHIMAZU)),
        (None, None, None)
    );
    s.assert_result(ODA, "DISSOLVED");
    s.assert_result(SHIMAZU, "AT_WAR");
    assert!(s
        .log()
        .contains(&"JAC_Sengoku_UnificationPact: 1 left the pact of 0 (at war with 3)".into()));
    assert!(s
        .log()
        .contains(&"JAC_Sengoku_UnificationPact: pact of 0 dissolved".into()));
    s.assert_result(DATE, "PROPOSED");
    assert_eq!(
        s.eval::<i64>("Game:GetProperty('JAC_UNIFICATION_PACT_PROPOSAL_0')"),
        DATE
    );
}
#[test]
fn test_a_pact_of_two_dissolves_when_they_go_to_war() {
    let s = Pact::new(false);
    s.ally(ODA, DATE);
    s.run("DATE_CASTLE.owner=2");
    s.request("Propose", ODA, Some(DATE));
    s.run("WAR['0-1']=true; DATE_CASTLE.owner=0");
    s.fire("CityConquered", "0,2,0,25,14");
    assert_eq!(s.marked(), vec![]);
    s.run("CITIES[5*32+25].owner=0");
    s.fire("CityConquered", "0,1,0,25,5");
    assert_eq!((s.lead(ODA), s.lead(DATE)), (None, None));
    s.assert_result(ODA, "DISSOLVED");
    s.assert_result(DATE, "AT_WAR");
    assert_eq!(s.marked(), vec![]);
}
#[test]
fn test_ai_proposes_to_an_allied_ai_when_they_hold_enough_castles() {
    let s = Pact::new(false);
    s.ally(DATE, ASHINA);
    s.fire("OnGameTurnStarted", "16604");
    assert_eq!((s.lead(DATE), s.lead(ASHINA)), (Some(DATE), Some(DATE)));
}
#[test]
fn test_ai_proposal_to_a_human_waits_for_acceptance_and_is_not_repeated() {
    let s = Pact::new(false);
    s.ally(DATE, ODA);
    s.fire("OnGameTurnStarted", "16604");
    assert_eq!(
        s.eval::<i64>("Game:GetProperty('JAC_UNIFICATION_PACT_PROPOSAL_0')"),
        DATE
    );
    s.assert_result(DATE, "PROPOSED");
    assert_eq!(s.lead(ODA), None);
    s.fire("OnGameTurnStarted", "16605");
    assert_eq!(
        s.log()
            .iter()
            .filter(|line| line.contains("proposed a pact"))
            .count(),
        1
    );
    s.request("Accept", ODA, Some(DATE));
    assert_eq!((s.lead(ODA), s.lead(DATE)), (Some(DATE), Some(DATE)));
    assert_eq!(
        s.eval::<Option<i64>>("Game:GetProperty('JAC_UNIFICATION_PACT_PROPOSAL_0')"),
        None
    );
}
#[test]
fn test_ai_does_not_propose_to_a_house_it_is_at_war_with() {
    let s = Pact::new(false);
    s.ally(DATE, ODA);
    s.run("WAR['0-1']=true");
    s.fire("OnGameTurnStarted", "16604");
    assert_eq!(
        s.eval::<Option<i64>>("Game:GetProperty('JAC_UNIFICATION_PACT_PROPOSAL_0')"),
        None
    );
    assert_eq!(s.result(DATE), None);
    s.request("Propose", DATE, Some(ODA));
    s.assert_result(DATE, "INVALID");
}
#[test]
fn test_ai_does_not_propose_below_the_share_or_without_an_alliance() {
    let s = Pact::new(false);
    s.ally(ASHINA, SHIMAZU);
    s.fire("OnGameTurnStarted", "16604");
    assert_eq!(
        s.eval::<Option<i64>>("Game:GetProperty('JAC_UNIFICATION_PACT_PROPOSAL_3')"),
        None
    );
    assert_eq!(s.lead(ASHINA), None);
    assert_eq!(s.lead(DATE), None);
}
#[test]
fn test_an_active_start_alliance_pools_and_its_holder_of_more_castles_leads() {
    let s = Pact::new(false);
    s.run("Game:SetProperty('JAC_1534_ALLIANCE_ASHINA-DATE',16603); for _,xy in ipairs({{6,5},{6,14},{25,14}}) do CITIES[xy[2]*32+xy[1]].owner=2 end; CITIES[5*32+6].capital=false");
    s.fire("CityConquered", "2,0,0,6,5");
    s.assert_marker(10 * 32 + 16);
    assert_eq!(s.lead(ASHINA), None);
}
#[test]
fn test_an_ended_start_alliance_does_not_pool() {
    let s = Pact::new(false);
    s.run("Game:SetProperty('JAC_1534_ALLIANCE_ASHINA-DATE',false); CITIES[5*32+6].owner=2; CITIES[14*32+6].owner=2");
    s.fire("OnGameTurnStarted", "16604");
    assert_eq!(s.marked(), vec![]);
}
#[test]
fn test_a_start_alliance_tie_goes_to_the_lowest_id() {
    let s = Pact::new(false);
    s.run("Game:SetProperty('JAC_1534_ALLIANCE_ASHINA-DATE',16603); CITIES[5*32+6].owner=2; CITIES[14*32+6].owner=2");
    s.fire("OnGameTurnStarted", "16604");
    s.assert_marker(5 * 32 + 25);
}
#[test]
fn test_a_start_alliance_inside_one_pact_is_left_to_the_pact() {
    let s = Pact::new(false);
    s.run("Game:SetProperty('JAC_1534_ALLIANCE_ASHINA-DATE',16603); CITIES[5*32+6].owner=2; CITIES[14*32+6].owner=2");
    s.ally(ASHINA, DATE);
    s.request("Propose", ASHINA, Some(DATE));
    s.assert_marker(10 * 32 + 16);
}
#[test]
fn test_eliminated_member_castle_taken_by_an_outsider_blocks_the_pact() {
    let s = Pact::new(false);
    s.run("ALLIED['0-1']=true; DATE_CASTLE.owner=2");
    s.request("Propose", ODA, Some(DATE));
    s.run("CITIES[5*32+25].owner=2; Players[1].dead=true");
    s.fire("CityConquered", "2,1,0,25,5");
    s.fire("OnGameTurnStarted", "16604");
    assert_eq!(s.marked(), vec![]);
    s.request("Propose", ODA, Some(DATE));
    s.assert_result(ODA, "INVALID");
}
#[test]
fn test_without_registered_castles_the_pact_pools_home_capitals() {
    let s = Pact::new(true);
    s.ally(ODA, DATE);
    s.request("Propose", ODA, Some(DATE));
    s.assert_result(ODA, "JOINED");
    assert_eq!(s.lead(DATE), Some(ODA));
    assert_eq!(
        s.eval::<i64>("Game:GetProperty('JAC_UNIFICATION_HOME_CAPITAL_2')"),
        10 * 32 + 16
    );
    assert_eq!(s.marked(), vec![]);
    s.run("CITIES[10*32+16].owner=1; CITIES[10*32+16].capital=false; Players[2].dead=true");
    s.fire("CityConquered", "1,2,0,16,10");
    assert_eq!(s.marked(), vec![]);
    s.run("CITIES[3*32+3].owner=0; CITIES[3*32+3].capital=false; Players[3].dead=true");
    s.fire("CityConquered", "0,3,0,3,3");
    s.assert_marker(5 * 32 + 6);
}
#[test]
fn test_home_capitals_count_only_once_every_major_has_founded() {
    let s = Pact::new(true);
    s.run("CITIES[3*32+3]=nil");
    s.fire("OnGameTurnStarted", "16603");
    s.run("CITIES[10*32+16].owner=0; CITIES[10*32+16].capital=false");
    s.ally(ODA, DATE);
    s.request("Propose", ODA, Some(DATE));
    assert_eq!(s.marked(), vec![]);
    s.run("CITY(3,3,3,true)");
    s.fire("CityBuilt", "3,0,3,3");
    assert_eq!(s.marked(), vec![]);
    s.run("CITIES[3*32+3].owner=1");
    s.fire("CityConquered", "1,3,0,3,3");
    s.assert_marker(5 * 32 + 6);
}
#[test]
fn test_lead_eliminated_passes_the_pact_to_the_member_with_most_castles() {
    let s = Pact::new(false);
    s.pact_of_oda_date_ashina();
    s.eliminate_oda(ASHINA, ASHINA);
    assert_eq!((s.lead(DATE), s.lead(ASHINA)), (Some(ASHINA), Some(ASHINA)));
    assert!(s.log().contains(
        &"JAC_Sengoku_UnificationPact: lead 0 gone; 2 leads the pact (3 targets)".into()
    ));
    s.assert_marker(10 * 32 + 16);
    s.fire("OnGameTurnStarted", "16604");
    assert_eq!((s.lead(DATE), s.marked().len()), (Some(ASHINA), 1));
}
#[test]
fn test_lead_eliminated_tie_goes_to_the_lowest_id_who_can_then_take_members() {
    let s = Pact::new(false);
    s.pact_of_oda_date_ashina();
    s.eliminate_oda(SHIMAZU, SHIMAZU);
    assert_eq!((s.lead(DATE), s.lead(ASHINA)), (Some(DATE), Some(DATE)));
    assert_eq!(s.marked(), vec![]);
    s.request("Propose", DATE, Some(SHIMAZU));
    s.request("Accept", SHIMAZU, Some(DATE));
    assert_eq!(s.lead(SHIMAZU), Some(DATE));
    s.assert_marker(5 * 32 + 25);
}
#[test]
fn test_pact_with_no_survivor_is_gone() {
    let s = Pact::new(false);
    s.run("ALLIED['0-1']=true; DATE_CASTLE.owner=2");
    s.request("Propose", ODA, Some(DATE));
    s.run("Players[1].dead=true");
    s.eliminate_oda(ASHINA, ASHINA);
    s.fire("OnGameTurnStarted", "16604");
    assert_eq!(s.marked(), vec![]);
    assert!(!s.log().iter().any(|line| line.contains("leads the pact")));
    s.request("Propose", ODA, Some(SHIMAZU));
    s.assert_result(ODA, "INVALID");
}
