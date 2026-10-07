//! Two humans in one hotseat process, not network/multiple-device evidence.
//! Normal UI auto-payment is not paused. BASE-only rejection is observed, not
//! forced or saved. The mid-round save is human0 paid / human1 joined-unpaid.
use crate::sp_runtime::SERIAL;
use serde_json::{json, Value};

pub const LEADERS: [&str; 2] = ["LEADER_JAC_DATE_TANEMUNE", "LEADER_JAC_HOJO_UJITSUNA"];
pub const MID: &str = "JAC-Persona-Hotseat-Mid-Unpaid";
pub const FULL: &str = "JAC-Persona-Hotseat-Both-Paid";
pub const READ: &str = r#"
local function person(owner, gate)
 local subject=assert(subjects[owner+1]);local leader=subject.leader
 local row=assert(GameInfo.JAC_LeaderPersonas[leader]);local def=assert(GameInfo.Governors[row.GovernorType])
 local p=assert(Players[owner]);local gs=p:GetGovernors();local api=assert(ExposedMembers.JAC_LeaderPersona)
 local s=api.EventsGameplayStatus(owner,leader);assert(s.active and s.source=='civ6-persona-events-rs/v3/normal-Gameplay')
 assert(s.registrations.GovernorAppointed and s.registrations.GovernorPromoted)
 assert(row.EvolutionCost==2 and PlayerConfigurations[subject.source]:GetLeaderTypeName()==leader)
 local prefix='JAC_PERSONA_'..leader..'_';local function prop(k)return Game:GetProperty(prefix..k) end
 local state=prop('STATE');local actualOwner=prop('OWNER');local unit=prop('UNIT')
 local g=gs:GetGovernor(def.Hash);local spent=gs:GetGovernorPointsSpent()
 assert(type(spent)=='number' and spent>=0 and spent<=2 and spent%1==0)
 local cities={};for _,c in p:GetCities():Members() do assert(c:GetOwner()==owner);cities[#cities+1]=c end
 table.sort(cities,function(a,b)return a:GetID()<b:GetID() end)
 local assigned=g and g:GetAssignedCity();local city
 for _,c in ipairs(cities)do if not assigned or c:GetID()~=assigned:GetID()then city=c;break end end
 city=assert(city,'an owned city distinct from the assigned city is required')
 local allowed=nil;if gate then allowed=gs:CanAssignGovernor(def.Hash,city);assert(type(allowed)=='boolean')end
 local out={owner=owner,leader=leader,source=subject.source,index=def.Index,hash=def.Hash,state=state or 'UNJOINED',
  unit=unit or -1,spent=spent,paid=s.paid==nil and -1 or s.paid,remaining=s.paid==nil and -1 or 2-s.paid,
  points=gs:GetGovernorPoints(),city=city:GetID(),assigned=assigned and assigned:GetID() or -1,allowed=allowed,
  acquired=s.acquired or {},account=s.account,reservation=s.reservation}
 if actualOwner==nil then
  assert(state==nil and unit==nil and not g and spent==0 and s.paid==0)
  return out
 end
 assert(actualOwner==owner and prop('SOURCE')==subject.source and prop('EVOLUTION_COST')==2,'person/owner/source mismatch')
 local count=0;local individual=assert(GameInfo.GreatPersonIndividuals[row.PersonType])
 local kind=assert(GameInfo.Units[GameInfo.GreatPersonClasses[individual.GreatPersonClassType].UnitType])
 for _,pid in ipairs(PlayerManager.GetAliveMajorIDs())do for _,u in Players[pid]:GetUnits():Members()do
  if u:GetType()==kind.Index then local id=u:GetGreatPerson():GetIndividual();local r=id~=nil and id~=-1 and GameInfo.GreatPersonIndividuals[id]
   if r and r.GreatPersonIndividualType==row.PersonType then count=count+1;assert(pid==owner and u:GetOwner()==owner and u:GetID()==unit,'foreign or duplicate GP')end
  end
 end end
 if state=='GREAT_PERSON' then
  assert(not g and spent==0 and s.paid==0 and type(unit)=='number' and count==1)
  if gate then assert(allowed==false,'unpaid GP assignment permitted')end
  return out
 end
 if not g and state=='EVOLVING' then return nil end
 assert(g and g:GetOwner()==owner and g:GetType()==def.Index,'native person identity differs')
 local base=assert(GameInfo.GovernorPromotions[row.BasePromotionType]);local title=assert(GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_1'])
 local b,t=g:HasPromotion(base.Hash),g:HasPromotion(title.Hash)
 assert(type(b)=='boolean' and type(t)=='boolean' and (not t or b))
 local a=s.acquired or {}
 -- UI native notifications can arrive before the normal Gameplay receipt.
 if not b or not a.appointed or not a[1] then return nil end
 local seen={}
 for _,key in ipairs({'appointed',1,2})do local r=a[key]
  if key~=2 or t then
   if not r then return nil end
   local promo=key==1 and base or key==2 and title or nil
   assert(r.owner==owner and r.leader==leader and r.sourcePlayer==subject.source and r.accepted==true)
   assert(r.source==s.source and r.context=='Gameplay' and r.governor==def.Index and r.governorHash==def.Hash)
   assert(not seen[r.sequence]);seen[r.sequence]=true
   assert(r.raw[1].type=='number' and r.raw[1].value==owner and r.raw[2].type=='number' and r.raw[2].value==def.Index)
   assert(r.name==(promo and 'GovernorPromoted' or 'GovernorAppointed') and r.arity==(promo and 3 or 2))
   if promo then assert(r.promotion==promo.Index and r.promotionHash==promo.Hash and r.raw[3].type=='number' and r.raw[3].value==promo.Index)end
  end
 end
 out.base=b;out.title=t
 if state=='EVOLVING' and not t then
  assert(spent==1 and (s.paid==nil or s.paid==1) and count==1 and type(unit)=='number' and a[2]==nil)
  if gate then assert(allowed==false,'BASE-only unpaid assignment permitted')end
  out.phase='partial';return out
 end
 if state~='GOVERNOR' then return nil end
 assert(t and spent==2 and s.paid==2 and unit==nil and count==0 and prop('EVOLUTION_SURCHARGE')==1)
 for n=1,2 do local r=a[n];local debit=assert(s.account.debits[r.spendingSerial]);local c=assert(debit.confirmed)
  assert(debit.owner==owner and debit.governor==def.Index and debit.promotion==r.promotion and debit.binding==r.binding and not debit.invalidated)
  assert(c.owner==owner and c.binding==r.binding and c.afterSpent==c.expected and c.afterSpent<=spent)
 end
 out.phase='full';return out
end
local function ledger()
 -- Do not query native eligibility without explicitly installed runtime.
 return {active=Game.GetLocalPlayer(),turn=Game.GetCurrentGameTurn(),people={assert(person(0,false)),assert(person(1,false))},
  store=Game:GetProperty('JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1')}
end
"#;

pub fn header(sources: [usize; 2]) -> String {
    format!(
        r#"
assert(GameConfiguration.IsHotseat(),'single-machine hotseat required; not network evidence')
local subjects={{{{leader='{a}',source={sa}}},{{leader='{b}',source={sb}}}}}
local ids=PlayerManager.GetAliveMajorIDs();assert(#ids==22)
local seen={{}};for _,id in ipairs(ids)do assert(not seen[id]);seen[id]=true;assert(Players[id]:IsHuman()==(id==0 or id==1))end
assert(seen[0] and seen[1])
for _,key in ipairs({{'JAC_TEST324_EVENTS_PROBE','JAC_TEST853_ADMISSION_ACTIVE','JAC_TEST853_ADMISSION_MANUAL_APPOINT','JAC_TEST853_PAID_ASSIGNMENT_V1'}})do assert(Game:GetProperty(key)==nil,'private diagnostic flag present')end
"#,
        a = LEADERS[0],
        b = LEADERS[1],
        sa = sources[0],
        sb = sources[1]
    )
}
fn ui(state: &str, code: impl Into<String>) -> Value {
    json!({"action":"ui_lua","state_name":state,"code":code.into()})
}
fn wait(code: impl Into<String>) -> Value {
    json!({"action":"wait_for","timeout":90,"condition":{"type":"lua","state_name":"InGame","code":code.into()}})
}
fn active(owner: usize, round: usize) -> String {
    format!("assert(Game.GetLocalPlayer()=={owner} and Players[{owner}]:IsTurnActive() and Game.GetCurrentGameTurn()==GameConfiguration.GetStartTurn()+{round},'unexpected hotseat owner/round');")
}
pub fn observer(h: &str, owner: usize) -> String {
    format!(
        r#"{h}
{READ}
assert(JAC_MP_OBSERVER==nil,'old observer not detached')
local q={{owner={owner}}};JAC_MP_OBSERVER=q
q.observe=function()
 if q.error then return end
 local ok,v=pcall(person,{owner},true)
 if not ok then q.error=tostring(v);print('JAC_MP_OBSERVER_ERROR '..q.error);return end
 if not v then return end
 if v.phase=='partial' and not q.partial then q.partial=v end
 if v.phase=='full' and not q.full then if v.allowed~=true then q.error='paid assignment denied';return end;q.full=v end
end
Events.GovernorChanged.Add(q.observe);Events.GameCoreEventPublishComplete.Add(q.observe)
return 'read-only per-owner observer; standard UI remains sole title sender'
"#
    )
}
fn attach(steps: &mut Vec<Value>, suffix: &str) {
    steps.push(json!({"action":"start_native_paid_runtime","name":format!("hotseat-paid-{suffix}"),"max_seconds":300,"timeout":30}));
    steps.push(ui("InGame","local owner=Game.GetLocalPlayer();assert(Players[owner]:IsHuman() and Players[owner]:IsTurnActive());UI.RequestPlayerOperation(owner,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_LeaderPersonaRepublish'});return 'one normal republish to new native runtime; no payment'"));
}
fn stop(steps: &mut Vec<Value>) {
    steps.push(json!({"action":"stop_native_paid_runtime_observation","timeout":30}));
}
fn capture(steps: &mut Vec<Value>, code: &str, key: &str, compare: bool) {
    let mut s = ui("InGame", code);
    s[if compare {
        "equals_capture"
    } else {
        "capture_as"
    }] = key.into();
    steps.push(s);
}
fn pay(steps: &mut Vec<Value>, h: &str, owner: usize) {
    steps.push(ui("InGame", observer(h, owner)));
    steps.push(ui("JAC_LeaderPersona_View",format!("assert(Game.GetLocalPlayer()=={owner} and Players[{owner}]:IsTurnActive()); JAC_EvolveLeader('{}');return 'one ordinary evolution request for active human'",LEADERS[owner])));
    steps.push(wait("local q=assert(JAC_MP_OBSERVER);q.observe();assert(not q.error,q.error);return q.partial~=nil and q.full~=nil"));
    steps.push(ui("InGame",format!(r#"{h}
{SERIAL}
local q=assert(JAC_MP_OBSERVER);assert(not q.error,q.error);local a,b=assert(q.partial),assert(q.full)
assert(a.owner=={owner} and b.owner=={owner} and a.leader==b.leader and a.index==b.index and a.city==b.city)
assert(a.allowed==false and b.allowed==true and a.spent==1 and b.spent==2 and a.points==b.points)
assert(b.paid==2 and b.remaining==0 and a.acquired[1].sequence==b.acquired[1].sequence and a.acquired.appointed.sequence==b.acquired.appointed.sequence)
Events.GovernorChanged.Remove(q.observe);Events.GameCoreEventPublishComplete.Remove(q.observe);JAC_MP_OBSERVER=nil
return serial({{partial=a,full=b}})
"#)));
    steps.push(ui("JAC_LeaderPersona_View",format!(r#"assert(Game.GetLocalPlayer()=={owner} and Players[{owner}]:IsTurnActive())
local ids={{}};for _,c in Players[{owner}]:GetCities():Members()do ids[#ids+1]=c:GetID()end;table.sort(ids)
JAC_AssignLeader('{}',assert(ids[1]));return 'one void normal assignment; actual completion follows'
"#,LEADERS[owner])));
    steps.push(wait(format!(r#"{h}
{READ}
local v=person({owner},false);if not v or v.phase~='full' then return false end
local ids={{}};for _,c in Players[{owner}]:GetCities():Members()do ids[#ids+1]=c:GetID()end;table.sort(ids)
return v.assigned==ids[1]
"#)));
}
/// Start from the two proven templates; only expand the 22-player roster and
/// add persona assertions. Configure remains normal hotseat, no bridge nonce.
pub fn build(normal: &Value, hotseat: &Value) -> Result<Value, String> {
    if hotseat["session_type"] != "hotseat"
        || hotseat["steps"][1]["ruleset"] != "RULESET_JAC_BANSEIKI"
    {
        return Err("PR121 normal hotseat template required".into());
    }
    let full = normal["steps"][1]["players"]
        .as_array()
        .filter(|p| p.len() == 22)
        .ok_or("normal 22-player roster required")?;
    let humans = hotseat["steps"][1]["players"]
        .as_array()
        .filter(|p| p.len() == 2 && p.iter().all(|v| v["human"] == true))
        .ok_or("two PR121 human players required")?;
    let mut roster = humans.clone();
    for p in full {
        if !humans.iter().any(|v| v["leader"] == p["leader"]) {
            let mut p = p.clone();
            p["human"] = false.into();
            roster.push(p)
        }
    }
    if roster.len() != 22 {
        return Err("duplicate or foreign template leaders".into());
    }
    let mut unique = std::collections::BTreeSet::new();
    for p in &roster {
        if !unique.insert(p["leader"].as_str().ok_or("leader absent")?) {
            return Err("duplicate roster leader".into());
        }
    }
    let mut sources = [0; 2];
    for (i, l) in LEADERS.iter().enumerate() {
        sources[i] = roster
            .iter()
            .position(|p| p["leader"] == *l)
            .filter(|n| *n >= 2)
            .ok_or("persona source must be a foreign AI")?;
    }
    let h = header(sources);
    let mut configure = normal["steps"][1].clone();
    configure["players"] = json!(roster);
    configure["seed"] = hotseat["steps"][1]["seed"].clone();
    let mut enable = normal["steps"][0].clone();
    // Preserve the persona MOD set, while retaining PR121's isolated hotseat
    // setup so unrelated user MODs cannot enter this finite diagnostic.
    enable["exclusive_user_mods"] = true.into();
    let mut steps = vec![
        enable,
        json!({"action":"restart_game"}),
        configure,
        hotseat["steps"][2].clone(),
        wait(format!(
            "{h}\nreturn Game.GetLocalPlayer()==0 and Players[0]:IsTurnActive()"
        )),
    ];
    steps.push(ui("InGame",format!(r#"{h}
{READ}
{}
for owner=0,1 do local v=assert(person(owner,false));assert(v.state=='UNJOINED' and v.paid==0 and v.remaining==2)end
local count=0;for _,id in ipairs(PlayerManager.GetAliveMajorIDs())do for r in GameInfo.JAC_LeaderPersonas()do
 assert(Game:GetProperty('JAC_PERSONA_'..r.LeaderType..'_OWNER')==nil)
 assert(not Players[id]:GetGovernors():HasGovernor(GameInfo.Governors[r.GovernorType].Hash));count=count+1
end end;assert(count==484);return 'fresh22, two human owners and484 absent governors'
"#,active(0,0))));
    steps.push(ui("GameCore_Tuner",format!(r#"{h}
for owner=0,1 do local p=Players[owner];local gs=p:GetGovernors();assert(gs:GetGovernorPointsSpent()==0)
 local culture=p:GetCulture();for row in GameInfo.JAC_PersonaTitleCivics()do if gs:GetGovernorPoints()>=2 then break end
  local c=assert(GameInfo.Civics[row.CivicType]);if not culture:HasCivic(c.Index)then culture:SetCulturalProgress(c.Index,culture:GetCultureCost(c.Index))end
 end;assert(gs:GetGovernorPoints()>=2 and gs:GetGovernorPointsSpent()==0)
end;return 'official civic title grants for both humans; no balance setters'
"#)));
    attach(&mut steps, "initial");
    for (owner, source) in sources.iter().enumerate() {
        steps.push(ui(
            "JAC_LeaderPersona_View",
            format!(
                "{} JAC_RequestLeaderJoin({});return 'one ordinary Join request'",
                active(owner, 0),
                source
            ),
        ));
        steps.push(wait(format!("{h}\n{READ}\nlocal v=person({owner},true);return v and v.state=='GREAT_PERSON' and v.paid==0 and v.remaining==2")));
        if owner == 1 {
            let code=format!("{h}\n{SERIAL}\n{READ}\n{} local v=ledger();assert(v.people[1].phase=='full' and v.people[2].state=='GREAT_PERSON' and v.people[2].paid==0 and v.people[2].remaining==2);return serial(v)",active(1,0));
            capture(&mut steps, &code, "HOTSEAT_MID_JOINED_UNPAID", false);
            steps.extend([json!({"action":"save","name":MID,"exact_name":true,"checkpoint":true}),json!({"action":"restart_game"}),json!({"action":"load","name":MID,"exact_name":true,"checkpoint":true,"timeout":300})]);
            capture(&mut steps, &code, "HOTSEAT_MID_JOINED_UNPAID", true);
            attach(&mut steps, "mid-reconnected");
            steps.push(wait(format!("{h}\n{READ}\nlocal a,b=person(0,true),person(1,true);return a and b and a.allowed==true and b.allowed==false")));
        }
        pay(&mut steps, &h, owner);
        if owner == 1 {
            capture(
                &mut steps,
                &format!("{h}\n{SERIAL}\n{READ}\nreturn serial(assert(person(0,false)))"),
                "HUMAN0_PAID",
                true,
            );
        }
        let personal=format!("{h}\n{SERIAL}\n{READ}\nlocal v=assert(person({owner},false));assert(v.phase=='full');return serial(v)");
        capture(&mut steps, &personal, &format!("HUMAN{owner}_PAID"), false);
        stop(&mut steps);
        steps.push(json!({"action":"end_turn","timeout":600}));
        steps.push(ui(
            "InGame",
            format!(
                "{h}\n{} return 'actual hotseat handoff confirmed'",
                active(1 - owner, owner)
            ),
        ));
        capture(&mut steps, &personal, &format!("HUMAN{owner}_PAID"), true);
    }
    let both=format!("{h}\n{SERIAL}\n{READ}\nlocal v=ledger();for _,p in ipairs(v.people)do assert(p.phase=='full' and p.paid==2 and p.remaining==0 and p.spent==2)end;return serial(v)");
    capture(&mut steps, &both, "HOTSEAT_BOTH_PAID_SAVE", false);
    steps.extend([
        json!({"action":"save","name":FULL,"exact_name":true,"checkpoint":true}),
        json!({"action":"restart_game"}),
        json!({"action":"load","name":FULL,"exact_name":true,"checkpoint":true,"timeout":300}),
    ]);
    capture(&mut steps, &both, "HOTSEAT_BOTH_PAID_SAVE", true);
    attach(&mut steps, "full-reconnected");
    steps.push(wait(format!("{h}\n{READ}\nlocal a,b=person(0,true),person(1,true);return a and b and a.allowed==true and b.allowed==true")));
    steps.push(ui("InGame",format!(r#"{h}
{READ}
for owner=0,1 do local v=assert(person(owner,true));local city=assert(CityManager.GetCity(owner,v.city))
 local foreign=GameInfo.Governors[GameInfo.JAC_LeaderPersonas[subjects[2-owner].leader].GovernorType]
 assert(Players[owner]:GetGovernors():CanAssignGovernor(foreign.Hash,city)==false,'other human person accepted')
end;return 'reconnected own paid permits and cross-owner denial'
"#)));
    capture(&mut steps, &both, "HOTSEAT_BOTH_PAID_SAVE", true);
    stop(&mut steps);
    steps.push(json!({"action":"restart_game"}));
    Ok(
        json!({"name":"Persona two-human22-major hotseat: per-owner official cost2 BASE-only denial/full permit, actual handoff, joined-unpaid mid-round save and full save, explicit runtime reconnect; not network multi-device or saved Spent1 proof",
 "session_type":"hotseat","judge_profile":"generic","strict_lua_errors":true,"steps":steps}),
    )
}

/// MP10 hit the stock StagingRoom twelve-slot limit before Ready. Isolate
/// startup at that limit with the same six MODs and an ordinary generated map.
/// This never opens normal persona accounting (which still requires 22).
pub fn build_setup_probe(normal: &Value, hotseat: &Value) -> Result<Value, String> {
    let payment = build(normal, hotseat)?;
    let mut config = payment["steps"][2].clone();
    config["players"]
        .as_array_mut()
        .ok_or("players absent")?
        .truncate(12);
    // Official SetupParameters.xml uses Pangaea.lua as the MP default; never
    // infer a map from defaults: request the official MP default explicitly.
    config["map_file"] = "Pangaea.lua".into();
    let roster = config["players"].as_array().unwrap();
    let mut expected = String::from("local expected={\n");
    for p in roster {
        expected.push_str(&format!(
            "{{leader='{}',civ='{}'}},\n",
            p["leader"].as_str().unwrap(),
            p["civ"].as_str().unwrap()
        ));
    }
    expected.push_str("}\n");
    let preflight = format!(
        r#"{expected}
assert(UI.IsInFrontEnd() and GameConfiguration.IsHotseat(),'hotseat FrontEnd required')
assert(GameConfiguration.GetRuleSet()=='RULESET_JAC_BANSEIKI')
local actualMap=MapConfiguration.GetValue('MAP_SCRIPT')
print('MP_SETUP12 MAP_SCRIPT type='..type(actualMap)..' value='..tostring(actualMap))
assert(type(actualMap)=='string' and actualMap=='Pangaea.lua','ordinary generated Pangaea map required')
assert(MapConfiguration.GetMaxMajorPlayers()==12,'map maximum differs from stock staging limit')
assert(GameConfiguration.GetParticipatingPlayerCount()==12+GameConfiguration.GetHiddenPlayerCount())
assert(GameConfiguration.GetValue('CITY_STATE_COUNT')==0)
for id=0,11 do local p=assert(PlayerConfigurations[id]);local e=expected[id+1]
 assert(p:GetLeaderTypeName()==e.leader and p:GetCivilizationTypeName()==e.civ)
 assert(p:GetSlotStatus()==(id<2 and SlotStatus.SS_TAKEN or SlotStatus.SS_COMPUTER),'human/AI slot mismatch')
end
return 'MP_SETUP12 PREFLIGHT map=Pangaea.lua max=12 majors=12 humans=0,1 ai=2..11; no Host sent by this read'
"#
    );
    let arrived = format!(
        r#"{expected}
assert(GameConfiguration.IsHotseat() and Game.GetLocalPlayer()==0 and Players[0]:IsTurnActive())
assert(Game.GetCurrentGameTurn()==GameConfiguration.GetStartTurn(),'unexpected starting turn')
local actualMap=MapConfiguration.GetValue('MAP_SCRIPT')
print('MP_SETUP12 STARTED MAP_SCRIPT type='..type(actualMap)..' value='..tostring(actualMap))
assert(type(actualMap)=='string' and actualMap=='Pangaea.lua')
local ids=PlayerManager.GetAliveMajorIDs();assert(#ids==12,'actual major count differs');local seen={{}}
for _,id in ipairs(ids)do assert(type(id)=='number' and id>=0 and id<12 and not seen[id]);seen[id]=true
 local p=assert(Players[id]);local cfg=assert(PlayerConfigurations[id]);local e=expected[id+1]
 assert(p:IsAlive() and p:IsMajor() and p:IsHuman()==(id<2))
 assert(cfg:GetLeaderTypeName()==e.leader and cfg:GetCivilizationTypeName()==e.civ)
end
assert(seen[0] and seen[1] and not GameConfiguration.IsPaused())
local api=assert(ExposedMembers.JAC_LeaderPersona,'persona MOD not loaded')
local status=api.EventsGameplayStatus(0,'LEADER_JAC_DATE_TANEMUNE')
assert(status.active==false and Game:GetProperty('JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1')==nil,'12-major probe must not bootstrap normal22 accounting')
return 'MP_SETUP12 STARTED actual12majors human0,1 ai10; player0 active; persona accounting remains unavailable; no payment acceptance'
"#
    );
    Ok(
        json!({"name":"MP setup-only12: two humans ten AI, six persona MODs, generated Pangaea; preflight and actual game start, no native runtime/payment/22-major acceptance",
        "session_type":"hotseat","judge_profile":"generic","strict_lua_errors":true,"steps":[
         payment["steps"][0],{"action":"restart_game"},config,
         {"action":"ui_lua","state_name":"FrontEnd","code":preflight},
         hotseat["steps"][2],
         {"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":"return GameConfiguration.IsHotseat() and Game.GetLocalPlayer()==0 and Players[0] and Players[0]:IsTurnActive()"}},
         {"action":"ui_lua","state_name":"InGame","code":arrived},
         {"action":"exit_to_menu","timeout":90}
        ]}),
    )
}
