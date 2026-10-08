//! Code-level two-client replay, not a Civ6 network/engine integration test.
//! Each gameplay VM runs shipped sources, with its own properties, event handlers,
//! enumeration order and RNG. UI VMs are separate, read-only projections: the
//! EXECUTE_SCRIPT envelope is replayed in engine order with the engine actor.
//! Save reconstruction persists properties/cities, never closures or MOD locals.
use civ6_lua_tests_rs::{exec, exec_args, new_lua, read_repo};
use mlua::{Lua, LuaSerdeExt, Value};
use serde_json::{json, Value as Json};

const CUSTOM: &str = "poc/JAC_PoC_CustomCivs/Scripts/JAC_CustomCivs.lua";
const NAMES: &str = "poc/JAC_PoC_CustomCivs/UI/JAC_CivNameOverride.lua";
const PACT: &str = "poc/JAC_PoC_SengokuTrees/Scripts/JAC_Sengoku_UnificationPact.lua";
const PANEL: &str = "poc/JAC_PoC_SengokuTrees/UI/JAC_UnificationPact.lua";

const GAME_SETUP: &str = r#"
-- Persisted engine state is separate from runtime closures and event subscriptions.
GPROPS, PPROPS, WRITES = {}, {}, {}
Game.GetProperty = function(_, key) return GPROPS[key] end
Game.SetProperty = function(_, key, value)
  GPROPS[key] = value
  WRITES[#WRITES+1] = {key=key, value=value}
end
local methods = {
  GetCivilizationTypeName=function(self) return self.civ end,
  GetLeaderTypeName=function(self) return 'LEADER_' .. self.civ end,
  GetCivilizationShortDescription=function(self) return 'LOC_' .. self.civ end,
  GetCivilizationDescription=function(self) return 'LOC_DESC_' .. self.civ end,
}
for id=0,3 do
  PPROPS[tostring(id)] = {}
  local pid=id
  Players[id].GetProperty=function(_,key) return PPROPS[tostring(pid)][key] end
  Players[id].SetProperty=function(_,key,value) PPROPS[tostring(pid)][key]=value end
  Players[id].GetCities=(function(original)
    return function(self)
      local cities=original(self)
      cities.GetCount=function() local n=0; for _,c in pairs(CITIES) do if c.owner==pid then n=n+1 end end; return n end
      return cities
    end
  end)(Players[id].GetCities)
  PlayerConfigurations[id]=setmetatable({civ=({'ODA','DATE','ASHINA','TAKEDA'})[id+1]}, {__index=methods})
  PlayerConfigurations[id].civ='CIVILIZATION_JAC_' .. PlayerConfigurations[id].civ
end
CITY(0,6,5,true); CITY(0,6,14); CITY(1,25,5,true)
CITY(1,25,14); CITY(2,16,10,true); CITY(3,3,3,true)
local originalMajors=PlayerManager.GetAliveMajors
PlayerManager.GetAliveMajors=function()
  local list=originalMajors()
  if REVERSE then for i=1,math.floor(#list/2) do list[i],list[#list+1-i]=list[#list+1-i],list[i] end end
  return list
end
-- Allow only Gameplay events that have official engine-side usage. In particular
-- LoadScreenClose is absent here; it belongs to UI, and must not abort loading.
Events = nil
local allowed={CityBuilt=true, CityConquered=true, OnGameTurnStarted=true,
  JAC_UnificationPactPropose=true, JAC_UnificationPactAccept=true, JAC_UnificationPactLeave=true}
local eventRegistry=GameEvents
GameEvents=setmetatable({}, {__index=function(_,name)
  assert(allowed[name], 'unsupported gameplay event: ' .. name)
  return eventRegistry[name]
end})
math.randomseed(SEED); for i=1,SEED%7 do math.random() end
math.random=function() error('clan/pact gameplay used client RNG') end
math.randomseed=function() error('clan/pact gameplay reseeded client RNG') end
Game.GetRandNum=function() error('clan/pact gameplay consumed engine RNG unexpectedly') end
function SAVE_STATE()
  local cities,players={},{}
  for index,c in pairs(CITIES) do
    local buildings={}; for b,value in pairs(c.buildings) do buildings[tostring(b)]=value end
    cities[tostring(index)]={owner=c.owner, capital=c.capital, buildings=buildings}
  end
  for id=0,3 do players[tostring(id)]={dead=Players[id].dead or false, team=Players[id].team} end
  return {game=GPROPS, players=PPROPS, cities=cities, specs=players, allied=ALLIED, war=WAR, turn=TURN, start=START}
end
function RESTORE_STATE(state)
  GPROPS=state.game; PPROPS=state.players; ALLIED=state.allied; WAR=state.war; TURN=state.turn; START=state.start
  CITIES={}
  for key,c in pairs(state.cities) do
    local index=tonumber(key); local city=CITY(c.owner,index%32,math.floor(index/32),c.capital)
    for b,value in pairs(c.buildings) do city.buildings[tonumber(b)]=value end
  end
  for id=0,3 do Players[id].dead=state.specs[tostring(id)].dead; Players[id].team=state.specs[tostring(id)].team end
  BUILT={}; WRITES={}
end
"#;

const UI_SETUP: &str = r#"
local state,localPlayer=...
LOCAL_PLAYER=localPlayer; REQUESTS={}; ROWS={}
Game={GetLocalPlayer=function() return LOCAL_PLAYER end,
  GetProperty=function(_,key) return state.game[key] end,
  SetProperty=function() error('UI attempted to write Game state') end}
local methods={GetCivilizationTypeName=function(self) return self.civ end,
  GetCivilizationShortDescription=function(self) return 'LOC_' .. self.civ end,
  GetCivilizationDescription=function(self) return 'LOC_DESC_' .. self.civ end}
Players={}; PlayerConfigurations={}
for id=0,3 do
  local pid=id
  PlayerConfigurations[id]=setmetatable({civ='CIVILIZATION_JAC_' .. ({'ODA','DATE','ASHINA','TAKEDA'})[id+1]}, {__index=methods})
  Players[id]={GetID=function() return pid end,
    GetTeam=function() return state.specs[tostring(pid)].team end,
    GetProperty=function(_,key) return state.players[tostring(pid)][key] end,
    SetProperty=function() error('UI attempted to write Player state') end,
    GetDiplomacy=function() return {HasMet=function() return true end} end}
end
PlayerManager={GetAliveMajors=function()
  local result={}; for id=0,3 do if not state.specs[tostring(id)].dead then result[#result+1]=Players[id] end end; return result
end}
Locale={Lookup=function(key,...) return key end}
local function control()
  return {hidden=true, SetHide=function(self,v) self.hidden=v end,
    IsHidden=function(self) return self.hidden end,
    SetText=function(self,v) self.text=v end, SetDisabled=function(self,v) self.disabled=v end,
    RegisterCallback=function(self,_,f) self.click=f end,
    CalculateSize=function() end, DoAutoSize=function() end}
end
Controls={Rows=control(), PactPanel=control(), LeaveButton=control(), Status=control(), PactToggle=control()}
InstanceManager={new=function() return {
  ResetInstances=function() ROWS={} end,
  GetInstance=function() local row={Name=control(),Action=control()}; ROWS[#ROWS+1]=row; return row end} end}
ContextPtr={SetHide=function() end}; Mouse={eLClick=1}; PlayerOperations={EXECUTE_SCRIPT=77}
UI={RequestPlayerOperation=function(actor,operation,params)
  assert(operation==PlayerOperations.EXECUTE_SCRIPT)
  REQUESTS[#REQUESTS+1]={actor=actor, operation=operation, params=params}
end}
function include(name) assert(name=='InstanceManager') end
local handlers={}
local allowed={LocalPlayerTurnBegin=true, GameCoreEventPublishComplete=true, LoadScreenClose=true}
Events=setmetatable({}, {__index=function(_,name)
  assert(allowed[name], 'unsupported UI event: ' .. name)
  return {Add=function(f) handlers[name]=handlers[name] or {}; table.insert(handlers[name],f) end}
end})
function UI_EVENT(name) for _,f in ipairs(handlers[name] or {}) do f() end end
function UPDATE_STATE(nextState) state=nextState end
"#;

struct Client(Lua);
impl Client {
    fn new(reverse: bool, seed: i64, no_castles: bool) -> Self {
        Self::boot(reverse, seed, no_castles, None)
    }
    fn boot(reverse: bool, seed: i64, no_castles: bool, saved: Option<&Json>) -> Self {
        let lua = new_lua();
        exec_args(
            &lua,
            include_str!("../fixtures/unification/stubs.lua"),
            (
                "JAC_UNIFICATION_CITY_IS_CASTLE",
                "BUILDING_JAC_REBUILT_ON_CASTLE",
                "BUILDING_JAC_PACT_UNIFIED",
            ),
        );
        lua.globals().set("REVERSE", reverse).unwrap();
        lua.globals().set("SEED", seed).unwrap();
        exec(&lua, GAME_SETUP);
        if no_castles {
            exec(&lua, "SET_NO_CASTLES()");
        }
        if let Some(state) = saved {
            lua.globals()
                .get::<mlua::Function>("RESTORE_STATE")
                .unwrap()
                .call::<()>(lua.to_value(state).unwrap())
                .unwrap();
        }
        for path in [
            "poc/JAC_PoC_SengokuTrees/Scripts/JAC_Sengoku_RebuiltCastles.lua",
            PACT,
        ] {
            lua.load(read_repo(path)).set_name(path).exec().unwrap();
        }
        Self(lua)
    }
    fn run(&self, script: &str) {
        exec(&self.0, script);
    }
    fn check(&self, expression: &str) {
        assert!(
            self.0
                .load(format!("return {expression}"))
                .eval::<bool>()
                .unwrap(),
            "{expression}"
        );
    }
    fn state(&self) -> Json {
        self.0
            .from_value(self.0.load("return SAVE_STATE()").eval::<Value>().unwrap())
            .unwrap()
    }
    fn restored(state: &Json, reverse: bool, no_castles: bool) -> Self {
        Self::boot(reverse, 9281, no_castles, Some(state))
    }
    fn dispatch(&self, envelope: &Json) {
        let params = self.0.to_value(&envelope["params"]).unwrap();
        let event = envelope["params"]["OnStart"].as_str().unwrap();
        self.0
            .globals()
            .get::<mlua::Function>("FIRE")
            .unwrap()
            .call::<()>((event, envelope["actor"].as_i64().unwrap(), params))
            .unwrap();
    }
}

struct Panel(Lua);
impl Panel {
    fn new(state: &Json, local_player: i64) -> Self {
        let lua = new_lua();
        exec_args(&lua, UI_SETUP, (lua.to_value(state).unwrap(), local_player));
        exec(&lua, &read_repo(NAMES));
        exec(&lua, "JAC_InstallCivNameOverride('two-client-test')");
        exec(&lua, &read_repo(PANEL));
        Self(lua)
    }
    fn run(&self, code: &str) {
        exec(&self.0, code);
    }
    fn update(&self, state: &Json) {
        self.0
            .globals()
            .get::<mlua::Function>("UPDATE_STATE")
            .unwrap()
            .call::<()>(self.0.to_value(state).unwrap())
            .unwrap();
        self.run("UI_EVENT('GameCoreEventPublishComplete')");
    }
    fn request(&self, action: &str, target: i64) -> Json {
        self.0
            .globals()
            .get::<mlua::Function>("JAC_RequestPact")
            .unwrap()
            .call::<()>((action, target))
            .unwrap();
        self.0
            .from_value(
                self.0
                    .load("return REQUESTS[#REQUESTS]")
                    .eval::<Value>()
                    .unwrap(),
            )
            .unwrap()
    }
    fn name(&self, id: i64) -> String {
        self.0
            .load("local id=...; return PlayerConfigurations[id]:GetCivilizationShortDescription()")
            .call(id)
            .unwrap()
    }
    fn shown(&self) -> bool {
        self.0
            .load("return not Controls.PactPanel:IsHidden()")
            .eval()
            .unwrap()
    }
}

struct Pair {
    a: Client,
    b: Client,
}
impl Pair {
    fn new(no_castles: bool) -> Self {
        Self {
            a: Client::new(false, 17, no_castles),
            b: Client::new(true, 918, no_castles),
        }
    }
    fn both(&self, code: &str) {
        self.a.run(code);
        self.b.run(code);
        self.same();
    }
    fn same(&self) {
        assert_eq!(self.a.state(), self.b.state());
    }
    fn check(&self, expression: &str) {
        self.a.check(expression);
        self.b.check(expression);
    }
    fn request(&self, actor: i64, action: &str, target: i64) {
        let panel = Panel::new(&self.a.state(), actor);
        let envelope = panel.request(action, target);
        assert_eq!(envelope["actor"], actor);
        assert_eq!(
            envelope["params"]["OnStart"],
            format!("JAC_UnificationPact{action}")
        );
        self.a.dispatch(&envelope);
        self.b.dispatch(&envelope);
        self.same();
    }
    fn reload(&mut self, no_castles: bool) {
        let a = self.a.state();
        let b = self.b.state();
        self.a = Client::restored(&a, true, no_castles);
        self.b = Client::restored(&b, false, no_castles);
        self.same();
        assert_eq!(self.a.state(), a);
    }
}

#[test]
fn clan_rename_uses_gameplay_event_and_survives_fresh_ui_rejoin() {
    let mut p = Pair::new(false);
    exec(&p.a.0, &read_repo(CUSTOM));
    exec(&p.b.0, &read_repo(CUSTOM));
    let ui_a = Panel::new(&p.a.state(), 0);
    let ui_b = Panel::new(&p.b.state(), 3);
    for turn in [1, 2] {
        p.both(&format!("TURN={turn}; FIRE('OnGameTurnStarted',TURN)"));
        p.check("GPROPS.JAC_POC_CUSTOM_CIVS_RENAMED_NAME==nil");
    }
    p.both("TURN=3; FIRE('OnGameTurnStarted',TURN)");
    p.check("GPROPS.JAC_POC_CUSTOM_CIVS_RENAMED_NAME=='LOC_JAC_TAKEDA_RENAMED_NAME' and GPROPS.JAC_POC_CUSTOM_CIVS_RENAME_ATTEMPTED==true");
    ui_a.update(&p.a.state());
    ui_b.update(&p.b.state());
    assert_eq!(ui_a.name(3), "LOC_JAC_TAKEDA_RENAMED_NAME");
    assert_eq!(ui_b.name(3), ui_a.name(3));
    assert_eq!(ui_a.name(0), "LOC_CIVILIZATION_JAC_ODA");
    p.check(
        "PlayerConfigurations[3]:GetCivilizationShortDescription()=='LOC_CIVILIZATION_JAC_TAKEDA'",
    );
    let before = p.a.state();
    p.both("FIRE('OnGameTurnStarted',3)");
    assert_eq!(p.a.state(), before);
    p.reload(false);
    exec(&p.a.0, &read_repo(CUSTOM));
    exec(&p.b.0, &read_repo(CUSTOM));
    p.both("TURN=4; FIRE('OnGameTurnStarted',TURN)");
    let rejoined = Panel::new(&p.b.state(), 3);
    assert_eq!(rejoined.name(3), "LOC_JAC_TAKEDA_RENAMED_NAME");
    rejoined.run("assert(PlayerConfigurations[3]:GetCivilizationDescription()=='LOC_JAC_TAKEDA_RENAMED_NAME')");
}

#[test]
fn public_ui_envelope_uses_engine_actor_and_requires_the_actual_human_acceptor() {
    let p = Pair::new(false);
    let panel = Panel::new(&p.a.state(), 0);
    let mut envelope = panel.request("Propose", 3);
    envelope["params"]["ForPlayer"] = json!(1);
    envelope["params"]["PlayerID"] = json!(1);
    p.a.dispatch(&envelope);
    p.b.dispatch(&envelope);
    p.same();
    p.check(
        "GPROPS.JAC_UNIFICATION_PACT_PROPOSAL_3==0 and GPROPS.JAC_UNIFICATION_PACT_LEAD_0==nil",
    );
    p.request(1, "Accept", 0);
    p.check("GPROPS.JAC_UNIFICATION_PACT_LEAD_1==nil and PPROPS['1'].JAC_UNIFICATION_PACT_RESULT=='INVALID'");
    p.request(3, "Accept", 0);
    p.check("GPROPS.JAC_UNIFICATION_PACT_LEAD_0==0 and GPROPS.JAC_UNIFICATION_PACT_LEAD_3==0 and GPROPS.JAC_UNIFICATION_PACT_PROPOSAL_3==nil");
}

#[test]
fn proposal_accept_and_leave_duplicates_never_add_extra_members_or_markers() {
    let p = Pair::new(false);
    p.request(0, "Propose", 3);
    p.request(0, "Propose", 3);
    p.request(3, "Accept", 0);
    p.request(3, "Accept", 0);
    p.check("GPROPS.JAC_UNIFICATION_PACT_LEAD_3==0 and #BUILT==0");
    p.request(3, "Leave", 3);
    p.request(3, "Leave", 3);
    p.request(3, "Accept", 0);
    p.check("GPROPS.JAC_UNIFICATION_PACT_LEAD_0==nil and GPROPS.JAC_UNIFICATION_PACT_LEAD_3==nil and #BUILT==0");
}

#[test]
fn competing_proposals_follow_engine_order_on_both_clients() {
    for order in [[0, 1], [1, 0]] {
        let p = Pair::new(false);
        for actor in order {
            p.request(actor, "Propose", 3);
        }
        let last = order[1];
        let first = order[0];
        p.request(3, "Accept", first);
        p.check("GPROPS.JAC_UNIFICATION_PACT_LEAD_3==nil");
        p.request(3, "Accept", last);
        p.check(&format!("GPROPS.JAC_UNIFICATION_PACT_LEAD_3=={last} and GPROPS.JAC_UNIFICATION_PACT_LEAD_{first}==nil"));
    }
}

#[test]
fn invalid_targets_and_actors_do_not_mutate_pacts_or_abort_handlers() {
    let p = Pair::new(false);
    for target in [
        json!(null),
        json!("3"),
        json!(99),
        json!(-1),
        json!({"id":3}),
    ] {
        let envelope =
            json!({"actor":0,"params":{"OnStart":"JAC_UnificationPactPropose","Target":target}});
        p.a.dispatch(&envelope);
        p.b.dispatch(&envelope);
        p.same();
        p.check("GPROPS.JAC_UNIFICATION_PACT_PROPOSAL_3==nil and PPROPS['0'].JAC_UNIFICATION_PACT_RESULT=='INVALID'");
    }
    let before = p.a.state();
    for actor in [-1, 99] {
        for action in ["Propose", "Accept", "Leave"] {
            let envelope = json!({"actor":actor,"params":{"OnStart":format!("JAC_UnificationPact{action}"),"Target":3}});
            p.a.dispatch(&envelope);
            p.b.dispatch(&envelope);
            p.same();
            assert_eq!(p.a.state(), before);
        }
    }
}

#[test]
fn ai_choice_is_sorted_and_warring_or_same_team_houses_cannot_join() {
    let p = Pair::new(false);
    p.both("ALLIED['1-0']=true; ALLIED['1-3']=true; TURN=START+1; FIRE('OnGameTurnStarted',TURN)");
    p.check(
        "GPROPS.JAC_UNIFICATION_PACT_PROPOSAL_0==1 and GPROPS.JAC_UNIFICATION_PACT_PROPOSAL_3==nil",
    );
    p.both("FIRE('OnGameTurnStarted',TURN)");
    p.check(
        "GPROPS.JAC_UNIFICATION_PACT_PROPOSAL_0==1 and GPROPS.JAC_UNIFICATION_PACT_PROPOSAL_3==1",
    );
    p.both("WAR['0-1']=true");
    p.request(0, "Accept", 1);
    p.check("GPROPS.JAC_UNIFICATION_PACT_LEAD_0==nil");
    p.both("WAR['0-1']=nil; Players[3].team=0");
    p.request(0, "Propose", 3);
    p.check("GPROPS.JAC_UNIFICATION_PACT_PROPOSAL_3==1 and PPROPS['0'].JAC_UNIFICATION_PACT_RESULT=='INVALID'");
    p.request(0, "Propose", 2);
    p.check("PPROPS['0'].JAC_UNIFICATION_PACT_RESULT=='NOT_ALLIED' and GPROPS.JAC_UNIFICATION_PACT_LEAD_2==nil");
}

#[test]
fn saved_pending_proposal_is_accepted_after_ui_rejoin_and_alliance_survives_reload() {
    let mut p = Pair::new(false);
    p.request(0, "Propose", 3);
    p.reload(false);
    let panel = Panel::new(&p.b.state(), 3);
    panel.run("UI_EVENT('LoadScreenClose')");
    assert!(panel.shown());
    let envelope = panel.request("Accept", 0);
    p.a.dispatch(&envelope);
    p.b.dispatch(&envelope);
    p.same();
    p.reload(false);
    p.both("TURN=START+1; FIRE('OnGameTurnStarted',TURN)");
    p.check("GPROPS.JAC_UNIFICATION_PACT_LEAD_0==0 and GPROPS.JAC_UNIFICATION_PACT_LEAD_3==0");
    let rejoined = Panel::new(&p.b.state(), 3);
    rejoined.run("UI_EVENT('LoadScreenClose'); JAC_TogglePactPanel()");
    rejoined.run("assert(Controls.Status.text=='LOC_JAC_UNIFICATION_PACT_MEMBER[NEWLINE]LOC_JAC_UNIFICATION_PACT_RESULT_JOINED')");
}

#[test]
fn ownership_transfer_passes_lead_to_most_castles_and_places_one_marker_only() {
    // Lead 1 has three castles when the second city transfers to 1, and wins
    // the 2-vs-2 tie when it transfers to 2. Test both succession rules.
    for second_owner in [1, 2] {
        let mut p = Pair::new(false);
        p.both("ALLIED['0-1']=true; ALLIED['0-2']=true");
        p.request(0, "Propose", 1);
        p.request(0, "Propose", 2);
        p.check("#BUILT==1 and CITIES[166].buildings[8]==true and CITIES[185].buildings[8]==nil");
        p.reload(false);
        p.both(&format!("CITIES[166].owner=2; CITIES[166].capital=false; CITIES[454].owner={second_owner}; Players[0].dead=true; FIRE('CityConquered',2,0,166,6,5)"));
        p.check("GPROPS.JAC_UNIFICATION_PACT_LEAD_1==1 and GPROPS.JAC_UNIFICATION_PACT_LEAD_2==1 and CITIES[185].buildings[8]==true and CITIES[336].buildings[8]==nil");
        p.both("FIRE('CityConquered',2,0,166,6,5); FIRE('OnGameTurnStarted',TURN)");
        p.check("#BUILT==1");
    }
}

#[test]
fn actual_panel_callbacks_send_requests_without_writing_engine_properties() {
    let p = Pair::new(false);
    let before = p.a.state();
    let proposer = Panel::new(&before, 0);
    proposer.run("Controls.PactToggle.click(); assert(not Controls.PactPanel:IsHidden()); for _,row in ipairs(ROWS) do if row.Name.text=='LOC_CIVILIZATION_JAC_TAKEDA' then assert(not row.Action.disabled); row.Action.click() end end; assert(#REQUESTS==1)");
    let envelope: Json = proposer
        .0
        .from_value(
            proposer
                .0
                .load("return REQUESTS[1]")
                .eval::<Value>()
                .unwrap(),
        )
        .unwrap();
    assert_eq!(envelope["params"]["OnStart"], "JAC_UnificationPactPropose");
    assert_eq!(envelope["actor"], 0);
    assert_eq!(envelope["params"]["Target"], 3);
    assert_eq!(p.a.state(), before);
    p.a.dispatch(&envelope);
    p.b.dispatch(&envelope);
    p.same();
    let acceptor = Panel::new(&p.b.state(), 3);
    acceptor.run("UI_EVENT('LoadScreenClose'); for _,row in ipairs(ROWS) do if row.Name.text=='LOC_CIVILIZATION_JAC_ODA' then assert(row.Action.text=='LOC_JAC_UNIFICATION_PACT_ACCEPT' and not row.Action.disabled); row.Action.click() end end; assert(#REQUESTS==1)");
    let accept: Json = acceptor
        .0
        .from_value(
            acceptor
                .0
                .load("return REQUESTS[1]")
                .eval::<Value>()
                .unwrap(),
        )
        .unwrap();
    assert_eq!(accept["actor"], 3);
    assert_eq!(accept["params"]["OnStart"], "JAC_UnificationPactAccept");
    p.a.dispatch(&accept);
    p.b.dispatch(&accept);
    p.same();
    acceptor.update(&p.b.state());
    acceptor.run("assert(not Controls.LeaveButton.hidden); Controls.LeaveButton.click(); assert(#REQUESTS==2)");
    let leave: Json = acceptor
        .0
        .from_value(
            acceptor
                .0
                .load("return REQUESTS[2]")
                .eval::<Value>()
                .unwrap(),
        )
        .unwrap();
    assert_eq!(leave["actor"], 3);
    assert_eq!(leave["params"]["OnStart"], "JAC_UnificationPactLeave");
    p.a.dispatch(&leave);
    p.b.dispatch(&leave);
    p.same();
    p.check("GPROPS.JAC_UNIFICATION_PACT_LEAD_0==nil and GPROPS.JAC_UNIFICATION_PACT_LEAD_3==nil");
}

#[test]
fn war_drops_both_non_leads_before_city_victory_check_after_reload() {
    let mut p = Pair::new(false);
    p.both("ALLIED['0-1']=true");
    p.request(0, "Propose", 1);
    p.request(0, "Propose", 3);
    p.request(3, "Accept", 0);
    p.reload(false);
    p.both("WAR['1-3']=true; FIRE('CityBuilt',0,166,6,5)");
    p.check("GPROPS.JAC_UNIFICATION_PACT_LEAD_0==nil and GPROPS.JAC_UNIFICATION_PACT_LEAD_1==nil and GPROPS.JAC_UNIFICATION_PACT_LEAD_3==nil and PPROPS['1'].JAC_UNIFICATION_PACT_RESULT=='AT_WAR' and PPROPS['3'].JAC_UNIFICATION_PACT_RESULT=='AT_WAR' and #BUILT==0");
}

#[test]
fn start_alliance_counts_only_while_its_saved_property_is_numeric() {
    for active in [true, false] {
        let mut p = Pair::new(false);
        p.both(&format!(
            "GPROPS['JAC_1534_ALLIANCE_DATE-ODA']={};",
            if active { "16603" } else { "false" }
        ));
        p.reload(false);
        p.both("FIRE('OnGameTurnStarted',TURN)");
        p.check(if active {
            "CITIES[166].buildings[8]==true and CITIES[185].buildings[8]==nil"
        } else {
            "#BUILT==0"
        });
    }
}

#[test]
fn no_castle_map_tracks_saved_original_homes_and_counts_eliminated_owner() {
    let mut p = Pair::new(true);
    p.request(0, "Propose", 3);
    p.request(3, "Accept", 0);
    p.check("GPROPS.JAC_UNIFICATION_HOME_CAPITAL_0==166 and GPROPS.JAC_UNIFICATION_HOME_CAPITAL_1==185 and #BUILT==0");
    p.reload(true);
    p.both("CITIES[185].owner=3; CITIES[185].capital=false; Players[1].dead=true; FIRE('CityConquered',3,1,185,25,5)");
    p.check("#BUILT==0");
    p.both("CITIES[336].owner=3; CITIES[336].capital=false; Players[2].dead=true; FIRE('CityConquered',3,2,336,16,10)");
    p.check("CITIES[166].buildings[8]==true and CITIES[99].buildings[8]==nil");
    p.reload(true);
    p.both("FIRE('OnGameTurnStarted',TURN)");
    p.check("#BUILT==0 and CITIES[166].buildings[8]==true");
}

#[test]
fn hotseat_switch_opens_same_proposer_for_the_new_accepting_house() {
    let p = Pair::new(false);
    p.request(1, "Propose", 0);
    p.request(1, "Propose", 3);
    let panel = Panel::new(&p.a.state(), 0);
    panel.run("UI_EVENT('LoadScreenClose')");
    assert!(panel.shown());
    panel.run("Controls.PactPanel:SetHide(true); UI_EVENT('GameCoreEventPublishComplete')");
    assert!(!panel.shown());
    panel.run("LOCAL_PLAYER=3; UI_EVENT('LocalPlayerTurnBegin')");
    assert!(
        panel.shown(),
        "a different human with the same proposer still needs its accept panel"
    );
    let envelope = panel.request("Accept", 1);
    assert_eq!(envelope["actor"], 3);
    p.a.dispatch(&envelope);
    p.b.dispatch(&envelope);
    p.same();
    p.check("GPROPS.JAC_UNIFICATION_PACT_LEAD_3==1 and GPROPS.JAC_UNIFICATION_PACT_LEAD_0==nil");
}
