# PR100 ホットシート検証用ログ通信

この文書は共有ToDo #9の検証用通信について、実装した範囲と実機で未確認の境界を記録する。成功条件は元の `tools/civ6-autotest/scenarios/multiplayer-hotseat-growth.json` の21手順のままである。2人の人間プレイヤー、各2ユニット、自然成長、2回の保存・再読込後の全値一致を省略しない。単独プレイのTCP経路は変更しない。

## 構成

`civ6-hotseat-bridge prepare` は実行ごとにnonceとUUIDを作り、専用MOD、元の21手順を保ったシナリオのコピー、`bundle.json` を空の出力先へ生成する。MODのInGame UI Addinは何も描画しない。`GameConfiguration.IsHotseat()` とゲーム設定内の実行nonceが一致した場合だけキーを扱う。設定nonceは準備済みシナリオの `configure` 手順で書き込む。

Lua側は公式の `Game`、`Players`、`Unit:GetProperty` から、その時点のターン、ローカルプレイヤー、人間・手番・一時停止、`PlayerChange` の実際の表示状態、2人×2ユニットのLevel、XP、性格、Trait、装備、Artifact、最終加算ターンを読む。値を作ったり成長値を書き換えたりしない。元の `ui_lua` のコードを9個の関数として生成し、F13〜F21で対応する手順を一度だけ実行する。F22は公式 `UI.RequestAction(ACTION_ENDTURN)`、F23は `Network.GetGameConfigurationSaveType()` と `Network.SaveGame`、F24はゲーム内メニューと同じ `Events.ExitToMainMenu()` を一度要求する。公式 `PlayerChange` のReturnは、表示中かつ停止中のログを確認した後に一度送る。メニューでの読込前には公式の `Network.LeaveGame()` を一度呼ぶ。

ログは `JAC_HS|1|nonce|scene|serial|step|kind|part/total|hex` で分割する。RustはLua.logの世代・オフセット・先頭/末尾ハッシュを保存し、同じ世代の新しいバイトだけを受け取る。nonce、場面世代、連番、手順番号、チャンクの欠落・重複・順序を検査する。送信前に操作済み記録を同期書き込みし、返答が途絶えても更新操作を再送しない。照会は新しいログの読取だけを繰り返す。PID、起動時刻、ウィンドウの所有が変われば停止する。保存は実行nonce入りの一意名と `SaveComplete`、空でない新しい実ファイルで確認する。読込後は新しい `BOOT` と4ユニット全値を保存前と比較し、さらに元シナリオの `equals_capture` を照合する。

## 実機入口

最初の実装担当はゲームを起動していない。以下は主担当が実機枠で使うための入口である。`<...>` は主担当の実環境のパスに置き換える。生成先は空のフォルダとする。

共有の実機枠は `live_chain` で `hotseat-bridge-live-hold.json` を実行して保持する。この短いシナリオはゲームを起動し、FireTunerの接続を解放してから、別のRustコマンドが元の21手順を実行して最後の実ログを出すまで待つ。受け入れの代替にはならない。枠の取得時刻以降に作られた成果物だけを観測する。枠が終わるとゲームを閉じるため、最終手順の記録後の30秒以内にRust側の照合を終える。

```powershell
cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-hotseat-bridge -- prepare tools/civ6-autotest/scenarios/multiplayer-hotseat-growth.json --output <空の出力先>
```

生成された `JAC_Test_HotseatBridge` をCiv6のModsフォルダに置き、元のGrowthUIとともに有効にする。`bundle.json` とMOD内のnonceが一致することを確認する。ゲームがメインメニューにいる状態で、準備済みシナリオの有効化・設定・Host送信を次で一度だけ行う。

```powershell
cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-hotseat-bridge -- pregame <出力先>/bundle.json --logs <Civ6ログ> --saves <Saves/Multi> --mods <Modsフォルダ>
```

Host後はStagingRoomの**円形80×80の `ReadyButton`** を、実controlの矩形または確認できる画面測定から一度だけ操作する。`StartLabel` は装飾ラベルであり、固定の画面下端座標はこのツールに含めない。InGameでnonce付き `BOOT` と表示中の `DIALOG_OPEN` を確認したら、`init` がReturnを一度送る。

```powershell
cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-hotseat-bridge -- init <出力先>/bundle.json --logs <Civ6ログ>
```

`step --number` は 4〜10、12〜19、21を順に実行する。各回の `--logs` と `--saves` は同じ実パスを指定する。10と19はメニューへ退出を要求する。メニュー復帰を確認できたときに `menu-load` を使い、StagingRoomが出た場合は再び測定済みReadyButtonを一度操作する。新しいInGameの `BOOT` と `DIALOG_OPEN` の後に `adopt-load` を使う。

```powershell
cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-hotseat-bridge -- step <出力先>/bundle.json --logs <Civ6ログ> --saves <Saves/Multi> --number 4
cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-hotseat-bridge -- menu-load <出力先>/bundle.json --logs <Civ6ログ> --saves <Saves/Multi>
cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-hotseat-bridge -- adopt-load <出力先>/bundle.json --logs <Civ6ログ>
```

`bridge-journal.jsonl` に送信済み手順とReturn、保存・読込の場面世代を残す。同じ手順を再実行する入力は拒否する。失敗時はこの記録とログを診断し、新しいnonceで別の実機試行にする。

## 実機で見つかった準備処理の不具合

2026年10月6日の実機11では、現在の候補 `f97a455e` でゲームの再起動とメニュー到達を確認した。その後、`pregame` が設定ファイルを読む際にWindowsのエラー33で停止した。MOD有効化・設定・HostGameは未送信であり、ホットシートの成功には数えない。待機用の検査を終了し、実機枠の終了処理でゲームを閉じた。

原因は、設定ファイル `bundle.json` に排他ロックを掛けた後、別のハンドルで同じファイルを読み直していたことだった。Windowsのバイト範囲ロックは同じプロセスの別ハンドルからの読取も拒否する。排他用の `bundle.lock` を設定ファイルと分け、すべての操作で共通に使う。設定ファイルの実体のパスからロック先を決め、別表記でも同時実行を拒否する。設定の読取、競合する操作の拒否、ロック解放後の再取得をRustの検査に追加した。

修正前の全workspace検査は102対象・1,356件が成功し、Lua 5.1を含め失敗・ignored・filteredは0件だった。実機で見つかった不具合は、その成功だけでは検出できなかった。修正後の全件検査と実機の受け入れは別に確認する。

別モデルのレビューで指摘された送信前の接続断は、専用CLIと通常Runnerで同じ送信処理を使う形に直した。有効化、設定の各段階、Host、メニューでの読込は、未送信と確認できる接続エラーだけを期限内で待つ。専用CLIは再試行のたびにゲームとウィンドウの所有を確認する。すでに応答を受けた設定の段階や、送信後の返答不明を再送しない。準備前のメニュー確認が失敗した場合には送信済みの印を作らない。

共通化の途中でMODの切り替えにも5秒の上限を適用したところ、手順が指定する90秒を維持する既存の検査が失敗した。MODの切り替えは手順の時間を使うよう修正し、関連92件が成功した。全件検査はこの修正後にやり直す。

元シナリオのMOD有効化には `exclusive_user_mods` を指定し、対象外のユーザーMODを無効にする。公式DLCは残す。元の21手順と成長・交代・保存読込の判定は維持する。生成版は、この元シナリオに実行識別子と検証用MOD IDだけを加える。

実機12では `540fdbbf` と生成した四ファイルの一致を確認し、MOD有効化、二人の設定、HostGameへの応答を取得した。準備画面の円形ボタンを今回の画像で確認し、所有するウィンドウを前面へ移した後、ウィンドウメッセージの押下・解放を一度だけ送った。ゲーム内で同じ実行識別子のBOOT、DIALOG_WAIT、DIALOG_OPENが記録された。最初の焦点取得だけの試行は入力前に止まったため、押下の再送には当たらない。

公式のReturnも一度だけ送れたが、DIALOG_CLOSEDの値はターン1・開始ターン1・人間0・手番有効・一時停止中だった。交代画面は閉じていた。検証器がこの値を直ちに不正と判定して停止したため、元の手順4以降、自然成長、保存読込は未合格である。保持用シナリオは最終結果が出ず期限切れになり、ゲームと共有枠を終了した。原ログと送信済み記録を保全した。

公式のPlayerChangeは、一時停止の希望を変更してプレイヤー情報を通知し、画面を閉じる。この二つの完了を同じ時点だと仮定しない。検証用UIは画面の閉鎖を記録した後、同じターンと人間の手番が有効で、一時停止が解除された値を読めた時だけTURN_READYを記録する。Rust側はこの新しい記録を期限内で待つ。解除されない場合、別のターンに移った場合、閉鎖が記録されていない場合は合格にせず、Returnを繰り返さない。解除が遅れるLua 5.1の実行と、肯定・否定の受け入れ検査を含む関連94件が成功した。実機での解除完了は次の試行で確認する。

実機13では `c6ce9a3c` を配置し、初回の一時停止解除と手番開始まで確認した。Returnは一度だけだった。しかし手順4の送信前に所有者照合で停止した。プロセス57148、起動時刻、ウィンドウ2625186は実際には同じだった。JSONに保存した起動時刻 `1791254084.2057667` を読むと、浮動小数点の末尾が1段階ずれ、元のビット列 `41dab117910d2b48` が `41dab117910d2b49` になっていた。所有確認を弱めず、JSONを正確に復元する機能を有効にして、隣接する二つの実測値を区別する検査を追加した。関連95件は成功した。手順4の入力は未送信で、成長・保存読込は未合格のままである。原ログと所有者・送信済み記録を保全し、保持用検査の終了でゲームと共有枠を解放した。

## 未実証の境界

- Civ6実機で、追加したInGame UI AddinがF13〜F24を受け取ること、`PlayerChange` を別コンテキストから正しく読めることは未確認。Lua 5.1スタブでは元の9個の手順コードを実行した。
- FrontendのMOD登録は未実証なので、前画面に検証用UI Addinを登録していない。HostとメニューLoadには既存FireTunerのFrontend経路を使う。Host後またはLoad後にFireTunerが停止した場合は再送せず、StagingRoomのReady操作とログの新場面を診断する。
- ReadyButtonの実control矩形から入力する処理はこのRustツールには入れていない。主担当の画面測定結果を利用し、文字側の座標を流用しない。
- SaveCompleteと実ファイル、Return後の非表示・一時停止解除・人間の手番、ロード後の新BOOTを実機でまだ確認していない。ログがないときは結果不明として止まる。単独プレイやLANをホットシート成功に読み替えない。
- 実ゲームの状態ポインターとUIスレッド呼出経路が不明なため、DLL呼出やバイナリパッチは含めない。公式Lua全文も配布しない。

## 検査

生成Luaと元シナリオの9個の関数をmluaのLua 5.1でロード・実行する。テストでは成長値を模擬し、4ユニットの全値比較、古いログ、欠落・重複・順序違い、nonce相違、送信後の返答不明、所有喪失、保存・読込の場面世代を扱う。実機の自然成長や入力到達の証拠にはしない。実機では追加UIを画面に何も描かずに有効にして、待機中と開始可能な交代画面を別に記録する。元の21手順はnonceと専用MOD ID以外の差を拒否し、配置済みLuaとMOD定義の内容を検査する。さらに実行中の3種のログを既存の厳格判定に通す。
