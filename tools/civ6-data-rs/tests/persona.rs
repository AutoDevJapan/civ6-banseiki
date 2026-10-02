//! One-for-one port of the 14 contracts in tests/test_leader_persona.py at d801f97.
//! The official schema and data are loaded by the shared cascade loader.

use civ6_data_rs::{cascade, Repository};
use roxmltree::{Document, Node};
use rusqlite::{backup::Backup, params, types::Value as SqlValue, Connection, OpenFlags, ToSql};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::OnceLock,
};
use tempfile::TempDir;
use walkdir::WalkDir;

const MOD: &str = "poc/JAC_PoC_LeaderPersona";
const CUSTOM: &str = "poc/JAC_PoC_CustomCivs";
type ForeignKeyViolation = (String, Option<i64>, String, i64);

struct PersonaFixture {
    _directory: TempDir,
    before: PathBuf,
    after: PathBuf,
    baseline_foreign_keys: BTreeSet<ForeignKeyViolation>,
}

fn fixture() -> &'static PersonaFixture {
    static FIXTURE: OnceLock<PersonaFixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let repo = Repository::checked_in();
        let original = cascade::official_db_for(&repo).expect("load official game-source fixtures");
        // The shared loader handles Row/Replace but not the official Agendas.xml
        // InsertOrIgnore that declares KIND_TRAIT. Keep that official seed here.
        let agendas_source = repo
            .game_read("Base/Assets/Gameplay/Data/Agendas.xml")
            .unwrap();
        let agendas = Document::parse(&agendas_source).unwrap();
        let kind = child(child(agendas.root_element(), "Kinds"), "InsertOrIgnore")
            .attribute("Kind")
            .unwrap();
        assert_eq!(kind, "KIND_TRAIT");
        original
            .execute(
                "INSERT OR IGNORE INTO Kinds (Kind) VALUES (?1)",
                params![kind],
            )
            .unwrap();
        for file in ["JAC_CustomCivs_Gameplay.sql", "JAC_Japan1534_Gameplay.sql"] {
            original
                .execute_batch(&repo.read(format!("{CUSTOM}/Data/{file}")).unwrap())
                .unwrap_or_else(|error| panic!("apply {file}: {error}"));
        }
        let db = cascade::run_sengoku(&repo, &original, true).expect("apply Sengoku tree deletes");
        let baseline_foreign_keys = foreign_keys(&db);
        let directory = tempfile::tempdir().expect("make persona test snapshots");
        let before = directory.path().join("before.sqlite");
        let after = directory.path().join("after.sqlite");
        save_snapshot(&db, &before);
        apply_persona(&db, &repo);
        save_snapshot(&db, &after);
        PersonaFixture {
            _directory: directory,
            before,
            after,
            baseline_foreign_keys,
        }
    })
}

fn save_snapshot(db: &Connection, path: &Path) {
    let mut target = Connection::open(path).unwrap();
    Backup::new(db, &mut target)
        .unwrap()
        .run_to_completion(1000, std::time::Duration::ZERO, None)
        .unwrap();
}

fn open_snapshot(path: &Path) -> Connection {
    let source = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut db = Connection::open_in_memory().unwrap();
    Backup::new(&source, &mut db)
        .unwrap()
        .run_to_completion(1000, std::time::Duration::ZERO, None)
        .unwrap();
    cascade::register_audit_hash(&db).unwrap();
    db.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    db
}

fn persona_db() -> Connection {
    open_snapshot(&fixture().after)
}

fn apply_persona(db: &Connection, repo: &Repository) {
    for file in ["JAC_LeaderPersona.sql", "JAC_LeaderPersona_Trees.sql"] {
        db.execute_batch(&repo.read(format!("{MOD}/Data/{file}")).unwrap())
            .unwrap_or_else(|error| panic!("apply {file}: {error}"));
    }
}

fn foreign_keys(db: &Connection) -> BTreeSet<ForeignKeyViolation> {
    db.prepare("PRAGMA foreign_key_check")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn query(db: &Connection, sql: &str, args: &[&dyn ToSql]) -> Value {
    let mut statement = db
        .prepare(sql)
        .unwrap_or_else(|error| panic!("{sql}: {error}"));
    let column_count = statement.column_count();
    let rows = statement
        .query_map(args, |row| {
            (0..column_count)
                .map(|column| {
                    Ok(match row.get::<_, SqlValue>(column)? {
                        SqlValue::Null => Value::Null,
                        SqlValue::Integer(value) => json!(value),
                        SqlValue::Real(value) => json!(value),
                        SqlValue::Text(value) => json!(value),
                        SqlValue::Blob(_) => panic!("unexpected blob in {sql}"),
                    })
                })
                .collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap_or_else(|error| panic!("{sql}: {error}"))
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap_or_else(|error| panic!("{sql}: {error}"));
    json!(rows)
}

fn q(db: &Connection, sql: &str) -> Value {
    query(db, sql, &[])
}

fn rows(value: &Value) -> &Vec<Value> {
    value.as_array().unwrap()
}

fn row_set(value: &Value) -> BTreeSet<String> {
    rows(value).iter().map(Value::to_string).collect()
}

fn string_set(value: &Value) -> BTreeSet<String> {
    rows(value)
        .iter()
        .map(|row| row[0].as_str().unwrap().to_owned())
        .collect()
}

fn value_map(value: &Value) -> BTreeMap<String, Value> {
    rows(value)
        .iter()
        .map(|row| (row[0].as_str().unwrap().to_owned(), row[1].clone()))
        .collect()
}

fn count(db: &Connection, sql: &str) -> i64 {
    q(db, sql)[0][0].as_i64().unwrap()
}

#[test]
fn test_all_22_factions_keep_their_leader_identity() {
    let db = persona_db();
    let people = q(&db, "SELECT LeaderType,CivilizationType,PersonType,GovernorType,Name,PortraitIcon FROM JAC_LeaderPersonas");
    let expected = q(&db, "SELECT l.LeaderType,c.CivilizationType,l.Name FROM Leaders l JOIN CivilizationLeaders c ON c.LeaderType=l.LeaderType WHERE c.CivilizationType LIKE 'CIVILIZATION_JAC_%'");
    assert_eq!(rows(&people).len(), 22);
    let identities = json!(rows(&people)
        .iter()
        .map(|person| json!([person[0], person[1], person[4]]))
        .collect::<Vec<_>>());
    assert_eq!(row_set(&identities), row_set(&expected));
    for person in rows(&people) {
        let leader = person[0].as_str().unwrap();
        let person_type = person[2].as_str().unwrap();
        let governor = person[3].as_str().unwrap();
        let name = person[4].as_str().unwrap();
        assert_eq!(person[5], json!(format!("ICON_{leader}")), "{leader}");
        assert_eq!(query(&db, "SELECT Name,ActionCharges FROM GreatPersonIndividuals WHERE GreatPersonIndividualType=?", params![person_type]), json!([[name, 0]]), "{leader}");
        assert_eq!(query(&db, "SELECT Name,TransitionStrength,AssignCityState FROM Governors WHERE GovernorType=?", params![governor]), json!([[name, 100, 0]]), "{leader}");
    }
    assert!(foreign_keys(&db)
        .difference(&fixture().baseline_foreign_keys)
        .next()
        .is_none());
    let invalid = db.execute("INSERT INTO JAC_LeaderPersonas SELECT 'LEADER_MISSING','CIVILIZATION_MISSING','PERSON_MISSING','GOVERNOR_MISSING','LOC_MISSING','ICON_MISSING','BASE_MISSING','CALM','MUNDANE',NULL,0,'HEAD',1", []);
    match invalid {
        Err(rusqlite::Error::SqliteFailure(code, _)) => {
            assert_eq!(code.code, rusqlite::ErrorCode::ConstraintViolation)
        }
        other => panic!("invalid persona must fail a constraint: {other:?}"),
    }
}

#[test]
fn test_no_recruitment_or_retirement_effects() {
    let db = persona_db();
    assert_eq!(q(&db, "SELECT UnitType,AvailableInTimeline,GenerateDuplicateIndividuals,PseudoYieldType FROM GreatPersonClasses WHERE GreatPersonClassType='GREAT_PERSON_CLASS_JAC_PERSONA'"), json!([["UNIT_JAC_PERSONA", 0, 1, "PSEUDOYIELD_GPP_GENERAL"]]));
    assert_eq!(
        q(
            &db,
            "SELECT GreatPersonClassType FROM GreatPersonClasses WHERE UnitType='UNIT_JAC_PERSONA'"
        ),
        json!([["GREAT_PERSON_CLASS_JAC_PERSONA"]])
    );
    assert_eq!(q(&db, "SELECT CanTrain,CanCapture,CanRetreatWhenCaptured FROM Units WHERE UnitType='UNIT_JAC_PERSONA'"), json!([[0, 0, 1]]));
    assert_eq!(q(&db, "SELECT GreatPersonClassType FROM GreatPersonClasses WHERE UnitType='UNIT_GREAT_GENERAL'"), json!([["GREAT_PERSON_CLASS_GENERAL"]]));
    for table in [
        "GreatPersonIndividualActionModifiers",
        "GreatPersonIndividualBirthModifiers",
    ] {
        assert_eq!(count(&db, &format!("SELECT COUNT(*) FROM {table} WHERE GreatPersonIndividualType IN (SELECT PersonType FROM JAC_LeaderPersonas)")), 0, "{table}");
    }
}

#[test]
fn test_native_bases_and_cost_only_payment_promotions_are_registered() {
    let db = persona_db();
    let people = q(
        &db,
        "SELECT GovernorType,BasePromotionType,EvolutionCost FROM JAC_LeaderPersonas",
    );
    assert_eq!(rows(&people).len(), 22);
    for person in rows(&people) {
        let governor = person[0].as_str().unwrap();
        let base = person[1].as_str().unwrap();
        let cost = person[2].as_i64().unwrap();
        let expected = std::iter::once(json!([base]))
            .chain((1..cost).map(|step| json!([format!("{base}_TITLE_{step}")])))
            .collect::<Vec<_>>();
        assert_eq!(
            query(
                &db,
                "SELECT GovernorPromotion FROM GovernorPromotionSets WHERE GovernorType=?",
                params![governor]
            ),
            json!(expected),
            "{governor}"
        );
        assert_eq!(
            query(
                &db,
                "SELECT BaseAbility,Level FROM GovernorPromotions WHERE GovernorPromotionType=?",
                params![base]
            ),
            json!([[1, 0]]),
            "{base}"
        );
        for step in 1..cost {
            let payment = format!("{base}_TITLE_{step}");
            let prerequisite = if step == 1 {
                base.to_owned()
            } else {
                format!("{base}_TITLE_{}", step - 1)
            };
            assert_eq!(query(&db, "SELECT BaseAbility,Level,\"Column\" FROM GovernorPromotions WHERE GovernorPromotionType=?", params![payment]), json!([[0, step, 1]]), "{payment}");
            assert_eq!(query(&db, "SELECT PrereqGovernorPromotion FROM GovernorPromotionPrereqs WHERE GovernorPromotionType=?", params![payment]), json!([[prerequisite]]), "{payment}");
            assert_eq!(query(&db, "SELECT ModifierId FROM GovernorPromotionModifiers WHERE GovernorPromotionType=?", params![payment]), json!([]), "{payment}");
        }
    }
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM JAC_PersonaTitlePayments"),
        rows(&people)
            .iter()
            .map(|person| person[2].as_i64().unwrap() - 1)
            .sum::<i64>()
    );
    let leaders = q(
        &db,
        "SELECT LeaderType,BasePromotionType FROM JAC_LeaderPersonas",
    );
    for leader in rows(&leaders) {
        let leader_type = leader[0].as_str().unwrap();
        let base = leader[1].as_str().unwrap();
        let marker = format!("JAC_PERSONA_SETTLED_{leader_type}");
        assert_eq!(
            query(
                &db,
                "SELECT ModifierType FROM Modifiers WHERE ModifierId=?",
                params![marker]
            ),
            json!([["MODIFIER_SINGLE_CITY_ADJUST_PROPERTY"]])
        );
        assert_eq!(
            value_map(&query(
                &db,
                "SELECT Name,Value FROM ModifierArguments WHERE ModifierId=?",
                params![marker]
            )),
            BTreeMap::from([
                ("Key".to_owned(), json!(marker)),
                ("Amount".to_owned(), json!("1"))
            ])
        );
        assert_eq!(
            query(
                &db,
                "SELECT GovernorPromotionType FROM GovernorPromotionModifiers WHERE ModifierId=?",
                params![marker]
            ),
            json!([[base]])
        );
    }
    let nodes = q(
        &db,
        "SELECT LeaderType,ModifierId,EffectKind,Amount FROM JAC_PersonaTreeNodes",
    );
    assert_eq!(rows(&nodes).len(), 762);
    for node in rows(&nodes) {
        let leader = node[0].as_str().unwrap();
        let modifier = node[1].as_str().unwrap();
        let amount = node[3].as_i64().unwrap();
        assert!(amount > 0, "{modifier}");
        assert_eq!(
            query(
                &db,
                "SELECT GovernorPromotionType FROM GovernorPromotionModifiers WHERE ModifierId=?",
                params![modifier]
            ),
            query(
                &db,
                "SELECT BasePromotionType FROM JAC_LeaderPersonas WHERE LeaderType=?",
                params![leader]
            ),
            "{modifier}"
        );
        let set = format!("{modifier}_SET");
        let requirement = format!("{modifier}_REQ");
        assert_eq!(
            query(
                &db,
                "SELECT SubjectRequirementSetId FROM Modifiers WHERE ModifierId=?",
                params![modifier]
            ),
            json!([[set]]),
            "{modifier}"
        );
        assert_eq!(
            value_map(&query(
                &db,
                "SELECT Name,Value FROM RequirementArguments WHERE RequirementId=?",
                params![requirement]
            )),
            BTreeMap::from([
                ("PropertyName".to_owned(), json!(modifier)),
                ("PropertyMinimum".to_owned(), json!("1"))
            ]),
            "{modifier}"
        );
        assert_eq!(
            query(
                &db,
                "SELECT RequirementType FROM Requirements WHERE RequirementId=?",
                params![requirement]
            ),
            json!([["REQUIREMENT_PLOT_PROPERTY_MATCHES"]]),
            "{modifier}"
        );
        assert_eq!(
            query(
                &db,
                "SELECT RequirementId FROM RequirementSetRequirements WHERE RequirementSetId=?",
                params![set]
            ),
            json!([[requirement]]),
            "{modifier}"
        );
        let argument = query(
            &db,
            "SELECT Value FROM ModifierArguments WHERE ModifierId=? AND Name='Amount'",
            params![modifier],
        );
        assert_eq!(
            argument[0][0].as_str().unwrap().parse::<i64>().unwrap(),
            amount,
            "{modifier}"
        );
    }
}

#[test]
fn test_all_persons_have_reusable_chains_choices_and_one_unique_signature() {
    let db = persona_db();
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM JAC_PersonaRouteTemplates"),
        16
    );
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM JAC_PersonaNodeTemplates"),
        48
    );
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM JAC_PersonaSignatureTemplates"),
        66
    );
    let people = q(
        &db,
        "SELECT LeaderType,PreferredCategory,PreferredRoute FROM JAC_PersonaTreePeople",
    );
    assert_eq!(rows(&people).len(), 22);
    for person in rows(&people) {
        let leader = person[0].as_str().unwrap();
        let preferred = person[1].as_str().unwrap();
        let strong = person[2].as_str().unwrap();
        let access_rows = query(
            &db,
            "SELECT CategoryType FROM JAC_PersonaCategoryAccess WHERE LeaderType=?",
            params![leader],
        );
        let access = string_set(&access_rows);
        assert!([2, 3].contains(&rows(&access_rows).len()), "{leader}");
        assert_eq!(access.len(), rows(&access_rows).len(), "{leader}");
        assert!(access.contains(preferred), "{leader}");
        assert!(access.len() < 4, "{leader}");
        let signatures = query(&db, "SELECT CategoryType,RouteType,Depth,Cost FROM JAC_PersonaTreeNodes WHERE LeaderType=? AND Signature=1 ORDER BY Depth", params![leader]);
        assert_eq!(
            signatures,
            json!([
                [preferred, format!("SIGNATURE_{leader}"), 1, 8],
                [preferred, format!("SIGNATURE_{leader}"), 2, 12],
                [preferred, format!("SIGNATURE_{leader}"), 3, 18]
            ]),
            "{leader}"
        );
        for category in &access {
            let routes = query(&db, "SELECT DISTINCT RouteType,ExclusiveGroup FROM JAC_PersonaTreeNodes WHERE LeaderType=? AND CategoryType=? AND Signature=0", params![leader, category]);
            assert_eq!(rows(&routes).len(), 4, "{leader}/{category}");
            let mut groups = BTreeMap::<&str, usize>::new();
            for route in rows(&routes) {
                *groups.entry(route[1].as_str().unwrap()).or_default() += 1;
            }
            assert_eq!(
                groups.values().copied().collect::<Vec<_>>(),
                vec![2, 2],
                "{leader}/{category}"
            );
            assert_eq!(groups.len() * 3, 6, "{leader}/{category}");
            for route in rows(&routes) {
                let route_type = route[0].as_str().unwrap();
                let chain = query(&db, "SELECT NodeType,PrereqNode,Depth,Cost FROM JAC_PersonaTreeNodes WHERE LeaderType=? AND RouteType=? ORDER BY Depth", params![leader, route_type]);
                let chain = rows(&chain);
                assert_eq!(chain.len(), 3, "{leader}/{route_type}");
                assert_eq!(
                    json!([chain[0][1], chain[1][1], chain[2][1]]),
                    json!([null, chain[0][0], chain[1][0]]),
                    "{leader}/{route_type}"
                );
                assert_eq!(
                    json!([chain[0][3], chain[1][3], chain[2][3]]),
                    if route_type == strong {
                        json!([8, 12, 18])
                    } else {
                        json!([10, 15, 22])
                    },
                    "{leader}/{route_type}"
                );
            }
        }
    }
}

#[test]
fn test_cost_defaults_and_all_22_designs_match_named_provisional_formula() {
    let db = persona_db();
    let expected = BTreeMap::from([
        ("ODA_NOBUHIDE", 1),
        ("TAKEDA_NOBUTORA", 1),
        ("DATE_TANEMUNE", 2),
        ("ASHINA_MORIKIYO", 1),
        ("HOJO_UJITSUNA", 2),
        ("YAMANOUCHI_UESUGI_NORIMASA", 3),
        ("NAGAO_TAMEKAGE", 2),
        ("IMAGAWA_UJITERU", 2),
        ("MATSUDAIRA_KIYOYASU", 1),
        ("ASAKURA_TAKAKAGE", 2),
        ("ASHIKAGA_YOSHIHARU", 3),
        ("MIYOSHI_NAGAYOSHI", 2),
        ("ROKKAKU_SADAYORI", 2),
        ("HONGANJI_SHONYO", 3),
        ("OUCHI_YOSHITAKA", 2),
        ("AMAGO_TSUNEHISA", 2),
        ("MORI_MOTONARI", 1),
        ("CHOSOKABE_KUNICHIKA", 1),
        ("OTOMO_YOSHIAKI", 2),
        ("SHIMAZU_TAKAHISA", 1),
        ("RYUKYU_SHO_SEI", 3),
        ("AINU_TARIKONA", 2),
    ]);
    let people = q(
        &db,
        "SELECT LeaderType,EvolutionCost,Personality,Trait,Age1534 FROM JAC_LeaderPersonas",
    );
    let actual = rows(&people)
        .iter()
        .map(|row| {
            (
                row[0]
                    .as_str()
                    .unwrap()
                    .strip_prefix("LEADER_JAC_")
                    .unwrap(),
                row[1].as_i64().unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(actual, expected);
    assert!(rows(&people)
        .iter()
        .all(|row| !row[2].as_str().unwrap().is_empty() && !row[3].as_str().unwrap().is_empty()));
    assert_eq!(
        rows(&people)
            .iter()
            .filter(|row| row[4].is_null())
            .map(|row| row[0]
                .as_str()
                .unwrap()
                .strip_prefix("LEADER_JAC_")
                .unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["NAGAO_TAMEKAGE", "AINU_TARIKONA"])
    );
    assert_eq!(
        value_map(&q(&db, "SELECT Name,Value FROM JAC_PersonaCostParameters")),
        BTreeMap::from([
            ("BASE_TITLES".to_owned(), json!(1)),
            ("FAME_TITLE_WEIGHT".to_owned(), json!(1)),
            ("YOUTH_AGE_LIMIT".to_owned(), json!(16)),
            ("YOUTH_TITLE_SURCHARGE".to_owned(), json!(1)),
            ("REPRESENTATIVE_TITLE_SURCHARGE".to_owned(), json!(1)),
            ("MIN_TITLES".to_owned(), json!(1)),
            ("MAX_TITLES".to_owned(), json!(3)),
        ])
    );
}

#[test]
fn test_base_and_shared_effects_use_official_types_and_conservative_magnitudes() {
    let db = persona_db();
    let skills = q(
        &db,
        "SELECT LeaderType,YieldType,Amount FROM JAC_PersonaSkills ORDER BY LeaderType",
    );
    assert_eq!(rows(&skills).len(), 22);
    let bases = rows(&skills)
        .iter()
        .map(|row| (row[0].as_str().unwrap(), json!([row[1], row[2]])))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        bases["LEADER_JAC_DATE_TANEMUNE"],
        json!(["YIELD_CULTURE", 10])
    );
    assert_eq!(bases["LEADER_JAC_ODA_NOBUHIDE"], json!(["YIELD_GOLD", 10]));
    for (modifier, amount) in [
        ("LIBRARIAN_CULTURE_YIELD_BONUS", 15),
        ("PASHA_BONUS_UNIT_PRODUCTION", 20),
    ] {
        let result = query(
            &db,
            "SELECT Value FROM ModifierArguments WHERE ModifierId=? AND Name='Amount'",
            params![modifier],
        );
        assert_eq!(
            result[0][0].as_str().unwrap().parse::<i64>().unwrap(),
            amount
        );
    }
    assert_eq!(
        value_map(&q(
            &db,
            "SELECT EffectKind,MAX(Amount) FROM JAC_PersonaTreeNodes GROUP BY EffectKind"
        )),
        BTreeMap::from([
            ("AMENITIES".to_owned(), json!(1)),
            ("CULTURE".to_owned(), json!(4)),
            ("FAITH".to_owned(), json!(4)),
            ("FOOD".to_owned(), json!(4)),
            ("GOLD".to_owned(), json!(4)),
            ("MILITARY".to_owned(), json!(10)),
            ("PRODUCTION".to_owned(), json!(4)),
            ("SCIENCE".to_owned(), json!(4)),
        ])
    );
    assert_eq!(
        value_map(&q(
            &db,
            "SELECT Name,Value FROM JAC_PersonaTreeParameters WHERE Name LIKE '%COST_%'"
        )),
        BTreeMap::from([
            ("NODE_COST_1".to_owned(), json!(10)),
            ("NODE_COST_2".to_owned(), json!(15)),
            ("NODE_COST_3".to_owned(), json!(22)),
            ("STRONG_COST_1".to_owned(), json!(8)),
            ("STRONG_COST_2".to_owned(), json!(12)),
            ("STRONG_COST_3".to_owned(), json!(18)),
        ])
    );
}

#[test]
fn test_titles_come_only_from_15_sengoku_civics_and_only_persona_governors_exist() {
    let db = persona_db();
    assert_eq!(q(&db, "SELECT ModifierId FROM Modifiers JOIN DynamicModifiers USING(ModifierType) WHERE EffectType='EFFECT_ADJUST_PLAYER_GOVERNOR_POINTS'"), json!([["JAC_PERSONA_CIVIC_EVOLUTION_TITLE"]]));
    let grants = q(
        &db,
        "SELECT CivicType FROM CivicModifiers WHERE ModifierId='JAC_PERSONA_CIVIC_EVOLUTION_TITLE'",
    );
    assert_eq!(rows(&grants).len(), 15);
    assert!(rows(&grants)
        .iter()
        .all(|row| row[0].as_str().unwrap().starts_with("CIVIC_JAC_SENGOKU_")));
    assert_eq!(
        row_set(&grants),
        row_set(&q(&db, "SELECT CivicType FROM JAC_PersonaTitleCivics"))
    );
    assert_eq!(q(&db, "SELECT Name,Value FROM ModifierArguments WHERE ModifierId='JAC_PERSONA_CIVIC_EVOLUTION_TITLE'"), json!([["Delta", "1"]]));
    for table in [
        "GameModifiers",
        "TraitModifiers",
        "BuildingModifiers",
        "GovernorModifiers",
        "GreatPersonIndividualActionModifiers",
    ] {
        assert_eq!(count(&db, &format!("SELECT COUNT(*) FROM {table} WHERE ModifierId='JAC_PERSONA_CIVIC_EVOLUTION_TITLE'")), 0, "{table}");
    }
    assert_eq!(count(&db, "SELECT COUNT(*) FROM GoodyHutSubTypes WHERE ModifierID='JAC_PERSONA_CIVIC_EVOLUTION_TITLE'"), 0);
    assert_eq!(
        string_set(&q(
            &db,
            "SELECT GovernorType FROM JAC_PersonaDisabledGovernors"
        )),
        BTreeSet::from(
            [
                "GOVERNOR_THE_DEFENDER",
                "GOVERNOR_THE_CARDINAL",
                "GOVERNOR_THE_BUILDER",
                "GOVERNOR_THE_AMBASSADOR",
                "GOVERNOR_THE_RESOURCE_MANAGER",
                "GOVERNOR_THE_MERCHANT",
                "GOVERNOR_THE_EDUCATOR",
                "GOVERNOR_IBRAHIM",
            ]
            .map(str::to_owned)
        )
    );
    assert_eq!(
        row_set(&q(&db, "SELECT GovernorType FROM Governors")),
        row_set(&q(&db, "SELECT GovernorType FROM JAC_LeaderPersonas"))
    );
    for table in [
        "GovernorPromotionSets",
        "GovernorModifiers",
        "Governors_XP2",
        "GovernorsCannotAssign",
    ] {
        assert_eq!(count(&db, &format!("SELECT COUNT(*) FROM {table} WHERE GovernorType IN (SELECT GovernorType FROM JAC_PersonaDisabledGovernors)")), 0, "{table}");
    }
    assert_eq!(count(&db, "SELECT COUNT(*) FROM Governors WHERE GovernorType IN (SELECT GovernorType FROM JAC_LeaderPersonas) AND TraitType IS NOT NULL"), 0);
    assert_eq!(
        count(&db, "SELECT COUNT(*) FROM Kinds WHERE Kind='KIND_TRAIT'"),
        1
    );
}

#[test]
fn test_official_governor_removal_preserves_another_mods_governor_and_children() {
    let db = open_snapshot(&fixture().before);
    let foreign = "GOVERNOR_OTHER_MOD";
    db.execute(
        "INSERT INTO Types (Type,Kind) VALUES (?, 'KIND_GOVERNOR')",
        params![foreign],
    )
    .unwrap();
    db.execute("INSERT INTO Governors (GovernorType,Name,Description,Title,ShortTitle,PortraitImage,PortraitImageSelected) SELECT ?,Name,Description,Title,ShortTitle,PortraitImage,PortraitImageSelected FROM Governors WHERE GovernorType='GOVERNOR_THE_DEFENDER'", params![foreign]).unwrap();
    let promotion: String = db.query_row("SELECT GovernorPromotion FROM GovernorPromotionSets WHERE GovernorType='GOVERNOR_THE_DEFENDER' ORDER BY GovernorPromotion LIMIT 1", [], |row| row.get(0)).unwrap();
    db.execute(
        "INSERT INTO GovernorPromotionSets VALUES (?, ?)",
        params![foreign, promotion],
    )
    .unwrap();
    apply_persona(&db, &Repository::checked_in());
    assert_eq!(
        query(
            &db,
            "SELECT GovernorType FROM Governors WHERE GovernorType=?",
            params![foreign]
        ),
        json!([[foreign]])
    );
    assert_eq!(
        query(
            &db,
            "SELECT GovernorPromotion FROM GovernorPromotionSets WHERE GovernorType=?",
            params![foreign]
        ),
        json!([[promotion]])
    );
    assert_eq!(
        query(
            &db,
            "SELECT GovernorType FROM JAC_PersonaDisabledGovernors WHERE GovernorType=?",
            params![foreign]
        ),
        json!([])
    );
}

fn child<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Node<'a, 'input> {
    node.children()
        .find(|candidate| candidate.has_tag_name(name))
        .unwrap_or_else(|| panic!("missing {name} under {}", node.tag_name().name()))
}

#[test]
fn test_modinfo_files_exist_and_every_action_is_scoped_to_a_japan_map() {
    let repo = Repository::checked_in();
    let source = repo
        .read(format!("{MOD}/JAC_PoC_LeaderPersona.modinfo"))
        .unwrap();
    let document = Document::parse(&source).unwrap();
    let root = document.root_element();
    let declared = child(root, "Files")
        .children()
        .filter(|node| node.has_tag_name("File"))
        .map(|node| node.text().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    let actual = WalkDir::new(repo.path(MOD))
        .into_iter()
        .map(|entry| entry.unwrap())
        .filter(|entry| {
            entry.file_type().is_file()
                && entry.path().extension().and_then(|value| value.to_str()) != Some("modinfo")
        })
        .map(|entry| {
            entry
                .path()
                .strip_prefix(repo.path(MOD))
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(declared, actual);
    let criteria = child(root, "ActionCriteria")
        .children()
        .filter(|node| node.has_tag_name("Criteria"))
        .map(|node| (node.attribute("id").unwrap().to_owned(), node))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(criteria.len(), 4);
    let maps = criteria
        .values()
        .map(|node| {
            child(child(*node, "ConfigurationValueMatches"), "Value")
                .text()
                .unwrap()
                .split("Maps/")
                .nth(1)
                .unwrap()
                .to_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        maps,
        BTreeSet::from(
            [
                "JAC_Japan_15km.Civ6Map",
                "JAC_Japan_20km.Civ6Map",
                "JAC_Japan_15km_Settlers.Civ6Map",
                "JAC_Japan_20km_Settlers.Civ6Map",
            ]
            .map(str::to_owned)
        )
    );
    for action in child(root, "InGameActions")
        .children()
        .filter(Node::is_element)
    {
        let criterion = criteria.get(action.attribute("criteria").unwrap()).unwrap();
        assert_eq!(
            child(*criterion, "RuleSetInUse").text(),
            Some("RULESET_JAC_BANSEIKI")
        );
        for file in action.children().filter(|node| node.has_tag_name("File")) {
            assert!(
                declared.contains(file.text().unwrap()),
                "{}",
                file.text().unwrap()
            );
        }
        if action.has_tag_name("UpdateDatabase") {
            let order = child(child(action, "Properties"), "LoadOrder")
                .text()
                .unwrap()
                .parse::<i64>()
                .unwrap();
            assert!(order > 200, "{}", action.attribute("id").unwrap());
        }
    }
}

#[test]
fn test_each_text_tag_has_exactly_the_three_required_languages() {
    let repo = Repository::checked_in();
    let source = repo
        .read(format!("{MOD}/Text/JAC_LeaderPersona_Text.xml"))
        .unwrap();
    let document = Document::parse(&source).unwrap();
    let mut translations = BTreeMap::<String, BTreeSet<String>>::new();
    for row in child(document.root_element(), "LocalizedText")
        .children()
        .filter(|node| node.has_tag_name("Row"))
    {
        let tag = row.attribute("Tag").unwrap();
        let language = row.attribute("Language").unwrap();
        assert!(
            translations
                .entry(tag.to_owned())
                .or_default()
                .insert(language.to_owned()),
            "duplicate {tag}/{language}"
        );
        assert!(
            !child(row, "Text").text().unwrap_or("").is_empty(),
            "{tag}/{language}"
        );
    }
    assert_eq!(translations.len(), 293);
    let required = BTreeSet::from(["ja_JP", "en_US", "ru_RU"].map(str::to_owned));
    assert!(translations
        .values()
        .all(|languages| *languages == required));
}

#[test]
fn test_governor_icons_reference_the_correct_existing_leader_and_unique_pk() {
    let repo = Repository::checked_in();
    let mut leaders = BTreeMap::<String, (String, String)>::new();
    for file in ["JAC_CustomCivs_Icons.xml", "JAC_Japan1534_Icons.xml"] {
        let source = repo.read(format!("{CUSTOM}/Data/{file}")).unwrap();
        let document = Document::parse(&source).unwrap();
        for row in child(document.root_element(), "IconDefinitions")
            .children()
            .filter(|node| node.has_tag_name("Row"))
        {
            if let Some(suffix) = row
                .attribute("Name")
                .unwrap()
                .strip_prefix("ICON_LEADER_JAC_")
            {
                leaders.insert(
                    suffix.to_owned(),
                    (
                        row.attribute("Atlas").unwrap().to_owned(),
                        row.attribute("Index").unwrap().to_owned(),
                    ),
                );
            }
        }
    }
    let source = repo
        .read(format!("{MOD}/Data/JAC_LeaderPersona_Icons.xml"))
        .unwrap();
    let document = Document::parse(&source).unwrap();
    let icons = child(document.root_element(), "IconDefinitions")
        .children()
        .filter(|node| node.has_tag_name("Row"))
        .collect::<Vec<_>>();
    assert_eq!(leaders.len(), 22);
    assert_eq!(icons.len(), 111);
    assert_eq!(
        icons
            .iter()
            .map(|row| (
                row.attribute("Name").unwrap(),
                row.attribute("Atlas").unwrap()
            ))
            .collect::<BTreeSet<_>>()
            .len(),
        111
    );
    for (suffix, (atlas, index)) in &leaders {
        for end in ["", "_FILL", "_SLOT"] {
            let name = format!("ICON_GOVERNOR_JAC_PERSONA_{suffix}{end}");
            let found = icons
                .iter()
                .filter(|row| {
                    row.attribute("Name") == Some(name.as_str())
                        && row.attribute("Atlas") == Some(atlas.as_str())
                })
                .map(|row| row.attribute("Index").unwrap())
                .collect::<Vec<_>>();
            assert_eq!(found, vec![index.as_str()], "{name}");
        }
    }
    // This is the exact official Expansion 1 XML fixture, rather than a synthetic atlas list.
    let official_source = repo
        .game_read("DLC/Expansion1/Data/Expansion1_Icons_Governors.xml")
        .unwrap();
    let official = Document::parse(&official_source).unwrap();
    for (end, filename) in [("_FILL", "Fill"), ("_SLOT", "Slot")] {
        let atlas = format!("ICON_ATLAS_EXPANSION_1_GOVERNORS{end}");
        let sizes = child(official.root_element(), "IconTextureAtlases")
            .children()
            .filter(|row| row.has_tag_name("Row") && row.attribute("Name") == Some(atlas.as_str()))
            .map(|row| {
                (
                    row.attribute("IconSize").unwrap().parse::<i64>().unwrap(),
                    row.attribute("Filename").unwrap().to_owned(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            sizes,
            BTreeMap::from([
                (22, format!("XP1_GovernorsCityBanner{filename}22.dds")),
                (32, format!("XP1_GovernorsCityBanner{filename}32.dds")),
            ])
        );
        let meters = icons
            .iter()
            .filter(|row| row.attribute("Atlas") == Some(atlas.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            meters
                .iter()
                .map(|row| row.attribute("Name").unwrap().to_owned())
                .collect::<BTreeSet<_>>(),
            leaders
                .keys()
                .map(|suffix| format!("ICON_GOVERNOR_JAC_PERSONA_{suffix}{end}"))
                .collect()
        );
        assert_eq!(
            meters
                .iter()
                .map(|row| row.attribute("Index").unwrap())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["6"])
        );
    }
}

#[test]
fn test_gameplay_never_registers_ui_events_or_uses_a_void_unit_return() {
    let lua = Repository::checked_in()
        .read(format!("{MOD}/Scripts/JAC_LeaderPersona_Gameplay.lua"))
        .unwrap();
    for forbidden in [
        "Events.LoadScreenClose",
        "UnitManager.InitUnitValidAdjacentHex",
        "\"\"",
        "LuaEvents.",
        "GrantTestTitle",
        "TEST_TITLE_",
    ] {
        assert!(!lua.contains(forbidden), "unexpected {forbidden}");
    }
}

#[test]
fn test_dedicated_persona_unit_loads_the_official_general_model_on_all_target_maps() {
    let repo = Repository::checked_in();
    let art_source = repo.read(format!("{MOD}/ArtDefs/Units.artdef")).unwrap();
    let art = Document::parse(&art_source).unwrap();
    let units = child(art.root_element(), "m_RootCollections")
        .children()
        .filter(|node| node.has_tag_name("Element"))
        .flat_map(|collection| {
            collection
                .children()
                .filter(|node| node.has_tag_name("Element"))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        units
            .iter()
            .map(|unit| child(*unit, "m_Name").attribute("text").unwrap())
            .collect::<Vec<_>>(),
        vec!["UNIT_JAC_PERSONA"]
    );
    let members = units[0].descendants().filter(|node| {
        node.has_tag_name("Element")
            && node.attribute("class") == Some("AssetObjects..ArtDefReferenceValue")
    });
    assert!(members
        .into_iter()
        .any(|member| child(member, "m_ElementName").attribute("text")
            == Some("GreatGeneral_Classical_Male")
            && child(member, "m_RootCollectionName").attribute("text") == Some("UnitMemberTypes")));
    let dep_source = repo
        .read(format!("{MOD}/JAC_PoC_LeaderPersona.dep"))
        .unwrap();
    let dep = Document::parse(&dep_source).unwrap();
    let dependency_paths = child(dep.root_element(), "ArtDefDependencies")
        .children()
        .filter(|node| node.has_tag_name("Element"))
        .map(|node| child(node, "ArtDefPath").attribute("text").unwrap())
        .collect::<Vec<_>>();
    assert_eq!(dependency_paths, vec!["Units.artdef"]);
    let mod_source = repo
        .read(format!("{MOD}/JAC_PoC_LeaderPersona.modinfo"))
        .unwrap();
    let modinfo = Document::parse(&mod_source).unwrap();
    let actions = child(modinfo.root_element(), "InGameActions")
        .children()
        .filter(|node| node.has_tag_name("UpdateArt"))
        .collect::<Vec<_>>();
    assert_eq!(
        actions
            .iter()
            .map(|node| node.attribute("criteria").unwrap().to_owned())
            .collect::<BTreeSet<_>>(),
        (0..4)
            .map(|index| format!("JAC_Persona_Map_{index}"))
            .collect()
    );
    assert_eq!(
        actions
            .iter()
            .map(|node| child(*node, "File").text().unwrap())
            .collect::<Vec<_>>(),
        vec!["JAC_PoC_LeaderPersona.dep"; 4]
    );
    let icons_source = repo
        .read(format!("{MOD}/Data/JAC_LeaderPersona_Icons.xml"))
        .unwrap();
    let icons = Document::parse(&icons_source).unwrap();
    let unit_icon = child(icons.root_element(), "IconDefinitions")
        .children()
        .find(|node| {
            node.has_tag_name("Row") && node.attribute("Name") == Some("ICON_UNIT_JAC_PERSONA")
        })
        .unwrap();
    assert_eq!(
        (unit_icon.attribute("Atlas"), unit_icon.attribute("Index")),
        (Some("ICON_ATLAS_UNITS"), Some("8"))
    );
}

#[test]
fn test_live_scenario_compiles_and_asserts_civic_titles_merit_and_permanent_locks() {
    let scenario = Repository::checked_in()
        .json("tools/civ6-autotest/scenarios/leader-persona-15km.json")
        .unwrap();
    let steps = scenario["steps"].as_array().unwrap();
    let saves = steps
        .iter()
        .filter(|step| step["action"] == "save")
        .map(|step| step["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    let loads = steps
        .iter()
        .filter(|step| step["action"] == "load")
        .map(|step| step["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(loads, saves);
    assert!(saves.len() >= 4);
    assert!(steps
        .iter()
        .any(|step| step["action"] == "end_turn" && step["turns"].as_i64().unwrap_or(1) > 1));
    let codes = steps
        .iter()
        .filter(|step| step["action"] == "ui_lua")
        .map(|step| step["code"].as_str().unwrap())
        .chain(
            steps
                .iter()
                .filter(|step| step["action"] == "wait_for" && step["condition"]["type"] == "lua")
                .map(|step| step["condition"]["code"].as_str().unwrap()),
        )
        .collect::<Vec<_>>();
    let lua = mlua::Lua::new();
    for (index, code) in codes.iter().enumerate() {
        lua.load(*code)
            .into_function()
            .unwrap_or_else(|error| panic!("scenario Lua #{index}: {error}"));
    }
    let assertions = codes.join("\n");
    for expected in [
        "SetCulturalProgress",
        "JAC_PersonaDisabledGovernors",
        "GetGovernorPointsSpent",
        "JAC_TreeConfirm",
        "JAC_TreeCancel",
        "CATEGORY_LOCKED",
        "BRANCH_LOCKED",
        "PREREQ_LOCKED",
        "MERIT",
        "NODE_",
        "GOVERN_LAST_TURN",
        "EVOLUTION_SURCHARGE",
        "IsEstablished",
        "GetAssignedCity",
        "GameEffects.GetModifierSubjects",
        "GameEffects.GetModifierActive",
    ] {
        assert!(
            assertions.contains(expected),
            "scenario is missing {expected}"
        );
    }
    assert!(!assertions.contains("GrantTestTitle"));
}
