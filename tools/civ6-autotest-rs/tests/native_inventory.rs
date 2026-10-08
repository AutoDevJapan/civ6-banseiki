use civ6_autotest_rs::{carrier_transport as transport, repo_root};
use mlua::Lua;
use std::fs;
#[path = "support/native_inventory_world.rs"]
mod inventory_world;
use inventory_world::{loaded, publish, publish_parts, receive, receive_as, saved, world};
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
fn single_player_turn_events_preserve_saved_mp_inventory_intents() {
    for missing in [false, true] {
        let lua = world(2, 3);
        lua.globals().set("MISSING_CONFIG", missing).unwrap();
        lua.load(r#"
          JAC_AbsorptionGameplay.Apply(PLAN)
          local before=JAC_NativeInventoryCodec.Encode(Game:GetProperty('JAC_NATIVE_INVENTORY_V1'))
          local configuration=GameConfiguration
          if MISSING_CONFIG then GameConfiguration=nil
          else GameConfiguration.IsAnyMultiplayer=function() return false end end
          CURRENT_TURN=6;GameEvents.PlayerTurnStartComplete.Emit(2)
          assert(before==JAC_NativeInventoryCodec.Encode(Game:GetProperty('JAC_NATIVE_INVENTORY_V1')))
          assert(TRANSFERS==0 and MOVES==0 and #SENT==0)
          GameConfiguration=configuration;GameConfiguration.IsAnyMultiplayer=function() return true end
          GameEvents.PlayerTurnStartComplete.Emit(2)
          assert(before~=JAC_NativeInventoryCodec.Encode(Game:GetProperty('JAC_NATIVE_INVENTORY_V1')))
        "#).exec().unwrap();
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
