# Civ6用万世紀

Sid Meier's Civilization VI の周回育成型トータルコンバージョン MOD「日本列島・世界・宇宙」の開発リポジトリ。

戦国時代の日本列島で武将を育て、探索・装備・英雄・家名継承・周回を通じて日本統一を目指す。日本統一後はアジア圏と世界、世界統一・火星移住後は太陽系、さらに銀河から宇宙へと舞台を広げていく。

## 現在の状態

仕様（SSOT）を整備している段階で、MOD の実装はまだ始めていない。次の作業は SSOT 16.1 の PoC で、束0（単独プレイの成立）と束1（MVP基盤）から始める。

## 動作環境（開発の基準）

- Sid Meier's Civilization VI（Steam 版 1.0.12.68）
- 必要な拡張：Gathering Storm（ゲームの作成時はルールセット「万世紀」を選ぶ）
- 検証は当面シングルプレイで行う

## ディレクトリ構成

| パス | 内容 |
|---|---|
| [`documents/`](documents/README.md) | ドキュメント。索引は `documents/README.md` |
| [`documents/spec/ssot/`](documents/spec/ssot/) | 仕様の正本（SSOT）。章・節ごとのファイルに分割している |
| [`documents/spec/data/`](documents/spec/data/) | SSOT から作られるデータ（領土関係・タイル割当候補の JSON） |
| [`documents/research/`](documents/research/) | 調査記録（実装の準備度、プレイヤー枠の候補など） |
| [`poc/`](poc/) | PoC 用の検証 MOD（手順と結果は `documents/poc/README.md`） |
| [`tools/Cargo.toml`](tools/Cargo.toml) | Rust の実行ツール・生成ツール・Lua を含むテストの workspace |
| [`documents/questions.md`](documents/questions.md) | オーナーの判断が必要な事項 |
| [`documents/repository-rules.md`](documents/repository-rules.md) | リポジトリルール |

MOD 本体・マップ・ゲーム外の常駐アプリなどの実装用ディレクトリは、SSOT 15.1 の構成に沿って実装開始時に追加する。

## 作業を始める前に

- 必ず [リポジトリルール](documents/repository-rules.md) を読んで従う
- 仕様は SSOT だけに書く。SSOT は大きいので、[索引](documents/README.md) から必要な節のファイルだけを読む
- `main` に直接コミットせず、作業ブランチから PR を作る

ツールとテストは Rust を使用する。`cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets` で検査し、シナリオの実行方法は [検証ドライバー](documents/poc/autotest.md) を参照する。
