# Python → Rust parity for judge and metrics

Run the owned tests with:

```powershell
C:/Users/makhmeto/.cargo/bin/cargo.exe test --manifest-path tools/civ6-autotest-rs/Cargo.toml --target-dir tools/target --offline --test judge_parity
C:/Users/makhmeto/.cargo/bin/cargo.exe test --manifest-path tools/civ6-autotest-rs/Cargo.toml --target-dir tools/target --offline --test metrics_parity
```

The rows below name actual Rust tests. “Driver gap” means the assertion also uses `scenario.Runner` or `validate_scenario`, which is outside this worker's owned paths and is **not** counted as fully ported by a recorder-only test. No test is skipped or ignored.

| Old test | Rust test | Preserved assertions / driver gap |
|---|---|---|
| `test_judge.py::JudgeTests::test_bundle0_success_and_unrelated_database_error` | `judge_parity::bundle0_success_and_unrelated_database_error` | ≥9 checks pass; unrelated database error is ignored. |
| `test_judge.py::JudgeTests::test_restored_state_mismatch_fails` | `judge_parity::restored_state_mismatch_fails` | Saved-state mismatch fails the named check. |
| `test_judge.py::JudgeTests::test_missing_action_fails` | `judge_parity::missing_action_fails` | Missing script action fails. |
| `test_judge.py::JudgeTests::test_generic_errors_include_stack_and_api_error` | `judge_parity::generic_errors_include_stack_and_api_error` | Exact two generic failures, including stack context. |
| `test_judge.py::JudgeTests::test_missing_jac_icon_or_texture_fails` | `judge_parity::missing_jac_icon_or_texture_fails` | Official icon ignored; JAC icon rejected. |
| `test_judge.py::JudgeTests::test_missing_jac_texture_fails` | `judge_parity::missing_jac_texture_fails` | Two JAC textures counted; unrelated texture ignored. |
| `test_judge.py::JudgeTests::test_error_none_is_not_a_failure` | `judge_parity::error_none_is_not_a_failure` | `error=none` passes. |
| `test_judge.py::JudgeTests::test_database_error_with_mod_context_fails` | `judge_parity::database_error_with_mod_context_fails` | Following file diagnostic attributes an error to JAC. |
| `test_judge.py::JudgeTests::test_growth_success_and_restored_mismatch` | `judge_parity::growth_success_and_restored_mismatch` | ≥12 passing checks; changed restored XP fails. |
| `test_judge.py::JudgeTests::test_growth_expected_two_units_in_assignment_restore_and_journal` | `judge_parity::growth_expected_two_units_in_assignment_restore_and_journal` | Two units pass; missing assignment, missing restore and wrong journal each fail. |
| `test_judge.py::JudgeTests::test_growth_ignores_unrelated_game_error_and_catches_target_ui_error` | `judge_parity::growth_ignores_unrelated_game_error_and_catches_target_ui_error` | Exact sets of failed checks for unrelated and JAC UI errors. |
| `test_judge.py::JudgeTests::test_japan_map_fails_on_congress_dependent_ui_error` | `judge_parity::japan_map_fails_on_congress_dependent_ui_error` | Four Congress/GS path cases fail after unrelated base error passes. |
| `test_judge.py::JudgeTests::test_baseline_excludes_old_lines_and_handles_truncate` | `judge_parity::baseline_excludes_old_lines_and_handles_truncate` | Append, truncate and new inode slices. |
| `test_judge.py::JudgeTests::test_baseline_detects_same_inode_rewrite_past_old_offset` | `judge_parity::baseline_detects_same_inode_rewrite_past_old_offset` | Append and same-inode rewrite with changed tail hash. |
| `test_judge.py::JudgeTests::test_shipped_scenarios_validate_offline` | `judge_parity::shipped_scenario_inventory_and_json_are_intact` | Exact scenario inventory, JSON parsing and `growth.expected_units=2`. **Driver gap:** `validate_scenario` for every scenario and the missing-`expected_units` error await the driver's validator. The worker cannot edit its module or `lib.rs`. |
| `test_metrics.py::MeasurementTests::test_fast_forward_uses_core_clock_and_discards_stale_ui_measurement` | `metrics_parity::fast_forward_uses_core_clock_and_discards_stale_ui_measurement` | Recorder rows and research turns 95–97. **Driver gap:** Lua/UI clock selection and autoplay detail `95->97`. |
| `test_metrics.py::MeasurementTests::test_fast_forward_csv_files_warn_against_continuous_performance_comparison` | `metrics_parity::fast_forward_csv_files_warn_against_continuous_performance_comparison` | Both CSV files have `True` test mode and `False` performance validity. |
| `test_metrics.py::MeasurementTests::test_csv_and_summary_keep_skipped_turns_and_untimed_intervals_explicit` | `metrics_parity::csv_and_summary_keep_skipped_turns_and_untimed_intervals_explicit` | Exact CSV columns, observed/missed counts, duration and memory statistics, backward-turn error. |
| `test_metrics.py::MeasurementTests::test_first_interval_starts_before_autoplay_response` | `metrics_parity::first_interval_starts_before_autoplay_response` | First interval is 4.0 seconds from explicit start timestamp. |
| `test_metrics.py::MeasurementTests::test_stdlib_memory_api_samples_a_real_process` | `metrics_parity::stdlib_memory_api_samples_a_real_process` (Windows) / `process_memory_requires_windows` (other OS) | Real current-process working set and private bytes on Windows; Linux test checks explicit unsupported result. |
| `test_metrics.py::MeasurementTests::test_save_load_measure_actual_file_and_wait_until_ready` | `metrics_parity::save_load_measure_actual_file_and_wait_until_ready` | Exact recorder CSV rows and median. **Driver gap:** actual save file size, readiness wait, and step detail strings. |
| `test_metrics.py::MeasurementTests::test_result_keeps_partial_measurements_on_failed_run` | **Driver gap** | Runner failure result, partial autoplay metrics, and completed step count await driver integration. |
| `test_metrics.py::MeasurementTests::test_save_load_flags_require_boolean` | **Driver gap** | Scenario validation of save/load `metrics` flags awaits driver integration. |
| `test_metrics.py::MeasurementTests::test_result_separates_continuous_memory_from_post_load_memory` | `metrics_parity::result_separates_continuous_memory_from_post_load_memory` | Continuous/post-load growth 400/10 and combined growth −40. **Driver gap:** result.json group shape. |
| `test_metrics.py::MeasurementTests::test_interruption_is_recorded_and_summary_write_error_does_not_replace_it` | **Driver gap** | Runner interruption precedence and result.json failure await driver integration. |
| `test_metrics.py::MeasurementTests::test_metric_groups_are_validated` | **Driver gap** | Scenario `metrics_group` validation awaits driver integration. |
| `test_driver_features.py::DriverFeatureTests::test_research_samples_and_report_use_first_observed_completion` | `metrics_parity::research_samples_and_report_use_first_observed_completion` | Lua research API strings, sorted slot code, six JSONL rows, first observed completion, civic summary, changed-slot and invalid-count errors. |
| `test_driver_features.py::DriverFeatureTests::test_autoplay_emits_research_jsonl_for_each_observed_turn` | **Driver gap** | Autoplay must invoke `ResearchMetrics::sample` at turns 1, 2, 3 and include the artifact name in its detail. |

Additional tests: `judge_parity::marker_detects_inode_and_tail_changes`, `japan_map_attributes_errors_to_loaded_mod_and_ignores_following_loads`, `missing_required_log_and_bom_are_explicit`, `strict_errors_and_invalid_input_fail_closed`, `malformed_huge_turn_counters_fail_closed`; `metrics_parity::research_sampling_deduplicates_turn_and_rejects_backward_or_bad_ids`, `report_counts_start_baseline_and_rejects_invalid_rows`. `judge_cli::snapshot_then_judge_new_log_bytes_and_exit_codes` verifies the command-line exit codes and baseline JSON.

`test_judge.py::AutoplayTests` belongs to the main driver owner by assignment.
