#[test]
fn unreflected_stock_spending_cannot_overbook_a_new_person_reservation() {
    let lua = normal_context(STOCK_GOVERNOR);
    run(&lua, r#"
        Players[0]:GetGovernors().GetGovernorPoints=function() return 1 end
        emit(Events,'GovernorPromoted',0,240,4000)
        emit(Events,'GovernorAppointed',0,240)
        assert(used[0]==0 and properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.accounts[0].expected==1)
        local leader=PlayerConfigurations[3]:GetLeaderTypeName()
        emit(GameEvents,'JAC_LeaderPersonaJoin',0,{Source=3})
        assert(playerProps[0].JAC_PERSONA_RESULT=='JOINED')
        emit(GameEvents,'JAC_LeaderPersonaEvolve',0,{Leader=leader})
        assert(playerProps[0].JAC_PERSONA_RESULT=='NATIVE_UNAVAILABLE',
            'pending official consumption must prevent a new reservation')
        assert(properties['JAC_PERSONA_'..leader..'_STATE']=='GREAT_PERSON' and #unitLists[0]==1)
        used[0]=1
        emit(GameEvents,'JAC_LeaderPersonaEvolve',0,{Leader=leader})
        assert(playerProps[0].JAC_PERSONA_RESULT=='NO_TITLE')
        assert(properties['JAC_PERSONA_'..leader..'_STATE']=='GREAT_PERSON' and #unitLists[0]==1)
    "#);
}

#[test]
fn unjoined_person_debit_cannot_settle_another_persons_lagging_payment() {
    let lua = normal_context("");
    run(&lua, r#"
        local l,row=normalJoin(0,1)
        used[0]=1; success(0,row,0); appointed(0,row); success(0,row,1)
        finalize(0,l); assert(#unitLists[0]==1)
        local other=GameInfo.JAC_LeaderPersonas[PlayerConfigurations[3]:GetLeaderTypeName()]
        used[0]=2; success(0,other,0); appointed(0,other)
        finalize(0,l)
        assert(#unitLists[0]==1 and properties['JAC_PERSONA_'..l..'_STATE']=='EVOLVING',
            'an unjoined person consumed the extra title, not the pending person')
        assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.accounts[0].expected==3)
        used[0]=3; finalize(0,l)
        assert(#unitLists[0]==0 and properties['JAC_PERSONA_'..l..'_STATE']=='GOVERNOR')
        assert(uiReads==0 and operations==0)
    "#);
}

#[test]
fn confirmed_payment_survives_new_baseline_even_if_next_evolution_is_unaffordable() {
    for affordable in [false, true] {
        let lua = normal_context("");
        run(&lua, r#"
            la,ra=normalJoin(0,3)
            used[0]=1; success(0,ra,0); appointed(0,ra); finalize(0,la)
            assert(#unitLists[0]==0)
            local status=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,la)
            assert(status.paid==1)
            firstConfirmation=clone(status.account.debits[1].confirmed)
            used[0]=2 -- an unrelated debit was not attributed to any person
            emit(GameEvents,'JAC_LeaderPersonaJoin',0,{Source=1})
            assert(playerProps[0].JAC_PERSONA_RESULT=='JOINED')
        "#);
        lua.globals().set("affordable", affordable).unwrap();
        run(&lua, r#"
            Players[0]:GetGovernors().GetGovernorPoints=function() return affordable and 10 or 2 end
            emit(GameEvents,'JAC_LeaderPersonaEvolve',0,{Leader=PlayerConfigurations[1]:GetLeaderTypeName()})
            assert(playerProps[0].JAC_PERSONA_RESULT==(affordable and 'EVOLVING' or 'NO_TITLE'))
            local status=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,la)
            assert(status.paid==1 and properties['JAC_PERSONA_'..la..'_STATE']=='GOVERNOR')
            assert(status.account.debits[1].confirmed.afterSpent==firstConfirmation.afterSpent)
            assert(status.account.debits[1].confirmed.floorSerial==firstConfirmation.floorSerial)
            ExposedMembers.JAC_LeaderPersona.Reconcile()
            assert(playerProps[0][901032]==true and used[0]==2 and uiReads==0 and operations==0)
        "#);
    }
}

#[test]
fn missing_official_promotion_sets_cannot_be_inferred_from_title_names() {
    let lua = normal_context("GameInfo.GovernorPromotionSets=nil");
    run(&lua, r#"
        emit(GameEvents,'JAC_LeaderPersonaJoin',0,{Source=1})
        assert(playerProps[0].JAC_PERSONA_RESULT=='NATIVE_UNAVAILABLE')
        assert(#unitLists[0]==0 and properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1==nil)
    "#);
}
