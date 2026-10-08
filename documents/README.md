# documents/ 索引

本プロジェクト（Civ6 日本列島・世界・宇宙 周回育成型トータルコンバージョンMOD）のドキュメントの入口。

## 正本と派生物

| パス | 位置づけ |
|---|---|
| [`spec/data/banseiki-pool.json`](spec/data/banseiki-pool.json) | 万世紀プールの初期300人の採用済み人物台帳（SSOT 4.8） |
| [`spec/banseiki-pool-data.md`](spec/banseiki-pool-data.md) | 人物データの項目とRust検査の使い方 |
| [`spec/ssot/`](spec/ssot/) | **唯一の仕様正本（SSOT）**。章・節ごとのファイルに分割している。本文中の「本書」はこのディレクトリ全体を指す |
| [`spec/data/territory-allocation-candidates.json`](spec/data/territory-allocation-candidates.json) | SSOT 14.4 / 14.5 から再生成できる**派生データ**。インポート・自動テスト用 |
| [`spec/territory-allocation-data.md`](spec/territory-allocation-data.md) | 上記 JSON のデータ辞書、不変条件、SSOT との照合結果 |
| [`research/civ6-modding-readiness.md`](research/civ6-modding-readiness.md) | 実装着手の準備度調査。手元の Civ6 環境、Civ6 MOD の技術的制約、SSOT の不足 |
| [`research/court-arbitration.md`](research/court-arbitration.md) | 日本マップの朝廷・将軍による停戦裁定・所領安堵の設計案（選択肢と推奨） |
| [`research/japan-1534-start.md`](research/japan-1534-start.md) | 1534年日本マップの当主・城・旧国割当の出典付きレビュー草案 |
| [`spec/data/japan-1534-start.json`](spec/data/japan-1534-start.json) | 史料上の留保を含む開始データ。53拠点・勢力割当を日本地図の生成器が参照 |
| [`research/japan-1534-relations.md`](research/japan-1534-relations.md) | 1534年の22勢力間の同盟・婚姻・主従・敵対の出典付き調査草案 |
| [`spec/data/japan-1534-relations.json`](spec/data/japan-1534-relations.json) | 上記の機械可読データ。開始時に反映する関係の元データ |
| [`spec/data/japan-city-name-pools.json`](spec/data/japan-city-name-pools.json) | 22勢力の都市名の一覧（漢字・読み・旧国・種別・出典）。生成は `tools/civ6-assets-rs/src/write_city_names.rs` |
| [`research/japan-map-resources.md`](research/japan-map-resources.md) | 日本マップの資源・河川・湖沼・植生・火山の調査と出典 |
| [`spec/data/japan-map-resources.json`](spec/data/japan-map-resources.json) | 上記調査の機械可読な候補・配置規則 |
| [`research/japan-1534-prosperity.md`](research/japan-1534-prosperity.md) | 1534年開始時の各家の規模（石高・動員力の推定）と、Civ6 の開始状態（人口・建物・区域・部隊）への写像。#48 の実装の元になった調査で、決定は SSOT 3.1 |
| [`spec/data/japan-1534-prosperity.json`](spec/data/japan-1534-prosperity.json) | 上記の機械可読データ（22勢力・56都市）。生成は `tools/civ6-assets-rs/src/prosperity.rs`、地図への反映は `tools/civ6-assets-rs/src/prosper_start.rs` |
| [`research/player-grouping-candidates.md`](research/player-grouping-candidates.md) | 世界マップのプレイヤー枠を国際組織でまとめる候補と組み合わせ例 |
| [`research/sengoku-tech-civic-tree.md`](research/sengoku-tech-civic-tree.md) | 戦国パックの技術ツリー（32）と社会制度ツリー（30）の設計案。オーナー確認前の下書き |
| [`research/sengoku-ages.md`](research/sengoku-ages.md) | 戦国パックの時代（黄金期・暗黒期）設計。公式の時代システム調査、改元ごとの判定、Lua の武功の台帳（点・判定・保存・マルチプレイ同期）、家ごとの誓約と暗黒期の専用政策、画面の見せ方、現状を測る実機シナリオ |
| [`spec/data/sengoku-reigns.json`](spec/data/sengoku-reigns.json) | 戦国パックの改元の表（弘治から令和までの史実の元号の年月と ja・en・ru の名前、令和の100年後の遅代）。`tools/civ6-data-rs/src/reigns.rs` が PoC の表と文を作る |
| [`research/sengoku-events.md`](research/sengoku-events.md) | 戦国パックの史実イベント61件（鉄砲伝来・キリスト教伝来・検地・長尾→上杉ほか）。影響勢力・早期解放の研究・史実の年のBoostと助言ポップアップ・暫定の効果案 |
| [`spec/data/sengoku-events.json`](spec/data/sengoku-events.json) | 上記イベントの機械可読データ |
| [`poc/README.md`](poc/README.md) | PoC の検証手順と結果 |
| [`poc/sengoku-tree-unlocks.md`](poc/sengoku-tree-unlocks.md) | 戦国ツリー62ノードの公式代理・未接続解放と閃きの照合 |
| [`poc/sengoku-tree-compatibility.md`](poc/sengoku-tree-compatibility.md) | 戦国ツリーPoCのGathering Storm互換処理、ローカル定義の行番号、未検証項目 |
| [`poc/sengoku-ages.md`](poc/sengoku-ages.md) | 戦国パックの時代（黄金期・暗黒期）第1段階：公式の時代スコアの停止、武功の台帳、改元の判定と通知、オーナー判断 L（時代の名前を元号に、友好宣言の点、令和・遅代までの改元、誓約は各家に固有3つ）、実機確認項目。後続1：暗黒期の政策の選択画面（EXECUTE_SCRIPT）と家ごとの既定の政策（Q70〜Q72）。後続3：改元の画面と時代スコアの画面（公式の時代の画面の絵と組み立て、元号はこの2画面だけ） |
| [`poc/sengoku-oaths.md`](poc/sengoku-oaths.md) | 家ごとの誓約66件：公式Commemorationsの調査、史実資料、3言語の名前、年齢別の効果、台帳・選択画面・AI・保存と実機シナリオ |
| [`poc/data/sengoku-oaths.json`](poc/data/sengoku-oaths.json) | 上記誓約の暫定カタログ。SQLと3言語の翻訳の生成元 |
| [`poc/tuning-200.md`](poc/tuning-200.md) | 約200ターンで暫定値を測る手順・結果・再開記録 |
| [poc/data/tuning-200-03.json](poc/data/tuning-200-03.json) | 完全性を確認した200ターンの年齢判定・観測・裁定・滅亡の実測 |
| [`poc/historical-events.md`](poc/historical-events.md) | 史実の出来事（SSOT 3.2）の仕組みと61件の一覧の実装（JSON から生成）・データ形式・暦との関係・家名の段階（織田は研究による襲名）・旧領と AI の攻略目標・助言ポップアップ・実機確認項目 |
| [`poc/unification-victory.md`](poc/unification-victory.md) | 天下統一勝利の実装、公式根拠、実機確認項目 |
| [`poc/unification-pact.md`](poc/unification-pact.md) | 天下統一の合算（チーム・1534年の開始時の同盟・天下統一同盟）：同盟の申し込み・脱退の画面、交戦での解消、AI からの申し込み、登録城のない地図での本拠の合算、主体だけが勝つ仕組み、公式根拠、実機確認項目 |
| [`poc/city-names.md`](poc/city-names.md) | 日本マップの22勢力が新しく建てる都市の名前。家ごとの史実の地名一覧・共通の予備・公式の都市名の借用（BALTIMORE）の原因と対策・実機確認項目 |
| [`poc/house-names.md`](poc/house-names.md) | 人物・家に結びつく名前を、その家以外では一般名で表示する仕組み（SSOT 3.2）。棚卸し、取り付け先、実機確認 |
| [`poc/japan-no-world-congress.md`](poc/japan-no-world-congress.md) | 日本マップで世界会議を無効化する実装、公式の根拠、実機確認 |
| [`poc/court-arbitration.md`](poc/court-arbitration.md) | 日本マップの朝廷・将軍による停戦裁定（請求・費用・拒否の不利益・将軍家の3倍と1573年・正当性・討伐の勅命）の実装、公式の根拠、実機確認 |
| [`poc/absorption.md`](poc/absorption.md) | 束2の勢力吸収（SSOT 9.1）：都市・部隊・人物・Hero・所持品・宗教の移管と保存、未マージの機能との統合口、公式根拠、自己検証する日本地図シナリオと制限 |
| [`worklogs/pr87-recovery-20261002.md`](worklogs/pr87-recovery-20261002.md) | PR87の専任担当による実機再開、候補照合、今回の成功範囲と未達条件 |
| [`poc/religion-persistence.md`](poc/religion-persistence.md) | 束2の宗教持続：確定した再興と8分岐、市民の宗教変換、宗教同盟、必須の公式英雄モード、公式根拠と保存・再読込シナリオ |
| [`poc/court-arbitration-human-progress.md`](poc/court-arbitration-human-progress.md) | 人間が裁定に回答する実機検証と、保存復元・画面表示・行高の修正結果 |
| [`poc/court-arbitration-ai-progress.md`](poc/court-arbitration-ai-progress.md) | AIの当事者請求・将軍家の仲介と1573年の境界、保存復元、200ターン計測の記録 |
| [`poc/sengoku-starting-knowledge.md`](poc/sengoku-starting-knowledge.md) | 1534年の共通初期知識（暫定）、公式根拠、実機確認項目 |
| [`poc/era-equals-reign.md`](poc/era-equals-reign.md) | 元号＝時代（オーナー判断 T）：エンジンの時代の長さのプローブ（テスト専用 MOD `JAC_Test_EraTiming`）の結果と本実装（元号ごとの 48 の時代、速度とのずれ、ツリーの時代、台帳の判定、実機シナリオ `sengoku-era-reigns.json`、叙事詩・マラソン・追加時代・偉人画面・資源の年の追加確認） |
| [`poc/sengoku-calendar.md`](poc/sengoku-calendar.md) | 戦国パックの1534年開始・速度別暦進行と実機確認項目 |
| [`poc/autotest.md`](poc/autotest.md) | Civ6 PoC 検証ドライバの使い方、時間・メモリ・保存容量・ロード時間のCSVと集計、実機シナリオとログ判定 |
| [`poc/autotest-native.md`](poc/autotest-native.md) | PR87のPythonを実行しないRust検証ドライバー、標準の待ち列との接続、対応範囲と検査結果 |
| [`poc/banseiki-pool.md`](poc/banseiki-pool.md) | 300人の登録、2人の選択、保存・再読込の検証と画像の参照元 |
| [`poc/hotseat-log-bridge.md`](poc/hotseat-log-bridge.md) | PR100の検証専用MODによるLua.log通信、元21手順と2回の保存・再読込の実機結果 |
| [`poc/leader-portraits.md`](poc/leader-portraits.md) | 1534年日本22勢力の指導者肖像、画像台帳、生成・実機確認手順 |
| [`poc/leader-persona.md`](poc/leader-persona.md) | 22人の性格・史料、進化用称号と人物別の功績、専攻・分岐を永久固定するツリー全表と専用画面、公式APIの根拠、実機用シナリオ |
| [`poc/native-slots-diagnostic.md`](poc/native-slots-diagnostic.md) | PR86の匿名本体総督枠と支払い候補を分離したテストMOD、Rust/Lua検査結果、実機で未確認の条件 |
| [`poc/pr86-native-admission.md`](poc/pr86-native-admission.md) | PR86の加入人物と本体任命資格の静的解析、実機拒否、独立Rust bridgeの元判定を保つ試作と次の実機順序 |
| [`poc/pr86-saved-price.md`](poc/pr86-saved-price.md) | PR86の加入時価格契約、価格更新をまたぐ保存、称号予約・資格・UIの生成と回帰検査、親で継続する受入範囲 |
| [`poc/pr86-native-runtime-probe.md`](poc/pr86-native-runtime-probe.md) | PR86の本体資格関数を変更せずに呼出し元・引数・戻り値を観測するRust debugger、live_chain接続、専用子プロセスでの検査と実機未確認の条件 |
| [`poc/pr86-boundary-diagnostic.md`](poc/pr86-boundary-diagnostic.md) | PR86の本体中和・未配置回復・都市喪失を観測するテスト専用MOD、独立シナリオと未確定の敵スパイ操作 |
| [`poc/pr86-boundary-progress.md`](poc/pr86-boundary-progress.md) | 境界診断を実装する担当の検査結果と、実機未観測の項目 |
| [`poc/pr86-enemy-neutralization.md`](poc/pr86-enemy-neutralization.md) | 人間の本体スパイでAIの正式加入人物Governorを攻撃する、保存再開可能な中和・回復診断と未実機条件 |
| [`poc/japan-1534-relations.md`](poc/japan-1534-relations.md) | 日本マップの開始時に1534年の関係を外交状態へ反映する実装メモと実機確認項目 |
| [`poc/autotest-driver-extensions.md`](poc/autotest-driver-extensions.md) | 自動検証ドライバーの設定・セーブ・滅亡・研究計測の根拠と実機確認項目 |
| [`poc/resources.md`](poc/resources.md) | 資源の出現（SSOT 3.2）。戦国ツリー下の技術ゲート、後発地点の Lua API とデータ、独自資源（金・漆）、実機確認項目 |
| [`poc/house-crests.md`](poc/house-crests.md) | 1534年日本22勢力の家紋・出典・原画・アートビルド・実機確認項目 |
| [`poc/japan-prosperous-start.md`](poc/japan-prosperous-start.md) | 日本マップ（史実開始）の栄えた1534年開始の実装：人口・建物・区域・改善・部隊・初期金、規模の見直しと実機シナリオ（Q28） |
| [`poc/assassin.md`](poc/assassin.md) | 束2の暗殺者：毎ターン補正する育成、全国ロック・任務、自由都市分裂・複数要因の隣国吸収・外交支持の採決・後継者、固定罰則、保存、日英露UI、18ターンの実機シナリオ |
| [`questions.md`](questions.md) | オーナーの判断が必要な事項の一覧。決まったら SSOT に反映して削除する |
| [`repository-rules.md`](repository-rules.md) | **リポジトリルール**。作業する人・エージェントが必ず守る |
| [`rust-migration.md`](rust-migration.md) | 実行ツール・生成ツール・テストを Rust へ移す手順 |
| [`rust-migration-test-inventory.json`](rust-migration-test-inventory.json) | 移行前の全テスト名と、Rust で対応する workspace の区分 |
| [`rust-migration-incremental-inventory.json`](rust-migration-incremental-inventory.json) | 移行中に main へ追加された吸収・英雄・再起動の137項目 |
| [`tooling/civ6-live-rust-driver.md`](tooling/civ6-live-rust-driver.md) | 共有の実機実行ツールを Rust 実行器へ接続する処理と変更差分 |
| [`tooling/civ6-live-reliability.md`](tooling/civ6-live-reliability.md) | 基盤だけの最小保存読込を10回連続で確かめ、停止段階と元のエラーを記録する手順 |
| [`tooling/civ6-steam-cloud.md`](tooling/civ6-steam-cloud.md) | Steamの同期警告による起動停止の原因、保存の分離と終了後の同期待ち |
| [`tooling/civ6-live-reliability-2026-10-07.md`](tooling/civ6-live-reliability-2026-10-07.md) | 2026年10月6〜7日のPR別停止原因と基盤修正・実機確認の記録 |
| [`tooling/civ6-startup-recovery.md`](tooling/civ6-startup-recovery.md) | 起動停止の実測、表示処理の復旧、オーバーレイの設定と3回連続起動の証拠 |
| [`rust-driver/README.md`](rust-driver/README.md) | 実機ドライバーの214テストの対応、レビュー指摘への修正、統合するコミットの順序 |

仕様を変える場合は `spec/ssot/` の該当ファイルを更新する。変更の経緯は Git の履歴で管理する（旧「18 更新履歴」は廃止し、18番は欠番）。節番号（3.3、14.5 など）は分割後も変えない。SSOT 以外のドキュメントと SSOT が食い違ったら SSOT が正しい。

## 目的別の参照先

必要なファイルだけを読む。全ファイルを一度にコンテキストへ入れない（SSOT 19.1）。

| やりたいこと | 読むファイル |
|---|---|
| 実装に着手できるか・何が未決か | [準備度調査](research/civ6-modding-readiness.md)、[質問リスト](questions.md) |
| 実装に着手する | [19](spec/ssot/19-agent-handover.md) → [15](spec/ssot/15-implementation-structure.md) → [16](spec/ssot/16-roadmap.md) → [17](spec/ssot/17-open-issues.md) |
| ゲーム全体像を掴む | [1](spec/ssot/01-vision.md)、[2](spec/ssot/02-progression.md)、[16](spec/ssot/16-roadmap.md) |
| 世界マップ・国の規模・領土の規則 | [3](spec/ssot/03-map-and-scale.md)（3.3） |
| 特定の国・主体のデータ | [14.4](spec/ssot/14-4-country-ledger/README.md) から該当ファイルへ（下の「14.4 の探し方」） |
| 領土主張・係争地・タイル割当候補 | [14.5](spec/ssot/14-5-territory-model.md)、[JSON データ辞書](spec/territory-allocation-data.md) |
| 外交・小国懐柔・大国合併・暗殺者 | [9](spec/ssot/09-assimilation-religion.md)（9.4、9.5。「旧案」見出しは未採用） |
| 武将の育成 | [4](spec/ssot/04-factions-characters.md)、[5](spec/ssot/05-growth-build-combo.md) |
| 表示名・訳語（日英露） | [14.1〜14.2](spec/ssot/14-ui-localization.md)、[14.3 用語辞書](spec/ssot/14-3-glossary.md) |
| 決定の経緯 | `git log -- documents/spec/ssot/` |

## SSOT ファイル一覧

| 節 | 見出し | ファイル |
|---|---|---|
| 前文 | タイトル・SSOT の扱い | [00-introduction.md](spec/ssot/00-introduction.md) |
| 1 | ビジョン | [01-vision.md](spec/ssot/01-vision.md) |
| 2 | 全体進行と開始状態 | [02-progression.md](spec/ssot/02-progression.md) |
| 3 | マップとスケール | [03-map-and-scale.md](spec/ssot/03-map-and-scale.md) |
| 4 | 勢力・人物・キャラクター | [04-factions-characters.md](spec/ssot/04-factions-characters.md) |
| 5 | 成長・ビルド・コンボ | [05-growth-build-combo.md](spec/ssot/05-growth-build-combo.md) |
| 6 | 国家運営・物流・輸送（建物を含む） | [06-nation-logistics.md](spec/ssot/06-nation-logistics.md) |
| 7 | 航空と着陸場所 | [07-aviation.md](spec/ssot/07-aviation.md) |
| 8 | 探索・装備・Artifact・英雄 | [08-exploration-artifacts.md](spec/ssot/08-exploration-artifacts.md) |
| 9 | 勢力吸収と宗教（統治・正当性・自治、暗殺者Unit、秘密結社を含む） | [09-assimilation-religion.md](spec/ssot/09-assimilation-religion.md) |
| 10 | 暗号言語と異宇宙外交 | [10-cipher-language-diplomacy.md](spec/ssot/10-cipher-language-diplomacy.md) |
| 11 | Notebooks、Next Action、Secretaries | [11-notebooks-secretaries.md](spec/ssot/11-notebooks-secretaries.md) |
| 12 | 世界法律 | [12-world-laws.md](spec/ssot/12-world-laws.md) |
| 13 | 脅威・襲来・遠征・神 | [13-threats-incursions.md](spec/ssot/13-threats-incursions.md) |
| 14.1〜14.2 | UI方針、言語 | [14-ui-localization.md](spec/ssot/14-ui-localization.md) |
| 14.3 | 用語辞書（日・英・露） | [14-3-glossary.md](spec/ssot/14-3-glossary.md) |
| 14.4 | 世界マップ国別台帳（851主体） | [14-4-country-ledger/](spec/ssot/14-4-country-ledger/README.md) |
| 14.5 | 領土主張関係と初期タイル割当データ | [14-5-territory-model.md](spec/ssot/14-5-territory-model.md) |
| 15 | 実装構造 | [15-implementation-structure.md](spec/ssot/15-implementation-structure.md) |
| 16 | ロードマップ | [16-roadmap.md](spec/ssot/16-roadmap.md) |
| 17 | 未確定事項 | [17-open-issues.md](spec/ssot/17-open-issues.md) |
| 19 | コーディングエージェントへの開発引き継ぎ | [19-agent-handover.md](spec/ssot/19-agent-handover.md) |

## 14.4 の探し方

| 探している主体 | ファイル |
|---|---|
| 国連加盟国 | 英語名の頭文字で [A〜F](spec/ssot/14-4-country-ledger/un-members-a-f.md) / [G〜N](spec/ssot/14-4-country-ledger/un-members-g-n.md) / [O〜Z](spec/ssot/14-4-country-ledger/un-members-o-z.md) |
| 聖座（バチカン）、パレスチナ | [un-observers.md](spec/ssot/14-4-country-ledger/un-observers.md) |
| 州・県・自治領・係争主体など（656件） | 親国IDのファイル `14-4-country-ledger/subunits/<親国ID>.md`（例: [RUS.md](spec/ssot/14-4-country-ledger/subunits/RUS.md)）。一覧は [subunits/README.md](spec/ssot/14-4-country-ledger/subunits/README.md)。親国の記載がない TWN・XKX・ESH は [no-parent.md](spec/ssot/14-4-country-ledger/subunits/no-parent.md) |
| 未編入の主体（SOM-NES など） | [pending-unregistered.md](spec/ssot/14-4-country-ledger/pending-unregistered.md) |
| NATO・EU・自由連合などの協定 | [agreements.md](spec/ssot/14-4-country-ledger/agreements.md) |
| M49 の追加14地域 | [m49-audit.md](spec/ssot/14-4-country-ledger/m49-audit.md) |

ID が分かっているときは検索が速い。

```bash
grep -rn '^| CYP-N |' documents/spec/ssot/
```

## 起動・ロード境界の調査

- [通常起動とロード前閉鎖のPE一次資料、Rust候補照合と不足条件](poc/todo880-bootstrap-primary.md)

## 現在の状態（2026-09-28 時点）

- 仕様は揃いつつあるが、MOD の技術的成立性はまだ実証していない。次の作業は SSOT 16.1 の PoC で、束0（単独プレイの成立）と束1（MVP基盤）から始める
- 実装用ディレクトリ（SSOT 15.1 の `core-content/` 等）はまだ存在しない
- 世界マップの基準日・実ポリゴン・タイル座標は未入力。JSON のタイル関連フィールドはすべて空
