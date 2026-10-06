//! Finite acceptance using actual pre-oath saves; never manufacture a legacy ledger.
use anyhow::{bail, Context, Result};
use civ6_data_rs::Repository;
use serde_json::{json, Value};
use std::{fs, path::PathBuf};

const OLD_NAME: &str = "JAC-SengokuAges-02-ages-owner-l-05-543484304b0c";
const PRE_NAME: &str = "JAC-SengokuAges-01-ages-owner-l-05-543484304b0c";
const RESYNC_NAME: &str = "JAC-Todo134-Control-Resync";
const CATALOGUE: &str = "local count=0; for _ in GameInfo.JAC_SengokuOaths() do count=count+1 end; if count~=66 then error('live oath catalogue must contain 66 rows, got '..count) end";
const LEGACY: &str = r#"
local l=Game:GetProperty('JAC_AGES_LEDGER')
if type(l)~='table' or l.reign~=1 then error('actual legacy reign1 required') end
local turn=Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1
if turn~=67 or l.history.r1.turn~=66 then error('legacy save provenance/turn mismatch') end
local n=0
for k,h in pairs(l.houses) do
 n=n+1
 for _,field in ipairs({'oaths','oathReign','oathChosen','oathDue','oathKnowledge'}) do
  if h[field]~=nil then error('save already has oath field '..k..'.'..field) end
 end
 if (h.parts.oath or 0)~=0 then error('save has oath points') end
end
if n~=22 or not Players[0]:IsAlive() or not Players[0]:IsHuman() then error('legacy 22 houses/human missing') end
return 'LEGACY_ACCEPTED turn=67 reign=1 houses=22 oathFields=absent'
"#;

const SNAPSHOT: &str = r#"
local l=Game:GetProperty('JAC_AGES_LEDGER')
local function ser(v)
 if type(v)~='table' then return type(v)..':'..tostring(v) end
 local keys={} for k in pairs(v) do keys[#keys+1]=k end table.sort(keys)
 local out={} for _,k in ipairs(keys) do out[#out+1]=tostring(k)..'='..ser(v[k]) end
 return '{'..table.concat(out,',')..'}'
end
Game:SetProperty('JAC_TODO134_OLD_HISTORY',ser(l.history))
Game:SetProperty('JAC_TODO134_OLD_TURN',Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1)
return 'legacy history snapshot captured without modifying the ledger'
"#;

macro_rules! migration_lua {
    ($tail:literal) => { concat!(r#"
local function validate_migration()
local l=Game:GetProperty('JAC_AGES_LEDGER')
local turn=Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1
if turn~=68 or l.reign~=1 then error('one natural turn must migrate the same reign') end
local humans,ai=0,0
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
 local id=p:GetID() local h=l.houses['p'..id]
 if h.oathReign~=1 or type(h.oaths)~='table' or type(h.oathKnowledge)~='table' then error('missing migration for '..id) end
 local tech,civic=0,0
 for row in GameInfo.Technologies() do if p:GetTechs():HasTech(row.Index) then tech=tech+1 end end
 for row in GameInfo.Civics() do if p:GetCulture():HasCivic(row.Index) then civic=civic+1 end end
 if h.oathKnowledge.tech~=tech or h.oathKnowledge.civic~=civic then error('migrated research baseline mismatch '..id) end
 if (h.parts.oath or 0)~=0 then error('retroactive oath reward '..id) end
 local selected=0
 for oath,on in pairs(h.oaths) do
  if on then
   local row=GameInfo.JAC_SengokuOaths[oath]
   if not row or row.CivilizationType~=h.civ then error('foreign migrated oath '..id) end
   selected=selected+1
  end
 end
 if p:IsHuman() then
  humans=humans+1
  if h.oathChosen~=nil or selected~=0 or h.oathDue~=69 then error('human must get a new choice window') end
 else
  ai=ai+1
  if h.oathChosen~=1 or selected~=(h.age=='HEROIC' and 3 or 1) then error('AI migrated choice count '..id) end
 end
end
if humans~=1 or ai~=21 then error('missing human/AI migration coverage') end
return 'MIGRATED turn=68 human='..humans..' AI='..ai..' retroactiveOathPoints=0'
end
"#, "\n", $tail) };
}
const MIGRATED: &str = migration_lua!("return validate_migration()");

const CHOOSE: &str = r#"
if not JAC_ShowOaths(true) then error('migrated oath popup unavailable') end
local rows=JAC_GetOfferedOaths()
if #rows~=3 then error('migrated human must have exactly three own choices') end
local h=Game:GetProperty('JAC_AGES_LEDGER').houses.p0
if h.age=='HEROIC' then
 for _,row in ipairs(rows) do if not JAC_SelectOath(row.type) then error('heroic selection failed') end end
 if not JAC_ConfirmOaths() then error('heroic confirmation failed') end
elseif not JAC_ChooseOath(rows[1].type) then error('migrated confirmation failed') end
return 'migrated human chose through the existing popup and player operation'
"#;

const OATH_SNAPSHOT: &str = r#"
local l=Game:GetProperty('JAC_AGES_LEDGER') local h=l.houses.p0
if h.oathChosen~=l.reign then error('choice not received by gameplay') end
local function ser(v)
 if type(v)~='table' then return type(v)..':'..tostring(v) end
 local keys={} for k in pairs(v) do keys[#keys+1]=k end table.sort(keys)
 local out={} for _,k in ipairs(keys) do out[#out+1]=tostring(k)..'='..ser(v[k]) end
 return '{'..table.concat(out,',')..'}'
end
if ser(l.history)~=Game:GetProperty('JAC_TODO134_OLD_HISTORY') then error('migration changed legacy history') end
local out={}
for key,owner in pairs(l.houses) do
 out[key]={civ=owner.civ,age=owner.age,oaths=owner.oaths,oathReign=owner.oathReign,oathChosen=owner.oathChosen,oathDue=owner.oathDue,oathKnowledge=owner.oathKnowledge}
end
local cities=0
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
 local owner=l.houses['p'..p:GetID()]
 for _,city in p:GetCities():Members() do
  cities=cities+1
  local plot=Map.GetPlot(city:GetX(),city:GetY())
  for row in GameInfo.JAC_SengokuOaths() do
   local expected=(owner.civ==row.CivilizationType and (owner.age=='GOLDEN' or owner.age=='HEROIC') and owner.oaths[row.OathType]) and 1 or 0
   if plot:GetProperty(row.OathType..'_ON')~=expected then error('migration city flag mismatch '..row.OathType) end
  end
 end
end
if cities==0 then error('no migrated city flags checked') end
local now=ser(out)
local saved=Game:GetProperty('JAC_TODO134_MIGRATED_SNAPSHOT')
if verifySaved then
 if type(saved)~='string' then error('migration snapshot missing after load') end
 if saved~=now then error('migration state changed across save/load') end
else
 Game:SetProperty('JAC_TODO134_MIGRATED_SNAPSHOT',now)
end
if Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1~=68 then error('load advanced the turn') end
return 'MIGRATION_PERSISTED same history/oath state and all 66 flags in '..cities..' cities'
"#;

const CAPABILITIES: &str = r#"
local l=Game:GetProperty('JAC_AGES_LEDGER') local out={}
local p=Players[0] local dip=p:GetDiplomacy()
for _,name in ipairs({'HasDeclaredFriendship','SetHasDeclaredFriendship','SetHasMet','HasAllied'}) do out[#out+1]=name..'='..type(dip[name]) end
local city=p:GetCities():GetCapitalCity()
if not city then error('no capital') end
local q=city:GetBuildQueue() local g=city:GetGrowth()
for _,name in ipairs({'GetUnitProduction','GetBuildingProduction','GetProductionYield','GetUnitProgress','GetBuildingProgress','GetTurnsLeft'}) do out[#out+1]='queue.'..name..'='..type(q[name]) end
out[#out+1]='growth.GetOverallGrowthModifier='..type(g.GetOverallGrowthModifier)
out[#out+1]='reign='..tostring(l.reign)
return 'TODO134_CAPABILITIES '..table.concat(out,' ')
"#;

const TENBUN_BEFORE: &str = r#"
local l=Game:GetProperty('JAC_AGES_LEDGER')
if not l or l.reign~=0 then error('actual pre-reign legacy save required') end
local turn=Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1
if turn~=34 then error('unexpected legacy Tenbun turn '..turn) end
for _,h in pairs(l.houses) do
 for _,field in ipairs({'oaths','oathReign','oathChosen','oathDue','oathKnowledge'}) do
  if h[field]~=nil then error('legacy Tenbun already has oath field '..field) end
 end
end
return 'legacy Tenbun turn34 has no oath metadata'
"#;

const TENBUN_AFTER: &str = r#"
local l=Game:GetProperty('JAC_AGES_LEDGER')
local turn=Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1
if turn~=35 or l.reign~=0 then error('pre-reign migration advanced unexpectedly') end
local cities=0
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
 local h=l.houses['p'..p:GetID()]
 for _,field in ipairs({'oaths','oathReign','oathChosen','oathDue'}) do
  if h[field]~=nil then error('pre-reign oath metadata '..field) end
 end
 if (h.parts.oath or 0)~=0 then error('pre-reign oath reward') end
 -- Existing research intake records the current baseline even before the first reign.
 local tech,civic=0,0
 for row in GameInfo.Technologies() do if p:GetTechs():HasTech(row.Index) then tech=tech+1 end end
 for row in GameInfo.Civics() do if p:GetCulture():HasCivic(row.Index) then civic=civic+1 end end
 if type(h.oathKnowledge)~='table' or h.oathKnowledge.tech~=tech or h.oathKnowledge.civic~=civic then error('pre-reign knowledge baseline mismatch') end
 for _,city in p:GetCities():Members() do
  cities=cities+1
  local plot=Map.GetPlot(city:GetX(),city:GetY())
  for row in GameInfo.JAC_SengokuOaths() do
   if plot:GetProperty(row.OathType..'_ON')~=0 then error('pre-reign strong effect') end
  end
 end
end
if cities==0 then error('no cities checked') end
return 'LEGACY_TENBUN_PASSED turn35 flags=0 cities='..cities
"#;

fn lua(state: &str, code: &str) -> Value {
    json!({"action":"ui_lua","state_name":state,"code":code,"timeout":60})
}

fn catalogue_lua(code: &str) -> Value {
    lua("GameCore_Tuner", &format!("{CATALOGUE}\n{code}"))
}

fn oath_snapshot(verify_saved: bool) -> Value {
    catalogue_lua(&format!(
        "local verifySaved={verify_saved}\n{OATH_SNAPSHOT}"
    ))
}

// Every optional diagnostic API uses the same lookup and failure reporting.
macro_rules! diagnostic_lua {
    ($body:literal) => {
        concat!(
            r#"
local function method(object,name)
 if object==nil then return nil,'owner missing' end
 local ok,fn=pcall(function() return object[name] end)
 if not ok then return nil,name..' lookup failed:'..tostring(fn) end
 if type(fn)~='function' then return nil,name..' missing' end
 return fn
end
"#,
            $body
        )
    };
}

const TURN_DIAGNOSTIC: &str = diagnostic_lua!(
    r#"
local out={'raw='..Game.GetCurrentGameTurn(),'elapsed='..(Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1)}
local function probe(object,label,name,...)
 local fn,missing=method(object,name)
 if not fn then out[#out+1]=label..'.'..name..'=unavailable:'..missing; return end
 local success,value=pcall(fn,...)
 out[#out+1]=label..'.'..name..'='..(success and tostring(value) or ('error:'..tostring(value)))
end
local localPlayer
local getLocal,localError=method(Game,'GetLocalPlayer')
if getLocal then local ok,p=pcall(getLocal); if ok then localPlayer=p end; out[#out+1]='local='..(ok and tostring(p) or ('unavailable:call failed:'..tostring(p)))
else out[#out+1]='local=unavailable:'..localError end
-- Keep queue/ready state before the roster: engine print lines can be truncated.
for _,name in ipairs({'CanEndTurn','HasSentTurnComplete','IsProcessingMessages'}) do probe(UI,'UI',name) end
if type(localPlayer)=='number' and localPlayer>=0 then probe(UI,'UI','IsTurnTimerElapsed',localPlayer)
else out[#out+1]='UI.IsTurnTimerElapsed=unavailable:local player unavailable' end
for _,name in ipairs({'IsPaused','GetPausePlayer'}) do probe(GameConfiguration,'GameConfiguration',name) end
probe(UIManager,'UIManager','IsPopupQueueDisabled',UIManager)
probe(AutoplayManager,'AutoplayManager','IsActive')
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
 local label='p'..p:GetID()
 for _,name in ipairs({'IsAlive','IsHuman','IsTurnActive','IsTurnActiveComplete','CanUnreadyTurn','IsAITurnActive','GetTeam'}) do probe(p,label,name,p) end
end
local getBlocking,blockingError=method(NotificationManager,'GetAllEndTurnBlocking')
if getBlocking and type(localPlayer)=='number' and localPlayer>=0 then
 local ok,v=pcall(getBlocking,localPlayer)
 if ok and type(v)=='table' then for _,b in ipairs(v) do out[#out+1]='block='..tostring(b) end end
 if not ok then out[#out+1]='blocks=unavailable:call failed:'..tostring(v)
 elseif type(v)~='table' then out[#out+1]='blocks=unavailable:returned '..type(v) end
else
 out[#out+1]='blocks=unavailable:'..(blockingError or 'local player unavailable')
end
local l=Game:GetProperty('JAC_AGES_LEDGER'); local h=l and l.houses.p0
if h then for _,k in ipairs({'age','oathReign','oathChosen','oathDue'}) do out[#out+1]='human.'..k..'='..tostring(h[k]) end end
return 'LEGACY_CONTROL_DIAGNOSTIC '..table.concat(out,' ')
"#
);

// Read the engine's session and commemoration state without acknowledging it.
// FindOpenSessionID/GetSessionInfo: official DiplomacyActionView.lua:2052-2054.
const SESSION_DIAGNOSTIC: &str = diagnostic_lua!(
    r#"
local out={}
if ContextPtr and type(ContextPtr.IsHidden)=='function' then out[#out+1]='contextHidden='..tostring(ContextPtr:IsHidden()) end
local find,findError=method(DiplomacyManager,'FindOpenSessionID')
if not find then
 out[#out+1]='sessionLookup=unavailable:'..findError
else
 for _,p in ipairs(PlayerManager.GetAliveMajors()) do
  local id=p:GetID()
  if id~=0 then
   local ok,session=pcall(find,0,id)
   out[#out+1]='session.p'..id..'='..(ok and tostring(session) or 'unavailable')
   local label='sessionInfo.'..id
   if not ok then
    out[#out+1]=label..'=unavailable:FindOpenSessionID failed:'..tostring(session)
   elseif type(session)~='number' or session<0 then
    out[#out+1]=label..'=not queried:session type='..type(session)..' value='..tostring(session)
   else
    local get,getError=method(DiplomacyManager,'GetSessionInfo')
    local success,info
    if get then success,info=pcall(get,session) end
    if not get then
     out[#out+1]=label..'=unavailable:'..getError
    elseif not success then
     out[#out+1]=label..'=unavailable:GetSessionInfo failed:'..tostring(info)
    elseif type(info)~='table' then
     out[#out+1]=label..'=unavailable:GetSessionInfo returned '..type(info)
    else
     local keys={}; for key in pairs(info) do keys[#keys+1]=tostring(key) end; table.sort(keys)
     out[#out+1]=label..'=table fields='..#keys
     for _,key in ipairs(keys) do out[#out+1]=label..'.'..key..'='..tostring(info[key]) end
    end
   end
  end
 end
end
local getEras,erasError=method(Game,'GetEras')
if not getEras then
 out[#out+1]='commemorations=unavailable:'..erasError
else
 local ok,eras=pcall(getEras)
 if not ok or eras==nil then
  out[#out+1]='commemorations=unavailable:GetEras '..(ok and 'returned nil' or ('failed:'..tostring(eras)))
 else
  for _,name in ipairs({'GetPlayerNumAllowedCommemorations','GetPlayerCommemorateChoices'}) do
   local fn,missing=method(eras,name)
   if fn then
    local success,value=pcall(fn,eras,0)
    out[#out+1]=name..'='..(success and (type(value)=='table' and tostring(#value) or tostring(value)) or ('unavailable:call failed:'..tostring(value)))
   else
    out[#out+1]=name..'=unavailable:'..missing
   end
  end
 end
end
return 'LEGACY_SESSION_DIAGNOSTIC '..table.concat(out,' ')
"#
);

const CLOSE_VISIBLE_STATEMENT: &str = "if ContextPtr:IsHidden() then return 'statement already hidden; no close sent' end\nOnClose(); return 'visible statement close requested once; no end-turn resend'";

fn turn_event_observer() -> String {
    format!(
        r#"
local function snapshot()
{TURN_DIAGNOSTIC}
end
local last=''
for _,name in ipairs({{'TurnBegin','TurnEnd','PlayerTurnActivated','PlayerTurnDeactivated','LocalPlayerTurnBegin','LocalPlayerTurnEnd','GameCoreEventPublishComplete','GameCoreEventPlaybackComplete'}}) do
 if Events[name] and Events[name].Add then
  local event=name
  Events[name].Add(function(...)
   local args={{}}; for i,v in ipairs({{...}}) do args[i]=tostring(v) end
   local state=event..':'..table.concat(args,',')..' '..snapshot()
   if event=='GameCoreEventPlaybackComplete' or state~=last then print('LEGACY_CONTROL_EVENT '..state); last=state end
  end)
 end
end
return 'diagnostic event observers registered; no acceptance'
"#
    )
}

// A real save/load may rebuild a UI whose playback did not catch up. Record
// the whole migrated ledger and every city flag before asking the native save
// API; never write the migration or replay its turn in order to make it pass.
const RESYNC_SNAPSHOT: &str = migration_lua!(
    r#"
local migrated=validate_migration()
local l=assert(Game:GetProperty('JAC_AGES_LEDGER'),'resync ledger missing')
assert(Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1==68,'resync requires the single naturally advanced turn')
assert(Players[0]:IsTurnActive() and not Players[0]:IsTurnActiveComplete(),'human not active before/after resync')
local function ser(v)
 if type(v)~='table' then local s=type(v)..':'..tostring(v); return #s..':'..s end
 local entries={}
 for k,value in pairs(v) do entries[#entries+1]=ser(k)..ser(value) end
 table.sort(entries)
 return '{'..table.concat(entries)..'}'
end
local function oldser(v)
 if type(v)~='table' then return type(v)..':'..tostring(v) end
 local keys={} for k in pairs(v) do keys[#keys+1]=k end table.sort(keys)
 local out={} for _,k in ipairs(keys) do out[#out+1]=tostring(k)..'='..oldser(v[k]) end
 return '{'..table.concat(out,',')..'}'
end
assert(oldser(l.history)==Game:GetProperty('JAC_TODO134_OLD_HISTORY'),'resync legacy history changed')
local cities={}; local majors=0; local owners={}
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
 assert(not owners[p:GetID()],'duplicate resync house'); owners[p:GetID()]=true
 majors=majors+1
 local house=assert(l.houses['p'..p:GetID()],'resync house missing')
 for _,city in p:GetCities():Members() do
  local plot=assert(Map.GetPlot(city:GetX(),city:GetY()))
  assert(city:GetOwner()==p:GetID() and plot:GetOwner()==p:GetID(),'resync city owner mismatch')
  local flags={}
  for row in GameInfo.JAC_SengokuOaths() do
   local expected=(house.civ==row.CivilizationType and (house.age=='GOLDEN' or house.age=='HEROIC') and house.oaths[row.OathType]) and 1 or 0
   local value=plot:GetProperty(row.OathType..'_ON')
   assert(value==expected,'resync city flag missing or incorrect '..row.OathType)
   flags[row.OathType]=value
  end
  cities[#cities+1]={owner=p:GetID(),id=city:GetID(),plot=plot:GetIndex(),flags=flags}
 end
end
assert(majors==22 and #cities>0,'resync house/city coverage missing')
table.sort(cities,function(a,b)return a.plot<b.plot end)
local now=ser({ledger=l,cities=cities})
local saved=Game:GetProperty('JAC_TODO134_RESYNC_SNAPSHOT')
if __VERIFY__ then
 assert(type(saved)=='string' and saved==now,'resync changed migrated ledger or city flags')
else
 assert(saved==nil,'resync snapshot already exists')
 Game:SetProperty('JAC_TODO134_RESYNC_SNAPSHOT',now)
end
return 'RESYNC_STATE_UNCHANGED turn68 houses='..majors..' cities='..#cities..' verify='..tostring(__VERIFY__)..' '..migrated
"#
);

const RESYNC_SESSIONS_CLOSED: &str = r#"
assert(ContextPtr:IsHidden(),'diplomacy view still visible before resync save')
assert(type(DiplomacyManager.FindOpenSessionID)=='function','session API missing')
for _,p in ipairs(PlayerManager.GetAliveMajors()) do
 local id=p:GetID()
 if id~=0 then
  local session=DiplomacyManager.FindOpenSessionID(0,id)
  assert(session==nil,'diplomacy session remains or result unavailable')
 end
end
return 'RESYNC_DIPLOMACY_CLOSED all real peers checked'
"#;

fn resync_snapshot(verify: bool) -> Value {
    catalogue_lua(&RESYNC_SNAPSHOT.replace("__VERIFY__", &verify.to_string()))
}

fn scenarios(repo: &Repository) -> Result<Vec<(&'static str, Value)>> {
    let base = repo.json("tools/civ6-autotest/scenarios/sengoku-oaths-oda.json")?;
    let enabled = base["steps"][0].clone();
    let load = |name| json!({"action":"load","name":name,"exact_name":true,"timeout":600});
    let active = json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"InGame","code":"return Players[0]:IsTurnActive()"}});
    let probe = json!({"name":"誓約導入前の実保存：現在の台帳とAPIを読み取る","judge_profile":"generic","steps":[enabled.clone(),load(OLD_NAME),lua("GameCore_Tuner",LEGACY),catalogue_lua(CAPABILITIES)]});
    let migration = json!({"name":"誓約導入前の実保存：自然な次手番で人間とAIを移行し、旧履歴と保存復元を照合する","judge_profile":"generic","steps":[enabled.clone(),load(OLD_NAME),lua("GameCore_Tuner",LEGACY),lua("GameCore_Tuner",SNAPSHOT),active.clone(),json!({"action":"end_turn","timeout":900}),json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"GameCore_Tuner","code":"local l=Game:GetProperty('JAC_AGES_LEDGER'); return l and l.houses.p0.oathReign==1 and Game.GetCurrentGameTurn()-GameConfiguration.GetStartTurn()+1==68"}}),lua("GameCore_Tuner",MIGRATED),active.clone(),lua("DedicationPopup",CHOOSE),json!({"action":"wait_for","timeout":60,"condition":{"type":"lua","state_name":"GameCore_Tuner","code":"local l=Game:GetProperty('JAC_AGES_LEDGER'); return l and l.houses.p0.oathChosen==l.reign"}}),oath_snapshot(false),json!({"action":"save","name":"JAC-Todo134-Migrated","timeout":180}),json!({"action":"exit_to_menu","timeout":180}),json!({"action":"load","name":"JAC-Todo134-Migrated","timeout":600}),oath_snapshot(true)]});
    let pre = json!({"name":"誓約導入前の天文保存：改元前には候補と効果を持たない","judge_profile":"generic","steps":[enabled.clone(),load(PRE_NAME),lua("GameCore_Tuner",TENBUN_BEFORE),active.clone(),json!({"action":"end_turn","timeout":900}),catalogue_lua(TENBUN_AFTER)]});
    // A bounded observation always reaches the post-request snapshots, even
    // when the engine rejects the request or human control never returns.
    // This scene diagnoses a stall; its completion is never acceptance.
    let diagnostic = json!({"name":"旧保存：自然な1手番後の人間とAIの操作状態を読み取る（受入合格に数えない）","judge_profile":"generic","diagnostic_only":true,"steps":[enabled,load(OLD_NAME),lua("GameCore_Tuner",LEGACY),lua("GameCore_Tuner",&turn_event_observer()),lua("InGame",&turn_event_observer()),lua("GameCore_Tuner",TURN_DIAGNOSTIC),lua("InGame",TURN_DIAGNOSTIC),active,lua("InGame","UI.RequestAction(ActionTypes.ACTION_ENDTURN); return 'single native human turn requested for read-only diagnosis'"),json!({"action":"observe_for","seconds":120,"timeout":135}),lua("GameCore_Tuner",TURN_DIAGNOSTIC),lua("InGame",TURN_DIAGNOSTIC),json!({"action":"pause","seconds":10}),lua("GameCore_Tuner",TURN_DIAGNOSTIC),lua("InGame",TURN_DIAGNOSTIC)]});
    let mut recovery = diagnostic.clone();
    recovery["name"] =
        "旧保存：外交画面を1回閉じた後の手番とセッションを観測する（受入合格に数えない）".into();
    let steps = recovery["steps"].as_array_mut().unwrap();
    steps.extend([
        lua("InGame", SESSION_DIAGNOSTIC),
        lua("DiplomacyActionView", SESSION_DIAGNOSTIC),
        lua("DiplomacyActionView", CLOSE_VISIBLE_STATEMENT),
        json!({"action":"observe_for","seconds":120,"timeout":135}),
        lua("GameCore_Tuner", TURN_DIAGNOSTIC),
        lua("InGame", TURN_DIAGNOSTIC),
        lua("InGame", SESSION_DIAGNOSTIC),
        lua("DiplomacyActionView", SESSION_DIAGNOSTIC),
    ]);
    let mut resync = recovery.clone();
    resync["name"] =
        "旧保存：自然移行後の別名保存と正式読込で画面の再同期を診断する（受入合格に数えない）"
            .into();
    let steps = resync["steps"].as_array_mut().unwrap();
    steps.insert(3, lua("GameCore_Tuner", SNAPSHOT));
    steps.extend([
        lua("DiplomacyActionView", RESYNC_SESSIONS_CLOSED),
        lua("GameCore_Tuner", MIGRATED),
        resync_snapshot(false),
        json!({"action":"save","name":RESYNC_NAME,"timeout":180}),
        json!({"action":"exit_to_menu","timeout":180}),
        json!({"action":"load","name":RESYNC_NAME,"timeout":600}),
        lua("GameCore_Tuner", &turn_event_observer()),
        lua("InGame", &turn_event_observer()),
        json!({"action":"observe_for","seconds":30,"timeout":45}),
        resync_snapshot(true),
        lua(
            "InGame",
            &format!(
                "{CATALOGUE}\n{}",
                RESYNC_SNAPSHOT.replace("__VERIFY__", "true")
            ),
        ),
        lua("GameCore_Tuner", TURN_DIAGNOSTIC),
        lua("InGame", TURN_DIAGNOSTIC),
        lua("InGame", SESSION_DIAGNOSTIC),
    ]);
    Ok(vec![
        ("legacy-capabilities.json", probe),
        ("legacy-migration.json", migration),
        ("legacy-tenbun.json", pre),
        ("legacy-control-diagnostic.json", diagnostic),
        ("legacy-control-recovery-diagnostic.json", recovery),
        ("legacy-control-resync-diagnostic.json", resync),
    ])
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        bail!("usage: build-oaths-migration <output-directory>");
    }
    let out = PathBuf::from(&args[0]);
    fs::create_dir_all(&out)?;
    for (name, scene) in scenarios(&Repository::checked_in())? {
        fs::write(
            out.join(name),
            format!("{}\n", serde_json::to_string_pretty(&scene)?),
        )
        .with_context(|| name)?;
        println!(
            "{} {} steps",
            out.join(name).display(),
            scene["steps"].as_array().unwrap().len()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_save_and_load_names_match_the_actual_driver_resolver() -> Result<()> {
        let repo = Repository::checked_in();
        let runner = civ6_autotest_rs::runner::Runner::new(
            json!({}),
            repo.root.clone(),
            repo.root.clone(),
            repo.root.clone(),
            4318,
        );
        for (_, scene) in scenarios(&repo)? {
            let steps = scene["steps"].as_array().unwrap();
            for save in steps.iter().filter(|s| s["action"] == "save") {
                let load = steps
                    .iter()
                    .find(|s| s["action"] == "load" && s["name"] == save["name"])
                    .context("matching load")?;
                assert_eq!(runner.save_name(save)?, runner.save_name(load)?);
                assert_ne!(
                    runner.save_name(save)?,
                    save["name"].as_str().unwrap(),
                    "new saves need per-run names"
                );
                let mut wrong = load.clone();
                wrong["exact_name"] = true.into();
                assert_ne!(
                    runner.save_name(save)?,
                    runner.save_name(&wrong)?,
                    "resolver must detect the reviewed mismatch"
                );
            }
        }
        Ok(())
    }

    #[test]
    fn resync_requires_hidden_diplomacy_and_verified_empty_sessions() -> Result<()> {
        let vm = mlua::Lua::new();
        vm.load("hidden=true;session=nil;ContextPtr={IsHidden=function()return hidden end};PlayerManager={GetAliveMajors=function()return {{GetID=function()return 0 end},{GetID=function()return 4 end}} end};DiplomacyManager={FindOpenSessionID=function(a,b)assert(a==0 and b==4);return session end}").exec()?;
        let _: String = vm.load(RESYNC_SESSIONS_CLOSED).eval()?;
        for invalid in [
            "hidden=false",
            "session=7",
            "session=-1",
            "session='unavailable'",
            "DiplomacyManager.FindOpenSessionID=nil",
            "DiplomacyManager.FindOpenSessionID=function()error('API failed')end",
        ] {
            vm.load("hidden=true;session=nil;DiplomacyManager.FindOpenSessionID=function()return session end").exec()?;
            vm.load(invalid).exec()?;
            assert!(
                vm.load(RESYNC_SESSIONS_CLOSED).eval::<String>().is_err(),
                "accepted {invalid}"
            );
        }
        Ok(())
    }
    #[test]
    fn resync_checks_emitted_lua_against_changed_turn_ledger_flags_and_control() -> Result<()> {
        let scene = scenarios(&Repository::checked_in())?
            .into_iter()
            .find(|(name, _)| *name == "legacy-control-resync-diagnostic.json")
            .unwrap()
            .1;
        assert_eq!(scene["diagnostic_only"], true);
        let steps = scene["steps"].as_array().unwrap();
        let before = steps
            .iter()
            .find(|s| s["code"] == resync_snapshot(false)["code"])
            .unwrap()["code"]
            .as_str()
            .unwrap();
        let after = steps
            .iter()
            .find(|s| s["code"] == resync_snapshot(true)["code"])
            .unwrap()["code"]
            .as_str()
            .unwrap();
        assert_eq!(
            steps
                .iter()
                .filter(|s| s["code"]
                    .as_str()
                    .is_some_and(|c| c.contains("UI.RequestAction(")))
                .count(),
            1
        );
        assert_eq!(steps.iter().filter(|s| s["action"] == "save").count(), 1);
        assert_eq!(
            steps
                .iter()
                .filter(|s| s["action"] == "load" && s["name"] == OLD_NAME)
                .count(),
            1
        );
        assert_eq!(
            steps
                .iter()
                .filter(|s| s["action"] == "load" && s["name"] == RESYNC_NAME)
                .count(),
            1
        );
        for changed in [
            "turn=69",
            "active=false",
            "ledger.houses.p0.oathDue=70",
            "ledger.history.r1.turn=65",
            "ledger.houses.p1.oathChosen=nil",
            "ledger.houses.p1.oathKnowledge.tech=1",
            "flags.O1=1",
            "missing=true",
            "plotOwner=1",
        ] {
            let vm = mlua::Lua::new();
            vm.load(r#"
turn=68;active=true;plotOwner=nil;flags={};props={};ledger={reign=1,history={r1={turn=66}},houses={}}
Game={GetCurrentGameTurn=function()return turn end,GetProperty=function(_,k)if k=='JAC_AGES_LEDGER' then return ledger end return props[k] end,SetProperty=function(_,k,v)assert(k~='JAC_AGES_LEDGER');props[k]=v end}
GameConfiguration={GetStartTurn=function()return 1 end}
local catalogue={};for i=1,66 do catalogue['O'..i]={OathType='O'..i,CivilizationType='C'..math.floor((i-1)/3)} end
setmetatable(catalogue,{__call=function()local n=0;return function()n=n+1;return catalogue['O'..n] end end})
GameInfo={JAC_SengokuOaths=catalogue,Technologies=function()return function()return nil end end,Civics=function()return function()return nil end end}
Players={};local majors={}
for i=0,21 do
 local id=i
 local house={civ='C'..id,age='NORMAL',oathReign=1,oaths={},oathKnowledge={tech=0,civic=0},parts={oath=0}}
 if id==0 then house.oathDue=69 else house.oathChosen=1;house.oaths['O'..(3*id+1)]=true end
 ledger.houses['p'..id]=house
 local city={GetOwner=function()return id end,GetID=function()return 10+id end,GetX=function()return 3+id end,GetY=function()return 4 end}
 local p={GetID=function()return id end,IsHuman=function()return id==0 end,IsTurnActive=function()return active end,IsTurnActiveComplete=function()return false end,GetTechs=function()return {HasTech=function()return false end} end,GetCulture=function()return {HasCivic=function()return false end} end,GetCities=function()return {Members=function()local done=false;return function()if not done then done=true;return 10+id,city end end end} end}
 Players[id]=p;majors[#majors+1]=p
end
PlayerManager={GetAliveMajors=function()return majors end}
Map={GetPlot=function(x)return {GetOwner=function()return plotOwner or x-3 end,GetIndex=function()return 100+x end,GetProperty=function(_,k)if missing then return nil end return flags[k:sub(1,-4)] or 0 end} end}
"#).exec()?;
            let _: String = vm.load(SNAPSHOT).eval()?;
            let _: String = vm.load(before).eval()?;
            let _: String = vm.load(after).eval()?;
            vm.load(changed).exec()?;
            assert!(
                vm.load(after).eval::<String>().is_err(),
                "accepted changed state: {changed}"
            );
        }
        Ok(())
    }
    #[test]
    fn every_emitted_scene_passes_the_real_driver_schema() -> Result<()> {
        for (name, scene) in scenarios(&Repository::checked_in())? {
            civ6_autotest_rs::scenario::validate_scenario(&scene).with_context(|| name)?;
        }
        Ok(())
    }

    #[test]
    fn recovery_diagnostic_closes_only_visible_statement_and_never_resends_turn() -> Result<()> {
        let scene = scenarios(&Repository::checked_in())?
            .into_iter()
            .find(|(name, _)| *name == "legacy-control-recovery-diagnostic.json")
            .unwrap()
            .1;
        assert_eq!(scene["diagnostic_only"], true);
        let steps = scene["steps"].as_array().unwrap();
        assert_eq!(
            steps
                .iter()
                .filter(|s| s["code"]
                    .as_str()
                    .is_some_and(|c| c.contains("UI.RequestAction(")))
                .count(),
            1
        );
        assert_eq!(
            steps
                .iter()
                .filter(|s| s["action"] == "observe_for")
                .count(),
            2
        );
        let close = steps
            .iter()
            .position(|s| s["code"] == CLOSE_VISIBLE_STATEMENT)
            .unwrap();
        assert_eq!(steps[close - 1]["state_name"], "DiplomacyActionView");
        assert_eq!(steps[close + 1]["action"], "observe_for");
        assert_eq!(steps[close + 2]["state_name"], "GameCore_Tuner");
        assert_eq!(steps[close + 3]["state_name"], "InGame");
        let vm = mlua::Lua::new();
        vm.load("hidden=true; closes=0; ContextPtr={IsHidden=function()return hidden end}; OnClose=function()closes=closes+1;hidden=true end").exec()?;
        let hidden: String = vm.load(CLOSE_VISIBLE_STATEMENT).eval()?;
        assert!(hidden.contains("no close sent"));
        assert_eq!(vm.globals().get::<i32>("closes")?, 0);
        vm.load("hidden=false").exec()?;
        let visible: String = vm.load(CLOSE_VISIBLE_STATEMENT).eval()?;
        assert!(visible.contains("requested once"));
        let _: String = vm.load(CLOSE_VISIBLE_STATEMENT).eval()?;
        assert_eq!(vm.globals().get::<i32>("closes")?, 1);
        Ok(())
    }

    #[test]
    fn session_diagnostic_reads_sessions_and_choices_without_closing_or_selecting() -> Result<()> {
        let vm = mlua::Lua::new();
        vm.load(r#"
PlayerManager={GetAliveMajors=function()return {{GetID=function()return 0 end},{GetID=function()return 4 end}} end}
ContextPtr={IsHidden=function()return false end}
DiplomacyManager={FindOpenSessionID=function(a,b)assert(a==0 and b==4);return 7 end,GetSessionInfo=function(id)assert(id==7);return {FromPlayer=4,ToPlayer=0} end}
Game={GetEras=function()return {GetPlayerNumAllowedCommemorations=function(_,id)assert(id==0);return 1 end,GetPlayerCommemorateChoices=function(_,id)assert(id==0);return {8,9} end} end}
"#).exec()?;
        let text: String = vm.load(SESSION_DIAGNOSTIC).eval()?;
        for expected in [
            "contextHidden=false",
            "session.p4=7",
            "sessionInfo.4.FromPlayer=4",
            "GetPlayerNumAllowedCommemorations=1",
            "GetPlayerCommemorateChoices=2",
        ] {
            assert!(text.contains(expected), "{text}");
        }
        vm.load("DiplomacyManager=nil;Game.GetEras=nil").exec()?;
        let text: String = vm.load(SESSION_DIAGNOSTIC).eval()?;
        assert!(text.contains("sessionLookup=unavailable:owner missing"));
        assert!(text.contains("commemorations=unavailable:GetEras missing"));
        vm.load("DiplomacyManager={};Game.GetEras=function()return {} end")
            .exec()?;
        let text: String = vm.load(SESSION_DIAGNOSTIC).eval()?;
        assert!(text.contains("sessionLookup=unavailable:FindOpenSessionID missing"));
        assert!(text.contains("GetPlayerNumAllowedCommemorations=unavailable:"));
        assert!(text.contains("GetPlayerCommemorateChoices=unavailable:"));
        vm.load("DiplomacyManager=setmetatable({},{__index=function()error('lookup failed')end});Game.GetEras=function()error('eras failed')end").exec()?;
        let text: String = vm.load(SESSION_DIAGNOSTIC).eval()?;
        assert!(text.contains("sessionLookup=unavailable:FindOpenSessionID lookup failed:"));
        assert!(text.contains("commemorations=unavailable:GetEras failed:"));
        Ok(())
    }

    #[test]
    fn session_diagnostic_distinguishes_empty_info_from_unavailable_and_unqueried() -> Result<()> {
        let vm = mlua::Lua::new();
        vm.load(r#"
PlayerManager={GetAliveMajors=function()return {{GetID=function()return 4 end}} end}
Game={}
session=7; calls=0
DiplomacyManager={FindOpenSessionID=function()return session end,GetSessionInfo=function()calls=calls+1;return {} end}
"#).exec()?;
        let text: String = vm.load(SESSION_DIAGNOSTIC).eval()?;
        assert!(text.contains("sessionInfo.4=table fields=0"));
        for (setup, expected) in [
            (
                "DiplomacyManager.GetSessionInfo=nil",
                "unavailable:GetSessionInfo missing",
            ),
            (
                "DiplomacyManager.GetSessionInfo=function()error('probe failure') end",
                "unavailable:GetSessionInfo failed:",
            ),
            (
                "DiplomacyManager.GetSessionInfo=function()return nil end",
                "unavailable:GetSessionInfo returned nil",
            ),
            (
                "DiplomacyManager.GetSessionInfo=function()return false end",
                "unavailable:GetSessionInfo returned boolean",
            ),
            (
                "DiplomacyManager.FindOpenSessionID=function()error('lookup failure') end",
                "unavailable:FindOpenSessionID failed:",
            ),
        ] {
            vm.load(setup).exec()?;
            let text: String = vm.load(SESSION_DIAGNOSTIC).eval()?;
            assert!(text.contains(expected), "{text}");
        }
        vm.load("DiplomacyManager.FindOpenSessionID=function()return session end;DiplomacyManager.GetSessionInfo=function()calls=calls+1;error('must not query excluded IDs') end").exec()?;
        for setup in ["session=nil", "session=-1", "session='opaque'"] {
            vm.load(setup).exec()?;
            let text: String = vm.load(SESSION_DIAGNOSTIC).eval()?;
            assert!(
                text.contains("sessionInfo.4=not queried:session type="),
                "{text}"
            );
            assert!(!text.contains("closed"), "{text}");
        }
        assert_eq!(vm.globals().get::<i32>("calls")?, 1);
        Ok(())
    }

    #[test]
    fn turn_diagnostic_reads_ready_and_pause_state_with_correct_receivers() -> Result<()> {
        let vm = mlua::Lua::new();
        vm.load(r#"
ledger={houses={p0={age='NORMAL'}}}
Game={GetCurrentGameTurn=function()return 67 end,GetLocalPlayer=function()return 0 end,GetProperty=function()return ledger end,SetProperty=function()error('diagnostic must not write')end}
GameConfiguration={GetStartTurn=function()return 1 end,IsPaused=function(...)assert(select('#',...)==0);return false end,GetPausePlayer=function()return -1 end}
UI={HasSentTurnComplete=function(...)assert(select('#',...)==0);return true end,IsTurnTimerElapsed=function(id)assert(id==0);return false end,RequestAction=function()error('diagnostic must not request')end}
UIManager={};UIManager.IsPopupQueueDisabled=function(self)assert(self==UIManager);return false end
local p;p={GetID=function()return 0 end,CanUnreadyTurn=function(self)assert(self==p);return true end,IsHuman=function()error('probe failure')end}
PlayerManager={GetAliveMajors=function()return {p} end}
"#).exec()?;
        let text: String = vm.load(TURN_DIAGNOSTIC).eval()?;
        for expected in [
            "UI.HasSentTurnComplete=true",
            "UI.IsTurnTimerElapsed=false",
            "GameConfiguration.IsPaused=false",
            "GameConfiguration.GetPausePlayer=-1",
            "UIManager.IsPopupQueueDisabled=false",
            "p0.CanUnreadyTurn=true",
            "p0.IsHuman=error:",
            "p0.IsAlive=unavailable:IsAlive missing",
            "UI.CanEndTurn=unavailable:CanEndTurn missing",
            "blocks=unavailable:owner missing",
        ] {
            assert!(text.contains(expected), "{text}");
        }
        assert!(text.find("UI.HasSentTurnComplete").unwrap() < text.find("p0.").unwrap());
        vm.load("Game.GetLocalPlayer=nil;UI=nil;UIManager=nil;GameConfiguration.IsPaused=nil")
            .exec()?;
        let text: String = vm.load(TURN_DIAGNOSTIC).eval()?;
        assert!(text.contains("local=unavailable:GetLocalPlayer missing"));
        assert!(text.contains("UI.HasSentTurnComplete=unavailable:owner missing"));
        assert!(text.contains("UI.IsTurnTimerElapsed=unavailable:local player unavailable"));
        assert!(text.contains("GameConfiguration.IsPaused=unavailable:IsPaused missing"));
        Ok(())
    }

    #[test]
    fn stalled_diagnostic_still_captures_both_contexts_and_event_history() -> Result<()> {
        let scene = scenarios(&Repository::checked_in())?
            .into_iter()
            .find(|(name, _)| *name == "legacy-control-diagnostic.json")
            .unwrap()
            .1;
        let vm = legacy_vm()?;
        vm.load(r#"
active=true; requests=0; callbacks={}; logs={}
Players[0].GetID=function()return 0 end
Players[0].IsTurnActive=function()return active end
PlayerManager={GetAliveMajors=function()return {Players[0]} end}
Game.GetLocalPlayer=function()return 0 end
UI={RequestAction=function()requests=requests+1;active=false end,CanEndTurn=function()return active end}
ActionTypes={ACTION_ENDTURN=1}
Events=setmetatable({},{__index=function(t,name)local event={Add=function(fn)callbacks[name]=fn end};rawset(t,name,event);return event end})
print=function(s)logs[#logs+1]=s end
"#).exec()?;
        let mut snapshots = Vec::new();
        let mut paused = 0;
        for step in scene["steps"].as_array().unwrap() {
            match step["action"].as_str().unwrap() {
                "ui_lua" => {
                    let value: String = vm.load(step["code"].as_str().unwrap()).eval()?;
                    if value.starts_with("LEGACY_CONTROL_DIAGNOSTIC") {
                        snapshots.push((step["state_name"].as_str().unwrap(), value));
                    }
                }
                "wait_for" => {
                    // Only the pre-request readiness check may fail the scene.
                    assert!(vm
                        .load(step["condition"]["code"].as_str().unwrap())
                        .eval::<bool>()?);
                    assert_eq!(vm.globals().get::<i32>("requests")?, 0);
                }
                "pause" | "observe_for" => {
                    let seconds = step["seconds"].as_i64().unwrap();
                    assert!(step["timeout"].as_i64().unwrap_or(120) > seconds);
                    paused += seconds;
                }
                _ => {}
            }
        }
        assert_eq!(paused, 130);
        assert_eq!(vm.globals().get::<i32>("requests")?, 1);
        assert_eq!(snapshots.len(), 6);
        for pair in snapshots[2..].chunks(2) {
            assert_eq!([pair[0].0, pair[1].0], ["GameCore_Tuner", "InGame"]);
            for (_, value) in pair {
                assert!(value.contains("raw=67"));
                assert!(value.contains("p0.IsTurnActive=false"));
            }
        }
        vm.load(
            r#"
callbacks.PlayerTurnActivated(0,false)
callbacks.GameCoreEventPublishComplete()
callbacks.GameCoreEventPublishComplete()
callbacks.GameCoreEventPlaybackComplete()
assert(#logs==3)
assert(string.find(logs[1],'PlayerTurnActivated:0,false',1,true))
assert(string.find(logs[2],'p0.IsTurnActive=false',1,true))
assert(string.find(logs[3],'GameCoreEventPlaybackComplete:',1,true))
callbacks.GameCoreEventPlaybackComplete()
assert(#logs==4)
assert(string.find(logs[4],'GameCoreEventPlaybackComplete:',1,true))
assert(ledger.houses.p0.oaths==nil)
"#,
        )
        .exec()?;
        Ok(())
    }

    #[test]
    fn live_catalogue_rejects_missing_and_extra_rows() -> Result<()> {
        let vm = mlua::Lua::new();
        vm.load("rows=66; GameInfo={JAC_SengokuOaths=function()local n=0; return function()n=n+1; if n<=rows then return {} end end end}").exec()?;
        vm.load(CATALOGUE).exec()?;
        let mut checked = 0;
        for (_, scene) in scenarios(&Repository::checked_in())? {
            for step in scene["steps"].as_array().unwrap() {
                let Some(code) = step.get("code").and_then(Value::as_str) else {
                    continue;
                };
                if code.contains("'_ON'") || code.contains("TODO134_CAPABILITIES") {
                    checked += 1;
                    for count in [0, 65, 67] {
                        vm.globals().set("rows", count)?;
                        let error = vm.load(code).exec().unwrap_err().to_string();
                        assert!(
                            error.contains("live oath catalogue must contain 66 rows"),
                            "{error}"
                        );
                    }
                }
            }
        }
        assert_eq!(
            checked, 7,
            "capability, two snapshots, pre-reign flags and three resync snapshots"
        );
        Ok(())
    }
    #[test]
    fn every_emitted_lua_chunk_compiles_on_lua51() -> Result<()> {
        let vm = mlua::Lua::new();
        for (_, scene) in scenarios(&Repository::checked_in())? {
            for step in scene["steps"].as_array().unwrap() {
                for code in [
                    step.get("code"),
                    step.get("condition").and_then(|v| v.get("code")),
                ]
                .into_iter()
                .flatten()
                {
                    vm.load(code.as_str().unwrap()).into_function()?;
                }
            }
        }
        Ok(())
    }

    #[test]
    fn restored_snapshot_rejects_ai_changes_and_a_missing_baseline() -> Result<()> {
        let scene = scenarios(&Repository::checked_in())?
            .into_iter()
            .find(|(name, _)| *name == "legacy-migration.json")
            .unwrap()
            .1;
        let codes: Vec<_> = scene["steps"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|step| step.get("code").and_then(Value::as_str))
            .filter(|code| code.contains("MIGRATION_PERSISTED"))
            .collect();
        assert_eq!(codes.len(), 2);
        let vm = mlua::Lua::new();
        vm.load(r#"
ledger={reign=1,history={},houses={}}
for i=0,1 do ledger.houses['p'..i]={civ='CIV',age='NORMAL',oaths={OWN=true},oathReign=1,oathChosen=1,oathKnowledge={tech=2}} end
props={JAC_AGES_LEDGER=ledger,JAC_TODO134_OLD_HISTORY='{}'}
Game={GetProperty=function(_,k)return props[k] end,SetProperty=function(_,k,v)props[k]=v end,GetCurrentGameTurn=function()return 68 end}
GameConfiguration={GetStartTurn=function()return 1 end}
GameInfo={JAC_SengokuOaths=function()local n=0;return function()n=n+1;if n<=66 then return {OathType='O'..n,CivilizationType='CIV'} end end end}
local city={GetX=function()return 1 end,GetY=function()return 1 end}
local player={GetID=function()return 0 end,GetCities=function()return {Members=function()local emitted=false;return function()if not emitted then emitted=true;return 1,city end end end} end}
PlayerManager={GetAliveMajors=function()return {player} end}
Map={GetPlot=function()return {GetProperty=function()return 0 end} end}
"#).exec()?;
        let _: String = vm.load(codes[0]).eval()?;
        let _: String = vm.load(codes[1]).eval()?;
        vm.load("ledger.houses.p1.oaths={FOREIGN=true}").exec()?;
        let error = vm.load(codes[1]).eval::<String>().unwrap_err();
        assert!(error.to_string().contains("migration state changed"));
        vm.load("ledger.houses.p1.oaths={OWN=true};props.JAC_TODO134_MIGRATED_SNAPSHOT=nil")
            .exec()?;
        let error = vm.load(codes[1]).eval::<String>().unwrap_err();
        assert!(error.to_string().contains("snapshot missing after load"));
        vm.load("assert(props.JAC_TODO134_MIGRATED_SNAPSHOT==nil)")
            .exec()?;
        Ok(())
    }

    #[test]
    fn tenbun_checks_reject_premature_metadata_and_wrong_research_baselines() -> Result<()> {
        let scene = scenarios(&Repository::checked_in())?
            .into_iter()
            .find(|(name, _)| *name == "legacy-tenbun.json")
            .unwrap()
            .1;
        let steps = scene["steps"].as_array().unwrap();
        let before = steps[2]["code"].as_str().unwrap();
        let after = steps.last().unwrap()["code"].as_str().unwrap();
        let vm = mlua::Lua::new();
        vm.load(r#"
turn=35;ledger={reign=0,houses={p0={parts={},oathKnowledge={tech=1,civic=1}}}}
Game={GetProperty=function()return ledger end,GetCurrentGameTurn=function()return turn end}
GameConfiguration={GetStartTurn=function()return 1 end}
local function rows(limit)return function()local n=0;return function()n=n+1;if n<=limit then return {Index=n,OathType='O'..n} end end end end
GameInfo={JAC_SengokuOaths=rows(66),Technologies=rows(2),Civics=rows(2)}
local city={GetX=function()return 1 end,GetY=function()return 1 end}
local p={GetID=function()return 0 end,GetTechs=function()return {HasTech=function(_,id)return id==1 end} end,GetCulture=function()return {HasCivic=function(_,id)return id==1 end} end,GetCities=function()return {Members=function()local done=false;return function()if not done then done=true;return 1,city end end end} end}
PlayerManager={GetAliveMajors=function()return {p} end}
Map={GetPlot=function()return {GetProperty=function()return 0 end} end}
"#).exec()?;
        let _: String = vm.load(after).eval()?;
        for field in ["oaths", "oathReign", "oathChosen", "oathDue"] {
            vm.load(format!("ledger.houses.p0.{field}=1")).exec()?;
            let error = vm.load(after).eval::<String>().unwrap_err();
            assert!(error.to_string().contains("pre-reign oath metadata"));
            vm.load(format!("ledger.houses.p0.{field}=nil")).exec()?;
        }
        vm.load("ledger.houses.p0.oathKnowledge.tech=2").exec()?;
        assert!(vm
            .load(after)
            .eval::<String>()
            .unwrap_err()
            .to_string()
            .contains("knowledge baseline mismatch"));
        vm.load("turn=34;ledger.houses.p0.oathKnowledge=nil")
            .exec()?;
        let _: String = vm.load(before).eval()?;
        for field in [
            "oaths",
            "oathReign",
            "oathChosen",
            "oathDue",
            "oathKnowledge",
        ] {
            vm.load(format!("ledger.houses.p0.{field}=1")).exec()?;
            assert!(vm.load(before).eval::<String>().is_err());
            vm.load(format!("ledger.houses.p0.{field}=nil")).exec()?;
        }
        Ok(())
    }
    fn legacy_vm() -> Result<mlua::Lua> {
        let vm = mlua::Lua::new();
        vm.load(r#"ledger={reign=1,history={r1={turn=66}},houses={}}; for i=0,21 do ledger.houses['p'..i]={parts={}} end; Game={GetProperty=function() return ledger end,GetCurrentGameTurn=function() return 67 end}; GameConfiguration={GetStartTurn=function() return 1 end}; Players={[0]={IsAlive=function() return true end,IsHuman=function() return true end}}"#).exec()?;
        Ok(vm)
    }
    #[test]
    fn legacy_probe_rejects_current_or_incomplete_saves() -> Result<()> {
        let vm = legacy_vm()?;
        let _: String = vm.load(LEGACY).eval()?;
        vm.load("ledger.houses.p0.oaths={}").exec()?;
        assert!(vm.load(LEGACY).eval::<String>().is_err());
        let vm = legacy_vm()?;
        vm.load("ledger.houses.p21=nil").exec()?;
        assert!(vm.load(LEGACY).eval::<String>().is_err());
        let vm = legacy_vm()?;
        vm.load("ledger.history.r1.turn=65").exec()?;
        assert!(vm.load(LEGACY).eval::<String>().is_err());
        Ok(())
    }
}
