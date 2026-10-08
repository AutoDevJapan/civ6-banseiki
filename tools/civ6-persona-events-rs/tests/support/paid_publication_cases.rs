// Observed getter order models the consumer boundary, not a native runtime.
const PAID_OBSERVER: &str = r#"
paidVisible={}; paidCommits={}; paidBegins={}; paidFalse={}; paidManifest={}; paidBits={}; paidResets={}; numericWrites=0; paidCommitEvents=0
for owner=0,21 do
 local p=Players[owner]; local getter,setter=p.GetProperty,p.SetProperty
 paidVisible[owner]={}; paidResets[owner]=0
 p.SetProperty=function(self,key,value)
  if type(key)=='number' then numericWrites=numericWrites+1 end
  return setter(self,key,value)
 end
 p.GetProperty=function(self,key)
  local value=getter(self,key)
  if key==907000+owner or key==902000+owner then
   paidVisible[owner]={}; paidCommits[owner]=nil; paidBegins[owner]=nil
   paidFalse[owner]={}; paidManifest[owner]={}; paidBits[owner]={}; paidResets[owner]=paidResets[owner]+1
  elseif key==904999 then
   if value==false then paidBegins[owner]=true; paidCommits[owner]=false end
   if value==true then
    paidCommitEvents=paidCommitEvents+1 -- monotonic, survives every reset/closure
    assert(paidBegins[owner], 'COMMIT lacked begin')
    for row in GameInfo.JAC_LeaderPersonas() do
     local index=GameInfo.Governors[row.GovernorType].Index
     assert(paidFalse[owner][index] and paidManifest[owner][index], 'COMMIT preceded closed manifest')
    end
    for bit=0,1011 do assert(paidBits[owner][bit]~=nil,'COMMIT missed catalog bit') end
    paidCommits[owner]=true
   end
  elseif type(key)=='number' then
   if key>=905256 and key<906000 then paidBits[owner][key-905256]=value end
   if key>=906256 and key<906524 then paidBits[owner][744+key-906256]=value end
   if key>=904000 and key<904256 then paidManifest[owner][key-904000]=value==true end
   if key>=905000 and key<905256 then
    local index=key-905000
    if value==false then paidFalse[owner][index]=true end
    if value==true then assert(paidCommits[owner], 'true preceded complete catalog COMMIT') end
    paidVisible[owner][index]=value==true
   end
  end
  return value
 end
end
function noPaidTrue()
 for owner=0,21 do for _,v in pairs(paidVisible[owner]) do assert(not v, 'stale paid true') end end
end
"#;

#[test]
fn normal_paid_publication_explicit_refresh_replays_canonical_data_without_flags_or_payments() {
    let lua=normal_context(PAID_OBSERVER);
    run(&lua,r#"
local l,row=normalJoin(0,1); used[0]=2; success(0,row,0); appointed(0,row); success(0,row,1); finalize(0,l)
assert(paidVisible[0][14]==true)
local seq=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.sequence; local count=paidCommitEvents
-- Simulate a newly attached consumer, keeping the Lua output cache intact.
for owner=0,21 do paidVisible[owner]={}; paidCommits[owner]=nil end
ExposedMembers.JAC_LeaderPersona.Reconcile(); assert(paidVisible[0][14]==nil)
emit(GameEvents,'JAC_LeaderPersonaRepublish',0,{})
assert(paidVisible[0][14]==true and paidCommitEvents==count+22)
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.sequence==seq and used[0]==2 and #unitLists[0]==0)
assert(properties.JAC_TEST853_PAID_ASSIGNMENT_V1==nil and properties.JAC_TEST324_EVENTS_PROBE==nil and operations==0)
properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.version=2
emit(GameEvents,'JAC_LeaderPersonaRepublish',0,{}); noPaidTrue()
"#);
}

#[test]
fn normal_paid_publication_closes_cached_true_when_private_appointment_flags_appear() {
    for flag in ["JAC_TEST853_ADMISSION_ACTIVE", "JAC_TEST853_ADMISSION_MANUAL_APPOINT"] {
        for value in ["true", "false"] {
            for refresh in [false, true] {
                let lua=normal_context(PAID_OBSERVER);
                run(&lua,"local l,row=normalJoin(0,1); used[0]=2; success(0,row,0); appointed(0,row); success(0,row,1); finalize(0,l); assert(paidVisible[0][14]==true)");
                run(&lua,&format!("commitsBeforePrivate=paidCommitEvents; properties['{flag}']={value}"));
                run(&lua, if refresh { "emit(GameEvents,'JAC_LeaderPersonaRepublish',0,{})" } else { "ExposedMembers.JAC_LeaderPersona.Reconcile()" });
                run(&lua,"noPaidTrue(); assert(paidCommitEvents==commitsBeforePrivate,'private rejection republished a paid catalog'); assert(used[0]==2 and operations==0 and properties.JAC_TEST853_PAID_ASSIGNMENT_V1==nil)");
            }
        }
    }
}

#[test]
fn normal_paid_publication_private_flags_close_schemaless_saved_permissions() {
    for flag in ["JAC_TEST853_ADMISSION_ACTIVE", "JAC_TEST853_ADMISSION_MANUAL_APPOINT"] {
        let setup=format!(r#"{PAID_OBSERVER}
properties['{flag}']=false
for owner=0,21 do paidVisible[owner][14]=true end
"#);
        let lua=normal_context(&setup);
        run(&lua,"noPaidTrue(); assert(properties.JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA==nil and paidCommitEvents==0); for owner=0,21 do assert(paidResets[owner]>0) end");
    }
}

#[test]
fn normal_paid_publication_false_private_flag_preserves_explicit_event_diagnostic_only() {
    for flag in ["JAC_TEST853_ADMISSION_ACTIVE", "JAC_TEST853_ADMISSION_MANUAL_APPOINT"] {
        let lua=normal_context(PAID_OBSERVER);
        run(&lua,"emit(GameEvents,'JAC_LeaderPersonaEventsProbe',0,{Source=1}); normalJoin(0,1); assert(playerProps[0][901014]==true)");
        run(&lua,&format!("properties['{flag}']=false; commitsBeforePrivate=paidCommitEvents; ExposedMembers.JAC_LeaderPersona.Reconcile()"));
        run(&lua,"noPaidTrue(); assert(playerProps[0][901014]==true and paidCommitEvents==commitsBeforePrivate and operations==0 and used[0]==0)");
    }
}

#[test]
fn normal_paid_publication_rechecks_a_cached_true_after_receipt_loss() {
    let lua=normal_context(PAID_OBSERVER);
    run(&lua,r#"
local l,row=normalJoin(0,1); used[0]=2; success(0,row,0); appointed(0,row); success(0,row,1); finalize(0,l)
assert(paidVisible[0][14]==true and #unitLists[0]==0)
local store=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
local title=store.people[l].acquired[2]; store.people[l].acquired[2]=nil
ExposedMembers.JAC_LeaderPersona.Reconcile(); noPaidTrue()
store.people[l].acquired[2]=title
ExposedMembers.JAC_LeaderPersona.Reconcile(); assert(paidVisible[0][14]==true)
assert(used[0]==2 and operations==0 and properties.JAC_TEST853_PAID_ASSIGNMENT_V1==nil)
"#);
}

#[test]
fn normal_paid_publication_schema_write_must_be_read_back_before_catalog() {
    for value in ["nil", "99"] {
        let setup=format!(r#"{PAID_OBSERVER}
local setter=Game.SetProperty
Game.SetProperty=function(self,key,value)
 if key=='JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA' then return setter(self,key,{value}) end
 return setter(self,key,value)
end
"#);
        let lua=normal_context(&setup);
        run(&lua,r#"
assert(type(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1)=='table','normal startup must have succeeded')
noPaidTrue()
assert(paidCommitEvents==0,'a later closure cannot erase a premature COMMIT')
for owner=0,21 do assert(paidCommits[owner]==nil, 'catalog committed without namespace acknowledgement') end
assert(properties.JAC_TEST853_PAID_ASSIGNMENT_V1==nil and operations==0)
"#);
    }
}

#[test]
fn normal_paid_publication_keeps_independent_event_diagnostic_opt_in() {
    let lua=normal_context(PAID_OBSERVER);
    run(&lua,r#"
assert(properties.JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA==1)
emit(GameEvents,'JAC_LeaderPersonaEventsProbe',0,{Source=1})
assert(playerProps[0].JAC_PERSONA_RESULT=='EVENT_PROBE_READY',tostring(playerProps[0].JAC_PERSONA_RESULT))
assert(ExposedMembers.JAC_LeaderPersona.EventsProbeStatus(0,leader).active)
normalJoin(0,1)
assert(playerProps[0][901014]==true,'independent diagnostic appointment was disabled')
noPaidTrue()
assert(properties.JAC_TEST853_PAID_ASSIGNMENT_V1==nil and used[0]==0 and operations==0)
"#);
}

#[test]
fn normal_paid_publication_needs_no_diagnostic_flag_and_orders_the_commit() {
    for source in [3, 1, 2] {
        let lua=normal_context(PAID_OBSERVER);
        lua.globals().set("source",source).unwrap();
        run(&lua,r#"
assert(properties.JAC_TEST853_PAID_ASSIGNMENT_V1==nil)
assert(properties.JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA==1,'normal publication was not enabled')
for owner=0,21 do assert(paidCommits[owner]==true) end
noPaidTrue()
local l,row=normalJoin(0,source); local index=GameInfo.Governors[row.GovernorType].Index
noPaidTrue()
for step=0,row.EvolutionCost-1 do
 used[0]=step+1; success(0,row,step); if step==0 then appointed(0,row) end
 if step+1<row.EvolutionCost then finalize(0,l); noPaidTrue(); assert(#unitLists[0]==1) end
end
finalize(0,l); assert(#unitLists[0]==0 and paidVisible[0][index]==true)
for owner=0,21 do for i,allowed in pairs(paidVisible[owner]) do
 assert(allowed==(owner==0 and i==index), 'wrong owner/person opened')
end end
local before=numericWrites; local sequence=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.sequence
ExposedMembers.JAC_LeaderPersona.Reconcile(); ExposedMembers.JAC_LeaderPersona.Reconcile()
assert(numericWrites==before,'unchanged reconciliation republished numeric cells')
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.sequence==sequence)
assert(properties.JAC_TEST853_PAID_ASSIGNMENT_V1==nil and used[0]==row.EvolutionCost and uiReads==0 and operations==0)
"#);
    }
}

#[test]
fn normal_paid_publication_cannot_use_forged_complete_ledger_or_missing_receipts() {
    for fault in ["missing-title", "missing-appointment", "version", "subscription-gap"] {
        let lua=normal_context(PAID_OBSERVER);
        lua.globals().set("fault",fault).unwrap();
        run(&lua,r#"
assert(properties.JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA==1 and paidCommits[0]==true,'positive startup prerequisite')
local l,row=normalJoin(0,1); used[0]=2; success(0,row,0)
if fault~='missing-appointment' then appointed(0,row) end
if fault~='missing-title' then success(0,row,1) end
-- A forged final ledger must not substitute for the normal acquisition set.
unitLists[0]={}; properties['JAC_PERSONA_'..l..'_STATE']='GOVERNOR'; properties['JAC_PERSONA_'..l..'_UNIT']=nil
if fault=='version' then properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.version=2 end
if fault=='subscription-gap' then properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.continuityTainted=true end
ExposedMembers.JAC_LeaderPersona.Reconcile(); noPaidTrue()
assert(properties.JAC_TEST853_PAID_ASSIGNMENT_V1==nil and used[0]==2 and operations==0)
"#);
    }
}

#[test]
fn normal_paid_publication_reload_discards_old_true_before_fresh_confirmation() {
    let a=normal_context(PAID_OBSERVER);
    run(&a,"l,row=normalJoin(0,1); used[0]=2; success(0,row,0); appointed(0,row); success(0,row,1); finalize(0,l); assert(paidVisible[0][14]==true)");
    for old in [false,true] {
        // Record each paid true write with the observer's state at that moment,
        // so a saved true that was merely read back cannot pass as published.
        let setup=format!(r#"used[0]=2; playerProps[0][905014]=true; playerProps[21][905005]=true;
            if {old} then properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.version=2 end
            {PAID_OBSERVER}
            paidTrueWrites={{}}
            for owner=0,21 do
             local p=Players[owner]; local setter=p.SetProperty
             p.SetProperty=function(self,key,value)
              if type(key)=='number' and key>=905000 and key<905256 and value==true then
               paidTrueWrites[#paidTrueWrites+1]={{owner=owner,key=key,resets=paidResets[owner],committed=paidCommits[owner]}}
              end
              return setter(self,key,value)
             end
            end"#);
        let b=restore_normal(&a,&setup);
        run(&b,"assert(paidResets[0]>0 and paidResets[21]>0)");
        if old {run(&b,"noPaidTrue(); assert(#paidTrueWrites==0)");}
        else {run(&b,r#"
-- Loading alone republishes: every owner closed, then a full COMMIT, then
-- only the paid owner's person is written true by the new validation.
assert(#paidTrueWrites==1,'load must write exactly one fresh paid true')
local w=paidTrueWrites[1]
assert(w.owner==0 and w.key==905014 and w.resets>0 and w.committed==true)
for owner=0,21 do assert(paidCommits[owner]==true,'owner lacked a fresh COMMIT') end
assert(paidVisible[0][14]==true and not paidVisible[21][5])
"#);}
        run(&b,"ExposedMembers.JAC_LeaderPersona.Reconcile()");
        if old {run(&b,"noPaidTrue()");}
        else {run(&b,"assert(paidVisible[0][14]==true and not paidVisible[21][5])");}
        run(&b,"assert(used[0]==2 and operations==0 and properties.JAC_TEST853_PAID_ASSIGNMENT_V1==nil)");
    }
}

#[test]
fn normal_paid_publication_nonthrowing_appointment_refusal_closes_all_owners() {
    for fault in ["schema", "collision", "readback"] {
        let lua=normal_context(PAID_OBSERVER);
        lua.globals().set("fault",fault).unwrap();
        run(&lua,r#"
local l,row=normalJoin(0,3); used[0]=1; success(0,row,0); appointed(0,row); finalize(0,l)
assert(paidVisible[0][32]==true)
if fault=='schema' then properties.JAC_PERSONA_ADMISSION_SCHEMA_V1=99 end
if fault=='collision' then properties.JAC_PERSONA_ADMISSION_SCHEMA_V1=nil; playerProps[21][902021]='foreign' end
local p=Players[21]; local getter=p.GetProperty
if fault=='readback' then
 p.GetProperty=function(self,key) if key==900014 then return false end; return getter(self,key) end
 normalJoin(21,1)
end
ExposedMembers.JAC_LeaderPersona.Reconcile(); noPaidTrue()
for owner=0,21 do assert(paidResets[owner]>0) end
assert(used[0]==1 and operations==0 and properties.JAC_TEST853_PAID_ASSIGNMENT_V1==nil)
p.GetProperty=getter; properties.JAC_PERSONA_ADMISSION_SCHEMA_V1=3; playerProps[21][902021]=true
ExposedMembers.JAC_LeaderPersona.Reconcile(); assert(paidVisible[0][32]==true)
assert(used[0]==1 and operations==0)
"#);
    }
}
