# PR #87 実機検証とマージの再開記録

2026-10-02 07:57 JST。PR #87は未マージ。担当ジョブは `20261002-075213-todo-437`。

## 確認済み

- 指定のSkill、PCのAGENTS.md、対象の `.claude/CLAUDE.md` と `documents/repository-rules.md` を読んだ。
- `feat/absorb` とGitHubの先端は `df8d41a8facdcadfa3430c637d23d7a5fdab057d`。pushとls-remoteも一致。
- 元のabsorbには製品の未コミット差分なし。未追跡 `.claude/state/` は内容を開かず保持。
- CodeQL成功。別モデルレビュー `20261002-003350-rev-absorb` とPRコメントが一致し、コードMustは0。実機3本が残るMust。
- 前担当の検査証拠は `D:/CC/Work/pr87-shepherd-20261002/` と `D:/CC/Work/pr87-shepherd-20261002b/self-review.md`。通常691件、ドライバー252件、吸収Lua120件、省略0、3本checkが成功。

## 実行中

標準ツールが実機ロックを取得。PIDは53696。前担当のロック・待機ファイルは変更していない。

```text
D:/CC/Tools/civ6-live/live_chain.exe --test-mode --stop-on-failure feat/absorb:absorption-japan.json feat/absorb:absorption-hero-recall.json feat/absorb:absorption-priest-diagnostic.json
```

- 起動元: `C:/Users/makhmeto/Desktop/dev/ADJ/civ6-banseiki-wt/absorb`
- 標準出力: `D:/CC/Work/pr87-todo437-20261002/live-chain-1.stdout.txt`
- 標準エラー: `D:/CC/Work/pr87-todo437-20261002/live-chain-1.stderr.txt`
- コマンドの子プロセスはCreateNoWindowで起動。ゲームの起動・終了はロックを持つ標準ツールに任せる。
- 調査時の `live_chain --help` は待機予約になったため、自分のPID80972とその待機ファイルだけ終了・削除した。ゲームは起動しておらず、実機試行には数えない。

## 残作業

1. 同一候補の3本を127・101・13手順すべて完走し、RESULT、result.json、steps.jsonl、Lua.log、Modding.logを照合する。
2. 失敗を調査・修正し、必要な検査、コミット・push、別モデルレビューを行う。実機はこの担当で最大4回。上限時は未完と理由をPRへ残す。
3. Mustがなく全ゲートが成功したらmerge_pr.exeでマージし、そのmainコミットのCIを確認する。
4. マージ後に共有ToDo306・433・437・5を完了とする。397・116は未完了を維持する。
5. 未コミットの記録を保全し、マージ済みworktreeを整理する。通知あり・メンションなしのマージ報告を1件送る。

クラッシュ後は、先に担当ジョブ・実機ロック・上記PID・既存のRESULTを照合し、二重実行を避ける。mainやleaderpersonでブランチを切り替えない。
