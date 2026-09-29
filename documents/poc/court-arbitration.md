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
| 成立 | 全員が応じれば裁くターンの初めにその場で講和し、停戦中（10 または 30 ターン）に戦争になっていれば、次のターンの初めに講和し直す。講和は `MakePeaceWith(相手, true)` で、エンジンの最短の戦争期間（開戦から10ターン）の中でも効く | 設計案「一定ターン再宣戦を制限」 |
| 拒否 | 拒んだ勢力は正当性 -15、10ターンの「討伐の大義」を全勢力に通知する | 3.2「正当性の低下や討伐の大義」（Q57・Q58） |
| 件数 | 裁定は全体で1ターンに1件まで（審理中の請求があれば次を受け付けない） | 設計案「詰まり防止」 |

### 流れ

1. 画面上部中央の「朝廷に停戦を請う」ボタン（請求できる組か、答えるべき請求があるときだけ出る）から一覧を開き、組ごとの「裁定を請う」を押す。費用が足りないなどで請求できない組は押せず、理由をツールチップに出す。
2. UI は `UI.RequestPlayerOperation(…, PlayerOperations.EXECUTE_SCRIPT, {OnStart = "JAC_CourtArbitrationRequest", A, B})` を送る。ゲームプレイ側の `GameEvents.JAC_CourtArbitrationRequest` が全マシンで同じ検証（`Scripts/JAC_CourtArbitration_Rules.lua` の `CheckRequest`）をして費用を引き、請求を Game プロパティ `JAC_ARB_PENDING` に置く。答える側に通知する。
3. 人間の答える側は、同じ一覧の「応じる」「拒む」で `JAC_CourtArbitrationRespond` を送る。
4. 次のターンの初め（`GameEvents.OnGameTurnStarted`）に裁く。停戦中の組の講和し直し（と、安全網の講和待ちの組の講和）を先に行う。

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
- 公式の根拠：講和の呼び出しは `DLC/PiratesScenario/Scripts/PiratesScenario_StartScript.lua:767`（引数1つ）。第2引数の公式の使用例は無く、上の実機の結果が根拠。

### 調査用シナリオ `probe-make-peace.json`

15 km・史実の領土・足利将軍家が人間で、裁定の MOD を入れる。`GameCore_Tuner`（ゲームプレイ）から、R01（大内・大友、ターン1から交戦）と、新たに宣戦させた3組（F1 長宗我部・アイヌ、F2 琉球・伊達、F3 島津・蘆名）に次を試し、呼ぶたびに `IsAtWarWith` を読む。前後で `CanMakePeaceWith`（両方向）と `GetAtWarChangeTurn` を読む。すべて `pcall` で値を返し、止まらない。ログは `[JAC_PEACE_PROBE]`。結果は上の表。

1. ターン1：`DIPLOMACY_WAR_MIN_TURNS`・`DIPLOMACY_PEACE_MIN_TURNS`・`Game.GetGameDiplomacy():GetMinPeaceDuration()` を読む。F1 は `a:MakePeaceWith(b)`、F2 は `a:MakePeaceWith(b, true)`、F3 は相手側から `b:MakePeaceWith(a)` の後に `a:MakePeaceWith(b)`、R01 は4通り全部。
2. 1ターン進めてターン2：前のターンの呼び出しが後から効いたかを読み、F1（court-arbitration-05 と同じ）と R01（`true` 付き）を試し直す。
3. 9ターン進めてターン11（開戦から10ターン）：F1・F2・F3・R01 をそれぞれ前と同じ形で試す。
4. 1ターン進めてターン12：まだ戦争中の組に `a:MakePeaceWith(b)` と `true` 付きを試す。

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
| `JAC_ARB_AWAIT_<小さいID>_<大きいID>` | Game | 安全網：応諾したのに講和できなかった組。講和した時から数える停戦のターン数（実機では置かれていない） |
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

- `tests/test_court_arbitration.py`：全処理が日本マップ4種でだけ読み込まれること、使う文言キーが3言語そろうこと、UI スクリプトが構文として読めること。lupa でゲームプレイ側の Lua をターンごとに動かし、請求と費用、1ターン1件、将軍家の3倍と1573年（ターン120）以降、当事者以外・非交戦の拒否、費用不足と `ChangeFavor` の失敗で何も引かれないこと、Favor を UI の Player とゲームプレイ側の外交オブジェクトのどちらからも読めること、AI の応否と拒否の不利益、人間の応否（無回答は拒否、二重回答は無視）、停戦中の戦争の講和し直しと期限切れ、戦争が先に終わった請求の無効を確かめる。
- スタブにはエンジンの最短の戦争期間を入れてある（開戦から10ターンは引数1つの `MakePeaceWith` が何もせず、`true` 付きなら講和する）。最短期間の中でも応諾した裁定がその場で講和し、停戦中の宣戦が次のターンに講和し直されることを確かめる。`true` が効かないエンジンを模した場合の安全網（講和待ちと停戦の開始、待つ間の再請求の拒否、待つ間に自分たちで講和した組、停戦中の宣戦を期限の後でも講和させること、`SAFETY NET` のログ）も確かめる。
- 実機シナリオ `court-arbitration.json` のゲームプレイ側の手順を、スタブの上で順に動かして通ることを確かめる（画面の操作と手番待ちは飛ばす）。`probe-make-peace.json` の各手順がスタブ上でも止まらずに結果を返すことも確かめる。

## 実機確認（メインセッション）

シナリオ：`tools/civ6-autotest/scenarios/court-arbitration.json`（15 km 史実領土、足利将軍家を人間、ほか21勢力 AI）。応諾と拒否の両方を、1回の実行で決まった形で通す。

1. `GameCore_Tuner` と `InGame` で Favor の関数があるかを記録する（`Player.GetFavor=…` `Diplomacy.ChangeFavor=…` の形。失敗しない）。
2. ターン1：大内・大友（R01）が交戦中で、将軍家が裁定の力を持ち、室町幕府の終わりがターン120。ゲームプレイ側の `GetDiplomacy():ChangeFavor(50)` で外交的支持が増える（2回分の費用）。
3. 応諾させる組を選ぶ：将軍家・大内・大友を除き、互いに講和中で都市数が同じ（2以上）の2家を ID の小さい順に探し、ターン1に宣戦させる（`JAC_ARB_TEST_PAIR`）。AI の暫定規則（都市数が相手の 1.5 倍を超えると拒む）では、どちらかがターン1に1都市増やしても両方が応じる。
4. 人間の手番が始まるのを待ち（`IsTurnActive`）、「朝廷に停戦を請う」の一覧を開く（`01-arbitration-panel.png`）。UI からその組の停戦を請求すると、`EXECUTE_SCRIPT` 経由で請求が置かれ、外交的支持 -20・金 -100・正当性 45 になり、力は3倍。
   - court-arbitration-03 では、開始直後のターン処理中に送った請求がゲームプレイ側に届かなかった。画面は手番が始まるまで請求・応否のボタンを押せなくし、送らない。
5. 1ターン進めてターン2：両家が応じ、最短の戦争期間の中でもその場で講和している（`JAC_ARB_PEACE_TURN` = 2、停戦はターン32まで＝10×3、講和待ちの記録なし、両家に不利益なし）。大内・大友はまだ交戦中（`02-after-accepted-ruling.png`）。
6. 同じターン2に、UI から大内・大友の停戦を請求する（正当性 40 になる）。続けて、応諾した組をテストのため停戦中に宣戦させる（`DeclareWarOn(相手, FORMAL_WAR, true)`）。講和後の最短期間（10ターン）の中なので、エンジンが宣戦させない可能性がある。その場合もここでは止めず、ターン3で理由を出して失敗にする。
7. 1ターン進めてターン3：
   - 大内・大友の裁定は大内が拒む（court-arbitration-05・07 と同じ。都市数で優勢）。交戦中のまま、停戦・講和待ちの記録なし、大内は正当性35・討伐の大義がターン13まで。大友も拒んでいれば結果に出す。
   - 停戦中の宣戦は、ターン3の初めにスクリプトが講和させている（`JAC_ARB_PEACE_TURN` = 3、停戦の期限はターン32のまま）。
   - `03-after-refused-ruling.png` を残す。
8. 画面の見た目（ボタンの位置が他の UI と重ならないか）と通知の文面は、スクリーンショットで人が確かめる。マルチプレイでの応否は実機で未確認。`Lua.log` に `SAFETY NET` が出ていないことも見る。

これまでの実機の結果：court-arbitration-07 は通ったが、大内・大友の裁定を大内が拒んだため、応諾の流れは実機で一度も動いていなかった。court-arbitration-06 は `probe-make-peace.json`（上の表）。
