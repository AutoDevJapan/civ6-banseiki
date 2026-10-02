# Fixture and scenario parity

Every Rust entry below is an executable `cargo test --test <target> <name>` filter. The
tests read the committed JSON, XML, SQL, and Lua assets by `repo_root()`. They never
load Python modules or connect to Civ6. All embedded Lua is compiled or executed by
the vendored Lua 5.1 runtime. `fixtures/*.lua` contains only harness code copied from
legacy tests; no MOD Lua was changed. No Rust test is skipped or ignored.

## MOD dependency execution (`test_enable_mods.py`)

Target: `enable_mods`. Every row executes the **actual** `scenario::enable_mods_code`
output against `fixtures/modding_stub.lua`.

| Legacy `test_enable_mods.py::` | Rust test | Assertions retained |
| --- | --- | --- |
| `test_transitive_dependents_are_disabled_first_once_and_reported` | `transitive_dependents_are_disabled_first_once_and_reported` | Leaf, middle, sibling, base order and exact returned list; requested enable, unrelated disabled MOD, one update |
| `test_traversal_includes_enabled_descendants_of_a_disabled_intermediate` | `traversal_includes_enabled_descendants_of_a_disabled_intermediate` | Disabled intermediate is traversed; only leaf and base disabled; unrelated selection intact |
| `test_requested_dependents_are_preserved_and_conflicts_fail_before_mutations` | `requested_dependents_are_preserved_and_conflicts_fail_before_mutations` | Both initial leaf states; conflict error; no mutation; base and leaf selection intact |
| `test_dependency_cycle_fails_before_mutations` | `dependency_cycle_fails_before_mutations` | Cycle error and no disable mutation |
| `test_disable_verifies_the_engine_state_and_names_the_failed_mod` | `disable_verifies_the_engine_state_and_names_the_failed_mod` | Engine refusal names base, base remains enabled, no settings update |
| `test_no_disable_targets_leaves_unrelated_mods_enabled` | `no_disable_targets_leaves_unrelated_mods_enabled` | Empty disabled report, no disable calls, prerequisite and requested MOD enabled |
| `test_late_disable_rejection_is_detected_before_any_settings_change` | `late_disable_rejection_is_detected_before_any_settings_change` | Late preflight error; no enable/disable mutations; original selection intact |
| `test_missing_enable_target_fails_before_disabling_other_mods` | `missing_enable_target_fails_before_disabling_other_mods` | Missing target named before any disable; prerequisite/dependent still enabled |
| `test_engine_refusal_restores_changes_without_enabling_disabled_intermediates` | `engine_refusal_restores_changes_without_enabling_disabled_intermediates` | Leaf disabled then restored; base and leaf enabled; initially disabled middle and keep stay disabled |
| `test_failed_restoration_identifies_the_mod_still_disabled` | `failed_restoration_identifies_the_mod_still_disabled` | Restoration error names leaf; leaf remains disabled and base enabled |
| `test_late_enable_failure_restores_disabled_and_newly_enabled_mods` | `late_enable_failure_restores_disabled_and_newly_enabled_mods` | Enable refusal names keep; all five original states restored; one update |
| `test_implicitly_enabled_prerequisite_is_restored_when_enable_throws` | `implicitly_enabled_prerequisite_is_restored_when_enable_throws` | Exception after prerequisite enable; leaf/base/off disable order; four final states |
| `test_enable_failure_without_disable_targets_restores_the_selection` | `enable_failure_without_disable_targets_restores_the_selection` | Refusal names keep; base, keep and sibling restored |
| `test_enable_and_disable_overlap_is_case_insensitive` | `enable_and_disable_overlap_is_case_insensitive` | Validation rejects mixed-case overlap |

The legacy two `_run_step` cases also assert FrontEnd dispatch and human-readable
detail. Those assertions require the Runner dispatcher owned by the main migration;
the fixture target tests the returned disabled list and engine state directly.

## Scenario fixtures (`test_ages_probe.py`, `test_banseiki_ruleset.py`, `test_era_timing_probe.py`)

Target: `scenario_fixtures`. `banseiki_ruleset::` denotes the included module in
`fixtures/banseiki_ruleset.rs`.

| Legacy test | Rust test | Assertions retained |
| --- | --- | --- |
| `test_ages_probe.py::test_every_chunk_is_followed_by_a_full_previous_era_record` | `ages_every_chunk_is_followed_by_a_full_previous_era_record` | Every autoplay chunk records all 22 players, the same chunk length, no swallowed Lua error |
| `test_ages_probe.py::test_chunk_length_is_guarded_by_the_minimum_era_length` | `ages_chunk_length_is_guarded_by_the_minimum_era_length` | Guard equals longest chunk; ruleset and alive-major guards |
| `test_ages_probe.py::test_api_listing_separates_not_enumerable_from_absent` | `ages_api_listing_separates_not_enumerable_from_absent` | Three distinct listing outputs |
| `test_ages_probe.py::test_changed_since_last_boundary` | `ages_changed_since_last_boundary` | Extracted Lua expression run against five boundary cases for every record; never skipped |
| `test_ages_probe.py::test_moments_cover_all_players` | `ages_moments_cover_all_players` | Final UI step queries moments for players 0–21 |
| `test_banseiki_ruleset.py::test_audited_exclusion_is_exact_and_applies_to_every_banseiki_slot` | `banseiki_ruleset::audited_exclusion_is_exact_and_applies_to_every_banseiki_slot` | Registered SQL, exact Sejong exclusion, executable SQLite filters for three slots, human pool and official ruleset |
| `test_banseiki_ruleset.py::test_duplicate_policy_and_runtime_scenario_cover_same_civilization` | `banseiki_ruleset::duplicate_policy_and_runtime_scenario_cover_same_civilization` | Both duplicate settings and before/after start checks including Greece leaders |
| `test_era_timing_probe.py::test_every_single_turn_is_logged_in_both_contexts` | `era_every_single_turn_is_logged_in_both_contexts` | At least 15 one-turn chunks and two logger contexts per speed |
| `test_era_timing_probe.py::test_speeds_differ_only_in_speed` | `era_speeds_differ_only_in_speed` | Probe MOD and exact step equality after speed normalization |
| `test_era_timing_probe.py::test_setup_guard_matches_the_test_mod_sql` | `era_setup_guard_matches_the_test_mod_sql` | SQL era limits equal Lua guard; countdown parameter |
| `test_era_timing_probe.py::test_other_scenarios_on_the_test_map_disable_the_probe_mod` | `era_other_scenarios_on_the_test_map_disable_the_probe_mod` | Every matching JSON, special beyond-nine enable and tail flag |
| `test_era_timing_probe.py::test_gamecore_logger_never_raises_and_reports_changes` | `era_gamecore_logger_never_raises_and_reports_changes` | Real Lua execution, absent/raising getters, first and changed records, exact printed sequence |
| `test_era_timing_probe.py::test_checked_turns_are_the_generated_era_starts` | `era_checked_turns_are_the_generated_era_starts` | Committed generated era SQL yields 66/73/110; scenario map, MOD disable, era and ledger checks, total 75 turns |

## Oaths and Japan map (`test_oaths.py`, `test_japan_map.py`)

Target: `scenario_fixtures`.

| Legacy test | Rust test | Assertions retained |
| --- | --- | --- |
| `test_oaths.py::test_scenario_is_valid_starts_as_oda_and_checks_load_before_advancing` | `oath_scenario_is_valid_starts_as_oda_and_checks_load_before_advancing` | Validation, 22 players and sole Oda human, post-load GameCore checks |
| `test_oaths.py::test_every_embedded_lua_step_and_wait_compiles` | `oath_every_embedded_lua_step_and_wait_compiles` | Every code or Lua wait compiles; at least 12 chunks |
| `test_oaths.py::test_automatic_popup_assertion_cannot_force_a_missing_screen_open` | `oath_automatic_popup_assertion_cannot_force_a_missing_screen_open` | Executed popup Lua fails with the expected message when hidden |
| `test_oaths.py::test_fast_forward_runs_engine_jump_with_scope_checks_and_keeps_source_intact` | `oath_fast_forward_marks_engine_jump_and_keeps_source_intact` | Immutable source, exact transformed step and test metadata; engine jump, ruleset/multiplayer guards, skipped count and autoplay call count remain Runner-owned |
| `test_japan_map.py::test_every_scale_scenario_is_valid` | `japan_every_scale_scenario_is_valid` | Both scales, generated configuration order and 22 slots, capture names, city/territory/loyalty and reload checks |
| `test_japan_map.py::test_takeda_variant_checks_selected_faction_and_full_initial_territory` | `japan_takeda_variant_checks_selected_faction_and_full_initial_territory` | Both scales, four steps, sole Takeda human, territory and capital checks |
| `test_japan_map.py::test_resource_scenario_matches_the_manifest_on_the_adopted_15km_map` | `japan_resource_scenario_matches_the_manifest_on_the_adopted_15km_map` | Manifest and map, nine deferred resources, all Lua tuples, calendar checks, 24 turns |
| `test_japan_map.py::test_rejects_bad_map_file_and_metrics_flag` | `japan_rejects_bad_map_file_and_metrics_flag` | Invalid map path and nonboolean metrics rejected |

## Verification and driver fixture assertions

Target: `scenario_fixtures`.

| Legacy test | Rust test | Assertions retained |
| --- | --- | --- |
| `test_verification_scenarios.py::test_run_rejects_invalid_metadata_even_without_a_fast_forward_step` | `verification_run_rejects_invalid_metadata_even_without_a_fast_forward_step` | Both step variants rejected by validation; Runner side-effect checks remain main-owned |
| `test_verification_scenarios.py::test_invalid_test_mode_is_rejected_before_any_game_call` | `verification_invalid_test_mode_is_rejected_before_any_game_call` | All six malformed metadata shapes rejected; Runner call count remains main-owned |
| `test_verification_scenarios.py::test_scenario_localization_keys_exist_in_mod_text` | `verification_scenario_localization_keys_exist_in_mod_text` | Scan MOD XML tags and every relevant Lua chunk; undefined keys empty |
| `test_verification_scenarios.py::test_fast_forward_keeps_checkpoints_without_claiming_continuous_metrics` | `verification_fast_forward_keeps_checkpoints_without_claiming_continuous_metrics` | Exact 50×6+1 chunks, test metadata, renamed metrics, preserved save/load and source |
| `test_verification_scenarios.py::test_all_new_chunks_validate_and_compile_as_lua` | `verification_all_new_chunks_validate_and_compile_as_lua` | Seven scenarios validate, strict flag, every embedded Lua chunk compiles |
| `test_verification_scenarios.py::test_full_map_measurements_cover_continuous_play_and_both_reloads` | `verification_full_map_measurements_cover_continuous_play_and_both_reloads` | 20/300 turn plans, 22 players, metrics groups, save/load checkpoints and snapshots |
| `test_verification_scenarios.py::test_saved_state_snapshot_uses_indexed_city_iterator_and_detects_changes` | `verification_saved_state_snapshot_uses_indexed_city_iterator_and_detects_changes` | Real Lua snapshot determinism and mutation/error cases |
| `test_verification_scenarios.py::test_boundary_checks_accept_verified_state_and_reject_wrong_turn_era_and_ledger` | `verification_boundary_checks_accept_verified_state_and_reject_wrong_turn_era_and_ledger` | Exactly 34 Lua checks with accepted state and rejected turn, era and ledger mutations |
| `test_verification_scenarios.py::test_optional_tail_is_scoped_hidden_and_preserves_endless_last_era` | `verification_optional_tail_is_scoped_hidden_and_preserves_endless_last_era` | MOD criteria, hidden parameter, probe flag, executable SQLite tail SQL, 47 two-turn eras and endless Chidai |
| `test_verification_scenarios.py::test_resource_calendar_calls_actual_getter_and_rejects_wrong_year` | `verification_resource_calendar_calls_actual_getter_and_rejects_wrong_year` | Six executable Lua checks, 1534/1540/1543, wrong-year rejection and GameCore isolation |
| `test_verification_scenarios.py::test_strict_judge_rejects_official_screen_errors` | `verification_strict_judge_flag_validation` | Nonboolean strict flag rejected; log classification remains judge-owned |
| `test_driver_features.py::test_end_turn_rejects_invalid_counts` | `driver_end_turn_rejects_invalid_counts` | Five invalid `turns` and `test_turns` values each |
| `test_driver_features.py::test_build_setup_checks_every_player_after_refresh` | `driver_build_setup_checks_every_player_after_refresh` | Generated setup order, two slot checks, Lua compilation; Runner dispatch remains main-owned |
| `test_driver_features.py::test_build_setup_defaults_on_for_banseiki_only` | `driver_build_setup_defaults_on_for_banseiki_only` | Four ruleset/override combinations; Runner dispatch remains main-owned |
| `test_driver_features.py::test_flag_validation` | `driver_flag_validation` | `build_setup`, `research_metrics`, `allow_human_elimination`, `exact_name` type errors |
| `test_driver_features.py::test_new_scenarios_check_requested_states` | `driver_new_scenarios_check_requested_states` | Setup screen, settler map, 200-turn plan, Banseiki and research safeguards |

`test_diplomacy.py` and `test_dedication.py` contain Runner behavior tests only;
`test_japan_map.py` and `test_driver_features.py` also contain Runner, metrics,
and judge tests. `test_verification_scenarios.py` has three additional Runner/
judge behaviors: lost jump acknowledgment, era-boundary stepping, and strict log
judging. Those are outside this fixture assignment and must be mapped to actual
Rust Runner/judge tests by the main migration before claiming full legacy parity.
