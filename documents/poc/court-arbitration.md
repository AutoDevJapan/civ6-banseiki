# 朝廷・将軍の停戦裁定（実装メモ）

SSOT 3.2（2026-09-29）の決定「日本マップでは世界会議の代わりに朝廷・将軍の裁定を作る」の最小実装（停戦裁定）と、実機確認の項目。設計案は [`../research/court-arbitration.md`](../research/court-arbitration.md)、世界会議の無効化は [`japan-no-world-congress.md`](japan-no-world-congress.md)。

MOD：`poc/JAC_PoC_CourtArbitration`（ID `4785cfe4-8d9a-4deb-89ea-d2d566551fec`）。すべての処理は、世界会議を消したのと同じ日本マップ4種の条件（`RULESET_JAC_BANSEIKI` + `MAP_SCRIPT`）でだけ読み込まれる。暦（1573年の判定）に戦国パックの暦を使うため、戦国ツリー・日本マップの MOD に依存する。

## 仕組み

| 項目 | 内容 | SSOT / 設計案 |
|---|---|---|
| 請求 | 戦争中の当事者が、相手との停戦裁定を請求する | 3.2「申請制」 |
| 将軍家の発議 | 足利将軍家（`CIVILIZATION_JAC_ASHIKAGA`）は、人間・AI を問わず、他家どうしの停戦も発議できる | 3.2 |
| 将軍家の3倍 | 将軍家の請求による裁定は、停戦の再宣戦制限のターン数（10 → 30）と、拒んだ勢力の不利益（正当性 -15 → -45、討伐の勅命 10 → 30ターン）が3倍、費用は3分の1（切り上げ：金 34・正当性 2） | 3.2「裁定の力が3倍」。対象はオーナー判断 O-3、この解釈のままとすることはオーナー判断 R-3（2026-09-30） |
| 1573年 | 元亀4年7月（`EV_MUROMACHI_END`）のターン以降、将軍家は他家どうしの発議ができなくなる。勢力は残り、当事者としては他家と同じく等倍で請求できる | 3.2「勢力は残り、裁定の力だけを失う」（オーナー判断 O-4：SSOT どおり） |
| 費用 | 請求者が金 100・正当性 5 を払う（将軍家は3分の1）。外交的支持（Favor）は使わない。正当性が30未満の家は請求できない | 3.2「費用は金と正当性」（オーナー判断 R-1：正当性が Favor の代わり。数値は暫定：O-3） |
| 応否 | 答えるのは、当事者の請求では相手、将軍家の発議では両当事者。人間は画面で答え、届かなければ拒否。AI は暫定規則（下記） | 設計案「マルチプレイ」 |
| 成立 | 全員が応じれば裁くターンの初めにその場で講和し、停戦中（10 または 30 ターン）に戦争になっていれば、次のターンの初めに講和し直す（スクリプトが講和させたターンの中の宣戦だけは1ターン待つ）。講和は `MakePeaceWith(相手, true)` で、エンジンの最短の戦争期間（開戦から10ターン）の中でも効く | 設計案「一定ターン再宣戦を制限」 |
| 拒否 | 拒んだ勢力は正当性 -15 で、10ターンの討伐の勅命の対象になる（将軍家の裁定では3倍）。応じた勢力は正当性 +5（力は掛けない。相手が拒んでも上がる） | 3.2（オーナー判断 R-1・R-2） |
| 正当性 | 家ごとの 0〜100（初期値 50、足利将軍家 70）。毎ターンの初めに +1〜3（基本 +1、本拠を保つ +1、1573年まで将軍家と戦争していない +1）。70以上で全都市の金と文化 +10%、30未満で -10%。30未満の家は請求できず、AI は都市数で相手を上回るだけで拒む | 3.2（オーナー判断 R-1・S） |
| 討伐の勅命 | 期間中、どの家も参加できる（人間は画面のボタン、AI は対象と戦争中か、自分の都市と対象の都市の距離が6以下なら自動）。参加した家は戦争疲弊 -50%。期間中に対象の都市を攻め取るたびに金 200・正当性 +10、対象は正当性 -10。期間が終われば参加と戦争疲弊の軽減も終わる。拒否が重なれば長い方の期限にする | 3.2（オーナー判断 R-2） |
| 件数 | 裁定は全体で1ターンに1件まで（審理中の請求があれば次を受け付けない） | 設計案「詰まり防止」 |

### 流れ

1. 画面上部中央の「朝廷に停戦を請う」ボタン（請求できる組か、答えるべき請求があるときだけ出る）から一覧を開き、組ごとの「裁定を請う」を押す。費用が足りないなどで請求できない組は押せず、理由をツールチップに出す。
2. UI は `UI.RequestPlayerOperation(…, PlayerOperations.EXECUTE_SCRIPT, {OnStart = "JAC_CourtArbitrationRequest", A, B})` を送る。ゲームプレイ側の `GameEvents.JAC_CourtArbitrationRequest` が全マシンで同じ検証（`Scripts/JAC_CourtArbitration_Rules.lua` の `CheckRequest`）をして費用を引き、請求を Game プロパティ `JAC_ARB_PENDING` に置く。答える側に通知する。
3. 人間の答える側は、同じ一覧の「応じる」「拒む」で `JAC_CourtArbitrationRespond` を送る。
4. 次のターンの初め（`GameEvents.OnGameTurnStarted`）に、本拠の記録 → 期限の過ぎた勅命の終了 → 正当性の収入 → 停戦中の組の講和し直し（と、安全網の講和待ちの組の講和） → 裁定 → AI の勅命への参加 → 都市の旗の書き直し、の順に行う。
5. 勅命が出ている間は、同じ一覧に「討伐の勅命：<対象>（<期限>ターンまで）。参加：<家>」の行と「参加する」ボタンが出る。ボタンは `JAC_CourtArbitrationJoin`（`Target` = 対象）を送る。
6. 都市が攻め取られると `GameEvents.CityConquered`（攻略者・元の所有者・都市 ID・座標。公式：`DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:174,248`）で、勅命の参加者が対象の都市を取ったかを判定する。

### エンジンの最短の戦争期間と `MakePeaceWith` の第2引数

エンジンは開戦から `DIPLOMACY_WAR_MIN_TURNS`（10ターン、`Base/Assets/Gameplay/Data/GlobalParameters.xml:260`）の間、引数1つの `MakePeaceWith(相手)` をエラーも出さずに無視する。court-arbitration-05 では、ターン1に宣戦させた長宗我部・アイヌに対し、ターン2のゲームプレイ側（`OnGameTurnStarted`）で呼んでも戦争のままだった。

第2引数に `true` を渡すと、この最短期間を越えてその場で講和する（実機 court-arbitration-06 の `probe-make-peace.json`、ログは `Lua.log` の `[JAC_PEACE_PROBE]`）。

| ターン | 組 | 呼び出し | 呼んだ後の `IsAtWarWith` |
|---|---|---|---|
| 1 | F1 長宗我部・アイヌ（ターン1に宣戦） | `MakePeaceWith(相手)` | true（戦争のまま） |
| 1 | F2 琉球・伊達（ターン1に宣戦） | `MakePeaceWith(相手, true)` | **false（講和）** |
| 1 | F3 蘆名・島津（ターン1に宣戦） | 両方向から `MakePeaceWith(相手)` | true・true |
| 1 | R01 大内・大友（開始時から交戦） | 両方向から `MakePeaceWith(相手)` | true・true |
| 1 | R01 大内・大友 | `MakePeaceWith(相手, true)` | **false（講和）** |
| 2 | F1 | `MakePeaceWith(相手)` | true |
| 11 | F1・F3（開戦から10ターン） | `MakePeaceWith(相手)` | false（講和） |

- 前後に読んだ値：`CanMakePeaceWith` はターン1〜11の開始時点で false、ターン12の R01（ターン1に講和）で true（講和後の最短期間 `DIPLOMACY_PEACE_MIN_TURNS` = 10 と同じ間隔）。`DIPLOMACY_WAR_MIN_TURNS` と `DIPLOMACY_PEACE_MIN_TURNS` はどちらも 10。`Game.GetGameDiplomacy():GetMinPeaceDuration()` と `GetAtWarChangeTurn` は `GameCore_Tuner` では nil（`function expected instead of nil`）。
- 対応：講和は `MakePeaceWith(相手, true)` にした。応諾した裁定は裁くターンにすぐ講和し、停戦はそのターンから数える（将軍家の裁定は30ターン）。停戦中の宣戦は次のターンの初めに講和し直す。
- 安全網：呼んだ後に `IsAtWarWith` で確かめ、戦争のままなら次のターンの初めに試し直す（応諾した裁定は `JAC_ARB_AWAIT_<組>` に停戦のターン数を置き、講和できたターンから停戦を数える。待つ間は同じ組を請求できない：`AWAITING_PEACE`）。実機では起きていないので、起きたら `SAFETY NET` で始まるログを出す。
- 同じターンの強制講和と強制宣戦（court-arbitration-08）：ターン2の初めにスクリプトが `MakePeaceWith(相手, true)` で講和させた組に、同じターン2に `DeclareWarOn(相手, FORMAL_WAR, true)` で宣戦させると、次の1ターンの自動進行でエンジンが止まった（FireTuner が応答せず、Civ6 のメモリが約32 GB まで増えた）。`Lua.log` にターン3の `truce reimposed` が無いので、スクリプトの講和し直しより前、ターン2の処理中に止まっている。通常のプレイでは講和後の最短期間（`DIPLOMACY_PEACE_MIN_TURNS` = 10）の間は宣戦できず、この状態は起きない。
  - シナリオ：停戦破りの確認は講和から最短期間を過ぎたターン13（講和のターン2 + 10 + 1）に宣戦させ、ターン14に講和し直されたことを見る形に変えた。
  - スクリプト：前のターンか同じターンにスクリプトが講和させた組（`JAC_ARB_PEACE_TURN` ≥ いまのターン - 1）が戦争になっていたら、講和し直しを1ターン待つ（`truce breach deferred` のログ）。講和と宣戦を同じターンの前後で往復させないため。強制の宣戦でしか起きない状態なので、通常のプレイには影響しない。
- 公式の根拠：講和の呼び出しは `DLC/PiratesScenario/Scripts/PiratesScenario_StartScript.lua:767`（引数1つ）。第2引数の公式の使用例は無く、上の実機の結果が根拠。

### 調査用シナリオ `probe-make-peace.json`

15 km・史実の領土・足利将軍家が人間で、裁定の MOD を入れる。`GameCore_Tuner`（ゲームプレイ）から、R01（大内・大友、ターン1から交戦）と、新たに宣戦させた3組（F1 長宗我部・アイヌ、F2 琉球・伊達、F3 島津・蘆名）に次を試し、呼ぶたびに `IsAtWarWith` を読む。前後で `CanMakePeaceWith`（両方向）と `GetAtWarChangeTurn` を読む。すべて `pcall` で値を返し、止まらない。ログは `[JAC_PEACE_PROBE]`。結果は上の表。

1. ターン1：`DIPLOMACY_WAR_MIN_TURNS`・`DIPLOMACY_PEACE_MIN_TURNS`・`Game.GetGameDiplomacy():GetMinPeaceDuration()` を読む。F1 は `a:MakePeaceWith(b)`、F2 は `a:MakePeaceWith(b, true)`、F3 は相手側から `b:MakePeaceWith(a)` の後に `a:MakePeaceWith(b)`、R01 は4通り全部。
2. 1ターン進めてターン2：前のターンの呼び出しが後から効いたかを読み、F1（court-arbitration-05 と同じ）と R01（`true` 付き）を試し直す。
3. 9ターン進めてターン11（開戦から10ターン）：F1・F2・F3・R01 をそれぞれ前と同じ形で試す。
4. 1ターン進めてターン12：まだ戦争中の組に `a:MakePeaceWith(b)` と `true` 付きを試す。

### AI の応諾（暫定）

自分の都市数が相手の 1.5 倍を超えるとき（明らかに優勢）だけ拒み、それ以外は応じる。正当性が30未満の AI は、都市数で相手を上回るだけで拒む（R-1「AI が拒みやすい」）。戦争疲弊と軍事力はゲームプレイ側で読めない（`GetStats():GetMilitaryStrength()` は UI の使用例しか無い）ため、都市数で代える。拒否の不利益の見積もり、AI からの請求、将軍家 AI の発議は入れていない（設計案でも「最小実装のあと」）。

### 保存しているもの

| プロパティ | 置き場 | 値 |
|---|---|---|
| `JAC_LEGITIMACY` | Player | 正当性 0〜100（無ければ初期値 50、足利将軍家は 70） |
| `JAC_ARB_HOME` | Player | 本拠（最初に記録した首都）のプロットの番号 |
| `JAC_ARB_OUTLAW_UNTIL` | Player | 討伐の勅命の対象である最後のターン（期限を過ぎたターンの初めに消す） |
| `JAC_ARB_JOINED_<対象のID>` | Player | 1 = その勅命に参加中（勅命の終わりに消す） |
| `JAC_ARB_LEGITIMACY_HIGH` / `JAC_ARB_LEGITIMACY_LOW` | 都市の中心のプロット | 0/1。正当性 70 以上 / 30 未満 |
| `JAC_ARB_SUBJUGATION_ANCHOR` | 都市の中心のプロット | 0/1。勅命に参加中の家の代表の都市（プロット番号が最小）だけ 1 |
| `JAC_ARB_PENDING` | Game | 審理中の請求 `{Requester, A, B, Turn, Power, AnswerA, AnswerB}`。当事者の請求では A が請求者 |
| `JAC_ARB_LAST_REQUEST_TURN` | Game | 最後に請求を受け付けたターン |
| `JAC_ARB_TRUCE_<小さいID>_<大きいID>` | Game | 停戦が続く最後のターン。停戦中の宣戦を講和させるまでは、期限が過ぎても残す |
| `JAC_ARB_AWAIT_<小さいID>_<大きいID>` | Game | 安全網：応諾したのに講和できなかった組。講和した時から数える停戦のターン数（実機では置かれていない） |
| `JAC_ARB_PEACE_TURN_<小さいID>_<大きいID>` | Game | スクリプトが最後に講和させたターン（検証用の記録） |

プロットの旗は毎ターンの初め、参加の受付、都市の攻略のたびに書き直す（冪等）。ターンの途中で建てた都市や正当性の変化は、次の書き直しまで旗に出ない。

ターンはすべてゲーム開始からの数（`Game.GetCurrentGameTurn() - GameConfiguration.GetStartTurn() + 1`）。1573年7月の換算は史実イベントと同じ `TurnForYear`（標準速度でターン120）。

## 正当性の効果と戦争疲弊の軽減

`Data/JAC_CourtArbitration_Legitimacy.sql`（日本マップ4種の条件でだけ読み込む）。条件はすべて都市の中心のプロットの 0/1 の旗で、戦国パックの時代（`poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Ages.sql`）と同じ形。

| 修正値 | 種類 | 条件 | 値 |
|---|---|---|---|
| `JAC_ARB_HIGH_GOLD`・`JAC_ARB_HIGH_CULTURE` | `MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_MODIFIER` | 都市（Subject）の `JAC_ARB_LEGITIMACY_HIGH` ≥ 1 | 金・文化 +10% |
| `JAC_ARB_LOW_GOLD`・`JAC_ARB_LOW_CULTURE` | 同上 | 都市の `JAC_ARB_LEGITIMACY_LOW` ≥ 1 | 金・文化 -10% |
| `JAC_ARB_ATTACH_WAR_WEARINESS` → `JAC_ARB_WAR_WEARINESS` | `MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER` → `MODIFIER_PLAYER_ADJUST_WAR_WEARINESS` | 付けた先の都市（Owner）の `JAC_ARB_SUBJUGATION_ANCHOR` ≥ 1 | -50%（`Overall` = true） |

- 金と文化：オーナー判断 S（2026-09-30、旧 Q73 の②）で、忠誠度 ±2/ターン から変えた。日本マップは忠誠度の安定化として全都市に +100/ターン を与えており（`poc/JAC_PoC_JapanMapScale/Data/JAC_Japan_MapRules.xml`、[PoC の記録](README.md)）、±2 では目に見える差にならないため。全都市の産出の修正値を都市の条件で絞る形は戦国パックの黄金時代（`JAC_AGE_GOLDEN_GOLD`）と同じで、種類は公式の `MONASTICISM_CULTURE_MODIFIER`（`DLC/Expansion1/Data/Expansion1_Policies.xml:279-282`）。黄金時代の生産 +10% は旗の付け外しで実機の産出が変わることを確かめてある（`tools/civ6-autotest/scenarios/sengoku-ages.json`）。
- 戦争疲弊：`MODIFIER_PLAYER_ADJUST_WAR_WEARINESS`（`Base/Assets/Gameplay/Data/Modifiers.xml:84`、定義 `:1205-1207` は `COLLECTION_OWNER`）は家に1つ効く種類なので、全都市に付けて代表の都市だけで効かせる（時代の家ごとの効果と同じ。`MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER` は `Modifiers.xml:780`）。値の形は公式の戒厳令・宣伝（`Base/Assets/Gameplay/Data/Policies.xml:836-837`、`:2919-2927` の `Amount` -25・`Overall` true）に合わせ、倍の -50 を暫定値にした。期間が終われば旗が 0 になり効かなくなる。
- **要実機確認**：代表の都市に付けた家単位の修正値が戦争疲弊に効くか、旗を 0 に戻すと外れるか（時代の家ごとの効果と同じく、実機で効き方を見ていない）。

## 公式の根拠

- UI → ゲームプレイの経路：`DLC/BlackDeathScenario/UI/BlackDeathScenario_PopupHandler.lua:22-28`（`OnStart` に GameEvent 名を入れて `EXECUTE_SCRIPT`）、受け手 `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:66`（`GameEvents.EventPopupChoice.Add`）
- 規則ファイルの共有：`BlackDeathScenario.lua:3`（`include "BlackDeathScenario_Rules"`）と `BlackDeathScenario.modinfo:129-133`（`ImportFiles`）
- 講和：`DLC/PiratesScenario/Scripts/PiratesScenario_StartScript.lua:767`（`MakePeaceWith`）と、その前の確認 `:795`（`CanMakePeaceWith`）
- 最短の戦争期間：`Base/Assets/Gameplay/Data/GlobalParameters.xml:260`（`DIPLOMACY_WAR_MIN_TURNS` = 10）
- 都市の攻略：`DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:174,248`、`DLC/NubiaScenario/Scripts/NubiaScenario.lua:937,969`（`GameEvents.CityConquered`）
- 本拠の首都：`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:1103`（`GetCapitalCity`）、プロット：`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:107`（`Map.GetPlotByIndex`）、距離：`AustraliaScenario.lua:132`（`Map.GetPlotDistance`）
- 金：`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:847`（`ChangeGoldBalance`）、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:404`（ゲームプレイ側の `GetGoldBalance`）
- 都市数：`AustraliaScenario.lua:1198`（`GetCities():GetCount()`）
- 通知文の各言語化：`DLC/PiratesScenario/Scripts/PiratesScenario_GameCore_Script.lua:136-137`（ゲームプレイ側の `Locale.Lookup` に引数）
- 停戦の基本ターン数：`Base/Assets/Gameplay/Data/GlobalParameters.xml:248`（`DIPLOMACY_PEACE_MIN_TURNS` = 10）
- 画面：一覧の枠は `poc/JAC_PoC_GrowthUI/UI/JAC_Notebooks.xml` と同じ形、`ScrollPanel` は `Base/Assets/UI/Popups/ChatPanel.xml:12-13`

### 公式に使用例が無いもの・無い API

- 宣戦の検知：`Events.DiplomacyDeclareWar` は UI 側の使用例しか無い（`Base/Assets/UI/DiplomacyRibbon.lua:925`）。そのため停戦中の戦争は「次のターンの初めに講和し直す」だけで、宣戦した側に不利益は付けない。
- 開戦事由・不満（Grievance）の付与：Lua には読み取り（`GetGrievancesAgainst`、`DLC/Expansion2/UI/Additions/DiplomacyActionView_AllianceRow.lua:46`）しか無く、修正値の効果も不満の生成倍率（`EFFECT_ADJUST_PLAYER_GRIEVANCE_GENERATION`）だけで、特定の相手への開戦事由を足すものが無い。そのため討伐の勅命は、開戦事由ではなく、参加者の戦争疲弊の軽減と攻略の褒賞にした（オーナー判断 R-2）。
- 外交的支持（Favor）：日本マップでは正当性が代わりになったので使わない（R-1）。以前の調べ（court-arbitration-02：ゲームプレイ側の Favor は `Player:GetDiplomacy()` の `GetFavor` / `ChangeFavor`、UI 側は `Player:GetFavor`）は Git の履歴に残る。

## 入れていないもの（後回し）

- 所領安堵：設計案では「2つ目の実装として Loyalty 加算から」。SSOT に効果の数値も対象都市の決め方も無いため、この MVP には入れない。
- 官位授与、第三者（将軍家以外）の仲介、AI からの請求、将軍家 AI の発議、停戦破りの不利益。
- 日本マップで Favor そのものを画面から消すこと（上部の表示などは残る。使い道が無いだけ）。

## テスト

- `tests/test_court_arbitration.py`：全処理が日本マップ4種でだけ読み込まれること、使う文言キーが3言語そろうこと、UI スクリプトが構文として読めること。lupa でゲームプレイ側の Lua をターンごとに動かし、請求と費用、1ターン1件、将軍家の3倍（停戦・拒否の不利益の3倍と費用の3分の1。3分の1ちょうどの残高で払えること）と1573年（ターン120）以降、当事者以外・非交戦の拒否、金の不足と正当性30未満で何も引かれないこと、正当性の初期値（50・70）と収入（本拠・将軍家との戦争・1573年で変わる）と 0〜100 の範囲、応諾の +5、70以上・30未満の都市の旗、正当性の低い AI が拒みやすいこと、AI の応否と拒否の不利益、人間の応否（無回答は拒否、二重回答は無視）、停戦中の戦争の講和し直しと期限切れ、戦争が先に終わった請求の無効を確かめる。
- スタブにはエンジンの最短の戦争期間を入れてある（開戦から10ターンは引数1つの `MakePeaceWith` が何もせず、`true` 付きなら講和する）。最短期間の中でも応諾した裁定がその場で講和し、停戦中の宣戦が次のターンに講和し直されること、スクリプトが講和させたターンの中の宣戦は1ターン待ってから講和し直すことを確かめる。`true` が効かないエンジンを模した場合の安全網（講和待ちと停戦の開始、待つ間の再請求の拒否、待つ間に自分たちで講和した組、停戦中の宣戦を期限の後でも講和させること、`SAFETY NET` のログ）も確かめる。
- 討伐の勅命（`SubjugationOrderTests`）：拒んだ家が対象になり、交戦中の AI がその場で、隣接する AI が次のターンに参加し、人間は `EXECUTE_SCRIPT` で1回だけ参加できること、戦争疲弊の旗が参加者の代表の都市にだけ立ち、期限の次のターンに参加と旗が消えること、期間中に参加者が対象の都市を攻め取ると金 200・正当性 +10（対象は -10）と通知、参加していない家と期限後の攻略には褒賞が無いこと、将軍家の裁定で30ターンになり重なった拒否で期限が縮まないこと。
- 修正値の SQL が、スクリプトの書く旗の名前を読むこと、±2 と戦争疲弊の修正値が共通の特性に付くこと。
- 実機シナリオ `court-arbitration.json` のゲームプレイ側の手順を、スタブの上で順に動かして通ることを確かめる（画面の操作と手番待ちは飛ばす）。`probe-make-peace.json` の各手順がスタブ上でも止まらずに結果を返すことも確かめる。

## 実機確認（メインセッション）

シナリオ：`tools/civ6-autotest/scenarios/court-arbitration.json`（15 km 史実領土、足利将軍家を人間、ほか21勢力 AI）。応諾と拒否の両方を、1回の実行で決まった形で通す。

1. ターン1：大内・大友（R01）が交戦中で、将軍家が裁定の力を持ち、室町幕府の終わりがターン120。正当性は将軍家 70 以上・大内 50 以上。
2. 応諾させる組を選ぶ：将軍家・大内・大友を除き、互いに講和中で都市数が同じ（2以上）の2家を ID の小さい順に探し、ターン1に宣戦させる（`JAC_ARB_TEST_PAIR`）。AI の暫定規則（都市数が相手の 1.5 倍を超えると拒む）では、どちらかがターン1に1都市増やしても両方が応じる。
3. 人間の手番が始まるのを待ち（`IsTurnActive`）、「朝廷に停戦を請う」の一覧を開く（`01-arbitration-panel.png`）。UI からその組の停戦を請求すると、`EXECUTE_SCRIPT` 経由で請求が置かれ、力は3倍で、費用は3分の1（金 -34・正当性 -2）。
   - court-arbitration-03 では、開始直後のターン処理中に送った請求がゲームプレイ側に届かなかった。画面は手番が始まるまで請求・応否のボタンを押せなくし、送らない。
4. 1ターン進めてターン2：両家が応じ（正当性 +5 と収入 +1〜3）、最短の戦争期間の中でもその場で講和している（`JAC_ARB_PEACE_TURN` = 2、停戦はターン32まで＝10×3、講和待ちの記録なし、両家に不利益なし）。大内・大友はまだ交戦中（`02-after-accepted-ruling.png`）。
5. 同じターン2に、UI から大内・大友の停戦を請求する。
6. 1ターン進めてターン3：大内・大友の裁定は大内が拒む（court-arbitration-05・07 と同じ。都市数で優勢）。交戦中のまま、停戦・講和待ちの記録なし、将軍家の裁定なので不利益は3倍で、大内の正当性はターン2の値から 42〜44 下がり（-45 とこのターンの収入）、討伐の勅命がターン33まで（3 + 10 × 3）。大内は30未満で請求できず、全都市に低い正当性の旗、将軍家の都市には高い正当性の旗。大内と交戦中の大友がその場で勅命に加わり、代表の都市に戦争疲弊の旗が立つ。人間の将軍家は勝手に加わらない。応諾した組は講和のまま（`03-after-refused-ruling.png`）。
7. 同じターン3に、手番が始まるのを待ち、開いている AI の外交画面（取引の提案と声明）を閉じてから、画面から将軍家が勅命に参加する（`JAC_JoinSubjugation`）。参加と代表の都市の旗を確かめ、一覧の行を撮る（`03b-subjugation-order.png`）。
8. 10ターン進めてターン13（講和から最短期間 10 ターンを過ぎた後。court-arbitration-08 の停止を避ける）：将軍家と大友が勅命に参加したままであること、応諾した組が講和のまま、`JAC_ARB_PEACE_TURN` = 2・停戦はターン32までであることを確かめ（AI が先に宣戦していれば、ここで止めて失敗にする）、テストのため停戦中に宣戦させる（`DeclareWarOn(相手, FORMAL_WAR, true)`）。
9. 1ターン進めてターン14：停戦中の宣戦は、ターン14の初めにスクリプトが講和させている（`JAC_ARB_PEACE_TURN` = 14、停戦の期限はターン32のまま、`04-after-truce-breach.png`）。テストのため、勅命の期限をターン14に縮める（ターン34まで自動進行させない）。
10. 1ターン進めてターン15：勅命が終わり、参加の記録と代表の都市の旗が消え、もう参加できない（`05-after-order-ended.png`）。
11. 都市の攻略と褒賞は、決まった形で起こせないので実機では確かめない（Tuner の都市の移転は攻略ではなく、`CityConquered` が出ない）。スタブの上のテストで確かめる。
12. 画面の見た目（ボタンの位置が他の UI と重ならないか）と通知の文面は、スクリーンショットで人が確かめる。マルチプレイでの応否は実機で未確認。`Lua.log` に `SAFETY NET` が出ていないことも見る。

費用と拒否の不利益の3倍（オーナー判断 O-3）と、正当性・討伐の勅命（オーナー判断 R-1・R-2）は、このシナリオの期待値を変えた後の実機でまだ確かめていない。

owner-o-03・05 は、手順6の後、手番を待つ間に AI の外交の声明（`TutorialUIRoot: diplo stmt type=-45646777`、`DiplomacyActionView` と `LeaderScene`）が人間に出て、`InGame` が FireTuner に応答しなくなり止まった。この MOD は人間との外交状態を変えない（拒否・勅命・AI の参加は `SetProperty` と通知だけで、宣戦・講和は AI どうしの組とシナリオの強制だけ）。同じ種類の声明は別のシナリオ（sengoku-events-list-02 のターン66）でも出ているので、MOD が引き起こしたものではなく通常の AI の声明とみる（種類の名前は未確認）。ドライバは `InGame` の応答が無いときに外交画面を閉じて待ち直す（[自動テスト](autotest.md)）。

これまでの実機の結果：court-arbitration-07 は通ったが、大内・大友の裁定を大内が拒んだため、応諾の流れは実機で一度も動いていなかった。court-arbitration-06 は `probe-make-peace.json`（上の表）。court-arbitration-08 は応諾の流れ（ターン2の講和）と大内・大友の請求まで通ったが、同じターン2に強制した宣戦の後でエンジンが止まった（上の「同じターンの強制講和と強制宣戦」）。

### まだ実機で確かめていないこと

1. 人間が裁定を請われた側になったときの「応じる」「拒む」ボタン（今のシナリオは人間が請求する側だけ）
2. 審理中の請求や停戦中にセーブ／ロードしたときの状態の復元
3. マルチプレイでの請求・応答の同期
4. 実際の攻略で `GameEvents.CityConquered` の引数（攻略者・元の所有者）が公式の使用例どおりに届くこと
5. 正当性の金と文化の効果（シナリオに旗の付け外しの確認を入れたが未実行）と、勅命の参加者の戦争疲弊の軽減（上の「要実機確認」）

### 人間が回答するシナリオ

`tools/civ6-autotest/scenarios/court-arbitration-human.json` は、人間の足利将軍家が回答する側になる。既存シナリオと同じ15 km・史実領土・22勢力で、承諾と拒否を別の相手との戦争で確かめる。実際に進めるのは合計2ターンだけ。講和した組に同じターンに宣戦し直すことも、期限切れまで何十ターンも待つこともしない。

AI からの請求はまだ実装されていないため、ゲームプレイ側から、通常の当事者の請求と同じ審理中の状態（力1・回答は未設定）と通知をテスト用に置く。相手の金100・正当性5も差し引く。この準備は請求ハンドラや AI の請求を検証するものではない。人間の回答と裁定には本来の処理を使う。

1. 未回答の請求を保存する。保存後に審理中の状態を消してから、メニューに戻って読み込む。請求、正当性、戦争などの記録が保存前と一致することを確かめる。
2. `JAC_CourtArbitrationPanel` を開き、一覧の子コントロールから、翻訳されたボタンの文言で対象を探す。`RowInstance.Action` の「応じる」、または `RowInstance.Action2` の「拒む」を `DoLeftClick()` で押す。回答関数は直接呼ばない。両ボタンのハンドラは UI Lua の `Refresh` 内で登録され、`JAC_RespondArbitration` から `EXECUTE_SCRIPT` を送る。
3. 人間の手番が有効なときに押し、ゲーム側の `AnswerB` と最後に受け取ったイベントを待つ。画面を更新し、回答ボタンが消えたことも確かめる。
4. 回答した審理中の状態を保存する。保存後に回答を逆にしてから、メニューに戻って読み込む。保存した回答（承諾の `true`・拒否の `false`）が戻り、もう一度回答する必要がないことを確かめる。
5. 人間のまま1ターン進め、翌ターンの裁定を確かめる。承諾なら正当性は収入に加えて+5、即時講和と10ターンの停戦になる。拒否なら収入に加えて-15、戦争が続き、人間を対象とする10ターンの討伐の勅命に相手 AI が参加する。テスト用の初期値65・40により、各ケースで高い／低い正当性の都市の旗も確かめる。低い場合は請求できない理由も検査する。
6. 裁定後にも保存、保存後の状態変更、メニュー経由の読込を行う。停戦の期限・講和したターン・正当性・勅命の期限・参加・都市の旗が一致することを確かめる。未回答・回答済み・裁定後を両ケースで調べ、保存と読込は計6回になる。

形式検査、全60個の Lua の構文検査、実際の UI コールバックを使ったスタブ上の手順確認は通った。スタブの保存と読込はメモリ内のコピーであり、Civ6 の保存・読込を検証した結果ではない。都市の旗は検査するが、金・文化の実際の産出や戦争疲弊の効果はこのシナリオでは測定しない。

実機 `arbitration-ui-live-01` は、ゲーム開始・請求の準備・回答ボタンの表示まで成功したが、保存前の状態記録で停止した。Civ6 の未設定プロパティが戻り値を返さないため、直接の文字列化が失敗した。値を引数に束縛してから変換するよう、12個の記録処理を修正した。回帰テストは修正前に36ケースで失敗し、修正後に成功した。両回答と保存復元6回の実機確認はまだ解消していない。

実行時はブランチを push してから `python D:/CC/Tools/civ6-live/live_chain.py feat/arbitration-ui-live:court-arbitration-human.json --test-mode` を使う。実行ごとの結果と残作業は [再開用記録](court-arbitration-human-progress.md) にまとめる。
