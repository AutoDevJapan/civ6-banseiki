use crate::support_fixture::{action, code, root, scenario, steps};
use regex::Regex;
use roxmltree::{Document, Node};
use rusqlite::{Connection, params_from_iter, types::Value as SqlValue};
use std::collections::{BTreeMap, BTreeSet};

const DOMAIN: &str = "Players:Expansion2_Players";
const ORIGINAL: [&str; 7] = [
    "LEADER_JAC_ODA_NOBUHIDE",
    "LEADER_JAC_TAKEDA_NOBUTORA",
    "LEADER_CLEOPATRA",
    "LEADER_GORGO",
    "LEADER_LUDWIG",
    "LEADER_SEJONG",
    "LEADER_HAMMURABI",
];

fn rows<'a, 'input>(doc: &'a Document<'input>, parent: &str) -> Vec<Node<'a, 'input>> {
    doc.root_element()
        .children()
        .find(|node| node.has_tag_name(parent))
        .unwrap()
        .children()
        .filter(|node| node.has_tag_name("Row"))
        .collect()
}

fn choices(doc: &Document<'_>, ruleset: &str, player_id: i64, pool: &str) -> BTreeSet<String> {
    let mut connection = Connection::open_in_memory().unwrap();
    connection
        .execute("CREATE TABLE Players (Domain TEXT, LeaderType TEXT)", [])
        .unwrap();
    let tx = connection.transaction().unwrap();
    for leader in ORIGINAL {
        tx.execute("INSERT INTO Players VALUES (?1, ?2)", (DOMAIN, leader))
            .unwrap();
    }
    tx.commit().unwrap();
    let queries: BTreeMap<&str, &str> = rows(doc, "Queries")
        .into_iter()
        .map(|row| {
            (
                row.attribute("QueryId").unwrap(),
                row.attribute("SQL").unwrap(),
            )
        })
        .collect();
    let mut parameters: BTreeMap<&str, BTreeMap<usize, (&str, &str)>> = BTreeMap::new();
    for row in rows(doc, "QueryParameters") {
        parameters
            .entry(row.attribute("QueryId").unwrap())
            .or_default()
            .insert(
                row.attribute("Index").unwrap().parse().unwrap(),
                (
                    row.attribute("ConfigurationGroup").unwrap(),
                    row.attribute("ConfigurationId").unwrap(),
                ),
            );
    }
    let mut excluded = BTreeSet::new();
    let mut intersection: Option<BTreeSet<String>> = None;
    for row in rows(doc, "DomainValueFilterQueries") {
        let id = row.attribute("QueryId").unwrap();
        let args: Vec<SqlValue> = parameters[id]
            .values()
            .map(|&(group, key)| match (group, key) {
                ("Game", "RULESET") => SqlValue::Text(ruleset.to_string()),
                ("Player", "PLAYER_ID") => SqlValue::Integer(player_id),
                ("Game", "JAC_BANSEIKI_HUMAN_POOL") => SqlValue::Text(pool.to_string()),
                ("Map", "MAP_SCRIPT") => SqlValue::Text("Continents.lua".to_string()),
                _ => panic!("unexpected query parameter: {group}/{key}"),
            })
            .collect();
        let mut statement = connection.prepare(queries[id]).unwrap();
        let result = statement
            .query_map(params_from_iter(args.iter()), |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .unwrap();
        for row in result {
            let (domain, value, operation) = row.unwrap();
            assert_eq!(domain, DOMAIN);
            assert!(ORIGINAL.contains(&value.as_str()));
            match operation.as_str() {
                "difference" => {
                    excluded.insert(value);
                }
                "intersect" => {
                    intersection.get_or_insert_default().insert(value);
                }
                _ => panic!("unexpected domain filter {operation}"),
            }
        }
    }
    let mut leaders: BTreeSet<String> = ORIGINAL.into_iter().map(str::to_string).collect();
    for leader in excluded {
        leaders.remove(&leader);
    }
    if let Some(intersection) = intersection {
        leaders = leaders.intersection(&intersection).cloned().collect();
    }
    leaders
}

#[test]
fn audited_exclusion_is_exact_and_applies_to_every_banseiki_slot() {
    let xml = std::fs::read_to_string(
        root().join("poc/JAC_PoC_BanseikiRuleset/Config/JAC_Banseiki_Config.xml"),
    )
    .unwrap();
    let doc = Document::parse(&xml).unwrap();
    let registered: BTreeSet<_> = rows(&doc, "DomainValueFilterQueries")
        .into_iter()
        .map(|row| row.attribute("QueryId").unwrap())
        .collect();
    let id = "JAC_Banseiki_UnsupportedLeaderFilter";
    assert!(registered.contains(id));
    let query = rows(&doc, "Queries")
        .into_iter()
        .find(|row| row.attribute("QueryId") == Some(id))
        .unwrap();
    let re = Regex::new(r"'LEADER_[A-Z0-9_]+'").unwrap();
    assert_eq!(
        re.find_iter(query.attribute("SQL").unwrap())
            .map(|m| m.as_str())
            .collect::<Vec<_>>(),
        ["'LEADER_SEJONG'", "'LEADER_HAMMURABI'"]
    );
    let expected: BTreeSet<String> = ORIGINAL
        .into_iter()
        .filter(|&s| !["LEADER_SEJONG", "LEADER_HAMMURABI"].contains(&s))
        .map(str::to_string)
        .collect();
    for player_id in [0, 1, 2] {
        assert_eq!(
            choices(&doc, "RULESET_JAC_BANSEIKI", player_id, "JAC_ALL_POOLS"),
            expected
        );
    }
    let banseiki_only: BTreeSet<String> = ["LEADER_JAC_ODA_NOBUHIDE", "LEADER_JAC_TAKEDA_NOBUTORA"]
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(
        choices(&doc, "RULESET_JAC_BANSEIKI", 0, "JAC_BANSEIKI_ONLY"),
        banseiki_only
    );
    for player_id in [1, 2] {
        assert_eq!(
            choices(&doc, "RULESET_JAC_BANSEIKI", player_id, "JAC_BANSEIKI_ONLY"),
            expected
        );
    }
    let original: BTreeSet<String> = ORIGINAL.into_iter().map(str::to_string).collect();
    assert_eq!(
        choices(&doc, "RULESET_EXPANSION_2", 0, "JAC_ALL_POOLS"),
        original
    );
}

#[test]
fn unsupported_babylon_partial_gameplay_is_excluded_without_changing_expansion2_choices() {
    let xml = std::fs::read_to_string(
        root().join("poc/JAC_PoC_BanseikiRuleset/Config/JAC_Banseiki_Config.xml"),
    )
    .unwrap();
    let doc = Document::parse(&xml).unwrap();
    for player_id in [0, 1, 2, 21] {
        let filtered = choices(&doc, "RULESET_JAC_BANSEIKI", player_id, "JAC_ALL_POOLS");
        assert!(!filtered.contains("LEADER_HAMMURABI"));
        for supported in [
            "LEADER_JAC_ODA_NOBUHIDE",
            "LEADER_JAC_TAKEDA_NOBUTORA",
            "LEADER_CLEOPATRA",
            "LEADER_GORGO",
            "LEADER_LUDWIG",
        ] {
            assert!(filtered.contains(supported));
        }
        let standard = choices(&doc, "RULESET_EXPANSION_2", player_id, "JAC_ALL_POOLS");
        assert!(standard.contains("LEADER_HAMMURABI"));
        assert!(standard.contains("LEADER_SEJONG"));
    }
}

#[test]
fn duplicate_policy_and_runtime_scenario_cover_same_civilization() {
    let xml = std::fs::read_to_string(
        root().join("poc/JAC_PoC_BanseikiRuleset/Config/JAC_Banseiki_Config.xml"),
    )
    .unwrap();
    let doc = Document::parse(&xml).unwrap();
    let updates: BTreeMap<_, _> = rows(&doc, "ConfigurationUpdates")
        .into_iter()
        .filter(|row| row.attribute("SourceValue") == Some("RULESET_JAC_BANSEIKI"))
        .map(|row| {
            (
                row.attribute("TargetId").unwrap(),
                row.attribute("TargetValue").unwrap(),
            )
        })
        .collect();
    assert_eq!(updates["NO_DUPLICATE_LEADERS"], "1");
    assert_eq!(updates["NO_DUPLICATE_CIVILIZATIONS"], "0");
    let data = scenario("banseiki-ruleset.json");
    let s = steps(&data);
    let configure = s.iter().position(|step| action(step, "configure")).unwrap();
    let start = s
        .iter()
        .position(|step| action(step, "start_game"))
        .unwrap();
    let players = s[configure]["players"].as_array().unwrap();
    assert!(players.iter().enumerate().any(|(i, left)| {
        players[i + 1..]
            .iter()
            .any(|right| left["civ"] == right["civ"] && left["leader"] != right["leader"])
    }));
    assert!(
        s[configure + 1..start]
            .iter()
            .filter(|step| action(step, "ui_lua")
                && ["FrontEnd", "HostGame"].contains(&step["state_name"].as_str().unwrap_or("")))
            .any(
                |step| code(step).contains("GameConfiguration.GetValue('NO_DUPLICATE_LEADERS')")
                    && code(step)
                        .contains("GameConfiguration.GetValue('NO_DUPLICATE_CIVILIZATIONS')")
            )
    );
    assert!(s[start + 1..].iter().any(|step| {
        action(step, "ui_lua")
            && step["state_name"] == "InGame"
            && ["LEADER_GORGO", "LEADER_PERICLES", "CIVILIZATION_GREECE"]
                .iter()
                .all(|needle| code(step).contains(needle))
    }));
}
