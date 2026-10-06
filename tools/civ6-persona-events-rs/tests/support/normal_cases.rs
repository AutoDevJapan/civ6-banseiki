// Included by gameplay.rs to exercise exactly the same shipped script loader.
const NORMAL_FIXTURE: &str = r#"
Game.GetCurrentGameTurn=function() return 0 end
GameConfiguration={GetStartTurn=function() return 0 end}
local people,governors,promotions,sets={},{},{},{}
local names={'ASHINA_MORIKIYO','DATE_TANEMUNE','YAMANOUCHI_UESUGI_NORIMASA','TAKEDA_NOBUTORA',
 'HOJO_UJITSUNA','NAGAO_TAMEKAGE','IMAGAWA_UJITERU','ODA_NOBUHIDE','MATSUDAIRA_KIYOYASU',
 'ASAKURA_TAKAKAGE','ASHIKAGA_YOSHIHARU','MIYOSHI_NAGAYOSHI','ROKKAKU_SADAYORI',
 'HONGANJI_SHONYO','OUCHI_YOSHITAKA','AMAGO_TSUNEHISA','MORI_MOTONARI','CHOSOKABE_KUNICHIKA',
 'OTOMO_YOSHIAKI','SHIMAZU_TAKAHISA','RYUKYU_SHO_SEI','AINU_TARIKONA'}
local costs={1,2,3,1,2,2,2,1,1,2,3,2,2,3,2,2,1,1,2,1,3,2}
for row in GameInfo.JAC_LeaderPersonas() do
 local i=GameInfo.GreatPersonIndividuals[row.PersonType].Index
 local g=GameInfo.Governors[row.GovernorType]
 row.LeaderType='LEADER_JAC_'..names[i+1]
 row.GovernorType='GOVERNOR_JAC_PERSONA_'..names[i+1]; g.GovernorType=row.GovernorType
 row.EvolutionCost=costs[i+1]
 row.BasePromotionType='GOVERNOR_PROMOTION_JAC_PERSONA_'..string.sub(row.LeaderType,12)..'_BASE'
 local individual=GameInfo.GreatPersonIndividuals[row.PersonType]
 row.PersonType='GREAT_PERSON_INDIVIDUAL_JAC_PERSONA_'..string.sub(row.LeaderType,12)
 individual.GreatPersonIndividualType=row.PersonType
 GameInfo.GreatPersonIndividuals[row.PersonType]=individual
 g.Index=5+i*9
 people[#people+1]=row; governors[#governors+1]=g
 for step=0,2 do
  local name=row.BasePromotionType..(step==0 and '' or '_TITLE_'..step)
  promotions[#promotions+1]={GovernorPromotionType=name,Index=70+i*11+step*2,Hash=500+i*11+step*2,BaseAbility=step==0 and 1 or 0}
  sets[#sets+1]={GovernorType=row.GovernorType,GovernorPromotion=name}
 end
end
GameInfo.JAC_LeaderPersonas=catalog(people,'LeaderType')
GameInfo.Governors=catalog(governors,'GovernorType')
GameInfo.GovernorPromotions=catalog(promotions,'GovernorPromotionType')
GameInfo.GovernorPromotionSets=setmetatable({},{__call=function() local i=0; return function() i=i+1; return sets[i] end end})
-- No private operation, no UI receipt or Core per-person getter is available.
ExposedMembers.JAC_LeaderPersonaNativeRead=function() uiReads=uiReads+1; error('public route consulted UI') end
function normalStatus(owner,l)
 local s=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
 local e=s and s.people[l]
 return s,e and e.acquired
end
function normalJoin(owner,source)
 local l=PlayerConfigurations[source]:GetLeaderTypeName()
 emit(GameEvents,'JAC_LeaderPersonaJoin',owner,{Source=source})
 assert(playerProps[owner].JAC_PERSONA_RESULT=='JOINED',tostring(playerProps[owner].JAC_PERSONA_RESULT))
 emit(GameEvents,'JAC_LeaderPersonaEvolve',owner,{Leader=l})
 assert(playerProps[owner].JAC_PERSONA_RESULT=='EVOLVING',tostring(playerProps[owner].JAC_PERSONA_RESULT))
 return l,GameInfo.JAC_LeaderPersonas[l]
end
function success(owner,row,step)
 local gov=GameInfo.Governors[row.GovernorType].Index
 local name=row.BasePromotionType..(step==0 and '' or '_TITLE_'..step)
 emit(Events,'GovernorPromoted',owner,gov,GameInfo.GovernorPromotions[name].Index)
end
function appointed(owner,row) emit(Events,'GovernorAppointed',owner,GameInfo.Governors[row.GovernorType].Index) end
function finalize(owner,l) emit(GameEvents,'JAC_LeaderPersonaFinalize',owner,{Leader=l}) end
"#;

fn normal_context(setup: &str) -> Lua {
    let lua = Lua::new();
    lua.load(FIXTURE).exec().unwrap();
    lua.load(NORMAL_FIXTURE).exec().unwrap();
    run(&lua, setup);
    load(&lua, &fs::read_to_string(root().join(GAMEPLAY)).unwrap());
    lua
}

#[test]
fn public_all_22_owners_and_people_sparse_indices_costs_123() {
    // Every owned pair except self-Join; real definitions use a sparse Index,
    // which must never be confused with slot order, player ID or Hash.
    for owner in 0..22 {
        for source in 0..22 {
            if owner == source { continue; }
            let lua = normal_context("");
            run(&lua, &format!(r#"
local owner,source={owner},{source}
local s=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
assert(s and s.commit.nativeClosedAll22 and s.commit.schema==3)
local view=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(owner,PlayerConfigurations[source]:GetLeaderTypeName())
assert(view.active and view.paid==0 and view.reserved==0)
assert(properties.JAC_TEST324_EVENTS_PROBE==nil and properties.JAC_TEST853_PAID_ASSIGNMENT_V1==nil)
local l,row=normalJoin(owner,source); local gov=GameInfo.Governors[row.GovernorType].Index
used[owner]=1
if source%2==0 then appointed(owner,row); success(owner,row,0)
else success(owner,row,0); appointed(owner,row) end
local _,a=normalStatus(owner,l); local seq=a[1].sequence
success(owner,row,0); appointed(owner,row)
assert(select(2,normalStatus(owner,l))[1].sequence==seq)
for paid=1,row.EvolutionCost do
 if paid>1 then used[owner]=paid; success(owner,row,paid-1) end
 if paid<row.EvolutionCost then
  finalize(owner,l); assert(#unitLists[owner]==1 and properties['JAC_PERSONA_'..l..'_STATE']=='EVOLVING')
 end
end
-- An extra promotion is not a title or a merit deed.
local merit=properties['JAC_PERSONA_'..l..'_MERIT']
local extra=0
if row.EvolutionCost<3 then extra=1; used[owner]=used[owner]+1; success(owner,row,row.EvolutionCost) end
finalize(owner,l)
assert(playerProps[owner].JAC_PERSONA_RESULT=='EVOLVED' and #unitLists[owner]==0)
assert(properties['JAC_PERSONA_'..l..'_STATE']=='GOVERNOR' and playerProps[owner][901000+gov]==true)
assert(properties['JAC_PERSONA_'..l..'_MERIT']==merit)
assert(uiReads==0 and operations==0 and used[owner]==row.EvolutionCost+extra)
"#));
        }
    }
}

#[test]
fn public_multiple_reservations_derive_from_each_acquisition_set() {
    let lua=normal_context("");
    run(&lua,r#"
local l2,r2=normalJoin(0,1); local l3,r3=normalJoin(0,2)
-- Allow exactly the two partial reservations, after one base payment each.
used[0]=2; appointed(0,r2); success(0,r2,0); success(0,r3,0); appointed(0,r3)
-- Ordinary reconciliation confirms the now-visible debit. Status stays read-only.
ExposedMembers.JAC_LeaderPersona.Reconcile()
assert(ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,l2).reserved==3)
Players[0]:GetGovernors().GetGovernorPoints=function() return 5 end
ExposedMembers.JAC_LeaderPersona.Reconcile()
assert(playerProps[0][901000+14]==true and playerProps[0][901000+23]==true)
-- Available 3, remaining 1+2, so an additional cost-one evolution is refused.
emit(GameEvents,'JAC_LeaderPersonaJoin',0,{Source=3})
local l1=PlayerConfigurations[3]:GetLeaderTypeName()
emit(GameEvents,'JAC_LeaderPersonaEvolve',0,{Leader=l1})
assert(playerProps[0].JAC_PERSONA_RESULT=='NO_TITLE')
used[0]=3; success(0,r2,1); finalize(0,l2)
assert(properties['JAC_PERSONA_'..l2..'_STATE']=='GOVERNOR')
-- The third person still needs 2. An unrelated title spend cannot pay them.
used[0]=8; finalize(0,l3)
assert(properties['JAC_PERSONA_'..l3..'_STATE']=='EVOLVING' and #unitLists[0]==2)
Players[0]:GetGovernors().GetGovernorPoints=function() return 12 end
emit(GameEvents,'JAC_LeaderPersonaEvolve',0,{Leader=l1})
assert(playerProps[0].JAC_PERSONA_RESULT=='EVOLVING' and uiReads==0)
"#);
}

#[test]
fn public_callback_spent_nil_and_zero_are_immutable_until_later_query() {
    for unavailable in [false,true] {
        let lua=normal_context("");
        run(&lua,"l,row=normalJoin(0,2); gs=Players[0]:GetGovernors(); original=gs.GetGovernorPointsSpent");
        if unavailable {run(&lua,"gs.GetGovernorPointsSpent=function() return nil end");}
        run(&lua,r#"
success(0,row,0); appointed(0,row); success(0,row,1)
finalize(0,l); assert(#unitLists[0]==1)
local _,a=normalStatus(0,l); callbackSpent=a[1].spent; baseSequence=a[1].sequence
gs.GetGovernorPointsSpent=original; used[0]=2
finalize(0,l); assert(properties['JAC_PERSONA_'..l..'_STATE']=='EVOLVING')
success(0,row,2); finalize(0,l); assert(#unitLists[0]==1)
used[0]=3; finalize(0,l); assert(#unitLists[0]==0 and properties['JAC_PERSONA_'..l..'_STATE']=='GOVERNOR')
local _,a=normalStatus(0,l); assert(a[1].spent==callbackSpent and a[1].sequence==baseSequence)
assert(uiReads==0 and operations==0)
"#);
        run(&lua,if unavailable {"assert(callbackSpent==nil)"} else {"assert(callbackSpent==0)"});
    }
}

fn restore_normal(a: &Lua, setup: &str) -> Lua {
    let saved: mlua::Table=a.globals().get("properties").unwrap();
    let b=Lua::new(); b.load(FIXTURE).exec().unwrap(); b.load(NORMAL_FIXTURE).exec().unwrap();
    b.globals().set("properties",transfer(&b,saved)).unwrap();
    run(&b,setup);
    run(&b,"savedNormalStore=clone(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1)");
    load(&b,&fs::read_to_string(root().join(GAMEPLAY)).unwrap());
    run(&b,r#"
local function same(a,b)
 if type(a)~=type(b) then return false end
 if type(a)~='table' then return a==b end
 for k,v in pairs(a) do if not same(v,b[k]) then return false end end
 for k in pairs(b) do if a[k]==nil then return false end end
 return true
end
assert(same(savedNormalStore,properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1),
 'new Lua context changed serialized acquisitions, full raw receipt history, sequence or binding')
"#);
    b
}

#[test]
fn public_partial_replay_saved_context_and_missing_callbacks_never_bootstrap_ui() {
    for missing in ["none","appointed","base","title1","title2"] {
        let a=normal_context("");
        run(&a,"l,row=normalJoin(0,2); used[0]=3");
        if missing!="appointed" {run(&a,"appointed(0,row)");}
        for (step,key) in [(0,"base"),(1,"title1"),(2,"title2")] {
            if missing!=key {run(&a,&format!("success(0,row,{step})"));}
        }
        let b=restore_normal(&a,r#"
used[0]=3; Game.GetGreatPeople():GrantPerson(202,7,8,0,0)
ExposedMembers.JAC_LeaderPersonaNativeRead=function() uiReads=uiReads+1; return {appointed=true,base=true,surchargePaid=2} end
l=PlayerConfigurations[2]:GetLeaderTypeName(); row=GameInfo.JAC_LeaderPersonas[l]
"#);
        run(&b,"local s=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1; seq=s.sequence; before=clone(s.people[l].acquired)");
        for _ in 0..3 {run(&b,"finalize(0,l); ExposedMembers.JAC_LeaderPersona.Reconcile()");}
        if missing=="none" {
            run(&b,"assert(properties['JAC_PERSONA_'..l..'_STATE']=='GOVERNOR' and #unitLists[0]==0)");
        } else {
            run(&b,"assert(properties['JAC_PERSONA_'..l..'_STATE']=='EVOLVING' and #unitLists[0]==1)");
        }
        run(&b,r#"
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.sequence==seq)
assert(uiReads==0 and operations==0 and used[0]==3)
local _,a=normalStatus(0,l); for k,p in pairs(before) do assert(a[k].sequence==p.sequence and a[k].spent==p.spent) end
"#);
        if missing=="none" {
            run(&b,"success(0,row,0); appointed(0,row); success(0,row,1); success(0,row,2); assert(select(2,normalStatus(0,l))[1].sequence==before[1].sequence and used[0]==3)");
        }
    }
}

#[test]
fn public_title_prefix_holes_owner_cross_person_and_dead_major_are_closed() {
    let lua=normal_context("");
    run(&lua,r#"
l,row=normalJoin(0,2); used[0]=3
success(1,row,0); appointed(1,row)
emit(Events,'GovernorPromoted',0,23,81) -- other person's promotion
emit(Events,'GovernorPromoted',0,23,'92')
emit(Events,'GovernorAppointed',0,23,99)
Players[0].IsAlive=function() return false end
success(0,row,0); appointed(0,row)
Players[0].IsAlive=function() return true end
assert(next(select(2,normalStatus(0,l)))==nil)
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.accounts[0].tainted)
success(0,row,0); appointed(0,row); success(0,row,2)
finalize(0,l); ExposedMembers.JAC_LeaderPersona.Reconcile()
assert(#unitLists[0]==1 and playerProps[0][901023]==false)
success(0,row,1); finalize(0,l); assert(#unitLists[0]==1)
assert(uiReads==0 and operations==0)
"#);
    let prefix=normal_context("");
    run(&prefix,"l,row=normalJoin(0,2); used[0]=3; success(0,row,0); appointed(0,row); success(0,row,2); finalize(0,l); assert(#unitLists[0]==1); success(0,row,1); finalize(0,l); assert(#unitLists[0]==0)");
    let early=normal_context("");
    run(&early,r#"
local l=PlayerConfigurations[2]:GetLeaderTypeName(); local row=GameInfo.JAC_LeaderPersonas[l]
success(0,row,0); appointed(0,row)
assert(next(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.people[l].acquired)==nil)
emit(GameEvents,'JAC_LeaderPersonaJoin',0,{Source=2})
assert(playerProps[0].JAC_PERSONA_RESULT=='NATIVE_UNAVAILABLE')
"#);
}

#[test]
fn public_freshness_foreign_schema_and_failed_closed_commit_cannot_retry_later() {
    for setup in [
        "used[0]=1",
        "Game.GetCurrentGameTurn=function() return 1 end",
        "properties.JAC_PERSONA_ADMISSION_SCHEMA_V1=3",
        "properties.JAC_PERSONA_ADMISSION_SCHEMA_V1=77",
        "properties.JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA=1",
        "properties.JAC_TEST853_PAID_ASSIGNMENT_V1=false",
        "properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1={version=77}",
        "properties[prefix..'OWNER']=0",
        "playerProps[4][901014]=true",
        "Players[4].IsAlive=function() return false end; playerProps[4][901014]=true",
        "Players[21].GetProperty=function(_,k) if k==901014 then return true end return playerProps[21][k] end",
        "Events.GovernorAppointed.Add=function() error('no binding') end",
        "Players[21].GetProperty=function() error('failed native close') end",
        "GameInfo.Governors[14].Index=5",
        "GameInfo.GovernorPromotions[92].Index=70",
        "GameInfo.GreatPersonIndividuals[2].Hash=201",
        "PlayerConfigurations[21].GetLeaderTypeName=PlayerConfigurations[20].GetLeaderTypeName",
        "GameInfo.GovernorPromotionSets=nil",
    ] {
        let lua=normal_context(setup);
        run(&lua,r#"
local s=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1
assert(not s or s.version==77)
used[0]=0; properties.JAC_PERSONA_ADMISSION_SCHEMA_V1=3
ExposedMembers.JAC_LeaderPersona.Reconcile()
emit(GameEvents,'JAC_LeaderPersonaJoin',0,{Source=2})
assert(playerProps[0].JAC_PERSONA_RESULT=='NATIVE_UNAVAILABLE' and uiReads==0)
"#);
    }
}

#[test]
fn public_configured_start_turn_initializes_and_restores_without_late_bootstrap() {
    for start in [0,1,50,180] {
        let a=normal_context(&format!("GameConfiguration.GetStartTurn=function() return {start} end; Game.GetCurrentGameTurn=function() return {start} end"));
        run(&a,&format!(r#"
local s=assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1)
assert(s.turn=={start} and s.commit.turn=={start},"configured initial boundary")
local l,row=normalJoin(0,1)
used[0]=1; success(0,row,0); appointed(0,row); finalize(0,l)
assert(ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,l).paid==1,"partial payment before save")
"#));
        let b=restore_normal(&a,&format!("used[0]=1; GameConfiguration.GetStartTurn=function() return {start} end; Game.GetCurrentGameTurn=function() return {} end; Game.GetGreatPeople():GrantPerson(201,7,8,0,0)",start+3));
        run(&b,r#"local l=PlayerConfigurations[1]:GetLeaderTypeName();local s=ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,l);assert(s.active and s.paid==1,"restored partial payment");finalize(0,l);assert(properties['JAC_PERSONA_'..l..'_STATE']=='EVOLVING',"partial person retained")"#);
        run(&b,&format!("GameConfiguration.GetStartTurn=function() return {} end; local l=PlayerConfigurations[1]:GetLeaderTypeName(); assert(not ExposedMembers.JAC_LeaderPersona.EventsGameplayStatus(0,l).active,'changed start boundary accepted old receipt')",start+1));
        let late=normal_context(&format!("GameConfiguration.GetStartTurn=function() return {start} end; Game.GetCurrentGameTurn=function() return {} end",start+1));
        run(&late,"assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1==nil)");
    }
    for setup in ["GameConfiguration=nil","GameConfiguration.GetStartTurn=nil",
        "GameConfiguration.GetStartTurn=function() error('unreadable') end",
        "GameConfiguration.GetStartTurn=function() return '0' end",
        "GameConfiguration.GetStartTurn=function() return -1 end"] {
        let a=normal_context(setup);
        run(&a,"assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1==nil)");
    }
}

#[test]
fn public_fixture_uses_the_shipped_22_person_definitions() {
    let lua=normal_context("");
    let sql=fs::read_to_string(root().join("poc/JAC_PoC_LeaderPersona/Data/JAC_LeaderPersona.sql")).unwrap();
    lua.globals().set("definitionSql",sql).unwrap();
    run(&lua,r#"
local n=0
for row in GameInfo.JAC_LeaderPersonas() do
 assert(string.find(definitionSql,"('"..row.LeaderType.."',",1,true))
 n=n+1
end
assert(n==22 and properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.commit.nativeClosedAll22)
"#);
    let sql=fs::read_to_string(root().join("poc/JAC_PoC_LeaderPersona/Data/JAC_LeaderPersona.sql")).unwrap();
    let expected=lua.create_table().unwrap();
    let mut count=0;
    let designs=sql.split("INSERT INTO JAC_PersonaDesigns VALUES").nth(1).unwrap()
        .split("INSERT INTO JAC_LeaderPersonas").next().unwrap();
    for line in designs.lines().map(str::trim).filter(|s|s.starts_with("('LEADER_JAC_")) {
        let fields: Vec<_>=line.split(',').map(str::trim).collect();
        let leader=fields[0].trim_start_matches('(').trim_matches('\'');
        let age=fields[1].parse::<u32>().ok();
        let fame=fields[2].parse::<u32>().unwrap();
        let role=fields[3].trim_matches('\'');
        let cost=(1+fame+u32::from(age.is_some_and(|n|n<16))+u32::from(role=="REPRESENTATIVE")).min(3);
        expected.set(leader,cost).unwrap(); count+=1;
    }
    assert_eq!(count,22);
    lua.globals().set("expectedCosts",expected).unwrap();
    run(&lua,"for row in GameInfo.JAC_LeaderPersonas() do assert(row.EvolutionCost==expectedCosts[row.LeaderType],row.LeaderType) end");
}

#[test]
fn public_signed_hashes_are_valid_definitions() {
    let lua=normal_context("GameInfo.Governors[14].Hash=-2147483648; GameInfo.GovernorPromotions[92].Hash=-999; GameInfo.GreatPersonIndividuals[2].Hash=-333");
    run(&lua,"assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1); local l,row=normalJoin(0,1); used[0]=2; success(0,row,0); appointed(0,row); success(0,row,1); finalize(0,l); assert(#unitLists[0]==0)");
}

#[test]
fn public_saved_definition_hash_and_receipt_binding_corruption_keep_gp() {
    for corrupt in [
        "properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.version=77",
        "properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.commit.nativeClosedAll22=false",
        "properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.people[l].acquired[1].raw[3].value=70",
        "properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.people[l].acquired[1].owner=1",
        "properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.people[l].acquired[1].context='UI'",
        "properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.people[l].acquired[1].sequence=properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1.people[l].acquired.appointed.sequence",
        "GameInfo.Governors[row.GovernorType].Hash=9999",
        "GameInfo.GovernorPromotions[row.BasePromotionType].Hash=9999",
        "GameInfo.GovernorPromotions[row.BasePromotionType].Index=3000",
        "GameInfo.GreatPersonIndividuals[row.PersonType].Hash=9999",
        "properties['JAC_PERSONA_'..l..'_OWNER']=1",
        "properties['JAC_PERSONA_'..l..'_EVOLUTION_COST']=2",
        "properties['JAC_PERSONA_'..l..'_EVOLUTION_SURCHARGE']=nil",
        "properties['JAC_PERSONA_'..l..'_EVOLUTION_SURCHARGE']=0",
        "properties['JAC_PERSONA_'..l..'_EVOLUTION_SURCHARGE']=1",
    ] {
        let lua=normal_context("");
        run(&lua,"l,row=normalJoin(0,2); used[0]=3; success(0,row,0); appointed(0,row); success(0,row,1); success(0,row,2)");
        run(&lua,corrupt);
        run(&lua,"finalize(0,l); assert(#unitLists[0]==1 and properties['JAC_PERSONA_'..l..'_STATE']=='EVOLVING' and uiReads==0)");
    }
}

#[test]
fn public_removing_original_callbacks_keeps_normal_gp() {
    let source=fs::read_to_string(root().join(GAMEPLAY)).unwrap();
    let callback="Events[name].Add(function(...) receive(name,...) end)";
    assert_eq!(source.matches(callback).count(),1);
    let broken=source.replace(callback,"Events[name].Add(function(...) end)");
    let lua=Lua::new(); lua.load(FIXTURE).exec().unwrap(); lua.load(NORMAL_FIXTURE).exec().unwrap();
    load(&lua,&broken);
    run(&lua,r#"
l,row=normalJoin(0,2); used[0]=3; success(0,row,0); appointed(0,row); success(0,row,1); success(0,row,2)
finalize(0,l); ExposedMembers.JAC_LeaderPersona.Reconcile()
assert(#unitLists[0]==1 and properties['JAC_PERSONA_'..l..'_STATE']=='EVOLVING' and uiReads==0 and operations==0)
"#);
    assert!(lua.load("assert(properties['JAC_PERSONA_'..l..'_STATE']=='GOVERNOR','missing callbacks must break the positive authority')").exec().is_err());
}

#[test]
fn public_old_paid_and_old_partial_without_receipts_remain_unknown_closed() {
    for stage in ["EVOLVING","GOVERNOR"] {
        let lua=normal_context(&format!(r#"
l=PlayerConfigurations[2]:GetLeaderTypeName(); row=GameInfo.JAC_LeaderPersonas[l]
properties.JAC_PERSONA_ADMISSION_SCHEMA_V1=3
properties['JAC_PERSONA_'..l..'_OWNER']=0; properties['JAC_PERSONA_'..l..'_SOURCE']=2
properties['JAC_PERSONA_'..l..'_STATE']='{stage}'
properties['JAC_PERSONA_'..l..'_EVOLUTION_COST']=3
properties['JAC_PERSONA_'..l..'_EVOLUTION_SURCHARGE']=2
used[0]=3
if '{stage}'=='EVOLVING' then
 Game.GetGreatPeople():GrantPerson(202,7,8,0,0); properties['JAC_PERSONA_'..l..'_UNIT']=500
end
ExposedMembers.JAC_LeaderPersonaNativeRead=function() uiReads=uiReads+1; return {{appointed=true,base=true,surchargePaid=2}} end
"#));
        run(&lua,r#"
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1==nil and playerProps[0][901023]==false)
appointed(0,row); success(0,row,0); success(0,row,1); success(0,row,2)
ExposedMembers.JAC_LeaderPersona.Reconcile(); finalize(0,l)
assert(properties.JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1==nil and playerProps[0][901023]==false)
assert(used[0]==3 and uiReads==0 and operations==0)
"#);
        if stage=="EVOLVING" {run(&lua,"assert(#unitLists[0]==1 and properties['JAC_PERSONA_'..l..'_STATE']=='EVOLVING')");}
    }
}

#[test]
fn public_gameplay_route_cannot_call_private_cost_two_api() {
    let source=fs::read_to_string(root().join(GAMEPLAY)).unwrap();
    // Poison private-only entry points; keep their Enabled flag reader intact.
    let injected=source.replace("local function NativeGovernor(owner, row)",
        "PersonaEvents.Paid=function() error('normal path called private Paid') end\nPersonaEvents.Native=function() error('normal path called private Native') end\nPersonaEvents.Scope=function() error('normal path called private Scope') end\nPersonaEvents.Start=function() error('normal path called private Start') end\nlocal function NativeGovernor(owner, row)");
    let lua=Lua::new(); lua.load(FIXTURE).exec().unwrap(); lua.load(NORMAL_FIXTURE).exec().unwrap();
    load(&lua,&injected);
    run(&lua,"local l,row=normalJoin(0,1); used[0]=2; success(0,row,0); appointed(0,row); success(0,row,1); finalize(0,l); assert(#unitLists[0]==0 and playerProps[0][901014]==true and uiReads==0)");
}

#[test]
fn public_and_private_generated_sections_remain_current_and_embed_is_idempotent() {
    let source=fs::read_to_string(root().join(GAMEPLAY)).unwrap().replace("\r\n","\n");
    let one=civ6_persona_events_rs::embed(&source).unwrap();
    assert!(one==source, "public/private generated Gameplay is stale");
    assert!(civ6_persona_events_rs::embed(&one).unwrap()==one, "embed is not idempotent");
    for marker in ["-- BEGIN RUST GENERATED PAID ASSIGNMENT GLUE","-- BEGIN RUST GENERATED APPOINTMENT GLUE"] {
        assert_eq!(source.matches(marker).count(),1);
    }
}
