use anyhow::{anyhow, bail, ensure, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::Repository;

/// Pure city-name generation, independent of generated MOD files.
pub mod city_names {
    use super::*;

    pub const SHARED_SORT_BASE: i64 = 1000;
    pub const RANDOM_DEPTH: i64 = 3;

    pub fn data(repo: &Repository) -> Result<Value> {
        repo.json("documents/spec/data/japan-city-name-pools.json")
    }

    fn syllables(word: &str) -> Result<Vec<(String, String)>> {
        let kana = "あいうえおかきくけこさしすせそたちつてとなにぬねのはひふへほまみむめもやゆよらりるれろわをがぎぐげござじずぜぞだぢづでどばびぶべぼぱぴぷぺぽ";
        let en = "a i u e o ka ki ku ke ko sa shi su se so ta chi tsu te to na ni nu ne no ha hi fu he ho ma mi mu me mo ya yu yo ra ri ru re ro wa o ga gi gu ge go za ji zu ze zo da ji zu de do ba bi bu be bo pa pi pu pe po";
        let ru = "а и у э о ка ки ку кэ ко са си су сэ со та ти цу тэ то на ни ну нэ но ха хи фу хэ хо ма ми му мэ мо я ю ё ра ри ру рэ ро ва о га ги гу гэ го дза дзи дзу дзэ дзо да дзи дзу дэ до ба би бу бэ бо па пи пу пэ по";
        let rows: BTreeMap<char, (&str, &str)> = kana
            .chars()
            .zip(en.split_whitespace().zip(ru.split_whitespace()))
            .collect();
        let chars: Vec<char> = word.chars().collect();
        let mut out = Vec::new();
        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];
            let next = chars.get(i + 1).copied();
            if let (Some(&(e, r)), Some(y @ ('ゃ' | 'ゅ' | 'ょ'))) = (rows.get(&ch), next) {
                let (ve, vr) = match y {
                    'ゃ' => ("a", "я"),
                    'ゅ' => ("u", "ю"),
                    'ょ' => ("o", "ё"),
                    _ => unreachable!(),
                };
                let (he, hr) = match ch {
                    'し' => ("sh".to_owned(), "с".to_owned()),
                    'ち' => ("ch".to_owned(), "т".to_owned()),
                    'じ' | 'ぢ' => ("j".to_owned(), "дз".to_owned()),
                    _ => (
                        format!("{}y", &e[..e.len() - 1]),
                        r.chars().take(r.chars().count() - 1).collect(),
                    ),
                };
                out.push((format!("{he}{ve}"), format!("{hr}{vr}")));
                i += 2;
            } else if let Some(&(e, r)) = rows.get(&ch) {
                out.push((e.to_owned(), r.to_owned()));
                i += 1;
            } else if matches!(ch, 'っ' | 'ん' | 'ー') {
                out.push((ch.to_string(), ch.to_string()));
                i += 1;
            } else {
                bail!("unsupported kana {ch:?} in {word:?}");
            }
        }
        Ok(out)
    }

    fn capitalize(text: String) -> String {
        let mut chars = text.chars();
        match chars.next() {
            Some(first) => {
                first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase()
            }
            None => String::new(),
        }
    }

    fn word(word: &str) -> Result<(String, String)> {
        let sylls = syllables(word)?;
        let mut en = String::new();
        let mut ru = String::new();
        for (i, (se, sr)) in sylls.iter().enumerate() {
            let previous = sylls[..i]
                .iter()
                .rev()
                .find(|(e, _)| !matches!(e.as_str(), "っ" | "ん" | "ー"))
                .map(|(e, _)| e.as_str())
                .unwrap_or("");
            let (ae, ar) = sylls
                .get(i + 1)
                .map(|(e, r)| (e.as_str(), r.as_str()))
                .unwrap_or(("", ""));
            if se == "ー" {
                continue;
            }
            if se == "っ" {
                if ae.starts_with("ch") {
                    en.push('t');
                } else if let Some(c) = ae.chars().next() {
                    en.push(c);
                }
                if let Some(c) = ar.chars().next() {
                    ru.push(c);
                }
                continue;
            }
            if se == "ん" {
                let vowel_or_y = ae.chars().next().is_some_and(|c| "aiueoy".contains(c));
                en.push_str(if vowel_or_y { "n'" } else { "n" });
                ru.push_str(if vowel_or_y {
                    "нъ"
                } else if ar.chars().next().is_some_and(|c| "бпм".contains(c)) {
                    "м"
                } else {
                    "н"
                });
                continue;
            }
            let last = if i > 0 && !matches!(sylls[i - 1].0.as_str(), "っ" | "ん") {
                previous.chars().last().filter(|c| "aiueo".contains(*c))
            } else {
                None
            };
            if se == "u" && matches!(last, Some('o' | 'u')) {
                continue;
            }
            if se == "o" && last == Some('o') {
                continue;
            }
            if se == "i"
                && ((matches!(last, Some('a' | 'u' | 'o' | 'e')) && !ae.is_empty())
                    || last == Some('e'))
            {
                en.push('i');
                ru.push('й');
                continue;
            }
            en.push_str(se);
            ru.push_str(sr);
        }
        Ok((capitalize(en), capitalize(ru)))
    }

    pub fn romanize(kana: &str) -> Result<(String, String)> {
        let hira: String = kana
            .chars()
            .map(|c| {
                if ('ァ'..='ヶ').contains(&c) {
                    char::from_u32(c as u32 - 0x60).unwrap()
                } else {
                    c
                }
            })
            .collect();
        let words = hira
            .split_whitespace()
            .map(word)
            .collect::<Result<Vec<_>>>()?;
        Ok((
            words
                .iter()
                .map(|p| p.0.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            words
                .iter()
                .map(|p| p.1.as_str())
                .collect::<Vec<_>>()
                .join("-"),
        ))
    }

    fn field<'a>(entry: &'a Value, key: &str) -> Result<&'a str> {
        entry[key]
            .as_str()
            .ok_or_else(|| anyhow!("missing string {key}: {entry}"))
    }

    pub fn names(entry: &Value) -> Result<(String, String)> {
        let (en, ru) = romanize(field(entry, "kana")?)?;
        Ok((
            entry
                .get("en")
                .map(|v| v.as_str().ok_or_else(|| anyhow!("invalid en")))
                .transpose()?
                .unwrap_or(&en)
                .to_owned(),
            entry
                .get("ru")
                .map(|v| v.as_str().ok_or_else(|| anyhow!("invalid ru")))
                .transpose()?
                .unwrap_or(&ru)
                .to_owned(),
        ))
    }

    pub fn tag(entry: &Value) -> Result<String> {
        let en = names(entry)?.0.to_uppercase();
        let identifier = regex::Regex::new(r"[^A-Z0-9]+")?
            .replace_all(&en, "_")
            .trim_matches('_')
            .to_owned();
        Ok(format!("LOC_JAC_CITYNAME_{identifier}"))
    }

    fn houses(source: &Value) -> Result<&serde_json::Map<String, Value>> {
        source["houses"]
            .as_object()
            .ok_or_else(|| anyhow!("houses must be an object"))
    }

    fn shared(source: &Value) -> Result<&Vec<Value>> {
        source["shared"]
            .as_array()
            .ok_or_else(|| anyhow!("shared must be an array"))
    }

    pub fn entries(source: &Value) -> Result<Vec<&Value>> {
        let mut entries = Vec::new();
        for pool in houses(source)?.values() {
            entries.extend(
                pool.as_array()
                    .ok_or_else(|| anyhow!("house pool must be an array"))?,
            );
        }
        entries.extend(shared(source)?);
        Ok(entries)
    }

    pub fn pools(source: &Value) -> Result<indexmap::IndexMap<String, Vec<(String, i64)>>> {
        let mut seen = BTreeSet::new();
        let mut duplicates = BTreeSet::new();
        for entry in entries(source)? {
            let name = tag(entry)?;
            if !seen.insert(name.clone()) {
                duplicates.insert(name);
            }
        }
        ensure!(
            duplicates.is_empty(),
            "duplicate city names: {duplicates:?}"
        );
        let shared = shared(source)?
            .iter()
            .enumerate()
            .map(|(i, e)| Ok((tag(e)?, SHARED_SORT_BASE + i as i64)))
            .collect::<Result<Vec<_>>>()?;
        let mut out = indexmap::IndexMap::new();
        for (house, entries) in houses(source)? {
            let mut pool = entries
                .as_array()
                .ok_or_else(|| anyhow!("house pool must be an array"))?
                .iter()
                .enumerate()
                .map(|(i, e)| Ok((tag(e)?, i as i64)))
                .collect::<Result<Vec<_>>>()?;
            pool.extend(shared.iter().cloned());
            out.insert(format!("CIVILIZATION_JAC_{house}"), pool);
        }
        Ok(out)
    }

    pub fn gameplay_sql(source: &Value) -> Result<String> {
        let pools = pools(source)?;
        let quoted = pools
            .keys()
            .map(|c| format!("'{c}'"))
            .collect::<Vec<_>>()
            .join(", ");
        let rows = pools
            .iter()
            .flat_map(|(civ, pool)| {
                pool.iter()
                    .map(move |(name, sort)| format!("    ('{civ}', '{name}', {sort})"))
            })
            .collect::<Vec<_>>()
            .join(",\n");
        Ok(format!(
            "-- Generated by tools/japan_map_scale/write_city_names.py from documents/spec/data/japan-city-name-pools.json.\n\
             -- Each house has its own historical names; a shared pool of province names comes last.\n\
             -- The capital row (SortIndex -1) lives next to each civilization's definition.\n\
             UPDATE Civilizations SET RandomCityNameDepth = {RANDOM_DEPTH} WHERE CivilizationType IN ({quoted});\n\n\
             INSERT INTO CityNames (CivilizationType, CityName, SortIndex) VALUES\n{rows};\n"
        ))
    }

    pub fn text_xml(source: &Value) -> Result<String> {
        let mut out =
            String::from("<?xml version='1.0' encoding='utf-8'?>\n<GameData>\n\t<LocalizedText>");
        for entry in entries(source)? {
            let (en, ru) = names(entry)?;
            let tag = tag(entry)?;
            for (language, value) in [
                ("ja_JP", field(entry, "ja")?),
                ("en_US", &en),
                ("ru_RU", &ru),
            ] {
                out.push_str(&format!(
                    "\n\t\t<Row Tag=\"{tag}\" Language=\"{language}\">\n\t\t\t<Text>{}</Text>\n\t\t</Row>",
                    crate::xml_escape(value)
                ));
            }
        }
        out.push_str("\n\t</LocalizedText>\n</GameData>");
        Ok(out)
    }
}

/// Source inventory and pure image-layout helpers; no generation service is invoked.
pub mod art {
    use super::*;
    use image::{imageops, RgbaImage};

    pub const LEADER_ORDER: [&str; 22] = [
        "ODA_NOBUHIDE",
        "TAKEDA_NOBUTORA",
        "DATE_TANEMUNE",
        "ASHINA_MORIKIYO",
        "HOJO_UJITSUNA",
        "YAMANOUCHI_UESUGI_NORIMASA",
        "NAGAO_TAMEKAGE",
        "IMAGAWA_UJITERU",
        "MATSUDAIRA_KIYOYASU",
        "ASAKURA_TAKAKAGE",
        "ASHIKAGA_YOSHIHARU",
        "MIYOSHI_NAGAYOSHI",
        "ROKKAKU_SADAYORI",
        "HONGANJI_SHONYO",
        "OUCHI_YOSHITAKA",
        "AMAGO_TSUNEHISA",
        "MORI_MOTONARI",
        "CHOSOKABE_KUNICHIKA",
        "OTOMO_YOSHIAKI",
        "SHIMAZU_TAKAHISA",
        "RYUKYU_SHO_SEI",
        "AINU_TARIKONA",
    ];

    // Extracted source tuples from generate_portraits.py.
    pub const SUBJECTS: [(&str, &str, &str, &str, &str); 22] = [
        ("ODA_NOBUHIDE", "織田信秀", "about 23", "young Owari daimyo's dark silk kosode, hakama and eboshi", "an Owari timber castle residence with Muromachi paper screens"),
        ("TAKEDA_NOBUTORA", "武田信虎", "about 40, not the elderly appearance in his 1574 Daisenji portrait", "Kai daimyo's dark red and indigo silk kosode and eboshi", "a Kai daimyo residence with timber, tatami and mountain light"),
        ("DATE_TANEMUNE", "伊達稙宗", "about 46", "formal muted indigo silk kosode and eboshi", "an Oshu timber residence with distant northern hills"),
        ("ASHINA_MORIKIYO", "蘆名盛舜", "about 44", "deep green silk kosode, restrained warrior noble dress", "an Aizu Kurokawa residence with forested mountains"),
        ("HOJO_UJITSUNA", "北条氏綱", "about 47", "dark blue formal kosode and eboshi", "an Odawara reception room with a glimpse of the Sagami coast"),
        ("YAMANOUCHI_UESUGI_NORIMASA", "上杉憲政", "a boy around 11 if the disputed 1523 birth is accepted", "age-appropriate noble boy's Muromachi robes", "a Hirai residence in Kozuke, restrained furnishings"),
        ("NAGAO_TAMEKAGE", "長尾為景", "a mature older adult; birth year uncertain", "weathered indigo silk kosode and eboshi", "a Kasugayama timber residence with Echigo mountain light"),
        ("IMAGAWA_UJITERU", "今川氏輝", "about 21", "refined Suruga court-influenced silk robes", "an Imagawa residence at Sunpu, elegant painted screens"),
        ("MATSUDAIRA_KIYOYASU", "松平清康", "about 23", "practical Mikawa warrior noble silk kosode and hakama", "an Okazaki timber reception room"),
        ("ASAKURA_TAKAKAGE", "朝倉孝景", "about 41; the 1493-born daimyo, not his 15th-century namesake", "cultured Echizen noble silk kosode", "an Ichijodani residence with refined Muromachi screens"),
        ("ASHIKAGA_YOSHIHARU", "足利義晴", "about 23", "Muromachi shogun's formal court robes and eboshi", "a modest shogunal reception room with Kyoto-style screens"),
        ("MIYOSHI_NAGAYOSHI", "三好長慶", "a boy about 12", "age-appropriate Awa noble boy's kosode", "an Awa timber residence, restrained courtly furnishings"),
        ("ROKKAKU_SADAYORI", "六角定頼", "about 39", "formal Omi daimyo's silk kosode and eboshi", "a Kannonji hilltop residence overlooking Omi"),
        ("HONGANJI_SHONYO", "証如", "about 18", "historically plausible Jodo Shinshu head priest's dark robes, shaved head", "an early Ishiyama temple reception hall, wooden beams and tatami"),
        ("OUCHI_YOSHITAKA", "大内義隆", "about 27", "refined Yamaguchi court-influenced silk robes", "a cultured Ouchi residence in Yamaguchi with painted screens"),
        ("AMAGO_TSUNEHISA", "尼子経久", "about 76", "elderly Izumo daimyo's restrained silk kosode and eboshi", "a Gassan Toda hill fortress reception room"),
        ("MORI_MOTONARI", "毛利元就", "about 37", "modest Aki warrior noble's indigo kosode", "a Yoshida Koriyama timber residence amid Aki hills"),
        ("CHOSOKABE_KUNICHIKA", "長宗我部国親", "about 30", "practical Tosa daimyo's dark silk kosode", "an Oko timber residence with Tosa mountain light"),
        ("OTOMO_YOSHIAKI", "大友義鑑", "about 32", "Bungo noble's formal dark teal silk kosode", "a Funai residence with a glimpse of Bungo coastal light"),
        ("SHIMAZU_TAKAHISA", "島津貴久", "about 20; use Takahisa as the game's chosen representative", "young Satsuma noble's formal kosode and hakama", "an early Satsuma timber residence, without later Kagoshima Castle"),
        ("RYUKYU_SHO_SEI", "尚清王", "about 37", "16th-century Ryukyuan royal robes and court headwear, distinct from Japanese samurai attire", "an unoccupied Shuri royal audience room with Ryukyuan architecture"),
        ("AINU_TARIKONA", "タリコナ", "mature adult; birth year and precise appearance unknown", "plausible 16th-century Ainu woven attus robe and simple headband; no later tourist costume", "an unoccupied timber and thatch Ainu communal interior near the Saru River"),
    ];

    pub fn filename(key: &str, kind: &str) -> String {
        format!("{}_{kind}_codex.png", key.to_lowercase())
    }

    pub fn leaders() -> BTreeMap<String, BTreeMap<String, String>> {
        LEADER_ORDER
            .into_iter()
            .map(|key| {
                let assets = ["portrait", "figure", "background"]
                    .into_iter()
                    .map(|kind| {
                        (
                            kind.to_owned(),
                            format!("{}/{}", key.to_lowercase(), filename(key, kind)),
                        )
                    })
                    .collect();
                (key.to_owned(), assets)
            })
            .collect()
    }

    pub fn prompt(key: &str) -> Result<String> {
        let &(_, name, age, clothing, scene) = SUBJECTS
            .iter()
            .find(|s| s.0 == key)
            .ok_or_else(|| anyhow!("unknown portrait subject {key}"))?;
        let portrait = filename(key, "portrait");
        let figure = filename(key, "figure");
        let background = filename(key, "background");
        Ok(format!(
            r#"Use the built-in image generation tool in this official Codex CLI session to generate three distinct original images of {name} ({key}), the chosen 1534 leader. Generate each asset one at a time and copy its image_gen output into the current directory under the exact requested filename. No API keys, browser, other services, subagents, or changes to other files.

Historical reconstruction: {age}; {clothing}. Use the age and role in 1534, avoiding later-life depictions and anachronistic armor. No surviving image is assumed to establish an exact facial likeness; create a unique credible face. Keep facial features, clothes and palette consistent between portrait and figure. Painterly dimensional strategy-game style, warm directional light and textured brushwork, reminiscent of the visual finish of Civilization VI without copying official figures or pixels. No text, logos or watermarks. No family crests (kamon) or crest-like roundels anywhere: keep robes, screens and hangings plain or use only non-heraldic weave patterns.

1. {portrait}: square head-and-shoulders portrait, face centered, whole head with generous margins for circular icon crop, neutral warm background, minimum 1024px each side.
2. {figure}: tall near-full-length figure, hands and feet visible, entire silhouette with margin, uniformly vivid magenta (#FF00FF) background for later Rust chroma-key removal, minimum 1024x1536.
3. {background}: unoccupied {scene}, landscape 3:2 or wider, calm empty center for the leader overlay, no people, minimum 1536x1024.

After all three are copied, use Rust image validation or the built-in image tool to confirm files, dimensions and PNG format. If generation fails or reaches a usage limit, report the error and stop; do not substitute another service or an older placeholder."#
        ))
    }

    pub fn atlas(masters: &[RgbaImage], size: u32, per_row: Option<u32>) -> Result<RgbaImage> {
        ensure!(
            !masters.is_empty() && size > 0,
            "atlas requires images and a positive size"
        );
        let columns = per_row
            .filter(|n| *n > 0)
            .unwrap_or(u32::try_from(masters.len())?);
        let count = u32::try_from(masters.len())?;
        let width = size
            .checked_mul(columns)
            .ok_or_else(|| anyhow!("atlas width overflow"))?;
        let height = size
            .checked_mul(count.div_ceil(columns))
            .ok_or_else(|| anyhow!("atlas height overflow"))?;
        let mut out = RgbaImage::new(width, height);
        for (i, master) in masters.iter().enumerate() {
            ensure!(
                master.width() > 0 && master.height() > 0,
                "empty atlas source"
            );
            let resized = imageops::resize(master, size, size, imageops::FilterType::Lanczos3);
            let i = u32::try_from(i)?;
            imageops::overlay(
                &mut out,
                &resized,
                i64::from((i % columns) * size),
                i64::from((i / columns) * size),
            );
        }
        Ok(out)
    }
}

/// Deterministic diplomacy XML generation from the researched relation records.
pub mod feuds {
    use super::*;

    pub const PENALTY: i64 = -10;
    pub const TEXT_TAG: &str = "LOC_JAC_DIPLO_MODIFIER_1534_FEUD";
    pub const SAMPLE_TAG: &str = "LOC_TOOLTIP_SAMPLE_DIPLOMACY_ALL";
    pub const ACCESS_MODIFIER: &str = "JAC_1534_DIPLO_VISIBILITY";
    pub const ACCESS_SOURCE: &str = "SOURCE_JAC_1534_ACCESS";
    pub const ACCESS_TRAIT: &str = "TRAIT_JAC_1534_ACCESS";
    pub const ALLY_KINDS: [&str; 4] = ["alliance", "marriage", "vassal", "friendship"];
    pub const ALLY_VISION_MODIFIER: &str = "JAC_1534_ALLY_SHARED_VIS";
    pub const ALLY_VISION_TRAIT: &str = "TRAIT_JAC_1534_ALLY_VISION";

    pub fn data(repo: &Repository) -> Result<Value> {
        repo.json("documents/spec/data/japan-1534-relations.json")
    }

    pub fn date_key(value: Option<&str>, missing: i64) -> Result<[i64; 3]> {
        match value {
            None => Ok([missing * 100; 3]),
            Some(value) => {
                let parts = value
                    .split('-')
                    .map(str::parse::<i64>)
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                ensure!((1..=3).contains(&parts.len()), "invalid date {value}");
                let mut out = [missing; 3];
                out[..parts.len()].copy_from_slice(&parts);
                Ok(out)
            }
        }
    }

    pub fn applies(relation: &Value, start: &str) -> Result<bool> {
        if relation["confidence"] == "low" {
            return Ok(false);
        }
        Ok(
            date_key(relation["from"].as_str(), 0)? <= date_key(Some(start), 0)?
                && date_key(Some(start), 0)? < date_key(relation["to"].as_str(), 99)?,
        )
    }

    pub fn relation_pairs(source: &Value, kinds: &[&str]) -> Result<Vec<(String, String)>> {
        let start = source["start_date"]
            .as_str()
            .ok_or_else(|| anyhow!("missing start date"))?;
        let mut out = Vec::new();
        for relation in source["relations"]
            .as_array()
            .ok_or_else(|| anyhow!("missing relations"))?
        {
            let kind = relation["type"]
                .as_str()
                .ok_or_else(|| anyhow!("missing relation type"))?;
            if kinds.contains(&kind) && applies(relation, start)? {
                let a = relation["a"]
                    .as_str()
                    .ok_or_else(|| anyhow!("missing faction a"))?;
                let b = relation["b"]
                    .as_str()
                    .ok_or_else(|| anyhow!("missing faction b"))?;
                out.push((a.to_owned(), b.to_owned()));
            }
        }
        Ok(out)
    }

    pub fn feud_pairs(repo: &Repository) -> Result<Vec<(String, String)>> {
        relation_pairs(&data(repo)?, &["hostility"])
    }

    type Tables = indexmap::IndexMap<&'static str, Vec<Vec<(String, String)>>>;

    fn row(tables: &mut Tables, table: &'static str, attributes: &[(&str, &str)]) {
        tables.get_mut(table).unwrap().push(
            attributes
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        );
    }

    fn attribute_escape(value: &str) -> String {
        crate::xml_escape(value)
            .replace('"', "&quot;")
            .replace('\t', "&#09;")
            .replace('\n', "&#10;")
            .replace('\r', "&#13;")
    }

    pub fn build(repo: &Repository) -> Result<String> {
        let source = data(repo)?;
        let pairs = relation_pairs(&source, &["hostility"])?;
        let directed: Vec<_> = pairs
            .iter()
            .flat_map(|(a, b)| [(a.clone(), b.clone()), (b.clone(), a.clone())])
            .collect();
        let civs: BTreeSet<_> = directed.iter().map(|p| p.0.clone()).collect();
        let mut tables: Tables = [
            "Types",
            "Traits",
            "CivilizationTraits",
            "Modifiers",
            "ModifierArguments",
            "TraitModifiers",
            "RequirementSets",
            "Requirements",
            "RequirementArguments",
            "RequirementSetRequirements",
            "ModifierStrings",
            "DiplomaticVisibilitySources",
        ]
        .into_iter()
        .map(|name| (name, Vec::new()))
        .collect();
        for c in civs {
            let trait_id = format!("TRAIT_JAC_1534_FEUD_{c}");
            let civ = format!("CIVILIZATION_JAC_{c}");
            let set_id = format!("JAC_1534_PLAYER_IS_{c}");
            let req_id = format!("JAC_1534_REQ_IS_{c}");
            row(
                &mut tables,
                "Types",
                &[("Type", &trait_id), ("Kind", "KIND_TRAIT")],
            );
            row(&mut tables, "Traits", &[("TraitType", &trait_id)]);
            row(
                &mut tables,
                "CivilizationTraits",
                &[("CivilizationType", &civ), ("TraitType", &trait_id)],
            );
            row(
                &mut tables,
                "RequirementSets",
                &[
                    ("RequirementSetId", &set_id),
                    ("RequirementSetType", "REQUIREMENTSET_TEST_ALL"),
                ],
            );
            row(
                &mut tables,
                "Requirements",
                &[
                    ("RequirementId", &req_id),
                    ("RequirementType", "REQUIREMENT_PLAYER_TYPE_MATCHES"),
                ],
            );
            row(
                &mut tables,
                "RequirementArguments",
                &[
                    ("RequirementId", &req_id),
                    ("Name", "CivilizationType"),
                    ("Value", &civ),
                ],
            );
            row(
                &mut tables,
                "RequirementSetRequirements",
                &[("RequirementSetId", &set_id), ("RequirementId", &req_id)],
            );
        }
        for (a, b) in directed {
            let modifier = format!("JAC_1534_FEUD_{a}_{b}");
            let set_id = format!("JAC_1534_PLAYER_IS_{b}");
            let trait_id = format!("TRAIT_JAC_1534_FEUD_{a}");
            row(
                &mut tables,
                "Modifiers",
                &[
                    ("ModifierId", &modifier),
                    ("ModifierType", "MODIFIER_PLAYER_DIPLOMACY_SIMPLE_MODIFIER"),
                    ("SubjectRequirementSetId", &set_id),
                ],
            );
            row(
                &mut tables,
                "ModifierArguments",
                &[
                    ("ModifierId", &modifier),
                    ("Name", "InitialValue"),
                    ("Value", &PENALTY.to_string()),
                ],
            );
            row(
                &mut tables,
                "ModifierArguments",
                &[
                    ("ModifierId", &modifier),
                    ("Name", "SimpleModifierDescription"),
                    ("Value", TEXT_TAG),
                ],
            );
            row(
                &mut tables,
                "ModifierStrings",
                &[
                    ("ModifierId", &modifier),
                    ("Context", "Sample"),
                    ("Text", SAMPLE_TAG),
                ],
            );
            row(
                &mut tables,
                "TraitModifiers",
                &[("TraitType", &trait_id), ("ModifierId", &modifier)],
            );
        }
        row(
            &mut tables,
            "Types",
            &[("Type", ACCESS_TRAIT), ("Kind", "KIND_TRAIT")],
        );
        row(&mut tables, "Traits", &[("TraitType", ACCESS_TRAIT)]);
        for c in source["factions"]
            .as_array()
            .ok_or_else(|| anyhow!("missing factions"))?
        {
            let civ = format!(
                "CIVILIZATION_JAC_{}",
                c.as_str().ok_or_else(|| anyhow!("invalid faction"))?
            );
            row(
                &mut tables,
                "CivilizationTraits",
                &[("CivilizationType", &civ), ("TraitType", ACCESS_TRAIT)],
            );
        }
        row(
            &mut tables,
            "Modifiers",
            &[
                ("ModifierId", ACCESS_MODIFIER),
                ("ModifierType", "MODIFIER_PLAYER_ADD_DIPLO_VISIBILITY"),
            ],
        );
        for (name, value) in [
            ("Amount", "1"),
            ("Source", ACCESS_SOURCE),
            ("SourceType", "DIPLO_SOURCE_ALL_NAMES"),
        ] {
            row(
                &mut tables,
                "ModifierArguments",
                &[
                    ("ModifierId", ACCESS_MODIFIER),
                    ("Name", name),
                    ("Value", value),
                ],
            );
        }
        row(
            &mut tables,
            "TraitModifiers",
            &[("TraitType", ACCESS_TRAIT), ("ModifierId", ACCESS_MODIFIER)],
        );
        row(
            &mut tables,
            "DiplomaticVisibilitySources",
            &[
                ("VisibilitySourceType", ACCESS_SOURCE),
                ("Description", "LOC_JAC_VIZSOURCE_1534_ACCESS"),
                ("ActionDescription", "LOC_JAC_VIZSOURCE_ACTION_1534_ACCESS"),
                ("GossipString", "LOC_JAC_GOSSIP_SOURCE_1534_ACCESS"),
                ("FromCitizen", "true"),
                ("TraitType", ACCESS_TRAIT),
            ],
        );
        row(
            &mut tables,
            "Types",
            &[("Type", ALLY_VISION_TRAIT), ("Kind", "KIND_TRAIT")],
        );
        row(&mut tables, "Traits", &[("TraitType", ALLY_VISION_TRAIT)]);
        let allies: BTreeSet<_> = relation_pairs(&source, &ALLY_KINDS)?
            .into_iter()
            .flat_map(|(a, b)| [a, b])
            .collect();
        for c in allies {
            let civ = format!("CIVILIZATION_JAC_{c}");
            row(
                &mut tables,
                "CivilizationTraits",
                &[("CivilizationType", &civ), ("TraitType", ALLY_VISION_TRAIT)],
            );
        }
        row(
            &mut tables,
            "Modifiers",
            &[
                ("ModifierId", ALLY_VISION_MODIFIER),
                (
                    "ModifierType",
                    "MODIFIER_PLAYER_ADJUST_ALLIANCES_SHARED_VIS",
                ),
            ],
        );
        row(
            &mut tables,
            "ModifierArguments",
            &[
                ("ModifierId", ALLY_VISION_MODIFIER),
                ("Name", "ShareVis"),
                ("Value", "true"),
            ],
        );
        row(
            &mut tables,
            "TraitModifiers",
            &[
                ("TraitType", ALLY_VISION_TRAIT),
                ("ModifierId", ALLY_VISION_MODIFIER),
            ],
        );
        let mut out = String::from("<?xml version='1.0' encoding='utf-8'?>\n<GameInfo>");
        for (table, rows) in tables {
            if rows.is_empty() {
                out.push_str(&format!("\n  <{table} />"));
            } else {
                out.push_str(&format!("\n  <{table}>"));
                for attrs in rows {
                    out.push_str("\n    <Row");
                    for (key, value) in attrs {
                        out.push_str(&format!(" {key}=\"{}\"", attribute_escape(&value)));
                    }
                    out.push_str(" />");
                }
                out.push_str(&format!("\n  </{table}>"));
            }
        }
        out.push_str("\n</GameInfo>");
        Ok(out)
    }
}

// Seat order extracted from generate.py; these are source fixtures, not MOD snapshots.
pub const SEATS: [&str; 22] = [
    "ODA",
    "DATE",
    "ASHINA",
    "HOJO",
    "YAMANOUCHI_UESUGI",
    "TAKEDA",
    "NAGAO",
    "IMAGAWA",
    "MATSUDAIRA",
    "ASAKURA",
    "ASHIKAGA",
    "MIYOSHI",
    "ROKKAKU",
    "HONGANJI",
    "OUCHI",
    "AMAGO",
    "MORI",
    "CHOSOKABE",
    "OTOMO",
    "SHIMAZU",
    "RYUKYU",
    "AINU",
];
