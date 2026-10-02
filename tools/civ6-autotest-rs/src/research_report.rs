//! First observed research completion turns across major factions.
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
};

pub fn summarize(path: &Path, kind: &str) -> std::result::Result<Vec<Value>, String> {
    if !matches!(kind, "tech" | "civic") {
        return Err("kind must be tech or civic".into());
    }
    let field = format!("completed_{kind}s");
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut first: BTreeMap<(i64, String), i64> = BTreeMap::new();
    let mut baseline: BTreeSet<(i64, String)> = BTreeSet::new();
    for (i, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|e| e.to_string())?;
        let row: Value = serde_json::from_str(&line)
            .map_err(|e| format!("Invalid research row {}:{}: {e}", path.display(), i + 1))?;
        let invalid = || format!("Invalid research row {}:{}", path.display(), i + 1);
        let phase = row
            .get("phase")
            .and_then(Value::as_str)
            .ok_or_else(invalid)?;
        let observed = row
            .get("observed")
            .and_then(Value::as_bool)
            .ok_or_else(invalid)?;
        let turn = row
            .get("turn")
            .and_then(Value::as_i64)
            .ok_or_else(invalid)?;
        let slot = row
            .get("slot")
            .and_then(Value::as_i64)
            .ok_or_else(invalid)?;
        let completed = row
            .get(&field)
            .and_then(Value::as_array)
            .ok_or_else(invalid)?;
        if !matches!(phase, "start" | "turn")
            || !observed
            || turn < 0
            || slot < 0
            || completed.iter().any(|v| !v.is_string())
        {
            return Err(invalid());
        }
        for name in completed.iter().filter_map(Value::as_str) {
            let key = (slot, name.to_string());
            if !first.contains_key(&key) {
                first.insert(key.clone(), turn);
                if phase == "start" {
                    baseline.insert(key);
                }
            }
        }
    }
    let mut by_name: BTreeMap<String, Vec<(i64, i64)>> = BTreeMap::new();
    for ((slot, name), turn) in first {
        by_name.entry(name).or_default().push((slot, turn));
    }
    let mut result = Vec::new();
    for (name, factions) in by_name {
        let mut turns: Vec<_> = factions.iter().map(|(_, turn)| *turn).collect();
        turns.sort();
        let n = turns.len();
        let median = if n % 2 == 1 {
            json!(turns[n / 2])
        } else {
            json!((turns[n / 2 - 1] as f64 + turns[n / 2] as f64) / 2.0)
        };
        let min = turns[0];
        let max = turns[n - 1];
        let baseline_factions = factions
            .iter()
            .filter(|(slot, _)| baseline.contains(&(*slot, name.clone())))
            .count();
        result.push(
            json!({"id":name,"factions":n,"median_turn":median,"min_turn":min,"max_turn":max,
            "spread_turns":max-min,"baseline_factions":baseline_factions}),
        );
    }
    result.sort_by(|a, b| {
        a["median_turn"]
            .as_f64()
            .unwrap_or(0.0)
            .total_cmp(&b["median_turn"].as_f64().unwrap_or(0.0))
            .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
    });
    Ok(result)
}
