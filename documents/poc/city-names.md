# 各家の都市名（2026-09-30）

日本マップの22勢力（`CIVILIZATION_JAC_*`）が新しく建てる都市に、その家の領国・周辺の史実の地名を付ける。データは [`spec/data/japan-city-name-pools.json`](../spec/data/japan-city-name-pools.json)、生成は `tools/civ6-assets-rs/src/write_city_names.rs`、出力は `poc/JAC_PoC_CustomCivs/Data/JAC_CityNames_Gameplay.sql` と `Text/JAC_CityNames_Text.xml`。

## 不具合と原因

15 km 史実領土版の実機画面（`ages-screens-01/02-era-score-panel.png`、66ターン）で、新しく建った都市の名前が「BALTIMORE」だった。`LOC_CITY_NAME_BALTIMORE` は公式のアメリカの都市名（`Base/Assets/Gameplay/Data/Civilizations.xml:2709`、`CIVILIZATION_AMERICA`）。

修正前は、22勢力すべてが公式の日本（`CIVILIZATION_JAPAN`、同ファイル 3044〜3076 行の33件）の都市名をそのまま複製していた（`JAC_CustomCivs_Gameplay.sql` の織田・武田と、`JAC_Japan1534_Gameplay.sql` の20勢力）。ゲーム内で同じ都市名は二度使われないため、22家が同じ33件を取り合うと早々に尽きる。尽きた家は、ほかの文明の一覧から名前を借りる。この借用はゲーム本体（DLL）の処理で、公式の Lua・XML に実装や説明はない。今回の「BALTIMORE」がその実例になる。

関係する公式の定義：

- `Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:597-607` の `CityNames`（`CivilizationType`・`CityName`・`SortIndex` ほか）
- 同 `:664` の `Civilizations.RandomCityNameDepth`（既定値1）。公式の主要文明は `Civilizations.xml` で10、`AustraliaScenario_GameplayData.xml:129-132` や `WarMachineScenario_GameplayData.xml:77-81` のシナリオ文明は1。値は「`SortIndex` 順で未使用の名前の先頭から何件の中で無作為に選ぶか」と読める（名前と公式の使い分けからの推定。実装は DLL 内で確認できない）

## 修正

1. 各家に、その家の領国（`japan-1534-start.json` の旧国割当）と周辺の城・城下・門前町・港・宿場・荘園の地名を25件以上持たせる。200ターンの健全性検査（#66）で1家あたり最大約13都市だったため、25件あれば1家だけでは尽きない。アイヌは近世の記録にあるアイヌ語地名（カタカナ）、琉球はグスク・間切の名前を使う
2. 家ごとの一覧の後ろに、全家共通の予備として旧国名（令制国）の一覧を付ける（`SortIndex` 1000以上）。家の一覧が尽きても、ほかの文明の名前に行く前にこちらが使われる
3. 地名はゲーム全体で重複させない。家どうし、共通の予備、首都22件、地図の56都市（`LOC_JAC_1534_CITY_*`）、公式の日本の33件と、日英露いずれの表示名も重ならない（例外は既存の織田の那古野と今川の那古野で、地図上の妥協として README に記録済み）
4. 公式の日本の都市名の複製をやめた。首都（`SortIndex` -1）は従来どおり各勢力の定義の隣に置く
5. `RandomCityNameDepth` を22勢力とも3にした。各家の一覧は本拠に近い地名から並べてあり、窓を小さくすると序盤の都市が本拠の近くの地名になる。10のままだと本拠から遠い地名が早く出やすい

表示名：日本語は漢字（アイヌはカタカナ）。英語は長音記号なしのヘボン式、ロシア語はポリワノフ式キリル文字で、どちらも読み（かな）から生成器が機械的に作る。地図の城名（#56、`tools/civ6-assets-rs/src/generate.rs` の `CASTLE_NAMES`）と同じ表記の規則（例：ほうじょう → Hojo／Ходзё、すんぷ → Sunpu／Сумпу）。

## 出典

各地名に日本語版 Wikipedia の記事 URL を付けた（`source`）。2026-09-30 に MediaWiki API で、記事が存在し曖昧さ回避ページでないこと、記事冒頭に読み（かな）があることを確かめた。城は城の記事、寺社は寺社の記事、港・宿場は湊・宿の記事を優先し、なければ地名・自治体の記事にした。記事や読みを確認できなかった地名は採らなかった。例外はアイヌ語地名8件（シブチャリ・ネモロ・クスリ・シコツ・オタルナイ・シマコマキ・リイシリ・ルルモッペ）で、出典は現在の自治体・島の記事、カタカナ表記は記事冒頭に無いため `reading_checked: false` とした。

件数：家固有の地名は1家25〜37件（計612件）、共通の予備は旧国名65件（重複する阿波・和泉・対馬は除外）。すべての地名は日英露の表示名で互いに重ならない。

## 検査

- `tools/civ6-data-rs/src/data_tests.rs`：22勢力すべてに家固有の都市名が25件以上あること、JAC 以外のタグ（公式の `LOC_CITY_NAME_*`）や他文明からの `INSERT ... SELECT` がないこと、家の中で重複がないこと、すべてのタグに日英露の行があること、上の重複禁止、`RandomCityNameDepth`、modinfo の読み込み順、生成物がデータと一致すること、各地名に出典があること、ローマ字化の例
- 実機シナリオ `tools/civ6-autotest/scenarios/japan-house-city-names.json`：15 km 史実領土版を開始し、FireTuner のゲームプレイ側で22勢力それぞれに都市を1つ建てる（`PlayerCities:Create`、公式の `Base/Assets/UI/Tuner/TunerMapPanel.lua:119` と同じ呼び出し）。付いた名前がその家の固有の一覧（`SortIndex` 0〜999）にあることを確かめる。未実機

## 実機確認項目

1. `japan-house-city-names.json` を実行し、22勢力すべてで固有の一覧の名前が付くこと
2. 長時間の対局（200ターン）で「BALTIMORE」のような他文明の名前が出ないこと。`PlayerCities:Create` が開拓者の入植と同じ名付けの経路を通るかは未確認のため、この確認で補う
3. ロシア語・日本語 UI での都市名の表示
