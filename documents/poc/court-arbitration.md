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
| 成立 | 全員が応じれば講和し、停戦中（10 または 30 ターン）に戦争になっていれば毎ターンの初めに講和し直す。開戦から10ターンの間はエンジンが講和させないので、講和できるターンの初めまで毎ターン試し、停戦は講和した時から数える | 設計案「一定ターン再宣戦を制限」 |
| 拒否 | 拒んだ勢力は正当性 -15、10ターンの「討伐の大義」を全勢力に通知する | 3.2「正当性の低下や討伐の大義」（Q57・Q58） |
| 件数 | 裁定は全体で1ターンに1件まで（審理中の請求があれば次を受け付けない） | 設計案「詰まり防止」 |

### 流れ

1. 画面上部中央の「朝廷に停戦を請う」ボタン（請求できる組か、答えるべき請求があるときだけ出る）から一覧を開き、組ごとの「裁定を請う」を押す。費用が足りないなどで請求できない組は押せず、理由をツールチップに出す。
2. UI は `UI.RequestPlayerOperation(…, PlayerOperations.EXECUTE_SCRIPT, {OnStart = "JAC_CourtArbitrationRequest", A, B})` を送る。ゲームプレイ側の `GameEvents.JAC_CourtArbitrationRequest` が全マシンで同じ検証（`Scripts/JAC_CourtArbitration_Rules.lua` の `CheckRequest`）をして費用を引き、請求を Game プロパティ `JAC_ARB_PENDING` に置く。答える側に通知する。
3. 人間の答える側は、同じ一覧の「応じる」「拒む」で `JAC_CourtArbitrationRespond` を送る。
4. 次のターンの初め（`GameEvents.OnGameTurnStarted`）に裁く。講和待ちの組の講和と、停戦中の組の講和し直しを先に行う。

### エンジンの最短の戦争期間

エンジンは開戦から `DIPLOMACY_WAR_MIN_TURNS`（10ターン、`Base/Assets/Gameplay/Data/GlobalParameters.xml:260`）の間は講和させない。court-arbitration-05 では、ターン1に宣戦させた長宗我部・アイヌに対し、ターン2のゲームプレイ側（`OnGameTurnStarted`）で `MakePeaceWith` を呼ぶと、エラーは出ないが戦争のままだった（`truce reimposed pair=17-21 turn=2 until=5 ok=false`）。大内・大友（R01）も開始ターンの宣戦なので、ターン2の裁定では同じく講和できない。

- 根拠：公式の講和の呼び出し `DLC/PiratesScenario/Scripts/PiratesScenario_StartScript.lua:767` は、先に同 `:795` で `CanMakePeaceWith` を確かめた相手にだけ呼ぶ。永続の戦争にしたいシナリオはこの値を大きくする（`DLC/AlexanderScenario/Data/AlexanderScenario_GlobalParameters.xml:42`＝100、`DLC/VikingsScenario/Data/VikingsScenario_GlobalParameters.xml:11`・`DLC/NubiaScenario/Data/NubiaScenario_GameplayData.xml:55`＝500）。都市国家の画面は残りターンを `GetMinPeaceDuration() + GetAtWarChangeTurn(相手) - 現在のターン` で出す（`Base/Assets/UI/PartialScreens/CityStates.lua:75,1133,1516`）。DLL の文字列には `LOC_DIPLOMACY_ACTION_FAILURE_REASON_PEACE_TOO_RECENT` もある。
- `MakePeaceWith` の第2引数：公式の使用例は引数1つだけ。`DeclareWarOn(相手, WarTypes, true)` の第3引数に当たるものがあるかは DLL から分からないので、`probe-make-peace.json` で確かめる。
- 対応：講和は呼んだ後に `IsAtWarWith` で確かめ、できなければ理由（`CanMakePeaceWith` が false なら `min_war`）を記録して次のターンの初めに試し直す。
  - 応諾した裁定：`JAC_ARB_AWAIT_<組>` に停戦のターン数を置き、講和できたターンから停戦を数える（`truce started`）。待つ間は同じ組を請求できない（`AWAITING_PEACE`）。応諾の通知は「講和できるターンになったら講和する」文面に変える。待つ間に自分たちで講和していれば、そのターンから停戦を数える。
  - 停戦中の宣戦：新しい戦争も10ターンは終わらせられない。講和できるまで試し、停戦の期限が過ぎても、講和するまで停戦の記録を残して試し続ける。
- 最短の戦争期間そのものを日本マップで下げるかは Q65（オーナー判断）。

### 調査用シナリオ `probe-make-peace.json`

15 km・史実の領土・足利将軍家が人間で、裁定の MOD を入れる。`GameCore_Tuner`（ゲームプレイ）から、R01（大内・大友、ターン1から交戦）と、新たに宣戦させた3組（F1 長宗我部・アイヌ、F2 琉球・伊達、F3 島津・蘆名）に次を試し、呼ぶたびに `IsAtWarWith` を読む。前後で `CanMakePeaceWith`（両方向）と `GetAtWarChangeTurn` を読む。すべて `pcall` で値を返し、止まらない。ログは `[JAC_PEACE_PROBE]`。

1. ターン1：`DIPLOMACY_WAR_MIN_TURNS`・`DIPLOMACY_PEACE_MIN_TURNS`・`Game.GetGameDiplomacy():GetMinPeaceDuration()` を読む。F1 は `a:MakePeaceWith(b)`、F2 は `a:MakePeaceWith(b, true)`、F3 は相手側から `b:MakePeaceWith(a)` の後に `a:MakePeaceWith(b)`、R01 は4通り全部。
2. 1ターン進めてターン2：前のターンの呼び出しが後から効いたかを読み、F1（court-arbitration-05 と同じ）と R01（`true` 付き）を試し直す。
3. 9ターン進めてターン11（開戦から10ターン）：F1・F2・F3・R01 をそれぞれ前と同じ形で試す。
4. 1ターン進めてターン12：まだ戦争中の組に `a:MakePeaceWith(b)` と `true` 付きを試す。

期待（未確認）：ターン1・2は `CanMakePeaceWith` が false で、どの形でも戦争のまま。ターン11（または12）で true になり、引数1つで講和できる。第2引数で最短期間を越えられるなら、実装をその形に変える。

### AI の応諾（暫定）

自分の都市数が相手の 1.5 倍を超えるとき（明らかに優勢）だけ拒み、それ以外は応じる。戦争疲弊と軍事力はゲームプレイ側で読めない（`GetStats():GetMilitaryStrength()` は UI の使用例しか無い）ため、都市数で代える。拒否の不利益の見積もり、AI からの請求、将軍家 AI の発議は入れていない（設計案でも「最小実装のあと」）。

### 保存しているもの

| プロパティ | 置き場 | 値 |
|---|---|---|
| `JAC_LEGITIMACY` | Player | 正当性（無ければ初期値 50）。効果は無い（Q57） |
| `JAC_ARB_OUTLAW_UNTIL` | Player | 討伐の大義が立つ最後のターン |
| `JAC_ARB_PENDING` | Game | 審理中の請求 `{Requester, A, B, Turn, Power, AnswerA, AnswerB}`。当事者の請求では A が請求者 |
| `JAC_ARB_LAST_REQUEST_TURN` | Game | 最後に請求を受け付けたターン |
| `JAC_ARB_TRUCE_<小さいID>_<大きいID>` | Game | 停戦が続く最後のターン。停戦中の宣戦を講和させるまでは、期限が過ぎても残す |
| `JAC_ARB_AWAIT_<小さいID>_<大きいID>` | Game | 応諾済みで講和を待つ組。講和した時から数える停戦のターン数 |
| `JAC_ARB_PEACE_TURN_<小さいID>_<大きいID>` | Game | スクリプトが最後に講和させたターン（検証用の記録） |

ターンはすべてゲーム開始からの数（`Game.GetCurrentGameTurn() - GameConfiguration.GetStartTurn() + 1`）。1573年7月の換算は史実イベントと同じ `TurnForYear`（標準速度でターン120）。

## 公式の根拠

- UI → ゲームプレイの経路：`DLC/BlackDeathScenario/UI/BlackDeathScenario_PopupHandler.lua:22-28`（`OnStart` に GameEvent 名を入れて `EXECUTE_SCRIPT`）、受け手 `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:66`（`GameEvents.EventPopupChoice.Add`）
- 規則ファイルの共有：`BlackDeathScenario.lua:3`（`include "BlackDeathScenario_Rules"`）と `BlackDeathScenario.modinfo:129-133`（`ImportFiles`）
- 講和：`DLC/PiratesScenario/Scripts/PiratesScenario_StartScript.lua:767`（`MakePeaceWith`）と、その前の確認 `:795`（`CanMakePeaceWith`）
- 最短の戦争期間：`Base/Assets/Gameplay/Data/GlobalParameters.xml:260`（`DIPLOMACY_WAR_MIN_TURNS` = 10）
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

- `tests/test_court_arbitration.py`：全処理が日本マップ4種でだけ読み込まれること、使う文言キーが3言語そろうこと、UI スクリプトが構文として読めること。lupa でゲームプレイ側の Lua をターンごとに動かし、請求と費用、1ターン1件、将軍家の3倍と1573年（ターン120）以降、当事者以外・非交戦の拒否、費用不足と `ChangeFavor` の失敗で何も引かれないこと、Favor を UI の Player とゲームプレイ側の外交オブジェクトのどちらからも読めること、AI の応否と拒否の不利益、人間の応否（無回答は拒否、二重回答は無視）、停戦中の戦争の講和し直しと期限切れ、戦争が先に終わった請求の無効を確かめる。スタブにエンジンの最短の戦争期間（開戦から10ターンは `MakePeaceWith` が何もしない）を入れ、応諾した裁定の講和待ちと停戦の開始、待つ間の再請求の拒否、待つ間に自分たちで講和した組、停戦中の宣戦を期限の後でも講和させることを確かめる。`probe-make-peace.json` の各手順がスタブ上でも止まらずに結果を返すことも確かめる。

## 実機確認（メインセッション）

シナリオ：`tools/civ6-autotest/scenarios/court-arbitration.json`（15 km 史実領土、足利将軍家を人間、ほか21勢力 AI）。

1. `GameCore_Tuner` と `InGame` で Favor の関数があるかを記録する（`Player.GetFavor=…` `Diplomacy.ChangeFavor=…` の形。失敗しない）。
2. 開始時に大内・大友（R01）が交戦中で、将軍家が裁定の力を持ち、室町幕府の終わりがターン120。ゲームプレイ側の `GetDiplomacy():ChangeFavor(50)` で外交的支持が増える（関数が無ければ、どれがあるかをエラーに出す）。
3. 人間の手番が始まるのを待つ（`IsTurnActive`）。「朝廷に停戦を請う」ボタンが出て、一覧が開く（スクリーンショット `01-arbitration-panel.png` を人が見る）。
4. UI から大内・大友の停戦を請求すると、`EXECUTE_SCRIPT` 経由で請求が置かれ、外交的支持 -20・金 -100・正当性 45 になり、力は3倍。届かなければ、ゲームプレイ側が最後に受けた要求（`JAC_ARB_LAST_EVENT`）と手番の状態を待機の結果に出す。
   - court-arbitration-03 では、開始直後のターン処理中（右下が「お待ちください」、13/22）に送った請求がゲームプレイ側に届かなかった。画面は手番が始まるまで請求・応否のボタンを押せなくし、送らない。引数名は動作済みの天下統一同盟に合わせて `Target`・`Other` にした。
5. テストのため長宗我部とアイヌを交戦させ、停戦期間中（ターン5まで）の扱いにしてから1ターン進める。ターン2では最短の戦争期間のため戦争のまま（court-arbitration-05 の結果。講和し直しは次のターン以降に持ち越す）。
6. 同じターンの初めに大内・大友の請求が裁かれる。応じれば講和待ち（`JAC_ARB_AWAIT` = 30、まだ戦争中）、拒めば拒んだ側が正当性35・大義がターン12まで。AI の判断は都市数によるので、どちらかを受け入れる。court-arbitration-05 では大内が拒み、大友が応じた。
7. 10ターン進めてターン12：長宗我部・アイヌはスクリプトが講和させている（`JAC_ARB_PEACE_TURN` がターン11か12）。停戦の期限（ターン5）は過ぎたので記録は消えている。裁定を応諾していれば、大内・大友も講和し、停戦は講和したターン + 30 まで。
8. 画面の見た目（ボタンの位置が他の UI と重ならないか）と通知の文面は、スクリーンショットで人が確かめる。マルチプレイでの応否は実機で未確認。
