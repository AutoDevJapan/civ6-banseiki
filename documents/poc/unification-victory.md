# 天下統一 PoC

SSOT 3.1 の「登録した全城を支配すると天下統一」「破却された登録城は条件から外れる」「開拓者から始める版では全勢力の本拠（最初の首都）の支配で天下統一」（PR #38 のオーナー判断）と、3.2 の「時間切れ・スコア勝利なし」を対象とする。勝利の成立と勝利画面は実機未確認。

## 登録と勝利条件

- 史実開始の 15 km・20 km `.Civ6Map` の `Cities` 行を登録城の正本にする。現在は各地図 56 都市で、琉球とアイヌを含む。`tools/japan_map_scale/generate.py` の `insert_historical_state()` は各行の都市へ `BUILDING_JAC_REGISTERED_CASTLE` を事前配置する。既存の両地図には `tools/japan_map_scale/registered_castles.py --stamp` で同じ処理を適用済み。開拓者開始地図には配置しない。
- この建物は `InternalOnly=1` の内部マーカーで、通常の建築候補には出さない。後から建てた都市は、登録城と同じ地点に再建してもマーカーを引き継がない。
- `VICTORY_JAC_SENGOKU_UNIFICATION` は `JAC_Sengoku_Victories` に登録する。条件に城の数を書かない。`tools/generate_unification_victory.py` が SQL を生成し、ネイティブ要件エンジンが次のどちらかでチームの勝利を判定する。
  1. **登録城の条件**：マーカーのある都市がゲーム内に1つ以上あり、全都市のうちマーカーのある都市がすべてそのチームの所有である。所有者は主要勢力・都市国家・自由都市を区別しない。
  2. **本拠の条件**：マーカーのある都市がゲーム内に1つもなく、公式の制覇勝利と同じ `REQUIREMENT_TEAM_DOMINATION_VICTORY`（全勢力の最初の首都を支配）を満たす。
- 破却された登録城は、都市とともにマーカーも消えるので、残りの登録城だけが条件になる。史実開始地図では各勢力の最初の首都も登録城であり、公式ルールでは最初の首都を破却できないため、登録城が0件になって本拠の条件へ切り替わることはない。開拓者開始地図と通常の Gathering Storm 地図は登録城が0件なので、本拠の条件で判定する。
- 判定はチーム単位（公式の Alexander シナリオと同じ形）。チーム戦では、同じチームの別プレイヤーが持つ登録城を合算する。旧実装の「1人のプレイヤーで数える」から変わった。**要実機確認**：`REQUIREMENT_PLAYER_OWNS_OBJECT` をチームを対象に評価したときの挙動（公式例はシングルプレイのシナリオだけ）。
- マーカー建物と都市所有権はゲームのセーブ対象であり、勝利条件はロード後も同じ SQL 要件で評価される設計。ゲーム側 Lua のローカル変数や端末別 UI 状態に依存しないため、マルチプレイの判定も GameCore に任せる。マルチプレイの実機動作は未確認。
- World Rankings の天下統一タブには、勝利要件の直下の2件（登録城の条件・本拠の条件）が `RequirementStrings` の文で並ぶ。

## 公式実装との照合

以下はすべて `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/` 以下の公式ファイル（行番号はこの PC の導入版）。

| 利用する挙動 | 公式ファイル:行 |
|---|---|
| カスタム勝利を `Types`・`Victories`・`RequirementSets` に登録する | `DLC/AlexanderScenario/Data/AlexanderScenario_Victories.xml:7,14,21,26,45` |
| ルールセットの専用 `VictoryDomain` と勝利選択肢 | `DLC/AlexanderScenario/Data/AlexanderScenario_Config.xml:9,97-99`、`DLC/NubiaScenario/Data/NubiaScenario_Config.xml:50` |
| 勝利名を FrontEnd と InGame の両方へ読み込む `UpdateText` | `DLC/AlexanderScenario/AlexanderScenario.modinfo:62-74,89-107` |
| `REQUIREMENT_COLLECTION_COUNT_EQUALS` と `Count`、下位要件のフィルタ | `Base/Assets/Gameplay/Data/Victories.xml:514-536`、`DLC/NubiaScenario/Data/NubiaScenario_GameplayData.xml:620-651` |
| 都市の建物を要件で判定する `REQUIREMENT_CITY_HAS_BUILDING` | `Base/Assets/Gameplay/Data/Buildings.xml:1800-1802,1894-1897` |
| 建物の `PrereqDistrict` と `InternalOnly`、公式の内部建物例 | `Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:423-460`、`Base/Assets/Gameplay/Data/Buildings.xml:184-186` |
| World Builder が都市へ建物を配置する公式の処理 | `Base/Assets/UI/WorldBuilderPlacement.lua:1233-1239` |
| カスタム勝利を世界ランキングのタブへ載せ、要件文と進捗を読む | `Base/Assets/UI/PartialScreens/WorldRankings.lua:336-342,704,2174-2195,2292-2315` |
| 世界ランキングを開閉する UI イベント | `Base/Assets/UI/PartialScreenHooks.lua:144-146,438`、`Base/Assets/UI/PartialScreens/WorldRankings.lua:2620-2624` |
| `VictoryProgress` 要件文の公式例 | `DLC/BlackDeathScenario/Data/BlackDeathScenario_GameplayData.xml:771-782` |
| 勝利後のチームと勝利タイプをゲームから取得し、固有スタイルのない勝利に汎用画面を使う | `Base/Assets/UI/EndGame/EndGameMenu.lua:896-904,1074-1078` |
| 全都市を対象に、各都市が「自分の所有」を満たすかを調べる勝利（`COLLECTION_ALL_CITIES`・`REQUIREMENT_COLLECTION_ALL_MET`・`REQUIREMENT_PLAYER_OWNS_OBJECT`） | `DLC/AlexanderScenario/Data/AlexanderScenario_Victories.xml:18,25-30,53-58,72-81` |
| 下位の要件セットを要件として入れ子にする `REQUIREMENT_REQUIREMENTSET_IS_MET` と、いずれかで満たす `REQUIREMENTSET_TEST_ANY` の勝利での使用例 | `DLC/AustraliaScenario/Data/AustraliaScenario_Victories.xml:92-93,111,140-141,167-169` |
| 件数の下限 `REQUIREMENT_COLLECTION_COUNT_ATLEAST` と `Count` | `Base/Assets/Gameplay/Data/Leaders.xml:6532-6533,6748-6756` |
| 要件の否定 `Inverse` | `Base/Assets/Gameplay/Data/Victories.xml:519-523` |
| 公式の制覇勝利の要件 `REQUIREMENT_TEAM_DOMINATION_VICTORY` | `Base/Assets/Gameplay/Data/Victories.xml:36,584-593` |
| 検証地図の地図サイズ `MAPSIZE_DUEL` | `Base/Assets/Configuration/Data/MapSizes.xml:5`、`Base/Assets/Gameplay/Data/Maps.xml:52,105` |

登録城の個数を示す独自の常時表示は未実装。公式の World Rankings はカスタム勝利タブと条件文を表示する。実機で標準の進捗表示が有用でなければ、数値表示を追加する TODO とする。

## 実機確認

2026-09-29 の実機では、15 km 史実開始で55城を `CityManager.TransferCity` で人間へ一括移管すると、手順1〜25（登録56城、55/56で勝者なし、World Rankings の条件表示）は通ったが、その後の1ターンの自動進行がターン1から5分以上進まなかった（阻む画面なし、生存する主要勢力は2）。大きな地図での大量移管で AI が止まるため、勝利・ロード後の無勝利・勝利画面は未確認のまま残った。そこで実機確認を次の2本に分けた。

### 検証地図での勝利確認（`unification-victory.json`）

テスト専用 MOD `poc/JAC_Test_UnificationMap`（`5c1e7a3d-9b2f-4e61-8d47-2a6f0c9e1b85`）に 32×20 の検証地図を置く。`tools/japan_map_scale/unification_test_map.py` が、本番の地図と同じ `generate.create_schema()`・`insert_historical_state()` で作るので、登録城のマーカーは本番と同じく地図データから入る。勢力は織田（人間）・伊達・蘆名の3つ。登録城は4つで、織田が本拠と1城、伊達が本拠と1城を持つ。蘆名は城を持たず、開拓者から始める。

地図の登録は日本マップと同じ `Maps:JAC_Banseiki`・`MapSizes`・`MapDomainOverrides`・`MapLeaders` の形だが、別 MOD にしたので、この MOD を有効にしたときだけ地図一覧に出る。MOD 名と地図名には「テスト専用」を付けた。プレイ用の地図 MOD（`JAC_PoC_JapanMapScale`）の一覧には入れない。

シナリオの流れ：

1. 開始時に登録城4、織田2、勝者なしを確認する
2. GameCore_Tuner で伊達の本拠でない城を `CityManager.DestroyCity` で破却する。`DestroyCity` は GameCore の DLL に文字列があるだけで、署名もどの Manager かも未確認。都市が残った場合は、その城を織田へ移して以降の確認を続け、最後の手順で「破却は未確認」として失敗させる
3. 登録城が1つ足りない状態（伊達の本拠）で勝者なしを確認し、World Rankings の天下統一タブを開いて `unification-progress-negative.png` を撮る
4. 1ターン自動進行 → 勝者なし → セーブ／ロード → 勝者なし
5. 伊達の本拠を織田へ移し、1ターン自動進行 → 勝者が織田のチーム、勝利タイプが `VICTORY_JAC_SENGOKU_UNIFICATION` → `unification-victory.png`
6. 残る登録城が3（破却した1城を除く）で、すべて織田の所有であることを確認する

`CityManager.TransferCity` は公式の `DLC/Expansion2/Binaries/Win64/GameCore_XP2_Release.dll` に `Lua_ICityManager.cpp` の登録文字列がある（ファイル内オフセット 14318752）。`DestroyCity` の文字列はオフセット 14316768。どちらもテキストの公式 Lua 使用例は見つからない。勝利本体はこれらのデバッグ用 API を使わない。

**実行前の準備（1回だけ）**：`Documents\My Games\Sid Meier's Civilization VI\Mods\JAC_Test_UnificationMap` から `poc/JAC_Test_UnificationMap` へジャンクションを張る（[PoC README「共通の準備」](README.md#共通の準備)と同じ方法）。張っていないと、シナリオは「MOD が入っていない」で止まる。

### 本番地図の軽い確認（`unification-registry-15km.json`）

15 km 史実開始、22勢力で開始し、全プレイヤーの都市にマーカーのある登録城が56あること、勝利が有効で勝者がいないことだけを確認する。都市の移管と自動進行はしない。

### 実行コマンド

```
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/unification-victory.json
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/unification-registry-15km.json
```

生成は `python tools/generate_unification_victory.py`（SQL・検証地図・2本のシナリオ）。静的検査は `python -m unittest discover -s tests -t .`。要件の行を Python で模擬評価し、1城不足で勝たない、全城で勝つ、破却後は残りの城で勝つ、後から建てた都市は数えない、史実開始地図では本拠だけでは勝たない、登録城のない地図では本拠の条件で勝つ、を確かめる。

### 未確認・要実機確認

- 開拓者開始地図での本拠の条件による勝利（検証地図は登録城があるため通らない）
- チーム戦での登録城の合算、マルチプレイでの判定
- 自動進行中に勝利が成立したときの `autoplay` の終わり方（ターン数が目標に届かず止まる場合は、`wait_for` の勝者確認が先に失敗する）
- 通常の戦闘による都市占領でマーカーが残るか（この強制移管シナリオとは別に確認する）
