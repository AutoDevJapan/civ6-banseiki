use std::{env, fs};

fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

const CONTEXT: &str = r#"
local leader='LEADER_JAC_ASHINA_MORIKIYO'; local owner=1
local prefix='JAC_PERSONA_'..leader..'_'; local turn=Game.GetCurrentGameTurn()
local function prop(f) return Game:GetProperty(prefix..f) end
local function serial(v)
 if type(v)~='table' then return type(v)..':'..tostring(v) end
 local keys={}; for k in pairs(v) do keys[#keys+1]=k end
 table.sort(keys,function(a,b) return tostring(a)<tostring(b) end)
 local parts={}; for _,k in ipairs(keys) do parts[#parts+1]=serial(k)..'='..serial(v[k]) end
 return '{'..table.concat(parts,';')..'}'
end
local function snapshot()
 return {merit=prop('MERIT'),last=prop('GOVERN_LAST_TURN'),aiLast=prop('AI_LAST_TURN'),category=prop('CATEGORY'),
 countTurn=prop('GOVERN_TURN_COUNT_TURN'),count=prop('GOVERN_TURN_COUNT'),deed=prop('DEED_GOVERN_TURN_'..turn),
 pending=Game:GetProperty('JAC_PERSONA_PENDING_DEEDS'),buckets=Game:GetProperty('JAC_PERSONA_MERIT_BUCKETS_V2'),
 sequence=Game:GetProperty('JAC_PERSONA_PENDING_SEQUENCE'),history=Game:GetProperty('JAC_PERSONA_PENDING_NATIVE_HISTORY')}
end
assert(turn==45 and prop('OWNER')==owner and prop('STATE')=='GOVERNOR','preserved turn45 joined Governor required')
local cfg=assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_CONFIG'))
assert(cfg.leader==leader and cfg.owner==owner,'wrong saved native persona')
local city=assert(CityManager.GetCity(owner,cfg.cityID))
assert(city:GetProperty('JAC_PERSONA_SETTLED_'..leader)==1,'qualified event needs synchronized settlement')
local api=assert(ExposedMembers.JAC_LeaderPersona)
local queue=assert(api.MeritQueue,'production gameplay queue context unavailable')
local function notify(freeze)
 local savedDrain=queue.Drain; local oldLast,oldAI=prop('GOVERN_LAST_TURN'),prop('AI_LAST_TURN')
 if freeze then queue.Drain=function() end end
 Game:SetProperty(prefix..'GOVERN_LAST_TURN',nil)
 Game:SetProperty(prefix..'AI_LAST_TURN',turn) -- suppress diagnostic AI purchases
 local ok,err=pcall(api.OnPlayerTurnStarted,owner)
 queue.Drain=savedDrain
 Game:SetProperty(prefix..'GOVERN_LAST_TURN',oldLast)
 Game:SetProperty(prefix..'AI_LAST_TURN',oldAI)
 assert(ok,err)
end
"#;

fn core(code: &str, extra: &str) -> String {
    format!(
        "{{\"action\":\"ui_lua\",\"state_name\":\"GameCore_Tuner\",\"code\":{}{extra}}}",
        quote(&format!("{CONTEXT}\n{code}"))
    )
}

fn main() {
    let out = env::args().nth(1).expect("scenario output path");
    let wait = r#"{"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":"return Players[0] and Players[0]:IsTurnActive() and type(ExposedMembers.JAC_LeaderPersona)=='table'"}}"#;
    let native = r#"local c=assert(Game:GetProperty('JAC_BOUNDARY_ENEMY_CONFIG')); local row=assert(GameInfo.JAC_LeaderPersonas[c.leader]); local gs=Players[c.owner]:GetGovernors(); assert(gs:HasGovernor(GameInfo.Governors[row.GovernorType].Hash)); local g=assert(gs:GetGovernor(GameInfo.Governors[row.GovernorType].Hash)); local city=assert(g:GetAssignedCity()); assert(g:IsEstablished() and g:GetNeutralizedTurns()==0 and city:GetOwner()==c.owner and city:GetID()==c.cityID); return 'actual native Governor settled, no fixture marker changes'"#;
    let check_native = format!(
        "{{\"action\":\"ui_lua\",\"state_name\":\"InGame\",\"code\":{}}}",
        quote(native)
    );
    let menu = r#"{"action":"exit_to_menu"}"#.to_string();
    let steps = vec![
        r#"{"action":"load","name":"JAC-Enemy-Neutralization-91f02912dd354a18821959c99aa89868-0000000000000008","exact_name":true,"timeout":300}"#.to_string(),
        wait.into(), check_native.clone(),
        core(r#"local before=snapshot(); assert(not next(before.pending or {}) and not before.deed,'baseline already processed or pending'); notify(true); local after=snapshot(); local e=assert(after.pending[leader..'_GOVERN_TURN_'..turn]); local n=0; for _ in pairs(after.pending) do n=n+1 end; local reward=GameInfo.JAC_PersonaMeritRules[GameInfo.JAC_PersonaTreePeople[leader].MeritProfile].GOVERN_TURN; assert(n==1 and e.version==2 and e.eligible==true and e.eligibilityUnknown==false and e.leader==leader and e.owner==owner and e.turn==turn and e.identity==tostring(turn) and e.cityID==city:GetID() and e.amount==reward and e.reservedCount==1 and e.sequence==(before.sequence or 0)+1,'qualified event identity differs'); assert(after.merit==before.merit and after.count==before.count and after.countTurn==before.countTurn and after.deed==before.deed and after.history==before.history and after.last==before.last and after.aiLast==before.aiLast and after.category==before.category,'capture changed committed merit/cap/AI state'); assert(after.buckets[leader..'_GOVERN_TURN_'..turn]==1); local seq=after.sequence; notify(true); assert(serial(snapshot())==serial(after),'queued duplicate changed reservation/order'); return serial(after)"#, ",\"capture_as\":\"TODO181_QUALIFIED_PENDING\""),
        core(r#"local expected=snapshot(); local e=expected.pending[leader..'_GOVERN_TURN_'..turn]; expected.merit=(expected.merit or 0)+e.amount; expected.countTurn=turn; expected.count=1; expected.deed=turn; expected.pending=nil; expected.buckets=nil; return serial(expected)"#, ",\"capture_as\":\"TODO181_SETTLED\""),
        r#"{"action":"save","name":"JAC-Todo181-Qualified","exact_name":true,"checkpoint":true}"#.into(),menu.clone(),
        r#"{"action":"load","name":"JAC-Todo181-Qualified","exact_name":true,"checkpoint":true,"timeout":300}"#.into(), wait.into(),
        core("return serial(snapshot())", ",\"equals_capture\":\"TODO181_SETTLED\""),
        core("local before=serial(snapshot()); notify(false); assert(serial(snapshot())==before,'same-turn repeat paid twice'); return serial(snapshot())", ",\"equals_capture\":\"TODO181_SETTLED\""),
        r#"{"action":"save","name":"JAC-Todo181-Settled","exact_name":true,"checkpoint":true}"#.into(),menu.clone(),
        r#"{"action":"load","name":"JAC-Todo181-Settled","exact_name":true,"checkpoint":true,"timeout":300}"#.into(),wait.into(),
        core("notify(false); return serial(snapshot())", ",\"equals_capture\":\"TODO181_SETTLED\""),check_native,menu,
    ];
    fs::write(out, format!("{{\"name\":\"ToDo181 qualified pending: event-time capture, save/load settlement once, second load and duplicate\",\"judge_profile\":\"generic\",\"strict_lua_errors\":true,\"steps\":[{}]}}\n", steps.join(",\n"))).unwrap();
}
