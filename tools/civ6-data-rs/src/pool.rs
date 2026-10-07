//! The first, unapproved Banseiki character catalogue. No game registration.
use anyhow::{bail, ensure, Context, Result};
use serde_json::Value;
use std::collections::BTreeSet;

pub const DATA_PATH: &str = "documents/spec/data/banseiki-pool.json";
pub const CATEGORIES: [(&str, usize); 6] = [
    ("武将", 40),
    ("英傑武将", 8),
    ("軍師", 12),
    ("僧侶", 10),
    ("陰陽師", 10),
    ("一般ジョブUnit", 20),
];
pub const REGIONS: [&str; 16] = [
    "北海道",
    "東北",
    "関東",
    "中部",
    "近畿",
    "中国",
    "四国",
    "九州・琉球",
    "東アジア",
    "南・東南アジア",
    "西・中央アジア",
    "欧州",
    "アフリカ",
    "南北アメリカ",
    "地球圏",
    "宇宙",
];
const ERAS: [&str; 7] = ["古代", "中世", "戦国", "近世", "近現代", "未来", "宇宙"];
const TRAITS: [&str; 8] = [
    "武人", "鬼神", "策士", "魔神", "教者", "霊神", "凡人", "天才",
];

fn keys(value: &Value, expected: &[&str]) -> Result<()> {
    let object = value.as_object().context("expected JSON object")?;
    ensure!(
        object.len() == expected.len() && expected.iter().all(|k| object.contains_key(*k)),
        "missing or unknown field"
    );
    Ok(())
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    let s = value[key]
        .as_str()
        .with_context(|| format!("{key}: expected string"))?;
    ensure!(
        !s.is_empty() && s.trim() == s && !s.chars().any(char::is_control),
        "{key}: empty, padded or control characters"
    );
    Ok(s)
}

fn date(s: &str) -> bool {
    let Some((year, tail)) = s.split_once('-') else {
        return false;
    };
    let Some((month, day)) = tail.split_once('-') else {
        return false;
    };
    if year.len() != 4 || month.len() != 2 || day.len() != 2 {
        return false;
    }
    let (Ok(y), Ok(m), Ok(d)) = (
        year.parse::<u32>(),
        month.parse::<u32>(),
        day.parse::<u32>(),
    ) else {
        return false;
    };
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let max = match m {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    };
    y > 0 && d > 0 && d <= max
}

/// Fail with the row's ID and field context; this checks records, not historical truth.
pub fn validate(data: &Value) -> Result<()> {
    keys(
        data,
        &["schema_version", "status", "target_size", "characters"],
    )?;
    ensure!(
        data["schema_version"].as_u64() == Some(1),
        "schema_version must be 1"
    );
    ensure!(
        text(data, "status")? == "方向確認待ち",
        "first batch must remain a proposal"
    );
    ensure!(
        data["target_size"].as_u64() == Some(300),
        "target_size must be the proposed 300"
    );
    let rows = data["characters"]
        .as_array()
        .context("characters must be an array")?;
    ensure!(
        rows.len() == 100,
        "first batch must contain 100 characters, got {}",
        rows.len()
    );
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut leaders = BTreeSet::new();
    let mut regions = BTreeSet::new();
    let mut eras = BTreeSet::new();
    let mut counts = [0usize; 6];
    let mut historical = 0;
    let mut candidates = 0;
    for (index, row) in rows.iter().enumerate() {
        let result = (|| -> Result<()> {
            keys(
                row,
                &[
                    "id",
                    "name",
                    "reading",
                    "origin",
                    "era",
                    "region",
                    "faction",
                    "category",
                    "trait",
                    "leader_candidate",
                    "existing_leader_id",
                    "description",
                    "sources",
                ],
            )?;
            let id = text(row, "id")?;
            let origin = text(row, "origin")?;
            let prefix = match origin {
                "新規" => "JAC_POOL_O",
                "史実" => {
                    historical += 1;
                    "JAC_POOL_H"
                }
                _ => bail!("origin: expected 新規 or 史実"),
            };
            let suffix = id
                .strip_prefix(prefix)
                .context("id does not match origin")?;
            ensure!(
                suffix.len() == 3 && suffix.bytes().all(|b| b.is_ascii_digit()) && suffix != "000",
                "id: expected three nonzero decimal digits"
            );
            ensure!(ids.insert(id), "duplicate id {id}");
            let name = text(row, "name")?;
            ensure!(
                names.insert(name.replace([' ', '　', '・'], "")),
                "duplicate name {name}"
            );
            let reading = text(row, "reading")?;
            ensure!(
                reading
                    .chars()
                    .any(|c| ('ぁ'..='ゖ').contains(&c) || ('ァ'..='ヺ').contains(&c))
                    && reading.chars().all(|c| ('ぁ'..='ゖ').contains(&c)
                        || ('ァ'..='ヺ').contains(&c)
                        || "ー・ ".contains(c)),
                "reading must be kana"
            );
            let era = text(row, "era")?;
            ensure!(ERAS.contains(&era), "unknown era {era}");
            eras.insert(era);
            let region = text(row, "region")?;
            ensure!(REGIONS.contains(&region), "unknown region {region}");
            regions.insert(region);
            text(row, "faction")?;
            text(row, "description")?;
            let category = text(row, "category")?;
            let position = CATEGORIES
                .iter()
                .position(|(c, _)| *c == category)
                .context("unknown category")?;
            counts[position] += 1;
            ensure!(TRAITS.contains(&text(row, "trait")?), "unknown trait");
            let candidate = row["leader_candidate"]
                .as_bool()
                .context("leader_candidate must be boolean")?;
            candidates += usize::from(candidate);
            if !row["existing_leader_id"].is_null() {
                let leader = text(row, "existing_leader_id")?;
                ensure!(
                    leader.starts_with("LEADER_JAC_")
                        && leader
                            .bytes()
                            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_'),
                    "invalid existing_leader_id"
                );
                ensure!(leaders.insert(leader), "duplicate existing_leader_id");
                ensure!(
                    origin == "史実",
                    "new fiction must not alias a historical leader"
                );
            }
            let sources = row["sources"]
                .as_array()
                .context("sources must be an array")?;
            ensure!(
                (origin == "史実") != sources.is_empty(),
                "historical rows need sources; new rows must have none"
            );
            for source in sources {
                keys(
                    source,
                    &["title", "publisher", "url", "verified_on", "facts"],
                )?;
                text(source, "title")?;
                text(source, "publisher")?;
                text(source, "facts")?;
                let url = text(source, "url")?;
                let address = url
                    .strip_prefix("https://")
                    .context("source URL must use https")?;
                let host = address.split(['/', '?', '#']).next().unwrap_or("");
                ensure!(
                    host.contains('.')
                        && !host.contains('@')
                        && !url.chars().any(char::is_whitespace),
                    "invalid source URL"
                );
                ensure!(
                    date(text(source, "verified_on")?),
                    "invalid verification date"
                );
            }
            Ok(())
        })();
        result.with_context(|| format!("character {} ({})", index + 1, row["id"]))?;
    }
    ensure!(
        historical == 20,
        "first batch must contain 20 historical and 80 new characters"
    );
    ensure!(
        candidates >= 2,
        "need at least two distinct leader candidates"
    );
    for (i, (category, expected)) in CATEGORIES.iter().enumerate() {
        ensure!(
            counts[i] == *expected,
            "{category}: expected {expected}, got {}",
            counts[i]
        );
    }
    ensure!(
        REGIONS.iter().all(|r| regions.contains(r)),
        "first batch must cover all 16 regions"
    );
    ensure!(
        ERAS.iter().all(|e| eras.contains(e)),
        "first batch must cover all seven eras"
    );
    Ok(())
}
