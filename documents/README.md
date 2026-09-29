# documents/ 索引

本プロジェクト（Civ6 日本列島・世界・宇宙 周回育成型トータルコンバージョンMOD）のドキュメントの入口。

## 正本と派生物

| パス | 位置づけ |
|---|---|
| [`spec/ssot/`](spec/ssot/) | **唯一の仕様正本（SSOT）**。章・節ごとのファイルに分割している。本文中の「本書」はこのディレクトリ全体を指す |
| [`spec/data/territory-allocation-candidates.json`](spec/data/territory-allocation-candidates.json) | SSOT 14.4 / 14.5 から再生成できる**派生データ**。インポート・自動テスト用 |
| [`spec/territory-allocation-data.md`](spec/territory-allocation-data.md) | 上記 JSON のデータ辞書、不変条件、SSOT との照合結果 |
| [`research/civ6-modding-readiness.md`](research/civ6-modding-readiness.md) | 実装着手の準備度調査。手元の Civ6 環境、Civ6 MOD の技術的制約、SSOT の不足 |
| [`research/japan-1534-start.md`](research/japan-1534-start.md) | 1534年日本マップの当主・城・旧国割当の出典付きレビュー草案 |
| [`spec/data/japan-1534-start.json`](spec/data/japan-1534-start.json) | 史料上の留保を含む開始データ。53拠点・勢力割当を日本地図の生成器が参照 |
| [`research/japan-map-resources.md`](research/japan-map-resources.md) | 日本マップの資源・河川・湖沼・植生・火山の調査と出典 |
| [`spec/data/japan-map-resources.json`](spec/data/japan-map-resources.json) | 上記調査の機械可読な候補・配置規則 |
| [`research/player-grouping-candidates.md`](research/player-grouping-candidates.md) | 世界マップのプレイヤー枠を国際組織でまとめる候補と組み合わせ例 |
| [`research/sengoku-tech-civic-tree.md`](research/sengoku-tech-civic-tree.md) | 戦国パックの技術ツリー（32）と社会制度ツリー（30）の設計案。オーナー確認前の下書き |
| [`poc/README.md`](poc/README.md) | PoC の検証手順と結果 |
| [`poc/sengoku-tree-unlocks.md`](poc/sengoku-tree-unlocks.md) | 戦国ツリー62ノードの公式代理・未接続解放と閃きの照合 |
| [`poc/sengoku-tree-compatibility.md`](poc/sengoku-tree-compatibility.md) | 戦国ツリーPoCのGathering Storm互換処理、ローカル定義の行番号、未検証項目 |
| [`poc/autotest.md`](poc/autotest.md) | Civ6 PoC 検証ドライバの使い方とログ判定 |
| [`poc/autotest-driver-extensions.md`](poc/autotest-driver-extensions.md) | 自動検証ドライバーの設定・セーブ・滅亡・研究計測の根拠と実機確認項目 |
| [`questions.md`](questions.md) | オーナーの判断が必要な事項の一覧。決まったら SSOT に反映して削除する |
| [`repository-rules.md`](repository-rules.md) | **リポジトリルール**。作業する人・エージェントが必ず守る |

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

## 現在の状態（2026-09-28 時点）

- 仕様は揃いつつあるが、MOD の技術的成立性はまだ実証していない。次の作業は SSOT 16.1 の PoC で、束0（単独プレイの成立）と束1（MVP基盤）から始める
- 実装用ディレクトリ（SSOT 15.1 の `core-content/` 等）はまだ存在しない
- 世界マップの基準日・実ポリゴン・タイル座標は未入力。JSON のタイル関連フィールドはすべて空
