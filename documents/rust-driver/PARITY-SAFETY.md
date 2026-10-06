# Scenario safety test parity

Source: `tools/civ6-autotest/tests/test_safety.py::ScenarioSafetyTests` (39 methods). The `InputSafetyTests` class in the same file belongs to the native input worker.

Run all rows with `C:/Users/makhmeto/.cargo/bin/cargo.exe test --offline --manifest-path tools/civ6-autotest-rs/Cargo.toml --target-dir tools/target --test driver_safety`. To run one row, append its **Cargo filter**. Every filter is an actual `#[test]` function in `tests/driver_safety.rs`. No tests are skipped or ignored. The `SafetyBackend` prevents real game process, Steam, input, screenshot, and FireTuner calls; `FakeClock` advances waits without wall-clock delays.

| Legacy `test_safety.py::ScenarioSafetyTests::` method | Cargo filter | Assertions retained |
| --- | --- | --- |
| `test_owner_start_opens_setup_without_hosting_and_waits_long` | `test_owner_start_opens_setup_without_hosting_and_waits_long` | Advanced Setup path, no HostGame/default reset, 3600-second transition deadline. |
| `test_load_wait_sends_one_escape_even_if_ingame_state_exists` | `test_load_wait_sends_one_escape_even_if_ingame_state_exists` | One Escape; local-player predicate must change from false to true. |
| `test_load_wait_presses_begin_game_through_loadscreen_lua_without_focus` | `test_load_wait_presses_begin_game_through_loadscreen_lua_without_focus` | LoadScreen Lua activates Begin Game; no key input. |
| `test_load_wait_never_sends_escape_before_load_complete_log` | `test_load_wait_never_sends_escape_before_load_complete_log` | Timeout without a new load-complete line; no state query or key. |
| `test_load_wait_sends_begin_game_escape_only_once` | `test_load_wait_sends_begin_game_escape_only_once` | Later InGame availability does not send a second Escape. |
| `test_load_wait_reconnects_when_frontend_state_list_is_stale` | `test_load_wait_reconnects_when_frontend_state_list_is_stale` | Stale FrontEnd state is disconnected before InGame confirmation; one Escape. |
| `test_load_wait_timeout_never_repeats_escape` | `test_load_wait_timeout_never_repeats_escape` | Timeout after one Escape never repeats input. |
| `test_transition_lua_errors_stop_without_retrying_mutations` | `test_transition_lua_errors_stop_without_retrying_mutations` | Unknown and confirmed Lua errors both stop; options-menu close mutation once; no sleep/retry. |
| `test_transition_detects_lua_log_rotation_and_truncation_before_input` | `test_transition_detects_lua_log_rotation_and_truncation_before_input` | Both rotation and truncation are undecidable before any state query or key. |
| `test_transition_detects_lua_log_rotation_after_escape` | `test_transition_detects_lua_log_rotation_after_escape` | Generation change after exactly one Escape is undecidable. |
| `test_transition_rejects_same_inode_rewrite_even_if_log_grows` | `test_transition_rejects_same_inode_rewrite_even_if_log_grows` | Same file identity with rewritten prefix/tail is undecidable; no key. |
| `test_run_creates_missing_output_parents_but_rejects_existing_run` | `test_run_creates_missing_output_parents_but_rejects_existing_run` | Nested output is created; second run rejects existing leaf; step log exists. |
| `test_multiple_civ_processes_stop_run_before_tuner_access` | `test_multiple_civ_processes_stop_run_before_tuner_access` | Duplicate-process error ends run before tuner access and records run failure. |
| `test_failed_cold_start_is_reported_before_tuner_access` | `test_failed_cold_start_is_reported_before_tuner_access` | Failed launch is recorded once before tuner access; failure text is retained. |
| `test_unreachable_tuner_reports_connection_failure` | `test_unreachable_tuner_reports_connection_failure` | Connection failure remains a tuner error; exactly two attempts. |
| `test_screenshot_paths_reject_drives_absolute_and_parent_escape` | `test_screenshot_paths_reject_drives_absolute_and_parent_escape` | Drive-relative, drive-absolute, UNC, rooted and parent traversal paths reject; valid relative path accepts. |
| `test_screenshot_rejects_resolved_path_outside_output` | `test_screenshot_rejects_resolved_path_outside_output` | Directory link/junction escaping output rejects before capture. |
| `test_screenshot_fails_if_capture_finishes_after_deadline` | `test_screenshot_fails_if_capture_finishes_after_deadline` | Capture receives the deadline once; late completion fails. |
| `test_nonfinite_timeouts_are_rejected` | `test_nonfinite_timeouts_are_rejected` | Boolean, zero, negative, string and null timeouts reject; finite positive accepts. JSON rejects infinity, NaN and overflowing numbers at parse/number construction. |
| `test_scenario_validates_lua_code_before_running` | `test_scenario_validates_lua_code_before_running` | NUL and UTF-8 oversize reject for ui_lua and Lua wait; exact byte limit accepts. |
| `test_manual_wait_has_no_step_deadline_or_timeout_option` | `test_manual_wait_has_no_step_deadline_or_timeout_option` | Manual step has no timeout option and does not read the clock. |
| `test_wait_caps_state_and_lua_queries_to_remaining_time` | `test_wait_caps_state_and_lua_queries_to_remaining_time` | State and Lua backend timeouts are positive and at most the wait budget. |
| `test_lua_wait_unknown_reply_fails_without_reexecution` | `test_lua_wait_unknown_reply_fails_without_reexecution` | Unknown InGame result is not resent; popup probe may run; no sleep. |
| `test_lua_wait_confirmed_error_fails_without_reexecution` | `test_lua_wait_confirmed_error_fails_without_reexecution` | Confirmed Lua error propagates after one call, without sleep. |
| `test_pre_send_framing_error_is_logged_as_step_failure` | `test_pre_send_framing_error_is_logged_as_step_failure` | ui_lua and Lua wait each log one failed step and exact framing error. |
| `test_transition_pre_send_failure_is_logged_without_waiting` | `test_transition_pre_send_failure_is_logged_without_waiting` | Start and load mutations each send once; connection error logged; no transition key. |
| `test_transition_waits_after_unknown_reply_without_resending` | `test_transition_waits_after_unknown_reply_without_resending` | Start and load observe a fresh load-complete line after unknown reply; mutation once. |
| `test_unknown_transition_without_read_only_confirmation_is_undecidable` | `test_unknown_transition_without_read_only_confirmation_is_undecidable` | Unknown reply without log confirmation times out as undecidable; no key. |
| `test_confirmed_transition_lua_error_never_enters_wait` | `test_confirmed_transition_lua_error_never_enters_wait` | Start and load confirmed Lua errors stop after one mutation; no transition input. |
| `test_exit_to_menu_waits_for_state_before_lua_predicate` | `test_exit_to_menu_waits_for_state_before_lua_predicate` | Exit mutation, disconnect, state wait and FrontEnd Lua predicate occur in order. |
| `test_log_wait_requires_new_line_and_does_not_reuse_a_match` | `test_log_wait_requires_new_line_and_does_not_reuse_a_match` | Old line times out; one new line succeeds once; consumed line cannot satisfy next wait. |
| `test_log_wait_stops_if_generation_changes` | `test_log_wait_stops_if_generation_changes` | Rewritten log generation rejects as undecidable. |
| `test_later_step_cannot_reuse_previous_steps_log_line` | `test_later_step_cannot_reuse_previous_steps_log_line` | Step 2 passes on new line; step 4 fails after fresh step baseline; no launch. |
| `test_oserror_after_autoplay_start_attempts_cleanup_without_replay` | `test_oserror_after_autoplay_start_attempts_cleanup_without_replay` | I/O error after one start mutation triggers one cleanup, without replay. |
| `test_autoplay_lua_error_stops_without_cleanup_or_resend` | `test_autoplay_lua_error_cleans_up_only_after_acknowledged_start` | Unknown/confirmed errors preserve one start mutation; cleanup runs once only after an acknowledged start. |
| `test_end_turn_closes_successive_ai_statements_without_repeating_action` | `test_end_turn_closes_successive_ai_statements_without_repeating_action` | Two successive diplomacy screens close; turn advances 7→8; end-turn action once. |
| `test_end_turn_reopening_diplomacy_has_a_fixed_limit` | `test_end_turn_reopening_diplomacy_has_a_fixed_limit` | The 26th screen trips the fixed limit. |
| `test_end_turn_honours_turns_and_rejects_invalid_values` | `test_end_turn_honours_turns_and_rejects_invalid_values` | Three requested turns and one default turn each act once; zero, negative, string and fraction reject. |
| `test_step_deadline_prevents_late_autoplay_and_end_turn_side_effects` | `test_step_deadline_prevents_late_autoplay_and_end_turn_side_effects` | Expired budgets after notebook close prevent autoplay activation and end-turn input. |

The source mock patches `_run_step` or `_wait` in a few cases; this suite uses the actual Rust `Runner` methods with an injected backend and clock. Timing checks use a fake clock, so the bounded waits are deterministic and can be tested on Linux as well as Windows. The screenshot escape test attempts a symlink, then a Windows junction if symlink creation is denied; a failure to create either fails the test.
