use civ6_data_rs::cascade::load_xml;
use rusqlite::{Connection, Error, ErrorCode};
use std::collections::BTreeSet;

fn fixture() -> (Connection, BTreeSet<String>) {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE Records (Id TEXT PRIMARY KEY, Value TEXT NOT NULL);
         INSERT INTO Records VALUES ('existing', 'original');",
    )
    .unwrap();
    (db, BTreeSet::from(["Records".to_owned()]))
}

fn records(db: &Connection) -> Vec<(String, String)> {
    db.prepare("SELECT Id, Value FROM Records ORDER BY Id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[test]
fn row_rejects_duplicate_primary_keys_instead_of_hiding_game_load_errors() {
    let (db, tables) = fixture();
    let error = load_xml(
        &db,
        r#"<GameData><Records><Row Id="existing" Value="changed"/></Records></GameData>"#,
        &tables,
    )
    .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<Error>(),
        Some(Error::SqliteFailure(code, _)) if code.code == ErrorCode::ConstraintViolation
    ));
    assert_eq!(records(&db), [("existing".into(), "original".into())]);
}

#[test]
fn row_inserts_new_keys_and_rejects_missing_required_fields() {
    let (db, tables) = fixture();
    load_xml(
        &db,
        r#"<GameData><Records><Row><Id>new</Id><Value>inserted</Value></Row></Records></GameData>"#,
        &tables,
    )
    .unwrap();
    let error = load_xml(
        &db,
        r#"<GameData><Records><Row Id="missing"/></Records></GameData>"#,
        &tables,
    )
    .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<Error>(),
        Some(Error::SqliteFailure(code, _)) if code.code == ErrorCode::ConstraintViolation
    ));
    assert_eq!(
        records(&db),
        [
            ("existing".into(), "original".into()),
            ("new".into(), "inserted".into()),
        ]
    );
}

#[test]
fn insert_or_ignore_preserves_existing_rows_and_inserts_new_rows() {
    let (db, tables) = fixture();
    load_xml(
        &db,
        r#"<GameData><Records>
            <InsertOrIgnore Id="existing" Value="changed"/>
            <InsertOrIgnore><Id>new</Id><Value>inserted</Value></InsertOrIgnore>
        </Records></GameData>"#,
        &tables,
    )
    .unwrap();
    assert_eq!(
        records(&db),
        [
            ("existing".into(), "original".into()),
            ("new".into(), "inserted".into()),
        ]
    );
}

#[test]
fn replace_changes_existing_rows_and_inserts_new_rows() {
    let (db, tables) = fixture();
    load_xml(
        &db,
        r#"<GameData><Records>
            <Replace Id="existing" Value="changed"/>
            <Replace Id="new" Value="inserted"/>
        </Records></GameData>"#,
        &tables,
    )
    .unwrap();
    assert_eq!(
        records(&db),
        [
            ("existing".into(), "changed".into()),
            ("new".into(), "inserted".into()),
        ]
    );
}
