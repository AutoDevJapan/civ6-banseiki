//! Reproduce the official Base + Gathering Storm foreign-key cascade audit.
use crate::Repository;
use anyhow::{bail, Context, Result};
use roxmltree::Node;
use rusqlite::{functions::FunctionFlags, params_from_iter, types::Value as SqlValue, Connection};
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::{Arc, Mutex},
};

pub const TREE_DELETES: [&str; 2] = [
    "DELETE FROM Technologies WHERE TechnologyType NOT LIKE 'TECH_JAC_SENGOKU_%';",
    "DELETE FROM Civics WHERE CivicType NOT LIKE 'CIVIC_JAC_SENGOKU_%';",
];

fn identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn fields(node: Node<'_, '_>) -> Vec<(String, SqlValue)> {
    fn value(text: &str) -> SqlValue {
        match text.to_ascii_lowercase().as_str() {
            "true" => SqlValue::Integer(1),
            "false" => SqlValue::Integer(0),
            _ => SqlValue::Text(text.to_owned()),
        }
    }
    let mut out: Vec<_> = node
        .attributes()
        .map(|a| (a.name().to_owned(), value(a.value())))
        .collect();
    for child in node.children().filter(Node::is_element) {
        let pair = (
            child.tag_name().name().to_owned(),
            value(child.text().unwrap_or("")),
        );
        if let Some(old) = out.iter_mut().find(|(name, _)| name == &pair.0) {
            *old = pair;
        } else {
            out.push(pair);
        }
    }
    out
}

fn condition(values: &[(String, SqlValue)]) -> (String, Vec<SqlValue>) {
    if values.is_empty() {
        return ("1".to_owned(), Vec::new());
    }
    (
        values
            .iter()
            .map(|(k, _)| format!("{}=?", identifier(k)))
            .collect::<Vec<_>>()
            .join(" AND "),
        values.iter().map(|(_, v)| v.clone()).collect(),
    )
}

pub fn load_xml(db: &Connection, text: &str, tables: &BTreeSet<String>) -> Result<()> {
    let doc = roxmltree::Document::parse(text)?;
    for table in doc.root_element().children().filter(Node::is_element) {
        if !tables.contains(table.tag_name().name()) {
            continue;
        }
        let name = identifier(table.tag_name().name());
        for op in table.children().filter(Node::is_element) {
            match op.tag_name().name() {
                "Row" | "Replace" => {
                    let row = fields(op);
                    let verb = if op.has_tag_name("Replace") {
                        "INSERT OR REPLACE"
                    } else {
                        "INSERT OR IGNORE"
                    };
                    let columns = row
                        .iter()
                        .map(|(k, _)| identifier(k))
                        .collect::<Vec<_>>()
                        .join(",");
                    let placeholders = vec!["?"; row.len()].join(",");
                    db.execute(
                        &format!("{verb} INTO {name} ({columns}) VALUES ({placeholders})"),
                        params_from_iter(row.iter().map(|(_, v)| v)),
                    )?;
                }
                "Update" => {
                    let where_node = op.children().find(|n| n.has_tag_name("Where"));
                    let sets = fields(
                        op.children()
                            .find(|n| n.has_tag_name("Set"))
                            .context("XML Update missing Set")?,
                    );
                    let (predicate, args) = condition(&where_node.map(fields).unwrap_or_default());
                    let changes = sets
                        .iter()
                        .map(|(k, _)| format!("{}=?", identifier(k)))
                        .collect::<Vec<_>>()
                        .join(",");
                    let values: Vec<_> = sets.into_iter().map(|(_, v)| v).chain(args).collect();
                    db.execute(
                        &format!("UPDATE {name} SET {changes} WHERE {predicate}"),
                        params_from_iter(values),
                    )?;
                }
                "Delete" => {
                    let (predicate, args) = condition(&fields(op));
                    db.execute_batch("PRAGMA foreign_keys=ON;")?;
                    db.execute(
                        &format!("DELETE FROM {name} WHERE {predicate}"),
                        params_from_iter(args),
                    )?;
                    db.execute_batch("PRAGMA foreign_keys=OFF;")?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

pub fn register_audit_hash(db: &Connection) -> Result<()> {
    let has_types: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='Types')",
        [],
        |r| r.get(0),
    )?;
    let mut assigned = BTreeMap::<String, i64>::new();
    if has_types {
        let mut statement = db.prepare("SELECT Type, Hash FROM Types")?;
        for pair in
            statement.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        {
            let (key, v) = pair?;
            assigned.insert(key, v);
        }
    }
    let next_id = assigned.values().copied().max().unwrap_or(0) + 1;
    let state = Arc::new(Mutex::new((assigned, next_id)));
    db.create_scalar_function("Make_Hash", 1, FunctionFlags::SQLITE_UTF8, move |ctx| {
        let name: String = ctx.get(0)?;
        let mut state = state
            .lock()
            .map_err(|_| rusqlite::Error::UserFunctionError("audit hash lock poisoned".into()))?;
        if let Some(value) = state.0.get(&name) {
            return Ok(*value);
        }
        let value = state.1;
        state.0.insert(name, value);
        state.1 += 1;
        Ok(value)
    })?;
    Ok(())
}

pub fn xp2_files(repo: &Repository) -> Result<Vec<std::path::PathBuf>> {
    let text = repo.game_read("DLC/Expansion2/Expansion2.modinfo")?;
    let doc = roxmltree::Document::parse(&text)?;
    let mut files = Vec::new();
    for component in [
        "Expansion2Core",
        "Expansion2CoreContent",
        "Expansion2MajorContent",
    ] {
        let action = doc
            .descendants()
            .find(|n| n.has_tag_name("UpdateDatabase") && n.attribute("id") == Some(component))
            .with_context(|| format!("missing {component}"))?;
        let mut entries: Vec<_> = action
            .children()
            .filter(|n| n.has_tag_name("File"))
            .collect();
        entries.sort_by_key(|n| {
            std::cmp::Reverse(
                n.attribute("Priority")
                    .unwrap_or("0")
                    .parse::<i64>()
                    .unwrap_or(0),
            )
        });
        files.extend(
            entries
                .into_iter()
                .map(|n| Path::new("DLC/Expansion2").join(n.text().unwrap_or(""))),
        );
    }
    Ok(files)
}

pub fn official_db() -> Result<Connection> {
    official_db_for(&Repository::checked_in())
}

pub fn official_db_for(repo: &Repository) -> Result<Connection> {
    let db = Connection::open_in_memory()?;
    db.execute_batch("PRAGMA foreign_keys=OFF;")?;
    register_audit_hash(&db)?;
    for path in [
        "Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql",
        "Base/Assets/Gameplay/Data/Schema/02_AddTriggers.sql",
    ] {
        db.execute_batch(&repo.game_read(path)?)?;
    }
    let xp2 = xp2_files(repo)?;
    db.execute_batch(&repo.game_read(&xp2[0])?)?;
    let tables = table_names(&db)?;
    let mut paths: Vec<_> = std::fs::read_dir(repo.game_path("Base/Assets/Gameplay/Data"))?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<_>>()?;
    paths.retain(|p| p.extension().and_then(|s| s.to_str()) == Some("xml"));
    paths.sort();
    for path in paths {
        load_xml(&db, &crate::read_utf8(&path)?, &tables)
            .with_context(|| format!("load {}", path.display()))?;
    }
    for path in &xp2[1..] {
        load_xml(&db, &repo.game_read(path)?, &tables)
            .with_context(|| format!("load {}", path.display()))?;
    }
    Ok(db)
}

fn table_names(db: &Connection) -> Result<BTreeSet<String>> {
    let mut query =
        db.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")?;
    let names = query
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(names)
}

pub fn run_sengoku(
    repo: &Repository,
    source: &Connection,
    with_deletes: bool,
) -> Result<Connection> {
    let mut db = Connection::open_in_memory()?;
    {
        let backup = rusqlite::backup::Backup::new(source, &mut db)?;
        backup.run_to_completion(1000, std::time::Duration::ZERO, None)?;
    }
    register_audit_hash(&db)?;
    db.execute_batch("PRAGMA foreign_keys=ON;")?;
    let text = repo.read("poc/JAC_PoC_SengokuTrees/JAC_PoC_SengokuTrees.modinfo")?;
    let doc = roxmltree::Document::parse(&text)?;
    let actions = doc
        .root_element()
        .children()
        .find(|n| n.has_tag_name("InGameActions"))
        .context("missing InGameActions")?;
    for action in actions
        .children()
        .filter(|n| n.has_tag_name("UpdateDatabase"))
    {
        for file in action.children().filter(|n| n.has_tag_name("File")) {
            let relative = file.text().unwrap_or("");
            if !relative.ends_with(".sql") {
                continue;
            }
            let mut script = repo.read(Path::new("poc/JAC_PoC_SengokuTrees").join(relative))?;
            if !with_deletes {
                for statement in TREE_DELETES {
                    if relative.ends_with("Unlocks.sql") && !script.contains(statement) {
                        bail!("missing tree DELETE {statement}");
                    }
                    script = script.replace(statement, "");
                }
            }
            db.execute_batch(&format!(
                "BEGIN; PRAGMA defer_foreign_keys=ON;\n{script}\nCOMMIT;"
            ))
            .with_context(|| format!("run {relative}"))?;
        }
    }
    Ok(db)
}

fn as_json(value: SqlValue) -> Value {
    match value {
        SqlValue::Null => Value::Null,
        SqlValue::Integer(n) => n.into(),
        SqlValue::Real(n) => serde_json::json!(n),
        SqlValue::Text(s) => s.into(),
        SqlValue::Blob(b) => serde_json::json!(b),
    }
}

fn rows(db: &Connection, table: &str) -> Result<BTreeMap<i64, Value>> {
    let mut statement = db.prepare(&format!("SELECT rowid, * FROM {}", identifier(table)))?;
    let columns: Vec<_> = statement
        .column_names()
        .iter()
        .skip(1)
        .map(|s| (*s).to_owned())
        .collect();
    let out = statement
        .query_map([], |row| {
            let mut object = Map::new();
            for (index, name) in columns.iter().enumerate() {
                object.insert(name.clone(), as_json(row.get::<_, SqlValue>(index + 1)?));
            }
            Ok((row.get::<_, i64>(0)?, Value::Object(object)))
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(out)
}

pub fn audit(repo: &Repository, source: &Connection) -> Result<Value> {
    let shipped = run_sengoku(repo, source, true)?;
    let kept = run_sengoku(repo, source, false)?;
    let mut result = Map::new();
    for table in table_names(&kept)? {
        if table == "Technologies" || table == "Civics" {
            continue;
        }
        let before = rows(&kept, &table)?;
        let after = rows(&shipped, &table)?;
        let mut lost = Vec::new();
        let mut changed = Vec::new();
        for (id, row) in before {
            match after.get(&id) {
                None => lost.push(row),
                Some(new) if new != &row => changed.push(serde_json::json!([row, new])),
                _ => {}
            }
        }
        if !lost.is_empty() || !changed.is_empty() {
            result.insert(table, serde_json::json!({"lost":lost,"changed":changed}));
        }
    }
    Ok(Value::Object(result))
}

fn python_display(value: &Value) -> String {
    match value {
        Value::Null => "None".into(),
        Value::String(s) => s.clone(),
        _ => value.to_string(),
    }
}

fn key(row: &Value) -> String {
    let row = row.as_object().expect("audit row object");
    row.iter()
        .find(|(k, v)| k.ends_with("Type") && !v.is_null() && v.as_str() != Some(""))
        .or_else(|| row.iter().next())
        .map(|(_, v)| python_display(v))
        .unwrap_or_default()
}

pub fn report(result: &Value) -> String {
    let mut out = String::new();
    let mut tables: Vec<_> = result.as_object().expect("audit object").iter().collect();
    tables.sort_by_key(|(k, _)| *k);
    for (table, diff) in tables {
        let mut lost: Vec<_> = diff["lost"].as_array().unwrap().iter().map(key).collect();
        lost.sort();
        let mut changed: Vec<_> = diff["changed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| {
                let a = pair[0].as_object().unwrap();
                let b = pair[1].as_object().unwrap();
                let changes = a
                    .iter()
                    .filter(|(k, v)| b.get(*k) != Some(*v))
                    .map(|(k, v)| format!("{k} {}->{}", python_display(v), python_display(&b[k])))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}: {changes}", key(&pair[0]))
            })
            .collect();
        changed.sort();
        out.push_str(&format!(
            "{table}: lost {}, changed {}\n",
            lost.len(),
            changed.len()
        ));
        for line in lost {
            out.push_str(&format!("  - {line}\n"));
        }
        for line in changed {
            out.push_str(&format!("  ~ {line}\n"));
        }
    }
    out
}

pub fn cli(repo: &Repository, _args: &[String]) -> Result<()> {
    print!("{}", report(&audit(repo, &official_db_for(repo)?)?));
    Ok(())
}

#[cfg(test)]
mod sengoku_cascade_tests {
    use super::*;
    const C11: &str = "CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES";
    const ALLIANCES: [&str; 7] = [
        "DIPLOACTION_ALLIANCE",
        "DIPLOACTION_ALLIANCE_MILITARY",
        "DIPLOACTION_ALLIANCE_RESEARCH",
        "DIPLOACTION_ALLIANCE_CULTURAL",
        "DIPLOACTION_ALLIANCE_ECONOMIC",
        "DIPLOACTION_ALLIANCE_RELIGIOUS",
        "DIPLOACTION_RENEW_ALLIANCE",
    ];
    fn snapshot() -> Result<(Repository, Connection, Value, Connection)> {
        let repo = Repository::checked_in();
        let official = official_db()?;
        let result = audit(&repo, &official)?;
        let final_db = run_sengoku(&repo, &official, true)?;
        Ok((repo, official, result, final_db))
    }
    fn strings(db: &Connection, sql: &str) -> Result<BTreeSet<String>> {
        let mut stmt = db.prepare(sql)?;
        let out = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(out)
    }

    #[test]
    fn test_no_diplomatic_action_is_lost() -> Result<()> {
        let (_, official, result, final_db) = snapshot()?;
        for table in [
            "DiplomaticActions",
            "DiplomaticActions_XP1",
            "DiplomaticStateActions",
        ] {
            assert!(result.get(table).is_none(), "{table}");
            let before: i64 = official.query_row(&format!("SELECT COUNT(*) FROM {table} WHERE DiplomaticActionType <> 'DIPLOACTION_DECLARE_GOLDEN_AGE_WAR'"),[],|r| r.get(0))?;
            let after: i64 =
                final_db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
            assert_eq!(after, before, "{table}");
        }
        let mut stmt = final_db.prepare(&format!("SELECT DiplomaticActionType, InitiatorPrereqCivic || '|' || TargetPrereqCivic FROM DiplomaticActions WHERE DiplomaticActionType IN ({})",vec!["?";7].join(",")))?;
        let actual: BTreeMap<String, String> = stmt
            .query_map(ALLIANCES, |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        assert_eq!(
            actual,
            ALLIANCES
                .map(|a| (a.to_owned(), format!("{C11}|{C11}")))
                .into_iter()
                .collect()
        );
        assert_eq!(strings(&final_db,"SELECT DiplomaticActionType FROM DiplomaticActions_XP1 WHERE AllianceType IS NOT NULL")?,ALLIANCES[1..6].iter().map(|s|s.to_string()).collect());
        Ok(())
    }

    #[test]
    fn test_only_documented_rows_are_lost() -> Result<()> {
        let (_, _, result, _) = snapshot()?;
        let expected = [
            ("Adjacency_YieldChanges", (0, 12)),
            ("Boosts", (126, 0)),
            ("Buildings", (0, 70)),
            ("CivicModifiers", (27, 0)),
            ("CivicPrereqs", (80, 0)),
            ("CivicQuotes", (113, 0)),
            ("CivicRandomCosts", (10, 0)),
            ("Civics_XP2", (6, 0)),
            ("Improvement_BonusYieldChanges", (25, 0)),
            ("Improvement_Tourism", (11, 0)),
            ("Improvement_ValidFeatures", (1, 0)),
            ("Improvements", (0, 26)),
            ("RandomAgendaCivicTags", (2, 0)),
            ("RandomAgendasForCivic", (1, 0)),
            ("Routes_XP2", (1, 0)),
            ("StartingBoostedCivics", (11, 0)),
            ("StartingBoostedTechnologies", (21, 0)),
            ("Technologies_XP2", (9, 0)),
            ("TechnologyModifiers", (14, 0)),
            ("TechnologyPrereqs", (95, 0)),
            ("TechnologyQuotes", (147, 0)),
            ("TechnologyRandomCosts", (12, 0)),
            ("Units", (0, 94)),
        ];
        let actual: BTreeMap<_, _> = result
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str(),
                    (
                        v["lost"].as_array().unwrap().len(),
                        v["changed"].as_array().unwrap().len(),
                    ),
                )
            })
            .collect();
        assert_eq!(actual, expected.into_iter().collect());
        Ok(())
    }

    #[test]
    fn test_no_governor_title_source_remains() -> Result<()> {
        let (_, official, _, final_db) = snapshot()?;
        let query = "SELECT ModifierId FROM Modifiers WHERE ModifierType IN (SELECT ModifierType FROM DynamicModifiers WHERE EffectType='EFFECT_ADJUST_PLAYER_GOVERNOR_POINTS')";
        let sources = strings(&official, query)?;
        assert_eq!(sources.len(), 15);
        assert!(strings(&final_db, query)?.is_empty());
        for table in table_names(&final_db)? {
            let mut stmt =
                final_db.prepare(&format!("PRAGMA table_info({})", identifier(&table)))?;
            let columns = stmt
                .query_map([], |r| r.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for column in columns
                .into_iter()
                .filter(|c| c.eq_ignore_ascii_case("modifierid"))
            {
                let mut stmt = final_db.prepare(&format!(
                    "SELECT {} FROM {}",
                    identifier(&column),
                    identifier(&table)
                ))?;
                let used: BTreeSet<String> = stmt
                    .query_map([], |r| r.get::<_, Option<String>>(0))?
                    .collect::<rusqlite::Result<Vec<_>>>()?
                    .into_iter()
                    .flatten()
                    .collect();
                assert!(used.is_disjoint(&sources), "{table}");
            }
        }
        Ok(())
    }

    #[test]
    fn test_owner_decided_gates() -> Result<()> {
        let (_, _, _, db) = snapshot()?;
        let mut stmt = db.prepare("SELECT TerrainType, PrereqTech, PrereqCivic FROM Improvement_ValidTerrains WHERE ImprovementType='IMPROVEMENT_FARM' AND TerrainType LIKE '%_HILLS' ORDER BY TerrainType")?;
        let hills = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        assert_eq!(
            hills,
            vec![
                (
                    "TERRAIN_GRASS_HILLS".into(),
                    "TECH_JAC_SENGOKU_IRRIGATION_WORKS".into(),
                    None
                ),
                (
                    "TERRAIN_PLAINS_HILLS".into(),
                    "TECH_JAC_SENGOKU_IRRIGATION_WORKS".into(),
                    None
                )
            ]
        );
        let mut stmt = db.prepare("SELECT CommandType, PrereqTech, PrereqCivic FROM UnitCommands WHERE CommandType IN ('UNITCOMMAND_FORM_CORPS','UNITCOMMAND_FORM_ARMY') ORDER BY CommandType")?;
        let corps = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        assert_eq!(
            corps,
            vec![
                (
                    "UNITCOMMAND_FORM_ARMY".into(),
                    "TECH_JAC_SENGOKU_REGIONAL_SUPPLY".into(),
                    None
                ),
                (
                    "UNITCOMMAND_FORM_CORPS".into(),
                    "TECH_JAC_SENGOKU_REGIONAL_SUPPLY".into(),
                    None
                )
            ]
        );
        Ok(())
    }

    #[test]
    fn test_nulled_items_are_not_available() -> Result<()> {
        let (_, _, result, db) = snapshot()?;
        for (table, flag) in [
            ("Units", "CanTrain=0"),
            ("Buildings", "InternalOnly=1"),
            ("Improvements", "Buildable=0"),
        ] {
            let pk: String =
                db.query_row(&format!("PRAGMA table_info({table})"), [], |r| r.get(1))?;
            let closed = strings(
                &db,
                &format!("SELECT {} FROM {table} WHERE {flag}", identifier(&pk)),
            )?;
            assert!(!result[table]["changed"].as_array().unwrap().is_empty());
            for pair in result[table]["changed"].as_array().unwrap() {
                let before = pair[0].as_object().unwrap();
                let after = pair[1].as_object().unwrap();
                let columns: BTreeSet<_> = before
                    .iter()
                    .filter(|(k, v)| after.get(*k) != Some(*v))
                    .map(|(k, _)| k.as_str())
                    .collect();
                assert!(
                    closed.contains(before[&pk].as_str().unwrap())
                        || columns
                            .is_subset(&BTreeSet::from(["MandatoryObsoleteTech", "ObsoleteTech"])),
                    "{table} {} {columns:?}",
                    before[&pk]
                );
            }
        }
        Ok(())
    }

    #[test]
    fn test_every_remap_matches_an_official_row_and_leaves_no_dangling_gate() -> Result<()> {
        let (repo, _, _, db) = snapshot()?;
        let text = repo.read("poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Cascade.sql")?;
        let updates = regex::Regex::new(r"(?m)^UPDATE (\w+) SET (.+?) WHERE (.+?);$")?;
        let setter = regex::Regex::new(r"(\w+)=(NULL|'[^']*')")?;
        let mut checked = 0;
        for capture in updates.captures_iter(&text) {
            let values: Vec<_> = setter
                .captures_iter(&capture[2])
                .map(|c| (c[1].to_owned(), c[2].to_owned()))
                .collect();
            assert!(!values.is_empty());
            let mut conditions: Vec<_> = values
                .iter()
                .map(|(k, v)| {
                    if v == "NULL" {
                        format!("{k} IS NULL")
                    } else {
                        format!("{k}={v}")
                    }
                })
                .collect();
            conditions.extend(
                capture[3]
                    .split(" AND ")
                    .filter(|predicate| {
                        !values.iter().any(|(key, _)| {
                            predicate.starts_with(&format!("{key}="))
                                || predicate.starts_with(&format!("{key} IN "))
                        })
                    })
                    .map(str::to_owned),
            );
            let count: i64 = db.query_row(
                &format!(
                    "SELECT COUNT(*) FROM {} WHERE {}",
                    &capture[1],
                    conditions.join(" AND ")
                ),
                [],
                |r| r.get(0),
            )?;
            assert!(count > 0, "{} {}", &capture[1], &capture[3]);
            checked += 1;
        }
        assert_eq!(checked, 31);
        let mut stmt = db.prepare("PRAGMA foreign_key_check")?;
        let violations = stmt
            .query_map([], |r| r.get::<_, String>(2))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        assert!(
            violations
                .iter()
                .all(|s| s != "Technologies" && s != "Civics"),
            "{violations:?}"
        );
        Ok(())
    }
}

#[cfg(test)]
mod alliance_scenario_tests {
    use super::*;
    #[test]
    fn test_live_scenario_checks_every_alliance_and_the_c11_grant() -> Result<()> {
        let scenario = Repository::checked_in()
            .json("tools/civ6-autotest/scenarios/sengoku-alliances.json")?;
        let steps = scenario["steps"].as_array().unwrap();
        let configure = steps.iter().find(|s| s["action"] == "configure").unwrap();
        assert_eq!(configure["ruleset"], "RULESET_JAC_BANSEIKI");
        assert!([2, 3].contains(&configure["players"].as_array().unwrap().len()));
        let codes: Vec<_> = steps
            .iter()
            .map(|s| s["code"].as_str().unwrap_or(""))
            .collect();
        for action in [
            "DIPLOACTION_ALLIANCE",
            "DIPLOACTION_ALLIANCE_MILITARY",
            "DIPLOACTION_ALLIANCE_RESEARCH",
            "DIPLOACTION_ALLIANCE_CULTURAL",
            "DIPLOACTION_ALLIANCE_ECONOMIC",
            "DIPLOACTION_ALLIANCE_RELIGIOUS",
            "DIPLOACTION_RENEW_ALLIANCE",
        ] {
            assert!(codes.iter().any(|c| c.contains(action)), "{action}");
        }
        let find = |needle: &str| {
            codes
                .iter()
                .enumerate()
                .filter(|(_, c)| c.contains(needle))
                .map(|(i, _)| i)
                .collect::<Vec<_>>()
        };
        let friend = find("SetHasDeclaredFriendship");
        let grant = find("SetCivic");
        let checks = find("IsDiplomaticActionValid");
        assert_eq!(friend.len(), 1);
        assert_eq!(grant.len(), 2);
        assert_eq!(checks.len(), 3);
        let mut ordered: Vec<_> = friend
            .iter()
            .chain(&grant)
            .chain(&checks)
            .copied()
            .collect();
        ordered.sort();
        assert_eq!(
            ordered,
            vec![friend[0], checks[0], grant[0], checks[1], grant[1], checks[2]]
        );
        for &i in friend.iter().chain(&grant) {
            assert_eq!(steps[i]["state_name"], "GameCore_Tuner");
        }
        for &i in &grant {
            assert!(codes[i].contains("CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES"));
        }
        assert!(codes[grant[0]].contains("ipairs({0})"));
        assert!(codes[grant[1]].contains("ipairs({1})"));
        assert!(codes[checks[0]].contains("valid==0"));
        assert!(codes[checks[1]].contains("valid==0"));
        assert!(codes[checks[2]].contains("valid>0"));
        let decided = find("EFFECT_ADJUST_PLAYER_GOVERNOR_POINTS");
        assert_eq!(decided.len(), 1);
        assert!(decided[0] < grant[0]);
        for needle in [
            "GetGovernorPoints",
            "TECH_JAC_SENGOKU_IRRIGATION_WORKS",
            "TECH_JAC_SENGOKU_REGIONAL_SUPPLY",
            "UNITCOMMAND_FORM_CORPS",
            "UNITCOMMAND_FORM_ARMY",
        ] {
            assert!(codes[decided[0]].contains(needle), "{needle}");
        }
        Ok(())
    }
}
