# 約200ターンの作業の再開先

共有 ToDo 98 の依頼どおり、`../tuning` に `origin/main` から作業ブランチ `feat/tuning-200` を作成した。

実装・結果・残作業は、その作業フォルダの `documents/poc/tuning-200.md` に記録する。スケジューラが再開するこのフォルダは維持する。共有 ToDo の自分の項目の状態は変更しない。

実行中の記録は、このフォルダの `.tuning-runtime/run-state.json` と `live.log`。実行成果物は `../live/tools/civ6-autotest/artifacts/tuning-200-XX`。再開時はプロセス、共通の `D:/CC/Tools/civ6-live/civ6.lock`、成果物の `steps.jsonl` を照合して、二重実行を避ける。

まだ終わっていないこと：200ターンの実機実行、データの集計、設計意図との照合、根拠がある定数の調整、可能なら再実行、結果のPR作成とPR shepherdへの引き継ぎ。
