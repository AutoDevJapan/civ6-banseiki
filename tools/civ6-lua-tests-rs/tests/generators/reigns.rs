//! Rust port of the reign generator used by the original file-currentness gate.
//! Includes its seeded MT19937 search, rather than copying rendered MOD files.

use civ6_lua_tests_rs::read_repo;
use serde_json::Value as Json;
use std::collections::{BTreeMap, HashSet};

const SPEEDS: [&str; 5] = [
    "GAMESPEED_ONLINE",
    "GAMESPEED_QUICK",
    "GAMESPEED_STANDARD",
    "GAMESPEED_EPIC",
    "GAMESPEED_MARATHON",
];
const MULTIPLIERS: [i64; 5] = [50, 67, 100, 150, 300];
const OFFICIAL: [&str; 9] = [
    "ERA_ANCIENT",
    "ERA_CLASSICAL",
    "ERA_MEDIEVAL",
    "ERA_RENAISSANCE",
    "ERA_INDUSTRIAL",
    "ERA_MODERN",
    "ERA_ATOMIC",
    "ERA_INFORMATION",
    "ERA_FUTURE",
];
const LATE: [(&str, i64); 5] = [
    ("ERA_INDUSTRIAL", 1725),
    ("ERA_MODERN", 1890),
    ("ERA_ATOMIC", 1945),
    ("ERA_INFORMATION", 1995),
    ("ERA_FUTURE", 2020),
];
const LANGS: [(&str, &str); 3] = [("ja", "ja_JP"), ("en", "en_US"), ("ru", "ru_RU")];
type Run = (i64, i64);
type Options = Vec<(Vec<Run>, f64)>;

struct Mt19937 {
    state: [u32; 624],
    index: usize,
}
impl Mt19937 {
    fn seed_zero() -> Self {
        // CPython's integer seed 0 uses init_by_array([0]), not init_genrand(0).
        let mut state = [0u32; 624];
        state[0] = 19650218;
        for i in 1..624 {
            state[i] = 1812433253u32
                .wrapping_mul(state[i - 1] ^ (state[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        let mut i = 1;
        for _ in 0..624 {
            state[i] ^= (state[i - 1] ^ (state[i - 1] >> 30)).wrapping_mul(1664525);
            i += 1;
            if i >= 624 {
                state[0] = state[623];
                i = 1;
            }
        }
        for _ in 0..623 {
            state[i] = (state[i] ^ (state[i - 1] ^ (state[i - 1] >> 30)).wrapping_mul(1566083941))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= 624 {
                state[0] = state[623];
                i = 1;
            }
        }
        state[0] = 0x80000000;
        Self { state, index: 624 }
    }
    fn next(&mut self) -> u32 {
        if self.index == 624 {
            for i in 0..624 {
                let y = (self.state[i] & 0x80000000) | (self.state[(i + 1) % 624] & 0x7fffffff);
                self.state[i] = self.state[(i + 397) % 624]
                    ^ (y >> 1)
                    ^ if y & 1 != 0 { 0x9908b0df } else { 0 };
            }
            self.index = 0;
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c5680;
        y ^= (y << 15) & 0xefc60000;
        y ^= y >> 18;
        y
    }
    fn random(&mut self) -> f64 {
        let a = (self.next() >> 5) as u64;
        let b = (self.next() >> 6) as u64;
        (a * 67108864 + b) as f64 / 9007199254740992.0
    }
}

fn round(value: f64) -> i64 {
    value.round_ties_even() as i64
}
fn chunked(turns: i64, months: i64, chunks: i64, mut high: bool) -> Option<Vec<Run>> {
    let cuts: Vec<_> = (0..=chunks)
        .map(|i| round((turns * i) as f64 / chunks as f64))
        .collect();
    let mut runs: Vec<Run> = Vec::new();
    for pair in cuts.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        let span = round((months * end) as f64 / turns as f64)
            - round((months * start) as f64 / turns as f64);
        let low = span.div_euclid(end - start);
        let extra = span.rem_euclid(end - start);
        if low < 1 {
            return None;
        }
        let parts = if high {
            [(low + 1, extra), (low, end - start - extra)]
        } else {
            [(low, end - start - extra), (low + 1, extra)]
        };
        for run in parts {
            if run.1 == 0 {
                continue;
            }
            if runs.last().is_some_and(|last| last.0 == run.0) {
                runs.last_mut().unwrap().1 += run.1;
            } else {
                runs.push(run);
            }
        }
        high = !high;
    }
    Some(runs)
}
fn gap(runs: &[Run], turns: i64, months: i64) -> f64 {
    let (mut total, mut turn, mut worst) = (0, 0, 0f64);
    for &(increment, count) in runs {
        for _ in 0..count {
            total += increment;
            turn += 1;
            worst = worst.max((total as f64 - (months * turn) as f64 / turns as f64).abs());
        }
    }
    worst
}
fn put(options: &mut Options, runs: Vec<Run>, gap: f64, replace: bool) {
    if let Some(index) = options.iter().position(|(old, _)| old == &runs) {
        if replace {
            options[index].1 = gap;
        }
    } else {
        options.push((runs, gap));
    }
}
fn span_options(turns: i64, months: i64, first_above: i64) -> Options {
    let pace = months as f64 / turns as f64;
    let mut out = Vec::new();
    if months % turns == 0 && months / turns > first_above {
        put(&mut out, vec![(months / turns, turns)], 0.0, true);
    }
    for first in first_above + 1..(pace as i64) + 4 {
        for count in 1..turns {
            let remainder = (months - first * count).rem_euclid(turns - count);
            let later = (months - first * count).div_euclid(turns - count);
            if remainder == 0 && later >= 1 && later != first && (later as f64 - pace).abs() < 4.0 {
                put(
                    &mut out,
                    vec![(first, count), (later, turns - count)],
                    (first as f64 - pace).abs() * count as f64,
                    true,
                );
            }
        }
    }
    for chunks in 2..=2.max(turns / 8) {
        for high in [true, false] {
            if let Some(runs) = chunked(turns, months, chunks, high) {
                if runs[0].0 > first_above {
                    let g = gap(&runs, turns, months);
                    put(&mut out, runs, g, false);
                }
            }
        }
    }
    out.retain(|(runs, _)| runs.iter().map(|r| r.0).max().unwrap() <= 12);
    out.sort_by(|a, b| a.1.total_cmp(&b.1));
    out
}
fn rows(run: Run, used: &HashSet<Run>) -> Option<Vec<Run>> {
    if !used.contains(&run) {
        return Some(vec![run]);
    }
    let (inc, count) = run;
    for part in (1..=count / 2).rev() {
        if part != count - part
            && !used.contains(&(inc, part))
            && !used.contains(&(inc, count - part))
        {
            return Some(vec![(inc, part), (inc, count - part)]);
        }
    }
    None
}
fn try_calendar(
    starts: &[i64],
    months: &[i64],
    multiplier: i64,
    rng: &mut Mt19937,
    noise: f64,
    cache: &mut BTreeMap<(i64, i64, i64), Options>,
) -> Option<Vec<Run>> {
    let mut used = HashSet::new();
    let mut result = Vec::new();
    let mut current: Option<Run> = None;
    let mut slack = 0;
    for k in 0..starts.len() - 1 {
        let mut candidates = Vec::new();
        let early_values: Vec<_> = if k + 2 < starts.len() {
            (0..=2).collect()
        } else {
            vec![0]
        };
        for early in early_values {
            let key = (
                starts[k + 1] - starts[k],
                months[k + 1] - months[k] + slack - early,
                slack,
            );
            let opts = cache
                .entry(key)
                .or_insert_with(|| span_options(key.0, key.1, key.2));
            for (runs, gap) in opts {
                candidates.push((
                    *gap + 6.0 * runs.len() as f64 + 2.0 * early as f64 + rng.random() * noise,
                    early,
                    runs.clone(),
                ));
            }
        }
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut chosen = None;
        for (_, early, mut runs) in candidates {
            let mut closing = Vec::new();
            if let Some(last) = current {
                if last.0 == runs[0].0 {
                    runs[0].1 += last.1;
                } else {
                    closing.push(last);
                }
            }
            closing.extend_from_slice(&runs[..runs.len() - 1]);
            let mut taken = used.clone();
            let mut added = Vec::new();
            let mut failed = false;
            for run in closing {
                if let Some(split) = rows(run, &taken) {
                    taken.extend(split.iter().copied());
                    added.extend(split);
                } else {
                    failed = true;
                    break;
                }
            }
            let last = *runs.last().unwrap();
            if !failed && rows(last, &taken).is_some() {
                chosen = Some((taken, added, last, early));
                break;
            }
        }
        let (taken, added, last, early) = chosen?;
        used = taken;
        result.extend(added);
        current = Some(last);
        slack = early;
    }
    let current = current.unwrap();
    let tail = (
        round(400.0 / multiplier as f64),
        10000 - (starts[starts.len() - 1] - 1),
    );
    let tail_runs = if current.0 == tail.0 {
        vec![(tail.0, current.1 + tail.1)]
    } else {
        vec![current, tail]
    };
    for run in tail_runs {
        let split = rows(run, &used)?;
        used.extend(split.iter().copied());
        result.extend(split);
    }
    Some(result)
}

#[derive(Clone)]
struct Era {
    kind: String,
    id: String,
    length: Option<i64>,
    index: usize,
}
fn era_rows(data: &Json) -> Vec<Era> {
    let mut all = vec![data["start"].clone()];
    all.extend_from_slice(data["reigns"].as_array().unwrap());
    let mut starts = vec![1];
    starts.extend(data["reigns"].as_array().unwrap().iter().map(|r| {
        1 + ((r["year"].as_i64().unwrap() - 1534) * 12 + r["month"].as_i64().unwrap() - 1) / 4
    }));
    all.iter()
        .enumerate()
        .map(|(i, r)| {
            let id = r["id"].as_str().unwrap().to_string();
            let length = starts.get(i + 1).map(|next| next - starts[i]);
            assert!(
                length.is_none_or(|n| n >= 2),
                "{id}: era lasts fewer than two turns"
            );
            Era {
                kind: OFFICIAL
                    .get(i)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("ERA_JAC_{id}")),
                id,
                length,
                index: i + 1,
            }
        })
        .collect()
}
fn engine_turns(eras: &[Era], multiplier: i64) -> Vec<i64> {
    let mut turn = 1;
    eras.iter()
        .filter_map(|era| era.length)
        .map(|length| {
            turn += (length * multiplier / 100).max(2);
            turn
        })
        .collect()
}
fn calendars(data: &Json, eras: &[Era]) -> BTreeMap<String, Vec<Run>> {
    let mut months = vec![0];
    months.extend(
        data["reigns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (r["year"].as_i64().unwrap() - 1534) * 12 + r["month"].as_i64().unwrap() - 1),
    );
    let mut out = BTreeMap::new();
    for (speed, multiplier) in SPEEDS.into_iter().zip(MULTIPLIERS) {
        if speed == "GAMESPEED_STANDARD" {
            out.insert(speed.into(), vec![(4, 10000)]);
            continue;
        }
        let mut starts = vec![1];
        starts.extend(engine_turns(eras, multiplier));
        let mut rng = Mt19937::seed_zero();
        let mut cache = BTreeMap::new();
        let mut solution = None;
        for attempt in 0..500 {
            if let Some(rows) = try_calendar(
                &starts,
                &months,
                multiplier,
                &mut rng,
                if attempt == 0 {
                    0.0
                } else {
                    1.0 + attempt as f64 / 20.0
                },
                &mut cache,
            ) {
                solution = Some(rows);
                break;
            }
        }
        out.insert(
            speed.into(),
            solution.unwrap_or_else(|| {
                panic!("{speed}: no calendar avoids a repeated GameSpeed_Turns key")
            }),
        );
    }
    out
}
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn build(data: &Json, eras: &[Era]) -> (String, String) {
    let reigns = data["reigns"].as_array().unwrap();
    let engine: Vec<_> = MULTIPLIERS.map(|m| engine_turns(eras, m)).into();
    let mut sql=concat!("-- Generated by tools/generate_sengoku_reigns.py from documents/spec/data/sengoku-reigns.json.\n",
        "-- The 改元 after the start (天文) in date order, each with its engine era (Data/JAC_Sengoku_Eras.sql); the\n",
        "-- ledger judges when that era starts. Trailing comment: that turn at the online / quick / standard / epic /\n",
        "-- marathon speeds.\n",
        "INSERT INTO JAC_SengokuReigns (ReignType, Year, Month, Name, Announcement, Notification, ForcedAge, EraType) VALUES\n").to_string();
    for (i, r) in reigns.iter().enumerate() {
        let id = r["id"].as_str().unwrap();
        let key = format!("LOC_JAC_REIGN_{id}");
        let forced = r
            .get("forced_age")
            .and_then(Json::as_str)
            .map(quote)
            .unwrap_or("NULL".into());
        let values = [
            quote(&format!("REIGN_{id}")),
            r["year"].to_string(),
            r["month"].to_string(),
            quote(&format!("{key}_NAME")),
            quote(&format!("{key}_ANNOUNCEMENT")),
            quote(&format!("{key}_NOTIFICATION")),
            forced,
            quote(&eras[i + 1].kind),
        ]
        .join(", ");
        sql.push_str(&format!(
            "  ({values}){} -- {} {}\n",
            if i + 1 < reigns.len() { "," } else { ";" },
            r["ja"].as_str().unwrap(),
            engine
                .iter()
                .map(|turns| turns[i].to_string())
                .collect::<Vec<_>>()
                .join(" / ")
        ));
    }
    let mut text: Vec<(String, [String; 3])> = Vec::new();
    let mut previous = &data["start"];
    text.push((
        format!("LOC_JAC_REIGN_{}_NAME", previous["id"].as_str().unwrap()),
        LANGS.map(|(short, _)| previous[short].as_str().unwrap().to_string()),
    ));
    for r in reigns {
        let key = format!("LOC_JAC_REIGN_{}", r["id"].as_str().unwrap());
        text.push((
            format!("{key}_NAME"),
            LANGS.map(|(short, _)| r[short].as_str().unwrap().into()),
        ));
        let mut announcements = [String::new(), String::new(), String::new()];
        let mut notices = announcements.clone();
        for (i, (short, _)) in LANGS.iter().enumerate() {
            let p = previous[*short].as_str().unwrap();
            let n = r[*short].as_str().unwrap();
            let y = r["year"].as_i64().unwrap();
            announcements[i] = match i {
                0 => format!("改元[NEWLINE]{p}から{n}へ（{y}年）"),
                1 => format!("New era name:[NEWLINE]from {p} to {n} ({y})"),
                _ => format!("Смена девиза правления:[NEWLINE]с {p} на {n} ({y})"),
            };
            notices[i] = match i {
                0 => format!("改元：{n}の世"),
                1 => format!("New era name: {n} Era"),
                _ => format!("Смена девиза: эпоха {n}"),
            };
        }
        text.push((format!("{key}_ANNOUNCEMENT"), announcements));
        text.push((format!("{key}_NOTIFICATION"), notices));
        previous = r;
    }
    for r in std::iter::once(&data["start"]).chain(reigns.iter()) {
        text.push((
            format!("LOC_JAC_ERA_{}_NAME", r["id"].as_str().unwrap()),
            [
                format!("{}の世", r["ja"].as_str().unwrap()),
                format!("{} Era", r["en"].as_str().unwrap()),
                format!("Эпоха {}", r["ru"].as_str().unwrap()),
            ],
        ));
    }
    let mut xml=concat!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n","<!-- Generated by tools/generate_sengoku_reigns.py from documents/spec/data/sengoku-reigns.json. -->\n","<GameData>\n  <LocalizedText>\n").to_string();
    for (key, translations) in text {
        for ((_, lang), value) in LANGS.into_iter().zip(translations) {
            xml.push_str(&format!(
                "    <Row Tag=\"{key}\" Language=\"{lang}\"><Text>{}</Text></Row>\n",
                escape(&value)
            ));
        }
    }
    xml.push_str("  </LocalizedText>\n</GameData>\n");
    (sql, xml)
}

fn build_eras(data: &Json, eras: &[Era]) -> String {
    let mut out = include_str!("../../fixtures/reigns-generation/eras-header.txt")
        .trim_end_matches('\n')
        .to_string();
    for era in &eras[..9] {
        let name = quote(&format!("LOC_JAC_ERA_{}_NAME", era.id));
        out.push_str(&format!(
            "\nUPDATE Eras SET Name={name}, Description={name} WHERE EraType={};",
            quote(&era.kind)
        ));
    }
    out.push_str(
        include_str!("../../fixtures/reigns-generation/eras-new.txt").trim_end_matches('\n'),
    );
    out.push_str("\nINSERT INTO Types (Type, Kind) VALUES\n");
    out.push_str(
        &eras[9..]
            .iter()
            .map(|era| format!("  ({}, 'KIND_ERA')", quote(&era.kind)))
            .collect::<Vec<_>>()
            .join(",\n"),
    );
    out.push(';');
    let columns="WarmongerPoints, GreatPersonBaseCost, EraTechBackgroundTexture, EraCivicBackgroundTexture, WarmongerLevelDescription, EmbarkedUnitStrength, EraTechBackgroundTextureOffsetX, EraCivicBackgroundTextureOffsetX, TechTreeLayoutMethod";
    for era in &eras[9..] {
        let kind = quote(&era.kind);
        let name = quote(&format!("LOC_JAC_ERA_{}_NAME", era.id));
        out.push_str(&format!("\nINSERT INTO Eras (EraType, Name, Description, ChronologyIndex, {columns}) SELECT {kind}, {name}, {name}, {}, {columns} FROM Eras WHERE EraType='ERA_FUTURE';",era.index));
        out.push_str(&format!("\nINSERT INTO Eras_XP1 (EraType, LiberatedEnvoys) SELECT {kind}, LiberatedEnvoys FROM Eras_XP1 WHERE EraType='ERA_FUTURE';"));
        out.push_str(&format!("\nINSERT INTO Eras_XP2 (EraType, GrievanceDecayRate, TensionDecayRate, TradeRouteMinimumEndTurnChange, EraScoreThresholdShift) SELECT {kind}, GrievanceDecayRate, TensionDecayRate, TradeRouteMinimumEndTurnChange, EraScoreThresholdShift FROM Eras_XP2 WHERE EraType='ERA_FUTURE';"));
    }
    out.push_str("\n\n-- Era lengths: the standard turns from the reign's 改元 to the next (the calendar turn containing each month).\n-- Official 40/60 (DLC/Expansion2/Data/Expansion1_Eras.xml:3-12).");
    for era in eras {
        let length = era.length.map(|n| n.to_string()).unwrap_or("NULL".into());
        out.push_str(&format!("\nUPDATE Eras_XP1 SET GameEraMinimumTurns={length}, GameEraMaximumTurns={length} WHERE EraType={}; -- {}",quote(&era.kind),era.id));
    }
    out.push_str(
        include_str!("../../fixtures/reigns-generation/eras-late.txt").trim_end_matches('\n'),
    );
    out.push_str(&format!("\nDELETE FROM Types WHERE Type IN (SELECT GreatPersonIndividualType FROM GreatPersonIndividuals WHERE EraType IN ({}));",LATE.map(|(kind,_)|quote(kind)).join(", ")));
    out.push_str("\n-- The great works of those eras with no great person (the industrial artifacts GREATWORK_ARTIFACT_21-25,\n-- Base/Assets/Gameplay/Data/GreatWorks.xml) go to the reign of the official era's start year\n-- (LATE_OFFICIAL_ERA_YEARS in the generator).");
    for (old, year) in LATE {
        let mut current = &eras[0].kind;
        for (era, reign) in eras.iter().skip(1).zip(data["reigns"].as_array().unwrap()) {
            if (
                reign["year"].as_i64().unwrap(),
                reign["month"].as_i64().unwrap(),
            ) <= (year, 1)
            {
                current = &era.kind;
            }
        }
        out.push_str(&format!(
            "\nUPDATE GreatWorks SET EraType={} WHERE EraType={}; -- {year}",
            quote(current),
            quote(old)
        ));
    }
    out.push('\n');
    out
}
fn build_calendar(calendars: &BTreeMap<String, Vec<Run>>) -> String {
    let mut out = include_str!("../../fixtures/reigns-generation/calendar-header.txt")
        .trim_end_matches('\n')
        .to_string();
    for speed in [
        "GAMESPEED_STANDARD",
        "GAMESPEED_QUICK",
        "GAMESPEED_ONLINE",
        "GAMESPEED_EPIC",
        "GAMESPEED_MARATHON",
    ] {
        for &(inc, turns) in &calendars[speed] {
            out.push_str(&format!("\n\t\t<Row>\n\t\t\t<GameSpeedType>{speed}</GameSpeedType>\n\t\t\t<MonthIncrement>{inc}</MonthIncrement>\n\t\t\t<TurnsPerIncrement>{turns}</TurnsPerIncrement>\n\t\t</Row>"));
        }
    }
    out.push_str("\n\t</GameSpeed_Turns>\n</GameInfo>\n");
    out
}
pub fn outputs() -> BTreeMap<&'static str, String> {
    let data: Json =
        serde_json::from_str(&read_repo("documents/spec/data/sengoku-reigns.json")).unwrap();
    let eras = era_rows(&data);
    let calendar = calendars(&data, &eras);
    let (sql, xml) = build(&data, &eras);
    BTreeMap::from([
        ("Data/JAC_Sengoku_Reigns.sql", sql),
        ("Text/JAC_Sengoku_Reigns_Text.xml", xml),
        ("Data/JAC_Sengoku_Eras.sql", build_eras(&data, &eras)),
        ("Data/JAC_Sengoku_Calendar.xml", build_calendar(&calendar)),
    ])
}
