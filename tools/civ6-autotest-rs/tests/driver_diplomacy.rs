mod support_driver;

use civ6_autotest_rs::{runner::AutoplayOptions, Error, ErrorKind, Result};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};
use support_driver::{FakeBackend, Harness};

#[derive(Default)]
struct Game {
    turn: i64,
    active: bool,
    done: bool,
    left: i64,
    pending: BTreeSet<i64>,
    stop_early: bool,
    deal: bool,
    starve: bool,
    reopen: bool,
    visible: BTreeMap<String, bool>,
    closed: Vec<String>,
    cleanup_fail_once: bool,
    cleanup_calls: usize,
    alive_ack_lost_once: bool,
    next_screen: Option<String>,
}
impl Game {
    fn new(turns: &[i64]) -> Self {
        let mut game = Self {
            turn: 1,
            pending: turns.iter().copied().collect(),
            ..Default::default()
        };
        for state in ["DiplomacyActionView", "DiplomacyDealView"] {
            game.visible.insert(state.into(), false);
        }
        game
    }
    fn states(&self) -> Vec<String> {
        let mut values = vec!["InGame".into()];
        values.extend(self.visible.keys().cloned());
        values
    }
    fn lua(&mut self, state: &str, code: &str) -> Result<String> {
        if state != "InGame" {
            assert!(code.starts_with("if ContextPtr:IsHidden() then return false end"));
            if !self.visible.get(state).copied().unwrap_or(false) {
                return Ok("false".into());
            }
            if state == "DiplomacyDealView" || !self.visible["DiplomacyDealView"] {
                self.visible.insert(state.into(), false);
                self.closed.push(state.into());
                if state == "JAC_SengokuReignPopup" {
                    if let Some(next) = &self.next_screen {
                        self.visible.insert(next.clone(), true);
                    }
                }
            }
            return Ok("true".into());
        }
        if code.starts_with("if JAC_AUTOTEST_AUTO_HANDLER then") {
            self.cleanup_calls += 1;
            if self.cleanup_fail_once && self.cleanup_calls == 1 {
                self.visible.insert("DiplomacyActionView".into(), true);
                return Err(Error::unknown(
                    "cleanup acknowledgment lost after final turn",
                ));
            }
        }
        if let Some(start) = code.find("SetTurns(") {
            let rest = &code[start + "SetTurns(".len()..];
            self.left = rest.split(')').next().unwrap().parse().unwrap();
            self.active = true;
            self.done = false;
            return Ok(if code.contains("return tostring(startTurn)") {
                format!("{}|0", self.turn)
            } else {
                "true".into()
            });
        }
        if code.contains("tostring(AutoplayManager.IsActive())") {
            if self.starve && self.visible.values().any(|v| *v) {
                return Err(Error::unknown("FireTuner response timed out"));
            }
            self.advance();
            return Ok(format!(
                "{}|{}|{}|{}|0",
                self.turn, self.active, self.done, self.left
            ));
        }
        if code.contains("COMMEMORATE") {
            return Ok("none".into());
        }
        if code.contains("IsAlive()") {
            if self.alive_ack_lost_once {
                self.alive_ack_lost_once = false;
                self.advance();
                return Err(Error::unknown(
                    "alive read ran but FireTuner acknowledgement was lost",
                ));
            }
            return Ok(format!("{}|true", self.turn));
        }
        Ok("true".into())
    }
    fn advance(&mut self) {
        if !self.active || self.visible["DiplomacyActionView"] || self.visible["DiplomacyDealView"]
        {
            return;
        }
        if self.pending.remove(&self.turn) || self.reopen {
            self.visible.insert("DiplomacyActionView".into(), true);
            self.visible.insert("DiplomacyDealView".into(), self.deal);
            if self.stop_early {
                self.active = false;
            }
            return;
        }
        self.turn += 1;
        self.left -= 1;
        if self.left == 0 {
            self.active = false;
            self.done = true;
        }
    }
}
fn harness(game: Game) -> (Harness, Arc<Mutex<Game>>) {
    let game = Arc::new(Mutex::new(game));
    let for_states = game.clone();
    let for_lua = game.clone();
    let fake = FakeBackend::new()
        .states(move || Ok(for_states.lock().unwrap().states()))
        .lua(move |state, code| for_lua.lock().unwrap().lua(state, code));
    (Harness::new(json!({}), fake), game)
}
fn autoplay(game: Game, turns: i64) -> (Harness, Arc<Mutex<Game>>, Result<String>) {
    let (mut h, game) = harness(game);
    let result = h.runner.autoplay(turns, 600.0, AutoplayOptions::default());
    (h, game, result)
}
fn count(h: &Harness, state: &str, fragment: &str) -> usize {
    h.calls()
        .iter()
        .filter(|(s, c)| s == state && c.contains(fragment))
        .count()
}
fn assert_closed(h: &Harness, game: &Game, expected: &[&str]) {
    assert_eq!(game.closed, expected);
    assert!(h.calls().iter().all(|(s, c)| s == "InGame"
        || s == "GameCore_Tuner"
        || c.starts_with("if ContextPtr:IsHidden() then return false end")));
}

#[test]
fn test_statement_after_final_turn_retries_only_idempotent_handler_cleanup() {
    let mut game = Game::new(&[]);
    game.cleanup_fail_once = true;
    let (h, game, detail) = autoplay(game, 2);
    let game = game.lock().unwrap();
    assert_eq!(game.turn, 3);
    assert_eq!(game.cleanup_calls, 2);
    assert_eq!(count(&h, "InGame", "SetActive(true)"), 1);
    assert_closed(&h, &game, &["DiplomacyActionView"]);
    assert!(detail.unwrap().contains("autoplay completed 2 turns (1->3"));
    assert!(h
        .log()
        .contains("closed AI diplomacy statement after an InGame timeout"));
}

#[test]
fn autoplay_retries_only_the_alive_read_after_lost_acknowledgement() {
    let mut game = Game::new(&[]);
    game.alive_ack_lost_once = true;
    let (h, game, result) = autoplay(game, 2);
    assert!(result.unwrap().contains("autoplay completed 2 turns (1->3"));
    assert_eq!(game.lock().unwrap().turn, 3);
    assert_eq!(count(&h, "InGame", "AutoplayManager.SetActive(true)"), 1);
    assert_eq!(count(&h, "InGame", "Players[0]:IsAlive()"), 3);
    assert!(h.events().iter().any(|event| event == "disconnect"));
}

#[test]
fn test_test_mode_counts_from_core_when_ui_turn_is_stale_after_jump() {
    let mut game = Game::new(&[]);
    game.turn = 63;
    let (mut h, game) = harness(game);
    h.runner.scenario = json!({"test_mode":{"skipped_turns":62}});
    let old = h.runner.backend;
    // The fixture's Lua adapter models a stale UI start turn while GameCore remains at 63.
    // Recreate the backend to keep the test independent of the concrete adapter type.
    let g1 = game.clone();
    let g2 = game.clone();
    h.runner.backend = Box::new(
        FakeBackend::new()
            .states(move || Ok(g1.lock().unwrap().states()))
            .lua(move |s, c| {
                if s == "GameCore_Tuner" {
                    assert_eq!(c, "return Game.GetCurrentGameTurn()");
                    return Ok("63".into());
                }
                let value = g2.lock().unwrap().lua(s, c)?;
                Ok(if c.contains("return tostring(startTurn)") {
                    "1|0".into()
                } else {
                    value
                })
            }),
    );
    drop(old);
    let detail = h.runner.autoplay(2, 30.0, Default::default()).unwrap();
    assert!(detail.contains("autoplay completed 2 turns (63->65"));
}

#[test]
fn test_statement_mid_autoplay_is_closed_and_autoplay_completes() {
    let (h, game, detail) = autoplay(Game::new(&[3]), 5);
    assert!(detail.unwrap().contains("autoplay completed 5 turns (1->6"));
    assert!(h.log().contains("closed AI diplomacy statement"));
    assert_closed(&h, &game.lock().unwrap(), &["DiplomacyActionView"]);
    assert_eq!(count(&h, "InGame", "SetActive(true)"), 1);
}

#[test]
fn test_repeated_early_stops_by_statements_resume_without_using_restart_budget() {
    let mut game = Game::new(&[2, 3, 4]);
    game.stop_early = true;
    let (h, game, detail) = autoplay(game, 5);
    assert!(detail.unwrap().contains("(1->6"));
    assert_closed(&h, &game.lock().unwrap(), &["DiplomacyActionView"; 3]);
    assert_eq!(count(&h, "InGame", "SetActive(true)"), 4);
    assert!(h
        .log()
        .contains("resumed autoplay for 4 turns after closing diplomacy"));
}

#[test]
fn test_deal_proposal_is_refused_before_the_statement_view_closes() {
    let mut game = Game::new(&[2]);
    game.deal = true;
    let (h, game, detail) = autoplay(game, 3);
    assert!(detail.unwrap().contains("(1->4"));
    assert_closed(
        &h,
        &game.lock().unwrap(),
        &["DiplomacyDealView", "DiplomacyActionView"],
    );
    assert!(count(&h, "DiplomacyDealView", "OnLocalPlayerTurnEnd()") > 0);
    assert!(count(&h, "DiplomacyActionView", "OnClose()") > 0);
    assert!(h
        .log()
        .contains("closed AI deal proposal, AI diplomacy statement"));
}

#[test]
fn test_status_query_starved_by_statement_closes_it_and_asks_again() {
    let mut game = Game::new(&[2]);
    game.starve = true;
    let (h, game, detail) = autoplay(game, 3);
    assert!(detail.unwrap().contains("(1->4"));
    assert_closed(&h, &game.lock().unwrap(), &["DiplomacyActionView"]);
}

#[test]
fn test_dark_age_policy_choice_is_closed_without_holding_autoplay() {
    let mut game = Game::new(&[]);
    game.visible.insert("JAC_SengokuDarkPolicy".into(), true);
    let (h, game, detail) = autoplay(game, 2);
    assert!(detail.unwrap().contains("(1->3"));
    assert_closed(&h, &game.lock().unwrap(), &["JAC_SengokuDarkPolicy"]);
    assert!(count(&h, "JAC_SengokuDarkPolicy", "OnClose()") > 0);
    assert!(h.log().contains("closed dark-age policy choice"));
}

#[test]
fn test_oath_choice_is_closed_after_reign_without_selecting_official_dedications() {
    let mut game = Game::new(&[]);
    game.visible.insert("JAC_SengokuReignPopup".into(), true);
    game.visible.insert("DedicationPopup".into(), false);
    game.next_screen = Some("DedicationPopup".into());
    let (h, game, detail) = autoplay(game, 2);
    assert!(detail.unwrap().contains("(1->3"));
    assert_closed(
        &h,
        &game.lock().unwrap(),
        &["JAC_SengokuReignPopup", "DedicationPopup"],
    );
    assert!(h
        .log()
        .contains("closed era-name change screen, house oath choice"));
    assert!(count(&h, "DedicationPopup", "OnClose()") > 0);
}

#[test]
fn test_era_name_change_screen_is_closed_then_the_dark_policy_choice_it_opens() {
    let mut game = Game::new(&[]);
    game.visible.insert("JAC_SengokuReignPopup".into(), true);
    game.visible.insert("JAC_SengokuDarkPolicy".into(), false);
    game.next_screen = Some("JAC_SengokuDarkPolicy".into());
    let (h, game, detail) = autoplay(game, 2);
    assert!(detail.unwrap().contains("(1->3"));
    assert_closed(
        &h,
        &game.lock().unwrap(),
        &["JAC_SengokuReignPopup", "JAC_SengokuDarkPolicy"],
    );
    assert!(h
        .log()
        .contains("closed era-name change screen, dark-age policy choice"));
}

#[test]
fn test_screen_that_keeps_reopening_is_bounded() {
    let mut game = Game::new(&[]);
    game.reopen = true;
    let (_h, game, result) = autoplay(game, 1);
    assert!(result.unwrap_err().to_string().contains("keeps reopening"));
    assert_eq!(game.lock().unwrap().closed.len(), 26);
}

const WAIT: &str = r#"{"action":"wait_for","timeout":600,"condition":{"type":"lua","state_name":"InGame","code":"return Players[0]:IsTurnActive()"}}"#;
#[test]
fn test_wait_closes_statement_and_polls_again() {
    let open = Arc::new(Mutex::new(true));
    let open_lua = open.clone();
    let fake = FakeBackend::new()
        .states(|| Ok(vec!["InGame".into(), "DiplomacyActionView".into()]))
        .lua(move |state, _| {
            if state == "DiplomacyActionView" {
                let mut v = open_lua.lock().unwrap();
                let was = *v;
                *v = false;
                return Ok(was.to_string());
            }
            if *open_lua.lock().unwrap() {
                Err(Error::unknown(
                    "FireTuner response timed out after 0/8 bytes",
                ))
            } else {
                Ok("true".into())
            }
        });
    let mut h = Harness::new(json!({}), fake);
    let step = serde_json::from_str(WAIT).unwrap();
    assert!(h.runner.run_step(&step).unwrap().contains("condition met"));
    assert!(h
        .log()
        .contains("closed AI diplomacy statement after an InGame timeout; retry 1/3"));
}

#[test]
fn test_wait_still_fails_when_nothing_was_open_or_retries_run_out() {
    for visible in [false, true] {
        let fake = FakeBackend::new()
            .states(|| Ok(vec!["InGame".into(), "DiplomacyActionView".into()]))
            .lua(move |state, _| {
                if state == "DiplomacyActionView" {
                    Ok(visible.to_string())
                } else {
                    Err(Error::unknown("timed out"))
                }
            });
        let mut h = Harness::new(json!({}), fake);
        let step = serde_json::from_str(WAIT).unwrap();
        assert_eq!(
            h.runner.run_step(&step).unwrap_err().kind,
            ErrorKind::LuaResponseUnknown
        );
        if visible {
            assert_eq!(h.log().matches("ingame_unblock").count(), 3);
        }
    }
}

#[test]
fn test_ui_lua_is_never_replayed_after_it_may_have_run() {
    let fake = FakeBackend::new().lua(|state, _| {
        if state == "DiplomacyActionView" {
            Ok("true".into())
        } else {
            Err(Error::unknown("timed out"))
        }
    });
    let mut h = Harness::new(json!({}), fake);
    assert_eq!(
        h.runner
            .run_step(&json!({"action":"ui_lua","state_name":"InGame","code":"return 1"}))
            .unwrap_err()
            .kind,
        ErrorKind::LuaResponseUnknown
    );
    assert_eq!(
        h.calls()
            .iter()
            .map(|(s, _)| s.as_str())
            .collect::<Vec<_>>(),
        ["InGame"]
    );
}
