結論: Must1の復旧永続化は候補コードで満たします。Must2の実機証明は未達なので完了判定できません。

## 対象

- 候補: `fb67c41d771305c7ef5c15a7b256e54f8e05399e`（前候補 `a6d1cc5b32b6137efd898399896360e5b7d385c8` との差分3ファイル）
- 公開PR143の通常一回分離: `8248c3970c134330449d088a53acfa5aa3e5cd06`。PR現head `7c3c265b8088cc934a782679853aade099ede459` との差は状態記録JSONだけでした。

## 確認結果

`live_chain.rs:79-90,205-226,492-522` では、AppOptionsの原バイトを新規バックアップへ書いて同期し、復旧receiptも新規作成・同期してから設定を書き換えます。START境界で標準leaseのPID・作成FILETIME・tokenとゲーム不在を検証し、前receiptのバックアップを同じ読込バイトのSHA・長さで確かめ、復元後の原バイト照合とreceipt削除が終わるまで新driverへ進みません。元backupは残り、次のbackupは前receiptの復旧後に作られます。Must1はコード上解消です。

`scoped_tests.rs:526-620` はWindows共有違反後の次回再開、`--stop-on-failure` 時のdriver未起動、receipt破損・backup差異・欠落時のdriver未起動を扱います。既存記録は重点2件成功、候補全体194件成功・従来skip4です。この追審ではテストを再実行していません。

候補差分は指定された3ファイルだけで、製品の試験回数・cold導入・save/load条件を変えていません。PR143の8248c397は通常実行ごとのautosave保管先を分け、reliability側は従来の保管先分岐を維持しています。10回のreliability条件も変わっていません。

## 指摘

- **Must 2・未達（受入証拠）** — `examples/todo2317_proof.rs:249-263`。ここは実際のrenderer、Steam終了、原Autosave/AppOptions/seedの復元、receiptとlease消失をそろえて成功にしますが、今回の実機実行・配備は0件です。静的試験の成功を代用しないでください。解除には、候補ソースと一致する実行ファイルSHAで標準経路をCPU描画で一度実行し、各原本の前後SHAと終了・復元証拠を記録してください。
- **Suggestion・同一実行の照合が残る** — `examples/todo2317_proof.rs:128-149,220-240`。leaseは`chain_pid`だけで照合し、AppOptions receiptのPID/FILETIME/tokenをleaseと比較しません。Steam journalもPID接頭辞に合う既存ファイルのどれか1つで終了相を認めるため、PID再利用時に古い記録を拾えます。receiptの3識別値を現在のleaseと照合し、journalは今回の実行で生成した正確なパスまたは一意な実行IDに結び付けてください。総合成功には子の終了コードと原本復元条件も必要なので、これは成功判定のMustとはしません。
- **Nit: 0件。**

## 検証範囲

候補ソースSHAは `fb67c41d771305c7ef5c15a7b256e54f8e05399e`。指定の前候補との差分と既存検査記録を読み、公開PR143の8248c397および現headとの差を照合しました。実機0のためMust2は未達のままです。