# 共有の実機実行ツールを Rust 実行器へ接続する

共有ツールのソースは `D:/CC/Tools/rs/cc-live`、実行ファイルは `D:/CC/Tools/civ6-live` にある。[変更差分](civ6-live-rust-driver.patch) に、実行器を選ぶ処理とそのテストを保存した。

`ship.exe live` は、共有の作業フォルダで対象ブランチを checkout し、main を統合した後に、そのブランチの実行器をビルドする。成功した場合だけ、実行用 JSON と成果物の保存先を標準出力へ返す。

- Cargo の manifest は対象ブランチの `tools/civ6-autotest-rs/Cargo.toml` を指定する。`--locked` と `--release` を使い、成果物は同じブランチの `tools/target` に作る。
- Cargo の JSON 出力にある `compiler-artifact` から、manifest と2つの実行ファイルの場所を確認する。ビルド失敗や出力先の不一致では停止する。古い実行ファイルが残っていても使用しない。
- Rust の manifest がない旧ブランチだけは、同じブランチの旧実行器を呼ぶ。Rust が存在する場合のビルド失敗は、旧実行器へ切り替える理由にしない。
- 実行前の FireTuner の確認も、同じブランチの実行器で行う。既存のシナリオ引数、成果物の形式、`RESULT` 行は維持する。
- Windows では、ビルドを含む処理を Job Object に入れてから実行を開始する。中断・タイムアウト時には子と孫のプロセスも終了し、全終了を確認する。実機の共有ロックと30分の期限は既存の処理を使う。

検査は `cargo test --locked --manifest-path D:/CC/Tools/rs/cc-live/Cargo.toml` で行う。実際の子と孫のプロセスを起動して終了を検査するテストを含む。

2026年10月2日、Windows で56件が成功した。別モデルのレビューで、ビルド中断時の子プロセスの終了と、Cargo の出力先の変更で古い実行器を選ぶ問題を修正した。再レビューの Must は0件。ビルドした実行ファイルを共有の配置先へ更新し、配置前後の SHA-256 が一致することを確認した。

PR を引き継ぐ担当への指示は `D:/CC/Tools/shepherd/shepherd_prompt.md` にあり、Rust の workspace テストとシナリオの `--check` を使用する。
