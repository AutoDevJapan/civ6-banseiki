//! Runs the shipped gameplay Lua in separate VMs. These are code-level checks,
//! not a claim that two real LAN endpoints or engine synchronization were tested.
use civ6_lua_tests_rs::{new_lua, read_repo};
use mlua::{Lua, LuaSerdeExt, Value};
use serde_json::{json, Value as Json};

const SOURCE: &str = "poc/JAC_PoC_GrowthUI/Scripts/JAC_PoC_GrowthUI_Gameplay.lua";

struct Endpoint(Lua);

impl Endpoint {
    fn new(local: u8, reversed: bool, saved: Option<&Json>) -> Self {
        let lua = new_lua();
        lua.globals().set("localPlayer", local).unwrap();
        lua.globals().set("reversed", reversed).unwrap();
        lua.load(
            r#"
          local function copy(v)
            if type(v) ~= 'table' then return v end
            local r={}; for k,x in pairs(v) do r[k]=copy(x) end; return r
          end
          turn=1; props={}; unitprops={[0]={},[1]={}}; present={[0]=true,[1]=true}
          for owner=0,1 do
            unitprops[owner][65536]={}; unitprops[owner][131073]={}
          end
          Game={GetCurrentGameTurn=function() return turn end,
            GetLocalPlayer=function() return localPlayer end,
            GetProperty=function(_,key) return copy(props[key]) end,
            SetProperty=function(_,key,value) props[key]=copy(value) end}
          Players={}
          for owner=0,1 do
            local pid=owner
            Players[pid]={GetID=function() return pid end, IsHuman=function() return true end,
              GetUnits=function() return {Members=function()
                local ids=reversed and {131073,65536} or {65536,131073}
                local n=0
                return function()
                  if not present[pid] then return nil end
                  n=n+1; local id=ids[n]; if not id then return nil end
                  return id,{GetID=function() return id end,
                    GetProperty=function(_,key) return copy(unitprops[pid][id][key]) end,
                    SetProperty=function(_,key,value) unitprops[pid][id][key]=copy(value) end}
                end
              end} end}
          end
          PlayerManager={GetAliveMajors=function()
            if reversed then return {Players[1],Players[0]} end
            return {Players[0],Players[1]}
          end}
          Events={LoadScreenClose={Add=function(f) loadscreen=f end}}
          GameEvents={PlayerTurnStartComplete={Add=function(f) beginturn=f end}}
          function export()
            local units={}
            for owner=0,1 do for _,id in ipairs({65536,131073}) do
              units[#units+1]={owner=owner,id=id,properties=copy(unitprops[owner][id])}
            end end
            return {turn=turn,properties=copy(props),units=units}
          end
          function restore(save)
            turn=save.turn; props=copy(save.properties)
            for _,u in ipairs(save.units) do unitprops[u.owner][u.id]=copy(u.properties) end
          end
        "#,
        )
        .exec()
        .unwrap();
        if let Some(saved) = saved {
            lua.globals()
                .set("saved", lua.to_value(saved).unwrap())
                .unwrap();
            lua.load("restore(saved)").exec().unwrap();
        }
        lua.load(read_repo(SOURCE)).exec().unwrap();
        Self(lua)
    }

    fn exec(&self, source: &str) {
        self.0.load(source).exec().unwrap();
    }

    fn state(&self) -> Json {
        let state: Value = self.0.load("return export()").eval().unwrap();
        self.0.from_value(state).unwrap()
    }

    fn record(&self, owner: u8, id: i64) -> Json {
        let record: Value = self
            .0
            .load(format!(
                "return unitprops[{owner}][{id}].JAC_POC_GROWTHUI_WARLORD"
            ))
            .eval()
            .unwrap();
        self.0.from_value(record).unwrap()
    }
}

#[test]
fn independent_vms_agree_despite_local_player_and_enumeration_order() {
    let host = Endpoint::new(0, false, None);
    let guest = Endpoint::new(1, true, None);
    host.exec("loadscreen(); beginturn(0); beginturn(1)");
    guest.exec("loadscreen(); beginturn(1); beginturn(0)");
    assert_eq!(host.state(), guest.state());
    for owner in 0..=1 {
        let first = host.record(owner, 65536);
        let second = host.record(owner, 131073);
        assert_eq!(first["XP"], 20);
        assert_eq!(first["Level"], 1);
        assert_eq!(first["LastXPTurn"], 1);
        assert_eq!(first["Personality"], "JAC_PERSONALITY_BRAVE");
        assert_eq!(second["Personality"], "JAC_PERSONALITY_CALM");
        assert_ne!(first["Equipment"], second["Equipment"]);
    }
}

#[test]
fn duplicate_delivery_does_not_add_xp_or_touch_inactive_players() {
    let endpoint = Endpoint::new(0, false, None);
    endpoint.exec("loadscreen()");
    let inactive = endpoint.record(1, 65536);
    endpoint.exec("beginturn(0)");
    let once = endpoint.state();
    endpoint.exec("beginturn(0); beginturn(0)");
    assert_eq!(once, endpoint.state());
    assert_eq!(inactive, endpoint.record(1, 65536));
    endpoint.exec("beginturn(-1); beginturn(99)");
    assert_eq!(once, endpoint.state());
}

#[test]
fn serialized_save_and_fresh_rejoin_preserve_records_and_duplicate_guard() {
    let host = Endpoint::new(0, false, None);
    host.exec("loadscreen(); beginturn(0); beginturn(1)");
    // Cross a real serialization boundary: no Lua tables or VM are shared.
    let bytes = serde_json::to_vec(&host.state()).unwrap();
    let saved: Json = serde_json::from_slice(&bytes).unwrap();
    let restored = Endpoint::new(0, true, Some(&saved));
    let rejoined = Endpoint::new(1, false, Some(&saved));
    restored.exec("loadscreen(); beginturn(0); beginturn(1)");
    rejoined.exec("loadscreen(); beginturn(1); beginturn(0); beginturn(1)");
    assert_eq!(saved, restored.state());
    assert_eq!(saved, rejoined.state());
    restored.exec("turn=2; beginturn(0)");
    rejoined.exec("turn=2; beginturn(0); beginturn(0)");
    assert_eq!(restored.state(), rejoined.state());
    assert_eq!(restored.record(0, 65536)["Level"], 2);
    assert_eq!(restored.record(0, 65536)["XP"], 10);
    assert_eq!(restored.record(0, 65536)["LastXPTurn"], 2);
    assert_eq!(restored.record(1, 65536)["XP"], 20);
    assert_eq!(restored.record(1, 65536)["LastXPTurn"], 1);
    let second_save = restored.state();
    let second_reload = Endpoint::new(1, true, Some(&second_save));
    second_reload.exec("loadscreen(); beginturn(0)");
    assert_eq!(second_save, second_reload.state());
    second_reload.exec("beginturn(1)");
    assert_eq!(second_reload.record(1, 65536)["XP"], 10);
    assert_eq!(second_reload.record(1, 65536)["Level"], 2);
}

#[test]
fn one_ready_human_cannot_finalize_initialization_before_the_other_has_units() {
    let endpoint = Endpoint::new(0, false, None);
    endpoint.exec("present[1]=false; loadscreen(); beginturn(0)");
    assert!(endpoint.state()["properties"]["JAC_POC_GROWTHUI_INITIALIZED"].is_null());
    assert!(endpoint.record(0, 65536).is_null());
    endpoint.exec("present[1]=true; beginturn(0); beginturn(1)");
    for owner in 0..=1 {
        assert_eq!(endpoint.record(owner, 65536)["XP"], 20);
        assert_eq!(endpoint.record(owner, 131073)["XP"], 20);
    }
    assert_eq!(
        endpoint.state()["properties"]["JAC_POC_GROWTHUI_INITIALIZED"],
        true
    );
}

#[test]
fn restored_level_cap_and_all_metadata_survive_duplicate_turns() {
    let endpoint = Endpoint::new(0, false, None);
    endpoint.exec("loadscreen(); beginturn(0)");
    let mut saved = endpoint.state();
    let record = &mut saved["units"][0]["properties"]["JAC_POC_GROWTHUI_WARLORD"];
    record["Level"] = json!(30);
    record["XP"] = json!(100);
    record["LastXPTurn"] = json!(0);
    record["Equipment"] = json!(["item,b", "item-a"]);
    record["Artifact"] = json!("unique-relic");
    let rejoined = Endpoint::new(1, true, Some(&saved));
    rejoined.exec("loadscreen(); beginturn(0)");
    let once = rejoined.state();
    assert_eq!(rejoined.record(0, 65536)["Level"], 30);
    assert_eq!(rejoined.record(0, 65536)["XP"], 120);
    assert_eq!(
        rejoined.record(0, 65536)["Equipment"],
        json!(["item,b", "item-a"])
    );
    assert_eq!(rejoined.record(0, 65536)["Artifact"], "unique-relic");
    rejoined.exec("beginturn(0)");
    assert_eq!(once, rejoined.state());
}
