# 天下統一 PoC

SSOT 3.1 の「登録した全城を支配すると天下統一」、3.2 の「時間切れ・スコア勝利なし」を対象とする。実機確認前の実装である。

## 登録と勝利条件

- 史実開始の 15 km・20 km `.Civ6Map` の `Cities` 行を登録城の正本にする。現在は各地図 56 都市で、琉球とアイヌを含む。`tools/japan_map_scale/generate.py` は各行の都市へ `BUILDING_JAC_REGISTERED_CASTLE` を事前配置する。既存の両地図には `tools/japan_map_scale/registered_castles.py --stamp` で同じ処理を適用済み。開拓者開始地図には配置しない。
- この建物は `InternalOnly=1` の内部マーカーで、通常の建築候補には出さない。後から建てた都市は、登録城と同じ地点に再建してもマーカーを引き継がない。`tools/generate_unification_victory.py` が両地図の `Cities` 行を検査し、同数ならその数を SQL の条件値に出力する。数値を勝利コードへ手書きしない。
- `VICTORY_JAC_SENGOKU_UNIFICATION` は `JAC_Sengoku_Victories` に登録する。ネイティブ要件エンジンが「チーム内のいずれか**1人**のプレイヤーの、マーカーがある所有都市の数 = 登録城の数」を判定する。別のチーム員の都市を合算しない。ゲーム側 Lua から勝利画面を擬似的に開かない。
- マーカー建物と都市所有権はゲームのセーブ対象であり、勝利条件はロード後も同じ SQL 要件で評価される設計。ゲーム側 Lua のローカル変数や端末別 UI 状態に依存しないため、マルチプレイの判定も GameCore に任せる。ただしセーブ／ロードとマルチプレイの実機動作は未確認。
- **暫定**: 登録城が破却された場合、そのマーカーは消え、再建都市は登録城として数えないため、天下統一は達成不能になる。**暫定**: 開拓者開始地図および通常の GS 地図には登録城がないため、天下統一は達成不能にする。これらの扱いはオーナー判断待ち。

## 公式実装との照合

以下はすべて `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/` 以下の公式ファイル（行番号はこの PC の導入版）。

| 利用する挙動 | 公式ファイル:行 |
|---|---|
| カスタム勝利を `Types`・`Victories`・`RequirementSets` に登録する | `DLC/AlexanderScenario/Data/AlexanderScenario_Victories.xml:7,14,21,26,45` |
| ルールセットの専用 `VictoryDomain` と勝利選択肢 | `DLC/AlexanderScenario/Data/AlexanderScenario_Config.xml:9,97-99`、`DLC/NubiaScenario/Data/NubiaScenario_Config.xml:50` |
| 勝利名を FrontEnd と InGame の両方へ読み込む `UpdateText` | `DLC/AlexanderScenario/AlexanderScenario.modinfo:62-74,89-107` |
| 勝利対象のチームから各プレイヤーへ要件を渡す `COLLECTION_TEAM_PLAYERS` | `Base/Assets/Gameplay/Data/Victories.xml:734-742,800-807` |
| `REQUIREMENT_COLLECTION_COUNT_EQUALS` と `Count`、下位要件のフィルタ | `Base/Assets/Gameplay/Data/Victories.xml:514-536`、`DLC/NubiaScenario/Data/NubiaScenario_GameplayData.xml:620-651` |
| 都市の建物を要件で判定する `REQUIREMENT_CITY_HAS_BUILDING` | `Base/Assets/Gameplay/Data/Buildings.xml:1800-1802,1894-1897` |
| 建物の `PrereqDistrict` と `InternalOnly`、公式の内部建物例 | `Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:423-460`、`Base/Assets/Gameplay/Data/Buildings.xml:184-186` |
| World Builder が都市へ建物を配置する公式の処理 | `Base/Assets/UI/WorldBuilderPlacement.lua:1233-1239` |
| カスタム勝利を世界ランキングのタブへ載せ、要件文と進捗を読む | `Base/Assets/UI/PartialScreens/WorldRankings.lua:336-342,704,2174-2195,2292-2315` |
| 世界ランキングを開閉する UI イベント | `Base/Assets/UI/PartialScreenHooks.lua:144-146,438`、`Base/Assets/UI/PartialScreens/WorldRankings.lua:2620-2624` |
| `VictoryProgress` 要件文の公式例 | `DLC/BlackDeathScenario/Data/BlackDeathScenario_GameplayData.xml:771-782` |
| 勝利後のチームと勝利タイプをゲームから取得し、固有スタイルのない勝利に汎用画面を使う | `Base/Assets/UI/EndGame/EndGameMenu.lua:896-904,1074-1078` |

登録城の個数を示す独自の常時表示は未実装。公式の World Rankings はカスタム勝利タブと条件文を表示する。実機で標準の進捗表示が有用でなければ、数値表示を追加する TODO とする。

## メインセッションの実機確認

`python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/unification-victory.json` を実行する。事前の静的検証は `python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/unification-victory.json --check`。

このシナリオは 15 km の史実開始、22 勢力、`RULESET_JAC_BANSEIKI` を使用し、`HostGame` のセットアップ更新を経由する。GameCore_Tuner から初期登録城がすべて存在し、勝利が有効なことを確認する。アイヌの 1 城を残して他の登録城を人間の織田へ移し、**55/56 では勝者なし**を確認する。World Rankings の天下統一タブを開いて `unification-progress-negative.png` を撮る。1 ターン進めても勝者が出ないこと、セーブ／ロード後も同じ条件と無勝利状態が保たれることを確認する。最後のアイヌ城を移し、**56/56 で勝者が織田、勝利タイプが `VICTORY_JAC_SENGOKU_UNIFICATION`** になることを確認する。勝利メニューと World Rankings の表記も目視する。

都市移管は FireTuner の GameCore_Tuner で `CityManager.TransferCity(city, owner)` を呼ぶ。公式の `DLC/Expansion2/Binaries/Win64/GameCore_XP2_Release.dll` に `Lua_ICityManager.cpp` の `TransferCity` 登録文字列がある（ファイル内オフセット 14318752）。テキストの公式 Lua 使用例と署名は見つからず、このテスト操作は実機未確認。失敗したら転送呼び出しのエラーを記録し、GameCore の利用可能な引数を実機で確認する。勝利本体はこのデバッグ用 API を使わない。通常の戦闘による都市占領でも内部マーカーが残るかは、この強制移管シナリオとは別に確認する。
