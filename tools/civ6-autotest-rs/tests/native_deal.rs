use civ6_autotest_rs::{carrier_transport as transport, repo_root};
use mlua::Lua;
use std::fs;
#[path = "support/native_inventory_world.rs"]
mod inventory_world;
use inventory_world::{loaded, receive, saved};

fn setup(lua: &Lua, artifact: bool) {
    lua.globals().set("ARTIFACT", artifact).unwrap();
    lua.load(r#"
      local e=Game:GetProperty('TEST_DEAL_ENGINE')
      if not e then
        e={Calls=0,Item=nil,Works={
          [900]={Owner=1,X=4,Building=ARTIFACT and 1 or 0,Slot=0,Creator=1,Kind=4},
          [901]={Owner=RECIPIENT,X=RECIPIENT==2 and 12 or 16,Building=1,Slot=0,Creator=RECIPIENT,Kind=5}}}
        Game:SetProperty('TEST_DEAL_ENGINE',e)
      end
      function ENGINE() return Game:GetProperty('TEST_DEAL_ENGINE') end
      CITIES[4].palace=not ARTIFACT
      GameInfo.GreatWorks[4].Index=4
      GameInfo.GreatWorks[4].GreatWorkObjectType=ARTIFACT and 'GREATWORKOBJECT_ARTIFACT' or 'GREATWORKOBJECT_RELIC'
      GameInfo.GreatWorks[5]={Index=5,GreatWorkType='GREATWORK_EXISTING',GreatWorkObjectType=GameInfo.GreatWorks[4].GreatWorkObjectType,Tourism=2}
      GameInfo.GreatWorks.GREATWORK_EXISTING=GameInfo.GreatWorks[5]
      GameInfo.GreatWorkSlotTypes={[1]={GreatWorkSlotType='GREATWORKSLOT_TEST'}}
      GameInfo.GreatWork_ValidSubTypes=function()
        local done=false;return function() if not done then done=true;return {
          GreatWorkObjectType=GameInfo.GreatWorks[4].GreatWorkObjectType,GreatWorkSlotType='GREATWORKSLOT_TEST'} end end
      end
      function BUILDINGS(c,ui)
        local b={HasBuilding=function(_,id) return id==1 and c.monument or id==0 and c.palace==true end,
          IsPillaged=function() return false end}
        if ui or DIRECT_CORE then
          b.GetNumGreatWorkSlots=function(_,building) return building==0 and 1 or ARTIFACT and 3 or 2 end
          b.GetGreatWorkSlotType=function() return 1 end
          b.GetGreatWorkInSlot=function(_,building,slot)
            for index,w in pairs(ENGINE().Works) do if w.Owner==c.owner and w.X==c.x and w.Building==building and w.Slot==slot then return index end end
            return -1
          end
          b.GetGreatWorkTypeFromIndex=function(_,index) return assert(ENGINE().Works[index]).Kind end
          b.GetCreatorNameFromIndex=function(_,index) return 'creator '..assert(ENGINE().Works[index]).Creator end
          b.GetTurnFromIndex=function() return 3 end
        end
        return b
      end
      for _,c in pairs(CITIES) do c.GetBuildings=function() return BUILDINGS(c,false) end end
      if DIRECT_CORE then Game.GetGreatWorkPlayer=function(index) return ENGINE().Works[index].Creator end end
      DealDirection={OUTGOING=1};DealItemTypes={GREATWORK=1}
      local function item()
        return {SetSubType=function(_,v) ENGINE().Subtype=v end,SetValueType=function(_,v) ENGINE().Item=v end,
          GetValueType=function() return ENGINE().Item end}
      end
      local working={
        GetItemCount=function() return ENGINE().Item and 1 or 0 end,
        FindItemsByType=function(_,_,subtype) return ENGINE().Item and ENGINE().Subtype==subtype and {item()} or {} end,
        AddItemOfType=function() return item() end,Validate=function() end,IsValid=function() return not ENGINE().Invalid end}
      DealManager={GetWorkingDeal=function() return working end,
        ClearWorkingDeal=function() ENGINE().Item=nil end,
        GetPossibleDealItems=function() return {{ForType=900,IsValid=not ENGINE().Invalid}} end,
        EnactWorkingDeal=function(from,to)
          local e=ENGINE();e.Calls=e.Calls+1
          local tx=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID]
          assert(tx.NativeWorks[1].MPDeal.Issued and from==1 and to==RECIPIENT)
          if e.ThrowBefore then error('before native mutation') end
          if not e.NoEffect then e.Works[900].Owner=to;e.Works[900].X=to==2 and 12 or 16;e.Works[900].Building=1;e.Works[900].Slot=1 end
          if e.ThrowAfter then error('after native mutation') end
          return not e.NoEffect
        end}
      if ARTIFACT then
        PLAN.NativeWorks={[900]={Outcome='RETURN',ReturnOwner=RECIPIENT}}
        PLAN.To=RECIPIENT==2 and 3 or 2
        GameInfo.JAC_AbsorptionOutcomes=function()
          local list={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='RETURN',Destination='RETURN'}};local i=0;return function() i=i+1;return list[i] end
        end
      end
      function PUBLISH_DEAL()
        SENT={};local oldGame=Game;local oldBuildings={}
        for x,c in pairs(CITIES) do oldBuildings[x]=c.GetBuildings;c.GetBuildings=function() return BUILDINGS(c,true) end end
        Game={GetLocalPlayer=function() return LOCAL end,GetCurrentGameTurn=function() return CURRENT_TURN end,
          GetProperty=function(_,key) return oldGame:GetProperty(key) end,
          GetGreatWorkPlayer=function(index) return ENGINE().Works[index].Creator end}
        Events.GameCoreEventPublishComplete.Emit()
        Game=oldGame;for x,c in pairs(CITIES) do c.GetBuildings=oldBuildings[x] end
        local packets={};for _,p in ipairs(SENT) do packets[#packets+1]=JAC_NativeInventoryPacket(p) end
        return packets
      end
    "#).exec().unwrap();
    lua.load(transport::deal_binding()).exec().unwrap();
}
fn world(local: i64, recipient: i64, artifact: bool) -> Lua {
    let lua = inventory_world::world(local, recipient);
    setup(&lua, artifact);
    lua
}
fn packets(lua: &Lua) -> Vec<String> {
    lua.load("return PUBLISH_DEAL()").eval().unwrap()
}
fn step(a: &Lua, b: &Lua) {
    let p = packets(a);
    assert!(!p.is_empty(), "no pending deal observation");
    for raw in p {
        for lua in [a, b] {
            assert!(receive(lua, &raw));
            let counters = lua
                .load("return tostring(ENGINE().Calls)..':'..TRANSFERS..':'..MOVES")
                .eval::<String>()
                .unwrap();
            // An unconfirmed observation advances only its read nonce. Its
            // duplicate is then stale; accepted receipts or stale rejection
            // must both leave native effects unchanged.
            let _ = receive(lua, &raw);
            assert_eq!(
                counters,
                lua.load("return tostring(ENGINE().Calls)..':'..TRANSFERS..':'..MOVES")
                    .eval::<String>()
                    .unwrap()
            );
        }
    }
}
fn complete(a: &Lua, b: &Lua) {
    for _ in 0..12 {
        if a.load(
            "return JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID].Status=='COMPLETE'",
        )
        .eval::<bool>()
        .unwrap()
        {
            break;
        }
        step(a, b);
    }
    for lua in [a, b] {
        lua.load(
            r#"
          local tx=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID]
          assert(tx.Status=='COMPLETE',tx.Status..' '..tostring(tx.Error)..' '..tostring(DIAG))
          local d=tx.NativeWorks[1].MPDeal;assert(d.Complete and d.Issued and d.FinalSlot.Slot==1)
          assert(ENGINE().Calls==1 and MOVES==0 and TRANSFERS==2)
          assert(ENGINE().Works[901].Slot==0 and ENGINE().Works[900].Owner==RECIPIENT)
        "#,
        )
        .exec()
        .unwrap();
    }
}
#[test]
fn original_palace_and_original_artifact_use_one_core_deal_without_move_or_city_loan() {
    for recipient in [2, 3] {
        for artifact in [false, true] {
            let a = world(2, recipient, artifact);
            let b = world(3, recipient, artifact);
            for lua in [&a, &b] {
                lua.load("JAC_AbsorptionGameplay.Apply(PLAN)")
                    .exec()
                    .unwrap();
            }
            complete(&a, &b);
        }
    }
}
#[test]
fn issued_unknown_result_never_reissues_and_save_rejoin_accepts_later_physical_observation() {
    let a = world(2, 2, false);
    let b = world(3, 2, false);
    for lua in [&a, &b] {
        lua.load("ENGINE().NoEffect=true;JAC_AbsorptionGameplay.Apply(PLAN)")
            .exec()
            .unwrap();
    }
    step(&a, &b);
    step(&a, &b);
    for lua in [&a, &b] {
        lua.load("assert(ENGINE().Calls==1 and TRANSFERS==0 and MOVES==0)")
            .exec()
            .unwrap();
    }
    let data = saved(&b);
    drop(b);
    let b = loaded(3, 2, &data);
    setup(&b, false);
    for _ in 0..3 {
        step(&a, &b);
    }
    for lua in [&a, &b] {
        lua.load("assert(ENGINE().Calls==1 and TRANSFERS==0);ENGINE().Works[900].Owner=2;ENGINE().Works[900].X=12;ENGINE().Works[900].Building=1;ENGINE().Works[900].Slot=1").exec().unwrap();
    }
    complete(&a, &b);
}
#[test]
fn original_deal_rejects_retrade_changed_snapshot_and_foreign_sender_before_native_effect() {
    for attack in ["ENGINE().Works[900].Creator=3",
        "Players[1]:SetProperty('JAC_ABSORPTION_ASSETS_V1',{['native-work:900']={ID='native-work:900',Kind='ITEM',Owner=1,NativeOwnerMoves=1,NativeWork={Index=900}}})",
        "ENGINE().Invalid=true",
        "local s=JAC_AbsorptionGameplay.GetState();s.Transactions.old={From=1,To=2,Status='COMPLETE',SourceRosterMoved=true,NativeWorks={{Source={Index=900},Destination=2,DealSubmitted=true}}};Game:SetProperty('JAC_ABSORPTION_STATE_V1',s)"] {
        let a=world(2,2,false);a.load(attack).exec().unwrap();
        a.load("pcall(JAC_AbsorptionGameplay.Apply,PLAN)").exec().unwrap();
        for _ in 0..3 { let ps=packets(&a);if ps.is_empty() {break;}for raw in ps {let _=receive(&a,&raw);} }
        a.load("assert(ENGINE().Calls==0 and TRANSFERS==0 and MOVES==0)").exec().unwrap();
    }
    let a = world(2, 2, false);
    a.load("JAC_AbsorptionGameplay.Apply(PLAN)").exec().unwrap();
    let p = packets(&a).remove(0);
    a.globals().set("RAW", p.clone()).unwrap();
    a.load(
        "assert(not RECEIVE(3,JAC_NativeInventoryCodec.Decode(RAW)));CITIES[4].pop=CITIES[4].pop+1",
    )
    .exec()
    .unwrap();
    assert!(!receive(&a, &p));
    a.load("assert(ENGINE().Calls==0 and TRANSFERS==0)")
        .exec()
        .unwrap();
}
#[test]
fn post_issue_missing_replaced_or_displaced_existing_works_never_prove_success() {
    for attack in [
        "ENGINE().Works[901]=nil",
        "ENGINE().Works[903]=ENGINE().Works[901];ENGINE().Works[901]=nil",
        "ENGINE().Works[901].Slot=1;ENGINE().Works[900].Slot=0",
        "ENGINE().Works[900].Creator=3",
    ] {
        let a = world(2, 2, false);
        let b = world(3, 2, false);
        for lua in [&a, &b] {
            lua.load("JAC_AbsorptionGameplay.Apply(PLAN)")
                .exec()
                .unwrap();
        }
        step(&a, &b);
        step(&a, &b);
        for lua in [&a, &b] {
            lua.load(attack).exec().unwrap();
        }
        step(&a, &b);
        for lua in [&a, &b] {
            lua.load("local tx=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID];assert(tx.Status~='COMPLETE' and not tx.NativeWorks[1].PhysicalMoved and ENGINE().Calls==1 and TRANSFERS==0)").exec().unwrap();
        }
    }
}
#[test]
fn generated_original_deal_binding_matches_and_core_builder_is_not_exposed_to_ui() {
    assert_eq!(
        fs::read_to_string(repo_root().join("poc/JAC_PoC_Absorption/Scripts/JAC_NativeDeal.lua"))
            .unwrap(),
        transport::deal_binding()
    );
    Lua::new()
        .load(transport::deal_binding())
        .into_function()
        .unwrap();
    let lua = world(2, 2, false);
    lua.load("assert(type(JAC_AbsorptionGameplay.BuildNativeWorkDealCore)=='function' and ExposedMembers.JAC_Absorption.BuildNativeWorkDealCore==nil and ExposedMembers.JAC_NativeDeal==nil)").exec().unwrap();
}

#[test]
fn interrupted_native_call_and_stale_saved_source_do_not_reissue() {
    for flag in ["ThrowBefore", "ThrowAfter"] {
        let a = world(2, 2, false);
        let b = world(3, 2, false);
        for lua in [&a, &b] {
            lua.load(format!(
                "ENGINE().{flag}=true;JAC_AbsorptionGameplay.Apply(PLAN)"
            ))
            .exec()
            .unwrap();
        }
        step(&a, &b);
        step(&a, &b);
        if flag == "ThrowAfter" {
            complete(&a, &b);
        } else {
            for _ in 0..3 {
                step(&a, &b);
            }
            for lua in [&a, &b] {
                lua.load("assert(ENGINE().Calls==1 and TRANSFERS==0 and MOVES==0)")
                    .exec()
                    .unwrap();
            }
        }
    }
    let a = world(2, 2, false);
    let b = world(3, 2, false);
    for lua in [&a, &b] {
        lua.load("JAC_AbsorptionGameplay.Apply(PLAN)")
            .exec()
            .unwrap();
    }
    step(&a, &b);
    step(&a, &b);
    for lua in [&a, &b] {
        lua.load("local s=JAC_AbsorptionGameplay.GetState();s.Transactions[PLAN.ID].NativeWorks[1].Source.CreatorName='changed saved snapshot';Game:SetProperty('JAC_ABSORPTION_STATE_V1',s)").exec().unwrap();
    }
    step(&a, &b);
    for lua in [&a, &b] {
        lua.load("local tx=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID];assert(tx.Status=='BLOCKED' and not tx.NativeWorks[1].PhysicalMoved and ENGINE().Calls==1 and TRANSFERS==0)").exec().unwrap();
    }
}

#[test]
fn original_last_capital_uses_no_loan_and_full_artifact_museum_is_not_swapped() {
    let a = world(2, 2, false);
    let b = world(3, 2, false);
    for lua in [&a, &b] {
        lua.load("Players[1].cities[8]=nil;CITIES[8].owner=0;Players[0].cities[8]=CITIES[8];PLOTS[8].owner=0;PLOTS[9].owner=0;JAC_AbsorptionGameplay.Apply(PLAN)").exec().unwrap();
    }
    for _ in 0..12 {
        if a.load(
            "return JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID].Status=='COMPLETE'",
        )
        .eval::<bool>()
        .unwrap()
        {
            break;
        }
        step(&a, &b);
    }
    for lua in [&a, &b] {
        lua.load("local tx=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID];assert(tx.Status=='COMPLETE' and ENGINE().Calls==1 and TRANSFERS==1 and MOVES==0 and tx.NativeWorks[1].Carrier==nil)").exec().unwrap();
    }
    let a = world(2, 2, true);
    a.load("for i=1,2 do ENGINE().Works[901+i]={Owner=2,X=12,Building=1,Slot=i,Creator=2,Kind=5} end;JAC_AbsorptionGameplay.Apply(PLAN)").exec().unwrap();
    for _ in 0..4 {
        let ps = packets(&a);
        if ps.is_empty() {
            break;
        }
        for raw in ps {
            let _ = receive(&a, &raw);
        }
    }
    a.load("assert(ENGINE().Calls==0 and TRANSFERS==0 and MOVES==0)")
        .exec()
        .unwrap();
}

#[test]
fn next_turn_manual_contract_reconciles_issued_deal_only_without_new_city_absorption() {
    let a = world(2, 2, false);
    let b = world(3, 2, false);
    for lua in [&a, &b] {
        lua.load("ENGINE().NoEffect=true;JAC_AbsorptionGameplay.Apply(PLAN)")
            .exec()
            .unwrap();
    }
    step(&a, &b);
    step(&a, &b);
    for lua in [&a, &b] {
        lua.load("CURRENT_TURN=6;JAC_NativeInventory.Refresh();ENGINE().Works[900].Owner=2;ENGINE().Works[900].X=12;ENGINE().Works[900].Building=1;ENGINE().Works[900].Slot=1").exec().unwrap();
    }
    step(&a, &b);
    for lua in [&a, &b] {
        lua.load("local tx=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID];assert(tx.NativeWorks[1].PhysicalMoved and tx.WaitingFor=='MP_DEAL_CONTINUATION_ONLY' and tx.Status=='WAITING_UI' and ENGINE().Calls==1 and TRANSFERS==0 and MOVES==0)").exec().unwrap();
    }
}

#[test]
fn direct_core_inventory_can_prove_original_deal_without_any_ui_publication() {
    let lua = inventory_world::world(2, 2);
    lua.load("DIRECT_CORE=true").exec().unwrap();
    setup(&lua, false);
    lua.load("local tx=JAC_AbsorptionGameplay.Apply(PLAN);assert(tx.Status=='COMPLETE' and ENGINE().Calls==1 and TRANSFERS==2 and MOVES==0 and #SENT==0);assert(tx.NativeWorks[1].MPDeal.BeforeEvidenceKind=='DIRECT_CORE' and tx.NativeWorks[1].MPDeal.AfterEvidenceKind=='DIRECT_CORE')").exec().unwrap();
}
