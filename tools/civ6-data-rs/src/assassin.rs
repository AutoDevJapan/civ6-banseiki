use anyhow::{ensure, Context, Result};
use serde_json::{json, Value};
use std::path::PathBuf;

use crate::{apply_outputs, Outputs, Repository};

pub const MOD_ID: &str = "fbb5219d-d7b9-4b31-884f-057f623476a0";
pub const MAP_ID: &str = "403be889-1455-4de7-967b-ad54c0ff3431";

const MOD: &str = "poc/JAC_PoC_Assassin";
const BASE_SCENARIO: &str = "tools/civ6-autotest/scenarios/court-arbitration.json";
const SCENARIO: &str = "tools/civ6-autotest/scenarios/assassin-leader-save-load.json";
const LANGUAGES: [&str; 3] = ["ja_JP", "en_US", "ru_RU"];
const PANEL_TEMPLATE: &str = include_str!("../../../poc/JAC_PoC_Assassin/UI/JAC_AssassinPanel.lua");
#[path = "assassin_panel_initializer.rs"]
mod panel_initializer;

const TEXT: &[(&str, [&str; 3])] = &[
    ("MOD_NAME", ["JAC PoC：暗殺者", "JAC PoC: Assassin", "JAC PoC: убийца"]),
    ("MOD_DESCRIPTION", ["日本マップの暗殺者育成・任務・保存の試作。自由都市への分裂、隣国吸収、外交支持による採決、後継者の継承を処理。", "Japan-map training, missions and persistence. Leader death uses free cities, weighted neighbours, favor-weighted voting or succession.", "Подготовка, задания и сохранение на картах Японии. После гибели правителя: вольные города, поглощение соседом, голосование с дипломатическим влиянием или наследование."]),
    ("NINJA_NAME", ["暗殺忍者", "Assassin Ninja", "Ниндзя-убийца"]),
    ("COMMON_NAME", ["暗殺者", "Assassin", "Убийца"]),
    ("UNIT_DESCRIPTION", ["地図を歩く暗殺者。都市を指定して通常100ターン・最短20ターンで育成。完成後は全国10ターン育成不可。", "An assassin who travels on the map. Train in one city: normally 100 turns, at least 20. After completion, training is locked nationwide for 10 turns.", "Убийца, передвигающийся по карте. Подготовка в одном городе: обычно 100 ходов, минимум 20. После завершения подготовка запрещена во всей стране на 10 ходов."]),
    ("INFILTRATION", ["潜入", "Infiltration", "Проникновение"]),
    ("INFILTRATION_DESCRIPTION", ["他国の領土に入れる。", "May enter foreign territory.", "Может входить на чужую территорию."]),
    ("TITLE", ["暗殺者", "Assassins", "Убийцы"]),
    ("CLOSE", ["閉じる", "Close", "Закрыть"]),
    ("PRODUCE", ["育成を開始", "Start training", "Начать подготовку"]),
    ("CANCEL", ["育成を中止", "Cancel training", "Отменить подготовку"]),
    ("ATTEMPT", ["暗殺を実行", "Attempt assassination", "Начать покушение"]),
    ("WAIT", ["任務中", "On a mission", "На задании"]),
    ("SELECT", ["育成は都市を指定。暗殺は地図上の暗殺者を選び、対象の近くへ移動して実行。", "Choose a city for training. Select an assassin on the map and move near a target to attempt a mission.", "Выберите город для подготовки. Выберите убийцу на карте и переместите его к цели, чтобы начать задание."]),
    ("STATUS", ["育成ロック：あと{1}ターン／暗殺コマンド待機：あと{2}ターン", "Training lock: {1} turns / command wait: {2} turns", "Запрет подготовки: {1} ходов / ожидание команды: {2} ходов"]),
    ("CITY", ["{1}：育成{2}ターン", "{1}: train in {2} turns", "{1}: подготовка за {2} ходов"]),
    ("TRAINING", ["都市{1}で育成中：あと{2}ターン", "Training in city {1}: {2} turns remaining", "Подготовка в городе {1}: осталось {2} ходов"]),
    ("MISSION", ["{1}への任務：あと{2}ターン", "Mission against {1}: {2} turns remaining", "Задание против {1}: осталось {2} ходов"]),
    ("TARGET", ["{1}・{2}：基本{3}%、補正後{5}%、{4}ターン", "{1}, {2}: base {3}%, adjusted {5}%, {4} turns", "{1}, {2}: базовый шанс {3}%, итоговый {5}%, {4} ходов"]),
    ("GOVERNOR", ["Governor", "Governor", "Губернатор"]),
    ("SUCCESSOR", ["後継者", "Successor", "Преемник"]),
    ("PRODUCED", ["暗殺者の育成が完了。全国の次の育成まで{1}ターン。", "Training complete. Nationwide training unlocks in {1} turns.", "Подготовка завершена. Подготовка во всей стране станет доступна через {1} ходов."]),
    ("RESULT", ["暗殺任務：{1}", "Assassination mission: {1}", "Задание убийцы: {1}"]),
    ("RESULT_LEADER_FREE_STATE", ["指導者死亡。各都市が自由都市へ分裂し、旧軍は解散。", "Leader dead. The nation splits into free cities and its army is disbanded.", "Правитель погиб. Страна распалась на вольные города, армия распущена."]),
    ("RESULT_LEADER_ABSORPTION", ["指導者死亡。隣国へ都市を移管し、旧軍を解散。存命人物は受入国へ加入。", "Leader dead. Cities transferred to a neighbor; old units disbanded and surviving people enrolled.", "Правитель погиб. Города переданы соседу; войска распущены, выжившие личности приняты."]),
    ("RESULT_LEADER_SUCCESSION", ["指導者死亡。登録済みの後継者が同じ国と資産を継承。", "Leader dead. A registered successor inherits the same nation and assets.", "Правитель погиб. Зарегистрированный преемник наследует страну и имущество."]),
    ("RESULT_GOVERNOR_DEAD_PENDING", ["Governor死亡。既存枠の処理は未確認。", "Governor dead; native-slot handling is unverified.", "Губернатор погиб; обработка штатного слота не проверена."]),
    ("RESULT_UNIT_KILLED", ["対象Unit死亡。", "Target unit killed.", "Целевой юнит погиб."]),
    ("RESULT_FAILED", ["失敗。暗殺者死亡。", "Failed. Assassin killed.", "Неудача. Убийца погиб."]),
    ("RESULT_TARGET_GONE", ["対象が失われたため任務終了。", "Mission ended: target no longer exists.", "Задание завершено: цель больше не существует."]),
    ("RESULT_ACTOR_GONE", ["暗殺者消失。すべての結果を取消し、固定の友好度・不平の罰則を適用。", "Assassin lost. All results cancelled; fixed friendship and grievance penalties applied.", "Убийца потерян. Все результаты отменены; применены фиксированные штрафы к дружбе и недовольству."]),
    ("REASON_PRODUCING", ["別の都市で既に育成中。", "Training is already in progress.", "Подготовка уже идёт."]),
    ("REASON_PRODUCTION_LOCK", ["完成後の全国育成ロック中。", "Nationwide training lock is active.", "Действует запрет подготовки во всей стране."]),
    ("REASON_MISSION_ACTIVE", ["自国の暗殺任務が進行中。", "A national assassination mission is active.", "Задание убийцы от вашей страны уже идёт."]),
    ("REASON_COMMAND_LOCK", ["前回の実行からの全国待機中。", "Nationwide command waiting period is active.", "Действует общестрановое ожидание после предыдущей команды."]),
    ("REASON_TARGET_GONE", ["対象が存在しない。", "Target no longer exists.", "Цель больше не существует."]),
    ("REASON_TARGET_UNKNOWN", ["対象の所在を確認する。", "Discover the target location.", "Установите местонахождение цели."]),
    ("REASON_CITY_GONE", ["育成する都市が存在しない。", "Training city no longer exists.", "Города для подготовки больше не существует."]),
    ("REASON_TARGET_KIND", ["この対象には実行できない。", "Invalid target.", "Недопустимая цель."]),
    ("REASON_ASSASSIN_REQUIRED", ["暗殺者Unitを選択する。", "Select an assassin unit.", "Выберите юнит убийцы."]),
    ("REASON_TOO_FAR", ["対象の近くまで移動する。", "Move closer to the target.", "Подойдите ближе к цели."]),
    ("REASON_DURATION_UNSET", ["任務期間が未設定。", "Mission duration is unset.", "Длительность задания не задана."]),
];

struct Element {
    name: String,
    attributes: Vec<(String, String)>,
    text: Option<String>,
    children: Vec<Element>,
}

impl Element {
    fn new(name: &str, attributes: &[(&str, &str)], children: Vec<Self>) -> Self {
        Self {
            name: name.to_owned(),
            attributes: attributes
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
            text: None,
            children,
        }
    }

    fn text(name: &str, text: &str) -> Self {
        Self {
            name: name.to_owned(),
            attributes: Vec::new(),
            text: Some(text.to_owned()),
            children: Vec::new(),
        }
    }

    fn append_xml(&self, output: &mut String, depth: usize) {
        let indentation = "  ".repeat(depth);
        output.push_str(&indentation);
        output.push('<');
        output.push_str(&self.name);
        for (key, value) in &self.attributes {
            output.push(' ');
            output.push_str(key);
            output.push_str("=\"");
            output.push_str(
                &crate::xml_escape(value)
                    .replace('"', "&quot;")
                    .replace('\r', "&#13;")
                    .replace('\n', "&#10;")
                    .replace('\t', "&#09;"),
            );
            output.push('"');
        }
        if self.children.is_empty() && self.text.as_deref().unwrap_or("").is_empty() {
            output.push_str(" />\n");
        } else if self.children.is_empty() {
            output.push('>');
            output.push_str(&crate::xml_escape(self.text.as_deref().unwrap_or("")));
            output.push_str("</");
            output.push_str(&self.name);
            output.push_str(">\n");
        } else {
            output.push_str(">\n");
            for child in &self.children {
                child.append_xml(output, depth + 1);
            }
            output.push_str(&indentation);
            output.push_str("</");
            output.push_str(&self.name);
            output.push_str(">\n");
        }
    }

    fn xml(&self) -> String {
        let mut output = "<?xml version='1.0' encoding='utf-8'?>\n".to_owned();
        self.append_xml(&mut output, 0);
        output
    }
}

pub fn packaging() -> Outputs {
    let properties = Element::new(
        "Properties",
        &[],
        [
            ("Name", "LOC_JAC_ASN_MOD_NAME"),
            ("Teaser", "LOC_JAC_ASN_MOD_DESCRIPTION"),
            ("Description", "LOC_JAC_ASN_MOD_DESCRIPTION"),
            ("Authors", "civ6-banseiki"),
            ("CompatibleVersions", "2.0"),
        ]
        .into_iter()
        .map(|(name, value)| Element::text(name, value))
        .collect(),
    );
    let dependencies = Element::new(
        "Dependencies",
        &[],
        [
            (
                "4873eb62-8ccc-4574-b784-dda455e74e68",
                "LOC_EXPANSION2_MOD_TITLE",
            ),
            (
                "bd7a82c5-6b38-4b69-a7f5-8dc9af534249",
                "LOC_JAC_BANSEIKI_MOD_NAME",
            ),
            (
                "ba3a1c58-9287-4654-a22f-85f9e6841e9b",
                "LOC_JAC_POC_CUSTOM_CIVS_MOD_NAME",
            ),
            (
                "26c967d0-c10d-4eeb-8aaf-c83dc7e621e7",
                "LOC_JAC_SENGOKU_TREES_MOD_NAME",
            ),
            (MAP_ID, "LOC_JAC_JAPAN_SCALE_MOD_NAME"),
        ]
        .into_iter()
        .map(|(id, title)| Element::new("Mod", &[("id", id), ("title", title)], Vec::new()))
        .collect(),
    );
    let localized = Element::new(
        "LocalizedText",
        &[],
        TEXT[..2]
            .iter()
            .map(|(tag, values)| {
                Element::new(
                    "Text",
                    &[("id", &format!("LOC_JAC_ASN_{tag}"))],
                    LANGUAGES
                        .iter()
                        .zip(values.iter())
                        .map(|(language, value)| Element::text(language, value))
                        .collect(),
                )
            })
            .collect(),
    );
    let mut criteria = Vec::new();
    let mut actions = Vec::new();
    for km in [15, 20] {
        for suffix in ["", "_Settlers"] {
            let id = format!("JAC_Assassin_{km}{suffix}");
            let map = format!("{{{MAP_ID}}}Maps/JAC_Japan_{km}km{suffix}.Civ6Map");
            criteria.push(Element::new(
                "Criteria",
                &[("id", &id)],
                vec![
                    Element::text("RuleSetInUse", "RULESET_JAC_BANSEIKI"),
                    Element::new(
                        "ConfigurationValueMatches",
                        &[],
                        vec![
                            Element::text("Group", "Map"),
                            Element::text("ConfigurationId", "MAP_SCRIPT"),
                            Element::text("Value", &map),
                        ],
                    ),
                ],
            ));
            for (kind, path) in [
                ("UpdateDatabase", "Data/JAC_Assassin.sql"),
                ("UpdateText", "Text/JAC_Assassin_Text.xml"),
                ("UpdateIcons", "Data/JAC_Assassin_Icons.xml"),
                ("AddGameplayScripts", "Scripts/JAC_Assassin_Gameplay.lua"),
                ("AddUserInterfaces", "UI/JAC_AssassinPanel.xml"),
            ] {
                let mut properties = vec![Element::text("LoadOrder", "350")];
                if kind == "AddUserInterfaces" {
                    properties.push(Element::text("Context", "InGame"));
                }
                actions.push(Element::new(
                    kind,
                    &[("id", &format!("{id}_{kind}")), ("criteria", &id)],
                    vec![
                        Element::new("Properties", &[], properties),
                        Element::text("File", path),
                    ],
                ));
            }
        }
    }
    actions.push(Element::new(
        "ImportFiles",
        &[("id", "JAC_Assassin_SharedFiles")],
        vec![
            Element::text("File", "Scripts/JAC_Assassin_Rules.lua"),
            Element::text("File", "UI/JAC_AssassinPanel.lua"),
        ],
    ));
    let mut files = [
        "Data/JAC_Assassin.sql",
        "Data/JAC_Assassin_Icons.xml",
        "Text/JAC_Assassin_Text.xml",
        "Scripts/JAC_Assassin_Gameplay.lua",
        "Scripts/JAC_Assassin_Rules.lua",
        "UI/JAC_AssassinPanel.xml",
        "UI/JAC_AssassinPanel.lua",
    ];
    files.sort_unstable();
    let root = Element::new(
        "Mod",
        &[("id", MOD_ID), ("version", "1")],
        vec![
            properties,
            dependencies,
            localized,
            Element::new("ActionCriteria", &[], criteria),
            Element::new("InGameActions", &[], actions),
            Element::new(
                "Files",
                &[],
                files
                    .into_iter()
                    .map(|path| Element::text("File", path))
                    .collect(),
            ),
        ],
    );
    let mut rows = Vec::new();
    for (tag, values) in TEXT {
        for (language, value) in LANGUAGES.iter().zip(values.iter()) {
            rows.push(Element::new(
                "Row",
                &[
                    ("Tag", &format!("LOC_JAC_ASN_{tag}")),
                    ("Language", language),
                ],
                vec![Element::text("Text", value)],
            ));
        }
    }
    let localization = Element::new(
        "GameData",
        &[],
        vec![Element::new("LocalizedText", &[], rows)],
    );
    let mut definitions = Vec::new();
    for unit in ["UNIT_JAC_JAPAN_SENGOKU_ASSASSIN", "UNIT_JAC_ASSASSIN"] {
        for (suffix, atlas, index) in [
            ("", "ICON_ATLAS_UNITS", "19"),
            ("_FOW", "ICON_ATLAS_UNITS_FOW", "19"),
            ("_PORTRAIT", "ICON_ATLAS_UNIT_PORTRAITS", "17"),
            ("_WHITE", "ICON_ATLAS_UNIT_FLAG_SYMBOLS_WHITE", "19"),
        ] {
            definitions.push(Element::new(
                "Row",
                &[
                    ("Name", &format!("ICON_{unit}{suffix}")),
                    ("Atlas", atlas),
                    ("Index", index),
                ],
                Vec::new(),
            ));
        }
    }
    let icons = Element::new(
        "GameData",
        &[],
        vec![Element::new("IconDefinitions", &[], definitions)],
    );
    [
        (
            PathBuf::from(MOD).join("JAC_PoC_Assassin.modinfo"),
            root.xml(),
        ),
        (
            PathBuf::from(MOD).join("Text/JAC_Assassin_Text.xml"),
            localization.xml(),
        ),
        (
            PathBuf::from(MOD).join("Data/JAC_Assassin_Icons.xml"),
            icons.xml(),
        ),
    ]
    .into_iter()
    .collect()
}

const PREFIX: &str = "local R=ExposedMembers.JAC_Assassin; assert(R,'Assassin gameplay absent'); local t=Game:GetProperty('JAC_ASN_LIVE_TEST'); ";

const GRANT: &str = r#"local function FixtureReveal(owner,x,y)
      local ok,why=R.TestRevealTile(owner,x,y)
      assert(ok,'fixture reveal failed: '..tostring(why))
    end; local function Grant(owner,kind,city)
      assert(Game:GetProperty(R.TEST)==1,'fixture grant requires test mode')
      local x,y=city:GetX(),city:GetY()
      assert(x>=0 and y>=0,'fixture anchor has invalid coordinates: '..x..','..y)
      local found
      for radius=1,6 do
        local before={}; for _,u in Players[owner]:GetUnits():Members() do before[u:GetID()]=true end
        UnitManager.InitUnitValidAdjacentHex(owner,kind,x,y,radius)
        for _,u in Players[owner]:GetUnits():Members() do
          if not before[u:GetID()] and u:GetType()==GameInfo.Units[kind].Index then
            if u:GetX()>=0 and u:GetY()>=0 and Map.GetPlotDistance(x,y,u:GetX(),u:GetY())<=6 then found=u
            else
              print('[JAC_ASN_FIXTURE] unplaced unit owner='..owner..' kind='..kind..' radius='..radius..
                ' xy='..u:GetX()..','..u:GetY())
              UnitManager.Kill(u)
            end
          end
        end
        if found then break end
      end
      assert(found,'fixture unit has no valid placement within six tiles: '..kind..' owner='..owner)
      if R.IsAssassin(found) then FixtureReveal(owner,x,y); FixtureReveal(owner,found:GetX(),found:GetY()) end
      return found
    end; local function TrainedUnit(owner,id)
      local u=Players[owner]:GetUnits():FindID(id)
      assert(u and R.IsAssassin(u) and u:GetX()>=0 and u:GetY()>=0,
        'trained unit is not placed on the map: id='..tostring(id)..' turn='..R.Turn()..
        ' type='..tostring(u and u:GetType())..' x='..tostring(u and u:GetX())..' y='..tostring(u and u:GetY()))
      return u
    end; local function TrainedSnapshot(owner,id)
      local u=Players[owner]:GetUnits():FindID(id)
      local row={ID=id,Present=u~=nil}
      if u then row.Type=u:GetType(); row.X=u:GetX(); row.Y=u:GetY(); row.Damage=u:GetDamage() end
      print('[JAC_ASN_TRACE] snapshot turn='..R.Turn()..' id='..id..' present='..tostring(row.Present)..
        ' type='..tostring(row.Type)..' xy='..tostring(row.X)..','..tostring(row.Y)..' damage='..tostring(row.Damage))
      return row
    end; local function ParkTrainedUnit(owner,id,cityID)
      assert(Game:GetProperty(R.TEST)==1,'trained parking requires test mode')
      local u=TrainedUnit(owner,id); local city=Players[owner]:GetCities():FindID(cityID)
      assert(city and city:GetOwner()==owner,'trained parking requires an owned city')
      local blockers={}
      for _,other in Players[owner]:GetUnits():Members() do
        if other:GetID()~=id and other:GetX()==city:GetX() and other:GetY()==city:GetY() then
          assert(not R.IsAssassin(other),'trained parking would remove another assassin')
          blockers[#blockers+1]=other
        end
      end
      -- Clear the fixture's native garrison to avoid stacking two combat units.
      -- Keep the production ID; do not replace it with a fixture-granted unit.
      for _,other in ipairs(blockers) do UnitManager.Kill(other) end
      -- Official DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:854.
      UnitManager.PlaceUnit(u,city:GetX(),city:GetY())
      assert(u:GetX()==city:GetX() and u:GetY()==city:GetY(),'trained parking did not reach city center')
      return TrainedSnapshot(owner,id)
    end; local function TrainingUnits(owner)
      local ids={}; for _,u in Players[owner]:GetUnits():Members() do
        if R.IsAssassin(u) then ids[tostring(u:GetID())]=true end
      end; return ids
    end; local function NewTrainedUnit(owner,before,id)
      local count=0; for key in pairs(TrainingUnits(owner)) do if not before[key] then
        count=count+1; assert(tonumber(key)==id,'completion recorded an old or different unit')
      end end
      assert(count==1,'training did not create exactly one new assassin')
      return TrainedUnit(owner,id)
    end; "#;

const EQUALITY: &str = r#"local function Same(a,b)
      if type(a)~=type(b) then return false end
      if type(a)~='table' then return a==b end
      for k,v in pairs(a) do if not Same(v,b[k]) then return false end end
      for k,v in pairs(b) do if not Same(v,a[k]) then return false end end
      return true
    end; "#;

const TRACE: &str = r#"if t and t.LockState then
          TrainedSnapshot(t.Actor,t.LockState.LastProducedUnit)
          local id=R.State(t.Actor).LastProducedUnit
          if id and id~=t.LockState.LastProducedUnit then TrainedSnapshot(t.Actor,id) end
        end; "#;

pub fn ui(code: &str, state: &str) -> Value {
    json!({"action":"ui_lua","state_name":state,"code":code})
}

pub fn active() -> Value {
    json!({
        "action":"wait_for",
        "timeout":600,
        "condition":{
            "type":"lua",
            "state_name":"InGame",
            "code":r#"local p=Players[Game.GetLocalPlayer()]
        if not p or not p:IsTurnActive() then return false end
        if not JAC_ASN_TRACE then
          JAC_ASN_TRACE=true
          Events.UnitDamageChanged.Add(function(owner,id,damage)
            local t=Game:GetProperty('JAC_ASN_LIVE_TEST')
            if t and owner==t.Actor then print('[JAC_ASN_TRACE] damage turn='..Game.GetCurrentGameTurn()..' id='..id..' damage='..damage) end
          end)
          Events.UnitRemovedFromMap.Add(function(owner,id)
            local t=Game:GetProperty('JAC_ASN_LIVE_TEST')
            if t and owner==t.Actor then print('[JAC_ASN_TRACE] removed turn='..Game.GetCurrentGameTurn()..' id='..id) end
          end)
        end
        return true"#
        }
    })
}

fn check(steps: &mut Vec<Value>, code: &str, state: &str) {
    let flattened = code.lines().map(str::trim).collect::<Vec<_>>().join(" ");
    let code = [PREFIX, GRANT, EQUALITY, TRACE, &flattened].concat();
    steps.push(ui(&code, state));
}

fn advance(steps: &mut Vec<Value>, turns: i64) {
    steps.push(json!({
        "action":"end_turn",
        "turns":turns,
        "timeout":600_i64.max(turns * 240)
    }));
    steps.push(active());
}

fn round_trip(steps: &mut Vec<Value>, name: &str) {
    steps.extend([
        json!({"action":"save","name":name,"timeout":60}),
        json!({"action":"exit_to_menu","timeout":60}),
        json!({"action":"load","name":name,"timeout":300}),
        active(),
    ]);
}

pub fn build_scenario(repo: &Repository) -> Result<Value> {
    let source = repo.json(BASE_SCENARIO)?;
    let source_steps = source["steps"]
        .as_array()
        .context("base scenario steps must be an array")?;
    ensure!(
        source_steps.len() >= 2,
        "base scenario requires enable and configure steps"
    );
    let mut enable = source_steps[0].clone();
    let ids = enable["ids"]
        .as_array_mut()
        .context("enable step ids must be an array")?;
    ids.retain(|id| id.as_str() != Some("4785cfe4-8d9a-4deb-89ea-d2d566551fec"));
    ids.push(Value::String(MOD_ID.to_owned()));
    let disabled = enable["disable_ids"]
        .as_array_mut()
        .context("enable step disable_ids must be an array")?;
    disabled.extend([
        Value::String("4785cfe4-8d9a-4deb-89ea-d2d566551fec".to_owned()),
        Value::String("57a3af09-fb38-41da-b367-818de11a995c".to_owned()),
    ]);
    let mut configure = source_steps[1].clone();
    configure["seed"] = json!(620995);
    for player in configure["players"]
        .as_array_mut()
        .context("configure step players must be an array")?
    {
        let human = player["civ"]
            .as_str()
            .context("player civ must be a string")?
            == "CIVILIZATION_JAC_ODA";
        player
            .as_object_mut()
            .context("player must be an object")?
            .insert("human".to_owned(), Value::Bool(human));
    }
    let mut steps = vec![
        enable,
        configure,
        json!({"action":"start_game","timeout":300}),
        active(),
    ];

    check(
        &mut steps,
        r#"
      Game:SetProperty(R.TEST,1)
      local houses={}; for _,p in ipairs(PlayerManager.GetAliveMajors()) do
        houses[PlayerConfigurations[p:GetID()]:GetCivilizationTypeName()]=p:GetID()
      end
      local a,b=houses.CIVILIZATION_JAC_ODA,houses.CIVILIZATION_JAC_MATSUDAIRA
      assert(a and b,'test houses missing'); assert(not Players[b]:IsHuman(),'target must be AI')
      local city=Players[b]:GetCities():GetCapitalCity(); local u=Grant(a,R.UNIT,city)
      assert(PlayersVisibility[a]:IsRevealed(city:GetX(),city:GetY()),'fixture capital not revealed')
      local gp=Grant(b,'UNIT_GREAT_GENERAL',city)
      Players[b]:SetProperty(R.INPUT,{Successors={{Identity='JAC_LIVE_FREE_PERSON',Kind='GREAT_PERSON',Unit=gp:GetID()}}})
      local assets={}; for _,c in Players[b]:GetCities():Members() do assets[#assets+1]={X=c:GetX(),Y=c:GetY()} end
      local cities={}; for _,c in Players[a]:GetCities():Members() do cities[#cities+1]=c:GetID() end
      table.sort(cities); assert(#cities>=2,'two training cities required')
      Game:SetProperty(R.TEST,1); Players[a]:SetProperty(R.INPUT,{Security=50})
      Players[a]:GetCities():FindID(cities[1]):SetProperty(R.INPUT,nil)
      Players[a]:SetProperty('JAC_ASN_FORCE_NEXT',{Success=1,Detected=0,Branch='FREE_STATE'})
      Game:SetProperty('JAC_ASN_LIVE_TEST',{Actor=a,Target=b,Unit=u:GetID(),Cities=cities,Houses=houses,
        Start=R.Turn(),FreeAssets=assets,FreePersonUnit=gp:GetID(),LossTarget=houses.CIVILIZATION_JAC_IMAGAWA})
      assert(R.CHANCES.LEADER==1 and R.ProductionTurns({})==100 and R.ProductionTurns({Building=1000})==20)
      assert(R.WaitTurns({})==50 and R.WaitTurns({Security=0,Relation=1,School=1,Governor=1,Hero=1,GreatPerson=1})==1)
      return 'base 100/minimum 20; no-return unit grant found by ID delta'
    "#,
        "GameCore_Tuner",
    );
    check(
        &mut steps,
        r#"assert(Game.GetLocalPlayer()==t.Actor,'wrong human')
      assert(JAC_AssassinProduce(t.Cities[1]),'training not sent')
      assert(JAC_AssassinAttempt(t.Unit,t.Target,'LEADER'),'attempt not sent')
      return 'training and mission sent via EXECUTE_SCRIPT during the active turn'"#,
        "JAC_AssassinPanel",
    );
    steps.push(json!({
        "action":"wait_for",
        "timeout":30,
        "condition":{
            "type":"lua",
            "state_name":"GameCore_Tuner",
            "code":format!("{PREFIX}local s=R.State(t.Actor); return s.Production~=nil and s.Mission~=nil")
        }
    }));
    check(
        &mut steps,
        r#"
      local s=R.State(t.Actor); assert(s.Production.Turns==100 and s.Production.Speed==1,'base training')
      assert(s.Mission.Success and not s.Mission.Detected and s.Mission.Branch=='FREE_STATE','frozen outcome')
      assert(s.CommandUnlock-s.LastCommand==50,'command clock')
      local ok,why=R.StartProduction(t.Actor,t.Cities[2]); assert(not ok and why=='PRODUCING','parallel city')
      local u=Grant(t.Actor,R.UNIT,Players[t.Target]:GetCities():GetCapitalCity())
      local ok,why=R.StartAttempt(t.Actor,u:GetID(),t.Target,'LEADER'); assert(not ok and why=='MISSION_ACTIVE','second unit')
      t.SecondUnit=u:GetID(); t.Frozen=s.Mission; t.CommandUnlock=s.CommandUnlock
      t.TrainingBefore=TrainingUnits(t.Actor)
      Game:SetProperty('JAC_ASN_LIVE_TEST',t)
      Players[t.Actor]:SetProperty(R.INPUT,{Security=100}); return 'separate clocks and saved national plan'
    "#,
        "GameCore_Tuner",
    );
    advance(&mut steps, 1);
    check(
        &mut steps,
        r#"local s=R.State(t.Actor); assert(R.Turn()==t.Start+1,'end_turn must honor turns')
      assert(s.Production.Due==t.Start+120 and s.Production.Turns==120,'current security did not extend training')
      t.Dynamic=s.Production; Game:SetProperty('JAC_ASN_LIVE_TEST',t)
      return 'training extended from 100 to 120 on the next turn'"#,
        "GameCore_Tuner",
    );
    round_trip(&mut steps, "JAC-Assassin-Dynamic");
    check(
        &mut steps,
        r#"
      local s=R.State(t.Actor); assert(Same(s.Production,t.Dynamic),'training lost on load')
      assert(s.Production.Start==t.Start and s.Production.Due==t.Start+120,'training clocks lost')
      assert(Same(s.Mission,t.Frozen),'mission snapshot/SuccessRoll/DetectionRoll/BranchRoll rerolled')
      assert(s.Mission.PostDeathPlan.Policy.FreeState=='ENGINE_FREE_CITIES','plan lost')
      assert(s.CommandUnlock==t.CommandUnlock,'command wait lost')
      Players[t.Actor]:SetProperty(R.INPUT,{Security=100,Relations={[tostring(t.LossTarget)]=1}})
      return 'load retains all training and mission fields; hostility set on a third country'
    "#,
        "GameCore_Tuner",
    );
    advance(&mut steps, 1);
    check(
        &mut steps,
        r#"
      local s=R.State(t.Actor)
      assert(s.Production.Due==t.Start+110 and s.Production.Turns==110,'hostility did not shorten training by 10')
      assert(Same(s.Mission,t.Frozen) and s.CommandUnlock==t.CommandUnlock,'relation changed frozen mission or wait')
      Players[t.Actor]:SetProperty(R.INPUT,{Security=100,Relations={[tostring(t.LossTarget)]=-1}})
      return 'worst living major hostility shortens training to 110; mission and command wait unchanged'
    "#,
        "GameCore_Tuner",
    );
    advance(&mut steps, 1);
    check(
        &mut steps,
        r#"
      local s=R.State(t.Actor)
      assert(s.Production.Due==t.Start+120 and s.Production.Turns==120,'friendly relation did not restore training')
      Players[t.Actor]:SetProperty(R.INPUT,{Security=0,Relations={[tostring(t.LossTarget)]=1}})
      return 'friendly relations restore the 120-turn training deadline'
    "#,
        "GameCore_Tuner",
    );
    advance(&mut steps, 1);
    check(
        &mut steps,
        r#"
      local s=R.State(t.Actor); assert(R.Turn()==t.Start+4,'turn count')
      assert(s.Production.Due==t.Start+70 and s.Production.Turns==70,'security and hostility did not shorten training')
      assert(s.Production and s.Mission and s.LastCompletion==nil,'premature result')
      assert(R.TestAdvanceDeadlines(t.Actor,{Training=true,Mission=true}))
      local accelerated=R.State(t.Actor); t.FastDue=R.Turn()+1; Game:SetProperty('JAC_ASN_LIVE_TEST',t)
      assert(accelerated.Production.Due==t.FastDue and accelerated.Mission.Due==t.FastDue,'test deadlines not saved')
      assert(accelerated.Production.Start==t.Start and accelerated.Production.Turns==70,'hook changed training rule')
      assert(accelerated.CommandUnlock==s.CommandUnlock and accelerated.Mission.SuccessRoll==s.Mission.SuccessRoll,'hook changed outcome/wait')
      return 'normal 70-turn deadline verified; test-only training and mission deadlines shortened'
    "#,
        "GameCore_Tuner",
    );
    advance(&mut steps, 1);
    check(
        &mut steps,
        r#"
      local s=R.State(t.Actor); local n=R.Nation(t.Target)
      assert(R.Turn()==t.FastDue and s.Mission==nil and s.LastMission.Result=='LEADER_FREE_STATE','mission result')
      assert(n.State=='FREE_STATE' and n.LeaderDead==1 and n.DeathTurn==t.FastDue and n.Government=='FREE_CITIES')
      assert(n.Slot=='RETIRED' and n.AI=='FREE_CITIES_ENGINE' and not Players[t.Target]:IsAlive(),'free-city old slot')
      assert(Players[t.Target]:GetCities():GetCount()==0 and Players[t.Target]:GetUnits():GetCount()==0,'old assets remain')
      local free=PlayerManager.GetFreeCitiesPlayerID(); assert(n.FreeCitiesPlayer==free and Players[free]:GetCities():GetCount()>0,'free cities absent')
      for _,c in ipairs(t.FreeAssets) do local actual=CityManager.GetCityAt(c.X,c.Y)
        assert(actual and actual:GetOwner()==free,'city did not become free') end
      assert(n.People.JAC_LIVE_FREE_PERSON.Role=='UNAFFILIATED' and n.People.JAC_LIVE_FREE_PERSON.Unit==nil,'free person not archived')
      assert(n.People.JAC_LIVE_FREE_PERSON.SourceUnit==t.FreePersonUnit,'free person identity lost')
      local ok,why=R.CanCommand(s,R.Turn()); assert(not ok and why=='COMMAND_LOCK','command wait')
      assert(s.Production==nil and s.LastCompletion==t.FastDue and Players[t.Actor]:GetUnits():FindID(s.LastProducedUnit),'training not processed')
      NewTrainedUnit(t.Actor,t.TrainingBefore,s.LastProducedUnit)
      t.LockUnit=ParkTrainedUnit(t.Actor,s.LastProducedUnit,t.Cities[1])
      assert(s.ProductionUnlock==s.LastCompletion+10,'lock origin')
      for _,city in ipairs(t.Cities) do local ok,why=R.StartProduction(t.Actor,city); assert(not ok and why=='PRODUCTION_LOCK') end
      R.CancelProduction(t.Actor); assert(R.State(t.Actor).ProductionUnlock==s.ProductionUnlock,'cancel cleared lock')
      t.Completion=s.LastCompletion; t.Lock=s.ProductionUnlock; t.FreeNation=n; t.LockState=R.State(t.Actor)
      Game:SetProperty('JAC_ASN_LIVE_TEST',t); return 'real turn creates free cities and trained unit; nationwide 10-turn lock'
    "#,
        "GameCore_Tuner",
    );
    round_trip(&mut steps, "JAC-Assassin-Lock");
    check(
        &mut steps,
        r#"
      local s=R.State(t.Actor); assert(Same(s,t.LockState),'lock/state lost')
      TrainedUnit(t.Actor,s.LastProducedUnit)
      assert(Same(TrainedSnapshot(t.Actor,s.LastProducedUnit),t.LockUnit),'load changed first parked individual')
      assert(Same(R.Nation(t.Target),t.FreeNation),'free-city history or people lost')
      assert(not Players[t.Target]:IsAlive(),'load resurrected retired slot')
      for _,c in ipairs(t.FreeAssets) do local actual=CityManager.GetCityAt(c.X,c.Y)
        assert(actual and actual:GetOwner()==t.FreeNation.FreeCitiesPlayer,'load lost free city') end
      local ok,why=R.CanProduce(s,t.Lock-1); assert(not ok and why=='PRODUCTION_LOCK','completion+9 boundary')
      assert(R.CanProduce(s,t.Lock),'completion+10 boundary')
      assert(R.TestAdvanceDeadlines(t.Actor,{ProductionLock=true,CommandLock=true}))
      t.FastLock=R.Turn()+1; Game:SetProperty('JAC_ASN_LIVE_TEST',t)
      for _,city in ipairs(t.Cities) do local ok,why=R.StartProduction(t.Actor,city); assert(not ok and why=='PRODUCTION_LOCK','shortened lock early') end
      return 'saved original +9/+10 boundaries verified; shortened nationwide locks still reject before deadline'
    "#,
        "GameCore_Tuner",
    );
    advance(&mut steps, 1);
    check(
        &mut steps,
        r#"
      assert(R.Turn()==t.FastLock,'wrong shortened unlock boundary')
      t.TrainingBefore=TrainingUnits(t.Actor)
      Players[t.Actor]:GetCities():FindID(t.Cities[2]):SetProperty(R.INPUT,{Building=10000})
      assert(R.StartProduction(t.Actor,t.Cities[2])); local s=R.State(t.Actor)
      t.MinStart=R.Turn(); assert(s.Production.Start==t.MinStart and s.Production.Turns==20 and s.Production.Due==t.MinStart+20,'minimum rule')
      assert(s.LastCompletion==t.Completion,'minimum completed early')
      assert(R.TestAdvanceDeadlines(t.Actor,{Training=true}))
      Game:SetProperty('JAC_ASN_LIVE_TEST',t); return 'nationwide unlock permits other city; normal minimum 20 verified before acceleration'
    "#,
        "GameCore_Tuner",
    );
    advance(&mut steps, 1);
    check(
        &mut steps,
        r#"local s=R.State(t.Actor); assert(s.LastCompletion==t.MinStart+1 and s.Production==nil,'shortened minimum completion')
      assert(s.ProductionUnlock==s.LastCompletion+10,'second nationwide lock')
      assert(Players[t.Actor]:GetUnits():FindID(s.LastProducedUnit),'second trained unit absent')
      assert(s.LastProducedUnit~=t.LockState.LastProducedUnit,'two trainings reused one unit')
      NewTrainedUnit(t.Actor,t.TrainingBefore,s.LastProducedUnit)
      ParkTrainedUnit(t.Actor,s.LastProducedUnit,t.Cities[2])
      Players[t.Actor]:GetCities():FindID(t.Cities[2]):SetProperty(R.INPUT,nil)
      return 'real turn completes second training and starts a new full 10-turn lock'"#,
        "GameCore_Tuner",
    );

    for (branch, house) in [
        ("SUCCESSION", "IMAGAWA"),
        ("ABSORPTION", "ASAKURA"),
        ("CONGRESS", "ASHIKAGA"),
    ] {
        let mut code = r#"
          local b=t.Houses.CIVILIZATION_JAC_@HOUSE@; assert(b and Players[b]:IsAlive() and not Players[b]:IsHuman(),'branch target')
          local city=Players[b]:GetCities():GetCapitalCity(); assert(city,'capital absent')
          local u=Grant(t.Actor,R.UNIT,city); local input=R.Input(Players[b])
        "#.replace("@HOUSE@", house);
        if branch == "SUCCESSION" {
            code.push_str(
                r#"
          local gp=Grant(b,'UNIT_GREAT_GENERAL',city)
          input.Successors={{Identity='JAC_LIVE_HEIR',Kind='GREAT_PERSON',Unit=gp:GetID(),Kin=0}}
          t.HeirUnit=gp:GetID()
        "#,
            );
        }
        if branch == "ABSORPTION" {
            code.push_str(
                r#"
          input.Successor={Identity='JAC_LIVE_ABSORBED_PERSON',Kind='PERSON'}
        "#,
            );
        }
        if branch == "CONGRESS" {
            code.push_str(r#"
          local votes={}; t.FavorBefore={}; local count=0
          for _,p in ipairs(PlayerManager.GetAliveMajors()) do
            local id=p:GetID(); local diplo=p:GetDiplomacy(); local favor=diplo:GetFavor()
            t.FavorBefore[tostring(id)]=favor; diplo:ChangeFavor(-favor)
            assert(diplo:GetFavor()==0,'favor reset'); votes[tostring(id)]='FREE_STATE'; count=count+1
          end
          assert(count>2,'favor fixture requires opposing majority')
          Players[t.Actor]:GetDiplomacy():ChangeFavor(100000); votes[tostring(t.Actor)]='SUCCESSION'
          input.Successor={Identity='JAC_LIVE_VOTE_HEIR',Kind='PERSON'}
          input.Congress={Enabled=true,Votes=votes}
        "#);
        }
        code.push_str(&r#"
          Players[b]:SetProperty(R.INPUT,input)
          local s=R.State(t.Actor); assert(s.Mission==nil and R.CanCommand(s,R.Turn()),'branch wait not expired')
          Players[t.Actor]:SetProperty('JAC_ASN_FORCE_NEXT',{Success=1,Detected=0,Branch='@BRANCH@'})
          assert(R.StartAttempt(t.Actor,u:GetID(),b,'LEADER')); local m=R.State(t.Actor).Mission
          local distance=Map.GetPlotDistance(u:GetX(),u:GetY(),city:GetX(),city:GetY())
          assert(m.Due==R.Turn()+(distance<=1 and 5 or distance<=3 and 8 or 10),'leader mission distance rule')
          t.BranchTarget=b; t.BranchPlan=m.PostDeathPlan; t.BranchSnapshot=m.Snapshot
          t.BranchAssets={}; for _,c in Players[b]:GetCities():Members() do t.BranchAssets[#t.BranchAssets+1]={X=c:GetX(),Y=c:GetY()} end
        "#.replace("@BRANCH@", branch));
        if branch == "ABSORPTION" {
            code.push_str(r#"
          local x=m.Snapshot.Selection; local best,score
          for _,n in ipairs(m.Snapshot.Neighbors) do
            assert(n.Strength==n.Cities+n.Military/x.MilitaryPerCity,'strength input')
            local f={Strength=n.Strength/math.max(1,x.MaxStrength),Friendship=n.Friendship,
              Border=n.Border and n.Border/math.max(1,x.MaxBorder),Distance=math.max(0,1-n.Distance/x.NeighborDistance),
              Religion=n.Religion,Culture=n.Culture}
            local sum,total=0,0
            for _,key in ipairs({'Strength','Friendship','Border','Distance','Religion','Culture'}) do
              if f[key]~=nil then sum=sum+x.Weights[key]*f[key]; total=total+x.Weights[key] end
            end
            local actual=sum/total; assert(math.abs(actual-n.Score)<0.000001,'receiver formula')
            if not score or actual>score or (actual==score and n.Player<best) then best,score=n.Player,actual end
          end
          assert(best and m.PostDeathPlan.Recipient==best,'receiver choice not maximum score')
        "#);
        }
        if branch == "CONGRESS" {
            code.push_str(r#"
          local plan=m.PostDeathPlan; local ballot=plan.Ballots[tostring(t.Actor)]
          assert(ballot.Favor==100000 and ballot.Weight>plan.Tally.FREE_STATE,'favor did not outweigh majority')
          assert(plan.Tally.SUCCESSION==ballot.Weight and plan.Resolution=='SUCCESSION' and plan.Voting=='FAVOR_WEIGHTED_EQUIVALENT','favor ballot')
          Players[t.Actor]:GetDiplomacy():ChangeFavor(-100000)
          input.Congress.Votes={}; Players[b]:SetProperty(R.INPUT,input)
        "#);
        }
        code.push_str(&r#"
          assert(R.TestAdvanceDeadlines(t.Actor,{Mission=true,CommandLock=true})); t.BranchDue=R.Turn()+1
          assert(Same(R.State(t.Actor).Mission.PostDeathPlan,t.BranchPlan),'hook changed plan')
          Game:SetProperty('JAC_ASN_LIVE_TEST',t); return '@BRANCH@ plan frozen; real next turn resolves it'
        "#.replace("@BRANCH@", branch));
        check(&mut steps, &code, "GameCore_Tuner");
        advance(&mut steps, 1);
        let mut code = r#"
          local s=R.State(t.Actor); local n=R.Nation(t.BranchTarget)
          assert(s.Mission==nil and n and n.Branch=='@BRANCH@' and n.DecisionRequired==nil,'branch unresolved')
          assert(n.DeathTurn==t.BranchDue and Same(n.Plan,t.BranchPlan) and Same(n.Snapshot,t.BranchSnapshot),'frozen plan changed')
        "#.replace("@BRANCH@", branch);
        if branch == "SUCCESSION" {
            code.push_str(r#"
          assert(n.State=='SUCCESSION' and n.CurrentLeader=='JAC_LIVE_HEIR' and n.Slot=='RETAINED')
          assert(Players[t.BranchTarget]:GetUnits():FindID(t.HeirUnit)==nil,'GP occupies two roles')
          assert(R.ResolveTarget(t.BranchTarget,'LEADER').Identity=='JAC_LIVE_HEIR','successor not targetable')
        "#);
        }
        if branch == "ABSORPTION" {
            code.push_str(r#"
          assert(n.State=='ABSORPTION' and n.Recipient==t.BranchPlan.Recipient,'wrong recipient')
          assert(Players[t.BranchTarget]:GetCities():GetCount()==0 and Players[t.BranchTarget]:GetUnits():GetCount()==0,'old assets remain')
          assert(not Players[t.BranchTarget]:IsAlive(),'empty native slot still active')
          for _,c in ipairs(t.BranchAssets) do local actual=CityManager.GetCityAt(c.X,c.Y)
            assert(actual and actual:GetOwner()==n.Recipient,'city transfer failed') end
          assert(Players[n.Recipient]:GetProperty(R.PEOPLE).JAC_LIVE_ABSORBED_PERSON.Role=='MEMBER','living person not joined')
          assert(n.Slot=='RETIRED' and n.AI=='RECIPIENT_SLOT','old AI retained')
        "#);
        }
        if branch == "CONGRESS" {
            code.push_str(r#"
          assert(n.State=='SUCCESSION' and n.Plan.Resolution=='SUCCESSION' and n.CurrentLeader=='JAC_LIVE_VOTE_HEIR','favor/ballot changed')
          assert(n.Plan.Tally.SUCCESSION>n.Plan.Tally.FREE_STATE and Players[t.BranchTarget]:IsAlive(),'favor-weighted vote failed')
          for id,favor in pairs(t.FavorBefore) do local diplo=Players[tonumber(id)]:GetDiplomacy()
            diplo:ChangeFavor(favor-diplo:GetFavor()) end
        "#);
        }
        code.push_str(&format!(
            "return '{branch} city/unit/leader/slot consequences verified'"
        ));
        check(&mut steps, &code, "GameCore_Tuner");
    }

    for (index, (success, detected)) in [(1, 0), (1, 1), (0, 0), (0, 1)].into_iter().enumerate() {
        if index != 0 {
            check(
                &mut steps,
                r#"assert(R.TestAdvanceDeadlines(t.Actor,{CommandLock=true}))
              t.LossWait=R.Turn()+1; Game:SetProperty('JAC_ASN_LIVE_TEST',t)
              local ok,why=R.CanCommand(R.State(t.Actor),R.Turn()); assert(not ok and why=='COMMAND_LOCK')
              return 'next fixture waits one real turn for shortened nationwide command lock'"#,
                "GameCore_Tuner",
            );
            advance(&mut steps, 1);
        }
        let code = r#"
          local b=t.LossTarget; local city=Players[b]:GetCities():GetCapitalCity(); assert(city,'loss-test capital')
          local victim=Grant(b,'UNIT_WARRIOR',city); local u=Grant(t.Actor,R.UNIT,victim)
          local s=R.State(t.Actor); assert(s.Mission==nil and R.CanCommand(s,R.Turn()),'loss command wait')
          Players[t.Actor]:SetProperty('JAC_ASN_FORCE_NEXT',{Success=@SUCCESS@,Detected=@DETECTED@})
          FixtureReveal(t.Actor,victim:GetX(),victim:GetY())
          assert(R.StartAttempt(t.Actor,u:GetID(),b,'UNIT',victim:GetID()))
          local m=R.State(t.Actor).Mission; local ledger=Players[b]:GetProperty('JAC_ASN_DIPLOMACY_V1') or {}
          local distance=Map.GetPlotDistance(u:GetX(),u:GetY(),victim:GetX(),victim:GetY())
          assert(m.Due==R.Turn()+(distance<=1 and 2 or distance<=3 and 3 or 4),'unit mission distance rule')
          t.LossMission=m; t.LossVictim=victim:GetID(); t.LossGrievances=(ledger.Grievances or {})[tostring(t.Actor)] or 0
          t.LossCommandUnlock=R.State(t.Actor).CommandUnlock
          t.LossFriendship=(ledger.Friendship or {})[tostring(t.Actor)] or 0
          UnitManager.Kill(u); assert(R.TestAdvanceDeadlines(t.Actor,{Mission=true}))
          assert(R.State(t.Actor).CommandUnlock==t.LossCommandUnlock,'hook changed lost-actor wait')
          Game:SetProperty('JAC_ASN_LIVE_TEST',t)
          return 'lost-actor fixture with success=@SUCCESS@, detected=@DETECTED@'
        "#
            .replace("@SUCCESS@", &success.to_string())
            .replace("@DETECTED@", &detected.to_string());
        check(&mut steps, &code, "GameCore_Tuner");
        advance(&mut steps, 1);
        check(
            &mut steps,
            r#"
          local s=R.State(t.Actor); local ledger=Players[t.LossTarget]:GetProperty('JAC_ASN_DIPLOMACY_V1')
          assert(s.LastMission.ID==t.LossMission.ID and s.LastMission.Result=='ACTOR_GONE' and s.LastMission.Cancelled,'ghost result')
          assert(Players[t.LossTarget]:GetUnits():FindID(t.LossVictim),'ghost success killed target')
          assert(ledger.Grievances[tostring(t.Actor)]==t.LossGrievances+25 and ledger.Friendship[tostring(t.Actor)]==t.LossFriendship-10,'fixed penalty')
          assert((ledger.WarReason or {})[tostring(t.Actor)]==nil,'ghost detection')
          for _,p in ipairs(PlayerManager.GetAliveMajors()) do assert(p:GetProperty('JAC_ASN_DENOUNCE_'..t.Actor)==nil,'ghost denunciation') end
          local before=ledger.Grievances[tostring(t.Actor)]; R.Tick()
          assert(Players[t.LossTarget]:GetProperty('JAC_ASN_DIPLOMACY_V1').Grievances[tostring(t.Actor)]==before,'duplicate penalty')
          assert(s.CommandUnlock==t.LossCommandUnlock,'wait changed')
          return 'all outcomes cancelled; fixed penalty once; original nationwide wait retained'
        "#,
            "GameCore_Tuner",
        );
    }
    check(
        &mut steps,
        r#"
      local ledger=Players[t.LossTarget]:GetProperty('JAC_ASN_DIPLOMACY_V1')
      local s=R.State(t.Actor); assert(s.LastMission.Result=='ACTOR_GONE')
      t.FinalGrievances=ledger.Grievances[tostring(t.Actor)]; t.FinalFriendship=ledger.Friendship[tostring(t.Actor)]
      t.FinalApplied=ledger.Applied; t.FinalMissionID=s.LastMission.ID; t.FinalWait=s.CommandUnlock
      t.FinalNations=Game:GetProperty(R.NATIONS); t.FinalState=s
      t.FinalUnits={}
      for _,id in ipairs({t.LockState.LastProducedUnit,s.LastProducedUnit}) do
        t.FinalUnits[tostring(id)]=TrainedSnapshot(t.Actor,id); TrainedUnit(t.Actor,id)
      end
      Game:SetProperty('JAC_ASN_LIVE_TEST',t); return 'full national histories, state, penalties and applied missions recorded before save'
    "#,
        "GameCore_Tuner",
    );
    round_trip(&mut steps, "JAC-Assassin-Outcomes");
    check(
        &mut steps,
        r#"
      assert(Same(Game:GetProperty(R.NATIONS),t.FinalNations),'load changed national histories/people/plans')
      local ledger=Players[t.LossTarget]:GetProperty('JAC_ASN_DIPLOMACY_V1')
      assert(ledger.Grievances[tostring(t.Actor)]==t.FinalGrievances and ledger.Friendship[tostring(t.Actor)]==t.FinalFriendship,'load changed penalties')
      for id,applied in pairs(t.FinalApplied) do assert(ledger.Applied[id]==applied,'load lost applied mission') end
      for id,applied in pairs(ledger.Applied) do assert(t.FinalApplied[id]==applied,'load added applied mission') end
      local s=R.State(t.Actor); assert(Same(s,t.FinalState),'load changed state')
      TrainedUnit(t.Actor,t.LockState.LastProducedUnit); TrainedUnit(t.Actor,s.LastProducedUnit)
      for key,row in pairs(t.FinalUnits) do
        assert(Same(TrainedSnapshot(t.Actor,tonumber(key)),row),'load changed trained individual: '..key)
      end
      assert(s.LastMission.ID==t.FinalMissionID and s.LastMission.Result=='ACTOR_GONE' and s.CommandUnlock==t.FinalWait,'load changed result or wait')
      R.Tick(); return 'all branch histories, cancelled results and pre-save penalties survive load'
    "#,
        "GameCore_Tuner",
    );
    check(
        &mut steps,
        r#"
      assert(Game:GetProperty(R.TEST)==1 and Game.GetLocalPlayer()==t.Actor,'unit watch requires the test human')
      local watch={Owner=t.Actor,Rows={}}
      for _,id in ipairs({t.LockState.LastProducedUnit,R.State(t.Actor).LastProducedUnit}) do
        local u=TrainedUnit(t.Actor,id)
        watch.Rows[tostring(id)]={Type=u:GetType(),X=u:GetX(),Y=u:GetY(),MaxDamage=u:GetMaxDamage(),
          InitialDamage=u:GetDamage(),Damage=u:GetDamage(),DamageChanged=false,Removed=false}
      end
      JAC_ASN_LIVE_WATCH=watch
      JAC_ASN_LIVE_DAMAGE=function(owner,id,damage)
        local row=owner==watch.Owner and watch.Rows[tostring(id)]
        if row then
          row.Damage=damage; row.DamageChanged=row.DamageChanged or damage~=row.InitialDamage
          print('[JAC_ASN_FIXTURE] trained damage id='..id..' damage='..damage)
        end
      end
      JAC_ASN_LIVE_REMOVED=function(owner,id)
        local row=owner==watch.Owner and watch.Rows[tostring(id)]
        if row then row.Removed=true; print('[JAC_ASN_FIXTURE] trained removed id='..id) end
      end
      Events.UnitDamageChanged.Add(JAC_ASN_LIVE_DAMAGE)
      Events.UnitRemovedFromMap.Add(JAC_ASN_LIVE_REMOVED)
      return 'both trained individuals survive load; native damage/removal watch installed for next turn'
    "#,
        "JAC_AssassinPanel",
    );
    check(
        &mut steps,
        r#"
      assert(Game:GetProperty(R.TEST)==1,'native removal fixture requires test mode')
      local id=t.LockState.LastProducedUnit; local u=TrainedUnit(t.Actor,id)
      u:ChangeDamage(u:GetMaxDamage()-u:GetDamage())
      local remaining=Players[t.Actor]:GetUnits():FindID(id)
      if remaining then UnitManager.Kill(remaining) end
      assert(not Players[t.Actor]:GetUnits():FindID(id),'native removal fixture did not remove the individual')
      t.ExpectedNativeDeath=id; Game:SetProperty('JAC_ASN_LIVE_TEST',t)
      return 'fixture applies native lethal damage and removal to the first trained individual after save/load'
    "#,
        "GameCore_Tuner",
    );
    advance(&mut steps, 1);
    check(
        &mut steps,
        r#"
      local watch=JAC_ASN_LIVE_WATCH
      local ok,why=pcall(function()
      assert(watch and watch.Owner==t.Actor,'unit watch missing')
      for key,row in pairs(watch.Rows) do
        local u=Players[watch.Owner]:GetUnits():FindID(tonumber(key))
        if tonumber(key)==t.ExpectedNativeDeath then
          assert(not u and row.Removed and row.Damage>=row.MaxDamage,'trained native damage/removal fixture not observed')
          print('[JAC_ASN_FIXTURE] trained native death confirmed id='..key)
        else
          assert(u and not row.Removed and u:GetType()==row.Type and R.IsAssassin(u) and
            u:GetX()==row.X and u:GetY()==row.Y and u:GetDamage()==row.InitialDamage and
            not row.DamageChanged and row.Damage==row.InitialDamage,
            'surviving trained individual changed after native fixture')
        end
      end
      end)
      if JAC_ASN_LIVE_DAMAGE then Events.UnitDamageChanged.Remove(JAC_ASN_LIVE_DAMAGE) end
      if JAC_ASN_LIVE_REMOVED then Events.UnitRemovedFromMap.Remove(JAC_ASN_LIVE_REMOVED) end
      JAC_ASN_LIVE_WATCH=nil; JAC_ASN_LIVE_DAMAGE=nil; JAC_ASN_LIVE_REMOVED=nil
      assert(ok,why)
      return 'first trained individual has witnessed native lethal damage and removal; second remains unchanged'
    "#,
        "JAC_AssassinPanel",
    );
    check(
        &mut steps,
        r#"
      assert(Same(Game:GetProperty(R.NATIONS),t.FinalNations),'next turn changed national histories')
      local ledger=Players[t.LossTarget]:GetProperty('JAC_ASN_DIPLOMACY_V1')
      assert(ledger.Grievances[tostring(t.Actor)]==t.FinalGrievances and ledger.Friendship[tostring(t.Actor)]==t.FinalFriendship,'next turn repeated penalties')
      for id,applied in pairs(t.FinalApplied) do assert(ledger.Applied[id]==applied,'next turn lost applied mission') end
      for id,applied in pairs(ledger.Applied) do assert(t.FinalApplied[id]==applied,'next turn added applied mission') end
      local s=R.State(t.Actor); assert(Same(s,t.FinalState),'next turn changed state')
      assert(s.LastMission.ID==t.FinalMissionID and s.CommandUnlock==t.FinalWait,'next turn changed result or wait')
      assert(not Players[t.Target]:IsAlive() and not Players[t.Houses.CIVILIZATION_JAC_ASAKURA]:IsAlive(),'retired slot resurrected')
      assert(R.Turn()-t.Start<=20,'scenario exceeded live turn budget')
      Game:SetProperty(R.TEST,nil); Players[t.Actor]:SetProperty(R.INPUT,nil)
      local ok,why=R.TestAdvanceDeadlines(t.Actor); assert(not ok and why=='TEST_MODE_OFF','hook active outside test mode')
      assert(Same(R.State(t.Actor),s),'disabled hook changed saved state')
      return 'next turn cannot repeat effects; test mode disabled; hook inert; at most 20 end_turns'
    "#,
        "GameCore_Tuner",
    );

    let turns: i64 = steps
        .iter()
        .filter(|step| step["action"] == "end_turn")
        .map(|step| step["turns"].as_i64().unwrap_or(1))
        .sum();
    ensure!(turns == 18, "unexpected live turn budget: {turns}");
    Ok(json!({
        "name":"Assassin: 18 live turns, dynamic 100/120/110/120/70/20 training with diplomacy, decided death outcomes, favor vote, lost actors, three save/load cycles",
        "judge_profile":"japan_map",
        "steps":steps
    }))
}

pub fn scenarios(repo: &Repository) -> Result<Outputs> {
    let content = serde_json::to_string_pretty(&build_scenario(repo)?)? + "\n";
    Ok([(PathBuf::from(SCENARIO), content)].into_iter().collect())
}

pub fn outputs(repo: &Repository) -> Result<Outputs> {
    let mut output = packaging();
    output.extend(scenarios(repo)?);
    // InGame.LoadNewContext creates every add-in context hidden. Emit only
    // this engine initialization; keep the existing panel implementation.
    output.insert(
        PathBuf::from(MOD).join("UI/JAC_AssassinPanel.lua"),
        panel_initializer::initialize(PANEL_TEMPLATE),
    );
    Ok(output)
}

pub fn cli(repo: &Repository, _args: &[String]) -> Result<()> {
    // The original entry point ignores command-line arguments and prints nothing.
    apply_outputs(repo, outputs(repo)?, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn packaging() -> Result<()> {
        let repo = Repository::checked_in();
        let generated = super::packaging();
        assert_eq!(generated.len(), 3);
        for (path, content) in &generated {
            assert_eq!(
                &repo.read(path)?,
                content,
                "packaging mismatch: {}",
                path.display()
            );
            roxmltree::Document::parse(content)
                .with_context(|| format!("parse generated {}", path.display()))?;
        }
        let modinfo = generated
            .get(&PathBuf::from(MOD).join("JAC_PoC_Assassin.modinfo"))
            .context("modinfo output absent")?;
        let document = roxmltree::Document::parse(modinfo)?;
        let root = document.root_element();
        assert_eq!(root.tag_name().name(), "Mod");
        assert_eq!(root.attribute("id"), Some(MOD_ID));
        assert_eq!(root.attribute("version"), Some("1"));
        let dependencies: Vec<_> = root
            .children()
            .find(|node| node.has_tag_name("Dependencies"))
            .context("Dependencies absent")?
            .children()
            .filter(|node| node.has_tag_name("Mod"))
            .collect();
        assert_eq!(dependencies.len(), 5);
        let criteria: Vec<_> = root
            .descendants()
            .filter(|node| node.has_tag_name("Criteria"))
            .collect();
        assert_eq!(criteria.len(), 4);
        for (node, id) in criteria.iter().zip([
            "JAC_Assassin_15",
            "JAC_Assassin_15_Settlers",
            "JAC_Assassin_20",
            "JAC_Assassin_20_Settlers",
        ]) {
            assert_eq!(node.attribute("id"), Some(id));
            assert_eq!(
                node.children()
                    .find(|node| node.has_tag_name("RuleSetInUse"))
                    .and_then(|node| node.text()),
                Some("RULESET_JAC_BANSEIKI")
            );
        }
        let actions = root
            .children()
            .find(|node| node.has_tag_name("InGameActions"))
            .context("InGameActions absent")?;
        assert_eq!(
            actions.children().filter(|node| node.is_element()).count(),
            21
        );
        for action in actions
            .children()
            .filter(|node| node.is_element() && !node.has_tag_name("ImportFiles"))
        {
            assert_eq!(
                action
                    .descendants()
                    .find(|node| node.has_tag_name("LoadOrder"))
                    .and_then(|node| node.text()),
                Some("350")
            );
            assert!(action.attribute("criteria").is_some());
            assert!(action.children().any(|node| node.has_tag_name("File")));
            let context = action
                .descendants()
                .find(|node| node.has_tag_name("Context"));
            if action.has_tag_name("AddUserInterfaces") {
                assert_eq!(context.and_then(|node| node.text()), Some("InGame"));
            } else {
                assert!(context.is_none());
            }
        }
        let text = generated
            .get(&PathBuf::from(MOD).join("Text/JAC_Assassin_Text.xml"))
            .context("localization output absent")?;
        let document = roxmltree::Document::parse(text)?;
        let rows: Vec<_> = document
            .descendants()
            .filter(|node| node.has_tag_name("Row"))
            .collect();
        assert_eq!(rows.len(), TEXT.len() * LANGUAGES.len());
        let mut seen = std::collections::BTreeSet::new();
        for ((tag, values), chunk) in TEXT.iter().zip(rows.chunks_exact(3)) {
            let expected_tag = format!("LOC_JAC_ASN_{tag}");
            for ((language, value), row) in LANGUAGES.iter().zip(values.iter()).zip(chunk) {
                assert_eq!(row.attribute("Tag"), Some(expected_tag.as_str()));
                assert_eq!(row.attribute("Language"), Some(*language));
                assert_eq!(
                    row.children()
                        .find(|node| node.has_tag_name("Text"))
                        .and_then(|node| node.text()),
                    Some(*value)
                );
                assert!(seen.insert((expected_tag.clone(), *language)));
            }
        }
        let icons = generated
            .get(&PathBuf::from(MOD).join("Data/JAC_Assassin_Icons.xml"))
            .context("icons output absent")?;
        let document = roxmltree::Document::parse(icons)?;
        let rows: Vec<_> = document
            .descendants()
            .filter(|node| node.has_tag_name("Row"))
            .collect();
        assert_eq!(rows.len(), 8);
        for (unit, chunk) in ["UNIT_JAC_JAPAN_SENGOKU_ASSASSIN", "UNIT_JAC_ASSASSIN"]
            .into_iter()
            .zip(rows.chunks_exact(4))
        {
            for ((suffix, atlas, index), row) in [
                ("", "ICON_ATLAS_UNITS", "19"),
                ("_FOW", "ICON_ATLAS_UNITS_FOW", "19"),
                ("_PORTRAIT", "ICON_ATLAS_UNIT_PORTRAITS", "17"),
                ("_WHITE", "ICON_ATLAS_UNIT_FLAG_SYMBOLS_WHITE", "19"),
            ]
            .into_iter()
            .zip(chunk)
            {
                let name = format!("ICON_{unit}{suffix}");
                assert_eq!(row.attribute("Name"), Some(name.as_str()));
                assert_eq!(row.attribute("Atlas"), Some(atlas));
                assert_eq!(row.attribute("Index"), Some(index));
            }
        }
        Ok(())
    }

    #[test]
    fn build_scenario() -> Result<()> {
        let repo = Repository::checked_in();
        let generated = super::build_scenario(&repo)?;
        let oracle = repo.json(SCENARIO)?;
        assert_eq!(generated, oracle);
        let steps = generated["steps"].as_array().context("steps absent")?;
        assert_eq!(steps.len(), 88);
        let mut counts = BTreeMap::new();
        for step in steps {
            let action = step["action"].as_str().context("step action absent")?;
            *counts.entry(action).or_insert(0_usize) += 1;
        }
        assert_eq!(
            counts,
            BTreeMap::from([
                ("configure", 1),
                ("enable_mods", 1),
                ("end_turn", 18),
                ("exit_to_menu", 3),
                ("load", 3),
                ("save", 3),
                ("start_game", 1),
                ("ui_lua", 35),
                ("wait_for", 23),
            ])
        );
        let turns: i64 = steps
            .iter()
            .filter(|step| step["action"] == "end_turn")
            .map(|step| step["turns"].as_i64().unwrap_or(1))
            .sum();
        assert_eq!(turns, 18);
        assert_eq!(steps[1]["seed"], 620995);
        for player in steps[1]["players"].as_array().context("players absent")? {
            assert_eq!(
                player["human"].as_bool(),
                Some(player["civ"] == "CIVILIZATION_JAC_ODA")
            );
        }
        let ids = steps[0]["ids"].as_array().context("enabled ids absent")?;
        assert_eq!(ids.last().and_then(Value::as_str), Some(MOD_ID));
        assert!(!ids
            .iter()
            .any(|id| id == "4785cfe4-8d9a-4deb-89ea-d2d566551fec"));
        let disabled = steps[0]["disable_ids"]
            .as_array()
            .context("disabled ids absent")?;
        assert!(disabled
            .iter()
            .any(|id| id == "4785cfe4-8d9a-4deb-89ea-d2d566551fec"));
        assert!(disabled
            .iter()
            .any(|id| id == "57a3af09-fb38-41da-b367-818de11a995c"));

        let saved: Vec<_> = steps
            .iter()
            .filter(|step| step["action"] == "save")
            .map(|step| step["name"].as_str().context("save name absent"))
            .collect::<Result<_>>()?;
        let loaded: Vec<_> = steps
            .iter()
            .filter(|step| step["action"] == "load")
            .map(|step| step["name"].as_str().context("load name absent"))
            .collect::<Result<_>>()?;
        assert_eq!(
            saved,
            [
                "JAC-Assassin-Dynamic",
                "JAC-Assassin-Lock",
                "JAC-Assassin-Outcomes"
            ]
        );
        assert_eq!(saved, loaded);

        for branch in ["SUCCESSION", "ABSORPTION", "CONGRESS"] {
            let command = format!("Branch='{branch}'");
            assert_eq!(
                steps
                    .iter()
                    .filter(|step| step["action"] == "ui_lua")
                    .filter(|step| step["code"]
                        .as_str()
                        .is_some_and(|code| code.contains(&command)))
                    .count(),
                1
            );
            let consequence = format!("{branch} city/unit/leader/slot consequences verified");
            assert_eq!(
                steps
                    .iter()
                    .filter(|step| step["action"] == "ui_lua")
                    .filter(|step| step["code"]
                        .as_str()
                        .is_some_and(|code| code.contains(&consequence)))
                    .count(),
                1
            );
        }
        for (success, detected) in [(1, 0), (1, 1), (0, 0), (0, 1)] {
            let command = format!("{{Success={success},Detected={detected}}}");
            let message = format!("lost-actor fixture with success={success}, detected={detected}");
            let matching: Vec<_> = steps
                .iter()
                .filter(|step| step["action"] == "ui_lua")
                .filter(|step| {
                    step["code"]
                        .as_str()
                        .is_some_and(|code| code.contains(&message))
                })
                .collect();
            assert_eq!(matching.len(), 1);
            assert!(matching[0]["code"]
                .as_str()
                .context("fixture code absent")?
                .contains(&command));
        }
        let final_code = steps.last().context("final step absent")?["code"]
            .as_str()
            .context("final code absent")?;
        assert!(final_code.contains("assert(R.Turn()-t.Start<=20"));
        assert!(final_code.contains("why=='TEST_MODE_OFF'"));
        assert!(final_code.contains("assert(Same(R.State(t.Actor),s)"));
        Ok(())
    }

    #[test]
    fn scenarios() -> Result<()> {
        let repo = Repository::checked_in();
        let generated = super::scenarios(&repo)?;
        assert_eq!(generated.len(), 1);
        let content = generated
            .get(&PathBuf::from(SCENARIO))
            .context("scenario output absent")?;
        assert_eq!(content, &repo.read(SCENARIO)?);
        assert!(content.ends_with('\n'));
        assert!(!content.ends_with("\n\n"));
        assert_eq!(
            serde_json::from_str::<Value>(content)?,
            super::build_scenario(&repo)?
        );
        Ok(())
    }

    #[test]
    fn all_scenario_lua_compiles_as_lua51() -> Result<()> {
        let repo = Repository::checked_in();
        let scenario = super::build_scenario(&repo)?;
        let lua = mlua::Lua::new();
        let version: String = lua.globals().get("_VERSION")?;
        assert_eq!(version, "Lua 5.1");
        let steps = scenario["steps"].as_array().context("steps absent")?;
        let mut compiled = 0;
        for (index, step) in steps.iter().enumerate() {
            let code = if step["action"] == "ui_lua" {
                Some(step["code"].as_str().context("ui_lua code absent")?)
            } else if step["action"] == "wait_for" && step["condition"]["type"] == "lua" {
                Some(
                    step["condition"]["code"]
                        .as_str()
                        .context("wait_for code absent")?,
                )
            } else {
                None
            };
            if let Some(code) = code {
                lua.load(code)
                    .set_name(format!("assassin-scenario-step-{index}"))
                    .into_function()
                    .with_context(|| {
                        format!("Lua 5.1 compilation failed for scenario step {index}")
                    })?;
                compiled += 1;
            }
        }
        assert_eq!(compiled, 58);
        Ok(())
    }

    #[test]
    fn outputs_are_deterministic_and_repository_relative() -> Result<()> {
        let repo = Repository::checked_in();
        let first = super::outputs(&repo)?;
        let second = super::outputs(&repo)?;
        assert_eq!(first, second);
        assert_eq!(first.len(), 4);
        assert_eq!(
            first.keys().cloned().collect::<Vec<_>>(),
            vec![
                PathBuf::from(MOD).join("Data/JAC_Assassin_Icons.xml"),
                PathBuf::from(MOD).join("JAC_PoC_Assassin.modinfo"),
                PathBuf::from(MOD).join("Text/JAC_Assassin_Text.xml"),
                PathBuf::from(SCENARIO),
            ]
        );
        for (path, content) in first {
            assert!(path.is_relative());
            assert!(!path.components().any(|component| matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )));
            assert_eq!(
                repo.read(&path)?,
                content,
                "output mismatch: {}",
                path.display()
            );
        }
        Ok(())
    }
}
