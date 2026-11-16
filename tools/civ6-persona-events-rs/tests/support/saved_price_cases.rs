// Price updates change GameInfo, not the saved units, acquisition callbacks or
// official spending. Reload executes the shipped producer in a new Lua VM.
#[test]
fn saved_price_survives_price_only_reload_with_partial_and_completed_payments() {
    for old in 1..=3 {
        for new in (1..=3).filter(|new| *new != old) {
            for paid in 0..=old {
                let a = normal_context(&format!(
                    "GameInfo.JAC_LeaderPersonas[PlayerConfigurations[1]:GetLeaderTypeName()].EvolutionCost={old}"
                ));
                run(
                    &a,
                    &format!(
                        r#"
l,row=normalJoin(0,1); key='JAC_PERSONA_'..l..'_'
if {paid}>0 then used[0]=1; success(0,row,0); appointed(0,row) end
for step=1,{paid}-1 do used[0]=used[0]+1; success(0,row,step) end
ExposedMembers.JAC_LeaderPersona.Reconcile()
properties[key..'MERIT']=37; properties[key..'NODE_PRESERVED']=1
history=clone(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.people[l].acquired)
"#
                    ),
                );
                let b = restore_normal(
                    &a,
                    &format!(
                        r#"
l=PlayerConfigurations[1]:GetLeaderTypeName(); row=GameInfo.JAC_LeaderPersonas[l]
row.EvolutionCost={new}; used[0]={paid}; key='JAC_PERSONA_'..l..'_'
if {paid}<{old} then Game.GetGreatPeople():GrantPerson(201,7,8,0,0) end
{PAID_OBSERVER}
"#
                    ),
                );
                run(
                    &b,
                    &format!(
                        r#"
local api=ExposedMembers.JAC_LeaderPersona
assert(api.EventsGameplayStatus(0,l).active,'price-only reload closed normal receipts')
api.Reconcile() -- the existing normal turn callback republishes restored qualifications
assert(api.EvolutionCost(l)=={old},'saved joining price changed')
assert(properties[key..'EVOLUTION_COST']=={old} and properties[key..'EVOLUTION_SURCHARGE']=={old}-1)
assert(properties[key..'MERIT']==37 and properties[key..'NODE_PRESERVED']==1)
assert(api.EventsGameplayStatus(0,l).reserved=={old}-{paid})
assert(playerProps[0][901014]==true,'appointment eligibility was lost after repricing')
assert(paidVisible[0][14]==({paid}=={old}))
assert(paidVisible[1][14]==false,'another owner acquired placement eligibility')
local word=0
for bit=0,45 do if paidBits[0][46+bit] then word=word+2^bit end end
assert(math.floor(word/256)%4=={old},'native catalog transported the new price for a saved person')
local saved=clone(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.people[l].acquired)
if {paid}>0 then success(0,row,0); appointed(0,row) end
for step=1,{paid}-1 do success(0,row,step) end
for k,v in pairs(saved) do
 local p=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.people[l].acquired[k]
 assert(p.sequence==v.sequence and p.spendingSerial==v.spendingSerial and p.binding==v.binding)
end
if {paid}<{old} then
 finalize(0,l); assert(#unitLists[0]==1,'unpaid Great Person was discarded')
 if {paid}==0 then used[0]=1; success(0,row,0); appointed(0,row) end
 for step=math.max(1,{paid}),{old}-1 do used[0]=used[0]+1; success(0,row,step) end
 finalize(0,l)
end
assert(properties[key..'STATE']=='GOVERNOR' and #unitLists[0]==0 and used[0]=={old})
api.Reconcile(); assert(paidVisible[0][14]==true and paidVisible[1][14]==false)
assert(properties[key..'MERIT']==37 and properties[key..'NODE_PRESERVED']==1)
assert(operations==0 and uiReads==0)
-- A different, newly joined person uses the updated current price.
local fresh=GameInfo.JAC_LeaderPersonas[PlayerConfigurations[3]:GetLeaderTypeName()]
fresh.EvolutionCost={new}
local freshLeader=normalJoin(0,3)
assert(api.EvolutionCost(freshLeader)=={new} and api.EvolutionCost(l)=={old})
assert(api.EventsGameplayStatus(0,l).reserved=={new})
"#
                    ),
                );
            }
        }
    }
}

#[test]
fn price_only_update_does_not_repair_damaged_amount_contract_or_identity() {
    for fault in [
        "properties[key..'EVOLUTION_COST']=nil",
        "properties[key..'EVOLUTION_COST']=1; properties[key..'EVOLUTION_SURCHARGE']=0",
        "properties[key..'EVOLUTION_SURCHARGE']=nil",
        "s.prices=nil",
        "s.prices[l]=nil",
        "s.prices[l].cost=0",
        "s.prices[l].cost=1; properties[key..'EVOLUTION_COST']=1; properties[key..'EVOLUTION_SURCHARGE']=0",
        "s.prices[l].binding=nil",
        "s.prices[l].owner=1",
        "s.prices[l].sourcePlayer=2",
        "s.prices[l].personType=GameInfo.JAC_LeaderPersonas[PlayerConfigurations[2]:GetLeaderTypeName()].PersonType",
        "properties[key..'SOURCE']=2",
        "properties[key..'OWNER']=1",
    ] {
        let a=normal_context(PAID_OBSERVER);
        run(&a, &format!(r#"
l,row=normalJoin(0,1); key='JAC_PERSONA_'..l..'_'
used[0]=1; success(0,row,0); appointed(0,row)
ExposedMembers.JAC_LeaderPersona.Reconcile()
row.EvolutionCost=1
local s=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
{fault}
properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1=s
ExposedMembers.JAC_LeaderPersona.Reconcile(); finalize(0,l)
assert(#unitLists[0]==1 and used[0]==1 and operations==0)
assert(properties[key..'STATE']=='EVOLVING')
assert(not paidVisible[0][14] and not paidVisible[1][14])
"#));
    }
}

#[test]
fn old_price_requires_its_official_payment_definition_even_when_current_price_is_lower() {
    let a = normal_context("");
    run(&a,"l,row=normalJoin(0,2); used[0]=1; success(0,row,0); appointed(0,row); ExposedMembers.JAC_LeaderPersona.Reconcile()");
    let b = restore_normal(
        &a,
        r#"
l=PlayerConfigurations[2]:GetLeaderTypeName(); row=GameInfo.JAC_LeaderPersonas[l]
row.EvolutionCost=1; used[0]=1; Game.GetGreatPeople():GrantPerson(202,7,8,0,0)
local missing=GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_2']
GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_2']=nil
"#,
    );
    run(
        &b,
        r#"
finalize(0,l); ExposedMembers.JAC_LeaderPersona.Reconcile()
assert(#unitLists[0]==1 and used[0]==1 and properties['JAC_PERSONA_'..l..'_STATE']=='EVOLVING')
assert(playerProps[0][901023]==false)
"#,
    );
}

#[test]
fn legacy_price_binding_survives_price_update_but_rejects_changed_identity() {
    for old in 1..=3 {
        for new in (1..=3).filter(|price| *price != old) {
            let a=normal_context(&format!("GameInfo.JAC_LeaderPersonas[PlayerConfigurations[1]:GetLeaderTypeName()].EvolutionCost={old}"));
            run(
                &a,
                r#"
l,row=normalJoin(0,1); used[0]=1; success(0,row,0); appointed(0,row)
ExposedMembers.JAC_LeaderPersona.Reconcile()
-- Exact d21 store shape, before independent joining contracts existed.
properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.prices=nil
properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.priceVersion=nil
"#,
            );
            let b = restore_normal(
                &a,
                &format!(
                    r#"
l=PlayerConfigurations[1]:GetLeaderTypeName(); row=GameInfo.JAC_LeaderPersonas[l]
row.EvolutionCost={new}; used[0]=1
if {old}>1 then Game.GetGreatPeople():GrantPerson(201,7,8,0,0) end
"#
                ),
            );
            run(
                &b,
                &format!(
                    r#"
assert(ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,l).active)
assert(ExposedMembers.JAC_LeaderPersona.EvolutionCost(l)=={old})
for step=1,{old}-1 do used[0]=used[0]+1; success(0,row,step) end
finalize(0,l)
assert(properties['JAC_PERSONA_'..l..'_STATE']=='GOVERNOR' and #unitLists[0]==0)
GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_1'].Hash=9999
ExposedMembers.JAC_LeaderPersona.Reconcile()
assert(not ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,l).active)
assert(playerProps[0][901014]==false and used[0]=={old})
"#
                ),
            );
        }
    }
}

#[test]
fn price_increase_may_append_payment_definitions_without_changing_old_identities() {
    let a = normal_context(
        r#"
local row=GameInfo.JAC_LeaderPersonas[PlayerConfigurations[1]:GetLeaderTypeName()]
row.EvolutionCost=1
local removed={[row.BasePromotionType..'_TITLE_1']=true,[row.BasePromotionType..'_TITLE_2']=true}
local promotions,sets={},{}
for p in GameInfo.GovernorPromotions() do if not removed[p.GovernorPromotionType] then promotions[#promotions+1]=p end end
for set in GameInfo.GovernorPromotionSets() do if not removed[set.GovernorPromotion] then sets[#sets+1]=set end end
GameInfo.GovernorPromotions=catalog(promotions,'GovernorPromotionType')
GameInfo.GovernorPromotionSets=catalog(sets,'GovernorPromotion')
"#,
    );
    run(
        &a,
        r#"
l,row=normalJoin(0,1); used[0]=1; success(0,row,0); appointed(0,row); finalize(0,l)
assert(#unitLists[0]==0)
"#,
    );
    // The latest catalog appends both canonical definitions with unused sparse
    // indices. It preserves every old definition, payment and native identity.
    let b = restore_normal(
        &a,
        &format!(
            r#"
l=PlayerConfigurations[1]:GetLeaderTypeName(); row=GameInfo.JAC_LeaderPersonas[l]
row.EvolutionCost=3; used[0]=1
{PAID_OBSERVER}
"#
        ),
    );
    run(
        &b,
        r#"
local api=ExposedMembers.JAC_LeaderPersona
assert(api.EventsGameplayStatus(0,l).active and api.EvolutionCost(l)==1)
api.Reconcile(); assert(paidVisible[0][14]==true and used[0]==1 and #unitLists[0]==0)
local fresh=GameInfo.JAC_LeaderPersonas[PlayerConfigurations[3]:GetLeaderTypeName()]
fresh.EvolutionCost=3
local newLeader=normalJoin(0,3)
assert(api.EvolutionCost(newLeader)==3 and api.EvolutionCost(l)==1)
-- Additional payment types are permitted only without skill effects.
local title=row.BasePromotionType..'_TITLE_1'
GameInfo.GovernorPromotionModifiers=catalog({{GovernorPromotionType=title,ModifierId='UNEXPECTED_EFFECT'}},'GovernorPromotionType')
api.Reconcile(); assert(not api.EventsGameplayStatus(0,l).active)
assert(#unitLists[0]==1 and used[0]==1 and not paidVisible[0][14])
"#,
    );
}
