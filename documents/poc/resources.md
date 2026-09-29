# 資源の出現（SSOT 3.2）実装メモ

1534年に未開発の資源は、最初は地図に現れず、技術の研究か史実の年で現れる。参照元はこの PC の `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/`（以下 `Civ6/`）。数値・対応表は暫定。

## 全体像

| 種類 | 仕組み | 実装 |
|---|---|---|
| 種類ごとの技術ゲート | 公式 `Resources.PrereqTech`（未研究の間は資源が見えない） | 戦国ツリーで公式技術が消えても空にならないよう戦国技術へ付け替え済み（`JAC_Sengoku_Unlocks.sql:86-93`）。収穫の `Resource_Harvests.PrereqTech` を今回追加（`JAC_Sengoku_Resources.sql`） |
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

- 生成器（`tools/japan_map_scale/generate.py`）は `start_year` が1534より後の候補（9件）に、開始資源と重ならず海を越えない最寄りの空きタイルを割り当て、`manifest.json` の `resource_candidates`（`placement_status: reserved_for_later_release`、`plot`、`reveal_year`、`reveal_tech`）に記録する。20 km は8件（西三川砂金は佐渡の3タイルが相川・鶴子と取り合いになり置けない。暫定）、15 km は9件。地図の `PlotResources` には入れない。予約タイルは森林・湿地の抽選からも除く。
- 予約の一覧: 生野銀山（1542）、鶴子銀山（1542）、足尾銅山（1550）、土肥金山（1577）、宝達金山（1586）、会津漆器（1590）、西三川砂金（1593）、相川金銀山（1596）、延沢銀山（1596。年範囲 1596〜1615 の下限。暫定）。相川は SSOT 3.2 が例に挙げる地点なので、候補 JSON（`documents/spec/data/japan-map-resources.json`）に今回加えた。
- 技術との対応（暫定。年と技術の**早い方**で現れる）: 生野・鶴子は T09 灰吹法、足尾は T06 鉱脈採掘、土肥・宝達・西三川・相川・延沢は T14 金銀山経営。会津漆器は年のみ。`generate.py` の `REVEAL_TECH`。
- `Scripts/JAC_Resources.lua`（`JAC_PoC_JapanMapScale` の `AddGameplayScripts`）:
  - `ExposedMembers.JAC_Resources.RevealSite(siteId)` 記録済みタイルに資源を置く。置いたら true。再呼び出し・未知の ID・タイルに既存資源ありは false（ログに理由）。置いた ID はゲームプロパティ `JAC_RESOURCE_SITES_REVEALED` に保存（セーブ・ロードで再配置しない）。
  - `RevealDue(year)`、`RevealForTech(techType)`、`GetSites()`、`IsRevealed(siteId)`。イベント枠組みは `RevealSite` を史実イベントから呼べばよい。
  - 枠組みができるまで `Events.TurnBegin` で、年（`Calendar.GetTurnYearForGame(turn)`）と、いずれかの主要勢力の研究完了を見て自動で置く。カレンダーは現状 4000 BC 始まりなので、1534 年に直るまで年による自動出現は起きない（安全側）。
  - ゲーム開始時点で既に持っている技術（初期知識）は解禁の対象にしない。ターン1に各勢力の所持技術を記録し、以後に増えた分だけを見る。
  - 試験用: `GameConfiguration` の `JAC_RESOURCES_TEST_YEAR`（ターン1の年。以後1ターンで+1年）。シナリオ `japan-resources.json` が使う。
- 置く操作は `ResourceBuilder.SetResourceType(plot, resourceIndex, 1)`。公式の使用例は `Civ6/Base/Assets/Maps/Utility/MapUtilities.lua:876`（マップ生成）と `Civ6/Base/Assets/UI/Tuner/TunerMapPanel.lua:59`（実行中のゲームでの配置）。ゲームプレイ用スクリプトからの呼び出しに公式の例は見つけられなかったため、**要実機確認**（失敗時は pcall で拾いログに `SetResourceType failed` を出す）。

## 3. 独自資源

- `RESOURCE_JAC_GOLD`（金）と `RESOURCE_JAC_LACQUER`（漆）。公式シナリオの金 `RESOURCE_GOLD`（`Civ6/DLC/AustraliaScenario/Data/AustraliaScenario_Resources.xml:5,122,142-145,156,192`）の書き方に合わせた。DLC のシナリオ限定なので通常マップでは使わず独自 ID にした。
  - 金: 贅沢資源、金 +4、丘（草・平原）と平地（砂金）、鉱山で改善。
  - 漆: 贅沢資源、文化 +1・金 +1、草・平原・その丘と森林、鉱山で改善（暫定。戦国パックは畑・鉱山・採石場・漁船・砦しか建設できないため）。
  - `Frequency=0`（ランダム生成では撒かない）。前提技術なし。
- 銀は公式では砂漠・ツンドラ系にしか置けない（`Civ6/Base/Assets/Gameplay/Data/Resources.xml:276-279`）ので、生野・鶴子・石見が置けなかった。今回、`JAC_Japan_Resources.xml` で草・平原の丘を許可した。
- アイコンは新規絵なし。公式アトラス `ICON_ATLAS_RESOURCES` のセルを流用（金=ダイヤ `Icons_Resources.xml:26`、漆=染料 `:27`、FOW は `:76-77`）。暫定。
- 開始時に置く資源（生成器で再生成済み。`manifest.json` の `resource_placement`）: 20 km は53件、15 km は57件。新たに置いたのは黒川金山・中山金山・輪島漆・石見銀山（銀が丘に置けるようになったため）。20 km では中山金山が穂坂牧の馬（37.6 km ずれの候補）とタイルを取り合い、馬が1件減った。

## 再生成

`python tools/japan_map_scale/generate.py`（初回は地形データを `tools/japan_map_scale/cache/` に取得）。地図・`Scripts/JAC_ResourceSites.lua`・設定・訳語・シナリオ（`japan-map-*.json`、`japan-resources.json`）が一括で更新される。他ブランチと地図がぶつかったら、統合後にもう一度これを実行する。

## 実機で見る項目（`tools/civ6-autotest/scenarios/japan-resources.json`）

20 km マップ、`JAC_RESOURCES_TEST_YEAR=1541`、戦国ツリーあり。

1. `Modding.log` に `JAC_Japan_Resources (UpdateDatabase)` の適用、`Lua.log` に `[JAC_RESOURCES] loaded`。
2. 独自資源が定義され前提技術なし。`Resources`／`Resource_Harvests` の前提技術が戦国技術だけ。鉄は `IRON_FORGING`、米の収穫は `IRRIGATION_WORKS`。銀が草の丘に置ける。
3. 地図上の金2・漆1・銀1（開始時配置）。
4. ターン1（1541年）は後発の8地点が空。ターン2（1542年）に生野・鶴子だけが銀になり、`Lua.log` に `revealed silver-ikuno`。ターン10（1550年）に足尾の銅が加わり、1577年以降の地点はまだ空。

実機で確認できていない点: ゲームプレイ側の `ResourceBuilder`、研究完了（初期知識を除く）による解禁（シナリオでは駆動できないので、`RevealForTech` の単体確認は別途）、カレンダーが1534年になったときの年の値、アイコンの見た目。

## オーナーへの質問

1. 会津漆・延沢銀など年の範囲がある地点の解禁年（1590／1596）でよいか。
2. 年と技術の割当（上の表）でよいか。初期知識で灰吹法を持つ勢力（石見の大内・尼子など）が最初から持っていても、開始時は解禁しない扱いでよいか。
3. 漆・金の改善を暫定で鉱山にした。茶・絹・塩を活かすために、プランテーション等を戦国パックで建設可にするか。
4. 20 km マップは佐渡が3タイルなので西三川砂金を置けない。相川・鶴子を優先した扱いでよいか。
5. 金・漆のアイコンをダイヤ・染料の流用のままでよいか。
