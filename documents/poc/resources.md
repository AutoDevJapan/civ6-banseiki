# 資源の出現（SSOT 3.2）実装メモ

1534年に未開発の資源は、最初は地図に現れず、技術の研究か史実の年で現れる。参照元はこの PC の `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/`（以下 `Civ6/`）。数値・対応表は暫定。

## 全体像

| 種類 | 仕組み | 実装 |
|---|---|---|
| 種類ごとの技術ゲート | 公式 `Resources.PrereqTech`（未研究の間は資源が見えない） | 戦国ツリーで公式技術が消えても空にならないよう戦国技術へ付け替え済み（`JAC_Sengoku_Unlocks.sql:88-95`）。収穫の `Resource_Harvests.PrereqTech` を今回追加（`JAC_Sengoku_Resources.sql`） |
| 場所ごとの後発資源 | 記録済みの地点に Lua で資源を置く | `Scripts/JAC_Resources.lua`。地点データは生成器が `Scripts/JAC_ResourceSites.lua` に出力 |
| 独自資源 | `RESOURCE_JAC_GOLD`、`RESOURCE_JAC_LACQUER` | `Data/JAC_Japan_Resources.xml`（アイコン・訳語も別ファイル） |

## 1. 技術ゲート（公式仕組みの付け替え）

- `Resources.PrereqTech` も `Resource_Harvests.PrereqTech` も `ON DELETE SET NULL`（`Civ6/Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:2325-2343, 2360-2368`）。戦国ツリーは公式技術を全部削除する（`JAC_Sengoku_Unlocks.sql` の最後の `DELETE FROM Technologies`）ので、付け替えなければ**永遠に隠れるのではなく、1ターン目から見える／収穫できる**状態になる。この種類の問題を、公式の全資源行で洗い出した。
- 公式の `PrereqTech` を持つ資源は `Civ6/Base/Assets/Gameplay/Data/Resources.xml:214-220`（アルミ・石炭・馬・鉄・硝石・石油・ウラン）と、GS の更新 `Civ6/DLC/Expansion2/Data/Expansion2_Resources.xml:9-30`（馬 `TECH_ANIMAL_HUSBANDRY`、石油 `TECH_REFINING`）。
- 付け替え後（既存 + 今回）:

| 資源 | ゲート |
|---|---|
| 鉄 | `TECH_JAC_SENGOKU_IRON_FORGING`（T03、根ノード） |
| 硝石 | `TECH_JAC_SENGOKU_FIREARMS_CONTACT`（T12）。日本マップには鉱床を置かない |
| 石炭・石油・アルミ・ウラン | `TECH_JAC_SENGOKU_OUTER_DEFENSES`（T32、早期出現を防ぐ仮置き） |
| 馬 | なし（最初から見える） |
| 米・小麦・バナナの収穫 | `TECH_JAC_SENGOKU_IRRIGATION_WORKS`（今回） |
| 銅・石材の収穫 | `TECH_JAC_SENGOKU_ORE_PROSPECTING`（今回） |
| 魚・蟹の収穫 | `TECH_JAC_SENGOKU_COASTAL_PILOTAGE`（今回） |
| 牛・鹿・羊の収穫 | なし（日本マップに置かない） |

- 日本マップに置く資源の型（魚・馬・鉄・米・塩・絹・茶・鯨・銀・金・漆）で、戦国以外の技術を指すものは残らない。テストは `tests/test_resources.py`（公式データから `Resources` と `Resource_Harvests` の前提技術を読み、モデル DB で「付け替え → 公式技術の削除」を再現して確かめる）。
- 資源ゲートは「種類」単位なので、**地点単位の後発**（生野・鶴子・延沢・足尾・相川など）は次の Lua で扱う。

## 2. 後発地点（Lua API と地点データ）

- 生成器（`tools/japan_map_scale/generate.py`）は `start_year` が1534より後の候補（9件）に、開始資源と重ならず海を越えない最寄りの空きタイルを割り当て、`manifest.json` の `resource_candidates`（`placement_status: reserved_for_later_release`、`plot`、`reveal_year`、`reveal_tech`）に記録する。採用縮尺の 15 km（SSOT 3.4）は9件すべて。20 km は比較用の地図で機能をそろえない（SSOT 3.4）ため、佐渡が3タイルで相川・鶴子と取り合いになる西三川砂金は置かず8件のまま。20 km に無理に置く処理は入れない。地図の `PlotResources` には入れない。予約タイルは森林・湿地の抽選からも除く。
- 予約の一覧: 生野銀山（1542）、鶴子銀山（1542）、足尾銅山（1550）、土肥金山（1577）、宝達金山（1586）、会津漆器（1590）、西三川砂金（1593）、相川金銀山（1596）、延沢銀山（1596。年範囲 1596〜1615 の下限。暫定）。相川は SSOT 3.2 が例に挙げる地点なので、候補 JSON（`documents/spec/data/japan-map-resources.json`）に今回加えた。
- 技術との対応（暫定。年と技術の**早い方**で現れる）: 生野・鶴子は T09 灰吹法、足尾は T06 鉱脈採掘、土肥・宝達・西三川・相川・延沢は T14 金銀山経営。会津漆器は年のみ。`generate.py` の `REVEAL_TECH`。
- `Scripts/JAC_Resources.lua`（`JAC_PoC_JapanMapScale` の `AddGameplayScripts`。日本マップ4種を選んだときだけ読み込む。条件は他の地図用アクションと同じ `JAC_Japan_{20,15}km_{Historical,Settlers}`）:
  - `ExposedMembers.JAC_Resources.RevealSite(siteId)` 記録済みタイルに資源を置く。置いたら true。再呼び出し・未知の ID・タイルに既存資源ありは false（ログに理由）。置いた ID はゲームプロパティ `JAC_RESOURCE_SITES_REVEALED` に保存（セーブ・ロードで再配置しない）。
  - `RevealDue(year)`、`RevealForTech(techType)`、`GetSites()`、`IsRevealed(siteId)`。イベント枠組みは `RevealSite` を史実イベントから呼べばよい。
  - 枠組みができるまで `Events.TurnBegin` で、暦の年と、いずれかの主要勢力の研究完了を見て自動で置く。
  - 暦の年（`CurrentYear()`、API にも公開）は、エンジンが日付に使うデータから計算する: 開始時代の `StartEras.Year`（`Civ6/Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:2545-2560`。戦国パックでは `JAC_Sengoku_Calendar.xml` が1534にする）＋経過ターンの月数（`GameSpeed_Turns` を行順に。同 `:1403-1408`、行順は `:3337`）÷12。経過ターンは `Game.GetCurrentGameTurn() - GameConfiguration.GetStartTurn()`。1534年開始ではエンジンのターン番号が約16603から始まるため（`sengoku-calendar.md`）、公式の表示も同じ引き算をしている（`Civ6/DLC/Expansion2/UI/Replacements/TopPanel.lua:387-389`）。標準速度は4か月/ターンなので、1534年から24ターンで1542年。
  - 公式 Lua に「ターン→年」を返す関数の使用例はない（`Calendar.MakeYearStr` は表示文字列だけ）。そのため数値はデータから計算し、実機シナリオで `Calendar.MakeYearStr` の文字列と一致するかを確かめる。以前の試験用フック `JAC_RESOURCES_TEST_YEAR` は削除した。
  - ゲーム開始時点で既に持っている技術（初期知識）は解禁の対象にしない。ターン1に各勢力の所持技術を記録し、以後に増えた分だけを見る。
- 置く操作は `ResourceBuilder.SetResourceType(plot, resourceIndex, 1)`。公式の使用例は `Civ6/Base/Assets/Maps/Utility/MapUtilities.lua:876`（マップ生成）と `Civ6/Base/Assets/UI/Tuner/TunerMapPanel.lua:59`（実行中のゲームでの配置）。ゲームプレイ用スクリプトからの呼び出しに公式の例は見つけられなかったため、**要実機確認**（失敗時は pcall で拾いログに `SetResourceType failed` を出す）。

## 3. 独自資源と改良

- 農園: 戦国パックでも建設できるようにした（`JAC_Sengoku_Unlocks.sql`）。前提技術は T01 用水普請 `TECH_JAC_SENGOKU_IRRIGATION_WORKS`。公式の農園の前提は `TECH_IRRIGATION`（`Civ6/Base/Assets/Gameplay/Data/Improvements.xml:43`）で、戦国ツリーで灌漑にあたるのが用水普請のため。用水普請は第I期の根ノードで初期知識に入るので、茶・絹・漆は開始時から農園で改良できる（1534年時点で栽培されていたため妥当と判断。暫定）。茶・絹の農園の行は公式のまま（`Improvements.xml:263,266`）。
- 鉱山を平地に置けるか（レビュー Must 3）: 公式データでは置ける。鉱山自体の地形は丘だけ（`Improvements.xml:105-109`）だが、`Improvement_ValidResources` に載った資源のタイルなら地形に関係なく置ける。根拠は、塩が平地（砂漠・平原・ツンドラ）にしか出ない（`Civ6/Base/Assets/Gameplay/Data/Resources.xml:273-275`）のに鉱山でしか改良できない（`Improvements.xml:236`）こと、公式シナリオの金も平地（平原・砂漠）に出て鉱山で改良する（`Civ6/DLC/AustraliaScenario/Data/AustraliaScenario_Resources.xml:144-145,192`）こと。なお鉄は公式では丘にしか出ない（`Resources.xml:295-298`）ので、「平地の鉄を鉱山で掘る」例ではない。JAC の金はこの仕組み（`IMPROVEMENT_MINE` と `RESOURCE_JAC_GOLD` の行）だけで平地の砂金を鉱山にする。地形の条件は増やしていない。エンジン内部の判定なので、実機では金の平地タイルで鉱山が建てられるかを見る（下の項目5）。
- テスト（`tests/test_resources.py` の `ImprovementTests`）: 塩の例で上の仕組みが公式データに成り立つこと、戦国パックの SQL 適用後に農園・鉱山が建設可能なこと、4つの地図で置いた・予約した金・漆・茶・絹の全タイルが対応する改良を受けられる（`Improvement_ValidResources` に行があり、地物があるなら除去不要）こと。


- `RESOURCE_JAC_GOLD`（金）と `RESOURCE_JAC_LACQUER`（漆）。公式シナリオの金 `RESOURCE_GOLD`（`Civ6/DLC/AustraliaScenario/Data/AustraliaScenario_Resources.xml:5,122,142-145,156,192`）の書き方に合わせた。DLC のシナリオ限定なので通常マップでは使わず独自 ID にした。
  - 金: 贅沢資源、金 +4、丘（草・平原）と平地（砂金）、鉄と同じく鉱山で改善（SSOT 3.2）。
  - 漆: 贅沢資源、文化 +1・金 +1、草・平原・その丘と森林、茶・絹と同じく農園で改善（SSOT 3.2）。森林は残したまま（絹と同じ `MustRemoveFeature="false"`、`Civ6/Base/Assets/Gameplay/Data/Improvements.xml:263`）。
  - `Frequency=0`（ランダム生成では撒かない）。前提技術なし。
- 銀は公式では砂漠・ツンドラ系にしか置けない（`Civ6/Base/Assets/Gameplay/Data/Resources.xml:276-279`）ので、生野・鶴子・石見が置けなかった。今回、`JAC_Japan_Resources.xml` で草・平原の丘を許可した。
- アイコンは新規絵なし。公式アトラス `ICON_ATLAS_RESOURCES` のセルを流用（金=ダイヤ `Icons_Resources.xml:26`、漆=染料 `:27`、FOW は `:76-77`）。暫定。
- 開始時に置く資源（生成器で再生成済み。`manifest.json` の `resource_placement`）: 20 km は53件、15 km は57件。新たに置いたのは黒川金山・中山金山・輪島漆・石見銀山（銀が丘に置けるようになったため）。20 km では中山金山が穂坂牧の馬（37.6 km ずれの候補）とタイルを取り合い、馬が1件減った。

## 再生成

`python tools/japan_map_scale/generate.py`（初回は地形データを `tools/japan_map_scale/cache/` に取得）。地図・`Scripts/JAC_ResourceSites.lua`・設定・訳語・シナリオ（`japan-map-*.json`、`japan-resources.json`）が一括で更新される。他ブランチと地図がぶつかったら、統合後にもう一度これを実行する。

## 実機で見る項目（`tools/civ6-autotest/scenarios/japan-resources.json`）

採用縮尺の 15 km マップ、標準速度、戦国パックの1534年の暦、戦国ツリーあり。試験用の年の上書きはしない。

1. `Modding.log` に `JAC_Japan_Resources (UpdateDatabase)` の適用、`Lua.log` に `[JAC_RESOURCES] loaded`。
2. 独自資源が定義され前提技術なし。`Resources`／`Resource_Harvests` の前提技術が戦国技術だけ。鉄は `IRON_FORGING`、米の収穫は `IRRIGATION_WORKS`。銀が草の丘に置ける。農園が建設可能で前提は `IRRIGATION_WORKS`。金・鉄→鉱山、漆・茶・絹→農園の行がある。
3. 開始時に置いた資源（`manifest.json` で `placed` の全件）がそれぞれのタイルにある。後発の9地点すべてが `ExposedMembers.JAC_Resources.GetSites()` に同じタイルで載っている。1件でも欠ければ失敗にする。
4. 開始時は `CurrentYear()` が1534で、`Calendar.MakeYearStr` も1534。24ターン進めると両方1542になり、生野・鶴子が銀になる（`Lua.log` に `revealed silver-ikuno`）。1543年以降の地点は、その技術をどこかの勢力が研究していなければ空のまま（研究による早い出現は SSOT 3.2 どおりなので許す）。
5. 金の平地タイルに鉱山、漆・茶・絹に農園を建てられるか（シナリオでは建設を駆動しないので、目視か別途）。

実機で確認できていない点: ゲームプレイ側の `ResourceBuilder`、研究完了（初期知識を除く）による解禁（シナリオでは駆動できないので、`RevealForTech` の単体確認は別途）、`CurrentYear()` とエンジンの日付の一致（上の項目4で確かめる）、平地の金への鉱山の建設、アイコンの見た目。

## オーナーへの質問

1. 会津漆・延沢銀など年の範囲がある地点の解禁年（1590／1596）でよいか。
2. 年と技術の割当（上の表）でよいか。初期知識で灰吹法を持つ勢力（石見の大内・尼子など）が最初から持っていても、開始時は解禁しない扱いでよいか。
3. 金・漆のアイコンをダイヤ・染料の流用のままでよいか。
4. SSOT 3.2 には「20 km の地図でも佐渡の西三川の砂金を置く」とあるが、3.4 は 20 km を比較用とし機能をそろえないとしている。今回は 3.4 に従い 20 km には置いていない。3.2 の該当文を削るか、20 km にも置くかを決めてほしい。
5. 農園の前提技術を T01 用水普請（開始時から使える）にした。後の技術にするか。
