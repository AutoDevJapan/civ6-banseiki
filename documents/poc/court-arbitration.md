# 朝廷・将軍の停戦裁定（実装メモ）

SSOT 3.2（2026-09-29）の決定「日本マップでは世界会議の代わりに朝廷・将軍の裁定を作る」の最小実装（停戦裁定）と、実機確認の項目。設計案は [`../research/court-arbitration.md`](../research/court-arbitration.md)、世界会議の無効化は [`japan-no-world-congress.md`](japan-no-world-congress.md)。

MOD：`poc/JAC_PoC_CourtArbitration`（ID `4785cfe4-8d9a-4deb-89ea-d2d566551fec`）。すべての処理は、世界会議を消したのと同じ日本マップ4種の条件（`RULESET_JAC_BANSEIKI` + `MAP_SCRIPT`）でだけ読み込まれる。暦（1573年の判定）に戦国パックの暦を使うため、戦国ツリー・日本マップの MOD に依存する。

## 仕組み

| 項目 | 内容 | SSOT / 設計案 |
|---|---|---|
| 請求 | 戦争中の当事者が、相手との停戦裁定を請求する | 3.2「申請制」 |
| 将軍家の発議 | 足利将軍家（`CIVILIZATION_JAC_ASHIKAGA`）は、人間・AI を問わず、他家どうしの停戦も発議できる | 3.2 |
| 将軍家の3倍 | 将軍家の請求による裁定は、停戦の再宣戦制限のターン数が3倍（10 → 30） | 3.2「裁定の力が3倍」。対象は設計案の仮置き（効力のみ） |
| 1573年 | 元亀4年7月（`EV_MUROMACHI_END`）のターン以降、将軍家は他家どうしの発議ができなくなる。勢力は残り、当事者としては他家と同じく等倍で請求できる | 3.2「勢力は残り、裁定の力だけを失う」（Q60） |
| 費用 | 請求者が外交的支持 20・金 100・正当性 5 を払う | 3.2「Favor に加え金と正当性」（数値は暫定：Q59） |
| 応否 | 答えるのは、当事者の請求では相手、将軍家の発議では両当事者。人間は画面で答え、届かなければ拒否。AI は暫定規則（下記） | 設計案「マルチプレイ」 |
| 成立 | 全員が応じれば講和し、停戦中（10 または 30 ターン）に戦争になっていれば毎ターンの初めに講和し直す | 設計案「一定ターン再宣戦を制限」 |
| 拒否 | 拒んだ勢力は正当性 -15、10ターンの「討伐の大義」を全勢力に通知する | 3.2「正当性の低下や討伐の大義」（Q57・Q58） |
| 件数 | 裁定は全体で1ターンに1件まで（審理中の請求があれば次を受け付けない） | 設計案「詰まり防止」 |

### 流れ

1. 画面上部中央の「朝廷に停戦を請う」ボタン（請求できる組か、答えるべき請求があるときだけ出る）から一覧を開き、組ごとの「裁定を請う」を押す。費用が足りないなどで請求できない組は押せず、理由をツールチップに出す。
2. UI は `UI.RequestPlayerOperation(…, PlayerOperations.EXECUTE_SCRIPT, {OnStart = "JAC_CourtArbitrationRequest", A, B})` を送る。ゲームプレイ側の `GameEvents.JAC_CourtArbitrationRequest` が全マシンで同じ検証（`Scripts/JAC_CourtArbitration_Rules.lua` の `CheckRequest`）をして費用を引き、請求を Game プロパティ `JAC_ARB_PENDING` に置く。答える側に通知する。
3. 人間の答える側は、同じ一覧の「応じる」「拒む」で `JAC_CourtArbitrationRespond` を送る。
4. 次のターンの初め（`GameEvents.OnGameTurnStarted`）に裁く。停戦中の組の講和し直しを先に行う。

### AI の応諾（暫定）

自分の都市数が相手の 1.5 倍を超えるとき（明らかに優勢）だけ拒み、それ以外は応じる。戦争疲弊と軍事力はゲームプレイ側で読めない（`GetStats():GetMilitaryStrength()` は UI の使用例しか無い）ため、都市数で代える。拒否の不利益の見積もり、AI からの請求、将軍家 AI の発議は入れていない（設計案でも「最小実装のあと」）。

### 保存しているもの

| プロパティ | 置き場 | 値 |
|---|---|---|
| `JAC_LEGITIMACY` | Player | 正当性（無ければ初期値 50）。効果は無い（Q57） |
| `JAC_ARB_OUTLAW_UNTIL` | Player | 討伐の大義が立つ最後のターン |
| `JAC_ARB_PENDING` | Game | 審理中の請求 `{Requester, A, B, Turn, Power, AnswerA, AnswerB}`。当事者の請求では A が請求者 |
| `JAC_ARB_LAST_REQUEST_TURN` | Game | 最後に請求を受け付けたターン |
| `JAC_ARB_TRUCE_<小さいID>_<大きいID>` | Game | 停戦が続く最後のターン |

ターンはすべてゲーム開始からの数（`Game.GetCurrentGameTurn() - GameConfiguration.GetStartTurn() + 1`）。1573年7月の換算は史実イベントと同じ `TurnForYear`（標準速度でターン120）。

## 公式の根拠

- UI → ゲームプレイの経路：`DLC/BlackDeathScenario/UI/BlackDeathScenario_PopupHandler.lua:22-28`（`OnStart` に GameEvent 名を入れて `EXECUTE_SCRIPT`）、受け手 `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:66`（`GameEvents.EventPopupChoice.Add`）
- 規則ファイルの共有：`BlackDeathScenario.lua:3`（`include "BlackDeathScenario_Rules"`）と `BlackDeathScenario.modinfo:129-133`（`ImportFiles`）
- 講和：`DLC/PiratesScenario/Scripts/PiratesScenario_StartScript.lua:767`（`MakePeaceWith`）
- 金：`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:847`（`ChangeGoldBalance`）、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:404`（ゲームプレイ側の `GetGoldBalance`）
- 都市数：`AustraliaScenario.lua:1198`（`GetCities():GetCount()`）
- 通知文の各言語化：`DLC/PiratesScenario/Scripts/PiratesScenario_GameCore_Script.lua:136-137`（ゲームプレイ側の `Locale.Lookup` に引数）
- 停戦の基本ターン数：`Base/Assets/Gameplay/Data/GlobalParameters.xml:248`（`DIPLOMACY_PEACE_MIN_TURNS` = 10）
- 「外交的支持」の訳語：`DLC/Expansion2/Text/Expansion2_Translations_Text.xml:108638-108639`
- 画面：一覧の枠は `poc/JAC_PoC_GrowthUI/UI/JAC_Notebooks.xml` と同じ形、`ScrollPanel` は `Base/Assets/UI/Popups/ChatPanel.xml:12-13`

### 公式に使用例が無いもの・無い API

- 外交的支持（Favor）の読み書き：公式 Lua の使用例は UI 側の `Player:GetFavor()` だけ（`DLC/Expansion2/UI/Replacements/TopPanel_Expansion2.lua:169`、`DiplomacyRibbon_Expansion2.lua:77`）。実機の court-arbitration-02 では、ゲームプレイ側（`GameCore_Tuner`）の `Players[id]:GetFavor()` が `function expected instead of nil` で失敗した。
  - `DLC/Expansion2/Binaries/Win64/GameCore_XP2_FinalRelease.dll` の Lua 登録を、関数名の文字列を参照するコードと同じ場所で参照されるソースファイル名から調べると、次のとおり分かれている。
    - UI の Player（`XP2_Cache_Lua_IPlayer.cpp`）：`GetFavor`・`GetFavorPerTurn`・`GetFavorPerTurnToolTip`・`GetFavorEnteringCongress`
    - ゲームプレイ側の `Player:GetDiplomacy()`（`XP2_Lua_IPlayerDiplomacy.cpp`）：`GetFavor`・`ChangeFavor`・`GetFavorPerTurn`・`CalcFavorPerTurnFromAlliances`
  - そのため Rules の `R.Favor(id)` は、Player に `GetFavor` があればそれを使い（UI）、無ければ `GetDiplomacy():GetFavor()` を使う（ゲームプレイ）。支払いは `GetDiplomacy():ChangeFavor(-20)`。公式 Lua に使用例が無いので、呼べなければ何も払わせずに請求を断る（`pcall`）。
  - 代わりの道として、一度きりの修正値 `MODIFIER_PLAYER_ADD_FAVOR`（`DLC/Expansion2/Data/Expansion2_Modifiers.xml:59`）を `Player:AttachModifierByID` で付ける方法もある（ゲームプレイ側の使用例：`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:1204`）。ただし量が修正値の定義で固定になり、読み取りはできないので、上の API が実機で効かないときだけ使う。
  - シナリオの最初に、`GameCore_Tuner` と `InGame` の両方でこれらの関数があるかを型で返す確認の手順を置いた（失敗しない）。
- 宣戦の検知：`Events.DiplomacyDeclareWar` は UI 側の使用例しか無い（`Base/Assets/UI/DiplomacyRibbon.lua:925`）。そのため停戦中の戦争は「次のターンの初めに講和し直す」だけで、宣戦した側に不利益は付けない。
- 開戦事由・不満（Grievance）の付与：Lua には読み取り（`GetGrievancesAgainst`、`DLC/Expansion2/UI/Additions/DiplomacyActionView_AllianceRow.lua:46`）しか無く、修正値の効果も不満の生成倍率（`EFFECT_ADJUST_PLAYER_GRIEVANCE_GENERATION`）だけで、特定の相手への開戦事由を足すものが無い。討伐の大義は記録と通知だけにした（Q58）。

## 入れていないもの（後回し）

- 所領安堵：設計案では「2つ目の実装として Loyalty 加算から」。SSOT に効果の数値も対象都市の決め方も無いため、この MVP には入れない。
- 官位授与・討伐の勅命、第三者（将軍家以外）の仲介、AI からの請求、将軍家 AI の発議、停戦破りの不利益。

## テスト

- `tests/test_court_arbitration.py`：全処理が日本マップ4種でだけ読み込まれること、使う文言キーが3言語そろうこと、UI スクリプトが構文として読めること。lupa でゲームプレイ側の Lua をターンごとに動かし、請求と費用、1ターン1件、将軍家の3倍と1573年（ターン120）以降、当事者以外・非交戦の拒否、費用不足と `ChangeFavor` の失敗で何も引かれないこと、Favor を UI の Player とゲームプレイ側の外交オブジェクトのどちらからも読めること、AI の応否と拒否の不利益、人間の応否（無回答は拒否、二重回答は無視）、停戦中の戦争の講和し直しと期限切れ、戦争が先に終わった請求の無効を確かめる。

## 実機確認（メインセッション）

シナリオ：`tools/civ6-autotest/scenarios/court-arbitration.json`（15 km 史実領土、足利将軍家を人間、ほか21勢力 AI）。

1. `GameCore_Tuner` と `InGame` で Favor の関数があるかを記録する（`Player.GetFavor=…` `Diplomacy.ChangeFavor=…` の形。失敗しない）。
2. 開始時に大内・大友（R01）が交戦中で、将軍家が裁定の力を持ち、室町幕府の終わりがターン120。ゲームプレイ側の `GetDiplomacy():ChangeFavor(50)` で外交的支持が増える（関数が無ければ、どれがあるかをエラーに出す）。
3. 「朝廷に停戦を請う」ボタンが出て、一覧が開く（スクリーンショット `01-arbitration-panel.png` を人が見る）。
4. UI から大内・大友の停戦を請求すると、`EXECUTE_SCRIPT` 経由で請求が置かれ、外交的支持 -20・金 -100・正当性 45 になり、力は3倍。
5. テストのため長宗我部とアイヌを交戦させ、停戦期間中の扱いにしてから1ターン進めると、講和し直されている（`MakePeaceWith` がゲームプレイ側で効く）。
6. 同じターンの初めに大内・大友の請求が裁かれる。応じれば停戦がターン32まで（3倍）、拒めば拒んだ側が正当性35・大義がターン12まで。AI の判断は都市数によるので、どちらかを受け入れる。
7. 画面の見た目（ボタンの位置が他の UI と重ならないか）と通知の文面は、スクリーンショットで人が確かめる。マルチプレイでの応否は実機で未確認。
