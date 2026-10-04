-- Python test scaffolding transposed to Lua; assertions cross the strict Rust boundary.
local T = {}
function T.test_all_assets_and_native_components_transfer_before_last_city_elimination()
  local L=STRINGS["AbsorptionTests.test_all_assets_and_native_components_transfer_before_last_city_elimination"]
  local tx
  tx = APPLY()
  EQ(function() return tx.Status end,function() return L[1] end)
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(6,5,2) end)
  TRUTH(function() return VERIFY(tx.ID, true) end,true)
  RUN(L[2])
end

function T.test_save_reload_discards_lua_session_and_restores_properties_without_replaying()
  local L=STRINGS["AbsorptionTests.test_save_reload_discards_lua_session_and_restores_properties_without_replaying"]
  local before
  APPLY()
  before = SNAPSHOT()
  RUN(L[1])
  RELOAD()
  EQ(function() return SNAPSHOT() end,function() return before end)
  TRUTH(function() return VERIFY(L[2], true) end,true)
  APPLY()
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(6,5,2) end)
end

function T.test_non_transfer_branches_keep_original_snapshots_and_detach_person_references()
  local L=STRINGS["AbsorptionTests.test_non_transfer_branches_keep_original_snapshots_and_detach_person_references"]
  RUN(L[1])
  APPLY()
  TRUTH(function() return VERIFY(L[2], true) end,true)
  RUN(L[3])
  local retired=A.GetState().Archive['native-person:1:3']
  EQ(function() return retired.Outcome end,function() return 'RETIRE' end)
  EQ(function() return retired.Warlord.Level end,function() return 4 end)
end

function T.test_returned_belonging_goes_to_explicit_surviving_third_party()
  local L=STRINGS["AbsorptionTests.test_returned_belonging_goes_to_explicit_surviving_third_party"]
  RUN(L[1])
  APPLY()
  EQ(function() return A.GetRoster(3).artifact.Owner end,function() return 3 end)
  TRUTH(function() return VERIFY(L[2], true) end,true)
end

function T.test_great_people_cannot_be_converted_to_yields()
  local L=STRINGS["AbsorptionTests.test_great_people_cannot_be_converted_to_yields"]
  for _, outcome in pairs({L[1], L[2], L[3]}) do
    REJECT(FORMAT(L[4],{outcome=outcome}), L[5])
  end
end

function T.test_missing_unit_decision_is_rejected_before_any_mutation()
  local L=STRINGS["AbsorptionTests.test_missing_unit_decision_is_rejected_before_any_mutation"]
  REJECT(L[1], L[2])
end

function T.test_missing_asset_decision_is_rejected_before_any_mutation()
  local L=STRINGS["AbsorptionTests.test_missing_asset_decision_is_rejected_before_any_mutation"]
  REJECT(L[1], L[2])
end

function T.test_inconsistent_linked_person_and_unit_decisions_are_rejected()
  local L=STRINGS["AbsorptionTests.test_inconsistent_linked_person_and_unit_decisions_are_rejected"]
  REJECT(L[1], L[2])
end

function T.test_coexistence_is_not_absorption()
  local L=STRINGS["AbsorptionTests.test_coexistence_is_not_absorption"]
  for _, mode in pairs({L[1], L[2], L[3], L[4], L[5], L[6]}) do
    REJECT(FORMAT(L[7],{mode=mode}), L[8])
  end
end

function T.test_entry_and_external_relations_require_explicit_resolution()
  local L=STRINGS["AbsorptionTests.test_entry_and_external_relations_require_explicit_resolution"]
  REJECT(L[1], L[2])
end

function T.test_asset_collision_is_rejected_before_engine_changes()
  local L=STRINGS["AbsorptionTests.test_asset_collision_is_rejected_before_engine_changes"]
  REJECT(L[1], L[2])
end

function T.test_invalid_return_is_rejected()
  local L=STRINGS["AbsorptionTests.test_invalid_return_is_rejected"]
  REJECT(L[1], L[2])
end

function ADD_HERO(second)
  local L=STRINGS["AbsorptionTests.add_native_hero"]
  RUN(L[1])
  if second then
    RUN(L[2])
  end
end

function SPLIT_HERO()
  local L=STRINGS["AbsorptionTests.split_hero_contexts"]
  ADD_HERO(true)
  RUN(L[1])
  RUN(READ("poc/JAC_PoC_Absorption/"..L[2]))
  RUN(L[4])
end

function SPLIT_UNIT()
  local L=STRINGS["AbsorptionTests.split_unit_contexts"]
  RUN(L[1])
  RUN(READ("poc/JAC_PoC_Absorption/"..L[2]))
  RUN(L[4])
end

function FINISH()
  local L=STRINGS["AbsorptionTests.finish_spread_publications"]
  local tx
  for _=1,16 do
    RUN(L[1])
    tx = A.Resume(L[2])
    if tx.Status == L[3] then
      return tx
    end
  end
  error(L[4])
end

function T.test_native_clones_stage_at_recipient_city_then_return_to_original_positions()
  local L=STRINGS["AbsorptionTests.test_native_clones_stage_at_recipient_city_then_return_to_original_positions"]
  local tx
  RUN(L[1])
  tx = APPLY()
  EQ(function() return tx.Status end,function() return L[2] end)
  RUN(L[3])
end

function T.test_large_army_batches_publication_requests_instead_of_waiting_per_unit()
  local L=STRINGS["AbsorptionTests.test_large_army_batches_publication_requests_instead_of_waiting_per_unit"]
  local tx,publications
  RUN(L[1])
  SPLIT_UNIT()
  tx = APPLY()
  publications = 0
  while tx.Status ~= L[2] and publications < 4 do
    RUN(L[3])
    tx = A.Resume(L[4])
    publications=publications+1
  end
  EQ(function() return tx.Status end,function() return L[5] end)
  EQ(function() return SPAWNS end,function() return 26 end)
  TRUTH(function() return VERIFY(L[6], true) end,true)
end

function T.test_off_map_native_candidates_are_placed_before_transfer_and_verified()
  local L=STRINGS["AbsorptionTests.test_off_map_native_candidates_are_placed_before_transfer_and_verified"]
  local tx
  RUN(L[1])
  tx = APPLY()
  EQ(function() return tx.Status end,function() return L[2] end)
  EQ(function() return SPAWNS end,function() return 6 end)
  TRUTH(function() return VERIFY(tx.ID, true) end,true)
  RUN(L[3])
  ERROR(function()
    VERIFY(tx.ID, true)
  end, L[4])
end

function T.test_failed_off_map_placement_keeps_donors_and_reuses_saved_candidate_on_resume()
  local L=STRINGS["AbsorptionTests.test_failed_off_map_placement_keeps_donors_and_reuses_saved_candidate_on_resume"]
  local candidate_id,tx,unit
  RUN(L[1])
  ERROR(function()
    APPLY()
  end, L[2])
  candidate_id = A.FindUnit(2, L[3]).GetID(A.FindUnit(2, L[4]))
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(1,0,0) end)
  RUN(L[5])
  tx = A.Resume(L[6])
  EQ(function() return tx.Status end,function() return L[7] end)
  EQ(function() return SPAWNS end,function() return 6 end)
  unit = A.FindUnit(2, L[8])
  EQ(function() return unit.GetID(unit) end,function() return candidate_id end)
  TRUTH(function() return VERIFY(tx.ID, true) end,true)
end

function T.test_empty_army_still_waits_for_city_cache_publication_before_preparation()
  local L=STRINGS["AbsorptionTests.test_empty_army_still_waits_for_city_cache_publication_before_preparation"]
  local tx
  SPLIT_UNIT()
  RUN(L[1])
  tx = APPLY()
  EQ(function() return tx.Status end,function() return L[2] end)
  TRUTH(function() return tx.AwaitingPreparation end,true)
  EQ(function() return TUPLE(SPAWNS,TRANSFERS) end,function() return TUPLE(0,0) end)
  EQ(function() return A.Resume(L[3]).Status end,function() return L[4] end)
  RUN(L[5])
  tx = A.Resume(L[6])
  EQ(function() return tx.Cities[1].Faith[L[7]] end,function() return 5 end)
  EQ(function() return tx.Status end,function() return L[8] end)
  RUN(L[9])
  EQ(function() return A.Resume(L[10]).Status end,function() return L[11] end)
end

function T.test_public_verification_requires_a_new_publication_and_consumes_it()
  local L=STRINGS["AbsorptionTests.test_public_verification_requires_a_new_publication_and_consumes_it"]
  SPLIT_UNIT()
  APPLY()
  FINISH()
  ERROR(function()
    A.Verify(L[2], true)
  end, L[1])
  A.RequestVerification(L[3])
  ERROR(function()
    A.Verify(L[5], true)
  end, L[4])
  RUN(L[6])
  TRUTH(function() return A.Verify(L[7], true) end,true)
  ERROR(function()
    A.Verify(L[9], true)
  end, L[8])
  RUN(L[10])
  A.RequestVerification(L[11])
  RUN(L[12])
  ERROR(function()
    A.Verify(L[14], true)
  end, L[13])
  IS_NIL(function() return A.GetState().Transactions[L[15]].VerificationUI end,true)
end

function T.test_ui_only_purchase_cost_and_faith_balance_return_scalars_to_core()
  local L=STRINGS["AbsorptionTests.test_ui_only_purchase_cost_and_faith_balance_return_scalars_to_core"]
  SPLIT_UNIT()
  RUN(L[1])
  RUN(READ("poc/JAC_PoC_Absorption/"..L[2]))
  RUN(L[4])
  EQ(function() return A.CityPurchaseCost(CITIES[4], 6, 104) end,function() return 1000 end)
  EQ(function() return A.PlayerFaithBalance(2) end,function() return 777 end)
end

function T.test_ui_only_spread_getter_waits_for_publication_and_reuses_candidates()
  local L=STRINGS["AbsorptionTests.test_ui_only_spread_getter_waits_for_publication_and_reuses_candidates"]
  local tx
  SPLIT_UNIT()
  tx = APPLY()
  EQ(function() return tx.Status end,function() return L[1] end)
  TRUTH(function() return tx.AwaitingPreparation end,true)
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(0,0,0) end)
  RUN(L[2])
  tx = A.Resume(L[3])
  EQ(function() return tx.Status end,function() return L[4] end)
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(6,0,0) end)
  RUN(L[5])
  RELOAD()
  EQ(function() return A.Resume(L[6]).Status end,function() return L[7] end)
  RUN(L[8])
  EQ(function() return A.Resume(L[9]).Status end,function() return L[10] end)
  RUN(L[11])
  EQ(function() return A.Resume(L[12]).Status end,function() return L[13] end)
  EQ(function() return TUPLE(SPAWNS,KILLS) end,function() return TUPLE(6,0) end)
  tx = FINISH()
  EQ(function() return tx.Status end,function() return L[14] end)
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(6,5,2) end)
  TRUTH(function() return VERIFY(L[15], true) end,true)
  RUN(L[16])
  ERROR(function()
    VERIFY(L[18], true)
  end, L[17])
end

function T.test_old_turn_ui_spread_cache_waits_before_preparation_and_resumes()
  local L=STRINGS["AbsorptionTests.test_old_turn_ui_spread_cache_waits_before_preparation_and_resumes"]
  local tx
  SPLIT_UNIT()
  RUN(L[1])
  tx = APPLY()
  EQ(function() return tx.Status end,function() return L[2] end)
  TRUTH(function() return tx.AwaitingPreparation end,true)
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(0,0,0) end)
  RUN(L[3])
  RELOAD()
  EQ(function() return A.Resume(L[4]).Status end,function() return L[5] end)
  RUN(L[6])
  EQ(function() return A.Resume(L[7]).Status end,function() return L[8] end)
  EQ(function() return SPAWNS end,function() return 0 end)
  RUN(L[9])
  EQ(function() return A.Resume(L[10]).Status end,function() return L[11] end)
  EQ(function() return SPAWNS end,function() return 6 end)
  EQ(function() return FINISH().Status end,function() return L[12] end)
end

function T.test_rejected_preparation_releases_factions_and_corrected_request_waits_for_new_publication()
  local L=STRINGS["AbsorptionTests.test_rejected_preparation_releases_factions_and_corrected_request_waits_for_new_publication"]
  local tx,previous_token
  SPLIT_UNIT()
  RUN(L[1])
  tx = APPLY()
  previous_token = tx.SourceSpreadReads[10].Token
  TRUTH(function() return tx.AwaitingPreparation end,true)
  RUN(L[2])
  ERROR(function()
    A.Resume(L[4])
  end, L[3])
  IS_NIL(function() return A.GetState().Transactions[L[5]] end,true)
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(0,0,0) end)
  RUN(L[6])
  tx = APPLY()
  TRUTH(function() return tx.AwaitingPreparation end,true)
  NE(function() return tx.SourceSpreadReads[10].Token end,function() return previous_token end)
  EQ(function() return A.Resume(L[7]).Status end,function() return L[8] end)
  EQ(function() return SPAWNS end,function() return 0 end)
  RUN(L[9])
  tx = A.Resume(L[10])
  EQ(function() return tx.Status end,function() return L[11] end)
  EQ(function() return FIND_SPREAD(tx.Units,10) end,function() return 1 end)
end

function T.test_native_hero_ui_only_spread_wait_never_becomes_suicide()
  local L=STRINGS["AbsorptionTests.test_native_hero_ui_only_spread_wait_never_becomes_suicide"]
  local saw_hero_wait,tx,creations
  ADD_HERO()
  SPLIT_UNIT()
  EQ(function() return APPLY().Status end,function() return L[1] end)
  saw_hero_wait = false
  for _=1,20 do
    RUN(L[2])
    tx = A.Resume(L[3])
    if tx.Status == L[4] and HERO_CREATIONS == 1 then
      saw_hero_wait = true
      creations = HERO_CREATIONS
      EQ(function() return A.Resume(L[5]).Status end,function() return L[6] end)
      EQ(function() return HERO_CREATIONS end,function() return creations end)
      NE(function() return tx.HeroResults.native.Outcome end,function() return L[7] end)
      RUN(L[8])
    end
    if tx.Status == L[9] then
      break
    end
  end
  TRUTH(function() return saw_hero_wait end,true)
  EQ(function() return tx.Status end,function() return L[10] end)
  EQ(function() return HERO_CREATIONS end,function() return 1 end)
  EQ(function() return tx.HeroResults.native.Outcome end,function() return L[11] end)
  TRUTH(function() return VERIFY(L[12], true) end,true)
end

function T.test_published_hero_spread_mismatch_records_suicide_instead_of_blocking_other_assets()
  local L=STRINGS["AbsorptionTests.test_published_hero_spread_mismatch_records_suicide_instead_of_blocking_other_assets"]
  local tx,corrupted
  ADD_HERO()
  SPLIT_UNIT()
  tx = APPLY()
  corrupted = false
  for _=1,24 do
    if BOOL(HERO_CREATIONS) and not corrupted then
      RUN(L[1])
      corrupted = true
    end
    RUN(L[2])
    tx = A.Resume(L[3])
    if tx.Status == L[4] then
      break
    end
  end
  TRUTH(function() return corrupted end,true)
  EQ(function() return tx.Status end,function() return L[5] end)
  EQ(function() return tx.HeroResults.native.Outcome end,function() return L[6] end)
  EQ(function() return tx.HeroResults.native.Cause end,function() return L[7] end)
  EQ(function() return HERO_CREATIONS end,function() return 1 end)
  TRUTH(function() return VERIFY(L[8], true) end,true)
end

function T.test_split_ui_getters_and_gameplay_mutations_preserve_native_heroes()
  local L=STRINGS["AbsorptionTests.test_split_ui_getters_and_gameplay_mutations_preserve_native_heroes"]
  local tx,spawns
  SPLIT_HERO()
  tx = APPLY()
  EQ(function() return tx.Status end,function() return L[1] end)
  spawns = SPAWNS
  ERROR(function()
    VERIFY(L[3], true)
  end, L[2])
  RELOAD()
  EQ(function() return A.Resume(L[4]).Status end,function() return L[5] end)
  EQ(function() return SPAWNS end,function() return spawns end)
  for _=1,3 do
    RUN(L[6])
    tx = A.Resume(L[7])
  end
  EQ(function() return tx.Status end,function() return L[8] end)
  RELOAD()
  TRUTH(function() return VERIFY(L[9], true) end,true)
  RUN(L[10])
end

function T.test_fresh_published_hero_mismatch_is_archived_without_accepting_stale_ui_as_success()
  local L=STRINGS["AbsorptionTests.test_fresh_published_hero_mismatch_is_archived_without_accepting_stale_ui_as_success"]
  local tx
  SPLIT_HERO()
  tx = APPLY()
  EQ(function() return tx.Status end,function() return L[1] end)
  RUN(L[2])
  tx = A.Resume(L[3])
  EQ(function() return tx.Status end,function() return L[4] end)
  RUN(L[5])
  tx = A.Resume(L[6])
  EQ(function() return tx.Status end,function() return L[7] end)
  EQ(function() return tx.HeroResults.native.Outcome end,function() return L[8] end)
  EQ(function() return tx.HeroResults.native.Cause end,function() return L[9] end)
  TRUTH(function() return VERIFY(L[10], true) end,true)
end

function T.test_ui_only_city_citizens_and_work_slots_wait_for_final_publication()
  local L=STRINGS["AbsorptionTests.test_ui_only_city_citizens_and_work_slots_wait_for_final_publication"]
  local before,tx
  SPLIT_HERO()
  RUN(L[1])
  before = RUN(L[2])
  tx = APPLY()
  EQ(function() return tx.Status end,function() return L[3] end)
  for _=1,4 do
    RUN(L[4])
    tx = A.Resume(L[5])
  end
  EQ(function() return tx.Status end,function() return L[6] end)
  EQ(function() return RUN(L[7]) end,function() return before end)
  GREATER(function() return UI_CITY_READS end,function() return 0 end)
  TRUTH(function() return VERIFY(L[8], true) end,true)
end

function T.test_only_missing_work_slot_getter_still_requires_final_ui_publication()
  local L=STRINGS["AbsorptionTests.test_only_missing_work_slot_getter_still_requires_final_ui_publication"]
  local tx,request
  RUN(L[1])
  tx = APPLY()
  EQ(function() return tx.Status end,function() return L[2] end)
  TRUTH(function() return tx.AwaitingPreparation end,true)
  UI_READY[tx.SourceReadyToken] = {Published=true}
  tx = A.Resume(L[4])
  TRUTH(function() return tx.AwaitingFinalUI end,true)
  EQ(function() return A.Resume(L[5]).Status end,function() return L[6] end)
  RUN(L[7])
  EQ(function() return A.Resume(L[8]).Status end,function() return L[9] end)
  request = A.RequestVerification(L[10])
  UI_READY[request.Token] = {Published=true}
  TRUTH(function() return A.Verify(L[12], true) end,true)
end

function T.test_native_heroes_keep_identity_spent_charges_renown_origin_and_person_record()
  local L=STRINGS["AbsorptionTests.test_native_heroes_keep_identity_spent_charges_renown_origin_and_person_record"]
  local before
  ADD_HERO(true)
  APPLY()
  EQ(function() return HERO_CREATIONS end,function() return 2 end)
  TRUTH(function() return VERIFY(L[1], true) end,true)
  RUN(L[2])
  before = SNAPSHOT()
  RELOAD()
  TRUTH(function() return VERIFY(L[3], true) end,true)
  APPLY()
  EQ(function() return SNAPSHOT() end,function() return before end)
  EQ(function() return HERO_CREATIONS end,function() return 2 end)
end

function T.test_native_hero_absent_gameplay_manager_becomes_suicide_history()
  local L=STRINGS["AbsorptionTests.test_native_hero_absent_gameplay_manager_becomes_suicide_history"]
  ADD_HERO()
  RUN(L[1])
  SUICIDE(L[2])
end

function T.test_dormant_native_claim_without_unit_is_archived_as_suicide()
  local L=STRINGS["AbsorptionTests.test_dormant_native_claim_without_unit_is_archived_as_suicide"]
  local tx
  RUN(L[1])
  tx = APPLY()
  EQ(function() return tx.Status end,function() return L[2] end)
  RUN(L[3])
end

function T.test_dormant_claim_with_stale_person_reference_preserves_requested_outcome()
  local L=STRINGS["AbsorptionTests.test_dormant_claim_with_stale_person_reference_preserves_requested_outcome"]
  RUN(L[1])
  APPLY()
  RUN(L[2])
end

function T.test_native_hero_missing_origin_city_becomes_suicide()
  local L=STRINGS["AbsorptionTests.test_native_hero_missing_origin_city_becomes_suicide"]
  ADD_HERO()
  RUN(L[1])
  SUICIDE(L[2])
end

function T.test_native_hero_missing_origin_building_becomes_suicide()
  local L=STRINGS["AbsorptionTests.test_native_hero_missing_origin_building_becomes_suicide"]
  ADD_HERO()
  RUN(L[1])
  SUICIDE(L[2])
end

function T.test_native_heroes_sharing_origin_archive_the_second_claim()
  local L=STRINGS["AbsorptionTests.test_native_heroes_sharing_origin_archive_the_second_claim"]
  ADD_HERO(true)
  RUN(L[1])
  APPLY()
  RUN(L[2])
end

function T.test_native_hero_no_spawn_after_claim_change_finishes_as_suicide()
  local L=STRINGS["AbsorptionTests.test_native_hero_no_spawn_after_claim_change_finishes_as_suicide"]
  ADD_HERO()
  HERO_NO_SPAWN = true
  SUICIDE(L[1])
  EQ(function() return HERO_CREATIONS end,function() return 1 end)
  EQ(function() return HERO_CLAIMS[0] end,function() return 2 end)
  EQ(function() return A.GetState().Archive.native.NativeClaimAfter end,function() return 2 end)
end

function T.test_native_hero_init_throw_after_creation_adopts_candidate_without_recreating()
  local L=STRINGS["AbsorptionTests.test_native_hero_init_throw_after_creation_adopts_candidate_without_recreating"]
  ADD_HERO()
  HERO_THROW_AFTER_SPAWN = true
  APPLY()
  TRUTH(function() return VERIFY(L[1], true) end,true)
  EQ(function() return HERO_CREATIONS end,function() return 1 end)
end

function T.test_native_hero_token_write_failure_kills_only_its_candidate()
  local L=STRINGS["AbsorptionTests.test_native_hero_token_write_failure_kills_only_its_candidate"]
  ADD_HERO()
  RUN(L[1])
  SUICIDE(L[2])
  EQ(function() return HERO_CREATIONS end,function() return 1 end)
  RUN(L[3])
end

function T.test_native_hero_wrong_origin_is_suicide_without_duplicate()
  local L=STRINGS["AbsorptionTests.test_native_hero_wrong_origin_is_suicide_without_duplicate"]
  ADD_HERO()
  HERO_WRONG_ORIGIN = true
  SUICIDE(L[1])
  EQ(function() return HERO_CREATIONS end,function() return 1 end)
end

function T.test_native_hero_city_failure_can_resume_from_escrow_without_second_kill()
  local L=STRINGS["AbsorptionTests.test_native_hero_city_failure_can_resume_from_escrow_without_second_kill"]
  ADD_HERO()
  FAIL_CITY = true
  ERROR(function()
    APPLY()
  end, L[1])
  EQ(function() return TUPLE(KILLS,HERO_CREATIONS) end,function() return TUPLE(1,0) end)
  FAIL_CITY = false
  RELOAD()
  A.Resume(L[2])
  TRUTH(function() return VERIFY(L[3], true) end,true)
  EQ(function() return TUPLE(KILLS,HERO_CREATIONS) end,function() return TUPLE(6,1) end)
end

function T.test_native_hero_manager_mismatch_is_not_reported_as_inheritance()
  local L=STRINGS["AbsorptionTests.test_native_hero_manager_mismatch_is_not_reported_as_inheritance"]
  ADD_HERO()
  HERO_WRONG_OWNER = true
  SUICIDE(L[1])
  EQ(function() return HERO_CREATIONS end,function() return 1 end)
end

function T.test_hero_creation_error_does_not_kill_a_third_party_unit()
  local L=STRINGS["AbsorptionTests.test_hero_creation_error_does_not_kill_a_third_party_unit"]
  ADD_HERO()
  RUN(L[1])
  SUICIDE(L[2])
  RUN(L[3])
end

function T.test_reused_donor_id_blocks_without_killing_an_unrelated_unit()
  local L=STRINGS["AbsorptionTests.test_reused_donor_id_blocks_without_killing_an_unrelated_unit"]
  ADD_HERO()
  RUN(L[1])
  ERROR(function()
    APPLY()
  end, L[2])
  RUN(L[3])
end

function T.test_native_hero_restore_error_becomes_suicide_without_losing_other_assets()
  local L=STRINGS["AbsorptionTests.test_native_hero_restore_error_becomes_suicide_without_losing_other_assets"]
  ADD_HERO()
  RUN(L[1])
  SUICIDE(L[2])
  RUN(L[3])
end

function T.test_native_hero_recall_binds_new_id_and_person_after_reload()
  local L=STRINGS["AbsorptionTests.test_native_hero_recall_binds_new_id_and_person_after_reload"]
  local record
  ADD_HERO()
  APPLY()
  RUN(L[1])
  RELOAD()
  ERROR(function()
    A.CompleteHeroRecall(L[3])
  end, L[2])
  RUN(L[4])
  record = A.CompleteHeroRecall(L[5])
  NE(function() return record.Unit.ID end,function() return OLD_HERO_ID end)
  RUN(L[6])
  RELOAD()
  A.CompleteHeroRecall(L[7])
  TRUTH(function() return VERIFY(L[8], false) end,true)
  EQ(function() return PY_LEN(A.GetRoster(2).native.RecallHistory) end,function() return 1 end)
end

function T.test_recall_after_second_absorption_preserves_original_owner_and_uses_current_mapping()
  local L=STRINGS["AbsorptionTests.test_recall_after_second_absorption_preserves_original_owner_and_uses_current_mapping"]
  ADD_HERO()
  APPLY()
  RUN(L[1])
  RELOAD()
  A.BeginHeroRecall(L[2], L[3], L[4])
  RUN(L[5])
  A.CompleteHeroRecall(L[6])
  RUN(L[7])
  RELOAD()
  A.CompleteHeroRecall(L[8])
  EQ(function() return PY_LEN(A.GetRoster(3).native.RecallHistory) end,function() return 1 end)
end

function T.test_recall_rejects_candidate_and_token_collisions_without_killing_them()
  local L=STRINGS["AbsorptionTests.test_recall_rejects_candidate_and_token_collisions_without_killing_them"]
  ADD_HERO()
  APPLY()
  RUN(L[1])
  ERROR(function()
    A.CompleteHeroRecall(L[3])
  end, L[2])
  RUN(L[4])
  ERROR(function()
    A.CompleteHeroRecall(L[6])
  end, L[5])
  RUN(L[7])
end

function T.test_recall_resumes_after_token_write_and_does_not_duplicate_history()
  local L=STRINGS["AbsorptionTests.test_recall_resumes_after_token_write_and_does_not_duplicate_history"]
  local record
  ADD_HERO()
  APPLY()
  RUN(L[1])
  ERROR(function()
    A.CompleteHeroRecall(L[3])
  end, L[2])
  RELOAD()
  record = A.CompleteHeroRecall(L[4])
  EQ(function() return PY_LEN(record.RecallHistory) end,function() return 1 end)
  EQ(function() return record.Unit.Token end,function() return L[5] end)
  EQ(function() return PY_LEN(A.CompleteHeroRecall(L[6]).RecallHistory) end,function() return 1 end)
  EQ(function() return HERO_CREATIONS end,function() return 2 end)
end

function T.test_native_lifespan_changes_after_turn_but_owner_recall_identity_must_remain()
  local L=STRINGS["AbsorptionTests.test_native_lifespan_changes_after_turn_but_owner_recall_identity_must_remain"]
  ADD_HERO()
  APPLY()
  CURRENT_TURN = 6
  RUN(L[1])
  RELOAD()
  TRUTH(function() return VERIFY(L[2], false) end,true)
  ERROR(function()
    VERIFY(L[4], true)
  end, L[3])
  RUN(L[5])
  ERROR(function()
    VERIFY(L[7], false)
  end, L[6])
end

function T.test_ui_publishes_engine_enum_when_gameplay_has_no_unit_property_keys()
  local L=STRINGS["AbsorptionTests.test_ui_publishes_engine_enum_when_gameplay_has_no_unit_property_keys"]
  local ui
  ADD_HERO()
  ui=READ("poc/JAC_PoC_Absorption/UI/JAC_Absorption_HeroKeys.lua")
  RUN(L[3])
  RUN(ui)
  EQ(function() return ExposedMembers.JAC_HeroLifespanKey end,function() return -1318296071 end)
  RUN(L[4])
end

function T.test_shipped_lifespan_property_is_used_without_legacy_lifespan_or_renown_methods()
  local L=STRINGS["AbsorptionTests.test_shipped_lifespan_property_is_used_without_legacy_lifespan_or_renown_methods"]
  ADD_HERO()
  RUN(L[1])
  APPLY()
  RUN(L[2])
  TRUTH(function() return VERIFY(L[3], true) end,true)
end

function T.test_native_hero_position_is_restored_separately_from_its_recall_origin()
  local L=STRINGS["AbsorptionTests.test_native_hero_position_is_restored_separately_from_its_recall_origin"]
  ADD_HERO()
  RUN(L[1])
  APPLY()
  RUN(L[2])
  TRUTH(function() return VERIFY(L[3], true) end,true)
end

function T.test_native_hero_unrestorable_exposed_renown_becomes_suicide()
  local L=STRINGS["AbsorptionTests.test_native_hero_unrestorable_exposed_renown_becomes_suicide"]
  ADD_HERO()
  RUN(L[1])
  SUICIDE(L[2])
end

function T.test_native_hero_is_not_escrowed_when_an_ordinary_unit_cannot_preserve_spent_charges()
  local L=STRINGS["AbsorptionTests.test_native_hero_is_not_escrowed_when_an_ordinary_unit_cannot_preserve_spent_charges"]
  ADD_HERO()
  RUN(L[1])
  ERROR(function()
    APPLY()
  end, L[2])
  EQ(function() return TUPLE(HERO_CREATIONS,KILLS,TRANSFERS) end,function() return TUPLE(0,0,0) end)
  IS_NIL(function() return Players[1].units[13] end,false)
end

function T.test_native_hero_duplicate_at_absorber_survives_suicide()
  local L=STRINGS["AbsorptionTests.test_native_hero_duplicate_at_absorber_survives_suicide"]
  ADD_HERO()
  RUN(L[1])
  SUICIDE(L[2])
  RUN(L[3])
end

function T.test_native_hero_city_transfer_throw_after_mutation_resumes_with_the_new_city_id()
  local L=STRINGS["AbsorptionTests.test_native_hero_city_transfer_throw_after_mutation_resumes_with_the_new_city_id"]
  ADD_HERO()
  RUN(L[1])
  ERROR(function()
    APPLY()
  end, L[2])
  RELOAD()
  A.Resume(L[3])
  TRUTH(function() return VERIFY(L[4], true) end,true)
  EQ(function() return TUPLE(TRANSFERS,HERO_CREATIONS) end,function() return TUPLE(2,1) end)
end

function T.test_native_hero_token_set_then_throw_becomes_suicide()
  local L=STRINGS["AbsorptionTests.test_native_hero_token_set_then_throw_becomes_suicide"]
  ADD_HERO()
  RUN(L[1])
  SUICIDE(L[2])
  EQ(function() return HERO_CREATIONS end,function() return 1 end)
end

function T.test_engine_spawn_failure_preserves_every_donor_unit_and_city_then_resume_is_idempotent()
  local L=STRINGS["AbsorptionTests.test_engine_spawn_failure_preserves_every_donor_unit_and_city_then_resume_is_idempotent"]
  FAIL_SPAWN = 2
  ERROR(function()
    APPLY()
  end, L[1])
  EQ(function() return TUPLE(KILLS,TRANSFERS) end,function() return TUPLE(0,0) end)
  EQ(function() return A.GetState().Transactions[L[2]].Status end,function() return L[3] end)
  FAIL_SPAWN = nil
  RELOAD()
  A.Resume(L[4])
  TRUTH(function() return VERIFY(L[5], true) end,true)
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(7,5,2) end)
end

function T.test_property_copy_failure_is_blocked_before_removing_any_donor()
  local L=STRINGS["AbsorptionTests.test_property_copy_failure_is_blocked_before_removing_any_donor"]
  FAIL_COPY = true
  ERROR(function()
    APPLY()
  end, L[1])
  EQ(function() return TUPLE(KILLS,TRANSFERS) end,function() return TUPLE(0,0) end)
  FAIL_COPY = false
  A.Resume(L[2])
  EQ(function() return SPAWNS end,function() return 6 end)
end

function T.test_spent_build_charges_need_an_adapter_and_never_destroy_the_donor()
  local L=STRINGS["AbsorptionTests.test_spent_build_charges_need_an_adapter_and_never_destroy_the_donor"]
  RUN(L[1])
  ERROR(function()
    APPLY()
  end, L[2])
  EQ(function() return TUPLE(KILLS,TRANSFERS) end,function() return TUPLE(0,0) end)
  IS_NIL(function() return Players[1].units[13] end,false)
end

function T.test_city_failure_retains_snapshots_and_resume_does_not_recreate_units()
  local L=STRINGS["AbsorptionTests.test_city_failure_retains_snapshots_and_resume_does_not_recreate_units"]
  FAIL_CITY = true
  ERROR(function()
    APPLY()
  end, L[1])
  EQ(function() return SPAWNS end,function() return 6 end)
  EQ(function() return PY_LEN(A.GetState().Transactions[L[2]].Cities) end,function() return 2 end)
  FAIL_CITY = false
  A.Resume(L[3])
  EQ(function() return SPAWNS end,function() return 6 end)
  TRUTH(function() return VERIFY(L[4], true) end,true)
end

function T.test_later_turn_resume_is_rejected_without_reapplying_old_damage_or_movement()
  local L=STRINGS["AbsorptionTests.test_later_turn_resume_is_rejected_without_reapplying_old_damage_or_movement"]
  local before
  FAIL_CITY = true
  ERROR(function()
    APPLY()
  end)
  CURRENT_TURN = 6
  before = SNAPSHOT()
  ERROR(function()
    A.Resume(L[2])
  end, L[1])
  EQ(function() return SNAPSHOT() end,function() return before end)
end

function T.test_same_transaction_id_with_changed_decisions_is_rejected()
  local L=STRINGS["AbsorptionTests.test_same_transaction_id_with_changed_decisions_is_rejected"]
  APPLY()
  RUN(L[1])
  ERROR(function()
    APPLY()
  end, L[2])
end

function T.test_verification_catches_missing_or_corrupted_transfers_instead_of_only_counting()
  local L=STRINGS["AbsorptionTests.test_verification_catches_missing_or_corrupted_transfers_instead_of_only_counting"]
  APPLY()
  RUN(L[1])
  ERROR(function()
    VERIFY(L[3], true)
  end, L[2])
end

function T.test_completed_transaction_is_not_replayed_after_later_turns()
  local L=STRINGS["AbsorptionTests.test_completed_transaction_is_not_replayed_after_later_turns"]
  APPLY()
  CURRENT_TURN = 6
  RUN(L[1])
  APPLY()
  TRUTH(function() return VERIFY(L[2], false) end,true)
  EQ(function() return SPAWNS end,function() return 6 end)
end

function T.test_registered_unit_and_city_properties_are_preserved_and_unsupported_values_rejected()
  local L=STRINGS["AbsorptionTests.test_registered_unit_and_city_properties_are_preserved_and_unsupported_values_rejected"]
  A.RegisterUnitProperty(L[1])
  A.RegisterCityProperty(L[2])
  RUN(L[3])
  APPLY()
  RUN(L[4])
end

function T.test_adapter_preflight_apply_verify_and_reload_contract()
  local L=STRINGS["AbsorptionTests.test_adapter_preflight_apply_verify_and_reload_contract"]
  RUN(L[1])
  APPLY()
  EQ(function() return ADAPTER_APPLIED end,function() return 1 end)
  RELOAD()
  ERROR(function()
    VERIFY(L[3], true)
  end, L[2])
end

function T.test_bad_adapter_and_empty_string_fail_before_native_mutation()
  local L=STRINGS["AbsorptionTests.test_bad_adapter_and_empty_string_fail_before_native_mutation"]
  RUN(L[1])
  ERROR(function()
    APPLY()
  end, L[2])
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(0,0,0) end)
end

function REGISTER_MISSION()
  local L=STRINGS["AbsorptionTests.register_mission_rebind"]
  RUN(L[1])
end

function T.test_foreign_reference_rebind_keeps_target_deadline_and_fixed_roll_and_survives_reload()
  local L=STRINGS["AbsorptionTests.test_foreign_reference_rebind_keeps_target_deadline_and_fixed_roll_and_survives_reload"]
  REGISTER_MISSION()
  APPLY()
  RUN(L[1])
  RELOAD()
  TRUTH(function() return VERIFY(L[2], true) end,true)
  APPLY()
  EQ(function() return REBIND_CALLS end,function() return 1 end)
end

function T.test_saved_rebound_expectation_is_used_on_resume_without_reloading_callback()
  local L=STRINGS["AbsorptionTests.test_saved_rebound_expectation_is_used_on_resume_without_reloading_callback"]
  REGISTER_MISSION()
  FAIL_COPY = true
  ERROR(function()
    APPLY()
  end, L[1])
  EQ(function() return TUPLE(KILLS,TRANSFERS) end,function() return TUPLE(0,0) end)
  RELOAD()
  FAIL_COPY = false
  A.Resume(L[2])
  TRUTH(function() return VERIFY(L[3], true) end,true)
  RUN(L[4])
  EQ(function() return REBIND_CALLS end,function() return 1 end)
end

function T.test_corrupted_rebound_reference_is_detected_by_generic_verification()
  local L=STRINGS["AbsorptionTests.test_corrupted_rebound_reference_is_detected_by_generic_verification"]
  REGISTER_MISSION()
  APPLY()
  RUN(L[1])
  ERROR(function()
    VERIFY(L[3], true)
  end, L[2])
end

function T.test_city_property_rebind_uses_saved_expectations_on_reload()
  local L=STRINGS["AbsorptionTests.test_city_property_rebind_uses_saved_expectations_on_reload"]
  RUN(L[1])
  APPLY()
  EQ(function() return CITIES[4].props.FOREIGN_CITY_OWNER.Custodian end,function() return 2 end)
  RELOAD()
  TRUTH(function() return VERIFY(L[2], true) end,true)
end

function T.test_nonserializable_property_rebind_blocks_before_destroying_donors()
  local L=STRINGS["AbsorptionTests.test_nonserializable_property_rebind_blocks_before_destroying_donors"]
  RUN(L[1])
  ERROR(function()
    APPLY()
  end, L[2])
  EQ(function() return TUPLE(KILLS,TRANSFERS) end,function() return TUPLE(0,0) end)
end

function T.test_native_hero_detected_by_engine_flag_with_a_nonstandard_type_is_rejected()
  local L=STRINGS["AbsorptionTests.test_native_hero_detected_by_engine_flag_with_a_nonstandard_type_is_rejected"]
  REJECT(L[1], L[2])
end

function T.test_missing_native_great_person_charge_api_blocks_before_any_mutation()
  local L=STRINGS["AbsorptionTests.test_missing_native_great_person_charge_api_blocks_before_any_mutation"]
  REJECT( L[1], L[2])
end

function T.test_spent_native_great_person_charges_do_not_reset_on_transfer()
  local L=STRINGS["AbsorptionTests.test_spent_native_great_person_charges_do_not_reset_on_transfer"]
  RUN(L[1])
  ERROR(function()
    APPLY()
  end, L[2])
  EQ(function() return TUPLE(KILLS,TRANSFERS) end,function() return TUPLE(0,0) end)
  EQ(function() return Players[1].units[8].gpcharges end,function() return 1 end)
end

function T.test_missing_staged_replacement_blocks_instead_of_breaking_saved_references()
  local L=STRINGS["AbsorptionTests.test_missing_staged_replacement_blocks_instead_of_breaking_saved_references"]
  local before
  REGISTER_MISSION()
  FAIL_COPY = true
  ERROR(function()
    APPLY()
  end, L[1])
  RUN(L[2])
  FAIL_COPY = false
  before = TUPLE(SPAWNS, KILLS, TRANSFERS)
  ERROR(function()
    A.Resume(L[4])
  end, L[3])
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return before end)
end

function T.test_live_scenario_builds_missing_hero_origins_on_their_actual_city_plots()
  local L=STRINGS["AbsorptionTests.test_live_scenario_builds_missing_hero_origins_on_their_actual_city_plots"]
  local steps
  steps = LIVE()
  RUN(L[1])
  RUN(steps[1][L[2]])
  EQ(function() return BUILDING_CREATIONS end,function() return 2 end)
  TRUTH(function() return CITIES[4].monument end,true)
  TRUTH(function() return CITIES[8].monument end,true)
end

function T.test_live_scenario_checks_execute_in_lua_and_detect_asset_corruption_after_reload()
  local L=STRINGS["AbsorptionTests.test_live_scenario_checks_execute_in_lua_and_detect_asset_corruption_after_reload"]
  local steps,before,after
  steps = LIVE()
  RUN(steps[1][L[1]])
  RUN(steps[2][L[2]])
  before = RUN(steps[3][L[3]])
  RELOAD()
  EQ(function() return RUN(steps[4][L[4]]) end,function() return before end)
  CURRENT_TURN = 6
  RUN(L[5])
  after = RUN(steps[5][L[6]])
  RELOAD()
  EQ(function() return RUN(steps[6][L[7]]) end,function() return after end)
  RUN(L[8])
  ERROR(function()
    RUN(steps[6][L[10]])
  end, L[9])
  RUN(L[11])
  for _, asset in pairs({L[12], L[13]}) do
    MISSING_LIVE_ASSET = asset
    RUN(L[14])
    ERROR(function()
      RUN(steps[6][L[16]])
    end, L[15])
    RUN(L[17])
  end
  RUN(L[18])
  ERROR(function()
    RUN(steps[6][L[20]])
  end, L[19])
end

function T.test_live_transfer_rejects_missing_cleric_before_ai_turn()
  local L=STRINGS["AbsorptionTests.test_live_transfer_rejects_missing_cleric_before_ai_turn"]
  local steps
  steps = LIVE()
  RUN(steps[1][L[1]])
  RUN(steps[2][L[2]])
  RUN(L[3])
  ERROR(function()
    RUN(steps[3][L[5]])
  end, L[4])
end

function T.test_live_ai_turn_allows_cleric_consumption_but_checks_current_save_snapshot()
  local L=STRINGS["AbsorptionTests.test_live_ai_turn_allows_cleric_consumption_but_checks_current_save_snapshot"]
  local steps,after
  steps = LIVE()
  RUN(steps[1][L[1]])
  RUN(steps[2][L[2]])
  RUN(steps[3][L[3]])
  CURRENT_TURN = 6
  RUN(L[4])
  after = RUN(steps[5][L[5]])
  RELOAD()
  EQ(function() return RUN(steps[6][L[6]]) end,function() return after end)
  RUN(L[7])
  NE(function() return RUN(steps[6][L[8]]) end,function() return after end)
  RUN(L[9])
  ERROR(function()
    RUN(steps[6][L[11]])
  end, L[10])
end

function T.test_live_recall_probes_execute_native_eligibility_checks_and_detect_wrong_owner()
  local L=STRINGS["AbsorptionTests.test_live_recall_probes_execute_native_eligibility_checks_and_detect_wrong_owner"]
  local steps,data,setup,checks,death,observed,begin,purchase,payment,complete,proof,capture,saved
  steps = LIVE()
  RUN(steps[1][L[1]])
  RUN(steps[2][L[2]])
  RUN(L[3])
  data=JSON(READ(L[4]))
  setup=SELECT_CODES(data.steps,L[8])
  checks=SELECT_CODES(data.steps,L[13])
  EQ(function() return COUNT(setup) end,function() return 2 end)
  EQ(function() return COUNT(checks) end,function() return 2 end)
  RUN(setup[1])
  RUN(L[16])
  EQ(function() return RECALL_QUERIES end,function() return 0 end)
  ERROR(function()
    RUN(checks[1])
  end, L[17])
  RUN(L[18])
  RUN(checks[1])
  EQ(function() return RECALL_QUERIES end,function() return 4 end)
  RUN(L[19])
  RUN(setup[1])
  RUN(L[20])
  ERROR(function()
    RUN(checks[1])
  end, L[21])
  RUN(L[22])
  death=FIRST_CODE(data.steps,L[25])
  RUN(death)
  RUN(setup[2])
  RUN(L[28])
  RUN(checks[2])
  observed = JAC_ABSORPTION_AI_PROBE.Results[1]
  TRUTH(function() return observed.Visible end,true)
  TRUTH(function() return observed.Allowed end,false)
  EQ(function() return observed.Alive end,function() return 0 end)
  RUN(L[29])
  ERROR(function()
    RUN(checks[2])
  end, L[30])
  GLOBAL_ERA = 1
  RUN(setup[2])
  RUN(L[31])
  TRUTH(function() return JAC_ABSORPTION_AI_PROBE.Results[1].Allowed end,true)
  ERROR(function()
    RUN(checks[2])
  end, L[32])
  RUN(L[33])
  RUN(setup[2])
  RUN(L[34])
  ERROR(function()
    RUN(checks[2])
  end, L[35])
  begin=FIRST_CODE(data.steps,L[38])
  purchase=FIRST_CODE(data.steps,L[43])
  payment=FIRST_CODE(data.steps,L[48])
  complete=FIRST_CODE(data.steps,L[53])
  RUN(L[56])
  RUN(begin)
  RUN(L[57])
  RUN(purchase)
  RUN(L[58])
  EQ(function() return PURCHASE_REQUESTS end,function() return 0 end)
  ERROR(function()
    RUN(payment)
  end, L[59])
  RUN(L[60])
  EQ(function() return PURCHASE_REQUESTS end,function() return 0 end)
  ERROR(function()
    RUN(payment)
  end, L[61])
  RUN(L[62])
  RUN(purchase)
  RUN(L[63])
  EQ(function() return PURCHASE_REQUESTS end,function() return 1 end)
  TRUTH(function() return BOOL(JAC_ABSORPTION_AI_PURCHASE.Complete) end,false)
  RUN(L[64])
  EQ(function() return PURCHASE_REQUESTS end,function() return 1 end)
  proof = JAC_ABSORPTION_AI_PURCHASE
  TRUTH(function() return proof.Complete end,true)
  EQ(function() return PY_SUB(proof.FaithBefore,proof.FaithAfter) end,function() return 1000 end)
  EQ(function() return proof.CostAfter end,function() return 1600 end)
  RUN(payment)
  RUN(complete)
  EQ(function() return A.GetRoster(2)[L[65]].Unit.ID end,function() return proof.NewID end)
  capture=FIRST_CAPTURE_STEP(data.steps,L[69]).code
  saved = RUN(capture)
  RUN(L[70])
  RUN(L[71])
  ERROR(function()
    RUN(capture)
  end, L[72])
  RUN(L[73])
  RUN(L[74])
  ERROR(function()
    RUN(capture)
  end, L[75])
  RUN(L[76])
  EQ(function() return RUN(capture) end,function() return saved end)
  RUN(L[77])
  NE(function() return RUN(capture) end,function() return saved end)
  RUN(L[78])
  RUN(L[79])
  ERROR(function()
    RUN(payment)
  end, L[80])
end

function T.test_human_receiver_scenario_executes_real_recall_and_rebinds_person_in_lua_vm()
  local L=STRINGS["AbsorptionTests.test_human_receiver_scenario_executes_real_recall_and_rebinds_person_in_lua_vm"]
  local steps,current_steps,funding,preparation,published,all_steps,binding,recall_owner
  LIVE()
  steps=JSON(READ(L[1])).steps
  steps=HUMAN_FILTER(steps)
  RUN(L[19])
  RUN(steps[6][L[20]])
  RUN(steps[7][L[21]])
  RELOAD()
  RUN(steps[12][L[22]])
  CURRENT_TURN = 6
  RUN(L[23])
  RUN(steps[15][L[24]])
  RELOAD()
  ERROR(function()
    RUN(steps[16][L[26]])
  end, L[25])
  GLOBAL_ERA = 1
  RUN(steps[16][L[27]])
  current_steps=JSON(READ(L[28])).steps
  funding=FIRST_CONDITION_STEP(current_steps,L[31])
  preparation=FIRST_CAPTURE_STEP(current_steps,L[36])
  published=FIRST_CONDITION_STEP(current_steps,L[37])
  TRUTH(function() return RUN(funding[L[41]][L[42]]) end,true)
  RUN(preparation[L[43]])
  TRUTH(function() return RUN(published[L[44]][L[45]]) end,true)
  RUN(L[46])
  TRUTH(function() return RUN(published[L[47]][L[48]]) end,false)
  ERROR(function()
    RUN(steps[17][L[50]])
  end, L[49])
  RUN(preparation[L[51]])
  RUN(L[52])
  RUN(steps[17][L[53]])
  RUN(L[54])
  all_steps=JSON(READ(L[55])).steps
  binding=FIRST_CAPTURE_STEP(all_steps,L[59])
  RUN(L[60])
  RUN(binding[L[61]])
  RUN(L[62])
  RUN(L[63])
  RUN(steps[19][L[64]])
  RELOAD()
  RUN(steps[24][L[65]])
  RUN(L[66])
  recall_owner = RUN(L[67])
  for _, invalid_balance in pairs({999500, 998500}) do
    FAITH[recall_owner] = invalid_balance
    ERROR(function()
      RUN(steps[19][L[69]])
    end, L[68])
  end
  FAITH[recall_owner] = 999000
end

function T.test_partial_expected_properties_freeze_references_to_every_staged_unit()
  local L=STRINGS["AbsorptionTests.test_partial_expected_properties_freeze_references_to_every_staged_unit"]
  local before
  RUN(L[1])
  ERROR(function()
    APPLY()
  end, L[2])
  RUN(L[3])
  before = TUPLE(SPAWNS, KILLS, TRANSFERS)
  ERROR(function()
    A.Resume(L[5])
  end, L[4])
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return before end)
end

function T.test_receiver_assets_added_or_updated_while_blocked_survive_resume()
  local L=STRINGS["AbsorptionTests.test_receiver_assets_added_or_updated_while_blocked_survive_resume"]
  local roster
  FAIL_CITY = true
  ERROR(function()
    APPLY()
  end, L[1])
  RUN(L[2])
  FAIL_CITY = false
  A.Resume(L[3])
  roster = A.GetRoster(2)
  EQ(function() return roster.existing.Level end,function() return 9 end)
  EQ(function() return roster.new.Owner end,function() return 2 end)
  TRUTH(function() return VERIFY(L[4], true) end,true)
end

function T.test_receiver_assets_added_by_city_transfer_hooks_are_not_overwritten()
  local L=STRINGS["AbsorptionTests.test_receiver_assets_added_by_city_transfer_hooks_are_not_overwritten"]
  RUN(L[1])
  APPLY()
  EQ(function() return A.GetRoster(2)[L[2]].Owner end,function() return 2 end)
  EQ(function() return A.GetRoster(2)[L[3]].Owner end,function() return 2 end)
end

function T.test_new_destination_id_collision_is_stopped_before_removing_donors()
  local L=STRINGS["AbsorptionTests.test_new_destination_id_collision_is_stopped_before_removing_donors"]
  FAIL_COPY = true
  ERROR(function()
    APPLY()
  end, L[1])
  RUN(L[2])
  FAIL_COPY = false
  ERROR(function()
    A.Resume(L[4])
  end, L[3])
  EQ(function() return TUPLE(KILLS,TRANSFERS) end,function() return TUPLE(0,0) end)
end

function T.test_source_assets_created_while_blocked_are_preserved_instead_of_cleared()
  local L=STRINGS["AbsorptionTests.test_source_assets_created_while_blocked_are_preserved_instead_of_cleared"]
  FAIL_COPY = true
  ERROR(function()
    APPLY()
  end, L[1])
  RUN(L[2])
  FAIL_COPY = false
  ERROR(function()
    A.Resume(L[4])
  end, L[3])
  EQ(function() return A.GetRoster(1).new.Owner end,function() return 1 end)
  EQ(function() return TUPLE(KILLS,TRANSFERS) end,function() return TUPLE(0,0) end)
end

function T.test_unsupported_charge_types_stop_before_source_assets_are_changed()
  local L=STRINGS["AbsorptionTests.test_unsupported_charge_types_stop_before_source_assets_are_changed"]
  for _, counter in pairs({L[1], L[2], L[3], L[4]}) do
    UNSUPPORTED_COUNTER = counter
    RUN(L[5])
    ERROR(function()
      APPLY()
    end, L[6]..counter)
    EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(0,0,0) end)
    EQ(function() return Players[1].units[10].owner end,function() return 1 end)
    RUN(L[7])
  end
end

function T.test_runtime_special_charge_without_definition_is_also_rejected()
  local L=STRINGS["AbsorptionTests.test_runtime_special_charge_without_definition_is_also_rejected"]
  RUN(L[1])
  ERROR(function()
    APPLY()
  end, L[2])
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(0,0,0) end)
  EQ(function() return Players[1].units[10].owner end,function() return 1 end)
end

function T.test_spent_park_charge_cannot_be_reset_by_absorption()
  local L=STRINGS["AbsorptionTests.test_spent_park_charge_cannot_be_reset_by_absorption"]
  local source
  RUN(L[1])
  ERROR(function()
    APPLY()
  end, L[2])
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(0,0,0) end)
  source = Players[1].units[3]
  EQ(function() return source.GetParkCharges(source) end,function() return 1 end)
end

function T.test_nonhero_action_charges_declared_in_mode_table_are_rejected()
  local L=STRINGS["AbsorptionTests.test_nonhero_action_charges_declared_in_mode_table_are_rejected"]
  RUN(L[1])
  ERROR(function()
    APPLY()
  end, L[2])
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(0,0,0) end)
end

function CANDIDATE(counter)
  local L=STRINGS["AbsorptionTests.assert_candidate_counter_blocks_and_resumes"]
  local candidate,candidate_id
  RUN(L[1])
  ERROR(function()
    APPLY()
  end, L[2]..counter)
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(6,0,0) end)
  EQ(function() return Players[1].units[3].owner end,function() return 1 end)
  candidate = A.FindUnit(2, L[3])
  candidate_id = candidate.GetID(candidate)
  RELOAD()
  ERROR(function()
    A.Resume(L[5])
  end, L[4]..counter)
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(6,0,0) end)
  CANDIDATE_CHARGES = 0
  A.Resume(L[6])
  candidate = A.FindUnit(2, L[7])
  EQ(function() return candidate.GetID(candidate) end,function() return candidate_id end)
  EQ(function() return SPAWNS end,function() return 6 end)
  TRUTH(function() return VERIFY(L[8], true) end,true)
end

function T.test_city_district_loss_is_detected_with_buildings_and_territory_intact()
  local L=STRINGS["AbsorptionTests.test_city_district_loss_is_detected_with_buildings_and_territory_intact"]
  RUN(L[1])
  APPLY()
  TRUTH(function() return VERIFY(L[2], true) end,true)
  RUN(L[3])
  ERROR(function()
    VERIFY(L[5], true)
  end, L[4])
  ERROR(function()
    VERIFY(L[7], false)
  end, L[6])
  RUN(L[8])
  ERROR(function()
    VERIFY(L[10], false)
  end, L[9])
  RUN(L[11])
  RELOAD()
  TRUTH(function() return VERIFY(L[12], true) end,true)
end

function T.test_legacy_city_assets_keep_recorded_checks_without_inventing_district_coverage()
  local L=STRINGS["AbsorptionTests.test_legacy_city_assets_keep_recorded_checks_without_inventing_district_coverage"]
  APPLY()
  RUN(L[1])
  RELOAD()
  TRUTH(function() return VERIFY(L[2], true) end,true)
  RUN(L[3])
  ERROR(function()
    VERIFY(L[5], true)
  end, L[4])
  RUN(L[6])
  TRUTH(function() return VERIFY(L[7], true) end,true)
  RUN(L[8])
  ERROR(function()
    VERIFY(L[10], true)
  end, L[9])
end

function T.test_new_city_asset_coverage_rejects_missing_district_snapshot()
  local L=STRINGS["AbsorptionTests.test_new_city_asset_coverage_rejects_missing_district_snapshot"]
  APPLY()
  RUN(L[1])
  ERROR(function()
    VERIFY(L[3], true)
  end, L[2])
end

function T.test_multiple_same_type_districts_and_ui_only_enumeration_are_preserved()
  local L=STRINGS["AbsorptionTests.test_multiple_same_type_districts_and_ui_only_enumeration_are_preserved"]
  local tx
  RUN(L[1])
  SPLIT_UNIT()
  RUN(L[2])
  tx = APPLY()
  EQ(function() return tx.Status end,function() return L[3] end)
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(0,0,0) end)
  FINISH()
  TRUTH(function() return VERIFY(L[4], true) end,true)
  RUN(L[5])
  ERROR(function()
    VERIFY(L[7], false)
  end, L[6])
end

function T.test_district_only_ui_cache_waits_after_transfer_and_across_resume()
  local L=STRINGS["AbsorptionTests.test_district_only_ui_cache_waits_after_transfer_and_across_resume"]
  local tx,counts
  RUN(L[1])
  RUN(READ("poc/JAC_PoC_Absorption/"..L[2]))
  RUN(L[4])
  tx = APPLY()
  EQ(function() return tx.Status end,function() return L[5] end)
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(0,0,0) end)
  RUN(L[6])
  tx = A.Resume(L[7])
  EQ(function() return tx.Status end,function() return L[8] end)
  TRUTH(function() return tx.AwaitingFinalUI end,true)
  EQ(function() return TRANSFERS end,function() return 2 end)
  counts = TUPLE(SPAWNS, KILLS, TRANSFERS)
  RELOAD()
  EQ(function() return A.Resume(L[9]).Status end,function() return L[10] end)
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return counts end)
  RUN(L[11])
  EQ(function() return A.Resume(L[12]).Status end,function() return L[13] end)
  TRUTH(function() return VERIFY(L[14], true) end,true)
end

function T.test_palace_work_loss_is_detected_even_when_the_palace_building_is_exempt()
  local L=STRINGS["AbsorptionTests.test_palace_work_loss_is_detected_even_when_the_palace_building_is_exempt"]
  RUN(L[1])
  ERROR(function()
    APPLY()
  end, L[2])
  EQ(function() return A.GetState().Transactions[L[3]].Cities[1].PalaceWorks[1] end,function() return 9001 end)
  EQ(function() return A.GetState().Transactions[L[4]].Status end,function() return L[5] end)
end

function T.test_palace_work_moved_to_receiver_capital_is_preserved_and_verified()
  local L=STRINGS["AbsorptionTests.test_palace_work_moved_to_receiver_capital_is_preserved_and_verified"]
  RUN(L[1])
  APPLY()
  TRUTH(function() return A.OwnedGreatWorks(2)[9001] end,true)
  TRUTH(function() return VERIFY(L[2], true) end,true)
end

function T.test_previous_governance_is_saved_before_engine_clears_city_properties()
  local L=STRINGS["AbsorptionTests.test_previous_governance_is_saved_before_engine_clears_city_properties"]
  RUN(L[1])
  APPLY()
  RUN(L[2])
  RELOAD()
  TRUTH(function() return VERIFY(L[3], true) end,true)
end

function T.test_exact_verification_detects_city_rename_but_later_turns_allow_it()
  local L=STRINGS["AbsorptionTests.test_exact_verification_detects_city_rename_but_later_turns_allow_it"]
  APPLY()
  RUN(L[1])
  ERROR(function()
    A.Verify(L[3], true)
  end, L[2])
  A.Verify(L[4], false)
end

function T.test_control_missionary_publication_checks_charges_then_core_religion()
  local L=STRINGS["AbsorptionPackageTests.test_control_missionary_publication_checks_charges_then_core_religion"]
  local data,condition,check
  data=JSON(READ(L[1]))
  condition=FIRST_CONDITION_STEP(data.steps,L[8],true).condition.code
  check=FIRST_CODE(data.steps,L[15])
  -- VM is provided by the Rust Harness.
  RUN(L[18])
  TRUTH(function() return RUN(condition) end,true)
  CONTAINS(L[19],RUN(check),true)
  RUN(L[20])
  TRUTH(function() return RUN(condition) end,false)
  ERROR(function()
    RUN(check)
  end, L[21])
  RUN(L[22])
  TRUTH(function() return RUN(condition) end,false)
  ERROR(function()
    RUN(check)
  end, L[23])
  RUN(L[24])
  ERROR(function()
    RUN(check)
  end, L[25])
end

function T.test_recall_publication_uses_ui_class_getter_and_waits_for_native_claim()
  local L=STRINGS["AbsorptionPackageTests.test_recall_publication_uses_ui_class_getter_and_waits_for_native_claim"]
  local data,code
  data=JSON(READ(L[1]))
  code=FIRST_CONDITION_STEP(data.steps,L[8],true).condition.code
  -- VM is provided by the Rust Harness.
  RUN(L[11])
  TRUTH(function() return RUN(code) end,false)
  RUN(L[12])
  TRUTH(function() return RUN(code) end,true)
  for _, state in pairs({L[13], L[14], L[15], L[16], L[17]}) do
    RUN(L[18])
    RUN(state)
    TRUTH(function() return RUN(code) end,false)
  end
end

function T.test_recall_spawn_preparation_moves_only_blockers_and_preserves_them()
  local L=STRINGS["AbsorptionPackageTests.test_recall_spawn_preparation_moves_only_blockers_and_preserves_them"]
  local data,code,ai_data,ai_code
  data=JSON(READ(L[1]))
  code=FIRST_CODE(data.steps,L[5])
  ai_data=JSON(READ(L[8]))
  ai_code=FIRST_CODE(ai_data.steps,L[12])
  -- VM is provided by the Rust Harness.
  RUN(L[15])
  CONTAINS(L[16],RUN(code),true)
  RUN(L[17])
  ERROR(function()
    RUN(code)
  end, L[18])
  RUN(L[19])
  RUN(L[20])
  CONTAINS(L[21],RUN(ai_code),true)
  RUN(L[22])
end

return T
