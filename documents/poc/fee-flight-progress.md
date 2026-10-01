# PR86 候補会計の第三回レビュー対応

## 実装前の設計

対象は標準Governorを使えない場合の同等代替の会計候補。正本9.4・4.7・16.1の仕様を変更しない。

- Beginで人物・段階・単調増加する処理IDを保存し、状態をPREPAREDにする。GameCoreがまだ送信許可を出しておらず、取得証跡もない場合だけ、取消しをABORTED_UNSENTとして記録して段階を解放する。
- UIは同期操作で送信許可を要求する。GameCoreは全要求の整合、残高、人物、処理ID・段階・試行番号を確認し、AUTHORIZEDと許可ターンを保存する。同一ターンの再許可はUI再初期化・セーブ再開でも拒否する。
- UIは今回の応答だけを受け、同じNative段階の未取得・任命または昇進の資格を再確認してIndexを送る。許可と実送信の間に停止・人物喪失があれば結果不明として元要求を保持する。時間経過だけで解放しない。次ターンも同じNative段階だけを再試行する。
- 支払確定はGameCoreのPlayer property receiptsと公式使用済み数だけで行う。取得済みは残高0でも追加消費せずAdvanceする。取消済みの遅い証跡は元要求の消費として保存し、次の人物へ割り当てない。
- RequestとBeginの受付で必ず全要求をReconcileし、取消後に残高を計算する。Gameplay wrapperも保存済みのCANCELLEDを実体検査より先に返す。
- 旧形式のflightは未送信と判定できないため、保存済みの結果不明状態へ移す。元要求との対応を維持する。
- 予約・Begin・送信・Advanceを実UIフレームから共通入口へ流す診断入口を追加する。元の113手順は変更しない。

## 検査と担当範囲

実コードLuaをモック環境で実行し、未送信の取消し、古い許可要求の拒否、Native不成立と同一／次ターン、UI再初期化と保存再読込、無効後続予約、取消後の遅延証跡、残高0、人物復活なしの取消済み再送を確認する。通常全体とdriverの検査も実行する。

実機・push・PR投稿・merge・新job・第三者レビューの起動は担当外。Native取得証跡の発効／保存とAI無断支出は未検証。複数端末MP実証は後続ToDo9。

指定の `D:/CC/Work/pr86-orchestrator/fee-flight-progress.md` はセッションの書き込み許可範囲外のため、この専用worktreeに記録した。親の共有ToDo449/428と進捗ファイルは更新しない。

## UIフレームでの診断入口

候補用の `leader-persona-title-proxy-15km.json` は、予約3件・準備6回・Native送信6回・確定6回を次の入口から実UIフレームへ委ねる。直後の既存の待機・残高・取得証跡・人物別割当の判定は維持する。`leader-persona-15km.json` の113手順は変更しない。

```lua
JAC_ProxyOnFrame('REQUEST', leader)
JAC_ProxyOnFrame('BEGIN')
JAC_ProxyOnFrame('PAY')
JAC_ProxyOnFrame('ADVANCE')
```

戻り値trueは実行予約の受付を意味し、支払い成功を意味しない。送信許可の応答には照合番号を付け、古い拒否応答を今回の応答として受け取らない。判定の本体は通常入口と共通のGameCoreにある。UIには今回の応答を待つ処理だけを置き、再試行できるターンは保存済みのGameCore状態が決める。

## 実施結果

- 会計関連の実コードLua検査39件が成功。追加17件ではUIとGameCoreを別のLua環境に分け、同期操作の配送を遅らせた。セーブ再読込の検査は保存対象を新しいLua実行環境へ移す。モックのNativeであり実機結果ではない。
- Must1の解放、Must2の受付時整合、Must3の同一ターン制限を、それぞれメモリ上で外した場合に対応する1件・2件・1件がassertionで失敗することを確認。検査環境の例外による失敗ではない。
- 通常全体699件、driver247件が成功。失敗・スキップとも0件。候補シナリオのLua70断片（操作41、待機条件29）も構文検査に成功。
- driverの固定一覧に既存の診断3本が未登録だったため、名前を追加した。集合の完全一致による検査を維持した。
- 通常のunittestコマンドは、このセッションでは一時フォルダの権限で失敗した。Python 3.12.10のmode 0700で作ったフォルダだけがアクセス拒否になり、mode 0777で親の権限を継承したフォルダは読み書きできることを確認。再実行は専用worktree内の一時フォルダ作成だけを親の権限継承にし、同じunittest discoveryと全assertionを実行した。
- `poc/JAC_Test_NativeSlots` と元の113手順に差分がないこと、および `git diff --check` の成功を確認。

送信許可後にNative未成立のまま人物が失われた場合、結果不明の段階は元要求に保持する。この保留を時間切れで解放しない。旧形式の保存データでも、最初の支出基準を保存して遅延証跡を受け付ける。

NativeのPlayer property receiptsの発効・保存、AI無断支出、実UIフレームからのエンジン動作は未検証。別モデルのレビューは親の担当。

## commitの阻害要因

対象9ファイルだけを指定した `git add` が、`C:/Users/makhmeto/Desktop/dev/ADJ/civ6-banseiki/.git/worktrees/pr86-fee-flight/index.lock` の作成時にPermission deniedで失敗した。Git管理領域はこのセッションの書き込み許可範囲外にある。新しいcommitは作成できず、HEADは親候補の `e6ffe2e` のまま。内容は専用worktreeに残っている。

検査用の `.fee-flight-tests/run_tests.py` と `main-result.txt`・`driver-result.txt` はworktree内の作業用出力として残した。同ディレクトリには、権限で削除できなかった一時フォルダも残る。初回の通常TEMP配下への検査出力も同じ権限障害だった。指定されたDownloadsの削除希望フォルダも、このセッションでは書き込み許可範囲外のため移動していない。
