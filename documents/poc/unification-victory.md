# 天下統一 PoC

SSOT 3.1 の「登録した全城を支配すると天下統一」「破却された登録城は条件から外れる」「開拓者から始める版では全勢力の本拠（最初の首都）の支配で天下統一」（PR #38 のオーナー判断）と、3.2 の「時間切れ・スコア勝利なし」を対象とする。2026-09-30 の実機で、検証地図での勝利の成立（unify-15）と 15 km 地図の登録城56（unify-16）を確認した。開拓者開始・通常地図の本拠の条件、チーム・同盟での合算、マルチプレイは未確認（「未確認・要実機確認」）。

## 登録と勝利条件

- 史実開始の 15 km・20 km `.Civ6Map` の `Cities` 行（都市の中心タイル）を登録城の正本にする。現在は各地図 56 都市で、琉球とアイヌを含む。`tools/generate_unification_victory.py` が各地図の `Cities` 行から、そのタイルの X・Y を並べた SQL（`poc/JAC_PoC_JapanMapScale/Data/JAC_Japan_20km_Castles.sql`・`JAC_Japan_15km_Castles.sql`）を生成する。modinfo の条件で、その地図を選んだときだけ読み込む（`LoadOrder` 300 で勝利の SQL の後）。開拓者開始地図には登録城がない。
- 登録城は「そのタイルに立っている都市」で判定する。公式の WarMachine シナリオがパリを座標で判定するのと同じ `REQUIREMENT_CITY_LOCATION_MATCHES` を使う。占領・割譲・`TransferCity` で所有者が変わってもタイルは変わらないので、登録城のまま数える。旧設計の内部マーカー建物（`BUILDING_JAC_REGISTERED_CASTLE`）は廃止した。都市の中心区域は `CaptureRemovesBuildings=true` で、中心区域の建物は占領時に取り除かれるため、占領された城が条件から黙って外れるおそれがあった（公式データの調査で判明）。
- `VICTORY_JAC_SENGOKU_UNIFICATION` は `JAC_Sengoku_Victories` に登録する。条件に城の数を書かない。`tools/generate_unification_victory.py` が SQL を生成し、ネイティブ要件エンジンが次のどちらかでチームの勝利を判定する。
  1. **登録城の条件**：登録城のタイルに立つ都市がゲーム内に1つ以上あり、それらがすべてそのチームの所有である。所有者は主要勢力・都市国家・自由都市を区別しない。
  2. **本拠の条件**：登録城のタイルに立つ都市がゲーム内に1つもなく、公式の制覇勝利と同じ `REQUIREMENT_TEAM_DOMINATION_VICTORY`（全勢力の最初の首都を支配）を満たす。登録城の SQL を読み込まない地図では、登録城の集合には座標 (-1, -1) の番兵だけが残り、どの都市にも一致しない。
- 破却された登録城はタイルに都市がなくなるので、残りの登録城だけが条件になる。史実開始地図では各勢力の最初の首都も登録城であり、公式ルールでは最初の首都を破却できないため、登録城が0件になって本拠の条件へ切り替わることはない。開拓者開始地図と通常の Gathering Storm 地図は登録城が0件なので、本拠の条件で判定する。
- 破却した登録城のタイルに後から建てた都市は登録城に戻らない（オーナー判断 3a。SSOT 3.1 の表「勝利」の「破却された登録城は条件から外れる」。退けた案 b は「同じ場所に再建すれば登録城に戻る」）。ゲーム側 Lua `poc/JAC_PoC_SengokuTrees/Scripts/JAC_Sengoku_RebuiltCastles.lua` が、その都市に隠しの印の建物 `BUILDING_JAC_REBUILT_ON_CASTLE`（`InternalOnly`、都市の中心）を付け、勝利要件は登録城のタイルに立つ都市のうち、この建物を持たない都市だけを数える（`REQUIREMENT_CITY_HAS_BUILDING` の `Inverse`）。印は「数えない」側にだけ使うので、占領で城の建物が消えても、元の登録城が条件から外れることはない。
  1. 記録：毎ターンの開始（`GameEvents.OnGameTurnStarted`）に登録城のタイルを見て回り、都市がない（`CityManager.GetCityAt` が nil）タイルを破却済みとして記録する。開始ターンは地図に置いた城のため調べない。占領・割譲・`TransferCity` ではタイルが空にならないので、元の城は記録されず数え続ける。破却の後に都市を建てるには、そのタイルへ開拓者を入れる必要があるので、ターン開始の確認が先に空のタイルを見る。記録はタイルごとの `Game` のプロパティで、セーブに含まれる（`JAC_Sengoku_StartingKnowledge.lua` と同じ方式）。公式の削除イベント `Events.CityRemovedFromMap` は UI 側の使用例しかなく、引数に座標がないため使わない
  2. 記録に `GameEvents.CityBuilt` を使わない。2026-09-29 の実機（unify-13）で、`CityManager.TransferCity` の直後に `CityBuilt` が登録城のタイルで発生し、旧実装が移管した元の城を「再建」と誤って記録した。`CityBuilt` は「新しく建てた」の意味にならない
  3. 再付与：印の建物も都市の中心の建物なので占領で消える。`GameEvents.CityConquered`・`GameEvents.CityBuilt`（移管でも発生する）・毎ターンの開始で、破却済みと記録したタイルの都市に印がなければ付け直す。忠誠による離反も毎ターンの確認で拾う
  4. 見逃し：同じターンのうちに破却と再建が両方起きると、ターン開始の確認が空のタイルを見ないので記録されない。通常のプレイでは起きにくいため、実際に起きたらプレイヤーごとのターン開始でも確認する
  5. 付け直すまでの間（占領したターン内）は、再建都市が一時的に登録城として数えられる。所有者が勝つ側に数える城が増えるだけなので、誤って勝利が成立することはない
- 天下統一同盟（SSOT 3.1）は勝利要件の3つ目の枝で扱う。詳細は [天下統一の合算 PoC](unification-pact.md)。
- 判定はチーム単位（公式の Alexander シナリオと同じ形）。チーム戦では、同じチームの別プレイヤーが持つ登録城を合算する。旧実装の「1人のプレイヤーで数える」から変わった。**要実機確認**：`REQUIREMENT_PLAYER_OWNS_OBJECT` をチームを対象に評価したときの挙動（公式例はシングルプレイのシナリオだけ）。
- 登録城の座標はゲームのデータベース（MOD の SQL）にあり、都市の位置と所有権はゲームのセーブ対象である。勝利条件はロード後も同じ SQL 要件で評価される設計。ゲーム側 Lua や端末別 UI 状態に依存しないため、マルチプレイの判定も GameCore に任せる。マルチプレイの実機動作は未確認。
- World Rankings の天下統一タブには、勝利要件の直下の2件（登録城の条件・本拠の条件）が `RequirementStrings` の文で並ぶ。

## 公式実装との照合

以下はすべて `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/` 以下の公式ファイル（行番号はこの PC の導入版）。

| 利用する挙動 | 公式ファイル:行 |
|---|---|
| カスタム勝利を `Types`・`Victories`・`RequirementSets` に登録する | `DLC/AlexanderScenario/Data/AlexanderScenario_Victories.xml:7,14,21,26,45` |
| ルールセットの専用 `VictoryDomain` と勝利選択肢 | `DLC/AlexanderScenario/Data/AlexanderScenario_Config.xml:9,97-99`、`DLC/NubiaScenario/Data/NubiaScenario_Config.xml:50` |
| 勝利名を FrontEnd と InGame の両方へ読み込む `UpdateText` | `DLC/AlexanderScenario/AlexanderScenario.modinfo:62-74,89-107` |
| `REQUIREMENT_COLLECTION_COUNT_EQUALS` と `Count`、下位要件のフィルタ | `Base/Assets/Gameplay/Data/Victories.xml:514-536`、`DLC/NubiaScenario/Data/NubiaScenario_GameplayData.xml:620-651` |
| 都市を座標（X・Y）で判定する `REQUIREMENT_CITY_LOCATION_MATCHES`（勝利要件の都市コレクション内での使用例） | `DLC/WarMachineScenario/Data/WarMachineScenario_GameplayData.xml:437,443,448-451,460` |
| 公式シナリオが特定の都市を座標で扱う例（占領イベントで座標を照合） | `DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:174,192-207,248` |
| 都市の中心区域は占領時に区域内の建物を取り除く（`CaptureRemovesBuildings`）。建物の列（`Capital`・`MustPurchase`・`InternalOnly`・`IsWonder` など）に占領で残す指定はない | `Base/Assets/Gameplay/Data/Districts.xml:43`、`Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:423-470,1031` |
| 宮殿は `Capital=true` の中心区域の建物。首都の扱いは建物でなく元の首都としてエンジンが追跡し、制覇勝利が使う | `Base/Assets/Gameplay/Data/Buildings.xml:98`、`Base/Assets/Gameplay/Data/Victories.xml:36,584-593` |
| カスタム勝利を世界ランキングのタブへ載せ、要件文と進捗を読む | `Base/Assets/UI/PartialScreens/WorldRankings.lua:336-342,704,2174-2195,2292-2315` |
| 世界ランキングを開閉する UI イベント | `Base/Assets/UI/PartialScreenHooks.lua:144-146,438`、`Base/Assets/UI/PartialScreens/WorldRankings.lua:2620-2624` |
| `VictoryProgress` 要件文の公式例 | `DLC/BlackDeathScenario/Data/BlackDeathScenario_GameplayData.xml:771-782` |
| 勝利後のチームと勝利タイプをゲームから取得し、固有スタイルのない勝利に汎用画面を使う | `Base/Assets/UI/EndGame/EndGameMenu.lua:896-904,1074-1078` |
| 全都市を対象に、各都市が「自分の所有」を満たすかを調べる勝利（`COLLECTION_ALL_CITIES`・`REQUIREMENT_COLLECTION_ALL_MET`・`REQUIREMENT_PLAYER_OWNS_OBJECT`） | `DLC/AlexanderScenario/Data/AlexanderScenario_Victories.xml:18,25-30,53-58,72-81` |
| 下位の要件セットを要件として入れ子にする `REQUIREMENT_REQUIREMENTSET_IS_MET` と、いずれかで満たす `REQUIREMENTSET_TEST_ANY` の勝利での使用例 | `DLC/AustraliaScenario/Data/AustraliaScenario_Victories.xml:92-93,111,140-141,167-169` |
| 件数の下限 `REQUIREMENT_COLLECTION_COUNT_ATLEAST` と `Count` | `Base/Assets/Gameplay/Data/Leaders.xml:6532-6533,6748-6756` |
| 要件の否定 `Inverse` | `Base/Assets/Gameplay/Data/Victories.xml:519-523` |
| 都市が建物を持つかの要件 `REQUIREMENT_CITY_HAS_BUILDING` と引数 `BuildingType` | `Base/Assets/Gameplay/Data/Buildings.xml:1800-1801,1894-1896` |
| 建物の `InternalOnly` 列 | `Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:423,459` |
| 都市の建設イベント `GameEvents.CityBuilt(playerID, cityID, x, y)`（ゲーム側スクリプト。`TransferCity` でも発生することを実機で確認） | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:56,255` |
| 占領イベント `GameEvents.CityConquered(capturerID, ownerID, cityID, x, y)` | `DLC/NubiaScenario/Scripts/NubiaScenario.lua:937,969` |
| 毎ターンのイベント `GameEvents.OnGameTurnStarted` | `DLC/AlexanderScenario/Scripts/AlexanderScenario.lua:246` |
| 都市の削除イベント `Events.CityRemovedFromMap(playerID, cityID)`（UI 側だけ。使わない） | `Base/Assets/UI/WorldView/CityBannerManager.lua:1929,2900` |
| タイルの都市を取る `CityManager.GetCityAt`（ゲーム側スクリプト。都市がなければ nil） | `DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:507-508` |
| Tuner で都市を置く `Players[p]:GetCities():Create(x, y)`（検証シナリオの再建に使う） | `Base/Assets/UI/Tuner/TunerMapPanel.lua:111-121` |
| 建物を付ける `CreateIncompleteBuilding(建物, タイル, 100)`（Tuner の使用例だけ） | `Base/Assets/UI/Tuner/TunerCityPanel.lua:10,103-104` |
| 都市が建物を持つか `GetBuildings():HasBuilding` | `Base/Assets/UI/CitySupport.lua:728` |
| 公式の制覇勝利の要件 `REQUIREMENT_TEAM_DOMINATION_VICTORY` | `Base/Assets/Gameplay/Data/Victories.xml:36,584-593` |
| 検証地図の地図サイズ `MAPSIZE_DUEL` | `Base/Assets/Configuration/Data/MapSizes.xml:5`、`Base/Assets/Gameplay/Data/Maps.xml:52,105` |

登録城の個数を示す独自の常時表示は未実装。公式の World Rankings はカスタム勝利タブと条件文を表示する。実機で標準の進捗表示が有用でなければ、数値表示を追加する TODO とする。

## 実機確認

2026-09-29 の実機では、15 km 史実開始で55城を `CityManager.TransferCity` で人間へ一括移管すると、手順1〜25（登録56城、55/56で勝者なし、World Rankings の条件表示）は通ったが、その後の1ターンの自動進行がターン1から5分以上進まなかった（阻む画面なし、生存する主要勢力は2）。大きな地図での大量移管で AI が止まるため、勝利・ロード後の無勝利・勝利画面は未確認のまま残った。そこで実機確認を次の2本に分けた。

### 検証地図での勝利確認（`unification-victory.json`）

テスト専用 MOD `poc/JAC_Test_UnificationMap`（`5c1e7a3d-9b2f-4e61-8d47-2a6f0c9e1b85`）に 32×20 の検証地図を置く。`tools/japan_map_scale/unification_test_map.py` が、本番の地図と同じ `generate.create_schema()`・`insert_historical_state()` で作るので、登録城（`Cities` 行）は本番と同じく地図データから入る。登録城の座標 SQL（`Data/JAC_Unification_Test_Castles.sql`）もこの MOD に置き、検証地図を選んだときだけ読み込む。勢力は織田（人間）・伊達・蘆名の3つ。登録城は4つで、織田が本拠と1城、伊達が本拠と1城を持つ。蘆名は城を持たず、開拓者から始める。

地図の登録は日本マップと同じ `Maps:JAC_Banseiki`・`MapSizes`・`MapDomainOverrides`・`MapLeaders` の形だが、別 MOD にしたので、この MOD を有効にしたときだけ地図一覧に出る。MOD 名と地図名には「テスト専用」を付けた。プレイ用の地図 MOD（`JAC_PoC_JapanMapScale`）の一覧には入れない。

シナリオの流れ：

1. 開始時に登録城4、織田2、勝者なしを確認する
2. GameCore_Tuner で伊達の本拠でない城を `CityManager.DestroyCity` で破却する。`DestroyCity` は GameCore の DLL に文字列があるだけだが、2026-09-29 の実機で都市が消えた。都市が残った場合は、その城を織田へ移して以降の確認を続け、再建の手順で「破却の記録がない」として失敗させる
3. 登録城が1つ足りない状態（伊達の本拠）で勝者なしを確認し、World Rankings の天下統一タブを開いて `unification-progress-negative.png` を撮る
4. 1ターン自動進行（次のターンの開始で破却を記録）→ 勝者なし → セーブ／ロード → 勝者なし
5. GameCore_Tuner で、破却の記録がロード後も残り、そのタイルだけが空であることを確かめ、そこへ蘆名の都市を置く（公式の Tuner と同じ `GetCities():Create`）
6. 1ターン自動進行 → 再建した都市に印があり、元の城にはなく、勝者なし
7. 伊達の本拠を織田へ移す → GameCore_Tuner で、残る登録城3（破却した1城を除く）がすべて織田の所有で、移した城に印がないこと、再建した都市が蘆名のままで印があることを確認する。再建した都市が数えられていれば、ここで勝利は成立しない
8. 1ターン自動進行を始め、勝者が織田のチーム、勝利タイプが `VICTORY_JAC_SENGOKU_UNIFICATION` になるのを待つ → `unification-victory.png`

`CityManager.TransferCity` は公式の `DLC/Expansion2/Binaries/Win64/GameCore_XP2_Release.dll` に `Lua_ICityManager.cpp` の登録文字列がある（ファイル内オフセット 14318752）。`DestroyCity` の文字列はオフセット 14316768。どちらもテキストの公式 Lua 使用例は見つからない。勝利本体はこれらのデバッグ用 API を使わない。

**実行前の準備（1回だけ）**：`Documents\My Games\Sid Meier's Civilization VI\Mods\JAC_Test_UnificationMap` から `poc/JAC_Test_UnificationMap` へジャンクションを張る（[PoC README「共通の準備」](README.md#共通の準備)と同じ方法）。張っていないと、シナリオは「MOD が入っていない」で止まる。

2026-09-29 の実機（unify-06・08・11・12・13）では、最後の城を移した直後に勝利画面（`EndGameMenu: Showing EndGame Menu`）が出た。勝利はターンの終わりを待たずに成立する。unify-08 では勝者が織田のチーム・天下統一であることも確認できた。一方、その後の InGame（UI 側）の数え直しは、30秒待っても「所有2/登録城3」のままだった。GameCore は移管済みとして勝利を判定しているので、UI 側の都市一覧が勝利後に更新されないと読める。そこで移管後の所有と印の確認は GameCore_Tuner で行う。unify-13 では、移管の直後に旧実装が「city rebuilt on razed castle 25,5」を出力した。`GameEvents.CityBuilt` が `TransferCity` でも発生するためで、破却の記録を上記のターン開始の確認に改めた。

2026-09-30 の実機結果：

1. unify-15（`unification-victory.json`）PASS。検証地図で登録城を破却 → 次のターンの開始で破却を記録 → セーブ／ロード後も記録が残る → 破却したタイルに建て直した都市に印が付き、登録城として数えない → 最後の城を織田へ移す → 勝者が織田のチーム、勝利タイプが `VICTORY_JAC_SENGOKU_UNIFICATION`
2. unify-16（`unification-registry-15km.json`）PASS。15 km 史実開始地図が登録城56で始まる

### 本番地図の軽い確認（`unification-registry-15km.json`）

15 km 史実開始、22勢力で開始し、56の登録城のタイルすべてに都市が立っていること、勝利が有効で勝者がいないことだけを確認する。都市の移管と自動進行はしない。

### 実行コマンド

```
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/unification-victory.json
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/unification-registry-15km.json
```

生成は `python tools/generate_unification_victory.py`（勝利の SQL・地図ごとの登録城の SQL・検証地図・2本のシナリオ）。静的検査は `python -m unittest discover -s tests -t .`。要件の行を Python で模擬評価し、1城不足で勝たない、全城で勝つ、破却後は残りの城で勝つ、登録城のタイル以外に建てた都市は数えない、破却した登録城のタイルに建て直した都市（印の建物あり）は数えない、占領・移管した登録城は新しい所有者の城として数える、読み込み順に依存しない、史実開始地図では本拠だけでは勝たない、登録城のない地図では本拠の条件で勝つ、を確かめる。

### 未確認・要実機確認

- 開拓者開始地図での本拠の条件による勝利（検証地図は登録城があるため通らない）
- チーム戦での登録城の合算、マルチプレイでの判定
- 自動進行中に勝利が成立したときの `autoplay` の終わり方（ターン数が目標に届かず止まる場合は、`wait_for` の勝者確認が先に失敗する）
- `INSERT OR IGNORE` を含む SQL と、別 MOD の `LoadOrder` 300 の読み込み順（Database.log にエラーがないこと）
- 通常の戦闘による都市占領でも登録城として数えられるか（タイルで判定するので残る設計。強制移管シナリオとは別に確認する）
- 開拓者による再建と、戦闘での占領後の印の再付与（検証地図では Tuner で都市を置いた。ターン開始での破却の記録、ロード後の記録、ゲーム側の `CreateIncompleteBuilding`、移管した城に印が付かないことは unify-15 で確認済み）
- 同じターン内の破却と再建：ターン開始の確認より前に建て直すと、破却の記録がないまま再建した都市が登録城として数えられるおそれがある。都市が消えた後に開拓者がそのタイルへ移って建てる必要があるので、起こるのは開拓者が隣に待機していた場合だけ。確認項目として残す
