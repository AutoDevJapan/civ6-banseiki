対象SHA: 4eb455b0d454aad7ba999c1976824933ccd577af
結論: 前回の観測器 Suggestion は解消。追加のコード Must はありません。

- Must（受入・未達）— tools/cc-live-rs/examples/todo2317_proof.rs:291-304。実機を動かしていないため、隔離・復元・CPU描画・Steam解放の実動作は未確認です。Must2 は静的確認で解除できません。Civ6 の割当て下で一度実行し、success=true と証跡を確認してください。
- Suggestion（解消）— tools/cc-live-rs/examples/todo2317_proof.rs:6-30,109,124-125,263。PID再利用や別世代のreceipt・古いSteam journalを今回の実行結果と誤認する懸念に対し、child PID/作成FILETIME/tokenとprofile receiptを照合し、子のTEMP/TMPを専用evidence-dirに設定、既存journalを拒否して同ディレクトリだけを走査します。tools/cc-live-rs/src/chain.rs:148-152 と src/lib.rs:22-30 でもjournal保存先がTEMPに接続することを確認しました。
- Nit — tools/cc-live-rs/examples/todo2317_proof.rs:352-364。テストがmanifest配下のPID固定名フォルダーを使うため、異常終了で残るとPID再利用後にcreate_dirが失敗します。一意なtemp-dirと後始末ガードを使うと再発を防げます。

共有runnerのcompiler-artifactが指す同一worktreeのlive_chain.exeを再照合し、SHA256は指定値 AD937028E461CAA2059388B626E506D19450FDE54BFBB1922FB71857D9222A43 と一致しました。