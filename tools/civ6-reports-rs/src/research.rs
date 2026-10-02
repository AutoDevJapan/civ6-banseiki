use crate::{common, Result};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

pub fn summarize(path: &Path, kind: &str) -> Result<Vec<Value>> {
    if kind != "tech" && kind != "civic" {
        return Err("kind must be tech or civic".into());
    }
    let completed_field = format!("completed_{kind}s");
    let mut first = BTreeMap::new();
    let mut baseline = BTreeSet::new();
    for (number, line) in common::read_text(path)?.lines().enumerate() {
        let read = || -> Result<(Value, u64, u64)> {
            let row: Value = serde_json::from_str(line)?;
            if !matches!(
                row.get("phase").and_then(Value::as_str),
                Some("start" | "turn")
            ) || row.get("observed") != Some(&Value::Bool(true))
            {
                return Err("unobserved row".into());
            }
            // Reject booleans and integral floats, as Python's type(x) is int does.
            let turn = row
                .get("turn")
                .and_then(Value::as_u64)
                .ok_or("invalid turn or slot")?;
            let slot = row
                .get("slot")
                .and_then(Value::as_u64)
                .ok_or("invalid turn or slot")?;
            let completed = row
                .get(&completed_field)
                .and_then(Value::as_array)
                .ok_or("invalid completed IDs")?;
            if completed.iter().any(|item| !item.is_string()) {
                return Err("invalid completed IDs".into());
            }
            Ok((row, turn, slot))
        };
        let (row, turn, slot) = read().map_err(|error| {
            format!(
                "Invalid research row {}:{}: {error}",
                path.display(),
                number + 1
            )
        })?;
        for item in row[&completed_field].as_array().expect("validated array") {
            let key = (slot, item.as_str().expect("validated string").to_owned());
            if let std::collections::btree_map::Entry::Vacant(entry) = first.entry(key.clone()) {
                entry.insert(turn);
                if row["phase"] == "start" {
                    baseline.insert(key);
                }
            }
        }
    }
    let mut by_name: BTreeMap<String, Vec<(u64, u64)>> = BTreeMap::new();
    for ((slot, name), turn) in first {
        by_name.entry(name).or_default().push((slot, turn));
    }
    let mut result = Vec::new();
    for (name, faction_turns) in by_name {
        let mut values: Vec<_> = faction_turns.iter().map(|(_, turn)| *turn).collect();
        values.sort_unstable();
        let n = values.len();
        let median = if n % 2 == 1 {
            json!(values[n / 2])
        } else {
            json!((values[n / 2 - 1] as f64 + values[n / 2] as f64) / 2.0)
        };
        result.push(json!({
            "id": name, "factions": n, "median_turn": median,
            "min_turn": values[0], "max_turn": values[n - 1],
            "spread_turns": values[n - 1] - values[0],
            "baseline_factions": faction_turns.iter().filter(|(slot, _)| baseline.contains(&(*slot, name.clone()))).count()
        }));
    }
    result.sort_by(|a, b| {
        a["median_turn"]
            .as_f64()
            .unwrap()
            .total_cmp(&b["median_turn"].as_f64().unwrap())
            .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
    });
    Ok(result)
}

pub fn tsv(rows: &[Value]) -> String {
    let keys = [
        "id",
        "factions",
        "median_turn",
        "min_turn",
        "max_turn",
        "spread_turns",
        "baseline_factions",
    ];
    let mut lines = vec![keys.join("\t")];
    for row in rows {
        lines.push(
            keys.iter()
                .map(|key| match &row[*key] {
                    Value::String(value) => value.clone(),
                    value => value.to_string(),
                })
                .collect::<Vec<_>>()
                .join("\t"),
        );
    }
    lines.join("\n") + "\n"
}
