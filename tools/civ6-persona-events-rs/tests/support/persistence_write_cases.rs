// A table returned by Game can alias its saved value. Refused writes must not
// confirm a debit by mutating that old value before SetProperty accepts it.
#[test]
fn normal_debit_confirmation_refused_write_keeps_the_prior_saved_table() {
    let lua = normal_context("");
    run(
        &lua,
        r#"
l,row=normalJoin(0,3)
assert(row.EvolutionCost==1)
success(0,row,0); appointed(0,row)
local store=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
assert(store.accounts[0].debits[1].confirmed==nil)
local getter,setter=Game.GetProperty,Game.SetProperty
writeAttempts=0
Game.GetProperty=function(self,key)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' then return properties[key] end
 return getter(self,key)
end
Game.SetProperty=function(self,key,value)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' then writeAttempts=writeAttempts+1; return end
 return setter(self,key,value)
end
used[0]=1
ExposedMembers.JAC_LeaderPersona.Reconcile()
assert(writeAttempts>0,'confirmation persistence was not attempted')
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.accounts[0].debits[1].confirmed==nil,
 'refused confirmation write mutated the prior saved debit')
assert(properties['JAC_PERSONA_'..l..'_STATE']=='EVOLVING' and #unitLists[0]==1,
 'unpersisted confirmation finalized the person')
-- Once storage accepts a write, consume only the already captured official
-- receipt. No APPOINT/PROMOTE notification or native operation is replayed.
local sequence=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.sequence
local price=clone(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.prices[l])
Game.SetProperty=setter
ExposedMembers.JAC_LeaderPersona.Reconcile()
local saved=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
assert(saved.accounts[0].debits[1].confirmed~=nil and saved.sequence==sequence)
assert(saved.prices[l].cost==price.cost and saved.prices[l].binding==price.binding)
assert(properties['JAC_PERSONA_'..l..'_STATE']=='GOVERNOR' and #unitLists[0]==0)
assert(used[0]==1 and operations==0 and uiReads==0)
"#,
    );
}

#[test]
fn normal_reservation_refused_write_preserves_the_saved_image_then_recovers() {
    let lua = normal_context("");
    run(
        &lua,
        r#"
l=PlayerConfigurations[1]:GetLeaderTypeName(); row=GameInfo.JAC_LeaderPersonas[l]
emit(GameEvents,'JAC_LeaderPersonaJoin',0,{Source=1})
assert(playerProps[0].JAC_PERSONA_RESULT=='JOINED')
local original=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
local getter,setter=Game.GetProperty,Game.SetProperty
Game.GetProperty=function(self,key)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' then return properties[key] end
 return getter(self,key)
end
Game.SetProperty=function(self,key,value)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' then return end
 return setter(self,key,value)
end
emit(GameEvents,'JAC_LeaderPersonaEvolve',0,{Leader=l})
assert(playerProps[0].JAC_PERSONA_RESULT=='NATIVE_UNAVAILABLE')
assert(original.people[l]==nil and properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1==original,
 'refused reservation write changed the previous store')
assert(properties['JAC_PERSONA_'..l..'_STATE']=='GREAT_PERSON' and #unitLists[0]==1)
Game.SetProperty=setter
emit(GameEvents,'JAC_LeaderPersonaEvolve',0,{Leader=l})
assert(playerProps[0].JAC_PERSONA_RESULT=='EVOLVING')
used[0]=1; success(0,row,0); appointed(0,row)
ExposedMembers.JAC_LeaderPersona.Reconcile()
assert(properties['JAC_PERSONA_'..l..'_STATE']=='EVOLVING' and #unitLists[0]==1)
used[0]=2; success(0,row,1)
ExposedMembers.JAC_LeaderPersona.Reconcile()
assert(properties['JAC_PERSONA_'..l..'_STATE']=='GOVERNOR' and #unitLists[0]==0)
assert(used[0]==2 and operations==0 and uiReads==0)
"#,
    );
}

#[test]
fn normal_confirmation_post_write_error_is_resolved_by_readback_once() {
    let lua = normal_context("");
    run(
        &lua,
        r#"
l,row=normalJoin(0,3); success(0,row,0); appointed(0,row)
local getter,setter=Game.GetProperty,Game.SetProperty
Game.GetProperty=function(self,key)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' then return properties[key] end
 return getter(self,key)
end
writes=0
Game.SetProperty=function(self,key,value)
 setter(self,key,value)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' then writes=writes+1; error('after accepted write') end
end
used[0]=1
ExposedMembers.JAC_LeaderPersona.Reconcile()
local s=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
assert(s.accounts[0].debits[1].confirmed and writes==1)
assert(properties['JAC_PERSONA_'..l..'_STATE']=='GOVERNOR' and #unitLists[0]==0)
ExposedMembers.JAC_LeaderPersona.Reconcile()
assert(writes==1 and used[0]==1 and operations==0 and uiReads==0)
"#,
    );
}

#[test]
fn normal_callbacks_wait_for_storage_without_mutating_or_replaying_old_receipts() {
    let lua = normal_context("");
    run(
        &lua,
        r#"
l,row=normalJoin(0,2)
local old=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
local oldSequence=old.sequence
local getter,setter=Game.GetProperty,Game.SetProperty
blocked=true; writes=0
Game.GetProperty=function(self,key)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' then return properties[key] end
 return getter(self,key)
end
Game.SetProperty=function(self,key,value)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' then
  writes=writes+1; if blocked then return end
 end
 return setter(self,key,value)
end
used[0]=1; success(0,row,0); appointed(0,row)
used[0]=2; success(0,row,1)
used[0]=3; success(0,row,2)
local attempted=writes
local api=ExposedMembers.JAC_LeaderPersona
local status=api.EventsGameplayStatus(0,l)
assert(not status.active and writes==attempted,'status retried a storage write')
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1==old and old.sequence==oldSequence)
assert(next(old.people[l].acquired)==nil and old.accounts[0].serial==0,
 'refused callback storage changed the original receipt graph')
api.Reconcile()
assert(#unitLists[0]==1 and properties['JAC_PERSONA_'..l..'_STATE']=='EVOLVING')
blocked=false
api.Reconcile()
local saved=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
local acquired=saved.people[l].acquired
assert(saved.sequence==oldSequence+4 and saved.accounts[0].serial==3)
assert(acquired[1].spent==1 and acquired.appointed.spent==1 and acquired[2].spent==2 and acquired[3].spent==3)
assert(properties['JAC_PERSONA_'..l..'_STATE']=='GOVERNOR' and #unitLists[0]==0)
local done=writes; api.Reconcile()
assert(writes==done and saved.sequence==properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.sequence)
assert(used[0]==3 and operations==0 and uiReads==0)
"#,
    );
}

#[test]
fn normal_pending_write_never_overwrites_a_foreign_saved_change() {
    let lua = normal_context("");
    run(
        &lua,
        r#"
l,row=normalJoin(0,3); success(0,row,0); appointed(0,row)
local getter,setter=Game.GetProperty,Game.SetProperty
Game.GetProperty=function(self,key)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' then return properties[key] end
 return getter(self,key)
end
Game.SetProperty=function(self,key,value)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' then return end
 return setter(self,key,value)
end
used[0]=1
local api=ExposedMembers.JAC_LeaderPersona
api.Reconcile()
local old=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
old.foreignChange='retained'
writes=0
Game.SetProperty=function(self,key,value)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' then writes=writes+1 end
 return setter(self,key,value)
end
api.Reconcile()
assert(writes==0 and old.foreignChange=='retained')
assert(old.accounts[0].debits[1].confirmed==nil and #unitLists[0]==1)
-- Only restoration of the exact saved before-image permits the pending
-- local persistence to resume. The product never performs this repair.
old.foreignChange=nil
api.Reconcile()
assert(writes>0 and properties['JAC_PERSONA_'..l..'_STATE']=='GOVERNOR' and #unitLists[0]==0)
assert(used[0]==1 and operations==0 and uiReads==0)
"#,
    );
}

#[test]
fn normal_confirmation_write_reentry_keeps_the_new_callback_after_the_saved_prefix() {
    let lua = normal_context("");
    run(
        &lua,
        r#"
l,row=normalJoin(0,1)
used[0]=1; success(0,row,0); appointed(0,row)
local setter=Game.SetProperty
reentered=false
Game.SetProperty=function(self,key,value)
 setter(self,key,value)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' and not reentered then
  reentered=true; used[0]=2; success(0,row,1)
 end
end
local api=ExposedMembers.JAC_LeaderPersona
api.Reconcile()
assert(reentered)
api.Reconcile()
local s=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
assert(s.sequence==3 and s.accounts[0].serial==2)
assert(s.people[l].acquired[1].spent==1 and s.people[l].acquired[2].spent==2)
assert(s.accounts[0].debits[1].confirmed and s.accounts[0].debits[2].confirmed)
assert(properties['JAC_PERSONA_'..l..'_STATE']=='GOVERNOR' and #unitLists[0]==0)
assert(used[0]==2 and operations==0 and uiReads==0)
"#,
    );
}

#[test]
fn normal_initial_commit_refused_write_recovers_only_on_a_domain_callback() {
    let lua = normal_context(
        r#"
blocked=true; writes=0
local setter=Game.SetProperty
Game.SetProperty=function(self,key,value)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' then
  writes=writes+1; if blocked then return end
 end
 return setter(self,key,value)
end
"#,
    );
    run(
        &lua,
        r#"
local l=PlayerConfigurations[1]:GetLeaderTypeName()
local api=ExposedMembers.JAC_LeaderPersona
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1==nil)
local before=writes
local s=api.EventsGameplayStatus(0,l)
assert(not s.active and s.initialization.stage=='commit_storage_pending' and writes==before)
blocked=false
-- Status remains a pure observation even after storage is available again.
s=api.EventsGameplayStatus(0,l)
assert(not s.active and writes==before)
api.Reconcile()
s=api.EventsGameplayStatus(0,l)
assert(s.active and s.initialization.stage=='committed' and writes>before)
local leader,row=normalJoin(0,1)
used[0]=1; success(0,row,0); appointed(0,row)
used[0]=2; success(0,row,1); finalize(0,leader)
assert(properties['JAC_PERSONA_'..leader..'_STATE']=='GOVERNOR' and #unitLists[0]==0)
assert(operations==0 and uiReads==0)
"#,
    );
}

#[test]
fn normal_pending_initial_commit_cannot_cross_its_original_start_boundary() {
    let lua = normal_context(
        r#"
blocked=true
local setter=Game.SetProperty
Game.SetProperty=function(self,key,value)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' and blocked then return end
 return setter(self,key,value)
end
"#,
    );
    run(
        &lua,
        r#"
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1==nil)
blocked=false
Game.GetCurrentGameTurn=function() return 1 end
ExposedMembers.JAC_LeaderPersona.Reconcile()
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1==nil,
 'pending initial commit crossed the original start boundary')
"#,
    );
}

#[test]
fn normal_pending_initial_commit_refuses_changed_price_owner_and_spent() {
    for fault in [
        "GameInfo.JAC_LeaderPersonas[PlayerConfigurations[1]:GetLeaderTypeName()].EvolutionCost=3",
        "PlayerConfigurations[1].GetLeaderTypeName=function() return 'OTHER' end",
        "used[0]=1",
        "properties.JAC_TEST853_PAID_ASSIGNMENT_V1=false",
        "playerProps[0][901014]=true",
        "playerProps[0][900014]=false",
        "Players[21].GetProperty=function() error('unreadable current native closure') end",
    ] {
        let lua = normal_context(
            r#"
blocked=true
local setter=Game.SetProperty
Game.SetProperty=function(self,key,value)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' and blocked then return end
 return setter(self,key,value)
end
"#,
        );
        run(&lua, fault);
        run(
            &lua,
            r#"
blocked=false
ExposedMembers.JAC_LeaderPersona.Reconcile()
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1==nil,
 'a changed initialization identity accepted the pending first write')
assert(operations==0 and uiReads==0)
"#,
        );
    }
}

#[test]
fn normal_confirmation_retries_a_thrown_write_without_replaying_payment() {
    let lua = normal_context("");
    run(
        &lua,
        r#"
l,row=normalJoin(0,3); success(0,row,0); appointed(0,row)
local setter=Game.SetProperty
blocked=true
Game.SetProperty=function(self,key,value)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' and blocked then error('write refused') end
 return setter(self,key,value)
end
used[0]=1
local api=ExposedMembers.JAC_LeaderPersona
api.Reconcile()
local old=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
assert(old.accounts[0].debits[1].confirmed==nil and #unitLists[0]==1)
local sequence=old.sequence
blocked=false
api.Reconcile()
local saved=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
assert(saved.sequence==sequence and saved.accounts[0].debits[1].confirmed)
assert(properties['JAC_PERSONA_'..l..'_STATE']=='GOVERNOR' and #unitLists[0]==0)
assert(used[0]==1 and operations==0 and uiReads==0)
"#,
    );
}

#[test]
fn rejected_notification_during_the_first_pending_write_keeps_the_nil_before_image() {
    let lua = normal_context(
        r#"
blocked=true; writes=0
local setter=Game.SetProperty
Game.SetProperty=function(self,key,value)
 if key=='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1' then
  writes=writes+1; if blocked then return end
 end
 return setter(self,key,value)
end
"#,
    );
    run(
        &lua,
        r#"
local api=ExposedMembers.JAC_LeaderPersona
local l=PlayerConfigurations[1]:GetLeaderTypeName()
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1==nil)
emit(Events,'GovernorPromoted','bad-owner',0,0)
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1==nil and used[0]==0)
blocked=false
for i=1,3 do api.Reconcile() end
assert(api.EventsGameplayStatus(0,l).active,
 'rejected initial notification lost the nil before-image and prevented storage recovery')
local saved=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
assert(saved.sequence==1 and saved.receipts[1].accepted==false)
emit(Events,'GovernorPromoted','bad-owner',0,0)
assert(api.EventsGameplayStatus(0,l).active and saved.sequence==1)
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.sequence==2)
assert(operations==0 and uiReads==0 and used[0]==0)
"#,
    );
}
