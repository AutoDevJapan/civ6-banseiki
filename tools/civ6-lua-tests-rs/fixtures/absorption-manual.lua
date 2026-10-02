-- Tests whose Python scaffolding includes sets, try/except, or fresh subtest VMs.
local T=CASES
function T.test_every_spec_disposition_is_routable_and_rejects_cross_kind_outcomes()
  local L=STRINGS['AbsorptionTests.test_every_spec_disposition_is_routable_and_rejects_cross_kind_outcomes']
  local expected={
    UNIT={SURRENDER=true,INCORPORATE=true,DISSOLVE=true,ESCAPE=true,RESIST=true},
    PERSON={CAPTURE=true,RECRUIT=true,SUBMIT=true,EXILE=true,EXECUTE=true,RETIRE=true,DEFECT=true},
    HERO={RECRUIT=true,PROTECT=true,RECONTRACT=true,DEFECT=true,HIDE=true,SUICIDE=true},
    GREAT_PERSON={RECRUIT=true,EXILE=true,RETIRE=true},
    ITEM={TRANSFER=true,LOOT=true,SEIZE=true,RETURN=true,SCATTER=true},
    UNLOCK={INHERIT=true,PRESERVE=true},RELIGION={PRESERVE=true}
  }
  local actual={}
  for _,row in pairs(OUTCOMES) do
    actual[row.Kind]=actual[row.Kind] or {};actual[row.Kind][row.Outcome]=true
    EQ(function() return A.Rules.Destination(row.Kind,row.Outcome) end,function() return row.Destination end)
  end
  EQ(function() return actual end,function() return expected end)
  REJECT(L[42],L[43])
end
function T.test_ui_only_spread_mismatch_blocks_and_preserves_donors_until_resume()
  local L=STRINGS['AbsorptionTests.test_ui_only_spread_mismatch_blocks_and_preserves_donors_until_resume']
  RUN(L[1]);SPLIT_UNIT()
  EQ(function() return APPLY().Status end,function() return L[2] end)
  RUN(L[3])
  EQ(function() return A.Resume(L[4]).Status end,function() return L[5] end)
  RUN(L[6])
  local caught=false
  for _=1,16 do
    RUN(L[7])
    local ok,tx=TRY(function() return A.Resume(L[8]) end)
    if not ok then CONTAINS(L[9],tx,true);caught=true;break end
    EQ(function() return tx.Status end,function() return L[10] end)
  end
  assert(caught,L[11])
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(6,0,0) end)
  EQ(function() return A.GetState().Transactions[L[12]].Status end,function() return L[13] end)
  RUN(L[14])
  local tx=FINISH()
  EQ(function() return tx.Status end,function() return L[15] end)
  EQ(function() return SPAWNS end,function() return 6 end)
  TRUTH(function() return VERIFY(L[16],true) end,true)
end
function T.test_same_turn_spent_spread_is_published_before_snapshot_and_never_restored_as_unused()
  local L=STRINGS['AbsorptionTests.test_same_turn_spent_spread_is_published_before_snapshot_and_never_restored_as_unused']
  SPLIT_UNIT();RUN(L[1])
  local tx=APPLY()
  TRUTH(function() return tx.AwaitingPreparation end,true)
  EQ(function() return SPAWNS end,function() return 0 end)
  RELOAD()
  EQ(function() return A.Resume(L[2]).Status end,function() return L[3] end)
  EQ(function() return SPAWNS end,function() return 0 end)
  RUN(L[4]);tx=A.Resume(L[5])
  EQ(function() return tx.Status end,function() return L[6] end)
  EQ(function() return FIND_SPREAD(tx.Units,10) end,function() return 1 end)
  local caught=false
  for _=1,20 do
    RUN(L[7])
    local ok,result=TRY(function() return A.Resume(L[8]) end)
    if not ok then CONTAINS(L[9],result,true);caught=true;break end
    tx=result
  end
  assert(caught,L[10])
  EQ(function() return A.GetState().Transactions[L[11]].Status end,function() return L[12] end)
  EQ(function() return TUPLE(KILLS,TRANSFERS) end,function() return TUPLE(0,0) end)
  EQ(function() return Players[1].units[10].spread end,function() return 1 end)
end
function T.test_native_hero_checks_detect_corruption_instead_of_only_testing_unit_count()
  local L=STRINGS['AbsorptionTests.test_native_hero_checks_detect_corruption_instead_of_only_testing_unit_count']
  local corruption,message=L[ARG1*2-1],L[ARG1*2]
  ADD_HERO();APPLY();RUN(L[13]..corruption)
  ERROR(function() VERIFY(L[14],true) end,message)
end
function T.test_native_hero_archival_and_explicit_suicide_preserve_requested_outcome()
  local L=STRINGS['AbsorptionTests.test_native_hero_archival_and_explicit_suicide_preserve_requested_outcome']
  local outcome=L[ARG1]
  ADD_HERO();NATIVE_OUTCOME=outcome;RUN(L[4])
  SUICIDE(outcome==L[6] and L[5] or L[7],outcome)
end
function T.test_runtime_park_and_nonhero_action_charges_are_rejected_before_changes()
  local L=STRINGS['AbsorptionTests.test_runtime_park_and_nonhero_action_charges_are_rejected_before_changes']
  local counter=L[ARG1]
  UNSUPPORTED_COUNTER=counter;RUN(L[3])
  ERROR(function() APPLY() end,L[4]..counter)
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(0,0,0) end)
  EQ(function() return Players[1].units[3].owner end,function() return 1 end)
end
function T.test_rock_band_state_is_rejected_by_type_class_or_tag_before_changes()
  local L=STRINGS['AbsorptionTests.test_rock_band_state_is_rejected_by_type_class_or_tag_before_changes']
  local definition=ARG1==3 and L[3]..L[4] or L[ARG1]
  RUN(definition);RUN(L[5])
  ERROR(function() APPLY() end,L[6])
  EQ(function() return TUPLE(SPAWNS,KILLS,TRANSFERS) end,function() return TUPLE(0,0,0) end)
  EQ(function() return Players[1].units[3].owner end,function() return 1 end)
end
function T.test_candidate_special_charge_blocks_and_reuses_the_candidate_on_resume()
  local L=STRINGS['AbsorptionTests.test_candidate_special_charge_blocks_and_reuses_the_candidate_on_resume']
  CANDIDATE_COUNTER=L[ARG1];CANDIDATE(CANDIDATE_COUNTER)
end
function T.test_ai_purchase_receipt_rejects_interleaved_faith_or_unit_events()
  local L=STRINGS['AbsorptionPackageTests.test_ai_purchase_receipt_rejects_interleaved_faith_or_unit_events']
  local steps=JSON(READ(L[1])).steps
  local purchase=FIRST_CODE(steps,L[5]);local check=FIRST_CODE(steps,L[9])
  local interference=L[11+ARG1]
  RUN(L[16]);RUN(purchase);RUN(L[17])
  EQ(function() return REQUESTS end,function() return 1 end)
  TRUTH(function() return BOOL(JAC_ABSORPTION_AI_PURCHASE.Complete) end,false)
  if interference==L[18] then RUN(L[19]) elseif interference==L[20] then RUN(L[21]) end
  RUN(L[22])
  TRUTH(function() return JAC_ABSORPTION_AI_PURCHASE.Complete end,true)
  EQ(function() return PY_SUB(JAC_ABSORPTION_AI_PURCHASE.FaithBefore,JAC_ABSORPTION_AI_PURCHASE.FaithAfter) end,function() return 1000 end)
  if interference==L[23] then RUN(L[24]) end
  if interference==L[25] then
    EQ(function() return RUN(check) end,function() return L[26] end)
  else ERROR(function() RUN(check) end,L[27]) end
end
