use civ6_persona_events_rs::{adapter, BEGIN, END, GAMEPLAY};
use mlua::Lua;
use std::{fs, path::PathBuf};
include!("support/normal_cases.rs");

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
const FIXTURE: &str = r#"
print=function() end
properties={}; playerProps={}; unitLists={}; used={}; uiReads=0; operations=0
UI={RequestPlayerOperation=function() operations=operations+1; error('Gameplay callback sent an operation') end}
leader='LEADER_JAC_DATE_TANEMUNE'; prefix='JAC_PERSONA_'..leader..'_'
function clone(v) if type(v)~='table' then return v end local t={} for k,x in pairs(v) do t[k]=clone(x) end return t end
Game={GetProperty=function(_,k) return clone(properties[k]) end, SetProperty=function(_,k,v) properties[k]=clone(v) end,
 GetCurrentGameTurn=function() return 12 end, GetLocalPlayer=function() return 0 end}
function catalog(rows,key)
 local t={}; for _,r in ipairs(rows) do t[r[key]]=r; if r.Index then t[r.Index]=r end end
 return setmetatable(t,{__call=function() local i=0; return function() i=i+1; return rows[i] end end})
end
local people,governors,promotions,individuals={},{},{},{}
for i=0,21 do
 local l=i==1 and leader or ('LEADER_JAC_TEST'..i)
 local g='GOVERNOR_JAC_PERSONA_'..string.sub(l,12); local b='PROMOTION_'..i
 people[#people+1]={LeaderType=l,GovernorType=g,BasePromotionType=b,EvolutionCost=i==1 and 2 or 1,CivilizationType='CIV'..i,PersonType='PERSON'..i,Trait='ADEPT',Personality='CALM'}
 governors[#governors+1]={GovernorType=g,Index=i,Hash=100+i}
 promotions[#promotions+1]={GovernorPromotionType=b,Index=80+i*3,Hash=180+i*3}
 promotions[#promotions+1]={GovernorPromotionType=b..'_TITLE_1',Index=81+i*3,Hash=181+i*3}
 individuals[#individuals+1]={GreatPersonIndividualType='PERSON'..i,Index=i,Hash=200+i,GreatPersonClassType='CLASS',EraType='ERA'}
end
GameInfo={JAC_LeaderPersonas=catalog(people,'LeaderType'),Governors=catalog(governors,'GovernorType'),
 GovernorPromotions=catalog(promotions,'GovernorPromotionType'),GreatPersonIndividuals=catalog(individuals,'GreatPersonIndividualType'),
 GreatPersonClasses={CLASS={UnitType='UNIT',Hash=7}},Units={UNIT={Index=30}},Eras={ERA={Hash=8}},
 JAC_PersonaTreeNodes=catalog({},'NodeType')}
Players={}; PlayerConfigurations={}; ids={}
for i=0,21 do
 local id=i; ids[#ids+1]=id; used[id]=0; playerProps[id]={}; unitLists[id]={}
 local gs={GetGovernorPoints=function() return 10 end,GetGovernorPointsSpent=function() return used[id] end}
 local units={Members=function() local n=0; return function() n=n+1; local u=unitLists[id][n]; if u then return n,u end end end,
 Destroy=function(_,unit) for n,u in ipairs(unitLists[id]) do if u==unit then table.remove(unitLists[id],n); break end end end}
 Players[id]={IsMajor=function() return true end,IsAlive=function() return true end,IsHuman=function() return id==0 end,GetID=function() return id end,
  GetGovernors=function() return gs end,GetUnits=function() return units end,
  GetProperty=function(_,k) return playerProps[id][k] end,SetProperty=function(_,k,v) playerProps[id][k]=v end,
  GetCities=function() return {GetCapitalCity=function() return {GetID=function() return 1 end} end} end,
  GetDiplomacy=function() return {HasAllied=function() return false end} end}
 PlayerConfigurations[id]={GetLeaderTypeName=function() return people[id+1].LeaderType end,GetCivilizationTypeName=function() return 'CIV'..id end}
end
PlayerManager={GetAliveMajorIDs=function() return ids end}
Game.GetGreatPeople=function() return {GrantPerson=function(_,hash,_,_,_,owner)
 local u={GetType=function() return 30 end,GetID=function() return 500 end,GetGreatPerson=function() return {GetIndividual=function() return hash-200 end} end}
 unitLists[owner][#unitLists[owner]+1]=u
end} end
local function bus()
 local events={}
 return setmetatable(events,{__index=function(t,k)
  local e={handlers={}}; e.Add=function(f) e.handlers[#e.handlers+1]=f end; rawset(t,k,e); return e
 end})
end
Events=bus(); GameEvents=bus(); ExposedMembers={JAC_LeaderPersonaNativeRead=function() uiReads=uiReads+1; return {appointed=false,base=false,surchargePaid=0} end}
function emit(bus,name,...) for _,f in ipairs(bus[name].handlers) do f(...) end end
function include() end
function start()
 emit(GameEvents,'JAC_LeaderPersonaEventsProbe',0,{Source=1})
 assert(playerProps[0].JAC_PERSONA_RESULT=='EVENT_PROBE_READY')
end
function join()
 emit(GameEvents,'JAC_LeaderPersonaJoin',0,{Source=1})
 assert(playerProps[0].JAC_PERSONA_RESULT=='JOINED')
 emit(GameEvents,'JAC_LeaderPersonaEvolve',0,{Leader=leader})
 assert(playerProps[0].JAC_PERSONA_RESULT=='EVOLVING')
end
function status() return ExposedMembers.JAC_LeaderPersona.EventsProbeStatus(0,leader) end
function pay(reverse)
 used[0]=1
 if reverse then emit(Events,'GovernorAppointed',0,1); emit(Events,'GovernorPromoted',0,1,83)
 else emit(Events,'GovernorPromoted',0,1,83); emit(Events,'GovernorAppointed',0,1) end
end
"#;

fn context() -> Lua {
    let lua = Lua::new();
    lua.load(FIXTURE).exec().unwrap();
    load(&lua, &fs::read_to_string(root().join(GAMEPLAY)).unwrap());
    lua
}
fn load(lua: &Lua, source: &str) {
    lua.load(
        fs::read_to_string(
            root().join("poc/JAC_PoC_LeaderPersona/Shared/JAC_PersonaTreeRules.lua"),
        )
        .unwrap(),
    )
    .exec()
    .unwrap();
    lua.load(source).exec().unwrap();
}
fn run(lua: &Lua, s: &str) {
    lua.load(s).exec().unwrap();
}

fn transfer(lua: &Lua, t: mlua::Table) -> mlua::Table {
    let result = lua.create_table().unwrap();
    for item in t.pairs::<mlua::Value, mlua::Value>() {
        let (k, v) = item.unwrap();
        fn value(lua: &Lua, v: mlua::Value) -> mlua::Value {
            match v {
                mlua::Value::String(s) => {
                    mlua::Value::String(lua.create_string(s.as_bytes()).unwrap())
                }
                mlua::Value::Table(t) => mlua::Value::Table(transfer(lua, t)),
                x => x,
            }
        }
        result.set(value(lua, k), value(lua, v)).unwrap();
    }
    result
}

#[test]
fn generated_block_is_current_and_in_the_gameplay_lexical_context() {
    let source = fs::read_to_string(root().join(GAMEPLAY))
        .unwrap()
        .replace("\r\n", "\n");
    assert!(
        civ6_persona_events_rs::embed(&source).unwrap() == source,
        "Gameplay generated blocks are stale; run the Rust embed CLI"
    );
    assert_eq!(source.matches(BEGIN).count(), 1);
    assert_eq!(source.matches(END).count(), 1);
    assert!(source.contains(adapter()));
    let lua = context();
    run(&lua,"local s=status(); assert(s.source=='civ6-persona-events-rs/v1/Gameplay' and s.context=='Gameplay' and not s.active); assert(s.registrations.GovernorAppointed.callOK and s.registrations.GovernorPromoted.callOK); assert(next(properties)==nil or properties.JAC_PERSONA_EVENTS_PROBE_V1==nil)");
}

#[test]
fn legacy_ui_without_normal_receipts_stays_unknown_closed() {
    let lua = context();
    run(&lua,"emit(Events,'GovernorPromoted',0,1,83); emit(Events,'GovernorAppointed',0,1); assert(properties.JAC_PERSONA_EVENTS_PROBE_V1==nil); emit(GameEvents,'JAC_LeaderPersonaJoin',0,{Source=1}); assert(uiReads==0 and playerProps[0].JAC_PERSONA_RESULT=='NATIVE_UNAVAILABLE')");
}

#[test]
fn both_orders_deduplicate_base_and_drive_normal_finalize_without_ui() {
    for reverse in [false, true] {
        let lua = context();
        run(&lua, &format!("start(); join(); pay({reverse});"));
        run(
            &lua,
            r#"
local s=status(); assert(s.paid==1 and s.acquired.base and s.acquired.appointed and not s.acquired.title)
assert(s.sequence==2 and #s.receipts==2 and s.acquired.base.raw[3].value==83 and s.acquired.base.spent==1)
assert(s.acquired.base.resolvedLeader==leader and s.acquired.base.context=='Gameplay')
emit(GameEvents,'JAC_LeaderPersonaFinalize',0,{Leader=leader})
assert(playerProps[0].JAC_PERSONA_RESULT=='TITLE_PAYMENT_PENDING' and #unitLists[0]==1)
used[0]=2; emit(Events,'GovernorPromoted',0,1,84)
assert(status().paid==2 and status().acquired.title.raw[3].value==84)
local original=clone(status().acquired)
emit(Events,'GovernorPromoted',0,1,83); emit(Events,'GovernorAppointed',0,1); emit(Events,'GovernorPromoted',0,1,84)
assert(status().paid==2 and status().acquired.base.sequence==original.base.sequence and status().acquired.title.sequence==original.title.sequence)
emit(GameEvents,'JAC_LeaderPersonaFinalize',0,{Leader=leader})
assert(playerProps[0].JAC_PERSONA_RESULT=='EVOLVED' and properties[prefix..'STATE']=='GOVERNOR' and #unitLists[0]==0 and properties[prefix..'UNIT']==nil)
-- Ordinary appointment projection reaches CorePaymentRead; the probe set is
-- required to retain the GOVERNOR qualification with both getters absent.
ExposedMembers.JAC_LeaderPersona.Reconcile()
assert(playerProps[0][901001]==true and uiReads==0 and operations==0 and used[0]==2)
"#,
        );
    }
}

#[test]
fn synchronized_probe_republishes_unchanged_closed_cells_after_core_attachment() {
    let lua = context();
    run(
        &lua,
        r#"
start(); coreReads={}; local originalEmit=emit
function emit(bus,name,...)
 local previous=readContext; readContext='GameplayOperation'
 originalEmit(bus,name,...); readContext=previous
end
for owner=0,21 do
 local p,id=Players[owner],owner; local original=p.GetProperty
 p.GetProperty=function(self,key)
  local value=original(self,key)
  if readContext=='GameplayOperation' and key>=900000 and key<=903255 then coreReads[#coreReads+1]={owner=id,key=key,value=value} end
  return value
 end
end
readContext='UI'; ExposedMembers.JAC_LeaderPersona.Reconcile()
for owner=0,21 do assert(Players[owner]:GetProperty(902000+owner)==true) end
assert(#coreReads==0,'UI reads/direct Reconcile must not count as Core operation binding')
emit(GameEvents,'JAC_LeaderPersonaEventsRepublish',0,{Source=1})
local r=status().admission
assert(r and r.ok and r.source=='civ6-persona-events-rs/v1/Gameplay' and r.synchronized==true)
assert(#coreReads==1474 and #r.reads==1474 and r.cells==484)
local at=0
for owner=0,21 do
 at=at+1; local got=coreReads[at]
 assert(got.owner==owner and got.key==902000+owner and got.value==true and r.reads[at].phase=='reset')
end
for owner=0,21 do
 for index=0,21 do
  at=at+1; assert(coreReads[at].owner==owner and coreReads[at].key==903000+index and coreReads[at].value==true and r.reads[at].phase=='close')
  at=at+1; assert(coreReads[at].owner==owner and coreReads[at].key==901000+index and coreReads[at].value==false and r.reads[at].phase=='false')
 end
 for index=0,21 do
  at=at+1; assert(coreReads[at].owner==owner and coreReads[at].key==900000+index and coreReads[at].value==true and r.reads[at].phase=='manifest')
 end
end
assert(at==1474 and status().sequence==0 and used[0]==0 and operations==0)
local detached=status(); detached.admission.reads[1].key=0; assert(status().admission.reads[1].key==902000)
"#,
    );
}

#[test]
fn republish_rejects_nonfresh_scope_foreign_schema_and_failed_false_ack() {
    for setup in [
        "properties.JAC_PERSONA_ADMISSION_SCHEMA_V1=99",
        "properties.JAC_PERSONA_ADMISSION_SCHEMA_V1=2",
        "properties[prefix..'OWNER']=0",
        "used[0]=1",
        "properties.JAC_TEST324_EVENTS_PROBE=nil",
    ] {
        let lua = context();
        run(&lua, "start()");
        run(&lua, setup);
        run(
            &lua,
            r#"
local reads=0; local old=Players[0].GetProperty
Players[0].GetProperty=function(self,key) if key>=900000 and key<=903255 then reads=reads+1 end; return old(self,key) end
emit(GameEvents,'JAC_LeaderPersonaEventsRepublish',0,{Source=1})
assert(reads==0 and playerProps[0].JAC_PERSONA_RESULT~='EVENT_PROBE_REPUBLISHED')
"#,
        );
    }
    let lua = context();
    run(&lua, "start()");
    run(
        &lua,
        r#"
local old=Players[0].GetProperty
Players[0].GetProperty=function(self,key) if key==901000 then return true end; return old(self,key) end
emit(GameEvents,'JAC_LeaderPersonaEventsRepublish',0,{Source=1})
local r=status().admission
assert(r and not r.ok and r.cells==0 and r.reason=='read-order-or-ack')
for _,entry in ipairs(r.reads) do assert(entry.owner~=0 or entry.phase~='manifest','failed false ACK published a manifest') end
"#,
    );
}

#[test]
fn removing_fresh_cache_reset_is_a_red_republication_mutation() {
    let source = fs::read_to_string(root().join(GAMEPLAY)).unwrap();
    let cache = "admissionSnapshots={}";
    assert_eq!(source.matches(cache).count(), 1);
    let lua = Lua::new();
    lua.load(FIXTURE).exec().unwrap();
    load(&lua, &source.replace(cache, ""));
    run(&lua,"start(); emit(GameEvents,'JAC_LeaderPersonaEventsRepublish',0,{Source=1}); assert(not status().admission.ok and #status().admission.reads==0)");
    assert!(lua
        .load("assert(status().admission.ok, 'installed Core must see every closed natural read')")
        .exec()
        .is_err());
}

#[test]
fn callback_spent_can_lag_base_and_title_until_the_gameplay_query_settles() {
    for reverse in [false, true] {
        let lua = context();
        run(&lua, "start(); join(); used[0]=0");
        run(
            &lua,
            if reverse {
                "emit(Events,'GovernorAppointed',0,1); emit(Events,'GovernorPromoted',0,1,83)"
            } else {
                "emit(Events,'GovernorPromoted',0,1,83); emit(Events,'GovernorAppointed',0,1)"
            },
        );
        run(
            &lua,
            r#"
local before=status()
assert(before.acquired.base and before.acquired.appointed,'callbackSpent zero must retain actual callback proof')
assert(before.acquired.base.spent==0 and before.acquired.appointed.spent==0)
assert(before.acquired.base.callbackPayment=='pending' and before.paid==nil)
assert(before.settlement.afterSpent==0 and before.settlement.pending==true)
used[0]=1
local base=status()
assert(base.paid==1 and base.settlement.afterSpent==1 and not base.settlement.pending)
assert(base.acquired.base.sequence==before.acquired.base.sequence and base.acquired.base.spent==0)
emit(Events,'GovernorPromoted',0,1,84)
local title=status()
assert(title.acquired.title and title.acquired.title.spent==1 and title.acquired.title.callbackPayment=='pending')
assert(title.paid==1 and title.settlement.pending and title.settlement.afterSpent==1)
used[0]=2
local done=status()
assert(done.paid==2 and done.settlement.afterSpent==2 and not done.settlement.pending)
assert(done.acquired.title.spent==1 and done.acquired.title.sequence==title.acquired.title.sequence)
assert(done.settlement.baseSequence==before.acquired.base.sequence and done.settlement.titleSequence==title.acquired.title.sequence)
emit(GameEvents,'JAC_LeaderPersonaFinalize',0,{Leader=leader})
assert(properties[prefix..'STATE']=='GOVERNOR' and operations==0 and uiReads==0)
"#,
        );
    }
}

#[test]
fn unavailable_callback_spent_is_preserved_and_query_is_read_only() {
    let lua = context();
    run(
        &lua,
        r#"
start(); join()
local gs=Players[0]:GetGovernors(); local savedRead=gs.GetGovernorPointsSpent
gs.GetGovernorPointsSpent=function() return nil end
emit(Events,'GovernorPromoted',0,1,83); emit(Events,'GovernorAppointed',0,1)
local pending=status()
assert(pending.acquired.base.spent==nil and pending.acquired.base.callbackPayment=='unknown')
assert(pending.paid==nil and pending.settlement.reason=='official-spent-unavailable')
gs.GetGovernorPointsSpent=savedRead; used[0]=1
local beforeWrites=0; local savedSet=Game.SetProperty
Game.SetProperty=function(...) beforeWrites=beforeWrites+1; return savedSet(...) end
local settled=status(); assert(settled.paid==1 and settled.settlement.afterSpent==1)
assert(beforeWrites==0 and settled.acquired.base.spent==nil and settled.acquired.base.callbackPayment=='unknown')
"#,
    );
}

#[test]
fn emitted_diagnostic_snapshots_compare_after_spent_without_rejecting_lag() {
    let lua = context();
    run(
        &lua,
        r#"
start(); join(); emit(Events,'GovernorPromoted',0,1,83); emit(Events,'GovernorAppointed',0,1)
Players[0]:GetGovernors().GetGovernor=function(_,hash)
 assert(hash==101)
 return {HasPromotion=function(_,promotion) return promotion==183 or (promotion==184 and used[0]>=2) end}
end
used[0]=1
"#,
    );
    run(&lua, &civ6_persona_events_rs::scenario::snapshot(1));
    run(&lua, "emit(Events,'GovernorPromoted',0,1,84); used[0]=2");
    run(&lua, &civ6_persona_events_rs::scenario::snapshot(2));
}

#[test]
fn spent_after_ready_cannot_create_missing_or_rejected_callback_proof() {
    let lua = context();
    run(
        &lua,
        r#"
start(); join(); used[0]=2
assert(status().paid==nil and next(status().acquired)==nil)
emit(Events,'GovernorPromoted',1,1,83); emit(Events,'GovernorPromoted',0,1,'83')
emit(Events,'GovernorAppointed',0,2)
assert(status().paid==nil and next(status().acquired)==nil)
used[0]=0; emit(Events,'GovernorPromoted',0,1,83)
used[0]=2
assert(status().paid==nil and not status().acquired.appointed and not status().acquired.title)
"#,
    );
}

#[test]
fn partial_and_late_notifications_preserve_the_set_without_inventing_proof() {
    let lua = context();
    run(
        &lua,
        r#"
start(); join(); used[0]=1; emit(Events,'GovernorPromoted',0,1,83)
assert(status().paid==nil and status().native==nil and status().acquired.base)
used[0]=2; emit(Events,'GovernorPromoted',0,1,84)
assert(status().paid==nil and status().acquired.title)
emit(Events,'GovernorAppointed',0,1)
assert(status().paid==2 and status().acquired.appointed.spent==2 and status().acquired.base.spent==1)
"#,
    );
}

#[test]
fn removing_the_core_payment_hook_removes_normal_governor_admission() {
    let source = fs::read_to_string(root().join(GAMEPLAY)).unwrap();
    let hook = "if PersonaEvents.Enabled() then return PersonaEvents.Paid(owner, row, cost); end";
    assert_eq!(source.matches(hook).count(), 1);
    let lua = Lua::new();
    lua.load(FIXTURE).exec().unwrap();
    load(&lua, &source.replace(hook, ""));
    run(&lua,"start(); join(); pay(false); used[0]=2; emit(Events,'GovernorPromoted',0,1,84); emit(GameEvents,'JAC_LeaderPersonaFinalize',0,{Leader=leader}); ExposedMembers.JAC_LeaderPersona.Reconcile(); assert(properties[prefix..'STATE']=='GOVERNOR' and status().paid==2)");
    assert!(lua
        .load(
            "assert(playerProps[0][901001]==true, 'CorePaymentRead must consume the positive set')"
        )
        .exec()
        .is_err());
}

#[test]
fn wrong_person_owner_types_arity_and_unjoined_are_diagnostic_only() {
    let lua = context();
    run(
        &lua,
        r#"
start(); used[0]=1; emit(Events,'GovernorPromoted',0,1,83)
assert(status().receipts[1].reason=='not-joined' and status().paid==nil)
used[0]=0; join(); used[0]=1
emit(Events,'GovernorPromoted',1,1,83); emit(Events,'GovernorPromoted',0,2,86)
emit(Events,'GovernorPromoted',0,1,86); emit(Events,'GovernorPromoted',0,1,'83')
emit(Events,'GovernorPromoted',0,1,nil); emit(Events,'GovernorPromoted',0,1,83,99)
emit(Events,'GovernorAppointed','0',1); emit(Events,'GovernorAppointed',0,'1')
assert(next(status().acquired)==nil and status().paid==nil)
local r=status().receipts; assert(r[#r-2].arity==4 and r[#r-3].raw[3].type=='nil')
pay(false); assert(status().paid==1)
local detached=status(); detached.acquired.base.owner=9; assert(status().paid==1)
for i=1,80 do emit(Events,'GovernorPromoted',0,1,83) end
assert(#status().receipts==64 and status().sequence==91 and status().acquired.base.sequence==10)
emit(Events,'GovernorPromoted',0,1,false)
assert(status().receipts[64].raw[3].type=='boolean' and status().receipts[64].raw[3].value==false and status().paid==1)
"#,
    );
}

#[test]
fn foreign_saved_proof_and_definition_changes_cannot_be_authority() {
    for corrupt in [
        "properties.JAC_PERSONA_EVENTS_PROBE_V1.version=9",
        "properties.JAC_PERSONA_EVENTS_PROBE_V1.acquired.base.owner=1",
        "properties.JAC_PERSONA_EVENTS_PROBE_V1.acquired.base.raw[3].value=86",
        "properties.JAC_PERSONA_EVENTS_PROBE_V1.acquired.base.accepted=false",
        "properties.JAC_PERSONA_EVENTS_PROBE_V1.acquired.base.context='UI'",
        "properties.JAC_PERSONA_EVENTS_PROBE_V1.acquired.base.resolvedLeader='LEADER_JAC_TEST2'",
        "properties.JAC_PERSONA_EVENTS_PROBE_V1.acquired.base.sequence=properties.JAC_PERSONA_EVENTS_PROBE_V1.acquired.appointed.sequence",
        "GameInfo.GovernorPromotions[83].Index=87",
        "properties[prefix..'OWNER']=1",
    ] {
        let lua = context();
        run(&lua, "start(); join(); pay(false)");
        run(&lua, corrupt);
        run(&lua,"assert(status().paid==nil); emit(GameEvents,'JAC_LeaderPersonaFinalize',0,{Leader=leader}); assert(properties[prefix..'STATE']=='EVOLVING' and #unitLists[0]==1)");
    }
}

#[test]
fn save_restores_into_a_new_lua_context_without_payment_replay() {
    for lagging in [false, true] {
        let a = context();
        run(
            &a,
            if lagging {
                "start(); join(); used[0]=0; emit(Events,'GovernorPromoted',0,1,83); emit(Events,'GovernorAppointed',0,1); used[0]=1; emit(Events,'GovernorPromoted',0,1,84); assert(status().paid==1 and status().settlement.pending)"
            } else {
                "start(); join(); pay(false); used[0]=2; emit(Events,'GovernorPromoted',0,1,84)"
            },
        );
        // Transfer only serialized Game properties, engine units and spent count,
        // never functions, the subscription bus or the old lexical module.
        let saved: mlua::Table = a.globals().get("properties").unwrap();
        let b = Lua::new();
        b.load(FIXTURE).exec().unwrap();
        b.globals().set("properties", transfer(&b, saved)).unwrap();
        run(
            &b,
            "used[0]=2; Game.GetGreatPeople():GrantPerson(201,7,8,0,0)",
        );
        // Gameplay initialization is repeated in the new context on load.
        load(&b, &fs::read_to_string(root().join(GAMEPLAY)).unwrap());
        run(
            &b,
            r#"
assert(status().paid==2 and status().sequence==3 and status().acquired.title.sequence==3)
local sequence=status().acquired.base.sequence
emit(Events,'GovernorPromoted',0,1,83); emit(Events,'GovernorAppointed',0,1)
assert(status().paid==2 and status().acquired.base.sequence==sequence and used[0]==2 and operations==0)
emit(GameEvents,'JAC_LeaderPersonaFinalize',0,{Leader=leader})
assert(properties[prefix..'STATE']=='GOVERNOR' and uiReads==0)
"#,
        );
        if lagging {
            run(&b,"assert(status().acquired.base.spent==0 and status().acquired.title.spent==1 and status().settlement.afterSpent==2)");
        }
    }
}

#[test]
fn missing_appointed_or_title_survives_reload_and_never_uses_fullpaid_ui() {
    for missing in ["appointed", "title"] {
        let a = context();
        run(
            &a,
            "start(); join(); emit(Events,'GovernorPromoted',0,1,83)",
        );
        run(
            &a,
            if missing == "appointed" {
                "used[0]=1; emit(Events,'GovernorPromoted',0,1,84)"
            } else {
                "emit(Events,'GovernorAppointed',0,1)"
            },
        );
        run(&a, "used[0]=2; assert(status().paid~=2)");
        let saved: mlua::Table = a.globals().get("properties").unwrap();
        let b = Lua::new();
        b.load(FIXTURE).exec().unwrap();
        b.globals().set("properties", transfer(&b, saved)).unwrap();
        run(
            &b,
            r#"
used[0]=2; Game.GetGreatPeople():GrantPerson(201,7,8,0,0)
ExposedMembers.JAC_LeaderPersonaNativeRead=function() uiReads=uiReads+1; return {appointed=true,base=true,surchargePaid=1} end
assert(ExposedMembers.JAC_LeaderPersonaNativeRead().surchargePaid==1); uiReads=0
"#,
        );
        load(&b, &fs::read_to_string(root().join(GAMEPLAY)).unwrap());
        run(
            &b,
            r#"
local sequence=status().sequence
for n=1,4 do
 ExposedMembers.JAC_LeaderPersona.Reconcile()
 emit(GameEvents,'JAC_LeaderPersonaFinalize',0,{Leader=leader})
 assert(status().paid~=2 and status().sequence==sequence and properties[prefix..'STATE']=='EVOLVING')
 assert(#unitLists[0]==1 and properties[prefix..'UNIT']==500 and used[0]==2)
end
assert(uiReads==0 and operations==0)
"#,
        );
        run(&b, &format!("assert(status().acquired.{missing}==nil)"));
    }
}

#[test]
fn removing_the_registered_callback_is_a_red_mutation() {
    let source = fs::read_to_string(root().join(GAMEPLAY)).unwrap();
    let original = "reg.callOK=pcall(add,function(...) receive(name,...) end)";
    assert_eq!(source.matches(original).count(), 1);
    let broken = source.replace(original, "reg.callOK=pcall(add,function(...) end)");
    let lua = Lua::new();
    lua.load(FIXTURE).exec().unwrap();
    load(&lua, &broken);
    run(
        &lua,
        "start(); join(); pay(false); assert(status().paid==nil and #status().receipts==0)",
    );
    assert!(lua.load("assert(status().paid==1, 'positive proof must disappear when the callback is removed')").exec().is_err());
}

#[test]
fn fresh_start_refuses_old_paid_or_joined_saves_and_subscription_failures() {
    for setup in [
        "used[0]=1",
        "properties[prefix..'OWNER']=0",
        "properties.JAC_PERSONA_EVENTS_PROBE_V1={version=77}",
    ] {
        let lua = context();
        run(&lua, setup);
        run(&lua,"emit(GameEvents,'JAC_LeaderPersonaEventsProbe',0,{Source=1}); assert(not status().active)");
    }
    let lua = Lua::new();
    lua.load(FIXTURE).exec().unwrap();
    run(&lua,"Events=setmetatable({DiplomacyRelationshipChanged={Add=function() end}},{__index=function() return setmetatable({},{__index=function() error('Add lookup failed') end}) end})");
    load(&lua, &fs::read_to_string(root().join(GAMEPLAY)).unwrap());
    run(&lua,"emit(GameEvents,'JAC_LeaderPersonaEventsProbe',0,{Source=1}); assert(not status().active and status().registrations.GovernorAppointed.indexOK and not status().registrations.GovernorAppointed.addOK)");
    for setup in [
        "Events=setmetatable({DiplomacyRelationshipChanged={Add=function() end}},{__index=function() error('event lookup failed') end})",
        "Events=setmetatable({DiplomacyRelationshipChanged={Add=function() end}},{__index=function() return {Add=function() error('Add call failed') end} end})",
    ] {
        let lua=Lua::new(); lua.load(FIXTURE).exec().unwrap(); run(&lua,setup);
        load(&lua,&fs::read_to_string(root().join(GAMEPLAY)).unwrap());
        run(&lua,"emit(GameEvents,'JAC_LeaderPersonaEventsProbe',0,{Source=1}); assert(not status().active and not status().registrations.GovernorAppointed.callOK)");
    }
}
