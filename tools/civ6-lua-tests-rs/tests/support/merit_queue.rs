use super::*;

fn run(code: &str) {
    let lua = new_lua();
    lua.globals()
        .set(
            "LOAD_CONTEXTS",
            lua.create_function(|lua, ()| load_contexts(lua)).unwrap(),
        )
        .unwrap();
    initialize(&lua).unwrap();
    lua.load(include_str!("../../fixtures/persona/cases.lua"))
        .exec()
        .unwrap();
    lua.load(code)
        .set_name("qualified-merit-save-load")
        .exec()
        .unwrap();
}

const FREEZE: &str = r#"
settled()
local queue=assert(ExposedMembers.JAC_LeaderPersona.MeritQueue)
-- FireTuner does not share this script's global environment. The diagnostic
-- must use the exact table held by the production callback's lexical scope.
JAC_PersonaMeritQueue=nil
local drain=queue.Drain
queue.Drain=function() end
"#;

#[test]
fn repaired_malformed_queue_never_requalifies_the_same_building_notification() {
    run(r#"
        settled(); set_merit(0)
        local get=CITIES[0][1].GetProperty; CITIES[0][1].GetProperty=nil
        Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',false)
        local reached=false
        GameEvents.BuildingConstructed.Add(function() reached=true end)
        FIRE('BuildingConstructed',0,1,0,100,true)
        assert(reached and prop('MERIT')==0)
        assert(Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')==false)
        reload()
        CITIES[0][1].GetProperty=get
        Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',nil)
        FIRE('BuildingConstructed',0,1,0,100,true)
        assert(prop('MERIT')==0,'repaired queue requalified an old unknown notification')
        TURN=2; reload(); FIRE('BuildingConstructed',0,1,0,100,true)
        assert(prop('MERIT')==0,'reload or a later turn erased the old identity')
        FIRE('BuildingConstructed',0,1,1,100,true)
        assert(prop('MERIT')==3,'a new eligible identity was discarded')
        FIRE('BuildingConstructed',0,1,1,100,true)
        assert(prop('MERIT')==3,'the new eligible identity paid twice')
    "#);
}

#[test]
fn malformed_queue_preserves_proven_event_amount_cap_and_order_until_recovery() {
    run(r#"
        settled(); set_merit(0)
        Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',false)
        FIRE('BuildingConstructed',0,1,0,100,true)
        FIRE('BuildingConstructed',0,1,1,100,true)
        FIRE('BuildingConstructed',0,1,2,100,true)
        assert(prop('MERIT')==0 and Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')==false)
        local saved=Game:GetProperty('JAC_PERSONA_MERIT_RECOVERY_DEEDS_V1')
        assert(count(saved)==3 and saved[DATE..'_BUILDING_100_0'].amount==3)
        for building=0,2 do assert(saved[DATE..'_BUILDING_100_'..building].recoverySequence==building+1) end
        FIRE('BuildingConstructed',0,1,0,100,true)
        assert(count(saved)==3)
        local profile=GameInfo.JAC_PersonaTreePeople[DATE].MeritProfile
        local rule=GameInfo.JAC_PersonaMeritRules[profile]; local amount=rule.BUILDING
        rule.BUILDING=30
        NATIVE_GOVS[0][GameInfo.Governors[GameInfo.JAC_LeaderPersonas[DATE].GovernorType].Hash].neutralized=5
        Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',nil)
        reload()
        assert(prop('MERIT')==6 and prop('BUILDING_COUNT')==2 and prop('BUILDING_COUNT_TURN')==1)
        assert(Game:GetProperty('JAC_PERSONA_MERIT_RECOVERY_DEEDS_V1')==nil)
        for building=0,2 do FIRE('BuildingConstructed',0,1,building,100,true) end
        assert(prop('MERIT')==6,'recovery reinterpreted eligibility or repaid an identity')
        rule.BUILDING=amount
        NATIVE_GOVS[0][GameInfo.Governors[GameInfo.JAC_LeaderPersonas[DATE].GovernorType].Hash].neutralized=0
        TURN=2; FIRE('BuildingConstructed',0,1,3,100,true)
        assert(prop('MERIT')==9,'a later normal event did not receive its own reward')
    "#);
}

#[test]
fn recovered_events_keep_original_owner_city_and_cross_deed_notification_order() {
    run(r#"
        settled(); set_merit(0)
        Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',false)
        FIRE('BuildingConstructed',1,1,0,100,true)
        FIRE('BuildingConstructed',0,2,0,101,true)
        assert(Game:GetProperty('JAC_PERSONA_MERIT_RECOVERY_DEEDS_V1')==nil)
        FIRE('BuildingConstructed',0,1,0,100,true)
        local queue=ExposedMembers.JAC_LeaderPersona.MeritQueue
        local rules=JAC_PersonaTreeRules
        local a={Get=function(l,f) return Game:GetProperty('JAC_PERSONA_'..l..'_'..f) end,
            Set=function(l,f,v) Game:SetProperty('JAC_PERSONA_'..l..'_'..f,v) end,
            Player=function(owner) return Players[owner] end,
            Parameter=function(name) return GameInfo.JAC_PersonaTreeParameters[name].Value end,
            Merit=function(l) return rules.Merit(l) end}
        queue.Capture(GameInfo.JAC_LeaderPersonas[DATE],'VICTORY','later',CITIES[0][1],a)
        local events=Game:GetProperty('JAC_PERSONA_MERIT_RECOVERY_DEEDS_V1')
        assert(events[DATE..'_BUILDING_100_0'].recoverySequence==1)
        assert(events[DATE..'_VICTORY_later'].recoverySequence==2)
        local commits={}; local set=Game.SetProperty
        Game.SetProperty=function(self,key,value)
            if key=='JAC_PERSONA_'..DATE..'_DEED_BUILDING_100_0' or key=='JAC_PERSONA_'..DATE..'_DEED_VICTORY_later' then commits[#commits+1]=key end
            return set(self,key,value)
        end
        Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',nil)
        queue.Drain(a)
        assert(commits[1]=='JAC_PERSONA_'..DATE..'_DEED_BUILDING_100_0' and commits[2]=='JAC_PERSONA_'..DATE..'_DEED_VICTORY_later')
        local merit=prop('MERIT'); assert(merit>3)
        queue.Drain(a); assert(prop('MERIT')==merit and #commits==2)
    "#);
}

#[test]
fn repaired_sequence_keeps_unknown_eligibility_but_preserves_known_event_rewards() {
    for ready in [false, true] {
        run(&format!(
            r#"
            settled(); set_merit(0)
            local get=CITIES[0][1].GetProperty
            if {ready}==false then CITIES[0][1].GetProperty=nil end
            Game:SetProperty('JAC_PERSONA_PENDING_SEQUENCE',false)
            FIRE('BuildingConstructed',0,1,0,100,true)
            assert(prop('MERIT')==0 and Game:GetProperty('JAC_PERSONA_PENDING_SEQUENCE')==false)
            CITIES[0][1].GetProperty=get
            Game:SetProperty('JAC_PERSONA_PENDING_SEQUENCE',nil)
            reload(); FIRE('BuildingConstructed',0,1,0,100,true)
            assert(prop('MERIT')==({ready} and 3 or 0),'event-time eligibility was lost or reinterpreted')
            FIRE('BuildingConstructed',0,1,0,100,true)
            assert(prop('MERIT')==({ready} and 3 or 0),'replayed building paid twice')
            TURN=2; FIRE('BuildingConstructed',0,1,1,100,true)
            assert(prop('MERIT')==({ready} and 6 or 3),'new eligible event failed')
            FIRE('BuildingConstructed',0,1,1,100,true)
            assert(prop('MERIT')==({ready} and 6 or 3))
        "#
        ));
    }
}

#[test]
fn damaged_identity_or_changed_owner_never_adopts_a_recovery_event() {
    run(r#"
        settled(); set_merit(0)
        local key=DATE..'_BUILDING_100_0'
        Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',{[key]=false})
        FIRE('BuildingConstructed',0,1,0,100,true)
        assert(prop('MERIT')==0 and Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')[key]==false)
        Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')[key]=nil
        reload(); FIRE('BuildingConstructed',0,1,0,100,true)
        assert(prop('MERIT')==0,'clearing a damaged per-identity value allowed retrospective merit')
        Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',false)
        FIRE('BuildingConstructed',0,1,1,100,true)
        local stored=Game:GetProperty('JAC_PERSONA_MERIT_RECOVERY_DEEDS_V1')
        assert(stored[DATE..'_BUILDING_100_1'].owner==0)
        Game:SetProperty('JAC_PERSONA_'..DATE..'_OWNER',1)
        Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',nil)
        reload()
        assert(prop('MERIT')==0,'a later owner inherited an older event')
        local pending=Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')
        assert(pending[DATE..'_BUILDING_100_1'].owner==0,'recovery rewrote original ownership')
    "#);
}

#[test]
fn direct_alliance_defer_preserves_a_false_entry_and_unknown_replay() {
    for formation in [false, true] {
        run(&format!(
            r#"
            settled(); set_merit(0)
            local key=DATE..'_ALLIANCE_1'
            local saved={{[key]=false}}
            Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',saved)
            Game:SetProperty('JAC_PERSONA_'..DATE..'_ALLIANCES_BASELINED',1)
            CITIES[0][1].GetProperty=nil
            Players[0].allies[1]=true
            if {formation} then FIREUI('DiplomacyRelationshipChanged',0,1)
            else FIRE('PlayerTurnStarted',0) end
            assert(Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')==saved and saved[key]==false,'direct Defer replaced an existing false identity')
            assert(prop('MERIT')==0 and prop('ALLY_SEEN_1')==1)
            reload(); FIREUI('DiplomacyRelationshipChanged',0,1)
            assert(saved[key]==false and prop('MERIT')==0)
        "#
        ));
    }
}

#[test]
fn a_recovery_conflict_keeps_the_original_bucket_until_both_events_settle() {
    run(r#"
        settled(); set_merit(0)
        local first=DATE..'_BUILDING_100_0'
        local bucket=DATE..'_BUILDING_1'
        Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',false)
        FIRE('BuildingConstructed',0,1,0,100,true)
        local original=Game:GetProperty('JAC_PERSONA_MERIT_RECOVERY_DEEDS_V1')[first]
        assert(original.amount==3 and original.reservedCount==1)
        Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',{[first]=false})
        FIRE('BuildingConstructed',0,1,1,100,true)
        local buckets=Game:GetProperty('JAC_PERSONA_MERIT_BUCKETS_V2')
        assert(type(buckets)=='table' and buckets[bucket]==2,'another commit erased a recovery reservation')
        assert(Game:GetProperty('JAC_PERSONA_MERIT_RECOVERY_DEEDS_V1')[first]==original)
        Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')[first]=nil
        reload()
        assert(prop('MERIT')==6,'the original proven event was stranded after resolving the conflict')
        assert(Game:GetProperty('JAC_PERSONA_MERIT_RECOVERY_DEEDS_V1')==nil)
        FIRE('BuildingConstructed',0,1,0,100,true)
        FIRE('BuildingConstructed',0,1,1,100,true)
        assert(prop('MERIT')==6)
    "#);
}

#[test]
fn malformed_saved_queue_keeps_building_callbacks_and_saved_values_intact() {
    for value in ["false", "true", "1", "0.5", "'invalid'"] {
        run(&format!(
            r#"
            settled(); set_merit(0)
            local damaged={value}
            local buckets={{keep='original'}}
            Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',damaged)
            Game:SetProperty('JAC_PERSONA_MERIT_BUCKETS_V2',buckets)
            Game:SetProperty('JAC_PERSONA_PENDING_SEQUENCE',17)
            for reloadIndex=1,2 do
                local reached=false
                GameEvents.BuildingConstructed.Add(function() reached=true end)
                FIRE('BuildingConstructed',0,1,0,100,true)
                assert(reached, 'later building callback did not run')
                assert(prop('MERIT')==0 and prop('DEED_BUILDING_100_0')==nil)
                assert(Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')==damaged)
                assert(Game:GetProperty('JAC_PERSONA_MERIT_BUCKETS_V2')==buckets)
                assert(Game:GetProperty('JAC_PERSONA_PENDING_SEQUENCE')==17)
                reload()
            end
        "#
        ));
    }
}

#[test]
fn malformed_saved_queue_keeps_turn_callbacks_without_late_merit() {
    for value in ["false", "true", "1", "'invalid'"] {
        run(&format!(
            r#"
            settled(); set_merit(0)
            local damaged={value}
            Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',damaged)
            Game:SetProperty('JAC_PERSONA_PENDING_SEQUENCE',17)
            local reached=false
            GameEvents.PlayerTurnStarted.Add(function() reached=true end)
            FIRE('PlayerTurnStarted',0)
            assert(reached and prop('MERIT')==0 and prop('GOVERN_LAST_TURN')==1)
            assert(Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')==damaged)
            assert(Game:GetProperty('JAC_PERSONA_PENDING_SEQUENCE')==17)
            reload()
            FIRE('PlayerTurnStarted',0)
            assert(prop('MERIT')==0 and Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')==damaged)
            -- Model a separately repaired save only inside this Lua fixture.
            -- The production callback must not replay the missed turn.
            Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',nil)
            FIRE('PlayerTurnStarted',0)
            assert(prop('MERIT')==0)
            TURN=2; FIRE('PlayerTurnStarted',0)
            assert(prop('MERIT')==2 and prop('GOVERN_LAST_TURN')==2)
            FIRE('BuildingConstructed',0,1,0,100,true)
            assert(prop('MERIT')==5)
            FIRE('BuildingConstructed',0,1,0,100,true)
            assert(prop('MERIT')==5)
        "#
        ));
    }
}

#[test]
fn deferred_deeds_preserve_invalid_saved_sequence_without_throwing() {
    for value in [
        "false",
        "-1",
        "0.5",
        "'invalid'",
        "9007199254740991",
        "math.huge",
    ] {
        run(&format!(
            r#"
            settled(); set_merit(0)
            CITIES[0][1].GetProperty=nil
            local damaged={value}
            local pending={{}}
            Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',pending)
            Game:SetProperty('JAC_PERSONA_PENDING_SEQUENCE',damaged)
            local reached=false
            GameEvents.BuildingConstructed.Add(function() reached=true end)
            FIRE('BuildingConstructed',0,1,0,100,true)
            assert(reached and prop('MERIT')==0 and next(pending)==nil)
            assert(Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')==pending)
            assert(Game:GetProperty('JAC_PERSONA_PENDING_SEQUENCE')==damaged)
            assert(Game:GetProperty('JAC_PERSONA_PENDING_NATIVE_HISTORY')==1)
            local unknown=prop('DEED_UNKNOWN_BUILDING_100_0')
            assert(type(unknown)=='table' and unknown.owner==0 and unknown.turn==1 and unknown.eligibilityUnknown==true)
        "#
        ));
    }
}

#[test]
fn qualified_buildings_reserve_in_order_and_settle_once_on_context_restore() {
    run(&format!(
        r#"{FREEZE}
        for building=0,2 do FIRE('BuildingConstructed',0,1,building,100,true) end
        assert(prop('MERIT')==nil or prop('MERIT')==0)
        local pending=Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')
        assert(count(pending)==3)
        for building=0,2 do
            local event=pending[DATE..'_BUILDING_100_'..building]
            assert(event.sequence==building+1 and event.eligible==true and event.eligibilityUnknown==false)
            assert(event.amount==(building<2 and 3 or 0) and event.turn==1 and event.owner==0)
        end
        assert(Game:GetProperty('JAC_PERSONA_PENDING_NATIVE_HISTORY')==nil)
        FIRE('BuildingConstructed',0,1,0,100,true)
        assert(count(pending)==3 and Game:GetProperty('JAC_PERSONA_PENDING_SEQUENCE')==3)
        -- Current placement deliberately differs from the proven event state.
        NATIVE_GOVS[0][GameInfo.Governors[GameInfo.JAC_LeaderPersonas[DATE].GovernorType].Hash].neutralized=5
        reload()
        assert(prop('MERIT')==6 and prop('BUILDING_COUNT')==2 and prop('BUILDING_COUNT_TURN')==1)
        assert(Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')==nil)
        for building=0,2 do assert(prop('DEED_BUILDING_100_'..building)==1) end
        reload()
        TURN=2
        for building=0,2 do FIRE('BuildingConstructed',0,1,building,100,true) end
        assert(prop('MERIT')==6, 'cap-excess or duplicate deed paid after reload')
    "#
    ));
}

#[test]
fn delayed_older_turn_never_overwrites_newer_turn_counter() {
    run(&format!(
        r#"{FREEZE}
        FIRE('BuildingConstructed',0,1,0,100,true)
        TURN=2
        Game:SetProperty('JAC_PERSONA_'..DATE..'_BUILDING_COUNT_TURN',2)
        Game:SetProperty('JAC_PERSONA_'..DATE..'_BUILDING_COUNT',1)
        reload()
        assert(prop('MERIT')==3 and prop('BUILDING_COUNT_TURN')==2 and prop('BUILDING_COUNT')==1)
        FIRE('BuildingConstructed',0,1,1,100,true)
        assert(prop('MERIT')==6 and prop('BUILDING_COUNT')==2)
        FIRE('BuildingConstructed',0,1,2,100,true)
        assert(prop('MERIT')==6 and prop('DEED_BUILDING_100_2')==2)
    "#
    ));
}

#[test]
fn legacy_unknown_record_cannot_be_promoted_by_ready_native_state() {
    run(&format!(
        r#"{FREEZE}
        local unknown={{leader=DATE,owner=0,deed='BUILDING',identity='100_9',turn=1,sequence=1,eligibilityUnknown=true}}
        Game:SetProperty('JAC_PERSONA_PENDING_DEEDS',{{[DATE..'_BUILDING_100_9']=unknown}})
        Game:SetProperty('JAC_PERSONA_PENDING_SEQUENCE',1)
        FIRE('BuildingConstructed',0,1,0,100,true)
        reload()
        assert(prop('MERIT')==3)
        local pending=Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')
        assert(count(pending)==1 and pending[DATE..'_BUILDING_100_9']==unknown)
        assert(prop('DEED_BUILDING_100_9')==nil and Game:GetProperty('JAC_PERSONA_PENDING_NATIVE_HISTORY')==1)
        FIRE('BuildingConstructed',0,1,9,100,true)
        reload()
        assert(prop('MERIT')==3 and count(Game:GetProperty('JAC_PERSONA_PENDING_DEEDS'))==1)
    "#
    ));
}

#[test]
fn owner_mismatch_and_malformed_qualified_records_never_change_merit() {
    for alteration in [
        "event.owner=2",
        "event.turn=2",
        "event.amount=100",
        "event.reservedCount=0",
        "event.version=nil",
        "event.eligibilityUnknown=nil",
        "event.cityID=nil",
        "event.identity='other'",
        "event.cap=nil",
        "event.reward=nil",
    ] {
        run(&format!(
            r#"{FREEZE}
            FIRE('BuildingConstructed',0,1,0,100,true)
            local event=Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')[DATE..'_BUILDING_100_0']
            {alteration}
            reload()
            assert((prop('MERIT') or 0)==0 and prop('DEED_BUILDING_100_0')==nil)
            assert(count(Game:GetProperty('JAC_PERSONA_PENDING_DEEDS'))==1)
        "#
        ));
    }
}

#[test]
fn pending_event_keeps_captured_rule_values_across_balance_update() {
    run(&format!(
        r#"{FREEZE}
        FIRE('BuildingConstructed',0,1,0,100,true)
        local profile=GameInfo.JAC_PersonaTreePeople[DATE].MeritProfile
        local rule=GameInfo.JAC_PersonaMeritRules[profile]
        local cap=GameInfo.JAC_PersonaTreeParameters.BUILDING_DEEDS_PER_TURN
        local oldReward,oldCap=rule.BUILDING,cap.Value
        rule.BUILDING=30; cap.Value=1
        reload()
        assert(prop('MERIT')==3 and prop('BUILDING_COUNT')==1)
        rule.BUILDING=oldReward; cap.Value=oldCap
        reload()
        assert(prop('MERIT')==3)
    "#
    ));
}

#[test]
fn duplicate_order_is_rejected_and_saved_receipt_prevents_repayment() {
    run(&format!(
        r#"{FREEZE}
        FIRE('BuildingConstructed',0,1,0,100,true)
        FIRE('BuildingConstructed',0,1,1,100,true)
        local pending=Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')
        pending[DATE..'_BUILDING_100_1'].sequence=1
        reload()
        assert((prop('MERIT') or 0)==0 and count(pending)==2)
        pending[DATE..'_BUILDING_100_1'].sequence=2
        -- This entry already has an independently saved commit receipt.
        Game:SetProperty('JAC_PERSONA_'..DATE..'_DEED_BUILDING_100_0',1)
        Game:SetProperty('JAC_PERSONA_'..DATE..'_MERIT',3)
        reload()
        assert(prop('MERIT')==6 and Game:GetProperty('JAC_PERSONA_PENDING_DEEDS')==nil)
        reload()
        assert(prop('MERIT')==6)
    "#
    ));
}
