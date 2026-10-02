#[cfg(test)]
use anyhow::anyhow;
use anyhow::{bail, ensure, Context, Result};
use regex::Regex;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::{apply_outputs, sql_quote, Outputs, Repository};

pub const MOD: &str = "poc/JAC_PoC_SengokuTrees";
pub const DESIGN: &str = "documents/research/sengoku-tech-civic-tree.md";
pub const TECH_COSTS: [i64; 12] = [40, 65, 90, 125, 160, 205, 255, 320, 400, 490, 600, 730];
pub const CIVIC_COSTS: [i64; 12] = [30, 45, 65, 90, 125, 165, 215, 275, 350, 430, 530, 650];
pub const PERIOD_YEARS: [(i64, i64); 4] = [(1534, 1542), (1543, 1567), (1568, 1581), (1582, 1600)];

pub type HouseName = (&'static str, &'static str, &'static str, [&'static str; 3]);
pub const HOUSE_NAMES: &[HouseName] = &[];

pub const RU_NAMES: [&str; 62] = [
    "Ирригационные работы",
    "Прибрежное мореходство",
    "Ковка железа",
    "Земляные укрепления",
    "Дороги и вьючные обозы",
    "Поиск рудных жил",
    "Хранение зерна",
    "Планировка горной крепости",
    "Купелирование",
    "Строительство деревянных судов",
    "Оружие и доспехи",
    "Знакомство с огнестрельным оружием",
    "Изготовление фитильных ружей",
    "Управление рудниками",
    "Укрощение рек",
    "Ряды оборонительных рвов",
    "Лоцманы и гавани",
    "Купальни и лечение",
    "Сеть снабжения",
    "Строительство военных судов",
    "Каменные стены",
    "Массовое производство ружей",
    "Пули и порох",
    "Снабжение через порты",
    "Угловые ворота",
    "Полевые укрепления",
    "Атакэбунэ",
    "Осадная артиллерия",
    "Главная башня и каменное основание",
    "Региональное снабжение",
    "Единые меры для межевания",
    "Внешние укрепления",
    "Правила дома",
    "Союзы местных землевладельцев",
    "Святилища и верующие",
    "Заставы и проезд",
    "Законы владения",
    "Сельские собрания",
    "Учёба и записи",
    "Самоуправление храмовых городов",
    "Горные паломничества",
    "Реестр воинской повинности",
    "Клятвы и союзы",
    "Местные союзы и разведка",
    "Советы портовых купцов",
    "Чайные встречи и ценности",
    "Указы для призамковых городов",
    "Посланники ко двору и сёгуну",
    "Свободные рынки",
    "Гарантии храмовых земель",
    "Посланники к южным варварам",
    "Межевание владений",
    "Организация стрелков",
    "Чайная дипломатия",
    "Союзные походы",
    "Посредничество в перемириях",
    "Тайко-кэнти (межевание земель)",
    "Грамоты с красной печатью",
    "Изъятие мечей",
    "Разделение воинов и земледельцев",
    "Управление портами",
    "Подтверждение земельных прав",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub short: String,
    pub id: String,
    pub ja: String,
    pub en: String,
    pub ru: String,
    pub prereqs: Vec<String>,
    pub column: usize,
    pub row: i64,
    pub cost: i64,
    pub era: String,
    pub unlock: String,
    pub proposed_boost: String,
}

pub fn column_era(repo: &Repository, column: usize) -> Result<String> {
    ensure!((1..=12).contains(&column), "invalid tree column: {column}");
    let (first, last) = PERIOD_YEARS[(column - 1) / 3];
    let months = (last - first + 1) * 12;
    let month = first * 12 + months * (2 * ((column - 1) % 3) as i64 + 1) / 6;
    crate::reigns::era_of_date(repo, month / 12, month % 12 + 1)
}

pub fn read_design(repo: &Repository) -> Result<Vec<Node>> {
    let text = repo.read(DESIGN)?;
    for (first, last) in PERIOD_YEARS {
        ensure!(
            text.contains(&format!("（{first}～{last}年ごろ）")),
            "the design no longer has the period {first}-{last}"
        );
    }
    for (label, expected) in [("技術費", TECH_COSTS), ("制度費", CIVIC_COSTS)] {
        let row = text
            .lines()
            .find(|line| line.starts_with(&format!("| {label} |")))
            .with_context(|| format!("missing design cost row: {label}"))?;
        let values = row
            .trim_matches(|c| c == '|' || c == ' ')
            .split('|')
            .skip(1)
            .map(|cell| cell.trim().parse::<i64>())
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ensure!(
            values.as_slice() == expected.as_slice(),
            "{label} no longer matches the design's 12 columns: {values:?}"
        );
    }
    let pattern = Regex::new(
        r"^\| (?P<short>[TC]\d\d) (?P<ja>.*?) / (?P<en>.*?)<br>`(?P<id>\w+)` \| (?P<prereqs>.*?) \| (?P<column>\d+)/(?P<row>-?\d+)・(?P<band>\w\d) \| (?P<unlock>.*?) \| (?P<boost>.*?) \|",
    )?;
    let matches: Vec<_> = text
        .lines()
        .filter_map(|line| pattern.captures(line))
        .collect();
    ensure!(
        matches.len() == 62 && RU_NAMES.len() == 62,
        "expected 62 designed nodes and Russian names, got {}",
        matches.len()
    );
    let names: BTreeMap<_, _> = matches
        .iter()
        .map(|m| (m["short"].to_owned(), m["id"].to_owned()))
        .collect();
    matches
        .iter()
        .zip(RU_NAMES)
        .map(|(m, ru)| {
            let short = m["short"].to_owned();
            let column = m["column"].parse::<usize>()?;
            ensure!(
                (1..=12).contains(&column),
                "invalid column for {short}: {column}"
            );
            let costs = if short.starts_with('T') {
                TECH_COSTS
            } else {
                CIVIC_COSTS
            };
            let prereqs = if &m["prereqs"] == "—" {
                Vec::new()
            } else {
                m["prereqs"]
                    .split(',')
                    .map(|p| {
                        names.get(p.trim()).cloned().with_context(|| {
                            format!("unknown prerequisite {} for {short}", p.trim())
                        })
                    })
                    .collect::<Result<Vec<_>>>()?
            };
            Ok(Node {
                short,
                id: m["id"].to_owned(),
                ja: m["ja"].to_owned(),
                en: m["en"].to_owned(),
                ru: ru.to_owned(),
                prereqs,
                column,
                row: m["row"].parse()?,
                cost: costs[column - 1],
                era: column_era(repo, column)?,
                unlock: m["unlock"].to_owned(),
                proposed_boost: m["boost"].to_owned(),
            })
        })
        .collect()
}

#[derive(Clone, Copy, Debug)]
pub struct Boost {
    pub short: &'static str,
    pub class: &'static str,
    pub attributes: &'static [(&'static str, &'static str)],
    pub text: [&'static str; 3],
}

pub const BOOSTS: &[Boost] = &[
    Boost {
        short: "T09",
        class: "BOOST_TRIGGER_IMPROVE_SPECIFIC_RESOURCE",
        attributes: &[
            ("ImprovementType", "IMPROVEMENT_MINE"),
            ("ResourceType", "RESOURCE_SILVER"),
        ],
        text: ["銀を採掘する。", "Mine silver.", "Добудьте серебро."],
    },
    Boost {
        short: "T10",
        class: "BOOST_TRIGGER_TRAIN_UNIT",
        attributes: &[("Unit1Type", "UNIT_GALLEY")],
        text: [
            "ガレー船を1隻建造する。",
            "Train a Galley.",
            "Постройте одну галеру.",
        ],
    },
];

pub const OFFICIAL_UNLOCKS: &[(&str, &str)] = &[
    (
        "T01",
        "IMPROVEMENT_FARM, IMPROVEMENT_PLANTATION, BUILDING_WATER_MILL",
    ),
    ("T02", "UNIT_GALLEY, IMPROVEMENT_FISHING_BOATS"),
    ("T03", "UNIT_SWORDSMAN, BUILDING_BARRACKS"),
    ("T04", "BUILDING_WALLS, DISTRICT_ENCAMPMENT"),
    ("T05", "UNIT_TRADER"),
    ("T06", "IMPROVEMENT_MINE, IMPROVEMENT_QUARRY"),
    ("T07", "BUILDING_GRANARY"),
    ("T08", "IMPROVEMENT_FORT"),
    ("T10", "UNIT_QUADRIREME"),
    ("T11", "BUILDING_ARMORY"),
    ("T13", "UNIT_MUSKETMAN"),
    ("T14", "DISTRICT_INDUSTRIAL_ZONE, BUILDING_WORKSHOP"),
    ("T15", "DISTRICT_AQUEDUCT"),
    ("T17", "DISTRICT_HARBOR, BUILDING_LIGHTHOUSE"),
    ("T18", "UNIT_MEDIC"),
    ("T20", "BUILDING_SHIPYARD"),
    ("T21", "BUILDING_CASTLE"),
    ("T26", "UNIT_CATAPULT"),
    ("T28", "UNIT_BOMBARD"),
    ("T29", "BUILDING_STAR_FORT"),
    ("C01", "POLICY_DISCIPLINE, POLICY_URBAN_PLANNING"),
    ("C03", "DISTRICT_HOLY_SITE, BUILDING_SHRINE"),
    ("C04", "POLICY_SURVEY"),
    ("C05", "GOVERNMENT_OLIGARCHY, DISTRICT_GOVERNMENT"),
    ("C06", "POLICY_ILKUM"),
    ("C07", "DISTRICT_CAMPUS, BUILDING_LIBRARY"),
    ("C08", "BUILDING_TEMPLE"),
    ("C10", "UNIT_HORSEMAN, POLICY_AGOGE"),
    ("C11", "POLICY_DIPLOMATIC_LEAGUE"),
    ("C12", "UNIT_SPY"),
    (
        "C13",
        "DISTRICT_COMMERCIAL_HUB, BUILDING_MARKET, GOVERNMENT_MERCHANT_REPUBLIC",
    ),
    ("C14", "DISTRICT_THEATER, BUILDING_AMPHITHEATER"),
    ("C16", "DISTRICT_DIPLOMATIC_QUARTER (optional Ethiopia DLC)"),
    ("C17", "POLICY_CARAVANSARIES"),
    ("C20", "POLICY_LAND_SURVEYORS"),
    ("C21", "POLICY_FEUDAL_CONTRACT"),
    ("C22", "POLICY_CHARISMATIC_LEADER"),
    ("C23", "POLICY_MANEUVER"),
    ("C24", "POLICY_MACHIAVELLIANISM"),
    ("C25", "POLICY_SERFDOM"),
    ("C26", "POLICY_TOWN_CHARTERS"),
    ("C28", "GOVERNMENT_MONARCHY, POLICY_PROFESSIONAL_ARMY"),
    ("C29", "POLICY_TRADE_CONFEDERATION"),
];

#[derive(Clone, Copy, Debug)]
pub enum Cell {
    Text(&'static str),
    Many(&'static [&'static str]),
    Null,
}
pub type Fields = &'static [(&'static str, Cell)];
pub type CascadeRemap = (&'static str, Fields, Fields, &'static str, &'static str);

macro_rules! tech {
    ($name:literal) => {
        Cell::Text(concat!("TECH_JAC_SENGOKU_", $name))
    };
}
macro_rules! civic {
    ($name:literal) => {
        Cell::Text(concat!("CIVIC_JAC_SENGOKU_", $name))
    };
}
macro_rules! fields {
    ($($name:literal => $value:expr),* $(,)?) => {
        &[$(($name, $value)),*]
    };
}

pub const CASCADE_REMAPS: &[CascadeRemap] = &[
    (
        "DiplomaticActions",
        fields!("InitiatorPrereqCivic" => Cell::Text("CIVIC_CIVIL_SERVICE")),
        fields!("InitiatorPrereqCivic" => civic!("OATHS_AND_ALLIANCES")),
        "同盟7種（通常・軍事・研究・文化・経済・宗教・更新）が行ごと消え、子の DiplomaticActions_XP1 とDiplomaticStateActions も連鎖して消える。どの家も同盟を結べない",
        "設計 C11 同盟",
    ),
    (
        "DiplomaticActions",
        fields!("TargetPrereqCivic" => Cell::Text("CIVIC_CIVIL_SERVICE")),
        fields!("TargetPrereqCivic" => civic!("OATHS_AND_ALLIANCES")),
        "同上（相手側の前提）",
        "設計 C11 同盟",
    ),
    (
        "DiplomaticActions",
        fields!("DiplomaticActionType" => Cell::Text("DIPLOACTION_OPEN_BORDERS")),
        fields!("InitiatorPrereqCivic" => civic!("PASSES_AND_PASSAGE")),
        "国境開放の取引ができない",
        "暫定 C04 関所・通行",
    ),
    (
        "DiplomaticActions",
        fields!("DiplomaticActionType" => Cell::Text("DIPLOACTION_DECLARE_WAR_OF_RETRIBUTION")),
        fields!("InitiatorPrereqCivic" => civic!("OATHS_AND_ALLIANCES")),
        "開戦事由「報復」が消える",
        "決定 C11（暫定の対応を維持。SSOT 3.2）",
    ),
    (
        "DiplomaticActions",
        fields!("DiplomaticActionType" => Cell::Many(&[
            "DIPLOACTION_JOINT_WAR",
            "DIPLOACTION_THIRD_PARTY_WAR",
            "DIPLOACTION_DECLARE_LIBERATION_WAR",
        ])),
        fields!("InitiatorPrereqCivic" => civic!("COALITION_CAMPAIGNS")),
        "共同戦線・参戦要請・開戦事由「解放」が消える",
        "決定 C23 連合出陣（暫定の対応を維持。SSOT 3.2）",
    ),
    (
        "DiplomaticActions",
        fields!("DiplomaticActionType" => Cell::Text("DIPLOACTION_RESIDENT_EMBASSY")),
        fields!("InitiatorPrereqCivic" => civic!("COURT_AND_SHOGUNATE_ENVOYS")),
        "大使館を置けない",
        "暫定 C16 使節",
    ),
    (
        "DiplomaticActions",
        fields!("DiplomaticActionType" => Cell::Text("DIPLOACTION_DIPLOMATIC_DELEGATION")),
        fields!("InitiatorObsoleteCivic" => civic!("COURT_AND_SHOGUNATE_ENVOYS")),
        "代表団が消える（公式の参照は旧式化の制度だけ）",
        "暫定 C16 使節（大使館と入れ替わり）",
    ),
    (
        "DiplomaticActions",
        fields!("DiplomaticActionType" => Cell::Text("DIPLOACTION_DECLARE_HOLY_WAR")),
        fields!("InitiatorPrereqCivic" => civic!("SHRINE_LAND_GUARANTEES")),
        "開戦事由「聖戦」が消える",
        "決定 C18 宗教対立（暫定の対応を維持。SSOT 3.2）",
    ),
    (
        "DiplomaticActions",
        fields!("DiplomaticActionType" => Cell::Many(&[
            "DIPLOACTION_DECLARE_PROTECTORATE_WAR",
            "DIPLOACTION_DECLARE_RECONQUEST_WAR",
        ])),
        fields!("InitiatorPrereqCivic" => civic!("MILITARY_SERVICE_LEDGER")),
        "開戦事由「保護」「再征服」が消える",
        "決定 C10（暫定の対応を維持。SSOT 3.2）",
    ),
    (
        "DiplomaticActions",
        fields!("DiplomaticActionType" => Cell::Many(&[
            "DIPLOACTION_DECLARE_COLONIAL_WAR",
            "DIPLOACTION_DECLARE_TERRITORIAL_WAR",
            "DIPLOACTION_DECLARE_IDEOLOGICAL_WAR",
        ])),
        fields!("InitiatorPrereqCivic" => civic!("LAND_TENURE_CONFIRMATION")),
        "近代以降の開戦事由3種が消える",
        "決定 最終C30（暫定の対応を維持。SSOT 3.2）",
    ),
    (
        "CivicModifiers",
        fields!(
            "CivicType" => Cell::Text("CIVIC_EARLY_EMPIRE"),
            "ModifierId" => Cell::Text("CIVIC_ENFORCE_BORDERS"),
        ),
        fields!("CivicType" => civic!("PASSES_AND_PASSAGE")),
        "国境が開いたままになり、他家のUnitが領内へ自由に入れる",
        "暫定 C04 関所",
    ),
    (
        "CivicModifiers",
        fields!(
            "CivicType" => Cell::Text("CIVIC_FOREIGN_TRADE"),
            "ModifierId" => Cell::Text("CIVIC_GRANT_ONE_TRADE_ROUTE"),
        ),
        fields!("CivicType" => civic!("PASSES_AND_PASSAGE")),
        "交易路枠が商業区・港まで0で、T05の交易商を作れない",
        "暫定 C04（T05と同じ時期）",
    ),
    (
        "CivicModifiers",
        fields!(
            "CivicType" => Cell::Text("CIVIC_MILITARY_TRADITION"),
            "ModifierId" => Cell::Text("CIVIC_GRANT_COMBAT_ADJACENCY_BONUS"),
        ),
        fields!("CivicType" => civic!("MILITARY_SERVICE_LEDGER")),
        "側面攻撃・援護ボーナスが無い",
        "暫定 C10",
    ),
    (
        "CivicModifiers",
        fields!(
            "CivicType" => Cell::Text("CIVIC_DIPLOMATIC_SERVICE"),
            "ModifierId" => Cell::Text("CIVIC_GRANT_SPY"),
        ),
        fields!("CivicType" => civic!("LOCAL_LEAGUES_AND_INTELLIGENCE")),
        "スパイ枠が0で、C12で解放したスパイを作れない",
        "設計 C12 諜報",
    ),
    (
        "TechnologyModifiers",
        fields!(
            "TechnologyType" => Cell::Text("TECH_MATHEMATICS"),
            "ModifierId" => Cell::Text("MATHEMATICS_ADJUST_SEA_MOVEMENT"),
        ),
        fields!("TechnologyType" => tech!("WOODEN_SHIPBUILDING")),
        "艦船の移動力+1が無い",
        "暫定 T10 和船",
    ),
    (
        "TechnologyModifiers",
        fields!(
            "TechnologyType" => Cell::Text("TECH_CARTOGRAPHY"),
            "ModifierId" => Cell::Text("CARTOGRAPHY_GRANT_OCEAN_NAVIGATION"),
        ),
        fields!("TechnologyType" => tech!("ATAKEBUNE")),
        "外洋タイルへ入れない",
        "暫定 T27 大型軍船",
    ),
    (
        "TechnologyModifiers",
        fields!(
            "TechnologyType" => Cell::Text("TECH_PRINTING"),
            "ModifierId" => Cell::Text("PRINTING_ADD_DIPLO_VISIBILITY"),
        ),
        fields!("TechnologyType" => tech!("ROADS_AND_PACKTRAINS")),
        "外交上の可視度+1が無い",
        "T05 街道と荷駄（#40 で実機確認済みの付け先に合わせる）",
    ),
    (
        "DiplomaticVisibilitySources",
        fields!("VisibilitySourceType" => Cell::Text("SOURCE_TECH")),
        fields!("PrereqTech" => tech!("ROADS_AND_PACKTRAINS")),
        "同上の可視度の出所が消える",
        "T05（上と対）",
    ),
    (
        "Improvement_BonusYieldChanges",
        fields!(
            "ImprovementType" => Cell::Text("IMPROVEMENT_MINE"),
            "PrereqTech" => Cell::Text("TECH_APPRENTICESHIP"),
        ),
        fields!("PrereqTech" => tech!("MINE_MANAGEMENT")),
        "鉱山の生産力+1が無い",
        "設計 T14 鉱山産出",
    ),
    (
        "Improvement_BonusYieldChanges",
        fields!(
            "ImprovementType" => Cell::Text("IMPROVEMENT_QUARRY"),
            "PrereqTech" => Cell::Text("TECH_GUNPOWDER"),
        ),
        fields!("PrereqTech" => tech!("KEEP_AND_STONE_BASE")),
        "採石場の生産力+1が無い",
        "暫定 T29 石垣",
    ),
    (
        "Improvement_BonusYieldChanges",
        fields!(
            "ImprovementType" => Cell::Text("IMPROVEMENT_FISHING_BOATS"),
            "PrereqTech" => Cell::Text("TECH_CARTOGRAPHY"),
        ),
        fields!("PrereqTech" => tech!("HARBOR_PILOTS")),
        "漁船のゴールド+2が無い",
        "暫定 T17 港",
    ),
    (
        "Improvement_BonusYieldChanges",
        fields!(
            "ImprovementType" => Cell::Text("IMPROVEMENT_PLANTATION"),
            "PrereqCivic" => Cell::Text("CIVIC_FEUDALISM"),
        ),
        fields!("PrereqCivic" => civic!("DOMAIN_LAND_SURVEYS")),
        "プランテーションの食料+1が無い",
        "暫定 C20（封建制の代理）",
    ),
    (
        "Improvement_ValidTerrains",
        fields!(
            "ImprovementType" => Cell::Text("IMPROVEMENT_FARM"),
            "PrereqCivic" => Cell::Text("CIVIC_CIVIL_ENGINEERING"),
        ),
        fields!("PrereqCivic" => Cell::Null, "PrereqTech" => tech!("IRRIGATION_WORKS")),
        "丘陵（平原・草原）に農場を作れない",
        "決定 T01 用水普請（最初から。SSOT 3.2）",
    ),
    (
        "Adjacency_YieldChanges",
        fields!("PrereqCivic" => Cell::Text("CIVIC_FEUDALISM")),
        fields!("PrereqCivic" => civic!("DOMAIN_LAND_SURVEYS")),
        "前提が空になり、農場の隣接食料+1が1ターン目から付く",
        "暫定 C20（封建制の代理）",
    ),
    (
        "Adjacency_YieldChanges",
        fields!("PrereqTech" => Cell::Text("TECH_REPLACEABLE_PARTS")),
        fields!("PrereqTech" => tech!("OUTER_DEFENSES")),
        "近代の隣接食料+1も1ターン目から付く（計+2）",
        "暫定 最終T32",
    ),
    (
        "Adjacency_YieldChanges",
        fields!("ObsoleteTech" => Cell::Text("TECH_REPLACEABLE_PARTS")),
        fields!("ObsoleteTech" => tech!("OUTER_DEFENSES")),
        "中世の隣接が旧式化しない（上と対）",
        "暫定 最終T32",
    ),
    (
        "Features",
        fields!("FeatureType" => Cell::Text("FEATURE_FOREST")),
        fields!(
            "RemoveTech" => tech!("ORE_PROSPECTING"),
            "AddCivic" => civic!("LAND_TENURE_CONFIRMATION"),
        ),
        "森の伐採が研究なしで可能。植林の制度が空（空の意味は要実機確認）",
        "暫定 伐採T06（採掘の代理）・植林C30",
    ),
    (
        "Features",
        fields!("FeatureType" => Cell::Text("FEATURE_JUNGLE")),
        fields!("RemoveTech" => tech!("IRON_FORGING")),
        "熱帯雨林の伐採が研究なしで可能",
        "暫定 T03（青銅器の代理）",
    ),
    (
        "Features",
        fields!("FeatureType" => Cell::Text("FEATURE_MARSH")),
        fields!("RemoveTech" => tech!("IRRIGATION_WORKS")),
        "湿原の除去が研究なしで可能",
        "暫定 T01（灌漑の代理）",
    ),
    (
        "Resources",
        fields!("ResourceType" => Cell::Many(&["RESOURCE_ANTIQUITY_SITE", "RESOURCE_SHIPWRECK"])),
        fields!("PrereqCivic" => civic!("LAND_TENURE_CONFIRMATION")),
        "遺跡・沈没船が1ターン目から見える",
        "暫定 最終C30（考古学は範囲外）",
    ),
    (
        "UnitCommands",
        fields!("CommandType" => Cell::Many(&["UNITCOMMAND_FORM_CORPS", "UNITCOMMAND_FORM_ARMY"])),
        fields!("PrereqCivic" => Cell::Null, "PrereqTech" => tech!("REGIONAL_SUPPLY")),
        "軍団・大軍団を編成できない",
        "決定 T30 広域兵站（SSOT 3.2）",
    ),
];

pub const CASCADE_DROPS: &[(&str, &str, &str)] = &[
    (
        "Boosts / CivicPrereqs / TechnologyPrereqs / CivicQuotes / TechnologyQuotes / CivicRandomCosts / TechnologyRandomCosts / Civics_XP2 / Technologies_XP2 / StartingBoostedCivics / StartingBoostedTechnologies",
        "削除した公式ノード自身の付随行",
        "なし（ノードごと消える）",
    ),
    (
        "CivicModifiers",
        "影響力トークン、観光、遺物、スパイ2人目以降、外交勝利点・好感度",
        "都市国家・観光・外交勝利は戦国パックで使わない。スパイの追加枠は要検討",
    ),
    (
        "CivicModifiers ほか",
        "総督称号の付与（公式制度14件、政府の広場とその建物、女王の図書館、偉人イレーネ・アダム・スミス、部族の村、スレイマン）",
        "SSOT 3.2 の決定で総督は戦国向けに作り直すまで無効。付け替えず、`JAC_Sengoku_Unlocks.sql` が付与する Modifier ごと削除する（監査の差分には出ない）",
    ),
    (
        "TechnologyModifiers",
        "乗船移動（横帆船・蒸気・内燃）、都市防御（鋼鉄）、観光、スパイ（コンピュータ）、GDR昇進、未来技術",
        "時代外",
    ),
    (
        "Improvement_BonusYieldChanges",
        "産業以降の改善ボーナス、建設不可の改善（キャンプ・牧草地・固有改善）",
        "時代外または Buildable=0",
    ),
    ("Improvement_Tourism / Improvement_ValidFeatures", "観光・製材所", "観光は不使用、製材所は Buildable=0"),
    (
        "Adjacency_YieldChanges",
        "公式文明の固有改善（クルガン・シャトー・モアイ・巨石人頭像・万里の長城）の時期",
        "Buildable=0 のため影響なし",
    ),
    (
        "Units / Buildings / Improvements",
        "研究ゲートが空になる公式行",
        "CanTrain=0・InternalOnly=1・Buildable=0。残した Unit は旧式化の研究だけが空になり、旧式化しない",
    ),
    ("RandomAgendasForCivic / RandomAgendaCivicTags", "ナショナリズムで AI に追加される隠しアジェンダ", "AI の性格付けのみ"),
    ("Routes_XP2", "鉄道", "時代外"),
];

fn sql_cell(cell: Cell) -> String {
    match cell {
        Cell::Text(s) => sql_quote(s),
        Cell::Null => "NULL".to_owned(),
        Cell::Many(values) => values
            .iter()
            .map(|s| sql_quote(s))
            .collect::<Vec<_>>()
            .join(", "),
    }
}

pub fn make_cascade_sql() -> String {
    let mut out = vec![
        "-- Generated by tools/generate_sengoku_trees.py (CASCADE_REMAPS). Loaded before".to_owned(),
        "-- JAC_Sengoku_Unlocks.sql deletes the official trees, so these rows keep a Sengoku gate".to_owned(),
        "-- instead of being cascade-deleted or nulled (01_GameplaySchema.sql, Expansion2_Schema.sql).".to_owned(),
        "-- See documents/poc/sengoku-tree-unlocks.md for the audit table; mappings marked 暫定 are provisional.".to_owned(),
    ];
    for &(table, where_fields, values, _, basis) in CASCADE_REMAPS {
        let conditions = where_fields
            .iter()
            .map(|&(key, value)| match value {
                Cell::Many(_) => format!("{key} IN ({})", sql_cell(value)),
                _ => format!("{key}={}", sql_cell(value)),
            })
            .collect::<Vec<_>>()
            .join(" AND ");
        let sets = values
            .iter()
            .map(|&(key, value)| format!("{key}={}", sql_cell(value)))
            .collect::<Vec<_>>()
            .join(", ");
        out.push(format!(
            "-- {basis}\nUPDATE {table} SET {sets} WHERE {conditions};"
        ));
    }
    out.join("\n") + "\n"
}

pub fn make_sql(nodes: &[Node]) -> Result<String> {
    let mut out = vec![
        "-- Generated by tools/generate_sengoku_trees.py from the research design.".to_owned(),
        "-- Costs and unlock effects are provisional. EraType is the reign in force at the column's intended research".to_owned(),
        "-- date (元号 = 時代, owner decision T; the eras are Data/JAC_Sengoku_Eras.sql).".to_owned(),
        "INSERT INTO Types (Type, Kind) VALUES".to_owned(),
        nodes
            .iter()
            .map(|n| {
                format!(
                    "  ({}, {})",
                    sql_quote(&n.id),
                    sql_quote(if n.short.starts_with('T') { "KIND_TECH" } else { "KIND_CIVIC" })
                )
            })
            .collect::<Vec<_>>()
            .join(",\n") + ";",
    ];
    for (kind, table, key, edge_table, target, source) in [
        (
            'T',
            "Technologies",
            "TechnologyType",
            "TechnologyPrereqs",
            "Technology",
            "PrereqTech",
        ),
        (
            'C',
            "Civics",
            "CivicType",
            "CivicPrereqs",
            "Civic",
            "PrereqCivic",
        ),
    ] {
        let group: Vec<_> = nodes.iter().filter(|n| n.short.starts_with(kind)).collect();
        out.push(format!(
            "INSERT INTO {table} ({key}, Name, Description, Cost, EraType, UITreeRow, AdvisorType) VALUES"
        ));
        out.push(
            group
                .iter()
                .map(|n| {
                    format!(
                        "  ({}, {}, {}, {}, {}, {}, 'ADVISOR_GENERIC')",
                        sql_quote(&n.id),
                        sql_quote(&format!("LOC_{}_NAME", n.id)),
                        sql_quote(&format!("LOC_{}_DESCRIPTION", n.id)),
                        n.cost,
                        sql_quote(&n.era),
                        n.row
                    )
                })
                .collect::<Vec<_>>()
                .join(",\n")
                + ";",
        );
        out.push(format!(
            "INSERT INTO {edge_table} ({target}, {source}) VALUES"
        ));
        out.push(
            group
                .iter()
                .flat_map(|n| {
                    n.prereqs
                        .iter()
                        .map(move |p| format!("  ({}, {})", sql_quote(&n.id), sql_quote(p)))
                })
                .collect::<Vec<_>>()
                .join(",\n")
                + ";",
        );
    }
    for boost in BOOSTS {
        let node = nodes
            .iter()
            .find(|n| n.short == boost.short)
            .with_context(|| format!("missing boost node: {}", boost.short))?;
        let mut fields = vec![
            "TechnologyType",
            "Boost",
            "TriggerDescription",
            "TriggerLongDescription",
            "BoostClass",
        ];
        fields.extend(boost.attributes.iter().map(|(key, _)| *key));
        let trigger = format!("LOC_JAC_SENGOKU_BOOST_{}", boost.short);
        let mut values = vec![
            sql_quote(&node.id),
            "40".to_owned(),
            sql_quote(&trigger),
            sql_quote(&trigger),
            sql_quote(boost.class),
        ];
        values.extend(boost.attributes.iter().map(|(_, value)| sql_quote(value)));
        out.push(format!(
            "INSERT INTO Boosts ({}) VALUES ({});",
            fields.join(", "),
            values.join(", ")
        ));
    }
    Ok(out.join("\n\n") + "\n")
}

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn text_row(tag: &str, translations: [&str; 3]) -> Vec<String> {
    ["ja_JP", "en_US", "ru_RU"]
        .into_iter()
        .zip(translations)
        .map(|(lang, text)| {
            format!(
                "    <Row Tag=\"{tag}\" Language=\"{lang}\"><Text>{}</Text></Row>",
                escape_text(text)
            )
        })
        .collect()
}

pub fn make_text(nodes: &[Node]) -> String {
    let mut out = vec![
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>".to_owned(),
        "<GameInfo>".to_owned(),
        "  <LocalizedText>".to_owned(),
    ];
    for n in nodes {
        out.extend(text_row(
            &format!("LOC_{}_NAME", n.id),
            [&n.ja, &n.en, &n.ru],
        ));
        out.extend(text_row(
            &format!("LOC_{}_DESCRIPTION", n.id),
            [
                "戦国ツリーPoC。解放先と効果量は暫定です。",
                "Sengoku tree PoC. Unlocks and effect values are provisional.",
                "Пробный образец древа Сэнгоку. Открываемые объекты и эффекты предварительны.",
            ],
        ));
    }
    for boost in BOOSTS {
        out.extend(text_row(
            &format!("LOC_JAC_SENGOKU_BOOST_{}", boost.short),
            boost.text,
        ));
    }
    for &(_, house, _, translations) in HOUSE_NAMES {
        out.extend(text_row(house, translations));
    }
    out.extend(
        text_row(
            "LOC_DISTRICT_GOVERNMENT_DESCRIPTION",
            [
                "政府の選択を記念する区域。自文明に1つだけ設置できる。この都市へのターン毎の忠誠心+8、隣接する区域へ隣接ボーナス+1。",
                "A district placed once in your empire to commemorate your government choices. Provides +8 Loyalty per turn to this city, and an increase of +1 to the adjacency bonuses being earned by any adjacent district.",
                "Район, построенный в вашей империи в память о решениях вашего правительства. Дает +8 лояльности за ход этому городу и +1 ко всем бонусам за соседство всем прилежащим районам.",
            ],
        )
        .into_iter()
        .map(|row| row.replace("<Row ", "<Replace ").replace("</Row>", "</Replace>")),
    );
    out.extend(["  </LocalizedText>".to_owned(), "</GameInfo>".to_owned()]);
    out.join("\n") + "\n"
}

pub fn make_house_names() -> String {
    let mut out = vec![
        "-- Generated by tools/generate_sengoku_trees.py (HOUSE_NAMES).  SSOT 3.2 house-specific names;".to_owned(),
        "-- the mechanism is UI/JAC_HouseNames.lua in the ruleset mod.".to_owned(),
    ];
    for &(generic, house, civ, _) in HOUSE_NAMES {
        out.push(format!(
            "JAC_RegisterHouseName(\"{generic}\", \"{house}\", \"{civ}\")"
        ));
    }
    out.join("\n") + "\n"
}

pub fn make_icons(nodes: &[Node]) -> String {
    let mut out = vec![
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>".to_owned(),
        "<GameInfo>".to_owned(),
        "  <IconDefinitions>".to_owned(),
    ];
    for (kind, atlas) in [('T', "ICON_ATLAS_TECH"), ('C', "ICON_ATLAS_CIVICS")] {
        for (index, node) in nodes
            .iter()
            .filter(|n| n.short.starts_with(kind))
            .enumerate()
        {
            let icon = format!("ICON_{}", node.id);
            out.push(format!(
                "    <Row Name=\"{icon}\" Index=\"{index}\" Atlas=\"{atlas}\"/>"
            ));
            out.push(format!(
                "    <Row Name=\"{icon}_FOW\" Index=\"{index}\" Atlas=\"{atlas}_FOW\"/>"
            ));
        }
    }
    out.extend(["  </IconDefinitions>".to_owned(), "</GameInfo>".to_owned()]);
    out.join("\n") + "\n"
}

fn documentation_cell(fields: Fields) -> String {
    fields
        .iter()
        .map(|&(key, value)| {
            let value = match value {
                Cell::Text(text) => format!("`{text}`"),
                Cell::Many(texts) => texts
                    .iter()
                    .map(|text| format!("`{text}`"))
                    .collect::<Vec<_>>()
                    .join("、"),
                Cell::Null => "NULL".to_owned(),
            };
            format!("`{key}`={value}")
        })
        .collect::<Vec<_>>()
        .join("<br>")
}

const CASCADE_DOC_HEADER: &str = r#"## 公式ツリー削除に伴う連鎖削除の監査

`JAC_Sengoku_Unlocks.sql` の最後で公式の技術・社会制度を削除すると、外部キーで参照する公式行が消えるか、前提が空になる。公式スキーマで `Civics(CivicType)`・`Technologies(TechnologyType)` を参照する外部キーは68件（XP1スキーマは0件）。

- `ON DELETE CASCADE`（行ごと消える）：`Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql` の Boosts:391,399-401、Civics_XP2:630、CivicModifiers:636、CivicPrereqs:643-644、CivicQuotes:651、CivicRandomCosts:657、DiplomaticActions:923-927、DiplomaticVisibilitySources:1010、Improvement_BonusYieldChanges:1773-1774、Improvement_Tourism:1800-1801、Improvement_ValidFeatures:1834-1835、Improvement_ValidTerrains:1853-1854、Policies.PrereqTech:2170、StartingBoostedCivics:2575、StartingBoostedTechnologies:2582、StartingCivics:2601、Technologies_XP2:2680、TechnologyModifiers:2686、TechnologyPrereqs:2692-2693、TechnologyQuotes:2700、TechnologyRandomCosts:2706、UnitCommands:2992-2993、UnitOperations:3028-3029。`DLC/Expansion2/Data/Expansion2_Schema.sql` の RandomAgendaCivicTags:688、RandomAgendasForCivic:703、Resource_Consumption:882、Routes_XP2:899
- `ON DELETE SET NULL`（前提が空＝最初から解放）：01_GameplaySchema.sql の Adjacency_YieldChanges:40-43、Features:1259-1260、Resources:2342-2343、Resource_Harvests:2367
- `ON DELETE SET DEFAULT`（既定値 NULL＝同上）：01_GameplaySchema.sql の Buildings:473,475、Districts:1056-1057、Governments:1467、Improvements:1752-1753、Policies.PrereqCivic:2168、Projects:2201-2202、Units:2903-2917

Districts・Governments・Policies・Projects・StartingCivics・Resources の研究ゲートと Units・Buildings・Improvements の残す行は従来どおり `JAC_Sengoku_Unlocks.sql` と `JAC_Sengoku_Resources.sql` で付け替える。それ以外で実害がある行は生成ファイル `Data/JAC_Sengoku_Cascade.sql`（`CASCADE_REMAPS`）が削除前に戦国ノードへ付け替える。監査は `cargo run --locked --manifest-path tools/Cargo.toml -p civ6-data-rs --bin sengoku-cascade-audit`（公式 Base＋GS のデータで戦国SQLを外部キー有効のまま実行し、公式ツリーの削除だけによる差分を出す）、固定は `tools/civ6-data-rs/src/cascade.rs`、関連する静的検査は `tools/civ6-data-rs/src/data_tests.rs`。他の DLC と他の MOD は監査の対象外。「暫定」は設計案に記載がなく、同じ時代枠（公式 Era ID と戦国の期の対応）と内容の近さで選んだもの。「決定」はオーナーが決めたもの（SSOT 3.2）。GS は防衛協定と研究協定を `Expansion2_RemoveData.xml` で削除済みのため対象外。

### 付け替えた行

| 表 | 対象の公式行 | 付け替えない場合の影響 | 付け替え先 | 根拠 |
|---|---|---|---|---|
"#;

const CASCADE_DOC_FOOTER: &str = r#"総督を無効にしても公式 UI は壊れない見込み：ランチバーの総督ボタンは `CAPABILITY_GOVERNORS` で出たままだが、通知の印は `CanAppoint()`・`CanPromote()` が真のときだけ出る（`DLC/Expansion1/UI/Replacements/LaunchBar_Expansion1.lua:129-136`）。総督画面は称号0でも `GetGovernorPoints()`・`CanAppoint()` で候補を任命不可として並べるだけで（`DLC/Expansion2/UI/Additions/GovernorPanel.lua:55-57`）、公式でも最初の制度を得るまでは称号0で始まる。政府の広場の説明文からは称号の一文を外した（生成ファイル `Text/JAC_Sengoku_Text.xml`）。偉人イレーネ・アダム・スミスの説明は付与の Modifier ごと消えるため、称号の行が出ない見込み。

2026-09-29 の自動実機結果（sengoku-cascade-deletes-02）：同盟7件が C11 で残ること、総督称号の付与元が無く称号0、丘陵の農場が T01、軍団と大軍団が T30 で解放されることを `GameInfo` で確かめた。同盟の提案可否は、C11 が無いときの失敗理由の一覧が空で C11 の名前を照合できず失敗した。そのため友好宣言を済ませた上で、C11 を両者なし→開始側だけ→両者ありと変えて有効性を比べる確認に作り直した。

**要実機確認**：作り直した同盟の提案可否の確認（`tools/civ6-autotest/scenarios/sengoku-alliances.json`）は未実行。総督画面を開いてもエラーが出ないことは実機で確かめる。連鎖削除の差分は SQLite での再現で、ゲーム本体の外部キー処理と一致するかは実機の`GameInfo` で確かめる。
"#;

pub fn make_cascade_doc() -> Vec<String> {
    let mut out: Vec<_> = CASCADE_DOC_HEADER.lines().map(str::to_owned).collect();
    for &(table, where_fields, values, impact, basis) in CASCADE_REMAPS {
        out.push(format!(
            "| {table} | {} | {impact} | {} | {basis} |",
            documentation_cell(where_fields),
            documentation_cell(values)
        ));
    }
    out.extend(
        [
            "",
            "### 付け替えずに失う行",
            "",
            "| 表 | 失う行 | 影響と理由 |",
            "|---|---|---|",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    for &(table, rows, reason) in CASCADE_DROPS {
        out.push(format!("| {table} | {rows} | {reason} |"));
    }
    out.push(String::new());
    out.extend(CASCADE_DOC_FOOTER.lines().map(str::to_owned));
    out.push(String::new());
    out
}

pub fn make_unlock_matrix(nodes: &[Node]) -> String {
    let mut out = vec![
        "# 戦国ツリーPoC：解放先と閃きの照合".to_owned(),
        String::new(),
        "設計案の各行を、現行の公式アイテムへ接続した範囲。公式アイテムの能力値・名称・時代感は暫定の代理表現であり、設計案固有の建物・Unit・政策・操作・効果はすべて TODO。未接続ノードも研究自体は可能。".to_owned(),
        "C25「太閤検地」は、研究したすべての家に同じ名前で表示する（SSOT 3.2）。".to_owned(),
        String::new(),
        "| ノード | 設計案の解放 | 現行PoCの公式代理 | 残課題 |".to_owned(),
        "|---|---|---|---|".to_owned(),
    ];
    for n in nodes {
        let unlock = n.unlock.replace('|', "\\|");
        if let Some((_, proxy)) = OFFICIAL_UNLOCKS.iter().find(|(short, _)| *short == n.short) {
            out.push(format!(
                "| {} {} | {unlock} | `{}` | TODO: 設計案の固有実装（{unlock}） |",
                n.short,
                n.ja,
                proxy.replace(", ", "`, `")
            ));
        } else {
            out.push(format!(
                "| {} {} | {unlock} | — | TODO: 公式代理なし（{unlock}） |",
                n.short, n.ja
            ));
        }
    }
    out.extend(
        [
            "",
            "## 閃き・着想",
            "",
            "公式のBoostClassで表せる2件だけを40%の暫定値で実装。T09は銀鉱山の新規改良、T10はT02で建造可能になったガレー船1隻の新規建造で判定する。",
            "それ以外は案の条件を省略・近似して自動達成させない。",
            "",
            "| ノード | 設計案の条件 | PoCの状態 |",
            "|---|---|---|",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    for n in nodes {
        let status = match n.short.as_str() {
            "T09" => "`BOOST_TRIGGER_IMPROVE_SPECIFIC_RESOURCE`（銀＋鉱山）",
            "T10" => "`BOOST_TRIGGER_TRAIN_UNIT`（ガレー船）",
            _ => "TODO: 公式BoostClassで完全には表せない",
        };
        out.push(format!(
            "| {} {} | {} | {status} |",
            n.short,
            n.ja,
            n.proposed_boost.replace('|', "\\|")
        ));
    }
    out.extend(
        [
            "",
            "具体的な原案は[設計表](../research/sengoku-tech-civic-tree.md)の各行を参照。",
            "",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    out.extend(make_cascade_doc());
    out.join("\n")
}

pub fn outputs(repo: &Repository) -> Result<Outputs> {
    let nodes = read_design(repo)?;
    let mut out = Outputs::new();
    for (path, content) in [
        ("Data/JAC_Sengoku_Trees.sql", make_sql(&nodes)?),
        ("Data/JAC_Sengoku_Cascade.sql", make_cascade_sql()),
        ("Text/JAC_Sengoku_Text.xml", make_text(&nodes)),
        ("Data/JAC_Sengoku_Icons.xml", make_icons(&nodes)),
        ("UI/JAC_HouseNamesData_Sengoku.lua", make_house_names()),
    ] {
        out.insert(PathBuf::from(MOD).join(path), content);
    }
    out.insert(
        PathBuf::from("documents/poc/sengoku-tree-unlocks.md"),
        make_unlock_matrix(&nodes),
    );
    Ok(out)
}

fn universal_newlines(text: String) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn cli(repo: &Repository, args: &[String]) -> Result<()> {
    let Some(check) = crate::parse_check_args(
        "generate-sengoku-trees",
        args,
        "Generate the Banseiki-only Sengoku tree rows from the reviewed design draft.",
    )?
    else {
        return Ok(());
    };
    let generated = outputs(repo)?;
    if check {
        for (path, expected) in &generated {
            match repo.read(path) {
                Ok(actual)
                    if crate::normalize_legacy_document(
                        path,
                        universal_newlines(actual.clone()),
                    ) == *expected => {}
                _ => bail!("stale generated file: {}", repo.path(path).display()),
            }
        }
    } else {
        apply_outputs(repo, generated, false)?;
    }
    println!(
        "{}",
        if check {
            "Sengoku tree files current"
        } else {
            "Sengoku tree files generated"
        }
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use roxmltree::{Document, Node as XmlNode};
    use rusqlite::{types::FromSql, Connection};
    use std::collections::BTreeSet;

    fn read_mod(repo: &Repository, path: &str) -> Result<String> {
        repo.read(PathBuf::from(MOD).join(path))
    }

    fn xml_nodes<'a, 'input>(root: XmlNode<'a, 'input>, path: &str) -> Vec<XmlNode<'a, 'input>> {
        let mut nodes = vec![root];
        for segment in path.split('/').filter(|s| !s.is_empty() && *s != ".") {
            nodes = nodes
                .into_iter()
                .flat_map(|node| {
                    node.children().filter(move |child| {
                        child.is_element() && (segment == "*" || child.tag_name().name() == segment)
                    })
                })
                .collect();
        }
        nodes
    }

    fn at<'a, 'input>(root: XmlNode<'a, 'input>, path: &str) -> XmlNode<'a, 'input> {
        xml_nodes(root, path)
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("missing XML path {path}"))
    }

    fn texts(root: XmlNode<'_, '_>, path: &str) -> Vec<String> {
        xml_nodes(root, path)
            .iter()
            .map(|node| node.text().unwrap_or("").to_owned())
            .collect()
    }

    fn attrs(node: XmlNode<'_, '_>) -> BTreeMap<String, String> {
        node.attributes()
            .map(|a| (a.name().to_owned(), a.value().to_owned()))
            .collect()
    }

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|&(k, v)| (k.to_owned(), v.to_owned()))
            .collect()
    }

    fn set<I, S>(values: I) -> BTreeSet<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        values.into_iter().map(|v| v.as_ref().to_owned()).collect()
    }

    fn captures(pattern: &str, source: &str) -> Result<BTreeSet<String>> {
        Ok(Regex::new(pattern)?
            .captures_iter(source)
            .map(|m| m[1].to_owned())
            .collect())
    }

    fn scalar<T: FromSql>(db: &Connection, sql: &str) -> Result<T> {
        Ok(db.query_row(sql, [], |row| row.get(0))?)
    }

    fn strings(db: &Connection, sql: &str) -> Result<Vec<String>> {
        let mut statement = db.prepare(sql)?;
        let rows = statement.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<String>>>()?)
    }

    #[test]
    fn test_ids_unique_and_complete() -> Result<()> {
        let nodes = read_design(&Repository::checked_in())?;
        assert_eq!(nodes.len(), 62);
        assert_eq!(set(nodes.iter().map(|n| &n.id)).len(), 62);
        assert_eq!(
            nodes.iter().filter(|n| n.short.starts_with('T')).count(),
            32
        );
        assert_eq!(
            nodes.iter().filter(|n| n.short.starts_with('C')).count(),
            30
        );
        let expected = (1..=32)
            .map(|i| format!("T{i:02}"))
            .chain((1..=30).map(|i| format!("C{i:02}")))
            .collect::<BTreeSet<_>>();
        assert_eq!(set(nodes.iter().map(|n| &n.short)), expected);
        Ok(())
    }

    #[test]
    fn test_prerequisites_acyclic_and_all_nodes_reachable() -> Result<()> {
        let nodes = read_design(&Repository::checked_in())?;
        let by_id: BTreeMap<_, _> = nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        fn walk(
            node: &Node,
            by_id: &BTreeMap<&str, &Node>,
            visiting: &mut BTreeSet<String>,
            visited: &mut BTreeSet<String>,
        ) {
            assert!(!visiting.contains(&node.id), "cycle at {}", node.short);
            if visited.contains(&node.id) {
                return;
            }
            visiting.insert(node.id.clone());
            for parent_id in &node.prereqs {
                assert!(by_id.contains_key(parent_id.as_str()), "{parent_id}");
                let parent = by_id[parent_id.as_str()];
                assert_eq!(parent.short.chars().next(), node.short.chars().next());
                assert!(parent.column < node.column);
                walk(parent, by_id, visiting, visited);
            }
            visiting.remove(&node.id);
            visited.insert(node.id.clone());
        }
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();
        for node in &nodes {
            walk(node, &by_id, &mut visiting, &mut visited);
        }
        assert_eq!(visited.len(), 62);
        assert_eq!(
            set(nodes
                .iter()
                .filter(|n| n.prereqs.is_empty())
                .map(|n| &n.short)),
            set(["T01", "T02", "T03", "T04", "C01", "C02", "C03"])
        );
        Ok(())
    }

    #[test]
    fn test_positions_and_costs() -> Result<()> {
        let repo = Repository::checked_in();
        let nodes = read_design(&repo)?;
        let order: Vec<_> = crate::reigns::eras(&repo)?
            .into_iter()
            .map(|era| era["EraType"].as_str().unwrap().to_owned())
            .collect();
        for (prefix, row_min, row_max, costs) in
            [('T', -3, 4, TECH_COSTS), ('C', -3, 3, CIVIC_COSTS)]
        {
            let mut group: Vec<_> = nodes
                .iter()
                .filter(|n| n.short.starts_with(prefix))
                .collect();
            let positions: BTreeSet<_> = group.iter().map(|n| (n.column, n.row)).collect();
            assert_eq!(
                positions.len(),
                group.len(),
                "overlapping {prefix} positions"
            );
            assert!(group
                .iter()
                .all(|n| (row_min..=row_max).contains(&n.row) && (1..=12).contains(&n.column)));
            assert!(group.iter().all(|n| n.cost == costs[n.column - 1]));
            assert!(costs.windows(2).all(|w| w[0] < w[1]));
            group.sort_by_key(|n| n.column);
            let indices: Vec<_> = group
                .iter()
                .map(|n| order.iter().position(|era| era == &n.era).unwrap())
                .collect();
            let mut sorted = indices.clone();
            sorted.sort();
            assert_eq!(indices, sorted);
            for node in group {
                assert_eq!(node.era, column_era(&repo, node.column)?);
            }
        }
        Ok(())
    }

    #[test]
    fn test_columns_map_to_the_reigns_of_their_intended_dates() -> Result<()> {
        let repo = Repository::checked_in();
        let eras = (1..=12)
            .map(|column| column_era(&repo, column))
            .collect::<Result<Vec<_>>>()?;
        assert_eq!(
            eras,
            [
                "ERA_ANCIENT",
                "ERA_ANCIENT",
                "ERA_ANCIENT",
                "ERA_ANCIENT",
                "ERA_ANCIENT",
                "ERA_MEDIEVAL",
                "ERA_RENAISSANCE",
                "ERA_INDUSTRIAL",
                "ERA_INDUSTRIAL",
                "ERA_INDUSTRIAL",
                "ERA_INDUSTRIAL",
                "ERA_ATOMIC",
            ]
        );
        Ok(())
    }

    const MOD_FILES: &[&str] = &[
        "Data/JAC_Sengoku_Oaths.sql",
        "Text/JAC_Sengoku_Oaths_Text.xml",
        "Scripts/JAC_Sengoku_Oaths.lua",
        "UI/JAC_DedicationPopup_Sengoku.lua",
        "Data/JAC_Sengoku_Calendar.xml",
        "Data/JAC_Sengoku_Eras.sql",
        "Data/JAC_Sengoku_Trees.sql",
        "Data/JAC_Sengoku_Resources.sql",
        "Data/JAC_Sengoku_Cascade.sql",
        "Data/JAC_Sengoku_Unlocks.sql",
        "Data/JAC_Sengoku_Victories.sql",
        "Data/JAC_Sengoku_Unification.sql",
        "Data/JAC_Sengoku_Ages.sql",
        "Data/JAC_Sengoku_Reigns.sql",
        "Data/JAC_Sengoku_Ages_Icons.xml",
        "Scripts/JAC_Sengoku_Ages.lua",
        "Text/JAC_Sengoku_Ages_Text.xml",
        "Text/JAC_Sengoku_Reigns_Text.xml",
        "Scripts/JAC_Sengoku_StartingKnowledge.lua",
        "Scripts/JAC_Sengoku_RebuiltCastles.lua",
        "Scripts/JAC_Sengoku_UnificationPact.lua",
        "UI/JAC_UnificationPact.xml",
        "UI/JAC_UnificationPact.lua",
        "UI/JAC_SengokuDarkPolicy.xml",
        "UI/JAC_SengokuDarkPolicy.lua",
        "UI/JAC_SengokuReignPopup.xml",
        "UI/JAC_SengokuReignPopup.lua",
        "UI/JAC_SengokuAgesView.lua",
        "UI/JAC_EraProgressPanel_Sengoku.lua",
        "UI/JAC_EraCompletePopup_Sengoku.lua",
        "UI/JAC_ActionPanel_Sengoku.lua",
        "UI/JAC_NotificationPanel_Sengoku.lua",
        "Data/JAC_Sengoku_Icons.xml",
        "Config/JAC_Sengoku_Victories.xml",
        "Config/JAC_Sengoku_Calendar.xml",
        "Text/JAC_Sengoku_Text.xml",
        "Text/JAC_Sengoku_Unification_Text.xml",
        "UI/JAC_HouseNamesData_Sengoku.lua",
    ];

    #[test]
    fn test_generated_files_current_and_all_text_keys_translated() -> Result<()> {
        let repo = Repository::checked_in();
        let nodes = read_design(&repo)?;
        for (path, expected) in outputs(&repo)? {
            assert_eq!(
                crate::normalize_legacy_document(&path, universal_newlines(repo.read(&path)?)),
                expected,
                "{}",
                path.display()
            );
        }
        let text = read_mod(&repo, "Text/JAC_Sengoku_Text.xml")?;
        let document = Document::parse(&text)?;
        let mut translations: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        for row in xml_nodes(document.root_element(), "LocalizedText/Row") {
            let tag = row.attribute("Tag").unwrap();
            let language = row.attribute("Language").unwrap();
            let value = at(row, "Text").text().unwrap_or("");
            assert!(
                translations
                    .entry(tag.to_owned())
                    .or_default()
                    .insert(language.to_owned(), value.to_owned())
                    .is_none(),
                "duplicate {tag}/{language}"
            );
        }
        let mut required: BTreeSet<_> = nodes
            .iter()
            .flat_map(|n| ["NAME", "DESCRIPTION"].map(|suffix| format!("LOC_{}_{suffix}", n.id)))
            .collect();
        required.extend(
            BOOSTS
                .iter()
                .map(|boost| format!("LOC_JAC_SENGOKU_BOOST_{}", boost.short)),
        );
        required.extend(
            HOUSE_NAMES
                .iter()
                .map(|(_, house, _, _)| (*house).to_owned()),
        );
        assert_eq!(set(translations.keys()), required);
        for (key, translated) in translations {
            assert_eq!(
                set(translated.keys()),
                set(["ja_JP", "en_US", "ru_RU"]),
                "{key}"
            );
            assert!(
                translated.values().all(|value| !value.trim().is_empty()),
                "{key}"
            );
        }

        let meta_text = read_mod(&repo, "JAC_PoC_SengokuTrees.modinfo")?;
        let meta = Document::parse(&meta_text)?;
        let root = meta.root_element();
        assert_eq!(
            at(root, "ActionCriteria/Criteria/RuleSetInUse").text(),
            Some("RULESET_JAC_BANSEIKI")
        );
        let criteria: BTreeSet<_> = xml_nodes(root, "InGameActions/*")
            .iter()
            .map(|action| action.attribute("criteria").map(str::to_owned))
            .collect();
        assert_eq!(
            criteria,
            BTreeSet::from([Some("JAC_Sengoku_BanseikiOnly".to_owned())])
        );
        for row in xml_nodes(root, "LocalizedText/Text") {
            for language in ["en_US", "ja_JP", "ru_RU"] {
                assert!(!at(row, language).text().unwrap_or("").is_empty());
            }
        }
        let files = texts(root, "Files/File");
        assert_eq!(files, MOD_FILES);
        for file in &files {
            assert!(repo.path(PathBuf::from(MOD).join(file)).is_file(), "{file}");
        }
        assert_eq!(
            texts(root, "FrontEndActions/UpdateDatabase/File"),
            [
                "Config/JAC_Sengoku_Victories.xml",
                "Config/JAC_Sengoku_Calendar.xml"
            ]
        );
        let database_files = texts(root, "InGameActions/UpdateDatabase/File");
        assert!(database_files.len() >= 4);
        assert_eq!(
            &database_files[database_files.len() - 4..],
            [
                "Data/JAC_Sengoku_Unification.sql",
                "Data/JAC_Sengoku_Ages.sql",
                "Data/JAC_Sengoku_Reigns.sql",
                "Data/JAC_Sengoku_Oaths.sql",
            ]
        );
        let icons_text = read_mod(&repo, "Data/JAC_Sengoku_Icons.xml")?;
        let icons = Document::parse(&icons_text)?;
        let icon_names: Vec<_> = xml_nodes(icons.root_element(), "IconDefinitions/Row")
            .iter()
            .map(|row| row.attribute("Name").unwrap().to_owned())
            .collect();
        assert_eq!(icon_names.len(), 124);
        assert_eq!(set(&icon_names).len(), 124);
        assert_eq!(
            set(&icon_names),
            nodes
                .iter()
                .flat_map(|n| ["", "_FOW"].map(|suffix| format!("ICON_{}{suffix}", n.id)))
                .collect::<BTreeSet<_>>()
        );
        Ok(())
    }

    #[test]
    fn test_every_unlock_proposal_has_explicit_proxy_or_todo() -> Result<()> {
        let repo = Repository::checked_in();
        let nodes = read_design(&repo)?;
        assert!(nodes
            .iter()
            .all(|n| !n.unlock.is_empty() && !n.proposed_boost.is_empty()));
        let shorts = set(nodes.iter().map(|n| &n.short));
        assert!(set(OFFICIAL_UNLOCKS.iter().map(|(short, _)| *short)).is_subset(&shorts));
        let matrix = repo.read("documents/poc/sengoku-tree-unlocks.md")?;
        for node in &nodes {
            assert!(matrix.contains(&format!("| {} {} |", node.short, node.ja)));
            assert!(matrix.contains(&node.unlock));
        }
        assert_eq!(
            matrix.matches("| TODO: 設計案の固有実装").count()
                + matrix.matches("| TODO: 公式代理なし").count(),
            62
        );
        assert_eq!(
            matrix
                .matches("| TODO: 公式BoostClassで完全には表せない")
                .count(),
            60
        );
        Ok(())
    }

    const TREE_TEST_SCHEMA: &str = r#"
CREATE TABLE Types (Type TEXT PRIMARY KEY, Kind TEXT);
CREATE TABLE Technologies (TechnologyType TEXT PRIMARY KEY, Name TEXT, Description TEXT, Cost INT, EraType TEXT, UITreeRow INT, AdvisorType TEXT);
CREATE TABLE Civics (CivicType TEXT PRIMARY KEY, Name TEXT, Description TEXT, Cost INT, EraType TEXT, UITreeRow INT, AdvisorType TEXT);
CREATE TABLE TechnologyPrereqs (Technology TEXT, PrereqTech TEXT);
CREATE TABLE CivicPrereqs (Civic TEXT, PrereqCivic TEXT);
CREATE TABLE Eras (EraType TEXT PRIMARY KEY, Name TEXT, Description TEXT);
CREATE TABLE Eras_XP2 (EraType TEXT, EraScoreThresholdShift INT);
CREATE TABLE ClimateParameters (Name TEXT);
CREATE TABLE Resolutions (ResolutionType TEXT);
CREATE TABLE Boosts (TechnologyType TEXT, Boost INT, TriggerDescription TEXT, TriggerLongDescription TEXT, BoostClass TEXT, ImprovementType TEXT, ResourceType TEXT, Unit1Type TEXT);
CREATE TABLE Units (UnitType TEXT PRIMARY KEY, PrereqTech TEXT, PrereqCivic TEXT, CanTrain INT);
CREATE TABLE UnitUpgrades (Unit TEXT, UpgradeUnit TEXT);
CREATE TABLE Districts (DistrictType TEXT PRIMARY KEY, PrereqTech TEXT, PrereqCivic TEXT, InternalOnly INT);
CREATE TABLE Buildings (BuildingType TEXT PRIMARY KEY, PrereqTech TEXT, PrereqCivic TEXT, InternalOnly INT);
CREATE TABLE Improvements (ImprovementType TEXT PRIMARY KEY, PrereqTech TEXT, PrereqCivic TEXT, Buildable INT);
CREATE TABLE Resources (ResourceType TEXT PRIMARY KEY, PrereqTech TEXT);
CREATE TABLE Governments (GovernmentType TEXT PRIMARY KEY, PrereqCivic TEXT);
CREATE TABLE StartingCivics (Era TEXT, Civic TEXT);
CREATE TABLE Policies (PolicyType TEXT PRIMARY KEY, PrereqCivic TEXT, PrereqTech TEXT);
CREATE TABLE Projects (ProjectType TEXT PRIMARY KEY, PrereqTech TEXT, PrereqCivic TEXT);
CREATE TABLE DynamicModifiers (ModifierType TEXT PRIMARY KEY, EffectType TEXT);
CREATE TABLE Modifiers (ModifierId TEXT PRIMARY KEY, ModifierType TEXT);
CREATE TABLE CivicModifiers (CivicType TEXT, ModifierId TEXT);
CREATE TABLE GameModifiers (ModifierId TEXT PRIMARY KEY);
CREATE TABLE GreatPersonIndividualActionModifiers (GreatPersonIndividualType TEXT, ModifierId TEXT);
CREATE TABLE GoodyHutSubTypes (SubTypeGoodyHut TEXT PRIMARY KEY, ModifierID TEXT);
CREATE TABLE TraitModifiers (TraitType TEXT, ModifierId TEXT);
CREATE TABLE TechnologyModifiers (TechnologyType TEXT, ModifierId TEXT);
CREATE TABLE DiplomaticVisibilitySources (VisibilitySourceType TEXT PRIMARY KEY, PrereqTech TEXT);
INSERT INTO TechnologyModifiers VALUES ('TECH_PRINTING','PRINTING_ADD_DIPLO_VISIBILITY'),('TECH_PRINTING','PRINTING_BOOST_WRITING_TOURISM');
INSERT INTO DiplomaticVisibilitySources VALUES ('SOURCE_TECH','TECH_PRINTING'),('SOURCE_TRADER',NULL);
INSERT INTO Technologies VALUES ('TECH_POTTERY','Pottery',NULL,25,'ERA_ANCIENT',0,NULL);
INSERT INTO Civics VALUES ('CIVIC_CODE_OF_LAWS','Code of Laws',NULL,20,'ERA_ANCIENT',0,NULL);
INSERT INTO Eras VALUES ('ERA_ANCIENT','Ancient',NULL),('ERA_CLASSICAL','Classical',NULL),('ERA_MEDIEVAL','Medieval',NULL),('ERA_RENAISSANCE','Renaissance',NULL);
INSERT INTO Eras_XP2 VALUES ('ERA_ANCIENT',-3);
INSERT INTO ClimateParameters VALUES ('TemperatureChange');
INSERT INTO Resolutions VALUES ('WC_RES_LUXURY');
INSERT INTO Units VALUES ('UNIT_GALLEY','TECH_SAILING',NULL,1),('UNIT_MODERN_ARMOR','TECH_COMPOSITES',NULL,1);
INSERT INTO Districts VALUES ('DISTRICT_HARBOR','TECH_CELESTIAL_NAVIGATION',NULL,0),('DISTRICT_AERODROME','TECH_FLIGHT',NULL,0);
INSERT INTO Buildings VALUES ('BUILDING_GRANARY','TECH_POTTERY',NULL,0);
INSERT INTO Improvements VALUES ('IMPROVEMENT_MINE','TECH_MINING',NULL,1);
INSERT INTO Resources VALUES ('RESOURCE_IRON','TECH_BRONZE_WORKING'),('RESOURCE_HORSES','TECH_ANIMAL_HUSBANDRY');
INSERT INTO Governments VALUES ('GOVERNMENT_CHIEFDOM',NULL),('GOVERNMENT_OLIGARCHY','CIVIC_POLITICAL_PHILOSOPHY');
INSERT INTO StartingCivics VALUES ('ERA_ANCIENT','CIVIC_CODE_OF_LAWS');
INSERT INTO Policies VALUES ('POLICY_DISCIPLINE','CIVIC_CODE_OF_LAWS',NULL);
INSERT INTO Projects VALUES ('PROJECT_REPAIR_OUTER_DEFENSES',NULL,NULL);
"#;

    #[test]
    fn test_sql_loads_and_rewires_without_erasing_gs_sentinels() -> Result<()> {
        let repo = Repository::checked_in();
        let db = Connection::open_in_memory()?;
        db.execute_batch(TREE_TEST_SCHEMA)?;
        db.execute_batch(&read_mod(&repo, "Data/JAC_Sengoku_Trees.sql")?)?;
        db.execute_batch(&read_mod(&repo, "Data/JAC_Sengoku_Unlocks.sql")?)?;
        for (table, col, count) in [
            ("Technologies", "TechnologyType", 32),
            ("Civics", "CivicType", 30),
        ] {
            assert_eq!(
                scalar::<i64>(&db, &format!("SELECT COUNT(*) FROM {table}"))?,
                count
            );
            assert_eq!(
                scalar::<i64>(
                    &db,
                    &format!("SELECT COUNT(*) FROM {table} WHERE {col} NOT LIKE '%JAC_SENGOKU%'")
                )?,
                0
            );
        }
        assert_eq!(
            scalar::<String>(
                &db,
                "SELECT PrereqTech FROM Units WHERE UnitType='UNIT_GALLEY'"
            )?,
            "TECH_JAC_SENGOKU_COASTAL_PILOTAGE"
        );
        assert_eq!(
            scalar::<i64>(
                &db,
                "SELECT CanTrain FROM Units WHERE UnitType='UNIT_MODERN_ARMOR'"
            )?,
            0
        );
        assert_eq!(
            scalar::<String>(
                &db,
                "SELECT PrereqTech FROM Districts WHERE DistrictType='DISTRICT_HARBOR'"
            )?,
            "TECH_JAC_SENGOKU_HARBOR_PILOTS"
        );
        assert_eq!(
            scalar::<String>(
                &db,
                "SELECT PrereqTech FROM Districts WHERE DistrictType='DISTRICT_AERODROME'"
            )?,
            "TECH_JAC_SENGOKU_OUTER_DEFENSES"
        );
        assert_eq!(
            scalar::<Option<String>>(
                &db,
                "SELECT PrereqTech FROM Resources WHERE ResourceType='RESOURCE_HORSES'"
            )?,
            None
        );
        {
            let mut statement = db.prepare("SELECT * FROM Eras ORDER BY EraType")?;
            let rows = statement
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            assert!(rows.len() >= 2);
            assert_eq!(
                &rows[..2],
                [
                    ("ERA_ANCIENT".to_owned(), "Ancient".to_owned(), None),
                    ("ERA_CLASSICAL".to_owned(), "Classical".to_owned(), None),
                ]
            );
        }
        assert_eq!(
            strings(
                &db,
                "SELECT Civic FROM StartingCivics WHERE Era='ERA_ANCIENT'"
            )?,
            ["CIVIC_JAC_SENGOKU_HOUSE_RULES"]
        );
        {
            let mut statement = db.prepare("SELECT * FROM TechnologyModifiers")?;
            let rows = statement
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            assert_eq!(
                rows,
                [
                    (
                        "TECH_JAC_SENGOKU_ROADS_AND_PACKTRAINS".to_owned(),
                        "PRINTING_ADD_DIPLO_VISIBILITY".to_owned()
                    ),
                    (
                        "TECH_PRINTING".to_owned(),
                        "PRINTING_BOOST_WRITING_TOURISM".to_owned()
                    ),
                ]
            );
        }
        assert_eq!(
            scalar::<String>(&db, "SELECT PrereqTech FROM DiplomaticVisibilitySources WHERE VisibilitySourceType='SOURCE_TECH'")?,
            "TECH_JAC_SENGOKU_ROADS_AND_PACKTRAINS"
        );
        for table in ["Eras_XP2", "ClimateParameters", "Resolutions"] {
            assert_eq!(
                scalar::<i64>(&db, &format!("SELECT COUNT(*) FROM {table}"))?,
                1
            );
        }
        db.close().map_err(|(_, error)| error)?;
        Ok(())
    }

    fn verify_starting_knowledge_lua(
        source: &str,
        tech_roots: &BTreeSet<String>,
        civic_roots: &BTreeSet<String>,
    ) -> Result<()> {
        let lua = mlua::Lua::new();
        let mut harness = r#"
GameInfo = { Technologies = {}, Civics = {} }
Game = {
  properties = {},
  GetProperty = function(self, key) return self.properties[key] end,
  SetProperty = function(self, key, value) self.properties[key] = value end,
  GetCurrentGameTurn = function() return currentTurn end
}
GameConfiguration = { GetStartTurn = function() return 0 end }
PlayerManager = { GetAliveMajors = function() return majors end }
LuaEvents = { NewGameInitialized = { Add = function(callback) initialize = callback end } }
function reset()
  currentTurn = 0
  techCalls = 0
  civicCalls = 0
  Game.properties = {}
  majors = {}
  for i = 1, 22 do
    local techs = {
      known = {},
      HasTech = function(self, index) return self.known[index] == true end,
      SetTech = function(self, index, value)
        assert(value == true)
        assert(not self.known[index])
        self.known[index] = value
        techCalls = techCalls + 1
      end
    }
    local culture = {
      known = {},
      HasCivic = function(self, index) return self.known[index] == true end,
      SetCivic = function(self, index, value)
        assert(value == true)
        assert(not self.known[index])
        self.known[index] = value
        civicCalls = civicCalls + 1
      end
    }
    local player = { id = i, techs = techs, culture = culture }
    function player:GetID() return self.id end
    function player:GetTechs() return self.techs end
    function player:GetCulture() return self.culture end
    majors[i] = player
  end
end
reset()
"#
        .to_owned();
        for (index, id) in tech_roots.iter().enumerate() {
            harness.push_str(&format!(
                "GameInfo.Technologies[{id:?}] = {{ Index = {index} }}\n"
            ));
        }
        for (index, id) in civic_roots.iter().enumerate() {
            harness.push_str(&format!(
                "GameInfo.Civics[{id:?}] = {{ Index = {index} }}\n"
            ));
        }
        lua.load(&harness)
            .exec()
            .map_err(|error| anyhow!(error.to_string()))?;
        lua.load(source)
            .exec()
            .map_err(|error| anyhow!(error.to_string()))?;
        lua.load(
            r#"
assert(type(initialize) == "function")
initialize()
assert(techCalls == 88 and civicCalls == 66)
assert(Game:GetProperty("JAC_SENGOKU_STARTING_KNOWLEDGE_GRANTED_V1") == true)
for _, player in ipairs(majors) do
  for index = 0, 3 do assert(player.techs:HasTech(index)) end
  for index = 0, 2 do assert(player.culture:HasCivic(index)) end
end
initialize()
assert(techCalls == 88 and civicCalls == 66)
currentTurn = 10
initialize()
assert(techCalls == 88 and civicCalls == 66)
reset()
currentTurn = 10
initialize()
assert(techCalls == 0 and civicCalls == 0)
assert(Game:GetProperty("JAC_SENGOKU_STARTING_KNOWLEDGE_GRANTED_V1") == nil)
reset()
majors[1].techs.known[0] = true
majors[1].culture.known[0] = true
initialize()
assert(techCalls == 87 and civicCalls == 65)
initialize()
assert(techCalls == 87 and civicCalls == 65)
reset()
"#,
        )
        .exec()
        .map_err(|error| anyhow!(error.to_string()))?;
        let first_tech = tech_roots.first().context("no starting techs")?;
        lua.load(format!("GameInfo.Technologies[{first_tech:?}] = nil"))
            .exec()
            .map_err(|error| anyhow!(error.to_string()))?;
        assert!(lua.load("initialize()").exec().is_err());
        Ok(())
    }

    #[test]
    fn test_starting_knowledge_matches_all_era_i_roots_and_is_new_game_only() -> Result<()> {
        let repo = Repository::checked_in();
        let nodes = read_design(&repo)?;
        let tech_roots = set(nodes
            .iter()
            .filter(|n| n.short.starts_with('T') && n.era == "ERA_ANCIENT" && n.prereqs.is_empty())
            .map(|n| &n.id));
        let civic_roots = set(nodes
            .iter()
            .filter(|n| n.short.starts_with('C') && n.era == "ERA_ANCIENT" && n.prereqs.is_empty())
            .map(|n| &n.id));
        assert_eq!(tech_roots.len(), 4);
        assert_eq!(civic_roots.len(), 3);
        let source = read_mod(&repo, "Scripts/JAC_Sengoku_StartingKnowledge.lua")?;
        assert_eq!(
            captures(r#""(TECH_JAC_SENGOKU_[A-Z_]+)""#, &source)?,
            tech_roots
        );
        assert_eq!(
            captures(r#""(CIVIC_JAC_SENGOKU_[A-Z_]+)""#, &source)?,
            civic_roots
        );
        for required in [
            "pCulture:SetCivic(civicIndex, true)",
            "PlayerManager.GetAliveMajors()",
            "LuaEvents.NewGameInitialized.Add(InitializeNewGame)",
            "Game:GetProperty(INITIALIZED_KEY)",
            "Game:SetProperty(INITIALIZED_KEY, true)",
            "Game.GetCurrentGameTurn() > GameConfiguration.GetStartTurn()",
            "playerTechs:SetTech(techIndex, true)",
        ] {
            assert!(source.contains(required), "{required}");
        }
        assert!(!source.contains("SetResearchProgress"));
        assert!(!source.contains("TriggerBoost"));

        let meta_text = read_mod(&repo, "JAC_PoC_SengokuTrees.modinfo")?;
        let meta = Document::parse(&meta_text)?;
        let root = meta.root_element();
        let script_nodes = xml_nodes(root, "InGameActions/AddGameplayScripts");
        assert!(!script_nodes.is_empty());
        let script = script_nodes[0];
        assert_eq!(
            script.attribute("criteria"),
            Some("JAC_Sengoku_BanseikiOnly")
        );
        let criteria: BTreeMap<_, _> = xml_nodes(root, "ActionCriteria/Criteria")
            .into_iter()
            .map(|node| (node.attribute("id").unwrap(), node))
            .collect();
        let only = criteria["JAC_Sengoku_BanseikiOnly"];
        assert_eq!(
            only.children()
                .filter(|n| n.is_element())
                .map(|n| n.tag_name().name())
                .collect::<Vec<_>>(),
            ["RuleSetInUse"]
        );
        assert_eq!(
            at(only, "RuleSetInUse").text(),
            Some("RULESET_JAC_BANSEIKI")
        );
        assert_eq!(
            texts(script, "File"),
            ["Scripts/JAC_Sengoku_StartingKnowledge.lua"]
        );

        let scenario =
            repo.json("tools/civ6-autotest/scenarios/sengoku-starting-knowledge.json")?;
        let steps = scenario["steps"]
            .as_array()
            .context("scenario steps must be an array")?;
        let players = steps
            .iter()
            .find(|step| step["action"] == "configure")
            .context("missing configure step")?["players"]
            .as_array()
            .context("missing players")?;
        assert_eq!(players.len(), 22);
        assert_eq!(
            players
                .iter()
                .filter(|player| player["human"].as_bool().unwrap())
                .count(),
            1
        );
        let checks = steps
            .iter()
            .filter(|step| step["action"] == "ui_lua")
            .map(|step| step["code"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(checks
            .iter()
            .any(|code| code.contains("GetLiveData()") && code.contains("ITEM_STATUS.RESEARCHED")));
        let game_checks = checks
            .iter()
            .filter(|code| code.contains("GetAllMomentsData"))
            .collect::<Vec<_>>();
        assert_eq!(game_checks.len(), 2);
        for code in game_checks {
            assert!(code.contains("#majors~=22"));
            assert_eq!(
                captures(r"\['(TECH_JAC_SENGOKU_[A-Z_]+)'\]=true", code)?,
                tech_roots
            );
            assert_eq!(
                captures(r"\['(CIVIC_JAC_SENGOKU_[A-Z_]+)'\]=true", code)?,
                civic_roots
            );
        }
        let actions = steps
            .iter()
            .map(|step| step["action"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(actions.len() >= 4);
        assert_eq!(
            &actions[actions.len() - 4..],
            ["save", "exit_to_menu", "load", "ui_lua"]
        );
        verify_starting_knowledge_lua(&source, &tech_roots, &civic_roots)?;
        Ok(())
    }

    #[test]
    fn test_banseiki_sengoku_disables_official_victories_and_time_limit() -> Result<()> {
        let repo = Repository::checked_in();
        let base_text = repo.read("poc/JAC_PoC_BanseikiRuleset/Config/JAC_Banseiki_Config.xml")?;
        let base_doc = Document::parse(&base_text)?;
        let base = base_doc.root_element();
        assert_eq!(
            at(base, "Rulesets/Row").attribute("VictoryDomain"),
            Some("Expansion2_Victories")
        );
        let config_text = read_mod(&repo, "Config/JAC_Sengoku_Victories.xml")?;
        let config_doc = Document::parse(&config_text)?;
        let config = config_doc.root_element();
        let update = at(config, "Rulesets/Update");
        assert_eq!(
            attrs(at(update, "Where")),
            map(&[("RulesetType", "RULESET_JAC_BANSEIKI")])
        );
        assert_eq!(
            attrs(at(update, "Set")),
            map(&[
                ("VictoryDomain", "JAC_Sengoku_Victories"),
                ("MaxTurns", "0")
            ])
        );
        assert_eq!(
            xml_nodes(config, "Victories/Row")
                .into_iter()
                .map(attrs)
                .collect::<Vec<_>>(),
            vec![map(&[
                ("Domain", "JAC_Sengoku_Victories"),
                ("VictoryType", "VICTORY_JAC_SENGOKU_UNIFICATION"),
                ("Name", "LOC_VICTORY_JAC_UNIFICATION_NAME"),
                ("Description", "LOC_VICTORY_JAC_UNIFICATION_DESCRIPTION"),
                ("ReadOnly", "true"),
                ("EnabledByDefault", "true"),
            ])]
        );
        assert_eq!(
            xml_nodes(config, "RulesetSupportedValues/Row")
                .into_iter()
                .map(attrs)
                .collect::<Vec<_>>(),
            vec![map(&[
                ("Ruleset", "RULESET_JAC_BANSEIKI"),
                ("Domain", "StandardTurnLimits"),
                ("Value", "TURNLIMIT_NONE"),
            ])]
        );

        let official = set([
            "VICTORY_DEFAULT",
            "VICTORY_CONQUEST",
            "VICTORY_TECHNOLOGY",
            "VICTORY_CULTURE",
            "VICTORY_RELIGIOUS",
            "VICTORY_DIPLOMATIC",
            "VICTORY_SCORE",
        ]);
        let updates = xml_nodes(config, "ConfigurationUpdates/Row");
        assert!(updates.iter().all(|row| {
            row.attribute("SourceGroup") == Some("Game")
                && row.attribute("SourceId") == Some("RULESET")
                && row.attribute("SourceValue") == Some("RULESET_JAC_BANSEIKI")
                && row.attribute("TargetGroup") == Some("Game")
        }));
        let values: BTreeMap<_, _> = updates
            .iter()
            .map(|row| {
                (
                    row.attribute("TargetId").unwrap().to_owned(),
                    row.attribute("TargetValue").unwrap().to_owned(),
                )
            })
            .collect();
        assert_eq!(values.len(), updates.len());
        let mut expected: BTreeMap<_, _> = official
            .iter()
            .map(|id| (id.clone(), "0".to_owned()))
            .collect();
        expected.insert("DEFEAT_TIME".to_owned(), "0".to_owned());
        expected.insert("GAME_MAX_TURNS".to_owned(), "0".to_owned());
        assert_eq!(values, expected);

        let base_limit: Vec<_> = xml_nodes(base, "ConfigurationUpdates/Row")
            .into_iter()
            .filter(|row| {
                row.attribute("SourceValue") == Some("RULESET_JAC_BANSEIKI")
                    && row.attribute("TargetId") == Some("GAME_TURN_LIMIT")
            })
            .map(attrs)
            .collect();
        assert_eq!(base_limit.len(), 1);
        let limit = at(config, "ConfigurationUpdates/Update");
        let expected_where: BTreeMap<_, _> = [
            "SourceGroup",
            "SourceId",
            "SourceValue",
            "TargetGroup",
            "TargetId",
        ]
        .into_iter()
        .map(|key| (key.to_owned(), base_limit[0][key].clone()))
        .collect();
        assert_eq!(attrs(at(limit, "Where")), expected_where);
        assert_eq!(
            attrs(at(limit, "Set")),
            map(&[("TargetValue", "TURNLIMIT_NONE")])
        );

        let db = Connection::open_in_memory()?;
        db.execute_batch(
            "CREATE TABLE Victories (VictoryType TEXT PRIMARY KEY, EnabledByDefault INT);
             CREATE TABLE Defeats (DefeatType TEXT PRIMARY KEY, EnabledByDefault INT);
             INSERT INTO Victories VALUES ('VICTORY_CUSTOM', 1);
             INSERT INTO Defeats VALUES ('DEFEAT_DEFAULT', 1), ('DEFEAT_TIME', 1);",
        )?;
        for victory in &official {
            db.execute("INSERT INTO Victories VALUES (?, 1)", [victory])?;
        }
        db.execute_batch(&read_mod(&repo, "Data/JAC_Sengoku_Victories.sql")?)?;
        let mut statement = db.prepare("SELECT VictoryType, EnabledByDefault FROM Victories")?;
        let victory_values = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<BTreeMap<_, _>>>()?;
        let mut expected: BTreeMap<_, _> = official.into_iter().map(|id| (id, 0_i64)).collect();
        expected.insert("VICTORY_CUSTOM".to_owned(), 1);
        assert_eq!(victory_values, expected);
        drop(statement);
        let mut statement = db.prepare("SELECT DefeatType, EnabledByDefault FROM Defeats")?;
        let defeat_values = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<BTreeMap<_, _>>>()?;
        assert_eq!(
            defeat_values,
            BTreeMap::from([
                ("DEFEAT_DEFAULT".to_owned(), 1_i64),
                ("DEFEAT_TIME".to_owned(), 0_i64)
            ])
        );
        drop(statement);
        db.close().map_err(|(_, error)| error)?;
        Ok(())
    }
}
