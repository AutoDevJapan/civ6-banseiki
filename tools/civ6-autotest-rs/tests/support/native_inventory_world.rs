// Shared fixture: individual integration binaries use different helper subsets.
#![allow(dead_code)]
use civ6_autotest_rs::{carrier_transport as transport, repo_root};
use mlua::{Function, Lua};
use std::fs;

pub fn world(local: i64, recipient: i64) -> Lua {
    let lua = Lua::new();
    lua.load(include_str!(
        "../../../civ6-autotest-native/tests/fixtures/absorption_world.lua"
    ))
    .call::<()>(lua.create_table().unwrap())
    .unwrap();
    lua.load("function include(_) end").exec().unwrap();
    for name in ["JAC_Absorption_Rules.lua", "JAC_Absorption_Gameplay.lua"] {
        lua.load(
            fs::read_to_string(
                repo_root()
                    .join("poc/JAC_PoC_Absorption/Scripts")
                    .join(name),
            )
            .unwrap(),
        )
        .exec()
        .unwrap();
    }
    lua.globals().set("LOCAL", local).unwrap();
    lua.globals().set("RECIPIENT", recipient).unwrap();
    lua.load(r#"
      GameConfiguration.IsAnyMultiplayer=function() return true end
      print=function(s) DIAG=s end
      Game.GetLocalPlayer=function() error('Gameplay local player read') end
      Players[2].IsTurnActive=function() return true end
      Players[1].units={};Players[1]:SetProperty('JAC_ABSORPTION_ASSETS_V1',{})
      Players[2]:SetProperty('JAC_ABSORPTION_ASSETS_V1',{});Players[3]:SetProperty('JAC_ABSORPTION_ASSETS_V1',{})
      GameInfo.GreatWorks={[4]={GreatWorkType='GREATWORK_TEST',GreatWorkObjectType='GREATWORKOBJECT_RELIC',Tourism=4}}
      GameInfo.GreatWorks.GREATWORK_TEST=GameInfo.GreatWorks[4]
      GameInfo.GreatWork_YieldChanges=function() return function() return nil end end
      Game.GetGreatWorkPlayer=nil -- Core has no getter; UI supplies this value
      CORE_BUILDINGS={};UI_CITY_READERS={}
      for x,c in pairs(CITIES) do
        UI_CITY_READERS[x]={Religion=c:GetReligion(),Districts=c:GetDistricts(),Identity=c:GetCulturalIdentity()}
        local b={HasBuilding=function(_,i) return i==1 and c.monument end,IsPillaged=function() return false end}
        CORE_BUILDINGS[x]=b;c.GetBuildings=function() return b end
      end
      PLAN={ID='native-city',From=1,To=RECIPIENT,Mode='MERGER',EntryConditionsMet=true,
        RelationsResolved=true,Leader='RECRUIT',Units={},Assets={},NativeWorks={}}
      GameInfo.JAC_AbsorptionOutcomes=function()
        local list={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
          {Kind='ITEM',Outcome='TRANSFER',Destination='ABSORBER'}};local i=0
        return function() i=i+1;return list[i] end
      end
      GameEvents.JAC_NativeInventory_Operation={Add=function(fn) RECEIVE=fn end}
      GameEvents.JAC_CarrierBoundary_Operation={Add=function() end}
      GameEvents.JAC_CarrierMP_Operation={Add=function() end}
      SENT={};MOVES=0
      PlayerOperations={EXECUTE_SCRIPT='script',MOVE_GREAT_WORK='move'}
      UI={RequestPlayerOperation=function(actor,op,p)
        if op=='move' then MOVES=MOVES+1;error('CITY must never send individual MOVE') end
        assert(actor==LOCAL and op=='script');SENT[#SENT+1]=p
        if SEND_THROW then error('send outcome unknown') end;return true
      end}
      local function event() local callbacks={};return {Add=function(fn) callbacks[#callbacks+1]=fn end,
        Emit=function() for _,fn in ipairs(callbacks) do fn() end end} end
      Events={GameCoreEventPublishComplete=event(),LoadScreenClose=event(),LocalPlayerTurnBegin=event()}
      GameEvents.PlayerTurnStartComplete=event();GameEvents.OnPlayerTurnEnded=event()
      ExposedMembers.JAC_HeroReadAPI={ReadCity=function() error('local UI fallback in Gameplay') end,
        ReadNativeWork=function() error('local native metadata fallback in Gameplay') end,
        GetObservation=function() error('local publication flag in Gameplay') end}
      UI_POS=4
      function PUBLISH()
        local oldPlayers,oldGame=Players,Game
        local uiPlayers={}
        for id,p in pairs(oldPlayers) do
          uiPlayers[id]={IsHuman=function() return p:IsHuman() end,IsTurnActive=function() return true end,
            GetCities=function() return {FindID=function(_,cityID)
              if UI_MISSING then return nil end
              for _,c in pairs(p.cities) do if c.id==cityID then
                local b={HasBuilding=function(_,i) return i==1 and c.monument end,
                  GetNumGreatWorkSlots=function() return GW_SLOT_COUNT or 1 end,
                  GetGreatWorkInSlot=function(_,_,slot) return c.x==UI_POS and slot==0 and 900 or -1 end,
                  GetGreatWorkTypeFromIndex=function() return 4 end,
                  GetCreatorNameFromIndex=function() return 'original creator' end,
                  GetTurnFromIndex=function() return 3 end}
                return {GetX=function() return c.x end,GetY=function() return 0 end,GetOwner=function() return c.owner end,
                  GetName=function() return c:GetName() end,GetPopulation=function() return c.pop end,
                  GetProperty=function(_,k) return c:GetProperty(k) end,GetBuildings=function() return b end,
                  GetReligion=function() return UI_CITY_READERS[c.x].Religion end,GetDistricts=function() return UI_CITY_READERS[c.x].Districts end,
                  GetCulturalIdentity=function() return UI_CITY_READERS[c.x].Identity end}
              end end
            end} end}
        end
        Players=uiPlayers;Game={GetLocalPlayer=function() return LOCAL end,
          GetProperty=function(_,key) return oldGame:GetProperty(key) end,
          GetCurrentGameTurn=function() return CURRENT_TURN end,GetGreatWorkPlayer=function() return 1 end}
        Events.GameCoreEventPublishComplete.Emit()
        Players=oldPlayers;Game=oldGame
        if #SENT==0 then return nil end
        return JAC_NativeInventoryPacket(SENT[#SENT])
      end
    "#).exec().unwrap();
    lua.load(transport::gameplay_binding()).exec().unwrap();
    lua.load(transport::inventory_binding()).exec().unwrap();
    lua.load("JAC_NativeInventoryPacket=function(p) return JAC_NativeInventoryCodec.Encode(p) end")
        .exec()
        .unwrap();
    lua.load(transport::inventory_ui_binding()).exec().unwrap();
    lua
}

pub fn receive(lua: &Lua, packet: &str) -> bool {
    receive_as(lua, packet, 2)
}
pub fn receive_as(lua: &Lua, packet: &str, actor: i64) -> bool {
    lua.load("return RECEIVE")
        .eval::<Function>()
        .unwrap()
        .call((
            actor,
            lua.load("return JAC_NativeInventoryCodec.Decode")
                .eval::<Function>()
                .unwrap()
                .call::<mlua::Value>(packet)
                .unwrap(),
        ))
        .unwrap()
}

pub fn saved(lua: &Lua) -> String {
    lua.load(
        r#"
      local s={Game=Game.props,Players={},Units={},Cities={},Plots={},Turn=CURRENT_TURN,Transfers=TRANSFERS,
        NextID=NEXT_ID,Spawns=SPAWNS,Kills=KILLS}
      for id,p in pairs(Players) do
        s.Players[id]=p.props;s.Units[id]={}
        for unitID,u in pairs(p.units) do
          local data={};for k,v in pairs(u) do if type(v)~='function' then data[k]=v end end
          s.Units[id][unitID]=data
        end
      end
      for x,c in pairs(CITIES) do s.Cities[x]={Owner=c.owner,ID=c.id,Props=c.props,Population=c.pop,
        Loyalty=c.loyalty,Faith=c.faith,Monument=c.monument,Districts=c.districts} end
      for x,p in pairs(PLOTS) do s.Plots[x]={Owner=p.owner,City=p.city} end
      return JAC_NativeInventoryCodec.Encode(s)
    "#,
    )
    .eval()
    .unwrap()
}
pub fn loaded(local: i64, recipient: i64, data: &str) -> Lua {
    let lua = world(local, recipient);
    lua.globals().set("SAVE", data).unwrap();
    lua.load(r#"
      local s=JAC_NativeInventoryCodec.Decode(SAVE);Game.props=s.Game;CURRENT_TURN=s.Turn;TRANSFERS=s.Transfers
      for id,p in pairs(Players) do p.props=s.Players[id];p.cities={};p.units={} end
      for owner,units in pairs(s.Units) do for unitID,data in pairs(units) do
        local u=UNIT(owner,unitID,GameInfo.Units[data.kind].UnitType,data.x)
        for k,v in pairs(data) do u[k]=v end
      end end
      NEXT_ID=s.NextID;SPAWNS=s.Spawns;KILLS=s.Kills
      for x,d in pairs(s.Cities) do local c=CITIES[x];c.owner=d.Owner;c.id=d.ID;c.props=d.Props;c.pop=d.Population
        c.loyalty=d.Loyalty;c.faith=d.Faith;c.monument=d.Monument;c.districts=d.Districts
        Players[c.owner].cities[c.id]=c end
      for x,d in pairs(s.Plots) do PLOTS[x].owner=d.Owner;PLOTS[x].city=d.City end
      SAVE=nil
    "#).exec().unwrap();
    lua
}

pub fn publish(lua: &Lua) -> String {
    lua.load("return PUBLISH()").eval::<String>().unwrap()
}
pub fn publish_parts(lua: &Lua) -> Vec<String> {
    lua.load("SENT={};PUBLISH();local packets={};for _,p in ipairs(SENT) do packets[#packets+1]=JAC_NativeInventoryPacket(p) end;return packets").eval().unwrap()
}
