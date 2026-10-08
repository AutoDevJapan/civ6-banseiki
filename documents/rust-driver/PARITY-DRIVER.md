# ドライバー実行テストの対応表

共通の実行形は次のとおり。各表の `Rust テスト` はこのコマンドの末尾に付ける完全一致のフィルターである。

```text
C:/Users/makhmeto/.cargo/bin/cargo.exe test --offline --manifest-path tools/civ6-autotest-rs/Cargo.toml --target-dir tools/target --test <Rust ファイル> <Rust テスト> -- --exact
```

`--offline` はこの環境で crates.io の TLS 資格情報が取得できないために指定した。依存パッケージはキャッシュから解決した。各テストは注入した `Backend` と `Clock` だけを使い、実際の Civ VI は起動しない。下表の旧名は `tools/civ6-autotest/tests/` 以下の `ファイル::クラス::メソッド` を表す。

## test_driver_features.py::DriverFeatureTests → driver_features

| 旧メソッド = Rust テスト | 確認する観測結果 |
| --- | --- |
| test_shortened_turns_require_explicit_test_mode | 通常は10回、テストモードでは1回だけターンを送る |
| test_configuration_clears_test_mode_on_ordinary_runs | 0/1 の設定値と有効 MOD 更新 Lua を送る |
| test_end_turn_honours_count_default_and_shared_deadline | 2回と既定の1回、同じ期限の残り時間、失敗時は再送しない |
| test_end_turn_rejects_invalid_counts | `turns` と `test_turns` の 0、負数、真偽値、小数、文字列を拒否する |
| test_build_setup_checks_every_player_after_refresh | セットアップ、更新、全プレイヤー確認、画面非表示の順序と送信先を確認する |
| test_build_setup_defaults_on_for_banseiki_only | 専用ルールセットの既定と明示的な有効/無効設定で送信先が変わる |
| test_start_turn_guard | 他ルールでは照会せず、専用ルールでは開始ターン1だけを許す |
| test_output_scoped_save_name_and_exact_name | 出力別の名前、ロードとの一致、80文字上限、正確な名前、上書き前の停止 |
| test_save_dismisses_blocking_advisor_before_saving | オプション画面、助言画面、保存 Lua の順序と保存ファイルの実在 |
| test_flag_validation | 構築、研究記録、人間消滅許可、正確な名前の各フラグを検証する |
| test_allow_elimination_observes_target_and_records_turn | 目標ターン3と最初の消滅観測ターン2を結果と JSONL に記録する |
| test_autoplay_emits_research_jsonl_for_each_observed_turn | 1/2/3 ターンの研究行、完了技術と成果物名を確認する |
| test_early_stop_resume_clears_done_flag_and_waits_for_new_end_event | 再開を1回だけ送り、完了印を先に消し、新しい終了イベントでターン4まで進む |

このファイルの残る `test_research_samples_and_report_use_first_observed_completion` は研究解析の担当、`test_new_scenarios_check_requested_states` はシナリオ静的検査の担当である。

## test_diplomacy.py::DiplomacyDuringAutoplayTests / StarvedInGameStepTests → driver_diplomacy

| 旧メソッド = Rust テスト | 確認する観測結果 |
| --- | --- |
| test_statement_after_final_turn_retries_only_idempotent_handler_cleanup | 最終ターン後の応答喪失で後処理だけ2回、開始は1回、外交画面を閉じる |
| test_test_mode_counts_from_core_when_ui_turn_is_stale_after_jump | UI の古い1ターンではなく GameCore の63から65を数える |
| test_statement_mid_autoplay_is_closed_and_autoplay_completes | 中断画面を閉じて目標6まで進み、開始を再送しない |
| test_repeated_early_stops_by_statements_resume_without_using_restart_budget | 3画面を閉じ、3回再開してターン6に達する |
| test_deal_proposal_is_refused_before_the_statement_view_closes | 取引提案を先に断り、次に声明を閉じる。公式ハンドラを使う |
| test_status_query_starved_by_statement_closes_it_and_asks_again | 状態応答を塞ぐ声明を閉じ、ターン4まで進む |
| test_dark_age_policy_choice_is_closed_without_holding_autoplay | 暗黒時代画面を閉じ、ターン3に達する |
| test_oath_choice_is_closed_after_reign_without_selecting_official_dedications | 改元画面の後で現れる誓約画面を閉じる |
| test_era_name_change_screen_is_closed_then_the_dark_policy_choice_it_opens | 改元画面、その終了時に開く暗黒時代画面の順で閉じる |
| test_screen_that_keeps_reopening_is_bounded | 再表示が続く場合は26画面で上限エラーにする |
| test_wait_closes_statement_and_polls_again | InGame 応答喪失時に声明を閉じ、1/3 回の記録を残して再照会する |
| test_wait_still_fails_when_nothing_was_open_or_retries_run_out | 閉じる画面がない場合と3回使い切った場合に未知応答エラーを返す |
| test_ui_lua_is_never_replayed_after_it_may_have_run | 応答喪失後も UI Lua の送信は1回に留める |

## test_dedication.py::DedicationTest → driver_dedication

| 旧メソッド = Rust テスト | 確認する観測結果 |
| --- | --- |
| test_stall_with_pending_dedication_chooses_closes_and_resumes | 誓約を1回選び Escape を1回押して再開する |
| test_repeated_dedication_is_bounded_then_fails | 選択3回、再開上限、Escape 3回以下で停止する |
| test_no_pending_dedication_keeps_old_restart_behaviour | 誓約がなければ従来の再開上限で止め、Escape を押さない |

## test_transient_timeouts.py::TransientTimeoutTests → driver_transient

| 旧メソッド = Rust テスト | 確認する観測結果 |
| --- | --- |
| test_status_query_retries_after_timeout | 状態照会を2回行い、古い接続を破棄する |
| test_advisor_dismissal_retries_after_timeout | 助言画面の処理を一過性の応答喪失後に再試行する |
| test_research_query_retries_after_timeout | 研究照会の応答喪失を1回再試行する |
| test_gives_up_only_after_deadline | 期限まで複数回試み、未知応答エラーで停止する |
| test_stale_research_snapshot_stops_at_deadline | 古いターンの研究標本を採用せず、ちょうど期限で止まる |
| test_lua_error_is_not_retried | 確定した Lua エラーは1回で返す |
| test_state_changing_lua_is_not_replayed | 状態を変える Lua は応答不明でも再送しない |

## test_restart.py::RestartActionTests → driver_restart

| 旧メソッド = Rust テスト | 確認する観測結果 |
| --- | --- |
| test_wrong_steam_executable_stops_before_click_or_tuner | DX11 の実行ファイルが違えばクリックと Lua 送信の前に停止する |
| test_restart_action_validation | 正常な再起動設定を許し、無効な期限と余計な MOD 指定を拒否する |
| test_auto_restart_precedes_first_step_and_log_baseline | 古いゲームを先に再起動し、その後のログ位置を基準にする |
| test_no_auto_restart_skips_freshness_query | 自動再起動無効時は MOD の鮮度照会と起動をしない |
| test_still_stale_after_restart_stops_before_first_step | 再起動後も古ければ最初のステップ前に失敗を記録する |
| test_restart_action_launches_and_clicks_verified_window | 起動、窓確認、DX11 確認、クリックの順序と回数を確認する |
| test_frontend_probe_reconnects_after_unknown_read_only_reply | 読み取り照会の応答不明後に切断して再照会する |
| test_frontend_probe_clicks_again_if_movie_still_plays | オープニングが続けば12秒後にもう一度クリックする |
| test_steam_launch_waits_for_delayed_prior_session_without_replaying_uri | 起動窓が遅れても Steam 起動要求は一度だけ送り、結果を待つ |
| test_single_process_waits_for_exiting_process_to_disappear | 重複プロセスが消えるまで待ち、残った PID と開始時刻を返す |
| test_single_process_still_refuses_when_duplicates_persist | 重複が残れば入力エラーで拒否する |
| test_window_wait_ignores_transient_duplicate_processes | 一時的な重複の後で単一のゲーム窓を認識する |
| test_steam_wait_does_not_launch_again_during_process_overlap | 起動中の重複を認め、Steam 起動は1回にする |
| test_first_restart_action_can_start_stopped_game_with_auto_disabled | 自動再起動無効でも最初の再起動アクションで起動できる |
| test_restart_requests_ui_quit_before_steam_launch | UI へ終了を要求してから起動し、強制終了はしない |
| test_restart_forces_only_same_process_after_grace | 同一 PID と開始時刻のゲームだけを猶予後に強制終了する |
| test_restart_without_process_launches_then_waits_for_frontend | 起動、窓、クリック、メインメニュー照会の順で進む |
| test_restart_still_refuses_when_several_processes_persist | 重複が続く場合は終了も起動もしない |
| test_run_launches_civ_when_not_running_and_first_step_needs_game | ゲーム停止時に最初の通常ステップの前で起動する |

## test_japan_map.py::JapanMapScenarioTests → driver_japan

| 旧メソッド = Rust テスト | 確認する観測結果 |
| --- | --- |
| test_capture_and_compare_lua_results | Lua の取得値を保存し、同値を受理、異値を拒否する |
| test_turn_metrics_preserve_missing_samples | 欠測ターンと時間 null、実測2件、欠測1件を保存する |
| test_autoplay_writes_turn_and_memory_samples | ターン1/2/3、メモリ100バイト、実測2件と欠測0件を保存する |
| test_autoplay_reports_eliminated_human_instead_of_overshoot | 人間消滅をターン超過より先に報告する |

同ファイルの地図・シナリオ静的検査4件とログ判定2件は、それぞれシナリオ検査・判定解析の担当である。

## test_judge.py::AutoplayTests → driver_japan

| 旧メソッド = Rust テスト | 確認する観測結果 |
| --- | --- |
| test_event_and_transient_inactive_state_do_not_complete_or_restart_early | 古い終了イベントと一時停止では完了扱い・再起動をせず、状態を5回照会してターン4で終える |

`test_judge.py::JudgeTests` の15件は判定解析の担当である。今回の担当範囲は合計60件で、すべて同名の Rust テストに対応する。

統合担当は、この対応表を `tests/PARITY-SCOPE.md` の全体表へ反映する。この担当では全体表のファイルを編集しない。
