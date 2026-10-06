//! Finite continuation of a successful reassignment save, using original PoC UI.
use crate::{
    sp_reassign::READER,
    sp_runtime::{HEADER, SERIAL},
};
use serde_json::{json, Value};
pub const NODE: &str = "LEADER_JAC_DATE_TANEMUNE_ENVOYS_1";
pub const SAVE: &str = "JAC-SP-Normal-Merit-Skill";
pub const MAX_EARNING_TURNS: u32 = 4;
pub const EARN_REQUEST: &str = r#"
local q=assert(JAC_SKILL_AUDIT); local current=snapshot()
assert(Players[0]:IsTurnActive() and current.established==true and current.neutralized==0)
q.advanced=current.merit<q.cost
q.expectedTurn=current.turn+(q.advanced and 1 or 0)
if q.advanced then
 q.realTurns=q.realTurns+1; assert(q.realTurns<=q.requiredTurns,'earning exceeded GameInfo-derived bound')
 UI.RequestAction(ActionTypes.ACTION_ENDTURN)
end
return q.advanced and 'one ordinary UI end-turn requested; never resent by wait' or 'enough earned merit; no turn requested'
"#;
fn ui(state: &str, code: impl Into<String>) -> Value {
    json!({"action":"ui_lua","state_name":state,"code":code.into()})
}
fn wait(code: impl Into<String>) -> Value {
    json!({"action":"wait_for","timeout":90,"condition":{"type":"lua","state_name":"InGame","code":code.into()}})
}
fn original(template: &Value, needle: &str) -> Result<Value, String> {
    template["steps"]
        .as_array()
        .ok_or("missing original steps")?
        .iter()
        .find(|s| s["code"].as_str().is_some_and(|c| c.contains(needle)))
        .cloned()
        .ok_or_else(|| format!("original PoC operation missing: {needle}"))
}
pub fn build(template: &Value, input: Option<&str>, merit: Option<u32>) -> Result<Value, String> {
    if input.is_some() != merit.is_some() {
        return Err("physical save and its observed merit must be supplied together".into());
    }
    let name = input.unwrap_or("REASSIGN_SUCCESS_SAVE_NOT_YET_SUPPLIED");
    if name.is_empty()
        || name.len() > 80
        || !name.as_bytes()[0].is_ascii_alphanumeric()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        || name == SAVE
    {
        return Err("invalid physical save basename".into());
    }
    if template["steps"][0]["action"] != "enable_mods" {
        return Err("normal mod setup missing".into());
    }
    let expected_merit = merit.unwrap_or(0);
    let mut steps = vec![];
    if input.is_none() {
        steps.push(ui("MainMenu","error('DRAFT: successful reassignment physical save and observed merit are not supplied')"));
    }
    steps.extend([template["steps"][0].clone(),json!({"action":"restart_game"}),json!({"action":"load","name":name,"exact_name":true,"timeout":300}),wait("return Players[0] and Players[0]:IsTurnActive() and type(ExposedMembers.JAC_LeaderPersona)=='table'")]);
    steps.push(ui("InGame",format!(r#"{HEADER}
{SERIAL}
{READER}
local current=snapshot(); assert(current.established==true and current.neutralized==0)
assert(current.merit=={expected_merit},'input merit differs; regenerate finite funding plan')
local node=assert(GameInfo.JAC_PersonaTreeNodes['{NODE}'])
local profile=GameInfo.JAC_PersonaTreePeople[leader].MeritProfile
local reward=GameInfo.JAC_PersonaMeritRules[profile].GOVERN_TURN
assert(type(node.Cost)=='number' and node.Cost>0 and node.Cost%1==0 and type(reward)=='number' and reward>0 and reward%1==0)
local minimum
for candidate in GameInfo.JAC_PersonaTreeNodes() do if candidate.LeaderType==leader and not candidate.PrereqNode then
 assert(type(candidate.Cost)=='number' and candidate.Cost>0 and candidate.Cost%1==0)
 minimum=minimum and math.min(minimum,candidate.Cost) or candidate.Cost
end end
assert(node.Cost==minimum,'original legal PoC node is no longer the cheapest; do not overpay')
assert(node.CategoryType=='CONTACT' and node.RouteType=='ENVOYS' and node.ExclusiveGroup=='RELATIONS')
assert(current.stable.category.kind=='nil')
for _,v in pairs(current.stable.skills) do assert(v.value~=1,'input already owns a skill') end
for _,v in pairs(current.stable.branches) do assert(v.kind=='nil','input already selected a branch') end
assert(JAC_SKILL_AUDIT==nil,'do not replay skill continuation')
local required=math.ceil(math.max(0,node.Cost-current.merit)/reward)
assert(required<={MAX_EARNING_TURNS},'GameInfo price requires more than four ordinary earning turns')
JAC_SKILL_AUDIT={{initial=current,previous=current,cost=node.Cost,reward=reward,requiredTurns=required,realTurns=0,lastFundingStep=0}}
return serial({{state=current,node=node.NodeType,cost=node.Cost,reward=reward,requiredRealTurns=required}})
"#)));
    steps.push(json!({"action":"start_native_paid_runtime","name":"normal-merit-skill-runtime","max_seconds":300,"timeout":30}));
    steps.push(ui("InGame","UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{OnStart='JAC_LeaderPersonaRepublish'}); return 'one normal republish; no title payment'"));
    steps.push(json!({"action":"stop_native_paid_runtime_observation","timeout":30}));
    for n in 1..=MAX_EARNING_TURNS {
        steps.push(ui("InGame",format!("{HEADER}\n{READER}\nlocal prior=assert(JAC_SKILL_AUDIT); assert(prior.lastFundingStep<{n},'do not resend end turn'); prior.lastFundingStep={n}\n{EARN_REQUEST}")));
        let mut turn_wait=wait("local q=assert(JAC_SKILL_AUDIT); return Game.GetCurrentGameTurn()>=q.expectedTurn and Players[0]:IsTurnActive()");
        turn_wait["timeout"] = 600.into();
        steps.push(turn_wait);
        steps.push(ui("InGame",format!(r#"{HEADER}
{SERIAL}
{READER}
local q=assert(JAC_SKILL_AUDIT); local current=snapshot()
assert(current.turn==q.expectedTurn and current.turn==q.initial.turn+q.realTurns and Players[0]:IsTurnActive())
assert(current.city.id==q.initial.city.id and current.established==true and current.neutralized==0)
assert(serial(current.stable)==serial(q.initial.stable),'title history or skills changed during earning')
if q.advanced then
assert(current.merit>=q.previous.merit+q.reward,'ordinary established turn did not earn the configured merit')
assert(Game:GetProperty('JAC_PERSONA_'..leader..'_DEED_GOVERN_TURN_'..current.turn)==current.turn)
assert(Game:GetProperty('JAC_PERSONA_'..leader..'_GOVERN_LAST_TURN')==current.turn)
else assert(current.merit>=q.previous.merit) end
q.previous=current; return serial(current)
"#)));
    }
    steps.push(ui("InGame",format!(r#"{HEADER}
{SERIAL}
{READER}
local q=assert(JAC_SKILL_AUDIT); local current=snapshot()
assert(current.established==true and current.neutralized==0 and current.city.id==q.initial.city.id)
assert(serial(current.stable)==serial(q.initial.stable) and current.merit>=q.cost,'normal earned merit insufficient')
q.before=current; q.purchaseSerial=(Players[0]:GetProperty('JAC_PERSONA_PURCHASE_SERIAL') or 0)+1
return serial(current)
"#)));
    // Exact original UI review and confirm, not a new product entry point.
    steps.push(original(template, "return 'review shows permanent locks'")?);
    steps.push(original(template, "return 'requested gameplay purchase'")?);
    steps.push(wait(format!("local q=assert(JAC_SKILL_AUDIT);local p=Players[0];return p:GetProperty('JAC_PERSONA_PURCHASE_SERIAL')==q.purchaseSerial and p:GetProperty('JAC_PERSONA_PURCHASE_RESULT')=='PURCHASED' and p:GetProperty('JAC_PERSONA_PURCHASE_NODE')=='{NODE}' and Game:GetProperty('JAC_PERSONA_LEADER_JAC_DATE_TANEMUNE_NODE_{NODE}')==1")));
    steps.push(ui("InGame",format!(r#"{HEADER}
{SERIAL}
{READER}
local q=assert(JAC_SKILL_AUDIT); local current=snapshot()
assert(current.turn==q.before.turn and current.city.id==q.before.city.id)
assert(current.merit==q.before.merit-q.cost and current.points==q.before.points,'skill price or native titles differ')
assert(Players[0]:GetProperty('JAC_PERSONA_PURCHASE_LEADER')==leader and ExposedMembers.JAC_LeaderPersona.Purchase==nil)
local expected=q.before.stable
expected.category=boxed('CONTACT');expected.branches.BRANCH_CONTACT_RELATIONS=boxed('ENVOYS');expected.skills['{NODE}']=boxed(1)
assert(serial(current.stable)==serial(expected),'unexpected skill or payment history change')
q.purchased=current;return serial(current)
"#)));
    let locks = original(
        template,
        "return 'category, exclusive sibling and prerequisites locked permanently'",
    )?;
    steps.push(locks.clone());
    for (node, result) in [
        (NODE, "OWNED"),
        ("LEADER_JAC_DATE_TANEMUNE_EXCHANGE_1", "BRANCH_LOCKED"),
    ] {
        steps.push(ui("InGame",format!(r#"local q=assert(JAC_SKILL_AUDIT)
q.rejectionSerial=(Players[0]:GetProperty('JAC_PERSONA_PURCHASE_SERIAL') or 0)+1
UI.RequestPlayerOperation(0,PlayerOperations.EXECUTE_SCRIPT,{{OnStart='JAC_LeaderPersonaPurchase',Leader='LEADER_JAC_DATE_TANEMUNE',Node='{node}',ConfirmCategory='CONTACT',ConfirmRoute='ENVOYS',RequestSerial=q.rejectionSerial}})
return 'one normal negative purchase request; no automatic retry'
"#)));
        steps.push(wait(format!("local q=assert(JAC_SKILL_AUDIT);local p=Players[0];return p:GetProperty('JAC_PERSONA_PURCHASE_SERIAL')==q.rejectionSerial and p:GetProperty('JAC_PERSONA_PURCHASE_RESULT')=='{result}' and p:GetProperty('JAC_PERSONA_PURCHASE_NODE')=='{node}'")));
        steps.push(ui("InGame",format!("{HEADER}\n{SERIAL}\n{READER}\nlocal q=assert(JAC_SKILL_AUDIT);assert(serial(snapshot())==serial(q.purchased),'rejected purchase changed actual state');return 'rejection kept complete state unchanged'")));
    }
    // Reuse the original GameEffects proof, selecting the unique actual settled
    // city from Gameplay's engine marker instead of the old fixture city key.
    let mut effects = original(
        template,
        "local wanted={['JAC_PERSONA_DATE_TANEMUNE_BASE']=1, ['JAC_PERSONA_TREE_LEADER_JAC_DATE_TANEMUNE_ENVOYS_1']=1}",
    )?;
    let old = "local city=CityManager.GetCity(0,Game:GetProperty('JAC_PERSONA_SCENARIO_CITY_A'));";
    let new="local city; local names={}; for _,c in Players[0]:GetCities():Members() do local n=Locale.Lookup(c:GetName()); assert(not names[n],'ambiguous city name'); names[n]=true; if c:GetProperty('JAC_PERSONA_SETTLED_LEADER_JAC_DATE_TANEMUNE')==1 then assert(not city,'multiple settled markers'); city=c end end;";
    let body = effects["code"]
        .as_str()
        .ok_or("original effect code missing")?;
    if !body.contains(old) {
        return Err("original effect city selector changed".into());
    }
    effects["code"] = body.replacen(old, new, 1).into();
    steps.push(effects);
    let capture = format!("{HEADER}\n{SERIAL}\n{READER}\nreturn serial(snapshot())");
    let mut before = ui("InGame", &capture);
    before["capture_as"] = "NORMAL_MERIT_SKILL_SAVED".into();
    steps.push(before);
    steps.extend([json!({"action":"save","name":SAVE,"exact_name":true,"checkpoint":true}),json!({"action":"restart_game"}),json!({"action":"load","name":SAVE,"exact_name":true,"checkpoint":true,"timeout":300}),wait("return Players[0] and Players[0]:IsTurnActive() and type(ExposedMembers.JAC_LeaderPersona)=='table'")]);
    let mut after = ui("InGame", capture);
    after["equals_capture"] = "NORMAL_MERIT_SKILL_SAVED".into();
    steps.push(after);
    steps.push(locks);
    steps.push(json!({"action":"exit_to_menu"}));
    Ok(
        json!({"name":"Normal earned merit: GameInfo cheapest legal original ENVOYS_1 UI purchase once, actual price/reward earning bound, owned/branch rejection and exact save reload; no merit setter or manual turn callback",
        "input_status":if input.is_some(){"parent_pinned_reassignment_save_and_observed_merit_required"}else{"placeholder_not_executable"},"expected_input_merit":merit,"maximum_real_earning_turns":MAX_EARNING_TURNS,"required_real_earning_turns":"computed at first snapshot from GameInfo Cost and GOVERN_TURN; no end-turn once affordable","judge_profile":"generic","strict_lua_errors":true,"steps":steps}),
    )
}
