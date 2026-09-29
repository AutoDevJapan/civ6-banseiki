# Civ6 PoC 検証ドライバ

`tools/civ6-autotest/` は Steam 版 Civ6 1.0.12.68（Gathering Storm、シングルプレイ）に FireTuner で接続し、JSON の手順を順に実行する。シナリオで有効化する MOD の更新が起動中のゲームより新しい場合や、ゲームが停止している場合は、手順の開始前に Civ6 を再起動する。各ステップの成否と出力、画面 PNG、ログ判定用の開始位置を新しい成果物ディレクトリへ保存する。

## 準備と実行

1. 対象 MOD を Civ6 の `Mods` フォルダから参照できる状態にする。ドライバは `.modinfo` の UUID で対象を探し、ジャンクションの参照先を走査する。`bundle0-load-save.json` は `JAC_PoC_Bundle0`、クランと手帳のシナリオは `JAC_PoC_CustomCivs` と `JAC_PoC_GrowthUI` を使う。`banseiki-ruleset.json` はこの3 MOD と `JAC_PoC_BanseikiRuleset` を使う。実際に Civ6 が参照する `Mods` の配置先を確認する。
2. Civ6 は**ウィンドウ表示**で起動できる設定にする。停止中ならドライバが Steam 経由で起動する。`AppOptions.txt` の `EnableTuner 1` が必要。FireTuner GUI は閉じる。同じ Tuner ポートに同時接続しない。
3. PowerShell でリポジトリのルートから実行する。Python 3.12 で確認済み。追加パッケージは不要。

```powershell
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/bundle0-load-save.json --check
# 既にゲームが起動している場合の診断
python tools/civ6-autotest/tuner.py states
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/bundle0-load-save.json --output tools/civ6-autotest/artifacts/bundle0-run-01
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/banseiki-ruleset.json --output tools/civ6-autotest/artifacts/banseiki-run-01
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/clans-notebooks.json --no-auto-restart --output tools/civ6-autotest/artifacts/debug-01
python -m unittest discover -s tools/civ6-autotest/tests -v
```

オーナーが PC の前にいるとき（確認モード）は `--owner-start` を付ける。`start_game` で自動開始せず、設定済みの「ゲームの作成」画面を開いて、オーナーが「ゲーム開始」を押すまで最大1時間待つ。以降の手順は自動で進む。

`states` は既にゲームが起動している場合の診断に使う。`FrontEnd` と `InGame` が異なる名前で現れた場合は、実際の状態名を JSON と `scenario.py` の固定ステップに反映してから実行する。ログや保存先が標準の場所と違う場合は `--logs <Logs ディレクトリ>` と `--saves <Saves/Single ディレクトリ>`、MOD の場所が違う場合は `--mods <Mods ディレクトリ>` を指定する。

### 自動鮮度確認と再起動

実行開始時、全 `enable_mods.ids` に対応する `.modinfo` を `Mods` 直下の各 MOD フォルダで探し、ジャンクションを解決した参照先の全ファイルから最も新しい更新時刻を取得する。起動中の `CivilizationVI.exe` または `CivilizationVI_DX12.exe` の開始時刻よりその時刻が**新しい**場合、またはゲームが起動していない場合は、`steps.jsonl` の `auto_restart` に理由を記録して再起動する。MOD が見つからない、同じ UUID が複数ある、Civ6 プロセスが複数ある場合は失敗として止める。更新時刻が等しい場合は再起動しない。`--no-auto-restart` はデバッグ時のみ鮮度確認を省略する。

再起動はまず FireTuner の `FrontEnd` / `InGame` UI 状態から `Events.UserConfirmedClose()` を呼ぶ。接続できなければ、実行ファイル名とタイトルを照合した Civ6 ウィンドウに閉じる要求を送る。15 秒待っても元のプロセスが残る場合は PID と開始時刻を再確認してから終了させる。旧 EXE の終了後も Steam が旧セッションを解放するまで 8 秒待ち、`steam://rungameid/289070` で再起動する。新しいゲームが現れなければ 45 秒間隔で最大 4 回起動を試す。確認済みの Civ6 ウィンドウを左クリックしてオープニングを飛ばす。動画が続いて `FrontEnd` が応答しなければ、安全な位置を 12 秒間隔で再度左クリックし、`UI.IsInFrontEnd()` が `true` を返すまで待つ（the opening can be skipped with a left click）。起動直後にこの読み取り専用照会の応答が欠けた場合だけ再接続して再試行する。再起動後に MOD の更新時刻を再確認し、まだ新しければ手順を始めず失敗する。ログの開始位置も取り直すため、古いゲームのログを判定に混ぜない。

ゲームプレイ Lua・SQL・テキストは新しいゲームの開始またはセーブの読込で再読込される。フロントエンド設定（ルールセット、プレイヤー、パラメータ）は MOD の再適用または再起動が必要。アート（BLP、DDS、ArtDefs、テクスチャ）は完全な再起動が必要。自動判定はファイル種別を問わず新しい更新時刻で再起動するため、これらを共通に扱う。

`tests/test_restart.py` は、模擬ファイル更新時刻と模擬プロセス開始時刻による鮮度判定、再起動が最初の手順より前に行われること、`restart_game` の JSON 入力検証、Steam 起動・左クリック・`FrontEnd` 待機を単体テストする。

実行先ディレクトリは**新規**にする。`baseline.json`、`steps.jsonl`、PNG が入る。既存名のセーブは上書きせず停止するため、再実行時は JSON の `save` と `load` の名前を両方変える。失敗時は `steps.jsonl` の失敗ステップと `Lua.log` を確認する。Tuner の応答が途絶えた場合、操作済みの可能性があるため自動再送しない。

途中で止まった実行のログだけを判定する場合:

```powershell
python tools/civ6-autotest/judge.py --baseline tools/civ6-autotest/artifacts/bundle0-run-01/baseline.json --profile bundle0
```

手作業で開始位置を作る場合は、ゲーム操作の直前に `python tools/civ6-autotest/judge.py --snapshot tools/civ6-autotest/artifacts/manual-baseline.json` を実行する。Civ6 のログ行にある角括弧内の数値は日時ではない。ドライバは各ログのバイト位置と先頭ハッシュを開始マーカーとして保存し、以降に書かれた部分だけを判定する。ログが再生成・切り詰めされた場合は新しいファイル全体を読む。

## JSON 手順

`scenarios/` に束0の保存・再読込、`clans-notebooks.json`（織田が人間・武田が AI）、`clans-both-ai.json`（公式の日本が人間・織田と武田が AI）、成長 UI と Notebooks、`japan-map-{20,15}km.json`（固定日本地図・22人・50ターン）、`japan-map-{20,15}km-takeda.json`（武田信虎を人間にした開始状態）がある。各ステップには `action` を置く。`timeout` は秒単位で省略可能で、ステップ内の問い合わせと操作は同じ期限を使う。`manual` はこの期限の対象外で、`timeout` を指定できず、Enter が押されるまで待つ。`growth` 判定ではシナリオ直下の `expected_units` に期待するユニット数を指定する。`growth-notebooks.json` は 2 体を割当・復元・手帳表示で確認する。

| action | 必要な値 | 動作 |
|---|---|---|
| `restart_game` | なし（`timeout` は任意、既定 360 秒） | Civ6 を閉じて Steam から再起動し、左クリックでオープニングを飛ばして `FrontEnd` の応答を待つ。実行後はログ判定の開始位置を取り直す |
| `enable_mods` | `ids`（MOD UUID 配列） | メインメニューで存在・有効化可否を確認し、有効化する。ほかの有効な MOD は変更しない |
| `configure` | `ruleset`, `map_size`, `speed`, `seed`, `players`。固定マップでは `map_file` も指定 | 既定のゲーム設定に戻し、ルールセット、地図ファイル・マップサイズ、速度、両シード、指導者・文明・人間/AI 枠を設定・照合する。`map_file` は `{MOD UUID}Maps/<ファイル>.Civ6Map` 形式 |
| `start_game` | なし | シングルプレイを開始し、`InGame` の操作可能状態を待つ |
| `autoplay` | `turns`、任意で `metrics: true` | 指定ターンを自動進行し、`LuaEvents.AutoPlayEnd` の受信、実ターン数、停止、人間プレイヤーへの復帰を合わせて確認する。`metrics` が真ならターン境界ごとの所要時間と Civ6 プロセスのメモリを記録する |
| `ui_lua` | `state_name`, `code` | 指定 Lua 状態で任意のコードを1回実行する |
| `screenshot` | `path` | Civ6 のウィンドウだけを PNG に保存する。出力先からの相対パス |
| `save` / `load` | `name` | 単独プレイのローカル保存。保存完了イベントとファイルを確認する |
| `exit_to_menu` | なし | メインメニューへ戻ったことを確認する |
| `wait_for` | `condition` | `{"type":"lua","state_name":"InGame","code":"return ..."}`、`{"type":"log_line","log":"Lua.log","contains":"..."}`、`{"type":"state","name":"InGame"}` のいずれかを待つ |
| `manual` | `prompt` | 人または Computer Use の操作を促し、期限なしで Enter まで待つ。`timeout` は指定不可 |

`wait_for` の Lua 条件は繰り返し評価されるため、ゲーム状態を変更しない、副作用のない判定式に限定する。確認済みの Lua `ERR` は直ちに失敗とする。送信後に応答が失われて実行有無が不明な場合は、状態変更を伴う Lua コードを再送しない。`start_game` と `load` では、Lua コマンド送信前の接続・状態照会エラーをそのまま失敗として記録し、送信後の応答不明時だけ読み取り専用のログ・状態確認で遷移を判定する。確認できなければ判定不能として停止する。ロード遷移中に `Lua.log` の再生成・切り詰めを検出した場合も、世代をまたいだ件数比較をせず判定不能として停止する。スクリーンショットは撮影完了後にステップ期限を確認し、超過していれば失敗とする。

`growth-notebooks.json` は `PartialScreenHooks` の `OnToggleNotebooks()` で Notebooks を開き、`JAC_Notebooks` 状態の `Controls.CharacterJournalButton:DoLeftClick()` で Character Journal に切り替える。Lua 状態とログを確認してから撮影する。PNG の画面内容と見た目は人が確認する。ログの自動判定だけで視覚的な成功を主張しない。

## 判定内容

`judge.py` は `Lua.log` の `[JAC_POC]` エラーと、JAC のファイルが現れる `Runtime Error` のスタックを全シナリオで検出する。`bundle0` プロファイルは [PoC 記録](README.md) の成功条件に対応し、次も確認する。

- `Modding.log` に束0の MOD ID、`JAC_PoC_Bundle0_Gameplay (UpdateDatabase)`、`JAC_PoC_Bundle0_Scripts (AddGameplayScripts)` の適用がある。
- `Database.log` に JAC 関連のエラーがない。`CurrentClickouts` など由来の無関係な重複エラーは許容する。
- `Lua.log` に `loaded version=1`、新規ゲームの `no saved state (new game)`、進行に伴う `turnsSeen` の増加がある。
- 読込後の `restored firstTurn/lastTurn/turnsSeen` が保存前最後の行と一致し、次のターンでターン番号と `turnsSeen` が1増える。

クランの `generic` 判定はエラー不在を表し、シナリオ内の `wait_for` で所定のログを追加確認する。成長 UI の `growth` 判定は MOD の適用、成長ログ、保存・復元後の値と手帳表示などを検査する。画面の見た目はスクリーンショットで別途確認する。

`japan_map` 判定は MOD ID、Gameplay の適用、`Database.log`・`Modding.log` の本 MOD に関係するエラー不在を確認する。Config 適用ログは開始マーカーより前にある場合があるため、設定画面の候補表示と開始後のマップ寸法で別途確認する。地図シナリオは開始ユニットが各本拠の2ヘクス以内にいることも確認する。`metrics: true` の自動進行は成果物に `autoplay-metrics-1.jsonl`（50ターン）と `autoplay-metrics-2.jsonl`（ロード後1ターン）を作る。`duration_seconds` は観測した隣接ターン境界間の実時間で、Lua の問い合わせとポーリングの時間も含む。境界を飛ばしたターンは `observed: false`、`duration_seconds: null` として記録し、架空の値を補間しない。メモリは Windows の `GetProcessMemoryInfo` が返す作業セットとプライベート使用量のバイト数。

ロード画面の完了ログが出た後の Esc は1回だけ送る。以後は `InGame` の状態と操作可能な人間プレイヤーを期限まで問い合わせ、入れなければ追加のキー入力なしで失敗する。入力先は、ウィンドウ名と実行ファイル名（`CivilizationVI.exe` / `CivilizationVI_DX12.exe`）を照合した唯一の Civ6 HWND に限定する。2026-09-29 の実機ロード画面で、その HWND への `PostMessageW` による `WM_KEYDOWN/WM_KEYUP` 1組で `InGame` への移行を確認したため、この方式を使う。送信直前に HWND を再照合し、ほかのアプリが前面でも Civ6 の HWND にだけ送る。キー押下後の確認が失敗しても、解放メッセージを送る。

別の Civ6 ビルドでウィンドウ宛てメッセージが無視される場合に備え、`click.py` には `SendInput` 方式も残す。切替前にロード画面で実機確認する。この方式は送信直前と直後に前景 HWND を照合し、変化すればシナリオを中止する。ただし照合と送信は OS 上で原子的ではなく、両者の間に前景が移るごく短い競合は残る。

既存の実機記録では、`clans-notebooks-run-18` は 17/17 ステップ・判定 2/2、`clans-both-ai-run-02` は 41/41 ステップ・判定 2/2、`growth-run-19` は 41/41 ステップ・判定 13/13、`clans-house-names-run-20` は 102/102 ステップ・判定 2/2 を通過した。2026-09-29 の先行修正では、オフライン単体テスト 53 件と実機 `clans-notebooks-run-21` の 17/17 ステップ・判定 2/2 が成功した。今回の修正後は単体テスト 68 件、実機 `clans-notebooks-run-23` の 17/17 ステップ・判定 2/2、`growth-run-24` の 41/41 ステップ・判定 13/13 が成功した。`AutoPlayEnd` がターン表示より先に届くことがあるため、自動進行の完了はイベント、実ターン数、停止、人間プレイヤーへの復帰を組み合わせて判定している。

2026-09-29 の自動再起動確認では、`JAC_PoC_CustomCivs.modinfo` の更新時刻を起動中の Civ6 より新しくしてから `scenarios/clans-notebooks.json` を実行した。`artifacts/auto-restart-live-20260929-091628/steps.jsonl` は MOD 更新時刻 `1790640988.192` と旧プロセス開始時刻 `1790640934.839` を理由に `auto_restart` の `START`・`OK` を**ステップ 1 より前**に記録した。その後 17/17 ステップとログ判定 2/2 件が成功した。4 枚の PNG、判定ログ、生成セーブを同じ成果物ディレクトリに保存し、実行前の同名セーブは元の場所へ復元した。オフライン単体テストは 84 件成功した。

## 根拠と注意

FireTuner の通信は 127.0.0.1:4318 のみ。4バイト little-endian 長さ、4バイト tag、NUL 終端 payload、`APP:`・`LSQ:`・`CMD:<stateIndex>:<lua>` の方式は [公開クライアントの実装](https://github.com/lmwilki/civ6-mcp/blob/main/src/civ_mcp/tuner_client.py)を参考に独立して実装した。Tuner は任意の Lua を実行できるため、信頼できるローカル JSON だけを使う。Gameplay Lua と UI Lua は別の状態なので、`ui_lua` には対象 UI 状態を明示する。

公式 Lua（Steam 版 1.0.12.68 のゲームインストール先、**参照のみ**）で確認した呼び出し:

| ファイルと行 | 実際の呼び出し |
|---|---|
| `Base/Assets/UI/FrontEnd/Mods.lua:28,67,391,904`、`LoadGameMenu.lua:58` | `Modding.GetModGroups()`、`Modding.GetInstalledMods()`、`Modding.GetModHandle(id)`、`Modding.EnableMod(handle, true)`、`GameConfiguration.UpdateEnabledMods()` |
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
