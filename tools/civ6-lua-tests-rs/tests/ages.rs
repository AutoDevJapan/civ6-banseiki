mod ages_support;
#[path = "generators/reigns.rs"]
mod reigns_generator;
use ages_support::*;
use serde_json::json;

fn sql_rows(db: &rusqlite::Connection, sql: &str) -> Vec<Vec<serde_json::Value>> {
    let mut stmt = db.prepare(sql).unwrap_or_else(|e| panic!("{sql}: {e}"));
    let cols = stmt.column_count();
    stmt.query_map([], |r| {
        let mut row = Vec::new();
        for i in 0..cols {
            row.push(match r.get_ref(i)? {
                rusqlite::types::ValueRef::Null => json!(null),
                rusqlite::types::ValueRef::Integer(n) => json!(n),
                rusqlite::types::ValueRef::Real(n) => json!(n),
                rusqlite::types::ValueRef::Text(s) => json!(std::str::from_utf8(s).unwrap()),
                rusqlite::types::ValueRef::Blob(_) => panic!("blob in {sql}"),
            });
        }
        Ok(row)
    })
    .unwrap()
    .collect::<rusqlite::Result<Vec<_>>>()
    .unwrap()
}
fn sql_one(db: &rusqlite::Connection, sql: &str) -> serde_json::Value {
    let rows = sql_rows(db, sql);
    assert_eq!(rows.len(), 1, "{sql}");
    assert_eq!(rows[0].len(), 1, "{sql}");
    rows[0][0].clone()
}

mod ledger_tests {
    use super::*;
    #[test]
    fn test_capture_classes_home_regain_and_castle() {
        let w = World::new();
        w.turns(1, 2);
        w.run("CAPTURE(1,1,1)");
        assert_eq!(w.j("LEDGER().houses.p1.parts"), json!({"home":6}));
        w.run("CAPTURE(1,2,1)");
        assert_eq!(
            w.j("LEDGER().houses.p1.parts"),
            json!({"home":6,"castle":2,"ko":8})
        );
        w.run("CAPTURE(0,1,1)");
        assert_eq!(w.j("LEDGER().houses.p0.parts"), json!({"regain":3}));
        w.turn(3);
        assert_eq!(
            [
                w.j("LEDGER().houses.p0.score"),
                w.j("LEDGER().houses.p1.score")
            ],
            [json!(3), json!(16)]
        );
    }
    #[test]
    fn test_capture_event_and_turn_start_difference_count_once() {
        let w = World::new();
        w.turn(1);
        w.run("CAPTURE(3,9,9)");
        w.turns(2, 4);
        assert_eq!(w.parts(3), json!({"home":6,"ko":8}));
        assert_eq!(w.j("(function() local n=0 for k in pairs(LEDGER().keys) do if string.sub(k,1,4)=='cap:' then n=n+1 end end return n end)()"),json!(1));
    }
    #[test]
    fn test_recaptures_in_one_turn_each_score() {
        let w = World::new();
        w.turn(1);
        w.run("CAPTURE(1,2,1); CAPTURE(0,2,1)");
        assert_eq!(w.parts(1), json!({"castle":2}));
        assert_eq!(w.parts(0), json!({"regain":3}));
        w.run("CAPTURE(1,2,1)");
        assert_eq!(w.parts(1), json!({"castle":4}));
        w.turn(2);
        assert_eq!(
            [w.house(0, "score"), w.house(1, "score")],
            [json!(3), json!(4)]
        );
        assert_eq!(w.j("LEDGER().cities.plot102.n"), json!(3));
    }
    #[test]
    fn test_capture_then_raze_keeps_the_points_and_the_fall() {
        let w = World::new();
        w.turn(1);
        w.run("CAPTURE(0,9,9,true)");
        w.turns(2, 3);
        assert_eq!(w.parts(0), json!({"home":6,"ko":8}));
        assert_eq!(w.j("LEDGER().cities.plot909"), json!(null));
    }
    #[test]
    fn test_transfer_scores_once_at_the_next_turn_start_not_on_city_built() {
        let w = World::new();
        w.turn(1);
        w.run("TRANSFER(2,13,12)");
        assert_eq!(w.house(2, "score"), json!(0));
        w.turns(2, 3);
        assert_eq!(w.parts(2), json!({"castle":2}));
    }
    #[test]
    fn test_silent_owner_change_is_scored_by_the_difference() {
        let w = World::new();
        w.turn(1);
        w.run("for _,city in ipairs(CITIES) do if city.x==14 then city.owner=0 end end");
        w.turn(2);
        assert_eq!(w.parts(0), json!({"castle":2}));
    }
    #[test]
    fn test_settler_start_house_takes_its_first_founded_castle_as_home() {
        let w = World::new();
        w.run("local keep={} for _,c in ipairs(CITIES) do if c.owner~=2 then keep[#keep+1]=c end end CITIES=keep");
        w.turn(1);
        assert_eq!(w.house(2, "home"), json!(null));
        w.run("FOUND(2,20,20,true); FOUND(2,21,20,false)");
        assert_eq!(w.house(2, "home"), json!("plot2020"));
        w.turn(2);
        w.run("CAPTURE(1,20,20)");
        assert_eq!(w.parts(1), json!({"home":6}));
    }
    #[test]
    fn test_castle_through_the_free_cities_keeps_its_history_and_regain_scores() {
        let w = World::new();
        w.turn(1);
        w.run("TRANSFER(FREE_CITIES,2,1)");
        w.turns(2, 3);
        assert_eq!(w.j("{LEDGER().cities.plot102.owner,LEDGER().cities.plot102.original,LEDGER().cities.plot102.n}"),json!({"1":62,"2":0,"3":1}));
        assert_eq!(w.j("LEDGER().houses.p62"), json!(null));
        w.run("CAPTURE(0,2,1)");
        w.turn(4);
        assert_eq!(w.parts(0), json!({"regain":3}));
        assert_eq!(w.j("LEDGER().cities.plot102.n"), json!(2));
    }
    #[test]
    fn test_home_taken_from_a_city_state_is_still_a_home() {
        let w = World::new();
        w.turn(1);
        w.run("CAPTURE(FREE_CITIES,1,1)");
        w.turn(2);
        assert_eq!(w.house(0, "home"), json!("plot101"));
        w.run("CAPTURE(1,1,1)");
        w.turn(3);
        assert_eq!(w.parts(1), json!({"home":6}));
        assert_eq!(
            [
                w.house(0, "score"),
                w.house(2, "score"),
                w.house(3, "score")
            ],
            [json!(0), json!(0), json!(0)]
        );
    }
    #[test]
    fn test_castle_of_a_non_major_has_no_age_flags() {
        let w = World::new();
        w.turns(1, 66);
        assert_eq!(w.plot_property(14, 12, "JAC_AGE_POLICY_TEMPLE"), json!(1));
        w.run("TRANSFER(FREE_CITIES,14,12)");
        assert_eq!(
            w.plot_properties()["1214"],
            json!([0, 0, 0, 0, 0, 0, 0, 0, 0])
        );
    }
    #[test]
    fn test_research_scores_the_difference_but_not_the_knowledge_held_at_initialization() {
        let w = World::new();
        w.run("for i=0,3 do Players[0].research.techs[i]=true end");
        w.turn(1);
        assert_eq!(w.house(0, "score"), json!(0));
        w.run("Players[0].research.techs[5]=true; Players[0].research.civics[2]=true");
        w.turn(2);
        assert_eq!(w.parts(0), json!({"research":2}));
    }
    #[test]
    fn test_event_flags_score_once_per_house_and_the_reign_turn_flag_goes_to_the_new_period() {
        let w = World::new();
        w.turns(1, 30);
        w.run("Players[0]:SetProperty('JAC_EVENT_FIRED_JAC_EVENT_TEPPO',30)");
        w.turns(31, 40);
        assert_eq!(w.parts(0), json!({"event":2}));
        w.turns(41, 65);
        w.run("Players[0]:SetProperty('JAC_EVENT_FIRED_JAC_EVENT_IKUNO_SILVER',65); Players[1]:SetProperty('JAC_EVENT_FIRED_JAC_EVENT_IKUNO_SILVER',66)");
        w.turn(66);
        assert_eq!(
            [
                w.j("LEDGER().history.r1.p0.score"),
                w.j("LEDGER().history.r1.p1.score")
            ],
            [json!(3), json!(0)]
        );
        assert_eq!(w.parts(1), json!({"event":1}));
        w.run("Players[2]:SetProperty('JAC_EVENT_FIRED_JAC_EVENT_IKUNO_SILVER',66)");
        w.turns(67, 70);
        assert_eq!(
            [
                w.house(0, "score"),
                w.house(1, "score"),
                w.house(2, "score")
            ],
            [json!(0), json!(1), json!(1)]
        );
    }
    #[test]
    fn test_start_alliance_and_its_lapse_score_nothing_with_the_ledger_first() {
        let w = World::new();
        w.relations(false);
        w.turns(1, 31);
        assert_eq!(w.j("LEDGER().allies['2-3']"), json!("start"));
        w.lapse_at_32();
        assert_eq!(
            (0..4).map(|p| w.house(p, "score")).collect::<Vec<_>>(),
            vec![json!(0); 4]
        );
        assert_eq!(w.j("LEDGER().allies['2-3']"), json!("none"));
        assert_eq!(
            w.j("Game:GetProperty('JAC_1534_ALLIANCE_HOJO-IMAGAWA')"),
            json!(false)
        );
    }
    #[test]
    fn test_start_alliance_and_its_lapse_score_nothing_with_the_ledger_last() {
        let w = World::new();
        w.relations(true);
        w.lapse_at_32();
        assert_eq!(
            (0..4).map(|p| w.house(p, "score")).collect::<Vec<_>>(),
            vec![json!(0); 4]
        );
        assert_eq!(w.j("LEDGER().allies['2-3']"), json!("none"));
    }
    #[test]
    fn test_a_pair_that_re_forms_on_the_lapse_turn_scores_in_either_order() {
        for before in [false, true] {
            let w = World::new();
            w.relations(before);
            w.turns(1, 31);
            w.run("ALLIED['2-3']=false");
            w.turn(32);
            w.run("ALLIED['2-3']=true");
            w.turn(33);
            assert_eq!(
                [w.parts(2), w.parts(3)],
                [json!({"ally":2}), json!({"ally":2})],
                "before={before}"
            );
        }
    }
    #[test]
    fn test_ended_start_alliance_lingers_without_points_then_a_new_alliance_scores() {
        let w = World::new();
        w.relations(false);
        w.turns(1, 10);
        w.run("Game:SetProperty('JAC_1534_ALLIANCE_HOJO-IMAGAWA',false)");
        w.turns(11, 20);
        assert_eq!(w.house(2, "score"), json!(0));
        w.run("ALLIED['2-3']=false");
        w.turns(21, 30);
        w.run("ALLIED['2-3']=true");
        w.turn(31);
        assert_eq!(
            [w.parts(2), w.parts(3)],
            [json!({"ally":2}), json!({"ally":2})]
        );
    }
    #[test]
    fn test_alliance_scores_once_per_period_and_same_team_never() {
        let w = World::new();
        w.run("TEAM[1]=0");
        w.turns(1, 5);
        w.run("ALLIED['0-1']=true; ALLIED['2-3']=true");
        w.turn(6);
        w.run("ALLIED['2-3']=false");
        w.turn(7);
        w.run("ALLIED['2-3']=true");
        w.turn(8);
        assert_eq!(
            (0..4).map(|p| w.house(p, "score")).collect::<Vec<_>>(),
            vec![json!(0), json!(0), json!(2), json!(2)]
        );
        w.run("ALLIED['2-3']=false");
        w.turns(9, 66);
        w.run("ALLIED['2-3']=true");
        w.turn(67);
        assert_eq!(
            [w.house(2, "score"), w.house(3, "score")],
            [json!(2), json!(2)]
        );
    }
    #[test]
    fn test_thresholds_follow_the_period_length_cities_and_past_ages() {
        let w = World::new();
        w.turns(1, 66);
        assert_eq!(
            [
                w.j("LEDGER().history.r1.turn"),
                w.j("LEDGER().history.r1.length")
            ],
            [json!(66), json!(65)]
        );
        assert_eq!(
            [
                w.j("LEDGER().history.r1.p3.golden"),
                w.j("LEDGER().history.r1.p3.dark"),
                w.j("LEDGER().history.r1.p3.age")
            ],
            [json!(7), json!(3), json!("DARK")]
        );
        assert_eq!(
            [
                w.j("LEDGER().history.r1.p2.golden"),
                w.j("LEDGER().history.r1.p2.dark")
            ],
            [json!(7), json!(3)]
        );
        w.run("for i=1,4 do Players[3].research.techs[i]=true end");
        w.turns(67, 73);
        assert_eq!(
            [
                w.j("LEDGER().history.r2.length"),
                w.j("LEDGER().history.r2.p3.golden"),
                w.j("LEDGER().history.r2.p3.dark"),
                w.j("LEDGER().history.r2.p3.score")
            ],
            [json!(7), json!(2), json!(0), json!(4)]
        );
        assert_eq!(w.j("LEDGER().history.r2.p3.age"), json!("HEROIC"));
        assert_eq!(
            [w.house(3, "golden"), w.house(3, "dark")],
            [json!(1), json!(1)]
        );
        assert_eq!(
            [
                w.j("LEDGER().history.r2.p0.golden"),
                w.j("LEDGER().history.r2.p0.dark"),
                w.j("LEDGER().history.r2.p0.age")
            ],
            [json!(2), json!(0), json!("NORMAL")]
        );
        assert_eq!(w.house(0, "dark"), json!(1));
        w.turns(74, 110);
        assert_eq!(
            [
                w.j("LEDGER().history.r3.p2.golden"),
                w.j("LEDGER().history.r3.p2.dark")
            ],
            [json!(3), json!(1)]
        );
        w.run("LEDGER().houses.p2.dark=9");
        w.turns(111, 119);
        assert_eq!(
            [
                w.j("LEDGER().history.r4.p2.golden"),
                w.j("LEDGER().history.r4.p2.dark")
            ],
            [json!(2), json!(0)]
        );
    }
    #[test]
    fn test_reign_turns_follow_the_calendar() {
        let w = World::new();
        w.turns(1, 200);
        assert_eq!(
            (1..=6)
                .map(|i| w.j(&format!("LEDGER().history.r{i}.turn")))
                .collect::<Vec<_>>(),
            vec![
                json!(66),
                json!(73),
                json!(110),
                json!(119),
                json!(178),
                json!(189)
            ]
        );
        assert_eq!(w.j("LEDGER().history.r7"), json!(null));
        assert_eq!(w.j("LEDGER().reign"), json!(6));
        assert_eq!(w.j("#NOTES"), json!(24));
        assert_eq!(w.j("(function() for _,n in ipairs(NOTES) do if n[1]==0 and n[2]==2 and n[3]=='LOC_JAC_REIGN_KOJI_NOTIFICATION' and n[4]=='LOC_JAC_AGE_DARK_ELITE_ANNOUNCEMENT' and n[5]==66 then return true end end return false end)()"),json!(true));
        let headings = w.j(
            "(function() local h={} for _,n in ipairs(NOTES) do h[n[3]]=true end return h end)()",
        );
        assert_eq!(
            headings,
            json!({"LOC_JAC_REIGN_KOJI_NOTIFICATION":true,"LOC_JAC_REIGN_EIROKU_NOTIFICATION":true,"LOC_JAC_REIGN_GENKI_NOTIFICATION":true,"LOC_JAC_REIGN_TENSHO_NOTIFICATION":true,"LOC_JAC_REIGN_BUNROKU_NOTIFICATION":true,"LOC_JAC_REIGN_KEICHO_NOTIFICATION":true})
        );
    }
    #[test]
    fn test_projection_for_the_era_score_screen_matches_the_next_judgement() {
        let w = World::new();
        w.turns(1, 40);
        assert_eq!(w.j("LEDGER().nextDue"), json!(66));
        let projected = (0..4)
            .map(|p| [w.house(p, "next.golden"), w.house(p, "next.dark")])
            .collect::<Vec<_>>();
        w.turns(41, 66);
        assert_eq!(
            projected,
            (0..4)
                .map(|p| [
                    w.j(&format!("LEDGER().history.r1.p{p}.golden")),
                    w.j(&format!("LEDGER().history.r1.p{p}.dark"))
                ])
                .collect::<Vec<_>>()
        );
        assert_eq!(w.j("LEDGER().nextDue"), json!(73));
        w.turn(1757);
        assert_eq!(
            w.j("LEDGER().reign"),
            json!(reigns()["reigns"].as_array().unwrap().len())
        );
        assert_eq!(w.j("LEDGER().nextDue"), json!(null));
        assert_eq!(w.house(0, "next"), json!(null));
    }
    #[test]
    fn test_new_friendship_scores_one_point_each_once_per_period() {
        let w = World::new();
        w.turns(1, 5);
        w.run("FRIENDS['2-3']=true");
        w.turn(6);
        assert_eq!(
            [w.parts(2), w.parts(3)],
            [json!({"ally":1}), json!({"ally":1})]
        );
        w.run("FRIENDS['2-3']=false");
        w.turn(7);
        w.run("FRIENDS['2-3']=true");
        w.turn(8);
        assert_eq!(
            [w.house(2, "score"), w.house(3, "score")],
            [json!(1), json!(1)]
        );
        w.run("ALLIED['2-3']=true");
        w.turn(9);
        assert_eq!(
            [w.parts(2), w.parts(3)],
            [json!({"ally":3}), json!({"ally":3})]
        );
        w.run("FRIENDS['2-3']=false");
        w.turns(10, 66);
        w.run("FRIENDS['2-3']=true");
        w.turn(67);
        assert_eq!(
            [w.house(2, "score"), w.house(3, "score")],
            [json!(1), json!(1)]
        );
    }
    #[test]
    fn test_friendship_of_the_start_and_of_one_team_never_scores() {
        let w = World::new();
        w.relations(false);
        w.run("TEAM[1]=0");
        w.turns(1, 5);
        w.run("Game:SetProperty('JAC_1534_ALLIANCE_ODA-TAKEDA',false); FRIENDS['0-1']=false");
        w.turn(6);
        w.run("FRIENDS['0-1']=true");
        w.lapse_at_32();
        assert_eq!(
            (0..4).map(|p| w.house(p, "score")).collect::<Vec<_>>(),
            vec![json!(0); 4]
        );
        assert_eq!(w.j("LEDGER().friends['2-3']"), json!("none"));
        w.run("FRIENDS['2-3']=true");
        w.turn(35);
        assert_eq!(
            (0..4).map(|p| w.house(p, "score")).collect::<Vec<_>>(),
            vec![json!(0), json!(0), json!(1), json!(1)]
        );
    }
    #[test]
    fn test_friendship_existing_at_initialization_does_not_score() {
        let w = World::new();
        w.run("FRIENDS['1-2']=true");
        w.turns(1, 5);
        assert_eq!(w.house(1, "score"), json!(0));
    }
    #[test]
    fn test_without_the_gameplay_friendship_call_nothing_scores_and_the_log_says_so() {
        let w = World::new();
        w.run("NO_FRIENDSHIP_API=true");
        w.turns(1, 5);
        w.run("FRIENDS['2-3']=true");
        w.turns(6, 8);
        assert_eq!(w.house(2, "score"), json!(0));
        assert_eq!(w.j("(function() local n=0 for _,x in ipairs(LOG) do if string.find(x,'no gameplay HasDeclaredFriendship',1,true) then n=n+1 end end return n end)()"),json!(1));
        w.run("ALLIED['2-3']=true");
        w.turn(9);
        assert_eq!(w.parts(2), json!({"ally":2}));
    }
    #[test]
    fn test_reign_table_from_koji_to_chidai() {
        let rs = reigns();
        let list = rs["reigns"].as_array().unwrap();
        assert_eq!(list.len(), 47);
        assert_eq!(
            list.iter()
                .take(8)
                .map(|r| r["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["KOJI", "EIROKU", "GENKI", "TENSHO", "BUNROKU", "KEICHO", "GENNA", "KAN_EI"]
        );
        assert_eq!(
            list.iter()
                .skip(41)
                .map(|r| r["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["MEIJI", "TAISHO", "SHOWA", "HEISEI", "REIWA", "CHIDAI"]
        );
        let dates = list
            .iter()
            .map(|r| (r["year"].as_i64().unwrap(), r["month"].as_i64().unwrap()))
            .collect::<Vec<_>>();
        let mut sorted = dates.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(dates, sorted);
        assert_eq!(&dates[45..], &[(2019, 5), (2119, 5)]);
        assert_eq!(
            list.iter()
                .filter(|r| !r["forced_age"].is_null())
                .map(|r| r["id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["CHIDAI"]
        );
        let ids = list
            .iter()
            .map(|r| r["id"].as_str().unwrap())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(ids.len(), list.len());
    }
    #[test]
    fn test_reign_turns_at_every_speed_match_the_generator_and_never_share_a_turn() {
        assert_eq!(
            &engine_turns("GAMESPEED_STANDARD")[..8],
            &[66, 73, 110, 119, 178, 189, 246, 271]
        );
        assert_eq!(&engine_turns("GAMESPEED_STANDARD")[45..], &[1457, 1757]);
        assert_eq!(engine_turns("GAMESPEED_STANDARD"), standard_turns());
        assert_eq!(&engine_turns("GAMESPEED_QUICK")[..2], &[44, 48]);
        for speed in SPEEDS {
            let w = World::with_civs(
                &[
                    "CIVILIZATION_JAC_ODA",
                    "CIVILIZATION_JAC_TAKEDA",
                    "CIVILIZATION_JAC_HOJO",
                    "CIVILIZATION_JAC_IMAGAWA",
                ],
                speed,
                true,
            );
            w.turn(1);
            let mut dues = Vec::new();
            while let Some(due) = w.j("LEDGER().nextDue").as_i64() {
                dues.push(due);
                w.turn(due);
                assert_eq!(
                    w.j(&format!("LEDGER().history.r{}.turn", dues.len())),
                    json!(due),
                    "{speed}"
                );
            }
            assert_eq!(dues, engine_turns(speed), "{speed}");
            let mut sorted = dues.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(dues, sorted, "{speed}");
        }
    }
    #[test]
    fn test_engine_era_change_judges_the_reign_and_the_period_ends_there() {
        let w = World::new();
        w.turns(1, 59);
        w.run("ENGINE_ERA=0");
        w.turn(60);
        assert_eq!(w.j("LEDGER().reign"), json!(0));
        w.run("ENGINE_ERA=1");
        w.turn(61);
        assert_eq!(
            [
                w.j("LEDGER().reign"),
                w.j("LEDGER().reignStart"),
                w.j("LEDGER().history.r1.turn"),
                w.j("LEDGER().history.r1.length"),
                w.j("LEDGER().history.r1.engineEra")
            ],
            [json!(1), json!(61), json!(61), json!(60), json!(1)]
        );
        w.turns(62, 72);
        assert_eq!(w.j("LEDGER().reign"), json!(1));
        w.turn(73);
        assert_eq!(
            [
                w.j("LEDGER().reign"),
                w.j("LEDGER().history.r2.turn"),
                w.j("LEDGER().history.r2.length"),
                w.j("LEDGER().history.r2.engineEra")
            ],
            [json!(2), json!(73), json!(12), json!(1)]
        );
        assert_eq!(w.j("(function() for _,line in ipairs(LOG) do if string.find(line,'judged reign=REIGN_EIROKU turn=73 length=12 era=2 engineEra=1',1,true) then return true end end return false end)()"),json!(true));
    }
    #[test]
    fn test_reigns_are_the_engine_eras_in_order() {
        let tables = ages_tables();
        let eras = tables["Eras"].as_array().unwrap();
        let rs = tables["JAC_SengokuReigns"].as_array().unwrap();
        let index = eras
            .iter()
            .enumerate()
            .map(|(i, e)| (e["EraType"].as_str().unwrap(), i))
            .collect::<std::collections::HashMap<_, _>>();
        let mut ordered = rs.clone();
        ordered.sort_by_key(|r| (r["Year"].as_i64().unwrap(), r["Month"].as_i64().unwrap()));
        assert_eq!(
            ordered
                .iter()
                .map(|r| index[r["EraType"].as_str().unwrap()])
                .collect::<Vec<_>>(),
            (1..=rs.len()).collect::<Vec<_>>()
        );
    }
    #[test]
    fn test_chidai_makes_every_house_dark_whatever_its_score_and_lasts_to_the_end() {
        let w = World::new();
        w.turns(1, 5);
        w.turn(1457);
        assert_eq!(w.j("LEDGER().history.r46.turn"), json!(1457));
        w.run("LEDGER().houses.p1.age='DARK'");
        assert_eq!(w.house(0, "next.forced"), json!("DARK"));
        w.run("for p=0,3 do LEDGER().houses['p'..p].score=1000 end");
        w.turn(1757);
        assert_eq!(
            [
                w.j("LEDGER().history.r47.turn"),
                w.j("LEDGER().history.r47.forced")
            ],
            [json!(1757), json!("DARK")]
        );
        assert_eq!(
            (0..4)
                .map(|p| w.j(&format!("LEDGER().history.r47.p{p}.age")))
                .collect::<Vec<_>>(),
            vec![json!("DARK"); 4]
        );
        assert_eq!(
            (0..4)
                .map(|p| w.j(&format!("LEDGER().history.r47.p{p}.score")))
                .collect::<Vec<_>>(),
            vec![json!(1000); 4]
        );
        assert_eq!(
            (0..4).map(|p| w.house(p, "policy")).collect::<Vec<_>>(),
            vec![
                json!("JAC_AGE_POLICY_ELITE"),
                json!("JAC_AGE_POLICY_ELITE"),
                json!("JAC_AGE_POLICY_VALOR"),
                json!("JAC_AGE_POLICY_TEMPLE")
            ]
        );
        w.choose(1, Some("JAC_AGE_POLICY_PIRATE"), 47);
        assert_eq!(
            [w.house(1, "policy"), w.house(1, "chosen")],
            [json!("JAC_AGE_POLICY_PIRATE"), json!(47)]
        );
        assert_eq!(w.j("LEDGER().nextDue"), json!(null));
        w.turn(3000);
        assert_eq!(
            [w.j("LEDGER().reign"), w.house(1, "age")],
            [json!(47), json!("DARK")]
        );
        assert_eq!(w.j("LEDGER().history.r48"), json!(null));
    }
    #[test]
    fn test_property_writer_is_idempotent_with_one_anchor_per_house() {
        let w = World::new();
        w.turns(1, 66);
        w.run("LEDGER().houses.p1.age='GOLDEN'; LEDGER().houses.p1.policy=nil");
        w.turn(67);
        assert_eq!(w.plot_property(12, 12, "JAC_AGE_ANCHOR_TEMPLE"), json!(1));
        w.run("FOUND(3,11,12,false)");
        let first = w.plot_properties();
        w.turn(68);
        w.turn(68);
        assert_eq!(w.plot_properties(), first);
        for x in [11, 12, 13, 14] {
            assert_eq!(w.plot_property(x, 12, "JAC_AGE_POLICY_TEMPLE"), json!(1));
            assert_eq!(
                w.plot_property(x, 12, "JAC_AGE_ANCHOR_TEMPLE"),
                json!(if x == 11 { 1 } else { 0 })
            );
            assert_eq!(w.plot_property(x, 12, "JAC_AGE_GOLDEN"), json!(0));
        }
        for x in [5, 6] {
            assert_eq!(w.plot_property(x, 5, "JAC_AGE_GOLDEN"), json!(1));
            assert_eq!(w.plot_property(x, 5, "JAC_AGE_ANCHOR_ELITE"), json!(0));
        }
        assert_eq!(
            w.plot_properties()
                .as_object()
                .unwrap()
                .values()
                .flat_map(|v| v.as_array().unwrap()[5..].iter())
                .filter(|v| **v == json!(1))
                .count(),
            3
        );
    }
    #[test]
    fn test_normal_house_has_no_policy_and_dark_house_flags_every_city() {
        let w = World::new();
        w.turns(1, 30);
        w.run("for i=1,5 do Players[0].research.techs[i]=true end");
        w.turns(31, 67);
        assert_eq!(
            [
                w.j("LEDGER().history.r1.p0.age"),
                w.j("LEDGER().history.r1.p3.age")
            ],
            [json!("NORMAL"), json!("DARK")]
        );
        assert_eq!(w.house(0, "policy"), json!(null));
        assert_eq!(w.house(3, "policy"), json!("JAC_AGE_POLICY_TEMPLE"));
        for x in [1, 2] {
            assert_eq!(
                w.plot_properties()[format!("{}", 100 + x)],
                json!([0, 0, 0, 0, 0, 0, 0, 0, 0])
            );
        }
        for x in [12, 13, 14] {
            assert_eq!(w.plot_property(x, 12, "JAC_AGE_POLICY_TEMPLE"), json!(1));
            assert_eq!(
                w.plot_property(x, 12, "JAC_AGE_ANCHOR_TEMPLE"),
                json!(if x == 12 { 1 } else { 0 })
            );
        }
    }
    #[test]
    fn test_dark_houses_take_their_own_default_and_unlisted_civs_the_first_priority() {
        let w = World::new();
        w.run("PlayerConfigurations[2].GetCivilizationTypeName=function() return 'CIVILIZATION_OTHER_MAP' end");
        w.turns(1, 66);
        assert_eq!(
            (0..4).map(|p| w.house(p, "policy")).collect::<Vec<_>>(),
            vec![
                json!("JAC_AGE_POLICY_ELITE"),
                json!("JAC_AGE_POLICY_ELITE"),
                json!("JAC_AGE_POLICY_VALOR"),
                json!("JAC_AGE_POLICY_TEMPLE")
            ]
        );
        assert_eq!(w.house(0, "chosen"), json!(null));
    }
    #[test]
    fn test_dark_house_choice_applies_its_flags_once_per_reign() {
        let w = World::new();
        w.turns(1, 66);
        assert_eq!(w.policy_flags(5, 5)["ELITE"], json!([1, 1]));
        w.choose(1, Some("JAC_AGE_POLICY_PIRATE"), 1);
        assert_eq!(
            [w.house(1, "policy"), w.house(1, "chosen")],
            [json!("JAC_AGE_POLICY_PIRATE"), json!(1)]
        );
        assert_eq!(
            w.policy_flags(5, 5),
            json!({"VALOR":[0,0],"ELITE":[0,0],"PIRATE":[1,1],"TEMPLE":[0,0]})
        );
        assert_eq!(
            w.policy_flags(6, 5),
            json!({"VALOR":[0,0],"ELITE":[0,0],"PIRATE":[1,0],"TEMPLE":[0,0]})
        );
        w.choose(1, Some("JAC_AGE_POLICY_TEMPLE"), 1);
        w.turns(67, 72);
        assert_eq!(w.house(1, "policy"), json!("JAC_AGE_POLICY_PIRATE"));
        assert_eq!(w.policy_flags(5, 5)["PIRATE"], json!([1, 1]));
        assert_eq!(w.j("(function() for _,line in ipairs(LOG) do if string.find(line,'already chosen',1,true) then return true end end return false end)()"),json!(true));
        w.turn(73);
        assert_eq!(
            [w.house(1, "age"), w.house(1, "policy")],
            [json!("NORMAL"), json!(null)]
        );
        w.turns(74, 110);
        assert_eq!(
            [w.house(1, "age"), w.house(1, "policy")],
            [json!("DARK"), json!("JAC_AGE_POLICY_ELITE")]
        );
        w.choose(1, Some("JAC_AGE_POLICY_TEMPLE"), 3);
        assert_eq!(
            [w.house(1, "policy"), w.house(1, "chosen")],
            [json!("JAC_AGE_POLICY_TEMPLE"), json!(3)]
        );
    }
    #[test]
    fn test_invalid_choices_are_dropped() {
        let w = World::new();
        w.turns(1, 30);
        w.run("for i=1,5 do Players[0].research.techs[i]=true end");
        w.turns(31, 66);
        assert_eq!(w.house(0, "age"), json!("NORMAL"));
        let before = w.snapshot();
        w.choose(0, Some("JAC_AGE_POLICY_PIRATE"), 1);
        w.choose(1, Some("JAC_AGE_POLICY_UNKNOWN"), 1);
        w.choose(1, None, 1);
        w.choose(1, Some("JAC_AGE_POLICY_PIRATE"), 0);
        w.choose(62, Some("JAC_AGE_POLICY_PIRATE"), 1);
        w.run("CHOOSE(1,nil)");
        assert_eq!(w.snapshot(), before);
        assert_eq!(w.array("(function() local r={} for _,line in ipairs(LOG) do if string.find(line,'choice dropped',1,true) then r[#r+1]=string.match(line,': (.*)$') end end return r end)()"),vec![json!("not dark"),json!("unknown policy"),json!("unknown policy"),json!("stale period 0"),json!("no house"),json!("stale period nil")]);
    }
    #[test]
    fn test_choice_before_the_first_judgement_is_dropped() {
        let w = World::new();
        w.turns(1, 10);
        w.choose(1, Some("JAC_AGE_POLICY_PIRATE"), 0);
        assert_eq!(w.house(1, "policy"), json!(null));
    }
    #[test]
    fn test_choice_survives_load() {
        let w = World::new();
        w.turns(1, 66);
        w.choose(3, Some("JAC_AGE_POLICY_VALOR"), 1);
        w.reload();
        w.turn(67);
        assert_eq!(
            [w.house(3, "policy"), w.house(3, "chosen")],
            [json!("JAC_AGE_POLICY_VALOR"), json!(1)]
        );
        assert_eq!(w.policy_flags(12, 12)["VALOR"], json!([1, 1]));
        assert_eq!(w.policy_flags(12, 12)["TEMPLE"], json!([0, 0]));
    }
    #[test]
    fn test_unselected_policy_cannot_be_changed_after_the_judgement_turn_even_after_load() {
        let w = World::new();
        w.turns(1, 66);
        w.reload();
        w.turn(67);
        let before = w.snapshot();
        w.choose(1, Some("JAC_AGE_POLICY_TEMPLE"), 1);
        assert_eq!(w.snapshot(), before);
        assert_eq!(w.j("(function() for _,line in ipairs(LOG) do if string.find(line,'not judgement turn',1,true) then return true end end return false end)()"),json!(true));
        assert_eq!(w.house(1, "chosen"), json!(null));
    }
    #[test]
    fn test_unselected_policy_can_be_chosen_after_load_on_the_judgement_turn() {
        let w = World::new();
        w.turns(1, 66);
        w.reload();
        w.choose(1, Some("JAC_AGE_POLICY_TEMPLE"), 1);
        assert_eq!(
            [w.house(1, "policy"), w.house(1, "chosen")],
            [json!("JAC_AGE_POLICY_TEMPLE"), json!(1)]
        );
    }
    #[test]
    fn test_ai_keeps_its_default_and_only_dark_humans_get_the_reopen_notification() {
        let w = World::new();
        w.turns(1, 66);
        let before = w.snapshot();
        w.choose(0, Some("JAC_AGE_POLICY_TEMPLE"), 1);
        assert_eq!(w.snapshot(), before);
        assert_eq!(w.array("(function() local r={} for _,n in ipairs(NOTIFICATIONS) do r[#r+1]=n.kind end return r end)()"),vec![json!(2),json!(3),json!(2),json!(3)]);
        assert_eq!(w.array("(function() local r={} for _,n in ipairs(NOTIFICATIONS) do if n.data.JAC_DARK_POLICY_REIGN then r[#r+1]=n.data.JAC_DARK_POLICY_REIGN end end return r end)()"),vec![json!(1),json!(1)]);
        assert_eq!(
            w.j("NotificationManager.FindType(3,1):CanUserDismiss()"),
            json!(false)
        );
    }
    #[test]
    fn test_load_does_not_count_again() {
        let w = World::new();
        w.turns(1, 5);
        w.run("CAPTURE(1,2,1); Players[1].research.techs[3]=true");
        w.turn(6);
        let before = w.snapshot();
        w.reload();
        assert_eq!(w.snapshot(), before);
        w.turns(7, 8);
        assert_eq!(w.parts(1), json!({"castle":2,"research":1}));
    }
}

mod dark_policy_screen_tests {
    use super::*;
    fn setup() -> World {
        let w = World::new();
        w.ui_base();
        w.run(&mod_text("UI/JAC_SengokuDarkPolicy.lua"));
        w
    }
    fn shown(w: &World) -> bool {
        w.j("not ContextPtr.hidden") == json!(true)
    }
    #[test]
    fn test_screen_lists_the_policies_and_a_click_applies_the_choice() {
        let w = setup();
        w.turns_with_oath(1, 65);
        w.run("LOCAL_TURN_BEGIN()");
        assert!(!shown(&w));
        w.turns_with_oath(66, 66);
        w.run("LOCAL_TURN_BEGIN(); LuaEvents.JAC_SengokuReignPopup_Closed(true)");
        assert!(shown(&w));
        assert_eq!(w.array("(function() local r={} for _,row in ipairs(ROWS) do r[#r+1]=row.Name.text end return r end)()"),vec![json!("LOC_JAC_AGE_POLICY_VALOR_NAME"),json!("LOC_JAC_AGE_POLICY_ELITE_NAME"),json!("LOC_JAC_AGE_POLICY_PIRATE_NAME"),json!("LOC_JAC_AGE_POLICY_TEMPLE_NAME")]);
        assert_eq!(
            w.j("ROWS[3].Description.text"),
            json!("LOC_JAC_AGE_POLICY_PIRATE_DESCRIPTION")
        );
        assert_eq!(
            w.j("Controls.Body.text"),
            json!("LOC_JAC_AGE_DARK_CHOICE_BODY(LOC_JAC_AGE_POLICY_ELITE_NAME)")
        );
        w.run("ROWS[3].Choose.click()");
        assert!(!shown(&w));
        assert_eq!(
            [w.house(1, "policy"), w.house(1, "chosen")],
            [json!("JAC_AGE_POLICY_PIRATE"), json!(1)]
        );
        assert_eq!(w.policy_flags(5, 5)["PIRATE"], json!([1, 1]));
        w.turns_with_oath(67, 67);
        w.run("LOCAL_TURN_BEGIN()");
        assert!(!shown(&w));
    }
    #[test]
    fn test_later_closes_until_the_notification_and_nothing_is_sent_before_the_turn_is_active() {
        let w = setup();
        w.turns_with_oath(1, 66);
        w.run(
            "TURN_ACTIVE=false; LOCAL_TURN_BEGIN(); LuaEvents.JAC_SengokuReignPopup_Closed(true)",
        );
        assert!(shown(&w));
        w.run("ROWS[3].Choose.click()");
        assert_eq!(w.j("#REQUESTS"), json!(0));
        assert!(shown(&w));
        w.run("Controls.LaterButton.click()");
        assert!(!shown(&w));
        w.run("TURN_ACTIVE=true; LOCAL_TURN_BEGIN(); LuaEvents.JAC_SengokuReignPopup_Closed(true)");
        assert!(!shown(&w));
        w.run("LuaEvents.JAC_SengokuDarkPolicy_Open(1)");
        assert!(shown(&w));
        assert_eq!(w.j("JAC_DARK_POLICY_SHOW_COUNT"), json!(2));
        assert_eq!(
            [w.house(1, "policy"), w.house(1, "chosen")],
            [json!("JAC_AGE_POLICY_ELITE"), json!(null)]
        );
    }
    #[test]
    fn test_turn_end_closes_and_the_next_turn_cannot_choose_even_if_unchosen() {
        let w = setup();
        w.turns_with_oath(1, 66);
        w.run("LOCAL_TURN_BEGIN(); LuaEvents.JAC_SengokuReignPopup_Closed(true)");
        assert!(shown(&w));
        w.run("LOCAL_TURN_END()");
        assert!(!shown(&w));
        assert_eq!(w.j("#REQUESTS"), json!(0));
        w.turns_with_oath(67, 67);
        w.run("LOCAL_TURN_BEGIN(); LuaEvents.JAC_SengokuReignPopup_Closed(true); LuaEvents.JAC_SengokuDarkPolicy_Open(1)");
        assert!(!shown(&w));
        assert_eq!(
            w.j("JAC_ChooseDarkPolicy('JAC_AGE_POLICY_TEMPLE')"),
            json!(false)
        );
        assert_eq!(
            [w.house(1, "policy"), w.house(1, "chosen")],
            [json!("JAC_AGE_POLICY_ELITE"), json!(null)]
        );
    }
    #[test]
    fn test_dismissed_screen_keeps_the_blocker_until_an_accepted_choice() {
        let w = setup();
        w.turns_with_oath(1, 66);
        w.run("LuaEvents.JAC_SengokuReignPopup_Closed(true); TEST_NOTIFICATION=NotificationManager.FindType(3,1); OnClose(); PUBLISH()");
        assert_eq!(w.j("TEST_NOTIFICATION.dismissed"), json!(false));
        w.run("LuaEvents.JAC_SengokuDarkPolicy_Open(1)");
        assert_eq!(
            w.j("JAC_ChooseDarkPolicy('JAC_AGE_POLICY_TEMPLE')"),
            json!(true)
        );
        assert_eq!(w.j("TEST_NOTIFICATION.dismissed"), json!(true));
        assert_eq!(
            [w.house(1, "policy"), w.house(1, "chosen")],
            [json!("JAC_AGE_POLICY_TEMPLE"), json!(1)]
        );
    }
    #[test]
    fn test_rejected_request_does_not_clear_the_blocker() {
        let w = setup();
        w.turns_with_oath(1, 66);
        w.run("TEST_NOTIFICATION=NotificationManager.FindType(3,1); CHOOSE(1,{PolicyType='JAC_AGE_POLICY_TEMPLE',Reign=0}); PUBLISH()");
        assert_eq!(w.j("TEST_NOTIFICATION.dismissed"), json!(false));
        assert_eq!(w.house(1, "chosen"), json!(null));
    }
    #[test]
    fn test_cleanup_scans_past_official_notifications_and_keeps_other_houses() {
        let w = setup();
        w.run("NotificationManager.SendNotification(1,71,{MESSAGE='official'}); TEST_OFFICIAL=NotificationManager.FindType(71,1)");
        w.turns_with_oath(1, 66);
        w.run("TEST_CURRENT=NotificationManager.FindType(3,1); TEST_OTHER=NotificationManager.FindType(3,3); NotificationManager.SendNotification(1,3,{JAC_DARK_POLICY_REIGN=0}); TEST_STALE=NOTIFICATIONS[#NOTIFICATIONS]; PUBLISH()");
        assert_eq!(w.j("TEST_STALE.dismissed"), json!(true));
        assert_eq!(w.j("TEST_OFFICIAL.dismissed"), json!(false));
        assert_eq!(w.j("TEST_CURRENT.dismissed"), json!(false));
        assert_eq!(
            w.j("JAC_ChooseDarkPolicy('JAC_AGE_POLICY_TEMPLE')"),
            json!(true)
        );
        assert_eq!(w.j("TEST_CURRENT.dismissed"), json!(true));
        assert_eq!(w.j("TEST_OFFICIAL.dismissed"), json!(false));
        assert_eq!(w.j("TEST_OTHER.dismissed"), json!(false));
    }
    #[test]
    fn test_load_keeps_pending_notification_and_does_not_show_the_screen_automatically() {
        let w = setup();
        w.turns_with_oath(1, 66);
        w.run("LuaEvents.JAC_SengokuReignPopup_Closed(true); OnClose(); LOAD_SCREEN_CLOSE(); LOCAL_TURN_BEGIN()");
        assert!(!shown(&w));
        assert_eq!(
            w.j("NotificationManager.FindType(3,1).dismissed"),
            json!(false)
        );
        w.run("LuaEvents.JAC_SengokuDarkPolicy_Open(1)");
        assert!(shown(&w));
    }
    #[test]
    fn test_cleanup_waits_for_the_notification_judgement_to_reach_the_ui() {
        let w = setup();
        w.turns_with_oath(1, 65);
        w.run("TEST_OLD=LEDGER()");
        w.turns_with_oath(66, 66);
        w.run("TEST_CURRENT=NotificationManager.FindType(3,1); TEST_SAVED=LEDGER(); Game:SetProperty('JAC_AGES_LEDGER',TEST_OLD); PUBLISH()");
        assert_eq!(w.j("TEST_CURRENT.dismissed"), json!(false));
        w.run("Game:SetProperty('JAC_AGES_LEDGER',nil); PUBLISH()");
        assert_eq!(w.j("TEST_CURRENT.dismissed"), json!(false));
        w.run("Game:SetProperty('JAC_AGES_LEDGER',TEST_SAVED); PUBLISH()");
        assert_eq!(w.j("TEST_CURRENT.dismissed"), json!(false));
        w.turns_with_oath(67, 67);
        w.run("PUBLISH()");
        assert_eq!(w.j("TEST_CURRENT.dismissed"), json!(true));
    }
    #[test]
    fn test_not_shown_for_a_house_that_is_not_dark() {
        let w = setup();
        w.turns_with_oath(1, 30);
        w.run("for i=1,5 do Players[1].research.techs[i]=true end");
        w.turns_with_oath(31, 66);
        w.run("LOCAL_TURN_BEGIN(); LuaEvents.JAC_SengokuReignPopup_Closed(true)");
        assert_eq!(w.house(1, "age"), json!("NORMAL"));
        assert!(!shown(&w));
    }
}

mod koji_distribution_tests {
    use super::*;
    #[test]
    fn test_observed_koji_scores_land_mostly_normal() {
        let observed: serde_json::Value =
            serde_json::from_str(&fixture("koji_observed.json")).unwrap();
        let civs = (0..observed.as_array().unwrap().len())
            .map(|i| format!("CIVILIZATION_JAC_H{i}"))
            .collect::<Vec<_>>();
        let names = civs.iter().map(|s| s.as_str()).collect::<Vec<_>>();
        let w = World::with_civs_and_script(&names, "GAMESPEED_STANDARD", false, false);
        for (pid, pair) in observed.as_array().unwrap().iter().enumerate() {
            let cities = pair[1].as_i64().unwrap();
            for n in 0..cities {
                w.run(&format!("FOUND({pid},{n},{pid},{},true)", n == 0));
            }
        }
        w.load_script();
        for turn in 1..=66 {
            if turn == 30 {
                for (pid, pair) in observed.as_array().unwrap().iter().enumerate() {
                    for i in 0..pair[0].as_i64().unwrap() {
                        let field = if i < 10 { "techs" } else { "civics" };
                        w.run(&format!("Players[{pid}].research.{field}[{}]=true", i % 10));
                    }
                }
            }
            w.turn(turn);
        }
        assert_eq!(
            observed
                .as_array()
                .unwrap()
                .iter()
                .enumerate()
                .map(|(p, _)| w.j(&format!("LEDGER().history.r1.p{p}.score")))
                .collect::<Vec<_>>(),
            observed
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v[0].clone())
                .collect::<Vec<_>>()
        );
        let ages = (0..observed.as_array().unwrap().len())
            .map(|p| w.j(&format!("LEDGER().history.r1.p{p}.age")))
            .collect::<Vec<_>>();
        assert_eq!(ages.iter().filter(|a| **a == json!("GOLDEN")).count(), 3);
        assert_eq!(ages.iter().filter(|a| **a == json!("NORMAL")).count(), 19);
        assert_eq!(ages.iter().filter(|a| **a == json!("DARK")).count(), 0);
    }
}

mod short_reign_distribution_tests {
    use super::*;
    fn ages(old_rule: bool) -> Vec<[usize; 4]> {
        let mut script = mod_text("Scripts/JAC_Sengoku_Ages.lua");
        if old_rule {
            for (new, old) in [
                ("math.max(0, math.floor(", "math.max(1, math.ceil("),
                ("math.max(MIN_GOLDEN, ", "math.max(dark + 1, "),
            ] {
                assert_eq!(script.matches(new).count(), 1);
                script = script.replace(new, old);
            }
        }
        let civs = (0..22)
            .map(|i| format!("CIVILIZATION_JAC_H{i}"))
            .collect::<Vec<_>>();
        let names = civs.iter().map(|s| s.as_str()).collect::<Vec<_>>();
        let w = World::with_civs_and_script(&names, "GAMESPEED_STANDARD", false, false);
        w.turn(1);
        w.run(&script);
        w.turn(2);
        let health: serde_json::Value = serde_json::from_str(&fixture("health_200.json")).unwrap();
        let mut counts = Vec::new();
        for (index, row) in health.as_array().unwrap().iter().enumerate() {
            let due = row[0].as_i64().unwrap();
            let houses = row[1].as_array().unwrap();
            let mut cities = Vec::new();
            for (pid, house) in houses.iter().enumerate() {
                w.run(&format!("ALIVE[{pid}]={}", !house.is_null()));
                if !house.is_null() {
                    w.run(&format!(
                        "LEDGER().houses.p{pid}.score={}",
                        house[0].as_i64().unwrap()
                    ));
                    for n in 0..house[1].as_i64().unwrap() {
                        cities.push(json!({"owner":pid,"x":n,"y":pid,"capital":n==0}));
                    }
                }
            }
            w.lua
                .globals()
                .set(
                    "CITIES",
                    civ6_lua_tests_rs::json_to_lua(&w.lua, &json!(cities)),
                )
                .unwrap();
            w.turn(due);
            let mut record = [0; 4];
            for (pid, house) in houses.iter().enumerate() {
                if !house.is_null() {
                    let age = w.j(&format!("LEDGER().history.r{}.p{pid}.age", index + 1));
                    record[["GOLDEN", "HEROIC", "NORMAL", "DARK"]
                        .iter()
                        .position(|s| age == json!(s))
                        .unwrap()] += 1;
                }
            }
            counts.push(record);
        }
        counts
    }
    #[test]
    fn test_the_old_rule_gives_back_the_logged_ages() {
        let expected: serde_json::Value =
            serde_json::from_str(&fixture("health_200_old.json")).unwrap();
        assert_eq!(
            ages(true).into_iter().map(|a| json!(a)).collect::<Vec<_>>(),
            expected.as_array().unwrap().clone()
        );
    }
    #[test]
    fn test_short_periods_leave_most_houses_normal() {
        let counts = ages(false);
        for [golden, heroic, normal, dark] in &counts {
            let total = golden + heroic + normal + dark;
            assert!(dark * 3 <= total);
            assert!(*normal > (*golden + *heroic).max(*dark));
        }
        assert!(counts.iter().any(|r| r[0] + r[1] > 0));
        assert_eq!(
            counts,
            vec![
                [4, 0, 17, 1],
                [0, 0, 22, 0],
                [5, 0, 13, 3],
                [1, 1, 19, 0],
                [6, 0, 10, 5],
                [4, 0, 15, 2]
            ]
        );
    }
}

mod reign_files_tests {
    use super::*;
    #[test]
    fn test_generated_reign_files_are_current() {
        for (path, expected) in reigns_generator::outputs() {
            assert_eq!(mod_text(path), expected, "generated file {path}");
        }
    }
    #[test]
    fn test_every_engine_era_lasts_at_least_two_standard_turns_in_order() {
        let eras = era_rows();
        let rs = reigns();
        assert_eq!(eras.len(), 1 + rs["reigns"].as_array().unwrap().len());
        assert_eq!(
            eras.iter()
                .map(|e| e["ChronologyIndex"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            (1..=eras.len() as u64).collect::<Vec<_>>()
        );
        assert_eq!(
            eras.iter()
                .map(|e| e["EraType"].as_str().unwrap())
                .collect::<std::collections::HashSet<_>>()
                .len(),
            eras.len()
        );
        assert!(eras[..eras.len() - 1]
            .iter()
            .all(|e| e["length"].as_i64().unwrap() >= 2));
        assert_eq!(
            eras[..eras.len() - 1]
                .iter()
                .map(|e| e["length"].as_i64().unwrap())
                .min(),
            Some(3)
        );
        assert!(eras.last().unwrap()["length"].is_null());
        for speed in SPEEDS {
            let turns = engine_turns(speed);
            let mut prev = 1;
            for turn in turns {
                assert!(turn - prev >= 2, "{speed}");
                prev = turn;
            }
        }
    }
    #[test]
    fn test_the_calendar_dates_each_engine_era_with_its_reign_at_every_speed() {
        let stages = calendar();
        let months = month_offsets();
        for speed in SPEEDS {
            let rows = &stages[speed];
            assert_eq!(
                rows.iter().collect::<std::collections::HashSet<_>>().len(),
                rows.len(),
                "{speed}"
            );
            assert_eq!(
                rows.iter().map(|(_, turns)| turns).sum::<i64>(),
                10000,
                "{speed}"
            );
            let engine = engine_turns(speed);
            assert_eq!(
                engine,
                months
                    .iter()
                    .map(|&m| calendar_turn(m, rows))
                    .collect::<Vec<_>>(),
                "{speed}"
            );
            let mut begins = vec![0];
            let mut current = 0;
            for &(increment, turns) in rows {
                for _ in 0..turns {
                    current += increment;
                    begins.push(current);
                }
            }
            let early = months
                .iter()
                .zip(engine.iter())
                .map(|(&month, &turn)| month - begins[turn as usize - 1])
                .collect::<Vec<_>>();
            assert!(
                early
                    .iter()
                    .all(|&e| e >= 0 && e <= if speed == "GAMESPEED_STANDARD" { 3 } else { 2 }),
                "{speed}: {early:?}"
            );
            if speed != "GAMESPEED_STANDARD" {
                assert_eq!(*early.last().unwrap(), 0, "{speed}");
            }
        }
        assert_eq!(stages["GAMESPEED_STANDARD"], vec![(4, 10000)]);
        let calendar_doc = mod_text("Data/JAC_Sengoku_Calendar.xml");
        assert!(calendar_doc.contains("<Year>1534</Year>"));
    }
}

mod ages_data_tests {
    use super::*;
    use ages_support::official::fixture_db;
    #[test]
    fn test_pending_policy_notification_survives_turn_end_without_retaining_ordinary_notices() {
        let db = fixture_db();
        assert_eq!(sql_rows(&db,"SELECT NotificationType,ExpiresEndOfTurn,AutoNotify FROM Notifications WHERE NotificationType IN ('NOTIFICATION_USER_DEFINED_2','NOTIFICATION_USER_DEFINED_3') ORDER BY NotificationType"),vec![vec![json!("NOTIFICATION_USER_DEFINED_2"),json!(1),json!(0)],vec![json!("NOTIFICATION_USER_DEFINED_3"),json!(0),json!(0)]]);
    }
    #[test]
    fn test_official_ages_and_dedications_are_off() {
        let db = fixture_db();
        let mut params=sql_rows(&db,"SELECT Name,Value FROM GlobalParameters WHERE Name LIKE '%AGE_SCORE_BASE_THRESHOLD' OR Name LIKE 'THRESHOLD_SHIFT_%' OR Name LIKE 'COMMEMORATE_%'").into_iter().map(|r|(r[0].as_str().unwrap().to_string(),r[1].as_str().unwrap().to_string())).collect::<std::collections::HashMap<_,_>>();
        assert_eq!(
            params.remove("DARK_AGE_SCORE_BASE_THRESHOLD").as_deref(),
            Some("-9999")
        );
        assert_eq!(
            params.remove("GOLDEN_AGE_SCORE_BASE_THRESHOLD").as_deref(),
            Some("9999")
        );
        assert_eq!(
            params.remove("COMMEMORATE_BASE_CHOICES_ALLOWED").as_deref(),
            Some("0")
        );
        assert_eq!(
            params.remove("COMMEMORATE_OPTIONS_MAX").as_deref(),
            Some("0")
        );
        assert_eq!(params.len(), 9);
        assert_eq!(
            params
                .values()
                .map(String::as_str)
                .collect::<std::collections::HashSet<_>>(),
            std::collections::HashSet::from(["0"])
        );
        assert_eq!(
            sql_one(
                &db,
                "SELECT COUNT(*) FROM Eras_XP2 WHERE IFNULL(EraScoreThresholdShift,0)<>0"
            ),
            json!(0)
        );
        assert_eq!(sql_one(&db, "SELECT SUM(EraScore) FROM Moments"), json!(0));
        assert!(
            sql_one(&db, "SELECT COUNT(*) FROM Moments")
                .as_i64()
                .unwrap()
                > 100
        );
    }
    #[test]
    fn test_no_attachment_of_an_era_score_modifier_in_any_table() {
        let db = fixture_db();
        let tables = sql_rows(&db, "SELECT name FROM sqlite_master WHERE type='table'");
        let mut checked = 0;
        for item in tables {
            let table = item[0].as_str().unwrap();
            let columns = sql_rows(&db, &format!("PRAGMA table_info(\"{table}\")"))
                .into_iter()
                .map(|r| r[1].as_str().unwrap().to_ascii_lowercase())
                .collect::<std::collections::HashSet<_>>();
            if !columns.contains("modifierid")
                || ["Modifiers", "ModifierArguments", "ModifierStrings"].contains(&table)
            {
                continue;
            }
            checked += 1;
            assert!(sql_rows(&db,&format!("SELECT x.* FROM \"{table}\" x JOIN Modifiers m ON m.ModifierId=x.ModifierId WHERE m.ModifierType LIKE '%ERA_SCORE%'")).is_empty(),"{table}");
        }
        assert!(checked >= 25, "checked {checked}");
        assert!(sql_rows(&db,"SELECT a.ModifierId FROM ModifierArguments a JOIN Modifiers m ON m.ModifierId=a.Value WHERE a.Name='ModifierId' AND m.ModifierType LIKE '%ERA_SCORE%'").is_empty());
    }
    #[test]
    fn test_official_age_effects_and_age_policies_are_gone() {
        let db = fixture_db();
        assert!(sql_rows(&db,"SELECT ModifierId FROM TraitModifiers WHERE ModifierId IN ('GOLDEN_AGE_CITY_IDENTITY','DARK_AGE_CITY_IDENTITY')").is_empty());
        assert!(sql_rows(&db,"SELECT PolicyType FROM Policies_XP1 JOIN Policies USING (PolicyType) WHERE RequiresDarkAge=1 OR RequiresGoldenAge=1").is_empty());
        assert!(sql_rows(&db,"SELECT * FROM DiplomaticActions WHERE DiplomaticActionType='DIPLOACTION_DECLARE_GOLDEN_AGE_WAR'").is_empty());
    }
    #[test]
    fn test_every_reign_is_an_engine_era_lasting_its_turns() {
        let db = fixture_db();
        let eras=sql_rows(&db,"SELECT e.EraType,e.Name,e.Description,e.ChronologyIndex,x.GameEraMinimumTurns,x.GameEraMaximumTurns FROM Eras e JOIN Eras_XP1 x USING (EraType) ORDER BY e.ChronologyIndex");
        let source = reigns();
        let mut reigns = vec![source["start"].clone()];
        reigns.extend(source["reigns"].as_array().unwrap().iter().cloned());
        assert_eq!(eras.len(), 48);
        assert_eq!(sql_one(&db, "SELECT COUNT(*) FROM Eras"), json!(48));
        assert_eq!(
            eras.iter()
                .map(|e| e[3].as_i64().unwrap())
                .collect::<Vec<_>>(),
            (1..=48).collect::<Vec<_>>()
        );
        assert_eq!(
            eras.iter().map(|e| e[1].clone()).collect::<Vec<_>>(),
            reigns
                .iter()
                .map(|r| json!(format!("LOC_JAC_ERA_{}_NAME", r["id"].as_str().unwrap())))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            eras.iter().map(|e| e[1].clone()).collect::<Vec<_>>(),
            eras.iter().map(|e| e[2].clone()).collect::<Vec<_>>()
        );
        assert_eq!(
            eras[..9]
                .iter()
                .map(|e| e[0].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec![
                "ERA_ANCIENT",
                "ERA_CLASSICAL",
                "ERA_MEDIEVAL",
                "ERA_RENAISSANCE",
                "ERA_INDUSTRIAL",
                "ERA_MODERN",
                "ERA_ATOMIC",
                "ERA_INFORMATION",
                "ERA_FUTURE"
            ]
        );
        assert_eq!(&eras[47][4..], &[json!(null), json!(null)]);
        let mut starts = vec![1_i64];
        for era in &eras[..47] {
            assert_eq!(era[4], era[5], "{}", era[0]);
            let length = era[4].as_i64().unwrap();
            assert!(length >= 2, "{}", era[0]);
            starts.push(starts.last().unwrap() + length);
        }
        assert_eq!(&starts[1..], standard_turns());
        assert_eq!(&starts[1..3], &[66, 73]);
        assert_eq!(
            sql_one(
                &db,
                "SELECT Value FROM GlobalParameters WHERE Name='NEXT_ERA_TURN_COUNTDOWN'"
            ),
            json!("1")
        );
        assert_eq!(
            sql_one(
                &db,
                "SELECT COUNT(*) FROM Eras_XP1 WHERE GameEraMaximumTurns>=1000"
            ),
            json!(0)
        );
        assert_eq!(sql_one(&db, "SELECT COUNT(*) FROM Eras_XP2"), json!(48));
        assert_eq!(
            sql_one(&db, "SELECT COUNT(*) FROM Types WHERE Kind='KIND_ERA'"),
            json!(48)
        );
        let rows=sql_rows(&db,"SELECT e.ChronologyIndex FROM JAC_SengokuReigns r JOIN Eras e USING (EraType) ORDER BY r.Year,r.Month");
        assert_eq!(
            rows.iter()
                .map(|r| r[0].as_i64().unwrap())
                .collect::<Vec<_>>(),
            (2..=48).collect::<Vec<_>>()
        );
        for (table, key, count) in [
            ("Technologies", "TechnologyType", 32),
            ("Civics", "CivicType", 30),
        ] {
            let sql=format!("SELECT e.ChronologyIndex FROM {table} t JOIN Eras e USING (EraType) ORDER BY t.Cost,t.{key}");
            let order = sql_rows(&db, &sql)
                .iter()
                .map(|r| r[0].as_i64().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(order.len(), count, "{table}");
            let mut sorted = order.clone();
            sorted.sort();
            assert_eq!(order, sorted, "{table}");
        }
        let late = "('ERA_INDUSTRIAL','ERA_MODERN','ERA_ATOMIC','ERA_INFORMATION','ERA_FUTURE')";
        assert_eq!(
            sql_one(
                &db,
                &format!("SELECT COUNT(*) FROM GreatPersonIndividuals WHERE EraType IN {late}")
            ),
            json!(0)
        );
        assert_eq!(
            sql_one(&db, "SELECT COUNT(*) FROM GreatPersonIndividuals"),
            json!(75)
        );
        assert_eq!(
            sql_one(
                &db,
                "SELECT COUNT(*) FROM Types WHERE Kind='KIND_GREAT_PERSON_INDIVIDUAL'"
            ),
            json!(75)
        );
        assert_eq!(sql_rows(&db,"SELECT GreatPersonClassType FROM GreatPersonClasses c WHERE NOT EXISTS (SELECT 1 FROM GreatPersonIndividuals i WHERE i.GreatPersonClassType=c.GreatPersonClassType)"),vec![vec![json!("GREAT_PERSON_CLASS_MUSICIAN")]]);
        assert_eq!(sql_one(&db,"SELECT COUNT(*) FROM GreatWorks WHERE GreatPersonIndividualType IS NOT NULL AND GreatPersonIndividualType NOT IN (SELECT GreatPersonIndividualType FROM GreatPersonIndividuals)"),json!(0));
        assert_eq!(sql_one(&db,"SELECT COUNT(*) FROM GreatPersonIndividualActionModifiers WHERE GreatPersonIndividualType NOT IN (SELECT GreatPersonIndividualType FROM GreatPersonIndividuals)"),json!(0));
        assert!(sql_rows(&db,"SELECT VisibilitySourceType FROM DiplomaticVisibilitySources WHERE GreatPersonIndividualType IS NOT NULL").is_empty());
        assert_eq!(
            sql_one(
                &db,
                &format!("SELECT COUNT(*) FROM GreatWorks WHERE EraType IN {late}")
            ),
            json!(0)
        );
        assert_eq!(
            sql_rows(
                &db,
                "SELECT GreatWorkType FROM GreatWorks WHERE EraType='ERA_JAC_KYOHO' ORDER BY 1"
            ),
            (21..=25)
                .map(|n| vec![json!(format!("GREATWORK_ARTIFACT_{n}"))])
                .collect::<Vec<_>>()
        );
        for table in [
            "Eras",
            "Eras_XP1",
            "Technologies",
            "Civics",
            "GreatPersonIndividuals",
            "GreatWorks",
            "GreatPersonIndividualActionModifiers",
            "GreatPersonIndividualBirthModifiers",
            "DiplomaticVisibilitySources",
            "JAC_SengokuReigns",
        ] {
            assert!(
                sql_rows(&db, &format!("PRAGMA foreign_key_check({table})")).is_empty(),
                "{table}"
            );
        }
    }
    #[test]
    fn test_reigns_policies_and_effect_modifiers() {
        let db = fixture_db();
        let source = reigns();
        let actual = sql_rows(
            &db,
            "SELECT ReignType,Year,Month,ForcedAge FROM JAC_SengokuReigns ORDER BY Year,Month",
        );
        let expected = source["reigns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                vec![
                    json!(format!("REIGN_{}", r["id"].as_str().unwrap())),
                    r["year"].clone(),
                    r["month"].clone(),
                    r.get("forced_age").cloned().unwrap_or(json!(null)),
                ]
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        assert_eq!(
            sql_one(&db, "SELECT COUNT(*) FROM JAC_SengokuDarkAgePolicies"),
            json!(4)
        );
        assert_eq!(sql_rows(&db,"SELECT ModifierId FROM TraitModifiers WHERE ModifierId LIKE 'JAC_AGE_%' AND TraitType='TRAIT_LEADER_MAJOR_CIV'").len(),12);
        let mods=sql_rows(&db,"SELECT ModifierId,ModifierType,OwnerRequirementSetId,SubjectRequirementSetId FROM Modifiers WHERE ModifierId LIKE 'JAC_AGE_%'");
        assert_eq!(mods.len(), 20);
        let types = sql_rows(&db, "SELECT ModifierType FROM DynamicModifiers")
            .into_iter()
            .map(|r| r[0].clone())
            .collect::<std::collections::HashSet<_>>();
        let sets = sql_rows(&db, "SELECT RequirementSetId FROM RequirementSets")
            .into_iter()
            .map(|r| r[0].clone())
            .collect::<std::collections::HashSet<_>>();
        for row in &mods {
            assert!(types.contains(&row[1]), "{}", row[0]);
            for rs in [&row[2], &row[3]] {
                if !rs.is_null() {
                    assert!(sets.contains(rs), "{}", row[0]);
                    let id = rs.as_str().unwrap();
                    assert!(!sql_rows(&db,&format!("SELECT 1 FROM RequirementSetRequirements r JOIN Requirements q USING (RequirementId) WHERE r.RequirementSetId='{id}'")).is_empty(),"{id}");
                }
            }
        }
        let ids = mods
            .iter()
            .map(|r| r[0].clone())
            .collect::<std::collections::HashSet<_>>();
        for row in sql_rows(&db,"SELECT Value FROM ModifierArguments WHERE ModifierId LIKE 'JAC_AGE_ATTACH_%' AND Name='ModifierId'") {assert!(ids.contains(&row[0]),"{}",row[0]);}
        assert_eq!(sql_one(&db,"SELECT COUNT(*) FROM UnitAbilities WHERE UnitAbilityType='ABILITY_TWILIGHT_VALOR_ATTACK_BONUS'"),json!(1));
        let props=sql_rows(&db,"SELECT a.Value FROM RequirementArguments a JOIN Requirements q USING (RequirementId) WHERE q.RequirementId LIKE 'JAC_AGE_%' AND a.Name='PropertyName'").into_iter().map(|r|r[0].clone()).collect::<std::collections::HashSet<_>>();
        let policies = sql_rows(&db, "SELECT PolicyType FROM JAC_SengokuDarkAgePolicies")
            .into_iter()
            .map(|r| r[0].as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        let mut written = std::collections::HashSet::from([json!("JAC_AGE_GOLDEN")]);
        for policy in policies {
            written.insert(json!(policy));
            written.insert(json!(policy.replace("_POLICY_", "_ANCHOR_")));
        }
        assert!(props.is_subset(&written), "{props:?}");
    }
    #[test]
    fn test_every_house_has_a_valid_default_dark_policy() {
        let db = fixture_db();
        let mut civs = std::collections::HashSet::new();
        let re = regex::Regex::new(r"\('(CIVILIZATION_JAC_[A-Z_]+)', 'LOC_").unwrap();
        let dir = civ6_lua_tests_rs::repo_root().join("poc/JAC_PoC_CustomCivs/Data");
        for path in std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "sql"))
        {
            let source = std::fs::read_to_string(path).unwrap();
            for captures in re.captures_iter(&source) {
                civs.insert(captures[1].to_string());
            }
        }
        assert_eq!(civs.len(), 22);
        let rows = sql_rows(
            &db,
            "SELECT CivilizationType,PolicyType FROM JAC_SengokuDarkAgePolicyDefaults",
        );
        let defaults = rows
            .iter()
            .map(|r| {
                (
                    r[0].as_str().unwrap().to_string(),
                    r[1].as_str().unwrap().to_string(),
                )
            })
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            defaults
                .keys()
                .cloned()
                .collect::<std::collections::HashSet<_>>(),
            civs
        );
        let policies = sql_rows(&db, "SELECT PolicyType FROM JAC_SengokuDarkAgePolicies")
            .into_iter()
            .map(|r| r[0].as_str().unwrap().to_string())
            .collect::<std::collections::HashSet<_>>();
        let values = defaults
            .values()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        assert!(values.is_subset(&policies));
        assert!(values.len() > 1);
    }
    #[test]
    fn test_text_keys_are_translated() {
        let db = fixture_db();
        let mut keys=sql_rows(&db,"SELECT Name FROM JAC_SengokuReigns UNION SELECT Announcement FROM JAC_SengokuReigns UNION SELECT Notification FROM JAC_SengokuReigns UNION SELECT Name FROM JAC_SengokuDarkAgePolicies UNION SELECT Description FROM JAC_SengokuDarkAgePolicies UNION SELECT Announcement FROM JAC_SengokuDarkAgePolicies").into_iter().map(|r|r[0].as_str().unwrap().to_string()).collect::<std::collections::HashSet<_>>();
        for key in [
            "LOC_JAC_AGE_GOLDEN_ANNOUNCEMENT",
            "LOC_JAC_AGE_HEROIC_ANNOUNCEMENT",
            "LOC_JAC_AGE_NORMAL_ANNOUNCEMENT",
        ] {
            keys.insert(key.into());
        }
        keys.extend(
            sql_rows(
                &db,
                "SELECT Name FROM Eras UNION SELECT Description FROM Eras",
            )
            .into_iter()
            .map(|r| r[0].as_str().unwrap().to_string()),
        );
        keys.extend(sql_rows(&db,"SELECT Name FROM JAC_SengokuOaths UNION SELECT GoldenDescription FROM JAC_SengokuOaths UNION SELECT QuestDescription FROM JAC_SengokuOaths").into_iter().map(|r|r[0].as_str().unwrap().to_string()));
        let ui = format!(
            "{}{}",
            mod_text("UI/JAC_SengokuDarkPolicy.lua"),
            mod_text("UI/JAC_SengokuDarkPolicy.xml")
        );
        let ui_re = regex::Regex::new(r"LOC_JAC_AGE_DARK_CHOICE_[A-Z]+").unwrap();
        let ui_keys = ui_re
            .find_iter(&ui)
            .map(|m| m.as_str().to_string())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(
            ui_keys,
            std::collections::HashSet::from([
                "LOC_JAC_AGE_DARK_CHOICE_TITLE".into(),
                "LOC_JAC_AGE_DARK_CHOICE_BODY".into(),
                "LOC_JAC_AGE_DARK_CHOICE_CHOOSE".into(),
                "LOC_JAC_AGE_DARK_CHOICE_LATER".into()
            ])
        );
        keys.extend(ui_keys);
        let screens = [
            "UI/JAC_SengokuReignPopup.lua",
            "UI/JAC_SengokuReignPopup.xml",
            "UI/JAC_EraProgressPanel_Sengoku.lua",
            "UI/JAC_SengokuAgesView.lua",
            "UI/JAC_ActionPanel_Sengoku.lua",
            "Scripts/JAC_Sengoku_Ages.lua",
            "UI/JAC_DedicationPopup_Sengoku.lua",
        ]
        .iter()
        .map(|p| mod_text(p))
        .collect::<String>();
        let screen_re = regex::Regex::new(r"LOC_JAC_[A-Z0-9_]+").unwrap();
        let mut screen_keys = screen_re
            .find_iter(&screens)
            .map(|m| m.as_str().to_string())
            .filter(|s| !s.ends_with('_'))
            .collect::<std::collections::HashSet<_>>();
        for part in [
            "HOME", "REGAIN", "CASTLE", "KO", "ALLY", "RESEARCH", "EVENT",
        ] {
            screen_keys.insert(format!("LOC_JAC_AGE_PART_{part}"));
        }
        assert!([
            "LOC_JAC_AGE_ERA_NAME",
            "LOC_JAC_AGE_ERA_ENDS_IN",
            "LOC_JAC_REIGN_TENBUN_NAME"
        ]
        .iter()
        .all(|s| screen_keys.contains(*s)));
        keys.extend(screen_keys);
        let mut text =
            std::collections::HashMap::<String, std::collections::HashSet<String>>::new();
        for path in [
            "Text/JAC_Sengoku_Ages_Text.xml",
            "Text/JAC_Sengoku_Reigns_Text.xml",
            "Text/JAC_Sengoku_Oaths_Text.xml",
        ] {
            let xml = mod_text(path);
            let doc = roxmltree::Document::parse(&xml).unwrap();
            for row in doc.descendants().filter(|n| {
                n.has_tag_name("Row") && n.parent().is_some_and(|p| p.has_tag_name("LocalizedText"))
            }) {
                text.entry(row.attribute("Tag").unwrap().to_string())
                    .or_default()
                    .insert(row.attribute("Language").unwrap().to_string());
            }
        }
        assert_eq!(
            text.keys()
                .cloned()
                .collect::<std::collections::HashSet<_>>(),
            keys
        );
        assert!(text.values().all(|langs| *langs
            == std::collections::HashSet::from(["ja_JP".into(), "en_US".into(), "ru_RU".into()])));
    }
}
