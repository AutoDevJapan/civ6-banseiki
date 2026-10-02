use anyhow::{anyhow, bail, ensure, Context, Result};
use regex::Regex;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::{apply_outputs, sql_quote, xml_escape, Outputs, Repository};

const EVENTS_JSON: &str = "documents/spec/data/sengoku-events.json";
const RESOURCES_JSON: &str = "documents/spec/data/japan-map-resources.json";
const SITES_LUA: &str = "poc/JAC_PoC_JapanMapScale/Scripts/JAC_ResourceSites.lua";
const STARTING_KNOWLEDGE_LUA: &str =
    "poc/JAC_PoC_SengokuTrees/Scripts/JAC_Sengoku_StartingKnowledge.lua";
const MOD: &str = "poc/JAC_PoC_HistoricalEvents";
#[cfg(test)]
const TREES_SQL: &str = "poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Trees.sql";
const LANGS: [(&str, &str); 3] = [("ja", "ja_JP"), ("en", "en_US"), ("ru", "ru_RU")];

fn array(value: &Value) -> Result<&[Value]> {
    value
        .as_array()
        .map(Vec::as_slice)
        .context("expected a JSON array")
}

fn string(value: &Value) -> Result<&str> {
    value.as_str().context("expected a JSON string")
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}

fn sql_literal(value: &Value) -> Result<String> {
    match value {
        Value::Null => Ok("NULL".to_owned()),
        Value::String(value) => Ok(sql_quote(value)),
        Value::Number(value) if value.is_i64() || value.is_u64() => Ok(value.to_string()),
        _ => bail!("expected a string, integer, or null for SQL: {value}"),
    }
}

pub fn event_type(event_id: &str) -> Result<String> {
    let suffix = event_id
        .strip_prefix("EV_")
        .with_context(|| format!("event ID must start with EV_: {event_id}"))?;
    Ok(format!("JAC_EVENT_{suffix}"))
}

fn civilization(faction: &str) -> String {
    format!("CIVILIZATION_JAC_{faction}")
}

pub fn starting_knowledge(repo: &Repository) -> Result<BTreeSet<String>> {
    let source = repo.read(STARTING_KNOWLEDGE_LUA)?;
    let expression = Regex::new(r#""((?:TECH|CIVIC)_JAC_SENGOKU_[A-Z_]+)""#)?;
    let found: BTreeSet<String> = expression
        .captures_iter(&source)
        .map(|capture| capture[1].to_owned())
        .collect();
    ensure!(
        !found.is_empty(),
        "no starting knowledge found in {STARTING_KNOWLEDGE_LUA}"
    );
    Ok(found)
}

type ResourceKey = (String, u64, u64);

fn coordinate_bits(value: f64) -> u64 {
    if value == 0.0 {
        0.0_f64.to_bits()
    } else {
        value.to_bits()
    }
}

fn resource_key(resource: &str, longitude: f64, latitude: f64) -> Result<ResourceKey> {
    ensure!(
        longitude.is_finite() && latitude.is_finite(),
        "resource coordinates must be finite"
    );
    Ok((
        resource.to_owned(),
        coordinate_bits(longitude),
        coordinate_bits(latitude),
    ))
}

fn point_key(resource: &str, point: &Value) -> Result<ResourceKey> {
    let coordinates = array(&point["coordinates"])?;
    ensure!(
        coordinates.len() == 2,
        "resource coordinates must be [lon, lat]"
    );
    resource_key(
        resource,
        coordinates[0].as_f64().context("invalid longitude")?,
        coordinates[1].as_f64().context("invalid latitude")?,
    )
}

fn event_resource_key(resource: &Value) -> Result<ResourceKey> {
    let location = string(&resource["location"])?;
    let expression = Regex::new(r"\((-?[\d.]+), (-?[\d.]+)\)$")?;
    let capture = expression
        .captures(location)
        .with_context(|| format!("invalid resource location: {location}"))?;
    resource_key(
        string(&resource["resource_id"])?,
        capture[2].parse().context("invalid resource longitude")?,
        capture[1].parse().context("invalid resource latitude")?,
    )
}

fn resource_indexes(
    data: &Value,
) -> Result<(BTreeMap<ResourceKey, Value>, BTreeMap<ResourceKey, Value>)> {
    let mut points = BTreeMap::new();
    let mut trades = BTreeMap::new();
    for group in array(&data["resource_groups"])? {
        for point in array(&group["points"])? {
            points.insert(point_key(string(&group["civ6_id"])?, point)?, point.clone());
        }
    }
    for trade in array(&data["trade_access"])? {
        trades.insert(point_key(string(&trade["civ6_id"])?, trade)?, trade.clone());
    }
    Ok((points, trades))
}

#[derive(Debug, Clone)]
struct ReservedSite {
    #[cfg(test)]
    tech: Option<String>,
}

fn reserved_sites(repo: &Repository) -> Result<BTreeMap<String, Vec<ReservedSite>>> {
    fn evaluate(source: &str) -> mlua::Result<BTreeMap<String, Vec<ReservedSite>>> {
        let lua = mlua::Lua::new();
        lua.globals().set("ExposedMembers", lua.create_table()?)?;
        lua.load(source).set_name(SITES_LUA).exec()?;
        let exposed: mlua::Table = lua.globals().get("ExposedMembers")?;
        let maps: mlua::Table = exposed.get("JAC_ResourceSiteData")?;
        let mut sites: BTreeMap<String, Vec<ReservedSite>> = BTreeMap::new();
        for map in maps.pairs::<String, mlua::Table>() {
            let (_, rows) = map?;
            for row in rows.sequence_values::<mlua::Table>() {
                let row = row?;
                let id: String = row.get("id")?;
                sites.entry(id).or_default().push(ReservedSite {
                    #[cfg(test)]
                    tech: row.get("tech")?,
                });
            }
        }
        Ok(sites)
    }

    evaluate(&repo.read(SITES_LUA)?)
        .map_err(|error| anyhow!(error.to_string()))
        .context("evaluate resource site Lua")
}

struct ResourceSites {
    points: BTreeMap<ResourceKey, Value>,
    trades: BTreeMap<ResourceKey, Value>,
    reserved: BTreeSet<String>,
}

fn resource_sites(repo: &Repository) -> Result<ResourceSites> {
    let (points, trades) = resource_indexes(&repo.json(RESOURCES_JSON)?)?;
    let reserved = reserved_sites(repo)?.into_keys().collect();
    Ok(ResourceSites {
        points,
        trades,
        reserved,
    })
}

fn site_of(resource: &Value, sites: &ResourceSites) -> Result<String> {
    let key = event_resource_key(resource)?;
    let table = if resource["action"] == "trade_access" {
        &sites.trades
    } else {
        &sites.points
    };
    let point = table.get(&key).with_context(|| {
        format!(
            "no resource site for {}",
            resource["location"]
                .as_str()
                .unwrap_or("<invalid location>")
        )
    })?;
    Ok(string(&point["id"])?.to_owned())
}

fn translated(value: &Value) -> Result<[String; 3]> {
    Ok([
        string(&value["ja"])?.to_owned(),
        string(&value["en"])?.to_owned(),
        string(&value["ru"])?.to_owned(),
    ])
}

fn rename_names(name: &str) -> Result<(&'static str, &'static str, &'static str)> {
    match name {
        "上杉" => Ok(("UESUGI", "Uesugi", "Уэсуги")),
        "徳川" => Ok(("TOKUGAWA", "Tokugawa", "Токугава")),
        "羽柴" => Ok(("HASHIBA", "Hashiba", "Хасиба")),
        "豊臣" => Ok(("TOYOTOMI", "Toyotomi", "Тоётоми")),
        _ => bail!("unsupported house rename: {name}"),
    }
}

fn fixed_text() -> BTreeMap<String, [String; 3]> {
    [
        (
            "LOC_JAC_EVENT_SUMMARY",
            [
                "史実の出来事が起きた。",
                "A historical event has taken place.",
                "Произошло историческое событие.",
            ],
        ),
        (
            "LOC_JAC_EVENT_BOOST_PLACEHOLDER",
            [
                "史実の出来事の年に得られる（暫定）。",
                "Granted when the historical event's year arrives (placeholder).",
                "Даётся в год исторического события (временно).",
            ],
        ),
        (
            "LOC_JAC_EVENT_ADVISOR_OK",
            ["承知した", "Understood", "Понятно"],
        ),
    ]
    .into_iter()
    .map(|(key, values)| (key.to_owned(), values.map(str::to_owned)))
    .collect()
}

fn append_values(output: &mut String, prefix: &str, rows: &[Vec<String>]) {
    if rows.is_empty() {
        return;
    }
    output.push_str(prefix);
    for (index, row) in rows.iter().enumerate() {
        if index != 0 {
            output.push_str(",\n");
        }
        output.push_str("\t(");
        output.push_str(&row.join(", "));
        output.push(')');
    }
    output.push_str(";\n");
}

fn build_events(
    repo: &Repository,
    events: &[Value],
    start: &BTreeSet<String>,
) -> Result<(String, String)> {
    for event in events {
        let mut known = BTreeSet::new();
        for research in array(&event["earlier_by"])? {
            let id = string(&research["id"])?;
            if start.contains(id) {
                known.insert(id.to_owned());
            }
        }
        if truthy(&event["boost"]) {
            let id = string(&event["boost"]["id"])?;
            if start.contains(id) {
                known.insert(id.to_owned());
            }
        }
        ensure!(
            known.is_empty(),
            "{}: earlier_by/boost names starting knowledge {:?}",
            string(&event["id"])?,
            known
        );
    }

    let sites = resource_sites(repo)?;
    let mut rows = Vec::new();
    let mut prereq_techs = Vec::new();
    let mut prereq_civics = Vec::new();
    let mut targets = Vec::new();
    let mut renames = Vec::new();
    let mut resources = Vec::new();
    let mut boosts = BTreeMap::<String, String>::new();
    let mut text = fixed_text();

    for event in events {
        let id = string(&event["id"])?;
        let et = event_type(id)?;
        let suffix = et.strip_prefix("JAC_EVENT_").unwrap();
        let name_key = format!("LOC_JAC_EVENT_{suffix}_NAME");
        text.insert(name_key.clone(), translated(&event["name"])?);

        let (house, generic_key) = if truthy(&event["house_specific"]) {
            let house = civilization(string(&event["house"])?);
            let key = format!("LOC_JAC_EVENT_{suffix}_GENERIC_NAME");
            text.insert(key.clone(), translated(&event["generic_name"])?);
            (Some(house), Some(key))
        } else {
            (None, None)
        };

        let advisor = truthy(&event["advisor_popup"]);
        let description = if advisor {
            let key = format!("LOC_JAC_EVENT_{suffix}_ADVISOR");
            text.insert(key.clone(), translated(&event["advisor_popup"])?);
            key
        } else {
            "LOC_JAC_EVENT_SUMMARY".to_owned()
        };

        let mut boost_tech = "NULL".to_owned();
        let mut boost_civic = "NULL".to_owned();
        if truthy(&event["boost"]) {
            let boost = &event["boost"];
            let boost_id = string(&boost["id"])?;
            let kind = string(&boost["type"])?;
            match kind {
                "tech" => boost_tech = sql_quote(boost_id),
                "civic" => boost_civic = sql_quote(boost_id),
                _ => bail!("{id}: unsupported boost type {kind}"),
            }
            boosts.insert(boost_id.to_owned(), kind.to_owned());
        }

        rows.push(vec![
            sql_quote(&et),
            sql_quote("SENGOKU"),
            sql_literal(&event["year"])?,
            sql_literal(&event["month"])?,
            boost_tech,
            boost_civic,
            sql_quote(&name_key),
            sql_quote(&description),
            house
                .as_deref()
                .map(sql_quote)
                .unwrap_or_else(|| "NULL".into()),
            generic_key
                .as_deref()
                .map(sql_quote)
                .unwrap_or_else(|| "NULL".into()),
            i32::from(advisor).to_string(),
        ]);

        for research in array(&event["earlier_by"])? {
            let row = vec![sql_quote(&et), sql_quote(string(&research["id"])?)];
            if research["type"] == "tech" {
                prereq_techs.push(row);
            } else {
                prereq_civics.push(row);
            }
        }

        let affected = array(&event["affected_factions"])?;
        if !affected.iter().any(|faction| faction == "ALL") {
            for faction in affected {
                targets.push(vec![
                    sql_quote(&et),
                    sql_quote(&civilization(string(faction)?)),
                ]);
            }
        }

        if truthy(&event["rename"]) {
            let rename = &event["rename"];
            let to = string(&rename["to"])?;
            let (romaji, en, ru) = rename_names(to)?;
            let key = format!("LOC_JAC_EVENT_{romaji}_CIV_NAME");
            text.insert(key.clone(), [to.to_owned(), en.to_owned(), ru.to_owned()]);
            let by = &rename["by"];
            ensure!(
                truthy(by) && by["type"] == "civic" && by == &event["boost"],
                "{id}: rename.by must be a civic and the event's boost"
            );
            renames.push(vec![
                sql_quote(&et),
                sql_quote(&civilization(string(&rename["faction"])?)),
                sql_quote(&key),
                sql_quote(string(&by["id"])?),
            ]);
        }

        if truthy(&event["resource"]) {
            let resource = &event["resource"];
            let site = site_of(resource, &sites)?;
            ensure!(
                resource["action"] != "reveal" || sites.reserved.contains(&site),
                "{id}: {site} is not a reserved site of the resources mod"
            );
            let owner = if truthy(&resource["owner_to"]) {
                sql_quote(&civilization(string(&resource["owner_to"])?))
            } else {
                "NULL".to_owned()
            };
            resources.push(vec![
                sql_quote(&et),
                sql_quote(&site),
                sql_quote(string(&resource["action"])?),
                owner,
            ]);
        }
    }

    // Retain the original provenance headers so --check remains byte-compatible.
    let mut sql = String::from(
        "-- Generated by tools/generate_sengoku_events.py from documents/spec/data/sengoku-events.json. Do not edit.\n\
         -- Values are provisional (research draft); see documents/poc/historical-events.md.\n\n",
    );
    append_values(
        &mut sql,
        "INSERT INTO JAC_HistoricalEvents (EventType, Pack, Year, Month, BoostTech, BoostCivic, Name, Description, House, GenericName, Advisor) VALUES\n",
        &rows,
    );
    append_values(
        &mut sql,
        "\nINSERT INTO JAC_HistoricalEventPrereqTechs (EventType, TechnologyType) VALUES\n",
        &prereq_techs,
    );
    append_values(
        &mut sql,
        "\nINSERT INTO JAC_HistoricalEventPrereqCivics (EventType, CivicType) VALUES\n",
        &prereq_civics,
    );
    append_values(
        &mut sql,
        "\nINSERT INTO JAC_HistoricalEventTargets (EventType, CivilizationType) VALUES\n",
        &targets,
    );
    append_values(
        &mut sql,
        "\nINSERT INTO JAC_HistoricalEventRenames (EventType, CivilizationType, Name, CivicType) VALUES\n",
        &renames,
    );
    sql.push_str(
        "\n-- Records only. reveal: the reserved site of the resources mod, which reveals it in its own year or on its\n\
         -- research (SSOT 3.2: the event boosts that research instead); trade access opens by research and is not\n\
         -- implemented yet; ownership is never transferred automatically (SSOT 3.2: former domains are AI targets only).\n",
    );
    append_values(
        &mut sql,
        "INSERT INTO JAC_HistoricalEventResources (EventType, SiteId, Action, OwnerTo) VALUES\n",
        &resources,
    );
    sql.push_str(
        "\n-- TriggerBoost needs a Boosts row. Placeholder rows (暫定) for the event boosts the tree does not define;\n\
         -- the class never triggers by itself (official: Base/Assets/Gameplay/Data/Technologies.xml:440,505).\n",
    );
    for (boost_id, kind) in boosts {
        let (column, table) = if kind == "tech" {
            ("TechnologyType", "Technologies")
        } else {
            ("CivicType", "Civics")
        };
        let quoted = sql_quote(&boost_id);
        sql.push_str(&format!(
            "INSERT INTO Boosts ({column}, Boost, TriggerDescription, TriggerLongDescription, BoostClass)\n\
             SELECT {column}, 40, 'LOC_JAC_EVENT_BOOST_PLACEHOLDER', 'LOC_JAC_EVENT_BOOST_PLACEHOLDER', 'BOOST_TRIGGER_NONE_LATE_GAME_CRITICAL_TECH' FROM {table}\n\
             WHERE {column} = {quoted} AND NOT EXISTS (SELECT 1 FROM Boosts WHERE {column} = {quoted});\n"
        ));
    }

    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <!-- Generated by tools/generate_sengoku_events.py from documents/spec/data/sengoku-events.json. -->\n\
         <GameInfo>\n\t<LocalizedText>\n",
    );
    for (key, values) in text {
        for (index, (_, language)) in LANGS.iter().enumerate() {
            xml.push_str(&format!(
                "\t\t<Row Tag=\"{key}\" Language=\"{language}\"><Text>{}</Text></Row>\n",
                xml_escape(&values[index])
            ));
        }
    }
    xml.push_str("\t</LocalizedText>\n</GameInfo>\n");
    Ok((sql, xml))
}

pub fn build(repo: &Repository) -> Result<(String, String)> {
    let data = repo.json(EVENTS_JSON)?;
    build_events(repo, array(&data["events"])?, &starting_knowledge(repo)?)
}

pub fn outputs(repo: &Repository) -> Result<Outputs> {
    let (sql, xml) = build(repo)?;
    Ok(BTreeMap::from([
        (
            PathBuf::from(format!("{MOD}/Data/JAC_HistoricalEvents_Sengoku.sql")),
            sql,
        ),
        (
            PathBuf::from(format!("{MOD}/Text/JAC_HistoricalEvents_Text.xml")),
            xml,
        ),
    ]))
}

pub fn cli(repo: &Repository, args: &[String]) -> Result<()> {
    let Some(check) = crate::parse_check_args(
        "generate-sengoku-events",
        args,
        "Generate Sengoku event files.",
    )?
    else {
        return Ok(());
    };
    apply_outputs(repo, outputs(repo)?, check)?;
    println!(
        "{}",
        if check {
            "Sengoku event files current"
        } else {
            "Sengoku event files generated"
        }
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, ToSql};
    use serde_json::json;

    fn s(value: &Value) -> &str {
        value.as_str().expect("expected a string in event data")
    }

    fn a(value: &Value) -> &[Value] {
        value.as_array().expect("expected an array in event data")
    }

    fn events(repo: &Repository) -> Result<Vec<Value>> {
        Ok(array(&repo.json(EVENTS_JSON)?["events"])?.to_vec())
    }

    fn set(values: &[&str]) -> BTreeSet<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    fn strings(values: &Value) -> BTreeSet<String> {
        a(values).iter().map(|value| s(value).to_owned()).collect()
    }

    fn factions(repo: &Repository) -> Result<BTreeSet<String>> {
        Ok(
            array(&repo.json("documents/spec/data/japan-1534-start.json")?["factions"])?
                .iter()
                .map(|faction| s(&faction["id"]).to_owned())
                .collect(),
        )
    }

    fn query(db: &Connection, sql: &str, parameters: &[&dyn ToSql]) -> Result<Vec<Value>> {
        let mut statement = db.prepare(sql)?;
        let columns = statement.column_count();
        let mut cursor = statement.query(parameters)?;
        let mut result = Vec::new();
        while let Some(row) = cursor.next()? {
            let mut values = Vec::with_capacity(columns);
            for column in 0..columns {
                let value: rusqlite::types::Value = row.get(column)?;
                values.push(match value {
                    rusqlite::types::Value::Null => Value::Null,
                    rusqlite::types::Value::Integer(value) => json!(value),
                    rusqlite::types::Value::Real(value) => json!(value),
                    rusqlite::types::Value::Text(value) => Value::String(value),
                    rusqlite::types::Value::Blob(_) => bail!("unexpected BLOB in event query"),
                });
            }
            result.push(Value::Array(values));
        }
        Ok(result)
    }

    fn query_strings(
        db: &Connection,
        sql: &str,
        parameters: &[&dyn ToSql],
    ) -> Result<BTreeSet<String>> {
        let mut result = BTreeSet::new();
        for row in query(db, sql, parameters)? {
            if let Some(value) = row[0].as_str() {
                result.insert(value.to_owned());
            }
        }
        Ok(result)
    }

    fn count(db: &Connection, sql: &str) -> Result<i64> {
        Ok(db.query_row(sql, [], |row| row.get(0))?)
    }

    fn sengoku_types(repo: &Repository, kind: &str) -> Result<Vec<String>> {
        let expression = Regex::new(&format!(r"\('([A-Z_]+)', '{}'\)", regex::escape(kind)))?;
        Ok(expression
            .captures_iter(&repo.read(TREES_SQL)?)
            .map(|capture| capture[1].to_owned())
            .collect())
    }

    fn load_db(repo: &Repository) -> Result<Connection> {
        let db = Connection::open_in_memory()?;
        db.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE GlobalParameters (Name TEXT PRIMARY KEY, Value TEXT);
             CREATE TABLE Technologies (TechnologyType TEXT PRIMARY KEY);
             CREATE TABLE Civics (CivicType TEXT PRIMARY KEY);
             CREATE TABLE Boosts (
                 BoostID INTEGER PRIMARY KEY,
                 TechnologyType TEXT,
                 CivicType TEXT,
                 Boost INTEGER NOT NULL,
                 TriggerDescription TEXT NOT NULL,
                 TriggerLongDescription TEXT NOT NULL,
                 BoostClass TEXT NOT NULL,
                 Unit1Type TEXT,
                 ImprovementType TEXT,
                 ResourceType TEXT
             );
             CREATE TABLE Modifiers (ModifierId TEXT PRIMARY KEY, ModifierType TEXT NOT NULL);
             CREATE TABLE ModifierArguments (
                 ModifierId TEXT, Name TEXT, Value TEXT,
                 PRIMARY KEY (ModifierId, Name)
             );",
        )?;
        for technology in sengoku_types(repo, "KIND_TECH")? {
            db.execute("INSERT INTO Technologies VALUES (?)", [&technology])?;
        }
        for civic in sengoku_types(repo, "KIND_CIVIC")? {
            db.execute("INSERT INTO Civics VALUES (?)", [&civic])?;
        }
        let trees = repo.read(TREES_SQL)?;
        for statement in Regex::new(r"(?m)^INSERT INTO Boosts .*?;$")?.find_iter(&trees) {
            db.execute_batch(statement.as_str())?;
        }
        db.execute_batch(&repo.read(format!("{MOD}/Data/JAC_HistoricalEvents_Schema.sql"))?)?;
        db.execute_batch(&build(repo)?.0)?;
        Ok(db)
    }

    fn assert_integrity_error(error: rusqlite::Error) {
        match error {
            rusqlite::Error::SqliteFailure(error, _) => {
                assert_eq!(error.code, rusqlite::ErrorCode::ConstraintViolation);
            }
            other => panic!("expected an integrity error, got {other}"),
        }
    }

    fn turn_for_year(start: i64, stages: &[(i64, i64)], year: i64, month: Option<i64>) -> i64 {
        let target = (year - start) * 12 + month.unwrap_or(1) - 1;
        let mut months = 0;
        let mut turn = 1;
        for &(increment, turns) in stages {
            assert!(increment > 0 && turns > 0);
            let span = turns * increment;
            if months + span >= target {
                let delta = target - months;
                let ceiling = -(-delta).div_euclid(increment);
                return turn + ceiling.max(0);
            }
            months += span;
            turn += turns;
        }
        turn
    }

    fn months_of_turn(stages: &[(i64, i64)], turn: i64) -> i64 {
        let mut months = 0;
        let mut remaining = turn - 1;
        for &(increment, turns) in stages {
            let used = remaining.min(turns);
            months += used * increment;
            remaining -= used;
        }
        months
    }

    fn lua_turn_function(
        source: &str,
        start: i64,
        stages: &[(i64, i64)],
    ) -> Result<(mlua::Lua, mlua::Function)> {
        fn evaluate(
            source: &str,
            start: i64,
            stages: &[(i64, i64)],
        ) -> mlua::Result<(mlua::Lua, mlua::Function)> {
            let lua = mlua::Lua::new();
            lua.globals().set("START_YEAR", start)?;
            let rows = lua.create_table()?;
            for (index, &(increment, turns)) in stages.iter().enumerate() {
                let row = lua.create_table()?;
                row.set("MonthIncrement", increment)?;
                row.set("TurnsPerIncrement", turns)?;
                rows.set(index + 1, row)?;
            }
            lua.globals().set("STAGES", rows)?;
            let function = lua
                .load(source)
                .set_name("historical event calendar")
                .eval()?;
            Ok((lua, function))
        }

        let begin = source
            .find("local function TurnForYear(year, month)")
            .context("TurnForYear function missing")?;
        let end = source[begin..]
            .find("local function GameTurn()")
            .map(|offset| begin + offset)
            .context("GameTurn function missing")?;
        let chunk = format!("{}\nreturn TurnForYear", &source[begin..end]);
        evaluate(&chunk, start, stages).map_err(|error| anyhow!(error.to_string()))
    }

    fn call_lua_turn(function: &mlua::Function, year: i64, month: Option<i64>) -> Result<i64> {
        function
            .call((year, month))
            .map_err(|error| anyhow!(error.to_string()))
    }

    // Minimal resource-placement dependency, ported from the original map generator.
    // These functions consume JSON directly and never inspect Python at runtime.
    fn resource_is_deferred(point: &Value) -> bool {
        point["available_at_start"] == false
            || point["start_year"].as_i64().is_some_and(|year| year > 1534)
    }

    fn reveal_year(point: &Value) -> Option<i64> {
        point["start_year"].as_i64().or_else(|| {
            point["start_year_range"]
                .as_array()
                .and_then(|span| span.first())
                .and_then(Value::as_i64)
        })
    }

    fn reveal_tech(id: &str) -> Option<&'static str> {
        match id {
            "silver-ikuno" | "silver-sado-tsurushi" => Some("TECH_JAC_SENGOKU_CUPELLATION"),
            "copper-ashio" => Some("TECH_JAC_SENGOKU_ORE_PROSPECTING"),
            "gold-izu-toi"
            | "gold-noto-hodatsu"
            | "gold-sado-nishimikawa"
            | "gold-sado-aikawa"
            | "silver-nobesawa" => Some("TECH_JAC_SENGOKU_MINE_MANAGEMENT"),
            _ => None,
        }
    }

    fn trade_access_status(item: &Value) -> &'static str {
        if truthy(&item["unlock_civic"]) {
            "unlocked_by_research"
        } else if item["available_from_year"]
            .as_i64()
            .is_some_and(|year| year > 1534)
        {
            "deferred_until_year"
        } else {
            "undated_trade_access_not_placed"
        }
    }

    #[test]
    fn test_generated_files_are_current() -> Result<()> {
        let repo = Repository::checked_in();
        for (path, content) in outputs(&repo)? {
            assert_eq!(
                repo.read(&path)?,
                content,
                "stale generated file: {}",
                path.display()
            );
        }
        Ok(())
    }

    #[test]
    fn test_no_prerequisite_or_boost_is_starting_knowledge() -> Result<()> {
        let repo = Repository::checked_in();
        let start = starting_knowledge(&repo)?;
        assert_eq!(start.len(), 7);
        let db = load_db(&repo)?;
        let used = query_strings(
            &db,
            "SELECT TechnologyType FROM JAC_HistoricalEventPrereqTechs UNION
             SELECT CivicType FROM JAC_HistoricalEventPrereqCivics UNION
             SELECT BoostTech FROM JAC_HistoricalEvents UNION
             SELECT BoostCivic FROM JAC_HistoricalEvents",
            &[],
        )?;
        assert_eq!(
            used.intersection(&start).cloned().collect::<BTreeSet<_>>(),
            BTreeSet::<String>::new()
        );
        Ok(())
    }

    #[test]
    fn test_generator_refuses_starting_knowledge_in_the_list() -> Result<()> {
        let repo = Repository::checked_in();
        let error = build_events(
            &repo,
            &events(&repo)?,
            &set(&["TECH_JAC_SENGOKU_GRAIN_STORES"]),
        )
        .unwrap_err();
        assert!(error.to_string().contains("EV_TENBUN_FAMINE"));
        Ok(())
    }

    #[test]
    fn test_sql_loads_with_valid_references() -> Result<()> {
        let repo = Repository::checked_in();
        let db = load_db(&repo)?;
        assert_eq!(
            query(&db, "PRAGMA foreign_key_check", &[])?,
            Vec::<Value>::new()
        );
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM JAC_HistoricalEvents")?,
            events(&repo)?.len() as i64
        );
        assert_integrity_error(
            db.execute(
                "INSERT INTO JAC_HistoricalEvents (EventType, Pack, Name, Description)
                 VALUES ('X', 'SENGOKU', 'LOC_X', 'LOC_X')",
                [],
            )
            .unwrap_err(),
        );
        assert_integrity_error(
            db.execute(
                "INSERT INTO JAC_HistoricalEventPrereqTechs VALUES
                 ('JAC_EVENT_TEPPO', 'CIVIC_JAC_SENGOKU_DOMAIN_LAW')",
                [],
            )
            .unwrap_err(),
        );
        Ok(())
    }

    #[test]
    fn test_rows_follow_the_event_list() -> Result<()> {
        let repo = Repository::checked_in();
        let db = load_db(&repo)?;
        for event in events(&repo)? {
            let et = event_type(s(&event["id"]))?;
            let rows = query(
                &db,
                "SELECT Year, Month, BoostTech, BoostCivic, House, Advisor
                 FROM JAC_HistoricalEvents WHERE EventType = ?",
                &[&et],
            )?;
            assert_eq!(rows.len(), 1, "{et}");
            let row = &rows[0];
            assert_eq!(row[0], event["year"], "{et}");
            assert_eq!(row[1], event["month"], "{et}");
            let boost = if row[2].is_null() { &row[3] } else { &row[2] };
            assert_eq!(boost, &event["boost"]["id"], "{et}");
            let house = if truthy(&event["house_specific"]) {
                json!(civilization(s(&event["house"])))
            } else {
                Value::Null
            };
            assert_eq!(row[4], house, "{et}");
            assert_eq!(
                row[5],
                json!(i32::from(truthy(&event["advisor_popup"]))),
                "{et}"
            );
            let prerequisites = query_strings(
                &db,
                "SELECT TechnologyType FROM JAC_HistoricalEventPrereqTechs
                 WHERE EventType = ?1 UNION ALL
                 SELECT CivicType FROM JAC_HistoricalEventPrereqCivics
                 WHERE EventType = ?1",
                &[&et],
            )?;
            let expected: BTreeSet<_> = a(&event["earlier_by"])
                .iter()
                .map(|research| s(&research["id"]).to_owned())
                .collect();
            assert_eq!(prerequisites, expected, "{et}");
            let targets = query_strings(
                &db,
                "SELECT CivilizationType FROM JAC_HistoricalEventTargets WHERE EventType = ?",
                &[&et],
            )?;
            let expected = if strings(&event["affected_factions"]).contains("ALL") {
                BTreeSet::new()
            } else {
                a(&event["affected_factions"])
                    .iter()
                    .map(|faction| civilization(s(faction)))
                    .collect()
            };
            assert_eq!(targets, expected, "{et}");
        }
        assert_eq!(
            query(
                &db,
                "SELECT SUM(Advisor), COUNT(BoostTech) + COUNT(BoostCivic)
                 FROM JAC_HistoricalEvents",
                &[],
            )?,
            vec![json!([19, 55])]
        );
        Ok(())
    }

    #[test]
    fn test_renames_are_the_ssot_stages() -> Result<()> {
        let repo = Repository::checked_in();
        let db = load_db(&repo)?;
        assert_eq!(
            query(
                &db,
                "SELECT r.EventType, r.CivilizationType, r.Name, e.Year, e.Month, r.CivicType
                 FROM JAC_HistoricalEventRenames r
                 JOIN JAC_HistoricalEvents e USING (EventType)
                 ORDER BY e.Year, e.Month",
                &[],
            )?,
            vec![
                json!([
                    "JAC_EVENT_NAGAO_TO_UESUGI",
                    "CIVILIZATION_JAC_NAGAO",
                    "LOC_JAC_EVENT_UESUGI_CIV_NAME",
                    1561,
                    4,
                    "CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS"
                ]),
                json!([
                    "JAC_EVENT_MATSUDAIRA_TOKUGAWA",
                    "CIVILIZATION_JAC_MATSUDAIRA",
                    "LOC_JAC_EVENT_TOKUGAWA_CIV_NAME",
                    1567,
                    1,
                    "CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS"
                ]),
                json!([
                    "JAC_EVENT_HONNOJI",
                    "CIVILIZATION_JAC_ODA",
                    "LOC_JAC_EVENT_HASHIBA_CIV_NAME",
                    1582,
                    6,
                    "CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS"
                ]),
                json!([
                    "JAC_EVENT_TOYOTOMI_SURNAME",
                    "CIVILIZATION_JAC_ODA",
                    "LOC_JAC_EVENT_TOYOTOMI_CIV_NAME",
                    1586,
                    10,
                    "CIVIC_JAC_SENGOKU_TRUCE_ARBITRATION"
                ]),
            ]
        );
        assert_eq!(
            count(
                &db,
                "SELECT COUNT(*) FROM JAC_HistoricalEventRenames r
                 JOIN JAC_HistoricalEvents e USING (EventType)
                 WHERE r.CivicType IS NOT e.BoostCivic",
            )?,
            0
        );
        Ok(())
    }

    #[test]
    fn test_year_to_turn_follows_the_sengoku_calendar() -> Result<()> {
        let repo = Repository::checked_in();
        let (start, stages) = crate::reigns::calendar(&repo)?;
        assert_eq!(start, 1534);
        assert!(!stages.is_empty());
        for (speed, rows) in &stages {
            assert_eq!(
                rows.iter().copied().collect::<BTreeSet<_>>().len(),
                rows.len(),
                "{speed}"
            );
        }
        let standard = stages
            .get("GAMESPEED_STANDARD")
            .context("standard game speed missing")?;
        assert_eq!(
            [1534, 1535, 1543, 1550, 1561, 1600]
                .map(|year| turn_for_year(start, standard, year, None)),
            [1, 4, 28, 49, 82, 199]
        );

        let lua = repo.read(format!("{MOD}/Scripts/JAC_HistoricalEvents.lua"))?;
        for (speed, rows) in &stages {
            let (_lua_runtime, function) = lua_turn_function(&lua, start, rows)?;
            for year in 1534..1700 {
                let turn = turn_for_year(start, rows, year, None);
                assert_eq!(
                    call_lua_turn(&function, year, None)?,
                    turn,
                    "{speed}/{year}"
                );
                assert_eq!(
                    start + months_of_turn(rows, turn).div_euclid(12),
                    year,
                    "{speed}/{year}"
                );
                assert!(
                    turn == 1 || start + months_of_turn(rows, turn - 1).div_euclid(12) == year - 1,
                    "{speed}/{year}"
                );
            }
            for month in 1..=12 {
                let turn = turn_for_year(start, rows, 1543, Some(month));
                let target = 9 * 12 + month - 1;
                assert_eq!(
                    call_lua_turn(&function, 1543, Some(month))?,
                    turn,
                    "{speed}/{month}"
                );
                assert!(months_of_turn(rows, turn) >= target, "{speed}/{month}");
                assert!(months_of_turn(rows, turn - 1) < target, "{speed}/{month}");
            }
        }

        let db = load_db(&repo)?;
        let mut due = BTreeMap::new();
        for row in query(
            &db,
            "SELECT EventType, Year, Month FROM JAC_HistoricalEvents",
            &[],
        )? {
            due.insert(
                s(&row[0]).to_owned(),
                turn_for_year(
                    start,
                    standard,
                    row[1].as_i64().context("event year missing")?,
                    row[2].as_i64(),
                ),
            );
        }
        assert_eq!(
            (
                due["JAC_EVENT_TENBUN_FAMINE"],
                due["JAC_EVENT_TEPPO"],
                due["JAC_EVENT_NAGAO_TO_UESUGI"]
            ),
            (19, 30, 83)
        );
        assert!(
            due.values().all(|turn| 1 < *turn && *turn <= 200),
            "{due:?}"
        );
        assert!(lua.contains("(month or 1) - 1"));
        assert!(lua.contains(".GameSpeedTurnCollection"));
        assert!(!lua.contains("GameInfo.GameSpeed_Turns()"));
        assert!(!lua.contains("table.sort(STAGES"));
        assert!(!lua.contains("TURNS_PER_YEAR"));
        assert!(lua.contains("Game.GetCurrentGameTurn() - GameConfiguration.GetStartTurn() + 1"));
        assert_eq!(
            Regex::new(r"Game\.GetCurrentGameTurn\(\)")?
                .find_iter(&lua)
                .count(),
            1
        );
        let scenario = repo.read("tools/civ6-autotest/scenarios/historical-events-15km.json")?;
        assert!(!scenario.contains("Game.GetCurrentGameTurn()~="));
        Ok(())
    }

    #[test]
    fn test_resource_events_boost_the_research_of_the_reserved_sites() -> Result<()> {
        let repo = Repository::checked_in();
        let lua = repo.read(format!("{MOD}/Scripts/JAC_HistoricalEvents.lua"))?;
        for forbidden in [
            "RevealSite",
            "JAC_HistoricalEventResources",
            "ResourceBuilder",
            "TransferCity",
        ] {
            assert!(!lua.contains(forbidden), "{forbidden}");
        }
        let sites = reserved_sites(&repo)?;
        let db = load_db(&repo)?;
        let rows = query(
            &db,
            "SELECT EventType, SiteId, Action, OwnerTo
             FROM JAC_HistoricalEventResources ORDER BY EventType",
            &[],
        )?;
        assert_eq!(
            rows,
            vec![
                json!(["JAC_EVENT_AIKAWA", "gold-sado-aikawa", "reveal", null]),
                json!(["JAC_EVENT_AIZU_LACQUER", "lacquer-aizu", "reveal", null]),
                json!([
                    "JAC_EVENT_AMAGO_FALL",
                    "iron-izumo-yokota",
                    "ownership",
                    "CIVILIZATION_JAC_MORI"
                ]),
                json!([
                    "JAC_EVENT_HIRADO_PORTUGUESE",
                    "niter-hirado",
                    "trade_access",
                    null
                ]),
                json!(["JAC_EVENT_IKUNO_SILVER", "silver-ikuno", "reveal", null]),
                json!([
                    "JAC_EVENT_MORI_IWAMI",
                    "silver-iwami",
                    "ownership",
                    "CIVILIZATION_JAC_MORI"
                ]),
                json!([
                    "JAC_EVENT_SADO_SUBJUGATION",
                    "gold-sado-nishimikawa",
                    "reveal",
                    null
                ]),
            ]
        );
        for row in rows {
            if row[2] != "reveal" {
                continue;
            }
            let event = s(&row[0]);
            let site_id = s(&row[1]);
            assert!(sites.contains_key(site_id), "{site_id}");
            let techs: BTreeSet<String> = sites[site_id]
                .iter()
                .filter_map(|site| site.tech.clone())
                .collect();
            let prerequisites = query_strings(
                &db,
                "SELECT TechnologyType FROM JAC_HistoricalEventPrereqTechs WHERE EventType = ?",
                &[&event],
            )?;
            assert!(techs.is_subset(&prerequisites), "{site_id}");
            let boost: Option<String> = db.query_row(
                "SELECT BoostTech FROM JAC_HistoricalEvents WHERE EventType = ?",
                [event],
                |row| row.get(0),
            )?;
            if !techs.is_empty() {
                assert_eq!(
                    techs,
                    BTreeSet::from([boost.context("reserved site boost missing")?]),
                    "{site_id}"
                );
            }
        }
        assert_eq!(sites["gold-sado-nishimikawa"].len(), 1);
        Ok(())
    }

    #[test]
    fn test_missing_prereq_gets_a_boost_not_the_tech() -> Result<()> {
        let repo = Repository::checked_in();
        let db = load_db(&repo)?;
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM JAC_HistoricalEventModifiers")?,
            0
        );
        let mut boosted = BTreeSet::new();
        for row in query(&db, "SELECT TechnologyType, CivicType FROM Boosts", &[])? {
            boosted.insert(s(if row[0].is_null() { &row[1] } else { &row[0] }).to_owned());
        }
        let mut wanted = BTreeSet::new();
        for row in query(
            &db,
            "SELECT BoostTech, BoostCivic FROM JAC_HistoricalEvents
             WHERE BoostTech IS NOT NULL OR BoostCivic IS NOT NULL",
            &[],
        )? {
            wanted.insert(s(if row[0].is_null() { &row[1] } else { &row[0] }).to_owned());
        }
        assert!(wanted.is_subset(&boosted));
        assert_eq!(
            count(&db, "SELECT COUNT(*) FROM Boosts")?,
            boosted.len() as i64
        );
        assert_eq!(
            query_strings(
                &db,
                "SELECT BoostClass FROM Boosts
                 WHERE TriggerDescription = 'LOC_JAC_EVENT_BOOST_PLACEHOLDER'",
                &[],
            )?,
            set(&["BOOST_TRIGGER_NONE_LATE_GAME_CRITICAL_TECH"])
        );
        let class: String = db.query_row(
            "SELECT BoostClass FROM Boosts
             WHERE TechnologyType = 'TECH_JAC_SENGOKU_CUPELLATION'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(class, "BOOST_TRIGGER_IMPROVE_SPECIFIC_RESOURCE");
        let lua = repo.read(format!("{MOD}/Scripts/JAC_HistoricalEvents.lua"))?;
        assert!(lua.contains(":TriggerBoost(index, 1)"));
        assert!(lua.contains("reason == \"year\""));
        Ok(())
    }

    #[test]
    fn test_every_text_key_has_three_languages() -> Result<()> {
        let repo = Repository::checked_in();
        let db = load_db(&repo)?;
        let mut keys = BTreeSet::new();
        for row in query(
            &db,
            "SELECT Name, Description, GenericName FROM JAC_HistoricalEvents",
            &[],
        )? {
            for value in a(&row) {
                if let Some(key) = value.as_str().filter(|key| !key.is_empty()) {
                    keys.insert(key.to_owned());
                }
            }
        }
        keys.extend(query_strings(
            &db,
            "SELECT Name FROM JAC_HistoricalEventRenames",
            &[],
        )?);
        for key in query_strings(&db, "SELECT TriggerDescription FROM Boosts", &[])? {
            if key.starts_with("LOC_JAC_EVENT_") {
                keys.insert(key);
            }
        }
        let ui = repo.read(format!("{MOD}/UI/JAC_HistoricalEventsAdvisor.lua"))?;
        keys.extend(
            Regex::new(r#""(LOC_JAC_EVENT_\w+)""#)?
                .captures_iter(&ui)
                .map(|capture| capture[1].to_owned()),
        );
        let xml = build(&repo)?.1;
        let root = roxmltree::Document::parse(&xml)?;
        let mut languages: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for row in root.descendants().filter(|node| node.has_tag_name("Row")) {
            let tag = row.attribute("Tag").context("localized row Tag missing")?;
            let text = row
                .children()
                .find(|node| node.has_tag_name("Text"))
                .and_then(|node| node.text());
            assert!(text.is_some_and(|text| !text.is_empty()), "{tag}");
            languages.entry(tag.to_owned()).or_default().insert(
                row.attribute("Language")
                    .context("localized row Language missing")?
                    .to_owned(),
            );
        }
        assert_eq!(languages.keys().cloned().collect::<BTreeSet<_>>(), keys);
        for key in keys {
            assert_eq!(languages[&key], set(&["ja_JP", "en_US", "ru_RU"]), "{key}");
        }
        Ok(())
    }

    #[test]
    fn test_modinfo_lists_files() -> Result<()> {
        let repo = Repository::checked_in();
        let source = repo.read(format!("{MOD}/JAC_PoC_HistoricalEvents.modinfo"))?;
        let document = roxmltree::Document::parse(&source)?;
        let root = document.root_element();
        let files = root
            .children()
            .find(|node| node.has_tag_name("Files"))
            .context("modinfo Files missing")?;
        let listed: BTreeSet<String> = files
            .children()
            .filter(|node| node.is_element())
            .map(|node| node.text().expect("listed file missing").to_owned())
            .collect();
        let directory = repo.path(MOD);
        let mut on_disk = BTreeSet::new();
        for entry in walkdir::WalkDir::new(&directory) {
            let entry = entry?;
            if entry.file_type().is_file()
                && entry.path().extension().and_then(|value| value.to_str()) != Some("modinfo")
            {
                on_disk.insert(
                    entry
                        .path()
                        .strip_prefix(&directory)?
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
        assert_eq!(listed, on_disk);
        let actions = root
            .children()
            .find(|node| node.has_tag_name("InGameActions"))
            .context("modinfo InGameActions missing")?;
        let used: BTreeSet<String> = actions
            .children()
            .filter(|node| node.is_element())
            .flat_map(|action| action.children())
            .filter(|node| node.has_tag_name("File"))
            .map(|node| node.text().expect("action file missing").to_owned())
            .collect();
        assert_eq!(used, on_disk);
        let ui = actions
            .children()
            .find(|node| node.has_tag_name("AddUserInterfaces"))
            .context("advisor UI action missing")?;
        let context = ui
            .children()
            .find(|node| node.has_tag_name("Properties"))
            .and_then(|properties| {
                properties
                    .children()
                    .find(|node| node.has_tag_name("Context"))
            })
            .and_then(|node| node.text());
        let file = ui
            .children()
            .find(|node| node.has_tag_name("File"))
            .and_then(|node| node.text());
        assert_eq!(
            (context, file),
            (Some("InGame"), Some("UI/JAC_HistoricalEventsAdvisor.xml"))
        );
        Ok(())
    }

    #[test]
    fn test_lua_reads_only_existing_tables_and_properties() -> Result<()> {
        let repo = Repository::checked_in();
        let lua = repo.read(format!("{MOD}/Scripts/JAC_HistoricalEvents.lua"))?;
        let schema = repo.read(format!("{MOD}/Data/JAC_HistoricalEvents_Schema.sql"))?;
        let mut tables: BTreeSet<String> = Regex::new(r"CREATE TABLE (\w+)")?
            .captures_iter(&schema)
            .map(|capture| capture[1].to_owned())
            .collect();
        let referenced: BTreeSet<String> =
            Regex::new(r#""(JAC_HistoricalEvent\w*)"|GameInfo\.(JAC_\w+)"#)?
                .captures_iter(&lua)
                .map(|capture| {
                    capture
                        .get(1)
                        .or_else(|| capture.get(2))
                        .unwrap()
                        .as_str()
                        .to_owned()
                })
                .collect();
        tables.remove("JAC_HistoricalEventResources");
        assert_eq!(referenced, tables);
        let ui = repo.read("poc/JAC_PoC_CustomCivs/UI/JAC_CivNameOverride.lua")?;
        assert!(lua.contains("local CIV_NAME_PREFIX = \"JAC_CIV_NAME_\""));
        assert!(ui.contains("local CIV_NAME_PREFIX = \"JAC_CIV_NAME_\""));
        let advisor = repo.read(format!("{MOD}/UI/JAC_HistoricalEventsAdvisor.lua"))?;
        for prefix in [
            "FIRED_PREFIX = \"JAC_EVENT_FIRED_\"",
            "BOOSTED_PREFIX = \"JAC_EVENT_BOOSTED_\"",
        ] {
            assert!(lua.contains(&format!("local {prefix}")), "{prefix}");
            assert!(advisor.contains(&format!("local {prefix}")), "{prefix}");
        }
        assert!(advisor.contains("LuaEvents.TutorialUIRoot_AdvisorRaise(hmake AdvisorItem {"));
        assert!(advisor.contains("Game.GetLocalPlayer()"));
        assert!(!advisor.contains("SetProperty"));
        Ok(())
    }

    #[test]
    fn test_ages_ledger_scores_existing_events() -> Result<()> {
        let repo = Repository::checked_in();
        let ages = repo.read("poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Ages.sql")?;
        let scored: BTreeSet<String> = Regex::new(r"\('(JAC_EVENT_\w+)', \d\)")?
            .captures_iter(&ages)
            .map(|capture| capture[1].to_owned())
            .collect();
        assert!(!scored.is_empty());
        let ids: BTreeSet<String> = events(&repo)?
            .iter()
            .map(|event| event_type(s(&event["id"])))
            .collect::<Result<_>>()?;
        assert!(scored.is_subset(&ids));
        Ok(())
    }

    #[test]
    fn test_count_and_unique_ids() -> Result<()> {
        let repo = Repository::checked_in();
        let events = events(&repo)?;
        assert!((30..=70).contains(&events.len()), "{}", events.len());
        let ids: Vec<_> = events.iter().map(|event| s(&event["id"])).collect();
        assert_eq!(
            ids.len(),
            ids.iter().copied().collect::<BTreeSet<_>>().len()
        );
        Ok(())
    }

    #[test]
    fn test_factions_are_the_22_seats() -> Result<()> {
        let repo = Repository::checked_in();
        let factions = factions(&repo)?;
        assert_eq!(factions.len(), 22);
        for event in events(&repo)? {
            for faction in a(&event["affected_factions"])
                .iter()
                .chain(a(&event["primary_factions"]))
            {
                assert!(
                    faction == "ALL" || factions.contains(s(faction)),
                    "{}: {faction}",
                    event["id"]
                );
            }
            assert!(!a(&event["primary_factions"]).is_empty(), "{}", event["id"]);
        }
        Ok(())
    }

    #[test]
    fn test_earlier_by_ids_exist_in_trees() -> Result<()> {
        let repo = Repository::checked_in();
        let trees = repo.read(TREES_SQL)?;
        for event in events(&repo)? {
            for research in a(&event["earlier_by"]) {
                assert!(
                    trees.contains(&sql_quote(s(&research["id"]))),
                    "{}: {}",
                    event["id"],
                    research["id"]
                );
            }
        }
        Ok(())
    }

    #[test]
    fn test_years_month_and_names() -> Result<()> {
        let repo = Repository::checked_in();
        for event in events(&repo)? {
            let id = s(&event["id"]);
            let year = event["year"].as_i64().context("event year missing")?;
            assert!((1533..=1600).contains(&year), "{id}");
            assert!(
                event["month"].is_null()
                    || event["month"]
                        .as_i64()
                        .is_some_and(|month| (1..=12).contains(&month)),
                "{id}"
            );
            if truthy(&event["year_end"]) {
                let end = event["year_end"].as_i64().context("invalid year_end")?;
                assert!(end > year, "{id}");
                let end = end.to_string();
                assert!(
                    s(&event["date_text"]).contains(&end)
                        || s(&event["date_text"]).contains(&format!("〜{}", &end[2..])),
                    "{id}"
                );
            }
            assert!(
                ["ja", "en", "ru"]
                    .iter()
                    .all(|language| !s(&event["name"][*language]).is_empty()),
                "{id}"
            );
            assert!(
                !a(&event["sources"]).is_empty()
                    && ["high", "medium", "low"].contains(&s(&event["confidence"])),
                "{id}"
            );
        }
        Ok(())
    }

    #[test]
    fn test_month_set_when_date_text_names_a_month() -> Result<()> {
        let repo = Repository::checked_in();
        let expression = Regex::new(r"\d+月")?;
        for event in events(&repo)? {
            if expression.is_match(s(&event["date_text"])) {
                assert!(!event["month"].is_null(), "{}", event["id"]);
            }
        }
        Ok(())
    }

    #[test]
    fn test_boost_and_advisor_popup() -> Result<()> {
        let repo = Repository::checked_in();
        let events = events(&repo)?;
        for event in &events {
            let id = s(&event["id"]);
            let boost = &event["boost"];
            assert_eq!(boost.is_null(), a(&event["earlier_by"]).is_empty(), "{id}");
            if truthy(boost) {
                assert!(a(&event["earlier_by"]).contains(boost), "{id}");
                let (prefix, short) = match s(&boost["type"]) {
                    "tech" => ("TECH_JAC_SENGOKU_", "T"),
                    "civic" => ("CIVIC_JAC_SENGOKU_", "C"),
                    kind => bail!("{id}: unexpected boost type {kind}"),
                };
                assert!(
                    s(&boost["id"]).starts_with(prefix) && s(&boost["short"]).starts_with(short),
                    "{id}"
                );
            }
            let advisor = &event["advisor_popup"];
            if !advisor.is_null() {
                assert!(
                    ["ja", "en", "ru"]
                        .iter()
                        .all(|language| !s(&advisor[*language]).is_empty()),
                    "{id}"
                );
            }
        }
        assert!(events.iter().any(|event| {
            truthy(&event["advisor_popup"]) && s(&event["advisor_popup"]["ja"]).contains("和睦")
        }));
        Ok(())
    }

    #[test]
    fn test_ssot_decisions() -> Result<()> {
        let repo = Repository::checked_in();
        let events = events(&repo)?;
        let factions = factions(&repo)?;
        let ids: BTreeSet<_> = events.iter().map(|event| s(&event["id"])).collect();
        assert!(!ids.contains("EV_SEKIGAHARA"));
        for event in &events {
            assert!(
                event["house"].is_null() || factions.contains(s(&event["house"])),
                "{}",
                event["id"]
            );
        }
        let kenchi = events
            .iter()
            .find(|event| event["id"] == "EV_YAMAZAKI_KENCHI")
            .context("Yamazaki kenchi event missing")?;
        assert!(!truthy(&kenchi["house_specific"]));
        Ok(())
    }

    #[test]
    fn test_renames_chain_in_date_order() -> Result<()> {
        let repo = Repository::checked_in();
        type Stage = ((i64, Option<i64>), String, String, String);
        let mut stages: BTreeMap<String, Vec<Stage>> = BTreeMap::new();
        for event in events(&repo)? {
            let rename = &event["rename"];
            if truthy(rename) {
                assert!(
                    strings(&event["primary_factions"]).contains(s(&rename["faction"])),
                    "{}",
                    event["id"]
                );
                stages
                    .entry(s(&rename["faction"]).to_owned())
                    .or_default()
                    .push((
                        (
                            event["year"].as_i64().context("event year missing")?,
                            event["month"].as_i64(),
                        ),
                        s(&rename["from"]).to_owned(),
                        s(&rename["to"]).to_owned(),
                        s(&event["id"]).to_owned(),
                    ));
            }
        }
        let mut chains = BTreeMap::new();
        for (faction, mut rows) in stages {
            rows.sort();
            for pair in rows.windows(2) {
                assert!(pair[0].0 < pair[1].0, "{} / {}", pair[0].3, pair[1].3);
                assert_eq!(pair[0].2, pair[1].1, "{} / {}", pair[0].3, pair[1].3);
            }
            let mut chain = vec![rows[0].1.clone()];
            chain.extend(rows.into_iter().map(|row| row.2));
            chains.insert(faction, chain);
        }
        assert_eq!(
            chains,
            BTreeMap::from([
                (
                    "NAGAO".to_owned(),
                    vec!["長尾".to_owned(), "上杉".to_owned()]
                ),
                (
                    "MATSUDAIRA".to_owned(),
                    vec!["松平".to_owned(), "徳川".to_owned()]
                ),
                (
                    "ODA".to_owned(),
                    vec!["織田".to_owned(), "羽柴".to_owned(), "豊臣".to_owned()]
                ),
            ])
        );
        Ok(())
    }

    #[test]
    fn test_every_house_renames_by_research_only() -> Result<()> {
        let repo = Repository::checked_in();
        let events = events(&repo)?;
        let trees = repo.read(TREES_SQL)?;
        let by: BTreeMap<String, Value> = events
            .iter()
            .filter(|event| truthy(&event["rename"]))
            .map(|event| {
                (
                    s(&event["id"]).to_owned(),
                    event["rename"]["by"]["short"].clone(),
                )
            })
            .collect();
        assert_eq!(
            by,
            BTreeMap::from([
                ("EV_NAGAO_TO_UESUGI".to_owned(), json!("C16")),
                ("EV_MATSUDAIRA_TOKUGAWA".to_owned(), json!("C16")),
                ("EV_HONNOJI".to_owned(), json!("C16")),
                ("EV_TOYOTOMI_SURNAME".to_owned(), json!("C24")),
            ])
        );
        for event in events {
            if truthy(&event["rename"]) {
                assert_eq!(event["rename"]["by"], event["boost"], "{}", event["id"]);
                assert!(
                    trees.contains(&sql_quote(s(&event["boost"]["id"]))),
                    "{}",
                    event["id"]
                );
            }
        }
        assert!(Regex::new(
            r"\('CIVIC_JAC_SENGOKU_TRUCE_ARBITRATION', 'CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS'\)"
        )?
        .is_match(&trees));
        Ok(())
    }

    #[test]
    fn test_rakuichi_is_two_events() -> Result<()> {
        let repo = Repository::checked_in();
        let got: BTreeMap<String, Value> = events(&repo)?
            .into_iter()
            .filter(|event| event["id"] == "EV_RAKUICHI" || event["id"] == "EV_ROKKAKU_RAKUICHI")
            .map(|event| {
                (
                    s(&event["id"]).to_owned(),
                    json!([
                        event["year"],
                        event["month"],
                        event["affected_factions"],
                        event["house_specific"],
                        event["boost"]["short"]
                    ]),
                )
            })
            .collect();
        assert_eq!(
            got,
            BTreeMap::from([
                (
                    "EV_RAKUICHI".to_owned(),
                    json!([1567, 11, ["ODA"], false, "C17"])
                ),
                (
                    "EV_ROKKAKU_RAKUICHI".to_owned(),
                    json!([1549, 12, ["ROKKAKU"], false, "C15"])
                ),
            ])
        );
        Ok(())
    }

    #[test]
    fn test_catholicism_spreads_by_research_not_a_fixed_order() -> Result<()> {
        let repo = Repository::checked_in();
        let events = events(&repo)?;
        let event = events
            .iter()
            .find(|event| event["id"] == "EV_CHRISTIAN")
            .context("Christian event missing")?;
        let short: BTreeSet<String> = a(&event["earlier_by"])
            .iter()
            .map(|research| s(&research["short"]).to_owned())
            .collect();
        assert_eq!(short, set(&["C19", "C13"]));
        assert_eq!(event["affected_factions"], json!(["ALL"]));
        assert!(!s(&event["gameplay_effect"]).contains('→'));
        Ok(())
    }

    #[test]
    fn test_former_domains_are_ai_targets_only() -> Result<()> {
        let repo = Repository::checked_in();
        let events = events(&repo)?;
        let mut targeted: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for event in &events {
            let id = s(&event["id"]);
            if let Some(targets) = event["ai_targets"].as_object() {
                for (faction, provinces) in targets {
                    assert!(
                        strings(&event["affected_factions"]).contains(faction),
                        "{id}"
                    );
                    assert!(
                        !a(provinces).is_empty()
                            && strings(provinces).is_subset(&strings(&event["regions"])),
                        "{id}"
                    );
                    targeted
                        .entry(id.to_owned())
                        .or_default()
                        .insert(faction.clone());
                }
            }
            let resource = &event["resource"];
            if resource["action"] == "ownership" {
                assert!(
                    targeted
                        .get(id)
                        .is_some_and(|factions| factions.contains(s(&resource["owner_to"]))),
                    "{id}"
                );
            }
        }
        assert!(
            set(&["EV_TAKEDA_FALL", "EV_MATSUDAIRA_KANTO", "EV_KANTO_KANREI"])
                .is_subset(&targeted.keys().cloned().collect())
        );
        assert!(!serde_json::to_string(&events)?.contains("所有移転の候補"));
        Ok(())
    }

    #[test]
    fn test_house_specific_has_generic_name() -> Result<()> {
        let repo = Repository::checked_in();
        for event in events(&repo)? {
            if truthy(&event["house_specific"]) {
                assert!(
                    truthy(&event["house"])
                        && ["ja", "en", "ru"]
                            .iter()
                            .all(|language| !s(&event["generic_name"][*language]).is_empty()),
                    "{}",
                    event["id"]
                );
            }
        }
        Ok(())
    }

    #[test]
    fn test_required_events_present_and_every_faction_covered() -> Result<()> {
        let repo = Repository::checked_in();
        let events = events(&repo)?;
        let ids: BTreeSet<String> = events
            .iter()
            .map(|event| s(&event["id"]).to_owned())
            .collect();
        assert!(set(&[
            "EV_TEPPO",
            "EV_CHRISTIAN",
            "EV_YAMAZAKI_KENCHI",
            "EV_NAGAO_TO_UESUGI",
            "EV_AIKAWA",
            "EV_IKUNO_SILVER"
        ])
        .is_subset(&ids));
        let seen: BTreeSet<String> = events
            .iter()
            .flat_map(|event| {
                a(&event["affected_factions"])
                    .iter()
                    .chain(a(&event["primary_factions"]))
            })
            .map(|faction| s(faction).to_owned())
            .collect();
        assert_eq!(
            factions(&repo)?
                .difference(&seen)
                .cloned()
                .collect::<BTreeSet<_>>(),
            BTreeSet::<String>::new()
        );
        Ok(())
    }

    #[test]
    fn test_no_dangling_event_references() -> Result<()> {
        let repo = Repository::checked_in();
        let events = events(&repo)?;
        let ids: BTreeSet<String> = events
            .iter()
            .map(|event| s(&event["id"]).to_owned())
            .collect();
        let text = serde_json::to_string(&events)?;
        let references: BTreeSet<String> = Regex::new(r"EV_[A-Z0-9_]+")?
            .find_iter(&text)
            .map(|reference| reference.as_str().to_owned())
            .collect();
        assert_eq!(
            references
                .difference(&ids)
                .cloned()
                .collect::<BTreeSet<_>>(),
            BTreeSet::<String>::new()
        );
        Ok(())
    }

    #[test]
    fn test_scope_is_machine_readable() -> Result<()> {
        let repo = Repository::checked_in();
        let data = repo.json(EVENTS_JSON)?;
        let scope = &data["_schema"]["scope"];
        assert_eq!(scope["ruleset"], "RULESET_JAC_BANSEIKI");
        assert_eq!(scope["pack"], "sengoku");
        assert_eq!(scope["maps"], json!(["JAC_Japan_15km"]));
        assert!(repo
            .path("poc/JAC_PoC_JapanMapScale/Maps/JAC_Japan_15km.Civ6Map")
            .exists());
        Ok(())
    }

    #[test]
    fn test_resources_match_the_resource_placement() -> Result<()> {
        let repo = Repository::checked_in();
        let events = events(&repo)?;
        let factions = factions(&repo)?;
        let (points, trades) = resource_indexes(&repo.json(RESOURCES_JSON)?)?;
        let expected: BTreeMap<String, Value> = [
            (
                "EV_IKUNO_SILVER",
                json!([
                    "RESOURCE_SILVER",
                    "生野・但馬 (35.167, 134.790)",
                    "reveal",
                    1542,
                    null
                ]),
            ),
            (
                "EV_HIRADO_PORTUGUESE",
                json!([
                    "RESOURCE_NITER",
                    "平戸・肥前 (33.368, 129.553)",
                    "trade_access",
                    1586,
                    null
                ]),
            ),
            (
                "EV_MORI_IWAMI",
                json!([
                    "RESOURCE_SILVER",
                    "石見銀山・石見 (35.103, 132.437)",
                    "ownership",
                    1526,
                    "MORI"
                ]),
            ),
            (
                "EV_AMAGO_FALL",
                json!([
                    "RESOURCE_IRON",
                    "奥出雲・出雲 (35.199, 133.092)",
                    "ownership",
                    1534,
                    "MORI"
                ]),
            ),
            (
                "EV_SADO_SUBJUGATION",
                json!([
                    "RESOURCE_JAC_GOLD",
                    "西三川・佐渡 (37.819, 138.273)",
                    "reveal",
                    1593,
                    null
                ]),
            ),
            (
                "EV_AIZU_LACQUER",
                json!([
                    "RESOURCE_JAC_LACQUER",
                    "会津 (37.490, 139.930)",
                    "reveal",
                    1590,
                    null
                ]),
            ),
            (
                "EV_AIKAWA",
                json!([
                    "RESOURCE_JAC_GOLD",
                    "相川・佐渡 (38.032, 138.235)",
                    "reveal",
                    1596,
                    null
                ]),
            ),
        ]
        .into_iter()
        .map(|(id, value)| (id.to_owned(), value))
        .collect();
        let actual: BTreeMap<String, Value> = events
            .iter()
            .filter(|event| truthy(&event["resource"]))
            .map(|event| {
                let resource = &event["resource"];
                (
                    s(&event["id"]).to_owned(),
                    json!([
                        resource["resource_id"],
                        resource["location"],
                        resource["action"],
                        resource["start_year"],
                        resource["owner_to"]
                    ]),
                )
            })
            .collect();
        assert_eq!(actual, expected);

        let location = Regex::new(r"\((-?[\d.]+), (-?[\d.]+)\)$")?;
        for event in events {
            let resource = &event["resource"];
            if !truthy(resource) {
                continue;
            }
            let id = s(&event["id"]);
            if resource["action"] == "ownership" {
                let primary = strings(&event["primary_factions"]);
                assert!(
                    factions.contains(s(&resource["owner_to"]))
                        && primary.contains(s(&resource["owner_to"])),
                    "{id}"
                );
            }
            assert!(
                location.captures(s(&resource["location"])).is_some(),
                "{id}"
            );
            let key = event_resource_key(resource)?;
            if resource["action"] == "trade_access" {
                assert!(trades.contains_key(&key), "{id}: {key:?}");
                let trade = &trades[&key];
                assert_eq!(resource["start_year"], trade["available_from_year"], "{id}");
                assert_eq!(event["boost"]["id"], trade["unlock_civic"], "{id}");
                assert_eq!(trade_access_status(trade), "unlocked_by_research", "{id}");
                continue;
            }
            assert!(points.contains_key(&key), "{id}: {key:?}");
            let point = &points[&key];
            let deferred = resource_is_deferred(point);
            if resource["action"] == "reveal" {
                assert!(deferred, "{id}");
                assert_eq!(resource["start_year"].as_i64(), reveal_year(point), "{id}");
                if let Some(tech) = reveal_tech(s(&point["id"])) {
                    let prerequisites: BTreeSet<String> = a(&event["earlier_by"])
                        .iter()
                        .map(|research| s(&research["id"]).to_owned())
                        .collect();
                    assert!(prerequisites.contains(tech), "{id}");
                    assert_eq!(event["boost"]["id"], tech, "{id}");
                }
            } else {
                assert_eq!(resource["action"], "ownership", "{id}");
                assert!(!deferred, "{id}");
                if !point["start_year"].is_null() {
                    assert_eq!(resource["start_year"], point["start_year"], "{id}");
                }
            }
        }
        Ok(())
    }

    #[test]
    fn test_generator_refuses_inconsistent_rename_conditions() -> Result<()> {
        let repo = Repository::checked_in();
        let original = events(&repo)?;
        let start = starting_knowledge(&repo)?;
        let index = original
            .iter()
            .position(|event| event["id"] == "EV_NAGAO_TO_UESUGI")
            .context("Nagao rename event missing")?;

        for invalid in [
            Value::Null,
            json!({}),
            json!({
                "type": "tech",
                "id": "TECH_JAC_SENGOKU_GRAIN_STORES",
                "short": "T07"
            }),
            json!({
                "type": "civic",
                "id": "CIVIC_JAC_SENGOKU_TRUCE_ARBITRATION",
                "short": "C24"
            }),
        ] {
            let mut modified = original.clone();
            modified[index]["rename"]["by"] = invalid;
            let error = build_events(&repo, &modified, &start)
                .unwrap_err()
                .to_string();
            assert!(error.contains("EV_NAGAO_TO_UESUGI"), "{error}");
            assert!(
                error.contains("rename.by must be a civic and the event's boost"),
                "{error}"
            );
        }

        let mut missing = original.clone();
        missing[index]["rename"]
            .as_object_mut()
            .context("rename object missing")?
            .remove("by");
        let error = build_events(&repo, &missing, &start)
            .unwrap_err()
            .to_string();
        assert!(error.contains("EV_NAGAO_TO_UESUGI"), "{error}");
        assert!(
            error.contains("rename.by must be a civic and the event's boost"),
            "{error}"
        );

        let mut same_non_civic = original.clone();
        let technology = json!({
            "type": "tech",
            "id": "TECH_JAC_SENGOKU_GRAIN_STORES",
            "short": "T07"
        });
        same_non_civic[index]["boost"] = technology.clone();
        same_non_civic[index]["rename"]["by"] = technology;
        let error = build_events(&repo, &same_non_civic, &start)
            .unwrap_err()
            .to_string();
        assert!(error.contains("EV_NAGAO_TO_UESUGI"), "{error}");
        assert!(
            error.contains("rename.by must be a civic and the event's boost"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn test_generator_rejects_starting_knowledge_in_each_input_position() -> Result<()> {
        let repo = Repository::checked_in();
        let original = events(&repo)?;
        let start = starting_knowledge(&repo)?;
        let index = original
            .iter()
            .position(|event| event["id"] == "EV_TENBUN_FAMINE")
            .context("Tenbun famine event missing")?;
        let known = start.iter().next().context("starting knowledge missing")?;

        let mut prerequisite = original.clone();
        prerequisite[index]["earlier_by"]
            .as_array_mut()
            .context("prerequisite list missing")?
            .push(json!({"id": known, "type": "tech", "short": "T00"}));
        let error = build_events(&repo, &prerequisite, &start)
            .unwrap_err()
            .to_string();
        assert!(error.contains("EV_TENBUN_FAMINE"), "{error}");
        assert!(error.contains("starting knowledge"), "{error}");

        let mut boost = original.clone();
        boost[index]["boost"] = json!({"id": known, "type": "tech", "short": "T00"});
        let error = build_events(&repo, &boost, &start).unwrap_err().to_string();
        assert!(error.contains("EV_TENBUN_FAMINE"), "{error}");
        assert!(error.contains("starting knowledge"), "{error}");
        Ok(())
    }

    #[test]
    fn test_generator_rejects_missing_or_unreserved_resource_sites() -> Result<()> {
        let repo = Repository::checked_in();
        let original = events(&repo)?;
        let start = starting_knowledge(&repo)?;
        let index = original
            .iter()
            .position(|event| event["id"] == "EV_IKUNO_SILVER")
            .context("Ikuno event missing")?;
        let mut missing = original.clone();
        missing[index]["resource"]["location"] = json!("存在しない地点 (0.000, 0.000)");
        let error = build_events(&repo, &missing, &start)
            .unwrap_err()
            .to_string();
        assert!(error.contains("no resource site"), "{error}");

        let resource_data = repo.json(RESOURCES_JSON)?;
        let sites = resource_sites(&repo)?;
        let point = array(&resource_data["resource_groups"])?
            .iter()
            .filter(|group| group["civ6_id"] == "RESOURCE_SILVER")
            .flat_map(|group| a(&group["points"]))
            .find(|point| point["id"] == "silver-iwami")
            .context("Iwami site missing")?;
        assert!(!sites.reserved.contains(s(&point["id"])));
        let coordinates = a(&point["coordinates"]);
        let mut unreserved = original.clone();
        unreserved[index]["resource"]["location"] =
            json!(format!("石見 ({}, {})", coordinates[1], coordinates[0]));
        let error = build_events(&repo, &unreserved, &start)
            .unwrap_err()
            .to_string();
        assert!(error.contains("EV_IKUNO_SILVER"), "{error}");
        assert!(
            error.contains("silver-iwami is not a reserved site"),
            "{error}"
        );
        Ok(())
    }

    #[test]
    fn test_resource_placement_dependency_branches() {
        assert!(resource_is_deferred(&json!({"available_at_start": false})));
        assert!(resource_is_deferred(&json!({"start_year": 1535})));
        assert!(!resource_is_deferred(&json!({"start_year": 1534})));
        assert!(!resource_is_deferred(&json!({})));
        assert_eq!(
            reveal_year(&json!({"start_year": 1542, "start_year_range": [1540, 1545]})),
            Some(1542)
        );
        assert_eq!(
            reveal_year(&json!({"start_year": null, "start_year_range": [1540, 1545]})),
            Some(1540)
        );
        assert_eq!(reveal_year(&json!({"start_year_range": []})), None);
        assert_eq!(reveal_year(&json!({})), None);
        assert_eq!(
            trade_access_status(&json!({"unlock_civic": "C", "available_from_year": 1586})),
            "unlocked_by_research"
        );
        assert_eq!(
            trade_access_status(&json!({"available_from_year": 1586})),
            "deferred_until_year"
        );
        assert_eq!(
            trade_access_status(&json!({"available_from_year": 1534})),
            "undated_trade_access_not_placed"
        );
        assert_eq!(
            trade_access_status(&json!({})),
            "undated_trade_access_not_placed"
        );
        for id in ["silver-ikuno", "silver-sado-tsurushi"] {
            assert_eq!(reveal_tech(id), Some("TECH_JAC_SENGOKU_CUPELLATION"));
        }
        assert_eq!(
            reveal_tech("copper-ashio"),
            Some("TECH_JAC_SENGOKU_ORE_PROSPECTING")
        );
        for id in [
            "gold-izu-toi",
            "gold-noto-hodatsu",
            "gold-sado-nishimikawa",
            "gold-sado-aikawa",
            "silver-nobesawa",
        ] {
            assert_eq!(reveal_tech(id), Some("TECH_JAC_SENGOKU_MINE_MANAGEMENT"));
        }
        assert_eq!(reveal_tech("lacquer-aizu"), None);
    }

    #[test]
    fn test_cli_check_and_argument_validation() -> Result<()> {
        let repo = Repository::checked_in();
        cli(&repo, &["--check".to_owned()])?;
        let error = cli(&repo, &["--invalid".to_owned()])
            .unwrap_err()
            .to_string();
        assert!(error.contains("unrecognized argument"), "{error}");
        assert!(error.contains("--invalid"), "{error}");
        Ok(())
    }

    #[test]
    fn test_generation_is_deterministic_and_escapes_source_text() -> Result<()> {
        let repo = Repository::checked_in();
        assert_eq!(build(&repo)?, build(&repo)?);
        let mut events = events(&repo)?;
        events[0]["name"]["en"] = json!("O'Brien & <House>");
        let (sql, xml) = build_events(&repo, &events, &starting_knowledge(&repo)?)?;
        let document = roxmltree::Document::parse(&xml)?;
        let tag = format!(
            "LOC_JAC_EVENT_{}_NAME",
            s(&events[0]["id"]).strip_prefix("EV_").unwrap()
        );
        let row = document
            .descendants()
            .find(|node| {
                node.has_tag_name("Row")
                    && node.attribute("Tag") == Some(tag.as_str())
                    && node.attribute("Language") == Some("en_US")
            })
            .context("escaped English event text missing")?;
        let text = row
            .children()
            .find(|node| node.has_tag_name("Text"))
            .and_then(|node| node.text());
        assert_eq!(text, Some("O'Brien & <House>"));
        assert!(xml.contains("O'Brien &amp; &lt;House&gt;"));
        assert!(sql.contains("'SENGOKU'"));
        assert_eq!(sql_literal(&json!("O'Brien"))?, "'O''Brien'");
        assert_eq!(sql_literal(&Value::Null)?, "NULL");
        assert_eq!(sql_literal(&json!(1543))?, "1543");
        assert!(event_type("INVALID").is_err());
        Ok(())
    }
}
