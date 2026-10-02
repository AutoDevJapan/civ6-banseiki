//! The eight contracts added by 8ff592e in tests/test_banseiki_heroes.py.
//! All official inputs come from required, checked-in fixtures, including Babylon.

use crate::Repository;
use anyhow::{Context, Result};
use roxmltree::{Document, Node};
use rusqlite::{
    functions::FunctionFlags, params_from_iter, types::Value as SqlValue, Connection,
    OptionalExtension,
};
use std::collections::{BTreeMap, BTreeSet};

const MOD: &str = "poc/JAC_PoC_BanseikiRuleset";
const CUSTOM: [&str; 2] = ["HEROCLASS_JAC_WARLORD_A", "HEROCLASS_JAC_WARLORD_B"];
const BABYLON: &str = "DLC/Babylon/Data/Babylon_Heroes_MODE.xml";
type ForeignKeyViolation = (String, Option<i64>, String, i64);

fn strings<const N: usize>(values: [&str; N]) -> BTreeSet<String> {
    values.into_iter().map(str::to_owned).collect()
}

fn identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

// Match the old zlib.crc32(value.encode()) callback, not the engine's hash algorithm.
fn crc32(text: &str) -> i64 {
    let mut crc = u32::MAX;
    for byte in text.bytes() {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    i64::from(!crc)
}

fn query_strings(db: &Connection, sql: &str, args: &[&str]) -> Result<BTreeSet<String>> {
    Ok(db
        .prepare(sql)?
        .query_map(params_from_iter(args), |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}

fn foreign_keys(db: &Connection) -> Result<BTreeSet<ForeignKeyViolation>> {
    Ok(db
        .prepare("PRAGMA foreign_key_check")?
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<rusqlite::Result<_>>()?)
}

fn xml_value(text: Option<&str>) -> SqlValue {
    match text {
        Some(value) if value.eq_ignore_ascii_case("true") => SqlValue::Integer(1),
        Some(value) if value.eq_ignore_ascii_case("false") => SqlValue::Integer(0),
        Some(value) => SqlValue::Text(value.to_owned()),
        None => SqlValue::Null,
    }
}

// Only direct Row elements and supported Base columns are seeded by the old test.
// In particular, do not apply DLC Update/Replace/Delete operations or GS schema.
fn seed_xml(
    db: &Connection,
    text: &str,
    columns: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<()> {
    let doc = Document::parse(text)?;
    for section in doc.root_element().children().filter(Node::is_element) {
        let Some(canonical) = columns.get(section.tag_name().name()) else {
            continue;
        };
        for row in section.children().filter(|node| node.has_tag_name("Row")) {
            let mut values = BTreeMap::new();
            for attr in row.attributes() {
                if let Some(column) = canonical.get(&attr.name().to_ascii_lowercase()) {
                    values.insert(column, xml_value(Some(attr.value())));
                }
            }
            for child in row.children().filter(Node::is_element) {
                if let Some(column) = canonical.get(&child.tag_name().name().to_ascii_lowercase()) {
                    values.insert(column, xml_value(child.text()));
                }
            }
            if values.is_empty() {
                continue;
            }
            let names = values
                .keys()
                .map(|name| identifier(name))
                .collect::<Vec<_>>()
                .join(",");
            let marks = vec!["?"; values.len()].join(",");
            db.execute(
                &format!(
                    "INSERT OR IGNORE INTO {} ({names}) VALUES ({marks})",
                    identifier(section.tag_name().name())
                ),
                params_from_iter(values.values()),
            )?;
        }
    }
    Ok(())
}

struct HeroDatabase {
    db: Connection,
    original: Vec<(String, String, String)>,
    baseline: BTreeSet<ForeignKeyViolation>,
}

impl HeroDatabase {
    fn new() -> Result<Self> {
        let repo = Repository::checked_in();
        let db = Connection::open_in_memory()?;
        // Python's sqlite3 connection starts with FK enforcement disabled; rusqlite
        // bundles SQLite with it enabled. Preserve the old seed/baseline boundary.
        db.execute_batch("PRAGMA foreign_keys=OFF;")?;
        assert_eq!(crc32(""), 0);
        assert_eq!(crc32("123456789"), 0xcbf4_3926);
        db.create_scalar_function("Make_Hash", 1, FunctionFlags::SQLITE_UTF8, |ctx| {
            Ok(crc32(&ctx.get::<String>(0)?))
        })?;
        for name in ["01_GameplaySchema.sql", "02_AddTriggers.sql"] {
            db.execute_batch(&repo.game_read(format!("Base/Assets/Gameplay/Data/Schema/{name}"))?)?;
        }
        let tables = query_strings(
            &db,
            "SELECT name FROM sqlite_master WHERE type='table'",
            &[],
        )?;
        let mut columns = BTreeMap::new();
        for table in tables {
            let names = db
                .prepare(&format!("PRAGMA table_info({})", identifier(&table)))?
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            columns.insert(
                table,
                names
                    .into_iter()
                    .map(|name| (name.to_ascii_lowercase(), name))
                    .collect(),
            );
        }

        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../fixtures/game-sources.json"))?;
        let mut files: Vec<_> = manifest["files"]
            .as_array()
            .context("official fixture file list")?
            .iter()
            .map(|file| file["path"].as_str().context("official fixture path"))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .filter(|path| path.starts_with("Base/Assets/Gameplay/Data/") && path.ends_with(".xml"))
            .collect();
        files.sort_unstable();
        assert_eq!(files.len(), 82, "all shipped Base XML files are required");
        assert_eq!(files.iter().copied().collect::<BTreeSet<_>>().len(), 82);
        files.push(BABYLON);
        db.execute_batch("BEGIN;")?;
        for path in files {
            seed_xml(&db, &repo.game_read(path)?, &columns)
                .with_context(|| format!("seed required fixture {path}"))?;
        }
        let original = db
            .prepare("SELECT HeroClassType,UnitType,CreationProjectType FROM HeroClasses")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        assert_eq!(
            original.len(),
            12,
            "official seed must include all twelve shipped Heroes"
        );
        db.execute(
            "INSERT INTO Types(Type,Kind) VALUES ('ERA_JAC_HERO_TEST','KIND_ERA')",
            [],
        )?;
        let era_columns = db
            .prepare("PRAGMA table_info(Eras)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let fields = era_columns
            .iter()
            .map(|name| identifier(name))
            .collect::<Vec<_>>()
            .join(",");
        let expressions = era_columns
            .iter()
            .map(|name| match name.as_str() {
                "EraType" => "'ERA_JAC_HERO_TEST'".to_owned(),
                "ChronologyIndex" => "100".to_owned(),
                _ => identifier(name),
            })
            .collect::<Vec<_>>()
            .join(",");
        db.execute(
            &format!("INSERT INTO Eras({fields}) SELECT {expressions} FROM Eras WHERE EraType='ERA_RENAISSANCE'"),
            [],
        )?;
        db.execute_batch("COMMIT;")?;
        let baseline = foreign_keys(&db)?;
        db.execute_batch("PRAGMA foreign_keys=ON;")?;
        db.execute_batch(&repo.read(format!("{MOD}/Data/JAC_Banseiki_Heroes.sql"))?)?;
        Ok(Self {
            db,
            original,
            baseline,
        })
    }
}

mod native_hero_database_tests {
    use super::*;

    #[test]
    fn test_native_classes_reference_complete_projects_units_and_works() -> Result<()> {
        let HeroDatabase { db, baseline, .. } = HeroDatabase::new()?;
        let rows = query_strings(
            &db,
            "SELECT h.HeroClassType FROM HeroClasses h
            JOIN Units u ON u.UnitType=h.UnitType JOIN Units_MODE m ON m.UnitType=u.UnitType
            JOIN Projects p ON p.ProjectType=h.CreationProjectType
            JOIN GreatWorks a ON a.GreatWorkType=h.ArtifactGreatWorkType
            JOIN GreatWorks e ON e.GreatWorkType=h.EpicGreatWorkType
            WHERE u.CanTrain=1 AND u.MustPurchase=1 AND p.MaxPlayerInstances=1
            AND m.ActionCharges=3 AND a.GreatWorkObjectType='GREATWORKOBJECT_HERO'
            AND e.GreatWorkObjectType='GREATWORKOBJECT_HERO'",
            &[],
        )?;
        assert_eq!(rows, strings(CUSTOM));
        let after = foreign_keys(&db)?;
        assert_eq!(
            after.difference(&baseline).collect::<BTreeSet<_>>(),
            BTreeSet::new()
        );
        Ok(())
    }

    #[test]
    fn test_shipped_heroes_have_no_discovery_project_or_purchase_path() -> Result<()> {
        let HeroDatabase { db, original, .. } = HeroDatabase::new()?;
        for (hero, unit, project) in original {
            assert_eq!(
                db.query_row(
                    "SELECT 1 FROM HeroClasses WHERE HeroClassType=?",
                    [&hero],
                    |row| row.get::<_, i64>(0)
                )
                .optional()?,
                None,
                "{hero}"
            );
            assert_eq!(
                db.query_row(
                    "SELECT CanTrain FROM Units WHERE UnitType=?",
                    [&unit],
                    |row| row.get::<_, i64>(0)
                )?,
                0,
                "{hero}"
            );
            assert_eq!(
                db.query_row(
                    "SELECT MaxPlayerInstances FROM Projects WHERE ProjectType=?",
                    [&project],
                    |row| row.get::<_, i64>(0)
                )?,
                0,
                "{hero}"
            );
            assert_eq!(
                db.query_row(
                    "SELECT COUNT(*) FROM ProjectCompletionModifiers WHERE ProjectType=?",
                    [&project],
                    |row| row.get::<_, i64>(0)
                )?,
                0,
                "{hero}"
            );
        }
        Ok(())
    }

    #[test]
    fn test_native_ids_are_dense_and_every_custom_era_has_lifespan_and_strength_data() -> Result<()>
    {
        let HeroDatabase { db, .. } = HeroDatabase::new()?;
        let rowids = db
            .prepare("SELECT rowid FROM HeroClasses ORDER BY rowid")?
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        assert_eq!(rowids, vec![1, 2]);
        let eras = query_strings(&db, "SELECT EraType FROM Eras", &[])?;
        assert!(eras.contains("ERA_JAC_HERO_TEST"));
        for hero in CUSTOM {
            assert_eq!(
                query_strings(
                    &db,
                    "SELECT EraType FROM HeroClassProgressions WHERE HeroClassType=?",
                    &[hero]
                )?,
                eras,
                "{hero}"
            );
        }
        let units = query_strings(&db, "SELECT UnitType FROM HeroClasses", &[])?;
        let lifespan = query_strings(
            &db,
            "SELECT Type FROM TypeProperties WHERE Name='LIFESPAN' AND Value='30'",
            &[],
        )?;
        assert_eq!(
            lifespan
                .intersection(&units)
                .cloned()
                .collect::<BTreeSet<_>>(),
            units
        );
        Ok(())
    }

    #[test]
    fn test_framework_discounts_and_recruitment_bonus_target_new_definitions() -> Result<()> {
        let HeroDatabase { db, .. } = HeroDatabase::new()?;
        for suffix in ["A", "B"] {
            let unit = format!("UNIT_HERO_JAC_WARLORD_{suffix}");
            let buildings = query_strings(
                &db,
                "SELECT b.BuildingType FROM BuildingModifiers b
                JOIN ModifierArguments a ON a.ModifierId=b.ModifierId
                WHERE a.Name='UnitType' AND a.Value=?",
                &[&unit],
            )?;
            assert_eq!(
                buildings,
                strings([
                    "BUILDING_TEMPLE",
                    "BUILDING_ORACLE",
                    "BUILDING_STAVE_CHURCH"
                ])
            );
            let project = format!("PROJECT_CREATE_HERO_JAC_WARLORD_{suffix}");
            let traits = query_strings(
                &db,
                "SELECT t.TraitType FROM TraitModifiers t
                JOIN ModifierArguments a ON a.ModifierId=t.ModifierId
                WHERE a.Name='ProjectType' AND a.Value=?",
                &[&project],
            )?;
            assert_eq!(traits, strings(["TRAIT_LEADER_GILGAMESH_HEROES"]));
        }
        Ok(())
    }
}

fn select<'a, 'input>(node: Node<'a, 'input>, path: &str) -> Vec<Node<'a, 'input>> {
    let mut nodes = vec![node];
    for name in path.split('/') {
        nodes = nodes
            .into_iter()
            .flat_map(|node| {
                node.children()
                    .filter(move |child| child.has_tag_name(name))
            })
            .collect();
    }
    nodes
}

fn one<'a, 'input>(node: Node<'a, 'input>, path: &str) -> Node<'a, 'input> {
    *select(node, path)
        .first()
        .unwrap_or_else(|| panic!("missing XML path {path}"))
}

mod native_hero_package_tests {
    use super::*;

    #[test]
    fn test_native_dlc_uses_registered_id_and_is_enabled_before_dependents() -> Result<()> {
        let repo = Repository::checked_in();
        let canonical = "8424840C-92EF-4426-A9B4-B4E0CB818049";
        for folder in ["JAC_PoC_BanseikiRuleset", "JAC_PoC_Absorption"] {
            let text = repo.read(format!("poc/{folder}/{folder}.modinfo"))?;
            let doc = Document::parse(&text)?;
            let dependencies: BTreeSet<_> = select(doc.root_element(), "Dependencies/Mod")
                .into_iter()
                .map(|node| node.attribute("id"))
                .collect();
            assert!(dependencies.contains(&Some(canonical)), "{folder}");
        }
        for name in ["absorption-japan.json", "absorption-hero-recall.json"] {
            let data = repo.json(format!("tools/civ6-autotest/scenarios/{name}"))?;
            let ids = data["steps"][0]["ids"].as_array().context("enabled IDs")?;
            assert_eq!(ids.first(), Some(&serde_json::Value::from(canonical)));
            let position = |id: &str| {
                ids.iter()
                    .position(|value| value.as_str() == Some(id))
                    .unwrap()
            };
            assert!(
                position("ba3a1c58-9287-4654-a22f-85f9e6841e9b")
                    < position("bd7a82c5-6b38-4b69-a7f5-8dc9af534249")
            );
            assert!(
                position("bd7a82c5-6b38-4b69-a7f5-8dc9af534249")
                    < position("61320778-2e75-4c11-9009-94101b519aba")
            );
        }
        Ok(())
    }

    #[test]
    fn test_native_warlord_art_uses_generic_infantry_and_cavalry() -> Result<()> {
        let text = Repository::checked_in().read(format!("{MOD}/ArtDefs/Units.artdef"))?;
        let doc = Document::parse(&text)?;
        let units = select(doc.root_element(), "m_RootCollections/Element/Element");
        let names: BTreeSet<_> = units
            .iter()
            .map(|unit| one(*unit, "m_Name").attribute("text").unwrap().to_owned())
            .collect();
        assert_eq!(
            names,
            strings(["UNIT_HERO_JAC_WARLORD_A", "UNIT_HERO_JAC_WARLORD_B"])
        );
        let expected = BTreeMap::from([
            ("UNIT_HERO_JAC_WARLORD_A", "Warrior"),
            ("UNIT_HERO_JAC_WARLORD_B", "Horsemen"),
        ]);
        for unit in units {
            let formation = select(unit, "m_Fields/m_Values/Element")
                .into_iter()
                .find(|value| one(*value, "m_ParamName").attribute("text") == Some("Formation"))
                .context("unit Formation field")?;
            let name = one(unit, "m_Name").attribute("text").unwrap();
            assert_eq!(
                one(formation, "m_ElementName").attribute("text"),
                Some(expected[name])
            );
        }
        Ok(())
    }

    #[test]
    fn test_icons_preserve_every_size_of_generic_units() -> Result<()> {
        let db = Connection::open_in_memory()?;
        db.execute_batch("CREATE TABLE IconDefinitions(Name TEXT PRIMARY KEY,\"Index\" INTEGER,Atlas TEXT);
            CREATE TABLE IconTextureAtlases(Name TEXT,IconSize INTEGER,PRIMARY KEY(Name,IconSize));")?;
        for size in [32, 38, 50, 80, 256] {
            db.execute(
                "INSERT INTO IconTextureAtlases VALUES ('BASE_UNITS',?)",
                [size],
            )?;
        }
        for unit in ["WARRIOR", "HORSEMAN"] {
            db.execute(
                "INSERT INTO IconDefinitions VALUES (?,7,'BASE_UNITS')",
                [format!("ICON_UNIT_{unit}")],
            )?;
        }
        db.execute_batch(
            &Repository::checked_in().read(format!("{MOD}/Data/JAC_Banseiki_Hero_Icons.sql"))?,
        )?;
        let mut query = db.prepare("SELECT a.IconSize,d.\"Index\",d.Atlas FROM IconDefinitions d JOIN IconTextureAtlases a ON a.Name=d.Atlas WHERE d.Name=? ORDER BY a.IconSize")?;
        for (suffix, source) in [("A", "WARRIOR"), ("B", "HORSEMAN")] {
            let original = query
                .query_map([format!("ICON_UNIT_{source}")], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            assert_eq!(original.len(), 5);
            for prefix in ["ICON_UNIT_HERO_JAC_WARLORD_", "ICON_HEROCLASS_JAC_WARLORD_"] {
                let actual = query
                    .query_map([format!("{prefix}{suffix}")], |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                assert_eq!(actual, original);
            }
        }
        Ok(())
    }

    #[test]
    fn test_all_custom_names_have_three_languages_and_actions_are_scoped() -> Result<()> {
        let repo = Repository::checked_in();
        let text = repo.read(format!("{MOD}/Text/JAC_Banseiki_Heroes_Text.xml"))?;
        let doc = Document::parse(&text)?;
        let mut by_tag = BTreeMap::<_, BTreeSet<_>>::new();
        for row in one(doc.root_element(), "LocalizedText")
            .children()
            .filter(Node::is_element)
        {
            by_tag
                .entry(row.attribute("Tag"))
                .or_default()
                .insert(row.attribute("Language"));
            assert!(!one(row, "Text").text().unwrap_or("").is_empty());
        }
        assert_eq!(by_tag.len(), 10);
        let languages = BTreeSet::from([Some("ja_JP"), Some("en_US"), Some("ru_RU")]);
        assert!(by_tag.values().all(|actual| *actual == languages));
        let text = repo.read(format!("{MOD}/JAC_PoC_BanseikiRuleset.modinfo"))?;
        let doc = Document::parse(&text)?;
        let actions: Vec<_> = one(doc.root_element(), "InGameActions")
            .children()
            .filter(|node| {
                node.is_element()
                    && node
                        .attribute("id")
                        .unwrap_or("")
                        .starts_with("JAC_Banseiki_Heroes")
            })
            .collect();
        assert_eq!(actions.len(), 4);
        assert!(actions
            .iter()
            .all(|action| action.attribute("criteria") == Some("JAC_Banseiki_Heroes")));
        let criteria = select(doc.root_element(), "ActionCriteria/Criteria")
            .into_iter()
            .find(|node| node.attribute("id") == Some("JAC_Banseiki_Heroes"))
            .context("hero ActionCriteria")?;
        assert_eq!(
            one(criteria, "RuleSetInUse").text(),
            Some("RULESET_JAC_BANSEIKI")
        );
        assert_eq!(
            one(criteria, "ConfigurationValueMatches/ConfigurationId").text(),
            Some("GAMEMODE_HEROES")
        );
        assert_eq!(
            one(criteria, "ConfigurationValueMatches/Value").text(),
            Some("1")
        );
        Ok(())
    }
}
