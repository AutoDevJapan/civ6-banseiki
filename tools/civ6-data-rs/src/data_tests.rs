use crate::asset_dependencies::{art, city_names, feuds};
use crate::Repository;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
mod support {
    use super::*;
    use regex::Regex;
    use rusqlite::{Connection, OpenFlags};
    use std::path::{Path, PathBuf};

    pub const JAPAN: &str = "poc/JAC_PoC_JapanMapScale";
    pub const SENGOKU: &str = "poc/JAC_PoC_SengokuTrees";
    pub const CUSTOM: &str = "poc/JAC_PoC_CustomCivs";
    pub const RULESET: &str = "poc/JAC_PoC_BanseikiRuleset";
    pub const LANGUAGES: [&str; 3] = ["ja_JP", "en_US", "ru_RU"];
    pub use crate::asset_dependencies::SEATS;

    pub fn repo() -> Repository {
        Repository::checked_in()
    }
    pub fn read(path: impl AsRef<Path>) -> String {
        repo().read(path).unwrap().replace("\r\n", "\n")
    }
    pub fn json(path: impl AsRef<Path>) -> Value {
        repo().json(path).unwrap()
    }
    pub fn jp(path: &str) -> String {
        format!("{JAPAN}/{path}")
    }
    pub fn sg(path: &str) -> String {
        format!("{SENGOKU}/{path}")
    }
    pub fn cs(path: &str) -> String {
        format!("{CUSTOM}/{path}")
    }
    pub fn rs(path: &str) -> String {
        format!("{RULESET}/{path}")
    }
    pub fn string(value: &Value) -> &str {
        value.as_str().expect("expected JSON string")
    }
    pub fn integer(value: &Value) -> i64 {
        value.as_i64().expect("expected JSON integer")
    }
    pub fn array(value: &Value) -> &[Value] {
        value.as_array().expect("expected JSON array")
    }
    pub fn re(pattern: &str) -> Regex {
        Regex::new(pattern).unwrap()
    }
    pub fn strings<const N: usize>(items: [&str; N]) -> BTreeSet<String> {
        items.into_iter().map(str::to_owned).collect()
    }
    pub fn attrs(items: &[(&str, &str)]) -> BTreeMap<String, String> {
        items
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }
    pub fn contains(text: &str, fragment: &str) {
        assert!(text.contains(fragment), "missing {fragment:?}");
    }
    pub fn excludes(text: &str, fragment: &str) {
        assert!(!text.contains(fragment), "unexpected {fragment:?}");
    }
    pub fn before(text: &str, first: &str, second: &str) {
        assert!(
            text.find(first).unwrap() < text.find(second).unwrap(),
            "{first:?} must precede {second:?}"
        );
    }
    pub fn captures(pattern: &str, text: &str, group: usize) -> Vec<String> {
        re(pattern)
            .captures_iter(text)
            .map(|c| c[group].to_owned())
            .collect()
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Xml {
        pub name: String,
        pub attributes: BTreeMap<String, String>,
        pub text: String,
        pub children: Vec<Xml>,
    }
    impl Xml {
        pub fn parse(text: &str) -> Self {
            fn convert(node: roxmltree::Node<'_, '_>) -> Xml {
                Xml {
                    name: node.tag_name().name().to_owned(),
                    attributes: node
                        .attributes()
                        .map(|a| (a.name().to_owned(), a.value().to_owned()))
                        .collect(),
                    text: node.text().unwrap_or("").to_owned(),
                    children: node
                        .children()
                        .filter(|c| c.is_element())
                        .map(convert)
                        .collect(),
                }
            }
            convert(roxmltree::Document::parse(text).unwrap().root_element())
        }
        pub fn attr(&self, key: &str) -> &str {
            self.attributes
                .get(key)
                .unwrap_or_else(|| panic!("{} missing attribute {key}", self.name))
        }
        pub fn select<'a>(&'a self, path: &str) -> Vec<&'a Self> {
            let mut nodes = vec![self];
            for name in path.split('/').filter(|s| !s.is_empty()) {
                nodes = nodes
                    .into_iter()
                    .flat_map(|node| {
                        node.children
                            .iter()
                            .filter(move |child| name == "*" || child.name == name)
                    })
                    .collect();
            }
            nodes
        }
        pub fn one(&self, path: &str) -> &Self {
            self.select(path)
                .into_iter()
                .next()
                .unwrap_or_else(|| panic!("{} missing {path}", self.name))
        }
        pub fn value(&self, path: &str) -> &str {
            &self.one(path).text
        }
        pub fn all(&self) -> Vec<&Self> {
            let mut nodes = vec![self];
            for child in &self.children {
                nodes.extend(child.all());
            }
            nodes
        }
    }
    pub fn xml(path: impl AsRef<Path>) -> Xml {
        Xml::parse(&read(path))
    }
    pub fn game_xml(path: &str) -> Xml {
        Xml::parse(&repo().game_read(path).unwrap())
    }
    pub fn file_names(node: &Xml) -> Vec<String> {
        node.select("File")
            .into_iter()
            .map(|x| x.text.clone())
            .collect()
    }
    pub fn attr_set(root: &Xml, path: &str, key: &str) -> BTreeSet<String> {
        root.select(path)
            .into_iter()
            .map(|x| x.attr(key).to_owned())
            .collect()
    }
    pub fn find_attr<'a>(root: &'a Xml, path: &str, key: &str, value: &str) -> &'a Xml {
        root.select(path)
            .into_iter()
            .find(|r| r.attributes.get(key).is_some_and(|v| v == value))
            .unwrap()
    }
    pub fn paths(base: &str, extensions: &[&str]) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for entry in walkdir::WalkDir::new(repo().path(base)) {
            let entry = entry.unwrap();
            if entry.file_type().is_file()
                && entry
                    .path()
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| extensions.contains(&e))
            {
                paths.push(
                    entry
                        .path()
                        .strip_prefix(&repo().root)
                        .unwrap()
                        .to_path_buf(),
                );
            }
        }
        paths.sort();
        paths
    }
    pub fn direct_paths(base: &str, extensions: &[&str]) -> Vec<PathBuf> {
        let parent = Path::new(base);
        paths(base, extensions)
            .into_iter()
            .filter(|p| p.parent() == Some(parent))
            .collect()
    }
    pub fn mod_texts() -> BTreeMap<String, BTreeMap<String, String>> {
        let mut texts: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        for path in paths("poc", &["xml"]) {
            let parts: Vec<_> = path.components().collect();
            if parts.len() != 4 || path.parent().unwrap().file_name().unwrap() != "Text" {
                continue;
            }
            for row in xml(&path).all().into_iter().filter(|r| r.name == "Row") {
                let tag = row.attributes.get("Tag").cloned().unwrap_or_default();
                let language = row.attributes.get("Language").cloned().unwrap_or_default();
                let value = row
                    .select("Text")
                    .first()
                    .map(|x| x.text.clone())
                    .unwrap_or_default();
                texts.entry(tag).or_default().insert(language, value);
            }
        }
        texts
    }
    pub fn db(path: impl AsRef<Path>) -> Connection {
        Connection::open_with_flags(repo().path(path), OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
    }
    pub fn query<T, F>(db: &Connection, sql: &str, map: F) -> Vec<T>
    where
        F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        let mut statement = db.prepare(sql).unwrap();
        let rows = statement.query_map([], map).unwrap();
        rows.collect::<rusqlite::Result<Vec<_>>>().unwrap()
    }
    pub fn id_set(db: &Connection, sql: &str) -> BTreeSet<i64> {
        query(db, sql, |r| r.get(0)).into_iter().collect()
    }
    pub fn string_map(db: &Connection, sql: &str) -> BTreeMap<i64, String> {
        query(db, sql, |r| Ok((r.get(0)?, r.get(1)?)))
            .into_iter()
            .collect()
    }
    pub fn statements(path: &str) -> Vec<String> {
        read(path)
            .lines()
            .filter(|l| !l.trim_start().starts_with("--"))
            .collect::<Vec<_>>()
            .join("\n")
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()
    }
    pub fn scenario(name: &str) -> Vec<Value> {
        array(&json(format!("tools/civ6-autotest/scenarios/{name}.json"))["steps"]).to_vec()
    }
    pub fn code(step: &Value) -> &str {
        step["code"].as_str().unwrap_or("")
    }
    pub fn actions(steps: &[Value]) -> Vec<String> {
        steps
            .iter()
            .map(|s| string(&s["action"]).to_owned())
            .collect()
    }
    pub fn autoplays(steps: &[Value]) -> Vec<i64> {
        steps
            .iter()
            .filter(|s| s["action"] == "autoplay")
            .map(|s| integer(&s["turns"]))
            .collect()
    }
    pub fn human_seats(steps: &[Value]) -> Vec<String> {
        array(&steps.iter().find(|s| s["action"] == "configure").unwrap()["players"])
            .iter()
            .filter(|p| p["human"] == true)
            .map(|p| string(&p["civ"]).to_owned())
            .collect()
    }
    pub fn pair(a: &str, b: &str) -> BTreeSet<String> {
        strings([a, b])
    }

    #[derive(Debug, Clone)]
    pub struct Site {
        pub id: String,
        pub resource: String,
        pub plot: i64,
        pub year: i64,
        pub tech: Option<String>,
    }
    pub fn sites() -> BTreeMap<String, Vec<Site>> {
        let lua = mlua::Lua::new();
        lua.globals()
            .set("ExposedMembers", lua.create_table().unwrap())
            .unwrap();
        lua.load(read(jp("Scripts/JAC_ResourceSites.lua")))
            .exec()
            .unwrap();
        let exposed: mlua::Table = lua.globals().get("ExposedMembers").unwrap();
        let data: mlua::Table = exposed.get("JAC_ResourceSiteData").unwrap();
        let mut out = BTreeMap::new();
        for entry in data.pairs::<String, mlua::Table>() {
            let (size, rows) = entry.unwrap();
            let mut sites = Vec::new();
            for row in rows.sequence_values::<mlua::Table>() {
                let row = row.unwrap();
                sites.push(Site {
                    id: row.get("id").unwrap(),
                    resource: row.get("resource").unwrap(),
                    plot: row.get("plot").unwrap(),
                    year: row.get("year").unwrap(),
                    tech: row.get("tech").unwrap(),
                });
            }
            out.insert(size, sites);
        }
        out
    }
    pub fn manifest() -> Vec<Value> {
        array(&json(jp("Maps/manifest.json"))).to_vec()
    }
    pub fn records(item: &Value) -> BTreeMap<String, Value> {
        array(&item["resource_candidates"])
            .iter()
            .map(|r| (string(&r["id"]).to_owned(), r.clone()))
            .collect()
    }
    pub fn techs() -> BTreeSet<String> {
        captures(
            r"\('(TECH_JAC_SENGOKU_[A-Z_]+)', 'LOC_",
            &read(sg("Data/JAC_Sengoku_Trees.sql")),
            1,
        )
        .into_iter()
        .collect()
    }
    pub fn relations_lua() -> String {
        read(jp("Scripts/JAC_Japan_1534Relations.lua"))
    }
    pub fn relation_data() -> Value {
        feuds::data(&repo()).unwrap()
    }
}

#[cfg(test)]
mod test_resources {
    use super::support::*;
    use super::*;
    use rusqlite::{params, Connection};

    const BASE_RESOURCES: &str = "Base/Assets/Gameplay/Data/Resources.xml";
    const XP2_RESOURCES: &str = "DLC/Expansion2/Data/Expansion2_Resources.xml";
    const BASE_IMPROVEMENTS: &str = "Base/Assets/Gameplay/Data/Improvements.xml";
    const IMPROVEMENT_FOR: [(&str, &str); 4] = [
        ("RESOURCE_JAC_GOLD", "IMPROVEMENT_MINE"),
        ("RESOURCE_JAC_LACQUER", "IMPROVEMENT_PLANTATION"),
        ("RESOURCE_TEA", "IMPROVEMENT_PLANTATION"),
        ("RESOURCE_SILK", "IMPROVEMENT_PLANTATION"),
    ];

    type OfficialPrereqs = (
        BTreeMap<String, Option<String>>,
        BTreeMap<(String, String), Option<String>>,
    );

    fn official_prereqs() -> OfficialPrereqs {
        let base = game_xml(BASE_RESOURCES);
        let mut resources = base
            .select("Resources/Row")
            .into_iter()
            .map(|r| {
                (
                    r.attr("ResourceType").to_owned(),
                    r.attributes.get("PrereqTech").cloned(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let harvests = base
            .select("Resource_Harvests/Row")
            .into_iter()
            .map(|r| {
                (
                    (
                        r.attr("ResourceType").to_owned(),
                        r.attr("YieldType").to_owned(),
                    ),
                    r.attributes.get("PrereqTech").cloned(),
                )
            })
            .collect();
        for update in game_xml(XP2_RESOURCES).select("Resources/Update") {
            if let Some(prereq) = update.select("Set/PrereqTech").first() {
                resources.insert(
                    update.one("Where").attr("ResourceType").to_owned(),
                    Some(prereq.text.clone()),
                );
            }
        }
        (resources, harvests)
    }

    fn valid_resources() -> BTreeMap<(String, String), bool> {
        let mut out = BTreeMap::new();
        for source in [
            game_xml(BASE_IMPROVEMENTS),
            xml(jp("Data/JAC_Japan_Resources.xml")),
        ] {
            for row in source.select("Improvement_ValidResources/Row") {
                out.insert(
                    (
                        row.attr("ImprovementType").to_owned(),
                        row.attr("ResourceType").to_owned(),
                    ),
                    row.attributes
                        .get("MustRemoveFeature")
                        .map(String::as_str)
                        .unwrap_or("true")
                        != "false",
                );
            }
        }
        out
    }

    #[test]
    fn test_deleted_official_techs_never_open_a_resource_early() {
        let (resources, harvests) = official_prereqs();
        let sengoku_techs = techs();
        assert_eq!(sengoku_techs.len(), 32);
        let official_techs: BTreeSet<_> = resources
            .values()
            .chain(harvests.values())
            .flatten()
            .cloned()
            .collect();
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "PRAGMA foreign_keys=ON;
             CREATE TABLE Technologies(TechnologyType TEXT PRIMARY KEY);
             CREATE TABLE Resources(ResourceType TEXT PRIMARY KEY, PrereqTech TEXT
               REFERENCES Technologies(TechnologyType) ON DELETE SET NULL ON UPDATE CASCADE);
             CREATE TABLE Resource_Harvests(ResourceType TEXT, YieldType TEXT, PrereqTech TEXT
               REFERENCES Technologies(TechnologyType) ON DELETE SET NULL ON UPDATE CASCADE,
               PRIMARY KEY(ResourceType, YieldType));",
        )
        .unwrap();
        for tech in sengoku_techs.union(&official_techs) {
            db.execute("INSERT INTO Technologies VALUES (?)", [tech])
                .unwrap();
        }
        for (resource, tech) in &resources {
            db.execute(
                "INSERT INTO Resources VALUES (?, ?)",
                params![resource, tech],
            )
            .unwrap();
        }
        for ((resource, yield_type), tech) in &harvests {
            db.execute(
                "INSERT INTO Resource_Harvests VALUES (?, ?, ?)",
                params![resource, yield_type, tech],
            )
            .unwrap();
        }
        for statement in statements(&sg("Data/JAC_Sengoku_Resources.sql")) {
            db.execute_batch(&statement).unwrap();
        }
        for statement in statements(&sg("Data/JAC_Sengoku_Unlocks.sql")) {
            if statement.starts_with("UPDATE Resources")
                || statement.starts_with("UPDATE Resource_Harvests")
            {
                db.execute_batch(&statement).unwrap();
            }
        }
        db.execute(
            "DELETE FROM Technologies WHERE TechnologyType NOT LIKE 'TECH_JAC_SENGOKU_%'",
            [],
        )
        .unwrap();
        let final_resources: BTreeMap<String, Option<String>> =
            query(&db, "SELECT ResourceType, PrereqTech FROM Resources", |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .into_iter()
            .collect();
        for (resource, official) in resources {
            if let Some(tech) = &final_resources[&resource] {
                assert!(sengoku_techs.contains(tech), "{resource}: {tech}");
            }
            if official.is_some() && resource != "RESOURCE_HORSES" {
                assert!(
                    final_resources[&resource].is_some(),
                    "{resource}: official tech deleted, resource open from turn 1"
                );
            }
        }
        assert_eq!(
            final_resources["RESOURCE_IRON"].as_deref(),
            Some("TECH_JAC_SENGOKU_IRON_FORGING")
        );
        assert_eq!(final_resources["RESOURCE_HORSES"], None);
        let final_harvests: BTreeMap<(String, String), Option<String>> = query(
            &db,
            "SELECT ResourceType, YieldType, PrereqTech FROM Resource_Harvests",
            |r| Ok(((r.get(0)?, r.get(1)?), r.get(2)?)),
        )
        .into_iter()
        .collect();
        for (key, official) in harvests {
            if let Some(tech) = &final_harvests[&key] {
                assert!(sengoku_techs.contains(tech), "{key:?}: {tech}");
            }
            if official.is_some() {
                assert!(
                    final_harvests[&key].is_some(),
                    "{key:?}: harvest open from turn 1"
                );
            }
        }
        let animals: BTreeSet<_> = final_harvests
            .iter()
            .filter(|((resource, _), _)| {
                ["RESOURCE_CATTLE", "RESOURCE_DEER", "RESOURCE_SHEEP"].contains(&resource.as_str())
            })
            .map(|(_, tech)| tech.clone())
            .collect();
        assert_eq!(
            animals,
            BTreeSet::from([Some("TECH_JAC_SENGOKU_ROADS_AND_PACKTRAINS".to_owned())])
        );
    }

    #[test]
    fn test_modinfo_runs_the_resource_gate_between_trees_and_unlocks() {
        let root = xml(sg("JAC_PoC_SengokuTrees.modinfo"));
        let action = find_attr(
            &root,
            "InGameActions/UpdateDatabase",
            "id",
            "JAC_Sengoku_Trees",
        );
        let files = file_names(action);
        let position = |name: &str| files.iter().position(|f| f == name).unwrap();
        assert_eq!(
            position("Data/JAC_Sengoku_Trees.sql") + 1,
            position("Data/JAC_Sengoku_Resources.sql")
        );
        assert!(
            position("Data/JAC_Sengoku_Resources.sql") < position("Data/JAC_Sengoku_Unlocks.sql")
        );
        assert!(root
            .select("Files/File")
            .iter()
            .any(|f| f.text == "Data/JAC_Sengoku_Resources.sql"));
    }

    #[test]
    fn test_each_custom_resource_is_fully_defined_with_text_and_icons() {
        let root = xml(jp("Data/JAC_Japan_Resources.xml"));
        let custom = strings(["RESOURCE_JAC_GOLD", "RESOURCE_JAC_LACQUER"]);
        assert_eq!(attr_set(&root, "Types/Row", "Type"), custom);
        assert_eq!(attr_set(&root, "Resources/Row", "ResourceType"), custom);
        let text = read(jp("Text/JAC_Japan_Resources_Text.xml"));
        let icons = xml(jp("Data/JAC_Japan_Resources_Icons.xml"));
        let icon_names = attr_set(&icons, "IconDefinitions/Row", "Name");
        for row in root.select("Resources/Row") {
            let resource = row.attr("ResourceType");
            assert_eq!(row.attr("ResourceClassType"), "RESOURCECLASS_LUXURY");
            assert_eq!(row.attr("Frequency"), "0");
            assert!(!row.attributes.contains_key("PrereqTech"));
            for language in LANGUAGES {
                contains(
                    &text,
                    &format!("Tag=\"{}\" Language=\"{language}\"", row.attr("Name")),
                );
            }
            assert!(icon_names.contains(&format!("ICON_{resource}")));
            assert!(icon_names.contains(&format!("ICON_{resource}_FOW")));
            for (table, key) in [
                ("Resource_ValidTerrains", "TerrainType"),
                ("Resource_YieldChanges", "YieldType"),
                ("Improvement_ValidResources", "ImprovementType"),
            ] {
                assert!(
                    root.select(&format!("{table}/Row"))
                        .iter()
                        .any(|r| r.attr("ResourceType") == resource
                            && r.attributes.contains_key(key)),
                    "{table}: {resource}"
                );
            }
        }
        for row in icons.select("IconDefinitions/Row") {
            assert!(
                ["ICON_ATLAS_RESOURCES", "ICON_ATLAS_RESOURCES_FOW"].contains(&row.attr("Atlas"))
            );
        }
    }

    #[test]
    fn test_modinfo_wires_every_new_file() {
        let root = xml(jp("JAC_PoC_JapanMapScale.modinfo"));
        let listed: BTreeSet<_> = root
            .select("Files/File")
            .into_iter()
            .map(|f| f.text.clone())
            .collect();
        let mut used = BTreeSet::new();
        for action in &root.one("InGameActions").children {
            if action.attr("id").starts_with("JAC_Japan_Resources") {
                assert_eq!(action.attr("criteria"), "JAC_JapanMapScale_Banseiki");
                used.extend(file_names(action));
            }
        }
        for action in root.select("InGameActions/UpdateDatabase") {
            if action.attributes.contains_key("criteria") && action.attr("id").ends_with("_Rules") {
                assert_eq!(file_names(action), ["Data/JAC_Japan_MapRules.xml"]);
            }
        }
        let scripts: Vec<_> = root
            .select("InGameActions/AddGameplayScripts")
            .into_iter()
            .filter(|a| a.attr("id").ends_with("_Resources_Scripts"))
            .collect();
        let actual: BTreeSet<_> = scripts
            .iter()
            .map(|s| s.attr("criteria").to_owned())
            .collect();
        let expected: BTreeSet<_> = [15, 20]
            .into_iter()
            .flat_map(|km| {
                ["Historical", "Settlers"].map(|mode| format!("JAC_Japan_{km}km_{mode}"))
            })
            .collect();
        assert_eq!(scripts.len(), 4);
        assert_eq!(actual, expected);
        for action in scripts {
            assert_eq!(
                file_names(action),
                ["Scripts/JAC_ResourceSites.lua", "Scripts/JAC_Resources.lua"]
            );
            used.extend(file_names(action));
            let criteria = find_attr(
                &root,
                "ActionCriteria/Criteria",
                "id",
                action.attr("criteria"),
            );
            assert_eq!(criteria.value("RuleSetInUse"), "RULESET_JAC_BANSEIKI");
            contains(
                criteria.value("ConfigurationValueMatches/Value"),
                "Maps/JAC_Japan_",
            );
        }
        assert_eq!(
            used,
            strings([
                "Data/JAC_Japan_Resources.xml",
                "Data/JAC_Japan_Resources_Icons.xml",
                "Text/JAC_Japan_Resources_Text.xml",
                "Scripts/JAC_ResourceSites.lua",
                "Scripts/JAC_Resources.lua",
            ])
        );
        assert!(used.is_subset(&listed));
        for name in used {
            assert!(repo().path(jp(&name)).is_file(), "{name}");
        }
    }

    #[test]
    fn test_lua_data_is_exactly_the_reserved_sites_of_each_map() {
        let lua = sites();
        assert_eq!(
            lua.keys().cloned().collect::<BTreeSet<_>>(),
            strings(["154x67", "204x88"])
        );
        for item in manifest() {
            let size = format!("{}x{}", integer(&item["width"]), integer(&item["height"]));
            let reserved: BTreeMap<_, _> = records(&item)
                .into_iter()
                .filter(|(_, r)| r["placement_status"] == "reserved_for_later_release")
                .collect();
            assert_eq!(
                lua[&size]
                    .iter()
                    .map(|s| s.id.clone())
                    .collect::<BTreeSet<_>>(),
                reserved.keys().cloned().collect()
            );
            for site in &lua[&size] {
                let record = &reserved[&site.id];
                assert_eq!(site.plot, integer(&record["plot"]));
                assert_eq!(site.resource, string(&record["resource_id"]));
                assert_eq!(site.year, integer(&record["reveal_year"]));
            }
            let years: Vec<_> = lua[&size].iter().map(|s| s.year).collect();
            let mut sorted = years.clone();
            sorted.sort();
            assert_eq!(years, sorted);
        }
    }

    #[test]
    fn test_named_ssot_sites_are_deferred_with_a_year_and_a_free_land_tile() {
        let named = [
            ("silver-ikuno", 1542),
            ("silver-sado-tsurushi", 1542),
            ("silver-nobesawa", 1596),
            ("copper-ashio", 1550),
            ("gold-sado-aikawa", 1596),
        ];
        for item in manifest() {
            let records = records(&item);
            let db = db(jp(&format!("Maps/{}", string(&item["file"]))));
            let land = id_set(
                &db,
                "SELECT ID FROM Plots WHERE ContinentType='CONTINENT_ASIA'",
            );
            let busy = id_set(&db, "SELECT ID FROM PlotResources UNION SELECT ID FROM PlotFeatures UNION SELECT Plot FROM Cities");
            let reserved: Vec<_> = records
                .values()
                .filter(|r| r["placement_status"] == "reserved_for_later_release")
                .collect();
            for (id, year) in named {
                assert_eq!(
                    records[id]["placement_status"],
                    "reserved_for_later_release"
                );
                assert_eq!(integer(&records[id]["reveal_year"]), year);
            }
            assert_eq!(
                reserved
                    .iter()
                    .map(|r| integer(&r["plot"]))
                    .collect::<BTreeSet<_>>()
                    .len(),
                reserved.len()
            );
            for record in reserved {
                let plot = integer(&record["plot"]);
                assert!(land.contains(&plot), "{record}");
                assert!(!busy.contains(&plot), "{record}");
                assert!(
                    record["displacement_km"].as_f64().unwrap()
                        <= 30.0_f64.max(2.0 * item["km_per_tile"].as_f64().unwrap()),
                    "{record}"
                );
                assert_eq!(record["start_status"], "deferred");
            }
            let types: BTreeSet<String> =
                query(&db, "SELECT ResourceType FROM PlotResources", |r| r.get(0))
                    .into_iter()
                    .collect();
            assert!(!types.contains("RESOURCE_COPPER"));
        }
    }

    #[test]
    fn test_site_techs_exist_and_the_lua_api_and_test_hook_are_wired() {
        let techs = techs();
        for sites in sites().values() {
            for site in sites {
                if let Some(tech) = &site.tech {
                    assert!(techs.contains(tech), "{}: {tech}", site.id);
                }
                assert!(site.year > 1534);
            }
        }
        let lua = read(jp("Scripts/JAC_Resources.lua"));
        let block = &lua[lua.find("ExposedMembers.JAC_Resources = {").unwrap()..];
        for name in [
            "RevealSite",
            "RevealDue",
            "RevealForTech",
            "GetSites",
            "IsRevealed",
        ] {
            contains(block, &format!("{name} = {name},"));
        }
        let scenario = read("tools/civ6-autotest/scenarios/japan-resources.json");
        excludes(&lua, "TEST_YEAR");
        excludes(&scenario, "TEST_YEAR");
        for fragment in [
            "GameConfiguration.GetStartTurn()",
            "GameInfo.StartEras()",
            "ipairs(speed.GameSpeedTurnCollection)",
            "CurrentYear = CurrentYear,",
            "print(\"[JAC_RESOURCES] \" .. message)",
            "Log(\"loaded\")",
            "Log(string.format(\"revealed %s: ",
        ] {
            contains(&lua, fragment);
        }
        for line in [
            "[JAC_RESOURCES] loaded",
            "[JAC_RESOURCES] revealed silver-ikuno",
        ] {
            contains(&scenario, line);
        }
    }

    #[test]
    fn test_the_scenario_turns_reach_1542_on_the_sengoku_calendar() {
        let calendar = xml(sg("Data/JAC_Sengoku_Calendar.xml"));
        let start: i64 = calendar.value("StartEras/Update/Set/Year").parse().unwrap();
        let rows: Vec<_> = calendar
            .select("GameSpeed_Turns/Row")
            .iter()
            .map(|r| {
                (
                    r.value("GameSpeedType").to_owned(),
                    r.value("MonthIncrement").parse::<i64>().unwrap(),
                    r.value("TurnsPerIncrement").parse::<i64>().unwrap(),
                )
            })
            .collect();
        let year = |speed: &str, mut elapsed: i64| {
            let mut months = 0;
            for (kind, increment, turns) in &rows {
                if kind == speed && elapsed > 0 {
                    let step = elapsed.min(*turns);
                    months += step * increment;
                    elapsed -= step;
                }
            }
            start + months / 12
        };
        let turns: i64 = autoplays(&scenario("japan-resources")).into_iter().sum();
        assert_eq!(
            (
                start,
                year("GAMESPEED_STANDARD", 0),
                year("GAMESPEED_STANDARD", 3)
            ),
            (1534, 1534, 1535)
        );
        assert_eq!(year("GAMESPEED_STANDARD", turns), 1542);
        assert_eq!(year("GAMESPEED_STANDARD", turns - 1), 1541);
    }

    #[test]
    fn test_custom_resources_are_on_the_map_at_the_start() {
        for item in manifest() {
            let db = db(jp(&format!("Maps/{}", string(&item["file"]))));
            let rows = string_map(&db, "SELECT ID, ResourceType FROM PlotResources");
            let records = records(&item);
            for (site, resource) in [
                ("gold-kai-kurokawa", "RESOURCE_JAC_GOLD"),
                ("gold-kai-nakayama", "RESOURCE_JAC_GOLD"),
                ("lacquer-wajima", "RESOURCE_JAC_LACQUER"),
                ("silver-iwami", "RESOURCE_SILVER"),
            ] {
                assert_eq!(rows[&integer(&records[site]["plot"])], resource);
            }
        }
    }

    #[test]
    fn test_a_listed_resource_lets_a_mine_go_on_flat_land() {
        let improvements = game_xml(BASE_IMPROVEMENTS);
        let mine_terrains: BTreeSet<_> = improvements
            .select("Improvement_ValidTerrains/Row")
            .iter()
            .filter(|r| r.attr("ImprovementType") == "IMPROVEMENT_MINE")
            .map(|r| r.attr("TerrainType").to_owned())
            .collect();
        assert!(!mine_terrains.is_empty());
        assert!(mine_terrains.iter().all(|t| t.ends_with("_HILLS")));
        let resources = game_xml(BASE_RESOURCES);
        let salt: BTreeSet<_> = resources
            .select("Resource_ValidTerrains/Row")
            .iter()
            .filter(|r| r.attr("ResourceType") == "RESOURCE_SALT")
            .map(|r| r.attr("TerrainType").to_owned())
            .collect();
        assert!(!salt.is_empty());
        assert!(salt.is_disjoint(&mine_terrains));
        let salt_improvements: Vec<_> = valid_resources()
            .keys()
            .filter(|(_, r)| r == "RESOURCE_SALT")
            .map(|(i, _)| i.clone())
            .collect();
        assert_eq!(salt_improvements, ["IMPROVEMENT_MINE"]);
    }

    #[test]
    fn test_plantation_and_mine_are_buildable_under_the_sengoku_pack() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE Improvements (ImprovementType TEXT PRIMARY KEY, PrereqTech TEXT, PrereqCivic TEXT, Buildable INT)").unwrap();
        for row in game_xml(BASE_IMPROVEMENTS).select("Improvements/Row") {
            let buildable = i64::from(
                row.attributes
                    .get("Buildable")
                    .map(String::as_str)
                    .unwrap_or("true")
                    == "true",
            );
            db.execute(
                "INSERT INTO Improvements VALUES (?, ?, ?, ?)",
                params![
                    row.attr("ImprovementType"),
                    row.attributes.get("PrereqTech"),
                    row.attributes.get("PrereqCivic"),
                    buildable,
                ],
            )
            .unwrap();
        }
        for statement in statements(&sg("Data/JAC_Sengoku_Unlocks.sql")) {
            if statement.starts_with("UPDATE Improvements") {
                db.execute_batch(&statement).unwrap();
            }
        }
        let rows: BTreeMap<String, (Option<String>, i64)> = query(
            &db,
            "SELECT ImprovementType, PrereqTech, Buildable FROM Improvements",
            |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))),
        )
        .into_iter()
        .collect();
        assert_eq!(
            rows["IMPROVEMENT_PLANTATION"],
            (Some("TECH_JAC_SENGOKU_IRRIGATION_WORKS".to_owned()), 1)
        );
        assert_eq!(
            rows["IMPROVEMENT_MINE"],
            (Some("TECH_JAC_SENGOKU_ORE_PROSPECTING".to_owned()), 1)
        );
    }

    #[test]
    fn test_every_gold_lacquer_tea_silk_tile_can_take_its_improvement() {
        let rules = valid_resources();
        let reserved = sites();
        let mut checked: BTreeMap<String, usize> = BTreeMap::new();
        let maps = direct_paths(&jp("Maps"), &["Civ6Map"]);
        assert!(!maps.is_empty());
        for path in maps {
            let db = db(&path);
            let size: (i64, i64) = db
                .query_row("SELECT Width, Height FROM Map", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .unwrap();
            let terrain = string_map(&db, "SELECT ID, TerrainType FROM Plots");
            let features = string_map(&db, "SELECT ID, FeatureType FROM PlotFeatures");
            let tiles = string_map(&db, "SELECT ID, ResourceType FROM PlotResources");
            let later: BTreeMap<_, _> = reserved[&format!("{}x{}", size.0, size.1)]
                .iter()
                .map(|s| (s.plot, s.resource.clone()))
                .collect();
            assert!(
                later.keys().all(|p| !tiles.contains_key(p)),
                "{}",
                path.display()
            );
            for (plot, resource) in tiles.iter().chain(later.iter()) {
                let Some((_, improvement)) = IMPROVEMENT_FOR
                    .iter()
                    .find(|(r, _)| *r == resource.as_str())
                else {
                    continue;
                };
                let key = (improvement.to_string(), resource.clone());
                assert!(
                    rules.contains_key(&key),
                    "{}: {plot} {resource}",
                    path.display()
                );
                if let Some(feature) = features.get(plot) {
                    assert!(!rules[&key], "{feature}: {plot} {resource}");
                }
                assert!(!terrain[plot].starts_with("TERRAIN_COAST"));
                assert!(!terrain[plot].starts_with("TERRAIN_OCEAN"));
                *checked.entry(resource.clone()).or_default() += 1;
            }
        }
        for (resource, _) in IMPROVEMENT_FOR {
            assert!(
                checked.get(resource).copied().unwrap_or(0) > 0,
                "{checked:?}"
            );
        }
    }
}

#[cfg(test)]
mod test_portrait_art {
    use super::support::*;
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn test_atlas_places_all_22_faces_in_eight_columns() {
        let masters: Vec<_> = (0..22)
            .map(|i| RgbaImage::from_pixel(2, 2, Rgba([i + 1, 0, 0, 255])))
            .collect();
        let image = art::atlas(&masters, 2, Some(8)).unwrap();
        assert_eq!(image.dimensions(), (16, 6));
        for index in 0..22_u32 {
            assert_eq!(
                image.get_pixel((index % 8) * 2, (index / 8) * 2).0,
                [index as u8 + 1, 0, 0, 255]
            );
        }
        assert_eq!(image.get_pixel(15, 5).0[3], 0);
    }

    #[test]
    fn test_source_keys_match_frontend_leaders() {
        let mut names = Vec::new();
        for file in ["JAC_CustomCivs_Players.xml", "JAC_Japan1534_Players.xml"] {
            for row in xml(cs(&format!("Config/{file}"))).select("Players/Row") {
                names.push(
                    row.attr("LeaderType")
                        .strip_prefix("LEADER_JAC_")
                        .unwrap()
                        .to_owned(),
                );
            }
        }
        assert_eq!(names, art::LEADER_ORDER);
        assert_eq!(
            art::SUBJECTS
                .iter()
                .map(|s| s.0.to_owned())
                .collect::<BTreeSet<_>>(),
            names.into_iter().collect()
        );
        let leaders = art::leaders();
        for key in art::LEADER_ORDER {
            assert_eq!(
                leaders[key].keys().cloned().collect::<BTreeSet<_>>(),
                strings(["portrait", "figure", "background"])
            );
            for (kind, path) in &leaders[key] {
                assert_eq!(
                    *path,
                    format!("{}/{}", key.to_lowercase(), art::filename(key, kind))
                );
            }
        }
    }

    #[test]
    fn test_prompts_require_three_distinct_files_and_keyless_codex() {
        for subject in art::SUBJECTS {
            let text = art::prompt(subject.0).unwrap();
            contains(&text, "official Codex CLI");
            contains(&text, "No API keys");
            for kind in ["portrait", "figure", "background"] {
                assert_eq!(text.matches(&art::filename(subject.0, kind)).count(), 1);
            }
        }
    }
}

#[cfg(test)]
mod test_lua_file_names {
    use super::support::*;
    use super::*;

    #[test]
    fn test_lua_file_names_are_unique_across_all_mods() {
        let mut names: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let paths = paths("poc", &["lua"]);
        assert!(!paths.is_empty());
        for path in paths {
            names
                .entry(path.file_name().unwrap().to_str().unwrap().to_lowercase())
                .or_default()
                .push(path.to_string_lossy().replace('\\', "/"));
        }
        let duplicates: BTreeMap<_, _> = names
            .into_iter()
            .filter(|(_, paths)| paths.len() > 1)
            .collect();
        assert!(duplicates.is_empty(), "{duplicates:?}");
    }
}

#[cfg(test)]
mod test_localized_names {
    use super::support::*;
    use super::*;
    use rusqlite::types::ValueRef;
    use std::path::PathBuf;

    fn cjk(text: &str) -> bool {
        static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        PATTERN
            .get_or_init(|| re("[　-ヿ㐀-鿿豈-﫿＀-￯]"))
            .is_match(text)
    }
    fn jac_tag() -> &'static regex::Regex {
        static PATTERN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
        PATTERN.get_or_init(|| re(r"LOC_JAC_[A-Z0-9_]*[A-Z0-9]\b"))
    }

    fn text_rows() -> Vec<(PathBuf, String, String, String)> {
        let mut out = Vec::new();
        for path in paths("poc", &["xml"]) {
            let root = xml(&path);
            for table in root.all().into_iter().filter(|n| n.name == "LocalizedText") {
                for row in &table.children {
                    let tag = row.attributes.get("Tag").cloned().unwrap_or_default();
                    let lang = row.attributes.get("Language").cloned().unwrap_or_default();
                    let child_text = row
                        .select("Text")
                        .first()
                        .map(|t| t.text.as_str())
                        .unwrap_or("");
                    let text = if !child_text.is_empty() {
                        child_text.to_owned()
                    } else {
                        row.attributes.get("Text").cloned().unwrap_or_default()
                    };
                    out.push((path.clone(), tag, lang, text));
                }
            }
        }
        out
    }

    fn map_strings() -> Vec<(String, String)> {
        let mut out = Vec::new();
        for path in paths("poc", &["Civ6Map"]) {
            let db = db(&path);
            let tables: Vec<String> = query(
                &db,
                "SELECT name FROM sqlite_master WHERE type='table'",
                |r| r.get(0),
            );
            for table in tables {
                let quoted = table.replace('"', "\"\"");
                let mut statement = db.prepare(&format!("SELECT * FROM \"{quoted}\"")).unwrap();
                let columns = statement.column_count();
                let mut rows = statement.query([]).unwrap();
                while let Some(row) = rows.next().unwrap() {
                    for index in 0..columns {
                        if let ValueRef::Text(bytes) = row.get_ref(index).unwrap() {
                            let value = std::str::from_utf8(bytes).unwrap().to_owned();
                            out.push((
                                format!("{}:{table}", path.file_name().unwrap().to_string_lossy()),
                                value,
                            ));
                        }
                    }
                }
            }
        }
        out
    }

    #[test]
    fn test_every_jac_tag_has_three_languages() {
        let mut languages: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (_, tag, language, _) in text_rows() {
            languages.entry(tag).or_default().insert(language);
        }
        let mut used: BTreeSet<_> = languages.keys().cloned().collect();
        for path in paths("poc", &["sql", "lua", "xml"]) {
            used.extend(
                jac_tag()
                    .find_iter(&read(path))
                    .map(|m| m.as_str().to_owned()),
            );
        }
        for (_, value) in map_strings() {
            if jac_tag()
                .find(&value)
                .is_some_and(|m| m.start() == 0 && m.end() == value.len())
            {
                used.insert(value);
            }
        }
        let required = strings(LANGUAGES);
        let missing: BTreeMap<_, _> = used
            .into_iter()
            .filter_map(|tag| {
                let available = languages.get(&tag).cloned().unwrap_or_default();
                let absent: Vec<_> = required.difference(&available).cloned().collect();
                (!absent.is_empty()).then_some((tag, absent))
            })
            .collect();
        assert!(missing.is_empty(), "{missing:?}");
    }

    #[test]
    fn test_non_japanese_rows_have_no_cjk() {
        let bad: Vec<_> = text_rows()
            .into_iter()
            .filter(|(_, _, lang, text)| lang != "ja_JP" && cjk(text))
            .collect();
        assert!(bad.is_empty(), "{bad:?}");
    }

    #[test]
    fn test_map_names_are_loc_tags() {
        let bad: Vec<_> = map_strings()
            .into_iter()
            .filter(|(_, value)| cjk(value))
            .collect();
        assert!(bad.is_empty(), "{bad:?}");
        for path in paths("poc", &["Civ6Map"]) {
            let db = db(&path);
            let names: Vec<String> = query(&db, "SELECT Name FROM Cities", |r| r.get(0));
            assert!(
                names.iter().all(|n| n.starts_with("LOC_")),
                "{}: {names:?}",
                path.display()
            );
        }
    }

    #[test]
    fn test_no_literal_cjk_outside_japanese_text() {
        let mut bad = Vec::new();
        for path in paths("poc", &["sql", "lua"]) {
            for (number, line) in read(&path).lines().enumerate() {
                if cjk(line.split("--").next().unwrap()) {
                    bad.push(format!(
                        "{}:{}: {}",
                        path.display(),
                        number + 1,
                        line.trim()
                    ));
                }
            }
        }
        fn visit(path: &std::path::Path, node: &Xml, excluded: bool, bad: &mut Vec<String>) {
            let excluded = excluded
                || node.name == "ja_JP"
                || node
                    .attributes
                    .get("Language")
                    .is_some_and(|v| v == "ja_JP");
            if !excluded && (cjk(&node.text) || node.attributes.values().any(|v| cjk(v))) {
                bad.push(format!(
                    "{}:{} {:?} {:?}",
                    path.display(),
                    node.name,
                    node.attributes,
                    node.text
                ));
            }
            for child in &node.children {
                visit(path, child, excluded, bad);
            }
        }
        for path in paths("poc", &["xml", "modinfo", "artdef"]) {
            visit(&path, &xml(&path), false, &mut bad);
        }
        assert!(bad.is_empty(), "{bad:?}");
    }
}

#[cfg(test)]
mod test_japan_no_world_congress {
    use super::support::*;
    use super::*;

    #[test]
    fn test_congress_capability_removed_only_for_all_four_japan_maps() {
        let root = xml(jp("JAC_PoC_JapanMapScale.modinfo"));
        let actions: Vec<_> = root
            .select("InGameActions/UpdateDatabase")
            .into_iter()
            .filter(|r| {
                file_names(r)
                    .iter()
                    .any(|f| f == "Data/JAC_Japan_MapRules.xml")
            })
            .collect();
        assert_eq!(actions.len(), 4);
        let mut selected = BTreeSet::new();
        for action in actions {
            let criterion = find_attr(
                &root,
                "ActionCriteria/Criteria",
                "id",
                action.attr("criteria"),
            );
            assert_eq!(criterion.value("RuleSetInUse"), "RULESET_JAC_BANSEIKI");
            let matched = criterion.one("ConfigurationValueMatches");
            assert_eq!(matched.value("Group"), "Map");
            assert_eq!(matched.value("ConfigurationId"), "MAP_SCRIPT");
            selected.insert(matched.value("Value").to_owned());
        }
        let expected = [
            "JAC_Japan_20km.Civ6Map",
            "JAC_Japan_15km.Civ6Map",
            "JAC_Japan_20km_Settlers.Civ6Map",
            "JAC_Japan_15km_Settlers.Civ6Map",
        ]
        .into_iter()
        .map(|name| format!("{{403be889-1455-4de7-967b-ad54c0ff3431}}Maps/{name}"))
        .collect();
        assert_eq!(selected, expected);
    }

    #[test]
    fn test_capability_delete_does_not_remove_other_gs_systems() {
        let rules = xml(jp("Data/JAC_Japan_MapRules.xml"));
        let deletes: Vec<_> = rules
            .select("Types/Delete")
            .iter()
            .map(|r| r.attributes.clone())
            .collect();
        assert_eq!(
            deletes,
            vec![attrs(&[("Type", "CAPABILITY_WORLD_CONGRESS")])]
        );
        assert!(rules.select("GameCapabilities/Delete").is_empty());
        assert!(!rules.select("GlobalParameters/*/Where").iter().any(|r| {
            r.attributes
                .get("Name")
                .is_some_and(|name| name.starts_with("WORLD_CONGRESS_"))
        }));
    }
}

#[cfg(test)]
mod test_japan_1534_relations {
    use super::support::*;
    use super::*;

    fn feud_xml() -> Xml {
        xml(jp("Data/JAC_Japan_Feuds.xml"))
    }
    fn allies(data: &Value) -> BTreeSet<(String, String)> {
        feuds::relation_pairs(data, &feuds::ALLY_KINDS)
            .unwrap()
            .into_iter()
            .collect()
    }
    fn unordered(pairs: impl IntoIterator<Item = (String, String)>) -> BTreeSet<BTreeSet<String>> {
        pairs.into_iter().map(|(a, b)| pair(&a, &b)).collect()
    }
    fn body<'a>(lua: &'a str, signature: &str) -> &'a str {
        lua.split_once(signature)
            .unwrap()
            .1
            .split_once("\nend\n")
            .unwrap()
            .0
    }

    #[test]
    fn test_factions_match_the_22_map_seats() {
        assert_eq!(SEATS.len(), 22);
        let data = relation_data();
        let factions: Vec<_> = array(&data["factions"]).iter().map(string).collect();
        assert_eq!(factions, SEATS);
        for item in manifest() {
            let seats: Vec<_> = array(&item["starts"])
                .iter()
                .map(|s| string(&s["seat"]))
                .collect();
            assert_eq!(seats, SEATS);
        }
    }

    #[test]
    fn test_relations_are_valid_unordered_pairs() {
        let data = relation_data();
        let factions: BTreeSet<_> = array(&data["factions"])
            .iter()
            .map(|v| string(v).to_owned())
            .collect();
        let mut pairs = BTreeSet::new();
        let mut ids = BTreeSet::new();
        for relation in array(&data["relations"]) {
            let a = string(&relation["a"]);
            let b = string(&relation["b"]);
            assert!(factions.contains(a));
            assert!(factions.contains(b));
            assert_ne!(a, b);
            assert!(
                pairs.insert(pair(a, b)),
                "duplicate unordered pair: {relation}"
            );
            assert!(
                ids.insert(string(&relation["id"]).to_owned()),
                "duplicate id: {relation}"
            );
            assert!(data["relation_types"]
                .as_object()
                .unwrap()
                .contains_key(string(&relation["type"])));
            assert!(["high", "medium", "low"].contains(&string(&relation["confidence"])));
            if relation["type"] == "vassal" {
                assert!([a, b].contains(&string(&relation["lord"])));
            } else {
                assert!(relation.get("lord").is_none());
            }
            assert!(!string(&relation["summary"]).is_empty());
            assert!(!array(&relation["sources"]).is_empty());
            for source in array(&relation["sources"]) {
                assert!(!string(&source["title"]).is_empty());
                assert!(string(&source["url"]).starts_with("https://"));
            }
        }
    }

    #[test]
    fn test_dates_are_well_formed_and_ordered() {
        let data = relation_data();
        assert_eq!(data["start_date"], "1534-05-12");
        let date = re(r"^\d{4}(-\d{2}(-\d{2})?)?$");
        for relation in array(&data["relations"]) {
            for key in ["from", "to"] {
                let value = relation
                    .get(key)
                    .expect("relationship date field is required, including explicit null");
                assert!(
                    value.is_null() || value.as_str().is_some_and(|v| date.is_match(v)),
                    "{value}"
                );
                if let Some(value) = value.as_str() {
                    let parts: Vec<i64> = value
                        .split('-')
                        .skip(1)
                        .map(|v| v.parse().unwrap())
                        .collect();
                    if let Some(month) = parts.first() {
                        assert!((1..=12).contains(month));
                    }
                    if parts.len() == 2 {
                        assert!((1..=30).contains(&parts[1]));
                    }
                }
            }
            if let (Some(from), Some(to)) = (relation["from"].as_str(), relation["to"].as_str()) {
                assert!(
                    feuds::date_key(Some(from), 0).unwrap()
                        <= feuds::date_key(Some(to), 99).unwrap()
                );
            }
        }
    }

    #[test]
    fn test_apply_rule_uses_start_date_and_confidence() {
        let data = relation_data();
        let start = string(&data["start_date"]);
        let by_id: BTreeMap<_, _> = array(&data["relations"])
            .iter()
            .map(|r| (string(&r["id"]), r))
            .collect();
        assert!(feuds::applies(by_id["R01"], start).unwrap());
        assert!(!feuds::applies(by_id["R18"], start).unwrap());
        assert!(!feuds::applies(by_id["R16"], start).unwrap());
        assert!(!feuds::applies(
            &serde_json::json!({"confidence":"high","from":"1534-05-12","to":"1534-05-12"}),
            start
        )
        .unwrap());
        assert!(feuds::applies(
            &serde_json::json!({"confidence":"medium","from":"1534","to":"1534-05"}),
            start
        )
        .unwrap());
    }

    #[test]
    fn test_lua_copies_exactly_the_applicable_rows() {
        let data = relation_data();
        let start = string(&data["start_date"]);
        let expected: Vec<_> = array(&data["relations"])
            .iter()
            .filter(|r| feuds::applies(r, start).unwrap())
            .map(|r| {
                (
                    string(&r["a"]).to_owned(),
                    string(&r["b"]).to_owned(),
                    string(&r["type"]).to_owned(),
                    string(&r["id"]).to_owned(),
                )
            })
            .collect();
        let lua = relations_lua();
        let body = lua
            .split_once("local RELATIONS = {")
            .unwrap()
            .1
            .split_once("\n}")
            .unwrap()
            .0;
        let actual: Vec<_> = re(r#"\{"([A-Z_]+)", "([A-Z_]+)", "([a-z]+)", "(R\d+)"\}"#)
            .captures_iter(body)
            .map(|c| {
                (
                    c[1].to_owned(),
                    c[2].to_owned(),
                    c[3].to_owned(),
                    c[4].to_owned(),
                )
            })
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(actual.len(), body.matches('{').count());
        assert_eq!(
            actual.iter().map(|r| r.2.clone()).collect::<BTreeSet<_>>(),
            strings([
                "war",
                "alliance",
                "marriage",
                "vassal",
                "hostility",
                "friendship"
            ])
        );
        assert!(actual.contains(&(
            "SHIMAZU".to_owned(),
            "RYUKYU".to_owned(),
            "friendship".to_owned(),
            "R19".to_owned()
        )));
        contains(&lua, "LuaEvents.NewGameInitialized.Add(Apply)");
    }

    #[test]
    fn test_script_and_feuds_run_on_the_two_historical_maps_only() {
        let root = xml(jp("JAC_PoC_JapanMapScale.modinfo"));
        let config = xml(jp("Config/JAC_JapanMapScale_Config.xml"));
        let map_files = attr_set(&config, "Maps/Row", "File");
        let historical: BTreeSet<_> = map_files
            .iter()
            .filter(|f| !f.contains("Settlers"))
            .cloned()
            .collect();
        assert_eq!(map_files.len(), 4);
        assert_eq!(historical.len(), 2);
        for (tag, path) in [
            ("AddGameplayScripts", "Scripts/JAC_Japan_1534Relations.lua"),
            ("UpdateDatabase", "Data/JAC_Japan_Feuds.xml"),
        ] {
            let rows: Vec<_> = root
                .select(&format!("InGameActions/{tag}"))
                .into_iter()
                .filter(|r| r.select("File").first().is_some_and(|f| f.text == path))
                .collect();
            assert_eq!(rows.len(), 2, "{path}");
            let mut selected = BTreeSet::new();
            for row in rows {
                let criterion =
                    find_attr(&root, "ActionCriteria/Criteria", "id", row.attr("criteria"));
                assert_eq!(criterion.value("RuleSetInUse"), "RULESET_JAC_BANSEIKI");
                assert_eq!(
                    criterion.value("ConfigurationValueMatches/ConfigurationId"),
                    "MAP_SCRIPT"
                );
                selected.insert(
                    criterion
                        .value("ConfigurationValueMatches/Value")
                        .to_owned(),
                );
            }
            assert_eq!(selected, historical, "{path}");
        }
        let files: BTreeSet<_> = root
            .select("Files/File")
            .iter()
            .map(|r| r.text.clone())
            .collect();
        assert!(strings([
            "Scripts/JAC_Japan_1534Relations.lua",
            "Data/JAC_Japan_Feuds.xml"
        ])
        .is_subset(&files));
    }

    #[test]
    fn test_feud_xml_is_generated_from_the_hostility_rows() {
        let data = relation_data();
        let expected = unordered(
            array(&data["relations"])
                .iter()
                .filter(|r| {
                    r["type"] == "hostility"
                        && feuds::applies(r, string(&data["start_date"])).unwrap()
                })
                .map(|r| (string(&r["a"]).to_owned(), string(&r["b"]).to_owned())),
        );
        assert_eq!(expected.len(), 7);
        assert_eq!(unordered(feuds::feud_pairs(&repo()).unwrap()), expected);
        let root = feud_xml();
        let mods: BTreeMap<_, _> = root
            .select("Modifiers/Row")
            .into_iter()
            .filter(|r| {
                ![feuds::ACCESS_MODIFIER, feuds::ALLY_VISION_MODIFIER]
                    .contains(&r.attr("ModifierId"))
            })
            .map(|r| (r.attr("ModifierId").to_owned(), r))
            .collect();
        let owners: BTreeSet<_> = root
            .select("TraitModifiers/Row")
            .iter()
            .map(|r| {
                (
                    r.attr("TraitType").to_owned(),
                    r.attr("ModifierId").to_owned(),
                )
            })
            .collect();
        for pair in &expected {
            let members: Vec<_> = pair.iter().collect();
            for (a, b) in [(members[0], members[1]), (members[1], members[0])] {
                let id = format!("JAC_1534_FEUD_{a}_{b}");
                let modifier = mods[&id];
                assert_eq!(
                    modifier.attr("ModifierType"),
                    "MODIFIER_PLAYER_DIPLOMACY_SIMPLE_MODIFIER"
                );
                assert_eq!(
                    modifier.attr("SubjectRequirementSetId"),
                    format!("JAC_1534_PLAYER_IS_{b}")
                );
                assert!(owners.contains(&(format!("TRAIT_JAC_1534_FEUD_{a}"), id)));
            }
        }
        let args: BTreeMap<_, _> = root
            .select("ModifierArguments/Row")
            .iter()
            .map(|r| {
                (
                    (r.attr("ModifierId").to_owned(), r.attr("Name").to_owned()),
                    r.attr("Value").to_owned(),
                )
            })
            .collect();
        assert_eq!(mods.len(), 14);
        for id in mods.keys() {
            assert!(
                args[&(id.clone(), "InitialValue".to_owned())]
                    .parse::<i64>()
                    .unwrap()
                    < 0
            );
            assert!(!args.contains_key(&(id.clone(), "ReductionTurns".to_owned())));
            assert_eq!(
                args[&(id.clone(), "SimpleModifierDescription".to_owned())],
                feuds::TEXT_TAG
            );
        }
        let strings: BTreeMap<_, _> = root
            .select("ModifierStrings/Row")
            .iter()
            .map(|r| (r.attr("ModifierId").to_owned(), *r))
            .collect();
        assert_eq!(
            strings.keys().collect::<BTreeSet<_>>(),
            mods.keys().collect()
        );
        for row in strings.values() {
            assert_eq!(
                (row.attr("Context"), row.attr("Text")),
                ("Sample", feuds::SAMPLE_TAG)
            );
        }
        let checked_in = read(jp("Data/JAC_Japan_Feuds.xml"));
        excludes(&checked_in, "FEUD_SHIMAZU");
        let fresh = feuds::build(&repo()).unwrap();
        assert_eq!(
            checked_in.split_once("?>").unwrap().1.trim(),
            fresh.split_once("?>").unwrap().1.trim()
        );
        let text = read(jp("Text/JAC_JapanMapScale_Text.xml"));
        contains(&text, feuds::TEXT_TAG);
        contains(&text, "1534年の因縁");
    }

    #[test]
    fn test_every_major_starts_with_limited_access_so_reasons_are_not_unknown() {
        let root = feud_xml();
        let data = relation_data();
        let modifier = feuds::ACCESS_MODIFIER;
        let rows: Vec<_> = root
            .select("Modifiers/Row")
            .into_iter()
            .filter(|r| r.attr("ModifierId") == modifier)
            .map(|r| r.attributes.clone())
            .collect();
        assert_eq!(
            rows,
            vec![attrs(&[
                ("ModifierId", modifier),
                ("ModifierType", "MODIFIER_PLAYER_ADD_DIPLO_VISIBILITY")
            ])]
        );
        let args: BTreeMap<_, _> = root
            .select("ModifierArguments/Row")
            .iter()
            .filter(|r| r.attr("ModifierId") == modifier)
            .map(|r| (r.attr("Name").to_owned(), r.attr("Value").to_owned()))
            .collect();
        assert_eq!(
            args,
            attrs(&[
                ("Amount", "1"),
                ("Source", feuds::ACCESS_SOURCE),
                ("SourceType", "DIPLO_SOURCE_ALL_NAMES")
            ])
        );
        let holders: BTreeSet<_> = root
            .select("CivilizationTraits/Row")
            .iter()
            .filter(|r| r.attr("TraitType") == feuds::ACCESS_TRAIT)
            .map(|r| r.attr("CivilizationType").to_owned())
            .collect();
        assert_eq!(
            holders,
            array(&data["factions"])
                .iter()
                .map(|f| format!("CIVILIZATION_JAC_{}", string(f)))
                .collect()
        );
        assert_eq!(holders.len(), 22);
        let trait_mods: Vec<_> = root
            .select("TraitModifiers/Row")
            .iter()
            .filter(|r| r.attr("ModifierId") == modifier)
            .map(|r| {
                (
                    r.attr("TraitType").to_owned(),
                    r.attr("ModifierId").to_owned(),
                )
            })
            .collect();
        assert_eq!(
            trait_mods,
            [(feuds::ACCESS_TRAIT.to_owned(), modifier.to_owned())]
        );
        let sources = root.select("DiplomaticVisibilitySources/Row");
        assert_eq!(
            sources
                .iter()
                .map(|r| r.attr("VisibilitySourceType"))
                .collect::<Vec<_>>(),
            [feuds::ACCESS_SOURCE]
        );
        assert_eq!(
            sources[0]
                .attributes
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>(),
            strings([
                "VisibilitySourceType",
                "Description",
                "ActionDescription",
                "GossipString",
                "FromCitizen",
                "TraitType",
            ])
        );
        assert_eq!(sources[0].attr("FromCitizen"), "true");
        assert_eq!(sources[0].attr("TraitType"), feuds::ACCESS_TRAIT);
        assert!(attr_set(&root, "Traits/Row", "TraitType").contains(feuds::ACCESS_TRAIT));
        let text = read(jp("Text/JAC_JapanMapScale_Text.xml"));
        for key in ["Description", "ActionDescription", "GossipString"] {
            for lang in LANGUAGES {
                contains(
                    &text,
                    &format!("Tag=\"{}\" Language=\"{lang}\"", sources[0].attr(key)),
                );
            }
        }
        assert!(!sources[0].attributes.contains_key("PrereqTech"));
        let lua = relations_lua();
        contains(&lua, "RecheckVisibilityOnAll()");
        assert!(re(r"for _, a in pairs\(players\) do\s+for _, b in pairs\(players\) do\s+if a ~= b then Players\[a\]:GetDiplomacy\(\):SetHasMet\(b\) end").is_match(&lua));
        let apply = lua.split_once("local function Apply()").unwrap().1;
        before(
            apply,
            "SetHasMet(b) end",
            "for _, relation in ipairs(RELATIONS)",
        );
    }

    #[test]
    fn test_ruleset_shares_ally_vision_by_default() {
        let config = read(rs("Config/JAC_Banseiki_Config.xml"));
        let pattern = re(r#"<Row [^>]*ConfigurationId="GAME_ALLIES_SHARE_VISIBILITY"[^>]*/>"#);
        let row = pattern.find(&config).unwrap().as_str();
        contains(row, "DefaultValue=\"1\"");
    }

    #[test]
    fn test_ally_pairs_start_allied_so_they_share_vision() {
        let lua = relations_lua();
        contains(&lua, "local ALLIED_KINDS = {alliance = true, marriage = true, vassal = true, friendship = true}");
        assert!(re(r"if ALLIED_KINDS\[kind\] then\s+diploA:SetHasAllied\(b, true\)\s+Game:SetProperty\(AllianceKey\(relation\[1\], relation\[2\]\), Game.GetCurrentGameTurn\(\)\)").is_match(&lua));
        excludes(&lua, ":SetPermanentAlliance(");
        before(
            lua.split_once("local function Apply()").unwrap().1,
            "SetHasAllied(b, true)",
            "RecheckVisibilityOnAll()",
        );
        let allies = allies(&relation_data());
        assert_eq!(allies.len(), 8);
        assert!(allies.contains(&("SHIMAZU".to_owned(), "RYUKYU".to_owned())));
        let steps = scenario("japan-1534-relations");
        let codes: Vec<_> = steps
            .iter()
            .filter(|s| s.get("code").is_some())
            .map(code)
            .collect();
        assert_eq!(codes.len(), 4);
        for code in &codes {
            let pattern = re(r"local allies=\{(.*?)\}; ");
            let listed = pattern.captures(code).unwrap();
            let pairs: BTreeSet<_> = re(r"\{'([A-Z_]+)','([A-Z_]+)'\}")
                .captures_iter(&listed[1])
                .map(|c| (c[1].to_owned(), c[2].to_owned()))
                .collect();
            assert_eq!(pairs, allies);
            contains(code, "'DIPLO_STATE_ALLIED'");
        }
        for code in &codes[..3] {
            contains(code, "PlayersVisibility[id[d[1]]]");
            contains(
                code,
                "type(Game:GetProperty('JAC_1534_ALLIANCE_'..x..'-'..y))~='number'",
            );
            excludes(code, "~='DIPLO_STATE_DECLARED_FRIEND'");
            contains(code, "for _,d in ipairs({{'DATE','SHIMAZU'},{'SHIMAZU','DATE'}}) do local ok,city=seesNone");
        }
    }

    #[test]
    fn test_ally_houses_carry_poundmakers_shared_vision_modifier() {
        let root = feud_xml();
        let modifier = feuds::ALLY_VISION_MODIFIER;
        let rows: Vec<_> = root
            .select("Modifiers/Row")
            .iter()
            .filter(|r| r.attr("ModifierId") == modifier)
            .map(|r| r.attributes.clone())
            .collect();
        assert_eq!(
            rows,
            vec![attrs(&[
                ("ModifierId", modifier),
                (
                    "ModifierType",
                    "MODIFIER_PLAYER_ADJUST_ALLIANCES_SHARED_VIS"
                )
            ])]
        );
        let args: BTreeMap<_, _> = root
            .select("ModifierArguments/Row")
            .iter()
            .filter(|r| r.attr("ModifierId") == modifier)
            .map(|r| (r.attr("Name").to_owned(), r.attr("Value").to_owned()))
            .collect();
        assert_eq!(args, attrs(&[("ShareVis", "true")]));
        let traits: Vec<_> = root
            .select("TraitModifiers/Row")
            .iter()
            .filter(|r| r.attr("ModifierId") == modifier)
            .map(|r| r.attr("TraitType"))
            .collect();
        assert_eq!(traits, [feuds::ALLY_VISION_TRAIT]);
        let ally_houses: BTreeSet<_> = allies(&relation_data())
            .into_iter()
            .flat_map(|(a, b)| [a, b])
            .collect();
        assert_eq!(ally_houses.len(), 12);
        let holders: BTreeSet<_> = root
            .select("CivilizationTraits/Row")
            .iter()
            .filter(|r| r.attr("TraitType") == feuds::ALLY_VISION_TRAIT)
            .map(|r| r.attr("CivilizationType").to_owned())
            .collect();
        assert_eq!(
            holders,
            ally_houses
                .into_iter()
                .map(|f| format!("CIVILIZATION_JAC_{f}"))
                .collect()
        );
        assert!(!holders.contains("CIVILIZATION_JAC_TAKEDA"));
        contains(
            &relations_lua(),
            "{alliance = true, marriage = true, vassal = true, friendship = true}",
        );
        assert_eq!(
            feuds::ALLY_KINDS,
            ["alliance", "marriage", "vassal", "friendship"]
        );
        let steps = scenario("japan-1534-relations");
        let start_code = code(steps.iter().find(|s| s.get("code").is_some()).unwrap());
        contains(start_code, "row.TraitType=='TRAIT_JAC_1534_ALLY_VISION'");
    }

    #[test]
    fn test_ally_vision_probe_reads_both_contexts_before_and_after_a_turn() {
        let steps = scenario("probe-ally-vision");
        let probes: Vec<_> = steps
            .iter()
            .filter(|s| s["action"] == "ui_lua")
            .map(|s| string(&s["state_name"]))
            .collect();
        assert_eq!(
            probes,
            ["GameCore_Tuner", "InGame", "GameCore_Tuner", "InGame"]
        );
        assert_eq!(
            actions(&steps).iter().filter(|a| *a == "autoplay").count(),
            1
        );
        for step in steps.iter().filter(|s| s["action"] == "ui_lua") {
            contains(code(step), "pcall(f)");
            excludes(code(step), "error(");
            for api in [
                "IsVisible(",
                "IsRevealed(",
                "GetVisibilityCount(",
                "HasAllied(",
                "GetAllianceType(",
                "GetAllianceLevel(",
                "GAME_ALLIES_SHARE_VISIBILITY",
            ] {
                contains(code(step), api);
            }
        }
    }

    #[test]
    fn test_alliance_type_probe_sets_in_gameplay_and_reads_in_ui() {
        let steps = scenario("probe-alliance-type");
        let probes: Vec<_> = steps.iter().filter(|s| s["action"] == "ui_lua").collect();
        assert_eq!(
            probes
                .iter()
                .map(|s| string(&s["state_name"]))
                .collect::<Vec<_>>(),
            ["GameCore_Tuner", "InGame", "InGame"]
        );
        contains(code(probes[0]), "SetHasAllied(id.SHIMAZU, true, mil.Index)");
        contains(
            code(probes[0]),
            "SetHasAllied(id.MORI, true, DB.MakeHash('ALLIANCE_MILITARY'))",
        );
        for step in &probes {
            contains(code(step), "pcall(f)");
            excludes(code(step), "error(");
        }
        for api in [
            "GetAllianceType(",
            "GetAllianceLevel(",
            "GetAllianceTurnsUntilExpiration(",
        ] {
            contains(code(probes[1]), api);
        }
    }

    #[test]
    fn test_alliance_expiry_probe_measures_raw_engine_expiry_per_renewal_variant() {
        excludes(&relations_lua(), "JAC_1534_KEEP_ALLIANCES_DISABLED");
        let steps = scenario("probe-alliance-expiry");
        assert_eq!(human_seats(&steps), ["CIVILIZATION_JAC_OUCHI"]);
        let ui: Vec<_> = steps.iter().filter(|s| s["action"] == "ui_lua").collect();
        let setup = &ui[..2];
        assert_eq!(
            setup
                .iter()
                .map(|s| string(&s["state_name"]))
                .collect::<Vec<_>>(),
            ["GameCore_Tuner", "InGame"]
        );
        contains(
            code(setup[0]),
            "Game:SetProperty('JAC_1534_KEEP_ALLIANCES_DISABLED', true)",
        );
        for api in [
            "GetAllianceTurnsUntilExpiration(",
            "GetAllianceType(",
            "GetAllianceLevel(",
            "IsVisible(",
            "DealManager.GetPlayerDeals(",
            "'AllianceEnded'",
        ] {
            contains(code(setup[1]), api);
        }
        let codes: Vec<_> = ui.iter().map(|s| code(s)).collect();
        for code in &codes {
            excludes(code, "error(");
        }
        for step in setup {
            contains(code(step), "pcall(f)");
            contains(code(step), "[JAC_ALLIANCE_EXPIRY_PROBE] ");
        }
        assert_eq!(autoplays(&steps), [26, 1, 1, 1, 1, 1, 1, 1]);
        for tag in [
            "V2 step1 R04",
            "V2 step2 R04",
            "V2b R08 same call",
            "V5a R07",
            "V5c R19",
            "V3 step1 R05",
            "V3 step2 R05",
        ] {
            assert_eq!(
                codes
                    .iter()
                    .filter(|c| c.contains(&format!("Variant('{tag}")))
                    .count(),
                1,
                "{tag}"
            );
        }
        assert_eq!(
            codes
                .iter()
                .filter(|c| c.contains("RemakeIfLapsed('ASAKURA','ROKKAKU')"))
                .count(),
            8
        );
        assert_eq!(
            codes
                .iter()
                .filter(|c| c.contains("RequestSession(a,b,'RENEW_ALLIANCE')"))
                .count(),
            1
        );
    }

    #[test]
    fn test_end_alliance_probe_tries_one_variant_per_pair_and_never_wars_and_makes_peace_on_one_turn(
    ) {
        let steps = scenario("probe-end-alliance");
        assert_eq!(human_seats(&steps), ["CIVILIZATION_JAC_OUCHI"]);
        let setup: Vec<_> = steps
            .iter()
            .filter(|s| s["action"] == "ui_lua")
            .take(2)
            .collect();
        assert_eq!(
            setup
                .iter()
                .map(|s| string(&s["state_name"]))
                .collect::<Vec<_>>(),
            ["GameCore_Tuner", "InGame"]
        );
        contains(
            code(setup[0]),
            "Game:SetProperty('JAC_1534_KEEP_ALLIANCES_DISABLED', true)",
        );
        for step in &setup {
            contains(code(step), "pcall(f)");
            contains(code(step), "'[\\r\\n]+'");
            contains(code(step), "[JAC_END_ALLIANCE_PROBE] ");
        }
        contains(code(setup[1]), "IsVisible(");
        for step in &steps {
            excludes(code(step), "error(");
        }
        let pattern = re(r"\{'(R\d\d)','(E\d)','\w+','\w+'\}");
        let pairs: Vec<_> = pattern
            .captures_iter(code(setup[0]))
            .map(|c| (c[1].to_owned(), c[2].to_owned()))
            .collect();
        let mut relations: Vec<_> = pairs.iter().map(|p| p.0.clone()).collect();
        let mut variants: Vec<_> = pairs.iter().map(|p| p.1.clone()).collect();
        relations.sort();
        variants.sort();
        assert_eq!(
            relations,
            ["R02", "R03", "R04", "R05", "R06", "R07", "R08", "R19"]
        );
        assert_eq!(
            variants,
            (0..8).map(|i| format!("E{i}")).collect::<Vec<_>>()
        );
        let actions: Vec<_> = steps
            .iter()
            .map(|s| {
                if s["action"] == "ui_lua" {
                    code(s).to_owned()
                } else {
                    string(&s["action"]).to_owned()
                }
            })
            .collect();
        let run = actions
            .iter()
            .position(|s| {
                s == "return JAC_EAP and JAC_EAP.Run() or 'JAC_EAP missing (context reloaded?)'"
            })
            .unwrap();
        let peace = actions.iter().position(|s| s == "return JAC_EAP and JAC_EAP.RunPeace() or 'JAC_EAP missing (context reloaded?)'").unwrap();
        assert!(run < peace);
        assert!(actions[run..peace].iter().any(|s| s == "autoplay"));
        contains(code(setup[0]), "P.war[tag]>=P.rel()");
        assert_eq!(autoplays(&steps), [1, 1, 1, 1]);
    }

    #[test]
    fn test_start_alliances_lapse_at_the_alliance_limit_and_are_not_re_made() {
        let lua = relations_lua();
        contains(
            &lua,
            "GameEvents.OnGameTurnStarted.Add(TrackStartAlliances)",
        );
        contains(
            &lua,
            "ExposedMembers.JAC_Japan1534Relations = {\n\tEndStartAlliance = EndStartAlliance,",
        );
        let end = body(&lua, "local function EndStartAlliance(factionA, factionB)");
        contains(
            end,
            "Game:SetProperty(AllianceKey(factionA, factionB), false)",
        );
        excludes(end, "SetHasAllied");
        excludes(end, "DeclareWarOn");
        let track = body(&lua, "local function TrackStartAlliances()");
        contains(track, "IsStartAllianceActive(factionA, factionB)");
        before(
            track,
            "IsAtWarWith(b)",
            "elseif not diplo:HasAllied(b) then",
        );
        contains(
            track,
            "Game:SetProperty(AllianceKey(factionA, factionB), false)",
        );
        excludes(track, "SetHasAllied");
        for gone in [
            "KeepStartAlliances",
            "re-made",
            "JAC_1534_ALLIANCE_LAPSES",
            "DIPLOMACY_ALLIANCE_TIME_LIMIT",
            "GetAllianceTurnsUntilExpiration(",
        ] {
            excludes(&lua, gone);
        }
        contains(
            &lua,
            "return type(Game:GetProperty(AllianceKey(factionA, factionB))) == \"number\"",
        );
        contains(&lua, "local ALLIANCE_KEY_PREFIX = \"JAC_1534_ALLIANCE_\"");
        let key = body(&lua, "local function AllianceKey(factionA, factionB)");
        contains(
            key,
            "if factionA > factionB then factionA, factionB = factionB, factionA end",
        );
        contains(
            key,
            "return ALLIANCE_KEY_PREFIX .. factionA .. \"-\" .. factionB",
        );
        let steps = scenario("japan-1534-relations");
        let actions = actions(&steps);
        let loaded = actions.iter().position(|s| s == "load").unwrap();
        assert_eq!(actions[loaded + 1..], ["ui_lua", "autoplay", "ui_lua"]);
        assert_eq!(autoplays(&steps), [1, 30]);
        let check = code(steps.last().unwrap());
        for fragment in [
            "GameInfo.GlobalParameters['DIPLOMACY_ALLIANCE_TIME_LIMIT'].Value",
            "local lapseTurn=start+limit+1; if turn~=lapseTurn then error",
            "if v~=false then error",
            "if s=='DIPLO_STATE_ALLIED' then error",
            "IsAtWarWith(id[d[2]]) then error",
        ] {
            contains(check, fragment);
        }
        for gone in ["JAC_1534_ALLIANCE_LAPSES", "does not see ally"] {
            excludes(check, gone);
        }
        assert_eq!(human_seats(&steps), ["CIVILIZATION_JAC_OUCHI"]);
    }

    #[test]
    fn test_sengoku_tree_keeps_the_printing_access_bonus() {
        let sql = read(sg("Data/JAC_Sengoku_Unlocks.sql"));
        let tech = "TECH_JAC_SENGOKU_ROADS_AND_PACKTRAINS";
        let moved = format!("UPDATE TechnologyModifiers SET TechnologyType='{tech}'");
        contains(&sql, &moved);
        contains(&sql, "PRINTING_ADD_DIPLO_VISIBILITY");
        contains(
            &sql,
            &format!("UPDATE DiplomaticVisibilitySources SET PrereqTech='{tech}'"),
        );
        before(&sql, &moved, "DELETE FROM Technologies WHERE");
    }

    #[test]
    fn test_feud_ui_scenario_rejects_unknown_reason_rows() {
        let steps = scenario("japan-1534-feud-ui");
        let codes: Vec<_> = steps.iter().map(code).collect();
        let read = codes
            .iter()
            .find(|c| c.contains("RelationshipReasonStack"))
            .unwrap();
        contains(read, "LOC_TOOLTIP_DIPLOMACY_UNKNOWN_REASON");
        contains(read, "reason row(s) read");
        let opened = codes
            .iter()
            .find(|c| c.contains("DiplomacyRibbon_OpenDiplomacyActionView"))
            .unwrap();
        contains(opened, "GetVisibilityOn(id.TAKEDA)");
    }
}

#[cfg(test)]
mod test_house_names {
    use super::support::*;
    use super::*;
    use std::path::PathBuf;

    const REPLACED_CONTEXTS: [(&str, &str); 7] = [
        ("BoostUnlockedPopup", "BoostUnlockedPopup"),
        ("EraProgressPanel", "EraProgressPanel"),
        ("EraCompletePopup", "EraCompletePopup"),
        ("EraReviewPopup", "EraReviewPopup"),
        ("DedicationPopup", "DedicationPopup"),
        ("NewTurnPanel", "NewTurnPanel_Expansion1"),
        ("HistoricMoments", "HistoricMoments"),
    ];

    fn registry() -> Vec<(String, String, String, PathBuf)> {
        let register =
            re(r#"(?m)^JAC_RegisterHouseName\("([A-Z0-9_]+)", "([A-Z0-9_]+)", "([A-Z0-9_]+)"\)$"#);
        let mut rows = Vec::new();
        for path in paths("poc", &["lua"]) {
            let name = path.file_name().unwrap().to_str().unwrap();
            if !name.starts_with("JAC_HouseNamesData_")
                || path.parent().unwrap().file_name().unwrap() != "UI"
                || path.components().count() != 4
            {
                continue;
            }
            let text = read(&path);
            let found: Vec<_> = register.captures_iter(&text).collect();
            assert_eq!(
                found.len(),
                text.lines()
                    .filter(|l| l.starts_with("JAC_RegisterHouseName"))
                    .count(),
                "unparseable registration in {}",
                path.display()
            );
            rows.extend(found.into_iter().map(|c| {
                (
                    c[1].to_owned(),
                    c[2].to_owned(),
                    c[3].to_owned(),
                    path.clone(),
                )
            }));
        }
        rows
    }

    #[test]
    fn test_tree_periods_and_their_house_name_are_gone() {
        let texts = mod_texts();
        assert!(!texts
            .keys()
            .any(|tag| tag.starts_with("LOC_JAC_SENGOKU_ERA_")));
        assert!(!registry()
            .iter()
            .any(|r| r.0 == "LOC_JAC_SENGOKU_ERA_3_NAME"));
    }

    #[test]
    fn test_taiko_survey_is_the_name_for_every_house() {
        let taiko = "LOC_CIVIC_JAC_SENGOKU_TAIKO_LAND_SURVEY_NAME";
        let texts = mod_texts();
        assert!(!registry().iter().any(|r| r.0 == taiko));
        assert!(!texts.contains_key(&format!("{taiko}_HOUSE")));
        assert_eq!(texts[taiko]["ja_JP"], "太閤検地");
        assert_eq!(texts[taiko]["en_US"], "Taikō Land Survey");
    }

    #[test]
    fn test_registry_is_well_formed() {
        let registry = registry();
        let mut seen = BTreeSet::new();
        let civilizations = read(cs("Data/JAC_CustomCivs_Gameplay.sql"));
        let houses: BTreeSet<_> = registry.iter().map(|r| r.1.clone()).collect();
        let generics: BTreeSet<_> = registry.iter().map(|r| r.0.clone()).collect();
        for (generic, house, civ, _) in &registry {
            assert!(
                seen.insert((generic.clone(), civ.clone())),
                "duplicate registration {generic} / {civ}"
            );
            assert!(
                house.starts_with(generic) && house.ends_with("_HOUSE"),
                "{house}"
            );
            contains(&civilizations, &format!("'{civ}', 'KIND_CIVILIZATION'"));
        }
        assert!(
            houses.is_disjoint(&generics),
            "a house tag must not also be a generic tag"
        );
    }

    #[test]
    fn test_every_key_has_all_languages_and_a_distinct_generic_counterpart() {
        let texts = mod_texts();
        for (generic, house, _, _) in registry() {
            for tag in [&generic, &house] {
                for language in LANGUAGES {
                    assert!(
                        texts
                            .get(tag)
                            .and_then(|v| v.get(language))
                            .is_some_and(|v| !v.trim().is_empty()),
                        "{tag} missing {language}"
                    );
                }
            }
            for language in LANGUAGES {
                assert_ne!(
                    texts[&generic][language], texts[&house][language],
                    "{generic} and {house}: {language}"
                );
            }
        }
    }

    #[test]
    fn test_house_text_never_appears_under_a_generic_tag() {
        let texts = mod_texts();
        let house_tags: BTreeSet<_> = registry().into_iter().map(|r| r.1).collect();
        let house_texts: Vec<_> = house_tags
            .iter()
            .flat_map(|tag| LANGUAGES.map(|lang| (lang, texts[tag][lang].clone())))
            .collect();
        for (tag, by_language) in &texts {
            if house_tags.contains(tag) {
                continue;
            }
            for (language, value) in by_language {
                for (house_language, house_text) in &house_texts {
                    if language == house_language {
                        assert!(
                            !value.contains(house_text),
                            "{tag} ({language}) leaks {house_text}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_database_only_references_generic_tags() {
        let house_tags: BTreeSet<_> = registry().into_iter().map(|r| r.1).collect();
        for path in direct_paths(&sg("Data"), &["sql"]) {
            let content = read(&path);
            for tag in &house_tags {
                assert!(!content.contains(tag), "{} points at {tag}", path.display());
            }
        }
    }

    #[test]
    fn test_ui_wiring() {
        let ruleset = read(rs("JAC_PoC_BanseikiRuleset.modinfo"));
        let core = read(rs("UI/JAC_HouseNames.lua"));
        for name in ["Lookup", "ToUpper", "ConvertTextKey"] {
            contains(&core, &format!("\"{name}\""));
        }
        contains(&core, "include(\"JAC_HouseNamesData_\", true)");
        let ui = direct_paths(&rs("UI"), &["lua"]);
        for prefix in ["TechAndCivicUnlockables_", "CivilopediaPage_"] {
            let hooks: Vec<_> = ui
                .iter()
                .filter(|p| p.file_name().unwrap().to_str().unwrap().starts_with(prefix))
                .collect();
            assert_eq!(hooks.len(), 1, "{prefix}");
            let name = hooks[0].file_name().unwrap().to_str().unwrap();
            contains(&ruleset, &format!("UI/{name}"));
            contains(&read(hooks[0]), "JAC_InstallHouseNames");
        }
        for (context, script) in REPLACED_CONTEXTS {
            let source = read(rs(&format!("UI/JAC_{context}.lua")));
            contains(&source, "JAC_InstallHouseNames");
            let suffix = format!("\ninclude(\"JAC_{context}_\", true)");
            let body = source
                .trim_end()
                .strip_suffix(&suffix)
                .unwrap_or(source.trim_end());
            assert!(
                body.ends_with(&format!("include(\"{script}\")")),
                "{context}"
            );
            contains(&ruleset, &format!("<LuaContext>{context}</LuaContext>"));
            contains(
                &ruleset,
                &format!("<LuaReplace>UI/JAC_{context}.lua</LuaReplace>"),
            );
        }
        assert_eq!(
            ruleset.matches("<ReplaceUIScript ").count(),
            REPLACED_CONTEXTS.len()
        );
        let import = ruleset
            .split_once("<ImportFiles id=\"JAC_Banseiki_HouseNamesFiles\"")
            .unwrap()
            .1
            .split_once("</ImportFiles>")
            .unwrap()
            .0;
        let files = ruleset.split_once("<Files>").unwrap().1;
        for path in ui {
            let name = path.file_name().unwrap().to_str().unwrap();
            let file = format!("<File>UI/{name}</File>");
            contains(files, &file);
            contains(import, &file);
        }
    }

    #[test]
    fn test_house_text_does_not_contain_a_generic_name() {
        let texts = mod_texts();
        let registry = registry();
        for language in LANGUAGES {
            let generics: BTreeSet<_> = registry
                .iter()
                .map(|r| texts[&r.0][language].clone())
                .collect();
            for (_, house, _, _) in &registry {
                for generic in &generics {
                    assert!(
                        !texts[house][language].contains(generic),
                        "{house} ({language})"
                    );
                }
            }
        }
        let sengoku = read(sg("JAC_PoC_SengokuTrees.modinfo"));
        assert_eq!(
            sengoku
                .matches("<File>UI/JAC_HouseNamesData_Sengoku.lua</File>")
                .count(),
            2
        );
        contains(&sengoku, "<ImportFiles");
    }
}

#[cfg(test)]
mod test_city_names {
    use super::support::*;
    use super::*;

    const OFFICIAL_JAPAN: [&str; 33] = [
        "Kyoto",
        "Tokyo",
        "Osaka",
        "Nagoya",
        "Fukuoka",
        "Sendai",
        "Shizuoka",
        "Okayama",
        "Takamatsu",
        "Yokohama",
        "Gifu",
        "Nagano",
        "Takasaki",
        "Otsu",
        "Aomori",
        "Fukushima",
        "Morioka",
        "Hakodate",
        "Kobe",
        "Hamamatsu",
        "Okazaki",
        "Fukui",
        "Yamagata",
        "Kagoshima",
        "Hiroshima",
        "Himeji",
        "Matsumoto",
        "Ota",
        "Numazu",
        "Sapporo",
        "Nagasaki",
        "Tottori",
        "Nagahama",
    ];

    struct State {
        sql: String,
        texts: BTreeMap<String, BTreeMap<String, String>>,
        rows: BTreeMap<String, Vec<(String, i64)>>,
        selects: Vec<String>,
        civs: BTreeSet<String>,
        source: Value,
    }
    impl State {
        fn new() -> Self {
            let sql = direct_paths(&cs("Data"), &["sql"])
                .iter()
                .map(|p| {
                    read(p)
                        .lines()
                        .map(|l| l.split("--").next().unwrap())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .collect::<Vec<_>>()
                .join("\n");
            let mut rows: BTreeMap<String, Vec<(String, i64)>> = BTreeMap::new();
            let mut selects = Vec::new();
            let insert = re(r"(?s)INSERT INTO CityNames\s*\(([^)]*)\)\s*(VALUES|SELECT)(.*?);");
            let row = re(r"\('(CIVILIZATION_[A-Z0-9_]+)',\s*'([A-Z0-9_]+)',\s*(-?\d+)\)");
            for capture in insert.captures_iter(&sql) {
                if &capture[2] == "SELECT" {
                    selects.push(capture[3].to_owned());
                }
                for r in row.captures_iter(&capture[3]) {
                    rows.entry(r[1].to_owned())
                        .or_default()
                        .push((r[2].to_owned(), r[3].parse().unwrap()));
                }
            }
            let civs = captures(
                r"\('(CIVILIZATION_JAC_[A-Z0-9_]+)', 'LOC_CIVILIZATION_JAC_[A-Z0-9_]+_NAME'",
                &sql,
                1,
            )
            .into_iter()
            .collect();
            Self {
                sql,
                texts: mod_texts(),
                rows,
                selects,
                civs,
                source: city_names::data(&repo()).unwrap(),
            }
        }
    }

    #[test]
    fn test_twenty_two_houses_have_pools() {
        let s = State::new();
        assert_eq!(s.civs.len(), 22);
        assert_eq!(
            s.source["houses"]
                .as_object()
                .unwrap()
                .keys()
                .map(|h| format!("CIVILIZATION_JAC_{h}"))
                .collect::<BTreeSet<_>>(),
            s.civs
        );
    }

    #[test]
    fn test_every_house_has_enough_own_names() {
        let s = State::new();
        for civ in &s.civs {
            let rows = s.rows.get(civ).unwrap();
            let own: Vec<_> = rows
                .iter()
                .filter(|(_, sort)| (0..city_names::SHARED_SORT_BASE).contains(sort))
                .map(|(n, _)| n)
                .collect();
            let capital: Vec<_> = rows
                .iter()
                .filter(|(_, sort)| *sort < 0)
                .map(|(n, _)| n.clone())
                .collect();
            assert!(own.len() >= 25, "{civ}");
            assert_eq!(
                capital,
                [format!(
                    "LOC_JAC_1534_CAPITAL_{}",
                    civ.strip_prefix("CIVILIZATION_JAC_").unwrap()
                )]
            );
        }
    }

    #[test]
    fn test_no_names_copied_from_other_civilizations() {
        let s = State::new();
        assert!(
            s.selects.is_empty(),
            "CityNames must not be copied from official civilizations: {:?}",
            s.selects
        );
        for civ in &s.civs {
            let foreign: Vec<_> = s.rows[civ]
                .iter()
                .filter(|(n, _)| !n.starts_with("LOC_JAC_"))
                .collect();
            assert!(foreign.is_empty(), "{civ}: {foreign:?}");
        }
    }

    #[test]
    fn test_no_duplicates_within_a_house() {
        let s = State::new();
        for civ in &s.civs {
            let mut counts = BTreeMap::new();
            for (name, _) in &s.rows[civ] {
                *counts.entry(name).or_insert(0) += 1;
            }
            let duplicates: Vec<_> = counts.into_iter().filter(|(_, c)| *c > 1).collect();
            assert!(duplicates.is_empty(), "{civ}: {duplicates:?}");
        }
    }

    #[test]
    fn test_every_name_has_three_languages() {
        let s = State::new();
        let required = strings(LANGUAGES);
        let mut missing = BTreeSet::new();
        for civ in &s.civs {
            for (name, _) in &s.rows[civ] {
                let languages = s
                    .texts
                    .get(name)
                    .map(|v| v.keys().cloned().collect::<BTreeSet<_>>())
                    .unwrap_or_default();
                if !required.is_subset(&languages) {
                    missing.insert(name);
                }
            }
        }
        assert!(missing.is_empty(), "{missing:?}");
    }

    #[test]
    fn test_houses_do_not_share_names_on_screen() {
        let s = State::new();
        let mut shown: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
        let mut tags: BTreeSet<_> = s
            .civs
            .iter()
            .flat_map(|civ| s.rows[civ].iter().map(|r| r.0.clone()))
            .collect();
        tags.extend(
            s.texts
                .keys()
                .filter(|t| t.starts_with("LOC_JAC_1534_CITY_"))
                .cloned(),
        );
        for name in tags {
            for language in LANGUAGES {
                shown
                    .entry((language.to_owned(), s.texts[&name][language].to_lowercase()))
                    .or_default()
                    .insert(name.clone());
            }
        }
        let known = strings([
            "LOC_JAC_1534_CAPITAL_ODA",
            "LOC_JAC_1534_CITY_IMAGAWA_NAGOYA",
        ]);
        let clashes: BTreeMap<_, _> = shown
            .into_iter()
            .filter(|(_, tags)| tags.len() > 1 && *tags != known)
            .collect();
        assert!(clashes.is_empty(), "{clashes:?}");
        let own: Vec<_> = s
            .civs
            .iter()
            .flat_map(|civ| s.rows[civ].iter())
            .filter(|(_, sort)| (0..city_names::SHARED_SORT_BASE).contains(sort))
            .map(|(name, _)| name)
            .collect();
        assert_eq!(
            own.len(),
            own.iter().copied().collect::<BTreeSet<_>>().len(),
            "a place is listed by two houses"
        );
    }

    #[test]
    fn test_no_clash_with_official_japan_names() {
        let s = State::new();
        let ours: BTreeSet<_> = s
            .civs
            .iter()
            .flat_map(|civ| s.rows[civ].iter())
            .filter(|(_, sort)| *sort >= 0)
            .map(|(n, _)| s.texts[n]["en_US"].clone())
            .collect();
        assert!(
            ours.is_disjoint(&strings(OFFICIAL_JAPAN)),
            "{:?}",
            ours.intersection(&strings(OFFICIAL_JAPAN))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_random_depth_is_set_for_all_houses() {
        let s = State::new();
        let pattern = re(
            r"UPDATE Civilizations SET RandomCityNameDepth = (\d+) WHERE CivilizationType IN \(([^)]*)\)",
        );
        let matched = pattern
            .captures(&s.sql)
            .expect("missing RandomCityNameDepth update");
        assert_eq!(matched[1].parse::<i64>().unwrap(), city_names::RANDOM_DEPTH);
        assert_eq!(
            captures(r"'([A-Z_]+)'", &matched[2], 1)
                .into_iter()
                .collect::<BTreeSet<_>>(),
            s.civs
        );
    }

    #[test]
    fn test_city_names_load_after_both_civilization_files() {
        let modinfo = read(cs("JAC_PoC_CustomCivs.modinfo"));
        let files = captures(
            r"<File>(Data/[^<]+\.sql)</File>",
            modinfo.split_once("</UpdateDatabase>").unwrap().0,
            1,
        );
        assert_eq!(files.last().unwrap(), "Data/JAC_CityNames_Gameplay.sql");
        contains(
            modinfo.split_once("</UpdateText>").unwrap().0,
            "Text/JAC_CityNames_Text.xml",
        );
    }

    #[test]
    fn test_generated_files_are_current() {
        let source = city_names::data(&repo()).unwrap();
        assert_eq!(
            read(cs("Data/JAC_CityNames_Gameplay.sql")),
            city_names::gameplay_sql(&source).unwrap()
        );
        fn localized(root: &Xml) -> BTreeMap<(String, String), String> {
            root.all()
                .into_iter()
                .filter(|r| r.name == "Row")
                .map(|r| {
                    (
                        (r.attr("Tag").to_owned(), r.attr("Language").to_owned()),
                        r.value("Text").to_owned(),
                    )
                })
                .collect()
        }
        let fresh = Xml::parse(&city_names::text_xml(&source).unwrap());
        let on_disk = xml(cs("Text/JAC_CityNames_Text.xml"));
        assert_eq!(localized(&on_disk), localized(&fresh));
    }

    #[test]
    fn test_every_entry_cites_a_source() {
        let source = city_names::data(&repo()).unwrap();
        for entry in city_names::entries(&source).unwrap() {
            assert!(
                string(&entry["source"]).starts_with("https://ja.wikipedia.org/wiki/"),
                "{entry}"
            );
            for key in ["kana", "province", "kind"] {
                assert!(!string(&entry[key]).is_empty(), "{entry}");
            }
        }
    }

    #[test]
    fn test_romanization_matches_map_conventions() {
        for (kana, en, ru) in [
            ("ほうじょう", "Hojo", "Ходзё"),
            ("すんぷ", "Sunpu", "Сумпу"),
            ("ひらい", "Hirai", "Хираи"),
            ("びっちゅう まつやま", "Bitchu Matsuyama", "Биттю-Мацуяма"),
            ("えいへいじ", "Eiheiji", "Эйхэйдзи"),
            ("きゃん", "Kyan", "Кян"),
            ("ユウフツ", "Yufutsu", "Юфуцу"),
            ("とおとうみ", "Totomi", "Тотоми"),
        ] {
            assert_eq!(
                city_names::romanize(kana).unwrap(),
                (en.to_owned(), ru.to_owned()),
                "{kana}"
            );
        }
    }
}
