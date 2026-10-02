# 実行ツールとテストを Rust へ移す手順

2026年10月2日のオーナー指示に従い、Python の実行ツール・生成ツール・テストを Rust に移す。移行中も、既存のシナリオと成果物の形式を保つ。

1. `tools/` に Cargo workspace を作り、実機シナリオの実行器を移す。FireTuner の通信、ログの基準位置、入力、画像の取得、再起動、MOD の選択と計測を既存のテストで確認する。
2. MOD の Lua を実行するテストは、mlua に Lua 5.1 を組み込んで移す。データと生成ツールのテストも移し、実行を省略するテストを残さない。
3. 共有の実機実行ツールを、検査対象のブランチにある Rust の実行器をビルドして呼ぶ構成にする。Rust の実行器を含まない旧ブランチだけは、旧実行器を使えるようにする。
4. 共有ロックを持つ `live_chain.exe` で既存の短いシナリオを完走する。その後、呼び出し元と文書を切り替え、Python のファイルを削除する。
5. PR ごとに別モデルのレビューを行い、Must を修正する。全テストと必要な実機検査が通った後にマージする。

進み具合は共有 ToDo の civ6 プロジェクトに記録する。

## 移行後の構成

| Crate | 内容 | 移行前の対応項目 |
|---|---|---:|
| `civ6-autotest-rs` | 実機シナリオ、FireTuner、入力、撮影、再起動、ログ判定、計測 | 214 |
| `civ6-lua-tests-rs` | MOD の Lua を mlua の Lua 5.1 で実行する検査 | 358 |
| `civ6-data-rs` | 戦国データの生成7本と、データ・SQL・翻訳などの検査 | 166 |
| `civ6-assets-rs` | 地図・画像の生成と検査 | 49 |
| `civ6-reports-rs` | 計測報告と裁定シナリオの生成4本 | 29 |
| `civ6-migration-tests-rs` | 元の対応表の照合と、Python ファイルが残っていないことの検査 | — |

元の816項目を [移行前の一覧](rust-migration-test-inventory.json) に固定し、各 crate の `tests/parity.json` と照合する。データの166項目には、テスト用補助関数1件を含む。補助関数の ALL/ANY の動作も Rust で検査する。

移行中に main へ入った PR #87 の追加137項目も、[追加分の一覧](rust-migration-incremental-inventory.json) に記録する。吸収の Lua 120項目、英雄のデータ8項目、再起動の9項目を、それぞれの追加対応表で照合する。当初の816項目の一覧を上書きせず、合計953項目を引き継ぐ。

Lua の比較は、数値と文字列、`nil` と `false`、戻り値の数、数値キーと文字列キーを区別する。Lua テストはゲーム本体のインストールの有無によって省略しない。公式データを検査するテストには、出典とハッシュを記録した固定ファイルを使用する。

```powershell
cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets
cargo build --locked --release --manifest-path tools/Cargo.toml --workspace --bins
tools/target/release/civ6-autotest.exe tools/civ6-autotest/scenarios/bundle0-load-save.json --check
```

`--check` はシナリオの形式だけを検査する。実機を使うときは、[共有の実行ツール](tooling/civ6-live-rust-driver.md) に従って `D:/CC/Tools/civ6-live/live_chain.exe` を通す。ゲームを使っている担当がいる場合は共有ロックで待ち、他の担当が待っていれば30分の期限で譲る。実行後は Civ6 を終了する。

## 削除前に確認した実機の結果

`bundle0-load-save.json` は Rust の実行器で、Steam からの再起動、MOD の有効化、開始、通常の手番進行、撮影、保存、メニューへの退出、読込、再進行まで完走した。

| 実行 | 検査したコミット | 手順 | 判定 | 結果 |
|---|---|---:|---:|---|
| `rust-autotest-driver-02` | PR #93 の実行器（コミット ID 未記録） | 11/11 | 12/12 | exit 0、保存前と復元後の値が一致 |
| `rust-autotest-driver-03` | `a1ddf6a` | 11/11 | 14/14 | exit 0、MOD の読み込みエラーの検査を含む |
| `rust-autotest-driver-06` | `ce797d3` と main の `8ff592e` の統合 `9964715` | 11/11 | 14/14 | exit 0、最新の CLI と所有 MOD の検出を含む |

いずれも早送りのテストモードは使っていない。実行後に Civ6 の終了を確認した。後続では、Lua のテスト比較に加え、main に入った再起動の期限と残留ヘルパーの扱いを移植した。実機記録のコミットと、その後の単体テストの対象を区別して記録する。
