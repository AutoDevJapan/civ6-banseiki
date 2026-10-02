use crate::{common, Result};
use regex::Regex;
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::LazyLock,
};

pub const EXPECTED_CHECKPOINTS: [i64; 5] = [1, 51, 101, 151, 201];
pub const EXPECTED_REIGNS: [(&str, i64); 6] = [
    ("REIGN_KOJI", 66),
    ("REIGN_EIROKU", 73),
    ("REIGN_GENKI", 110),
    ("REIGN_TENSHO", 119),
    ("REIGN_BUNROKU", 178),
    ("REIGN_KEICHO", 189),
];

static AGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
    r"\[JAC_AGES\] judged reign=(\S+) turn=(\d+) length=(\d+)(?: era=(\d+))?(?: engineEra=\S+)? player=(\d+) age=(GOLDEN|NORMAL|DARK|HEROIC) score=(-?\d+) golden>=(\d+) dark>=(\d+) policy=(\S*)"
).unwrap()
});
static ARB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[JAC_ARB\] (.*)").unwrap());
static HOUSE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
    r"\Ahouse player=(\d+) civ=CIVILIZATION_JAC_\w+ alive=(true|false) cities=\d+ castles=\d+ legitimacy=(?:\d+|nil) age=(?:NONE|GOLDEN|HEROIC|NORMAL|DARK|nil) policy=(?:JAC_AGE_POLICY_\w+|nil) pact-lead=(?:\d+|nil)\z"
).unwrap()
});
static ERA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\Aera-values grievances=-?\d+(?:\.\d+)? tensions=-?\d+(?:\.\d+)? trade-minimum-change=-?\d+(?:\.\d+)?\z").unwrap()
});
static HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\Acheckpoint turn=(\d+) era=\S+ targets=\d+\z").unwrap());
static ROSTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\Ajudgment-roster reign-index=(\d+) turn=(\d+) players=(\d+(?:,\d+)*)\z").unwrap()
});
static REQUEST_TURN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bturn=(\d+)").unwrap());
static REFUSED_CASE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"pair=(\d+)-(\d+) turn=(\d+)").unwrap());
static FALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[JAC_AGES\] points player=(\d+) part=ko .*?key=ko:(\d+) turn=(\d+)").unwrap()
});
static CHECKPOINT_TURN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\Acheckpoint turn=(\d+) ").unwrap());

fn integer(row: &Value, key: &str) -> Result<i64> {
    row.get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| format!("missing or invalid {key}").into())
}

fn survivors(
    initial: &BTreeSet<i64>,
    falls: &[Value],
    turn: i64,
    position: Option<i64>,
    issues: &mut Vec<String>,
) -> Result<BTreeSet<i64>> {
    let mut expected = initial.clone();
    for fall in falls {
        let due = integer(fall, "turn")?;
        let before = if due != turn {
            due < turn
        } else {
            match (position, fall.get("log_position").and_then(Value::as_i64)) {
                (Some(position), Some(fall_position)) => fall_position < position,
                _ => {
                    issues.push(format!("cannot order elimination within T+{turn}"));
                    false
                }
            }
        };
        if before {
            expected.remove(&integer(fall, "player")?);
        }
    }
    Ok(expected)
}

pub fn coverage(
    checkpoints: &[String],
    judgments: &[Value],
    falls: &[Value],
    checkpoint_positions: &BTreeMap<i64, i64>,
) -> Result<(Vec<String>, usize)> {
    let mut issues = Vec::new();
    let mut seen = BTreeSet::new();
    let mut rosters: BTreeMap<i64, (i64, Vec<i64>)> = BTreeMap::new();
    let mut alive: Vec<(i64, BTreeSet<i64>)> = Vec::new();
    let all_houses: BTreeSet<i64> = (0..22).collect();
    for checkpoint in checkpoints {
        let parts: Vec<_> = checkpoint.split(" ; ").collect();
        let Some(header) = HEADER.captures(parts[0]) else {
            issues.push("invalid checkpoint header".into());
            continue;
        };
        let turn: i64 = header[1].parse()?;
        if !seen.insert(turn) {
            issues.push(format!("duplicate checkpoint: T+{turn}"));
        }
        let statuses: Vec<_> = parts
            .iter()
            .filter(|part| part.starts_with("house "))
            .filter_map(|part| HOUSE.captures(part))
            .map(|c| Ok((c[1].parse::<i64>()?, c[2] == *"true")))
            .collect::<Result<_>>()?;
        let ids: BTreeSet<_> = statuses.iter().map(|(id, _)| *id).collect();
        if statuses.len() != 22 || ids != all_houses {
            issues.push(format!("incomplete house observations: T+{turn}"));
            issues.push(format!("incomplete survivor observations: T+{turn}"));
        }
        let survivors: BTreeSet<_> = statuses
            .into_iter()
            .filter(|(_, alive)| *alive)
            .map(|(id, _)| id)
            .collect();
        if let Some((_, stored)) = alive.iter_mut().find(|(due, _)| *due == turn) {
            *stored = survivors;
        } else {
            alive.push((turn, survivors));
        }
        let era_rows: Vec<_> = parts
            .iter()
            .filter(|part| part.starts_with("era-values "))
            .collect();
        if era_rows.len() != 1 || !ERA.is_match(era_rows[0]) {
            issues.push(format!("incomplete era observations: T+{turn}"));
        }
        let mut local_rosters = BTreeSet::new();
        for part in parts
            .iter()
            .filter(|part| part.starts_with("judgment-roster "))
        {
            let Some(m) = ROSTER.captures(part) else {
                issues.push(format!("invalid judgment roster: T+{turn}"));
                continue;
            };
            let index: i64 = m[1].parse()?;
            let due: i64 = m[2].parse()?;
            let ids: Vec<i64> = m[3]
                .split(',')
                .map(|v| Ok(v.parse()?))
                .collect::<Result<_>>()?;
            if !local_rosters.insert(index) {
                issues.push(format!("duplicate roster within T+{turn}: reign {index}"));
            }
            let unique: BTreeSet<_> = ids.iter().copied().collect();
            if ids.len() != unique.len() || !unique.is_subset(&all_houses) {
                issues.push(format!("invalid player IDs in reign {index}"));
            }
            let roster = (due, ids);
            if rosters
                .get(&index)
                .is_some_and(|previous| previous != &roster)
            {
                issues.push(format!("inconsistent saved roster: reign {index}"));
            }
            rosters.insert(index, roster);
        }
    }
    let expected_checkpoints: BTreeSet<_> = EXPECTED_CHECKPOINTS.into_iter().collect();
    if seen != expected_checkpoints {
        issues.push(format!(
            "checkpoint turns mismatch: {:?}",
            seen.iter().collect::<Vec<_>>()
        ));
    }
    let initial = alive
        .iter()
        .find(|(turn, _)| *turn == 1)
        .map(|(_, ids)| ids.clone())
        .unwrap_or_default();
    if initial != all_houses {
        issues.push("initial roster is not the configured 22 houses".into());
    }
    for (turn, ids) in alive {
        let expected = survivors(
            &initial,
            falls,
            turn,
            checkpoint_positions.get(&turn).copied(),
            &mut issues,
        )?;
        if ids != expected {
            issues.push(format!(
                "cannot reconcile survivors with elimination events: T+{turn}"
            ));
        }
    }
    let actual_reigns: BTreeSet<_> = judgments
        .iter()
        .map(|row| common::field(row, "reign"))
        .collect::<Result<_>>()?;
    if actual_reigns != EXPECTED_REIGNS.iter().map(|(name, _)| *name).collect() {
        issues.push("age log does not contain all six expected reigns".into());
    }
    if rosters.keys().copied().collect::<BTreeSet<_>>() != (1..7).collect() {
        issues.push("saved judgment rosters do not cover all six expected reigns".into());
    }
    for (index, (name, turn)) in EXPECTED_REIGNS.iter().enumerate() {
        let rows: Vec<_> = judgments
            .iter()
            .filter(|row| row["reign"] == *name)
            .collect();
        let Some(expected) = rosters.get(&(index as i64 + 1)) else {
            continue;
        };
        if expected.0 != *turn || rows.iter().any(|row| row["turn"] != *turn) {
            issues.push(format!("judgment turn mismatch: {name}"));
        }
        let positions: Vec<_> = rows
            .iter()
            .filter_map(|row| row.get("log_position").and_then(Value::as_i64))
            .collect();
        let position = if positions.len() == rows.len() {
            positions.into_iter().min()
        } else {
            None
        };
        let survivors = survivors(&initial, falls, *turn, position, &mut issues)?;
        let expected_ids: BTreeSet<_> = expected.1.iter().copied().collect();
        if expected_ids != survivors {
            issues.push(format!("saved roster survivor coverage mismatch: {name}"));
        }
        let actual_ids: BTreeSet<_> = rows
            .iter()
            .map(|row| integer(row, "player"))
            .collect::<Result<_>>()?;
        if actual_ids != expected_ids || rows.len() != expected.1.len() {
            issues.push(format!("age log player coverage mismatch: {name}"));
        }
    }
    Ok((issues, seen.intersection(&expected_checkpoints).count()))
}

pub fn summarize(artifact: &Path) -> Result<Value> {
    let steps = common::read_steps(artifact, false)?;
    let (lua, lua_sha256) = common::read_lua(artifact)?;
    let mut completed = Vec::new();
    let mut checkpoints = Vec::new();
    let mut overview = Vec::new();
    let mut failures = Vec::new();
    let mut judge = Vec::new();
    let mut target = false;
    let mut load_step = false;
    for step in &steps {
        let status = common::field(step, "status")?;
        let action = common::field(step, "action")?;
        let detail = common::field(step, "detail")?;
        if status == "OK" {
            if action.ends_with("autoplay") {
                completed.push(detail.to_owned());
            }
            if action.ends_with("ui_lua") && detail.contains("return=checkpoint turn=") {
                checkpoints.push(detail.split_once("return=").unwrap().1.to_owned());
            }
            if detail.contains("return=T+") {
                overview.push(detail.split_once("return=").unwrap().1.to_owned());
            }
            target |= detail.contains("return=T+201 surviving majors=");
            load_step |= action.ends_with(" load");
        }
        if status == "FAIL" {
            failures.push(step);
        }
        if action == "judge" {
            judge.push(step);
        }
    }
    let mut judgments = Vec::new();
    let mut seen = BTreeSet::new();
    let mut grouped: Map<String, Value> = Map::new();
    for m in AGE.captures_iter(&lua) {
        let reign = m[1].to_owned();
        let turn: i64 = m[2].parse()?;
        let length: i64 = m[3].parse()?;
        let era: Option<i64> = m.get(4).map(|v| v.as_str().parse()).transpose()?;
        let player: i64 = m[5].parse()?;
        if !seen.insert((reign.clone(), player)) {
            return Err(format!(
                "duplicate age judgment: ('{reign}', {player}); inspect run/reload boundaries"
            )
            .into());
        }
        let policy = (!m[10].is_empty()).then(|| m[10].to_owned());
        judgments.push(json!({"reign": reign, "turn": turn, "length": length, "era": era, "player": player,
            "age": &m[6], "score": m[7].parse::<i64>()?, "golden": m[8].parse::<i64>()?, "dark": m[9].parse::<i64>()?, "policy": policy,
            "log_position": common::character_position(&lua, m.get(0).unwrap().start())}));
        let g = grouped
            .entry(&reign)
            .or_insert_with(|| json!({"turn": turn, "length": length, "ages": {}, "policies": {}}));
        if g["turn"] != turn || g["length"] != length {
            return Err(format!("inconsistent reign timing: {reign}").into());
        }
        common::increment(g["ages"].as_object_mut().unwrap(), &m[6], 1);
        if let Some(policy) = policy {
            common::increment(g["policies"].as_object_mut().unwrap(), &policy, 1);
        }
    }
    let arb_lines: Vec<_> = ARB.captures_iter(&lua).map(|m| m[1].to_owned()).collect();
    let mut counts = Map::new();
    for kind in [
        "request",
        "accepted",
        "refused",
        "decision",
        "void request",
        "rejected request",
    ] {
        counts.insert(
            kind.into(),
            json!(arb_lines
                .iter()
                .filter(|line| line.starts_with(&(kind.to_owned() + " ")))
                .count()),
        );
    }
    let request_turns: Vec<i64> = arb_lines
        .iter()
        .filter(|line| line.starts_with("request "))
        .filter_map(|line| REQUEST_TURN.captures(line))
        .map(|m| Ok(m[1].parse()?))
        .collect::<Result<_>>()?;
    let request_intervals: Vec<_> = request_turns.windows(2).map(|v| v[1] - v[0]).collect();
    counts.insert("request_turns".into(), json!(request_turns));
    counts.insert("request_intervals".into(), json!(request_intervals));
    counts.insert(
        "safety_net".into(),
        json!(arb_lines
            .iter()
            .filter(|line| line.contains("SAFETY NET"))
            .count()),
    );
    let refused_cases: BTreeSet<_> = arb_lines
        .iter()
        .filter(|line| line.starts_with("refused "))
        .filter_map(|line| REFUSED_CASE.captures(line))
        .map(|m| (m[1].to_owned(), m[2].to_owned(), m[3].to_owned()))
        .collect();
    counts.insert("refused_cases".into(), json!(refused_cases.len()));
    let falls: Vec<Value> = FALL.captures_iter(&lua).map(|m| Ok(json!({"capturer": m[1].parse::<i64>()?, "player": m[2].parse::<i64>()?, "turn": m[3].parse::<i64>()?, "log_position": common::character_position(&lua, m.get(0).unwrap().start())}))).collect::<Result<_>>()?;
    let mut full = completed.len() == 4
        && completed
            .iter()
            .all(|s| s.contains("autoplay completed 50 turns"))
        && target
        && judge.last().is_some_and(|s| s["status"] == "OK")
        && failures.is_empty();
    let result_path = artifact.join("result.json");
    let result: Value = if result_path.exists() {
        serde_json::from_str(&common::read_text(&result_path)?)?
    } else {
        json!({})
    };
    let continuous = result["passed"] == true
        && !common::truthy(&result["test_mode"])
        && !load_step
        && arb_lines
            .iter()
            .filter(|line| line.starts_with("loaded "))
            .count()
            <= 1;
    full &= continuous;
    let mut checkpoint_positions = BTreeMap::new();
    for checkpoint in &checkpoints {
        let header = checkpoint.split(" ; ").next().unwrap();
        if let (Some(m), Some(position)) = (CHECKPOINT_TURN.captures(header), lua.find(header)) {
            checkpoint_positions.insert(
                m[1].parse()?,
                common::character_position(&lua, position) as i64,
            );
        }
    }
    let (mut issues, checkpoint_count) =
        coverage(&checkpoints, &judgments, &falls, &checkpoint_positions)?;
    if !continuous {
        issues.push(
            "successful uninterrupted scenario result is missing, failed, synthetic or reloaded"
                .into(),
        );
    }
    Ok(json!({
        "artifact": common::artifact_name(artifact), "lua_sha256": lua_sha256, "complete_200_turns": full,
        "autoplay": completed, "overview": overview, "checkpoints": checkpoints,
        "reigns": grouped, "judgments": judgments, "arbitration": counts, "elimination_point_events": falls,
        "age_data_present": !judgments.is_empty(), "checkpoint_count": checkpoint_count,
        "measurement_complete": full && issues.is_empty(), "coverage_issues": issues, "failures": failures, "judge": judge,
        "limitations": ["Request sources and actual costs require arbitration_report.py.", "Refused counts respondents; accepted counts cases.", "Era values are observations, not measured trade/grievance/road effects."]
    }))
}

pub fn markdown(data: &Value) -> String {
    let completed = if data["complete_200_turns"] == true {
        "PASS"
    } else {
        "未達"
    };
    let measured = if data["measurement_complete"] == true {
        "PASS"
    } else {
        "未達（以下は部分集計。数値調整の根拠にしない）"
    };
    let mut lines = vec![
        format!(
            "実行: {}。200ターン完了とログ判定: {completed}。",
            data["artifact"].as_str().unwrap()
        ),
        format!("計測の完全性: {measured}。"),
        "".into(),
        "| 改元 | ターン | 期の長さ | 黄金 | 英雄 | 通常 | 暗黒 |".into(),
        "|---|---:|---:|---:|---:|---:|---:|".into(),
    ];
    let mut groups: Vec<_> = data["reigns"].as_object().unwrap().iter().collect();
    groups.sort_by_key(|(_, g)| g["turn"].as_i64().unwrap());
    for (reign, g) in groups {
        lines.push(format!(
            "| {reign} | {} | {} | {} | {} | {} | {} |",
            g["turn"],
            g["length"],
            common::counter(&g["ages"], "GOLDEN"),
            common::counter(&g["ages"], "HEROIC"),
            common::counter(&g["ages"], "NORMAL"),
            common::counter(&g["ages"], "DARK")
        ));
    }
    let c = &data["arbitration"];
    lines.extend([
        "".into(), format!("裁定の請求 {}件、成立 {}件、拒否した当事者 {}人。", c["request"], c["accepted"], c["refused"]),
        "請求数だけでは頻度・費用の適正さを評価できない。AIの請求経路と実際の支出は裁定専用の集計で照合する。".into(),
        "".into(), "各家の存続・城数・暦などのチェックポイント：".into(),
    ]);
    lines.extend(
        data["overview"]
            .as_array()
            .unwrap()
            .iter()
            .map(|line| format!("- {}", line.as_str().unwrap())),
    );
    if data["age_data_present"] != true {
        lines.extend([
            "".into(),
            "年齢判定の記録が無い。年齢の割合は評価できない。".into(),
        ]);
    }
    lines.extend([
        "".into(),
        format!(
            "記録した観測地点: {}/5。計測の完全性は別に判定する。",
            data["checkpoint_count"]
        ),
    ]);
    let issues = data["coverage_issues"].as_array().unwrap();
    if !issues.is_empty() {
        lines.extend(["".into(), "計測が不完全な理由：".into()]);
        lines.extend(
            issues
                .iter()
                .map(|issue| format!("- {}", issue.as_str().unwrap())),
        );
    }
    lines.join("\n") + "\n"
}
