# ToDo2261 CPU設定復元の独立検査

固定候補 `fe3d89a08d1bda94b66e07e63000636faa6fc798` を、基準 `ff7bce0b875cfbdacedab09399f9f2a9248239ad` と比較した。ToDo880から割り当てられた有限の独立検査であり、製品ソースや親worktreeは変更しない。

## 再開用の状態

- 初期ブランチ `scheduler/todo-2261-pr86-cpu` は `6d21b697c3ae69797fcdfa312b8597cddbe6e721` のclean状態で保持した。候補と分岐していたため、検査専用ブランチ `check/todo-2261-cpu-recovery-fe3d89a0` を固定候補から作成した。resetなし。
- 原ログと結果は `D:/CC/Work/todo880/cpu-recovery-fe3d89a0-2261-20261010/` に保存する。各検査のmetaには固定source、コマンド、終了コード、出力SHAを含める。
- 一時生成先の所有IDは `e5023b34e1a7044ba69467cd1797a4ec`。場所は `C:/Users/makhmeto/AppData/Local/Temp/codex-owned-e5023b34e1a7044ba69467cd1797a4ec/target`。共有Cargoゲートとsccacheを使い、同じ生成先の処理を直列実行した。
- 現候補のcc-liveは161件成功、ignored2。恒久収録の8回帰を含む。旧候補の153件は加算しない。全target Clippyとedition2021のfmtも成功した。
- native-probe bootstrap-orderは指定された所有OS検査2件のみ成功。停止後のCore不在拒否で5threadのDRが不変であることを2ケースで確認。関連probe/autotest全target Clippyと変更fixtureのRust2021fmtも成功した。
- 独立追審はMust0、Suggestion0、Nit0。旧Must3件は解消。復元guardはcallerへ渡してから復元し、失敗時はbackupとlease記録を保持する。prepare失敗後は元bytesへ戻してから次シナリオへ進む。
- 次の作業は同候補のrelease observer/diagnostics/autotest/bridge cdylib/live_chainのビルド、artifact SHA保存、標準path配備可否の再確認、最終結果JSON保存。稼働中なら配備を行わず原artifactを保持して有限検査を完了する。

## 範囲

GPU・Civ6・Steamを起動しない。ファイルとWindows共有handleは専用fixture内で実処理し、ゲーム・外部コマンドはRunnerへ注入する。Interruptedの結果は実Ctrl+C信号と区別する。本人profile、秘密情報、APIキーは使わない。追加AIジョブ、PR作成、マージ、親完了待ちは行わない。

Original113・保存・RNG・支払いに関する既存の検証範囲は維持し、元7条件は0/7のまま。この検査を製品完成や本体の全終了の証明にしない。
