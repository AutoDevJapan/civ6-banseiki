# 家固有名の表示切替（SSOT 3.2）

実行コマンドは Rust 版に更新している。移行前の検証結果は旧実行器での記録として残す。

SSOT 3.2「特定の人物・家に結びつく名前は、その家以外では一般的な名前で表示する」の実装メモ。2026-09-29 時点は**静的検査と Lua の模擬実行まで完了、実機確認待ち**（`Locale` 表へ書き込む最初の版は実機で失敗し、グローバル変数の差し替えへ作り直した）。

## オーナー判断

- B-5（2026-09-29）：エンジンが組み立てる文（通知の本文、シヴィロペディアの一部など）で一般名しか出ないのは**不可**。持ち主の家には、そこでも家固有名を出す
- U-1（2026-09-30）：「天下布武」は、誓約を実装するときに織田家の固有の誓約3つのうち1つの名前にする（SSOT 3.2、[時代の設計](../research/sengoku-ages.md) §7.2）。それまではどこにも表示しない。家固有名としては登録しない
- Q31（2026-09-30）：社会制度 C25「太閤検地」は、研究したすべての家に「太閤検地」と表示する。家固有名にせず、DB が指す名前そのものを「太閤検地」にした（一般名「広域検地」は廃止）

## 名前の棚卸し

戦国ツリー（`poc/JAC_PoC_SengokuTrees`、[設計案](../research/sengoku-tech-civic-tree.md)）の全名称を確認し、人物・家の名がそのまま付くものだけを切替対象にした。

| 対象 | 一般名（既定・DB が指す） | 家固有名 | 表示する家 | 理由 |
|---|---|---|---|---|
| （削除）時代3の名前と説明 `LOC_JAC_SENGOKU_ERA_3_NAME` `_DESCRIPTION` | 統一への広域戦 | 天下布武と広域戦 | 織田 | オーナー判断 T（元号＝時代）でツリーの4期が元号に置き換わり、期の名前ごと無くなった（[元号＝時代](era-equals-reign.md)）。「天下布武」はオーナー判断 U-1 で織田家の誓約の名前になる。今、戦国パックに登録した家固有名は無い。仕組みの検査は `tools/civ6-lua-tests-rs/tests/house.rs` の検査用の登録で行う |

対象外にしたもの：C25 太閤検地（オーナー判断 Q31。研究したすべての家に同じ名前で出す）。一般的な制度名・史実の出来事で、特定の人物・家の名ではないもの：C27 刀狩、C17 楽市楽座（楽市は織田以前から各地にある）、C10 軍役帳、C26 朱印状、C08 寺内町自治、C28 兵農分離、T12 鉄砲伝来、T32 惣構、T27 安宅船。オーナーが「家固有にする」と決めたものは、後述の手順で1行足すだけで切り替えられる。

イベント（[`documents/research/sengoku-events.md`](../research/sengoku-events.md)）は同じ登録を使う。イベント名に人物・家の名（例：桶狭間の戦い、長篠の戦い）が付く場合は、イベント側の作業で一般名と家固有名を組にして登録する。

## 方式

- データベースの `Name` は常に**一般名**のテキストキーを指す。エンジンが組み立てる文は一般名で作られるので、他の家には家固有名が漏れない
- 家固有名は別のテキストキー（一般名キー + `_HOUSE`）に置く。日本語・英語・ロシア語をすべて用意する（SSOT 14.2。未登録の言語は英語が出る）
- 表示側で、操作中のプレイヤーの文明（`PlayerConfigurations[Game.GetLocalPlayer()]:GetCivilizationTypeName()`）が登録された家なら、各 UI コンテキストの `Locale.Lookup`・`Locale.ToUpper`・`Locale.ConvertTextKey` を包んで次の2段で置き換える（`poc/JAC_PoC_BanseikiRuleset/UI/JAC_HouseNames.lua`）
  1. キーの置き換え：渡されたキー（と書式文への引数のキー）が一般名なら家固有名のキーへ
  2. 文の置き換え：結果の文に一般名の文字列（表示言語の翻訳と、その大文字形）が残っていれば家固有名の文字列へ。エンジンが一般名で組み立て済みの文を UI が `Locale.Lookup` に通す箇所（通知の本文など）はこれで切り替わる
- エンジンの `Locale` 表は読み取り専用で、書き込むと `Attempt to modify read-only table` になる（2026-09-29 実機。`Locale.Lookup = …` で包む最初の版は全コンテキストでこのエラーになり、何も切り替わらなかった）。そこで `Locale` 表には書き込まず、コンテキストの**グローバル変数** `Locale` を、包んだ3関数を持ち残りを本物へ委ねる表（`setmetatable({...}, { __index = 本物 })`）に差し替える。本物は `JAC_HouseNamesBaseLocale` に残す
  - グローバル変数はコンテキストごとにある。公式 UI は各コンテキストで同じ名前のグローバル関数（`Initialize` など）を定義し、`include` した元ファイルのグローバル関数を後から定義し直している（例：`DLC/Expansion1/UI/Replacements/NewTurnPanel_Expansion1.lua:7` で NewTurnPanel を読んでから `ShowEraCountdownPanel` などを定義）。差し替えはそのコンテキストの中だけに効き、同じコンテキストで後から読まれるファイルも差し替え後の `Locale` を見る
  - 公式 UI（Base・DLC）に `Locale` をローカル変数へ取り込む書き方（`local Locale = Locale` など）や `Locale:` のメソッド呼び出しは無い。どれも呼び出しのたびにグローバルの `Locale.Lookup` を引くので、差し替え後の表を通る
  - 1つのコンテキストに `ToolTipHelper` が複数の経路で読まれる（実機の `Lua.log` で WorldInput・CityPanel などに取り付けが複数回出た）。2回目以降は `already_installed` で二重に包まない
- 家固有名が一般名を含むと、置き換え済みの文をもう一度通したとき（`TechTree.lua:1013` は `Locale.Lookup` を入れ子で呼ぶ）に二重になる。この組は実行時に使わず、`tools/civ6-data-rs/src/data_tests.rs` でも禁止している
- 家固有名を持たない家（武田など）と、操作中のプレイヤーがいない画面では何も置き換えない。シヴィロペディアの部品の差し替えも、家固有名を持つ家が見ているときだけ動く
- 画面ごとの上書きは最小にする。公式 UI の名前の多くは `Locale.Lookup` を通るため、その関数を包む方が上書き箇所が少なく、公式の書式文（`LOC_UI_CIVIC_PROGRESS_COMPLETE_BLURB` など）にも効く
- 実機97の捕虜交換では、公式`DiplomacyDealView.lua`の捕虜ツールチップが`NameIndex=-1`に対して名前キー`nil`を`Locale.Lookup`へ渡し、表示処理が止まった。`DiplomacyDealView`コンテキストで引数のない`Lookup(nil)`を呼んだときだけ、境界テストのUIアダプターが本体の捕虜1件・取引項目ID・捕獲者・`NameIndex=-1`・ロック・名前キー欠落を再照合し、既存の`UNIT_SPY`名キーを表示に使う。条件外の`nil`は従来どおりエラーにする。捕虜名・本体取引の値・保存データは変更しない。英語・日本語・ロシア語は本体の既存翻訳を使う
- 文明ID は長尾→上杉の改名（SSOT 3.1）でも変わらないため、切替の判定にそのまま使える
- マルチプレイでは端末ごとに操作中の家が異なるので、端末ごとの表示になる。共有状態は使わない（家名の改名とは別の要件）

## 取り付け先

Locale の包みは各 Lua コンテキストごとに要る。以下のパスはすべて `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI` からの相対。

| 取り付けの仕組み | 根拠（公式） | 届く画面 |
|---|---|---|
| `UI/TechAndCivicUnlockables_JAC_HouseNames.lua`（ワイルドカード） | `Base/Assets/UI/TechAndCivicUnlockables.lua:264` が `include("TechAndCivicUnlockables_", true)`。`ToolTipHelper.lua:12` がそれを読み、`Civ6Common.lua:5` が ToolTipHelper を、`TechAndCivicSupport.lua:8` が Civ6Common を読む | 技術ツリー（`Screens/TechTree.lua:828`、時代 `:1013`）、社会制度ツリー（`Screens/CivicsTree.lua:944`）、リサーチ／社会制度チューザー（`Choosers/ResearchChooser.lua:8,191`、`CivicsChooser.lua:15,166`）、ワールドトラッカー（`WorldTracker.lua:7`、`TechAndCivicSupport.lua:683,721`）、研究完了ポップアップ（`Popups/TechCivicCompletedPopup.lua:56,60,144`）、通知パネル（`Panels/NotificationPanel.lua:3`）、偉人ポップアップの時代名（`Popups/GreatPeoplePopup.lua:8,179`）、ツールチップ全般（`ToolTipHelper.lua:430` の `GetCivicToolTip`） |
| `UI/CivilopediaPage_JAC_HouseNames.lua`（ワイルドカード） | `Civilopedia/CivilopediaScreen.lua:13` が `include("CivilopediaPage_", true)`。`:6` で CivilopediaSupport を読んだ後、`:17` の Initialize より前に読まれる | シヴィロペディア（下記の棚卸し） |
| `ReplaceUIScript` ＋ 元のスクリプトをそのまま読む小さいファイル（`UI/JAC_<コンテキスト>.lua`） | 下の各コンテキストは上の連鎖にもワイルドカードにも乗らない | 閃きポップアップ `BoostUnlockedPopup`（技術名 `Popups/BoostUnlockedPopup.lua:61`、社会制度名 `:167`）、時代の進み具合 `EraProgressPanel`（`DLC/Expansion2/UI/Additions/EraProgressPanel.lua:215`）、時代の終わり `EraCompletePopup`（`Additions/EraCompletePopup.lua:87`）、時代の振り返り `EraReviewPopup`（`:62`）、献身の選択 `DedicationPopup`（`:57`）、時代の終わりの予告 `NewTurnPanel`（`Replacements/NewTurnPanel_Expansion1.lua:19`）、歴史的瞬間 `HistoricMoments`（下記の棚卸し） |

`NewTurnPanel` は Gathering Storm 自身が `NewTurnPanel_Expansion1.lua` へ差し替えている（`DLC/Expansion2/Expansion2.modinfo` の `Expansion1_NewTurnPanel`）ので、こちらの差し替えファイルはその差し替え先を読み込む（`NewTurnPanel_Expansion1.lua:7` が元の NewTurnPanel を読む）。ワイルドカード方式の前例は都市バナー（`WorldView/CityBannerManager.lua:2948`、`poc/JAC_PoC_CustomCivs/UI/CityBannerManager_JAC_CivNames.lua`）。公式の注意どおり、ワイルドカードで読まれるファイルは元のファイルを `include` しない。

## エンジンが組み立てる文・Lua を通らない文の棚卸し（B-5）

家固有名（時代3の名前と説明）が UI の `Locale.Lookup` を通らずに画面へ出る経路を、公式 UI（Base・Expansion1・Expansion2）の `GetMessage`・`GetSummary`・`InstanceDescription`・`LocalizeAndSetText`・`LocalizeAndSetToolTip`・`GameInfo.Eras[` の全呼び出しから洗い出した。

| 経路 | 公式の該当箇所 | 対応 |
|---|---|---|
| 通知の見出しと本文（エンジンが `{1_Name}` を一般名で埋めた文。例：`LOC_NOTIFICATION_CIVIC_DISCOVERED_SUMMARY`「You have discovered {1_Name}!」`Base/Assets/Text/en_US/Notifications_Text.xml:560`） | UI がこの文を読むのは `Panels/NotificationPanel.lua:958-959`（`Locale.Lookup(pNotification:GetMessage())`／`GetSummary()`）と `:774`（段階違いのツールチップ）、`DLC/Ethiopia/UI/Replacements/NotificationPanel_Ethiopia.lua:130` だけ。どれも `Locale.Lookup` を通る | **切り替わる**。文の置き換え（方式の2）で、通知の種類を問わず家固有名になる。見出しは `:962` で `Locale.ToUpper` されるが、大文字形も置き換える。公式の上書き口 `ApplyNotificationTextOverrides`（`:948`）は使わない（Lookup の段階で直るので不要で、他 MOD の上書きとも競合しない） |
| シヴィロペディアの部品（キーのまま `LocalizeAndSetText` へ渡し、エンジンが翻訳） | `Civilopedia/CivilopediaSupport.lua:1287-1360`（章・段落・見出し）、`:1419`（アイコンのツールチップ）、`:1621`（引用）、`:1643-1738`（右列の枠）。例：時代名 `CivilopediaPage_Civic.lua:201`、前提の社会制度 `:210`、次の社会制度 `:241` | **切り替わる**。`CivilopediaPage_JAC_HouseNames.lua` がこれらの公式の部品関数（グローバル）を包み、家固有名を持つ家のときだけ `Locale.Lookup` で文にしてから渡す（翻訳済みの文を渡すのは公式と同じ使い方：`:316` と `:745`）。見出し `page.Title`（`:317`）と検索の索引（`:467`）は Initialize 時に包んだ `Locale.Lookup` を通る |
| 歴史的瞬間の説明（エンジンが組み立て済みで、`SetText` へ直接） | `DLC/Expansion2/UI/Additions/HistoricMoments.lua:289`（データは `:153` の `GetMomentData`、`:217` の `GetAllMomentsData`）。例：`MOMENT_CIVIC_CULTURVATED_IN_ERA_FIRST`（`DLC/Expansion2/Data/Expansion1_Moments.xml:229`） | **切り替わる**。`UI/JAC_HistoricMoments.lua` がこのコンテキストのグローバル `Game` だけを差し替え、`Game.GetHistoryManager()` の戻り値の `InstanceDescription` に文の置き換えをかける（エンジンの `Game` 表は書き換えない） |
| 時代名を `Locale.Lookup`・`Locale.ToUpper` で出す画面 | `EraProgressPanel.lua:215`、`EraCompletePopup.lua:87`、`EraReviewPopup.lua:62`、`DedicationPopup.lua:57`、`NewTurnPanel_Expansion1.lua:19` | **切り替わる**。上の ReplaceUIScript で各コンテキストに包みを入れた |
| C++ が組み立てるツールチップ | 公式 UI のツールチップはすべて Lua の `SetToolTipString`／`LocalizeAndSetToolTip` で付く。技術・社会制度の名前を含むものは `ToolTipHelper` か `TechAndCivicSupport` で、上の連鎖に乗る。`LocalizeAndSetToolTip` に社会制度・時代のキーを直接渡すのは `TechAndCivicSupport.lua:317`（`Description`。家固有名の登録なし）だけ | 対象なし |
| チャット・ログ | チャット欄に社会制度・時代の名前を出す処理は無い。`Lua.log` などのログはプレイヤーに見せない | 対象なし |

### 残るもの（切り替えられない、または今回は入れていない）

| 経路 | 根拠 | 理由 |
|---|---|---|
| 遺物の選択の時代名 | `Base/Assets/UI/Popups/ChooseArtifact.lua:47-48`（`Controls.EraString:LocalizeAndSetText("LOC_CHOOSE_ARTIFACT_ERA_STRING", szEraName)`） | エンジンがキーのまま翻訳し、Lua を通らない。直すには `ChooseArtifact` を差し替えて `OnOpen`（`:4`）の後で `EraString` を書き直す必要がある。考古学者（産業時代以降）が戦国ツリーの範囲で出るか未定のため入れていない。出る場合は同じ ReplaceUIScript 方式で直せる |
| 殿堂（ゲームの記録）の開始時代 | `Base/Assets/UI/GameSummaries.lua:729`（`LocalizeAndSetText("LOC_GAMESUMMARY_ERA_STARTED", startEra.Name)`） | フロントエンドの画面で、MOD の UI（`InGameActions`）が読まれず、操作中の家も無い。**切り替え不可** |
| セーブ一覧の「ホストの時代」 | `Base/Assets/UI/FrontEnd/LoadSaveMenu_Shared.lua:698`（セーブに入ったバンドル `fileInfo.HostEraName` を `Locale.LookupBundle` で読む：`:578`） | フロントエンドでは同上で**切り替え不可**。ゲーム中の読み込み画面も同じファイルを使うが、包みを入れていない（乗る口が無く、差し替えるなら読み込み画面を丸ごと ReplaceUIScript する） |
| 言語ファイルを操作中の家ごとに差し替える方式（テキストの重ね合わせ） | 公式 MOD の条件（`Criteria`）は `LeaderPlayable`・`RuleSetInUse`・`ConfigurationValueMatches`（`Group` は `Game` と `Map` だけ）・`ModInUse`・`GameCoreInUse`・`ModIsEnabled` のみ（`DLC/*/*.modinfo` の全件） | どれもゲーム全体で共通の値で、端末ごとの操作中の家で条件を分けられない。マルチプレイでは全端末に同じ文が入り他の家へ漏れるので**使えない** |
| `Locale.Compare`（並び替え）とエンジン側のテキスト検索 | `CivilopediaSupport.lua:360,386`、`:1102` | 表示ではないので包んでいない。シヴィロペディアの索引は家固有名（`:467`）なので、持ち主の家は一般名では検索で見つからない |
| ホットシート（1台で交代） | シヴィロペディアの見出しと索引は Initialize 時に1度だけ作る（`CivilopediaSupport.lua:1273-1275`） | 最初の操作プレイヤーの名前のまま残る。ホットシートを対象にする場合は別途 |
| 他の MOD が同じコンテキストを ReplaceUIScript で差し替える場合 | 差し替えは `LoadOrder` の大きい1つだけが効く | 競合した画面では包みが入らない。`Locale.Lookup` を包む他 MOD との順序も未検証 |

## 名前を足す手順

1. `<一般名キー>` の日英露と `<一般名キー>_HOUSE` の日英露をテキスト XML に足す（一般名が既存の Name なら Name は変えない）。家固有名の文に一般名の文を含めない
2. 所有 MOD の `UI/JAC_HouseNamesData_<名前>.lua` に `JAC_RegisterHouseName("<一般名キー>", "<一般名キー>_HOUSE", "CIVILIZATION_JAC_XXX")` を足す（複数の家に見せるなら家の数だけ行を足す）。ファイルを新設する場合は modinfo の `ImportFiles` と `Files` に載せる
3. `cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets` を通す。`tools/civ6-data-rs/src/data_tests.rs` が、全キーの3言語そろい・一般名と家固有名の差・家固有名の混入・家固有名への一般名の混入・登録の重複と文明IDの実在・取り付けの配線を確かめる。Lua の振る舞いは `cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets` で `tools/civ6-lua-tests-rs/tests/house.rs` も走らせる（Lua 5.1 を組み込み、毎回実行する）。模擬の `Locale` はゲームと同じく読み取り専用で、`Locale` 表へ書き込むとテストが落ちる

戦国ツリーの登録は `tools/civ6-data-rs/src/trees.rs` の `HOUSE_NAMES` から `poc/JAC_PoC_SengokuTrees/UI/JAC_HouseNamesData_Sengoku.lua` とテキスト XML を生成する。

## 実機確認（メインセッション）

`tools/civ6-autotest/scenarios/sengoku-house-names-oda.json`（織田を操作）と `sengoku-house-names-takeda.json`（武田を操作）。どちらも `RULESET_JAC_BANSEIKI` で開始し、Banseiki・CustomCivs・SengokuTrees を有効化する。マップは既定のマップ（`map_size` は `MAPSIZE_SMALL`）で、日本マップは使わない。名前の切り替えは UI だけの処理でマップに依存しないため、日本マップでも結果は変わらない。

| 確認 | 期待 |
|---|---|
| `Lua.log` の `houseNames context=BoostUnlockedPopup result=installed` | 出る。他のコンテキスト（EraProgressPanel・EraCompletePopup・EraReviewPopup・DedicationPopup・NewTurnPanel・HistoricMoments・Civilopedia など）も `installed`（同じコンテキストで2回目以降は `already_installed`）。`error …` や `no_locale` は取り付け失敗。`error Attempt to modify read-only table` は `Locale` 表へ書き込む旧方式の失敗 |
| 各 UI コンテキスト（TechTree・CivicsTree・CivicsChooser・ResearchChooser・WorldTracker・TechCivicCompletedPopup・BoostUnlockedPopup・NotificationPanel）で C25 の名前 | 織田・武田とも太閤検地（`_HOUSE` のテキストが無いことも確かめる）。`ui_lua` が書式文の引数（`LOC_UI_CIVIC_PROGRESS_COMPLETE_BLURB`）にも名前が入ることを確かめる |
| 上のコンテキストと EraProgressPanel・EraCompletePopup・EraReviewPopup・DedicationPopup・NewTurnPanel・HistoricMoments の `ERA_MEDIEVAL` の名前 | 織田・武田とも永禄の世（元号＝時代。時代3の家固有名は削除）。HistoricMoments では記録済みの瞬間の説明に LOC キーが残らないことも確かめる |
| 社会制度ツリーのツールチップ（TechTree・CivicsTree の `ToolTipHelper.GetToolTip`） | 織田・武田とも C25 の名前（太閤検地）を含む |
| シヴィロペディアの C25 のページ（`Civilopedia` で `NavigateTo`） | ページの見出しと `page.Title`：織田・武田とも太閤検地。`JAC_LocalHouseHasNames()` は織田・武田とも偽で、`JAC_HOUSE_NAMES` は空（戦国パックに家固有名の登録が無い。実機 `era-equals-reign-05` は織田を真と見ていたため失敗し、期待を直した） |
| **通知の本文**：`GameCore_Tuner` で C25 をプレイヤー0に与え（`SetCivic`）、NotificationPanel で C25 の名前を含む通知を探す | エンジンの文と、表示される文（`Locale.Lookup` を通した見出しと本文、通知パネルの `TitleInfo`・`Summary` の表示文字列）は、織田・武田とも太閤検地を含む。家固有名の通知での切り替えは、実機のシナリオでは確かめていない（`tools/civ6-lua-tests-rs/tests/house.rs` の模擬実行で時代3の名前を使って確かめる）。`house-names-*-notification.png` に記録 |
| `house-names-*-civics-tree.png`・`-tech-tree.png` | ツリーの見た目の記録。C25 が未公開だと「???」で、名前はスクリーンショットでは見えない（名前の判定は `ui_lua`） |

要実機確認：

- `SetCivic` が「社会制度を習得」の通知を出すか（公式の使用例は `Debug/Player.ltp:198` とシナリオ `DLC/VikingsScenario/Scripts/VikingScenario.lua:38` だけで、通知の有無は未確認）。出ない場合は30秒の待ちで失敗するので、通知を出す別の手段に替える
- `ui_lua` の `state_name` は既存シナリオが使っている名前（TechTree・CivicsTree・BoostUnlockedPopup・NotificationPanel・GameCore_Tuner）以外は推測。存在しない名前で失敗したら `state_name` を実際の名前へ直す
- 通知パネルの項目は、作られた時点で `TitleInfo`・`Summary` に文が入る前提（`NotificationPanel.lua:967-968`）。項目が無い（`m_Instance` が nil）場合は表示文字列の確認だけ飛ばす
- コンテキストのグローバル `Locale` の差し替えが効くか（エンジンの `Locale` 表が読み取り専用なのは実機で確認済み。グローバル変数への代入を拒む仕組みがあれば `error global Locale was not replaced` か別の `error …` が出る。その場合は呼び出し側での包み（公式の関数の上書き）に切り替える）
- ReplaceUIScript で差し替えたコンテキスト（特に Gathering Storm の差し替え先を読み込む NewTurnPanel）が、元どおり動くか

## オーナーへの質問

1. 上記の棚卸しで対象外にした名前（刀狩・楽市楽座・軍役帳・朱印状）のうち、家固有にしたいものはあるか
