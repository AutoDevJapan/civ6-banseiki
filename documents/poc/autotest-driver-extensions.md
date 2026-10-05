# Civ6 自動検証ドライバー拡張（2026-09-29）

## 公式作品の都市借用を保存前後・参加者間で照合する（ToDo #996）

Rust実行器の `carrier_checkpoint` は、指定した借用トークンを保存済み台帳から一件だけ選び、UIの本体getterで原本の全所在と借用都市を独立に採取する。採取は操作を送信せず、ゲーム処理や保存プロパティを変更しない。これは検証の接続であり、製品の都市借用経路をマルチプレイへ開く実装ではない。現行のMP拒否は維持する。

```json
{"action":"carrier_checkpoint","token":"取引ID:carrier:作品index","candidate":"配置したコミットID","path":"carrier/borrow-before.json","capture_as":"BORROW_BEFORE"}
```

同じ手番の保存を再読込し、同トークン・同候補の次の採取に `equals_capture: "BORROW_BEFORE"` を付ける。`path` は以前と別の相対パスにする。既存の記録は上書きしない。相違した比較の採取結果もファイルに保持する。原本の消失・重複、来歴と効果の欠落・変化、予約外の所在、都市の人口・建物・区域・別作品・宗教・忠誠・信仰・統治・購入区画・中心部隊の相違を拒む。借用状態だけでなく吸収台帳全体とフローの保存値も照合し、抽選結果や別取引の記録が違う場合も一致とは扱わない。Luaのテーブルは数値キーもJSONオブジェクトの文字列キーへ変換し、型の異なるキーが同じ文字列になる記録は採取を拒む。

別参加者の記録は `civ6-carrier-sync compare host.json peer.json --peers` で比較する。両方がMPで、異なる人のIDを持ち、同じ手番・トークン・候補と完全な状態でなければ比較を拒む。`candidate` は実行者が配置記録から指定する識別子で、配布物の実測ハッシュではない。同じ記録の一致は通信そのものの成功を証明せず、同一ゲームへ接続した両端末の一次記録と実際の都市・作品移動が別途必要である。ホットシートや二つのLua VMの検査をLANの成功に数えない。

`civ6-carrier-sync query <token> <candidate>` は同じ読取クエリを表示する。`carrier_checkpoint` はこのクエリをInGameコンテキストで一度実行する。返答が不明な場合は再送せず、成功記録も作らない。今後の製品同期は、端末固有のUI観測をGameplayの進行条件から外し、全クライアントへ届く公式PlayerOperationと保存された意図へ接続する。AI所有作品で操作者と所有者が違う組合せ、MOVE待ち・返却待ちからの実セーブ再開、別端末の通信同期は未確認であり、ToDo #996を継続する。

## 対象と使い方

借用の記録採取では、対象の取引に対応するフロー証拠も必要とする。フローを使わない直接適用の診断だけは `direct_apply: true`（CLIでは `query --direct-apply`）を明示する。製品の取引がフロー経由を記録している場合は、この指定でもフローの欠落を受け入れない。返却完了の記録では、移動対象の枠だけを空として正規化し、借用前の保存スナップショットと都市状態を照合する。地区と別作品も一致を要求する。

SSOT [3.2](../spec/ssot/03-map-and-scale.md) の約200ターン・22勢力の研究進行の計測、および [2.2](../spec/ssot/02-progression.md) の日本マップ指導者プールの確認を支援する。既存のシナリオ形式を維持し、追加機能は `configure.build_setup`、`autoplay.research_metrics`、`autoplay.allow_human_elimination` で有効化する。既存のセーブ名にも既定で出力ディレクトリ由来の接尾辞が付き、同じ実行中の `load` は同じ名前を解決する。固定名を使う場合は `save` と `load` の両方で `exact_name: true` とする。既存ファイルへの上書き禁止は維持する。

新規シナリオは次の3つ。主セッションはこの順に実行すると、設定画面、開始状態、長時間進行を個別に切り分けられる。

| シナリオ | 実機で判定するもの |
|---|---|
| `japan-map-setup-screen.json` | 通常マップの「ゲームの作成」で人間の候補が織田・武田の2人のみ。日本15 km史実領土版を同画面で選ぶと、候補が所定の22人と完全一致する。2枚の画面画像も保存する |
| `japan-map-15km-settlers.json` | 開拓者版が204×88で始まり、22メジャーの各枠に開拓者が1体、都市は0 |
| `japan-map-15km-200turn.json` | 史実領土版の初期22勢力・56都市、200ターン到達、生存勢力数、各観測ターンの時間・メモリ・研究状態。織田の滅亡は失敗条件にしない |

実行前のオフライン確認は、各 JSON に `cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-autotest -- <JSON> --check` を適用する。実ゲームはこの作業ツリーでは起動していない。

## 研究ログの意味

`autoplay-research-N.jsonl` に、最初の観測時点と以後に**観測できた**ターンごとに、各メジャーの `slot`、`leader`、`alive`、`tech_count`、`civic_count`、`current_tech`、`current_civic`、`completed_techs`、`completed_civics` を記録する。初回のメジャー枠一覧を保持し、参加枠一覧が変わっても滅亡後の枠を追跡する。件数は `HasTech` / `HasCivic` が真の ID 数と一致する。観測を飛ばしたターンの研究状態は作らない。時間計測側の `autoplay-metrics-N.jsonl` では飛ばしたターンを `observed: false` とする。

`civ6-research-report` は各勢力で技術または社会制度が初めて**観測上で完了済み**になったターンを使い、項目ごとの中央値、最小値、最大値、幅と勢力数を表示する。初回観測時に既に完了済みの項目は `baseline_factions` に数える。**暫定**: この「初回観測ターン」を到達ターンの上限として扱う。実際の完了ターンは、観測を飛ばした場合と初回観測前に完了した場合には特定できない。オーナー確認事項: 研究費調整にはこの粒度で足りるか、完了イベントによる厳密なターン記録が必要か。

現在の実機カレンダーは紀元前4000年開始のため、このレポートは年号へ換算せずゲームのターン番号だけを示す。1534年開始の暦への修正と、研究の到達ターンの合格範囲は別途オーナー判断を待つ。

`allow_human_elimination: true` は `Players[human]:IsAlive()` が初めて偽と観測されたターンを `steps.jsonl` の `autoplay_elimination` と完了詳細に残す。直前に生存を観測したターンも記録する。**暫定**: ターン境界のポーリング間に滅亡した場合は最初の非生存観測ターンを表示し、厳密な滅亡イベント時刻とは主張しない。人間枠が消えても、自動進行の終了イベント・対象ターン・停止を確認し、必要なら残りターンの自動進行を再開する。再開後に対象ターンへ達する挙動は要実機確認。

## Civ6 公式実装の根拠

次の行番号はローカルの Steam 版 Civ6 1.0.12.68 にある公式ファイル。共通の先頭パスは `C:/Program Files (x86)/Steam/steamapps/common/Sid Meier's Civilization VI/`。ここは参照のみで編集していない。

| 依存する挙動 | 公式ファイル:行 |
|---|---|
| Create Game の headless 構築、`FullRefresh`、終了時の `Shutdown` | `Base/Assets/UI/FrontEnd/GameSetupLogic.lua:689-721,725-731`。通常画面も `:685` で `FullRefresh` を行う |
| 指導者・文明・枠の維持確認 | `Base/Assets/UI/FrontEnd/PlayerSetupLogic.lua:80,911-916`、`Base/Assets/UI/Automation/Automation_StandardTests.lua:128-147,378-417` |
| Create Game の初期化、マップ選択と人間候補の取得 | `Base/Assets/UI/FrontEnd/MainMenu.lua:182-184`、`Base/Assets/UI/FrontEnd/AdvancedSetup.lua:268-278,1165-1167,1222-1225`、`Base/Assets/UI/FrontEnd/PlayerSetupLogic.lua:167-172`。画面の `OnSetMapByValue` と同じ経路で地図を選択する |
| 保存完了・ローカル保存と再読込 | `Base/Assets/UI/Automation/Automation_StandardTests.lua:16-37,298`、`Base/Assets/UI/FrontEnd/LoadGameMenu.lua:108`。接尾辞の生成と上書き拒否はドライバー側の安全策 |
| 自動進行のターン設定、観測対象、復帰対象、終了イベント | `Base/Assets/UI/Automation/Automation_StandardTests.lua:321,330-334,363-367` |
| ターン番号と生死・メジャー判定 | `Base/Assets/UI/TopPanel.lua:387`、`Base/Assets/UI/ARXManager.lua:40`、`Base/Assets/UI/FrontEnd/PlayerSetupLogic.lua:178`。`GetParticipatingPlayerIDs()` からメジャー枠を追う |
| 技術の列挙、完了判定、現在の研究とIDからの型参照 | `Base/Assets/UI/Choosers/ResearchChooser.lua:53,66,335`、`Base/Assets/UI/LaunchBar.lua:535`、`DLC/Expansion2/UI/Replacements/WorldRankings_Expansion2.lua:425,554` |
| 社会制度の列挙、完了判定、現在の研究とIDからの型参照 | `Base/Assets/UI/Choosers/CivicsChooser.lua:50,63,325`、`Base/Assets/UI/Panels/CivicBoostsPanel.lua:86-88`、`Base/Assets/UI/Screens/GovernmentScreen.lua:1705` |
| 開始状態の開拓者・都市・メジャー検査 | `Base/Assets/UI/PartialScreens/ReportScreen.lua:309,359`、`Base/Assets/UI/Popups/RazeCity.lua:87`、`Base/Assets/UI/PlayerSupport.lua:7` |

ゲーム固有の22指導者と4枚の地図の候補、2人フィルターは MOD 自身の `poc/JAC_PoC_JapanMapScale/Config/JAC_JapanMapScale_Config.xml:3-7,9-18,20-110` と `poc/JAC_PoC_BanseikiRuleset/Config/JAC_Banseiki_Config.xml:45-56` に定義される。公式の設定画面 API にこのデータを載せて検査する。
