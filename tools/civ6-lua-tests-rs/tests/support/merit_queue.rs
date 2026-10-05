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
