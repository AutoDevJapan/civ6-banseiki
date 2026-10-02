-- CASE test_base_100_and_minimum_20_for_all_speeds
-- SEGMENT
          for _,v in ipairs({{'ONLINE',50,10,5},{'QUICK',67,14,7},{'STANDARD',100,20,10},
                             {'EPIC',150,30,15},{'MARATHON',300,60,30}}) do
            SPEED='GAMESPEED_'..v[1]
            assert(A.ProductionTurns({})==v[2]); assert(A.ProductionTurns({Building=1000})==v[3])
            assert(A.Scale(A.PRODUCTION_LOCK)==v[4])
          end
-- CASE test_unplaced_spawn_keeps_training_pending_without_starting_nationwide_lock
-- SEGMENT
          assert(A.StartProduction(0,10))
          local spawn=UnitManager.InitUnitValidAdjacentHex
          UnitManager.InitUnitValidAdjacentHex=function(id,kind,x,y)
            BAD_UNIT=GRANT(id,GameInfo.Units[kind].Index,-9999,-9999):GetID()
          end
          ADVANCE(100)
          local s=A.State(0)
          assert(s.Production and s.Production.Due==1100)
          assert(s.LastCompletion==nil and s.LastProducedUnit==nil and (s.ProductionUnlock or 0)<=NOW)
          assert(Players[0]:GetUnits():FindID(BAD_UNIT)==nil and #KILLS==1)
          UnitManager.InitUnitValidAdjacentHex=spawn; ADVANCE(1)
          s=A.State(0)
          assert(s.Production==nil and s.LastCompletion==1101 and s.ProductionUnlock==1111)
          local unit=Players[0]:GetUnits():FindID(s.LastProducedUnit)
          assert(unit and unit:GetX()>=0 and unit:GetY()>=0)
-- CASE test_every_factor_and_security_direction
-- SEGMENT
          for _,k in ipairs({'Building','District','Policy','Government','Trait','Job'}) do
            assert(A.ProductionTurns({[k]=1})<100)
          end
          assert(A.ProductionTurns({Security=100})==120)
          assert(A.ProductionTurns({Security=0})==80)
          for security=0,100 do
            assert(A.ProductionTurns({Security=security,Building=10000})==20)
          end
-- CASE test_actual_completion_and_exact_lock_boundary
-- SEGMENT
          assert(A.StartProduction(0,10)); ADVANCE(99)
          local s=A.State(0); assert(s.Production and s.LastCompletion==nil)
          ADVANCE(1); s=A.State(0)
          assert(s.Production==nil and s.LastCompletion==1100 and s.ProductionUnlock==1110)
          assert(Players[0]:GetUnits():FindID(s.LastProducedUnit))
          ADVANCE(9)
          for _,id in ipairs({10,11}) do local ok,r=A.StartProduction(0,id); assert(not ok and r=='PRODUCTION_LOCK') end
          ADVANCE(1); assert(A.StartProduction(0,11))
-- CASE test_minimum_is_elapsed_time_even_with_extreme_bonuses
-- SEGMENT
          Players[0]:GetCities():FindID(10):SetProperty(A.INPUT,{Building=10000})
          assert(A.StartProduction(0,10)); ADVANCE(19); assert(A.State(0).LastCompletion==nil)
          ADVANCE(1); assert(A.State(0).LastCompletion==1020)
-- CASE test_cancellation_cannot_transfer_elapsed_progress
-- SEGMENT
          assert(A.StartProduction(0,10)); ADVANCE(99); A.CancelProduction(0)
          assert(A.StartProduction(0,11)); ADVANCE(1)
          assert(A.State(0).LastCompletion==nil and A.State(0).Production.Due==1199)
-- CASE test_cancel_does_not_clear_existing_completion_lock
-- SEGMENT
          assert(A.StartProduction(0,10)); ADVANCE(100); A.CancelProduction(0)
          local ok,r=A.StartProduction(0,11); assert(not ok and r=='PRODUCTION_LOCK')
          assert(A.State(0).ProductionUnlock==1110)
-- CASE test_lost_city_releases_only_the_production_reservation
-- SEGMENT
          assert(A.StartProduction(0,10)); DATA[0].Cities[10]=nil; ADVANCE(1)
          assert(A.State(0).Production==nil and A.State(0).LastCompletion==nil)
          assert(A.StartProduction(0,11))
-- CASE test_blocked_spawn_lock_begins_at_actual_completion
-- SEGMENT
          assert(A.StartProduction(0,10)); SPAWN_BLOCKED=true; ADVANCE(105)
          assert(A.State(0).Production and A.State(0).ProductionUnlock==nil)
          SPAWN_BLOCKED=false; ADVANCE(1)
          assert(A.State(0).LastCompletion==1106 and A.State(0).ProductionUnlock==1116)
-- CASE test_production_and_command_locks_are_independent
-- SEGMENT
          assert(A.StartProduction(0,10)); ADVANCE(100)
          U=GRANT(0,0,1,0):GetID()
          assert(A.StartAttempt(0,U,1,'LEADER')); assert(A.State(0).Mission)
          ADVANCE(10); assert(A.StartProduction(0,11))
          local ok,r=A.CanCommand(A.State(0),NOW); assert(not ok and r=='COMMAND_LOCK')
-- CASE test_training_recalculates_current_factors_and_never_rerolls
-- SEGMENT
          assert(A.StartProduction(0,10)); ADVANCE(10)
          Players[0]:SetProperty(A.INPUT,{Security=100}); ADVANCE(1)
          assert(A.State(0).Production.Due==1120 and A.State(0).Production.Start==1000)
          Players[0]:SetProperty(A.INPUT,{Security=0,Policy=1,Relations={['1']=1}}); ADVANCE(1)
          assert(A.State(0).Production.Due==1060)
          Players[0]:GetCities():FindID(10):SetProperty(A.INPUT,{Building=1000})
          ADVANCE(7); assert(NOW==1019 and A.State(0).Production.Due==1020)
          assert(A.State(0).LastCompletion==nil); ADVANCE(1)
          assert(A.State(0).LastCompletion==1020 and A.State(0).ProductionUnlock==1030 and RAND_CALLS==0)
-- CASE test_training_can_extend_again_and_finish_after_elapsed_reduction
-- SEGMENT
          local city=Players[0]:GetCities():FindID(10)
          city:SetProperty(A.INPUT,{Building=1000}); assert(A.StartProduction(0,10)); ADVANCE(19)
          city:SetProperty(A.INPUT,nil); ADVANCE(1)
          assert(A.State(0).Production.Due==1100 and A.State(0).LastCompletion==nil)
          ADVANCE(10); city:SetProperty(A.INPUT,{Building=1000}); ADVANCE(1)
          assert(A.State(0).LastCompletion==1031 and A.State(0).ProductionUnlock==1041)
-- CASE test_dynamic_training_round_trip_keeps_start_speed_and_current_inputs
-- SEGMENT
          assert(A.StartProduction(0,10)); ADVANCE(10)
          Players[0]:SetProperty(A.INPUT,{Security=100}); ADVANCE(1)
          SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}
          ADVANCE(7); GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now
          rebuild(); EVENTS={}
-- CASE test_dynamic_training_round_trip_keeps_start_speed_and_current_inputs
-- SEGMENT
          A=JAC_ASN; A.Tick(); assert(A.State(0).Production.Due==1120)
          Players[0]:SetProperty(A.INPUT,{Security=0}); ADVANCE(1)
          local p=A.State(0).Production
          assert(p.Start==1000 and p.Speed==1 and p.Due==1080 and p.EvaluatedTurn==1012 and RAND_CALLS==0)
          ADVANCE(68); assert(A.State(0).LastCompletion==1080)
-- CASE test_test_deadline_hook_is_inert_when_test_mode_is_off
-- SEGMENT
          assert(A.StartProduction(0,10)); assert(A.StartAttempt(0,U,1,'LEADER'))
          local original=A.State(0); original.ProductionUnlock=1099; Players[0]:SetProperty(A.STATE,original)
          local ok,reason=ExposedMembers.JAC_Assassin.TestAdvanceDeadlines(0)
          assert(not ok and reason=='TEST_MODE_OFF')
          local s=A.State(0)
          assert(s.Production.Due==1100 and s.Production.TestDue==nil and s.Production.Start==1000)
          assert(s.Mission.Due==1005 and s.Mission.SuccessRoll==original.Mission.SuccessRoll)
          assert(s.CommandUnlock==1050 and s.ProductionUnlock==1099 and RAND_CALLS==3)
-- CASE test_test_deadlines_survive_recomputation_reload_and_resolve_on_real_turn
-- SEGMENT
          FORCE(true,false,'FREE_STATE'); assert(A.StartProduction(0,10)); assert(A.StartAttempt(0,U,1,'LEADER'))
          local s=A.State(0); s.ProductionUnlock=1099; Players[0]:SetProperty(A.STATE,s)
          assert(A.TestAdvanceDeadlines(0))
          s=A.State(0)
          assert(s.Production.Due==1001 and s.Production.TestDue==1001 and s.Production.Turns==100)
          assert(s.Mission.Due==1001 and s.CommandUnlock==1001 and s.ProductionUnlock==1001)
          assert(s.LastMission==nil and s.LastCompletion==nil and FREE_CITY_CALLS==0 and RAND_CALLS==3)
          SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}
          GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now; rebuild(); EVENTS={}
-- CASE test_test_deadlines_survive_recomputation_reload_and_resolve_on_real_turn
-- SEGMENT
          A=JAC_ASN; ADVANCE(1); local s=A.State(0)
          assert(s.LastCompletion==1001 and s.Production==nil and s.ProductionUnlock==1011)
          assert(s.Mission==nil and s.LastMission.Result=='LEADER_FREE_STATE' and FREE_CITY_CALLS==2)
          assert(RAND_CALLS==3); ADVANCE(1); assert(FREE_CITY_CALLS==2)
-- CASE test_test_deadline_hook_selects_clocks_and_never_extends_expired_locks
-- SEGMENT
          Game:SetProperty(A.TEST,1); assert(A.StartProduction(0,10)); assert(A.StartAttempt(0,U,1,'LEADER'))
          local s=A.State(0); s.ProductionUnlock=1000; Players[0]:SetProperty(A.STATE,s)
          assert(A.TestAdvanceDeadlines(0,{ProductionLock=true,CommandLock=true}))
          s=A.State(0); assert(s.ProductionUnlock==1000 and s.CommandUnlock==1001)
          assert(s.Production.Due==1100 and s.Mission.Due==1005)
          assert(A.TestAdvanceDeadlines(0,{Training=true}))
          Game:SetProperty(A.TEST,nil); ADVANCE(1)
          assert(A.State(0).Production.Due==1100 and A.State(0).Production.Turns==100)
          assert(A.State(0).LastCompletion==nil)
-- CASE test_only_full_stack_reaches_wait_one
-- SEGMENT
          assert(A.WaitTurns({})==50)
          local f={Security=0,Relation=1,School=1,Governor=1,Hero=1,GreatPerson=1}
          assert(A.WaitTurns(f)==1)
          for _,k in ipairs({'Security','Relation','School','Governor','Hero','GreatPerson'}) do
            local g=clone(f); g[k]=(k=='Security') and 50 or 0; assert(A.WaitTurns(g)>1)
          end
          assert(A.WaitTurns(f,3)==3); assert(A.WaitTurns({},0.67)==34)
-- CASE test_distances_target_periods_and_rounding
-- SEGMENT
          for _,x in ipairs({{'LEADER',5,8,10},{'GOVERNOR',3,5,6},{'GREAT_PERSON',2,3,4}}) do
            assert(A.MissionTurns(x[1],1)==x[2]); assert(A.MissionTurns(x[1],2)==x[3])
            assert(A.MissionTurns(x[1],4)==x[4])
          end
          assert(A.MissionTurns('GOVERNOR',2,0.67)==4)
          DATA[0].Units[U].X=99
          local ok,r=A.StartAttempt(0,U,1,'LEADER'); assert(not ok and r=='TOO_FAR')
          DATA[0].Units[U].X=1; assert(A.StartAttempt(0,U,1,'LEADER'))
-- CASE test_all_six_success_rates_and_corrections
-- SEGMENT
          for _,r in ipairs({{'LEADER',1},{'GOVERNOR',15},{'GREAT_PERSON',30},{'HERO',50},{'UNIT',50},{'BUILDER',90}}) do
            assert(A.SuccessChance(r[1],{})==r[2])
            assert(A.SuccessChance(r[1],{Security=100,Guard=1})<r[2])
          end
-- CASE test_builder_hero_gp_and_assassin_are_classified_by_real_unit
-- SEGMENT
          assert(A.Classify(GRANT(1,0,2,0))=='UNIT')
          assert(A.Classify(GRANT(1,1,2,0))=='BUILDER')
          assert(A.Classify(GRANT(1,2,2,0))=='GREAT_PERSON')
          assert(A.Classify(GRANT(1,3,2,0))=='HERO')
          assert(A.Classify(GRANT(1,4,2,0))=='UNIT')
          local u=GRANT(1,1,2,0); local ok,r=A.StartAttempt(0,U,1,'UNIT',u:GetID())
          assert(not ok and r=='TARGET_KIND') -- no claiming a builder as a generic unit
-- CASE test_names_are_assigned_by_confirmed_country_not_whole_map
-- SEGMENT
          assert(A.UnitForPlayer(0)==A.UNIT)
          assert(A.UnitForPlayer(2)==A.COMMON_UNIT)
          PlayerConfigurations[2].GetCivilizationTypeName=function() return 'CIVILIZATION_JAC_RYUKYU' end
          assert(A.UnitForPlayer(2)==A.COMMON_UNIT)
          PlayerConfigurations[2].GetCivilizationTypeName=function() return 'CIVILIZATION_MONGOLIA' end
          assert(A.UnitForPlayer(2)==A.COMMON_UNIT)
          assert(A.IsAssassin(GRANT(2,5,0,0)))
          assert(A.StartProduction(2,10)); ADVANCE(100)
          assert(DATA[2].Units[A.State(2).LastProducedUnit].Type==5)
-- CASE test_one_active_mission_even_after_wait_expires
-- SEGMENT
          Players[0]:SetProperty(A.INPUT,{Security=0,Relations={['1']=1},School=1,Governor=1,Hero=1,GreatPerson=1})
          assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(1)
          local ok,r=A.StartAttempt(0,U2,2,'LEADER'); assert(not ok and r=='MISSION_ACTIVE')
          assert(A.State(0).LastCommand==1000)
-- CASE test_command_wait_cannot_be_bypassed_by_unit_or_country
-- SEGMENT
          FORCE(false,false); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(49)
          local ok,r=A.StartAttempt(0,U2,2,'LEADER'); assert(not ok and r=='COMMAND_LOCK')
          ADVANCE(1); assert(A.StartAttempt(0,U2,2,'LEADER'))
-- CASE test_outcome_roll_is_at_start_and_not_at_resolution
-- SEGMENT
          ROLL=99; assert(A.StartAttempt(0,U,1,'LEADER'))
          assert(A.State(0).Mission.Success); assert(RAND_CALLS==3)
          ROLL=9999; ADVANCE(5); assert(RAND_CALLS==3)
          assert(A.State(0).LastMission.Result=='LEADER_FREE_STATE')
-- CASE test_percent_threshold_is_not_inclusive
-- SEGMENT
          ROLL=100; assert(A.StartAttempt(0,U,1,'LEADER')); assert(not A.State(0).Mission.Success)
-- CASE test_forced_outcome_only_works_with_explicit_test_flag
-- SEGMENT
          Players[0]:SetProperty('JAC_ASN_FORCE_NEXT',{Success=1,Detected=0}); ROLL=9999
          assert(A.StartAttempt(0,U,1,'LEADER')); assert(not A.State(0).Mission.Success)
-- CASE test_ui_operation_cannot_supply_forced_outcome
-- SEGMENT
          ROLL=9999; EVENTS.JAC_AssassinOperation(0,{Action='ATTEMPT',Unit=U,Target=1,Kind='LEADER',Success=1})
          assert(not A.State(0).Mission.Success)
-- CASE test_leader_death_splits_all_cities_disbands_units_and_archives_people
-- SEGMENT
          local gp=GRANT(1,2,2,0); GRANT(1,4,2,0)
          Players[1]:SetProperty(A.INPUT,{Successors={{Identity='PERSON_GP',Kind='GREAT_PERSON',Unit=gp:GetID()}},
            Governors={['7']={City=10,Identity='GOV'}}})
          Players[1]:SetProperty(A.PEOPLE,{OLD={Identity='OLD',Dead=1,Unit=999,Governor=8,City=10}})
          FORCE(true,false,'FREE_STATE'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(5)
          local n=A.Nation(1)
          assert(n.LeaderDead==1 and n.State=='FREE_STATE' and n.DeathTurn==1005)
          assert(n.DecisionRequired==nil and n.Government=='FREE_CITIES' and n.Slot=='RETIRED' and not DATA[1].Alive)
          assert(n.FreeCitiesPlayer==3 and n.AI=='FREE_CITIES_ENGINE' and FREE_CITY_CALLS==2)
          assert(Players[1]:GetCities():GetCount()==0 and Players[1]:GetUnits():GetCount()==0)
          assert(DATA[0].Cities[10].Owner==0 and Players[3]:GetCities():GetCount()==2)
          assert(n.People.PERSON_GP.Role=='UNAFFILIATED' and n.People.PERSON_GP.SourceUnit==gp:GetID())
          assert(n.People.PERSON_GP.Unit==nil and n.People.GOV.Governor==nil and n.People.LEADER_1.Dead==1)
          assert(n.People.GOV.Role=='UNAFFILIATED' and n.People.GOV.SourceGovernor==7)
          assert(#n.Cities==2 and #n.DisbandedUnits==2)
          assert(n.People.OLD.Dead==1 and n.People.OLD.Role=='DEAD' and n.People.OLD.SourceUnit==999)
          assert(n.People.OLD.Unit==nil and n.People.OLD.Governor==nil and n.People.OLD.City==nil)
          assert(A.State(0).CommandUnlock==1050)
          Players[1]:SetProperty(A.NATION,nil); Players[1]:SetProperty(A.PEOPLE,nil)
          assert(A.Nation(1).People.PERSON_GP.Role=='UNAFFILIATED')
          local count=FREE_CITY_CALLS; A.Tick(); ADVANCE(1); assert(FREE_CITY_CALLS==count)
-- CASE test_eliminated_human_slot_cancels_saved_training_after_reload_without_retransferring
-- SEGMENT
          assert(A.StartProduction(0,10)); local enemy=GRANT(2,0,1,0)
          Game:SetProperty(A.TEST,1); Players[2]:SetProperty('JAC_ASN_FORCE_NEXT',{Success=1,Detected=0,Branch='FREE_STATE'})
          assert(A.StartAttempt(2,enemy:GetID(),0,'LEADER')); ADVANCE(5)
          assert(not Players[0]:IsAlive() and Players[0]:IsHuman() and A.Nation(0).Slot=='RETIRED')
          assert(Players[0]:GetUnits():GetCount()==0 and Players[0]:GetCities():GetCount()==0 and FREE_CITY_CALLS==2)
          SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}
          GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now; rebuild(); EVENTS={}
-- CASE test_eliminated_human_slot_cancels_saved_training_after_reload_without_retransferring
-- SEGMENT
          A=JAC_ASN; ADVANCE(1)
          assert(A.State(0).Production==nil and A.State(0).LastCompletion==nil and not Players[0]:IsAlive())
          assert(A.Nation(0).State=='FREE_STATE' and FREE_CITY_CALLS==2 and RAND_CALLS==3)
-- CASE test_post_death_snapshot_and_branch_are_frozen
-- SEGMENT
          FORCE(true,false,'SUCCESSION'); Players[1]:SetProperty(A.INPUT,{Security=10,Successor='PERSON_1'})
          assert(A.StartAttempt(0,U,1,'LEADER'))
          Players[1]:SetProperty(A.INPUT,{Security=90,Successor='PERSON_2'})
          ADVANCE(5); local n=Players[1]:GetProperty(A.NATION)
          assert(n.Branch=='SUCCESSION' and n.Snapshot.Security==10 and n.Snapshot.Successor.Identity=='PERSON_1')
          assert(n.CurrentLeader=='PERSON_1' and n.State=='SUCCESSION')
          assert(A.Input(Players[1]).Successor=='PERSON_2') -- a new, different candidate is retained
-- CASE test_equivalent_favor_vote_is_available_without_test_mode_on_japan_map
-- SEGMENT
          assert(Game:GetProperty(A.TEST)==nil); DATA[2].Favor=60
          assert(A.StartAttempt(0,U,1,'LEADER'))
          local snapshot=A.State(0).Mission.Snapshot
          assert(snapshot.Congress and snapshot.Ballots['2'].Favor==60 and snapshot.Ballots['2'].Weight==4)
          Players[0]:SetProperty(A.STATE,nil); Players[1]:SetProperty(A.INPUT,{Congress={Enabled=false}})
          FORCE(true,false,'CONGRESS') -- an explicit disabled vote provider still excludes this branch
-- CASE test_unit_success_and_failure_dispositions
-- SEGMENT
          local target=GRANT(1,0,2,0):GetID(); FORCE(true,false)
          assert(A.StartAttempt(0,U,1,'UNIT',target)); ADVANCE(2)
          assert(DATA[1].Units[target]==nil and DATA[0].Units[U]~=nil)
          ADVANCE(48); FORCE(false,false); assert(A.StartAttempt(0,U,2,'LEADER')); ADVANCE(8)
          assert(DATA[0].Units[U]==nil)
-- CASE test_failure_and_detection_independently_share_penalty_once
-- SEGMENT
                  local m=A.State(0).LastMission; local p=Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1')
                  if not m.Success or m.Detected then
                    assert(p.Grievances['0']==25 and p.Friendship['0']==-10)
                    assert((p.WarReason['0']~=nil)==m.Detected)
                    assert((Players[2]:GetProperty('JAC_ASN_DENOUNCE_0')~=nil)==m.Detected)
                  else assert(p==nil) end
                  assert((DATA[0].Units[U]==nil)==(not m.Success))
                  A.Tick(); ADVANCE(1)
                  if p then assert(Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1').Grievances['0']==25) end
-- CASE test_missing_target_does_not_redirect_or_reroll
-- SEGMENT
          local target=GRANT(1,4,2,0):GetID(); FORCE(true,false)
          assert(A.StartAttempt(0,U,1,'UNIT',target)); DATA[1].Units[target]=nil
          ADVANCE(2); assert(A.State(0).LastMission.Result=='TARGET_GONE')
          assert(A.State(0).CommandUnlock==1050 and RAND_CALLS==2)
-- CASE test_fixed_failure_detection_penalties_survive_target_disappearance
-- SEGMENT
          local target=GRANT(1,4,2,0):GetID(); FORCE(false,true)
          assert(A.StartAttempt(0,U,1,'UNIT',target)); DATA[1].Units[target]=nil
          ADVANCE(2)
          local p=Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1')
          assert(p.Grievances['0']==25 and p.Friendship['0']==-10 and p.WarReason['0']==1002)
          assert(DATA[0].Units[U]==nil and A.State(0).LastMission.Result=='TARGET_GONE')
-- CASE test_actor_disappearance_prevents_ghost_success
-- SEGMENT
          local target=GRANT(1,4,2,0):GetID(); FORCE(true,true)
          assert(A.StartAttempt(0,U,1,'UNIT',target)); DATA[0].Units[U]=nil
          ADVANCE(2)
          assert(DATA[1].Units[target] and A.State(0).LastMission.Result=='ACTOR_GONE')
          assert(Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1').Grievances['0']==25)
          assert(A.State(0).CommandUnlock==1050)
-- CASE test_eliminated_country_finishes_saved_jobs_and_fixed_penalties
-- SEGMENT
          assert(A.StartProduction(0,10)); FORCE(false,true)
          assert(A.StartAttempt(0,U,1,'LEADER'))
          DATA[0].Alive=false; DATA[0].Units={}; DATA[0].Cities={}
          ADVANCE(5)
          local s=A.State(0); assert(s.Mission==nil and s.Production==nil)
          assert(s.LastMission.Result=='ACTOR_GONE' and s.CommandUnlock==1050)
          assert(Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1').Grievances['0']==25)
          ADVANCE(1); assert(Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1').Grievances['0']==25)
-- CASE test_replacement_unit_with_same_id_and_type_is_not_the_target
-- SEGMENT
          local target=GRANT(1,4,2,0):GetID(); FORCE(true,false)
          assert(A.StartAttempt(0,U,1,'UNIT',target))
          DATA[1].Units[target]={ID=target,Owner=1,Type=4,X=2,Y=0,Properties={}}
          ADVANCE(2); assert(DATA[1].Units[target] and A.State(0).LastMission.Result=='TARGET_GONE')
-- CASE test_diplomacy_changes_recalculate_training_with_worst_living_major
-- SEGMENT
          assert(A.StartProduction(0,10))
          Players[0]:SetProperty(A.INPUT,{Relations={['0']=1,['1']=0.5,['2']=-1,['3']=1}})
          ADVANCE(1); assert(A.State(0).Production.Due==1095)
          DATA[0].Wars[2]=true
          ADVANCE(1); assert(A.State(0).Production.Due==1090)
          DATA[0].Wars[2]=nil; DATA[2].Alive=false
          Players[1]:SetProperty('JAC_ASN_DIPLOMACY_V1',{Friendship={['0']=-10}})
          ADVANCE(1); assert(A.State(0).Production.Due==1094)
          Players[0]:SetProperty(A.INPUT,{Relations={['1']=-1,['2']=1}})
          ADVANCE(1); assert(A.State(0).Production.Due==1100 and RAND_CALLS==0)
          assert(A.ProductionTurns({Relation=1})==90)
          assert(A.ProductionTurns({Relation=2})==90 and A.ProductionTurns({Relation=-1})==100)
          assert(A.ProductionTurns({Relation=1,Building=1000})==20)
          for _,v in ipairs({{0.5,45},{0.67,61},{1,90},{1.5,135},{3,270}}) do
            assert(A.ProductionTurns({Relation=1},v[1])==v[2])
          end
-- CASE test_training_relation_survives_reload_and_completes_at_reduced_boundary
-- SEGMENT
          Players[0]:SetProperty(A.INPUT,{Relations={['1']=1}})
          assert(A.StartProduction(0,10)); ADVANCE(89)
          assert(A.State(0).LastCompletion==nil and A.State(0).Production.Due==1090)
          SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}
          ADVANCE(1); GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now
          rebuild(); EVENTS={}
-- CASE test_training_relation_survives_reload_and_completes_at_reduced_boundary
-- SEGMENT
          A=JAC_ASN; A.Tick(); local p=A.State(0).Production
          assert(p.Start==1000 and p.Speed==1 and p.Turns==90 and p.Due==1090)
          ADVANCE(1); assert(A.State(0).LastCompletion==1090 and A.State(0).ProductionUnlock==1100)
          assert(RAND_CALLS==0)
-- CASE test_relation_penalty_increases_hostility_and_shortens_wait
-- SEGMENT
          local before=A.WaitTurns(A.CommandInput(0,1))
          FORCE(false,false); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(5)
          assert(A.Relation(0,1)>0); assert(A.WaitTurns(A.CommandInput(0,1))<before)
          assert(A.State(0).CommandUnlock==1050) -- accepted command clock remains frozen
-- CASE test_notification_exception_cannot_duplicate_production_or_penalties
-- SEGMENT
          NotificationManager.SendNotification=function() error('injected notification failure') end
          assert(A.StartProduction(0,10)); FORCE(false,true)
          assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(101)
          assert(Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1').Grievances['0']==25)
          assert(A.State(0).LastCompletion==1099 and A.State(0).Production==nil)
          local n=Players[0]:GetUnits():GetCount(); ADVANCE(2); assert(Players[0]:GetUnits():GetCount()==n)
-- CASE test_visibility_checks_do_not_disclose_or_accept_hidden_targets
-- SEGMENT
          HIDDEN_CITY=true; local ok,r=A.StartAttempt(0,U,1,'LEADER'); assert(not ok and r=='TARGET_UNKNOWN')
          HIDDEN_CITY=false; HIDDEN_UNIT=true; local target=GRANT(1,4,2,0):GetID()
          local ok,r=A.StartAttempt(0,U,1,'UNIT',target); assert(not ok and r=='TARGET_UNKNOWN')
          assert(RAND_CALLS==0)
-- CASE test_branch_weights_use_security_unrest_strength_relations_and_candidates
-- SEGMENT
          local a=A.BranchWeights({Security=100,Neighbor=true,Successor='P',Cities=2,
            Neighbors={{Cities=2,Relation=0}}})
          local b=A.BranchWeights({Security=0,Neighbor=true,Successor='P',Cities=2,
            Regions={Unrest=50},Neighbors={{Cities=4,Relation=1}}})
          assert(b[1][2]>a[1][2] and b[2][2]>a[2][2] and b[4][2]<a[4][2])
          local c=A.BranchWeights({Congress=false}); assert(c[2][2]==0 and c[3][2]==0 and c[4][2]==0)
-- CASE test_actor_loss_cancels_all_four_outcomes_but_always_applies_fixed_penalty_once
-- SEGMENT
                  local victim=GRANT(1,4,2,0):GetID(); FORCE({str(success).lower()},{str(detected).lower()})
                  assert(A.StartAttempt(0,U,1,'UNIT',victim)); DATA[0].Units[U]=nil
                  DATA[0].Units[U]={{ID=U,Owner=0,Type=0,X=1,Y=0,Properties={{}}}} -- replacement must survive
                  ADVANCE(2); local s=A.State(0); local p=Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1')
                  assert(s.LastMission.Result=='ACTOR_GONE' and s.LastMission.Cancelled)
                  assert(DATA[1].Units[victim] and DATA[0].Units[U] and #KILLS==0)
                  assert(p.Grievances['0']==25 and p.Friendship['0']==-10 and p.WarReason['0']==nil)
                  assert(Players[2]:GetProperty('JAC_ASN_DENOUNCE_0')==nil and s.CommandUnlock==1050)
                  A.Tick(); ADVANCE(2); assert(Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1').Grievances['0']==25)
-- CASE test_unregistered_great_person_keeps_original_unit_id_when_archived
-- SEGMENT
          local gp=GRANT(1,2,2,0); gp:SetProperty(A.PERSON,'UNREGISTERED_GP')
          FORCE(true,false,'FREE_STATE'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(5)
          local person=A.Nation(1).People.UNREGISTERED_GP
          assert(person.Role=='UNAFFILIATED' and person.SourceUnit==gp:GetID() and person.Unit==nil)
-- CASE test_absorption_freezes_recipient_transfers_cities_disbands_and_preserves_people
-- SEGMENT
          DATA[2].Cities[12]={ID=12,Owner=2,X=4,Y=2,Properties={}}
          local gp=GRANT(1,2,2,0); local token=gp:GetID()
          GRANT(1,4,2,0); Players[1]:SetProperty(A.INPUT,{Governors={['7']={City=10,Identity='GOV'}}})
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          local m=A.State(0).Mission; assert(m.PostDeathPlan.Recipient==2)
          DATA[0].Cities[12]={ID=12,Owner=0,X=0,Y=2,Properties={}} -- later strength cannot change recipient
          ADVANCE(5); local n=A.Nation(1)
          assert(n.State=='ABSORPTION' and n.Recipient==2 and n.Slot=='RETIRED' and n.AI=='RECIPIENT_SLOT')
          assert(#TRANSFERS==2 and TRANSFERS[1].To==2 and TRANSFERS[2].To==2)
          assert(Players[1]:GetCities():GetCount()==0 and Players[1]:GetUnits():GetCount()==0 and not DATA[1].Alive)
          assert(Players[2]:GetCities():GetCount()==5 and Players[0]:GetCities():GetCount()==3)
          assert(Players[2]:GetProperty(A.PEOPLE).GOV.Role=='MEMBER')
          assert(Players[2]:GetProperty(A.PEOPLE).GOV.SourceGovernor==7)
          assert(Players[2]:GetProperty(A.PEOPLE)['LEADER_1']==nil)
          assert(Game:GetProperty(A.NATIONS)['1'].State=='ABSORPTION' and RAND_CALLS==3)
          local count=#TRANSFERS; A.Tick(); ADVANCE(1); assert(#TRANSFERS==count)
-- CASE test_recipient_formula_combines_strength_friendship_border_distance_religion_culture
-- SEGMENT
          DATA[2].Military=200; DATA[0].Religion=1; DATA[1].Religion=1; DATA[2].Religion=2
          Players[0]:SetProperty(A.INPUT,{CultureGroup='A'})
          Players[2]:SetProperty(A.INPUT,{CultureGroup='B'})
          Players[1]:SetProperty(A.INPUT,{CultureGroup='A',Relations={['0']=-1,['2']=1}})
          PLOTS={{Owner=1,Adjacent={[0]=0,[1]=0,[2]=2}}}
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          local snapshot=A.State(0).Mission.Snapshot; local a,b=snapshot.Neighbors[1],snapshot.Neighbors[2]
          assert(a.Player==0 and b.Player==2 and snapshot.Recipient==0)
          assert(a.Military==0 and b.Military==200 and a.Border==2 and b.Border==1 and a.Distance==2)
          assert(math.abs(a.Score-0.775)<0.000001 and math.abs(b.Score-0.525)<0.000001)
          assert(a.AvailableWeight==100 and a.Components.Religion==1 and b.Components.Culture==0)
          DATA[2].Military=10000; DATA[1].Allies[2]=true; Players[1]:SetProperty(A.INPUT,{Relations={['2']=-1}})
          ADVANCE(5); assert(A.Nation(1).Recipient==0 and A.Nation(1).Snapshot.Neighbors[1].Score==a.Score)
-- CASE test_recipient_alliance_is_gameplay_and_optional_closeness_does_not_reward_missing_data
-- SEGMENT
          DATA[1].Allies[2]=true
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          local snapshot=A.State(0).Mission.Snapshot; local n=snapshot.Neighbors[1]
          assert(n.Player==2 and n.Friendship==1 and n.Allied and n.AvailableWeight==90)
          assert(n.Components.Religion==nil and n.Components.Culture==nil)
          assert(snapshot.Recipient==2)
-- CASE test_recipient_omits_pantheon_and_missing_apis_and_prefers_supplied_culture_over_proxy
-- SEGMENT
          DATA[0].Religion=0; DATA[1].Religion=0; DATA[2].Religion=0
          GameInfo.Civilizations={CIVILIZATION_JAC_ODA={Ethnicity='ASIAN'},
            CIVILIZATION_JAC_MATSUDAIRA={Ethnicity='ASIAN'},CIVILIZATION_JAC_AINU={Ethnicity='OTHER'}}
          Players[0]:SetProperty(A.INPUT,{CultureGroup='SUPPLIED'})
          Players[0].GetStats=nil; Map.GetAdjacentPlots=nil
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          for _,n in ipairs(A.State(0).Mission.Snapshot.Neighbors) do
            assert(n.Components.Religion==nil and n.Components.Border==nil and n.AvailableWeight==80)
            assert(n.Culture==0 and n.Military==0 and n.Strength==n.Cities)
          end
-- CASE test_recipient_distance_and_border_can_change_choice_and_ties_use_player_id
-- SEGMENT
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          assert(A.State(0).Mission.PostDeathPlan.Recipient==0)
          Players[0]:SetProperty(A.STATE,nil)
          DATA[2].Cities[10].X=3; DATA[2].Cities[11].X=3
          PLOTS={{Owner=1,Adjacent={[0]=2,[1]=2}}}
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          assert(A.State(0).Mission.PostDeathPlan.Recipient==2)
-- CASE test_favor_vote_weight_outvotes_more_nations_and_is_frozen_at_command
-- SEGMENT
          assert(A.FavorVotes(-10)==1 and A.FavorVotes(0)==1 and A.FavorVotes(9)==1)
          assert(A.FavorVotes(10)==2 and A.FavorVotes(29)==2 and A.FavorVotes(30)==3 and A.FavorVotes(60)==4)
          DATA[0].Favor=60
          Players[1]:SetProperty(A.INPUT,{Successor='HEIR',Congress={Votes={['0']='SUCCESSION',
            ['1']='FREE_STATE',['2']='FREE_STATE',['999']='FREE_STATE'}}})
          FORCE(true,false,'CONGRESS'); assert(A.StartAttempt(0,U,1,'LEADER'))
          local plan=A.State(0).Mission.PostDeathPlan
          assert(plan.Tally.SUCCESSION==4 and plan.Tally.FREE_STATE==2 and plan.Resolution=='SUCCESSION')
          assert(plan.Ballots['0'].Favor==60 and plan.Voting=='FAVOR_WEIGHTED_EQUIVALENT')
          DATA[0].Favor=0; DATA[1].Favor=1000; Players[1]:SetProperty(A.INPUT,{Congress={Votes={['0']='FREE_STATE'}}})
          ADVANCE(5); local n=A.Nation(1)
          assert(n.State=='SUCCESSION' and n.CurrentLeader=='HEIR' and n.Plan.Tally.SUCCESSION==4 and RAND_CALLS==3)
-- CASE test_lost_absorption_recipient_falls_back_without_new_draw
-- SEGMENT
          DATA[2].Cities[12]={ID=12,Owner=2,X=4,Y=2,Properties={}}
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER')); DATA[2].Alive=false
          ADVANCE(5); assert(A.Nation(1).State=='FREE_STATE' and FREE_CITY_CALLS==2 and RAND_CALLS==3)
-- CASE test_successor_kin_first_and_single_role_for_gp_and_governor
-- SEGMENT
                  GP=GRANT(1,2,2,0):GetID()
                  Players[1]:SetProperty(A.INPUT,{Governors={['7']={Identity='PERSON_KIN',City=10}},
                    Successors={{Identity='AAA',Kind='PERSON',Kin=0},
                    {Identity='PERSON_KIN',Name='Heir',Kin=1,CountryName='IGNORED',Kind='"""+kind+"""',Unit=GP,Governor=7}}})
                  FORCE(true,false,'SUCCESSION'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(5)
                  local n=A.Nation(1); assert(n.CurrentLeader=='PERSON_KIN' and n.CountryName==nil)
                  assert(n.State=='SUCCESSION' and n.Slot=='RETAINED' and DATA[1].Alive and #TRANSFERS==0)
                  assert(A.ResolveTarget(1,'LEADER').Identity=='PERSON_KIN')
-- CASE test_lost_successor_falls_back_without_selecting_next_candidate
-- SEGMENT
          local gp=GRANT(1,2,2,0):GetID()
          Players[1]:SetProperty(A.INPUT,{Successors={{Identity='A',Kind='GREAT_PERSON',Unit=gp},{Identity='B'}}})
          FORCE(true,false,'SUCCESSION'); assert(A.StartAttempt(0,U,1,'LEADER')); DATA[1].Units[gp]=nil
          ADVANCE(5); assert(A.Nation(1).State=='FREE_STATE' and A.Nation(1).CurrentLeader==nil and RAND_CALLS==3)
-- CASE test_registered_death_overrides_present_gp_governor_or_person
-- SEGMENT
                      GP=GRANT(1,2,2,0):GetID()
                      Players[1]:SetProperty(A.INPUT,{Governors={['7']={Identity='HEIR',City=10}},
                        Successor={Identity='HEIR',Kind='"""+kind+"""',Unit=GP,Governor=7}})
                      FORCE(true,false,'SUCCESSION'); assert(A.StartAttempt(0,U,1,'LEADER'))
-- CASE test_registered_death_overrides_present_gp_governor_or_person
-- SEGMENT
                      ADVANCE(5); assert(A.Nation(1).State=='FREE_STATE' and A.Nation(1).CurrentLeader==nil)
                      assert(DATA[1].Units[GP]==nil and A.Input(Players[1]).Governors==nil)
                      assert(RAND_CALLS==3)
-- CASE test_absorption_never_recreates_dead_registered_gp_under_a_unit_token
-- SEGMENT
                  GP=GRANT(1,2,2,0):GetID()
                  Players[1]:SetProperty(A.INPUT,{Successor={Identity='DEAD_GP',Kind='GREAT_PERSON',Unit=GP}})
                  FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
                  assert(DATA[1].Units[GP].Properties[A.PERSON]=='DEAD_GP')
-- CASE test_absorption_never_recreates_dead_registered_gp_under_a_unit_token
-- SEGMENT
                  ADVANCE(5); assert(A.Nation(1).State=='ABSORPTION')
                  local roster=Players[0]:GetProperty(A.PEOPLE)
                  assert(next(roster)==nil and DATA[1].Units[GP]==nil)
-- CASE test_persistent_person_binding_does_not_claim_same_id_replacement_gp
-- SEGMENT
          GP=GRANT(1,2,2,0):GetID()
          Players[1]:SetProperty(A.INPUT,{Successor={Identity='OLD_PERSON',Kind='GREAT_PERSON',Unit=GP}})
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          BOUND=Players[1]:GetProperty(A.BINDINGS).OLD_PERSON.Token
          assert(BOUND==DATA[1].Units[GP].Properties[A.TOKEN])
          DATA[1].Units[GP]={ID=GP,Owner=1,Type=2,X=2,Y=0,Properties={}}
          Players[1]:SetProperty(A.PEOPLE,{OLD_PERSON={Identity='OLD_PERSON',Dead=1}})
          SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}
          GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now; rebuild(); EVENTS={}
-- CASE test_persistent_person_binding_does_not_claim_same_id_replacement_gp
-- SEGMENT
          A=JAC_ASN; ADVANCE(5)
          local people=Players[0]:GetProperty(A.PEOPLE); local count=0
          for _,p in pairs(people) do count=count+1; assert(p.Identity~='OLD_PERSON' and p.Dead~=1) end
          assert(count==1 and Players[1]:GetProperty(A.BINDINGS).OLD_PERSON.Token==BOUND)
          assert(RAND_CALLS==3 and A.Nation(1).State=='ABSORPTION')
-- CASE test_absorption_people_have_one_identity_and_dead_leaders_never_rejoin
-- SEGMENT
          local gp=GRANT(1,2,2,0):GetID()
          Players[1]:SetProperty(A.INPUT,{Successors={{Identity='PERSON',Kind='GREAT_PERSON',Unit=gp}}})
          FORCE(true,false,'ABSORPTION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          local input=A.Input(Players[1]); input.Successors[#input.Successors+1]={Identity='NEW_PERSON'}
          Players[1]:SetProperty(A.INPUT,input); ADVANCE(5)
          local roster=Players[0]:GetProperty(A.PEOPLE); local count=0
          for _ in pairs(roster) do count=count+1 end
          assert(count==2 and roster.PERSON and roster.NEW_PERSON and roster.LEADER_1==nil)
          assert(roster.PERSON.Unit==nil and roster.PERSON.SourceUnit==gp and roster.PERSON.Role=='MEMBER')
-- CASE test_later_leader_death_updates_people_registry_and_cannot_revive_predecessor
-- SEGMENT
          Players[1]:SetProperty(A.INPUT,{Successor='HEIR'})
          FORCE(true,false,'SUCCESSION'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(50)
          assert(Players[1]:GetProperty(A.PEOPLE).LEADER_1.Dead==1)
          Players[1]:SetProperty(A.INPUT,{Successor='LEADER_1'})
          FORCE(true,false,'FREE_STATE'); assert(A.StartAttempt(0,U,1,'LEADER'))
          assert(A.State(0).Mission.Snapshot.Successor==nil); ADVANCE(5)
          local people=Players[1]:GetProperty(A.PEOPLE)
          assert(people.HEIR.Dead==1 and people.HEIR.Role=='DEAD_LEADER' and people.LEADER_1.Dead==1)
          assert(A.Nation(1).State=='FREE_STATE' and A.Nation(1).Previous.CurrentLeader=='HEIR')
-- CASE test_registered_country_name_survives_kin_succession_and_free_city_archive
-- SEGMENT
          Players[1]:SetProperty(A.INPUT,{Successor={Identity='FOUNDER',CountryName='LOC_NEW_COUNTRY'}})
          FORCE(true,false,'SUCCESSION'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(50)
          assert(A.Nation(1).CountryName=='LOC_NEW_COUNTRY')
          Players[1]:SetProperty(A.INPUT,{Successor={Identity='KIN',Kin=1,CountryName='IGNORE'}})
          FORCE(true,false,'SUCCESSION'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(50)
          assert(A.Nation(1).CountryName=='LOC_NEW_COUNTRY')
          FORCE(true,false,'FREE_STATE'); assert(A.StartAttempt(0,U,1,'LEADER')); ADVANCE(5)
          assert(A.Nation(1).CountryName=='LOC_NEW_COUNTRY')
-- CASE test_congress_ballots_freeze_favor_weight_and_tie_goes_to_free_cities
-- SEGMENT
                  Players[1]:SetProperty(A.INPUT,{Successor='HEIR',Congress={Enabled=true,Votes="""+votes+"""}})
                  FORCE(true,false,'CONGRESS'); assert(A.StartAttempt(0,U,1,'LEADER'))
                  local plan=A.State(0).Mission.PostDeathPlan
                  Players[1]:SetProperty(A.INPUT,{Congress={Enabled=true,Votes={['0']='FREE_STATE'}}})
                  ADVANCE(5); local n=A.Nation(1)
                  assert(n.Branch=='CONGRESS' and n.State=='"""+outcome+"""' and n.Plan.Resolution==plan.Resolution)
                  assert(RAND_CALLS==3 and n.DecisionRequired==nil)
-- CASE test_registered_governor_uses_stable_identity
-- SEGMENT
          Players[1]:SetProperty(A.INPUT,{Governors={['7']={City=10,Identity='GOV_TEST'}}})
          FORCE(true,false); assert(A.StartAttempt(0,U,1,'GOVERNOR',7)); ADVANCE(3)
          assert(A.State(0).LastMission.Result=='GOVERNOR_DEAD_PENDING')
          assert(A.Input(Players[1]).Governors['7'].Dead==1)
-- CASE test_round_trip_reload_reinitializes_script_without_reroll
-- SEGMENT
          assert(A.StartProduction(0,10)); FORCE(true,false,'FREE_STATE'); assert(A.StartAttempt(0,U,1,'LEADER'))
          SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}
          FROZEN=clone(A.State(0)); ADVANCE(2)
          GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now
          rebuild(); EVENTS={}
-- CASE test_round_trip_reload_reinitializes_script_without_reroll
-- SEGMENT
          A=JAC_ASN; local s=A.State(0)
          assert(s.Production.Due==FROZEN.Production.Due and s.Mission.Due==FROZEN.Mission.Due)
          assert(s.Mission.SuccessRoll==FROZEN.Mission.SuccessRoll and s.Mission.Branch==FROZEN.Mission.Branch)
          ADVANCE(5); assert(RAND_CALLS==3 and A.State(0).LastMission.Result=='LEADER_FREE_STATE')
          ADVANCE(95); assert(A.State(0).ProductionUnlock==1110)
          SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}
          local notes=#NOTES; A.Tick(); assert(#NOTES==notes)
          GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now; rebuild(); EVENTS={}
-- CASE test_round_trip_reload_reinitializes_script_without_reroll
-- SEGMENT
          A=JAC_ASN; A.Tick(); assert(A.State(0).ProductionUnlock==1110)
          assert(Players[1]:GetProperty(A.NATION).LeaderDead==1)
          local ok,r=A.StartProduction(0,11); assert(not ok and r=='PRODUCTION_LOCK')
-- CASE test_all_post_death_outcomes_round_trip_without_reapplying_assets
-- SEGMENT
                  Players[1]:SetProperty(A.INPUT,{Successor='HEIR',Congress={Enabled=true,Votes={['0']='SUCCESSION'}}})
                  FORCE(true,false,'"""+branch+"""'); assert(A.StartAttempt(0,U,1,'LEADER'))
                  ADVANCE(1); SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}
                  ADVANCE(4); EXPECTED=clone(A.Nation(1))
                  GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now; rebuild(); EVENTS={}
-- CASE test_all_post_death_outcomes_round_trip_without_reapplying_assets
-- SEGMENT
                  A=JAC_ASN; ADVANCE(4); local n=A.Nation(1)
                  assert(n.State==EXPECTED.State and n.CurrentLeader==EXPECTED.CurrentLeader and n.Recipient==EXPECTED.Recipient)
                  assert(n.Plan.Branch==EXPECTED.Plan.Branch and n.Plan.Resolution==EXPECTED.Plan.Resolution and RAND_CALLS==3)
                  SAVED={Game=clone(GAME_PROPS),Players=clone(DATA),Now=NOW}; COUNT=#TRANSFERS
                  GAME_PROPS,DATA,NOW=clone(SAVED.Game),clone(SAVED.Players),SAVED.Now; rebuild(); EVENTS={}
-- CASE test_earlier_pending_save_migrates_plan_without_new_rng
-- SEGMENT
          Players[1]:SetProperty(A.INPUT,{Successor='OLD_PERSON'})
          FORCE(true,false,'SUCCESSION'); assert(A.StartAttempt(0,U,1,'LEADER'))
          local s=A.State(0); s.Mission.PostDeathPlan=nil; s.Mission.Snapshot.Successor='OLD_PERSON'
          s.Mission.Snapshot.Recipient=nil; Players[0]:SetProperty(A.STATE,s)
          ADVANCE(5); assert(A.Nation(1).CurrentLeader=='OLD_PERSON' and RAND_CALLS==3)
          Players[1]:SetProperty(A.NATION,nil)
          assert(A.Nation(1).CurrentLeader=='OLD_PERSON') -- use Game archive when native slot data is unavailable
-- CASE test_live_fixture_reveals_hidden_target_without_disabling_gameplay_visibility
-- SEGMENT
          REVEALS=0; HIDDEN_CITY=true; HIDDEN_UNIT=true
          Map.GetPlot=function(x,y) return {GetIndex=function() return x*1000+y end} end
          PlayersVisibility[0].ChangeVisibilityCount=function(_,index,count)
            assert(index==2000 and count==1); REVEALS=REVEALS+1
            HIDDEN_CITY=false; HIDDEN_UNIT=false
          end
-- CASE test_live_fixture_reveals_unit_tile_separately_from_known_capital
-- SEGMENT
          VISIBLE_TILES={[2000]=true}; REVEALED_TILES={[2000]=true,[3001]=true}; REVEALS=0
          Map.GetPlot=function(x,y) return {GetIndex=function() return x*1000+y end} end
          PlayersVisibility[0].IsRevealed=function(_,x,y) return REVEALED_TILES[x*1000+y]==true end
          PlayersVisibility[0].IsVisible=function(_,x,y) return VISIBLE_TILES[x*1000+y]==true end
          PlayersVisibility[0].ChangeVisibilityCount=function(_,index,count)
            assert(index==3001 and count==1); REVEALS=REVEALS+1
            VISIBLE_TILES[index]=true; REVEALED_TILES[index]=true
          end
          VICTIM=GRANT(1,4,3,1):GetID()
-- CASE test_live_fixture_retries_unplaced_unit_without_killing_existing_units
-- SEGMENT
          Game:SetProperty(A.TEST,1); GRANT_CALLS=0
          UnitManager.InitUnitValidAdjacentHex=function(owner,kind,x,y,radius)
            GRANT_CALLS=GRANT_CALLS+1; assert(radius==GRANT_CALLS)
            local u=GRANT(owner,GameInfo.Units[kind].Index,radius==1 and -9999 or x,y)
            if radius==1 then BAD_GRANT=u:GetID() end
          end
-- CASE helper_live_watch_setup
-- SEGMENT
          Game.GetLocalPlayer=function() return 0 end; Game:SetProperty(A.TEST,1)
          Players[0]:SetProperty(A.STATE,{LastProducedUnit=U2})
          Game:SetProperty('JAC_ASN_LIVE_TEST',{Actor=0,LockState={LastProducedUnit=U},ExpectedNativeDeath=U})
-- CASE test_live_unit_watch_accepts_witnessed_native_death_and_removes_observers
-- SEGMENT
          UI_HANDLERS.UnitDamageChanged(0,U,100,83)
          UI_HANDLERS.UnitRemovedFromMap(0,U); DATA[0].Units[U]=nil
-- CASE test_live_unit_watch_accepts_witnessed_native_death_and_removes_observers
-- SEGMENT
          assert(Players[0]:GetUnits():FindID(U2))
          assert(JAC_ASN_LIVE_WATCH==nil and UI_HANDLERS.UnitDamageChanged==nil and UI_HANDLERS.UnitRemovedFromMap==nil)
-- CASE helper_panel_setup
-- SEGMENT
          Game.GetLocalPlayer=function() return 0 end
          Mouse={eLClick=1}; PlayerOperations={EXECUTE_SCRIPT=1}; UI_CALLS={}
          UI={GetHeadSelectedUnit=function() return Players[0]:GetUnits():FindID(U) end,
            RequestPlayerOperation=function(id,kind,params) UI_CALLS[#UI_CALLS+1]={ID=id,Kind=kind,Params=clone(params)} end}
          function Control()
            return {SetText=function(self,v) self.Text=v end,SetDisabled=function(self,v) self.Disabled=v end,
              SetToolTipString=function(self,v) self.Tooltip=v end,SetHide=function(self,v) self.Hidden=v end,
              IsHidden=function(self) return self.Hidden end,RegisterCallback=function(self,event,f) self.Callback=f end,
              CalculateSize=function() end,CalculateInternalSize=function() end}
          end
          Controls={}; for _,name in ipairs({'OpenButton','CloseButton','Panel','Rows','Status','Result','Scroll'}) do
            Controls[name]=Control()
          end
          Controls.Panel:SetHide(true)
          UI_ROWS={}
          InstanceManager={new=function() return {
            ResetInstances=function() UI_ROWS={} end,
            GetInstance=function() local row={Text=Control(),Action=Control()}; UI_ROWS[#UI_ROWS+1]=row; return row end} end}
-- CASE test_panel_open_close_produce_cancel_refresh_and_inactive_buttons
-- SEGMENT
          assert(Controls.Panel:IsHidden()); Controls.OpenButton.Callback(); assert(not Controls.Panel:IsHidden())
          local action=UI_ROWS[1].Action; assert(action.Text=='LOC_JAC_ASN_PRODUCE' and not action.Disabled)
          action.Callback(); local call=UI_CALLS[1]
          assert(call.ID==0 and call.Kind==PlayerOperations.EXECUTE_SCRIPT and call.Params.City==10)
          EVENTS.JAC_AssassinOperation(0,call.Params); UI_HANDLERS.GameCoreEventPublishComplete()
          assert(UI_ROWS[1].Action.Text=='LOC_JAC_ASN_CANCEL'); UI_ROWS[1].Action.Callback()
          EVENTS.JAC_AssassinOperation(0,UI_CALLS[2].Params); UI_HANDLERS.GameCoreEventPublishComplete()
          assert(UI_ROWS[1].Action.Text=='LOC_JAC_ASN_PRODUCE')
          Controls.CloseButton.Callback(); assert(Controls.Panel:IsHidden())
          Players[0].IsTurnActive=function() return false end; UI_HANDLERS.LocalPlayerTurnBegin()
          for _,row in ipairs(UI_ROWS) do assert(row.Action.Disabled) end
          local before=#UI_CALLS; UI_ROWS[1].Action.Callback(); assert(#UI_CALLS==before)
-- CASE test_panel_lists_visible_non_major_units_without_leader_candidates
-- SEGMENT
          for _,row in ipairs(UI_ROWS) do if row.Action.Text=='LOC_JAC_ASN_ATTEMPT' then row.Action.Callback() end end
          local found=false
          for _,call in ipairs(UI_CALLS) do if call.Params.Target==3 then
            assert(call.Params.Action=='ATTEMPT' and call.Params.Kind=='UNIT' and call.Params.Object==V)
            found=true
          end end
          assert(found,'visible non-major unit missing from panel')
          HIDDEN_UNIT=true; UI_CALLS={}; UI_HANDLERS.UnitSelectionChanged()
          for _,row in ipairs(UI_ROWS) do if row.Action.Text=='LOC_JAC_ASN_ATTEMPT' then row.Action.Callback() end end
          for _,call in ipairs(UI_CALLS) do assert(call.Params.Target~=3,'hidden non-major unit leaked') end
-- CASE test_non_major_person_targets_are_rejected_in_core_without_excluding_units
-- SEGMENT
          DATA[3].Alive=true
          DATA[3].Cities[10]={ID=10,Owner=3,X=2,Y=0,Properties={}}
          DATA[3].Properties[A.INPUT]={Governors={['4']={City=10,Identity='MINOR_GOVERNOR'}}}
          local victim=GRANT(3,4,2,0):GetID(); rebuild()
          for _,kind in ipairs({'LEADER','GOVERNOR'}) do
            local dest,why=A.ResolveTarget(3,kind,4); assert(dest==nil and why=='TARGET_KIND')
            local ok,why=A.StartAttempt(0,U,3,kind,4); assert(not ok and why=='TARGET_KIND')
          end
          assert(RAND_CALLS==0 and Players[0]:GetProperty(A.STATE)==nil and #KILLS==0)
          assert(A.CheckAttempt(0,U,3,'UNIT',victim),'non-major units must remain eligible')
-- CASE test_full_generated_live_sequence_resolves_in_18_real_ticks_and_round_trips
-- SEGMENT
          MAX_MAJOR=4
          local civs={'ODA','MATSUDAIRA','IMAGAWA','ASAKURA','ASHIKAGA'}
          DATA={}
          for id=0,MAX_MAJOR+1 do
            DATA[id]={Properties={},Units={},Cities={},Capital=10,Alive=id<=MAX_MAJOR,
              Wars={},Allies={},Favor=0,Military=0,Religion=-1,
              Civilization=civs[id+1] and 'CIVILIZATION_JAC_'..civs[id+1] or 'CIVILIZATION_FREE_CITIES'}
            if id<=MAX_MAJOR then for _,cid in ipairs({10,11}) do
              DATA[id].Cities[cid]={ID=cid,Owner=id,X=id*2,Y=cid-10,Properties={}}
            end end
          end
          rebuild()
          function JAC_AssassinProduce(city)
            EVENTS.JAC_AssassinOperation(0,{Action='PRODUCE',City=city}); return true
          end
          function JAC_AssassinAttempt(unit,target,kind)
            EVENTS.JAC_AssassinOperation(0,{Action='ATTEMPT',Unit=unit,Target=target,Kind=kind}); return true
          end
          LIVE_SAVES={}
-- CASE test_live_final_check_rejects_penalty_added_during_load
-- SEGMENT
          Game:SetProperty(A.NATIONS,{['1']={State='FREE_STATE'},['2']={CurrentLeader='JAC_LIVE_HEIR'},
            ['3']={State='ABSORPTION'},['4']={Branch='CONGRESS'}})
          Game:SetProperty('JAC_ASN_LIVE_TEST',{Actor=0,Target=1,LossTarget=1,Houses={CIVILIZATION_JAC_IMAGAWA=2,
            CIVILIZATION_JAC_ASAKURA=3,CIVILIZATION_JAC_ASHIKAGA=4},
            LockState={LastProducedUnit=U},
            FinalGrievances=100,FinalFriendship=-40,FinalApplied={m=1},FinalMissionID='m',FinalWait=1100})
          Players[1]:SetProperty('JAC_ASN_DIPLOMACY_V1',{Grievances={['0']=100},Friendship={['0']=-40},Applied={m=1}})
          Players[0]:SetProperty(A.STATE,{LastMission={ID='m',Result='ACTOR_GONE'},CommandUnlock=1100,LastProducedUnit=U2})
          local t=Game:GetProperty('JAC_ASN_LIVE_TEST'); t.FinalNations=Game:GetProperty(A.NATIONS)
          t.FinalState=A.State(0); Game:SetProperty('JAC_ASN_LIVE_TEST',t)
-- CASE test_live_final_check_rejects_penalty_added_during_load
-- SEGMENT
          local ledger=Players[1]:GetProperty('JAC_ASN_DIPLOMACY_V1')
          ledger.Grievances['0']=125; Players[1]:SetProperty('JAC_ASN_DIPLOMACY_V1',ledger)
