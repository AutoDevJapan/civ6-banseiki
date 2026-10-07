//! Host once, then consume PR121-format Lua.log frames, never post-Host Tuner.
use crate::{mp_staging as cap, sp_runtime::SERIAL};
use serde_json::{json, Value};
use std::path::Path;
const WITNESS: &str = include_str!("../../../poc/JAC_Test_HotseatBridge/UI/JAC_HotseatWitness.lua");
pub fn nonce_valid(n: &str) -> bool {
    n.len() == 32 && n.bytes().all(|c| c.is_ascii_hexdigit())
}
pub const HOST:&str="if not UI.IsInFrontEnd() or not GameConfiguration.IsHotseat() then error('hotseat main menu required') end\nNetwork.HostGame(ServerType.SERVER_TYPE_HOTSEAT)\nreturn true";
const READY_BODY: &str = "staging22 host1 launch0 ready";
pub fn ready_marker(n: &str) -> String {
    let hex = format!("{READY_BODY} {n}")
        .bytes()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    // A complete single-frame body includes this run's nonce. The observation
    // serial may vary as the official UI updates; no fixed serial is invented.
    format!("|STAGING_READY|1/1|{hex}")
}
fn original_emitter() -> &'static str {
    let start = WITNESS
        .find("local function emit(kind, step, body)")
        .expect("PR121 emitter start");
    let end = WITNESS[start..]
        .find("local function dialog()")
        .expect("PR121 emitter end")
        + start;
    &WITNESS[start..end]
}
pub fn observer(normal: &Value, hotseat: &Value, n: &str) -> Result<String, String> {
    if !nonce_valid(n) {
        return Err("boundary nonce must be32 hex digits".into());
    }
    let payment = crate::mp_hotseat::build(normal, hotseat)?;
    let config = &payment["steps"][2];
    let map = config["map_file"].as_str().ok_or("fixed map missing")?;
    let roster = config["players"].as_array().ok_or("roster missing")?;
    if roster.len() != 22 || !cap::MAPS.contains(&map) {
        return Err("original fixedJapan22 roster required".into());
    }
    let expected = roster
        .iter()
        .map(|p| {
            format!(
                "{{leader='{}',civ='{}'}}",
                p["leader"].as_str().unwrap(),
                p["civ"].as_str().unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!(r#"
do
local nonce='{n}';local scene='staging.'..nonce;local serial=0
{emitter}
{serializer}
local encode=serial
-- Reuse the existing frame emitter's counter separately from table serialization.
local expected={{{expected}}};local last;local passed=false
local function inspect()
 local s=JAC_Staging22Status();s.roster={{}};s.occupied=0;s.observedHumans=0;s.observedAI=0;s.rosterMatches=true
 for id=0,63 do local p=PlayerConfigurations[id];if p then
  local slot=p:GetSlotStatus();local level=p:GetCivilizationLevelTypeID()
  if level==CivilizationLevelTypes.CIVILIZATION_LEVEL_FULL_CIV and (slot==SlotStatus.SS_TAKEN or slot==SlotStatus.SS_COMPUTER) then
   s.occupied=s.occupied+1;local e=expected[id+1]
   local leader,civ=p:GetLeaderTypeName(),p:GetCivilizationTypeName()
   if not e or leader~=e.leader or civ~=e.civ or slot~=(id<2 and SlotStatus.SS_TAKEN or SlotStatus.SS_COMPUTER) then s.rosterMatches=false end
   if slot==SlotStatus.SS_TAKEN then s.observedHumans=s.observedHumans+1 else s.observedAI=s.observedAI+1 end
   s.roster[#s.roster+1]={{id=id,slot=slot,leader=leader,civ=civ}}
  end
 end end
 s.map=MapConfiguration.GetValue('MAP_SCRIPT');s.ruleset=GameConfiguration.GetRuleSet()
 s.hotseat=GameConfiguration.IsHotseat();s.host=Network.IsGameHost()
 local ready=s.version=='staging22/v1' and s.stock=='{stock}' and s.visible==true
  and s.hotseat==true and s.host==true and s.ruleset=='RULESET_JAC_BANSEIKI' and s.map=='{map}'
  and s.scopeCap==22 and s.effectiveMax==22 and s.effectiveMin==22
  and s.occupied==22 and s.observedHumans==2 and s.observedAI==20 and s.rosterMatches
  and s.humans==2 and s.ai==20 and s.badSlot==false and s.duplicate==false and s.teamsValid==true and s.readyDisabled==false
 return encode(s),ready
end
local function observe()
 if GameConfiguration.GetValue('JAC_HOTSEAT_BRIDGE_NONCE')~=nonce or ContextPtr:IsHidden() then return end
 local ok,body,ready=pcall(inspect)
 if not ok then if last~=tostring(body) then emit('STAGING_ERROR',0,tostring(body));last=tostring(body)end;return end
 if body~=last then emit('STAGING_FACTS',0,body);last=body end
 if ready and not passed then
  passed=true
  emit('STAGING_READY',0,'{ready_body} '..nonce)
 end
end
local original=UpdateReadyButton
function UpdateReadyButton()
 original()
 observe()
end
end
"#,emitter=original_emitter(),serializer=SERIAL.replace("local function serial(v)","local function encodeValue(v)").replace("serial(k)","encodeValue(k)").replace("serial(v[k])","encodeValue(v[k])"),stock=cap::STOCK_SHA,ready_body=READY_BODY).replace("local encode=serial","local encode=encodeValue"))
}
pub fn build(normal: &Value, hotseat: &Value, n: &str) -> Result<Value, String> {
    observer(normal, hotseat, n)?;
    let payment = crate::mp_hotseat::build(normal, hotseat)?;
    let mut enable = payment["steps"][0].clone();
    enable["ids"]
        .as_array_mut()
        .ok_or("modIDsmissing")?
        .push(json!(cap::MOD_ID));
    Ok(
        json!({"name":"Staging22 boundary via PR121 log framing: Host once, no postHost Tuner/Ready/Launch/native/payment; official UI gate only","session_type":"hotseat","hotseat_bridge_nonce":n,"judge_profile":"generic","strict_lua_errors":true,
      "steps":[enable,{"action":"restart_game"},payment["steps"][2],
       {"action":"ui_lua","state_name":"FrontEnd","code":HOST,"timeout":20},
       {"action":"wait_for","timeout":90,"condition":{"type":"log_line","log":"Lua.log","contains":ready_marker(n)}}]}),
    )
}
pub fn prepare(
    stock: &[u8],
    normal: &Value,
    hotseat: &Value,
    n: &str,
    out: &Path,
) -> Result<(), String> {
    let original = cap::transform(stock)?;
    let code = observer(normal, hotseat, n)?;
    let scene = build(normal, hotseat, n)?;
    if original.matches("\nInitialize();").count() != 1 {
        return Err("Initialize anchor changed".into());
    }
    let patched = original.replacen("\nInitialize();", &format!("{code}\nInitialize();"), 1);
    if out.exists() {
        return Err("output must not exist".into());
    }
    cap::prepare(stock, out)?;
    std::fs::write(
        out.join(cap::MOD_FOLDER).join("UI/StagingRoom.lua"),
        &patched,
    )
    .map_err(|e| e.to_string())?;
    let scene_bytes = serde_json::to_vec_pretty(&scene).unwrap();
    std::fs::write(out.join("boundary.json"), &scene_bytes).map_err(|e| e.to_string())?;
    let proof = json!({"nonce":n,"stock_sha256":cap::STOCK_SHA,"generated_sha256":cap::sha(patched.as_bytes()),"manifest_sha256":cap::sha(cap::manifest().as_bytes()),"scene_sha256":cap::sha(&scene_bytes),"mod_id":cap::MOD_ID,"host":1,"launch":0,"post_host_tuner":0,"transport":"PR121 JAC_HS frame emitter; original21 registration unchanged","deployment":"parent owns game/lock; install generatedMOD before restart; verify SHA and competingStagingRoom override absence; no native/sharedstock overwrite","unknown_host_response":"stop without resend, retain logs; not a success","native_capacity":"not proven"});
    std::fs::write(
        out.join("bundle.json"),
        serde_json::to_vec_pretty(&proof).unwrap(),
    )
    .map_err(|e| e.to_string())
}
