# Civ6 PoC 検証ドライバ

`tools/civ6-autotest-rs/` の Rust 実行器は Steam 版 Civ6 1.0.12.68（Gathering Storm、シングルプレイ）に FireTuner で接続し、JSON の手順を順に実行する。シナリオで有効化する MOD の更新が起動中のゲームより新しい場合や、ゲームが停止している場合は、手順の開始前に Civ6 を再起動する。各ステップの成否と出力、画面 PNG、ログ判定用の開始位置を新しい成果物ディレクトリへ保存する。

## 準備と実行

1. 対象 MOD を Civ6 の `Mods` フォルダから参照できる状態にする。ドライバは `.modinfo` の UUID で対象を探し、ジャンクションの参照先を走査する。`bundle0-load-save.json` は `JAC_PoC_Bundle0`、クランと手帳のシナリオは `JAC_PoC_CustomCivs` と `JAC_PoC_GrowthUI` を使う。`banseiki-ruleset.json` はこの3 MOD と `JAC_PoC_BanseikiRuleset` を使う。実際に Civ6 が参照する `Mods` の配置先を確認する。
2. Civ6 は**ウィンドウ表示**で起動できる設定にする。停止中ならドライバが Steam 経由で DX11 版を起動する。`AppOptions.txt` の `EnableTuner 1` が必要。FireTuner GUI は閉じる。同じ Tuner ポートに同時接続しない。
3. Rust をインストールし、リポジトリのルートから下記の静的検査を実行する。実機検査は push 後に共有の `live_chain.exe` を使う。シナリオと成果物は従来の `tools/civ6-autotest/` に置く。

```powershell
cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-autotest -- tools/civ6-autotest/scenarios/bundle0-load-save.json --check
# 既にゲームが起動している場合の診断
cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-tuner -- states
cargo test --locked --manifest-path tools/Cargo.toml --workspace --all-targets
# push 後の実機検査。成果物の保存先は実行結果の RESULT 行に出る。
D:/CC/Tools/civ6-live/live_chain.exe <ブランチ>:bundle0-load-save.json <ブランチ>:banseiki-ruleset.json <ブランチ>:clans-notebooks.json --stop-on-failure
```

オーナーが PC の前にいるとき（確認モード）は `--owner-start` を付ける。`start_game` で自動開始せず、設定済みの「ゲームの作成」画面を開いて、オーナーが「ゲーム開始」を押すまで最大1時間待つ。以降の手順は自動で進む。

`states` は既にゲームが起動している場合の診断に使う。`FrontEnd` と `InGame` が異なる名前で現れた場合は、実際の状態名を JSON と `tools/civ6-autotest-rs/src/scenario.rs` の固定ステップに反映してから実行する。ログや保存先が標準の場所と違う場合は `--logs <Logs ディレクトリ>` と `--saves <Saves/Single ディレクトリ>`、MOD の場所が違う場合は `--mods <Mods ディレクトリ>` を指定する。

### DX11 での起動

再起動は `steam://launch/289070/option2` を Win32 の `ShellExecuteW` に渡す。2026-10-01 にこの PC の Steam 起動情報（`appcache/appinfo.vdf`）を読み、Windows の `option2` が `Base/Binaries/Win64Steam/CivilizationVI.exe`（DX11）、`option1` が DX12 版であることを確認した。ゲーム番号だけを渡す旧 URI は Steam 側の選択に依存していた。[2K の公式説明](https://support.2k.com/hc/en-us/articles/34845053169939-2K-Launcher-Complete-Sunset)でも、Steam の起動メニューとプロパティで版を選べることが案内されている。

URI の送信でコンソールは開かず、起動中の Steam の `localconfig.vdf` は書き換えない。起動後はウィンドウの実行ファイル名を検査し、DX11 でなければキー・クリック入力の前に失敗する。起動情報の変更や実行ファイルを差し替える Steam の起動設定で DX12 が選ばれても、そのままテストを進めない。起動済みのゲームの検出・終了・入力先の照合は、両方の実行ファイルに対応する。

この PC で他の実機テストと並行して確認するときは、push 後に共用ロックを持つ `D:/CC/Tools/civ6-live/live_chain.exe <ブランチ>:bundle0-load-save.json` を使う。ゲームの起動・終了はこの処理を通す。

共有の実行ツールは検査対象のブランチを checkout した後、そのブランチの Rust 実行器をビルドする。Cargo が報告した実行ファイルの場所を検査し、ビルド失敗や別の場所への出力ではシナリオを実行しない。Rust の実行器を含まない旧ブランチだけは、旧実行器を使う。待っている担当がいる場合は30分の期限に従ってシナリオの区切りで順番を譲り、実行後は Civ6 を終了する。

### FireTuner の一時的なタイムアウト

`autoplay` が途中で止まる問題（2026-09-29、15 km 日本地図＋戦国パックで 31 ターン目に `Autoplay stopped early`）。原因は実機で確定した。人間（大内、枠 14）に AI の指導者（足利）が外交の声明（「Your people are lazy and unworthy...」と Goodbye ボタン）を出し、`DiplomacyActionView` と `LeaderScene` が表示されたまま、AI（枠 10）の手番が人間の返事を待って終わらない。そのため自動進行が途中で止まる（`active=False` で残りターンあり）か、問い合わせがタイムアウトする。`DiplomacyActionView` の Lua 状態で `OnClose()`（`Base/Assets/UI/DiplomacyActionView.lua:2883-2886`、ESC と同じ処理）を呼ぶと閉じ、次の AI（枠 11）の手番へ進んだ。

ドライバは、自動進行の監視ループの各回と、状態の問い合わせが失敗したときに、表示中の外交画面を公式の処理で閉じる。AI からの取引の提案（`DiplomacyDealView`）を先に、ゲーム自身のターン終了処理 `OnLocalPlayerTurnEnd()`（`DLC/Expansion2/UI/Replacements/DiplomacyDealView.lua:3120-3129`。`OnRefuseDeal(true)` で拒否してセッションを閉じ、`OnContinue` で隠す。素のゲームは `Base/Assets/UI/DiplomacyDealView.lua:3075-3085`）で断る。取引画面が出ている間は `DiplomacyActionView` が `DEAL_MODE` で `OnClose` を無視するため、この順番にしている。次に声明を `OnClose()` で閉じる。どちらも `ContextPtr:IsHidden()` で守り、閉じる途中の再呼び出しは公式の処理側でも無視されるので、タイムアウト後の再試行は冪等になる。閉じた内容はステップログ `autoplay_diplomacy` に残す（例：`closed AI diplomacy statement (turn 31, total 1)`）。外交画面が原因で自動進行が止まったときは、再開の上限（2 回）に数えずに残りターン数で再開する。1 ステップで閉じた回数が `20 + 5 × ターン数` を超えたら失敗にする。状態の問い合わせは 30 秒の枠で再試行し、失敗したときに閉じる画面が無ければ従来どおり止める。取引の拒否は実機では未確認（要実機確認）。単体テストは `tools/civ6-autotest-rs/tests`（偽の Tuner）。閉じる画面の一覧は `tools/civ6-autotest-rs/src/scenario.rs` の `POPUP_CLOSERS` で、戦国パックの暗黒期の政策の選択画面（`JAC_SengokuDarkPolicy` の `OnClose()`。閉じると既定の政策のまま。[時代の PoC](sengoku-ages.md)）も同じ扱いで閉じる（ステップログは `closed dark-age policy choice`）。

自動進行の外でも、AI の声明が `InGame` を止めることがある（owner-o-03・05：`wait_for` の `InGame` への問い合わせが `FireTuner response timed out after 0/8 bytes`。外交画面の Lua 状態は応答を続ける）。`InGame` への `wait_for` の問い合わせ（`save` の完了待ちも含む）が応答無しで失敗したら、ドライバは同じ `POPUP_CLOSERS` で一度だけ閉じ、閉じた画面があれば待ち直す。待ちの条件は読むだけの式なので再実行してよい。1 ステップで 3 回まで、ステップログは `ingame_unblock`（例：`closed AI diplomacy statement after an InGame timeout; retry 1/3`）。閉じる画面が無ければ従来どおり止める。`ui_lua` は、送った後に応答が無かったときは実行済みかもしれないので再実行しない。送る前（状態の一覧の取得）で失敗したときだけ、同じように閉じて送り直す。単体テストは `tools/civ6-autotest-rs/tests` の `StarvedInGameStepTests`。

時代の切り替わりで人間プレイヤーに出る奉献（Dedication、Gathering Storm）の選択も、自動進行は人間の代わりに選ばない（`DLC/Expansion2/UI/Additions/DedicationPopup.lua`）。これで止まった例は実機では確認していない。ドライバは、停止が 5 秒続いたとき、まず公式の操作 `UI.RequestPlayerOperation(human, PlayerOperations.COMMEMORATE, {[PlayerOperations.PARAM_COMMEMORATION_TYPE] = idx})` を、`Game.GetEras():GetPlayerCommemorateChoices()` の先頭から `GetPlayerNumAllowedCommemorations()` 件だけ決定的に送り、Escape でポップアップを閉じて、残りターン数で自動進行を再開する。選んだ番号はステップログ `autoplay_dedication` に残す。選択肢が無ければ従来どおり再開のみ（最大 2 回）。奉献の選択は 1 ステップ内で最大 3 回までで、その後もステップ期限内に進まなければ失敗にする。単体テストは `tools/civ6-autotest-rs/tests`（偽の状態列）。

ゲームが重いと、FireTuner が 5 秒以内に返事できず `No Lua acknowledgment ...` になることがある（2026-09-29、15 km 日本地図の 10 ターン自動進行で発生。ゲーム自体は生きていた）。`autoplay` の状態問い合わせとアドバイザーポップアップの解除は読み取り専用または冪等（`IsBlockingInput()` で守られている）なので、接続を張り直してバックオフ付きで再試行し、ステップの期限が切れたときだけ失敗にする。ゲームの状態を変えるコマンド（自動進行の開始、オプションメニューを閉じる操作など）は再送せず、従来どおり `LuaResponseUnknown` で止める。

### 自動鮮度確認と再起動

実行開始時、全 `enable_mods.ids` に対応する `.modinfo` を `Mods` 直下の各 MOD フォルダで探し、ジャンクションを解決した参照先の全ファイルから最も新しい更新時刻を取得する。起動中の `CivilizationVI.exe` または `CivilizationVI_DX12.exe` の開始時刻よりその時刻が**新しい**場合、またはゲームが起動していない場合は、`steps.jsonl` の `auto_restart` に理由を記録して再起動する。MOD が見つからない、同じ UUID が複数ある、Civ6 プロセスが複数ある場合は失敗として止める。更新時刻が等しい場合は再起動しない。`--no-auto-restart` はデバッグ時のみ鮮度確認を省略する。

再起動はまず FireTuner の `FrontEnd` / `InGame` UI 状態から `Events.UserConfirmedClose()` を呼ぶ。接続できなければ、実行ファイル名とタイトルを照合した Civ6 ウィンドウに閉じる要求を送る。15 秒待っても元のプロセスが残る場合は PID と開始時刻を再確認してから終了させる。旧 EXE の終了後も Steam が旧セッションを解放するまで 8 秒待ち、`steam://launch/289070/option2` で DX11 版を再起動する。新しいゲームが現れなければ 45 秒間隔で最大 4 回起動を試す。確認済みの Civ6 ウィンドウを左クリックしてオープニングを飛ばす。動画が続いて `FrontEnd` が応答しなければ、安全な位置を 12 秒間隔で再度左クリックし、`UI.IsInFrontEnd()` が `true` を返すまで待つ（the opening can be skipped with a left click）。起動直後にこの読み取り専用照会の応答が欠けた場合だけ再接続して再試行する。再起動後に MOD の更新時刻を再確認し、まだ新しければ手順を始めず失敗する。ログの開始位置も取り直すため、古いゲームのログを判定に混ぜない。

Civ6 がまったく起動していないときも、`restart_game`（自動再起動を含む）は「閉じる」手順を飛ばして Steam の起動経路でそのまま起動し、以後は通常の再起動と同じく（ウィンドウ待ち、オープニングのスキップ、FrontEnd 応答待ち）進む。最初の手順が `restart_game` 以外で `--no-auto-restart` のときも、Civ6 が無ければ同じ起動を先に行う。ドライバが起動していないプロセスは終了させず、待っても複数プロセスが残れば従来どおり拒否する。

直前のシナリオで閉じたばかりの Civ6 が終了処理中で一時的に 2 プロセスに見える場合、single_civ_process は最大 10 秒、消えるのを待ってから数える（他のプロセスは終了させない）。待っても複数残れば従来どおり失敗する。再起動後に FrontEnd が応答しない場合は、Civ6 が起動中に固まった可能性と FireTuner 設定の確認を促すエラーで失敗する。

ゲームプレイ Lua・SQL・テキストは新しいゲームの開始またはセーブの読込で再読込される。フロントエンド設定（ルールセット、プレイヤー、パラメータ）は MOD の再適用または再起動が必要。アート（BLP、DDS、ArtDefs、テクスチャ）は完全な再起動が必要。自動判定はファイル種別を問わず新しい更新時刻で再起動するため、これらを共通に扱う。

`tools/civ6-autotest-rs/tests` は、模擬ファイル更新時刻と模擬プロセス開始時刻による鮮度判定、再起動が最初の手順より前に行われること、`restart_game` の JSON 入力検証、Steam 起動・左クリック・`FrontEnd` 待機を単体テストする。

実行先ディレクトリは**新規**にする。省略時はマイクロ秒付き日時で作り、`baseline.json`、`steps.jsonl`、PNG が入る。`save` と `load` の名前には既定で出力ディレクトリの絶対パスから作った12桁の接尾辞を付ける。同じ実行の `save` / `load` は同じ実名を使い、再実行では新しい出力ディレクトリにより別名になる。既存名のセーブは上書きせず停止する。既存の保存を読む場合や名前を固定したい場合は両ステップで `"exact_name": true` を指定する。失敗時は `steps.jsonl` の失敗ステップと `Lua.log` を確認する。Tuner の応答が途絶えた場合、操作済みの可能性があるため自動再送しない。

途中で止まった実行のログだけを判定する場合:

```powershell
cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-judge -- --baseline tools/civ6-autotest/artifacts/bundle0-run-01/baseline.json --profile bundle0
```

手作業で開始位置を作る場合は、ゲーム操作の直前に `cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-judge -- --snapshot tools/civ6-autotest/artifacts/manual-baseline.json` を実行する。Civ6 のログ行にある角括弧内の数値は日時ではない。ドライバは各ログのバイト位置と先頭ハッシュを開始マーカーとして保存し、以降に書かれた部分だけを判定する。ログが再生成・切り詰めされた場合は新しいファイル全体を読む。

## JSON 手順

`scenarios/` に束0の保存・再読込、`clans-notebooks.json`（織田が人間・武田が AI）、`clans-both-ai.json`（公式の日本が人間・織田と武田が AI）、成長 UI と Notebooks、`japan-map-{20,15}km.json`（固定日本地図・22人・50ターン）、`japan-map-{20,15}km-takeda.json`（武田信虎を人間にした開始状態）がある。各ステップには `action` を置く。`timeout` は秒単位で省略可能で、ステップ内の問い合わせと操作は同じ期限を使う。`manual` はこの期限の対象外で、`timeout` を指定できず、Enter が押されるまで待つ。`growth` 判定ではシナリオ直下の `expected_units` に期待するユニット数を指定する。`growth-notebooks.json` は 2 体を割当・復元・手帳表示で確認する。

| action | 必要な値 | 動作 |
|---|---|---|
| `restart_game` | なし（`timeout` は任意、既定 360 秒） | Civ6 を閉じて Steam から再起動し、左クリックでオープニングを飛ばして `FrontEnd` の応答を待つ。実行後はログ判定の開始位置を取り直す |
| `enable_mods` | `ids`（MOD UUID 配列）、任意で `disable_ids` | メインメニューで指定した MOD を有効化する。無効化を指定した場合は、それに依存する有効な MOD を、間接的な依存も含めて先に無効化する。実際に無効化した ID はステップの結果に記録する |
| `configure` | `ruleset`, `map_size`, `speed`, `seed`, `players`。固定マップでは `map_file` も指定 | 既定のゲーム設定に戻し、ルールセット、地図ファイル・マップサイズ、速度、両シード、指導者・文明・人間/AI 枠を設定・照合する。`map_file` は `{MOD UUID}Maps/<ファイル>.Civ6Map` 形式。`build_setup` は HostGame で Create Game の設定をビルド・更新し、全指定枠が維持されたことを検査して設定を閉じる。`RULESET_JAC_BANSEIKI` では既定で true（明示的な `false` で無効）、他のルールセットでは既定 false。ビルドしないと暦の上書き（CALENDAR_STANDARD_FROM_START_ERA）が効かず、開始ターンが 16603 になって長い AI オートプレイが 31 ターン付近で止まった（2026-09-29 実機） |
| `start_game` | なし | シングルプレイを開始し、`InGame` の操作可能状態を待つ。直前の `configure` が `RULESET_JAC_BANSEIKI` なら `GameConfiguration.GetStartTurn()` が 1 でなければ失敗する |
| `autoplay` | `turns`、任意で `metrics: true`, `research_metrics: true`, `allow_human_elimination: true` | 指定ターンを自動進行し、`LuaEvents.AutoPlayEnd` の受信、実ターン数、停止、人間プレイヤーへの復帰を合わせて確認する。`metrics` はターン時間とプロセスメモリ、`research_metrics` は各メジャーの技術・社会制度の研究状態を JSONL に記録する。`allow_human_elimination` は人間勢力の滅亡をログに残し、目標ターンまで監視を続ける |
| `ui_lua` | `state_name`, `code` | 指定 Lua 状態で任意のコードを1回実行する |
| `screenshot` | `path` | Civ6 のウィンドウだけを PNG に保存する。出力先からの相対パス |
| `save` / `load` | `name`、任意で `exact_name: true` | 単独プレイのローカル保存。既定で出力先別の接尾辞を付ける。保存完了イベントとファイルを確認する |
| `exit_to_menu` | なし | メインメニューへ戻ったことを確認する |
| `wait_for` | `condition` | `{"type":"lua","state_name":"InGame","code":"return ..."}`、`{"type":"log_line","log":"Lua.log","contains":"..."}`、`{"type":"state","name":"InGame"}` のいずれかを待つ |
| `manual` | `prompt` | 人または Computer Use の操作を促し、期限なしで Enter まで待つ。`timeout` は指定不可 |

`enable_mods` は依存関係から無効化の順序を決め、同じ MOD を重ねて無効化しない。`ids` に含まれる MOD は無効化しない。その MOD が無効化対象に依存している場合や、対象の依存関係が循環している場合は、変更前にエラーにする。UUID の大文字・小文字の違いでこの検査を回避できない。無効化対象やその依存元に当たらない MOD は維持する。

`wait_for` の Lua 条件は繰り返し評価されるため、ゲーム状態を変更しない、副作用のない判定式に限定する。確認済みの Lua `ERR` は直ちに失敗とする。送信後に応答が失われて実行有無が不明な場合は、状態変更を伴う Lua コードを再送しない。`start_game` と `load` では、Lua コマンド送信前の接続・状態照会エラーをそのまま失敗として記録し、送信後の応答不明時だけ読み取り専用のログ・状態確認で遷移を判定する。確認できなければ判定不能として停止する。ロード遷移中に `Lua.log` の再生成・切り詰めを検出した場合も、世代をまたいだ件数比較をせず判定不能として停止する。スクリーンショットは撮影完了後にステップ期限を確認し、超過していれば失敗とする。

`growth-notebooks.json` は `PartialScreenHooks` の `OnToggleNotebooks()` で Notebooks を開き、`JAC_Notebooks` 状態の `Controls.CharacterJournalButton:DoLeftClick()` で Character Journal に切り替える。Lua 状態とログを確認してから撮影する。PNG の画面内容と見た目は人が確認する。ログの自動判定だけで視覚的な成功を主張しない。

### 今回追加した確認

- `japan-map-setup-screen.json`: 「ゲームの作成」で通常マップの人間候補が織田・武田の2人、日本15 kmマップを選んだ後の候補が所定の22人と完全一致することを検査し、それぞれ画面を保存する。
- `japan-map-15km-settlers.json`: 開拓者版の開始時に22メジャー勢力、各勢力ちょうど1人の開拓者、都市0、地図204×88を検査する。
- `japan-map-15km-200turn.json`: 史実領土版の初期22勢力・56都市を検査し、開始後に ruleset が `RULESET_JAC_BANSEIKI` で戦国ツリー（技術32・社会制度30、公式の行なし）が有効なことを検査してから、200ターンを進め、滅亡を許容しつつターン時間・メモリと全メジャーの研究状況を記録する。最後に目標ターン到達と生存勢力数を確認する。

研究ログは成果物の `autoplay-research-1.jsonl` に、**観測できた**各ターンの各メジャーについて、生死、研究済み技術・社会制度の件数とID、現在の研究IDを記録する。観測できなかったターンを補間しない。ターンごとの研究状況の問い合わせは読み取りだけなので、FireTuner の一時的なタイムアウトでは再試行する。途中で止まった自動進行を再開するときは、終了フラグ `JAC_AUTOTEST_AUTO_DONE` を消してから再開し、新しい終了イベントを待つ。到達ターンの中央値と勢力間の最小・最大・幅は次のコマンドで出す。`--kind civic` で社会制度を集計する。最初の観測時点で既に完了した項目は `baseline_factions` に数え、その正確な研究完了ターンは不明である。

```powershell
cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-research-report -- tools/civ6-autotest/artifacts/<実行先>/autoplay-research-1.jsonl
cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-research-report -- tools/civ6-autotest/artifacts/<実行先>/autoplay-research-1.jsonl --kind civic
```

実装の根拠、観測上の限界、主セッションで行う実機確認は [ドライバー拡張ノート](autotest-driver-extensions.md) に記す。

## 判定内容

`tools/civ6-autotest-rs/src/judge.rs` は `Lua.log` の `[JAC_POC]` エラーと、JAC のファイルが現れる `Runtime Error` のスタックを全シナリオで検出する。`bundle0` プロファイルは [PoC 記録](README.md) の成功条件に対応し、次も確認する。

- `Modding.log` に束0の MOD ID、`JAC_PoC_Bundle0_Gameplay (UpdateDatabase)`、`JAC_PoC_Bundle0_Scripts (AddGameplayScripts)` の適用がある。
- `Database.log` に JAC 関連のエラーがない。`CurrentClickouts` など由来の無関係な重複エラーは許容する。
- `Lua.log` に `loaded version=1`、新規ゲームの `no saved state (new game)`、進行に伴う `turnsSeen` の増加がある。
- 読込後の `restored firstTurn/lastTurn/turnsSeen` が保存前最後の行と一致し、次のターンでターン番号と `turnsSeen` が1増える。

クランの `generic` 判定はエラー不在を表し、シナリオ内の `wait_for` で所定のログを追加確認する。成長 UI の `growth` 判定は MOD の適用、成長ログ、保存・復元後の値と手帳表示などを検査する。画面の見た目はスクリーンショットで別途確認する。

`japan_map` 判定は MOD ID、Gameplay の適用、`Database.log`・`Modding.log` の本 MOD に関係するエラー不在を確認する。Config 適用ログは開始マーカーより前にある場合があるため、設定画面の候補表示と開始後のマップ寸法で別途確認する。地図シナリオは開始ユニットが各本拠の2ヘクス以内にいることも確認する。`metrics: true` の自動進行は成果物に `autoplay-metrics-1.jsonl`（50ターン）と `autoplay-metrics-2.jsonl`（ロード後1ターン）を作る。`duration_seconds` は観測した隣接ターン境界間の実時間で、Lua の問い合わせとポーリングの時間も含む。境界を飛ばしたターンは `observed: false`、`duration_seconds: null` として記録し、架空の値を補間しない。メモリは Windows の `GetProcessMemoryInfo` が返す作業セットとプライベート使用量のバイト数。

ロード画面の完了ログが出た後の Esc は1回だけ送る。以後は `InGame` の状態と操作可能な人間プレイヤーを期限まで問い合わせ、入れなければ追加のキー入力なしで失敗する。入力先は、ウィンドウ名と実行ファイル名（`CivilizationVI.exe` / `CivilizationVI_DX12.exe`）を照合した唯一の Civ6 HWND に限定する。2026-09-29 の実機ロード画面で、その HWND への `PostMessageW` による `WM_KEYDOWN/WM_KEYUP` 1組で `InGame` への移行を確認したため、この方式を使う。送信直前に HWND を再照合し、ほかのアプリが前面でも Civ6 の HWND にだけ送る。キー押下後の確認が失敗しても、解放メッセージを送る。

別の Civ6 ビルドでウィンドウ宛てメッセージが無視される場合に備え、`tools/civ6-autotest-rs/src/click.rs` には `SendInput` 方式も残す。切替前にロード画面で実機確認する。この方式は送信直前と直後に前景 HWND を照合し、変化すればシナリオを中止する。ただし照合と送信は OS 上で原子的ではなく、両者の間に前景が移るごく短い競合は残る。

既存の実機記録では、`clans-notebooks-run-18` は 17/17 ステップ・判定 2/2、`clans-both-ai-run-02` は 41/41 ステップ・判定 2/2、`growth-run-19` は 41/41 ステップ・判定 13/13、`clans-house-names-run-20` は 102/102 ステップ・判定 2/2 を通過した。2026-09-29 の先行修正では、オフライン単体テスト 53 件と実機 `clans-notebooks-run-21` の 17/17 ステップ・判定 2/2 が成功した。今回の修正後は単体テスト 68 件、実機 `clans-notebooks-run-23` の 17/17 ステップ・判定 2/2、`growth-run-24` の 41/41 ステップ・判定 13/13 が成功した。`AutoPlayEnd` がターン表示より先に届くことがあるため、自動進行の完了はイベント、実ターン数、停止、人間プレイヤーへの復帰を組み合わせて判定している。

2026-09-29 の自動再起動確認では、`JAC_PoC_CustomCivs.modinfo` の更新時刻を起動中の Civ6 より新しくしてから `scenarios/clans-notebooks.json` を実行した。`artifacts/auto-restart-live-20260929-091628/steps.jsonl` は MOD 更新時刻 `1790640988.192` と旧プロセス開始時刻 `1790640934.839` を理由に `auto_restart` の `START`・`OK` を**ステップ 1 より前**に記録した。その後 17/17 ステップとログ判定 2/2 件が成功した。4 枚の PNG、判定ログ、生成セーブを同じ成果物ディレクトリに保存し、実行前の同名セーブは元の場所へ復元した。オフライン単体テストは 84 件成功した。

## 根拠と注意

FireTuner の通信は 127.0.0.1:4318 のみ。4バイト little-endian 長さ、4バイト tag、NUL 終端 payload、`APP:`・`LSQ:`・`CMD:<stateIndex>:<lua>` の方式は [公開クライアントの実装](https://github.com/lmwilki/civ6-mcp/blob/main/src/civ_mcp/tuner_client.py)を参考に独立して実装した。Tuner は任意の Lua を実行できるため、信頼できるローカル JSON だけを使う。Gameplay Lua と UI Lua は別の状態なので、`ui_lua` には対象 UI 状態を明示する。

公式 Lua（Steam 版 1.0.12.68 のゲームインストール先、**参照のみ**）で確認した呼び出し:

| ファイルと行 | 実際の呼び出し |
|---|---|
| `Base/Assets/UI/FrontEnd/Mods.lua:28,67,391,904`、`LoadGameMenu.lua:58` | `Modding.GetModGroups()`、`Modding.GetInstalledMods()`、`Modding.GetModHandle(id)`、`Modding.EnableMod(handle, true)`、`GameConfiguration.UpdateEnabledMods()` |
| `Base/Assets/UI/FrontEnd/Mods.lua:255,296-327,545-569` | `Modding.GetModInfo(handle)`、`CanDisableMod(handle)`、`DisableMod(handle)`、`GetModAssociations(handle)`。無効化の依存元 ID と、依存・参照・競合の別配列を読む |
| `Base/Assets/UI/Automation/Automation_StandardTests.lua:175–207` | `GameConfiguration.SetRuleSet(ruleSet)`、`MapConfiguration.SetMapSize(mapSize)`、`GameConfiguration.SetGameSpeedType(gameSpeed)`、`MapConfiguration.SetValue("RANDOM_SEED", mapSeed)`、`GameConfiguration.SetValue("GAME_SYNC_RANDOM_SEED", gameSeed)` |
| 同 `:128–147,309,330–334`、`FrontEnd/PlayerSetupLogic.lua:80` | `PlayerConfigurations[id]:SetSlotStatus(...)`、`:SetLeaderTypeName(leaderType)`、`Network.HostGame(ServerType.SERVER_TYPE_NONE)`、`AutoplayManager.SetTurns(turnCount)` / `SetActive(true)` |
| 同 `:24–37,378–417`、`FrontEnd/LoadGameMenu.lua:523` | `Network.SaveGame({Name, Location=SaveLocations.LOCAL_STORAGE, Type=SaveTypes.SINGLE_PLAYER, IsAutosave=false, IsQuicksave=false})`、`Events.SaveComplete`、`Network.LoadGame(loadGame, ServerType.SERVER_TYPE_NONE)`、`:SetCivilizationTypeName(...)` |
| 同 `:272,384` | `Events.ExitToMainMenu()` |

スクリーンショットは Win32 `PrintWindow` で取り、黒画面なら `BitBlt` を試す。描画方法や遮蔽状態によっては取得できないので、ウィンドウを表示したまま実行する。ドライバはインストール先・AppData のファイルを直接編集しない。ゲーム内の公式 API による MOD 設定と保存は、実際にシナリオを走らせたときだけゲーム側で行われる。

既知の制約:

- 複数の Civ6 プロセスの同時起動は未対応。ドライバはローカルのゲームが1つであることを前提とし、非表示プロセスを含めて2つ以上見つけたら停止する。FireTuner の接続先 PID とウィンドウ PID の直接照合はしていない。
- 起動直後だけは Steam のランチャーとの一時的なプロセス重複を入力せず待つ。重複が解消せず、単一の Civ6 ウィンドウを確認できなければ期限で失敗する。実機試行中に DX12 実行ファイルが AMD の `amdxc64.dll` で一度クラッシュした（Windows Application Error 1000）。このクラッシュはドライバから防げず、その実行は手順開始前に停止した。次の実行で上記の成功結果を得た。
- `PrintWindow` と `BitBlt` は同期呼び出し。撮影期限は呼び出しから戻った後に確認するため、ゲームウィンドウが応答しない場合は期限を超えてもドライバが戻らない可能性がある。
- 再起動アクションをシナリオの途中に置くと、ログ判定の対象はその再起動後に発生したログだけになる。再起動前の確認は `steps.jsonl` に残る。

## 計測の成果物

`autoplay` の `metrics: true` は、JSONLに加えて `autoplay-metrics-N.csv` を作る。実行番号は研究計測と共通。CSVの列は次のとおり。

| 列 | 意味 |
|---|---|
| `phase`, `at`, `turn` | 開始／ターン境界、UTC記録時刻、到達したエンジンのターン番号 |
| `observed` | その境界を観測できたか |
| `duration_seconds` | 前の境界からこの境界までの実時間。最初だけ自動進行の要求直前から測る |
| `elapsed_seconds` | 自動進行の要求からの経過時間 |
| `working_set_bytes`, `private_bytes` | Civ6の作業セットとプライベート使用量（バイト） |
| `test_mode`, `continuous_performance_valid` | 早送りを使ったか、通常進行の性能比較に使える実行か。早送りでは全行に True／False を残す |

初期メモリは自動進行の前に採る。研究計測を併用した場合は、研究の読み取り時間もターン時間に含む。新しい20／300ターン計測は併用しない。以後は境界を観測するたびに採り、途中でPIDが変わったら失敗する。時間にはFireTunerの通信、ポーリング、画面を閉じる時間も含む。エンジン内部だけの処理時間ではない。飛ばしたターンの行は observed が false、時間とメモリが空欄になる。到達した境界のメモリは採るが、複数ターン分の時間は空欄にする。補間しない。

`save` と `load` にも `metrics: true` を指定できる。保存・ロードのCSVにも `test_mode` と `continuous_performance_valid` の列を持たせ、早送りの結果をCSV単体でも区別する。`save-load-metrics.csv` に、時刻・操作・実際のセーブ名・ターン・ファイルのバイト数・秒数を残す。保存は要求直前から、保存完了イベントと非空のファイルを確認するまで。ロードは要求直前から、完了ログ、開始操作、人間プレイヤー、操作を妨げる画面の閉鎖を確認するまで。純粋なディスク読込時間ではない。失敗を0秒の測定値にはしない。実行別のセーブ名と上書き拒否は従来どおり。

`result.json` は、成功／失敗、完了ステップ数、失敗理由、ログ判定を持つ。`metrics.autoplay` に各区間のPID・要求数・到達ターン・完了の有無・CSV名を残す。`metrics.turn_summary` に観測数／欠測数、時間の件数・最小・中央値・95パーセンタイル・最大、メモリの初回／最終／増加量を残す。95パーセンタイルは昇順の ceil(件数×0.95) 番目。時間が測れた件数は観測した境界数より少ないことがある。測定値がない集計は null。`metrics.save_load` は各チェックポイント、保存容量の増加、ロード時間の集計。途中で失敗しても採取済みのCSVと集計を残す。

300ターン版は、最初の6区間で連続300ターン、最後の1区間でロード後の次ターンを確認する。連続300ターンの集計は metrics.groups.CONTINUOUS、ロード後は metrics.groups.POST_LOAD に分ける。各区間の metrics_group で指定し、省略すると AUTOPLAY にまとめる。全体の集計にはロード後の1ターンも含む。保存7回／ロード2回を測る。20ターン版は保存2回／ロード2回。

Rust から Windows のメモリ API を呼ぶ。列は [Microsoftの構造体の説明](https://learn.microsoft.com/en-us/windows/win32/api/psapi/ns-psapi-process_memory_counters_ex) の WorkingSetSize と PrivateUsage に対応する。

## 今回追加した実機シナリオ

全ファイルは `tools/civ6-autotest/scenarios/` にある。20ターン版、追加時代の短縮試験、300ターン版の順に実行する。

長い待ちを省いて手順を検証する場合は `--test-mode` を付ける。3ターン以上の自動進行は、画面側の時代APIから最短の終期とカウントダウンを読み、時代が変わりうる境界の2ターン前までだけ進める。境界は通常進行させ、次の時代を確認してから再び早送りする。短縮した追加時代のように2ターンで切り替わる区間は省かない。ターン番号を変えるのは GameCore の `Game.SetCurrentGameTurn`。省いたターンでは研究・成長・戦争などの処理を実行しない。ゲーム本体の `GameCore_XP2_FinalRelease.dll` にこの関数があり、公式の `DLC/PolandScenario/Scripts/PolandScenario.lua:63` にも呼び出しの記載がある（コメント内）。実行時に関数と到達ターンを確認し、利用できなければ失敗する。

このモードは万世紀ルールセットの単独プレイに限定し、画面側で単独プレイを確認し、マルチプレイではターンを変更する前に失敗する（公式判定の例：`DLC/Expansion2/UI/Replacements/InGame.lua:115`）。このモードは時代の境界や保存・復元の手順を検証するためのもの。連続300ターンの性能やメモリ増加、長期プレイの安定性を検証したことにはならない。`result.json` の `test_mode` に実際に省いたターン数と `continuous_performance_valid: false` を残し、計測の区分にも `TEST_MODE_` を付ける。通常進行の計測シナリオは変更しない。

| シナリオ | 確認すること |
|---|---|
| `japan-15km-20turn-full-metrics.json` | 22勢力・全7 MODで20ターン。CSV、保存容量、開始時／終了時のロードと保存状態の一致、次ターン |
| `japan-15km-300turn-full-metrics.json` | 同構成で連続300ターン。50ターンごとの保存容量、ターン時間、メモリ、開始時／終了時のロード |
| `sengoku-era-reigns-epic.json` | 弘治98・永禄108・元亀163で、エンジンの時代と台帳が変わる |
| `sengoku-era-reigns-marathon.json` | 弘治196・永禄217・元亀328で同じ検査 |
| `sengoku-era-reigns-beyond-nine.json` | 任意の短縮データで正保19・慶安21。台帳と改元画面が一致し、公式ポップアップが出ない |
| `great-people-zero-musicians.json` | 音楽家0人／偉人75人で画面を開き、現在・履歴・現在を描き直し、画像を残して閉じる |
| `resources-calendar-epic.json` | 資源の実際の年計算が、叙事詩の1／25／41ターンで1534／1540／1543をログに出す。UIの暦とも一致する |

`cargo run --locked --manifest-path tools/Cargo.toml --bin civ6-autotest -- tools/civ6-autotest/scenarios/<ファイル名> --check` で静的検査し、実機では末尾の --check を外す。保存先を指定するなら --output に新しいディレクトリを渡す。

追加7本は `strict_lua_errors: true` を指定し、実行中の全ての Runtime Error・UI.DataError・公式時代画面の欠損画像エラーを失敗にする。JACの名前がない公式の偉人画面のエラーも検出する。この指定がない従来シナリオの判定は維持する。

追加時代の試験は既存の `JAC_Test_EraTiming` のジャンクションが必要。非表示の短縮モードをこのシナリオだけがtrueにする。標準／迅速の通常プローブはfalseを明示する。適用を開始直後に検査し、適用されていなければ失敗する。対象は万世紀と天下統一の検証地図だけ。終了後はメニューでfalseに戻す。

保存前後の比較にはユニットのID・所有者・種類・座標・被ダメージと武将の成長プロパティを含める。読み込みで軍勢が消えたり移動したりした場合も失敗する。束0の保存状態と成長UIの初期化プロパティも比較する。

全タイルの所有者は同じ値が連続する個数で記録し、情報を保ったまま短くする。セーブ前後の一致を確認する。長い試験は人間の滅亡や勝利の確定で終わる可能性がある。途中終了は到達成功にせず失敗を残す。速度の試験は3勢力の地図でこの点も要実機確認。

**実機確認済み**（2026-10-01）。偉人画面は `perf-08`、迅速／標準のプローブは `perf-10`／`perf-11`、叙事詩／マラソンは `perf-14`／`perf-15`、追加時代は `perf-16`、資源の暦は `perf-17`、20／300ターン版は `perf-18`／`perf-19` に成功した。長い待ちは `--test-mode` を使用し、時代の境界は通常進行した。300ターン版の通常進行は28ターンと復元後1ターンで、272ターンを省いた。通常進行300ターンの性能・長期安定性の許容範囲は未検証。

保存状態の読み取りに使う公式APIの根拠（ゲームの先頭パスは下の「根拠と注意」と同じ）：

| 前提 | 公式ファイル:行 |
|---|---|
| 保存完了イベントとロード要求 | `Base/Assets/UI/Automation/Automation_StandardTests.lua:16-37,298` |
| 都市の列挙と人口 | `Base/Assets/UI/Screens/ReportScreen.lua:227`、`Base/Assets/UI/CitySupport.lua:427` |
| 都市座標 | `Base/Assets/UI/WorldView/CityBannerManager.lua:207-208` |
| 地図の列挙 | `Base/Assets/UI/Popups/MapSearchPanel.lua:700` |
| タイルの所有者 | `Base/Assets/UI/WorldBuilderPlacement.lua:683` |
| 研究完了の照会 | `Base/Assets/UI/Choosers/ResearchChooser.lua:53,66`、`Base/Assets/UI/Choosers/CivicsChooser.lua:50,63` |
| ターンと生死 | `Base/Assets/UI/TopPanel.lua:387`、`Base/Assets/UI/ARXManager.lua:40` |
| Gameプロパティの保存 | 本PoC READMEの束0／束1の実機結果。テーブルの復元はシングルプレイで確認済み |

早送り直後はゲーム処理側の番号を使って自動進行の停止位置を計算する。画面側は直前の番号を返すことがあるため、開始番号より古い観測は採取しない。`perf-13` で古い番号を使った停止位置の誤りを確認し、修正後の `perf-14`〜`perf-19` が成功した。

シナリオ全体はCLIだけでなく `Runner.run()` の入口でも、出力先の作成・ゲームへの接続・設定より前に検査する。早送りの応答を送信後に失った場合は再送しない。適用されたターン数を確定できないため、`skipped_turns` を null にし、応答を確認できた数を `acknowledged_skipped_turns`、応答が不明の区間を `unconfirmed_jump` に残す。
