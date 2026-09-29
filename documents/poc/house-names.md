# 家固有名の表示切替（SSOT 3.2）

SSOT 3.2「『太閤検地』のような特定の人物・家に結びつく名前は、その家以外では一般的な名前（『検地』など）で表示する」の実装メモ。2026-09-29 時点は**静的検査まで完了、実機確認待ち**。

## 名前の棚卸し

戦国ツリー（`poc/JAC_PoC_SengokuTrees`、[設計案](../research/sengoku-tech-civic-tree.md)）の全名称を確認し、人物・家の名がそのまま付くものだけを切替対象にした。

| 対象 | 一般名（既定・DB が指す） | 家固有名 | 表示する家 | 理由 |
|---|---|---|---|---|
| 社会制度 C25 `CIVIC_JAC_SENGOKU_TAIKO_LAND_SURVEY` | 広域検地 / Regional Land Survey / Общее межевание земель | 太閤検地 / Taikō Land Survey / Тайко-кэнти (межевание земель) | 織田（暫定） | SSOT 3.2 の例そのもの。豊臣は勢力にないため、秀吉が仕えた織田を暫定の持ち主にした |
| 時代3の名前と説明 `LOC_JAC_SENGOKU_ERA_3_NAME` `_DESCRIPTION` | 統一への広域戦 / Campaigns for Unification / Походы за объединение | 天下布武と広域戦 / Tenka Fubu and Wide-Area Campaigns / «Тэнка фубу» и походы широкого масштаба | 織田（暫定） | 「天下布武」は信長の印文。従来の日本語名が家固有だったため、一般名を作り直した |

対象外と判断したもの（一般的な制度名・史実の出来事で、特定の人物・家の名ではない）：C27 刀狩、C17 楽市楽座（楽市は織田以前から各地にある）、C10 軍役帳、C26 朱印状、C08 寺内町自治、C28 兵農分離、T12 鉄砲伝来、T32 惣構、T27 安宅船。オーナーが「家固有にする」と決めたものは、後述の手順で1行足すだけで切り替えられる。

イベント（`documents/research/sengoku-events.md` は未作成）は同じ登録を使う。イベント名に人物・家の名（例：桶狭間の戦い、長篠の戦い）が付く場合は、イベント側の作業で一般名と家固有名を組にして登録する。

## 方式

- データベースの `Name` は常に**一般名**のテキストキーを指す。エンジンが組み立てる文（通知の本文など）は UI の Lua を通らないため、こうしておけば家固有名が他の家へ漏れない（漏れる側ではなく、漏れない側に倒す）
- 家固有名は別のテキストキー（一般名キー + `_HOUSE`）に置く。日本語・英語・ロシア語をすべて用意する（SSOT 14.2。未登録の言語は英語が出る）
- 表示側で、操作中のプレイヤーの文明（`PlayerConfigurations[Game.GetLocalPlayer()]:GetCivilizationTypeName()`）が登録された家なら、`Locale.Lookup`・`Locale.ToUpper`・`Locale.ConvertTextKey` に渡されたキー（と書式文への引数のキー）を家固有名のキーへ置き換える（`poc/JAC_PoC_BanseikiRuleset/UI/JAC_HouseNames.lua`）
- 画面ごとの上書きは書かない。公式 UI の技術・社会制度名はすべて `Locale.Lookup(xxx.Name)` を通るため、その関数を包む方が上書き箇所が少なく、公式の書式文（`LOC_UI_CIVIC_PROGRESS_COMPLETE_BLURB` など）にも効く
- 文明ID は長尾→上杉の改名（SSOT 3.1）でも変わらないため、切替の判定にそのまま使える
- マルチプレイでは端末ごとに操作中の家が異なるので、端末ごとの表示になる。共有状態は使わない（家名の改名とは別の要件）

## 取り付け先

Locale の包みは各 Lua コンテキストごとに要る。公式のワイルドカード include（`include("<接頭辞>_", true)`）に乗る小さいファイルで、MOD の競合なく取り付ける。以下のパスはすべて `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI` からの相対。

| 取り付けの仕組み | 根拠（公式） | 届く画面 |
|---|---|---|
| `UI/TechAndCivicUnlockables_JAC_HouseNames.lua`（ワイルドカード） | `Base/Assets/UI/TechAndCivicUnlockables.lua:264` が `include("TechAndCivicUnlockables_", true)`。`ToolTipHelper.lua:12` がそれを読み、`Civ6Common.lua:5` が ToolTipHelper を、`TechAndCivicSupport.lua:8` が Civ6Common を読む | 技術ツリー（`Screens/TechTree.lua:828`、時代 `:1013`）、社会制度ツリー（`Screens/CivicsTree.lua:944`）、リサーチ／社会制度チューザー（`Choosers/ResearchChooser.lua:8,191`、`CivicsChooser.lua:15,166`）、ワールドトラッカー（`WorldTracker.lua:7,977`）、研究完了ポップアップ（`Popups/TechCivicCompletedPopup.lua:56,60,144`）、通知パネル（`Panels/NotificationPanel.lua:3`）、ツールチップ全般（`ToolTipHelper.lua:430` の `GetCivicToolTip`） |
| `UI/JAC_BoostUnlockedPopup.lua`（`ReplaceUIScript`、元の `BoostUnlockedPopup` をそのまま読む） | `Popups/BoostUnlockedPopup.lua:6` は `SupportFunctions` しか読まず上の連鎖に乗らない。技術名 `:61`、社会制度名 `:167` | 閃きポップアップ |
| `UI/CivilopediaPage_JAC_HouseNames.lua`（ワイルドカード） | `Civilopedia/CivilopediaScreen.lua:13` が `include("CivilopediaPage_", true)` | シヴィロペディア（一部） |

ワイルドカード方式の前例：都市バナー（`WorldView/CityBannerManager.lua:2948`、`poc/JAC_PoC_CustomCivs/UI/CityBannerManager_JAC_CivNames.lua`）。公式の注意どおり、ワイルドカードで読まれるファイルは元のファイルを `include` しない。

## 制限（暫定）

- エンジンが組み立てる通知の本文など、UI の Lua を通らない文は一般名になる（SSOT 3.1 も同じ懸念を挙げている）。通知パネルの Lua 側で組む文（`NotificationPanel.lua` が `Locale.Lookup` で組む部分）だけは切り替わる
- シヴィロペディアは、見出しなどをタグのままエンジンの部品へ渡していれば一般名が出る（織田でも「広域検地」）。ワイルドカードの取り付けだけ入れてあり、効くかは未確認（「安ければ」の範囲）
- 他の MOD が `Locale.Lookup` を包む場合は、包む順で結果が変わる可能性がある（未検証）
- `Locale.Compare`（並び替え）と、エンジン側のテキスト検索は包んでいない

## 名前を足す手順

1. `<一般名キー>` の日英露と `<一般名キー>_HOUSE` の日英露をテキスト XML に足す（一般名が既存の Name なら Name は変えない）
2. 所有 MOD の `UI/JAC_HouseNamesData_<名前>.lua` に `JAC_RegisterHouseName("<一般名キー>", "<一般名キー>_HOUSE", "CIVILIZATION_JAC_XXX")` を足す（複数の家に見せるなら家の数だけ行を足す）。ファイルを新設する場合は modinfo の `ImportFiles` と `Files` に載せる
3. `python -m unittest discover -s tests -t .` を通す。`tests/test_house_names.py` が、全キーの3言語そろい・一般名と家固有名の差・家固有名の混入・登録の重複と文明IDの実在・取り付けの配線を確かめる

戦国ツリーの登録は `tools/generate_sengoku_trees.py` の `HOUSE_NAMES` から `poc/JAC_PoC_SengokuTrees/UI/JAC_HouseNamesData_Sengoku.lua` とテキスト XML を生成する。

## 実機確認（メインセッション）

`tools/civ6-autotest/scenarios/sengoku-house-names-oda.json`（織田を操作）と `sengoku-house-names-takeda.json`（武田を操作）。どちらも `RULESET_JAC_BANSEIKI` で開始し、Banseiki・CustomCivs・SengokuTrees を有効化する。

| 確認 | 期待 |
|---|---|
| `Lua.log` の `houseNames context=BoostUnlockedPopup result=installed` | 出る。他のコンテキストは `installed` か `shared_with_…`。`error …` や `no_locale` は取り付け失敗 |
| 各 UI コンテキスト（TechTree・CivicsTree・CivicsChooser・ResearchChooser・WorldTracker・TechCivicCompletedPopup・BoostUnlockedPopup・NotificationPanel）で C25 の名前 | 織田：家固有名（太閤検地）。武田：一般名（広域検地）。`ui_lua` が書式文の引数（`LOC_UI_CIVIC_PROGRESS_COMPLETE_BLURB`）と `Locale.ToUpper` も確かめる |
| 同じコンテキストの時代3（`ERA_MEDIEVAL`）の名前 | 織田：天下布武と広域戦。武田：統一への広域戦 |
| 社会制度ツリーのツールチップ（TechTree・CivicsTree の `ToolTipHelper.GetToolTip`） | 織田のみ家固有名を含む |
| `house-names-*-civics-tree.png`・`-tech-tree.png` | ツリーの見た目の記録。C25 が未公開だと「???」で、名前はスクリーンショットでは見えない（名前の判定は `ui_lua`） |

`ui_lua` の `state_name` は既存シナリオが使っている名前（TechTree・CivicsTree・BoostUnlockedPopup）以外は推測。存在しない名前で失敗したら `state_name` を実際の名前へ直す。

## オーナーへの質問

1. 「太閤検地」「天下布武」の持ち主を織田家（暫定）にしてよいか。豊臣家は勢力に無い。織田家が秀吉の系譜を継ぐ扱いにするか、最後に天下を取った家が名乗る扱いにするか
2. 上記の棚卸しで対象外にした名前（刀狩・楽市楽座・軍役帳・朱印状）のうち、家固有にしたいものはあるか
