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
fn relay(a: &Lua, b: &Lua) {
    let ps = packets(a);
    assert!(!ps.is_empty());
    for p in ps {
        for lua in [a, b] {
            assert!(receive(lua, &p));
            let _ = receive(lua, &p);
        }
    }
}
fn unknown(a: &Lua) {
    a.load("pcall(JAC_AbsorptionGameplay.Apply,PLAN)")
        .exec()
        .unwrap();
    for _ in 0..4 {
        let ps = packets(a);
        if ps.is_empty() {
            break;
        }
        for p in ps {
            let _ = receive(a, &p);
        }
    }
    a.load("assert(ENGINE().Calls==0 and TRANSFERS==0 and MOVES==0,'unknown history reached native enactment')").exec().unwrap();
}
// These fixtures reconstruct a potentially already-issued Version 1 journal.
// They authorize no new trade and never assert/mint an Original certificate.
fn seed_legacy(a: &Lua, b: &Lua) {
    for lua in [a, b] {
        lua.load(r#"
          JAC_NativeInventory.Tick=function() end
          function SEED_LEGACY()
            return JAC_NativeInventory.With(PLAN,'TEST_LEGACY_CAPTURE',function()
              local before=JAC_NativeInventory.Snapshot();local source
              for _,w in ipairs(before.Works) do if w.Index==900 then source=w end end
              assert(source);local x=RECIPIENT==2 and 12 or 16;local c=CITIES[x]
              local snap={ID='native-work:900',NativeRoute='DEAL',Source=JAC_AbsorptionRules.Copy(source),Destination=RECIPIENT,
                Custodian=RECIPIENT,DealSubmitted=true,MPDeal={Version=1,Issued=true,From=1,To=RECIPIENT,
                  Source=JAC_AbsorptionRules.Copy(source),Before=before,IssuedTurn=CURRENT_TURN,
                  EmptySlots={{Owner=RECIPIENT,CityID=c.id,City=c.id,X=x,Y=0,Building=1,Slot=1,SlotType='GREATWORKSLOT_TEST'}}}}
              local s=JAC_AbsorptionGameplay.GetState()
              s.Transactions[PLAN.ID]={ID=PLAN.ID,From=1,To=PLAN.To,Turn=CURRENT_TURN,Status='WAITING_UI',Request=PLAN,
                SourceRoster={},NativeWorks={snap},Cities={}}
              Game:SetProperty('JAC_ABSORPTION_STATE_V1',s)
              ENGINE().Calls=1 -- saved historical counter, never EnactWorkingDeal
            end)
          end
          local ok,err=pcall(SEED_LEGACY);assert(not ok and err=='JAC_ABSORPTION_WAITING_UI')
        "#).exec().unwrap();
    }
    relay(a, b);
    for lua in [a, b] {
        lua.load("SEED_LEGACY()").exec().unwrap();
    }
}
fn advance(a: &Lua) -> bool {
    a.load(
        r#"
      local s=JAC_AbsorptionGameplay.GetState();local tx=s.Transactions[PLAN.ID]
      local ok,value=pcall(JAC_NativeDeal.Advance,s,tx,tx.NativeWorks[1])
      DIAG=not ok and tostring(value) or nil
      return ok and value==true
    "#,
    )
    .eval()
    .unwrap()
}
fn observe(a: &Lua, b: &Lua) -> [bool; 2] {
    assert!(!advance(a));
    assert!(!advance(b));
    relay(a, b);
    [advance(a), advance(b)]
}
fn moved(lua: &Lua) {
    lua.load("ENGINE().Works[900].Owner=RECIPIENT;ENGINE().Works[900].X=RECIPIENT==2 and 12 or 16;ENGINE().Works[900].Building=1;ENGINE().Works[900].Slot=1").exec().unwrap();
}
#[test]
fn unknown_vanilla_roundtrip_or_periodic_initial_certificate_never_enacts() {
    for artifact in [false, true] {
        for roundtrip in [false, true] {
            let a = world(2, 2, artifact);
            if roundtrip {
                a.load("ENGINE().Works[900].Owner=3;ENGINE().Works[900].X=16;ENGINE().Works[900].Owner=1;ENGINE().Works[900].X=4").exec().unwrap();
            }
            a.load("Players[1]:SetProperty('JAC_ABSORPTION_ASSETS_V1',{['native-work:900']={ID='native-work:900',Kind='ITEM',Owner=1,NativeOwnerMoves=0,NativeWork={Index=900},InitialObservation={Owner=1,Creator=1,Turn=CURRENT_TURN}}})").exec().unwrap();
            unknown(&a);
        }
    }
}
#[test]
fn direct_core_current_inventory_and_valid_deal_are_not_complete_history() {
    let a = inventory_world::world(2, 2);
    a.load("DIRECT_CORE=true").exec().unwrap();
    setup(&a, false);
    unknown(&a);
    a.load("assert(JAC_NativeDeal.Original({}, {},1,2,2,PLAN.ID)==false)")
        .exec()
        .unwrap();
}
#[test]
fn after_issued_snapshot_other_new_work_and_legal_rearrangement_are_allowed() {
    for artifact in [false, true] {
        let a = world(2, 2, artifact);
        let b = world(3, 2, artifact);
        seed_legacy(&a, &b);
        for lua in [&a, &b] {
            moved(lua);
            lua.load("ENGINE().Works[901].Owner=3;ENGINE().Works[901].X=16;ENGINE().Works[903]={Owner=1,X=4,Building=ARTIFACT and 1 or 0,Slot=0,Creator=1,Kind=5};CURRENT_TURN=6;JAC_NativeInventory.Refresh()").exec().unwrap();
        }
        assert_eq!(observe(&a, &b), [true, true]);
        for lua in [&a, &b] {
            lua.load("assert(ENGINE().Calls==1 and TRANSFERS==0 and MOVES==0);local d=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID].NativeWorks[1].MPDeal;assert(d.Complete and d.FinalSlot.Slot==1)").exec().unwrap();
        }
    }
}
#[test]
fn already_issued_unknown_exception_survives_save_and_timeout_without_enactment() {
    let a = world(2, 2, false);
    let b = world(3, 2, false);
    seed_legacy(&a, &b);
    for lua in [&a, &b] {
        lua.load("local s=JAC_AbsorptionGameplay.GetState();s.Transactions[PLAN.ID].NativeWorks[1].MPDeal.CallError='native call outcome unknown';Game:SetProperty('JAC_ABSORPTION_STATE_V1',s)").exec().unwrap();
    }
    assert_eq!(observe(&a, &b), [false, false]);
    let data = saved(&b);
    drop(b);
    let b = loaded(3, 2, &data);
    setup(&b, false);
    b.load("JAC_NativeInventory.Tick=function() end")
        .exec()
        .unwrap();
    for lua in [&a, &b] {
        lua.load("CURRENT_TURN=100;JAC_NativeInventory.Refresh()")
            .exec()
            .unwrap();
    }
    assert_eq!(observe(&a, &b), [false, false]);
    for lua in [&a, &b] {
        moved(lua);
    }
    assert_eq!(observe(&a, &b), [true, true]);
    for lua in [&a, &b] {
        lua.load("assert(ENGINE().Calls==1 and TRANSFERS==0 and MOVES==0)")
            .exec()
            .unwrap();
    }
}
#[test]
fn missing_replaced_duplicate_or_changed_provenance_preserves_issued_journal() {
    for attack in [
        "ENGINE().Works[901]=nil",
        "ENGINE().Works[903]=ENGINE().Works[901];ENGINE().Works[901]=nil",
        "ENGINE().Works[901].Creator=3",
        "ENGINE().Works[900].Creator=3",
    ] {
        let a = world(2, 2, false);
        let b = world(3, 2, false);
        seed_legacy(&a, &b);
        for lua in [&a, &b] {
            moved(lua);
            lua.load(attack).exec().unwrap();
        }
        assert_eq!(observe(&a, &b), [false, false]);
        for lua in [&a, &b] {
            lua.load("assert(ENGINE().Calls==1 and TRANSFERS==0 and MOVES==0);assert(not JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID].NativeWorks[1].PhysicalMoved)").exec().unwrap();
        }
    }
    let a = world(2, 2, false);
    let b = world(3, 2, false);
    seed_legacy(&a, &b);
    for lua in [&a, &b] {
        moved(lua);
        lua.load("BUILDINGS_ORIGINAL=BUILDINGS;BUILDINGS=function(c,ui) local out=BUILDINGS_ORIGINAL(c,ui);if out.GetGreatWorkInSlot then local old=out.GetGreatWorkInSlot;out.GetGreatWorkInSlot=function(self,building,slot) if c.x==4 and building==0 and slot==0 then return 900 end;return old(self,building,slot) end end;return out end").exec().unwrap();
        assert!(!advance(lua));
    }
    for p in packets(&a) {
        for lua in [&a, &b] {
            assert!(!receive(lua, &p));
        }
    }
    for lua in [&a, &b] {
        lua.load("assert(ENGINE().Calls==1 and TRANSFERS==0)")
            .exec()
            .unwrap();
    }
}
#[test]
fn changed_saved_source_and_unrelated_city_rebuild_are_rejected() {
    for mutation in ["local s=JAC_AbsorptionGameplay.GetState();s.Transactions[PLAN.ID].NativeWorks[1].Source.CreatorName='forged';Game:SetProperty('JAC_ABSORPTION_STATE_V1',s)",
        "CITIES[4].id=CITIES[4].id+5000",
        "local s=JAC_AbsorptionGameplay.GetState();s.Transactions[PLAN.ID].NativeWorks[1].MPDeal.EmptySlots[1].Slot=0;Game:SetProperty('JAC_ABSORPTION_STATE_V1',s);ENGINE().Works[901].X=16;ENGINE().Works[901].Owner=3;ENGINE().Works[900].Slot=0"] {
        let a=world(2,2,false);let b=world(3,2,false);seed_legacy(&a,&b);
        for lua in [&a,&b] {moved(lua);lua.load(mutation).exec().unwrap();}
        if mutation.contains("CreatorName='forged'") {assert!(!advance(&a));assert!(!advance(&b));} else {assert_eq!(observe(&a,&b),[false,false]);}
    }
}
#[test]
fn issued_deal_accepts_source_identity_change_only_with_completed_native_city_journal() {
    for inherited in [false, true] {
        let a = world(2, 2, false);
        let b = world(3, 2, false);
        seed_legacy(&a, &b);
        for lua in [&a, &b] {
            moved(lua);
            lua.globals().set("INHERITED", inherited).unwrap();
            lua.load(r#"
              CITIES[4].id=2004;local s=JAC_AbsorptionGameplay.GetState();local tx=s.Transactions[PLAN.ID]
              if INHERITED then
                Players[1].cities[4]=nil;Players[tx.To].cities[4]=CITIES[4]
                CITIES[4].owner=tx.To;CITIES[4]:SetProperty('JAC_ABSORPTION_GOVERNANCE_V1',{ID=tx.ID,From=1,To=tx.To})
                tx.Cities={{X=4,Y=0,TransferTurn=5,TransferredCityID=2004}}
              else
                tx.NativeWorks[2]={Carrier={MP={Loans={{Owner=1,ID=4,X=4,Y=0,Issued=true,ReturnIssued=true,FinalID=2004,
                  RestoredCore={Owner=1,ID=2004,X=4,Y=0}}}}}}
              end
              Game:SetProperty('JAC_ABSORPTION_STATE_V1',s)
            "#).exec().unwrap();
        }
        assert_eq!(observe(&a, &b), [true, true]);
    }
}
#[test]
fn generated_binding_matches_and_no_new_core_enactment_or_ui_mutator_exists() {
    assert_eq!(
        fs::read_to_string(repo_root().join("poc/JAC_PoC_Absorption/Scripts/JAC_NativeDeal.lua"))
            .unwrap(),
        transport::deal_binding()
    );
    Lua::new()
        .load(transport::deal_binding())
        .into_function()
        .unwrap();
    assert!(!transport::deal_binding().contains("pcall(DealManager.EnactWorkingDeal"));
    let lua = world(2, 2, false);
    lua.load("assert(ExposedMembers.JAC_NativeDeal==nil and ExposedMembers.JAC_Absorption.BuildNativeWorkDealCore==nil)").exec().unwrap();
}
