#[path = "fixtures/banseiki_ruleset.rs"]
mod banseiki_ruleset;
mod support_fixture;

use civ6_autotest_rs::scenario::{
    build_setup_code, configure_code, fast_forward_scenario, validate_scenario,
};
use mlua::{Function, Lua};
use regex::Regex;
use serde_json::{Value, json};
use support_fixture::{action, code, compile_lua, expect_error, lua_error, root, scenario, steps};

const PROBE_MOD: &str = "57a3af09-fb38-41da-b367-818de11a995c";
const TITLE_PROXY_MOD: &str = "5d4393c0-c02a-48bf-83d2-7b6839d4e5cb";
const NATIVE_SLOTS_MOD: &str = "a1faab65-5371-4e25-9a46-7522075f865d";
const NATIVE_SLOT_ONLY_MOD: &str = "8c615430-5a09-4431-877a-bfd03bca9e80";
const STATIC_GATE_MOD: &str = "c678a216-58ab-4d40-a614-8f309d36ac86";
const ENEMY_GATE_MOD: &str = "bba3a0d2-8f08-4715-9263-00a773e6ae86";
const TRAIT_GRANT_MOD: &str = "2b20f0d9-64cd-42b8-9841-74067419b81e";
const PROPERTY_OBSERVER_MOD: &str = "d8532710-f853-4c53-8853-271000000001";
const NEW_SCENARIOS: [&str; 7] = [
    "japan-15km-20turn-full-metrics.json",
    "japan-15km-300turn-full-metrics.json",
    "sengoku-era-reigns-epic.json",
    "sengoku-era-reigns-marathon.json",
    "sengoku-era-reigns-beyond-nine.json",
    "great-people-zero-musicians.json",
    "resources-calendar-epic.json",
];

fn ui_codes(data: &Value) -> Vec<&str> {
    steps(data)
        .iter()
        .filter(|s| action(s, "ui_lua"))
        .map(code)
        .collect()
}

fn embedded_lua(step: &Value) -> Option<&str> {
    step["code"]
        .as_str()
        .filter(|source| !source.is_empty())
        .or_else(|| {
            step["condition"]["code"]
                .as_str()
                .filter(|source| !source.is_empty())
        })
}

#[test]
fn property_observer_scene_is_bounded_and_all_embedded_lua_parses() {
    let data = scenario("leader-persona-property-observer-15km.json");
    validate_scenario(&data).unwrap();
    assert_eq!(data["strict_lua_errors"], true);
    let actions = steps(&data);
    assert!(
        actions[0]["ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "d8532710-f853-4c53-8853-271000000001")
    );
    let start = actions
        .iter()
        .position(|step| action(step, "start_native_property_trace"))
        .unwrap();
    let stop = actions
        .iter()
        .position(|step| action(step, "stop_native_property_trace"))
        .unwrap();
    assert!(start < stop && actions[start]["max_seconds"] == 90);
    assert!(actions[start + 1..stop].iter().all(|step| {
        !["save", "load", "end_turn", "autoplay", "start_game"]
            .contains(&step["action"].as_str().unwrap())
    }));
    let lua = Lua::new();
    for step in actions {
        if let Some(source) = embedded_lua(step) {
            compile_lua(&lua, source);
        }
    }
}

#[test]
fn numeric_property_scene_uses_only_fixed_numeric_test_keys() {
    let data = scenario("leader-persona-property-numeric-observer-15km.json");
    validate_scenario(&data).unwrap();
    let actions = steps(&data);
    assert_eq!(actions.len(), 23);
    let start = actions
        .iter()
        .position(|s| action(s, "start_native_property_trace"))
        .unwrap();
    let stop = actions
        .iter()
        .position(|s| action(s, "stop_native_property_trace"))
        .unwrap();
    assert_eq!(actions[start]["scope"], "numeric_test_keys_853271_853272");
    assert!(start < stop && actions[start]["max_seconds"] == 90);
    assert!(actions[start + 1..stop].iter().all(|s| {
        !["save", "load", "end_turn", "autoplay"].contains(&s["action"].as_str().unwrap())
    }));
    let lua = Lua::new();
    let sources = actions.iter().filter_map(embedded_lua).collect::<Vec<_>>();
    assert!(
        sources
            .iter()
            .any(|s| s.contains("JAC_PropertyObserverNumericStep"))
    );
    assert!(
        sources
            .iter()
            .all(|s| !s.contains("'JAC_TEST853_VALUE'") && !s.contains("'JAC_TEST853_PHASE'"))
    );
    assert!(
        sources
            .iter()
            .any(|s| s.contains("Game:GetProperty(853271)"))
    );
    assert!(
        sources
            .iter()
            .any(|s| s.contains("Game:GetProperty(853272)"))
    );
    for source in sources {
        compile_lua(&lua, source);
    }
}

#[test]
fn persona_boundary_scenarios_validate_without_operating_the_game() {
    let lua = Lua::new();
    for name in [
        "leader-persona-city-loss-15km.json",
        "leader-persona-spy-access-15km.json",
        "leader-persona-spy-production-15km.json",
        "leader-persona-spy-recovery.json",
    ] {
        let data = scenario(name);
        validate_scenario(&data).unwrap();
        assert_eq!(data["strict_lua_errors"], true, "{name}");
        assert!(
            steps(&data)
                .iter()
                .all(|step| !["restart_game", "autoplay"]
                    .contains(&step["action"].as_str().unwrap()))
        );
        for step in steps(&data) {
            if let Some(source) = embedded_lua(step) {
                compile_lua(&lua, source);
            }
        }
    }
    let data = scenario("leader-persona-spy-recovery.json");
    let recovery = steps(&data);
    assert!(code(&recovery[0]).contains("Check('NEUTRALIZED')"));
    assert_eq!(recovery[0]["capture_as"], recovery[6]["equals_capture"]);
    assert_eq!(
        recovery
            .iter()
            .filter(|step| action(step, "end_turn"))
            .count(),
        13
    );
    assert!(code(recovery.last().unwrap()).contains("Check('SETTLED')"));
    assert!(recovery.iter().all(|step| {
        !["configure", "start_game", "enable_mods"].contains(&step["action"].as_str().unwrap())
    }));
}

#[test]
fn multi_failure_inspection_preserves_the_latest_checkpoint_without_mutation() {
    let data = scenario("leader-persona-spy-neutralization-multi-failure-diagnose.json");
    validate_scenario(&data).unwrap();
    assert_eq!(steps(&data).len(), 5);
    assert_eq!(
        steps(&data)[0]["name"],
        "JAC-Enemy-Neutralization-91f02912dd354a18821959c99aa89868-0000000000000008"
    );
    assert!(steps(&data)[0].get("checkpoint").is_none());
    assert_eq!(steps(&data)[0]["exact_name"], true);
    assert!(steps(&data).iter().all(|step| {
        !["save", "end_turn", "configure", "start_game", "enable_mods"]
            .contains(&step["action"].as_str().unwrap())
    }));
    let lua = Lua::new();
    compile_lua(&lua, civ6_autotest_rs::spy_failures::READ);
    let mut bad = data.clone();
    bad["steps"][3]["code"] = json!("Game:SetProperty('changed',1)");
    assert!(validate_scenario(&bad).is_err());
}

#[test]
fn third_capacity_probe_reads_the_preserved_failure_without_changing_the_game() {
    let data = scenario("leader-persona-spy-third-capacity-probe.json");
    validate_scenario(&data).unwrap();
    let actions = steps(&data);
    assert_eq!(actions.len(), 4);
    assert_eq!(
        actions[0]["name"],
        "JAC-Enemy-Neutralization-91f02912dd354a18821959c99aa89868-0000000000000008"
    );
    assert_eq!(actions[0]["exact_name"], true);
    assert!(actions[0].get("checkpoint").is_none());
    assert!(action(&actions[1], "pause") && actions[1]["seconds"] == 2);
    assert_eq!(actions[2]["lua_timeout"], 20);
    assert!(actions.iter().all(|step| {
        ![
            "save",
            "end_turn",
            "configure",
            "start_game",
            "enable_mods",
            "autoplay",
        ]
        .contains(&step["action"].as_str().unwrap())
    }));
    assert!(action(&actions[3], "exit_to_menu"));
    assert!(code(&actions[2]).contains("CIVIC_GRANT_SPY"));
    let lua = Lua::new();
    for step in actions {
        if let Some(source) = embedded_lua(step) {
            compile_lua(&lua, source);
        }
    }
}

#[test]
fn third_spy_research_requests_once_and_saves_each_real_turn_before_reading_it() {
    let request = scenario("leader-persona-spy-third-civic-request.json");
    let tick = scenario("leader-persona-spy-third-civic-tick.json");
    let ready = scenario("leader-persona-spy-third-civic-ready.json");
    validate_scenario(&request).unwrap();
    validate_scenario(&tick).unwrap();
    validate_scenario(&ready).unwrap();
    let request_steps = steps(&request);
    let tick_steps = steps(&tick);
    assert_eq!(
        request_steps[0]["name"],
        "JAC-Enemy-Neutralization-91f02912dd354a18821959c99aa89868-0000000000000008"
    );
    assert!(request_steps[0].get("checkpoint").is_none());
    assert_eq!(
        ui_codes(&request)
            .iter()
            .filter(|source| source.contains("Queue('THIRD_SPY_CIVIC')"))
            .count(),
        1
    );
    assert_eq!(
        ui_codes(&request)
            .iter()
            .filter(|source| source.contains("Queue('THIRD_SPY_CIVIC_INTENT')"))
            .count(),
        1
    );
    assert!(request_steps.iter().all(|step| !action(step, "end_turn")));
    let intent_index = request_steps
        .iter()
        .position(|step| {
            action(step, "ui_lua") && code(step).contains("Queue('THIRD_SPY_CIVIC_INTENT')")
        })
        .unwrap();
    let request_index = request_steps
        .iter()
        .position(|step| action(step, "ui_lua") && code(step).contains("Queue('THIRD_SPY_CIVIC')"))
        .unwrap();
    let saves: Vec<_> = request_steps
        .iter()
        .enumerate()
        .filter_map(|(i, step)| action(step, "save").then_some(i))
        .collect();
    assert_eq!(saves.len(), 2);
    let reload = request_steps
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(i, step)| action(step, "load").then_some(i))
        .unwrap();
    assert!(intent_index < saves[0] && saves[0] < request_index);
    assert!(request_index < saves[1] && saves[1] < reload);
    assert!(
        saves
            .iter()
            .all(|&i| request_steps[i]["checkpoint"] == true)
    );
    assert_eq!(request_steps[reload]["checkpoint"], true);
    assert_eq!(request_steps[8]["action"], "inspect_native_spy_research");
    assert_eq!(request_steps[8]["phase"], "after_acceptance");
    assert_eq!(
        request_steps[reload + 2]["action"],
        "inspect_native_spy_research"
    );
    assert_eq!(request_steps[reload + 2]["phase"], "after_reload");
    assert_eq!(tick_steps[0]["checkpoint"], true);
    assert_eq!(
        tick_steps
            .iter()
            .filter(|step| action(step, "end_turn"))
            .count(),
        1
    );
    let turn = tick_steps
        .iter()
        .position(|step| action(step, "end_turn"))
        .unwrap();
    let save = tick_steps
        .iter()
        .position(|step| action(step, "save"))
        .unwrap();
    assert!(turn < save && save < tick_steps.len() - 2);
    assert_eq!(tick_steps[turn]["turns"], 1);
    let ready_steps = steps(&ready);
    assert_eq!(ready_steps.len(), 4);
    assert_eq!(ready_steps[0]["checkpoint"], true);
    assert_eq!(ready_steps[2]["action"], "inspect_native_spy_research");
    assert_eq!(ready_steps[2]["phase"], "capacity_ready");
    assert_eq!(tick_steps[5]["action"], "inspect_native_spy_research");
    assert_eq!(tick_steps[5]["phase"], "after_turn");
    assert!(
        ready_steps
            .iter()
            .all(|step| !action(step, "save") && !action(step, "end_turn"))
    );
    for data in [&request, &tick, &ready] {
        assert_eq!(data["strict_lua_errors"], true);
        assert!(
            steps(data)
                .iter()
                .all(|step| !action(step, "autoplay") && !action(step, "start_game"))
        );
        let lua = Lua::new();
        for step in steps(data) {
            if let Some(source) = embedded_lua(step) {
                compile_lua(&lua, source);
            }
        }
    }
    compile_lua(&Lua::new(), &civ6_autotest_rs::spy_research::read());
    let mut bad = request.clone();
    bad["steps"][8]["phase"] = json!("../escape");
    assert!(validate_scenario(&bad).is_err());
    bad["steps"][8]["phase"] = json!("after_acceptance");
    bad["steps"][8]["code"] = json!("UI.RequestPlayerOperation(0,1,{})");
    assert!(validate_scenario(&bad).is_err());
}

#[test]
fn enemy_neutralization_scenarios_use_distinct_actors_and_single_turn_checkpoints() {
    let lua = Lua::new();
    let mut checkpoint_count = 0;
    let names = [
        "start-15km",
        "native-tick",
        "travel",
        "travel-tick",
        "mission",
        "mission-tick",
        "recovery",
        "recovery-tick",
        "reassign",
        "reassign-tick",
    ];
    for suffix in names {
        let name = format!("leader-persona-spy-neutralization-{suffix}.json");
        let data = scenario(&name);
        validate_scenario(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(data["strict_lua_errors"], true, "{name}");
        let steps = steps(&data);
        for step in steps
            .iter()
            .filter(|step| action(step, "save") || action(step, "load"))
        {
            checkpoint_count += 1;
            assert_eq!(step["name"], "JAC-Enemy-Neutralization", "{name}");
            assert_eq!(step["exact_name"], true, "{name}");
            assert_eq!(step["checkpoint"], true, "{name}");
        }
        assert!(
            steps
                .iter()
                .all(|step| !action(step, "autoplay") && !action(step, "restart_game"))
        );
        for step in steps {
            if let Some(source) = embedded_lua(step) {
                compile_lua(&lua, source);
            }
        }
        if suffix.ends_with("tick") {
            assert!(action(&steps[0], "load"), "{name}");
            assert!(action(steps.last().unwrap(), "exit_to_menu"), "{name}");
            assert_eq!(
                steps.iter().filter(|s| action(s, "end_turn")).count(),
                1,
                "{name}"
            );
            assert_eq!(
                steps.iter().filter(|s| action(s, "save")).count(),
                usize::from(suffix == "mission-tick") + 1,
                "{name}"
            );
        }
    }
    let start = scenario("leader-persona-spy-neutralization-start-15km.json");
    let setup = &start["steps"][1]["players"];
    assert_eq!(setup[0]["human"], true);
    assert_eq!(setup[1]["leader"], "LEADER_JAC_DATE_TANEMUNE");
    assert_eq!(setup[2]["leader"], "LEADER_JAC_ASHINA_MORIKIYO");
    assert!(
        start["steps"][0]["ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == ENEMY_GATE_MOD)
    );
    let joined = steps(&start)
        .iter()
        .filter_map(embedded_lua)
        .collect::<Vec<_>>();
    assert!(
        joined
            .iter()
            .any(|code| code.contains("Queue('PREPARE_ENEMY')"))
    );
    let position = |needle: &str| {
        joined
            .iter()
            .position(|code| code.contains(needle))
            .unwrap_or_else(|| panic!("missing enemy setup stage: {needle}"))
    };
    assert!(position("Queue('PREPARE_ENEMY')") < position("Queue('EVOLVE_ENEMY')"));
    assert!(position("Queue('EVOLVE_ENEMY')") < position("QueueEnemy('APPOINT')"));
    let evolve_step = steps(&start)
        .iter()
        .position(|step| {
            embedded_lua(step).is_some_and(|code| code.contains("Queue('EVOLVE_ENEMY')"))
        })
        .unwrap();
    let first_ai_turn = steps(&start)
        .iter()
        .position(|step| action(step, "end_turn"))
        .unwrap();
    assert!(
        evolve_step < first_ai_turn,
        "enemy must evolve before the first AI turn"
    );
    assert!(
        joined
            .iter()
            .any(|code| code.contains("QueueEnemy('APPOINT')"))
    );
    assert!(joined.iter().any(|code| code.contains("Queue('BIND_SPY')")));
    let mission = scenario("leader-persona-spy-neutralization-mission.json");
    assert_eq!(
        steps(&mission)
            .iter()
            .filter_map(embedded_lua)
            .filter(|code| code.contains("Queue('SPY_MISSION'"))
            .count(),
        1
    );
    let recovery = scenario("leader-persona-spy-neutralization-recovery.json");
    assert_eq!(
        steps(&recovery)
            .iter()
            .filter(|s| s.get("capture_as").is_some())
            .count(),
        1
    );
    assert_eq!(
        steps(&recovery)
            .iter()
            .filter(|s| s.get("equals_capture").is_some())
            .count(),
        1
    );
    assert_eq!(checkpoint_count, 21);
}

#[test]
fn enemy_failed_mission_retry_has_fixed_reusable_checkpoint_scenarios() {
    let lua = Lua::new();
    let names = [
        "retry-diagnose",
        "retry-captive-exchange-probe",
        "retry-captive-exchange",
        "retry-capacity-plan",
        "retry-capacity-civic",
        "retry-capacity-government-plan",
        "retry-capacity-government-classify",
        "retry-capacity-tier1-production-diagnose",
        "retry-capacity-government-tier1",
        "retry-capacity-government-tier2",
        "retry-capacity-tier2-civic",
        "retry-capacity-district",
        "retry-capacity-district-tick",
        "retry-capacity-build",
        "retry-capacity-tick",
        "retry-prepare",
        "retry-produce",
        "retry-production-tick",
        "retry-production-bind",
        "retry-return-tick",
        "retry-native-ready-tick",
        "retry-living-read",
        "retry-adopt",
    ];
    let mut saves = 0;
    for suffix in names {
        let name = format!("leader-persona-spy-neutralization-{suffix}.json");
        let data = scenario(&name);
        validate_scenario(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(data["strict_lua_errors"], true, "{name}");
        let steps = steps(&data);
        assert!(action(&steps[0], "load") && action(steps.last().unwrap(), "exit_to_menu"));
        for step in steps {
            assert!(!action(step, "configure") && !action(step, "start_game"));
            assert!(!action(step, "restart_game") && !action(step, "autoplay"));
            if action(step, "save") || action(step, "load") {
                assert_eq!(step["name"], "JAC-Enemy-Neutralization", "{name}");
                assert_eq!(step["exact_name"], true, "{name}");
                assert_eq!(step["checkpoint"], true, "{name}");
                if action(step, "save") {
                    saves += 1;
                }
            }
            if let Some(source) = embedded_lua(step) {
                compile_lua(&lua, source);
            }
        }
        assert_eq!(
            steps.iter().filter(|s| action(s, "end_turn")).count(),
            if suffix == "retry-capacity-tier2-civic" {
                // First publish researched ancestors to UI, then confirm the
                // target Civic and official Government after their own turn.
                2
            } else {
                usize::from(
                    suffix.ends_with("tick")
                        || suffix == "retry-capacity-civic"
                        || suffix == "retry-capacity-government-tier1"
                        || suffix == "retry-capacity-government-classify",
                )
            },
            "{name}"
        );
    }
    assert_eq!(saves, 18);
    let production =
        scenario("leader-persona-spy-neutralization-retry-capacity-tier1-production-diagnose.json");
    let production_steps = steps(&production);
    assert_eq!(production_steps.len(), 4);
    assert!(production_steps.iter().all(|step| {
        !action(step, "save") && !action(step, "end_turn") && !action(step, "configure")
    }));
    assert!(
        embedded_lua(&production_steps[2])
            .unwrap()
            .contains("Tier1ProductionDiagnosis()")
    );
    let classification =
        scenario("leader-persona-spy-neutralization-retry-capacity-government-classify.json");
    let classification_steps = steps(&classification);
    let request = classification_steps
        .iter()
        .position(|s| {
            action(s, "ui_lua")
                && embedded_lua(s)
                    .is_some_and(|code| code.contains("Queue('CLASSIFY_CAPACITY_GOVERNMENT')"))
        })
        .unwrap();
    let turn = classification_steps
        .iter()
        .position(|s| action(s, "end_turn"))
        .unwrap();
    let read = classification_steps
        .iter()
        .position(|s| {
            action(s, "ui_lua")
                && embedded_lua(s).is_some_and(|code| {
                    code.contains("Queue('READ_CAPACITY_GOVERNMENT_CLASSIFICATION')")
                })
        })
        .unwrap();
    let proof = classification_steps
        .iter()
        .position(|s| {
            action(s, "ui_lua")
                && embedded_lua(s)
                    .is_some_and(|code| code.contains("x.status~='NATIVE_GOV_ACCEPTED'"))
        })
        .unwrap();
    let save = classification_steps
        .iter()
        .position(|s| action(s, "save"))
        .unwrap();
    assert!(request < turn && turn < read && read < proof && proof < save);
    assert!(
        classification_steps[proof]["code"]
            .as_str()
            .unwrap()
            .contains("x.afterTotal==r.total")
    );
    for (suffix, civic_action) in [
        (
            "retry-capacity-government-tier1",
            "PREPARE_TIER1_GOV_WINDOW_CIVIC",
        ),
        ("retry-capacity-tier2-civic", "PREPARE_TIER2_CAPACITY_CIVIC"),
    ] {
        let data = scenario(&format!("leader-persona-spy-neutralization-{suffix}.json"));
        let steps = steps(&data);
        let civic = steps
            .iter()
            .position(|s| {
                action(s, "ui_lua")
                    && embedded_lua(s)
                        .is_some_and(|code| code.contains(&format!("Queue('{civic_action}')")))
            })
            .unwrap();
        let government = steps
            .iter()
            .position(|s| {
                action(s, "ui_lua")
                    && embedded_lua(s)
                        .is_some_and(|code| code.contains("Queue('CHANGE_CAPACITY_GOVERNMENT')"))
            })
            .unwrap();
        let turn = steps
            .iter()
            .enumerate()
            .skip(government + 1)
            .find_map(|(index, s)| action(s, "end_turn").then_some(index))
            .unwrap();
        if suffix == "retry-capacity-tier2-civic" {
            let research = steps
                .iter()
                .position(|s| action(s, "ui_lua") && s["state_name"] == "GameCore_Tuner")
                .unwrap();
            assert!(action(&steps[research + 1], "end_turn"));
            assert_eq!(steps[research + 1]["turns"], 1);
            assert!(action(&steps[research + 2], "wait_for"));
            assert!(
                embedded_lua(&steps[research + 2])
                    .unwrap()
                    .contains("#t.missingPrerequisites==0")
            );
            assert!(research + 2 < civic);
        }
        let save = steps.iter().position(|s| action(s, "save")).unwrap();
        assert!(
            civic < government && government < turn && turn < save,
            "{suffix}"
        );
        assert_eq!(steps[turn]["turns"], 1);
        assert!(
            steps[save - 1]["code"]
                .as_str()
                .unwrap()
                .contains("d.mission.turn==23")
        );
    }
    let old_tier2 =
        scenario("leader-persona-spy-neutralization-retry-capacity-government-tier2.json");
    assert!(steps(&old_tier2).iter().all(|s| !action(s, "save")
        && !action(s, "end_turn")
        && !embedded_lua(s).is_some_and(|code| code.contains("Queue("))));
    let probe = scenario("leader-persona-spy-neutralization-retry-captive-exchange-probe.json");
    let probe_codes: Vec<_> = steps(&probe).iter().filter_map(embedded_lua).collect();
    assert!(
        probe_codes
            .iter()
            .any(|code| code.contains("ProbeCapturedExchange()")
                && code.contains("d.occupied==1")
                && code.contains("r.nameIndex==-1"))
    );
    assert!(
        !probe_codes
            .iter()
            .any(|code| code.contains("Queue(") || code.contains("RequestSession"))
    );
    let exchange = scenario("leader-persona-spy-neutralization-retry-captive-exchange.json");
    let exchange_steps = steps(&exchange);
    let exchange_codes: Vec<_> = exchange_steps.iter().filter_map(embedded_lua).collect();
    let exchange_index = |needle: &str| {
        exchange_codes
            .iter()
            .position(|code| code.contains(needle))
            .unwrap()
    };
    assert!(
        exchange_index("BeginCaptiveExchange()") < exchange_index("Queue('MARK_CAPTIVE_EXCHANGE')")
    );
    assert!(
        exchange_index("Queue('MARK_CAPTIVE_EXCHANGE')")
            < exchange_index("ProposeCaptiveExchange()")
    );
    assert!(exchange_index("ProposeCaptiveExchange()") < exchange_index("AcceptCaptiveExchange()"));
    assert!(
        exchange_index("ProposeCaptiveExchange()")
            < exchange_index("CaptiveExchangeReplyDiagnostic()")
            && exchange_index("CaptiveExchangeReplyDiagnostic()")
                < exchange_index("AcceptCaptiveExchange()")
    );
    let reply_wait = exchange_steps
        .iter()
        .position(|step| {
            action(step, "wait_for")
                && embedded_lua(step)
                    .is_some_and(|code| code.contains("CaptiveExchangeReplyWaitElapsed(30)"))
        })
        .unwrap();
    assert_eq!(exchange_steps[reply_wait]["timeout"], 45);
    let reply_diagnostic = exchange_steps
        .iter()
        .position(|step| {
            action(step, "ui_lua")
                && embedded_lua(step).is_some_and(|code| {
                    code.contains("CaptiveExchangeReplyDiagnostic()")
                        && code.contains("r.sessionMatches")
                        && code.contains("r.stage=='AI_ACCEPTED_EXACT'")
                        && code.contains("r.stage=='AI_REFUSED'")
                })
        })
        .unwrap();
    assert!(reply_wait < reply_diagnostic);
    assert!(exchange_index("AcceptCaptiveExchange()") < exchange_index("#d.roster.captured==0"));
    let screen_ready = exchange_steps
        .iter()
        .position(|step| {
            action(step, "wait_for")
                && step["condition"]["type"] == "state"
                && step["condition"]["name"] == "DiplomacyDealView"
        })
        .unwrap();
    let session_observed = exchange_steps
        .iter()
        .position(|step| {
            action(step, "observe_captive_session")
                && step["attacker"] == 0
                && step["captor"] == 1
                && step["lua_timeout"] == 12
        })
        .unwrap();
    let outcome = exchange_steps
        .iter()
        .position(|step| {
            action(step, "ui_lua")
                && embedded_lua(step).is_some_and(|code| {
                    code.contains("#d.roster.captured==0") && code.contains("d.spyID==655368")
                })
        })
        .unwrap();
    let mark = exchange_steps
        .iter()
        .position(|step| {
            action(step, "ui_lua")
                && embedded_lua(step)
                    .is_some_and(|code| code.contains("Queue('MARK_CAPTIVE_EXCHANGE')"))
        })
        .unwrap();
    assert!(screen_ready < session_observed && session_observed < mark && mark < outcome);
    assert!(
        exchange_steps[session_observed + 1..=outcome]
            .iter()
            .all(|step| {
                if action(step, "wait_for") {
                    step["condition"]["preserve_diplomacy_session"] == true
                } else if action(step, "ui_lua") {
                    step["preserve_diplomacy_session"] == true
                } else {
                    false
                }
            })
    );
    let save = exchange_steps
        .iter()
        .position(|step| action(step, "save"))
        .unwrap();
    assert!(outcome < save);
    let civic = scenario("leader-persona-spy-neutralization-retry-capacity-civic.json");
    let civic_steps = steps(&civic);
    let civic_turn = civic_steps
        .iter()
        .position(|step| action(step, "end_turn"))
        .unwrap();
    let civic_resume = civic_steps
        .iter()
        .position(|step| {
            action(step, "ui_lua")
                && embedded_lua(step)
                    .is_some_and(|code| code.contains("Queue('RESUME_CAPACITY_CIVIC_READ')"))
        })
        .unwrap();
    assert!(civic_turn < civic_resume);
    assert_eq!(civic_steps[civic_turn]["turns"], 1);
    assert!(civic_steps[civic_turn]["timeout"].as_u64().unwrap() >= 240);
    assert!(civic["test_mode"].is_null());
    assert!(
        civic_steps
            .iter()
            .all(|step| !action(step, "autoplay") && step["fast_forward"] != true)
    );
    let civic_verified = civic_steps
        .iter()
        .position(|step| {
            action(step, "ui_lua")
                && embedded_lua(step).is_some_and(|code| {
                    code.contains("p.districtCivicOwned")
                        && code.contains("r.afterSpent==r.beforeSpent")
                })
        })
        .unwrap();
    assert!(civic_resume < civic_verified);
    assert!(
        civic_verified
            < civic_steps
                .iter()
                .position(|step| action(step, "save"))
                .unwrap()
    );
    let district = scenario("leader-persona-spy-neutralization-retry-capacity-district.json");
    let district_codes: Vec<_> = steps(&district).iter().filter_map(embedded_lua).collect();
    let district_index = |needle: &str| {
        district_codes
            .iter()
            .position(|code| code.contains(needle))
            .unwrap()
    };
    let district_build = district_codes[district_index("Queue('BUILD_CAPACITY_DISTRICT')")];
    assert!(
        district_build.find("p.plot and p.x and p.y").unwrap()
            < district_build
                .find("Queue('BUILD_CAPACITY_DISTRICT')")
                .unwrap()
            && district_index("Queue('BUILD_CAPACITY_DISTRICT')")
                < district_index("Queue('PROGRESS_CAPACITY_DISTRICT')")
    );
    let district_tick =
        scenario("leader-persona-spy-neutralization-retry-capacity-district-tick.json");
    let district_tick_steps = steps(&district_tick);
    let built = district_tick_steps
        .iter()
        .position(|step| {
            action(step, "ui_lua")
                && embedded_lua(step).is_some_and(|code| {
                    code.contains("o.stage=='DISTRICT_BUILT'")
                        && code.contains("o.targetBuilt")
                        && code.contains("o.owner==r.owner")
                        && code.contains("o.cityOwner==r.cityOwner")
                        && code.contains("o.cityID==r.cityID")
                        && code.contains("o.districtID==r.districtID")
                        && code.contains("o.plot==r.plot")
                        && code.contains("o.memberKey==r.memberKey")
                        && code.contains("o.acceptedNonce==r.acceptedNonce")
                        && code.contains("o.captured==1")
                })
        })
        .unwrap();
    assert!(
        district_tick_steps
            .iter()
            .position(|step| action(step, "end_turn"))
            .unwrap()
            < built
            && built
                < district_tick_steps
                    .iter()
                    .position(|step| action(step, "save"))
                    .unwrap()
    );
    let capacity_plan = scenario("leader-persona-spy-neutralization-retry-capacity-plan.json");
    let plan_codes: Vec<_> = steps(&capacity_plan)
        .iter()
        .filter_map(embedded_lua)
        .collect();
    assert!(
        plan_codes
            .iter()
            .any(|code| code.contains("CaptiveCapacityPlan()"))
    );
    assert!(!plan_codes.iter().any(|code| code.contains("Queue(")));
    let capacity_build = scenario("leader-persona-spy-neutralization-retry-capacity-build.json");
    let build_codes: Vec<_> = steps(&capacity_build)
        .iter()
        .filter_map(embedded_lua)
        .collect();
    let position = |needle: &str| {
        build_codes
            .iter()
            .position(|code| code.contains(needle))
            .unwrap()
    };
    assert!(position("Queue('BUILD_CAPACITY')") < position("Queue('PROGRESS_CAPACITY')"));
    assert_eq!(
        build_codes
            .iter()
            .filter(|code| code.contains("Queue('PROGRESS_CAPACITY')"))
            .count(),
        1
    );
    let capacity_tick = scenario("leader-persona-spy-neutralization-retry-capacity-tick.json");
    let tick_steps = steps(&capacity_tick);
    let turn = tick_steps
        .iter()
        .position(|step| action(step, "end_turn"))
        .unwrap();
    let confirmed = tick_steps
        .iter()
        .enumerate()
        .skip(turn + 1)
        .find_map(|(index, step)| {
            (action(step, "ui_lua")
                && embedded_lua(step).is_some_and(|code| {
                    code.contains("o.targetBuilt")
                        && code.contains("o.stage=='TIER1_BUILT'")
                        && code.contains("o.stage=='CAPACITY_READY' and o.capacity==2")
                        && code.contains("o.captured==1")
                }))
            .then_some(index)
        })
        .unwrap();
    let save = tick_steps
        .iter()
        .position(|step| action(step, "save"))
        .unwrap();
    assert!(turn < confirmed && confirmed < save);
    assert!(tick_steps[turn + 1..confirmed].iter().any(|step| {
        action(step, "wait_for")
            && step["timeout"]
                .as_u64()
                .is_some_and(|seconds| seconds <= 120)
            && embedded_lua(step).is_some_and(|code| {
                code.contains("CaptiveCapacityOutcome()")
                    && code.contains("o.stage=='TIER1_BUILT'")
                    && code.contains("o.stage=='CAPACITY_READY'")
            })
    }));
    let prepare = scenario("leader-persona-spy-neutralization-retry-prepare.json");
    let codes: Vec<_> = steps(&prepare).iter().filter_map(embedded_lua).collect();
    assert!(codes.iter().any(|code| code.contains("d.spyID==655368")
        && code.contains("d.eventID==0")
        && code.contains("d.eventTurn==23")
        && code.contains("Queue('BEGIN_SPY_RETRY')")));
    assert!(
        codes
            .iter()
            .any(|code| code.contains("d.mode=='CAPACITY_MAP'")
                && code.contains("d.currentID~=d.spyID")
                && code.contains("d.capacity==2")
                && code.contains("d.roster.capturedPlaceholder"))
    );
    assert!(
        codes
            .iter()
            .any(|code| code.contains("r.archive.spyID==655368")
                && code.contains("r.binding.spyID==r.spyID")
                && code.contains("s.spyRoster.physicalCount==2"))
    );
    let prepare_steps = steps(&prepare);
    let verified = prepare_steps
        .iter()
        .position(|step| {
            embedded_lua(step).is_some_and(|code| code.contains("r.binding.spyID==r.spyID"))
        })
        .unwrap();
    let saved = prepare_steps
        .iter()
        .position(|step| action(step, "save"))
        .unwrap();
    assert!(verified < saved);
    let produce = scenario("leader-persona-spy-neutralization-retry-produce.json");
    let codes: Vec<_> = steps(&produce).iter().filter_map(embedded_lua).collect();
    let index = |needle: &str| codes.iter().position(|code| code.contains(needle)).unwrap();
    assert!(index("Queue('SPAWN_SPY')") < index("Queue('BUILD_SPY')"));
    assert!(index("Queue('BUILD_SPY')") < index("Queue('PROGRESS_SPY')"));
    assert_eq!(
        codes
            .iter()
            .filter(|code| code.contains("Queue('PROGRESS_SPY')"))
            .count(),
        1
    );
}

#[test]
fn travel_tick_distinguishes_old_captive_placeholder_from_moving_spy() {
    let data = scenario("leader-persona-spy-neutralization-travel-tick.json");
    let source = ui_codes(&data).last().copied().unwrap();
    let lua = Lua::new();
    lua.load(
        "moving=false; ExposedMembers={JAC_PersonaBoundary={\
         ReadEnemy=function() return {spyRoster={offMap={{returnTurn=-1}},\
         activeOffMap=moving and {{returnTurn=30}} or {}}} end,\
         EnemyTarget=function() return {owner=1,city=20} end,\
         Probe=function(owner,city) assert(owner==1 and city==20); return {canStart=true} end}}",
    )
    .exec()
    .unwrap();
    assert_eq!(lua.load(source).eval::<String>().unwrap(), "MISSION_READY");
    lua.load("moving=true").exec().unwrap();
    assert_eq!(lua.load(source).eval::<String>().unwrap(), "TRAVEL");
}

#[test]
fn travel_window_preserves_each_verified_turn_without_replaying_dispatch() {
    let single = scenario("leader-persona-spy-neutralization-travel-tick.json");
    let window = scenario("leader-persona-spy-neutralization-travel-window.json");
    validate_scenario(&window).unwrap();
    assert_eq!(window["strict_lua_errors"], true);
    let actions = steps(&window);
    assert_eq!(&actions[..2], &steps(&single)[..2]);
    assert_eq!(actions.len(), 18);
    for turn in actions[2..17].chunks_exact(5) {
        assert_eq!(turn, &steps(&single)[2..7]);
        assert_eq!(turn[0]["action"], "end_turn");
        assert_eq!(turn[0]["turns"], 1);
        assert_eq!(turn[3]["action"], "save");
        assert_eq!(turn[3]["checkpoint"], true);
        assert_eq!(turn[4]["action"], "ui_lua");
    }
    assert_eq!(actions.last().unwrap(), steps(&single).last().unwrap());
}

#[test]
fn mission_window_preserves_results_and_refuses_to_overrun_native_duration() {
    let window = scenario("leader-persona-spy-neutralization-mission-window.json");
    validate_scenario(&window).unwrap();
    assert_eq!(window["strict_lua_errors"], true);
    let actions = steps(&window);
    assert_eq!(actions.len(), 24);
    let guard = code(&actions[2]);
    let lua = Lua::new();
    lua.load("stage='MISSION'; remaining=4; present=true; operation=59; GameInfo={UnitOperations={UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR={Index=59,Hash=9059}}}; ExposedMembers={JAC_PersonaBoundary={EnemyProgress=function() return stage end,ReadEnemy=function() return {spy={present=present,remaining=remaining,operation=operation}} end}}")
        .exec().unwrap();
    assert_eq!(
        lua.load(guard).eval::<String>().unwrap(),
        "four native turns verified"
    );
    for operation in ["operation=60", "operation=nil", "operation='59'"] {
        lua.load(operation).exec().unwrap();
        assert!(lua.load(guard).eval::<String>().is_err(), "{operation}");
    }
    lua.load("operation=9059").exec().unwrap();
    assert!(lua.load(guard).eval::<String>().is_ok());
    lua.load("operation=59").exec().unwrap();
    for state in [
        "remaining=3",
        "remaining=nil",
        "remaining='4'",
        "remaining=4; present=false",
        "present=true; stage='NEUTRALIZED'",
        "stage='FAILED'",
    ] {
        lua.load(state).exec().unwrap();
        assert!(lua.load(guard).eval::<String>().is_err(), "{state}");
    }
    for turn in actions[3..23].chunks_exact(5) {
        assert_eq!(turn[0]["action"], "end_turn");
        assert_eq!(turn[0]["turns"], 1);
        // A finished native turn is saved even if its event cannot be observed.
        assert_eq!(turn[1]["action"], "save");
        assert_eq!(turn[1]["checkpoint"], true);
        assert_eq!(turn[2]["action"], "wait_for");
        assert_eq!(turn[3]["action"], "save");
        assert_eq!(turn[3]["checkpoint"], true);
        assert_eq!(turn[4]["action"], "ui_lua");
        lua.load("ExposedMembers.JAC_PersonaBoundary.EnemyProgress=function() return 'FAILED' end")
            .exec()
            .unwrap();
        assert!(lua.load(code(&turn[4])).eval::<String>().is_err());
        lua.load(
            "ExposedMembers.JAC_PersonaBoundary.EnemyProgress=function() return 'NEUTRALIZED' end",
        )
        .exec()
        .unwrap();
        assert_eq!(
            lua.load(code(&turn[4])).eval::<String>().unwrap(),
            "NEUTRALIZED"
        );
    }
    assert_eq!(actions.last().unwrap()["action"], "exit_to_menu");
    assert!(actions.iter().all(|step| {
        !["autoplay", "start_game"].contains(&step["action"].as_str().unwrap())
            && step["fast_forward"] != true
    }));
    for source in actions.iter().filter_map(embedded_lua) {
        compile_lua(&lua, source);
        assert!(
            !source.contains("Queue("),
            "a mission window must not replay a mission"
        );
    }
}

#[test]
fn single_mission_tick_rejects_other_operations_before_advancing() {
    let tick = scenario("leader-persona-spy-neutralization-mission-tick.json");
    let actions = steps(&tick);
    assert_eq!(actions[2]["action"], "ui_lua");
    assert_eq!(actions[3]["action"], "end_turn");
    let guard = code(&actions[2]);
    let lua = Lua::new();
    lua.load("remaining=1; operation=59; GameInfo={UnitOperations={UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR={Index=59,Hash=9059}}}; ExposedMembers={JAC_PersonaBoundary={EnemyProgress=function() return 'MISSION' end,ReadEnemy=function() return {spy={present=true,remaining=remaining,operation=operation}} end}}")
        .exec().unwrap();
    assert_eq!(
        lua.load(guard).eval::<String>().unwrap(),
        "one native turn verified"
    );
    lua.load("operation=60").exec().unwrap();
    assert!(lua.load(guard).eval::<String>().is_err());
    lua.load("operation=59; remaining=0").exec().unwrap();
    assert!(lua.load(guard).eval::<String>().is_err());
}

#[test]
fn retry_prepare_saves_only_a_ready_or_waiting_capacity_spy() {
    let data = scenario("leader-persona-spy-neutralization-retry-prepare.json");
    let source = ui_codes(&data).last().copied().unwrap();
    let lua = Lua::new();
    lua.load(
        "mode='CAPACITY_MAP'; stage='HOME_READY'; Game={GetProperty=function(_,key)\
         assert(key=='JAC_BOUNDARY_ENEMY_SPY_RETRY'); return {current=2,mode=mode,spyID=786440,\
         archive={spyID=655368,eventID=0,eventTurn=23},\
         binding={source='BUILDING_GOV_SPIES',spyID=786440}} end};\
         ExposedMembers={JAC_PersonaBoundary={\
         ReadEnemy=function() return {spy={present=true,id=786440},\
         spyRoster={captured={{}},capturedPlaceholder={offMapIndex=1},physicalCount=2}} end,\
         RetryLivingSpy=function() return stage end}}",
    )
    .exec()
    .unwrap();
    assert_eq!(
        lua.load(source).eval::<String>().unwrap(),
        "CAPACITY_MAP HOME_READY"
    );
    lua.load("stage='WAIT_NATIVE_READY'").exec().unwrap();
    assert_eq!(
        lua.load(source).eval::<String>().unwrap(),
        "CAPACITY_MAP WAIT_NATIVE_READY"
    );
    lua.load("stage='ARRIVAL_UNOBSERVED'").exec().unwrap();
    assert!(lua.load(source).eval::<String>().is_err());
    for route in ["MAP", "RETURN", "PRODUCE"] {
        lua.globals().set("mode", route).unwrap();
        assert_eq!(lua.load(source).eval::<String>().unwrap(), route);
    }
}

#[test]
fn produced_spy_comparison_uses_native_build_and_a_short_acceptance_barrier() {
    let data = scenario("leader-persona-spy-production-15km.json");
    let steps = steps(&data);
    assert_eq!(steps.len(), 19);
    assert!(!steps.iter().any(|step| action(step, "end_turn")));
    let codes: Vec<_> = steps.iter().filter_map(embedded_lua).collect();
    let index = |needle: &str| {
        codes
            .iter()
            .position(|source| source.contains(needle))
            .unwrap_or_else(|| panic!("missing native spy step: {needle}"))
    };
    assert!(index("Queue('SPY_UNLOCK')") < index("s.civic and s.capacity>#s.spies"));
    assert!(index("Queue('SPY_BUILD')") < index("ACCEPTED_SPY_BUILD"));
    assert!(index("ACCEPTED_SPY_BUILD") < index("Queue('SPY_FINISH')"));
    assert!(index("JAC_BOUNDARY_SPY')~=nil") < index("Queue('SPY_TRAVEL'"));
    assert!(index("Queue('SPY_TRAVEL'") < index("ACCEPTED_SPY_TRAVEL"));
    assert!(
        codes
            .iter()
            .all(|source| !source.contains("Queue('PREPARE')")
                && !source.contains("Queue('EVOLVE')")
                && !source.contains("Queue('ASSIGN')")
                && !source.contains("Queue('BUY')"))
    );
}

#[test]
fn persona_native_eligibility_diagnostic_validates_without_operating_the_game() {
    let data = scenario("leader-persona-native-eligibility-15km.json");
    validate_scenario(&data).unwrap();
    assert_eq!(data["strict_lua_errors"], true);
    let turns: Vec<_> = steps(&data)
        .iter()
        .filter(|step| action(step, "end_turn"))
        .map(|step| step["turns"].as_i64().unwrap())
        .collect();
    assert_eq!(turns, [1]);
    assert!(
        steps(&data)
            .iter()
            .all(|step| !["restart_game", "autoplay"].contains(&step["action"].as_str().unwrap()))
    );
    let lua = Lua::new();
    for step in steps(&data) {
        if let Some(source) = embedded_lua(step) {
            compile_lua(&lua, source);
        }
    }
}

#[test]
fn persona_native_slot_diagnostics_preserve_the_live_failure_gates() {
    let lua = Lua::new();
    for (name, expected_steps) in [
        ("leader-persona-title-proxy-15km.json", 85),
        ("leader-persona-native-slots-15km.json", 44),
        ("leader-persona-native-slots-activation-15km.json", 33),
        ("leader-persona-native-slot-ai-only-15km.json", 12),
    ] {
        let data = scenario(name);
        validate_scenario(&data).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(steps(&data).len(), expected_steps, "{name}");
        assert_eq!(data["strict_lua_errors"], true, "{name}");
        assert!(
            data["steps"][0]["ids"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id == TITLE_PROXY_MOD)
        );
        assert!(steps(&data).iter().all(|step| !action(step, "autoplay")));
        for step in steps(&data) {
            if let Some(source) = embedded_lua(step) {
                compile_lua(&lua, source);
            }
        }
    }
    let title = scenario("leader-persona-title-proxy-15km.json");
    let only = scenario("leader-persona-native-slot-ai-only-15km.json");
    assert!(
        only["steps"][0]["ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == NATIVE_SLOT_ONLY_MOD)
    );
    assert!(code(&steps(&only)[3]).contains("CanEverAppointGovernor(proxy.Hash)"));
    assert!(code(&steps(&only)[3]).contains("not g:CanEverAppointGovernor(slot.Hash)"));
    assert_eq!(code(&steps(&only)[10]), "return JAC_SlotsAssertAI(true)");
    assert!(
        steps(&only)
            .iter()
            .all(|step| !action(step, "save") && !action(step, "load"))
    );
    assert!(
        steps(&only)
            .iter()
            .filter_map(code_if_ui)
            .all(|code| !code.contains("JAC_SlotsAppoint("))
    );
    assert!(
        title["steps"][0]["disable_ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == NATIVE_SLOTS_MOD)
    );
    assert!(
        steps(&title)
            .iter()
            .filter_map(|step| step["code"].as_str())
            .all(|code| !code.contains("SetCulturalProgress"))
    );
    assert!(steps(&title).iter().any(|step| {
        code_if_ui(step).is_some_and(|code| code.contains("JAC_TitleProxyGiveTitles"))
    }));

    for (name, receipt_step) in [
        ("leader-persona-native-slots-15km.json", 18),
        ("leader-persona-native-slots-activation-15km.json", 32),
    ] {
        let data = scenario(name);
        let steps = steps(&data);
        assert!(
            steps[0]["disable_ids"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id == NATIVE_SLOT_ONLY_MOD),
            "{name}"
        );
        assert!(
            data["steps"][0]["ids"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id == NATIVE_SLOTS_MOD)
        );
        assert_eq!(code(&steps[9]), "return JAC_SlotsAssertAI(true)", "{name}");
        assert_eq!(
            code(&steps[receipt_step]),
            "return JAC_SlotsRequireUnassignedReceipt()",
            "{name}"
        );
        assert_eq!(
            steps
                .iter()
                .filter(|step| action(step, "end_turn"))
                .map(|step| step["turns"].as_i64().unwrap())
                .collect::<Vec<_>>(),
            [1, 4, 1],
            "{name}"
        );
    }
    let main = scenario("leader-persona-native-slots-15km.json");
    let main_steps = steps(&main);
    assert!(main_steps[..18].iter().all(|step| !action(step, "save")));
    assert_eq!(
        main_steps
            .iter()
            .filter(|step| action(step, "save"))
            .count(),
        2
    );
    assert_eq!(
        main_steps
            .iter()
            .filter(|step| action(step, "load"))
            .count(),
        2
    );
    for phase in ["UNASSIGNED", "SETTLED"] {
        assert!(main_steps.iter().any(|step| {
            code_if_ui(step)
                .is_some_and(|code| code.contains(&format!("JAC_SlotsCheckReload('{phase}')")))
        }));
    }
    assert!(code(main_steps.last().unwrap()).contains("JAC_SlotsRequireUnassignedReceipt()"));
}

fn code_if_ui(step: &Value) -> Option<&str> {
    action(step, "ui_lua").then(|| code(step))
}

#[test]
fn persona_static_gate_diagnostic_separates_ai_boundary_and_one_official_request() {
    let data = scenario("leader-persona-static-gate-15km.json");
    validate_scenario(&data).unwrap();
    let steps = steps(&data);
    assert_eq!(steps.len(), 17);
    assert_eq!(data["strict_lua_errors"], true);
    assert!(
        steps[0]["ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == STATIC_GATE_MOD)
    );
    for other in [TITLE_PROXY_MOD, NATIVE_SLOTS_MOD, NATIVE_SLOT_ONLY_MOD] {
        assert!(
            steps[0]["disable_ids"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id == other)
        );
    }
    assert_eq!(code(&steps[3]), "return JAC_StaticGate.AssertClosed(false)");
    assert_eq!(code(&steps[8]), "return JAC_StaticGate.AssertAI()");
    assert!(code(&steps[11]).contains("JAC_StaticGate.Ready()"));
    assert_eq!(code(&steps[12]), "return JAC_StaticGate.Queue('APPOINT')");
    assert_eq!(code(&steps[16]), "return JAC_StaticGate.Outcome()");
    assert_eq!(
        steps.iter().filter(|step| action(step, "end_turn")).count(),
        1
    );
    assert_eq!(steps[6]["turns"], 1);
    assert!(steps[6]["timeout"].as_u64().unwrap() >= 240);
    assert!(
        steps
            .iter()
            .all(|step| !action(step, "autoplay") && !action(step, "save"))
    );
    assert_eq!(
        steps
            .iter()
            .filter_map(code_if_ui)
            .filter(|code| code.contains("Queue('APPOINT')"))
            .count(),
        1
    );
    let lua = Lua::new();
    for step in steps {
        if let Some(source) = embedded_lua(step) {
            compile_lua(&lua, source);
        }
    }
}

#[test]
fn persona_native_turn_observations_allow_the_15km_map_to_finish() {
    // Real turns on this map can take about 150 seconds. Native settlement
    // observations require every requested turn, rather than simulated markers.
    for name in [
        "leader-persona-15km.json",
        "leader-persona-native-eligibility-15km.json",
        "leader-persona-city-loss-15km.json",
        "leader-persona-spy-access-15km.json",
        "leader-persona-spy-recovery.json",
        "leader-persona-title-proxy-15km.json",
        "leader-persona-native-slots-15km.json",
        "leader-persona-native-slots-activation-15km.json",
        "leader-persona-native-slot-ai-only-15km.json",
        "leader-persona-static-gate-15km.json",
        "leader-persona-trait-grant-native-control-15km.json",
    ] {
        let data = scenario(name);
        let turns: Vec<_> = steps(&data)
            .iter()
            .filter(|step| action(step, "end_turn"))
            .collect();
        assert!(!turns.is_empty(), "{name}");
        for step in turns {
            let count = step["turns"].as_u64().unwrap();
            let budget = step["timeout"].as_f64().unwrap();
            assert!(
                count > 0 && budget >= 240.0 * count as f64,
                "{name}: {step}"
            );
        }
    }
}

#[test]
fn six_standalone_persona_scenarios_disable_both_diagnostic_mods() {
    for name in [
        "leader-persona-15km.json",
        "leader-persona-baseline-15km.json",
        "leader-persona-native-eligibility-15km.json",
        "leader-persona-city-loss-15km.json",
        "leader-persona-spy-access-15km.json",
        "leader-persona-title-proxy-15km.json",
    ] {
        let data = scenario(name);
        let disabled = data["steps"][0]["disable_ids"].as_array().unwrap();
        let enabled = data["steps"][0]["ids"].as_array().unwrap();
        assert!(disabled.iter().any(|id| id == NATIVE_SLOTS_MOD), "{name}");
        assert!(
            disabled.iter().any(|id| id == NATIVE_SLOT_ONLY_MOD),
            "{name}"
        );
        assert!(!enabled.iter().any(|id| id == NATIVE_SLOTS_MOD), "{name}");
        if name != "leader-persona-title-proxy-15km.json" {
            assert!(disabled.iter().any(|id| id == TITLE_PROXY_MOD), "{name}");
            assert!(!enabled.iter().any(|id| id == TITLE_PROXY_MOD), "{name}");
        }
    }
}

#[test]
fn every_other_new_game_isolates_diagnostic_gates_at_the_shared_selection_boundary() {
    let folder = civ6_autotest_rs::repo_root().join("tools/civ6-autotest/scenarios");
    let mut count = 0;
    let mut blocked_recovery_seen = false;
    let mut hotseat_growth_seen = false;
    let mut banseiki_hotseat_growth_seen = false;
    let mut japan_hotseat_growth_seen = false;
    let mut pool_scenarios_seen = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(folder).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".json")
            || name == "leader-persona-static-gate-15km.json"
            || name == "leader-persona-native-trace-15km.json"
            || name == "leader-persona-property-abi-15km.json"
            || name == "leader-persona-lifecycle-15km.json"
        {
            continue;
        }
        let data = scenario(&name);
        let first = &steps(&data)[0];
        if action(first, "enable_mods") {
            count += 1;
            if matches!(
                name.as_str(),
                "banseiki-pool-bridge.json"
                    | "banseiki-pool-hotseat.json"
                    | "banseiki-pool-random-bridge.json"
                    | "banseiki-pool-single-player.json"
            ) {
                pool_scenarios_seen.insert(name.clone());
            }
            blocked_recovery_seen |= name == "assassin-blocked-recovery.json";
            hotseat_growth_seen |= name == "multiplayer-hotseat-growth.json";
            banseiki_hotseat_growth_seen |= name == "multiplayer-banseiki-hotseat-growth.json";
            japan_hotseat_growth_seen |=
                name == "multiplayer-japan-15km-settlers-hotseat-growth.json";
            let strings = |key: &str| -> Vec<String> {
                first[key]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|id| id.as_str().unwrap().to_owned())
                    .collect()
            };
            let isolated = civ6_autotest_rs::scenario::isolated_disabled_mods(
                &strings("ids"),
                &strings("disable_ids"),
            );
            assert!(isolated.iter().any(|id| id == STATIC_GATE_MOD), "{name}");
            if name == "leader-persona-spy-neutralization-start-15km.json" {
                assert!(
                    first["ids"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|id| id == ENEMY_GATE_MOD)
                );
            } else {
                assert!(isolated.iter().any(|id| id == ENEMY_GATE_MOD), "{name}");
            }
            if matches!(
                name.as_str(),
                "leader-persona-trait-grant-15km.json"
                    | "leader-persona-trait-grant-native-control-15km.json"
            ) {
                assert!(strings("ids").iter().any(|id| id == TRAIT_GRANT_MOD));
            } else {
                assert!(isolated.iter().any(|id| id == TRAIT_GRANT_MOD), "{name}");
            }
            if matches!(
                name.as_str(),
                "leader-persona-property-observer-15km.json"
                    | "leader-persona-property-numeric-observer-15km.json"
                    | "leader-persona-joined-admission-calibration-15km.json"
                    | "leader-persona-joined-admission-cost1-15km.json"
                    | "leader-persona-promotion-call-calibration-15km.json"
                    | "leader-persona-assign-calibration-15km.json"
            ) {
                assert!(
                    strings("ids").iter().any(|id| id == PROPERTY_OBSERVER_MOD),
                    "{name}"
                );
                assert!(
                    !isolated.iter().any(|id| id == PROPERTY_OBSERVER_MOD),
                    "{name}"
                );
            } else {
                assert!(
                    isolated.iter().any(|id| id == PROPERTY_OBSERVER_MOD),
                    "{name}"
                );
            }
            assert!(
                !first["ids"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|id| id == STATIC_GATE_MOD),
                "{name}"
            );
        }
    }
    // Preserve all prior scenes and apply the same isolation assertions to
    // Japan Hotseat and all four pool starts. The witness hold starts no game.
    assert!(blocked_recovery_seen);
    assert!(hotseat_growth_seen);
    assert!(banseiki_hotseat_growth_seen);
    assert_eq!(count, 149);
    assert!(japan_hotseat_growth_seen);
    assert_eq!(pool_scenarios_seen.len(), 4);
}

#[test]
fn property_abi_probe_is_a_single_scoped_read_only_diagnostic() {
    let data = scenario("leader-persona-property-abi-15km.json");
    let s = steps(&data);
    assert_eq!(s.len(), 9);
    let enabled = s[0]["ids"].as_array().unwrap();
    let disabled = s[0]["disable_ids"].as_array().unwrap();
    assert!(enabled.iter().any(|id| id == STATIC_GATE_MOD));
    let ids = |rows: &[Value]| {
        rows.iter()
            .map(|id| id.as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    let isolated =
        civ6_autotest_rs::scenario::isolated_disabled_mods(&ids(enabled), &ids(disabled));
    assert!(isolated.iter().any(|id| id == ENEMY_GATE_MOD));
    assert_eq!(
        s.iter()
            .filter(|step| action(step, "start_native_governor_trace"))
            .count(),
        1
    );
    assert_eq!(
        s.iter()
            .find(|step| action(step, "start_native_governor_trace"))
            .unwrap()["mode"],
        "property"
    );
    assert_eq!(
        s.iter()
            .filter(|step| action(step, "stop_native_governor_trace"))
            .count(),
        1
    );
    assert!(!s.iter().any(|step| action(step, "end_turn")));
    let code = s
        .iter()
        .filter_map(|step| step["code"].as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(code.contains("JAC_ACCESSOR_853_NUMBER"));
    assert!(code.contains("JAC_ACCESSOR_853_TEXT"));
    assert!(!code.contains("ChangeGovernorPoints"));
    assert!(!code.contains("JAC_PERSONA_"));
}

#[test]
fn joined_admission_calibration_separates_owner_returns_and_typed_property_transition() {
    let data = scenario("leader-persona-joined-admission-calibration-15km.json");
    let s = steps(&data);
    assert_eq!(s.len(), 21);
    assert_eq!(s[s.len() - 3]["action"], "stop_native_can_ever_calibration");
    assert_eq!(s[s.len() - 2]["action"], "ui_lua");
    let after = s[s.len() - 2]["code"].as_str().unwrap();
    assert!(after.contains("total==484") && after.contains("GetProperty(key)==expected"));
    let actions = s
        .iter()
        .map(|step| step["action"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        actions
            .iter()
            .filter(|a| **a == "start_native_can_ever_calibration")
            .count(),
        2
    );
    assert_eq!(
        actions
            .iter()
            .filter(|a| **a == "stop_native_can_ever_calibration")
            .count(),
        2
    );
    assert_eq!(
        actions
            .iter()
            .filter(|a| **a == "start_native_property_trace")
            .count(),
        1
    );
    assert_eq!(
        actions
            .iter()
            .filter(|a| **a == "stop_native_property_trace")
            .count(),
        1
    );
    assert!(
        !s.iter()
            .any(|step| step["code"].as_str().is_some_and(|code| code
                .contains("APPOINT_GOVERNOR")
                || code.contains("PROMOTE_GOVERNOR")))
    );
    let property = s
        .iter()
        .find(|step| step["action"] == "start_native_property_trace")
        .unwrap();
    assert_eq!(property["scope"], "ashina_admission_853304");
    assert!(s.iter().any(|step| {
        step["code"]
            .as_str()
            .is_some_and(|code| code.contains("JAC_JoinedAdmissionPrepare"))
    }));
}

#[test]
fn joined_cost_one_scene_uses_one_native_window_and_one_official_appointment() {
    let data = scenario("leader-persona-joined-admission-cost1-15km.json");
    let s = steps(&data);
    assert_eq!(s.len(), 24);
    let actions = s
        .iter()
        .map(|step| step["action"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        actions
            .iter()
            .filter(|action| **action == "start_native_joined_admission")
            .count(),
        1
    );
    assert_eq!(
        actions
            .iter()
            .filter(|action| **action == "stop_native_joined_admission")
            .count(),
        1
    );
    assert_eq!(
        actions
            .iter()
            .filter(|action| **action == "end_turn")
            .count(),
        1
    );
    assert!(s.iter().any(|step| step["code"].as_str().is_some_and(
        |code| code.contains("count==484") && code.contains("CanEverAppointGovernor")
    )));
    assert_eq!(
        s.iter()
            .filter(|step| step["code"]
                .as_str()
                .is_some_and(|code| code.contains("PlayerOperations.APPOINT_GOVERNOR")))
            .count(),
        1
    );
    assert!(s.iter().any(|step| {
        step["code"].as_str().is_some_and(|code| {
            code.contains("GetGovernorPointsSpent()==r.spent+1") && code.contains("people==0")
        })
    }));
}

#[test]
fn no_breakpoint_baseline_reuses_the_failed_property_lua_with_a_bounded_observer() {
    let original = scenario("leader-persona-property-abi-15km.json");
    let diagnostic = scenario("leader-persona-lifecycle-15km.json");
    let source = steps(&original);
    let target = steps(&diagnostic);
    assert_eq!(target.len(), 7);
    assert_eq!(&target[..4], &source[..4]);
    assert_eq!(target[4]["action"], "start_native_governor_trace");
    assert_eq!(target[4]["mode"], "lifecycle");
    assert_eq!(target[4]["max_seconds"], 20);
    assert_eq!(target[5], source[5]);
    assert_eq!(target[6]["action"], "stop_native_governor_trace");
    assert_eq!(
        target
            .iter()
            .filter(|step| action(step, "start_native_governor_trace"))
            .count(),
        1
    );
}

#[test]
fn native_trace_separates_ui_ai_and_refused_official_request_without_changing_qualification() {
    let data = scenario("leader-persona-native-trace-15km.json");
    civ6_autotest_rs::scenario::validate_scenario(&data).unwrap();
    let s = steps(&data);
    let names: Vec<_> = s
        .iter()
        .filter(|step| action(step, "start_native_governor_trace"))
        .map(|step| step["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "eligibility-ui",
            "eligibility-ai",
            "eligibility-official-request"
        ]
    );
    assert_eq!(
        s.iter()
            .filter(|step| action(step, "stop_native_governor_trace"))
            .count(),
        3
    );
    assert_eq!(s.iter().filter(|step| action(step, "end_turn")).count(), 1);
    assert_eq!(
        s.iter()
            .filter(|step| step["code"].as_str() == Some("return JAC_StaticGate.Queue('APPOINT')"))
            .count(),
        1
    );
    let final_code = code(s.last().unwrap());
    assert!(final_code.contains("DIAGNOSTIC ONLY"));
    assert!(final_code.contains("not g:HasGovernor(def.Hash)"));
    assert!(final_code.contains("g:GetGovernorPointsSpent()==saved.spent"));
    assert!(final_code.contains("JAC_STATIC_GATE_BARRIER"));
    assert!(!final_code.contains("SetProperty"));
    assert!(!final_code.contains("RequestPlayerOperation"));
}

#[test]
fn carrier_exchange_rejects_ignored_snapshot_options_without_limiting_checkpoints() {
    let data = json!({
        "name": "carrier exchange snapshot options",
        "steps": [{"action":"carrier_exchange", "token":"txn:carrier:900", "candidate":"test"}]
    });
    validate_scenario(&data).unwrap();
    for key in ["capture_as", "equals_capture"] {
        for value in [json!("STATE"), json!(null)] {
            let mut exchange = data.clone();
            exchange["steps"][0][key] = value;
            let error = validate_scenario(&exchange).unwrap_err();
            assert!(
                error
                    .message
                    .contains(&format!("carrier_exchange does not support {key}"))
            );
        }
    }
    let checkpoint = json!({
        "name": "carrier checkpoints retain supported comparison",
        "steps": [
            {"action":"carrier_checkpoint", "token":"txn:carrier:900", "candidate":"test", "path":"before.json", "capture_as":"STATE"},
            {"action":"carrier_checkpoint", "token":"txn:carrier:900", "candidate":"test", "path":"after.json", "equals_capture":"STATE"}
        ]
    });
    validate_scenario(&checkpoint).unwrap();
}

#[test]
fn ui_lua_comparison_requires_a_recorded_value() {
    let mut data = json!({
        "name": "snapshot comparison",
        "steps": [
            {"action": "ui_lua", "state_name": "InGame", "code": "return 1", "capture": "BEFORE"},
            {"action": "ui_lua", "state_name": "InGame", "code": "return 1", "equals_capture": "BEFORE"}
        ]
    });
    let error = validate_scenario(&data).unwrap_err();
    assert!(error.message.contains("earlier capture_as"));

    data["steps"][0]["capture_as"] = json!("BEFORE");
    validate_scenario(&data).unwrap();
}

#[test]
fn carrier_exchange_rejects_unimplemented_comparisons_but_checkpoint_retains_them() {
    let base = json!({
        "name": "carrier comparison scope",
        "steps": [{"action": "carrier_exchange", "token": "saved_boundary", "candidate": "candidate"}]
    });
    validate_scenario(&base).unwrap();
    for key in ["capture_as", "equals_capture"] {
        for value in [json!("BEFORE"), Value::Null] {
            let mut data = base.clone();
            data["steps"][0][key] = value;
            let error = validate_scenario(&data).unwrap_err();
            assert!(error.message.contains(&format!("does not support {key}")));
        }
    }
    let mut checkpoint = base;
    checkpoint["steps"][0]["action"] = json!("carrier_checkpoint");
    checkpoint["steps"][0]["path"] = json!("carrier/before.json");
    checkpoint["steps"][0]["capture_as"] = json!("BEFORE");
    checkpoint["steps"][0]["equals_capture"] = json!("BEFORE");
    assert!(
        validate_scenario(&checkpoint)
            .unwrap_err()
            .message
            .contains("earlier capture_as")
    );
    checkpoint["steps"][0]
        .as_object_mut()
        .unwrap()
        .remove("equals_capture");
    let mut after = checkpoint["steps"][0].clone();
    after["path"] = json!("carrier/after.json");
    after.as_object_mut().unwrap().remove("capture_as");
    after["equals_capture"] = json!("BEFORE");
    checkpoint["steps"].as_array_mut().unwrap().push(after);
    validate_scenario(&checkpoint).unwrap();
}

#[test]
fn ages_every_chunk_is_followed_by_a_full_previous_era_record() {
    let data = scenario("ages-probe.json");
    let s = steps(&data);
    let chunks: Vec<_> = s
        .iter()
        .enumerate()
        .filter(|(_, step)| action(step, "autoplay"))
        .collect();
    assert!(chunks.len() >= 3);
    for (i, chunk) in chunks {
        assert_eq!(chunk["allow_human_elimination"], true, "step {i}");
        let record = &s[i + 1];
        assert!(action(record, "ui_lua"), "step {i}");
        let c = code(record);
        for needle in [
            "for pid = 0, 21 do",
            "GetPlayerPreviousEraScoreBreakdown(pid)",
            "error('listed '",
        ] {
            assert!(c.contains(needle), "step {i}: {needle}");
        }
        assert!(
            c.contains(&format!("local chunk = {}", chunk["turns"])),
            "step {i}"
        );
        assert!(!c.contains("pcall"), "step {i}");
    }
}

#[test]
fn ages_chunk_length_is_guarded_by_the_minimum_era_length() {
    let data = scenario("ages-probe.json");
    let longest = steps(&data)
        .iter()
        .filter(|s| action(s, "autoplay"))
        .map(|s| s["turns"].as_u64().unwrap())
        .max()
        .unwrap();
    let guard = ui_codes(&data)
        .into_iter()
        .find(|c| c.contains("GameEraMinimumTurns"))
        .unwrap();
    let re = Regex::new(r"GameEraMinimumTurns or 0\) < (\d+)").unwrap();
    assert_eq!(
        re.captures(guard).unwrap()[1].parse::<u64>().unwrap(),
        longest
    );
    assert!(guard.contains("error('wrong ruleset"));
    assert!(guard.contains("error('alive majors"));
}

#[test]
fn ages_api_listing_separates_not_enumerable_from_absent() {
    let data = scenario("ages-probe.json");
    let listing = ui_codes(&data)
        .into_iter()
        .find(|c| c.contains("local function list(label, obj)"))
        .unwrap();
    for needle in [
        "NOT_ENUMERABLE (0 keys)",
        "NOT_ENUMERABLE ERR:",
        "'enumerated ' .. total",
    ] {
        assert!(listing.contains(needle), "{needle}");
    }
}

#[test]
fn ages_changed_since_last_boundary() {
    let data = scenario("ages-probe.json");
    let records: Vec<_> = ui_codes(&data)
        .into_iter()
        .filter(|c| c.contains("changedSinceLast"))
        .collect();
    assert!(records.len() >= 4);
    let re = Regex::new(r"(?m)^local changed = (.+)$").unwrap();
    for c in records {
        let expr = &re.captures(c).unwrap()[1];
        let lua = Lua::new();
        let function: Function = lua
            .load(format!(
                "return function(chunk, eraStart, turn) return {expr} end"
            ))
            .eval()
            .unwrap();
        for (chunk, era_start, turn, want) in [
            (0, 1, 1, false),
            (30, 30, 60, false),
            (30, 31, 60, true),
            (30, 60, 60, true),
            (30, 1, 60, false),
        ] {
            assert_eq!(
                function.call::<bool>((chunk, era_start, turn)).unwrap(),
                want,
                "{expr}"
            );
        }
    }
}

#[test]
fn ages_moments_cover_all_players() {
    let data = scenario("ages-probe.json");
    let last = steps(&data).last().unwrap();
    assert!(action(last, "ui_lua"));
    assert!(code(last).contains("for pid = 0, 21 do"));
    assert!(code(last).contains("GetAllMomentsData(pid, 0)"));
}

fn era_probe(speed: &str) -> Value {
    scenario(&format!("probe-era-timing-{speed}.json"))
}

#[test]
fn era_every_single_turn_is_logged_in_both_contexts() {
    for speed in ["standard", "quick"] {
        let data = era_probe(speed);
        let s = steps(&data);
        let chunks: Vec<_> = s
            .iter()
            .enumerate()
            .filter(|(_, step)| action(step, "autoplay"))
            .collect();
        assert!(chunks.len() >= 15, "{speed}");
        for (i, chunk) in chunks {
            assert_eq!(chunk["turns"], 1, "{speed} {i}");
            assert_eq!(s[i + 1]["state_name"], "GameCore_Tuner");
            assert_eq!(s[i + 2]["state_name"], "InGame");
            for next in [&s[i + 1], &s[i + 2]] {
                assert!(code(next).contains("JAC_ERP.Log("), "{speed} {i}");
            }
        }
    }
}

#[test]
fn era_speeds_differ_only_in_speed() {
    let standard = era_probe("standard");
    let quick = era_probe("quick");
    assert!(
        steps(&standard)[0]["ids"]
            .as_array()
            .unwrap()
            .contains(&json!(PROBE_MOD))
    );
    assert_eq!(steps(&standard)[1]["speed"], "GAMESPEED_STANDARD");
    assert_eq!(steps(&quick)[1]["speed"], "GAMESPEED_QUICK");
    let normalized: Value = serde_json::from_str(
        &quick
            .to_string()
            .replace("GAMESPEED_QUICK", "GAMESPEED_STANDARD"),
    )
    .unwrap();
    assert_eq!(normalized["steps"], standard["steps"]);
}

#[test]
fn era_setup_guard_matches_the_test_mod_sql() {
    let sql =
        std::fs::read_to_string(root().join("poc/JAC_Test_EraTiming/Data/JAC_Test_EraTiming.sql"))
            .unwrap();
    let re = Regex::new(
        r"GameEraMinimumTurns = (\d+), GameEraMaximumTurns = (\d+) WHERE EraType = '(\w+)'",
    )
    .unwrap();
    let expected: std::collections::BTreeMap<_, _> = re
        .captures_iter(&sql)
        .map(|cap| (cap[3].to_string(), format!("{}/{}", &cap[1], &cap[2])))
        .collect();
    let data = era_probe("standard");
    let guard = ui_codes(&data)
        .into_iter()
        .find(|c| c.contains("local want="))
        .unwrap();
    let re = Regex::new(r"(ERA_\w+)='(\d+/\d+)'").unwrap();
    let actual: std::collections::BTreeMap<_, _> = re
        .captures_iter(guard)
        .map(|cap| (cap[1].to_string(), cap[2].to_string()))
        .collect();
    assert_eq!(actual, expected);
    assert!(sql.contains("('NEXT_ERA_TURN_COUNTDOWN', '1')"));
}

#[test]
fn era_other_scenarios_on_the_test_map_disable_the_probe_mod() {
    let dir = root().join("tools/civ6-autotest/scenarios");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        let name = path.file_name().unwrap().to_str().unwrap();
        if name == "sengoku-era-reigns-beyond-nine.json" {
            let data: Value = serde_json::from_str(&text).unwrap();
            assert!(
                data["steps"][0]["ids"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(PROBE_MOD))
            );
            assert!(code(&data["steps"][2]).contains("SetValue('JAC_ERA_TIMING_TAIL', true)"));
        } else if text.contains("JAC_Unification_Test.Civ6Map")
            && !name.starts_with("probe-era-timing")
        {
            let data: Value = serde_json::from_str(&text).unwrap();
            assert!(
                data["steps"][0]["disable_ids"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(PROBE_MOD)),
                "{name}"
            );
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
fn era_gamecore_logger_never_raises_and_reports_changes() {
    let lua = Lua::new();
    lua.load(include_str!("fixtures/era_probe_stub.lua"))
        .exec()
        .unwrap();
    let data = era_probe("standard");
    let gamecore = steps(&data)
        .iter()
        .filter(|s| s["state_name"] == "GameCore_Tuner")
        .collect::<Vec<_>>();
    let first: String = lua.load(code(gamecore[0])).eval().unwrap();
    for needle in [
        "[JAC_ERA_PROBE] ctx=GameCore tag=setup turn=1 elapsed=1",
        "era=0:ERA_ANCIENT",
        "countdown=ERR(",
        "minEnd=absent",
        "date=ERR(",
        "p0:era=0:ERA_ANCIENT/techs=1/civics=ERR(",
    ] {
        assert!(first.contains(needle), "{needle}: {first}");
    }
    assert!(!first.contains("CHANGED"));
    lua.load("state.turn=4; state.era=1").exec().unwrap();
    let after_code = gamecore
        .iter()
        .find(|s| code(s).contains("after3"))
        .unwrap();
    let after: String = lua.load(code(after_code)).eval().unwrap();
    assert!(after.contains("elapsed=4"));
    assert!(after.contains("CHANGED from=ERA_ANCIENT eraLenObserved=3"));
    let printed: mlua::Table = lua.globals().get("printed").unwrap();
    let values: Vec<String> = printed
        .sequence_values()
        .collect::<mlua::Result<_>>()
        .unwrap();
    assert_eq!(values, [first, after]);
}

#[test]
fn era_checked_turns_are_the_generated_era_starts() {
    // The committed generated SQL is the output of the legacy calendar generator.
    let generated =
        std::fs::read_to_string(root().join("poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Eras.sql"))
            .unwrap();
    let re = Regex::new(r"UPDATE Eras_XP1 SET GameEraMinimumTurns=(\d+), GameEraMaximumTurns=(\d+) WHERE EraType='([^']+)'").unwrap();
    let first_lengths: Vec<i64> = re
        .captures_iter(&generated)
        .take(3)
        .map(|cap| {
            assert_eq!(&cap[1], &cap[2]);
            cap[1].parse().unwrap()
        })
        .collect();
    assert_eq!(first_lengths, [65, 7, 37]);
    let (koji, eiroku, genki) = (
        1 + first_lengths[0],
        1 + first_lengths[0] + first_lengths[1],
        1 + first_lengths[0] + first_lengths[1] + first_lengths[2],
    );
    assert_eq!((koji, eiroku, genki), (66, 73, 110));
    let data = scenario("sengoku-era-reigns.json");
    let s = steps(&data);
    assert_eq!(s[1]["speed"], "GAMESPEED_STANDARD");
    assert!(
        s[1]["map_file"]
            .as_str()
            .unwrap()
            .contains("JAC_Japan_15km.Civ6Map")
    );
    assert!(
        s[0]["disable_ids"]
            .as_array()
            .unwrap()
            .contains(&json!(PROBE_MOD))
    );
    for (turn, era, ends) in [(1, 0, koji), (66, 1, eiroku), (73, 2, genki)] {
        assert!(
            s.iter()
                .filter_map(|step| step["code"].as_str())
                .any(|c| c.contains(&format!("elapsed ~= {turn} "))
                    && c.contains(&format!("expected {ends}')"))
                    && c.contains(&format!("~= {era} then"))),
            "turn {turn}"
        );
    }
    for (turn, index) in [(koji, 1), (eiroku, 2)] {
        assert!(
            s.iter()
                .filter_map(|step| step["code"].as_str())
                .any(|c| c.contains(&format!("l.history.r{index}"))
                    && c.contains(&format!("r.turn ~= {turn}"))),
            "turn {turn}"
        );
    }
    let turns = 1 + s
        .iter()
        .filter(|step| action(step, "autoplay"))
        .map(|step| step["turns"].as_u64().unwrap())
        .sum::<u64>();
    assert_eq!(turns, 75);
}

#[test]
fn oath_scenario_is_valid_starts_as_oda_and_checks_load_before_advancing() {
    let data = scenario("sengoku-oaths-oda.json");
    validate_scenario(&data).unwrap();
    let s = steps(&data);
    let config = s.iter().find(|step| action(step, "configure")).unwrap();
    assert_eq!(config["ruleset"], "RULESET_JAC_BANSEIKI");
    let players = config["players"].as_array().unwrap();
    assert_eq!(players[0]["civ"], "CIVILIZATION_JAC_ODA");
    assert_eq!(players[0]["human"], true);
    assert_eq!(
        players
            .iter()
            .filter(|player| player["human"] == true)
            .count(),
        1
    );
    assert_eq!(players.len(), 22);
    let load = s.iter().position(|step| action(step, "load")).unwrap();
    assert!(action(&s[load + 1], "ui_lua"));
    assert_eq!(s[load + 1]["state_name"], "GameCore_Tuner");
    for needle in [
        "snapshot~=Game:GetProperty('JAC_OATH_TEST_SNAPSHOT')",
        "city:GetYield(culture)",
        "on>off",
    ] {
        assert!(code(&s[load + 1]).contains(needle), "{needle}");
    }
}

#[test]
fn oath_every_embedded_lua_step_and_wait_compiles() {
    let data = scenario("sengoku-oaths-oda.json");
    let lua = Lua::new();
    let mut checked = 0;
    for step in steps(&data) {
        if let Some(source) = embedded_lua(step) {
            compile_lua(&lua, source);
            checked += 1;
        }
    }
    assert!(checked >= 12, "compiled {checked} chunks");
}

#[test]
fn oath_automatic_popup_assertion_cannot_force_a_missing_screen_open() {
    let data = scenario("sengoku-oaths-oda.json");
    let popup = steps(&data)
        .iter()
        .find(|step| step["state_name"] == "DedicationPopup")
        .unwrap();
    let lua = Lua::new();
    lua.load("ContextPtr={IsHidden=function() return true end}; JAC_ShowOaths=function() error('forced popup') end")
        .exec().unwrap();
    lua_error(lua.load(code(popup)).exec(), "did not automatically open");
}

#[test]
fn oath_fast_forward_marks_engine_jump_and_keeps_source_intact() {
    let source = json!({"name":"test","steps":[{"action":"autoplay","turns":64,"timeout":14400}]});
    let result = fast_forward_scenario(&source);
    assert_eq!(source["steps"][0]["turns"], 64);
    assert_eq!(
        result["steps"],
        json!([{"action":"autoplay","turns":64,"timeout":14400,"fast_forward":true}])
    );
    assert_eq!(
        result["test_mode"],
        json!({"skipped_turns":0,"continuous_performance_valid":false})
    );
}

#[test]
fn japan_every_scale_scenario_is_valid() {
    for km in [20, 15] {
        let data = scenario(&format!("japan-map-{km}km.json"));
        validate_scenario(&data).unwrap();
        let s = steps(&data);
        let generated = configure_code(&s[1]);
        assert!(generated.contains("MapConfiguration.SetValue('MAP_SCRIPT',"));
        assert!(generated.find("MAP_SCRIPT").unwrap() < generated.find("SetMapSize").unwrap());
        assert!(generated.contains("SetMaxMajorPlayers(22)"));
        assert!(generated.contains("SetValue('CITY_STATE_COUNT', 0)"));
        assert_eq!(s[5]["capture_as"], "JAPAN_STATE");
        assert_eq!(s[9]["equals_capture"], "JAPAN_STATE");
        let first = code(&s[3]);
        for needle in [
            "city count",
            "if totalCities~=56 then",
            "unowned land plot",
            "settler in slot",
            "UnitManager.GetReachableMovement(unit)",
            "UnitManager.CanStartOperation(unit,UnitOperationTypes.MOVE_TO,nil,params)",
            "not diplomacy:IsAtWarWith(owner)",
            "not diplomacy:HasOpenBordersFrom(owner)",
            "humanCapital:GetCulturalIdentity():GetLoyaltyPerTurn()",
            "negative initial city loyalty/turn",
            "Events.CulturalIdentityCityConverted.Add(JAC_JAPAN_LOYALTY_WATCH.handler)",
        ] {
            assert!(first.contains(needle), "{km}km: {needle}");
        }
        let after = code(&s[5]);
        for needle in [
            "freeCities=freeCities+m:GetCities():GetCount()",
            "Free Cities own ",
            "#JAC_JAPAN_LOYALTY_WATCH.majorDefections~=0",
            "negative city loyalty/turn",
            "city counts (start->now)=",
            "initialCounts=",
            "city:GetBuildings():HasBuilding(palaceType)",
            "identity:GetLoyalty()",
            "unitPlots[i..':'..plot]",
        ] {
            assert!(after.contains(needle), "{km}km: {needle}");
        }
        let reloaded = code(&s[9]);
        assert!(reloaded.contains("freeCities=freeCities+m:GetCities():GetCount()"));
        assert!(reloaded.contains("table.sort(cityRows)"));
    }
}

#[test]
fn japan_takeda_variant_checks_selected_faction_and_full_initial_territory() {
    for km in [20, 15] {
        let data = scenario(&format!("japan-map-{km}km-takeda.json"));
        validate_scenario(&data).unwrap();
        let s = steps(&data);
        assert_eq!(s.len(), 4);
        let players = s[1]["players"].as_array().unwrap();
        assert_eq!(
            players[0],
            json!({"leader":"LEADER_JAC_TAKEDA_NOBUTORA","civ":"CIVILIZATION_JAC_TAKEDA","human":true})
        );
        assert_eq!(
            players
                .iter()
                .filter(|player| player["human"] == true)
                .count(),
            1
        );
        for needle in [
            "local localPlayer=Game.GetLocalPlayer()",
            "local leader mismatch",
            "gameSlotForMapId[source]",
            "capital mismatch",
            "city position/owner mismatch",
            "initial territory owners mismatch",
            "capital garrison missing",
        ] {
            assert!(code(&s[3]).contains(needle), "{km}km: {needle}");
        }
    }
}

#[test]
fn japan_resource_scenario_matches_the_manifest_on_the_adopted_15km_map() {
    let data = scenario("japan-resources.json");
    validate_scenario(&data).unwrap();
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(root().join("poc/JAC_PoC_JapanMapScale/Maps/manifest.json")).unwrap(),
    )
    .unwrap();
    let item = manifest
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["km_per_tile"] == 15)
        .unwrap();
    let s = steps(&data);
    assert!(
        s[1]["map_file"]
            .as_str()
            .unwrap()
            .contains(item["file"].as_str().unwrap())
    );
    assert_eq!(s[1]["build_setup"], true);
    assert!(!data.to_string().contains("JAC_RESOURCES_TEST_YEAR"));
    let checks: Vec<_> = s
        .iter()
        .filter(|step| action(step, "ui_lua") && step["state_name"] == "InGame")
        .map(code)
        .collect();
    let candidates = item["resource_candidates"].as_array().unwrap();
    let deferred: Vec<_> = candidates
        .iter()
        .filter(|record| record["start_status"] == "deferred")
        .collect();
    assert_eq!(deferred.len(), 9);
    assert!(
        deferred
            .iter()
            .all(|record| record["placement_status"] == "reserved_for_later_release")
    );
    for c in &checks {
        for record in &deferred {
            let expected = format!(
                "{{'{}',{},'{}',{},'{}'}}",
                record["id"].as_str().unwrap(),
                record["plot"],
                record["resource_id"].as_str().unwrap(),
                record["reveal_year"],
                record["reveal_tech"].as_str().unwrap_or("")
            );
            assert!(c.contains(&expected), "{expected}");
        }
    }
    for record in candidates
        .iter()
        .filter(|record| record["placement_status"] == "placed")
    {
        let expected = format!(
            "{{'{}',{},'{}'}}",
            record["id"].as_str().unwrap(),
            record["plot"],
            record["resource_id"].as_str().unwrap()
        );
        assert!(checks[0].contains(&expected), "{expected}");
    }
    assert!(checks[0].contains("error('deferred site missing: '"));
    assert!(checks[1].contains("if year~=1534 then"));
    assert!(checks[2].contains("if year~=1542 then"));
    assert!(checks[2].contains("Calendar.MakeYearStr(turn)"));
    let auto: Vec<_> = s
        .iter()
        .filter(|step| action(step, "autoplay"))
        .map(|step| step["turns"].as_u64().unwrap())
        .collect();
    assert_eq!(auto, [24]);
}

#[test]
fn japan_rejects_bad_map_file_and_metrics_flag() {
    let mut data = scenario("japan-map-20km.json");
    data["steps"][1]["map_file"] = json!("../outside.Civ6Map");
    expect_error(validate_scenario(&data), "map_file");
    data["steps"][1]["map_file"] =
        json!("{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_20km.Civ6Map");
    data["steps"][4]["metrics"] = json!("yes");
    expect_error(validate_scenario(&data), "metrics");
}

#[test]
fn verification_run_rejects_invalid_metadata_even_without_a_fast_forward_step() {
    for step in [
        json!({"action":"restart_game"}),
        json!({"action":"pause","seconds":0.1}),
    ] {
        let data = json!({"name":"invalid entry","test_mode":{},"steps":[step]});
        expect_error(validate_scenario(&data), "test_mode needs");
    }
}

#[test]
fn verification_invalid_test_mode_is_rejected_before_any_game_call() {
    let malformed = [
        Value::Null,
        json!({}),
        json!({"skipped_turns":-1,"continuous_performance_valid":false}),
        json!({"skipped_turns":true,"continuous_performance_valid":false}),
        json!({"skipped_turns":"0","continuous_performance_valid":false}),
        json!({"skipped_turns":0,"continuous_performance_valid":true}),
    ];
    for mode in malformed {
        let data = json!({"name":"invalid","test_mode":mode,"steps":[{"action":"autoplay","turns":10,"fast_forward":true}]});
        expect_error(validate_scenario(&data), "test_mode needs");
    }
}

#[test]
fn verification_scenario_localization_keys_exist_in_mod_text() {
    let mut defined = std::collections::BTreeSet::new();
    for package in std::fs::read_dir(root().join("poc")).unwrap() {
        let text = package.unwrap().path().join("Text");
        if !text.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(text).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|s| s.to_str()) != Some("xml") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            let doc = roxmltree::Document::parse(&source)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            for row in doc.descendants().filter(|n| n.has_tag_name("Row")) {
                if let Some(tag) = row.attribute("Tag") {
                    defined.insert(tag.to_string());
                }
            }
        }
    }
    assert!(!defined.is_empty());
    let re = Regex::new(r#"['"](LOC_JAC_[A-Z0-9_]*[A-Z0-9])['"]"#).unwrap();
    for name in NEW_SCENARIOS.into_iter().chain(["sengoku-era-reigns.json"]) {
        let data = scenario(name);
        let text = steps(&data)
            .iter()
            .map(|step| {
                format!(
                    "{}{}",
                    step["code"].as_str().unwrap_or(""),
                    step["condition"]["code"].as_str().unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let missing: std::collections::BTreeSet<_> = re
            .captures_iter(&text)
            .map(|cap| cap[1].to_string())
            .filter(|key| !defined.contains(key))
            .collect();
        assert!(missing.is_empty(), "{name}: {missing:?}");
    }
}

#[test]
fn verification_fast_forward_keeps_checkpoints_without_claiming_continuous_metrics() {
    let original = scenario("japan-15km-300turn-full-metrics.json");
    let short = fast_forward_scenario(&original);
    validate_scenario(&short).unwrap();
    assert_eq!(
        short["test_mode"],
        json!({"skipped_turns":0,"continuous_performance_valid":false})
    );
    let auto_turns = |data: &Value| -> Vec<u64> {
        steps(data)
            .iter()
            .filter(|step| action(step, "autoplay"))
            .map(|step| step["turns"].as_u64().unwrap())
            .collect()
    };
    assert_eq!(auto_turns(&short), [50, 50, 50, 50, 50, 50, 1]);
    assert_eq!(auto_turns(&original), [50, 50, 50, 50, 50, 50, 1]);
    assert!(
        steps(&short)
            .iter()
            .filter(|step| action(step, "autoplay") && step["turns"].as_u64().unwrap() > 2)
            .all(|step| step["fast_forward"] == true)
    );
    assert!(
        steps(&original)
            .iter()
            .all(|step| step["fast_forward"] != true)
    );
    assert!(
        steps(&short)
            .iter()
            .filter(|step| action(step, "autoplay"))
            .all(|step| step["metrics_group"]
                .as_str()
                .unwrap()
                .starts_with("TEST_MODE_"))
    );
    let checkpoints = |data: &Value| -> Vec<Value> {
        steps(data)
            .iter()
            .filter(|step| action(step, "save") || action(step, "load"))
            .cloned()
            .collect()
    };
    assert_eq!(checkpoints(&short), checkpoints(&original));
}

#[test]
fn verification_all_new_chunks_validate_and_compile_as_lua() {
    let lua = Lua::new();
    for name in NEW_SCENARIOS {
        let data = scenario(name);
        validate_scenario(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(data["strict_lua_errors"], true, "{name}");
        let mut checked = 0;
        for step in steps(&data) {
            if let Some(source) = embedded_lua(step) {
                compile_lua(&lua, source);
                checked += 1;
            }
        }
        assert!(checked > 0, "{name}");
    }
}

#[test]
fn verification_full_map_measurements_cover_continuous_play_and_both_reloads() {
    let base = scenario("japan-15km-200turn-full.json");
    for (turns, check_turns) in [
        (20, vec![1, 21]),
        (300, vec![1, 51, 101, 151, 201, 251, 301]),
    ] {
        let data = scenario(&format!("japan-15km-{turns}turn-full-metrics.json"));
        let s = steps(&data);
        assert_eq!(s[0]["ids"], base["steps"][0]["ids"]);
        let players = s[1]["players"].as_array().unwrap();
        assert_eq!(players.len(), 22);
        assert_eq!(
            players
                .iter()
                .enumerate()
                .filter(|(_, p)| p["human"] == true)
                .map(|(i, _)| i)
                .collect::<Vec<_>>(),
            [0]
        );
        assert_eq!(players[0]["civ"], "CIVILIZATION_JAC_RYUKYU");
        assert!(
            s[1]["map_file"]
                .as_str()
                .unwrap()
                .contains("JAC_Japan_15km.Civ6Map")
        );
        let autos: Vec<_> = s.iter().filter(|step| action(step, "autoplay")).collect();
        assert_eq!(
            autos[..autos.len() - 1]
                .iter()
                .map(|step| step["turns"].as_u64().unwrap())
                .sum::<u64>(),
            turns
        );
        assert_eq!(autos.last().unwrap()["turns"], 1);
        assert!(autos.iter().all(|step| step["metrics"] == true));
        assert_eq!(
            autos
                .iter()
                .map(|step| step["metrics_group"].as_str().unwrap())
                .collect::<Vec<_>>(),
            std::iter::repeat_n("CONTINUOUS", autos.len() - 1)
                .chain(["POST_LOAD"])
                .collect::<Vec<_>>()
        );
        assert!(autos.iter().all(
            |step| step["research_metrics"] != true && step["allow_human_elimination"] != true
        ));
        let saves: Vec<_> = s.iter().filter(|step| action(step, "save")).collect();
        assert_eq!(
            saves
                .iter()
                .map(|step| step["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            check_turns
                .iter()
                .map(|t| format!("JAC-Perf-T{t}"))
                .collect::<Vec<_>>()
        );
        assert!(saves.iter().all(|step| step["metrics"] == true));
        let loads: Vec<_> = s
            .iter()
            .enumerate()
            .filter(|(_, step)| action(step, "load"))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(
            loads
                .iter()
                .map(|&i| s[i]["name"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["JAC-Perf-T1", &format!("JAC-Perf-T{}", turns + 1)]
        );
        assert!(loads.iter().all(|&i| s[i]["metrics"] == true));
        for &i in &loads {
            assert!(s[i + 2].get("equals_capture").is_some());
            assert!(code(&s[i + 2]).contains("resourceBaseline"));
            assert!(code(&s[i + 2]).contains("JAC_AGES_LEDGER"));
        }
        let first_auto = s.iter().position(|step| action(step, "autoplay")).unwrap();
        let last_save = s.iter().rposition(|step| action(step, "save")).unwrap();
        assert!(loads[0] < first_auto);
        assert!(loads[1] > last_save);
    }
}

#[test]
fn verification_saved_state_snapshot_uses_indexed_city_iterator_and_detects_changes() {
    let data = scenario("japan-15km-20turn-full-metrics.json");
    let source = code(
        steps(&data)
            .iter()
            .find(|step| step["capture_as"] == "PERF_INITIAL_STATE")
            .unwrap(),
    );
    let lua = Lua::new();
    lua.load(include_str!("fixtures/snapshot_stub.lua"))
        .exec()
        .unwrap();
    let run = || lua.load(source).eval::<String>();
    let before = run().unwrap();
    assert_eq!(before, run().unwrap());
    assert!(before.contains("population\"=number:3"));
    assert!(before.contains("\"count\"=number:2"));
    let state: mlua::Table = lua.globals().get("state").unwrap();
    state.set("population", 4).unwrap();
    assert_ne!(before, run().unwrap());
    state.set("population", 3).unwrap();
    assert_eq!(before, run().unwrap());
    for (key, changed) in [("unitX", 7), ("damage", 10)] {
        let original: i64 = state.get(key).unwrap();
        state.set(key, changed).unwrap();
        assert_ne!(before, run().unwrap(), "{key}");
        state.set(key, original).unwrap();
        assert_eq!(before, run().unwrap(), "{key}");
    }
    let warlord: mlua::Table = state.get("warlord").unwrap();
    warlord.set("XP", 40).unwrap();
    assert_ne!(before, run().unwrap());
    warlord.set("XP", 20).unwrap();
    state.set("unitPresent", false).unwrap();
    lua_error(run(), "human growth record missing");
    state.set("unitPresent", true).unwrap();
    assert_eq!(before, run().unwrap());
    state.set("initialized", false).unwrap();
    lua_error(run(), "growth initialization missing");
    state.set("initialized", true).unwrap();
    state.set("warlord", mlua::Nil).unwrap();
    lua_error(run(), "human growth record missing");
    state.set("warlord", warlord).unwrap();
    state.set("owner", 1).unwrap();
    assert_ne!(before, run().unwrap());
}

#[test]
fn verification_boundary_checks_accept_verified_state_and_reject_wrong_turn_era_and_ledger() {
    let kinds = [
        "ERA_ANCIENT",
        "ERA_CLASSICAL",
        "ERA_MEDIEVAL",
        "ERA_RENAISSANCE",
        "ERA_INDUSTRIAL",
        "ERA_MODERN",
        "ERA_ATOMIC",
        "ERA_INFORMATION",
        "ERA_FUTURE",
        "ERA_JAC_SHOHO",
        "ERA_JAC_KEIAN",
    ];
    type BoundaryCase<'a> = (&'a str, Vec<(i64, i64)>, Vec<(i64, i64)>);
    let cases: [BoundaryCase<'_>; 3] = [
        (
            "sengoku-era-reigns-epic.json",
            vec![(0, 1), (1, 98), (2, 108), (3, 163)],
            vec![(98, 97), (108, 10), (163, 55)],
        ),
        (
            "sengoku-era-reigns-marathon.json",
            vec![(0, 1), (1, 196), (2, 217), (3, 328)],
            vec![(196, 195), (217, 21), (328, 111)],
        ),
        (
            "sengoku-era-reigns-beyond-nine.json",
            vec![(8, 17), (9, 19), (10, 21)],
            vec![(19, 2), (21, 2)],
        ),
    ];
    let re = Regex::new(r"local T,n,kind=(\d+),(\d+),'([^']+)'").unwrap();
    let mut checked = 0;
    for (name, starts, lengths) in cases {
        let data = scenario(name);
        for step in steps(&data) {
            let Some(capture) = re.captures(step["code"].as_str().unwrap_or("")) else {
                continue;
            };
            let turn: i64 = capture[1].parse().unwrap();
            let reign: i64 = capture[2].parse().unwrap();
            let kind = &capture[3];
            let lua = Lua::new();
            lua.load(include_str!("fixtures/boundary_stub.lua"))
                .exec()
                .unwrap();
            let globals = lua.globals();
            let info: mlua::Table = globals.get("GameInfo").unwrap();
            let eras: mlua::Table = info.get("Eras").unwrap();
            for (index, era) in kinds.iter().enumerate() {
                let row = lua.create_table().unwrap();
                row.set("Index", index).unwrap();
                eras.set(*era, row).unwrap();
            }
            let state: mlua::Table = globals.get("state").unwrap();
            state.set("turn", turn).unwrap();
            state
                .set("era", kinds.iter().position(|era| *era == kind).unwrap())
                .unwrap();
            state
                .set(
                    "start",
                    starts.iter().find(|(index, _)| *index == reign).unwrap().1,
                )
                .unwrap();
            let ledger: mlua::Table = globals.get("ledger").unwrap();
            ledger.set("reign", reign).unwrap();
            if let Some((_, length)) = lengths.iter().find(|(t, _)| *t == turn) {
                let history: mlua::Table = ledger.get("history").unwrap();
                let row = lua.create_table().unwrap();
                row.set("turn", turn).unwrap();
                row.set("due", turn).unwrap();
                row.set("length", *length).unwrap();
                row.set("from", reign - 1).unwrap();
                row.set("p0", lua.create_table().unwrap()).unwrap();
                row.set("engineEra", -1).unwrap();
                history.set(format!("r{reign}"), row).unwrap();
            }
            let c = code(step);
            assert!(lua.load(c).eval::<bool>().unwrap(), "{name} turn {turn}");
            state.set("turn", turn + 1).unwrap();
            assert!(
                lua.load(c).eval::<bool>().is_err(),
                "{name} turn {turn}: wrong turn accepted"
            );
            state.set("turn", turn).unwrap();
            state.set("era", -1).unwrap();
            assert!(
                lua.load(c).eval::<bool>().is_err(),
                "{name} turn {turn}: wrong era accepted"
            );
            state
                .set("era", kinds.iter().position(|era| *era == kind).unwrap())
                .unwrap();
            if lengths.iter().any(|(t, _)| *t == turn) {
                let history: mlua::Table = ledger.get("history").unwrap();
                let row: mlua::Table = history.get(format!("r{reign}")).unwrap();
                row.set("due", turn + 1).unwrap();
                assert!(
                    lua.load(c).eval::<bool>().is_err(),
                    "{name} turn {turn}: wrong ledger accepted"
                );
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 34);
}

fn xml_child_text<'a, 'input>(node: roxmltree::Node<'a, 'input>, child: &str) -> &'a str {
    node.children()
        .find(|n| n.has_tag_name(child))
        .and_then(|n| n.text())
        .unwrap()
}

#[test]
fn verification_optional_tail_is_scoped_hidden_and_preserves_endless_last_era() {
    let mod_dir = root().join("poc/JAC_Test_EraTiming");
    let meta = std::fs::read_to_string(mod_dir.join("JAC_Test_EraTiming.modinfo")).unwrap();
    let doc = roxmltree::Document::parse(&meta).unwrap();
    let criteria = doc
        .descendants()
        .find(|n| {
            n.has_tag_name("Criteria") && n.attribute("id") == Some("JAC_Test_EraTiming_Tail")
        })
        .unwrap();
    assert_eq!(
        xml_child_text(criteria, "RuleSetInUse"),
        "RULESET_JAC_BANSEIKI"
    );
    let matches: std::collections::BTreeMap<_, _> = criteria
        .children()
        .filter(|n| n.has_tag_name("ConfigurationValueMatches"))
        .map(|n| {
            (
                (
                    xml_child_text(n, "Group"),
                    xml_child_text(n, "ConfigurationId"),
                ),
                xml_child_text(n, "Value"),
            )
        })
        .collect();
    assert_eq!(
        matches,
        [
            (
                ("Map", "MAP_SCRIPT"),
                "{5c1e7a3d-9b2f-4e61-8d47-2a6f0c9e1b85}Maps/JAC_Unification_Test.Civ6Map"
            ),
            (("Game", "JAC_ERA_TIMING_TAIL"), "1")
        ]
        .into_iter()
        .collect()
    );
    let config =
        std::fs::read_to_string(mod_dir.join("Config/JAC_Test_EraTiming_Config.xml")).unwrap();
    let doc = roxmltree::Document::parse(&config).unwrap();
    let parameter = doc
        .descendants()
        .find(|n| n.has_tag_name("Parameters"))
        .unwrap()
        .children()
        .find(|n| n.has_tag_name("Row"))
        .unwrap();
    assert_eq!(
        (
            parameter.attribute("Visible"),
            parameter.attribute("DefaultValue"),
            parameter.attribute("Domain")
        ),
        (Some("0"), Some("0"), Some("bool"))
    );
    for speed in ["standard", "quick"] {
        assert!(
            code(&steps(&era_probe(speed))[2]).contains("SetValue('JAC_ERA_TIMING_TAIL', false)")
        );
    }
    let generated =
        std::fs::read_to_string(root().join("poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Eras.sql"))
            .unwrap();
    let re = Regex::new(r"UPDATE Eras_XP1 SET GameEraMinimumTurns=(\d+), GameEraMaximumTurns=(\d+) WHERE EraType='([^']+)'").unwrap();
    let rows: Vec<_> = re
        .captures_iter(&generated)
        .map(|cap| {
            (
                cap[3].to_string(),
                cap[1].parse::<i64>().unwrap(),
                cap[2].parse::<i64>().unwrap(),
            )
        })
        .collect();
    assert_eq!(rows.len(), 47);
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE Eras(EraType TEXT PRIMARY KEY,ChronologyIndex INTEGER);
        CREATE TABLE Eras_XP1(EraType TEXT PRIMARY KEY,GameEraMinimumTurns INTEGER,GameEraMaximumTurns INTEGER);
        CREATE TABLE GlobalParameters(Name TEXT PRIMARY KEY,Value TEXT);").unwrap();
    for (i, (name, min, max)) in rows.iter().enumerate() {
        db.execute(
            "INSERT INTO Eras VALUES (?1,?2)",
            rusqlite::params![name, i + 1],
        )
        .unwrap();
        db.execute(
            "INSERT INTO Eras_XP1 VALUES (?1,?2,?3)",
            rusqlite::params![name, min, max],
        )
        .unwrap();
    }
    db.execute("INSERT INTO Eras VALUES ('ERA_JAC_CHIDAI',48)", [])
        .unwrap();
    db.execute(
        "INSERT INTO Eras_XP1 VALUES ('ERA_JAC_CHIDAI',NULL,NULL)",
        [],
    )
    .unwrap();
    db.execute_batch(
        &std::fs::read_to_string(mod_dir.join("Data/JAC_Test_EraTiming_Tail.sql")).unwrap(),
    )
    .unwrap();
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM Eras_XP1 WHERE GameEraMinimumTurns=2 AND GameEraMaximumTurns=2",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 47);
    let last: (Option<i64>, Option<i64>) = db.query_row("SELECT GameEraMinimumTurns,GameEraMaximumTurns FROM Eras_XP1 WHERE EraType='ERA_JAC_CHIDAI'", [], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    assert_eq!(last, (None, None));
    let version: String = db
        .query_row(
            "SELECT Value FROM GlobalParameters WHERE Name='JAC_TEST_ERA_TAIL_VERSION'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(version, "1");
}

#[test]
fn verification_resource_calendar_calls_actual_getter_and_rejects_wrong_year() {
    let data = scenario("resources-calendar-epic.json");
    let checks: Vec<_> = steps(&data)
        .iter()
        .filter(|step| {
            action(step, "ui_lua") && code(step).contains("local actual=api.CurrentYear()")
        })
        .collect();
    assert_eq!(checks.len(), 6);
    let re = Regex::new(r"if turn~=(\d+)").unwrap();
    for step in checks {
        let source = code(step);
        let turn: i64 = re.captures(source).unwrap()[1].parse().unwrap();
        let year = match turn {
            1 => 1534,
            25 => 1540,
            41 => 1543,
            _ => panic!("unexpected turn {turn}"),
        };
        let lua = Lua::new();
        lua.load(include_str!("fixtures/calendar_stub.lua"))
            .exec()
            .unwrap();
        let state: mlua::Table = lua.globals().get("state").unwrap();
        state.set("turn", turn).unwrap();
        state.set("year", year).unwrap();
        let result: String = lua.load(source).eval().unwrap();
        assert!(result.contains(&format!("turn={turn} year={year} rows=99")));
        state.set("year", year + 1).unwrap();
        assert!(lua.load(source).eval::<String>().is_err());
        if step["state_name"] == "GameCore_Tuner" {
            assert!(!source.contains("Calendar."));
        }
    }
}

#[test]
fn verification_strict_judge_flag_validation() {
    expect_error(
        validate_scenario(&json!({"name":"bad flag","strict_lua_errors":1,
            "steps":[{"action":"autoplay","turns":1}]})),
        "strict_lua_errors",
    );
}

#[test]
fn driver_end_turn_rejects_invalid_counts() {
    for turns in [json!(0), json!(-1), json!(true), json!(1.5), json!("2")] {
        expect_error(
            validate_scenario(
                &json!({"name":"turn count","steps":[{"action":"end_turn","turns":turns}]}),
            ),
            "turns",
        );
        expect_error(
            validate_scenario(
                &json!({"name":"turn count","steps":[{"action":"end_turn","test_turns":turns}]}),
            ),
            "test_turns",
        );
    }
}

#[test]
fn driver_build_setup_checks_every_player_after_refresh() {
    let players = vec![
        json!({"leader":"LEADER_JAC_ODA_NOBUHIDE","civ":"CIVILIZATION_JAC_ODA","human":true}),
        json!({"leader":"LEADER_JAC_TAKEDA_NOBUTORA","civ":"CIVILIZATION_JAC_TAKEDA","human":false}),
    ];
    let generated = build_setup_code(&json!({"players":players}));
    assert!(
        generated.find("BuildHeadlessGameSetup()").unwrap()
            < generated.find("g_GameParameters:FullRefresh()").unwrap()
    );
    assert!(
        generated.find("setup changed player 0").unwrap()
            < generated.find("setup changed player 1").unwrap()
    );
    assert!(
        generated.find("setup changed player 1").unwrap()
            < generated.find("HideGameSetup()").unwrap()
    );
    assert!(generated.contains("GetSlotStatus() ~= SlotStatus.SS_TAKEN"));
    assert!(generated.contains("GetSlotStatus() ~= SlotStatus.SS_COMPUTER"));
    compile_lua(&Lua::new(), &generated);
}

#[test]
fn driver_build_setup_defaults_on_for_banseiki_only() {
    use civ6_autotest_rs::scenario::builds_setup;
    let base = json!({"action":"configure","map_size":"M","speed":"S","seed":1,
        "players":[{"leader":"L","civ":"C","human":true}]});
    for (ruleset, explicit, expected) in [
        ("RULESET_JAC_BANSEIKI", None, true),
        ("RULESET_JAC_BANSEIKI", Some(false), false),
        ("RULESET_STANDARD", None, false),
        ("RULESET_STANDARD", Some(true), true),
    ] {
        let mut step = base.clone();
        step["ruleset"] = json!(ruleset);
        if let Some(value) = explicit {
            step["build_setup"] = json!(value);
        }
        assert_eq!(builds_setup(&step), expected, "{ruleset}, {explicit:?}");
    }
}

#[test]
fn setup_refresh_restores_requested_counts_and_rejects_ignored_settings() {
    let mut step = json!({"players":[
        {"leader":"LEADER_JAC_ODA_NOBUHIDE","civ":"CIVILIZATION_JAC_ODA","human":true},
        {"leader":"LEADER_JAC_TAKEDA_NOBUTORA","civ":"CIVILIZATION_JAC_TAKEDA","human":true}
    ],"city_states":0});
    for (keep_players, keep_cities, cities_requested) in [
        (false, false, true),
        (true, false, true),
        (false, true, true),
        (false, false, false),
    ] {
        if cities_requested {
            step["city_states"] = json!(0);
        } else {
            step.as_object_mut().unwrap().remove("city_states");
        }
        let lua = Lua::new();
        lua.globals().set("KEEP_PLAYERS", keep_players).unwrap();
        lua.globals().set("KEEP_CITIES", keep_cities).unwrap();
        lua.load(r#"
            count, cities, maximum, hidden = 4, 0, 2, 2
            SlotStatus = {SS_TAKEN=3, SS_COMPUTER=1}
            GameConfiguration = {
                GetHiddenPlayerCount=function() return hidden end,
                GetParticipatingPlayerCount=function() return count end,
                SetParticipatingPlayerCount=function(v) if not KEEP_PLAYERS then count=v end end,
                SetValue=function(k,v) assert(k=='CITY_STATE_COUNT'); if not KEEP_CITIES then cities=v end end,
                GetValue=function(k) assert(k=='CITY_STATE_COUNT'); return cities end
            }
            MapConfiguration = {
                SetMaxMajorPlayers=function(v) maximum=v end,
                GetMaxMajorPlayers=function() return maximum end
            }
            g_GameParameters = {FullRefresh=function()
                count, cities, maximum = 6 + hidden, 12, 10
            end}
            function BuildHeadlessGameSetup() end
            function HideGameSetup() end
            PlayerConfigurations = {}
            for i,n in ipairs({'ODA_NOBUHIDE','TAKEDA_NOBUTORA'}) do
                local name, civ = n, (i==1 and 'ODA' or 'TAKEDA')
                PlayerConfigurations[i-1] = {
                    GetLeaderTypeName=function() return 'LEADER_JAC_'..name end,
                    GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_'..civ end,
                    GetSlotStatus=function() return SlotStatus.SS_TAKEN end
                }
            end
        "#).exec().unwrap();
        let result = lua.load(build_setup_code(&step)).exec();
        if keep_players || keep_cities {
            let error = result.unwrap_err().to_string();
            assert!(
                error.contains(if keep_players {
                    "participating player count"
                } else {
                    "city-state count"
                }),
                "{error}"
            );
        } else {
            result.unwrap();
            assert_eq!(lua.globals().get::<i64>("count").unwrap(), 4);
            // Keep the map's supported limit. Replacing it with the requested
            // count would make MapSize_ValueNeedsChanging trigger another reset.
            assert_eq!(lua.globals().get::<i64>("maximum").unwrap(), 10);
            assert_eq!(
                lua.globals().get::<i64>("cities").unwrap(),
                if cities_requested { 0 } else { 12 }
            );
        }
    }
}

#[test]
fn driver_flag_validation() {
    let mut data = scenario("sengoku-trees.json");
    data["steps"][1]["build_setup"] = json!("yes");
    expect_error(validate_scenario(&data), "build_setup");
    for key in ["research_metrics", "allow_human_elimination"] {
        let mut step = json!({"action":"autoplay","turns":1});
        step[key] = json!("yes");
        expect_error(
            validate_scenario(&json!({"name":"flag","steps":[step]})),
            key,
        );
    }
    expect_error(
        validate_scenario(
            &json!({"name":"flag","steps":[{"action":"save","name":"x","exact_name":1}]}),
        ),
        "exact_name",
    );
}

#[test]
fn driver_new_scenarios_check_requested_states() {
    let screen = scenario("japan-map-setup-screen.json");
    assert!(code(&steps(&screen)[4]).contains("OnSetMapByValue(file)"));
    assert!(code(&steps(&screen)[4]).contains("#values~=22"));
    assert!(code(&steps(&screen)[2]).contains("#values~=2"));
    let settler = scenario("japan-map-15km-settlers.json");
    assert!(
        steps(&settler)[1]["map_file"]
            .as_str()
            .unwrap()
            .contains("_Settlers.Civ6Map")
    );
    assert!(code(&steps(&settler)[3]).contains("p:GetCities():GetCount()~=0"));
    assert!(code(&steps(&settler)[3]).contains("count~=1"));
    let long = scenario("japan-map-15km-200turn.json");
    let s = steps(&long);
    let auto = s.iter().position(|step| action(step, "autoplay")).unwrap();
    assert_eq!(s[auto]["turns"], 200);
    assert_eq!(s[auto]["allow_human_elimination"], true);
    assert_eq!(s[auto]["research_metrics"], true);
    assert!(
        s[0]["ids"]
            .as_array()
            .unwrap()
            .contains(&json!("26c967d0-c10d-4eeb-8aaf-c83dc7e621e7"))
    );
    assert_eq!(s[1]["build_setup"], true);
    assert_eq!(s[1]["ruleset"], "RULESET_JAC_BANSEIKI");
    let start = s
        .iter()
        .position(|step| action(step, "start_game"))
        .unwrap();
    let check = s[start + 1..auto]
        .iter()
        .filter_map(|step| step["code"].as_str())
        .find(|source| source.contains("RULESET_JAC_BANSEIKI"))
        .unwrap();
    for needle in [
        "GetRuleSet()~='RULESET_JAC_BANSEIKI'",
        "TECH_JAC_SENGOKU_",
        "CIVIC_JAC_SENGOKU_",
        "t~=32 or c~=30",
    ] {
        assert!(check.contains(needle), "{needle}");
    }
}
