# 史実の出来事の仕組み（JAC_PoC_HistoricalEvents）

SSOT 3.2「史実の出来事は史実の年に起き、先に関係する研究を終えた勢力はその年より早く使える」「史実の年が来たとき、関係する研究を終えていない勢力には、その研究へのひらめき（ブースト）だけを与える」「史実イベントは、暦の月までできるだけ合わせて起こす」と、3.1「長尾家が上杉家を継ぐ出来事で家名を切り替える」を、データ駆動で実装した PoC。`RULESET_JAC_BANSEIKI` のときだけ有効になる。

- MOD：`poc/JAC_PoC_HistoricalEvents`（ID `f6398d3b-707c-4d0a-8b19-e68333efa200`）
- 依存：Gathering Storm、万世紀ルールセット、CustomCivs（長尾家と家名表示の上書き）、戦国ツリー（前提技術・社会制度の ID）
- 実機シナリオ：`tools/civ6-autotest/scenarios/historical-events-15km.json`（採用縮尺の 15 km 地図。SSOT 3.4）
- 静的テスト：`tests/test_historical_events.py`

## 動き

1. 毎ターンの開始（`GameEvents.OnGameTurnStarted`）に、全出来事 × 生存中の主要勢力を調べる
2. 対象勢力（`JAC_HistoricalEventTargets`。行が無ければ全主要勢力）のうち、まだ効果を受けていない勢力を、次の順に判定する
   - 前提の技術・社会制度（`PrereqTech`／`PrereqCivic`）を持っている → 出来事の効果を与える（`reason=prereq`。研究を先に終えた勢力は年より早い）
   - 前提の無い出来事で、史実の年（と月）のターンに達した → 効果を与える（`reason=due`）
   - 前提があり、年に達したが未研究 → **ひらめきだけ**を与える（`reason=year`。1勢力1回）。効果は、その勢力が後で研究を終えたターンに `reason=prereq` で与える
3. 効果を与えるとき（`prereq`／`due`）
   - フラグ：プレイヤーのプロパティ `JAC_EVENT_FIRED_<EventType>` に与えたターンを書く。これが「1勢力1回」の記録で、セーブに残る
   - 効果（モディファイア）：`JAC_HistoricalEventModifiers` のモディファイアを付ける。発生回数 `JAC_EVENTS_FIRE_COUNT` を1増やす
4. ひらめきだけのとき（`year`）
   - プロパティ `JAC_EVENT_BOOSTED_<EventType>` に年が来たターンを書く（効果のフラグは書かない）
   - 前提の技術・社会制度のひらめきを発動する。ログの `boost=` に `tech=boosted`／`civic=boosted`（発動）か `no_boost`（`Boosts` の行が無い、または既に発動済み）を出す
5. どちらの場合も行うこと（出来事そのものは史実の年に起きるため）
   - 家名：`JAC_HistoricalEventRenames` の行がその勢力の文明なら、Game プロパティ `JAC_CIV_NAME_<CivilizationType>` に表示名の LOC キーを書く。CustomCivs の `UI/JAC_CivNameOverride.lua` がこれを読み、外交リボン・外交画面・ワールドランキング・都市バナーの文明名を差し替える。長尾→上杉は「1561年、または関連する社会制度を先に得る」（SSOT 3.2、オーナー判断2a）なので、年が来れば未研究でも切り替える
   - 資源：`JAC_HistoricalEventResources` の予約地点を、その出来事が誰かに初めて起きたときに1回だけ開示する（下の「資源を置く出来事」）
   - 通知：`NotificationTypes.USER_DEFINED_1` で本人に（`NotifyAll=1` なら全主要勢力に）送る。ひらめきの後に研究して効果を受けたときは、同じ通知を2度は送らない。文面は LOC キーのまま渡し、各端末の言語で表示する
6. `Lua.log` に `[JAC_EVENTS] fired event=… reason=prereq|due …`（効果）か `[JAC_EVENTS] boosted event=… reason=year … boost=…`（ひらめきだけ）を出す

## データ形式（出来事の一覧を足すとき）

コードを変えずに行を足すだけで出来事を増やせる。調査中の一覧（`documents/research/sengoku-events.md` 予定）は、この形の SQL に変換して `Data/` に置き、modinfo の `JAC_Events_Data` に1行足す。

| テーブル | 列 | 意味 |
|---|---|---|
| `JAC_HistoricalEvents` | `EventType`（主キー）、`Pack`、`Year`、`Month`、`Turn`、`PrereqTech`、`PrereqCivic`、`Name`、`Description`、`NotifyAll` | `Year` と `Turn` のどちらかは必須。両方あれば `Turn` を使う。`Month`（1〜12、任意）は史料の和暦の月。無ければ1月扱い。`Name`・`Description` は通知の見出しと本文の LOC キー |
| `JAC_HistoricalEventTargets` | `EventType`、`CivilizationType` | 対象勢力。行が無ければ全主要勢力 |
| `JAC_HistoricalEventModifiers` | `EventType`、`ModifierId` | 起きた勢力に付けるプレイヤーのモディファイア |
| `JAC_HistoricalEventRenames` | `EventType`、`CivilizationType`、`Name` | 文明の表示名の切替。文明 ID は変えない |
| `JAC_HistoricalEventResources` | `EventType`、`SiteId` | 開示する資源の予約地点。`SiteId` は資源 MOD の地点 ID（`poc/JAC_PoC_JapanMapScale/Scripts/JAC_ResourceSites.lua`）。地点が地図ごとのタイルを持つので、1行で日本マップ4種に効く |

## 暦との関係

Civ6 の公式 Lua には年を数値で返す API が無く、暦は表示用の文字列（`Calendar.MakeYearStr`）しか出さない。公式の Pirates シナリオもターン数から年を計算している。そこで、暦を作っている表そのものから年をターンに換算する。

- 暦は main にマージ済みの戦国パックの暦（[sengoku-calendar.md](sengoku-calendar.md)、PR #34）。`ERA_ANCIENT` の開始年が1534、ゲーム速度ごとに `GameSpeed_Turns`（`MonthIncrement` か月/ターンを `TurnsPerIncrement` ターン）が順に並ぶ
- 換算（`Scripts/JAC_HistoricalEvents.lua` の `TurnForYear`）：ターン1を開始年の1月とし、ターン `t` の手前までに経過した月数が `(年 - 開始年) × 12 + (月 - 1)` に達する最初のターン。開始年は `GameInfo.StartEras['ERA_ANCIENT'].Year`、速度は `GameConfiguration.GetGameSpeedType()`、区間は `GameInfo.GameSpeed_Turns()` を速度で絞って行順に読む（行順は `GameSpeed_Turns` の `ORDER BY rowid`、`01_GameplaySchema.sql:3337`。速度は `InGameTopOptionsMenu.lua:442` と同じ取り方）
- エンジンの現在ターンは1から始まらない。1534年開始の暦では `Game.GetCurrentGameTurn()` と `GameConfiguration.GetStartTurn()` がともに 16603 から始まる（実機、2026-09-29。暦の文字列は「1534 AD」）。そこでスクリプトは、公式の上部パネルが「ターンを1から数える」ために行う `現在 - GetStartTurn() + 1`（`Base/Assets/UI/TopPanel.lua:390`）を `GameTurn()` に取り、上の換算・発生ターンのプロパティ・ログはすべてこの経過ターンで扱う。開始ターンはセーブに含まれる設定なので、セーブ・ロードでも速度が違っても変わらない。暦の年を数値で得る公式 API は無い（`Calendar` は文字列のみ）ので、年からの直接算出は採らない
- 標準速度（4か月/ターン＝1年3ターン）では、月が無い出来事は `ターン = (年 - 1534) × 3 + 1`。鉄砲伝来1543年8月はターン30、足尾銅山1550年（月なし）はターン49、長尾家の上杉家継承1561年（閏）3月はターン83、1600年はターン199
- ほかの速度も同じ表で自動的に換算する。1年あたりのターン数は Online 1.5、Quick 2、Epic 4（200ターンまで。以後6）、Marathon 6（200ターンまで。以後12）
- 暦の月（SSOT 3.2「暦の月までできるだけ合わせて起こす」）：`Month` に史料の和暦の月をそのまま書く（鉄砲伝来は天文12年8月、上杉継承は永禄4年閏3月→3）。和暦と西暦の月のずれは1か月前後で、標準速度の1ターン（4か月）より小さいため換算しない。暦の起点を1月とみなす点は sengoku-calendar.md の未検証事項と同じ。画面は年だけを表示する（SSOT 3.2）ので、月は発生ターンにだけ効く
- 以前の暫定値（`JAC_EVENTS_START_YEAR`／`JAC_EVENTS_TURNS_PER_YEAR` の `GlobalParameters`）は廃止した。静的テスト（`tests/test_historical_events.py`）は、暦の XML から同じ換算を Python で再計算し、全速度で「換算したターンの暦上の年が出来事の年で、その前のターンは前の年」と「月を指定したら、そのターンで初めてその月に達する」を確かめる

## 資源を置く出来事

資源は、main の資源 MOD（[resources.md](resources.md)、`poc/JAC_PoC_JapanMapScale/Scripts/JAC_Resources.lua`）が生成器で予約した地点に置く。出来事の行は地点 ID（例 `copper-ashio`）だけを持ち、発生時に `ExposedMembers.JAC_Resources.RevealSite(SiteId)` を呼ぶ。置いたタイルは `GetSites()` の地点のタイル（15 km は 7482、20 km は 4416）で、Game プロパティ `JAC_EVENT_RESOURCES_<EventType>` に残す。ログは `resource event=… site=… plot=<タイル>`。

- 資源 MOD は自分でも、地点の年か、いずれかの勢力の研究完了（初期知識を除く）で同じ地点を開示する。`RevealSite` は開示済みなら何もしないので、どちらが先でも同じタイルに1つだけ置かれる。出来事の年と前提技術は地点の `year`／`tech` と同じにしてある（テストで照合。足尾は1550年・T06 鉱脈採掘 `TECH_JAC_SENGOKU_ORE_PROSPECTING`）
- 資源 MOD は日本マップ4種でだけ読み込まれる。ほかの地図ではログに `plot=resources_mod_not_loaded` を出して何も置かない
- 以前の「目標タイル周辺で置けるタイルを探す」処理と地図別の SQL（`PlotIndex`）は、資源 MOD の予約地点と別のタイルに置いてしまうため削除した
- 石見銀山（1526年）は開始時から地図に置く（SSOT 3.2、資源 MOD）ので、出来事にはしない

## サンプルの3件（すべて暫定）

| 出来事 | 年（ターン） | 対象 | 前提（早く起きる条件） | 効果 |
|---|---|---|---|---|
| 鉄砲伝来 `JAC_EVENT_TEPPO_DENRAI` | 1543年8月（30） | 全主要勢力 | 技術「鉄砲伝来」`TECH_JAC_SENGOKU_FIREARMS_CONTACT` | 通知。年が来たとき未研究の勢力には技術「鉄砲伝来」のひらめきだけを与える（技術も効果も与えない。研究済みの勢力には影響なし） |
| 足尾銅山の開坑 `JAC_EVENT_ASHIO_COPPER` | 1550（49） | 全主要勢力 | 技術「鉱脈採掘」`TECH_JAC_SENGOKU_ORE_PROSPECTING`（資源 MOD の地点と同じ） | 通知。資源 MOD の予約地点 `copper-ashio` に銅を置く（日本マップ4種のみ）。未研究の勢力にはひらめき |
| 長尾家の上杉家継承 `JAC_EVENT_NAGAO_UESUGI` | 1561年閏3月（83） | 長尾家 | 社会制度「朝廷・将軍への取次」`CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS` | 全勢力に通知。長尾家の表示名を「上杉」（Uesugi／Уэсуги）に切り替える |

`Boosts` の行が無いとひらめきは発動できないため、3件の前提には仮の行（`BOOST_TRIGGER_TRAIN_UNIT`＋ガレー船、`LOC_JAC_EVENT_BOOST_PLACEHOLDER`）を `JAC_HistoricalEvents_Sengoku.sql` で足している（**暫定**）。ツリー側に行があれば足さない。実際のひらめき条件はツリー側で決める。

## ひらめきの公式 API の根拠

- 技術：`DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:761-784`。`player:GetTechs():HasBeenBoosted` で未発動を確かめ、`player:GetTechs():TriggerBoost(techIndex, 1)`（コメント「ゴーディハット由来として発動」）。UI 側の判定は `Base/Assets/UI/Panels/TechBoostsPanel.lua:91`（`HasTech`／`HasBoostBeenTriggered`／`CanTriggerBoost`）
- 社会制度：`Base/Assets/UI/Panels/CivicBoostsPanel.lua:88` が `player:GetCulture()` に同名の `HasCivic`／`HasBoostBeenTriggered`／`CanTriggerBoost` を持つことを示す。`GetCulture():TriggerBoost(civicIndex, 1)` を呼ぶ公式の Lua 例は見つからなかった（**要実機確認**。技術側と同じ呼び出しを、上の3つの判定で守って使う）
- 代替（不採用）：モディファイア `MODIFIER_PLAYER_GRANT_SPECIFIC_TECH_BOOST`（`Base/Assets/Gameplay/Data/Modifiers.xml:2499-2503`）は技術のひらめきだけで、社会制度用の同種は無い。両方を同じ経路で扱うため Lua の API にした

## 決定事項（オーナー回答、B 組）

実装は「サンプルの3件」まで。全一覧への展開は後続作業。

| 番号 | 決定 | 状態 |
|---|---|---|
| 1a | 年が来たとき、前提の技術・社会制度を未研究の勢力には、それ自体ではなくひらめき**だけ**を与える（SSOT 3.2）。研究済みの勢力は影響なし | 実装済み（技術と社会制度。効果は研究を終えたときに与える。上記） |
| 2a | 長尾→上杉は「1561年、または関連する社会制度を先に研究」の条件を維持する | 既存の条件のまま（テストで固定） |
| 6a | 松平→徳川の改名も、長尾→上杉と同じ仕組み（`JAC_HistoricalEventRenames`）の出来事にする | 記録のみ。一覧の実装時に行を足す |
| 7a | 本能寺の変の後、織田家の家名表示を羽柴から豊臣へ切り替える（同じ仕組み） | 記録のみ。一覧の実装時に行を足す |
| 8a | 室町幕府が終わる1573年、足利家の勢力は残り、将軍として裁定する権能だけを失う | 記録のみ。勢力は消さない。裁定の権能の停止は世界会議の実装側で扱う |
| 10a | 関ヶ原の戦いの出来事は置かない | 記録のみ |
| E6b | 史実イベントは暦の月までできるだけ合わせる（SSOT 3.2） | 実装済み（`Month` 列。上記「暦との関係」） |
| E5a | 因縁を解消へ導く和睦を促すイベントは、ひらめきを与え、助言役のポップアップで知らせる（SSOT 3.1） | 後続。ひらめきはこの仕組みの `reason=year` と同じ API で与えられるが、助言役のポップアップ（UI）と和睦の対象・因縁の解消は因縁の実装側で作る。今の通知は `USER_DEFINED_1` の通知だけ |

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
| ゲーム中の資源配置 | 資源 MOD の `RevealSite` に任せる（根拠は [resources.md](resources.md)） |
| 研究済みの判定 | `DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:670`（`HasTech`）、`:555`（`HasCivic`） |
| ゲームプレイ側の文明 ID | `DLC/WarMachineScenario/Scripts/WarMachineScenario.lua:27`（`GetCivilizationTypeName`） |
| 独自テーブルの追加 | `DLC/Expansion2/Data/Expansion2_Schema.sql:2`（`CREATE TABLE` を UpdateDatabase で読む） |
| 年の表示と計算 | `Base/Assets/UI/TopPanel.lua:407`（`Calendar.MakeYearStr` のみ）、`DLC/PiratesScenario/UI/Replacements/TopPanel_PiratesScenario.lua:10-18`（ターンから年を計算） |
| シナリオでの研究の強制 | `DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:38`（`SetTech`）、`DLC/PiratesScenario/Scripts/PiratesScenario_Shared_Script.lua:956`（`SetCivic`） |

## 実機確認（メインセッションで実行）

```powershell
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/historical-events-15km.json --output tools/civ6-autotest/artifacts/historical-events-run-02
```

有効にする MOD は `japan-resources.json` と同じ5つに、この MOD を足したもの。15 km 史実領土版・22勢力・織田が人間（設定は `build_setup` で組み立てる）。

| 手順 | 確かめること |
|---|---|
| 開始直後 | `Lua.log` に `[JAC_EVENTS] loaded events=3 startYear=1534 speed=GAMESPEED_STANDARD`。ターン1で出来事は1件も起きていない |
| ターン1に強制 | 経過ターンは `Game.GetCurrentGameTurn() - GameConfiguration.GetStartTurn() + 1`（生のターンは 16603 などで、シナリオは経過ターンで比べる）。織田に技術「鉄砲伝来」「鉱脈採掘」、長尾家に社会制度「朝廷・将軍への取次」を与える（GameCore_Tuner） |
| 1ターン進めた後 | `fired event=JAC_EVENT_TEPPO_DENRAI player=0 civ=CIVILIZATION_JAC_ODA reason=prereq turn=2 due=30` のログ。織田の鉄砲伝来・足尾銅山と長尾家の上杉継承がターン2で起き、他の勢力の鉄砲伝来は起きていない。発生回数の合計が3。足尾の銅が資源 MOD の予約地点 7482 にあり、資源 MOD も `copper-ashio` を開示済みとしている。`WorldRankings` の画面で長尾家の文明名が `LOC_JAC_EVENT_UESUGI_CIV_NAME` に切り替わっている |
| セーブ→メニュー→ロード | 同じ確認がロード直後にも通る（プロパティ・銅・家名が保存されている） |
| ロード後に1ターン | 発生回数が3のままで、各勢力の発生ターンが2のまま（2度起きていない）。家名の切替も残る |
| ターン30まで進める | 暦が1543年。長尾家（鉄砲伝来を未研究）に `boosted event=JAC_EVENT_TEPPO_DENRAI … reason=year turn=30 due=30 boost=tech=boosted`。長尾家は技術を持たず、効果のフラグ `JAC_EVENT_FIRED_…` も無く、ひらめき（`JAC_EVENT_BOOSTED_…`=30）だけ。織田は技術を持ったまま。長尾家が自分で研究していた場合は `SKIP:` を返すので、ログの `reason=prereq` を確かめる |

判定は `generic`（`Lua.log` の JAC 由来のエラーが無いこと）。

**要実機確認（シナリオでは確かめない）：**

1. 通知の見た目。`ICON_NOTIFICATION_USER_DEFINED_1` に `ICON_ATLAS_NOTIFICATIONS` の15番（公式 Black Death と同じ）を当てたが、絵柄が出来事に合うかは画面で見る。通知はターン終了で消える（公式の既定）
2. 資源の開示は資源 MOD の実機確認（`japan-resources.json`）と同じ。`ResourceBuilder` が使えなければ資源 MOD 側のログに `SetResourceType failed` が出る
3. 年による発生（`reason=year`）で `TriggerBoost` が効くか。技術はシナリオのターン30で確かめる。社会制度の `GetCulture():TriggerBoost` は公式の Lua 例が無いため、長尾家の上杉継承（ターン83）の前に別途確かめる
4. マルチプレイでの同期
