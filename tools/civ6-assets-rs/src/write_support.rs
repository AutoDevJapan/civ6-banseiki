use crate::*;
use constants::{seat_index, SCALES, SEATS};
use rusqlite::Connection;
use serde_json::json;
use std::collections::BTreeMap;
use xmltree::Element;
pub const MOD_ID: &str = "403be889-1455-4de7-967b-ad54c0ff3431";
pub const BANSEIKI_RULESET_ID: &str = "bd7a82c5-6b38-4b69-a7f5-8dc9af534249";
pub const BUNDLE0_ID: &str = "dfa0717e-4d0b-4370-b693-27fc6758571b";
pub const STATIC_PERSONA_GATE_ID: &str = "c678a216-58ab-4d40-a614-8f309d36ac86";
pub const CUSTOM_CIVS_ID: &str = "ba3a1c58-9287-4654-a22f-85f9e6841e9b";
pub const GROWTH_UI_ID: &str = "7696e937-2044-4f3c-8e13-4e27632e2710";
pub const SENGOKU_TREES_ID: &str = "26c967d0-c10d-4eeb-8aaf-c83dc7e621e7";
pub const RESOURCE_SCENARIO_KM: i32 = 15;
pub const RESOURCE_SCENARIO_TURNS: i32 = 24;
pub const RESOURCE_SCENARIO_YEAR: i32 = 1542;
pub fn config() -> Element {
    let mut root = Element::new("GameInfo");
    let (mut maps, mut sizes, mut domains, mut leaders) = (
        Element::new("Maps"),
        Element::new("MapSizes"),
        Element::new("MapDomainOverrides"),
        Element::new("MapLeaders"),
    );
    for km in SCALES {
        for settlers in [false, true] {
            let suffix = if settlers { "_Settlers" } else { "" };
            let file = format!("{{{MOD_ID}}}Maps/JAC_Japan_{km}km{suffix}.Civ6Map");
            let mode = if settlers { "SETTLERS" } else { "HISTORICAL" };
            push(
                &mut maps,
                node(
                    "Row",
                    &[
                        ("Domain", "Maps:JAC_Banseiki".into()),
                        ("File", file.clone()),
                        ("Name", format!("LOC_JAC_JAPAN_{km}_{mode}_NAME")),
                        (
                            "Description",
                            format!("LOC_JAC_JAPAN_{km}_{mode}_DESCRIPTION"),
                        ),
                        ("Image", "Map_Earth".into()),
                        ("StaticMap", "1".into()),
                        ("RequiresUniqueLeaders", "1".into()),
                        (
                            "SortIndex",
                            (100 + km + if settlers { 50 } else { 0 }).to_string(),
                        ),
                    ],
                ),
            );
            push(
                &mut sizes,
                node(
                    "Row",
                    &[
                        ("Domain", file.clone()),
                        ("MapSizeType", "MAPSIZE_HUGE".into()),
                        ("Name", "LOC_MAPSIZE_HUGE_NAME".into()),
                        ("Description", "LOC_MAPSIZE_HUGE_DESCRIPTION".into()),
                        ("MinPlayers", "22".into()),
                        ("MaxPlayers", "22".into()),
                        ("DefaultPlayers", "22".into()),
                        ("MinCityStates", "0".into()),
                        ("MaxCityStates", "0".into()),
                        ("DefaultCityStates", "0".into()),
                        ("SortIndex", "10".into()),
                    ],
                ),
            );
            push(
                &mut domains,
                node(
                    "Row",
                    &[
                        ("Map", file.clone()),
                        ("ParameterId", "MapSize".into()),
                        ("Domain", file.clone()),
                    ],
                ),
            );
            for seat in SEATS.iter() {
                push(
                    &mut leaders,
                    node(
                        "Row",
                        &[("Map", file.clone()), ("LeaderType", seat.leader.clone())],
                    ),
                );
            }
        }
    }
    for e in [maps, sizes, domains, leaders] {
        push(&mut root, e);
    }
    root
}
pub fn text() -> Result<Element> {
    let mut records = vec![];
    for km in SCALES {
        for (mode, en, ja, ru) in [
            (
                "HISTORICAL",
                "historical territory",
                "史実の領土",
                "исторические владения",
            ),
            ("SETTLERS", "from settlers", "開拓者から", "с поселенцев"),
        ] {
            for (lang, name, desc) in [
                (
                    "en_US",
                    format!("Japan {km} km — start: {en}"),
                    format!("22 fixed 1534 factions; start with {en}."),
                ),
                (
                    "ja_JP",
                    format!("日本列島 {km} km — 開始方法: {ja}"),
                    format!("1534年の22勢力。開始方法は{ja}。"),
                ),
                (
                    "ru_RU",
                    format!("Япония {km} км — начало: {ru}"),
                    format!("22 державы 1534 года; начало: {ru}."),
                ),
            ] {
                records.push(json!({"tag":format!("LOC_JAC_JAPAN_{km}_{mode}_NAME"),"lang":lang,"text":name}));
                records.push(json!({"tag":format!("LOC_JAC_JAPAN_{km}_{mode}_DESCRIPTION"),"lang":lang,"text":desc}));
            }
        }
    }
    records.extend(serde_json::from_str::<Vec<Value>>(include_str!(
        "../tests/fixtures/map-misc-text.json"
    ))?);
    Ok(write_custom_civs::text_records_xml(&records))
}
fn substitute(template: &str, values: &[(&str, String)]) -> String {
    values.iter().fold(template.to_owned(), |t, (k, v)| {
        t.replace(&format!("@{k}@"), v)
    })
}
fn table(values: impl Iterator<Item = String>) -> String {
    format!("{{{}}}", values.collect::<Vec<_>>().join(","))
}
pub fn scenarios(maps: &Path, human_slug: &str) -> Result<Vec<(String, Value)>> {
    let manifest = read_json(maps.join("manifest.json"))?;
    let human = SEATS
        .iter()
        .find(|s| s.slug == human_slug)
        .context("unknown human faction")?;
    let ordered: Vec<_> = std::iter::once(human)
        .chain(SEATS.iter().filter(|s| s.slug != human_slug))
        .collect();
    let players: Vec<_> = ordered
        .iter()
        .enumerate()
        .map(|(i, s)| json!({"leader":s.leader,"civ":s.civ,"human":i==0}))
        .collect();
    let mut expected_counts = BTreeMap::<String, i32>::new();
    for seat in SEATS.iter() {
        expected_counts.insert(
            seat.slug.clone(),
            i32::from(constants::extra_capital(&seat.slug).is_some()),
        );
    }
    for c in arr(&start_1534::data()?["castles"]) {
        *expected_counts
            .get_mut(s(&c["owner_faction_1534_draft"]))
            .context("castle owner")? += 1;
    }
    let mut results = vec![];
    for item in arr(&manifest) {
        let km = n(&item["km_per_tile"]);
        let counts = generate::counts(arr(&item["cities"]).iter().map(|c| s(&c["owner"])));
        anyhow::ensure!(
            counts == expected_counts,
            "{km} km city inventory differs from research"
        );
        let starts: BTreeMap<_, _> = arr(&item["starts"])
            .iter()
            .map(|s| (crate::s(&s["seat"]), s))
            .collect();
        let expected = table(SEATS.iter().map(|seat| {
            format!(
                "{{{},{},{}}}",
                n(&starts[seat.slug.as_str()]["col"]),
                n(&starts[seat.slug.as_str()]["row"]),
                counts[&seat.slug]
            )
        }));
        let db = Connection::open_with_flags(
            maps.join(s(&item["file"])),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let owners: BTreeMap<_, _> = prosperity::owner_rows(&db)?
            .into_iter()
            .map(|(p, o, _)| (p, o))
            .collect();
        let land = db
            .prepare("SELECT ID FROM Plots WHERE ContinentType='CONTINENT_ASIA' ORDER BY ID")?
            .query_map([], |r| r.get::<_, i32>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let source_owners = land
            .iter()
            .map(|p| char::from_u32(65 + owners[p] as u32).unwrap())
            .collect::<String>();
        let values = vec![
            ("WIDTH", n(&item["width"]).to_string()),
            ("HEIGHT", n(&item["height"]).to_string()),
            ("HUMAN_LEADER", human.leader.clone()),
            ("EXPECTED", expected),
            (
                "LEADERS",
                table(SEATS.iter().map(|s| format!("'{}'", s.leader))),
            ),
            ("CIVS", table(SEATS.iter().map(|s| format!("'{}'", s.civ)))),
            (
                "CITY_OWNERS",
                table(
                    arr(&item["cities"])
                        .iter()
                        .map(|c| format!("[{}]={}", n(&c["plot"]), seat_index(s(&c["owner"])))),
                ),
            ),
            ("SOURCE_OWNERS", source_owners),
            ("LAND", n(&item["land_tiles"]).to_string()),
            (
                "COUNTS",
                table(ordered.iter().map(|s| counts[&s.slug].to_string())),
            ),
            (
                "NAMES",
                table(ordered.iter().map(|s| format!("'{}'", s.slug))),
            ),
        ];
        let mut start = substitute(include_str!("../tests/fixtures/map-start.lua"), &values);
        if human_slug != "ODA" {
            let first = start.find("local passage=").context("passage start")?;
            let end = start.find("local humanCapital=").context("passage end")?;
            start.replace_range(
                first..end,
                "local passage='not checked in faction mapping scenario'; ",
            );
        }
        let snapshot = substitute(include_str!("../tests/fixtures/map-snapshot.lua"), &values);
        let after = format!(
            "{}{snapshot}",
            include_str!("../tests/fixtures/map-after-autoplay.lua")
        );
        let name = format!("JAC-JapanMap-{km}km-01");
        let mut steps = vec![
            json!({"action":"enable_mods","ids":[BANSEIKI_RULESET_ID,CUSTOM_CIVS_ID,GROWTH_UI_ID,SENGOKU_TREES_ID,MOD_ID],"disable_ids":[BUNDLE0_ID,STATIC_PERSONA_GATE_ID]}),
            json!({"action":"configure","build_setup":true,"ruleset":"RULESET_JAC_BANSEIKI","map_file":format!("{{{MOD_ID}}}Maps/{}",s(&item["file"])),"map_size":"MAPSIZE_HUGE","speed":"GAMESPEED_STANDARD","seed":620929+km,"city_states":0,"players":players}),
            json!({"action":"start_game","timeout":600}),
            json!({"action":"ui_lua","state_name":"InGame","code":start}),
            json!({"action":"autoplay","turns":50,"metrics":true,"timeout":7200}),
            json!({"action":"ui_lua","state_name":"InGame","code":after,"capture_as":"JAPAN_STATE"}),
            json!({"action":"save","name":name,"timeout":180}),
            json!({"action":"exit_to_menu","timeout":180}),
            json!({"action":"load","name":name,"timeout":600}),
            json!({"action":"ui_lua","state_name":"InGame","code":snapshot,"equals_capture":"JAPAN_STATE"}),
            json!({"action":"autoplay","turns":1,"metrics":true,"timeout":900}),
        ];
        if human_slug != "ODA" {
            steps.truncate(4);
        }
        let suffix = if human_slug == "ODA" {
            String::new()
        } else {
            format!("-{}", human_slug.to_lowercase())
        };
        results.push((format!("japan-map-{km}km{suffix}.json"),json!({"name":format!("JAC Japan map scale {km} km, {human_slug} human{}",if human_slug=="ODA"{", 50 turns"}else{", start mapping"}),"judge_profile":"japan_map","steps":steps})));
    }
    Ok(results)
}
pub fn resource_scenario(maps: &Path) -> Result<Value> {
    let item = prosperity::manifest_item(maps, 15)?;
    anyhow::ensure!(SEATS[0].slug == "ODA", "Oda must be slot zero");
    let mut deferred: Vec<_> = arr(&item["resource_candidates"])
        .iter()
        .filter(|r| r["start_status"] == "deferred")
        .collect();
    anyhow::ensure!(
        deferred
            .iter()
            .all(|r| r["placement_status"] == "reserved_for_later_release"),
        "15 km map lacks deferred resource sites"
    );
    deferred.sort_by_key(|r| (n(&r["reveal_year"]), s(&r["id"])));
    let sites = table(deferred.iter().map(|r| {
        format!(
            "{{'{}',{},'{}',{},'{}'}}",
            s(&r["id"]),
            n(&r["plot"]),
            s(&r["resource_id"]),
            n(&r["reveal_year"]),
            s(&r["reveal_tech"])
        )
    }));
    let mut placed: Vec<_> = arr(&item["resource_candidates"])
        .iter()
        .filter(|r| r["placement_status"] == "placed")
        .collect();
    placed.sort_by_key(|r| s(&r["id"]));
    let starts = table(placed.iter().map(|r| {
        format!(
            "{{'{}',{},'{}'}}",
            s(&r["id"]),
            n(&r["plot"]),
            s(&r["resource_id"])
        )
    }));
    let values = vec![
        ("WIDTH", n(&item["width"]).to_string()),
        ("HEIGHT", n(&item["height"]).to_string()),
        ("STARTS", starts),
        ("SITES", sites),
    ];
    let static_code = substitute(
        include_str!("../tests/fixtures/resource-static.lua"),
        &values,
    );
    let release = |year: i32| {
        substitute(
            &substitute(
                include_str!("../tests/fixtures/resource-released.lua"),
                &values,
            ),
            &[("YEAR", year.to_string())],
        )
    };
    let players: Vec<_> = SEATS
        .iter()
        .enumerate()
        .map(|(i, s)| json!({"leader":s.leader,"civ":s.civ,"human":i==0}))
        .collect();
    Ok(
        json!({"name":"JAC Japan resources: custom rows, tech gates, improvements, calendar-year site release (15 km, 1534 to 1542)","judge_profile":"generic","steps":[{"action":"enable_mods","ids":[BANSEIKI_RULESET_ID,CUSTOM_CIVS_ID,GROWTH_UI_ID,SENGOKU_TREES_ID,MOD_ID],"disable_ids":[BUNDLE0_ID,STATIC_PERSONA_GATE_ID]},{"action":"configure","build_setup":true,"ruleset":"RULESET_JAC_BANSEIKI","map_file":format!("{{{MOD_ID}}}Maps/{}",s(&item["file"])),"map_size":"MAPSIZE_HUGE","speed":"GAMESPEED_STANDARD","seed":620960,"city_states":0,"players":players},{"action":"start_game","timeout":600},{"action":"wait_for","condition":{"type":"log_line","log":"Lua.log","contains":"[JAC_RESOURCES] loaded"},"timeout":60},{"action":"ui_lua","state_name":"InGame","code":static_code},{"action":"ui_lua","state_name":"InGame","code":release(1534)},{"action":"autoplay","turns":24,"timeout":3600},{"action":"wait_for","condition":{"type":"log_line","log":"Lua.log","contains":"[JAC_RESOURCES] revealed silver-ikuno"},"timeout":30},{"action":"ui_lua","state_name":"InGame","code":release(1542)}]}),
    )
}
pub fn main_to(maps: &Path, output: &Path, scenarios_dir: &Path) -> Result<()> {
    xml_write(
        output.join("Config/JAC_JapanMapScale_Config.xml"),
        &config(),
    )?;
    xml_write(output.join("Text/JAC_JapanMapScale_Text.xml"), &text()?)?;
    for human in ["ODA", "TAKEDA"] {
        for (name, data) in scenarios(maps, human)? {
            write_json(scenarios_dir.join(name), &data)?;
        }
    }
    write_json(
        scenarios_dir.join("japan-resources.json"),
        &resource_scenario(maps)?,
    )?;
    let scenario = scenarios_dir.join("japan-prosperous-15km.json");
    if !scenario.exists() {
        fs::create_dir_all(scenarios_dir)?;
        fs::copy(
            root().join("tools/civ6-autotest/scenarios/japan-prosperous-15km.json"),
            &scenario,
        )?;
    }
    prosper_start::write_scenario(&prosper_start::report_for(maps, 15)?, &scenario)
}
