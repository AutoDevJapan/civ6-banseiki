//! Display translations; canonical Japanese rule values are not UI labels.
use crate::xml_escape;
pub const OUTPUT: &str = "poc/JAC_PoC_Absorption/Text/JAC_Absorption_Admission_Text.xml";
const ROWS: &str = r#"READING|要求を読み取り中|Reading requirements|Загрузка условий
INTERRUPTED_CANCEL|開始処理が中断しています。登録の取消し操作が使えます。|Admission is interrupted. Cancellation is available.|Присоединение прервано. Доступна отмена заявки.
UNAVAILABLE|この相手の要求条件を現在表示できません。|This faction's requirements are currently unavailable.|Условия этой державы пока недоступны.
INTERRUPTED|処理が中断しています。保存した内容を保持しています。|Processing is interrupted. Saved records are retained.|Обработка прервана. Сохранённые записи удерживаются.
RETURNED|返却済み|Returned|Возвращено
CONSUMED|消費済み|Consumed|Израсходовано
LEGACY_SETTLED|旧版の返却分を除き精算済み|Settled, excluding legacy refunds|Расчёт завершён с учётом возвратов прежней версии
VERIFYING_SETTLEMENT|費用の精算を確認中|Verifying cost settlement|Проверка расчёта расходов
RETURNING|返却を確認中|Verifying return|Проверка возврата
COLLECTING|預託を確認中|Verifying deposit|Проверка залога
HELD|預託済み|Deposited|Залог внесён
NOT_PAID|未預託|Not deposited|Залог не внесён
GOLD|金|Gold|Золото
FAITH|信仰|Faith|Вера
UNREVIEWED|未確認|Unverified|Не проверено
ENVOYS|使節|Envoys|Послы
REQUIRED_ENVOYS|必要使節|Required envoys|Требуется послов
ESCROW_POLICY|条件達成時に保証金を預け、成立時の費用として消費。成立前の取消し・条件置換では実払分を返却|Deposit when conditions are met; consume as a cost when admission is verified. Refund actual payments for cancellation or changed terms before admission.|Залог вносится после выполнения условий и расходуется при подтверждении присоединения. При отмене или изменении условий до присоединения возвращаются фактические платежи.
CANCEL|登録を取り消す|Cancel registration|Отменить заявку
STARTED|取込みを開始済み|Admission started|Присоединение начато
REGISTER_SMALL|条件達成時の取り込みを登録|Register admission when conditions are met|Заявка на присоединение при выполнении условий
REGISTER_MERGER|条件達成時の合併を登録|Register merger when conditions are met|Заявка на объединение при выполнении условий
CONDITIONS|取り込みの条件|Admission conditions|Условия присоединения
ALL_CONDITIONS|すべての条件と必要使節を満たす|Meet every condition and the envoy requirement|Выполнить все условия и направить послов
SMALL_COUNTRY|小国|Small country|Малая страна
SEND_ENVOY|使節を1人送る|Send one envoy|Отправить одного посла
SEND_ENVOY_HELP|未配属の使節を1人消費して、この小国に送る|Assign one unassigned envoy to this small country|Направить одного свободного посла в эту малую страну
OPEN_SMALL|小国の条件と使節を確認|View small-country conditions and envoys|Условия малой страны и послы
VASSAL|主従関係 {1} / {2}|Vassal relationship {1} / {2}|Отношения вассалитета {1} / {2}
ALLIANCE|他国との同盟 {1}|Alliance with {1}|Союз с {1}
OTHER_COUNTRY|他国|Another country|Другая страна
RESERVE|預託後も保つ金の備蓄|Gold reserve retained after deposit|Резерв золота после внесения залога
DEPOSIT|受け渡しの保証金|Transfer deposit|Залог передачи
IDENTITY|現在の国と指導者|Current country and leader|Текущая страна и правитель
TERMS|提示した条件の版|Published version of the conditions|Опубликованная версия условий
FROZEN_RELATIONS|開始時に照合した関係と第三国の前提|Relationships and third-party prerequisites verified at start|Отношения и условия третьих стран, проверенные при запуске
RELATIONS|既存の関係と第三国の前提|Existing relationships and third-party prerequisites|Текущие отношения и условия третьих стран
METRIC_COMBAT_UNITS|統率する戦闘部隊|Controlled combat units|Подчинённые боевые отряды
METRIC_PEAK_COMBAT|決戦部隊の戦闘力|Battle force strength|Боевая мощь войск
METRIC_ACCESS|相手への外交アクセス|Diplomatic access to the faction|Дипломатический доступ к державе
METRIC_TECHNOLOGIES|習得した技術|Researched technologies|Изученные технологии
METRIC_POPULATION|都市の成長|City growth|Рост городов
METRIC_FAITH_YIELD|毎手番の信仰|Faith per turn|Вера за ход
METRIC_FOOD_YIELD|毎手番の食料|Food per turn|Пища за ход
METRIC_SCIENCE_YIELD|毎手番の科学|Science per turn|Наука за ход
PERSONALITY_BRAVE|勇敢|Brave|Храбрый
PERSONALITY_CALM|冷静|Calm|Хладнокровный
PERSONALITY_CAREFUL|慎重|Careful|Осторожный
PERSONALITY_JOLLY|陽気|Jolly|Жизнерадостный
PERSONALITY_GENTLE|温厚|Gentle|Мягкий
PERSONALITY_BOLD|強気|Bold|Напористый
PERSONALITY_FICKLE|自由|Fickle|Переменчивый
PERSONALITY_LOYAL|忠実|Loyal|Верный
TRAIT_VALIANT|武人|Valiant|Доблестный
TRAIT_FIERCE|鬼神|Fierce|Грозный
TRAIT_ASTUTE|策士|Astute|Хитроумный
TRAIT_SHREWD|魔神|Shrewd|Сверхъестественный
TRAIT_DEVOUT|教者|Devout|Просветитель
TRAIT_ASCETIC|霊神|Ascetic|Духовный
TRAIT_MUNDANE|凡人|Mundane|Обычный
TRAIT_ADEPT|天才|Adept|Гений"#;

pub fn label(japanese: &str, prefix: &str) -> Option<String> {
    ROWS.lines().find_map(|line| {
        let values: Vec<_> = line.splitn(4, '|').collect();
        assert_eq!(values.len(), 4, "complete display translations required");
        (values[0].starts_with(prefix) && values[1] == japanese)
            .then(|| format!("LOC_JAC_ADMISSION_{}", values[0]))
    })
}
/// Translate canonical save values and legacy character IDs to the same
/// locale keys at every screen. No translated text lives in executable Lua.
pub fn lookup_lua() -> String {
    let mut keys = std::collections::BTreeMap::new();
    for line in ROWS.lines() {
        let values: Vec<_> = line.splitn(4, '|').collect();
        if values[0].starts_with("PERSONALITY_") || values[0].starts_with("TRAIT_") {
            let key = format!("LOC_JAC_ADMISSION_{}", values[0]);
            for alias in [
                values[1].to_owned(),
                format!("JAC_{}", values[0]),
                format!("LOC_JAC_{}", values[0]),
            ] {
                assert!(keys.insert(alias, key.clone()).is_none());
            }
        }
    }
    format!("local DisplayLocaleKey = (function()\n  local keys={}\n  return function(value) return keys[value] or value end\nend)()\n",
        super::to_lua(&serde_json::json!(keys)))
}
pub const FLOW_DEPOSIT: &str = r#"local function DepositText()
  local saved = flow and flow.Request and flow.Request.ProductDeposit
  local api = ExposedMembers and ExposedMembers.JAC_AbsorptionProduct
  local ok, view = false, nil
  if flow and api and api.Get then ok, view = pcall(api.Get, flow.From, flow.To) end
  local amounts = ok and view and view.Deposit or saved
  if not amounts or ((amounts.Gold or 0) == 0 and (amounts.Faith or 0) == 0) then return nil end
  local settlement
  for _,row in ipairs(ok and view and view.Rows or {}) do
    if row.Kind == 'PRODUCT_DEPOSIT' then settlement = row.Settlement end
  end
  local tag = settlement == 'CONSUMED' and 'CONSUMED' or settlement == 'LEGACY_SETTLED' and 'LEGACY_SETTLED'
    or settlement == 'RETURNED' and 'RETURNED' or settlement == 'RETURNING' and 'RETURNING'
    or settlement == 'HELD' and 'HELD' or 'VERIFYING_SETTLEMENT'
  local function Text(key) return Locale.Lookup('LOC_JAC_ADMISSION_' .. key) end
  return Text('DEPOSIT') .. ' ' .. Text('GOLD') .. ' ' .. tostring(amounts.Gold or 0)
    .. ' / ' .. Text('FAITH') .. ' ' .. tostring(amounts.Faith or 0) .. ' (' .. Text(tag) .. ')'
    .. '[NEWLINE]' .. Text('ESCROW_POLICY')
end
"#;
pub fn xml() -> String {
    let mut out =
        String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<GameData>\n  <LocalizedText>\n");
    for line in ROWS.lines() {
        let values: Vec<_> = line.splitn(4, '|').collect();
        assert_eq!(values.len(), 4);
        for (language, text) in [
            ("ja_JP", values[1]),
            ("en_US", values[2]),
            ("ru_RU", values[3]),
        ] {
            out.push_str(&format!("    <Row Tag=\"LOC_JAC_ADMISSION_{}\" Language=\"{language}\"><Text>{}</Text></Row>\n",values[0],xml_escape(text)));
        }
    }
    out.push_str("  </LocalizedText>\n</GameData>\n");
    out
}
