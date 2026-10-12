use civ6_reports_rs::persona_unjoined::verify_directory as verify_committed;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const CONTRACT: &str = include_str!("../src/persona_unjoined_fixture.json");
const PHYSICAL: &[u8] =
    b"synthetic saved bytes for checker mutation tests, not native save evidence";

fn verify_directory(
    dir: &std::path::Path,
) -> civ6_reports_rs::Result<civ6_reports_rs::persona_unjoined::Report> {
    verify_committed(
        dir,
        PHYSICAL.len() as u64,
        &format!("{:x}", Sha256::digest(PHYSICAL)),
    )
}

fn fixture() -> (tempfile::TempDir, std::path::PathBuf, Vec<Value>, Value) {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("normal-ai-01");
    std::fs::create_dir(&dir).unwrap();
    let scenario: Value = serde_json::from_str(CONTRACT).unwrap();
    let mut steps = vec![json!({"action":"restart_game","timeout":420})];
    steps.extend(scenario["steps"].as_array().unwrap().iter().cloned());
    for step in &mut steps {
        if matches!(step["action"].as_str(), Some("save" | "load")) {
            step["name"] = json!(format!("{}-normal-ai-01", step["name"].as_str().unwrap()));
        }
    }
    let snapshot = |turn, total| {
        format!(
            "turn={turn}|ai=21|persons=22|negative=462|titles={}",
            (1..=21)
                .map(|id| format!("{id}:{total}:0"))
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let save = "JAC-ToDo407-Unjoined-AI-normal-ai-01-012345abcdef";
    let mut rows = vec![json!({"action":"start","status":"OK","detail":scenario["name"]})];
    for (index, step) in steps.iter().enumerate() {
        let action = format!("{}/22 {}", index + 1, step["action"].as_str().unwrap());
        rows.push(json!({"action":action,"status":"START","detail":step.to_string()}));
        let detail = match step["action"].as_str().unwrap() {
            "ui_lua" => {
                let value = match index {
                    5 => snapshot(1, 0),
                    7 => snapshot(1, 1),
                    8 => "actual turn baseline=1".into(),
                    10 | 19 => "one actual turn observed".into(),
                    11 | 16 => snapshot(2, 1),
                    17 => "restored actual turn baseline=2".into(),
                    20 => snapshot(3, 1),
                    _ => "all 21 AI can spend a title before native turns".into(),
                };
                format!(
                    "ran Lua in {}; return={value}",
                    step["state_name"].as_str().unwrap()
                )
            }
            "save" => format!("C:\\Saves\\{save}.Civ6Save"),
            "load" => format!("loaded {save}"),
            _ => "operation completed".into(),
        };
        rows.push(json!({"action":action,"status":"OK","detail":detail}));
    }
    rows.push(json!({"action":"judge","status":"OK","detail":"6/6 checks passed"}));
    let names = [
        "Generic: JAC Modding.log errors",
        "Generic: JAC Database.log errors",
        "Generic: [JAC_POC] errors",
        "Generic: JAC Runtime Error",
        "Generic: JAC missing texture",
        "Scenario: all Lua errors",
    ];
    let result = json!({"scenario":scenario["name"],"completed_steps":22,"passed":true,"failure":null,"test_mode":null,"checks":names.map(|name|json!({"name":name,"passed":true}))});
    write(&dir, &rows, &result);
    for name in ["Lua.log", "Modding.log", "Database.log"] {
        std::fs::write(dir.join(name), "").unwrap();
    }
    let physical = b"synthetic saved bytes for checker mutation tests, not native save evidence";
    std::fs::write(dir.join(format!("{save}.Civ6Save")), physical).unwrap();
    std::fs::write(dir.join("save-proof.json"), json!({"name":format!("{save}.Civ6Save"),"bytes":physical.len(),"sha256":format!("{:x}",Sha256::digest(physical))}).to_string()).unwrap();
    (temp, dir, rows, result)
}

fn write(dir: &std::path::Path, rows: &[Value], result: &Value) {
    std::fs::write(
        dir.join("steps.jsonl"),
        rows.iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    std::fs::write(dir.join("result.json"), result.to_string()).unwrap();
}

#[test]
fn complete_normal_rejection_has_two_actual_turns_and_one_native_restore() {
    let (_temp, dir, _, _) = fixture();
    let report = verify_directory(&dir).unwrap();
    assert!(report.normal_unjoined_rejection && !report.product_acceptance);
    assert_eq!(
        (
            report.original_steps,
            report.recorded_steps,
            report.actual_turns,
            report.matched_save_load_snapshots
        ),
        (21, 22, 2, 1)
    );
    assert_eq!(report.turns, [1, 2, 3]);
    assert_eq!(
        (
            report.ai_owners,
            report.persona_definitions,
            report.denied_pairs_per_audit,
            report.native_audits
        ),
        (21, 22, 462, 5)
    );
}

#[test]
fn forged_success_cannot_hide_altered_lua_native_coverage_or_turn_jumps() {
    for mutation in 0..4 {
        let (_temp, dir, mut rows, result) = fixture();
        match mutation {
            0 => {
                let mut step: Value =
                    serde_json::from_str(rows[11]["detail"].as_str().unwrap()).unwrap();
                step["code"] = json!("return 'no unjoined governors'");
                rows[11]["detail"] = json!(step.to_string());
            }
            1 => {
                let value = rows[12]["detail"]
                    .as_str()
                    .unwrap()
                    .replace("negative=462", "negative=461");
                rows[12]["detail"] = json!(value);
            }
            2 => {
                let value = rows[42]["detail"]
                    .as_str()
                    .unwrap()
                    .replace("turn=3", "turn=8");
                rows[42]["detail"] = json!(value);
            }
            3 => rows[22]["status"] = json!("FAIL"),
            _ => unreachable!(),
        }
        write(&dir, &rows, &result);
        assert!(
            verify_directory(&dir).is_err(),
            "accepted mutation {mutation}"
        );
    }
}

#[test]
fn owner_budget_native_restore_and_physical_save_must_be_the_same_run() {
    for mutation in 0..5 {
        let (_temp, dir, mut rows, result) = fixture();
        match mutation {
            0 => {
                let value = rows[34]["detail"]
                    .as_str()
                    .unwrap()
                    .replace("21:1:0", "21:2:0");
                rows[34]["detail"] = json!(value);
            }
            1 => {
                let value = rows[16]["detail"]
                    .as_str()
                    .unwrap()
                    .replace("21:1:0", "21:0:0");
                rows[16]["detail"] = json!(value);
            }
            2 => {
                rows[30]["detail"] = json!("loaded JAC-ToDo407-Unjoined-AI-another-01-012345abcdef")
            }
            3 => {
                let value = rows[12]["detail"]
                    .as_str()
                    .unwrap()
                    .replace("21:0:0", "20:0:0");
                rows[12]["detail"] = json!(value);
            }
            4 => {
                for row in &mut rows {
                    if let Some(detail) = row["detail"].as_str() {
                        if row["status"] == "OK"
                            && detail.contains("|titles=")
                            && detail.contains("21:1:0")
                        {
                            row["detail"] = json!(detail.replace("21:1:0", "21:2:1"));
                        }
                    }
                }
            }
            _ => unreachable!(),
        }
        write(&dir, &rows, &result);
        assert!(
            verify_directory(&dir).is_err(),
            "accepted mutation {mutation}"
        );
    }
}

#[test]
fn missing_or_changed_physical_save_cannot_use_native_summary_success() {
    for mutation in 0..6 {
        let (_temp, dir, _, _) = fixture();
        let file = dir.join("JAC-ToDo407-Unjoined-AI-normal-ai-01-012345abcdef.Civ6Save");
        match mutation {
            0 => std::fs::remove_file(file).unwrap(),
            1 => std::fs::write(file, b"changed").unwrap(),
            2 => std::fs::remove_file(dir.join("save-proof.json")).unwrap(),
            3 => {
                let changed = b"coordinated replacement";
                std::fs::write(&file, changed).unwrap();
                std::fs::write(dir.join("save-proof.json"),json!({"name":file.file_name().unwrap().to_str().unwrap(),"bytes":changed.len(),"sha256":format!("{:x}",Sha256::digest(changed))}).to_string()).unwrap();
            }
            4 => {
                let proof = std::fs::read_to_string(dir.join("save-proof.json")).unwrap();
                let mut proof: Value = serde_json::from_str(&proof).unwrap();
                proof["extra"] = json!(true);
                std::fs::write(dir.join("save-proof.json"), proof.to_string()).unwrap();
            }
            5 => std::fs::write(dir.join("save-proof.json"), "[]").unwrap(),
            _ => unreachable!(),
        }
        assert!(
            verify_directory(&dir).is_err(),
            "accepted physical mutation {mutation}"
        );
    }
}

#[test]
fn raw_errors_missing_slices_or_clock_skips_cannot_inherit_summary_success() {
    let (_temp, dir, rows, mut result) = fixture();
    std::fs::write(
        dir.join("Lua.log"),
        "GovernorPanel: Runtime Error: nil value",
    )
    .unwrap();
    assert!(verify_directory(&dir).is_err());
    std::fs::write(dir.join("Lua.log"), "").unwrap();
    result["test_mode"] = json!({"skipped_turns":0});
    write(&dir, &rows, &result);
    assert!(verify_directory(&dir).is_err());
    result["test_mode"] = Value::Null;
    result["checks"].as_array_mut().unwrap().pop();
    write(&dir, &rows, &result);
    assert!(verify_directory(&dir).is_err());
    let (_temp, dir, _, _) = fixture();
    std::fs::remove_file(dir.join("Database.log")).unwrap();
    assert!(verify_directory(&dir).is_err());
}
