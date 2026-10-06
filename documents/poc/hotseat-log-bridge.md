# PR100 ホットシート検証用ログ通信

この文書は共有ToDo #9の検証用通信について、実装した範囲と実機で未確認の境界を記録する。成功条件は元の `tools/civ6-autotest/scenarios/multiplayer-hotseat-growth.json` の21手順のままである。2人の人間プレイヤー、各2ユニット、自然成長、2回の保存・再読込後の全値一致を省略しない。単独プレイのTCP経路は変更しない。

## 構成

`civ6-hotseat-bridge prepare` は実行ごとにnonceとUUIDを作り、専用MOD、元の21手順を保ったシナリオのコピー、`bundle.json` を空の出力先へ生成する。MODのInGame UI Addinは何も描画しない。`GameConfiguration.IsHotseat()` とゲーム設定内の実行nonceが一致した場合だけキーを扱う。設定nonceは準備済みシナリオの `configure` 手順で書き込む。

Lua側は公式の `Game`、`Players`、`Unit:GetProperty` から、その時点のターン、ローカルプレイヤー、人間・手番・一時停止、`PlayerChange` の実際の表示状態、2人×2ユニットのLevel、XP、性格、Trait、装備、Artifact、最終加算ターンを読む。値を作ったり成長値を書き換えたりしない。元の `ui_lua` のコードを9個の関数として生成し、F13〜F21で対応する手順を一度だけ実行する。F22は公式 `UI.RequestAction(ACTION_ENDTURN)`、F23は `Network.GetGameConfigurationSaveType()` と `Network.SaveGame`、F24はゲーム内メニューと同じ `Events.ExitToMainMenu()` を一度要求する。公式 `PlayerChange` のReturnは、表示中かつ停止中のログを確認した後に一度送る。メニューでの読込前には公式の `Network.LeaveGame()` を一度呼ぶ。

ログは `JAC_HS|1|nonce|scene|serial|step|kind|part/total|hex` で分割する。RustはLua.logの世代・オフセット・先頭/末尾ハッシュを保存し、同じ世代の新しいバイトだけを受け取る。nonce、場面世代、連番、手順番号、チャンクの欠落・重複・順序を検査する。送信前に操作済み記録を同期書き込みし、返答が途絶えても更新操作を再送しない。照会は新しいログの読取だけを繰り返す。PID、起動時刻、ウィンドウの所有が変われば停止する。保存は実行nonce入りの一意名と `SaveComplete`、空でない新しい実ファイルで確認する。読込後は新しい `BOOT` と4ユニット全値を保存前と比較し、さらに元シナリオの `equals_capture` を照合する。

## 実機入口

この担当はゲームを起動していない。以下は主担当が実機枠で使うための入口である。`<...>` は主担当の実環境のパスに置き換える。生成先は空のフォルダとする。

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

## 未実証の境界

- Civ6実機で、追加したInGame UI AddinがF13〜F24を受け取ること、`PlayerChange` を別コンテキストから正しく読めることは未確認。Lua 5.1スタブでは元の9個の手順コードを実行した。
- FrontendのMOD登録は未実証なので、前画面に検証用UI Addinを登録していない。HostとメニューLoadには既存FireTunerのFrontend経路を使う。Host後またはLoad後にFireTunerが停止した場合は再送せず、StagingRoomのReady操作とログの新場面を診断する。
- ReadyButtonの実control矩形から入力する処理はこのRustツールには入れていない。主担当の画面測定結果を利用し、文字側の座標を流用しない。
- SaveCompleteと実ファイル、Return後の非表示・一時停止解除・人間の手番、ロード後の新BOOTを実機でまだ確認していない。ログがないときは結果不明として止まる。単独プレイやLANをホットシート成功に読み替えない。
- 実ゲームの状態ポインターとUIスレッド呼出経路が不明なため、DLL呼出やバイナリパッチは含めない。公式Lua全文も配布しない。

## 検査

生成Luaと元シナリオの9個の関数をmluaのLua 5.1でロード・実行する。テストでは成長値を模擬し、4ユニットの全値比較、古いログ、欠落・重複・順序違い、nonce相違、送信後の返答不明、所有喪失、保存・読込の場面世代を扱う。実機の自然成長や入力到達の証拠にはしない。実機では追加UIを画面に何も描かずに有効にして、待機中と開始可能な交代画面を別に記録する。元の21手順はnonceと専用MOD ID以外の差を拒否し、配置済みLuaとMOD定義の内容を検査する。さらに実行中の3種のログを既存の厳格判定に通す。
