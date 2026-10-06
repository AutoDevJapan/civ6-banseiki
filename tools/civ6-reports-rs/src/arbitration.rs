use crate::{common, Result};
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    str::FromStr,
    sync::LazyLock,
};

static ARB: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[JAC_ARB\] ([^\r\n]+)").unwrap());
static FIELDS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\w+)=([^\s]+)").unwrap());
type Pair = Vec<i64>;

// Updating an existing key must keep its original position, like a Python dict.
#[derive(Default)]
struct Ordered<K, V>(Vec<(K, V)>);

impl<K: PartialEq, V> Ordered<K, V> {
    fn get(&self, key: &K) -> Option<&V> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    fn insert(&mut self, key: K, value: V) -> bool {
        if let Some((_, v)) = self.0.iter_mut().find(|(k, _)| k == &key) {
            *v = value;
            true
        } else {
            self.0.push((key, value));
            false
        }
    }
}

#[derive(Clone, Default)]
struct Request {
    player: i64,
    power: i64,
    pair: Pair,
}

#[derive(Clone, Default, Serialize)]
struct Cost {
    player: i64,
    pair: Pair,
    source: String,
    gold: i64,
    legitimacy: i64,
    gold_before: Option<f64>,
    legitimacy_before: Option<i64>,
    gold_after: f64,
    legitimacy_after: i64,
}

#[derive(Default, Serialize)]
struct Ai {
    player: i64,
    source: String,
    score: i64,
    pair: Pair,
    detail: Map<String, Value>,
}

struct Refusal {
    player: i64,
    turn: i64,
    intended: i64,
    applied: i64,
    legitimacy_before: i64,
    legitimacy_after: i64,
    pair: Pair,
}
struct Weariness {
    player: i64,
    turn: i64,
    observed: i64,
    cities: i64,
}

fn number<T: FromStr>(fields: &Map<String, Value>, key: &str) -> Result<T> {
    let value = text(fields, key)?;
    value
        .parse()
        .map_err(|_| format!("invalid {key}: {value}").into())
}

fn text<'a>(fields: &'a Map<String, Value>, key: &str) -> Result<&'a str> {
    fields
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing field: {key}").into())
}

fn pair(fields: &Map<String, Value>) -> Result<Pair> {
    let mut ids: Pair = text(fields, "pair")?
        .split('-')
        .map(|v| v.parse().map_err(|_| format!("invalid pair: {v}").into()))
        .collect::<Result<_>>()?;
    ids.sort_unstable();
    Ok(ids)
}

fn tuple(ids: &[i64]) -> String {
    format!(
        "({}{})",
        ids.iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", "),
        if ids.len() == 1 { "," } else { "" }
    )
}

fn respondent(turn: i64, pair: &Pair, player: i64) -> String {
    format!("({turn}, {}, {player})", tuple(pair))
}

pub fn summarize(artifact: &Path) -> Result<Value> {
    let (lua, lua_sha256) = common::read_lua(artifact)?;
    let events: Vec<_> = ARB.captures_iter(&lua).map(|c| c[1].to_owned()).collect();
    let steps = common::read_steps(artifact, true)?;
    let mut issues = Vec::new();
    let mut requests = Ordered::<i64, Request>::default();
    let mut costs = Ordered::<i64, Cost>::default();
    let mut outcomes = Ordered::<(i64, Pair), String>::default();
    let mut ai = Ordered::<i64, Ai>::default();
    let mut refusals = Vec::new();
    let mut decisions = Map::new();
    let mut blocked = Map::new();
    let mut refused_players = BTreeMap::new();
    let mut loads = 0;
    let mut weariness = Vec::new();
    let mut rosters = Ordered::<i64, BTreeMap<i64, i64>>::default();
    let mut end_turns = BTreeSet::new();
    for event in &events {
        let mut f = Map::new();
        for c in FIELDS.captures_iter(event) {
            f.insert(c[1].to_owned(), Value::String(c[2].to_owned()));
        }
        if event.starts_with("loaded ") {
            loads += 1;
            end_turns.insert(number::<i64>(&f, "shogunate_end_turn")?);
        }
        if event.starts_with("request ") {
            let turn = number(&f, "turn")?;
            if requests.insert(
                turn,
                Request {
                    player: number(&f, "player")?,
                    power: number(&f, "power")?,
                    pair: pair(&f)?,
                },
            ) {
                issues.push(format!("duplicate request on turn {turn}"));
            }
        } else if event.starts_with("cost ") {
            let turn = number(&f, "turn")?;
            let cost = Cost {
                player: number(&f, "player")?,
                pair: pair(&f)?,
                source: text(&f, "by")?.into(),
                gold: number(&f, "gold")?,
                legitimacy: number(&f, "legitimacy")?,
                gold_before: f
                    .contains_key("gold_before")
                    .then(|| number(&f, "gold_before"))
                    .transpose()?,
                legitimacy_before: f
                    .contains_key("legitimacy_before")
                    .then(|| number(&f, "legitimacy_before"))
                    .transpose()?,
                gold_after: number(&f, "gold_after")?,
                legitimacy_after: number(&f, "legitimacy_after")?,
            };
            if costs.insert(turn, cost) {
                issues.push(format!("duplicate request cost on turn {turn}"));
            }
        } else if event.starts_with("ai request ") {
            let turn = number(&f, "turn")?;
            let row = Ai {
                player: number(&f, "player")?,
                source: text(&f, "by")?.into(),
                score: number(&f, "score")?,
                pair: pair(&f)?,
                detail: f,
            };
            if ai.insert(turn, row) {
                issues.push(format!("duplicate AI decision on turn {turn}"));
            }
        } else if ["accepted ", "refused ", "void request "]
            .iter()
            .any(|prefix| event.starts_with(prefix))
        {
            let turn = number(&f, "turn")?;
            let pair = pair(&f)?;
            let kind = event.split(' ').next().unwrap();
            let key = (turn, pair.clone());
            if let Some(previous) = outcomes.get(&key) {
                if kind != "refused" || previous != "refused" {
                    issues.push(format!(
                        "duplicate or conflicting ruling ({turn}, {})",
                        tuple(&pair)
                    ));
                }
            }
            outcomes.insert(key, kind.into());
            if kind == "refused" {
                let player = number(&f, "player")?;
                if refused_players
                    .insert(
                        (turn, pair.clone(), player),
                        number::<i64>(&f, "legitimacy")?,
                    )
                    .is_some()
                {
                    issues.push(format!(
                        "duplicate refused respondent {}",
                        respondent(turn, &pair, player)
                    ));
                }
            }
        } else if event.starts_with("refusal cost ") {
            refusals.push(Refusal {
                player: number(&f, "player")?,
                turn: number(&f, "turn")?,
                intended: number(&f, "intended")?,
                applied: number(&f, "applied")?,
                legitimacy_before: number(&f, "legitimacy_before")?,
                legitimacy_after: number(&f, "legitimacy_after")?,
                pair: pair(&f)?,
            });
        } else if event.starts_with("decision ") {
            common::increment(
                &mut decisions,
                &format!("{}:{}", text(&f, "by")?, text(&f, "accept")?),
                1,
            );
        } else if event.starts_with("ai blocked ") {
            for key in ["gold", "legitimacy", "interval"] {
                common::increment(&mut blocked, key, number(&f, key)?);
            }
        } else if event.starts_with("ai weariness roster ") {
            let turn = number(&f, "turn")?;
            let mut roster = BTreeMap::new();
            let mut duplicate = false;
            if text(&f, "cities")? != "none" {
                for entry in text(&f, "cities")?.split(',') {
                    let (player, cities) =
                        entry.split_once(':').ok_or("invalid weariness roster")?;
                    duplicate |= roster.insert(player.parse()?, cities.parse()?).is_some();
                }
            }
            if rosters.insert(turn, roster) || duplicate {
                issues.push(format!("duplicate weariness roster on turn {turn}"));
            }
        } else if event.starts_with("ai weariness ") {
            weariness.push(Weariness {
                player: number(&f, "player")?,
                turn: number(&f, "turn")?,
                observed: number(&f, "observed")?,
                cities: number(&f, "cities")?,
            });
        }
    }
    let mut sources = Map::new();
    let mut spending: Map<String, Value> = Map::new();
    let mut player_turns: Ordered<i64, Vec<i64>> = Ordered::default();
    let mut pair_turns: Ordered<Pair, Vec<i64>> = Ordered::default();
    for (turn, request) in &requests.0 {
        let Some(cost) = costs
            .get(turn)
            .filter(|cost| cost.player == request.player && cost.pair == request.pair)
        else {
            issues.push(format!("request cost missing or mismatched on turn {turn}"));
            continue;
        };
        common::increment(&mut sources, &cost.source, 1);
        let total = spending
            .entry(&cost.source)
            .or_insert_with(|| json!({}))
            .as_object_mut()
            .unwrap();
        common::increment(total, "gold", cost.gold);
        common::increment(total, "legitimacy", cost.legitimacy);
        if request.power == 0 {
            return Err(format!("zero request power on turn {turn}").into());
        }
        let expected = (
            (100 + request.power - 1).div_euclid(request.power),
            (5 + request.power - 1).div_euclid(request.power),
        );
        if (cost.gold, cost.legitimacy) != expected
            || cost.gold_after < 0.0
            || cost.legitimacy_after < 0
        {
            issues.push(format!(
                "request cost violates recorded power or balance on turn {turn}"
            ));
        }
        if !matches!((cost.gold_before, cost.legitimacy_before), (Some(gold), Some(legitimacy))
            if gold.is_finite() && cost.gold_after.is_finite()
                && (gold - cost.gold_after - cost.gold as f64).abs() <= 1e-6
                && legitimacy - cost.legitimacy_after == cost.legitimacy && (30..=100).contains(&legitimacy))
        {
            issues.push(format!(
                "actual request balance delta missing or mismatched on turn {turn}"
            ));
        }
        if cost.source.starts_with("ai_") {
            if !ai.get(turn).is_some_and(|d| {
                d.player == request.player && d.pair == request.pair && d.source == cost.source
            }) {
                issues.push(format!("AI decision missing or mismatched on turn {turn}"));
            }
            let mut turns = player_turns
                .get(&request.player)
                .cloned()
                .unwrap_or_default();
            turns.push(*turn);
            player_turns.insert(request.player, turns);
            let mut turns = pair_turns.get(&request.pair).cloned().unwrap_or_default();
            turns.push(*turn);
            pair_turns.insert(request.pair.clone(), turns);
        }
    }
    for (turn, _) in &costs.0 {
        if requests.get(turn).is_none() {
            issues.push(format!("cost without request on turn {turn}"));
        }
    }
    for (turn, _) in &ai.0 {
        if requests.get(turn).is_none() {
            issues.push(format!("AI decision without request on turn {turn}"));
        }
    }
    for ((turn, pair), _) in &outcomes.0 {
        if requests.get(&(turn - 1)).is_none_or(|r| &r.pair != pair) {
            issues.push(format!("ruling without preceding request on turn {turn}"));
        }
    }
    for (turn, request) in &requests.0 {
        if outcomes.get(&(turn + 1, request.pair.clone())).is_none() && *turn < 201 {
            issues.push(format!(
                "request has no following-turn ruling on turn {turn}"
            ));
        }
    }
    let mut refusal_keys: BTreeMap<_, i64> = BTreeMap::new();
    for row in &refusals {
        let key = (row.turn, row.pair.clone(), row.player);
        *refusal_keys.entry(key.clone()).or_default() += 1;
        let request = requests.get(&(row.turn - 1));
        if refused_players.get(&key) != Some(&row.legitimacy_after)
            || !request.is_some_and(|r| r.pair == row.pair && row.intended == 15 * r.power)
        {
            issues.push(format!(
                "refusal cost missing or mismatched respondent {}",
                respondent(row.turn, &row.pair, row.player)
            ));
        }
        if row.legitimacy_before - row.legitimacy_after != row.applied
            || !(0..=100).contains(&row.legitimacy_before)
            || row.applied != row.intended.min(row.legitimacy_before)
        {
            issues.push(format!("invalid applied refusal loss on turn {}", row.turn));
        }
    }
    if refusal_keys.keys().ne(refused_players.keys()) || refusal_keys.values().any(|n| *n != 1) {
        issues.push("refusal costs must match each respondent exactly once".into());
    }
    if end_turns.len() != 1 {
        issues.push("missing or inconsistent shogunate cutoff".into());
    }
    if rosters
        .0
        .iter()
        .map(|(turn, _)| *turn)
        .collect::<BTreeSet<_>>()
        != (1..202).collect()
    {
        issues.push("weariness rosters do not cover all turns 1 through 201".into());
    }
    let mut samples: BTreeMap<(i64, i64), i64> = BTreeMap::new();
    for row in &weariness {
        *samples.entry((row.turn, row.player)).or_default() += 1;
        if rosters.get(&row.turn).and_then(|r| r.get(&row.player)) != Some(&row.cities)
            || !(0..=row.cities).contains(&row.observed)
        {
            issues.push(format!(
                "weariness sample mismatches roster ({}, {})",
                row.turn, row.player
            ));
        }
    }
    let expected_samples: BTreeSet<_> = rosters
        .0
        .iter()
        .flat_map(|(turn, roster)| roster.keys().map(|player| (*turn, *player)))
        .collect();
    if samples.keys().copied().collect::<BTreeSet<_>>() != expected_samples
        || samples.values().any(|n| *n != 1)
    {
        issues.push("weariness samples must match every roster entry exactly once".into());
    }
    let cutoff = (end_turns.len() == 1).then(|| *end_turns.first().unwrap());
    let shogun_after: Vec<_> = costs
        .0
        .iter()
        .filter(|(turn, cost)| {
            cost.source == "ai_shogun" && cutoff.is_some_and(|cutoff| *turn >= cutoff)
        })
        .map(|(turn, _)| *turn)
        .collect();
    if !shogun_after.is_empty() {
        issues.push(format!(
            "third-party shogun requests after cutoff: {shogun_after:?}"
        ));
    }
    if cutoff.is_some_and(|cutoff| {
        requests
            .0
            .iter()
            .any(|(turn, r)| *turn >= cutoff && r.power != 1)
    }) {
        issues.push("empowered request after shogunate cutoff".into());
    }
    let intervals = |mut turns: Vec<i64>| {
        turns.sort_unstable();
        turns.windows(2).map(|v| v[1] - v[0]).collect::<Vec<_>>()
    };
    let player_intervals: Map<String, Value> = player_turns
        .0
        .into_iter()
        .map(|(k, turns)| (k.to_string(), json!(intervals(turns))))
        .collect();
    let pair_intervals: Map<String, Value> = pair_turns
        .0
        .into_iter()
        .map(|(k, turns)| {
            (
                k.iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("_"),
                json!(intervals(turns)),
            )
        })
        .collect();
    if player_intervals
        .values()
        .chain(pair_intervals.values())
        .any(|v| {
            v.as_array()
                .unwrap()
                .iter()
                .any(|i| i.as_i64().unwrap() < 10)
        })
    {
        issues.push("AI request interval shorter than ten turns".into());
    }
    let mut completed = 0;
    let mut judge = Vec::new();
    let mut failures = Vec::new();
    let mut load_step = false;
    for step in &steps {
        let status = common::required(step, "status")?;
        let action = common::required(step, "action")?;
        if status == "OK" {
            let action = common::field(step, "action")?;
            if action.ends_with("autoplay")
                && common::field(step, "detail")?.contains("autoplay completed 50 turns")
            {
                completed += 1;
            }
            load_step |= action.ends_with(" load");
        }
        if action == "judge" {
            judge.push(step);
        }
        if status == "FAIL" {
            failures.push(step);
        }
    }
    let mut full =
        completed == 4 && judge.last().is_some_and(|s| s["status"] == "OK") && failures.is_empty();
    if full {
        let mut target = false;
        for step in &steps {
            if step["status"] == "OK"
                && common::field(step, "detail")?.contains("return=T+201 surviving majors=")
            {
                target = true;
                break;
            }
        }
        full = target;
    }
    if loads != 1 || load_step {
        full = false;
        issues.push("reloaded or missing gameplay log is not an uninterrupted measurement".into());
    }
    let result_path = artifact.join("result.json");
    if result_path.exists() {
        let result: Value = serde_json::from_str(&common::read_text(&result_path)?)?;
        if result["passed"] != true {
            full = false;
            issues.push("scenario result does not confirm success".into());
        }
        if common::truthy(&result["test_mode"]) {
            full = false;
            issues.push(
                "controlled clock/test mode is not an uninterrupted balance measurement".into(),
            );
        }
    } else {
        full = false;
        issues.push("scenario result is missing".into());
    }
    let mut outcome_counts = Map::new();
    for (_, outcome) in &outcomes.0 {
        common::increment(&mut outcome_counts, outcome, 1);
    }
    let refused_rows = events
        .iter()
        .filter(|event| event.starts_with("refused "))
        .count();
    if refused_rows != refusals.len() {
        issues.push("refusal losses missing or duplicated".into());
    }
    let unfinished = requests
        .0
        .iter()
        .filter(|(turn, request)| outcomes.get(&(turn + 1, request.pair.clone())).is_none())
        .count();
    Ok(json!({
        "artifact": common::artifact_name(artifact), "lua_sha256": lua_sha256,
        "complete_200_turns": full, "measurement_complete": full && issues.is_empty(), "issues": issues,
        "requests": requests.0.len(), "sources": sources, "outcomes": outcome_counts, "unfinished": unfinished,
        "refused_respondents": refused_rows, "decisions": decisions, "spending": spending,
        "refusal_loss_intended": refusals.iter().map(|r| r.intended).sum::<i64>(),
        "refusal_loss_applied": refusals.iter().map(|r| r.applied).sum::<i64>(),
        "blocked_candidate_visits": blocked, "player_intervals": player_intervals, "pair_intervals": pair_intervals,
        "shogunate_cutoff": cutoff, "shogun_after_cutoff": shogun_after,
        "costs": costs.0.iter().map(|(_, cost)| cost).collect::<Vec<_>>(),
        "ai_decisions": ai.0.iter().map(|(_, row)| row).collect::<Vec<_>>(), "failures": failures,
        "weariness_coverage": {"samples": weariness.len(), "observed_cities": weariness.iter().map(|r| r.observed).sum::<i64>(), "total_cities": weariness.iter().map(|r| r.cities).sum::<i64>()},
        "limitations": ["A single seed does not isolate the causal effect of fees, refusal penalties or cooldowns.", "Blocked counts are candidate visits before selecting a request, not distinct houses.", "Army and city contractions are sampled proxies, not opponent-attributed losses."]
    }))
}

pub fn markdown(data: &Value) -> String {
    let result = if data["measurement_complete"] == true {
        "PASS"
    } else {
        "未達（部分集計）"
    };
    let mut lines = vec![
        format!(
            "200ターンの裁定計測: {result}。実行: {}。",
            data["artifact"].as_str().unwrap()
        ),
        format!(
            "請求 {}件、成立 {}件、拒否のあった裁定 {}件。",
            data["requests"],
            common::counter(&data["outcomes"], "accepted"),
            common::counter(&data["outcomes"], "refused")
        ),
        format!(
            "拒否した当事者 {}人、失効 {}件、未裁定 {}件。",
            data["refused_respondents"],
            common::counter(&data["outcomes"], "void"),
            data["unfinished"]
        ),
        "".into(),
        "| 請求経路 | 件数 | 金の支出 | 正当性の支出 |".into(),
        "|---|---:|---:|---:|".into(),
    ];
    let mut sources: Vec<_> = data["sources"].as_object().unwrap().iter().collect();
    sources.sort_by_key(|(key, _)| *key);
    for (source, count) in sources {
        let name = match source.as_str() {
            "ai_party" => "疲弊した当事者AI",
            "ai_shogun" => "将軍家AIの仲介",
            "request" => "画面からの請求",
            name => name,
        };
        lines.push(format!(
            "| {name} | {count} | {} | {} |",
            data["spending"][source]["gold"], data["spending"][source]["legitimacy"]
        ));
    }
    let mut blocked: Vec<_> = data["blocked_candidate_visits"]
        .as_object()
        .unwrap()
        .iter()
        .collect();
    blocked.sort_by_key(|(key, _)| *key);
    lines.extend([
        "".into(),
        format!(
            "拒否による正当性の損失: 規則上 {}、実際の控除 {}（下限0）。",
            data["refusal_loss_intended"], data["refusal_loss_applied"]
        ),
        format!(
            "権限終了後の将軍家の仲介: {}件。",
            data["shogun_after_cutoff"].as_array().unwrap().len()
        ),
        format!(
            "費用不足・正当性不足・間隔待ちで進めなかった候補の延べ観測: {}。",
            blocked
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        "1つの対局では費用・不利益・間隔の因果効果を分離できない。比較の根拠が無い値は変更しない。"
            .into(),
        format!(
            "戦争疲弊による快適性の読み取り: 延べ{}/{}都市、AI家の観測{}回。",
            data["weariness_coverage"]["observed_cities"],
            data["weariness_coverage"]["total_cities"],
            data["weariness_coverage"]["samples"]
        ),
    ]);
    let issues = data["issues"].as_array().unwrap();
    if !issues.is_empty() {
        lines.extend(["".into(), "計測の不足:".into()]);
        lines.extend(
            issues
                .iter()
                .map(|issue| format!("- {}", issue.as_str().unwrap())),
        );
    }
    lines.join("\n") + "\n"
}
