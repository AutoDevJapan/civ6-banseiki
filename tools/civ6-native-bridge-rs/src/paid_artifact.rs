//! Experimental capture validation. This proves capture consistency only;
//! it never declares the product's real-game acceptance criteria satisfied.
use serde_json::Value;
pub const KEY_RANGES: [[u32; 2]; 9] = [
    [900000, 900255],
    [901000, 901255],
    [902000, 902063],
    [903000, 903255],
    [904000, 904255],
    [904999, 904999],
    [905000, 905999],
    [906000, 906523],
    [907000, 907063],
];

fn hex(value: &Value) -> Option<u64> {
    let text = value.as_str()?.strip_prefix("0x")?;
    if text.len() != 16 {
        return None;
    }
    u64::from_str_radix(text, 16).ok()
}

fn property_value(row: &Value) -> bool {
    let Some(returned) = hex(&row["returned"]) else {
        return false;
    };
    match row["read_state"].as_u64() {
        Some(1) => {
            returned != 0
                && row["tag"] == 1
                && row["flags14"] == 0
                && hex(&row["payload8"]).is_some_and(|value| value <= 1)
        }
        Some(state @ (0 | 2)) => {
            (returned == 0) == (state == 0)
                && ["tag", "payload8", "flags14"]
                    .iter()
                    .all(|name| row.get(*name).is_some_and(Value::is_null))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn capture() -> Vec<Value> {
        vec![
            json!({"kind":"identity","pid":7,"created_filetime":123,"mode":"paid-assignment-experimental",
                "artifact_schema":1,"paid_assignment_experimental":true,"scope_key_ranges":KEY_RANGES,
                "gamecore_sha256":"324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c",
                "assignment_rva":0x2dd880,"can_ever_rva":0x2dddd0,"property_rva":0x99f5c0}),
            json!({"kind":"install","status":0}),
            json!({"kind":"uninstall","status":0,"entry_restored":true,"assignment_restored":true,"prefix_retained_until_exit":true}),
            json!({"kind":"calibration","sequence":1,"governor_index":77,"original_al":1,"effective_al":0}),
            json!({"kind":"property_get","sequence":1,"key_id":907002,"read_state":1,"property":"0x0000000000001000","returned":"0x0000000000002000","tag":1,"payload8":"0x0000000000000001","flags14":0}),
            json!({"kind":"paid_assignment","sequence":1,"player_id":2,"governor_index":77,"original_rax":"0xaabbccdd11223301",
                "effective_rax":"0xaabbccdd11223300","original_al":1,"effective_al":0,"payment_authority":false,"observation_only":true}),
            json!({"kind":"stop","rows":3,"observed_calls":3,"dropped":0,"in_flight":0,"result":"ok",
                "classification":"paid_assignment_experimental","snapshot_stable":true,"quiescence_proven":false}),
        ]
    }
    fn text(rows: &[Value]) -> String {
        rows.iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }
    #[test]
    fn validates_three_entry_capture_without_claiming_product_acceptance() {
        assert_eq!(validate(&text(&capture()), 7, 123), Ok(1));
        assert!(validate(&text(&capture()), 8, 123).is_err());
    }
    #[test]
    fn missing_restoration_unknown_keys_old_identity_and_rax_corruption_fail() {
        for (row, key, value) in [
            (0, "mode", json!("production-admission")),
            (2, "assignment_restored", json!(false)),
            (4, "key_id", json!(908000)),
            (5, "effective_rax", json!("0xaabbccdd11224400")),
            (5, "payment_authority", json!(true)),
            (6, "rows", json!(2)),
            (6, "snapshot_stable", json!(false)),
        ] {
            let mut rows = capture();
            rows[row][key] = value;
            assert!(validate(&text(&rows), 7, 123).is_err(), "{key}");
        }
        let mut rows = capture();
        rows.remove(5);
        assert!(validate(&text(&rows), 7, 123).is_err());
        let mut rows = capture();
        rows.push(json!({"kind":"install","status":0}));
        assert!(validate(&text(&rows), 7, 123).is_err());
    }

    #[test]
    fn copied_bool_requires_all_fields_and_rejects_wrong_types_or_non_bool_payload() {
        for key in ["read_state", "returned", "tag", "payload8", "flags14"] {
            let mut rows = capture();
            rows[4].as_object_mut().unwrap().remove(key);
            assert!(validate(&text(&rows), 7, 123).is_err(), "missing {key}");
        }
        for (key, value) in [
            ("tag", json!(2)),
            ("tag", json!("1")),
            ("flags14", json!(1)),
            ("payload8", json!("0x0000000000000002")),
            ("payload8", json!(1)),
            ("returned", json!("0x0000000000000000")),
        ] {
            let mut rows = capture();
            rows[4][key] = value;
            assert!(validate(&text(&rows), 7, 123).is_err(), "{key}");
        }
        let mut rows = capture();
        rows[4]["payload8"] = json!("0x0000000000000000");
        assert_eq!(validate(&text(&rows), 7, 123), Ok(1));
    }

    #[test]
    fn absent_and_unreadable_values_require_explicit_nulls_and_matching_return_pointer() {
        for (state, pointer) in [(0, "0x0000000000000000"), (2, "0x0000000000000001")] {
            let mut rows = capture();
            rows[4]["read_state"] = json!(state);
            rows[4]["returned"] = json!(pointer);
            for key in ["tag", "payload8", "flags14"] {
                rows[4][key] = Value::Null;
            }
            assert_eq!(validate(&text(&rows), 7, 123), Ok(1));
            for key in ["tag", "payload8", "flags14"] {
                let mut missing = rows.clone();
                missing[4].as_object_mut().unwrap().remove(key);
                assert!(validate(&text(&missing), 7, 123).is_err());
                let mut fake = rows.clone();
                fake[4][key] = json!(0);
                assert!(validate(&text(&fake), 7, 123).is_err());
            }
            rows[4]["read_state"] = json!(2 - state);
            assert!(validate(&text(&rows), 7, 123).is_err());
        }
        let mut rows = capture();
        rows[6]["dropped"] = json!(5);
        rows[6]["observed_calls"] = json!(8);
        assert_eq!(
            validate(&text(&rows), 7, 123),
            Ok(1),
            "bounded prefix remains valid capture"
        );
    }
}

pub fn validate(text: &str, pid: u32, created: u64) -> Result<usize, String> {
    let mut phase = 0;
    let mut count = 0;
    let mut assignment = 0;
    let mut property = 0;
    let mut calibration = 0;
    for line in text.lines() {
        let row: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        match row["kind"].as_str() {
            Some("identity") if phase == 0 => {
                if row["pid"] != pid
                    || row["created_filetime"] != created
                    || pid == 0
                    || created == 0
                    || row["mode"] != "paid-assignment-experimental"
                    || row["artifact_schema"] != 1
                    || row["paid_assignment_experimental"] != true
                    || row["scope_key_ranges"] != serde_json::json!(KEY_RANGES)
                    || row["gamecore_sha256"]
                        != "324c51e9ea3531758842e16c69e6cddbefbb226c5b675c3d6e60111646c2e98c"
                    || row["assignment_rva"] != 0x2dd880u64
                    || row["can_ever_rva"] != 0x2dddd0u64
                    || row["property_rva"] != 0x99f5c0u64
                {
                    return Err("experimental identity differs".into());
                }
                phase = 1;
            }
            Some("install") if phase == 1 && row["status"] == 0 => phase = 2,
            Some("uninstall")
                if phase == 2
                    && row["status"] == 0
                    && row["entry_restored"] == true
                    && row["assignment_restored"] == true
                    && row["prefix_retained_until_exit"] == true =>
            {
                phase = 3
            }
            Some("property_get") if phase == 3 => {
                let key = row["key_id"].as_u64().ok_or("property key missing")?;
                if key == 0
                    || !KEY_RANGES
                        .iter()
                        .any(|r| (r[0] as u64..=r[1] as u64).contains(&key))
                    || row["sequence"] != property + 1
                    || !property_value(&row)
                    || hex(&row["property"]).is_none_or(|p| p == 0 || p & 7 != 0)
                    || hex(&row["returned"]).is_none()
                {
                    return Err("invalid experimental property row".into());
                }
                property += 1;
                count += 1;
            }
            Some("calibration") if phase == 3 => {
                if row["sequence"] != calibration + 1
                    || !row["original_al"].as_u64().is_some_and(|v| v <= 255)
                    || !row["effective_al"].as_u64().is_some_and(|v| v <= 255)
                    || !row["governor_index"]
                        .as_u64()
                        .is_some_and(|v| v <= u32::MAX as u64)
                {
                    return Err("invalid appointment row".into());
                }
                calibration += 1;
                count += 1;
            }
            Some("paid_assignment") if phase == 3 => {
                let original = hex(&row["original_rax"]).ok_or("missing original RAX")?;
                let effective = hex(&row["effective_rax"]).ok_or("missing effective RAX")?;
                if row["sequence"] != assignment + 1
                    || (effective != original && effective != original & !255)
                    || row["original_al"] != original & 255
                    || row["effective_al"] != effective & 255
                    || row["payment_authority"] != false
                    || row["observation_only"] != true
                    || !row["player_id"]
                        .as_u64()
                        .is_some_and(|v| v <= u32::MAX as u64)
                    || !row["governor_index"]
                        .as_u64()
                        .is_some_and(|v| v <= u32::MAX as u64)
                {
                    return Err("invalid paid assignment decision".into());
                }
                assignment += 1;
                count += 1;
            }
            Some("stop") if phase == 3 => {
                let dropped = row["dropped"].as_u64().ok_or("missing dropped count")?;
                if assignment == 0
                    || property == 0
                    || calibration == 0
                    || row["rows"] != count
                    || row["observed_calls"] != count as u64 + dropped
                    || row["result"] != "ok"
                    || row["classification"] != "paid_assignment_experimental"
                    || row["in_flight"] != 0
                    || row["snapshot_stable"] != true
                    || row["quiescence_proven"] != false
                {
                    return Err("incomplete experimental capture".into());
                }
                phase = 4;
            }
            _ => return Err("unexpected experimental artifact row or lifecycle".into()),
        }
    }
    if phase != 4 {
        return Err("missing completed experimental capture".into());
    }
    Ok(assignment)
}
