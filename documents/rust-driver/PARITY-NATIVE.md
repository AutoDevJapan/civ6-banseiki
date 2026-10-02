# Native driver parity (担当範囲)

実行基準: `cargo test --offline --manifest-path tools/civ6-autotest-rs/Cargo.toml --target-dir tools/target`。各行の Rust テストは `cargo test ... --test <file> <name>` で個別実行できる。旧 Python テストは実行していない。

## `test_restart.py` — CrashReporterTests

| 旧 `file::test` | Rust `--test file name` | 維持する主な検証 |
| --- | --- | --- |
| `test_restart.py::test_only_civ6_reporter_is_closed_without_a_running_game` | `restart_parity only_civ6_reporter_is_closed_without_running_game` | Civ6 の報告プロセスだけを閉じる。大文字のパスも扱い、別ゲームと起動中の Civ6 は保持する。 |
| `test_restart.py::test_running_game_keeps_reporters` | `restart_parity running_game_keeps_reporters` | ゲーム起動中は報告プロセスへ触れない。 |
| `test_restart.py::test_unreadable_reporter_is_not_terminated` | `restart_parity unreadable_reporter_is_not_terminated` | 実行ファイルの確認が失敗したら終了させない。 |
| `test_restart.py::test_exited_reporter_is_ignored` | `restart_parity exited_reporter_is_ignored` | 列挙後に終了した報告プロセスは無視する。 |
| `test_restart.py::test_remaining_reporter_that_cannot_be_opened_stops_launch` | `restart_parity remaining_reporter_that_cannot_open_stops_launch` | 残存プロセスを開けなければ Steam 起動を止める。 |
| `test_restart.py::test_reporter_must_finish_exiting_before_launch` | `restart_parity reporter_must_finish_exiting_before_launch` | 待機結果の成功・時間切れ・失敗を区別し、終了待ち失敗時は起動しない。 |
| `test_restart.py::test_steam_launch_clears_reporters_before_sending_uri` | `restart_parity steam_launch_clears_reporters_before_sending_dx11_uri` | 報告プロセス処理が先、Steam の `option2` URI が後。 |
| `test_restart.py::test_close_failure_stops_steam_launch` | `restart_parity close_failure_stops_steam_launch` | 報告プロセス終了失敗時は URI を送らない。 |

## `test_restart.py` — DX11LaunchTests / FreshnessTests

| 旧 `file::test` | Rust `--test file name` | 維持する主な検証 |
| --- | --- | --- |
| `test_restart.py::test_only_dx11_window_is_accepted_after_launch` | `restart_parity only_dx11_window_is_accepted_after_launch` | `CivilizationVI.exe` のみ受け入れ、DX12 と他の実行ファイルを拒否する。 |
| `test_restart.py::test_unreadable_window_is_not_accepted` | `restart_parity unreadable_window_is_not_accepted` | 実行ファイル確認の失敗を伝える。 |
| `test_restart.py::test_freshness_decision_uses_strictly_newer_mod_file` | `restart_parity freshness_decision_uses_strictly_newer_mod_file` | 未起動、更新時刻が前・同時・後、MOD ファイル無しを判別する。 |
| `test_restart.py::test_mod_scan_resolves_folder_and_finds_newest_file` | `restart_parity mod_scan_resolves_folder_and_finds_newest_file` | UUID 大文字小文字、フォルダ解決、最新ファイルの時刻と再起動理由。 |
| `test_restart.py::test_missing_or_duplicate_enabled_mod_is_rejected` | `restart_parity missing_or_duplicate_enabled_mod_is_rejected` | 未導入 ID と重複 ID を拒否する。 |
| `test_restart.py::test_atlas_images_missing_from_the_mod_packages_are_reported` | `restart_parity atlas_images_missing_from_mod_packages_are_reported` | XML の atlas、単体 DDS、BLP 内の画像名を照合する。 |
| `test_restart.py::test_stale_art_stops_before_restart` | **Runner 統合待ち** | 古い画像を検出したとき、再起動と最初の手順を実行せず、失敗ログに画像名と `build_art.py` を残す。検出ロジックは上記 atlas テストが確認する。 |

## `test_safety.py` — InputSafetyTests

| 旧 `file::test` | Rust `--test file name` | 維持する主な検証 |
| --- | --- | --- |
| `test_safety.py::test_deadline_expiring_during_focus_sends_no_key` | `native_input deadline_expiring_during_focus_sends_no_key` | フォーカス待ちで期限が切れたら送信しない。 |
| `test_safety.py::test_deadline_expiring_after_key_mapping_sends_no_window_key` | `native_input deadline_expiring_after_key_mapping_sends_no_window_key` | キー変換後、送信直前に期限を確認する。 |
| `test_safety.py::test_deadline_expiring_after_foreground_check_sends_no_sendinput_key` | `native_input deadline_expiring_after_foreground_check_sends_no_sendinput_key` | 前面確認後に期限が切れたら SendInput しない。 |
| `test_safety.py::test_key_before_deadline_is_sent` | `native_input key_before_deadline_is_sent_in_both_modes` | 窓向けキーと SendInput が各々押下・解放を一度ずつ送る。 |
| `test_safety.py::test_foreground_mismatch_sends_no_keys_or_clicks` | `native_input foreground_mismatch_sends_no_keys_or_clicks` | 前面不一致ではキー、カーソル移動、クリックを送らない。 |
| `test_safety.py::test_window_key_releases_after_foreground_changes` | `native_input window_key_releases_after_foreground_changes` | フォーカス変化後も対象窓へキー解放を送る。 |
| `test_safety.py::test_window_key_targets_verified_game_while_background` | `native_input window_key_targets_verified_game_while_background` | 背景でも確認済み HWND にだけ窓向けキーを送る。 |
| `test_safety.py::test_window_key_rechecks_target_before_background_input` | `native_input window_key_rechecks_target_before_background_input` | HWND の変化で送信を中止する。 |
| `test_safety.py::test_sendinput_fallback_checks_foreground_and_releases_key` | `native_input sendinput_fallback_releases_key_after_focus_loss` | 押下後に前面が変わってもキーを解放する。 |
| `test_safety.py::test_input_error_is_logged_as_step_failure` | **Runner 統合待ち** | Input エラーを手順失敗として記録する。入力モジュールが `ErrorKind::Input` を返す点は上記期限テストが確認する。 |
| `test_safety.py::test_failed_foreground_activation_sends_no_input` | `native_input failed_foreground_activation_sends_no_input` | 前面化できなければ送信しない。 |
| `test_safety.py::test_window_must_have_unique_civ_executable` | `native_input window_must_have_unique_civ_executable` | DX11/DX12 窓だけを数え、補助プロセスや複数窓を拒否する。 |
| `test_safety.py::test_hidden_second_civ_process_blocks_input_before_window_scan` | `native_input hidden_second_civ_process_blocks_input_before_window_scan` | 隠れた二つ目のゲームプロセスを先に拒否する。 |
| `test_safety.py::test_process_snapshot_counts_both_civ_executables` | `native_input process_snapshot_counts_both_civ_executables` | プロセス名を大文字小文字を無視して照合し、DX11 と DX12 を数える。 |

## 担当境界と残る検証

`test_restart.py::RestartActionTests` の Runner 手順と、上表の **Runner 統合待ち** 2 件は main 担当。現在この crate の `src/lib.rs` に Runner はなく、担当外ファイルへの変更も禁止されているため、実際の Rust テスト名を捏造していない。main はこれらを実装して `tests/PARITY-SCOPE.md` に旧 `file::test` から実際の Cargo テスト名への対応を記録する必要がある。

Win32 の実窓・実プロセスでの入力、撮影、Steam 起動はこの作業では行っていない。注入したバックエンドによる判断テストと Windows 向けのビルド、Linux 向けライブラリチェックを実施した。Windows 上での Linux 向け `cargo check --tests` は、既存 dev 依存の Lua/SQLite をビルドする `x86_64-linux-gnu-gcc` がなく失敗したため、Linux 上でのテスト実行は未確認。
