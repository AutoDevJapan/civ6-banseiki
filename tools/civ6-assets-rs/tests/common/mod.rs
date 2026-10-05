#![allow(dead_code)]
use civ6_assets_rs::*;
use generate::{Dem, ResourceRule, Ring};
use rusqlite::{types::ValueRef, Connection};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::LazyLock,
};
pub struct Maps {
    pub temp: tempfile::TempDir,
    pub dir: PathBuf,
    pub manifest: Vec<Value>,
    pub japan: Vec<Ring>,
    pub official: Value,
    pub rules: BTreeMap<String, ResourceRule>,
}
pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}
pub fn temp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}
pub static MAPS: LazyLock<Maps> = LazyLock::new(|| {
    let t = temp();
    let dir = t.path().join("Maps");
    let mut zipped =
        zip::ZipArchive::new(fs::File::open(fixtures().join("geography.zip")).unwrap()).unwrap();
    let mut raw = vec![];
    zipped
        .by_name("geography.json")
        .unwrap()
        .read_to_end(&mut raw)
        .unwrap();
    let data: Value = serde_json::from_slice(&raw).unwrap();
    let land: Vec<Ring> = serde_json::from_value(data["land"].clone()).unwrap();
    let japan: Vec<Ring> = serde_json::from_value(data["japan"].clone()).unwrap();
    let dem: Dem = serde_json::from_value(data["dem"].clone()).unwrap();
    assert_eq!(data["sources"], json!(generate::SOURCES));
    let official = read_json(fixtures().join("official.json")).unwrap();
    let rules = serde_json::from_value(official["rules"].clone()).unwrap();
    let schema: Vec<String> = serde_json::from_value(official["schema"].clone()).unwrap();
    let mut manifest = vec![];
    for km in constants::SCALES {
        let mut m = generate::generate_map_with_rules(
            km,
            &land,
            &japan,
            &dem,
            &schema,
            &rules,
            &dir.join(format!("JAC_Japan_{km}km.Civ6Map")),
            false,
        )
        .unwrap();
        let name = format!("JAC_Japan_{km}km_Settlers.Civ6Map");
        generate::generate_map_with_rules(
            km,
            &land,
            &japan,
            &dem,
            &schema,
            &rules,
            &dir.join(&name),
            true,
        )
        .unwrap();
        m["settler_file"] = json!(name);
        manifest.push(m);
    }
    write_json(dir.join("manifest.json"), &json!(manifest)).unwrap();
    Maps {
        temp: t,
        dir,
        manifest,
        japan,
        official,
        rules,
    }
});
pub fn maps() -> &'static Maps {
    &MAPS
}
pub fn db(item: &Value) -> Connection {
    Connection::open_with_flags(
        maps().dir.join(s(&item["file"])),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap()
}
pub fn variant(item: &Value) -> Connection {
    Connection::open_with_flags(
        maps().dir.join(s(&item["settler_file"])),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap()
}
pub fn rows(db: &Connection, sql: &str) -> Vec<Vec<Value>> {
    rows_params(db, sql, [])
}
pub fn rows_params(db: &Connection, sql: &str, args: impl rusqlite::Params) -> Vec<Vec<Value>> {
    let mut statement = db.prepare(sql).unwrap();
    let cols = statement.column_count();
    statement
        .query_map(args, |r| {
            (0..cols)
                .map(|i| {
                    Ok(match r.get_ref(i)? {
                        ValueRef::Null => Value::Null,
                        ValueRef::Integer(n) => json!(n),
                        ValueRef::Real(n) => json!(n),
                        ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                        ValueRef::Blob(b) => json!(b),
                    })
                })
                .collect()
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}
pub fn count(db: &Connection, sql: &str) -> i32 {
    n(&rows(db, sql)[0][0])
}
pub fn table_dump(db: &Connection) -> BTreeMap<String, Vec<Vec<Value>>> {
    rows(db, "SELECT name FROM sqlite_master WHERE type='table'")
        .into_iter()
        .map(|r| {
            let name = s(&r[0]).to_owned();
            let mut records = rows(db, &format!("SELECT * FROM \"{name}\""));
            records.sort_by_key(|r| serde_json::to_string(r).unwrap());
            (name, records)
        })
        .collect()
}
pub fn official_xml(name: &str) -> xmltree::Element {
    xmltree::Element::parse(s(&maps().official["excerpts"][name]).as_bytes()).unwrap()
}
pub fn find_all<'a>(e: &'a xmltree::Element, path: &[&str]) -> Vec<&'a xmltree::Element> {
    if path.is_empty() {
        return vec![e];
    }
    elements(e)
        .filter(|e| e.name == path[0])
        .flat_map(|e| find_all(e, &path[1..]))
        .collect()
}
pub fn action_files(
    e: &xmltree::Element,
    context: &str,
    action: &str,
) -> std::collections::BTreeSet<String> {
    find_all(e, &[context, action, "File"])
        .iter()
        .map(|e| e.get_text().unwrap().into_owned())
        .collect()
}
pub fn checked_map_mod() -> PathBuf {
    root().join("poc/JAC_PoC_JapanMapScale")
}
