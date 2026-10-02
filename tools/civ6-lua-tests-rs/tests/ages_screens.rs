mod ages_support;
use ages_support::*;
use civ6_lua_tests_rs::json_to_lua;
use serde_json::json;

fn popup_world() -> World {
    let w = World::new();
    w.screen_base();
    let dark = w.context(
        "JAC_SengokuDarkPolicy",
        &[mod_text("UI/JAC_SengokuDarkPolicy.lua")],
    );
    w.bind_context("DARK", dark);
    let oaths = w.context(
        "DedicationPopup",
        &[mod_text("UI/JAC_DedicationPopup_Sengoku.lua")],
    );
    w.bind_context("OATHS", oaths);
    let popup = w.context(
        "JAC_SengokuReignPopup",
        &[mod_text("UI/JAC_SengokuReignPopup.lua")],
    );
    w.bind_context("POPUP", popup);
    w
}
fn make_dark(w: &World, pid: i64) {
    w.turns(1, 65);
    w.run(&format!(
        "LEDGER().houses.p{pid}.score=-1000; Game:SetProperty('JAC_AGES_LEDGER',LEDGER())"
    ));
}

fn notification_world() -> World {
    let w = World::new();
    w.screen_base();
    w.bind_context(
        "DARK",
        w.context(
            "JAC_SengokuDarkPolicy",
            &[mod_text("UI/JAC_SengokuDarkPolicy.lua")],
        ),
    );
    w.bind_context(
        "POPUP",
        w.context(
            "JAC_SengokuReignPopup",
            &[mod_text("UI/JAC_SengokuReignPopup.lua")],
        ),
    );
    let mut files = screen_files();
    files.as_object_mut().unwrap().insert(
        "ActionPanel_Expansion2".into(),
        json!(fixture("action_panel_stubs.lua")),
    );
    w.lua
        .load(fixture("screen_stubs.lua"))
        .call::<()>(json_to_lua(&w.lua, &files))
        .unwrap();
    w.bind_context(
        "PANEL",
        w.context(
            "NotificationPanel",
            &[
                fixture("notification_panel_stubs.lua"),
                mod_text("UI/JAC_NotificationPanel_Sengoku.lua"),
            ],
        ),
    );
    w.run("PANEL.RegisterHandlers()");
    w.bind_context(
        "ACTION",
        w.context("ActionPanel", &[mod_text("UI/JAC_ActionPanel_Sengoku.lua")]),
    );
    w
}
fn era_panel_world() -> World {
    let w = World::new();
    w.screen_base();
    w.bind_context(
        "PANEL",
        w.context(
            "EraProgressPanel",
            &[
                fixture("panel_stubs.lua"),
                mod_text("UI/JAC_EraProgressPanel_Sengoku.lua"),
            ],
        ),
    );
    w
}
fn indicator_world() -> World {
    let w = World::new();
    w.screen_base();
    let mut files = screen_files();
    files.as_object_mut().unwrap().insert(
        "ActionPanel_Expansion2".into(),
        json!(fixture("action_panel_stubs.lua")),
    );
    w.lua
        .load(fixture("screen_stubs.lua"))
        .call::<()>(json_to_lua(&w.lua, &files))
        .unwrap();
    w.bind_context(
        "PANEL",
        w.context("ActionPanel", &[mod_text("UI/JAC_ActionPanel_Sengoku.lua")]),
    );
    w
}

mod reign_popup_tests {
    use super::*;
    #[test]
    fn test_not_shown_before_the_first_era_name_change() {
        let w = popup_world();
        w.turns(1, 65);
        w.run("LOCAL_TURN_BEGIN(); AUTOPLAY_END()");
        assert_eq!(w.j("not POPUP.ContextPtr.hidden"), json!(false));
        assert_eq!(w.j("POPUP.JAC_REIGN_POPUP_LAST"), json!(null));
    }
    #[test]
    fn test_koji_turn_shows_the_house_result_once() {
        let w = popup_world();
        w.turns(1, 30);
        w.run("for i=1,10 do Players[1].research.techs[i]=true end");
        w.turns(31, 66);
        w.run("LOCAL_TURN_BEGIN()");
        assert_eq!(w.j("not POPUP.ContextPtr.hidden"), json!(true));
        assert_eq!(w.j("LEDGER().history.r1.p1.age"), json!("GOLDEN"));
        assert_eq!(
            w.j("POPUP.Controls.Title.text"),
            json!("UP:LOC_JAC_REIGN_KOJI_ANNOUNCEMENT")
        );
        assert_eq!(
            w.j("POPUP.Controls.EraRibbonValue.text"),
            json!("LOC_JAC_AGE_NAME_GOLDEN [ICON_GLORY_GOLDEN_AGE]")
        );
        assert_eq!(
            [
                w.j("POPUP.Controls.EraRibbon.texture"),
                w.j("POPUP.Controls.Background.texture"),
                w.j("POPUP.Controls.WindowFrame.texture")
            ],
            [
                json!("Ages_BannerLongGolden"),
                json!("Ages_ParchmentGolden"),
                json!("Ages_FrameGolden")
            ]
        );
        assert_eq!(w.j("POPUP.Controls.HeroicFlare.hidden"), json!(true));
        let score = w.j("LEDGER().history.r1.p1.score").as_i64().unwrap();
        let golden = w.j("LEDGER().history.r1.p1.golden").as_i64().unwrap();
        let dark = w.j("LEDGER().history.r1.p1.dark").as_i64().unwrap();
        assert_eq!(
            w.j("POPUP.Controls.Score.text"),
            json!(format!(
                "LOC_JAC_AGE_REIGN_SCORE(LOC_JAC_REIGN_TENBUN_NAME,{score})"
            ))
        );
        assert_eq!(
            w.j("POPUP.Controls.Thresholds.text"),
            json!(format!("LOC_JAC_AGE_THRESHOLDS({golden},{dark})"))
        );
        assert_eq!(
            w.j("POPUP.Controls.EraEffects.text"),
            json!("LOC_JAC_AGE_GOLDEN_ANNOUNCEMENT[NEWLINE]LOC_JAC_OATH_PENDING")
        );
        assert_eq!(
            w.j("POPUP.Controls.Continue.text"),
            json!("LOC_JAC_OATH_CHOOSE")
        );
        assert_eq!(
            [
                w.j("POPUP.JAC_REIGN_POPUP_LAST.reign"),
                w.j("POPUP.JAC_REIGN_POPUP_LAST.reignType"),
                w.j("POPUP.JAC_REIGN_POPUP_LAST.age"),
                w.j("POPUP.JAC_REIGN_POPUP_LAST.turn")
            ],
            [json!(1), json!("REIGN_KOJI"), json!("GOLDEN"), json!(66)]
        );
        w.run("POPUP.Controls.Continue.click(); LOCAL_TURN_BEGIN(); AUTOPLAY_END()");
        assert_eq!(w.j("not POPUP.ContextPtr.hidden"), json!(false));
        w.turn(67);
        w.run("LOCAL_TURN_BEGIN()");
        assert_eq!(w.j("not POPUP.ContextPtr.hidden"), json!(false));
        assert_eq!(w.j("POPUP.JAC_ShowReignPopup(true)"), json!(true));
        assert_eq!(w.j("not POPUP.ContextPtr.hidden"), json!(true));
        w.run("POPUP.OnClose()");
        assert_eq!(w.j("not POPUP.ContextPtr.hidden"), json!(false));
    }
    #[test]
    fn test_autoplay_end_shows_it_when_the_turn_begin_did_not() {
        let w = popup_world();
        w.turns(1, 66);
        w.run("AUTOPLAY_END()");
        assert_eq!(w.j("not POPUP.ContextPtr.hidden"), json!(true));
        assert_eq!(w.j("POPUP.JAC_REIGN_POPUP_LAST.reign"), json!(1));
    }
    #[test]
    fn test_dark_house_gets_the_policy_button_and_the_choice_after_closing() {
        let w = popup_world();
        make_dark(&w, 1);
        w.turn(66);
        w.run("LOCAL_TURN_BEGIN()");
        assert_eq!(w.j("not POPUP.ContextPtr.hidden"), json!(true));
        assert_eq!(w.j("not DARK.ContextPtr.hidden"), json!(false));
        assert_eq!(
            w.j("POPUP.Controls.EraRibbonValue.text"),
            json!("LOC_JAC_AGE_NAME_DARK [ICON_GLORY_DARK_AGE]")
        );
        assert_eq!(
            w.j("POPUP.Controls.Background.texture"),
            json!("Ages_ParchmentDark")
        );
        assert_eq!(
            w.j("POPUP.Controls.EraEffects.text"),
            json!("LOC_JAC_AGE_DARK_ELITE_ANNOUNCEMENT[NEWLINE]LOC_JAC_OATH_PENDING")
        );
        assert_eq!(
            w.j("POPUP.Controls.Continue.text"),
            json!("LOC_JAC_OATH_CHOOSE")
        );
        assert_eq!(w.j("POPUP.JAC_REIGN_POPUP_LAST.pendingChoice"), json!(true));
        w.run("POPUP.Controls.Continue.click()");
        assert_eq!(w.j("not POPUP.ContextPtr.hidden"), json!(false));
        assert_eq!(w.j("not DARK.ContextPtr.hidden"), json!(false));
        assert_eq!(w.j("not OATHS.ContextPtr.hidden"), json!(true));
        assert_eq!(
            w.j("OATHS.JAC_ChooseOath('JAC_OATH_TAKEDA_TSUTSUJIGASAKI')"),
            json!(true)
        );
        assert_eq!(w.j("not DARK.ContextPtr.hidden"), json!(true));
    }
    #[test]
    fn test_turn_end_closes_without_opening_the_choice() {
        let w = popup_world();
        make_dark(&w, 1);
        w.turn(66);
        w.run("LOCAL_TURN_BEGIN(); LOCAL_TURN_END()");
        assert_eq!(w.j("not POPUP.ContextPtr.hidden"), json!(false));
        assert_eq!(w.j("not DARK.ContextPtr.hidden"), json!(false));
    }
    #[test]
    fn test_heroic_after_dark_uses_the_heroic_art() {
        let w = popup_world();
        make_dark(&w, 1);
        w.turns(66, 70);
        w.run("for i=1,10 do Players[1].research.techs[i]=true end");
        w.turns(71, 73);
        w.run("LOCAL_TURN_BEGIN()");
        assert_eq!(w.j("LEDGER().history.r2.p1.age"), json!("HEROIC"));
        assert_eq!(
            w.j("POPUP.Controls.Background.texture"),
            json!("Ages_ParchmentHeroic")
        );
        assert_eq!(w.j("POPUP.Controls.HeroicFlare.hidden"), json!(false));
        assert_eq!(
            w.j("POPUP.Controls.Title.text"),
            json!("UP:LOC_JAC_REIGN_EIROKU_ANNOUNCEMENT")
        );
        assert_eq!(
            w.j("string.match(POPUP.Controls.Score.text,'^[^,]+')"),
            json!("LOC_JAC_AGE_REIGN_SCORE(LOC_JAC_REIGN_KOJI_NAME")
        );
    }
    #[test]
    fn test_no_local_player_shows_nothing() {
        let w = popup_world();
        w.turns(1, 66);
        w.run("LOCAL_PLAYER=-1; LOCAL_TURN_BEGIN()");
        assert_eq!(w.j("not POPUP.ContextPtr.hidden"), json!(false));
    }
    #[test]
    fn test_merged_period_names_the_era_it_judged() {
        let w = popup_world();
        w.turns(1, 73);
        w.run("LEDGER().history.r2['from']=0; Game:SetProperty('JAC_AGES_LEDGER',LEDGER())");
        assert_eq!(w.j("POPUP.JAC_ShowReignPopup(true)"), json!(true));
        assert_eq!(
            w.j("string.match(POPUP.Controls.Score.text,'^[^,]+')"),
            json!("LOC_JAC_AGE_REIGN_SCORE(LOC_JAC_REIGN_TENBUN_NAME")
        );
    }
    #[test]
    fn test_the_screen_writes_nothing() {
        let w = popup_world();
        w.turns(1, 66);
        let before = w.snapshot();
        w.run("LOCAL_TURN_BEGIN(); POPUP.Controls.Continue.click()");
        assert_eq!(w.snapshot(), before);
        assert_eq!(w.j("#REQUESTS"), json!(0));
    }
}

mod dark_policy_notification_tests {
    use super::*;
    #[test]
    fn test_screen_once_then_notification_and_end_turn_button_reopen_until_chosen() {
        let w = notification_world();
        make_dark(&w, 1);
        w.turns_with_oath(66, 66);
        w.run("LOCAL_TURN_BEGIN(); POPUP.OnClose()");
        assert_eq!(w.j("not DARK.ContextPtr.hidden"), json!(true));
        assert_eq!(w.j("DARK.JAC_DARK_POLICY_SHOW_COUNT"), json!(1));
        w.run("DARK.OnClose(); LOCAL_TURN_BEGIN(); AUTOPLAY_END()");
        assert_eq!(w.j("not DARK.ContextPtr.hidden"), json!(false));
        w.run("TEST_N=NotificationManager.FindType(3,1); PANEL.OnNotificationActivated(1,TEST_N.id,true)");
        assert_eq!(w.j("not DARK.ContextPtr.hidden"), json!(true));
        assert_eq!(w.j("DARK.JAC_DARK_POLICY_SHOW_COUNT"), json!(2));
        w.run("DARK.OnClose(); ACTION.DoEndTurn()");
        assert_eq!(w.j("ACTION.END_TURN_REQUESTS"), json!(0));
        assert_eq!(w.j("not DARK.ContextPtr.hidden"), json!(true));
        w.run("PANEL.g_notificationHandlers[3].TryDismiss(PANEL.Entry(TEST_N))");
        assert_eq!(w.j("TEST_N.dismissed"), json!(false));
        assert_eq!(
            w.j("DARK.JAC_ChooseDarkPolicy('JAC_AGE_POLICY_TEMPLE')"),
            json!(true)
        );
        assert_eq!(w.j("TEST_N.dismissed"), json!(true));
        w.run("ACTION.DoEndTurn()");
        assert_eq!(w.j("ACTION.END_TURN_REQUESTS"), json!(1));
        w.turns_with_oath(67, 67);
        w.run("PANEL.OnNotificationActivated(1,TEST_N.id,true)");
        assert_eq!(w.j("not DARK.ContextPtr.hidden"), json!(false));
        assert_eq!(
            w.j("DARK.JAC_ChooseDarkPolicy('JAC_AGE_POLICY_PIRATE')"),
            json!(false)
        );
    }
    #[test]
    fn test_notification_can_replace_an_open_reign_popup() {
        let w = notification_world();
        make_dark(&w, 1);
        w.turns_with_oath(66, 66);
        w.run("LOCAL_TURN_BEGIN()");
        assert_eq!(w.j("not POPUP.ContextPtr.hidden"), json!(true));
        w.run("TEST_N=NotificationManager.FindType(3,1); PANEL.OnNotificationActivated(1,TEST_N.id,true)");
        assert_eq!(w.j("not POPUP.ContextPtr.hidden"), json!(false));
        assert_eq!(w.j("not DARK.ContextPtr.hidden"), json!(true));
    }
    #[test]
    fn test_official_notifications_keep_their_own_handler() {
        let w = notification_world();
        w.run("NotificationManager.SendNotification(1,71,{MESSAGE='official'}); TEST_N=NotificationManager.FindType(71,1); PANEL.OnNotificationActivated(1,TEST_N.id,true)");
        assert_eq!(w.j("PANEL.OFFICIAL_ACTIVATIONS"), json!(1));
        assert_eq!(w.j("not DARK.ContextPtr.hidden"), json!(false));
    }
    #[test]
    fn test_notifications_for_another_house_or_an_earlier_reign_cannot_open_the_screen() {
        let w = notification_world();
        make_dark(&w, 1);
        w.turns_with_oath(66, 66);
        w.run("NotificationManager.SendNotification(1,3,{JAC_DARK_POLICY_REIGN=0}); PANEL.OnNotificationActivated(1,#NOTIFICATIONS,true)");
        w.run("TEST_OTHER=NotificationManager.FindType(3,3); PANEL.OnNotificationActivated(3,TEST_OTHER.id,true)");
        assert_eq!(w.j("not DARK.ContextPtr.hidden"), json!(false));
    }
    #[test]
    fn test_ui_recreated_from_save_does_not_repeat_automatic_choice_but_notification_works() {
        let w = notification_world();
        make_dark(&w, 1);
        w.turns_with_oath(66, 66);
        w.run("LuaEvents=setmetatable({}, {__index=function(t,k) local hs={}; local e=setmetatable({Add=function(f) hs[#hs+1]=f end},{__call=function(_,...) for _,f in ipairs(hs) do f(...) end end}); rawset(t,k,e);return e end})");
        w.bind_context(
            "DARK",
            w.context(
                "JAC_SengokuDarkPolicy",
                &[mod_text("UI/JAC_SengokuDarkPolicy.lua")],
            ),
        );
        w.bind_context(
            "POPUP",
            w.context(
                "JAC_SengokuReignPopup",
                &[mod_text("UI/JAC_SengokuReignPopup.lua")],
            ),
        );
        w.bind_context(
            "PANEL",
            w.context(
                "NotificationPanel",
                &[
                    fixture("notification_panel_stubs.lua"),
                    mod_text("UI/JAC_NotificationPanel_Sengoku.lua"),
                ],
            ),
        );
        w.bind_context(
            "ACTION",
            w.context("ActionPanel", &[mod_text("UI/JAC_ActionPanel_Sengoku.lua")]),
        );
        w.run("PANEL.RegisterHandlers(); LOAD_SCREEN_CLOSE()");
        assert_eq!(w.j("POPUP.JAC_ShowReignPopup(false)"), json!(false));
        w.run("LuaEvents.JAC_SengokuReignPopup_Closed(true)");
        assert_eq!(w.j("not DARK.ContextPtr.hidden"), json!(false));
        assert_eq!(w.j("DARK.JAC_DARK_POLICY_SHOW_COUNT"), json!(0));
        w.run("TEST_N=NotificationManager.FindType(3,1); PANEL.OnNotificationActivated(1,TEST_N.id,true)");
        assert_eq!(w.j("not DARK.ContextPtr.hidden"), json!(true));
        assert_eq!(w.j("TEST_N.dismissed"), json!(false));
        w.run("ACTION.OnInputActionTriggered('END_TURN')");
        assert_eq!(w.j("ACTION.END_TURN_REQUESTS"), json!(0));
    }
    #[test]
    fn test_button_hotkey_auto_end_and_captured_refresh_use_the_saved_ledger() {
        let w = notification_world();
        make_dark(&w, 1);
        w.turns_with_oath(66, 66);
        w.run("LOCAL_TURN_BEGIN(); POPUP.OnClose(); DARK.OnClose(); ACTION.ContextPtr:RequestRefresh()");
        assert_eq!(
            w.j("ACTION.Controls.EndTurnText.text"),
            json!("LOC_JAC_AGE_DARK_CHOICE_TITLE")
        );
        assert_eq!(w.j("ACTION.JAC_IsDarkPolicyEndTurnBlocked()"), json!(true));
        w.run("ACTION.OnInputActionTriggered('END_TURN')");
        assert_eq!(w.j("not DARK.ContextPtr.hidden"), json!(true));
        w.run("ACTION.OnEndTurnClicked()");
        for shift in [false, true] {
            assert_eq!(
                w.j(&format!(
                    "ACTION.ContextPtr.input(ACTION.KEY_INPUT(13,{shift}))"
                )),
                json!(true)
            );
        }
        assert_eq!(
            w.j("ACTION.ContextPtr.input(ACTION.KEY_INPUT(27,false))"),
            json!(false)
        );
        assert_eq!(w.j("ACTION.BASE_INPUTS"), json!(1));
        assert_eq!(w.j("ACTION.FORCED_END_TURN_REQUESTS"), json!(0));
        w.run("ACTION.CheckAutoEndTurn()");
        assert_eq!(
            [
                w.j("ACTION.END_TURN_REQUESTS"),
                w.j("ACTION.AUTO_END_TURN_REQUESTS")
            ],
            [json!(0), json!(0)]
        );
        w.run("TEST_N=NotificationManager.FindType(3,1); NotificationManager.Dismiss(1,TEST_N.id); ACTION.DoEndTurn(123)");
        assert_eq!(w.j("ACTION.END_TURN_REQUESTS"), json!(0));
        assert_eq!(
            w.j("DARK.JAC_ChooseDarkPolicy('JAC_AGE_POLICY_TEMPLE')"),
            json!(true)
        );
        assert_eq!(w.j("ACTION.JAC_IsDarkPolicyEndTurnBlocked()"), json!(false));
        w.run("ACTION.DoEndTurn(123); ACTION.CheckAutoEndTurn()");
        assert_eq!(
            [
                w.j("ACTION.END_TURN_REQUESTS"),
                w.j("ACTION.AUTO_END_TURN_REQUESTS")
            ],
            [json!(1), json!(1)]
        );
        assert_eq!(w.j("ACTION.LAST_BLOCKER"), json!(123));
        assert_eq!(w.j("ACTION.Controls.EndTurnText.text"), json!("NEXT TURN"));
        assert_eq!(
            w.j("ACTION.ContextPtr.input(ACTION.KEY_INPUT(13,true))"),
            json!(true)
        );
        assert_eq!(w.j("ACTION.FORCED_END_TURN_REQUESTS"), json!(1));
        w.run("ACTION.EXPANSION_ALLOWS_AUTO_END=false; ACTION.CheckAutoEndTurn()");
        assert_eq!(w.j("ACTION.AUTO_END_TURN_REQUESTS"), json!(1));
    }
    #[test]
    fn test_guard_applies_only_to_a_pending_human_on_the_judgement_turn() {
        let w = notification_world();
        w.turns_with_oath(1, 65);
        assert_eq!(w.j("ACTION.JAC_IsDarkPolicyEndTurnBlocked()"), json!(false));
        w.run("LEDGER().houses.p1.score=-1000");
        w.turns_with_oath(66, 66);
        assert_eq!(w.j("ACTION.JAC_IsDarkPolicyEndTurnBlocked()"), json!(true));
        w.run("LOCAL_PLAYER=0");
        assert_eq!(w.j("ACTION.JAC_IsDarkPolicyEndTurnBlocked()"), json!(false));
        w.run("LOCAL_PLAYER=-1");
        assert_eq!(w.j("ACTION.JAC_IsDarkPolicyEndTurnBlocked()"), json!(false));
        w.run("LOCAL_PLAYER=1");
        w.turns_with_oath(67, 67);
        assert_eq!(w.j("ACTION.JAC_IsDarkPolicyEndTurnBlocked()"), json!(false));
    }
}

mod era_panel_tests {
    use super::*;
    #[test]
    fn test_before_the_ledger_the_official_panel_stays() {
        let w = era_panel_world();
        w.run("PANEL.Refresh()");
        assert_eq!(w.j("PANEL.BASE_CALLS"), json!(1));
        assert_eq!(
            w.j("PANEL.Controls.Title.text"),
            json!("LOC_ERA_PROGRESS_TITLE")
        );
        assert_eq!(w.j("PANEL.JAC_AGES_PANEL_VIEW"), json!(null));
    }
    #[test]
    fn test_tenbun_period_shows_the_ledger() {
        let w = era_panel_world();
        w.turns(1, 5);
        w.run("CAPTURE(1,2,1); Players[1].research.techs[3]=true");
        w.turn(6);
        w.run("PANEL.Refresh()");
        assert_eq!(w.j("PANEL.BASE_CALLS"), json!(1));
        assert_eq!(
            w.j("PANEL.Controls.Title.text"),
            json!("UP:LOC_JAC_AGE_PANEL_TITLE")
        );
        assert_eq!(
            w.j("PANEL.Controls.EraName.text"),
            json!("LOC_JAC_AGE_PANEL_PERIOD(LOC_JAC_REIGN_TENBUN_NAME,1534)")
        );
        assert_eq!(w.j("PANEL.Controls.TimeToNextEra.text"), json!("60"));
        assert_eq!(
            w.j("PANEL.Controls.TimeToNextEraContainer.hidden"),
            json!(false)
        );
        assert_eq!(w.j("PANEL.Controls.EraScoreValue.text"), json!("3"));
        assert_eq!(
            w.j("PANEL.Controls.EraRibbonValue.text"),
            json!("LOC_JAC_AGE_NAME_NONE")
        );
        assert_eq!(
            w.j("PANEL.Controls.EraRibbon.texture"),
            json!("Controls_AgesBanner_Normal")
        );
        let golden = w.house(1, "next.golden").as_i64().unwrap();
        let dark = w.house(1, "next.dark").as_i64().unwrap();
        assert_eq!(
            w.j("PANEL.Controls.GoldenThreshold.text"),
            json!(format!("{golden}+"))
        );
        assert_eq!(
            w.j("PANEL.Controls.NormalThreshold.text"),
            json!(format!("{dark} - {}", golden - 1))
        );
        assert_eq!(
            w.j("PANEL.Controls.DarkThreshold.text"),
            json!(if dark > 0 {
                format!("0 - {}", dark - 1)
            } else {
                "-".into()
            })
        );
        assert_eq!(
            w.j("PANEL.Controls.GoldenLabel.text"),
            json!("LOC_ERA_REVIEW_HAVE_GOLDEN_AGE_LABEL")
        );
        let projected = if 3 >= golden {
            "GOLDEN"
        } else if 3 >= dark {
            "NORMAL"
        } else {
            "DARK"
        };
        assert_eq!(w.j("PANEL.JAC_AGES_PANEL_VIEW.projected"), json!(projected));
        assert_eq!(
            [
                w.j("not PANEL.Controls.GoldenCheckmark.hidden"),
                w.j("not PANEL.Controls.NormalCheckmark.hidden"),
                w.j("not PANEL.Controls.DarkCheckmark.hidden")
            ],
            [
                json!(projected == "GOLDEN"),
                json!(projected == "NORMAL"),
                json!(projected == "DARK")
            ]
        );
        let effect = w.j("PANEL.Controls.EraEffects.text");
        let lines = effect
            .as_str()
            .unwrap()
            .split("[NEWLINE]")
            .collect::<Vec<_>>();
        assert_eq!(
            lines,
            vec![
                "LOC_JAC_AGE_NONE_EFFECT",
                "",
                "LOC_JAC_AGE_PANEL_PARTS(LOC_JAC_REIGN_TENBUN_NAME)",
                "[ICON_BULLET]LOC_JAC_AGE_PART_HOME(0)",
                "[ICON_BULLET]LOC_JAC_AGE_PART_REGAIN(0)",
                "[ICON_BULLET]LOC_JAC_AGE_PART_CASTLE(2)",
                "[ICON_BULLET]LOC_JAC_AGE_PART_KO(0)",
                "[ICON_BULLET]LOC_JAC_AGE_PART_ALLY(0)",
                "[ICON_BULLET]LOC_JAC_AGE_PART_RESEARCH(1)",
                "[ICON_BULLET]LOC_JAC_AGE_PART_EVENT(0)",
                "[ICON_BULLET]LOC_JAC_OATH_SCORE(0)",
                "",
                "LOC_JAC_AGE_PANEL_HISTORY",
                "[ICON_BULLET]LOC_JAC_AGE_PANEL_HISTORY_NONE"
            ]
        );
        assert_eq!(
            w.j("PANEL.Controls.EraEffectsHeader.text"),
            json!("LOC_JAC_AGE_PANEL_LEDGER_HEADER")
        );
    }
    #[test]
    fn test_after_koji_history_other_houses_ages_and_the_heroic_label() {
        let w = era_panel_world();
        make_dark(&w, 1);
        w.run("for i=1,10 do Players[0].research.techs[i]=true end");
        w.turns(66, 70);
        w.run("PANEL.Refresh()");
        assert_eq!(w.j("rawget(PANEL,'Game')==Game"), json!(true));
        assert_eq!(
            w.j("PANEL.Controls.EraName.text"),
            json!("LOC_JAC_AGE_PANEL_PERIOD(LOC_JAC_REIGN_KOJI_NAME,1555)")
        );
        assert_eq!(w.j("PANEL.Controls.TimeToNextEra.text"), json!("3"));
        assert_eq!(
            w.j("PANEL.Controls.EraRibbonValue.text"),
            json!("LOC_JAC_AGE_NAME_DARK [ICON_GLORY_DARK_AGE]")
        );
        assert_eq!(
            w.j("PANEL.Controls.EraRibbon.texture"),
            json!("Controls_AgesBanner_Dark")
        );
        assert_eq!(
            w.j("PANEL.Controls.GoldenLabel.text"),
            json!("LOC_ERA_REVIEW_HAVE_HEROIC_AGE_LABEL")
        );
        let ages = (0..4)
            .map(|p| w.j(&format!("LEDGER().history.r1.p{p}.age")))
            .collect::<Vec<_>>();
        assert_eq!(
            [ages[0].clone(), ages[1].clone()],
            [json!("GOLDEN"), json!("DARK")]
        );
        let icons = ages
            .iter()
            .enumerate()
            .filter(|(p, _)| *p != 1)
            .map(|(p, a)| {
                (
                    p.to_string(),
                    json!(match a.as_str().unwrap() {
                        "GOLDEN" => "[ICON_GLORY_GOLDEN_AGE]",
                        "NORMAL" => "[ICON_GLORY_NORMAL_AGE]",
                        "DARK" => "[ICON_GLORY_DARK_AGE]",
                        other => panic!("{other}"),
                    }),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        assert_eq!(w.j("PANEL.ICONS"), serde_json::Value::Object(icons));
        let effect = w.j("PANEL.Controls.EraEffects.text");
        let lines = effect
            .as_str()
            .unwrap()
            .split("[NEWLINE]")
            .collect::<Vec<_>>();
        assert_eq!(lines[0], "LOC_JAC_AGE_DARK_ELITE_ANNOUNCEMENT");
        assert!(lines.contains(&"LOC_JAC_AGE_PANEL_PARTS(LOC_JAC_REIGN_KOJI_NAME)"));
        assert!(lines[1].contains("LOC_JAC_OATH_ACTIVE"));
        let score = w.j("LEDGER().history.r1.p1.score").as_i64().unwrap();
        let golden = w.j("LEDGER().history.r1.p1.golden").as_i64().unwrap();
        let dark = w.j("LEDGER().history.r1.p1.dark").as_i64().unwrap();
        assert_eq!(lines.last().unwrap(),&format!("[ICON_BULLET]LOC_JAC_AGE_PANEL_HISTORY_ROW(LOC_JAC_REIGN_KOJI_NAME,1555,LOC_JAC_AGE_NAME_DARK [ICON_GLORY_DARK_AGE],{score}) LOC_JAC_AGE_THRESHOLDS({golden},{dark})"));
    }
    #[test]
    fn test_before_chidai_the_projection_is_dark_whatever_the_score() {
        let w = era_panel_world();
        w.turns(1, 5);
        w.turn(1457);
        w.run("LEDGER().houses.p1.score=1000; Game:SetProperty('JAC_AGES_LEDGER',LEDGER()); PANEL.Refresh()");
        assert_eq!(w.j("PANEL.JAC_AGES_PANEL_VIEW.projected"), json!("DARK"));
        assert_eq!(w.j("PANEL.Controls.DarkCheckmark.hidden"), json!(false));
        assert_eq!(w.j("PANEL.Controls.GoldenCheckmark.hidden"), json!(true));
        assert_eq!(
            w.j("PANEL.Controls.EraName.text"),
            json!("LOC_JAC_AGE_PANEL_PERIOD(LOC_JAC_REIGN_REIWA_NAME,2019)")
        );
        assert_eq!(w.j("PANEL.Controls.TimeToNextEra.text"), json!("300"));
    }
    #[test]
    fn test_after_the_last_era_name_no_countdown_or_thresholds() {
        let w = era_panel_world();
        w.turns(1, 5);
        w.turn(1757);
        w.run("PANEL.Refresh()");
        let count = reigns()["reigns"].as_array().unwrap().len();
        assert_eq!(w.j("LEDGER().reign"), json!(count));
        assert_eq!(
            w.j("PANEL.Controls.TimeToNextEraContainer.hidden"),
            json!(true)
        );
        assert_eq!(w.j("PANEL.Controls.ThresholdStack.hidden"), json!(true));
        assert_eq!(
            w.j("PANEL.Controls.EraName.text"),
            json!("LOC_JAC_AGE_PANEL_PERIOD(LOC_JAC_REIGN_CHIDAI_NAME,2119)")
        );
        let effect = w.j("PANEL.Controls.EraEffects.text");
        assert_eq!(
            effect
                .as_str()
                .unwrap()
                .split("[NEWLINE]")
                .filter(|s| s.contains("HISTORY_ROW"))
                .count(),
            count
        );
    }
    #[test]
    fn test_the_panel_writes_nothing() {
        let w = era_panel_world();
        w.turns(1, 66);
        let before = w.snapshot();
        w.run("PANEL.Refresh()");
        assert_eq!(w.snapshot(), before);
    }
}

mod era_indicator_tests {
    use super::*;
    #[test]
    fn test_before_the_ledger_the_official_tooltip_stays() {
        let w = indicator_world();
        w.run("PANEL.RealizeEraIndicator()");
        assert_eq!(w.j("PANEL.BASE_ERA_CALLS"), json!(1));
        assert_eq!(
            [
                w.j("PANEL.Controls.EraToolTipArea1.tooltip"),
                w.j("PANEL.Controls.EraToolTipArea2.tooltip")
            ],
            [json!("LOC_GAME_ERA_DESC"), json!("LOC_GAME_ERA_DESC")]
        );
        assert_eq!(w.j("PANEL.JAC_ERA_INDICATOR_TEXT"), json!(null));
    }
    #[test]
    fn test_tenbun_then_koji_with_the_turns_to_the_next_era_name() {
        let w = indicator_world();
        w.turns(1, 6);
        w.run("PANEL.RealizeEraIndicator()");
        let want=json!("LOC_JAC_AGE_ERA_NAME(LOC_JAC_REIGN_TENBUN_NAME)[NEWLINE][NEWLINE]LOC_JAC_AGE_ERA_ENDS_IN(60)");
        assert_eq!(
            [
                w.j("PANEL.Controls.EraToolTipArea1.tooltip"),
                w.j("PANEL.Controls.EraToolTipArea2.tooltip")
            ],
            [want.clone(), want]
        );
        w.turns(7, 66);
        w.run("LOCAL_TURN_BEGIN()");
        assert_eq!(w.j("PANEL.JAC_ERA_INDICATOR_TEXT"),json!("LOC_JAC_AGE_ERA_NAME(LOC_JAC_REIGN_KOJI_NAME)[NEWLINE][NEWLINE]LOC_JAC_AGE_ERA_ENDS_IN(7)"));
        assert_eq!(
            w.j("PANEL.Controls.EraToolTipArea1.tooltip"),
            w.j("PANEL.JAC_ERA_INDICATOR_TEXT")
        );
    }
    #[test]
    fn test_chidai_has_no_countdown() {
        let w = indicator_world();
        w.turns(1, 5);
        w.turn(1757);
        w.run("PANEL.RealizeEraIndicator()");
        assert_eq!(
            w.j("PANEL.JAC_ERA_INDICATOR_TEXT"),
            json!("LOC_JAC_AGE_ERA_NAME(LOC_JAC_REIGN_CHIDAI_NAME)")
        );
    }
    #[test]
    fn test_the_indicator_writes_nothing() {
        let w = indicator_world();
        w.turns(1, 66);
        let before = w.snapshot();
        w.run("PANEL.RealizeEraIndicator()");
        assert_eq!(w.snapshot(), before);
    }
}

mod era_complete_popup_tests {
    use super::*;
    #[test]
    fn test_the_official_screen_stays_off_with_the_pack_and_on_without() {
        let w = World::new();
        w.screen_base();
        for (with_sengoku, shown) in [(true, 0), (false, 1)] {
            let mut files = screen_files();
            files.as_object_mut().unwrap().insert(
                "EraCompletePopup".into(),
                json!(fixture("era_complete_stubs.lua")),
            );
            if with_sengoku {
                files.as_object_mut().unwrap().insert(
                    "JAC_EraCompletePopup_Sengoku".into(),
                    json!(mod_text("UI/JAC_EraCompletePopup_Sengoku.lua")),
                );
            }
            w.lua
                .load(fixture("screen_stubs.lua"))
                .call::<()>(json_to_lua(&w.lua, &files))
                .unwrap();
            w.bind_context(
                "COMPLETE",
                w.context(
                    "EraCompletePopup",
                    &[civ6_lua_tests_rs::read_repo(
                        "poc/JAC_PoC_BanseikiRuleset/UI/JAC_EraCompletePopup.lua",
                    )],
                ),
            );
            w.run("COMPLETE.OnCheckGameEraChanged()");
            assert_eq!(
                w.j("COMPLETE.SHOWN"),
                json!(shown),
                "with_sengoku={with_sengoku}"
            );
        }
    }
}

mod wiring_tests {
    use super::*;
    #[test]
    fn test_notification_extension_does_not_recursively_include_the_official_panel() {
        let meta = mod_text("JAC_PoC_SengokuTrees.modinfo");
        let doc = roxmltree::Document::parse(&meta).unwrap();
        assert!(!doc.descendants().any(|n| n.has_tag_name("ReplaceUIScript")
            && n.attribute("id") == Some("JAC_Sengoku_NotificationPanel")));
        assert!(!mod_text("UI/JAC_NotificationPanel_Sengoku.lua")
            .contains("include(\"NotificationPanel\")"));
        let files = doc
            .descendants()
            .filter(|n| n.has_tag_name("ImportFiles"))
            .flat_map(|n| n.children().filter(|f| f.has_tag_name("File")))
            .filter_map(|n| n.text())
            .collect::<Vec<_>>();
        assert!(files.contains(&"UI/JAC_NotificationPanel_Sengoku.lua"));
    }
    #[test]
    fn test_ruleset_panel_loads_the_extension_after_the_official_script() {
        let source =
            civ6_lua_tests_rs::read_repo("poc/JAC_PoC_BanseikiRuleset/UI/JAC_EraProgressPanel.lua");
        assert_eq!(
            source.trim_end().lines().rev().take(2).collect::<Vec<_>>(),
            vec![
                "include(\"JAC_EraProgressPanel_\", true)",
                "include(\"EraProgressPanel\")"
            ]
        );
        let modinfo = mod_text("JAC_PoC_SengokuTrees.modinfo");
        let imports = modinfo
            .split_once("<ImportFiles id=\"JAC_Sengoku_HouseNames\"")
            .unwrap()
            .1
            .split_once("</ImportFiles>")
            .unwrap()
            .0;
        for name in [
            "JAC_EraProgressPanel_Sengoku.lua",
            "JAC_EraCompletePopup_Sengoku.lua",
            "JAC_SengokuAgesView.lua",
            "JAC_SengokuReignPopup.lua",
            "JAC_SengokuReignPopup.xml",
            "JAC_ActionPanel_Sengoku.lua",
        ] {
            let entry = format!("<File>UI/{name}</File>");
            assert!(imports.contains(&entry), "{name}");
            assert_eq!(
                modinfo.matches(&entry).count(),
                if name.ends_with(".xml") { 3 } else { 2 },
                "{name}"
            );
        }
        assert!(
            modinfo.contains("<File>UI/JAC_SengokuReignPopup.xml</File>\n\t\t</AddUserInterfaces>")
        );
        let replace = modinfo
            .split_once("<ReplaceUIScript id=\"JAC_Sengoku_ActionPanel\"")
            .unwrap()
            .1
            .split_once("</ReplaceUIScript>")
            .unwrap()
            .0;
        for part in [
            "<LoadOrder>100</LoadOrder>",
            "<LuaContext>ActionPanel</LuaContext>",
            "<LuaReplace>UI/JAC_ActionPanel_Sengoku.lua</LuaReplace>",
        ] {
            assert!(replace.contains(part), "{part}");
        }
        let action = mod_text("UI/JAC_ActionPanel_Sengoku.lua");
        assert!(
            action.find("include(\"ActionPanel_Expansion2\")").unwrap()
                < action.find("BASE_RealizeEraIndicator =").unwrap()
        );
    }
}

mod title_length_tests {
    use super::*;
    fn width(text: &str) -> usize {
        use icu_properties::{props::EastAsianWidth, CodePointMapData};
        let widths = CodePointMapData::<EastAsianWidth>::new();
        text.chars()
            .map(|ch| match widths.get(ch) {
                EastAsianWidth::Wide | EastAsianWidth::Fullwidth => 2,
                _ => 1,
            })
            .sum()
    }
    fn texts(prefix: &str, suffix: &str) -> Vec<(String, String, String)> {
        let mut rows = Vec::new();
        for path in [
            "Text/JAC_Sengoku_Ages_Text.xml",
            "Text/JAC_Sengoku_Reigns_Text.xml",
        ] {
            let xml = mod_text(path);
            let doc = roxmltree::Document::parse(&xml).unwrap();
            for row in doc.descendants().filter(|n| n.has_tag_name("Row")) {
                let tag = row.attribute("Tag").unwrap_or("");
                if tag.starts_with(prefix) && tag.ends_with(suffix) {
                    let lang = row.attribute("Language").unwrap().to_string();
                    let text = row
                        .children()
                        .find(|n| n.has_tag_name("Text"))
                        .unwrap()
                        .text()
                        .unwrap_or("")
                        .to_string();
                    rows.push((tag.to_string(), lang, text));
                }
            }
        }
        rows
    }
    #[test]
    fn test_width_uses_unicode_wide_and_fullwidth_properties() {
        assert_eq!(width("A\u{0301}¡ｶ"), 4);
        assert_eq!(width("漢Ｘ🌀"), 6);
        assert_eq!(width("\u{3000}"), 2);
    }
    #[test]
    fn test_era_name_popup_title_is_two_short_lines() {
        let titles = texts("LOC_JAC_REIGN_", "_ANNOUNCEMENT");
        assert_eq!(
            titles.len(),
            reigns()["reigns"].as_array().unwrap().len() * 3
        );
        for (tag, lang, text) in titles {
            let lines = text.split("[NEWLINE]").collect::<Vec<_>>();
            assert_eq!(lines.len(), 2, "{tag} {lang}");
            assert!(
                lines.iter().map(|s| width(s)).max().unwrap() <= 32,
                "{tag} {lang}: {text}"
            );
        }
    }
    #[test]
    fn test_era_score_panel_title_fits_the_header() {
        let titles = texts("LOC_JAC_AGE_PANEL_TITLE", "");
        assert_eq!(titles.len(), 3);
        for (tag, lang, text) in titles {
            assert!(width(&text) <= 15, "{tag} {lang}: {text}");
        }
    }
}
