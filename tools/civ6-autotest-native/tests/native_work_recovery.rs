use mlua::Lua;
use std::{fs, path::PathBuf};

fn load(lua: &Lua, name: &str) -> mlua::Result<()> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../poc/JAC_PoC_Absorption/Scripts")
        .join(name);
    lua.load(fs::read_to_string(path).map_err(mlua::Error::external)?)
        .set_name(name)
        .exec()
}

fn setup() -> mlua::Result<Lua> {
    let lua = Lua::new();
    lua.load(include_str!("fixtures/absorption_world.lua"))
        .call::<()>(lua.create_table()?)?;
    lua.load("function include(_) end").exec()?;
    load(&lua, "JAC_Absorption_Rules.lua")?;
    load(&lua, "JAC_Absorption_Gameplay.lua")?;
    lua.load(
        r#"
        WORK_CITIES={[4]=true}
        CREATOR='Original creator'
        CityManager.TransferCity(CITIES[16],2)
        local cities=Players[2].GetCities
        Players[2].GetCities=function(self)
          local collection=cities(self)
          collection.GetCapitalCity=function() return CITIES[12] end
          return collection
        end
        GameInfo.GreatWorks={[4]={GreatWorkType='GREATWORK_TEST_RELIC',
          GreatWorkObjectType='GREATWORKOBJECT_RELIC',Tourism=8}}
        ExposedMembers.JAC_HeroReadAPI={ReadNativeWork=function()
          return {Type=4,CreatorName=CREATOR,TurnCreated=2,CreatedByPlayer=0}
        end}
        for _,city in pairs(CITIES) do
          local original=city.GetBuildings
          city.GetBuildings=function(self)
            local buildings=original(self)
            buildings.GetGreatWorkInSlot=function(_,building,slot)
              return WORK_CITIES[city:GetX()] and building==1 and slot==0 and 900 or -1
            end
            return buildings
          end
        end
        A=JAC_AbsorptionGameplay or ExposedMembers.JAC_Absorption
        local source=assert(A.OwnedGreatWorkSlots(1)[1])
        local state=A.GetState()
        state.Archive.native={ID='native',Kind='ITEM',Owner=-1,
          Type='GREATWORKOBJECT_RELIC',NativeWork=source}
        Game:SetProperty(A.Rules.STATE_KEY,state)
        ARCHIVE_BEFORE=A.Canonical(A.GetState().Archive)
        function recover() return A.RecoverArchived('native',2,'recovery:native',nil) end
        function assert_retained()
          assert(A.GetRoster(2).native==nil,'native work was awarded without a physical move')
          assert(A.Canonical(A.GetState().Archive)==ARCHIVE_BEFORE,'original archive changed')
          assert(A.GetState().Recoveries['recovery:native'].Status~='COMPLETE')
        end
        "#,
    )
    .exec()?;
    Ok(lua)
}

#[test]
fn recovery_rejects_native_work_still_held_by_another_player() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load("assert(not pcall(recover));assert_retained()")
        .exec()
}

#[test]
fn native_location_scan_detects_missing_and_duplicate_original_indexes() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        local located=A.NativeWorkLocations(900)
        assert(#located==1 and located[1].Owner==1 and located[1].CityID==CITIES[4]:GetID())
        WORK_CITIES={}
        assert(#A.NativeWorkLocations(900)==0,'missing original index was invented')
        WORK_CITIES={[4]=true,[16]=true}
        located=A.NativeWorkLocations(900)
        assert(#located==2,'duplicate original index across owners was hidden')
        "#,
    )
    .exec()
}

#[test]
fn ordinary_city_transfer_requires_one_original_work_in_its_saved_slot() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        Players[1].units={}
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,{})
        GameInfo.JAC_AbsorptionOutcomes=function()
          local rows={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='TRANSFER',Destination='ABSORBER'}}
          local i=0;return function() i=i+1;return rows[i] end
        end
        PLAN={ID='native-ordinary',From=1,To=2,Mode='MERGER',
          EntryConditionsMet=true,RelationsResolved=true,Leader='RECRUIT'}
        WORK_CITIES={[4]=true,[16]=true}
        local accepted,reason=pcall(A.Apply,PLAN)
        assert(not accepted and tostring(reason):find('not unique at the donor source',1,true),
          'ordinary native transfer allowed a duplicate index in another faction: '..tostring(reason))
        assert(CITIES[4]:GetOwner()==1 and A.GetState().Transactions[PLAN.ID]==nil)
        WORK_CITIES={[4]=true}
        local tx=A.Apply(PLAN)
        if tx.Status=='WAITING_UI' then
          ExposedMembers.JAC_HeroReadAPI.GetObservation=function() return {Published=true} end
          tx=A.Resume(PLAN.ID)
        end
        assert(tx.Status=='COMPLETE','ordinary native transfer did not finish')
        assert(A.Verify(PLAN.ID,true))
        local record=assert(A.GetRoster(2)['native-work:900'])
        local current=assert(CityManager.GetCityAt(4,0))
        assert(record.NativeWork.Owner==2 and record.NativeWork.CityID==current:GetID()
          and record.NativeCustody.Owner==2 and record.NativeCustody.City==current:GetID()
          and record.NativeCustody.Building==1 and record.NativeCustody.Slot==0
          and record.City.Owner==2 and record.City.X==4,
          'ordinary native transfer retained its previous custody location')
        local rosterBefore=A.Canonical(A.GetRoster(2))
        local interrupted=A.GetState()
        interrupted.Transactions[PLAN.ID].Status='BLOCKED'
        interrupted.Transactions[PLAN.ID].Error='interrupted after roster commit'
        Game:SetProperty(A.Rules.STATE_KEY,interrupted)
        assert(A.Resume(PLAN.ID).Status=='COMPLETE',
          'saved final verification could not resume a committed native roster')
        assert(A.Canonical(A.GetRoster(2))==rosterBefore and #A.NativeWorkLocations(900)==1,
          'saved final verification changed the native work or roster')
        WORK_CITIES={[12]=true}
        local same,why=pcall(A.Verify,PLAN.ID,false)
        assert(not same and tostring(why):find('changed its saved physical slot',1,true),
          'later-turn verification accepted the original work in another city')
        "#,
    )
    .exec()
}

#[test]
fn palace_work_uses_a_verified_deal_before_its_city_can_change_owner() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        local palace={Index=1,BuildingType='BUILDING_PALACE'}
        GameInfo.Buildings=setmetatable({BUILDING_PALACE=palace,[1]=palace},
          {__call=function()
            local read=false;return function()
              if not read then read=true;return palace end
            end
          end})
        GameInfo.GreatWorks[4].Index=4
        GameInfo.GreatWorks.GREATWORK_TEST_RELIC=GameInfo.GreatWorks[4]
        GameInfo.GreatWork_ValidSubTypes=function()
          local read=false;return function()
            if not read then read=true;return {GreatWorkObjectType='GREATWORKOBJECT_RELIC',
              GreatWorkSlotType='GREATWORKSLOT_RELIC'} end
          end
        end
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function(owner,object)
          assert(owner==2 and object=='GREATWORKOBJECT_RELIC')
          return {{Owner=2,City=CITIES[12]:GetID(),X=12,Y=0,Building=1,Slot=0,
            SlotType='GREATWORKSLOT_RELIC'}}
        end
        ExposedMembers.JAC_HeroReadAPI.GetObservation=function() return {Published=true} end
        DealDirection={OUTGOING=1}
        DealItemTypes={GREATWORK=2}
        local deal={items={}}
        function deal:GetItemCount() return #self.items end
        function deal:AddItemOfType(kind,from)
          assert(kind==2 and from==1)
          local item={}
          function item:SetSubType(value) assert(value==4) end
          function item:SetValueType(value) assert(value==900) end
          self.items[#self.items+1]=item
          return item
        end
        function deal:Validate() return 1 end
        function deal:IsValid() return #self.items==1 end
        TRANSFERS=0
        ENACT_CALLS=0
        DealManager={
          GetWorkingDeal=function(direction,from,to)
            assert(direction==1 and from==1 and to==2);return deal
          end,
          GetPossibleDealItems=function(from,to,kind,working)
            assert(from==1 and to==2 and kind==2 and working==deal)
            return {{ForType=900,IsValid=true}}
          end,
          EnactWorkingDeal=function(from,to)
            assert(from==1 and to==2 and WORK_CITIES[4] and not WORK_CITIES[12])
            assert(TRANSFERS==0 and CITIES[4]:GetOwner()==1,
              'donor Palace city changed before its work was moved')
            ENACT_CALLS=ENACT_CALLS+1
            WORK_CITIES[4],WORK_CITIES[12]=nil,true
            return 1
          end}
        Players[1].units={}
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,{})
        GameInfo.JAC_AbsorptionOutcomes=function()
          local rows={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='TRANSFER',Destination='ABSORBER'}}
          local i=0;return function() i=i+1;return rows[i] end
        end
        local plan={ID='native-palace-escrow',From=1,To=2,Mode='MERGER',
          EntryConditionsMet=true,RelationsResolved=true,Leader='RECRUIT'}
        local first=A.Apply(plan)
        assert(first.Status=='WAITING_UI' and ENACT_CALLS==1 and TRANSFERS==0)
        local snap=assert(first.NativeWorks[1])
        assert(snap.Source.BuildingType=='BUILDING_PALACE' and snap.Custodian==2
          and snap.TargetSlot.City==CITIES[12]:GetID() and snap.DealSubmitted)
        local complete=A.Resume(plan.ID)
        assert(complete.Status=='COMPLETE' and ENACT_CALLS==1)
        assert(#A.NativeWorkLocations(900)==1 and A.NativeWorkLocations(900)[1].Owner==2)
        local record=assert(A.GetRoster(2)['native-work:900'])
        assert(record.NativeWork.Index==900 and record.NativeCustody.Owner==2
          and record.NativeCustody.City==CITIES[12]:GetID()
          and record.NativeCustody.Building==1 and record.NativeCustody.Slot==0)
        assert(A.Verify(plan.ID,true))
        "#,
    )
    .exec()
}

#[test]
fn two_original_works_to_one_custodian_clear_only_the_completed_first_item() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        CityManager.TransferCity(CITIES[16],3)
        TRANSFERS=0
        SECOND_SOURCE=true
        SECOND_TARGET=false
        local sourceBuildings=CITIES[8].GetBuildings
        CITIES[8].GetBuildings=function(self)
          local b=sourceBuildings(self)
          local old=b.GetGreatWorkInSlot
          b.GetGreatWorkInSlot=function(obj,building,slot)
            if building==1 and slot==0 then return SECOND_SOURCE and 901 or -1 end
            return old(obj,building,slot)
          end
          return b
        end
        local targetBuildings=CITIES[12].GetBuildings
        CITIES[12].GetBuildings=function(self)
          local b=targetBuildings(self)
          local oldSlots=b.GetNumGreatWorkSlots
          local oldWork=b.GetGreatWorkInSlot
          b.GetNumGreatWorkSlots=function(obj,building)
            if building==1 then return 2 end
            return oldSlots(obj,building)
          end
          b.GetGreatWorkInSlot=function(obj,building,slot)
            if building==1 and slot==1 then return SECOND_TARGET and 901 or -1 end
            return oldWork(obj,building,slot)
          end
          return b
        end
        GameInfo.GreatWorks[4].Index=4
        GameInfo.GreatWorks.GREATWORK_TEST_RELIC=GameInfo.GreatWorks[4]
        GameInfo.GreatWork_ValidSubTypes=function()
          local read=false;return function()
            if not read then read=true;return {GreatWorkObjectType='GREATWORKOBJECT_RELIC',
              GreatWorkSlotType='GREATWORKSLOT_RELIC'} end
          end
        end
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function(owner,object)
          assert(owner==2 and object=='GREATWORKOBJECT_RELIC')
          local slots={}
          for slot=0,1 do
            local empty=slot==0 and not WORK_CITIES[12] or slot==1 and not SECOND_TARGET
            if empty then slots[#slots+1]={Owner=2,City=CITIES[12]:GetID(),X=12,Y=0,
              Building=1,Slot=slot,SlotType='GREATWORKSLOT_RELIC'} end
          end
          return slots
        end
        ExposedMembers.JAC_HeroReadAPI.GetObservation=function() return {Published=true} end
        DealDirection={OUTGOING=1}
        DealItemTypes={GREATWORK=2}
        local deal={items={}}
        function deal:GetItemCount() return #self.items end
        function deal:AddItemOfType(kind,from)
          assert(kind==2 and from==1)
          local item={}
          function item:SetSubType(value) assert(value==4) end
          function item:SetValueType(value) self.value=value end
          function item:GetValueType() return self.value end
          self.items[#self.items+1]=item
          return item
        end
        function deal:FindItemsByType(kind,subtype,from)
          assert(kind==2 and subtype==4 and from==1)
          return self.items
        end
        function deal:Validate() return 1 end
        function deal:IsValid() return #self.items==1 end
        ENACT_CALLS,CLEAR_CALLS=0,0
        DealManager={
          GetWorkingDeal=function(direction,from,to)
            assert(direction==1 and from==1 and to==2);return deal
          end,
          GetPossibleDealItems=function(from,to,kind,working)
            assert(from==1 and to==2 and kind==2 and working==deal)
            local possible={}
            if WORK_CITIES[4] then possible[#possible+1]={ForType=900,IsValid=true} end
            if SECOND_SOURCE then possible[#possible+1]={ForType=901,IsValid=true} end
            return possible
          end,
          ClearWorkingDeal=function(direction,from,to)
            assert(direction==1 and from==1 and to==2 and #deal.items==1
              and deal.items[1]:GetValueType()==900 and not WORK_CITIES[4],
              'an unrelated or unmoved working item was cleared')
            CLEAR_CALLS=CLEAR_CALLS+1
            deal.items={}
          end,
          EnactWorkingDeal=function(from,to)
            assert(from==1 and to==2 and #deal.items==1 and TRANSFERS==0,
              'native works were not moved before donor cities')
            local index=deal.items[1]:GetValueType()
            ENACT_CALLS=ENACT_CALLS+1
            if index==900 then
              assert(WORK_CITIES[4] and not WORK_CITIES[12])
              WORK_CITIES[4],WORK_CITIES[12]=nil,true
            elseif index==901 then
              assert(SECOND_SOURCE and not SECOND_TARGET)
              SECOND_SOURCE,SECOND_TARGET=false,true
            else error('unplanned native work index') end
            return 1
          end}
        Players[1].units={}
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,{})
        GameInfo.JAC_AbsorptionOutcomes=function()
          local rows={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='RETURN',Destination='RETURN'}}
          local i=0;return function() i=i+1;return rows[i] end
        end
        local plan={ID='two-native-works',From=1,To=3,Mode='MERGER',
          EntryConditionsMet=true,RelationsResolved=true,Leader='RECRUIT',
          NativeWorks={[900]={Outcome='RETURN',ReturnOwner=2},
            [901]={Outcome='RETURN',ReturnOwner=2}}}
        assert(A.Apply(plan).Status=='WAITING_UI' and ENACT_CALLS==1 and CLEAR_CALLS==0)
        deal.items[1].value=777
        local accepted,reason=pcall(A.Resume,plan.ID)
        assert(not accepted and tostring(reason):find('working deal differs from saved intent',1,true),
          'unrelated working item was treated as the completed first work')
        assert(CLEAR_CALLS==0 and ENACT_CALLS==1 and TRANSFERS==0,
          'unrelated working item was cleared or donor cities were changed')
        deal.items[1].value=900
        assert(A.Resume(plan.ID).Status=='WAITING_UI' and ENACT_CALLS==2 and CLEAR_CALLS==1,
          'second work did not clear exactly the first completed item')
        local done=A.Resume(plan.ID)
        assert(done.Status=='COMPLETE' and ENACT_CALLS==2 and CLEAR_CALLS==1)
        assert(CITIES[4]:GetOwner()==3 and CITIES[8]:GetOwner()==3)
        local first=A.NativeWorkLocations(900)
        local second=A.NativeWorkLocations(901)
        assert(#first==1 and #second==1 and first[1].Owner==2 and second[1].Owner==2
          and first[1].CityID==CITIES[12]:GetID() and second[1].CityID==CITIES[12]:GetID()
          and first[1].Slot==0 and second[1].Slot==1)
        assert(A.GetRoster(2)['native-work:900'].NativeCustody.Slot==0
          and A.GetRoster(2)['native-work:901'].NativeCustody.Slot==1)
        assert(A.Verify(plan.ID,true))
        "#,
    )
    .exec()
}

#[test]
fn external_native_work_deal_precedes_donor_city_mutation_and_reconciles_before_retry()
-> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        CityManager.TransferCity(CITIES[16],3)
        TRANSFERS=0
        GameInfo.GreatWorks.GREATWORK_TEST_RELIC=GameInfo.GreatWorks[4]
        GameInfo.GreatWorks[4].Index=4
        GameInfo.GreatWork_ValidSubTypes=function()
          local once=false
          return function()
            if not once then
              once=true
              return {GreatWorkObjectType='GREATWORKOBJECT_RELIC',
                GreatWorkSlotType='GREATWORKSLOT_RELIC'}
            end
          end
        end
        PlayerManager.GetFreeCitiesPlayerID=function() return 3 end
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function(owner,object)
          assert(owner==3 and object=='GREATWORKOBJECT_RELIC')
          return {{Owner=3,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0,
            SlotType='GREATWORKSLOT_RELIC'}}
        end
        DealDirection={OUTGOING=1}
        DealItemTypes={GREATWORK=2}
        ENACT_CALLS=0
        local deal={items={}}
        function deal:GetItemCount() return #self.items end
        function deal:AddItemOfType(kind,from)
          assert(kind==2 and from==1)
          local item={}
          function item:SetSubType(value) assert(value==4);self.subtype=value end
          function item:SetValueType(value) assert(value==900);self.value=value end
          function item:GetValueType() return self.value end
          self.items[#self.items+1]=item
          return item
        end
        function deal:FindItemsByType(kind,subtype,from)
          assert(kind==2 and subtype==4 and from==1)
          return self.items
        end
        function deal:Validate() return 1 end
        function deal:IsValid() return #self.items==1 end
        DealManager={
          GetWorkingDeal=function(direction,from,to)
            assert(direction==1 and from==1 and to==3)
            return deal
          end,
          GetPossibleDealItems=function(from,to,kind,working)
            assert(from==1 and to==3 and kind==2 and working==deal)
            return {{ForType=900,IsValid=true}}
          end,
          EnactWorkingDeal=function(from,to)
            assert(from==1 and to==3 and WORK_CITIES[4] and not WORK_CITIES[16])
            ENACT_CALLS=ENACT_CALLS+1
            if ENACT_CALLS==2 then WORK_CITIES[4],WORK_CITIES[16]=nil,true end
            return 1
          end}
        Players[1].units={}
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,{})
        GameInfo.JAC_AbsorptionOutcomes=function()
          local rows={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='RETURN',Destination='RETURN'}}
          local i=0;return function() i=i+1;return rows[i] end
        end
        PLAN={ID='native-external',From=1,To=2,Mode='MERGER',FlowPolicy=true,
          EntryConditionsMet=true,RelationsResolved=true,Leader='RECRUIT',
          NativeWorks={[900]={Outcome='RETURN',ReturnOwner=3}}}
        local published=ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function() return {} end
        local waiting=A.Apply(PLAN)
        assert(waiting.Status=='WAITING_UI' and waiting.NativeWorks[1].Source.Index==900
          and waiting.NativeWorks[1].Custodian==3 and waiting.NativeWorks[1].TargetSlot==nil,
          'missing slot lost the saved work location or move intent')
        assert(ENACT_CALLS==0 and TRANSFERS==0 and KILLS==0 and WORK_CITIES[4],
          'missing external slot mutated donor before Prepare completed')
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=published
        CURRENT_TURN=CURRENT_TURN+1 -- the product Flow keeps this frozen intent across turns
        local tx=A.Resume(PLAN.ID)
        assert(tx.Status=='WAITING_UI' and ENACT_CALLS==1)
        assert(CITIES[4]:GetOwner()==1 and TRANSFERS==0 and KILLS==0,
          'donor was mutated before native work proof')
        local saved=assert(A.GetState().Transactions[PLAN.ID])
        assert(saved.NativeWorks[1].DealSubmitted and not saved.NativeWorks[1].PhysicalMoved)
        assert(saved.NativeDealUI and saved.NativeDealUI.Index==900)
        local token=saved.NativeDealUI.Token
        assert(A.Resume(PLAN.ID).Status=='WAITING_UI' and ENACT_CALLS==1,
          'unpublished work triggered a duplicate deal')
        ExposedMembers.JAC_HeroReadAPI.GetObservation=function(value)
          if value==token then return {Published=true} end
        end
        assert(A.Resume(PLAN.ID).Status=='WAITING_UI' and ENACT_CALLS==2,
          'published source still at origin did not retry the unchanged work')
        assert(CITIES[4]:GetOwner()==1 and TRANSFERS==0 and KILLS==0)
        local second=assert(A.GetState().Transactions[PLAN.ID].NativeDealUI).Token
        assert(second~=token,'retry reused an already published observation token')
        assert(A.Resume(PLAN.ID).Status=='WAITING_UI' and ENACT_CALLS==2,
          'retry was resent before its own publication')
        ExposedMembers.JAC_HeroReadAPI.GetObservation=function(value)
          if value==second then return {Published=true} end
        end
        local originalBuildings=CITIES[4].GetBuildings
        CITIES[4].GetBuildings=function(self)
          local buildings=originalBuildings(self)
          local originalWork=buildings.GetGreatWorkInSlot
          buildings.GetGreatWorkInSlot=function(obj,building,slot)
            if building==1 and slot==0 then return 901 end
            return originalWork(obj,building,slot)
          end
          return buildings
        end
        local rejected,reason=pcall(A.Resume,PLAN.ID)
        assert(not rejected and tostring(reason):find('donor slot was reused',1,true),
          'a new work in the saved source slot was accepted')
        assert(TRANSFERS==0 and CITIES[4]:GetOwner()==1,
          'donor city changed despite its source slot being reused')
        CITIES[4].GetBuildings=originalBuildings
        local ok,result=pcall(A.Resume,PLAN.ID)
        assert(ENACT_CALLS==2,'confirmed native work was enacted again')
        local current=A.GetState().Transactions[PLAN.ID]
        assert(current.NativeWorks[1].PhysicalMoved,'physical destination was not confirmed')
        assert(#A.NativeWorkLocations(900)==1 and A.NativeWorkLocations(900)[1].Owner==3)
        assert(ok and result.Status=='COMPLETE',
          'external native work did not finish the absorption after physical proof: '..
          tostring(not ok and result or current.Error))
        assert(CITIES[4]:GetOwner()==2 and CITIES[8]:GetOwner()==2)
        local returned=assert(A.GetRoster(3)['native-work:900'])
        assert(returned.Owner==3 and returned.Outcome=='RETURN' and
          returned.NativeWork.Index==900 and returned.NativeWork.Owner==3 and
          returned.NativeWork.CreatorName=='Original creator')
        assert(returned.NativeCustody.Owner==3 and returned.NativeCustody.X==16 and
          returned.NativeCustody.Building==1 and returned.NativeCustody.Slot==0)
        assert(returned.City.Owner==3 and returned.City.X==16 and returned.City.Y==0)
        assert(A.Verify(PLAN.ID,true))
        "#,
    )
    .exec()
}

#[test]
fn recovery_rejects_missing_or_duplicated_native_index() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        "WORK_CITIES={};assert(not pcall(recover));assert_retained();\
         WORK_CITIES={[4]=true,[16]=true};assert(not pcall(recover));assert_retained()",
    )
    .exec()
}

#[test]
fn recovery_rejects_changed_native_provenance() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        "WORK_CITIES={[16]=true};CREATOR='Another creator';\
         assert(not pcall(recover));assert_retained()",
    )
    .exec()
}

#[test]
fn recovery_requires_the_original_index_and_records_its_actual_city_once() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        WORK_CITIES={[16]=true}
        local recovered=recover()
        assert(recovered.Status=='COMPLETE')
        local item=assert(A.GetRoster(2).native)
        assert(item.NativeWork.Index==900 and item.NativeWork.Owner==2)
        assert(item.City.Owner==2 and item.City.X==16 and item.City.Y==0,
          'work was incorrectly assigned to the capital')
        assert(item.NativeWork.CreatorName=='Original creator'
          and item.NativeWork.TurnCreated==2 and item.NativeWork.CreatedByPlayer==0)
        assert(A.Canonical(A.GetState().Archive)==ARCHIVE_BEFORE)
        ROSTER_BEFORE=A.Canonical(A.GetRoster(2))
        Game.props=A.Rules.Copy(Game.props)
        "#,
    )
    .exec()?;
    load(&lua, "JAC_Absorption_Gameplay.lua")?;
    lua.load(
        "A=JAC_AbsorptionGameplay or ExposedMembers.JAC_Absorption;\
         assert(recover().Status=='COMPLETE');\
         assert(A.Canonical(A.GetRoster(2))==ROSTER_BEFORE);\
         assert(A.Canonical(A.GetState().Archive)==ARCHIVE_BEFORE)",
    )
    .exec()
}

#[test]
fn human_world_reward_accepts_published_native_work_token_after_physical_move() -> mlua::Result<()>
{
    let lua = setup()?;
    lua.load(
        r#"
        local state=A.GetState()
        local source=assert(state.Archive.native.NativeWork)
        WORK_CITIES={[12]=true}
        state.Recoveries['recovery:world:native']={ID='native',Owner=2,
          Status='WAITING_UI',NativeSource=A.Rules.Copy(source),NativePhysicalMoved=true,
          NativeTarget={Owner=2,City=CITIES[12]:GetID(),X=12,Y=0,Building=1,Slot=0},
          NativeDealUI={Token='native:published'},ObservationToken='native:published'}
        Game:SetProperty(A.Rules.STATE_KEY,state)
        Game:SetProperty('JAC_ABSORPTION_FLOW_V1',{Version=1,Flows={},World={
          ['world:native']={ID='world:native',AssetID='native',Kind='ITEM',
            Status='CLAIMED',Recipient=2,WaitingFor='native:published'}}})
        ExposedMembers.JAC_HeroReadAPI.GetObservation=function() return nil end
        "#,
    )
    .exec()?;
    load(&lua, "JAC_Absorption_Flow.lua")?;
    lua.load(
        r#"
        local F=assert(JAC_AbsorptionFlowGameplay)
        assert(not pcall(F.ResumeWorldFor,2,'world:native','native:published'),
          'unpublished native reward was accepted')
        ExposedMembers.JAC_HeroReadAPI.GetObservation=function(token)
          if token=='native:published' then return {Published=true} end
        end
        local world=F.ResumeWorldFor(2,'world:native','native:published')
        assert(world.Status=='COMPLETE' and F.GetWorld('world:native').Status=='COMPLETE')
        local item=assert(A.GetRoster(2).native)
        assert(item.NativeWork.Index==900 and item.NativeCustody.Owner==2)
        assert(#A.NativeWorkLocations(900)==1 and A.NativeWorkLocations(900)[1].Owner==2)
        "#,
    )
    .exec()
}

#[test]
fn free_cities_native_recovery_reconciles_before_retry_and_updates_roster_once() -> mlua::Result<()>
{
    let lua = setup()?;
    lua.load(
        r#"
        CityManager.TransferCity(CITIES[16],3)
        WORK_CITIES={[16]=true}
        local archived=A.GetState()
        archived.Archive.native.NativeWork=assert(A.OwnedGreatWorkSlots(3)[1])
        Game:SetProperty(A.Rules.STATE_KEY,archived)
        local before=A.Canonical(A.GetState().Archive)
        GameInfo.GreatWorks.GREATWORK_TEST_RELIC=GameInfo.GreatWorks[4]
        GameInfo.GreatWorks[4].Index=4
        GameInfo.GreatWork_ValidSubTypes=function()
          local once=false
          return function()
            if not once then
              once=true
              return {GreatWorkObjectType='GREATWORKOBJECT_RELIC',
                GreatWorkSlotType='GREATWORKSLOT_RELIC'}
            end
          end
        end
        PlayerManager.GetFreeCitiesPlayerID=function() return 3 end
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function(owner,object)
          assert(owner==2 and object=='GREATWORKOBJECT_RELIC')
          return {{Owner=2,City=CITIES[12]:GetID(),X=12,Y=0,Building=1,Slot=0,
            SlotType='GREATWORKSLOT_RELIC'}}
        end
        DealDirection={OUTGOING=1}
        DealItemTypes={GREATWORK=2}
        local deal={items={}}
        function deal:GetItemCount() return #self.items end
        function deal:AddItemOfType(kind,from)
          assert(kind==2 and from==3)
          local item={}
          function item:SetSubType(value) assert(value==4);self.subtype=value end
          function item:SetValueType(value) assert(value==900);self.value=value end
          function item:GetValueType() return self.value end
          self.items[#self.items+1]=item
          return item
        end
        function deal:FindItemsByType(kind,subtype,from)
          assert(kind==2 and subtype==4 and from==3)
          return self.items
        end
        function deal:Validate() return 1 end
        function deal:IsValid() return #self.items==1 end
        ENACT_CALLS=0
        DealManager={
          GetWorkingDeal=function(direction,from,to)
            assert(direction==1 and from==3 and to==2);return deal
          end,
          GetPossibleDealItems=function(from,to,kind,working)
            assert(from==3 and to==2 and kind==2 and working==deal)
            return {{ForType=900,IsValid=true}}
          end,
          EnactWorkingDeal=function(from,to)
            assert(from==3 and to==2 and WORK_CITIES[16] and not WORK_CITIES[12])
            ENACT_CALLS=ENACT_CALLS+1
            if ENACT_CALLS==2 then WORK_CITIES[16],WORK_CITIES[12]=nil,true end
            return 1
          end}
        local pending=A.RecoverArchived('native',2,'recovery:native',nil)
        assert(pending.Status=='WAITING_UI' and ENACT_CALLS==1)
        assert(A.GetRoster(2).native==nil and A.Canonical(A.GetState().Archive)==before)
        local token=assert(pending.NativeDealUI).Token
        assert(A.RecoverArchived('native',2,'recovery:native',nil).Status=='WAITING_UI')
        assert(ENACT_CALLS==1,'unpublished recovery resent the native deal')
        ExposedMembers.JAC_HeroReadAPI.GetObservation=function(value)
          if value==token then return {Published=true} end
        end
        local retry=A.RecoverArchived('native',2,'recovery:native',nil)
        assert(retry.Status=='WAITING_UI' and ENACT_CALLS==2)
        assert(A.GetRoster(2).native==nil and A.Canonical(A.GetState().Archive)==before)
        local second=assert(retry.NativeDealUI).Token
        assert(second~=token)
        assert(A.RecoverArchived('native',2,'recovery:native',nil).Status=='WAITING_UI'
          and ENACT_CALLS==2,'recovery retry ran again before new publication')
        ExposedMembers.JAC_HeroReadAPI.GetObservation=function(value)
          if value==second then return {Published=true} end
        end
        local saved=A.GetState()
        saved.Recoveries['recovery:native'].NativePhysicalMoved=true
        Game:SetProperty(A.Rules.STATE_KEY,saved) -- interruption after physical proof, before roster update
        local originalBuildings=CITIES[16].GetBuildings
        CITIES[16].GetBuildings=function(self)
          local buildings=originalBuildings(self)
          local originalWork=buildings.GetGreatWorkInSlot
          buildings.GetGreatWorkInSlot=function(obj,building,slot)
            if building==1 and slot==0 then return 901 end
            return originalWork(obj,building,slot)
          end
          return buildings
        end
        local rejected,reason=pcall(A.RecoverArchived,'native',2,'recovery:native',nil)
        assert(not rejected and tostring(reason):find('custodian slot was reused',1,true),
          'saved physical recovery accepted a newly occupied original slot')
        assert(A.GetRoster(2).native==nil and A.Canonical(A.GetState().Archive)==before,
          'rejected recovery changed the archive or recipient roster')
        CITIES[16].GetBuildings=originalBuildings
        local completed=A.RecoverArchived('native',2,'recovery:native',nil)
        assert(completed.Status=='COMPLETE' and ENACT_CALLS==2)
        local item=assert(A.GetRoster(2).native)
        assert(item.NativeWork.Index==900 and item.NativeWork.Owner==2)
        assert(item.City.Owner==2 and item.City.X==12 and item.City.Y==0)
        assert(item.NativeCustody.Owner==2 and item.NativeCustody.City==CITIES[12]:GetID()
          and item.NativeCustody.X==12 and item.NativeCustody.Building==1
          and item.NativeCustody.Slot==0)
        assert(#A.NativeWorkLocations(900)==1 and A.NativeWorkLocations(900)[1].Owner==2)
        assert(A.Canonical(A.GetState().Archive)==before)
        assert(A.RecoverArchived('native',2,'recovery:native',nil).Status=='COMPLETE')
        assert(ENACT_CALLS==2,'completed recovery resent the native deal')
        "#,
    )
    .exec()
}

#[test]
fn scattered_native_work_keeps_original_index_through_free_cities_and_recovery() -> mlua::Result<()>
{
    let lua = setup()?;
    lua.load(
        r#"
        CityManager.TransferCity(CITIES[16],3)
        TRANSFERS=0
        GameInfo.GreatWorks.GREATWORK_TEST_RELIC=GameInfo.GreatWorks[4]
        GameInfo.GreatWorks[4].Index=4
        GameInfo.GreatWork_ValidSubTypes=function()
          local once=false
          return function()
            if not once then
              once=true
              return {GreatWorkObjectType='GREATWORKOBJECT_RELIC',
                GreatWorkSlotType='GREATWORKSLOT_RELIC'}
            end
          end
        end
        PlayerManager.GetFreeCitiesPlayerID=function() return 3 end
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function(owner,object)
          assert(object=='GREATWORKOBJECT_RELIC')
          local city=owner==3 and CITIES[16] or owner==2 and CITIES[12]
          assert(city and city:GetOwner()==owner)
          return {{Owner=owner,City=city:GetID(),X=city:GetX(),Y=0,
            Building=1,Slot=0,SlotType='GREATWORKSLOT_RELIC'}}
        end
        PUBLISHED={}
        ExposedMembers.JAC_HeroReadAPI.GetObservation=function(token)
          if PUBLISHED[token] then return {Published=true} end
        end
        DealDirection={OUTGOING=1}
        DealItemTypes={GREATWORK=2}
        local deals={}
        local function working(from,to)
          local key=from..':'..to
          if not deals[key] then
            local deal={items={}}
            function deal:GetItemCount() return #self.items end
            function deal:AddItemOfType(kind,owner)
              assert(kind==2 and owner==from)
              local item={}
              function item:SetSubType(value) assert(value==4) end
              function item:SetValueType(value) assert(value==900) end
              self.items[#self.items+1]=item
              return item
            end
            function deal:Validate() return 1 end
            function deal:IsValid() return #self.items==1 end
            deals[key]=deal
          end
          return deals[key]
        end
        ENACT_CALLS=0
        DealManager={
          GetWorkingDeal=function(direction,from,to)
            assert(direction==1 and ((from==1 and to==3) or (from==3 and to==2)))
            return working(from,to)
          end,
          GetPossibleDealItems=function(from,to,kind,deal)
            assert(kind==2 and deal==working(from,to))
            return {{ForType=900,IsValid=true}}
          end,
          EnactWorkingDeal=function(from,to)
            ENACT_CALLS=ENACT_CALLS+1
            if from==1 and to==3 then
              assert(WORK_CITIES[4] and not WORK_CITIES[16])
              WORK_CITIES[4],WORK_CITIES[16]=nil,true
            elseif from==3 and to==2 then
              assert(WORK_CITIES[16] and not WORK_CITIES[12])
              WORK_CITIES[16],WORK_CITIES[12]=nil,true
            else error('unexpected native work deal') end
            return 1
          end}
        Players[1].units={}
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,{})
        GameInfo.JAC_AbsorptionOutcomes=function()
          local rows={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='SCATTER',Destination='WORLD'}}
          local i=0;return function() i=i+1;return rows[i] end
        end
        PLAN={ID='native-scatter',From=1,To=2,Mode='MERGER',
          EntryConditionsMet=true,RelationsResolved=true,Leader='RECRUIT',
          NativeWorks={[900]={Outcome='SCATTER'}}}
        local pending=A.Apply(PLAN)
        assert(pending.Status=='WAITING_UI' and ENACT_CALLS==1)
        assert(CITIES[4]:GetOwner()==1 and TRANSFERS==0 and KILLS==0)
        assert(A.Resume(PLAN.ID).Status=='WAITING_UI' and ENACT_CALLS==1)
        PUBLISHED[assert(A.GetState().Transactions[PLAN.ID].NativeDealUI).Token]=true
        local complete=A.Resume(PLAN.ID)
        assert(complete.Status=='COMPLETE' and ENACT_CALLS==1)
        local scattered=assert(A.GetState().Archive['native-work:900'])
        assert(scattered.Owner==-1 and scattered.Outcome=='SCATTER' and scattered.City==nil)
        assert(scattered.NativeWork.Index==900 and scattered.NativeWork.Owner==3 and
          scattered.NativeWork.CreatorName=='Original creator')
        assert(scattered.NativeCustody.Owner==3 and scattered.NativeCustody.X==16 and
          scattered.NativeCustody.Building==1 and scattered.NativeCustody.Slot==0)
        assert(CITIES[4]:GetOwner()==2 and CITIES[8]:GetOwner()==2)
        assert(#A.NativeWorkLocations(900)==1 and A.NativeWorkLocations(900)[1].Owner==3)
        local key='recovery:native-work:900'
        local reclaim=A.RecoverArchived('native-work:900',2,key,nil)
        assert(reclaim.Status=='WAITING_UI' and ENACT_CALLS==2)
        assert(A.RecoverArchived('native-work:900',2,key,nil).Status=='WAITING_UI'
          and ENACT_CALLS==2)
        PUBLISHED[assert(reclaim.NativeDealUI).Token]=true
        local recovered=A.RecoverArchived('native-work:900',2,key,nil)
        assert(recovered.Status=='COMPLETE' and ENACT_CALLS==2)
        local item=assert(A.GetRoster(2)['native-work:900'])
        assert(item.Owner==2 and item.Outcome=='RECOVERED' and
          item.NativeWork.Index==900 and item.NativeWork.Owner==2 and
          item.NativeWork.CreatorName=='Original creator')
        assert(item.City.Owner==2 and item.City.X==12 and item.City.Y==0)
        assert(item.NativeCustody.Owner==2 and item.NativeCustody.City==CITIES[12]:GetID()
          and item.NativeCustody.X==12 and item.NativeCustody.Building==1
          and item.NativeCustody.Slot==0)
        assert(#A.NativeWorkLocations(900)==1 and A.NativeWorkLocations(900)[1].Owner==2)
        assert(A.RecoverArchived('native-work:900',2,key,nil).Status=='COMPLETE'
          and ENACT_CALLS==2)
        "#,
    )
    .exec()
}
