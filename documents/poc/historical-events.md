# 史実の出来事の仕組み（JAC_PoC_HistoricalEvents）

SSOT 3.2「史実の出来事は史実の年に起き、先に関係する研究を終えた勢力はその年より早く使える」（研究していない勢力にはその年に、研究そのものではなくひらめきを与える。オーナー判断1a）と、3.1「長尾家が上杉家を継ぐ出来事で家名を切り替える」を、データ駆動で実装した PoC。`RULESET_JAC_BANSEIKI` のときだけ有効になる。

- MOD：`poc/JAC_PoC_HistoricalEvents`（ID `f6398d3b-707c-4d0a-8b19-e68333efa200`）
- 依存：Gathering Storm、万世紀ルールセット、CustomCivs（長尾家と家名表示の上書き）、戦国ツリー（前提技術・社会制度の ID）
- 実機シナリオ：`tools/civ6-autotest/scenarios/historical-events-20km.json`
- 静的テスト：`tests/test_historical_events.py`

## 動き

1. 毎ターンの開始（`GameEvents.OnGameTurnStarted`）に、全出来事 × 生存中の主要勢力を調べる
2. 対象勢力（`JAC_HistoricalEventTargets`。行が無ければ全主要勢力）のうち、まだ起きていない勢力について、次のどちらかなら出来事を起こす
   - 史実の年のターンに達した（`reason=year`）
   - 前提の技術・社会制度（`PrereqTech`／`PrereqCivic`）を既に持っている（`reason=prereq`。研究を先に終えた勢力は早く起きる）
3. 出来事が起きると、その勢力について次を行う
   - フラグ：プレイヤーのプロパティ `JAC_EVENT_FIRED_<EventType>` に起きたターンを書く。これが「1勢力1回」の記録で、セーブに残る
   - ひらめき（`reason=year` のときだけ）：前提の技術・社会制度をまだ持たない勢力には、その研究のひらめきを発動する。持っている勢力には何もしない。ログの `boost=` に `tech=boosted`／`civic=boosted`（発動）か `no_boost`（`Boosts` の行が無い、または既に発動済み）を出す
   - 効果（モディファイア）：`JAC_HistoricalEventModifiers` のモディファイアを付ける
   - 家名：`JAC_HistoricalEventRenames` の行がその勢力の文明なら、Game プロパティ `JAC_CIV_NAME_<CivilizationType>` に表示名の LOC キーを書く。CustomCivs の `UI/JAC_CivNameOverride.lua` がこれを読み、外交リボン・外交画面・ワールドランキング・都市バナーの文明名を差し替える
   - 資源：`JAC_HistoricalEventResources` の資源を、その出来事が誰かに初めて起きたときに1回だけ地図に置く。置いたタイルは Game プロパティ `JAC_EVENT_RESOURCES_<EventType>` に残す
   - 通知：`NotificationTypes.USER_DEFINED_1` で本人に（`NotifyAll=1` なら全主要勢力に）送る。文面は LOC キーのまま渡し、各端末の言語で表示する
4. `Lua.log` に `[JAC_EVENTS] fired event=… player=… civ=… reason=… turn=… due=…` を出す

## データ形式（出来事の一覧を足すとき）

コードを変えずに行を足すだけで出来事を増やせる。調査中の一覧（`documents/research/sengoku-events.md` 予定）は、この形の SQL に変換して `Data/` に置き、modinfo の `JAC_Events_Data` に1行足す。

| テーブル | 列 | 意味 |
|---|---|---|
| `JAC_HistoricalEvents` | `EventType`（主キー）、`Pack`、`Year`、`Turn`、`PrereqTech`、`PrereqCivic`、`Name`、`Description`、`NotifyAll` | `Year` と `Turn` のどちらかは必須。両方あれば `Turn` を使う。`Name`・`Description` は通知の見出しと本文の LOC キー |
| `JAC_HistoricalEventTargets` | `EventType`、`CivilizationType` | 対象勢力。行が無ければ全主要勢力 |
| `JAC_HistoricalEventModifiers` | `EventType`、`ModifierId` | 起きた勢力に付けるプレイヤーのモディファイア |
| `JAC_HistoricalEventRenames` | `EventType`、`CivilizationType`、`Name` | 文明の表示名の切替。文明 ID は変えない |
| `JAC_HistoricalEventResources` | `EventType`、`ResourceType`、`PlotIndex`、`SearchRange`、`Amount`、`SiteId` | 出現させる資源。`PlotIndex` は地図ごとに違うため、地図ファイル別の SQL（modinfo の条件で読み分け）に書く。`PlotIndex` から `SearchRange` 以内で、資源が無く都市でもない、公式の配置条件を満たす最も近いタイルに置く |

`PlotIndex` は[資源候補 JSON](../spec/data/japan-map-resources.json)の代表点を、`Maps/manifest.json` の格子（`grid_origin_projected_km` ほか）で地図生成器と同じ投影にかけた最寄りタイル。テストが同じ計算で照合する。史実領土版と開拓者版は同じ格子なので、同じ SQL を使う。

## 暦との関係

Civ6 の公式 Lua には年を数値で返す API が無く、暦は表示用の文字列（`Calendar.MakeYearStr`）しか出さない。公式の Pirates シナリオもターン数から年を計算している。そこで、暦を作っている表そのものから年をターンに換算する。

- 暦は main にマージ済みの戦国パックの暦（[sengoku-calendar.md](sengoku-calendar.md)、PR #34）。`ERA_ANCIENT` の開始年が1534、ゲーム速度ごとに `GameSpeed_Turns`（`MonthIncrement` か月/ターンを `TurnsPerIncrement` ターン）が順に並ぶ
- 換算（`Scripts/JAC_HistoricalEvents.lua` の `TurnForYear`）：ターン1を開始年の始まりとし、ターン `t` の手前までに経過した月数が `(年 - 開始年) × 12` に達する最初のターン。開始年は `GameInfo.StartEras['ERA_ANCIENT'].Year`、速度は `GameConfiguration.GetGameSpeedType()`、区間は `GameInfo.GameSpeed_Turns()` を速度で絞って行順に読む（行順は `GameSpeed_Turns` の `ORDER BY rowid`、`01_GameplaySchema.sql:3337`。速度は `InGameTopOptionsMenu.lua:442` と同じ取り方）
- 標準速度（4か月/ターン＝1年3ターン）では `ターン = (年 - 1534) × 3 + 1`。鉄砲伝来1543年はターン28、足尾銅山1550年はターン49、長尾家の上杉家継承1561年はターン82、1600年はターン199
- ほかの速度も同じ表で自動的に換算する。1年あたりのターン数は Online 1.5、Quick 2、Epic 4（200ターンまで。以後6）、Marathon 6（200ターンまで。以後12）。例：鉄砲伝来1543年は Quick でターン19、Online でターン15
- 暦の月は1月始まりとみなす（暦の起点計算はエンジン内部。sengoku-calendar.md の未検証事項と同じ）。年の途中の出来事（月日単位）は扱わない
- 以前の暫定値（`JAC_EVENTS_START_YEAR`／`JAC_EVENTS_TURNS_PER_YEAR` の `GlobalParameters`）は廃止した。静的テスト（`tests/test_historical_events.py`）は、暦の XML から同じ換算を Python で再計算し、全速度で「換算したターンの暦上の年が出来事の年で、その前のターンは前の年」を確かめる

## サンプルの3件（すべて暫定）

| 出来事 | 年（ターン） | 対象 | 前提（早く起きる条件） | 効果 |
|---|---|---|---|---|
| 鉄砲伝来 `JAC_EVENT_TEPPO_DENRAI` | 1543（28） | 全主要勢力 | 技術「鉄砲伝来」`TECH_JAC_SENGOKU_FIREARMS_CONTACT` | 通知。年が来たとき未研究の勢力には技術「鉄砲伝来」のひらめきを与える（技術そのものは与えない。研究済みの勢力には影響なし） |
| 足尾銅山の開坑 `JAC_EVENT_ASHIO_COPPER` | 1550（49） | 全主要勢力 | 技術「金銀山経営」`TECH_JAC_SENGOKU_MINE_MANAGEMENT` | 通知。足尾（下野）付近に銅（`RESOURCE_COPPER`）を1つ置く（日本マップ4種のみ） |
| 長尾家の上杉家継承 `JAC_EVENT_NAGAO_UESUGI` | 1561（82） | 長尾家 | 社会制度「朝廷・将軍への取次」`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS` | 全勢力に通知。長尾家の表示名を「上杉」（Uesugi／Уэсуги）に切り替える |

足尾銅山は資源候補 JSON で `start_year: 1550`・`available_at_start: false`（開始地図に置かない後年の資源）。最寄りタイルは 20 km 地図で森林の丘、15 km 地図で山岳のため銅を置けず、実行時に公式の配置判定で2タイル以内の丘へずらす。

`Boosts` の行が無いとひらめきは発動できないため、3件の前提には仮の行（`BOOST_TRIGGER_TRAIN_UNIT`＋ガレー船、`LOC_JAC_EVENT_BOOST_PLACEHOLDER`）を `JAC_HistoricalEvents_Sengoku.sql` で足している（**暫定**）。ツリー側に行があれば足さない。実際のひらめき条件はツリー側で決める。

## ひらめきの公式 API の根拠

- 技術：`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:761-784`。`player:GetTechs():HasBeenBoosted` で未発動を確かめ、`player:GetTechs():TriggerBoost(techIndex, 1)`（コメント「ゴーディハット由来として発動」）。UI 側の判定は `Base/Assets/UI/Panels/TechBoostsPanel.lua:91`（`HasTech`／`HasBoostBeenTriggered`／`CanTriggerBoost`）
- 社会制度：`Base/Assets/UI/Panels/CivicBoostsPanel.lua:88` が `player:GetCulture()` に同名の `HasCivic`／`HasBoostBeenTriggered`／`CanTriggerBoost` を持つことを示す。`GetCulture():TriggerBoost(civicIndex, 1)` を呼ぶ公式の Lua 例は見つからなかった（**要実機確認**。技術側と同じ呼び出しを、上の3つの判定で守って使う）
- 代替（不採用）：モディファイア `MODIFIER_PLAYER_GRANT_SPECIFIC_TECH_BOOST`（`Base/Assets/Gameplay/Data/Modifiers.xml:2499-2503`）は技術のひらめきだけで、社会制度用の同種は無い。両方を同じ経路で扱うため Lua の API にした

## 決定事項（オーナー回答、B 組）

実装は「サンプルの3件」まで。全一覧への展開は後続作業。

| 番号 | 決定 | 状態 |
|---|---|---|
| 1a | 年が来たとき、前提の技術・社会制度を未研究の勢力には、それ自体ではなくひらめきを与える。研究済みの勢力は影響なし | 実装済み（技術と社会制度。上記） |
| 2a | 長尾→上杉は「1561年、または関連する社会制度を先に研究」の条件を維持する | 既存の条件のまま（テストで固定） |
| 6a | 松平→徳川の改名も、長尾→上杉と同じ仕組み（`JAC_HistoricalEventRenames`）の出来事にする | 記録のみ。一覧の実装時に行を足す |
| 7a | 本能寺の変の後、織田家の家名表示を羽柴から豊臣へ切り替える（同じ仕組み） | 記録のみ。一覧の実装時に行を足す |
| 8a | 室町幕府が終わる1573年、足利家の勢力は残り、将軍として裁定する権能だけを失う | 記録のみ。勢力は消さない。裁定の権能の停止は世界会議の実装側で扱う |
| 10a | 関ヶ原の戦いの出来事は置かない | 記録のみ |

## マルチプレイとセーブ

- 状態はすべて Player／Game のプロパティにあり、Lua の変数にターンをまたいで持たない。ロード後もプロパティから同じ判定になり、同じ出来事は2度起きない
- 判定はゲームプレイ側のターン開始イベントだけで行い、出来事は「予定ターン→ID」、勢力は ID 順に処理する。乱数・ローカルプレイヤー・端末の言語に依存しない
- 家名は Game プロパティ（同期済みの共有状態）から UI が読む。端末ごとの設定値は使わない（SSOT 3.1）
- マルチプレイでの同期・決定性は未検証（[PoC README「マルチプレイ同期と決定性」](README.md#マルチプレイ同期と決定性)と同じ扱い）

## 公式ファイルの根拠

パスは `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/` からの相対。

| 使っている仕組み | 公式の例 |
|---|---|
| ターン開始の処理と現在ターンの取得 | `DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:1440-1455`（`GameEvents.OnGameTurnStarted` と `Game.GetCurrentGameTurn()`）、`DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:64` |
| プレイヤーのプロパティ | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:373`（`SetProperty`）、`DLC/CivRoyaleScenario/Scripts/CivRoyaleScenario_UnitCommands.lua:137`（`GetProperty`） |
| モディファイアを Lua で付ける | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:1204`（`pPlayer:AttachModifierByID`） |
| 技術を与えるモディファイア | `DLC/Expansion2/Data/Expansion2_Modifiers.xml:146,968-971`、マオリの特性 `DLC/Expansion2/Data/Expansion2_Civilizations_Major.xml:479-481,973-975` |
| 通知（LOC キーを渡す） | `DLC/PiratesScenario/Scripts/PiratesScenario_Shared_Script.lua:318-338`、`Base/Assets/Gameplay/Data/Notifications.xml:52,165`（`NOTIFICATION_USER_DEFINED_1`）、`Base/Assets/UI/Panels/NotificationPanel.lua:181,231` |
| 通知アイコン | `DLC/BlackDeathScenario/Data/BlackDeathScenario_Icons_Notifications.xml:4-6`（本体には `ICON_NOTIFICATION_USER_DEFINED_1` が無い） |
| ゲーム中の資源配置と配置判定 | `Base/Assets/UI/Tuner/TunerMapPanel.lua:59`（ゲーム中の `ResourceBuilder.SetResourceType`）、`DLC/Expansion2/Maps/Utility/ResourceGenerator.lua:316`（`ResourceBuilder.CanHaveResource`） |
| タイルの取得と距離 | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:491`（`Map.GetPlotByIndex`）、`DLC/WarMachineScenario/Scripts/WarMachineScenario.lua:163`（`Map.GetPlotDistance`） |
| 研究済みの判定 | `DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:670`（`HasTech`）、`:555`（`HasCivic`） |
| ゲームプレイ側の文明 ID | `DLC/WarMachineScenario/Scripts/WarMachineScenario.lua:27`（`GetCivilizationTypeName`） |
| 独自テーブルの追加 | `DLC/Expansion2/Data/Expansion2_Schema.sql:2`（`CREATE TABLE` を UpdateDatabase で読む） |
| 年の表示と計算 | `Base/Assets/UI/TopPanel.lua:407`（`Calendar.MakeYearStr` のみ）、`DLC/PiratesScenario/UI/Replacements/TopPanel_PiratesScenario.lua:10-18`（ターンから年を計算） |
| シナリオでの研究の強制 | `DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:38`（`SetTech`）、`DLC/PiratesScenario/Scripts/PiratesScenario_Shared_Script.lua:956`（`SetCivic`） |

## 実機確認（メインセッションで実行）

```powershell
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/historical-events-20km.json --output tools/civ6-autotest/artifacts/historical-events-run-01
```

有効にする MOD は `japan-map-20km.json` と同じ5つに、この MOD を足したもの。20 km 史実領土版・22勢力・織田が人間。

| 手順 | 確かめること |
|---|---|
| 開始直後 | `Lua.log` に `[JAC_EVENTS] loaded events=3 startYear=1534 speed=GAMESPEED_STANDARD`。ターン1で出来事は1件も起きていない |
| ターン1に強制 | 織田に技術「鉄砲伝来」「金銀山経営」、長尾家に社会制度「朝廷・将軍への取次」を与える（GameCore_Tuner） |
| 1ターン進めた後 | `fired event=JAC_EVENT_TEPPO_DENRAI player=0 civ=CIVILIZATION_JAC_ODA reason=prereq turn=2 due=28` のログ。織田の鉄砲伝来・足尾銅山と長尾家の上杉継承がターン2で起き、他の勢力の鉄砲伝来は起きていない。発生回数の合計が3。足尾の目標タイル（4416）から2タイル以内に銅が1つ置かれた。`WorldRankings` の画面で長尾家の文明名が `LOC_JAC_EVENT_UESUGI_CIV_NAME` に切り替わっている |
| セーブ→メニュー→ロード | 同じ確認がロード直後にも通る（プロパティ・銅・家名が保存されている） |
| ロード後に1ターン | 発生回数が3のままで、各勢力の発生ターンが2のまま（2度起きていない）。家名の切替も残る |
| ターン28まで進める | 長尾家（鉄砲伝来を未研究）に `fired event=JAC_EVENT_TEPPO_DENRAI … reason=year turn=28 due=28 boost=tech=boosted`。長尾家は技術を持たずひらめきだけが発動している。織田は技術を持ったまま。AI が先に研究していると失敗するので、その場合はログの `reason=prereq` を確かめる |

判定は `generic`（`Lua.log` の JAC 由来のエラーが無いこと）。

**要実機確認（シナリオでは確かめない）：**

1. 通知の見た目。`ICON_NOTIFICATION_USER_DEFINED_1` に `ICON_ATLAS_NOTIFICATIONS` の15番（公式 Black Death と同じ）を当てたが、絵柄が出来事に合うかは画面で見る。通知はターン終了で消える（公式の既定）
2. ゲームプレイ用スクリプトの中で `ResourceBuilder` が使えるか。公式の用例は地図生成と Tuner だけで、使えなければログに `resource … result=no_legal_plot` ではなく Lua エラーが出る
3. 年による発生（`reason=year`）で `TriggerBoost` が効くか。技術はシナリオのターン28で確かめる。社会制度の `GetCulture():TriggerBoost` は公式の Lua 例が無いため、長尾家の上杉継承（ターン82）の前に別途確かめる（15 km 地図では50ターン以内に織田が滅亡することがある）
4. マルチプレイでの同期
