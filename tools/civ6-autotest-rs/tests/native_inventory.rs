use civ6_autotest_rs::{carrier_transport as transport, repo_root};
use mlua::{Function, Lua};
use std::fs;

fn world(local: i64, recipient: i64) -> Lua {
    let lua = Lua::new();
    lua.load(include_str!(
        "../../civ6-autotest-native/tests/fixtures/absorption_world.lua"
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

fn receive(lua: &Lua, packet: &str) -> bool {
    receive_as(lua, packet, 2)
}
fn receive_as(lua: &Lua, packet: &str, actor: i64) -> bool {
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

fn saved(lua: &Lua) -> String {
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
fn loaded(local: i64, recipient: i64, data: &str) -> Lua {
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

#[test]
fn save_reconstruction_rejoin_and_lost_send_replay_preserve_one_city_transfer() {
    let lua = world(2, 3);
    lua.load("JAC_AbsorptionGameplay.Apply(PLAN);UI_MISSING=true;assert(PUBLISH()==nil and #SENT==0 and TRANSFERS==0)").exec().unwrap();
    lua.load("UI_MISSING=false;SEND_THROW=true").exec().unwrap();
    let initial = publish(&lua);
    assert!(receive(&lua, &initial));
    lua.load("assert(TRANSFERS==2 and MOVES==0)")
        .exec()
        .unwrap();
    let data = saved(&lua);
    drop(lua);
    let a = loaded(2, 3, &data);
    let b = loaded(3, 3, &data);
    b.load("UI_POS=8;UI_MISSING=true").exec().unwrap();
    for peer in [&a, &b] {
        assert!(receive(peer, &initial));
        peer.load("assert(TRANSFERS==2 and MOVES==0)")
            .exec()
            .unwrap();
    }
    let final_observation = publish(&a);
    for peer in [&b, &a] {
        assert!(receive(peer, &final_observation));
        assert!(receive(peer, &final_observation));
        peer.load("local tx=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID];assert(tx and tx.Status=='COMPLETE' and TRANSFERS==2 and MOVES==0,tostring(tx and tx.Status)..' '..tostring(tx and tx.Error)..' '..tostring(DIAG))").exec().unwrap();
    }
    let rejoined = loaded(2, 3, &saved(&a));
    assert!(receive(&rejoined, &final_observation));
    rejoined.load("assert(TRANSFERS==2 and MOVES==0);local p=ExposedMembers.JAC_NativeInventory.GetPending();assert(#p==0)").exec().unwrap();
}

#[test]
fn public_turn_events_refresh_fixed_observer_after_shared_human_roster_change() {
    let a = world(2, 3);
    let b = world(2, 3);
    for peer in [&a, &b] {
        peer.load("JAC_AbsorptionGameplay.Apply(PLAN)")
            .exec()
            .unwrap();
    }
    let stale = publish(&a);
    for peer in [&a, &b] {
        peer.load("Players[2].IsTurnActive=function() return false end;Players[0].IsHuman=function() return true end;Players[0].IsTurnActive=function() return true end;CURRENT_TURN=6;GameEvents.PlayerTurnStartComplete.Emit(0);LOCAL=0").exec().unwrap();
        assert!(!receive(peer, &stale));
    }
    let current = publish(&a);
    for peer in [&a, &b] {
        assert!(receive_as(peer, &current, 0));
        // Manual, non-Flow preparation cannot turn this into a new borrow on a later turn.
        peer.load("assert(TRANSFERS==0 and MOVES==0);local q=Game:GetProperty('JAC_NATIVE_INVENTORY_V1').Requests['native-city:SOURCE_PREPARE'];assert(q.To==3 and not Players[3]:IsHuman() and q.Actor==0 and q.Receipt.Actor==0)").exec().unwrap();
    }
}

#[test]
fn turn_activity_difference_never_changes_saved_provider_or_nonce() {
    let a = world(2, 0);
    let b = world(3, 0);
    for lua in [&a, &b] {
        lua.load("Players[3].IsHuman=function() return true end;Players[3].IsTurnActive=function() return true end").exec().unwrap();
    }
    b.load("Players[2].IsTurnActive=function() return false end")
        .exec()
        .unwrap();
    for lua in [&a, &b] {
        lua.load("JAC_AbsorptionGameplay.Apply(PLAN);GameEvents.PlayerTurnStartComplete.Emit(3);local q=ExposedMembers.JAC_NativeInventory.GetPending()[1];assert(q.Actor==2 and q.Nonce==1);q.Actor=3;assert(ExposedMembers.JAC_NativeInventory.GetPending()[1].Actor==2)").exec().unwrap();
    }
    let snapshot = |lua: &Lua| {
        lua.load(
            "return JAC_NativeInventoryCodec.Encode(Game:GetProperty('JAC_NATIVE_INVENTORY_V1'))",
        )
        .eval::<String>()
        .unwrap()
    };
    assert_eq!(snapshot(&a), snapshot(&b));
    b.load("assert(PUBLISH()==nil and #SENT==0 and TRANSFERS==0)")
        .exec()
        .unwrap();
    let packet = publish(&a);
    for lua in [&a, &b] {
        assert!(!receive_as(lua, &packet, 3));
    }
}

#[test]
fn ui_only_city_loyalty_and_district_reads_are_synchronized_before_and_after_change() {
    let a = world(2, 3);
    let b = world(3, 3);
    for lua in [&a, &b] {
        lua.load(
            r#"
          PLAN.Mode='CONQUEST';PLAN.FlowPolicy=true
          for _,c in pairs(CITIES) do
            c.GetCulturalIdentity=function() return {} end
            c.GetReligion=function() return {} end
            c.GetDistricts=function() return {} end
          end
          JAC_AbsorptionGameplay.Apply(PLAN)
        "#,
        )
        .exec()
        .unwrap();
    }
    b.load("UI_POS=8").exec().unwrap();
    for _ in 0..12 {
        if a.load(
            "local tx=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID];assert(tx,tostring(DIAG));return tx.Status=='COMPLETE'",
        )
        .eval::<bool>()
        .unwrap()
        {
            break;
        }
        let packet = publish(&a);
        for lua in [&b, &a] {
            assert!(receive(lua, &packet));
        }
    }
    for lua in [&a, &b] {
        lua.load("local tx=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID];assert(tx.Status=='COMPLETE' and TRANSFERS==2 and MOVES==0,tostring(tx.Error)..' '..tostring(DIAG));assert(CITIES[4].loyalty==72 and CITIES[8].loyalty==72)").exec().unwrap();
    }
    let data = saved(&b);
    drop(b);
    let b = loaded(3, 3, &data);
    for lua in [&a, &b] {
        lua.load("for _,c in pairs(CITIES) do c.GetCulturalIdentity=function() return {} end;c.GetReligion=function() return {} end;c.GetDistricts=function() return {} end end;CURRENT_TURN=6;JAC_AbsorptionGameplay.ApplyCityFriction()").exec().unwrap();
    }
    for _ in 0..8 {
        if a.load("return CITIES[4]:GetProperty('JAC_ABSORPTION_GOVERNANCE_V1').FrictionApplied==1 and CITIES[8]:GetProperty('JAC_ABSORPTION_GOVERNANCE_V1').FrictionApplied==1")
            .eval::<bool>()
            .unwrap()
        {
            break;
        }
        let packet = publish(&a);
        for lua in [&b, &a] {
            assert!(receive(lua, &packet));
        }
    }
    for lua in [&a, &b] {
        lua.load("assert(CITIES[4].loyalty==70 and CITIES[8].loyalty==70 and TRANSFERS==2 and MOVES==0);for _,x in ipairs({4,8}) do local g=CITIES[x]:GetProperty('JAC_ABSORPTION_GOVERNANCE_V1');assert(g.FrictionApplied==1 and not g.FrictionPending) end;JAC_AbsorptionGameplay.ApplyCityFriction();assert(CITIES[4].loyalty==70 and CITIES[8].loyalty==70)").exec().unwrap();
    }
}

#[test]
fn core_getters_win_and_reported_slot_definition_property_tampering_is_rejected() {
    for attack in [
        "o.Cities[1].Properties.forged=1",
        "o.Cities[1].Reads.LOYALTY=0",
        "o.Cities[1].Reads.SLOTS[77]=1;o.Cities[1].Reads.WORK[77]={[0]=900}",
        "o.Works[1].Effects={'YIELD_FAITH 999'}",
        "o.Works[2]=o.Works[1]",
    ] {
        let lua = world(2, 3);
        lua.load("JAC_AbsorptionGameplay.Apply(PLAN)")
            .exec()
            .unwrap();
        lua.globals().set("RAW", publish(&lua)).unwrap();
        lua.load(format!("local p=JAC_NativeInventoryCodec.Decode(RAW);local o=JAC_NativeInventoryCodec.Decode(p.Payload);{attack};p.Payload=JAC_NativeInventoryCodec.Encode(o);assert(RECEIVE(2,p)==false and TRANSFERS==0 and MOVES==0)")).exec().unwrap();
    }
    let lua = world(2, 3);
    lua.load(r#"
      Game.GetGreatWorkPlayer=function() return 1 end
      for x,b in pairs(CORE_BUILDINGS) do local position=x
        b.GetNumGreatWorkSlots=function() return 1 end
        b.GetGreatWorkInSlot=function() return position==4 and 900 or -1 end
        b.GetGreatWorkTypeFromIndex=function() return 4 end
        b.GetCreatorNameFromIndex=function() return 'original creator' end
        b.GetTurnFromIndex=function() return 3 end
      end
      UI_MISSING=true
      local tx=JAC_AbsorptionGameplay.Apply(PLAN)
      assert(tx.Status=='COMPLETE' and TRANSFERS==2 and MOVES==0 and #SENT==0)
      for _,q in pairs(Game:GetProperty('JAC_NATIVE_INVENTORY_V1').Requests) do assert(q.EvidenceKind=='DIRECT_CORE') end
    "#).exec().unwrap();
}

#[test]
fn flow_begin_and_inventory_recheck_use_shared_observations_before_rolls() {
    let a = world(2, 2);
    let b = world(3, 2);
    for lua in [&a, &b] {
        lua.load(
            fs::read_to_string(
                repo_root().join("poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Flow.lua"),
            )
            .unwrap(),
        )
        .exec()
        .unwrap();
        lua.load("RANDOM_QUEUE={9999,9999};local f=JAC_AbsorptionFlowGameplay.Begin(PLAN);assert(f.Phase=='WAITING_SNAPSHOT' and #RANDOM_CALLS==0 and TRANSFERS==0)").exec().unwrap();
    }
    b.load("UI_POS=8").exec().unwrap();
    let initial = publish(&a);
    for lua in [&b, &a] {
        assert!(receive(lua, &initial));
        lua.load(r#"
          local F=JAC_AbsorptionFlowGameplay;local f=F.Get(PLAN.ID)
          assert(f.Phase=='CHOICE' and #RANDOM_CALLS==2 and #f.Entries==2)
          for _,e in ipairs(f.Entries) do F.Select(2,PLAN.ID,e.ID,e.Kind=='ITEM' and 'TRANSFER' or 'RECRUIT') end
          local ok,err=pcall(F.Resolve,2,PLAN.ID)
          assert(not ok and err=='JAC_ABSORPTION_WAITING_UI' and #RANDOM_CALLS==2 and TRANSFERS==0)
        "#).exec().unwrap();
    }
    // Only the authenticated provider reports a work moved to another source city.
    // Both peers reject the changed frozen inventory before any final roll/transfer.
    a.load("UI_POS=8").exec().unwrap();
    let changed = publish(&a);
    for lua in [&a, &b] {
        assert!(receive(lua, &changed));
        lua.load("assert(JAC_AbsorptionFlowGameplay.Get(PLAN.ID).Phase=='BLOCKED' and #RANDOM_CALLS==2 and TRANSFERS==0 and MOVES==0)").exec().unwrap();
    }
}

#[test]
fn ai_flow_recipient_uses_human_inventory_provider_and_completes_city_inheritance() {
    let a = world(2, 3);
    let b = world(3, 3);
    for lua in [&a, &b] {
        lua.load(
            fs::read_to_string(
                repo_root().join("poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Flow.lua"),
            )
            .unwrap(),
        )
        .exec()
        .unwrap();
        lua.load(r#"
          RANDOM_QUEUE={9999,9999,0,0}
          GameInfo.JAC_AbsorptionOutcomes=function()
            local list={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
              {Kind='ITEM',Outcome='TRANSFER',Destination='ABSORBER'},
              {Kind='ITEM',Outcome='SEIZE',Destination='ABSORBER'}};local i=0
            return function() i=i+1;return list[i] end
          end
          local f=JAC_AbsorptionFlowGameplay.Begin(PLAN);assert(f.Phase=='WAITING_SNAPSHOT' and #RANDOM_CALLS==0)
        "#).exec().unwrap();
    }
    b.load("UI_POS=8").exec().unwrap();
    for _ in 0..8 {
        let complete=a.load("local tx=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID];return tx and tx.Status=='COMPLETE'").eval::<Option<bool>>().unwrap().unwrap_or(false);
        if complete {
            break;
        }
        let packet = publish(&a);
        for lua in [&b, &a] {
            assert!(receive(lua, &packet));
        }
    }
    for lua in [&a, &b] {
        lua.load(
            r#"
          local tx=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID]
          local f=JAC_AbsorptionFlowGameplay.Get(PLAN.ID)
          assert(tx and tx.Status=='COMPLETE' and TRANSFERS==2 and MOVES==0,
            'AI Flow '..tostring(f.Phase)..' '..tostring(f.Error)..' '..tostring(DIAG))
          assert(#RANDOM_CALLS==4 and tx.NativeWorks[1].Source.Index==900)
        "#,
        )
        .exec()
        .unwrap();
    }
}
fn publish(lua: &Lua) -> String {
    lua.load("return PUBLISH()").eval::<String>().unwrap()
}
fn publish_parts(lua: &Lua) -> Vec<String> {
    lua.load("SENT={};PUBLISH();local packets={};for _,p in ipairs(SENT) do packets[#packets+1]=JAC_NativeInventoryPacket(p) end;return packets").eval().unwrap()
}

#[test]
fn partial_snapshot_chunks_survive_load_reordering_and_do_not_commit_early() {
    let a = world(2, 3);
    let b = world(3, 3);
    for lua in [&a, &b] {
        lua.load("CITIES[4]:SetProperty('JAC_ABSORPTION_CITY_FAITH_V1',{Followers=4,Blob=string.rep('復',16000)});JAC_AbsorptionGameplay.Apply(PLAN)").exec().unwrap();
    }
    a.load("SEND_THROW=true").exec().unwrap();
    let interrupted = publish_parts(&a);
    assert_eq!(interrupted.len(), 1);
    for lua in [&a, &b] {
        assert!(receive(lua, &interrupted[0]));
        lua.load("assert(TRANSFERS==0)").exec().unwrap();
    }
    let save = saved(&b);
    drop(b);
    let b = loaded(3, 3, &save);
    a.load("SEND_THROW=false").exec().unwrap();
    let complete = publish_parts(&a);
    assert!(complete.len() > 2);
    for packet in complete.iter().skip(1).rev() {
        // Withhold one middle chunk: even reordered/replayed chunks cannot
        // expose an incomplete observation to preparation or CITY execution.
        if packet == &complete[1] {
            continue;
        }
        for lua in [&a, &b] {
            assert!(receive(lua, packet));
            assert!(receive(lua, packet));
            lua.load("assert(TRANSFERS==0)").exec().unwrap();
        }
    }
    for lua in [&a, &b] {
        assert!(receive(lua, &complete[1]));
        lua.load("assert(TRANSFERS==2 and MOVES==0)")
            .exec()
            .unwrap();
    }
    for _ in 0..4 {
        if a.load(
            "local tx=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID];assert(tx,tostring(DIAG));return tx.Status=='COMPLETE'",
        )
        .eval::<bool>()
        .unwrap()
        {
            break;
        }
        for packet in publish_parts(&a) {
            for lua in [&b, &a] {
                assert!(receive(lua, &packet));
            }
        }
    }
    for lua in [&a, &b] {
        lua.load("local tx=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID];assert(tx and tx.Status=='COMPLETE' and TRANSFERS==2 and MOVES==0,tostring(tx and tx.Status)..' '..tostring(tx and tx.Error)..' '..tostring(DIAG))").exec().unwrap();
    }
}

#[test]
fn generated_inventory_bindings_match_distribution() {
    for (file, expected) in [
        (
            "Scripts/JAC_NativeInventory.lua",
            transport::inventory_binding(),
        ),
        (
            "UI/JAC_NativeInventory_UI.lua",
            transport::inventory_ui_binding(),
        ),
    ] {
        assert_eq!(
            fs::read_to_string(repo_root().join("poc/JAC_PoC_Absorption").join(file)).unwrap(),
            expected
        );
        Lua::new().load(expected).into_function().unwrap();
    }
}

#[test]
fn two_vms_share_initial_and_city_observations_for_human_or_ai_recipient() {
    for recipient in [2, 3] {
        for kind in [
            "GREATWORKOBJECT_RELIC",
            "GREATWORKOBJECT_ARTIFACT",
            "GREATWORKOBJECT_SCULPTURE",
        ] {
            let left = world(2, recipient);
            let right = world(3, recipient);
            for lua in [&left, &right] {
                lua.globals().set("KIND", kind).unwrap();
                lua.load("GameInfo.GreatWorks[4].GreatWorkObjectType=KIND;GW_SLOT_COUNT=KIND=='GREATWORKOBJECT_ARTIFACT' and 3 or 1;CITIES[4]:SetProperty('JAC_ABSORPTION_CITY_FAITH_V1',{Followers=4,Precise=1.0000000000000002,[1]='numeric',['1']='string'})").exec().unwrap();
                lua.load("local tx=JAC_AbsorptionGameplay.Apply(PLAN);assert(tx.Status=='WAITING_UI' and TRANSFERS==0)").exec().unwrap();
            }
            right.load("UI_POS=8").exec().unwrap();
            let initial = publish(&left);
            for lua in [&right, &left] {
                assert!(receive(lua, &initial));
                assert!(receive(lua, &initial));
            }
            for _ in 0..4 {
                let complete=left.load("local tx=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID];assert(tx,tostring(DIAG));return tx.Status=='COMPLETE'").eval::<bool>().unwrap();
                if complete {
                    break;
                }
                let packet = publish(&left);
                for lua in [&left, &right] {
                    assert!(receive(lua, &packet));
                }
            }
            for lua in [&left, &right] {
                lua.load(r#"
              local tx=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID]
              assert(tx.Status=='COMPLETE' and TRANSFERS==2 and MOVES==0,
                tx.Status..' transfers='..TRANSFERS..' reason='..tostring(tx.Error)..' diag='..tostring(DIAG))
              assert(CITIES[4]:GetOwner()==RECIPIENT and CITIES[8]:GetOwner()==RECIPIENT)
              assert(#tx.NativeWorks==1 and tx.NativeWorks[1].Source.Index==900 and tx.NativeWorks[1].NativeRoute=='CITY')
              assert(Players[RECIPIENT]:GetProperty('JAC_ABSORPTION_ASSETS_V1')['native-work:900'].NativeWork.Index==900)
              local p=CITIES[4]:GetProperty('JAC_ABSORPTION_CITY_FAITH_V1')
              assert(p.Precise==1.0000000000000002 and p[1]=='numeric' and p['1']=='string')
            "#).exec().unwrap();
            }
        }
    }
}

#[test]
fn unauthorized_actor_and_changed_source_never_accept_snapshot() {
    let lua = world(2, 3);
    lua.load("JAC_AbsorptionGameplay.Apply(PLAN)")
        .exec()
        .unwrap();
    let raw = publish(&lua);
    lua.globals().set("PACKET", raw.clone()).unwrap();
    assert!(!lua
        .load("local p=JAC_NativeInventoryCodec.Decode(PACKET);return RECEIVE(3,p)")
        .eval::<bool>()
        .unwrap());
    lua.load("CITIES[4].pop=CITIES[4].pop+1").exec().unwrap();
    assert!(!receive(&lua, &raw));
    lua.load("assert(TRANSFERS==0 and MOVES==0)")
        .exec()
        .unwrap();
}
