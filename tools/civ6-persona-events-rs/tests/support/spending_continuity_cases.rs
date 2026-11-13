#[test]
fn normal_past_unrelated_five_cannot_confirm_new_cost_two_lag() {
    let a=normal_context("");
    run(&a,r#"
used[0]=5; l,row=normalJoin(0,1)
local view=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,l)
assert(view.reservation.baseline==5 and view.reservation.expected==5 and view.reservation.serial==0)
success(0,row,0); appointed(0,row); success(0,row,1)
local view=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,l)
assert(view.account.expected==7 and view.account.serial==2 and view.acquired[1].spent==5 and view.acquired[2].spent==5)
assert(view.acquired[1].expectedSpent==6 and view.acquired[2].expectedSpent==7 and view.paid==nil)
finalize(0,l); assert(#unitLists[0]==1 and properties['JAC_PERSONA_'..l..'_STATE']=='EVOLVING')
used[0]=6; finalize(0,l); assert(#unitLists[0]==1)
"#);
    let b=restore_normal(&a,r#"
used[0]=5; Game.GetGreatPeople():GrantPerson(201,7,8,0,0)
l=PlayerConfigurations[1]:GetLeaderTypeName(); row=GameInfo.JAC_LeaderPersonas[l]
"#);
    run(&b,r#"
finalize(0,l); assert(#unitLists[0]==1)
used[0]=7; finalize(0,l)
assert(#unitLists[0]==0 and properties['JAC_PERSONA_'..l..'_STATE']=='GOVERNOR')
local v=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,l)
assert(v.acquired[1].spent==5 and v.acquired[2].spent==5 and v.account.expected==7 and v.account.serial==2)
assert(uiReads==0 and operations==0)
"#);
}

const STOCK_GOVERNOR: &str = r#"
local gs,ps,sets={},{},{}
for g in GameInfo.Governors() do gs[#gs+1]=g end
for p in GameInfo.GovernorPromotions() do ps[#ps+1]=p end
for row in GameInfo.JAC_LeaderPersonas() do
 for step=0,2 do
  local name=row.BasePromotionType..(step==0 and '' or '_TITLE_'..step)
  sets[#sets+1]={GovernorType=row.GovernorType,GovernorPromotion=name}
 end
end
gs[#gs+1]={GovernorType='GOVERNOR_STOCK',Index=240,Hash=8000}
ps[#ps+1]={GovernorPromotionType='STOCK_BASE',Index=4000,Hash=8001,BaseAbility=1}
ps[#ps+1]={GovernorPromotionType='STOCK_TITLE',Index=4001,Hash=8002,BaseAbility=0}
sets[#sets+1]={GovernorType='GOVERNOR_STOCK',GovernorPromotion='STOCK_BASE'}
sets[#sets+1]={GovernorType='GOVERNOR_STOCK',GovernorPromotion='STOCK_TITLE'}
GameInfo.Governors=catalog(gs,'GovernorType'); GameInfo.GovernorPromotions=catalog(ps,'GovernorPromotionType')
GameInfo.GovernorPromotionSets=setmetatable({},{__call=function() local i=0; return function() i=i+1; return sets[i] end end})
"#;

#[test]
fn normal_concurrent_people_and_nonpersona_debits_are_serialized_without_double_base() {
    for reverse in [false,true] {
        let lua=normal_context(STOCK_GOVERNOR);
        run(&lua,&format!(r#"
used[0]=5; l2,r2=normalJoin(0,1); l3,r3=normalJoin(0,2)
if {reverse} then emit(Events,'GovernorPromoted',0,240,4000); emit(Events,'GovernorAppointed',0,240)
else emit(Events,'GovernorAppointed',0,240); emit(Events,'GovernorPromoted',0,240,4000) end
local s=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
assert(s.accounts[0].expected==6 and s.accounts[0].serial==1)
success(0,r3,2); success(0,r2,1); appointed(0,r2)
success(0,r3,0); appointed(0,r3); success(0,r2,0); success(0,r3,1)
emit(Events,'GovernorPromoted',0,240,4001)
s=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
assert(s.accounts[0].expected==12 and s.accounts[0].serial==7)
local sequence=s.sequence
success(0,r2,0); success(0,r2,1); emit(Events,'GovernorPromoted',0,240,4001)
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.accounts[0].serial==7)
used[0]=10; finalize(0,l2); finalize(0,l3); assert(#unitLists[0]==2)
used[0]=11; finalize(0,l2); finalize(0,l3); assert(#unitLists[0]==2)
used[0]=12; finalize(0,l2); finalize(0,l3)
assert(#unitLists[0]==0 and properties['JAC_PERSONA_'..l2..'_STATE']=='GOVERNOR' and properties['JAC_PERSONA_'..l3..'_STATE']=='GOVERNOR')
assert(uiReads==0 and operations==0)
"#));
    }
}

#[test]
fn normal_nonpersona_spending_cannot_supply_missing_person_callback() {
    let lua=normal_context(STOCK_GOVERNOR);
    run(&lua,r#"
l,row=normalJoin(0,1)
success(0,row,0); appointed(0,row)
emit(Events,'GovernorAppointed',0,240); emit(Events,'GovernorPromoted',0,240,4000)
emit(Events,'GovernorPromoted',0,240,4001); used[0]=3
finalize(0,l); assert(#unitLists[0]==1 and not select(2,normalStatus(0,l))[2])
success(0,row,1); finalize(0,l); assert(#unitLists[0]==1)
used[0]=4; finalize(0,l); assert(#unitLists[0]==0)
"#);
}

fn reload_without_subscription(a: &Lua, setup: &str, event: &str) -> Lua {
    let saved: mlua::Table=a.globals().get("properties").unwrap();
    let b=Lua::new(); b.load(FIXTURE).exec().unwrap(); b.load(NORMAL_FIXTURE).exec().unwrap();
    b.globals().set("properties",transfer(&b,saved)).unwrap(); run(&b,setup);
    run(&b,&format!("Events.{event}.Add=function() error('subscription unavailable in new context') end"));
    load(&b,&fs::read_to_string(root().join(GAMEPLAY)).unwrap()); b
}

#[test]
fn normal_saved_unappointed_add_failure_taints_continuity_across_recovery() {
    for event in ["GovernorPromoted","GovernorAppointed"] {
        let a=normal_context(""); run(&a,"l,row=normalJoin(0,1)");
        let b=reload_without_subscription(&a,r#"
used[0]=0; Game.GetGreatPeople():GrantPerson(201,7,8,0,0)
l=PlayerConfigurations[1]:GetLeaderTypeName(); row=GameInfo.JAC_LeaderPersonas[l]
"#,event);
        run(&b,r#"
local v=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,l)
assert(not v.active and v.continuityTainted and v.paid==nil)
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.subscriptionGap)
unitLists[0]={}; turn=0; Game.GetCurrentGameTurn=function() return turn end
ExposedMembers.JAC_LeaderPersona.Reconcile(); turn=1; ExposedMembers.JAC_LeaderPersona.Reconcile()
assert(properties['JAC_PERSONA_'..l..'_STATE']=='EVOLVING')
"#);
        let c=restore_normal(&b,r#"
used[0]=2; Game.GetGreatPeople():GrantPerson(201,7,8,0,0)
l=PlayerConfigurations[1]:GetLeaderTypeName(); row=GameInfo.JAC_LeaderPersonas[l]
Game.GetCurrentGameTurn=function() return 1 end
"#);
        run(&c,r#"
local v=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,l)
assert(v.registrations.GovernorAppointed and v.registrations.GovernorPromoted and not v.active and v.paid==nil)
success(0,row,0); appointed(0,row); success(0,row,1); finalize(0,l)
assert(#unitLists[0]==1 and playerProps[0][901014]==false and uiReads==0 and operations==0)
"#);
    }
}

#[test]
fn normal_paid_save_add_failure_closes_paid_person_and_new_person_admission() {
    let a=normal_context("");
    run(&a,"l,row=normalJoin(0,3); used[0]=1; success(0,row,0); appointed(0,row); finalize(0,l); assert(#unitLists[0]==0)");
    let b=reload_without_subscription(&a,"used[0]=1; l=PlayerConfigurations[3]:GetLeaderTypeName()","GovernorPromoted");
    run(&b,r#"
assert(playerProps[0][901032]==false and ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,l).paid==nil)
emit(GameEvents,'JAC_LeaderPersonaJoin',0,{Source=1})
assert(playerProps[0].JAC_PERSONA_RESULT=='NATIVE_UNAVAILABLE' and #unitLists[0]==0)
"#);
    let c=restore_normal(&b,"used[0]=1; l=PlayerConfigurations[3]:GetLeaderTypeName()");
    run(&c,r#"
assert(ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,l).continuityTainted)
emit(GameEvents,'JAC_LeaderPersonaJoin',0,{Source=1})
assert(playerProps[0].JAC_PERSONA_RESULT=='NATIVE_UNAVAILABLE' and playerProps[0][901032]==false)
"#);
}

#[test]
fn normal_completed_one_then_unappointed_missing_person_releases_only_proven_reservation() {
    for lose_old_proof in [false,true] {
        let lua=normal_context("");
        run(&lua,r#"
la,ra=normalJoin(0,3); used[0]=1; success(0,ra,0); appointed(0,ra); finalize(0,la)
lb,rb=normalJoin(0,8); assert(#unitLists[0]==1)
local v=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,lb)
assert(v.reservation.baseline==1 and v.account.expected==1 and v.paid==0)
unitLists[0]={}; turn=0; Game.GetCurrentGameTurn=function() return turn end
"#);
        if lose_old_proof {run(&lua,"properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.people[la]=nil");}
        run(&lua,"ExposedMembers.JAC_LeaderPersona.Reconcile(); turn=1; ExposedMembers.JAC_LeaderPersona.Reconcile()");
        if lose_old_proof {
            run(&lua,"assert(properties['JAC_PERSONA_'..lb..'_STATE']=='EVOLVING' and ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,lb).reserved==1)");
        } else {
            run(&lua,"assert(properties['JAC_PERSONA_'..lb..'_STATE']=='GREAT_PERSON' and ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,lb).reserved==0)");
        }
        run(&lua,"assert(used[0]==1 and uiReads==0 and operations==0)");
    }
}

#[test]
fn normal_reservation_or_spending_serial_corruption_cannot_confirm_payment() {
    for mutation in [
        "s.people[l].reservation=nil",
        "s.people[l].reservation.baseline=6",
        "s.people[l].acquired[2].expectedSpent=5",
        "s.people[l].acquired[2].spendingSerial=1",
        "s.accounts[0].expected=5",
        "s.accounts[0].debits[2].before=4",
        "s.accounts[0].serial=1",
    ] {
        let lua=normal_context("");
        run(&lua,"used[0]=5; l,row=normalJoin(0,1); success(0,row,0); appointed(0,row); success(0,row,1); used[0]=7; s=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1");
        run(&lua,mutation); run(&lua,"finalize(0,l); assert(#unitLists[0]==1)");
    }
}

#[test]
fn normal_later_unexplained_spending_cannot_rehabilitate_an_earlier_missing_callback() {
    let lua=normal_context("");
    run(&lua,r#"
la,ra=normalJoin(0,1)
used[0]=5 -- unavailable earlier person's success notifications, or other unexplained spending
lb,rb=normalJoin(0,3)
local s=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
assert(s.people[la].reservation.floorSerial<s.accounts[0].floorSerial)
-- Keep B's unit only; A's disappearance must remain unknown despite the new checkpoint.
local remaining={}
for _,u in ipairs(unitLists[0]) do if u:GetGreatPerson():GetIndividual()==3 then remaining[#remaining+1]=u end end
unitLists[0]=remaining
turn=0; Game.GetCurrentGameTurn=function() return turn end
ExposedMembers.JAC_LeaderPersona.Reconcile(); turn=1; ExposedMembers.JAC_LeaderPersona.Reconcile()
assert(properties['JAC_PERSONA_'..la..'_STATE']=='EVOLVING')
assert(ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,la).paid==nil)
"#);
}

#[test]
fn normal_spending_and_subscription_guards_are_red_mutations() {
    let source=fs::read_to_string(root().join(GAMEPLAY)).unwrap();
    for (guard,broken,setup,assertion) in [
        ("    PersonaEvents.Confirm(owner);",
         "",
         "used[0]=5; l,row=normalJoin(0,1); success(0,row,0); appointed(0,row); success(0,row,1); used[0]=7; finalize(0,l)",
         "assert(#unitLists[0]==0,'normal completion needs a real debit confirmation')"),
        ("if not subscribed() or (s and s.continuityTainted) then return nil,c end",
         "if false then return nil,c end",
         "l=PlayerConfigurations[1]:GetLeaderTypeName()",
         "assert(ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,l).paid==nil,'missing subscription must revoke saved absence')"),
    ] {
        assert_eq!(source.matches(guard).count(),if guard.contains("PersonaEvents.Confirm") {2} else {1});
        let lua=Lua::new(); lua.load(FIXTURE).exec().unwrap(); lua.load(NORMAL_FIXTURE).exec().unwrap();
        if guard.starts_with("if not subscribed") {
            let a=normal_context(""); run(&a,"l,row=normalJoin(0,1)");
            let saved: mlua::Table=a.globals().get("properties").unwrap();
            lua.globals().set("properties",transfer(&lua,saved)).unwrap();
            run(&lua,"Events.GovernorPromoted.Add=function() error('Add unavailable') end");
        }
        load(&lua,&source.replace(guard,broken)); run(&lua,setup);
        assert!(lua.load(assertion).exec().is_err());
    }
}
