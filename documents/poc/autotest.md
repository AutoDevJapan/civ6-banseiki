# Civ6 PoC 検証ドライバ

`tools/civ6-autotest/` は、起動済みの Steam 版 Civ6 1.0.12.68（Gathering Storm、シングルプレイ）に FireTuner で接続し、JSON の手順を順に実行する。ゲームの起動は行わない。各ステップの成否と出力、画面 PNG、ログ判定用の開始位置を新しい成果物ディレクトリへ保存する。

## 準備と実行

1. 対象 MOD を Civ6 の MOD フォルダから参照できる状態にする。`bundle0-load-save.json` はこのリポジトリの `poc/JAC_PoC_Bundle0` を使う。`custom-clans.json` と `growth-notebooks.json` の ID と Lua イベント名は、この PC の別 PoC ワークツリーにある `JAC_PoC_CustomCivs`、`JAC_PoC_GrowthUI` から読み取り確認した。これらは現時点でこのワークツリーの `poc/` にはないため、実行前に配置とバージョンを確認する。
2. オーナーまたは Computer Use で Civ6 を**ウィンドウ表示**で起動し、メインメニューまで進める。`AppOptions.txt` の `EnableTuner 1` が必要。FireTuner GUI は閉じる。同じ Tuner ポートに同時接続しない。
3. PowerShell でリポジトリのルートから実行する。Python 3.12 で確認済み。追加パッケージは不要。

```powershell
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/bundle0-load-save.json --check
python tools/civ6-autotest/tuner.py states
python tools/civ6-autotest/scenario.py tools/civ6-autotest/scenarios/bundle0-load-save.json --output tools/civ6-autotest/artifacts/bundle0-run-01
python -m unittest discover -s tools/civ6-autotest/tests -v
```

`states` で `FrontEnd` と `InGame` が現れるか先に確認する。異なる名前が出た場合は、実際の状態名を JSON と `scenario.py` の固定ステップに反映してから実行する。ログや保存先が標準の場所と違う場合は `--logs <Logs ディレクトリ>` と `--saves <Saves/Single ディレクトリ>` を指定する。

実行先ディレクトリは**新規**にする。`baseline.json`、`steps.jsonl`、PNG が入る。既存名のセーブは上書きせず停止するため、再実行時は JSON の `save` と `load` の名前を両方変える。失敗時は `steps.jsonl` の失敗ステップと `Lua.log` を確認する。Tuner の応答が途絶えた場合、操作済みの可能性があるため自動再送しない。

途中で止まった実行のログだけを判定する場合:

```powershell
python tools/civ6-autotest/judge.py --baseline tools/civ6-autotest/artifacts/bundle0-run-01/baseline.json --profile bundle0
```

手作業で開始位置を作る場合は、ゲーム操作の直前に `python tools/civ6-autotest/judge.py --snapshot tools/civ6-autotest/artifacts/manual-baseline.json` を実行する。Civ6 のログ行にある角括弧内の数値は日時ではない。ドライバは各ログのバイト位置と先頭ハッシュを開始マーカーとして保存し、以降に書かれた部分だけを判定する。ログが再生成・切り詰めされた場合は新しいファイル全体を読む。

## JSON 手順

`scenarios/` に束0の保存・再読込、織田（人間）と武田（AI）、成長 UI と Notebooks の例がある。各ステップには `action` を置く。`timeout` は秒単位で省略可能。

| action | 必要な値 | 動作 |
|---|---|---|
| `enable_mods` | `ids`（MOD UUID 配列） | メインメニューで存在・有効化可否を確認し、有効化する。ほかの有効な MOD は変更しない |
| `configure` | `ruleset`, `map_size`, `speed`, `seed`, `players` | 既定のゲーム設定に戻し、ルールセット、マップ、速度、両シード、指導者・文明・人間/AI 枠を設定・照合する |
| `start_game` | なし | シングルプレイを開始し、`InGame` の操作可能状態を待つ |
| `autoplay` | `turns` | 指定ターンを自動進行し、`LuaEvents.AutoPlayEnd` の受信、実ターン数、停止、人間プレイヤーへの復帰を合わせて確認する |
| `ui_lua` | `state_name`, `code` | 指定 Lua 状態で任意のコードを1回実行する |
| `screenshot` | `path` | Civ6 のウィンドウだけを PNG に保存する。出力先からの相対パス |
| `save` / `load` | `name` | 単独プレイのローカル保存。保存完了イベントとファイルを確認する |
| `exit_to_menu` | なし | メインメニューへ戻ったことを確認する |
| `wait_for` | `condition` | `{"type":"lua","state_name":"InGame","code":"return ..."}`、`{"type":"log_line","log":"Lua.log","contains":"..."}`、`{"type":"state","name":"InGame"}` のいずれかを待つ |
| `manual` | `prompt` | 人または Computer Use の操作を促し、Enter 後に続行する |

`growth-notebooks.json` は `PartialScreenHooks` の `OnToggleNotebooks()` で Notebooks を開き、`JAC_Notebooks` 状態の `Controls.CharacterJournalButton:DoLeftClick()` で Character Journal に切り替える。Lua 状態とログを確認してから撮影する。PNG の画面内容と見た目は人が確認する。ログの自動判定だけで視覚的な成功を主張しない。

## 判定内容

`judge.py` は `Lua.log` の `[JAC_POC]` エラーと、JAC のファイルが現れる `Runtime Error` のスタックを全シナリオで検出する。`bundle0` プロファイルは [PoC 記録](README.md) の成功条件に対応し、次も確認する。

- `Modding.log` に束0の MOD ID、`JAC_PoC_Bundle0_Gameplay (UpdateDatabase)`、`JAC_PoC_Bundle0_Scripts (AddGameplayScripts)` の適用がある。
- `Database.log` に JAC 関連のエラーがない。`CurrentClickouts` など由来の無関係な重複エラーは許容する。
- `Lua.log` に `loaded version=1`、新規ゲームの `no saved state (new game)`、進行に伴う `turnsSeen` の増加がある。
- 読込後の `restored firstTurn/lastTurn/turnsSeen` が保存前最後の行と一致し、次のターンでターン番号と `turnsSeen` が1増える。

クランの `generic` 判定はエラー不在を表し、シナリオ内の `wait_for` で所定のログを追加確認する。成長 UI の `growth` 判定は MOD の適用、成長ログ、保存・復元後の値と手帳表示などを検査する。画面の見た目はスクリーンショットで別途確認する。

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
