use serde_json::{json, Value};

pub const DATE: &str = "LEADER_JAC_DATE_TANEMUNE";
pub const UESUGI: &str = "LEADER_JAC_YAMANOUCHI_UESUGI_NORIMASA";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Step,
    Hold,
    Reload,
    Unjoined,
}
#[derive(Clone, Copy)]
pub struct Plan {
    pub phase: Phase,
    pub owner: u32,
    pub cost: u8,
    pub paid: u8,
}
pub fn bundle(owner: u32) -> Result<Vec<(&'static str, Plan)>, &'static str> {
    [
        ("01-unjoined", "unjoined", 2, 0),
        ("02-date-appoint", "step", 2, 0),
        ("03-date-title1", "step", 2, 1),
        ("04-date-complete-hold", "hold", 2, 2),
        ("05-uesugi-appoint", "step", 3, 0),
        ("06-uesugi-title1", "step", 3, 1),
        ("07-uesugi-reload2", "reload", 3, 2),
        ("08-uesugi-title2", "step", 3, 2),
        ("09-uesugi-complete-hold", "hold", 3, 3),
    ]
    .into_iter()
    .map(|(name, phase, cost, paid)| Ok((name, Plan::new(phase, owner, cost, paid)?)))
    .collect()
}

/// Compare actual decoded AI1466_SNAPSHOT objects across the exact physical
/// partial save. This is equality evidence, not native caller/epoch authority.
pub fn compare_reload(before: &Value, after: &Value) -> Result<Value, &'static str> {
    fn valid(v: &Value) -> bool {
        v.as_object().is_some_and(|o| o.len() == 11)
            && v["owner"].as_u64().is_some_and(|n| (1..=21).contains(&n))
            && v["leader"] == UESUGI
            && v["stage"] == "EVOLVING"
            && v["source"]
                .as_u64()
                .is_some_and(|n| n < 64 && v["owner"].as_u64() != Some(n))
            && v["unit"].as_u64().is_some()
            && v["turn"].as_u64().is_some()
            && v["total"]
                .as_u64()
                .zip(v["spent"].as_u64())
                .is_some_and(|(a, b)| a >= b)
            && v["remaining"].as_u64().is_some_and(|n| n >= 1)
            && v["operation_proven"] == false
            && v["people"].as_array().is_some_and(|people| {
                let mut leaders = std::collections::BTreeSet::new();
                let mut indices = std::collections::BTreeSet::new();
                let mut promotions = std::collections::BTreeSet::new();
                let mut attributed = 0u64;
                people.len() == 22
                    && people.iter().all(|p| {
                        let Some(leader) = p["leader"].as_str() else {
                            return false;
                        };
                        let Some(index) = p["index"].as_u64().filter(|n| *n < 256) else {
                            return false;
                        };
                        let Some(cost) = p["cost"].as_u64().filter(|n| (1..=3).contains(n)) else {
                            return false;
                        };
                        let Some(expected) = p["expected"].as_array() else {
                            return false;
                        };
                        let Some(owned) = p["owned"].as_array() else {
                            return false;
                        };
                        let unique_owned: std::collections::BTreeSet<_> =
                            owned.iter().filter_map(Value::as_u64).collect();
                        attributed += owned.len() as u64;
                        leaders.insert(leader)
                            && indices.insert(index)
                            && expected.len() == cost as usize
                            && expected.iter().all(|x| {
                                x.as_u64().is_some_and(|n| n < 4096 && promotions.insert(n))
                            })
                            && p["paid"].as_u64() == Some(owned.len() as u64)
                            && unique_owned.len() == owned.len()
                            && owned.len() <= expected.len()
                            && owned.iter().all(|x| expected[..owned.len()].contains(x))
                            && (leader != UESUGI || (cost == 3 && owned.len() == 2))
                    })
                    && leaders.contains(UESUGI)
                    && v["spent"].as_u64() == Some(attributed)
            })
    }
    if !valid(before) || !valid(after) || before != after {
        return Err("partial reload snapshot absent, invalid or changed");
    }
    Ok(
        json!({"partial_snapshot_equal":true,"operation_proven":false,"pid_generation_barrier_proven":false,"ai_payment_acceptance":false}),
    )
}
impl Plan {
    pub fn new(phase: &str, owner: u32, cost: u8, paid: u8) -> Result<Self, &'static str> {
        let phase = match phase {
            "step" => Phase::Step,
            "hold" => Phase::Hold,
            "reload" => Phase::Reload,
            "unjoined" => Phase::Unjoined,
            _ => return Err("unknown phase"),
        };
        if !(1..=21).contains(&owner)
            || ![2, 3].contains(&cost)
            || paid > cost
            || (phase == Phase::Step && paid == cost)
            || (phase == Phase::Unjoined && paid != 0)
            || (phase == Phase::Reload && (cost != 3 || paid != 2))
        {
            return Err("requires one real AI slot, explicit cost2/3 and valid paid prefix");
        }
        Ok(Self {
            phase,
            owner,
            cost,
            paid,
        })
    }
    fn leader(self) -> &'static str {
        if self.cost == 2 {
            DATE
        } else {
            UESUGI
        }
    }
    fn read(self, paid: u8) -> String {
        format!("local owner={}; local leader='{}'; local requiredCost={}; local requiredPaid={}; local unjoined={}; local requireFinal={}\n{}\n",
            self.owner, self.leader(), self.cost, paid, self.phase == Phase::Unjoined,
            self.phase == Phase::Hold && paid == self.cost, READ)
    }
    pub fn first(self) -> String {
        let mut code = self.read(self.paid);
        code.push_str("JAC1466_previous=nil; local value=read(); JAC1466_previous=value; return encode(value)\n");
        code
    }
    pub fn second(self) -> String {
        let paid = self.paid + u8::from(self.phase == Phase::Step);
        let mut code = self.read(paid);
        code.push_str(&format!(
            "local increment={}\n",
            u8::from(self.phase == Phase::Step)
        ));
        code.push_str(COMPARE);
        code
    }
    pub fn scenario(self) -> Value {
        let ui = |code| json!({"action":"ui_lua","state_name":"InGame","code":code});
        let mut steps = vec![ui(self.first())];
        if self.phase != Phase::Reload {
            steps.push(json!({"action":"end_turn","timeout":300}));
            steps.push(ui(self.second()));
        }
        json!({"name":format!("AI1466 {:?} owner{} cost{} paid{}; native observation only, lifecycle and operation receipt required separately",self.phase,self.owner,self.cost,self.paid),
            "judge_profile":"generic","strict_lua_errors":true,"steps":steps})
    }
    pub fn requirements(self) -> Value {
        json!({"schema":"todo1466-ai-calibration-preparation-v1","parent_reference":"44ade925",
            "owner":self.owner,"leader":self.leader(),"cost":self.cost,"paid_before":self.paid,
            "phase":format!("{:?}",self.phase),"actual_turns":u8::from(self.phase!=Phase::Reload),
            "writes":{"game_properties":0,"UI_AI_requests":0,"native_flags":0,"native_Spent":0},
            "candidate_default_enabled":false,"live_runs":0,"ai_payment_acceptance":false,
            "setup":"Parent must first establish the actual AI owner, normal joined ledger and full reservation. No human-owner UI request substituted. Step0 requires unappointed EVOLVING with a retained Great Person. No fixture authority is generated.",
            "appointment":"Step0->1 observes native APPOINT; current extra-title adapter skips paid0 and does not implement appointment dispatch.",
            "unjoined_denial":"separate Unjoined run requires all22 unjoined and native false qualification before/after one actual AI turn; never counted as paid evidence",
            "sequence":["unjoined independently", "Date0->1", "Date1->2", "Datehold2", "Uesugi0->1", "Uesugi1->2", "parent exact physical save and verified teardown/load", "Uesugireload2", "Uesugi2->3", "Uesugihold3"],
            "snapshot_location":"Use the runner UI result beginning AI1466_SNAPSHOT; decode the following JSON object. For compare-reload pass the saved Uesugi1->2 AFTER object and the reload2 object, not before1 or UI send text.",
            "extra_promotion_limit":3,"appointment_limit":2,"partial_reload_limit":1,
            "bootstrap_boundary":{"install":"completed before first 5E2D0/4012F0/4A7AD0 for the same PID creation and module set", "ordinary_order_observer":"44ade925 front/load probes do not by themselves observe first AI, job-set Wait or the payment gateway"},
            "drain_boundary":{"push":"4A1145", "spawn":"4A10F9", "wait_entry":"4A1191", "wait_return":"4A1196", "conditional_host_merge":"4A11B3", "world_notification_before_wait":"63E1B5", "destruction_before":"2F55B2",
                "requires":["native issuer stop before drain", "same world manager list ID job-set", "pending list published and empty", "real Wait path, not early return", "synchronous callback and upstream reader exit", "no subsequent Push before destruction"]},
            "operation_receipt":{"requires":["actual nonhuman owner and joined source cost/surcharge", "same PID creation, generation and synchronized thread", "natural stage2 callback and original1", "one APPOINT or next personal TITLE request with exact Index", "official filter and checker original result", "same-person Base/TITLE and Spent delta1", "all other native promotion bits unchanged"],
                "checker_rva":"2DDBB0", "official_call_rva":"30EC0F", "approval_call_rva":"30EC24", "execute_rva":"38C80"},
            "reload":"Use exact physical save receipt and hash, never latest pointer. Keep PID/creation/generation receipts external. Match reload snapshot to saved owner, catalog, all flags and Spent; then TITLE2 only. Current driver forbids same-process load under resident runtime; do not bypass it.",
            "missing_results":"unknown/inconclusive, never zero or pass; UI bit/Spent change is corroboration and not proof of which caller performed it",
            "runtime_gaps":["no normal helper action to register BridgeRuntimeAiObserve", "single owner/person registration", "one full turn does not cap multiple stage2 callbacks", "automatic callback execution is not an engine queue receipt", "initial install and issuer drain unproven"]})
    }
}

// UI getters below already exist in the parent human-payment and unjoined
// probes. Absence is a hard diagnostic failure; no Gameplay API is invented.
const READ: &str = r#"
local function integer(v) return type(v)=='number' and v==v and v>=0 and v%1==0 and v<9007199254740992 end
local function boolean(v) assert(type(v)=='boolean','native boolean getter absent/unknown'); return v end
local function identifier(v) assert(type(v)=='string' and v:match('^[A-Z0-9_]+$'),'invalid source identifier'); return v end
local function read()
 assert(Game.GetLocalPlayer()==0 and Players[0]:IsHuman()==true,'local human identity differs')
 local p=assert(Players[owner],'AI owner missing')
 assert(p:IsHuman()==false and p:IsAlive()==true and p:IsMajor()==true,'actual AI owner required')
 local gs=assert(p:GetGovernors(),'UI governors absent')
 for _,name in ipairs({'HasGovernor','GetGovernor','CanEverAppointGovernor','CanAppointGovernor','CanAssignGovernor','GetGovernorPoints','GetGovernorPointsSpent'}) do
  assert(type(gs[name])=='function','missing UI native getter: '..name)
 end
 local turn=Game.GetCurrentGameTurn(); local total=gs:GetGovernorPoints(); local spent=gs:GetGovernorPointsSpent()
 assert(integer(turn) and integer(total) and integer(spent) and total>=spent,'invalid actual balance or turn')
 local rows,seen,indexes,promotions,personTypes={}, {}, {}, {}, {}
 for row in GameInfo.JAC_LeaderPersonas() do
  local id=identifier(row.LeaderType); local def=assert(GameInfo.Governors[row.GovernorType]); local cost=row.EvolutionCost
  assert(not seen[id] and integer(def.Index) and def.Index<256 and not indexes[def.Index],'duplicate identity/index')
  assert(integer(cost) and cost>=1 and cost<=3,'explicit source cost missing')
  seen[id]=true; indexes[def.Index]=true
  personTypes[identifier(row.PersonType)]=true
  local base=assert(GameInfo.GovernorPromotions[row.BasePromotionType]); local expected={base}
  for n=1,cost-1 do expected[#expected+1]=assert(GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_'..n],'personal title definition missing') end
  for _,promotion in ipairs(expected) do
   assert(integer(promotion.Index) and promotion.Index<4096 and not promotions[promotion.Index],'duplicate personal promotion')
   promotions[promotion.Index]=true
  end
  rows[#rows+1]={row=row,def=def,expected=expected}
 end
 assert(#rows==22 and seen[leader],'complete actual catalog required')
 local class=assert(GameInfo.GreatPersonClasses['GREAT_PERSON_CLASS_JAC_PERSONA'],'personal unit class absent')
 local unitType=assert(GameInfo.Units[class.UnitType],'personal unit type absent').Index
 assert(integer(unitType),'unknown personal unit Index')
 local unitCounts,personalUnits={},{}
 for id=0,21 do
  local other=assert(Players[id],'missing configured owner while counting personal units')
  for _,unit in other:GetUnits():Members() do
   if unit:GetType()==unitType then
    local gp=assert(unit:GetGreatPerson(),'personal unit Great Person interface absent')
    assert(gp:IsGreatPerson()==true,'personal unit is not a typed Great Person')
    local person=assert(GameInfo.GreatPersonIndividuals[gp:GetIndividual()],'personal unit individual missing')
    local kind=person.GreatPersonIndividualType
    assert(personTypes[kind],'personal unit individual not in catalog')
    assert(unit:GetOwner()==id and integer(unit:GetID()),'personal unit owner or ID differs')
    unitCounts[kind]=(unitCounts[kind] or 0)+1
    personalUnits[kind]={owner=id,id=unit:GetID()}
    assert(not unjoined,'unjoined personal unit exists')
   end
  end
 end
 table.sort(rows,function(a,b) return a.row.LeaderType<b.row.LeaderType end)
 local allPromotions,seenPromotion={},{}
 for item in GameInfo.GovernorPromotions() do
  assert(integer(item.Index) and item.Index<4096 and not seenPromotion[item.Index],'invalid promotion catalog')
  seenPromotion[item.Index]=true; allPromotions[#allPromotions+1]=item
 end
 assert(#allPromotions>0,'empty promotion catalog')
 table.sort(allPromotions,function(a,b) return a.Index<b.Index end)
 local records,attributed,remaining={},0,0
 local targetSource,targetStage,targetUnit=nil,nil,nil
 for _,entry in ipairs(rows) do
  local row,def=entry.row,entry.def; local key='JAC_PERSONA_'..row.LeaderType..'_'
  local ledgerOwner=Game:GetProperty(key..'OWNER'); local stage=Game:GetProperty(key..'STATE')
  local appointed=boolean(gs:HasGovernor(def.Hash)); local g=gs:GetGovernor(def.Hash)
  assert((appointed and g~=nil) or (not appointed and g==nil),'appointment getters disagree')
  local owned,paid,prefix={},0,true
  if appointed then
   assert(g:GetOwner()==owner and g:GetType()==def.Index,'native person owner/index differs')
   for _,promotion in ipairs(allPromotions) do if boolean(g:HasPromotion(promotion.Hash)) then owned[#owned+1]=promotion.Index end end
   for _,promotion in ipairs(entry.expected) do
    assert(seenPromotion[promotion.Index],'personal promotion absent from full catalog')
    local present=boolean(g:HasPromotion(promotion.Hash)); assert(prefix or not present,'personal prefix has a hole')
    if present then paid=paid+1 else prefix=false end
   end
   assert(paid>=1 and #owned==paid,'Base missing or nonpayment promotion present')
  end
  local remainingForPerson=0
  if ledgerOwner==owner and (stage=='EVOLVING' or stage=='GOVERNOR') then
   assert(Game:GetProperty(key..'EVOLUTION_COST')==row.EvolutionCost and Game:GetProperty(key..'EVOLUTION_SURCHARGE')==row.EvolutionCost-1,'saved cost/surcharge missing or differs')
   assert(stage~='GOVERNOR' or paid==row.EvolutionCost,'unpaid completion')
   if stage=='EVOLVING' then remainingForPerson=row.EvolutionCost-paid end
  end
  if unjoined then
   assert(ledgerOwner==nil and stage==nil and Game:GetProperty(key..'UNIT')==nil,'unjoined lane contains joined ledger')
   assert(not appointed and paid==0,'unjoined AI appointment')
   assert(boolean(gs:CanEverAppointGovernor(def.Hash))==false and boolean(gs:CanAppointGovernor(def.Hash))==false,'unjoined native qualification opened')
  end
  local expectedIndices={}; for _,v in ipairs(entry.expected) do expectedIndices[#expectedIndices+1]=v.Index end
  records[#records+1]={leader=row.LeaderType,index=def.Index,cost=row.EvolutionCost,expected=expectedIndices,owned=owned,paid=paid}
  attributed=attributed+paid; remaining=remaining+remainingForPerson
  if row.LeaderType==leader then
   assert(row.EvolutionCost==requiredCost,'target source cost differs')
   if not unjoined then
    assert(ledgerOwner==owner and (stage=='EVOLVING' or stage=='GOVERNOR'),'target not normally joined/reserved')
    local source=Game:GetProperty(key..'SOURCE')
    assert(integer(source) and source<64 and source~=owner and PlayerConfigurations[source]:GetLeaderTypeName()==leader,'joined source identity differs')
    assert(paid==requiredPaid,'target formal paid prefix differs')
    local units=unitCounts[row.PersonType] or 0; local unitID=Game:GetProperty(key..'UNIT')
    targetSource=source; targetStage=stage; targetUnit=unitID
    if units>0 then local u=personalUnits[row.PersonType]; assert(u.owner==owner and u.id==unitID,'personal Great Person belongs to another owner/unit') end
    assert((stage=='EVOLVING' and units==1 and integer(unitID)) or (stage=='GOVERNOR' and units==0 and unitID==nil and paid==requiredCost),'premature Great Person removal or false completion')
    if paid<requiredCost then
     local cities=0
     for _,city in p:GetCities():Members() do cities=cities+1; assert(boolean(gs:CanAssignGovernor(def.Hash,city))==false,'unpaid placement admitted') end
     assert(cities>0,'no city to verify unpaid placement denial')
    end
    if requireFinal then
     assert(stage=='GOVERNOR' and paid==requiredCost,'normal full-payment completion missing')
     local capital=assert(p:GetCities():GetCapitalCity(),'capital missing')
     assert(boolean(gs:CanAssignGovernor(def.Hash,capital))==true,'fully paid assignment qualification missing')
    end
   end
  end
 end
 assert(attributed==spent,'native Spent not attributable to full personal catalog')
 assert(total-spent>=remaining,'other-person reservations not covered')
 if unjoined then assert(total-spent>=1,'unjoined negative requires a real available title') end
 return {owner=owner,leader=leader,turn=turn,total=total,spent=spent,remaining=remaining,source=targetSource,stage=targetStage,unit=targetUnit,records=records}
end
local function encode(s)
 local rows={}
 for _,r in ipairs(s.records) do rows[#rows+1]='{"leader":"'..r.leader..'","index":'..r.index..',"cost":'..r.cost..',"expected":['..table.concat(r.expected,',')..'],"owned":['..table.concat(r.owned,',')..'],"paid":'..r.paid..'}' end
 local source=s.source~=nil and tostring(s.source) or 'null'
 local unit=s.unit~=nil and tostring(s.unit) or 'null'
 local stage=s.stage~=nil and ('"'..identifier(s.stage)..'"') or 'null'
 return 'AI1466_SNAPSHOT {"owner":'..s.owner..',"leader":"'..s.leader..'","source":'..source..',"stage":'..stage..',"unit":'..unit..',"turn":'..s.turn..',"total":'..s.total..',"spent":'..s.spent..',"remaining":'..s.remaining..',"people":['..table.concat(rows,',')..'],"operation_proven":false}'
end
"#;

const COMPARE: &str = r#"
local previous=assert(JAC1466_previous,'baseline absent: do not infer missing receipt as zero')
local now=read()
assert(now.owner==previous.owner and now.leader==previous.leader and now.turn==previous.turn+1,'not same owner/person across exactly one actual turn')
assert(now.source==previous.source,'joined source changed')
assert(now.spent==previous.spent+increment,'wrong authoritative spend delta')
assert(#now.records==#previous.records,'catalog changed')
for n,r in ipairs(now.records) do
 local old=previous.records[n]
 assert(r.leader==old.leader and r.index==old.index and r.cost==old.cost and table.concat(r.expected,',')==table.concat(old.expected,','),'source identity changed')
 if r.leader==leader and increment==1 then
  assert(r.paid==old.paid+1,'not one formal personal step')
 else assert(table.concat(r.owned,',')==table.concat(old.owned,','),'another person changed; receipt is ambiguous') end
end
JAC1466_previous=nil
return encode(now)
"#;
