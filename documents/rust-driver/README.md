# 実機シナリオのRust移行とテスト対応

旧実機ドライバーの214テストは、`tools/civ6-autotest-rs/tests/parity.json` で実行可能なRustテストに対応付ける。`parity_inventory` が旧一覧との一致、重複、テスト関数の存在、無効化されたテストがないことを検査する。

以下の対応表は各担当の作業時点の記録である。表に残る未統合の記述よりも、上記の対応一覧と現在の実行結果を優先する。

| 記録 | 内容 |
| --- | --- |
| [PARITY-ANALYSIS.md](PARITY-ANALYSIS.md) | ログ判定・計測・研究集計 |
| [PARITY-DRIVER.md](PARITY-DRIVER.md) | 自動進行・外交・保存復元・再起動 |
| [PARITY-FIXTURES.md](PARITY-FIXTURES.md) | シナリオ・MOD依存・LuaとSQL |
| [PARITY-NATIVE.md](PARITY-NATIVE.md) | Windows入力・撮影・終了・MOD検査 |
| [PARITY-SAFETY.md](PARITY-SAFETY.md) | 画面遷移・ログ世代・期限 |
| [PARITY-TUNER.md](PARITY-TUNER.md) | FireTuner通信と応答不明時の扱い |
| [review-round1.txt](review-round1.txt) | 最初の別モデルレビューの指摘原文 |

## レビュー指摘への対応

- 研究レポートは本体側の `bb5c8d1` が提供する `civ6-reports-rs` を使用する。ドライバーに重複実装や追加CLIを作らず、移行した研究集計テストからこのライブラリを呼ぶ。
- ログの内容、サイズ、ファイル識別子を同じ開いたハンドルから取得する。スナップショット、基準以降の読取、待機中の読取をそろえる。
- 検査だけを行う `--check` は、ポート範囲を接続前まで検証しない。整数の構文は検査し、64ビットを超える整数も検査だけなら受け付ける。
- Luaの待機条件のnull・真偽値・配列・辞書を、旧実装の文字列表現で比較する。辞書の挿入順も維持する。
- 保存・読込CSVの合成データ判定を、実行時の `--test-mode` にそろえる。シナリオ内の記録の有無だけでは判定しない。
- 計測値を元の二進浮動小数点数から小数第3位へ丸める。正負の境界、境界の隣の値、負のゼロ、大きな有限値を検証する。CSVの改行は旧実装と同じCRLFにする。
- `.` など出力ディレクトリそのものを指す撮影先を検証時に拒否する。メニューを閉じたり撮影したりする前に停止する。

## 統合担当への受け渡し

`51c4451` より後のドライバー本体は次の順に取り込む。`2145bb7` は本体側の研究レポート変更と重複するため取り込まない。最後に、この文書を含む復旧後の修正コミットを取り込む。

第1PR用の `feat/rust-autotest-driver` には、下の本体12コミットと研究レポートの修正 `baaffd3` がすでに入っている。このブランチには、復旧後の最終修正コミットだけを取り込む。実機では、このPRのブランチで既存の `bundle0-load-save` を確認してから、後続PRで旧ツールを削除する。

```text
d144c219ee980524b5c566be88d37b6c1c32b604
923299a6d093a2ccc91e25ebd841e0158ebe766c
cddae4a18cc0599f5f3977539fff7d823eeb1151
4e2cbf7133454ad48304f8d7f42a887ce71994fb
d130ddf1c93494ee62a4a553a6344ea431654e0b
0552a5548968eeba66b5cff08ced9732b44e91e0
2fc21adb52f3f76ed37ca690f648b773465c12a1
fa2c9a837b0eb7482dd6b0aa751abdda5fe5064b
96bb226d2371a39eacf09dc749a05b6b3c6821ea
b48ab180322ad6ed3afc26ae1692f14e7cecfa12
226a10b7ce0d889a5ab7ed3bbe4ca7ad80186f31
f163a00458cc2929a4506aeab83de5c7372b381c
```

Rustテストと6CLIのビルドは、同じ出力先でCargoを一度に1つ、`--jobs 2`、`--test-threads=2` で実行する。ゲームを使う検証、別モデルによる統合後レビュー、pushとPRは統合担当が行う。旧Pythonファイルは削除しない。

## 復旧後の検証結果

Windowsで次のテストコマンドを実行し、273件が成功した。失敗、無効化、フィルターによる除外はいずれも0件だった。旧214件の対応検査を含む。6つのCLIは通常ビルドも成功した。

```powershell
cargo test --locked --offline --jobs 2 -p civ6-autotest-rs --no-fail-fast -- --test-threads=2
cargo build --locked --offline --jobs 2 -p civ6-autotest-rs --bins
```

これらは `tools/` を作業ディレクトリにして実行する。この担当ではCiv6を起動しておらず、実機での保存・読込成功を示す結果ではない。
