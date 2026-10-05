use civ6_autotest_rs::persona_payment::{self, Evidence, ProcessIdentity, Receipt};
use serde_json::{json, Value};

fn fixture(cost: u8) -> (Vec<Value>, Evidence) {
    let owned = (100..100 + u32::from(cost)).collect::<Vec<_>>();
    let receipt = Receipt {
        owner: 2,
        governor: 12,
        cost,
        base: 100,
        titles: (101..100 + u32::from(cost)).collect(),
        count: 500,
        owned: owned.clone(),
        paid: cost,
        before_spent: 7,
        spent: 7 + u32::from(cost),
        city: 65536,
        x: 134,
        y: 29,
        can_assign: true,
    };
    let row = |seq, rva| {
        json!({"kind":"assign_calibration","sequence":seq,"thread_id":8,"read_flags":7,
        "player_id":2,"governor_index":12,"governors_this":"0x0000000000001000","governor_pointer":"0x0000000000002000",
        "city_pointer":"0x0000000000003000","player_address":"0x0000000000004000","original_rax":"0x0000000000000001",
        "original_al":1,"effective_al":1,"promotion_snapshot_status":0,"promotion_owner":2,"promotion_index":12,
        "promotion_count":500,"owned_promotion_indices":owned,"payment_authority":false,
        "city_a8":65536,"city_c0":134,"city_c4":29,"city_d8":2,"return_site_rva":rva})
    };
    (
        vec![
            json!({"kind":"identity","mode":"assign-calibration","entry_rva":0x2dd880,"pid":10,"created_filetime":123,
        "gamecore_sha256":civ6_native_bridge_rs::PINNED_GAMECORE_SHA256}),
            json!({"kind":"install","status":0}),
            json!({"kind":"uninstall","status":0,"entry_restored":true,"prefix_retained_until_exit":false}),
            row(1, 0xba3e2),
            row(2, 0x2dd375),
            json!({"kind":"stop","classification":"assign_calibration","result":"ok",
        "rows":2,"observed_calls":2,"dropped":0,"in_flight":0,"snapshot_stable":true,"quiescence_proven":false,
        "counter_scope":"snapshot_observation_not_complete_call_history"}),
        ],
        Evidence {
            receipt,
            process: ProcessIdentity {
                pid: 10,
                created_filetime: 123,
            },
            artifact: "test.jsonl".into(),
            artifact_path: "D:/test/test.jsonl".into(),
        },
    )
}
fn trace(rows: &[Value]) -> String {
    rows.iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn all_three_costs_require_same_persons_native_ordered_flags() {
    for cost in 1..=3 {
        let (rows, r) = fixture(cost);
        let result = persona_payment::verify(&trace(&rows), &r).unwrap();
        assert_eq!(
            (
                result.cost,
                result.paid,
                result.matching_rows,
                result.ui_calls,
                result.core_update_calls
            ),
            (cost, cost, 2, 1, 1)
        );
        assert!(!result.payment_authority);
    }
}
#[test]
fn aggregate_spending_cannot_hide_an_unpaid_person() {
    let (mut rows, r) = fixture(3);
    rows[3]["owned_promotion_indices"] = json!([100, 101]);
    rows[4]["owned_promotion_indices"] = json!([100, 101]);
    assert!(persona_payment::verify(&trace(&rows), &r).is_err());
    let (mut rows, r) = fixture(3);
    rows[3]["owned_promotion_indices"] = json!([100, 102]);
    let mut r = r;
    r.receipt.owned = vec![100, 102];
    rows[4]["owned_promotion_indices"] = json!([100, 102]);
    assert!(persona_payment::verify(&trace(&rows), &r).is_err());
}
#[test]
fn malformed_or_failed_rows_are_not_replaced_by_last_good_sample() {
    for (field, value) in [
        ("promotion_snapshot_status", json!(5)),
        ("promotion_owner", json!(3)),
        ("promotion_index", json!(13)),
        ("promotion_count", json!(4097)),
        ("owned_promotion_indices", json!([100, 100])),
        ("read_flags", json!(3)),
        ("city_d8", json!(3)),
        ("effective_al", json!(0)),
        ("governor_pointer", json!("0x0000000000000000")),
        ("sequence", json!(2)),
        ("payment_authority", json!(true)),
    ] {
        let (mut rows, r) = fixture(1);
        rows[3][field] = value;
        assert!(
            persona_payment::verify(&trace(&rows), &r).is_err(),
            "{field}"
        );
    }
}
#[test]
fn complete_lossless_envelope_and_both_native_callers_required() {
    for (field, value) in [
        ("rows", json!(1)),
        ("observed_calls", json!(3)),
        ("dropped", json!(1)),
        ("result", json!("error")),
        ("in_flight", json!(1)),
        ("snapshot_stable", json!(false)),
        ("quiescence_proven", json!(true)),
    ] {
        let (mut rows, r) = fixture(1);
        rows[5][field] = value;
        assert!(
            persona_payment::verify(&trace(&rows), &r).is_err(),
            "{field}"
        );
    }
    let (mut rows, r) = fixture(1);
    rows[4]["return_site_rva"] = json!(0xba3e2);
    assert!(persona_payment::verify(&trace(&rows), &r).is_err());
    let (mut rows, r) = fixture(1);
    rows.pop();
    assert!(persona_payment::verify(&trace(&rows), &r).is_err());
    let (mut rows, r) = fixture(1);
    rows.push(json!({"kind":"install","status":0}));
    assert!(persona_payment::verify(&trace(&rows), &r).is_err());
}
fn completed_steps(r: &Evidence) -> Vec<Value> {
    vec![
        json!({"action":"25/32 start_native_assign_calibration","status":"OK", "detail":format!("native assign-calibration original-return telemetry ready; artifact test.jsonl; process={}",serde_json::to_string(&r.process).unwrap())}),
        json!({"action":"26/32 ui_lua","status":"OK","detail":format!("ran Lua in InGame; return={}{}; native_process={}",persona_payment::MARKER,serde_json::to_string(&r.receipt).unwrap(),serde_json::to_string(&r.process).unwrap())}),
        json!({"action":"30/32 stop_native_assign_calibration","status":"OK","detail":format!("native entry restored; artifact {}",r.artifact_path.display())}),
        json!({"action":"32/32 exit_to_menu","status":"OK","detail":"main menu reached"}),
        json!({"action":"judge","status":"OK","detail":"6/6 checks passed"}),
    ]
}
#[test]
fn receipt_requires_a_complete_same_process_observation() {
    let (_, r) = fixture(1);
    let steps = completed_steps(&r);
    assert_eq!(
        persona_payment::evidence_from_steps(&trace(&steps))
            .unwrap()
            .receipt
            .paid,
        1
    );
    for i in 0..steps.len() {
        let mut missing = steps.clone();
        missing.remove(i);
        assert!(
            persona_payment::evidence_from_steps(&trace(&missing)).is_err(),
            "missing {i}"
        );
    }
    let mut duplicate = steps.clone();
    duplicate.insert(2, steps[1].clone());
    assert!(persona_payment::evidence_from_steps(&trace(&duplicate)).is_err());
    let mut swapped = steps.clone();
    swapped.swap(0, 1);
    assert!(persona_payment::evidence_from_steps(&trace(&swapped)).is_err());
    let mut failed = steps;
    failed[4]["status"] = json!("FAIL");
    assert!(persona_payment::evidence_from_steps(&trace(&failed)).is_err());
    let mut different_ui = completed_steps(&r);
    different_ui[1]["detail"] = json!(different_ui[1]["detail"]
        .as_str()
        .unwrap()
        .replace("\"created_filetime\":123", "\"created_filetime\":124"));
    assert!(persona_payment::evidence_from_steps(&trace(&different_ui)).is_err());
}
#[test]
fn sentinel_failure_cannot_hide_between_healthy_ui_and_core_rows() {
    let (mut rows, r) = fixture(1);
    let mut bad = rows[3].clone();
    bad["sequence"] = json!(3);
    bad["player_id"] = json!(u32::MAX);
    bad["governor_index"] = json!(u32::MAX);
    bad["read_flags"] = json!(0);
    bad["promotion_snapshot_status"] = json!(7);
    rows.insert(5, bad);
    rows[6]["rows"] = json!(3);
    rows[6]["observed_calls"] = json!(3);
    assert!(persona_payment::verify(&trace(&rows), &r).is_err());
}
#[test]
fn trace_from_another_process_or_reused_pid_is_rejected() {
    for (field, value) in [("pid", json!(11)), ("created_filetime", json!(124))] {
        let (mut rows, r) = fixture(1);
        rows[0][field] = value;
        assert!(
            persona_payment::verify(&trace(&rows), &r).is_err(),
            "{field}"
        );
    }
}
#[test]
fn generated_ui_read_uses_real_definitions_and_does_not_mutate_game() {
    let lua = mlua::Lua::new();
    lua.load(r#"
        local promos={{Index=100,Hash=200},{Index=101,Hash=201},{Index=102,Hash=202}}
        GameInfo={JAC_LeaderPersonas={LEADER_JAC_ASHINA_MORIKIYO={GovernorType='ASHINA',BasePromotionType='BASE',EvolutionCost=1}},
            Governors={ASHINA={Index=12,Hash=15}},GovernorPromotions=setmetatable({BASE=promos[1]},
            {__call=function() local n=0;return function() n=n+1;return promos[n] end end})}
        local city={GetID=function()return 65536 end,GetX=function()return 134 end,GetY=function()return 29 end}
        local gov={GetAssignedCity=function()return city end,HasPromotion=function(_,hash)return hash==200 end}
        local g={GetGovernor=function(_,hash)assert(hash==15);return gov end,
            GetGovernorPointsSpent=function()return 1 end,CanAssignGovernor=function(_,hash,c)assert(hash==15 and c==city);CALLS=CALLS+1;return true end}
        Players={[0]={GetGovernors=function()return g end,GetCities=function()return {
            Members=function()local once=false;return function()if not once then once=true;return 1,city end end end} end}}
        Game={GetProperty=function(_,key)assert(key=='JAC_TEST853_ADMISSION_RECEIPT');return {owner=0,leader='LEADER_JAC_ASHINA_MORIKIYO',spent=0} end}
        CALLS=0
    "#).exec().unwrap();
    let raw: String = lua.load(persona_payment::READ_COST_ONE).eval().unwrap();
    let receipt: Receipt =
        serde_json::from_str(raw.strip_prefix(persona_payment::MARKER).unwrap()).unwrap();
    assert_eq!(
        (
            receipt.owner,
            receipt.governor,
            receipt.base,
            receipt.count,
            receipt.spent
        ),
        (0, 12, 100, 103, 1)
    );
    assert_eq!(receipt.owned, vec![100]);
    assert_eq!(lua.globals().get::<u32>("CALLS").unwrap(), 1);
}

#[test]
fn cli_generation_keeps_every_original_step_and_check_rejects_mixed_directories() {
    let temp = tempfile::tempdir().unwrap();
    let bin = env!("CARGO_BIN_EXE_civ6-persona-payment-check");
    let generated = temp.path().join("scene.json");
    assert!(std::process::Command::new(bin)
        .arg("generate")
        .arg(&generated)
        .status()
        .unwrap()
        .success());
    let mut scene: Value = serde_json::from_slice(&std::fs::read(&generated).unwrap()).unwrap();
    civ6_autotest_rs::scenario::validate_scenario(&scene).unwrap();
    let steps = scene["steps"].as_array_mut().unwrap();
    let start = steps
        .iter()
        .position(|s| s["action"] == "start_native_assign_calibration")
        .unwrap();
    assert_eq!(steps[start + 1]["code"], persona_payment::READ_COST_ONE);
    steps.remove(start + 1);
    let original: Value = serde_json::from_slice(
        &std::fs::read(
            civ6_autotest_rs::repo_root()
                .join("tools/civ6-autotest/scenarios/leader-persona-assign-calibration-15km.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(*steps, original["steps"].as_array().unwrap().clone());
    let (rows, mut r) = fixture(1);
    let native = temp.path().join("test.jsonl");
    r.artifact_path = native.clone();
    let step_path = temp.path().join("steps.jsonl");
    std::fs::write(&native, trace(&rows)).unwrap();
    std::fs::write(&step_path, trace(&completed_steps(&r))).unwrap();
    let invoke = |steps: &std::path::Path| {
        std::process::Command::new(bin)
            .arg("check")
            .arg(&native)
            .arg(steps)
            .output()
            .unwrap()
    };
    let ok = invoke(&step_path);
    assert!(
        ok.status.success(),
        "{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    let result: Value = serde_json::from_slice(&ok.stdout).unwrap();
    assert_eq!(result["payment_authority"], false);
    let other = temp.path().join("other");
    std::fs::create_dir(&other).unwrap();
    let mixed = other.join("steps.jsonl");
    std::fs::copy(&step_path, &mixed).unwrap();
    assert!(!invoke(&mixed).status.success());
    let other_trace = other.join("test.jsonl");
    std::fs::copy(&native, &other_trace).unwrap();
    r.artifact_path = other_trace;
    std::fs::write(&step_path, trace(&completed_steps(&r))).unwrap();
    assert!(
        !invoke(&step_path).status.success(),
        "different restoration directory accepted"
    );
}

#[test]
fn runner_pins_ui_reads_before_and_after_and_rejects_switched_processes() {
    use civ6_autotest_rs::{
        runner::{Backend, Runner},
        time::SystemClock,
        Result,
    };
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };
    struct Reader {
        identities: VecDeque<Option<(u32, u64)>>,
        calls: Arc<Mutex<usize>>,
        output: String,
    }
    impl Backend for Reader {
        fn validate_native_bridge_target(&mut self) -> Result<Option<(u32, u64)>> {
            Ok(self.identities.pop_front().unwrap())
        }
        fn require_single_process(&mut self) -> Result<()> {
            Ok(())
        }
        fn list_states(&mut self, _: f64) -> Result<Vec<String>> {
            Ok(vec!["InGame".into()])
        }
        fn run_lua(&mut self, _: &str, _: &str, _: f64) -> Result<String> {
            *self.calls.lock().unwrap() += 1;
            Ok(self.output.clone())
        }
    }
    let (_, receipt) = fixture(1);
    let output = format!(
        "{}{}",
        persona_payment::MARKER,
        serde_json::to_string(&receipt.receipt).unwrap()
    );
    for (after, expected) in [
        (Some((10, 123)), true),
        (Some((11, 123)), false),
        (Some((10, 124)), false),
        (None, false),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let calls = Arc::new(Mutex::new(0));
        let backend = Reader {
            identities: VecDeque::from([Some((10, 123)), after]),
            calls: calls.clone(),
            output: output.clone(),
        };
        let mut runner = Runner::with_backend(
            json!({"name":"current UI process","steps":[]}),
            temp.path().into(),
            temp.path().into(),
            temp.path().into(),
            4318,
            Box::new(backend),
            Arc::new(SystemClock),
        );
        let result = runner.run_step(
            &json!({"action":"ui_lua","state_name":"InGame","code":"return 'independent read'"}),
        );
        assert_eq!(result.is_ok(), expected);
        assert_eq!(*calls.lock().unwrap(), 1);
        if expected {
            assert!(result.unwrap().contains("; native_process={"));
        }
    }
}
