use crate::*;
use constants::{Seat, SEATS};
use serde_json::json;
use xmltree::{Element, XMLNode};
pub fn extras() -> Vec<&'static Seat> {
    SEATS
        .iter()
        .filter(|s| s.slug != "ODA" && s.slug != "TAKEDA")
        .collect()
}
fn sql_rows(out: &mut String, header: &str, rows: Vec<String>) {
    out.push_str(header);
    out.push_str("\n\n");
    out.push_str(&rows.join(",\n"));
    out.push_str(";\n\n");
}
/// Supply the engine's normal citizen-name collection for all scenario factions.
/// This adds database candidates only; it never renames a saved unit or captive.
pub fn citizen_names_sql() -> String {
    let civs = SEATS
        .iter()
        .map(|s| format!("'{}'", s.civ))
        .collect::<Vec<_>>()
        .join(",\n    ");
    format!("-- Official Japanese citizen-name candidates, including gender and era flags.\n-- Keep existing rows and their IDs; unnamed legacy captives are not rebound.\nINSERT OR IGNORE INTO CivilizationCitizenNames (CivilizationType, CitizenName, Female, Modern)\nSELECT c.CivilizationType, n.CitizenName, n.Female, n.Modern\nFROM Civilizations AS c\nJOIN CivilizationCitizenNames AS n ON n.CivilizationType='CIVILIZATION_JAPAN'\nWHERE c.CivilizationType IN (\n    {civs}\n)\nORDER BY c.CivilizationType, n.CitizenName;\n\n")
}
pub fn gameplay() -> String {
    let extra = extras();
    let mut out = String::from(
        "-- Generated 1534 factions. Oda and Takeda remain in JAC_CustomCivs_Gameplay.sql.\n\n",
    );
    let types = extra
        .iter()
        .map(|s| (s.civ.as_str(), "KIND_CIVILIZATION"))
        .chain(extra.iter().map(|s| (s.leader.as_str(), "KIND_LEADER")))
        .chain([
            ("TRAIT_JAC_1534_FACTION", "KIND_TRAIT"),
            ("TRAIT_JAC_1534_LEADER", "KIND_TRAIT"),
        ])
        .map(|(n, k)| format!("    ('{n}', '{k}')"))
        .collect();
    sql_rows(&mut out, "INSERT INTO Types (Type, Kind) VALUES", types);
    sql_rows(&mut out,"INSERT INTO Civilizations (CivilizationType, Name, Description, Adjective, StartingCivilizationLevelType, RandomCityNameDepth, Ethnicity) VALUES",extra.iter().map(|s|format!("    ('{}', 'LOC_{}_NAME', 'LOC_{}_DESCRIPTION', 'LOC_{}_ADJECTIVE', 'CIVILIZATION_LEVEL_FULL_CIV', 10, 'ETHNICITY_ASIAN')",s.civ,s.civ,s.civ,s.civ)).collect());
    sql_rows(
        &mut out,
        "INSERT INTO Leaders (LeaderType, Name, InheritFrom, SceneLayers) VALUES",
        extra
            .iter()
            .map(|s| {
                format!(
                    "    ('{}', 'LOC_{}_NAME', 'LEADER_DEFAULT', 4)",
                    s.leader, s.leader
                )
            })
            .collect(),
    );
    sql_rows(
        &mut out,
        "INSERT INTO CivilizationLeaders (CivilizationType, LeaderType, CapitalName) VALUES",
        extra
            .iter()
            .map(|s| {
                format!(
                    "    ('{}', '{}', 'LOC_JAC_1534_CAPITAL_{}')",
                    s.civ, s.leader, s.slug
                )
            })
            .collect(),
    );
    out+="INSERT INTO Traits (TraitType, Name, Description) VALUES\n\n    ('TRAIT_JAC_1534_FACTION', 'LOC_JAC_1534_FACTION_TRAIT', 'LOC_TRAIT_JAC_CLAN_POC_DESCRIPTION'),\n\n    ('TRAIT_JAC_1534_LEADER', 'LOC_JAC_1534_LEADER_TRAIT', 'LOC_TRAIT_JAC_LEADER_POC_DESCRIPTION');\n\n";
    for(header,rows)in[("INSERT INTO CivilizationTraits (CivilizationType, TraitType) VALUES",extra.iter().map(|s|format!("    ('{}', 'TRAIT_JAC_1534_FACTION')",s.civ)).collect()),("INSERT INTO LeaderTraits (LeaderType, TraitType) VALUES",extra.iter().map(|s|format!("    ('{}', 'TRAIT_JAC_1534_LEADER')",s.leader)).collect()),("INSERT INTO CityNames (CivilizationType, CityName, SortIndex) VALUES",extra.iter().map(|s|format!("    ('{}', 'LOC_JAC_1534_CAPITAL_{}', -1)",s.civ,s.slug)).collect()),("INSERT INTO LoadingInfo (LeaderType, ForegroundImage, BackgroundImage, LeaderText, PlayDawnOfManAudio) VALUES",extra.iter().map(|s|format!("    ('{}', '{}_NEUTRAL', '{}_BACKGROUND', 'LOC_JAC_1534_LOADING', 0)",s.leader,s.leader,s.leader)).collect()),("INSERT INTO DiplomacyInfo (Type, BackgroundImage) VALUES",extra.iter().map(|s|format!("    ('{}', '{}_DIPLOMACY_BACKGROUND')",s.leader,s.leader)).collect())]{sql_rows(&mut out,header,rows);}
    out.push_str(&citizen_names_sql());
    out.truncate(out.len() - 1);
    out
}
pub fn front_end() -> Element {
    let mut root = Element::new("GameInfo");
    let mut players = Element::new("Players");
    for seat in extras() {
        let (c, l) = (&seat.civ, &seat.leader);
        push(
            &mut players,
            node(
                "Row",
                &[
                    ("Domain", "Players:Expansion2_Players".into()),
                    ("CivilizationType", c.clone()),
                    ("LeaderType", l.clone()),
                    ("CivilizationName", format!("LOC_{c}_NAME")),
                    ("CivilizationIcon", format!("ICON_{c}")),
                    ("LeaderName", format!("LOC_{l}_NAME")),
                    ("LeaderIcon", format!("ICON_{l}")),
                    ("Portrait", format!("{l}_NEUTRAL")),
                    ("PortraitBackground", format!("{l}_BACKGROUND")),
                    (
                        "CivilizationAbilityName",
                        "LOC_JAC_1534_FACTION_TRAIT".into(),
                    ),
                    (
                        "CivilizationAbilityDescription",
                        "LOC_TRAIT_JAC_CLAN_POC_DESCRIPTION".into(),
                    ),
                    ("CivilizationAbilityIcon", "ICON_CIVILIZATION_JAPAN".into()),
                    ("LeaderAbilityName", "LOC_JAC_1534_LEADER_TRAIT".into()),
                    (
                        "LeaderAbilityDescription",
                        "LOC_TRAIT_JAC_LEADER_POC_DESCRIPTION".into(),
                    ),
                    ("LeaderAbilityIcon", "ICON_CIVILIZATION_JAPAN".into()),
                ],
            ),
        );
    }
    push(&mut root, players);
    root
}
pub fn icons() -> Element {
    let mut root = Element::new("GameInfo");
    let mut defs = Element::new("IconDefinitions");
    for (i, seat) in extras().iter().enumerate() {
        push(
            &mut defs,
            node(
                "Row",
                &[
                    ("Name", format!("ICON_{}", seat.leader)),
                    ("Atlas", "ICON_ATLAS_JAC_LEADERS".into()),
                    ("Index", (i + 2).to_string()),
                ],
            ),
        );
    }
    push(&mut root, defs);
    root
}
pub fn colors() -> Element {
    let mut palette = vec![];
    for shade in ["DK", "MD"] {
        for hue in [
            "RED", "ORANGE", "YELLOW", "GREEN", "AQUA", "BLUE", "PURPLE", "MAGENTA",
        ] {
            palette.push((
                format!("COLOR_STANDARD_{hue}_{shade}"),
                "COLOR_STANDARD_WHITE_LT".to_owned(),
            ));
        }
    }
    for hue in ["RED", "GREEN", "BLUE", "PURPLE"] {
        palette.push((
            format!("COLOR_STANDARD_{hue}_LT"),
            "COLOR_STANDARD_WHITE_DK".to_owned(),
        ));
    }
    palette[0].0 = "COLOR_STANDARD_WHITE_MD".into();
    let mut root = Element::new("GameInfo");
    let mut rows = Element::new("PlayerColors");
    for (seat, (p, s)) in extras().iter().zip(palette) {
        let mut row = Element::new("Row");
        for (k, v) in [
            ("Type", seat.leader.clone()),
            ("Usage", "Unique".into()),
            ("PrimaryColor", p.clone()),
            ("SecondaryColor", s.clone()),
            ("Alt1PrimaryColor", s),
            ("Alt1SecondaryColor", p.clone()),
            ("Alt2PrimaryColor", "COLOR_STANDARD_WHITE_LT".into()),
            ("Alt2SecondaryColor", p.clone()),
            ("Alt3PrimaryColor", p),
            ("Alt3SecondaryColor", "COLOR_STANDARD_WHITE_LT".into()),
        ] {
            let mut child = Element::new(k);
            child.children.push(XMLNode::Text(v));
            push(&mut row, child);
        }
        push(&mut rows, row);
    }
    push(&mut root, rows);
    root
}
pub fn text_records_xml(records: &[Value]) -> Element {
    let mut root = Element::new("GameData");
    let mut local = Element::new("LocalizedText");
    for r in records {
        let mut row = node(
            "Row",
            &[
                ("Tag", s(&r["tag"]).into()),
                ("Language", s(&r["lang"]).into()),
            ],
        );
        let mut text = Element::new("Text");
        text.children.push(XMLNode::Text(s(&r["text"]).into()));
        push(&mut row, text);
        push(&mut local, row);
    }
    push(&mut root, local);
    root
}
pub fn translated_text() -> Result<Element> {
    let source = start_1534::data()?;
    let ru: Value = serde_json::from_str(include_str!("../tests/fixtures/capital-ru.json"))?;
    let mut records = vec![];
    let mut add = |tag: String, ja: String, en: String, ru: String| {
        for (lang, text) in [("ja_JP", ja), ("en_US", en), ("ru_RU", ru)] {
            records.push(json!({"tag":tag,"lang":lang,"text":text}));
        }
    };
    add(
        "LOC_JAC_1534_FACTION_TRAIT".into(),
        "1534年の勢力".into(),
        "1534 faction".into(),
        "Держава 1534 года".into(),
    );
    add(
        "LOC_JAC_1534_LEADER_TRAIT".into(),
        "1534年の当主".into(),
        "1534 ruler".into(),
        "Правитель 1534 года".into(),
    );
    add(
        "LOC_JAC_1534_LOADING".into(),
        "天文3年5月12日の日本列島。".into(),
        "Japan, Tenbun 3, fifth month, twelfth day.".into(),
        "Япония, Тэмбун 3, пятый месяц, двенадцатый день.".into(),
    );
    for seat in SEATS.iter() {
        let slug = &seat.slug;
        let faction = arr(&source["factions"])
            .iter()
            .find(|f| f["id"] == *slug)
            .context("faction")?;
        if slug != "ODA" && slug != "TAKEDA" {
            for suffix in ["NAME", "DESCRIPTION", "ADJECTIVE"] {
                add(
                    format!("LOC_{}_{suffix}", seat.civ),
                    s(&faction["name"]).into(),
                    seat.en.clone(),
                    seat.ru.clone(),
                );
            }
            add(
                format!("LOC_{}_NAME", seat.leader),
                start_1534::head_override(slug)
                    .unwrap_or(s(&faction["head"]["name"]))
                    .into(),
                seat.head_en.clone(),
                seat.head_ru.clone(),
            );
        }
        let ja = if let Some(alias) = constants::seat_castle(slug) {
            s(&arr(&source["castles"])
                .iter()
                .find(|c| c["id"] == alias)
                .context("capital castle")?["name"])
        } else {
            constants::extra_capital(slug).context("extra capital")?
        };
        add(
            format!("LOC_JAC_1534_CAPITAL_{slug}"),
            ja.into(),
            seat.castle.clone(),
            s(&ru[slug]).into(),
        );
    }
    for (id, names) in constants::CONSTANTS["castle_names"].as_object().unwrap() {
        let c = arr(&source["castles"])
            .iter()
            .find(|c| s(&c["id"]) == id)
            .context("castle text")?;
        add(
            format!("LOC_JAC_1534_CITY_{id}"),
            s(&c["name"]).into(),
            s(&names[0]).into(),
            s(&names[1]).into(),
        );
    }
    Ok(text_records_xml(&records))
}
fn at_mut<'a>(e: &'a mut Element, path: &[&str]) -> Result<&'a mut Element> {
    if path.is_empty() {
        return Ok(e);
    }
    let child = e
        .get_mut_child(path[0])
        .with_context(|| format!("artdef missing {}", path[0]))?;
    at_mut(child, &path[1..])
}
pub fn artdefs(output: &Path) -> Result<()> {
    for (file, original, civs) in [
        ("Civilizations.artdef", "CIVILIZATION_JAC_ODA", true),
        ("Leaders.artdef", "LEADER_JAC_ODA_NOBUHIDE", false),
        ("FallbackLeaders.artdef", "LEADER_JAC_ODA_NOBUHIDE", false),
    ] {
        let path = output.join("ArtDefs").join(file);
        let mut root = xml(&path)?;
        let collection = at_mut(&mut root, &["m_RootCollections", "Element"])?;
        collection.children.retain(|n| {
            n.as_element().is_none_or(|e| {
                let name = e.get_child("m_Name").map(|e| attr(e, "text")).unwrap_or("");
                !name.starts_with("CIVILIZATION_JAC_") && !name.starts_with("LEADER_JAC_")
                    || [
                        original,
                        "CIVILIZATION_JAC_TAKEDA",
                        "LEADER_JAC_TAKEDA_NOBUTORA",
                    ]
                    .contains(&name)
            })
        });
        let template = elements(collection)
            .find(|e| {
                e.get_child("m_Name")
                    .is_some_and(|n| attr(n, "text") == original)
            })
            .context("artdef template")?
            .clone();
        for seat in extras() {
            let mut copy = template.clone();
            at_mut(&mut copy, &["m_Name"])?.attributes.insert(
                "text".into(),
                if civs {
                    seat.civ.clone()
                } else {
                    seat.leader.clone()
                },
            );
            if file == "FallbackLeaders.artdef" {
                at_mut(
                    &mut copy,
                    &[
                        "m_ChildCollections",
                        "Element",
                        "Element",
                        "m_Fields",
                        "m_Values",
                        "Element",
                        "m_EntryName",
                    ],
                )?
                .attributes
                .insert(
                    "text".into(),
                    format!(
                        "FALLBACK_NEUTRAL_JAC_{}",
                        seat.leader.strip_prefix("LEADER_JAC_").unwrap()
                    ),
                );
            }
            push(collection, copy);
        }
        xml_write(path, &root)?;
    }
    let path = output.join("ArtDefs/Cultures.artdef");
    let mut root = xml(&path)?;
    let collection = at_mut(&mut root, &["m_RootCollections", "Element"])?;
    for group in collection
        .children
        .iter_mut()
        .filter_map(XMLNode::as_mut_element)
        .filter(|element| element.name == "Element")
    {
        let values = at_mut(group, &["m_Fields", "m_Values", "Element", "m_Values"])?;
        values.children.retain(|n| {
            n.as_element().is_none_or(|e| {
                e.get_child("m_ElementName").is_none_or(|n| {
                    let name = attr(n, "text");
                    !name.starts_with("CIVILIZATION_JAC_")
                        || ["CIVILIZATION_JAC_ODA", "CIVILIZATION_JAC_TAKEDA"].contains(&name)
                })
            })
        });
        let template = elements(values)
            .find(|e| {
                e.get_child("m_ElementName")
                    .is_some_and(|n| attr(n, "text") == "CIVILIZATION_JAC_ODA")
            })
            .context("culture template")?
            .clone();
        for (i, seat) in extras().iter().enumerate() {
            let mut copy = template.clone();
            at_mut(&mut copy, &["m_ElementName"])?
                .attributes
                .insert("text".into(), seat.civ.clone());
            at_mut(&mut copy, &["m_ParamName"])?
                .attributes
                .insert("text".into(), format!("Civilizations{:03}", i + 3));
            push(values, copy);
        }
    }
    xml_write(path, &root)?;
    Ok(())
}
pub fn main_to(output: &Path) -> Result<()> {
    write(output.join("Data/JAC_Japan1534_Gameplay.sql"), gameplay())?;
    for (path, e) in [
        ("Config/JAC_Japan1534_Players.xml", front_end()),
        ("Data/JAC_Japan1534_Icons.xml", icons()),
        ("Data/JAC_Japan1534_Colors.xml", colors()),
        ("Text/JAC_Japan1534_Text.xml", translated_text()?),
    ] {
        xml_write(output.join(path), &e)?;
    }
    artdefs(output)
}
