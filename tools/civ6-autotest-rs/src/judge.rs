//! Judge Civ VI log bytes produced after a captured baseline.
use crate::{time, Error, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

pub const LOG_NAMES: [&str; 3] = ["Lua.log", "Database.log", "Modding.log"];
const BUNDLE0_ID: &str = "dfa0717e-4d0b-4370-b693-27fc6758571b";
const GROWTH_ID: &str = "7696e937-2044-4f3c-8e13-4e27632e2710";
const JAPAN_MAP_ID: &str = "403be889-1455-4de7-967b-ad54c0ff3431";
const OWN_MOD_IDS: &[&str] = include!(concat!(env!("OUT_DIR"), "/own_mod_ids.rs"));
fn re(s: &str) -> Regex {
    Regex::new(s).expect("constant regex")
}
fn sha(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}

pub fn default_log_dir() -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var_os("LOCALAPPDATA")
            .ok_or_else(|| Error::input("LOCALAPPDATA is unavailable; pass --logs"))?,
    )
    .join("Firaxis Games")
    .join("Sid Meier's Civilization VI")
    .join("Logs"))
}
#[cfg(unix)]
pub fn file_inode_from_handle(file: &File) -> Result<u64> {
    use std::os::unix::fs::MetadataExt;
    Ok(file.metadata()?.ino())
}
#[cfg(windows)]
pub fn file_inode_from_handle(file: &File) -> Result<u64> {
    use std::{ffi::c_void, os::windows::io::AsRawHandle};
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct FileInfo {
        attributes: u32,
        created: FileTime,
        accessed: FileTime,
        written: FileTime,
        volume: u32,
        size_high: u32,
        size_low: u32,
        links: u32,
        index_high: u32,
        index_low: u32,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetFileInformationByHandle(handle: *mut c_void, info: *mut FileInfo) -> i32;
    }
    let mut info = FileInfo::default();
    let ok = unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) };
    if ok == 0 {
        return Err(Error::from(std::io::Error::last_os_error()));
    }
    Ok((u64::from(info.index_high) << 32) | u64::from(info.index_low))
}
#[cfg(not(any(unix, windows)))]
pub fn file_inode_from_handle(_: &File) -> Result<u64> {
    Err(Error::input("File index unavailable"))
}
pub fn file_inode(path: &Path) -> Result<u64> {
    file_inode_from_handle(&File::open(path)?)
}
pub fn read_log_handle(file: &mut File) -> Result<(Vec<u8>, u64)> {
    let mut contents = Vec::new();
    file.read_to_end(&mut contents)?;
    Ok((contents, file_inode_from_handle(file)?))
}
pub fn read_log_file(path: &Path) -> Result<(Vec<u8>, u64)> {
    read_log_handle(&mut File::open(path)?)
}

pub fn snapshot_logs(log_dir: &Path) -> Result<Value> {
    let mut files = serde_json::Map::new();
    for name in LOG_NAMES {
        let path = log_dir.join(name);
        if path.exists() {
            let mut file = File::open(&path)?;
            let size = file.metadata()?.len();
            let n = size.min(256) as usize;
            let mut prefix = vec![0; n];
            file.read_exact(&mut prefix)?;
            file.seek(SeekFrom::Start(size - n as u64))?;
            let mut tail = vec![0; n];
            file.read_exact(&mut tail)?;
            files.insert(name.into(), json!({"offset":size,"inode":file_inode_from_handle(&file)?,
                "prefix_length":n,"prefix_sha256":sha(&prefix),"tail_length":n,"tail_sha256":sha(&tail)}));
        } else {
            files.insert(
                name.into(),
                json!({"offset":0,"inode":null,"prefix_length":0,
                "prefix_sha256":null,"tail_length":0,"tail_sha256":null}),
            );
        }
    }
    Ok(json!({"created_at":time::utc_now(),"log_dir":log_dir.to_string_lossy(),"files":files}))
}
pub fn log_marker_replaced(contents: &[u8], mark: &Value, inode: u64) -> bool {
    let Some(offset) = mark
        .get("offset")
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
    else {
        return true;
    };
    if offset > contents.len() {
        return true;
    }
    if mark
        .get("inode")
        .is_some_and(|v| !v.is_null() && v.as_u64() != Some(inode))
    {
        return true;
    }
    let prefix_len = mark
        .get("prefix_length")
        .and_then(Value::as_u64)
        .unwrap_or(offset.min(256) as u64) as usize;
    if prefix_len > offset || prefix_len > contents.len() {
        return true;
    }
    if mark
        .get("prefix_sha256")
        .and_then(Value::as_str)
        .is_some_and(|old| old != sha(&contents[..prefix_len]))
    {
        return true;
    }
    let tail_len = mark.get("tail_length").and_then(Value::as_u64).unwrap_or(0) as usize;
    if tail_len > offset {
        return true;
    }
    mark.get("tail_sha256")
        .and_then(Value::as_str)
        .is_some_and(|old| old != sha(&contents[offset - tail_len..offset]))
}
pub fn slice_log_bytes(contents: &[u8], mark: &Value, inode: u64) -> String {
    let start = if log_marker_replaced(contents, mark, inode) {
        0
    } else {
        mark["offset"].as_u64().unwrap_or(0) as usize
    };
    let bytes = &contents[start..];
    String::from_utf8_lossy(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes)).into_owned()
}
pub fn read_since(log_dir: &Path, baseline: &Value) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    for name in LOG_NAMES {
        let path = log_dir.join(name);
        if !path.is_file() {
            return Err(Error::input(format!(
                "Required Civ6 log is missing: {}",
                path.display()
            )));
        }
        let mark = baseline
            .get("files")
            .and_then(|v| v.get(name))
            .ok_or_else(|| Error::input(format!("Missing log baseline: {name}")))?;
        let (contents, inode) = read_log_file(&path)?;
        result.insert(name.into(), slice_log_bytes(&contents, mark, inode));
    }
    Ok(result)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Check {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}
impl Check {
    fn new(name: &str, passed: bool, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed,
            detail: detail.into(),
        }
    }
}
fn own_path(text: &str) -> bool {
    re(r"(?i)\bJAC[_\\/.-]|JAC_PoC").is_match(text)
        || OWN_MOD_IDS
            .iter()
            .any(|id| text.to_ascii_lowercase().contains(id))
}
fn error_context(lines: &[&str], i: usize) -> String {
    let mut out = vec![lines[i]];
    for line in lines.iter().skip(i + 1).take(8) {
        if line.chars().next().is_some_and(char::is_whitespace)
            || line.trim().to_lowercase().starts_with("stack traceback")
        {
            out.push(line)
        } else {
            break;
        }
    }
    out.join("\n")
}
fn error_block(lines: &[&str], i: usize) -> String {
    let mut out = vec![lines[i]];
    let marker = re(r"(?i)\b(?:ERROR|FAILED|FAILURE|EXCEPTION)\b");
    let prefix = re(r"^\[[\d.]+\](?:\s+\[[^]]+\])?:?\s*");
    let continuation =
        re(r"(?i)^(?:from file\b|in XMLSerializer\b|while executing\b|near\b|at line\b)");
    for line in lines.iter().skip(i + 1) {
        if marker.is_match(line) {
            break;
        }
        let msg = prefix.replace(line, "");
        let msg = msg.trim();
        if line.chars().next().is_some_and(char::is_whitespace)
            || re("(?i)\\bfrom file\\b").is_match(msg)
            || continuation.is_match(msg)
        {
            out.push(line)
        } else {
            break;
        }
    }
    out.join("\n")
}
fn database_errors(content: &str) -> Vec<String> {
    let lines: Vec<_> = content.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter(|(i, l)| l.to_uppercase().contains("ERROR") && own_path(&error_block(&lines, *i)))
        .map(|(_, l)| (*l).into())
        .collect()
}
fn related_errors(content: &str) -> Vec<String> {
    let lines: Vec<_> = content.lines().collect();
    let marker = re(r"(?i)\b(?:ERROR|FAILED|FAILURE|EXCEPTION)\b");
    lines
        .iter()
        .enumerate()
        .filter(|(i, l)| {
            if !marker.is_match(l) {
                return false;
            }
            let source = lines[i.saturating_sub(5)..*i]
                .iter()
                .rev()
                .find(|v| v.contains("Loading") || v.contains("Unable to load"))
                .copied()
                .unwrap_or("");
            let block = error_block(&lines, *i);
            own_path(&block) || own_path(source)
        })
        .map(|(_, l)| (*l).into())
        .collect()
}

fn jac_error_line(line: &str) -> bool {
    let Some((_, body)) = line.split_once("[JAC_POC]") else {
        return false;
    };
    let body = body.to_lowercase();
    if re(r"\b(?:api_error|failed|failure|exception|runtime error)\b").is_match(&body) {
        return true;
    }
    re(r"\berror\b").find_iter(&body).any(|m| {
        let rest = body[m.end()..].trim_start();
        if let Some(value) = rest.strip_prefix('=') {
            let value = value.trim_start();
            let token = value
                .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                .next()
                .unwrap_or("");
            !matches!(token, "none" | "nil")
        } else {
            true
        }
    })
}
fn runtime_errors(lua: &str) -> Vec<String> {
    let lines: Vec<_> = lua.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter(|(i, l)| l.contains("Runtime Error") && own_path(&error_context(&lines, *i)))
        .map(|(_, l)| (*l).into())
        .collect()
}
fn growth_ui_errors(lua: &str) -> Vec<String> {
    let lines: Vec<_> = lua.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter(|(i, l)| {
            if ![
                "UI.DataError",
                "Cannot determine if partial screen",
                "Runtime Error",
            ]
            .iter()
            .any(|m| l.contains(m))
            {
                return false;
            }
            let context = error_context(&lines, *i);
            own_path(&context) || re("(?i)PartialScreenHooks").is_match(&context)
        })
        .map(|(_, l)| (*l).into())
        .collect()
}
fn japan_map_checks(logs: &BTreeMap<String, String>) -> Vec<Check> {
    let modding = &logs["Modding.log"];
    let lines: Vec<_> = logs["Lua.log"].lines().collect();
    let paths = re(
        r"(?i)WorldCongress|CongressButton|ActionPanel_Expansion2|GovernmentScreen_Expansion2|GreatPeoplePopup_Expansion2|DiplomacyRibbon_Expansion2|DiplomacyActionView_Expansion2|GovernorPanel|CityPanel_Expansion2",
    );
    let congress = lines
        .iter()
        .enumerate()
        .filter(|(i, l)| {
            (l.contains("Runtime Error") || l.contains("UI.DataError"))
                && paths.is_match(&error_context(&lines, *i))
        })
        .count();
    let db = related_errors(&logs["Database.log"]).len();
    let mods = related_errors(modding).len();
    vec![
        Check::new(
            "Japan map: mod registered",
            modding.to_lowercase().contains(JAPAN_MAP_ID),
            "Modding.log contains mod ID",
        ),
        Check::new(
            "Japan map: in-game text applied",
            modding.contains("Applying Component - JAC_JapanMapScale_InGameText (UpdateText)"),
            "map names loaded under the running ruleset",
        ),
        Check::new(
            "Japan map: Database.log",
            db == 0,
            format!("{db} JAC-related database errors"),
        ),
        Check::new(
            "Japan map: Modding.log",
            mods == 0,
            format!("{mods} JAC-related modding errors"),
        ),
        Check::new(
            "Japan map: GS Congress paths",
            congress == 0,
            format!("{congress} World Congress-related UI or gameplay errors"),
        ),
    ]
}
fn bundle0_checks(logs: &BTreeMap<String, String>) -> Vec<Check> {
    let own: Vec<_> = logs["Lua.log"]
        .lines()
        .filter(|l| l.starts_with("JAC_PoC_Bundle0:"))
        .collect();
    let loaded = own
        .iter()
        .position(|l| l.contains("[JAC_POC] loaded version=1"));
    let new = own
        .iter()
        .position(|l| l.contains("[JAC_POC] no saved state (new game)"));
    let restore = re(r"\brestored firstTurn=(\d+) lastTurn=(\d+) turnsSeen=(\d+)");
    let turn = re(r"\bturn=(\d+)\s+firstTurn=(\d+)\s+turnsSeen=(\d+)");
    let restoration = own
        .iter()
        .enumerate()
        .find_map(|(i, l)| restore.captures(l).map(|m| (i, m)));
    let turns: Vec<_> = own
        .iter()
        .enumerate()
        .filter_map(|(i, l)| turn.captures(l).map(|m| (i, m)))
        .collect();
    let db = database_errors(&logs["Database.log"]).len();
    let modding = &logs["Modding.log"];
    let mut checks = vec![
        Check::new(
            "Bundle0: mod registered",
            modding.to_lowercase().contains(BUNDLE0_ID),
            "Modding.log contains mod ID",
        ),
        Check::new(
            "Bundle0: database action",
            modding.contains("Applying Component - JAC_PoC_Bundle0_Gameplay (UpdateDatabase)"),
            "UpdateDatabase applied",
        ),
        Check::new(
            "Bundle0: script action",
            modding.contains("Applying Component - JAC_PoC_Bundle0_Scripts (AddGameplayScripts)"),
            "AddGameplayScripts applied",
        ),
        Check::new(
            "Bundle0: database errors",
            db == 0,
            format!("JAC-related Database.log errors: {db}"),
        ),
        Check::new(
            "Bundle0: initial load",
            loaded.is_some(),
            "loaded version=1",
        ),
        Check::new(
            "Bundle0: new game",
            new.zip(loaded).is_some_and(|(n, l)| n > l),
            "no saved state after initial load",
        ),
    ];
    let (mut progress, mut saved, mut next) = (false, false, false);
    if let Some((index, state)) = restoration {
        let before: Vec<_> = turns
            .iter()
            .filter(|(i, _)| *i < index)
            .map(|(_, m)| m)
            .collect();
        let after: Vec<_> = turns
            .iter()
            .filter(|(i, _)| *i > index)
            .map(|(_, m)| m)
            .collect();
        if let (Some(previous), Some(following)) = (before.last(), after.first()) {
            let nums = |m: &regex::Captures<'_>| -> Option<[u64; 3]> {
                Some([m[1].parse().ok()?, m[2].parse().ok()?, m[3].parse().ok()?])
            };
            if let (
                Some([first, last, count]),
                Some([prev_turn, prev_first, prev_count]),
                Some([post_turn, post_first, post_count]),
            ) = (nums(&state), nums(previous), nums(following))
            {
                saved = first == prev_first && last == prev_turn && count == prev_count;
                next = last.checked_add(1) == Some(post_turn)
                    && post_first == first
                    && count.checked_add(1) == Some(post_count);
            }
            progress = before.len() >= 2
                && before.windows(2).all(|p| match (nums(p[0]), nums(p[1])) {
                    (Some(a), Some(b)) => a[2].checked_add(1) == Some(b[2]),
                    _ => false,
                });
        }
    }
    checks.extend([
        Check::new(
            "Bundle0: turns progress",
            progress,
            "turnsSeen increases by one before save",
        ),
        Check::new(
            "Bundle0: restored values",
            saved,
            "restored values match last pre-save turn",
        ),
        Check::new(
            "Bundle0: post-load turn",
            next,
            "turn and turnsSeen both increase by one",
        ),
    ]);
    checks
}

#[derive(Clone)]
struct GrowthRecord {
    kind: String,
    unit: String,
    level: i64,
    xp: i64,
    personality: String,
    trait_id: String,
    equipment: String,
    artifact: String,
    player: String,
    turn: Option<i64>,
}
fn growth_record(line: &str, pattern: &Regex) -> Option<GrowthRecord> {
    let m = pattern.captures(line)?;
    Some(GrowthRecord {
        kind: m["kind"].into(),
        unit: m["unit"].into(),
        level: m["level"].parse().ok()?,
        xp: m["xp"].parse().ok()?,
        personality: m["personality"].into(),
        trait_id: m["trait"].into(),
        equipment: m["equipment"].into(),
        artifact: m["artifact"].into(),
        player: m["player"].into(),
        turn: m.name("turn").and_then(|v| v.as_str().parse().ok()),
    })
}
fn growth_next(mut level: i64, mut xp: i64) -> (i64, i64) {
    xp += 20;
    while level < 30 {
        let needed = (30.0 * (level as f64).powf(1.5) + 0.5) as i64;
        if xp < needed {
            break;
        }
        xp -= needed;
        level += 1;
    }
    (level, xp)
}
fn same_growth(a: &GrowthRecord, b: &GrowthRecord) -> bool {
    a.level == b.level
        && a.xp == b.xp
        && a.personality == b.personality
        && a.trait_id == b.trait_id
        && a.equipment == b.equipment
        && a.artifact == b.artifact
}
fn growth_checks(logs: &BTreeMap<String, String>, expected: usize) -> Vec<Check> {
    let lua = &logs["Lua.log"];
    let modding = &logs["Modding.log"];
    let own: Vec<_> = lua
        .lines()
        .filter(|l| l.starts_with("JAC_PoC_GrowthUI_Gameplay:"))
        .collect();
    let pattern = re(
        r"\b(?P<kind>assigned |restored |)unit=(?P<unit>\d+) level=(?P<level>\d+) xp=(?P<xp>\d+) personality=(?P<personality>\S+) trait=(?P<trait>\S+) equipment=(?P<equipment>\S+) artifact=(?P<artifact>\S+) player=(?P<player>\d+)(?: turn=(?P<turn>\d+))?",
    );
    let parsed: Vec<_> = own
        .iter()
        .enumerate()
        .filter_map(|(i, l)| growth_record(l, &pattern).map(|m| (i, m)))
        .collect();
    let restores: Vec<_> = parsed
        .iter()
        .filter(|(_, m)| m.kind == "restored ")
        .collect();
    let restore_at = restores.first().map_or(own.len(), |(i, _)| *i);
    let assigned: Vec<_> = parsed
        .iter()
        .filter(|(i, m)| *i < restore_at && m.kind == "assigned ")
        .map(|(_, m)| m)
        .collect();
    let before: Vec<_> = parsed
        .iter()
        .filter(|(i, m)| *i < restore_at && m.kind.is_empty())
        .map(|(_, m)| m)
        .collect();
    let after: Vec<_> = parsed
        .iter()
        .filter(|(i, m)| *i > restore_at && m.kind.is_empty())
        .map(|(_, m)| m)
        .collect();
    let initialized = re(r"\binitialized count=(\d+)")
        .captures(&own[..restore_at].join("\n"))
        .and_then(|m| m[1].parse::<usize>().ok());
    let restored_count = re(r"\brestored count=(\d+)")
        .captures(&own[restore_at..].join("\n"))
        .and_then(|m| m[1].parse::<usize>().ok());
    let db = database_errors(&logs["Database.log"]).len();
    let ui = growth_ui_errors(lua).len();
    let opened = lua.matches("[JAC_POC] notebooks opened").count();
    let journal = lua
        .matches("[JAC_POC] character journal shown count=")
        .count();
    let assignment_ok = own[..restore_at]
        .join("\n")
        .contains("[JAC_POC] no saved state (new game)")
        && initialized == Some(assigned.len())
        && assigned.len() == expected
        && assigned
            .iter()
            .map(|m| (&m.player, &m.unit))
            .collect::<BTreeSet<_>>()
            .len()
            == expected
        && assigned.iter().all(|m| m.level == 1 && m.xp == 0)
        && assigned.first().is_none_or(|m| {
            m.equipment == "JAC_ITEM_POC_BLADE,JAC_ITEM_POC_CLOAK"
                && m.artifact == "JAC_ARTIFACT_POC_RELIC"
        })
        && assigned
            .get(1)
            .is_none_or(|m| m.equipment == "JAC_ITEM_POC_BLADE" && m.artifact == "none");
    let mut checks = vec![
        Check::new(
            "Growth: mod registered",
            modding.to_lowercase().contains(GROWTH_ID),
            "GrowthUI mod ID in Modding.log",
        ),
        Check::new(
            "Growth: five actions",
            [
                "Text",
                "Gameplay",
                "InGameUI",
                "PartialScreenHooks",
                "UIFiles",
            ]
            .iter()
            .all(|p| modding.contains(&format!("Applying Component - JAC_GrowthUI_{p} "))),
            "UpdateText, AddGameplayScripts, AddUserInterfaces, ReplaceUIScript, ImportFiles",
        ),
        Check::new(
            "Growth: database errors",
            db == 0,
            format!("JAC-related errors: {db}"),
        ),
        Check::new(
            "Growth: button attached",
            lua.contains("[JAC_POC] notebooks button attached parent=ButtonStack"),
            "parent=ButtonStack",
        ),
        Check::new(
            "Growth: partial-screen path",
            lua.contains("[JAC_POC] notebooks path lookup found=true")
                && !lua.contains("[JAC_POC] notebooks path lookup found=false"),
            "lookup true, never false",
        ),
        Check::new(
            "Growth: open and journal",
            opened >= 3 && journal >= 2,
            format!("opened={opened}, journal={journal}"),
        ),
        Check::new(
            "Growth: partial-screen errors",
            ui == 0,
            format!("JAC-related UI.DataError, partial-screen or Runtime Error: {ui}"),
        ),
        Check::new(
            "Growth: new-game assignment",
            assignment_ok,
            format!(
                "assigned={}, initialized={}, expected={expected}",
                assigned.len(),
                initialized.map_or("?".into(), |n| n.to_string())
            ),
        ),
    ];
    let mut by_unit: BTreeMap<(&str, &str), Vec<&GrowthRecord>> = BTreeMap::new();
    for m in &before {
        by_unit.entry((&m.player, &m.unit)).or_default().push(m);
    }
    let unique_turns = before
        .iter()
        .filter_map(|m| m.turn)
        .collect::<BTreeSet<_>>();
    let mut growth_ok = !by_unit.is_empty() && unique_turns.len() >= 4;
    let mut level_two = false;
    for records in by_unit.values() {
        let (mut level, mut xp, mut previous) = (1, 0, None);
        for record in records {
            if previous.is_some_and(|p| record.turn != Some(p + 1)) {
                growth_ok = false
            }
            (level, xp) = growth_next(level, xp);
            if (record.level, record.xp) != (level, xp) {
                growth_ok = false
            }
            if level == 2 && xp == 10 {
                level_two = true
            }
            previous = record.turn;
        }
    }
    checks.push(Check::new(
        "Growth: 20 XP each turn",
        growth_ok && level_two,
        format!(
            "turns={}, Lv2/10={}",
            unique_turns.len(),
            if level_two { "True" } else { "False" }
        ),
    ));
    let last_turn = before.iter().filter_map(|m| m.turn).max().unwrap_or(-1);
    let last_rows: BTreeMap<_, _> = before
        .iter()
        .filter(|m| m.turn == Some(last_turn))
        .map(|m| ((&m.player, &m.unit), *m))
        .collect();
    let restored_rows: BTreeMap<_, _> = restores
        .iter()
        .map(|(_, m)| ((&m.player, &m.unit), m))
        .collect();
    let same = last_rows.len() == expected
        && restored_rows.len() == expected
        && last_rows.iter().all(|(key, row)| {
            restored_rows
                .get(key)
                .is_some_and(|other| same_growth(row, other))
        });
    let restore_position = lua
        .find("JAC_PoC_GrowthUI_Gameplay: [JAC_POC] restored unit=")
        .unwrap_or(usize::MAX);
    let journals: Vec<_> = re(r"JAC_Notebooks: \[JAC_POC\] character journal shown count=(\d+)")
        .captures_iter(lua)
        .filter_map(|m| Some((m.get(0)?.start(), m[1].parse::<usize>().ok()?)))
        .collect();
    let before_journal: Vec<_> = journals
        .iter()
        .filter(|(at, _)| *at < restore_position)
        .map(|(_, n)| *n)
        .collect();
    let after_journal: Vec<_> = journals
        .iter()
        .filter(|(at, _)| *at > restore_position)
        .map(|(_, n)| *n)
        .collect();
    let counts_ok = restored_count == Some(expected)
        && !before_journal.is_empty()
        && !after_journal.is_empty()
        && before_journal
            .iter()
            .chain(after_journal.iter())
            .all(|n| *n == expected);
    checks.push(Check::new(
        "Growth: restored rows and journal",
        same && counts_ok,
        format!(
            "pre-save={}, restored={}, journal={:?}/{:?}, expected={expected}",
            last_rows.len(),
            restored_rows.len(),
            before_journal
                .last()
                .copied()
                .into_iter()
                .collect::<Vec<_>>(),
            after_journal
                .first()
                .copied()
                .into_iter()
                .collect::<Vec<_>>()
        ),
    ));
    let next_rows: BTreeMap<_, _> = after
        .iter()
        .filter(|m| m.turn == Some(last_turn + 1))
        .map(|m| ((&m.player, &m.unit), *m))
        .collect();
    let post_ok = same
        && next_rows.len() == restored_rows.len()
        && next_rows.iter().all(|(key, row)| {
            restored_rows
                .get(key)
                .is_some_and(|old| (row.level, row.xp) == growth_next(old.level, old.xp))
        })
        && !parsed
            .iter()
            .any(|(i, m)| *i > restore_at && m.kind == "assigned ");
    checks.push(Check::new(
        "Growth: one post-load turn",
        post_ok,
        format!(
            "turn={}, rows={}, no reassignment",
            last_turn + 1,
            next_rows.len()
        ),
    ));
    checks
}

pub fn judge(
    logs: &BTreeMap<String, String>,
    profile: &str,
    expected_units: Option<usize>,
    strict_lua_errors: bool,
) -> Vec<Check> {
    for name in LOG_NAMES {
        if !logs.contains_key(name) {
            return vec![Check::new(
                "Input: logs",
                false,
                format!("Missing log: {name}"),
            )];
        }
    }
    let lua = &logs["Lua.log"];
    let jac = lua.lines().filter(|line| jac_error_line(line)).count();
    let runtime = runtime_errors(lua).len();
    let modding = related_errors(&logs["Modding.log"]).len();
    let database = database_errors(&logs["Database.log"]).len();
    let texture =
        re(r"(?:Failed to load background image texture: |We are lacking a texture for )\S*JAC_");
    let missing = lua
        .lines()
        .filter(|line| {
            texture.is_match(line)
                || (line.contains("JAC")
                    && [
                        "Could not find icon",
                        "We are lacking a texture",
                        "Failed to load background image texture",
                    ]
                    .iter()
                    .any(|m| line.contains(m)))
        })
        .count();
    let mut checks = vec![
        Check::new(
            "Generic: JAC Modding.log errors",
            modding == 0,
            format!("{modding} JAC-related modding errors"),
        ),
        Check::new(
            "Generic: JAC Database.log errors",
            database == 0,
            format!("{database} JAC-related database errors"),
        ),
        Check::new(
            "Generic: [JAC_POC] errors",
            jac == 0,
            format!("{jac} matching lines"),
        ),
        Check::new(
            "Generic: JAC Runtime Error",
            runtime == 0,
            format!("{runtime} matching stack traces"),
        ),
        Check::new(
            "Generic: JAC missing texture",
            missing == 0,
            format!("{missing} matching lines"),
        ),
    ];
    if strict_lua_errors {
        let errors = lua
            .lines()
            .filter(|line| {
                ["Runtime Error", "UI.DataError", "EraComplete cannot show"]
                    .iter()
                    .any(|m| line.contains(m))
            })
            .count();
        checks.push(Check::new(
            "Scenario: all Lua errors",
            errors == 0,
            format!("{errors} error lines (including official UI)"),
        ));
    }
    match profile {
        "bundle0" => checks.extend(bundle0_checks(logs)),
        "growth" => {
            if let Some(n) = expected_units.filter(|n| *n > 0) {
                checks.extend(growth_checks(logs, n))
            } else {
                checks.push(Check::new(
                    "Input: growth",
                    false,
                    "growth judge needs a positive expected_units count",
                ))
            }
        }
        "japan_map" => checks.extend(japan_map_checks(logs)),
        "generic" => {}
        _ => checks.push(Check::new(
            "Input: profile",
            false,
            format!("Unknown judge profile: {profile}"),
        )),
    }
    checks
}
pub fn print_table(checks: &[Check]) {
    let width = checks.iter().map(|c| c.name.len()).max().unwrap_or(5);
    println!("{:<6}  {:<width$}  Detail", "Result", "Check");
    for check in checks {
        println!(
            "{:<6}  {:<width$}  {}",
            if check.passed { "PASS" } else { "FAIL" },
            check.name,
            check.detail
        );
    }
}
