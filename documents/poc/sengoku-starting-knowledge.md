# 戦国パックの初期知識（1534年）

## SSOT と選択

[SSOT 3.2](../spec/ssot/03-map-and-scale.md) は、1534年開始時に第I期の技術・社会制度の一部を全勢力へ共通付与すると定める。個別ノードは未指定のため、[戦国ツリー案](../research/sengoku-tech-civic-tree.md)の前提なしの第I期ノードだけを選ぶ。技術4/32、制度3/30で、次列以降は通常研究に残す。

| 種別 | 共通知識 | 理由 |
|---|---|---|
| 技術 | T01 用水普請、T02 沿岸航法、T03 砂鉄鍛冶、T04 堀切・土塁 | 1534年以前からある農業用水・近海移動・鉄器・城郭土木の基礎を「知っている」とする。沿岸航法は船舶保有や沿岸拠点を意味しない |
| 社会制度 | C01 家中掟、C02 国人盟約、C03 寺社と信徒 | 既存の支配組織、地域間の盟約、信仰集団との関係を抽象化する。取得によって協定・宗教・建物そのものを自動付与しない |

琉球王国・アイヌにも同一ノードを付与する。C01〜C03 の日本本土寄りの名称は、両勢力の史実をそのまま表す名称ではなくゲーム上の共通枠である。第II期の灰吹法・鉄砲接触や、前提のある第I期ノードは開始時に付与しない。

現行のゲーム内暦表示は 4000 BC であり、この実装は「1534年設定の初期知識」だけを扱う。暦の修正と開始時施設の詳細は別の作業である。

## 実装と公式根拠

公式ファイルの基点: `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/`。

- 社会制度も技術と同じく Gameplay Lua で与える。当初は公式の `StartingCivics`（`Base/Assets/Gameplay/Data/Eras.xml:177-179`）に C02・C03 を足したが、2026-09-29 の実機では古代開始で1件も付与されなかった（C01 も未取得）。公式 `DLC/Indonesia_KhmerScenario/Scripts/IndonesiaKhmerScenario.lua:31` と同じく `pCulture:SetCivic(index, true)` で与える。`StartingCivics` は既存の C01 への置換（`JAC_Sengoku_Unlocks.sql:99-100`）だけを残す。
- 技術と社会制度の初期付与には Gameplay Lua の `LuaEvents.NewGameInitialized` を使う。公式 `Base/Assets/Scenarios/NapoleonScenario/NapoleonScenario_StartScript.lua:3-15,28-31` は新規ゲーム時に `PlayerManager.GetAliveMajors()` の各勢力へ技術を与える。`Base/Assets/Scenarios/ColdWarScenario/ColdWarScenario_StartScript.lua:8-21,44` と `DLC/VikingsScenario/Scripts/VikingScenario.lua:35-39` は `SetTech(index, true)` の取得処理を示す。登録方法は公式 `Base/Assets/Scenarios/NapoleonScenario/NapoleonScenario.modinfo:20-29` の GameplayScripts に倣い、戦国 MOD の `AddGameplayScripts` で万世紀ルールセットに限定した。
- ロード後の重複付与を避けるため、新規ゲームイベントに加えて保存される `Game` プロパティと開始ターンを検査する。公式 `DLC/WarMachineScenario/Scripts/WarMachineScenario.lua:39-42` は初期化済みプロパティを確認してから新規ゲーム処理を行う。開始ターンの比較は公式 `DLC/Expansion2/UI/Replacements/TopPanel.lua:384-389` と `DLC/PiratesScenario/Scripts/PiratesScenario_StartScript.lua:925` の使い方に従う。
- `SetTech` で保有状態を直接設定し、研究進捗やブースト API は呼ばない。公式 UI は `Base/Assets/UI/Screens/TechTree.lua:1097-1115` と `Base/Assets/UI/Screens/CivicsTree.lua:1270-1290` で `HasTech` / `HasCivic` を研究済み表示の判定に使う。`Base/Assets/UI/Screens/TechTree.lua:88,194-198` と `Base/Assets/UI/Screens/CivicsTree.lua:78,196-200` は検査に使う `GetLiveData()` と完了状態の外観を定義し、同じファイルの `Base/Assets/UI/Screens/TechTree.lua:1113` / `Base/Assets/UI/Screens/CivicsTree.lua:1288` はブースト判定 API を使う。GS の置換画面は `DLC/Expansion2/UI/Replacements/TechTree_Expansion2.lua:5-13` / `DLC/Expansion2/UI/Replacements/CivicsTree_Expansion2.lua:5-13` からこれらを読み込む。ブースト行の技術・制度との関係は `Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:368-391` に定義されている。歴史的瞬間の読み取りと型の参照は `DLC/Expansion2/UI/Additions/HistoricMoments.lua:217,257-259`、技術・制度の完了による時代スコアの定義は `DLC/Expansion2/Data/Expansion1_Moments.xml:229-230,300-301` に従い、実機シナリオで誤発火がないことを検査する。

## メインセッションの実機確認

`cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-autotest -- tools/civ6-autotest/scenarios/sengoku-starting-knowledge.json --check` は JSON の検証のみ。メインセッションで同じファイルを `--check` なしで実行する。

シナリオは 15 km 日本マップ、22勢力、武田家を人間プレイヤーとして万世紀ルールセットで開始する。HostGame の設定を再構築した後、**ターン1**に全22勢力の32技術・30制度の保有を走査し、上記4技術・3制度だけが取得済みであることを厳密に照合する。全ブースト行の誤発火と、研究・制度取得に由来する歴史的瞬間がないことも検査する。技術・制度ツリー画面を開いて、各画面の表示状態がそれぞれ4/32、3/30だけ研究済みであることを確かめ、スクリーンショットを残す。その後セーブ・ロードし、開始ターンと保存プロパティ、全22勢力の保有集合、ブースト、歴史的瞬間を再検査する。ゲーム起動と実行結果は本ワークツリーでは未確認。

## 付与範囲

付与は日本マップに限らない。万世紀ルールセット（`RULESET_JAC_BANSEIKI`）で戦国パックが有効な全マップ・全22勢力に、開始時に同じ7ノードを付与する。戦国パックの1534年暦とツリーがルールセット単位で適用されるためで、[SSOT 3.2](../spec/ssot/03-map-and-scale.md) の「戦国パックの1534年開始で第I期の共通セットを全勢力へ付与」に沿う。ゲームプレイスクリプトの有効条件は地図ではなく `JAC_Sengoku_BanseikiOnly`（万世紀ルールセット）で、静的テストで固定している。

## オーナー確認済み

- 全22勢力（琉球王国・アイヌを含む）に同じ7ノードを付与する（Discord 2a、SSOT 3.2）。
- 第I期の前提ありノードは共通付与せず、研究対象に残す。
