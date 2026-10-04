use mlua::Lua;
use std::{fs, path::PathBuf};

fn load(lua: &Lua, name: &str) -> mlua::Result<()> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../poc/JAC_PoC_Absorption/Scripts")
        .join(name);
    let path = if name == "JAC_Absorption_Gameplay.lua" {
        std::env::var_os("JAC_FLOW_GAMEPLAY_SOURCE")
            .map(PathBuf::from)
            .unwrap_or(path)
    } else {
        path
    };
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
        end,ReadCarrierPlots=function(owner,id)
          local city=assert(Players[owner]:GetCities():FindID(id))
          local result={}
          for index=0,Map.GetPlotCount()-1 do
            local plot=Map.GetPlotByIndex(index)
            if plot.city==city:GetX() then
              assert(plot:GetOwner()==owner)
              result[#result+1]=index
            end
          end
          return result
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
fn inherited_native_types_use_city_owner_change_without_individual_move() -> mlua::Result<()> {
    for object_type in [
        "GREATWORKOBJECT_ARTIFACT",
        "GREATWORKOBJECT_SCULPTURE",
        "GREATWORKOBJECT_LANDSCAPE",
        "GREATWORKOBJECT_PORTRAIT",
        "GREATWORKOBJECT_RELIGIOUS",
        "GREATWORKOBJECT_RELIC",
        "GREATWORKOBJECT_WRITING",
    ] {
        let lua = setup()?;
        lua.globals().set("TEST_OBJECT_TYPE", object_type)?;
        lua.load(r#"
            GameInfo.GreatWorks[4].GreatWorkObjectType=TEST_OBJECT_TYPE
            Players[1].units={}
            Players[1]:SetProperty(A.Rules.ROSTER_KEY,{})
            DealManager=nil
            UI={RequestPlayerOperation=function() error('individual MOVE called for inherited work') end}
            GameInfo.JAC_AbsorptionOutcomes=function()
              local rows={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
                {Kind='ITEM',Outcome='TRANSFER',Destination='ABSORBER'}}
              local i=0;return function() i=i+1;return rows[i] end
            end
            local original=assert(A.NativeWorkLocations(900)[1])
            local plan={ID='inherited-type',From=1,To=2,Mode='MERGER',
              EntryConditionsMet=true,RelationsResolved=true,Leader='RECRUIT'}
            local tx=A.Apply(plan)
            if tx.Status=='WAITING_UI' then
              ExposedMembers.JAC_HeroReadAPI.GetObservation=function() return {Published=true} end
              tx=A.Resume(plan.ID)
            end
            assert(tx.Status=='COMPLETE',TEST_OBJECT_TYPE..' inheritance did not finish')
            local snap=assert(tx.NativeWorks[1])
            assert(snap.NativeRoute=='CITY' and snap.Custodian==nil
              and snap.TargetSlot==nil and snap.Carrier==nil and not snap.DealSubmitted,
              'inherited original was routed through an individual transfer')
            local locations=A.NativeWorkLocations(900)
            assert(#locations==1 and locations[1].Owner==2 and locations[1].Index==original.Index
              and locations[1].X==original.X and locations[1].Y==original.Y
              and locations[1].Building==original.Building and locations[1].Slot==original.Slot,
              'engine owner change moved the inherited original out of its source slot')
            for _,key in ipairs({'GreatWorkType','ObjectType','CreatorName','TurnCreated','CreatedByPlayer'}) do
              assert(locations[1][key]==original[key],'inherited metadata changed: '..key)
            end
            assert(A.Rules.Equal(locations[1].Effects,original.Effects))
            assert(A.Verify(plan.ID,true))
            local rosterBefore=A.Canonical(A.GetRoster(2))
            local saved=A.GetState()
            saved.Transactions[plan.ID].NativeWorks[1].NativeRoute='CORE'
            saved.Transactions[plan.ID].Status='BLOCKED'
            saved.Transactions[plan.ID].Error='old save interrupted before final proof'
            Game:SetProperty(A.Rules.STATE_KEY,saved)
            local resumed=A.Resume(plan.ID)
            assert(resumed.Status=='COMPLETE' and resumed.NativeWorks[1].NativeRoute=='CITY',
              'old CORE inheritance was not migrated to the engine city route')
            assert(A.Canonical(A.GetRoster(2))==rosterBefore and #A.NativeWorkLocations(900)==1,
              'legacy route migration repeated an award or lost the original')
        "#).exec()?;
    }
    Ok(())
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
        local readWork=ExposedMembers.JAC_HeroReadAPI.ReadNativeWork
        ExposedMembers.JAC_HeroReadAPI.ReadNativeWork=function(...)
          local result=readWork(...);result.CreatedByPlayer=1;return result
        end
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
fn long_native_slot_wait_starts_full_conquest_friction_at_city_transfer() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        Players[1].units={}
        UNIT(2,203,'UNIT_WARRIOR',12) -- an ordinary destination can keep its center unit
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,{})
        GameInfo.GreatWorks[4].Index=4
        GameInfo.GreatWorks.GREATWORK_TEST_RELIC=GameInfo.GreatWorks[4]
        GameInfo.GreatWork_ValidSubTypes=function()
          local once=false;return function()
            if not once then once=true;return {GreatWorkObjectType='GREATWORKOBJECT_RELIC',
              GreatWorkSlotType='GREATWORKSLOT_RELIC'} end
          end
        end
        GameInfo.JAC_AbsorptionOutcomes=function()
          local rows={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='RETURN',Destination='RETURN'}}
          local i=0;return function() i=i+1;return rows[i] end
        end
        SLOT_READY=false
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function(owner,object)
          assert(owner==2 and object=='GREATWORKOBJECT_RELIC')
          if not SLOT_READY then return {} end
          return {{Owner=2,City=CITIES[12]:GetID(),X=12,Y=0,Building=1,Slot=0,
            SlotType='GREATWORKSLOT_RELIC'}}
        end
        ExposedMembers.JAC_HeroReadAPI.GetObservation=function() return {Published=true} end
        DealDirection={OUTGOING=1};DealItemTypes={GREATWORK=2}
        local deal={items={}}
        function deal:GetItemCount() return #self.items end
        function deal:AddItemOfType(kind,owner)
          assert(kind==2 and owner==1)
          local item={}
          function item:SetSubType(value) assert(value==4) end
          function item:SetValueType(value) assert(value==900) end
          self.items[#self.items+1]=item;return item
        end
        function deal:Validate() return 1 end
        function deal:IsValid() return #self.items==1 end
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
            WORK_CITIES[4],WORK_CITIES[12]=nil,true
            return 1
          end}
        local plan={ID='delayed-native-policy',From=1,To=0,Mode='CONQUEST',
          FlowPolicy=true,EntryConditionsMet=true,RelationsResolved=true,Leader='RECRUIT',
          NativeWorks={[900]={Outcome='RETURN',ReturnOwner=2}}}
        local tx=A.Apply(plan)
        assert(tx.Status=='WAITING_UI' and tx.Turn==5 and not tx.NativeWorks[1].TargetSlot)
        for turn=6,9 do
          CURRENT_TURN=turn
          tx=A.Resume(plan.ID)
          assert(tx.Status=='WAITING_UI' and CITIES[4]:GetOwner()==1
            and CITIES[8]:GetOwner()==1 and #A.NativeWorkLocations(900)==1)
        end
        SLOT_READY=true;CURRENT_TURN=10
        assert(A.Resume(plan.ID).Status=='WAITING_UI' and CITIES[4]:GetOwner()==1)
        tx=A.Resume(plan.ID)
        assert(tx.Status=='COMPLETE' and CITIES[4]:GetOwner()==0
          and CITIES[8]:GetOwner()==0 and #A.NativeWorkLocations(900)==1
          and A.NativeWorkLocations(900)[1].Owner==2)
        for _,x in ipairs({4,8}) do
          local g=assert(CITIES[x]:GetProperty(A.Rules.GOVERNANCE_KEY))
          assert(g.Turn==10 and g.ImmediateApplied and g.FrictionTurns==3
            and g.FrictionApplied==0 and CITIES[x].loyalty==72,
            'native staging consumed the city friction duration before transfer')
        end
        Game.props=A.Rules.Copy(Game.props)
        for _,x in ipairs({4,8}) do CITIES[x].props=A.Rules.Copy(CITIES[x].props) end
    "#,
    )
    .exec()?;
    load(&lua, "JAC_Absorption_Gameplay.lua")?;
    lua.load(
        r#"
        local A=JAC_AbsorptionGameplay
        for turn=11,15 do
          CURRENT_TURN=turn
          A.ApplyCityFriction();A.ApplyCityFriction()
        end
        for _,x in ipairs({4,8}) do
          local g=CITIES[x]:GetProperty(A.Rules.GOVERNANCE_KEY)
          assert(g.Turn==10 and g.FrictionApplied==3 and g.LastFrictionTurn==13
            and CITIES[x].loyalty==66,
            'delayed city did not receive exactly three post-transfer turns')
        end
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
fn free_cities_palace_work_is_not_borrowed_without_a_verified_source_slot_route() -> mlua::Result<()>
{
    let lua = setup()?;
    lua.load(
        r#"
        CityManager.TransferCity(CITIES[16],3)
        TRANSFERS=0
        WORK_CITIES={}
        CITIES[16].palace=true
        CITIES[16].palaceWork=900
        local oldBuildings=CITIES[16].GetBuildings
        CITIES[16].GetBuildings=function(self)
          local b=oldBuildings(self)
          local oldWork=b.GetGreatWorkInSlot
          b.GetGreatWorkInSlot=function(obj,building,slot)
            if building==0 and slot==0 then return self.palaceWork or -1 end
            return oldWork(obj,building,slot)
          end
          return b
        end
        local source=assert(A.OwnedGreatWorkSlots(3)[1])
        assert(source.Index==900 and source.BuildingType=='BUILDING_PALACE')
        local state=A.GetState()
        state.Archive.native.NativeWork=source
        Game:SetProperty(A.Rules.STATE_KEY,state)
        GameInfo.GreatWorks.GREATWORK_TEST_RELIC=GameInfo.GreatWorks[4]
        GameInfo.GreatWorks[4].Index=4
        GameInfo.GreatWork_ValidSubTypes=function()
          local once=false;return function()
            if not once then once=true;return {GreatWorkObjectType='GREATWORKOBJECT_RELIC',
              GreatWorkSlotType='GREATWORKSLOT_RELIC'} end
          end
        end
        PlayerManager.GetFreeCitiesPlayerID=function() return 3 end
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function(owner,object)
          assert(owner==2 and object=='GREATWORKOBJECT_RELIC')
          return {{Owner=2,City=CITIES[12]:GetID(),X=12,Y=0,Building=1,Slot=0,
            SlotType='GREATWORKSLOT_RELIC'}}
        end
        local ok,why=pcall(A.RecoverArchived,'native',2,'recovery:palace',nil)
        assert(not ok and tostring(why):find('Palace carrier has no verified',1,true),
          'Free Cities Palace work entered an unverified carrier route')
        assert(TRANSFERS==0 and CITIES[16]:GetOwner()==3 and CITIES[16].palaceWork==900
          and A.GetRoster(2).native==nil and A.GetState().Archive.native.NativeWork.Index==900,
          'Palace rejection changed original work or owner')
        "#,
    )
    .exec()
}

#[test]
fn simultaneous_native_recoveries_reserve_distinct_empty_recipient_slots() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        CityManager.TransferCity(CITIES[16],3)
        CityManager.TransferCity(CITIES[8],3)
        TRANSFERS=0
        WORK_CITIES={[16]=true}
        local oldSource=CITIES[8].GetBuildings
        CITIES[8].GetBuildings=function(self)
          local b=oldSource(self)
          local oldWork=b.GetGreatWorkInSlot
          b.GetGreatWorkInSlot=function(obj,building,slot)
            if building==1 and slot==0 then return 901 end
            return oldWork(obj,building,slot)
          end
          return b
        end
        local oldTarget=CITIES[12].GetBuildings
        CITIES[12].GetBuildings=function(self)
          local b=oldTarget(self)
          local oldSlots=b.GetNumGreatWorkSlots
          local oldWork=b.GetGreatWorkInSlot
          b.GetNumGreatWorkSlots=function(obj,building)
            if building==1 then return 2 end
            return oldSlots(obj,building)
          end
          b.GetGreatWorkInSlot=function(obj,building,slot)
            if building==1 and slot==1 then return -1 end
            return oldWork(obj,building,slot)
          end
          return b
        end
        GameInfo.GreatWorks.GREATWORK_TEST_RELIC=GameInfo.GreatWorks[4]
        GameInfo.GreatWorks[4].Index=4
        GameInfo.GreatWork_ValidSubTypes=function()
          local once=false;return function()
            if not once then once=true;return {GreatWorkObjectType='GREATWORKOBJECT_RELIC',
              GreatWorkSlotType='GREATWORKSLOT_RELIC'} end
          end
        end
        PlayerManager.GetFreeCitiesPlayerID=function() return 3 end
        ExposedMembers.JAC_HeroReadAPI.NativeMoveAvailable=true
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function(owner,object)
          assert(owner==2 and object=='GREATWORKOBJECT_RELIC')
          return {{Owner=2,City=CITIES[12]:GetID(),X=12,Y=0,Building=1,Slot=0,
              SlotType='GREATWORKSLOT_RELIC'},
            {Owner=2,City=CITIES[12]:GetID(),X=12,Y=0,Building=1,Slot=1,
              SlotType='GREATWORKSLOT_RELIC'}}
        end
        local works=A.OwnedGreatWorkSlots(3)
        assert(#works==2 and works[1].Index==900 and works[2].Index==901)
        local state=A.GetState()
        state.Archive.first={ID='first',Kind='ITEM',Owner=-1,NativeWork=works[1]}
        state.Archive.second={ID='second',Kind='ITEM',Owner=-1,NativeWork=works[2]}
        Game:SetProperty(A.Rules.STATE_KEY,state)
        local first=A.RecoverArchived('first',2,'recovery:first',nil)
        local second=A.RecoverArchived('second',2,'recovery:second',nil)
        assert(first.Status=='WAITING_UI' and second.Status=='WAITING_UI'
          and first.NativeTarget.Slot==0 and second.NativeTarget.Slot==1
          and first.NativeTarget.City==second.NativeTarget.City,
          'concurrent native recoveries reserved the same official slot')
        assert(TRANSFERS==2 and A.GetRoster(2).first==nil and A.GetRoster(2).second==nil,
          'reservation completed or duplicated a recovery before physical proof')
        "#,
    )
    .exec()
}

#[test]
fn simultaneous_scatter_flows_do_not_borrow_two_slots_in_the_same_free_city() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        CityManager.TransferCity(CITIES[16],3)
        TRANSFERS=0
        local oldBuildings=CITIES[16].GetBuildings
        CITIES[16].GetBuildings=function(self)
          local b=oldBuildings(self)
          local oldSlots=b.GetNumGreatWorkSlots
          local oldWork=b.GetGreatWorkInSlot
          b.GetNumGreatWorkSlots=function(obj,building)
            if building==1 then return 2 end
            return oldSlots(obj,building)
          end
          b.GetGreatWorkInSlot=function(obj,building,slot)
            if building==1 and slot==1 then return -1 end
            return oldWork(obj,building,slot)
          end
          return b
        end
        GameInfo.GreatWorks.GREATWORK_TEST_RELIC=GameInfo.GreatWorks[4]
        GameInfo.GreatWorks[4].Index=4
        GameInfo.GreatWork_ValidSubTypes=function()
          local once=false;return function()
            if not once then once=true;return {GreatWorkObjectType='GREATWORKOBJECT_RELIC',
              GreatWorkSlotType='GREATWORKSLOT_RELIC'} end
          end
        end
        PlayerManager.GetFreeCitiesPlayerID=function() return 3 end
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function(owner,object)
          assert(owner==3 and object=='GREATWORKOBJECT_RELIC')
          return {{Owner=3,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0,
              SlotType='GREATWORKSLOT_RELIC'},
            {Owner=3,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=1,
              SlotType='GREATWORKSLOT_RELIC'}}
        end
        Players[1].units={}
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,{})
        GameInfo.JAC_AbsorptionOutcomes=function()
          local rows={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='SCATTER',Destination='WORLD'}}
          local i=0;return function() i=i+1;return rows[i] end
        end
        local state=A.GetState()
        state.Transactions.other={ID='other',From=0,To=3,Status='WAITING_UI',
          NativeWorks={{Destination=-1,TargetSlot={Owner=3,City=CITIES[16]:GetID(),
            X=16,Y=0,Building=1,Slot=0}}}}
        Game:SetProperty(A.Rules.STATE_KEY,state)
        local plan={ID='second-scatter',From=1,To=2,Mode='MERGER',
          EntryConditionsMet=true,RelationsResolved=true,Leader='RECRUIT',
          NativeWorks={[900]={Outcome='SCATTER'}}}
        local waiting=A.Apply(plan)
        assert(waiting.Status=='WAITING_UI' and waiting.NativeWorks[1].Source.Index==900
          and waiting.NativeWorks[1].Custodian==3 and not waiting.NativeWorks[1].TargetSlot,
          'second Flow reserved another slot in the already borrowed city')
        assert(TRANSFERS==0 and CITIES[16]:GetOwner()==3 and CITIES[4]:GetOwner()==1
          and #A.NativeWorkLocations(900)==1,
          'second Flow mutated its donor or the carrier before a separate city was available')
        "#,
    )
    .exec()
}

fn occupied_carrier_candidates(occupy_second: bool) -> mlua::Result<Lua> {
    let lua = setup()?;
    lua.globals().set("OCCUPY_SECOND", occupy_second)?;
    lua.load(
        r#"
        CITY(2,18)
        local buildings=CITIES[18].GetBuildings
        CITIES[18].GetBuildings=function(self)
          local result=buildings(self)
          result.GetGreatWorkInSlot=function() return -1 end
          return result
        end
        UNIT(2,201,'UNIT_WARRIOR',16) -- foreign-to-donor unit on first city center
        if OCCUPY_SECOND then UNIT(2,202,'UNIT_WARRIOR',18) end
        Players[1].units={}
        local original=assert(A.OwnedGreatWorkSlots(1)[1])
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,{['native-work:900']={
          ID='native-work:900',Kind='ITEM',Owner=1,Outcome='RETURN',
          NativeOwnerMoves=1,NativeWork=A.Rules.Copy(original)}})
        GameInfo.GreatWorks[4].Index=4
        GameInfo.GreatWorks.GREATWORK_TEST_RELIC=GameInfo.GreatWorks[4]
        GameInfo.GreatWork_ValidSubTypes=function()
          local once=false;return function()
            if not once then once=true;return {GreatWorkObjectType='GREATWORKOBJECT_RELIC',
              GreatWorkSlotType='GREATWORKSLOT_RELIC'} end
          end
        end
        GameInfo.JAC_AbsorptionOutcomes=function()
          local rows={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='RETURN',Destination='RETURN'}}
          local i=0;return function() i=i+1;return rows[i] end
        end
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function(owner,object)
          assert(owner==2 and object=='GREATWORKOBJECT_RELIC')
          return {{Owner=2,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0,
              SlotType='GREATWORKSLOT_RELIC'},
            {Owner=2,City=CITIES[18]:GetID(),X=18,Y=0,Building=1,Slot=0,
              SlotType='GREATWORKSLOT_RELIC'}}
        end
        ExposedMembers.JAC_HeroReadAPI.NativeMoveAvailable=false
        DealManager={GetWorkingDeal=function() error('Core deal was selected') end,
          GetPossibleDealItems=function() error('Core deal was queried') end,
          EnactWorkingDeal=function() error('Core deal was enacted') end}
        TRANSFERS=0
        PLAN={ID='occupied-carrier-center',From=1,To=0,Mode='MERGER',
          FlowPolicy=true,EntryConditionsMet=true,RelationsResolved=true,Leader='RECRUIT',
          NativeWorks={[900]={Outcome='RETURN',ReturnOwner=2}}}
    "#,
    )
    .exec()?;
    Ok(lua)
}

#[test]
fn borrowed_carrier_skips_occupied_centers_and_waits_without_an_empty_city() -> mlua::Result<()> {
    let lua = occupied_carrier_candidates(false)?;
    lua.load(
        r#"
        local tx=A.Apply(PLAN)
        local snap=assert(tx.NativeWorks[1])
        assert(tx.Status=='WAITING_UI' and snap.NativeRoute=='CARRIER'
          and snap.TargetSlot and snap.TargetSlot.X==18 and snap.Carrier
          and snap.Carrier.Phase=='BORROW' and not snap.Carrier.BorrowIssued,
          'the occupied first carrier stopped selection before the valid second city')
        assert(TRANSFERS==0 and CITIES[4]:GetOwner()==1
          and CITIES[16]:GetOwner()==2 and CITIES[18]:GetOwner()==2
          and #A.NativeWorkLocations(900)==1 and A.NativeWorkLocations(900)[1].Owner==1)
    "#,
    )
    .exec()?;

    let lua = occupied_carrier_candidates(true)?;
    lua.load(
        r#"
        local tx=A.Apply(PLAN)
        assert(tx.Status=='WAITING_UI' and not tx.NativeWorks[1].TargetSlot
          and not tx.NativeWorks[1].Carrier,
          'no vacant city center must leave a saved, retryable wait')
        assert(TRANSFERS==0 and KILLS==0 and CITIES[4]:GetOwner()==1
          and CITIES[8]:GetOwner()==1 and CITIES[16]:GetOwner()==2
          and CITIES[18]:GetOwner()==2 and #A.NativeWorkLocations(900)==1
          and A.NativeWorkLocations(900)[1].Owner==1,
          'the wait changed an original work or donor city')
    "#,
    )
    .exec()
}

#[test]
fn reserved_carrier_rechecks_a_new_center_unit_and_resumes_after_reload() -> mlua::Result<()> {
    let lua = occupied_carrier_candidates(false)?;
    lua.load(
        r#"
        local tx=A.Apply(PLAN)
        local carrier=assert(tx.NativeWorks[1].Carrier)
        assert(carrier.X==18 and carrier.Phase=='BORROW' and not carrier.BorrowIssued)
        local buildings=CITIES[18].GetBuildings
        CITIES[18].GetBuildings=function(self)
          local result=buildings(self)
          result.GetGreatWorkInSlot=function(_,building,slot)
            return WORK_CITIES[18] and building==1 and slot==0 and 900 or -1
          end
          return result
        end
        UNIT(2,203,'UNIT_WARRIOR',18)
        ExposedMembers.JAC_HeroReadAPI.NativeMoveAvailable=true
        local ok,reason=pcall(A.Resume,PLAN.ID)
        assert(not ok and tostring(reason):find('unit on its center',1,true),
          'a unit entering the reserved city did not stop its ownership transfer')
        local blocked=A.GetState().Transactions[PLAN.ID]
        assert(blocked.Status=='BLOCKED' and not blocked.NativeWorks[1].Carrier.BorrowIssued
          and TRANSFERS==0 and KILLS==0 and CITIES[18]:GetOwner()==2
          and CITIES[4]:GetOwner()==1 and CITIES[8]:GetOwner()==1
          and #A.NativeWorkLocations(900)==1 and A.NativeWorkLocations(900)[1].Owner==1,
          'the late center blocker changed a work, city, or unit')
        Game.props=A.Rules.Copy(Game.props)
        "#,
    )
    .exec()?;
    load(&lua, "JAC_Absorption_Gameplay.lua")?;
    lua.load(
        r#"
        A=JAC_AbsorptionGameplay or ExposedMembers.JAC_Absorption
        assert(not pcall(A.Resume,PLAN.ID) and TRANSFERS==0,
          'reload bypassed the late center blocker')
        local unit=assert(Players[2].units[203]);unit.x=19
        PUBLISHED={}
        ExposedMembers.JAC_HeroReadAPI.GetObservation=function(token)
          if PUBLISHED[token] then return {Published=true} end
        end
        local pending=A.Resume(PLAN.ID)
        local carrier=assert(pending.NativeWorks[1].Carrier)
        assert(pending.Status=='WAITING_UI' and carrier.BorrowIssued and TRANSFERS==1
          and CITIES[18]:GetOwner()==1 and Players[2].units[203]==unit and KILLS==0,
          'the saved route could not resume after the blocker left')
        PUBLISHED[carrier.BorrowToken]=true
        pending=A.Resume(PLAN.ID);carrier=pending.NativeWorks[1].Carrier
        assert(carrier.Phase=='MOVE' and carrier.MoveTarget.X==18)
        WORK_CITIES[4],WORK_CITIES[18]=nil,true
        PUBLISHED[carrier.MoveToken]=true
        pending=A.Resume(PLAN.ID);carrier=pending.NativeWorks[1].Carrier
        assert(carrier.Phase=='RETURN' and TRANSFERS==2 and CITIES[18]:GetOwner()==2)
        PUBLISHED[carrier.ReturnToken]=true
        assert(A.Resume(PLAN.ID).Status=='COMPLETE')
        local work=A.NativeWorkLocations(900)
        assert(#work==1 and work[1].Owner==2 and work[1].X==18
          and CITIES[4]:GetOwner()==0 and CITIES[8]:GetOwner()==0)
        assert(A.Resume(PLAN.ID).Status=='COMPLETE' and TRANSFERS==4
          and Players[2].units[203]==unit and unit.x==19 and KILLS==0,
          'completion replay changed the original unit or transferred cities twice')
        "#,
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
fn recovery_does_not_reserve_a_slot_inside_another_borrowed_carrier() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        CityManager.TransferCity(CITIES[16],3)
        TRANSFERS=0
        WORK_CITIES={[16]=true}
        local state=A.GetState()
        state.Archive.native.NativeWork=assert(A.OwnedGreatWorkSlots(3)[1])
        state.Recoveries.busy={ID='other',Owner=2,Status='WAITING_UI',
          NativeCarrier={Phase='MOVE',X=12,Y=0}}
        Game:SetProperty(A.Rules.STATE_KEY,state)
        GameInfo.GreatWork_ValidSubTypes=function()
          local once=false;return function()
            if not once then once=true;return {GreatWorkObjectType='GREATWORKOBJECT_RELIC',
              GreatWorkSlotType='GREATWORKSLOT_RELIC'} end
          end
        end
        PlayerManager.GetFreeCitiesPlayerID=function() return 3 end
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function(owner,object)
          assert(owner==2 and object=='GREATWORKOBJECT_RELIC')
          return {{Owner=2,City=CITIES[12]:GetID(),X=12,Y=0,Building=1,Slot=0,
            SlotType='GREATWORKSLOT_RELIC'}}
        end
        local accepted,reason=pcall(recover)
        assert(not accepted and tostring(reason):find('no compatible empty destination slot',1,true),
          'borrowed carrier city was offered as recovery destination: '..tostring(reason))
        local intent=assert(A.GetState().Recoveries['recovery:native'])
        assert(not intent.NativeTarget and not intent.NativeCarrier and TRANSFERS==0,
          'conflicting recovery moved a city or reserved its borrowed carrier')
        local locations=A.NativeWorkLocations(900)
        assert(#locations==1 and locations[1].Owner==3 and locations[1].X==16,
          'conflicting recovery changed the original work')
        "#,
    )
    .exec()
}

#[test]
fn recovered_original_work_can_be_absorbed_twice_with_completed_provenance() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        WORK_CITIES={[16]=true}
        local state=A.GetState()
        state.Archive['native-work:900']=A.Rules.Copy(state.Archive.native)
        state.Archive['native-work:900'].ID='native-work:900'
        state.Archive.native=nil
        Game:SetProperty(A.Rules.STATE_KEY,state)
        Players[2]:SetProperty(A.Rules.ROSTER_KEY,{})
        local recovered=A.RecoverArchived('native-work:900',2,'recovery:world:900',nil)
        assert(recovered.Status=='COMPLETE' and
          A.GetRoster(2)['native-work:900'].RecoveryToken=='recovery:world:900')
        local rosterIDs={}
        for id in pairs(A.GetRoster(2)) do rosterIDs[#rosterIDs+1]=id end
        assert(#rosterIDs==1 and rosterIDs[1]=='native-work:900',
          'unexpected recipient roster: '..A.Canonical(rosterIDs))
        Players[2].units={}
        Players[2].IsHuman=function() return false end
        GameInfo.JAC_AbsorptionOutcomes=function()
          local rows={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='TRANSFER',Destination='ABSORBER'}}
          local i=0;return function() i=i+1;return rows[i] end
        end
        local plan={ID='reabsorb-recovered-native',From=2,To=1,Mode='MERGER',
          EntryConditionsMet=true,RelationsResolved=true,Leader='RECRUIT',
          Assets={['native-work:900']='TRANSFER'}}
        local ok,result=pcall(A.Apply,plan)
        assert(ok,'valid recovered asset blocked during absorption: '..tostring(result))
        assert(result.Status=='COMPLETE' and CITIES[16]:GetOwner()==1,
          'recovered original work did not finish its next absorption')
        local work=A.NativeWorkLocations(900)
        assert(#work==1 and work[1].Owner==1 and work[1].X==16,
          'next absorption lost or duplicated recovered original index')
        assert(A.GetRoster(1)['native-work:900'].RecoveryToken=='recovery:world:900',
          'recovery identity was dropped in the next absorption')
        assert(A.GetState().Archive['native-work:900'].Owner==-1,
          'historical scatter archive was overwritten')
        local inherited=assert(A.GetRoster(1)['native-work:900'])
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,{['native-work:900']=A.Rules.Copy(inherited)})
        Players[1].units={}
        local again={ID='reabsorb-recovered-twice',From=1,To=0,Mode='MERGER',
          EntryConditionsMet=true,RelationsResolved=true,Leader='RECRUIT',
          Assets={['native-work:900']='TRANSFER'}}
        local history=A.GetState()
        local first=assert(history.Transactions[plan.ID])
        history.Transactions[plan.ID]=nil
        Game:SetProperty(A.Rules.STATE_KEY,history)
        local forged,why=pcall(A.Apply,again)
        assert(not forged and tostring(why):find('asset ID collision',1,true)
          and CITIES[16]:GetOwner()==1,
          'unproven intermediate owner was accepted as recovered history')
        history.Transactions[plan.ID]=first
        Game:SetProperty(A.Rules.STATE_KEY,history)
        local accepted,nextResult=pcall(A.Apply,again)
        assert(accepted and nextResult.Status=='COMPLETE',
          'second absorption of recovered original failed: '..tostring(nextResult))
        local twice=A.NativeWorkLocations(900)
        assert(#twice==1 and twice[1].Owner==0 and twice[1].X==16
          and CITIES[16]:GetOwner()==0,
          'second absorption lost or duplicated the same recovered original')
        assert(A.GetRoster(0)['native-work:900'].RecoveryToken=='recovery:world:900',
          'second absorption dropped the original recovery identity')
        assert(A.GetState().Archive['native-work:900'].Owner==-1,
          'second absorption overwrote historical scatter record')
        "#,
    )
    .exec()
}

#[test]
fn prepared_return_without_a_slot_reopens_owner_after_recipient_loses_city() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        CityManager.TransferCity(CITIES[16],3)
        TRANSFERS=0
        Players[1].units={}
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,{})
        GameInfo.JAC_AbsorptionOutcomes=function()
          local rows={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='RETURN',Destination='RETURN'}}
          local i=0;return function() i=i+1;return rows[i] end
        end
        DealManager={EnactWorkingDeal=function() error('unprepared work was enacted') end,
          GetWorkingDeal=function() error('unprepared work opened a deal') end,
          GetPossibleDealItems=function() error('unprepared work queried deal items') end}
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function(owner)
          assert(owner==3);return {}
        end
        GameInfo.GreatWork_ValidSubTypes=function()
          return function() return nil end
        end
        local plan={ID='prepared-return',From=1,To=2,Mode='MERGER',
          EntryConditionsMet=true,RelationsResolved=true,FlowPolicy=true,
          Leader='RECRUIT',NativeWorks={[900]={Outcome='RETURN',ReturnOwner=3}}}
        local waiting=A.Apply(plan)
        assert(waiting.Status=='WAITING_UI' and not waiting.AwaitingPreparation
          and waiting.NativeWorks[1].TargetSlot==nil and A.CanCancelUnprepared(plan.ID,plan),
          'native RETURN did not pause after read-only preparation')
        assert(TRANSFERS==0 and CITIES[4]:GetOwner()==1 and
          #A.NativeWorkLocations(900)==1 and A.NativeWorkLocations(900)[1].Owner==1)
        PLAN=plan
        Game:SetProperty('JAC_ABSORPTION_FLOW_V1',{Version=1,World={},Flows={
          [plan.ID]={ID=plan.ID,From=1,To=2,Phase='APPLYING',AppliedPlan=plan,
            FinalRollsComplete=true,Entries={{ID='native-work:900',Kind='ITEM',
              SelectedAction='RETURN',FinalOutcome='RETURN',ReturnOwner=3,
              Rolls={P3_SUCCESS={Used=0}}}}}}})
        Players[3].cities={}
        "#,
    )
    .exec()?;
    load(&lua, "JAC_Absorption_Flow.lua")?;
    lua.load(
        r#"
        local F=assert(JAC_AbsorptionFlowGameplay)
        local restored=F.Resume(PLAN.ID)
        assert(restored.Phase=='CHOICE' and restored.FinalRollsComplete
          and restored.AppliedPlan==nil and restored.Entries[1].ReturnOwnerPending,
          'prepared return owner loss did not reopen only the destination')
        assert(restored.Entries[1].FinalOutcome=='RETURN'
          and restored.Entries[1].Rolls.P3_SUCCESS.Used==0,
          'prepared return correction rerolled the final result')
        assert(A.GetState().Transactions[PLAN.ID]==nil and TRANSFERS==0
          and CITIES[4]:GetOwner()==1 and #A.NativeWorkLocations(900)==1,
          'prepared cancellation changed donor or original work')
        "#,
    )
    .exec()
}

#[test]
fn reserved_return_carrier_can_reselect_owner_only_before_city_borrow() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        Players[1].units={}
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,{['native-work:900']={
          ID='native-work:900',Kind='ITEM',Owner=1,Outcome='RETURN',
          NativeOwnerMoves=1,NativeWork=assert(A.OwnedGreatWorkSlots(1)[1])}})
        local state=A.GetState();state.Archive.native=nil
        Game:SetProperty(A.Rules.STATE_KEY,state)
        GameInfo.GreatWorks[4].Index=4
        GameInfo.GreatWorks.GREATWORK_TEST_RELIC=GameInfo.GreatWorks[4]
        GameInfo.GreatWork_ValidSubTypes=function()
          local once=false;return function()
            if not once then once=true;return {GreatWorkObjectType='GREATWORKOBJECT_RELIC',
              GreatWorkSlotType='GREATWORKSLOT_RELIC'} end
          end
        end
        GameInfo.JAC_AbsorptionOutcomes=function()
          local rows={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='RETURN',Destination='RETURN'}}
          local i=0;return function() i=i+1;return rows[i] end
        end
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function(owner)
          assert(owner==2)
          return {{Owner=2,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0,
            SlotType='GREATWORKSLOT_RELIC'}}
        end
        ExposedMembers.JAC_HeroReadAPI.NativeMoveAvailable=false
        DealManager={GetWorkingDeal=function() error('no Core deal permitted') end,
          GetPossibleDealItems=function() error('no Core deal permitted') end,
          EnactWorkingDeal=function() error('no Core deal permitted') end}
        TRANSFERS=0
        local plan={ID='reserved-return',From=1,To=0,Mode='MERGER',
          EntryConditionsMet=true,RelationsResolved=true,FlowPolicy=true,
          Leader='RECRUIT',Assets={['native-work:900']={Outcome='RETURN',ReturnOwner=2}},
          NativeWorks={[900]={Outcome='RETURN',ReturnOwner=2}}}
        local waiting=A.Apply(plan)
        local snap=assert(waiting.NativeWorks[1])
        assert(waiting.Status=='WAITING_UI' and snap.TargetSlot and snap.Carrier
          and snap.Carrier.Phase=='BORROW' and snap.Carrier.BorrowIssued==false
          and A.CanCancelUnprepared(plan.ID,plan) and TRANSFERS==0,
          'saved reservation or unissued borrow was treated as a city mutation')
        local stale=A.GetState()
        stale.Transactions[plan.ID].NativeWorks[1].Carrier.BorrowIssued=true
        Game:SetProperty(A.Rules.STATE_KEY,stale)
        assert(not A.CanCancelUnprepared(plan.ID,plan),
          'issued city borrow was cancellable even though its outcome is unknown')
        stale.Transactions[plan.ID].NativeWorks[1].Carrier.BorrowIssued=false
        Game:SetProperty(A.Rules.STATE_KEY,stale)
        PLAN=plan
        Game:SetProperty('JAC_ABSORPTION_FLOW_V1',{Version=1,World={},Flows={
          [plan.ID]={ID=plan.ID,From=1,To=0,Phase='APPLYING',AppliedPlan=plan,
            FinalRollsComplete=true,Entries={{ID='native-work:900',Kind='ITEM',
              SelectedAction='RETURN',FinalOutcome='RETURN',ReturnOwner=2,
              Rolls={P3_SUCCESS={Used=0}}}}}}})
        CityManager.TransferCity(CITIES[16],3)
        CityManager.TransferCity(CITIES[12],3)
        EXTERNAL_TRANSFERS=TRANSFERS
        assert(Players[2]:GetCities():GetCount()==0 and CITIES[16]:GetOwner()==3,
          'recipient cities were not actually lost to another owner')
        "#,
    )
    .exec()?;
    load(&lua, "JAC_Absorption_Flow.lua")?;
    lua.load(
        r#"
        local flow=assert(JAC_AbsorptionFlowGameplay).Resume(PLAN.ID)
        assert(flow.Phase=='CHOICE' and flow.FinalRollsComplete
          and flow.Entries[1].ReturnOwnerPending and flow.AppliedPlan==nil,
          'unborrowed reserved city prevented owner-only correction')
        assert(flow.Entries[1].FinalOutcome=='RETURN'
          and flow.Entries[1].Rolls.P3_SUCCESS.Used==0,
          'recipient correction changed the saved final outcome')
        assert(A.GetState().Transactions[PLAN.ID]==nil and TRANSFERS==EXTERNAL_TRANSFERS
          and CITIES[16]:GetOwner()==3 and CITIES[12]:GetOwner()==3
          and CITIES[4]:GetOwner()==1 and #A.NativeWorkLocations(900)==1,
          'owner correction lost the original work or moved a city')
        "#,
    )
    .exec()
}

#[test]
fn previously_moved_work_returns_by_recipient_city_carrier_without_core_deal() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        Players[1].units={}
        local original=assert(A.OwnedGreatWorkSlots(1)[1])
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,{['native-work:900']={
          ID='native-work:900',Kind='ITEM',Owner=1,Outcome='RETURN',
          NativeOwnerMoves=1,NativeWork=A.Rules.Copy(original)}})
        local state=A.GetState();state.Archive.native=nil
        Game:SetProperty(A.Rules.STATE_KEY,state)
        GameInfo.GreatWorks[4].Index=4
        GameInfo.GreatWorks.GREATWORK_TEST_RELIC=GameInfo.GreatWorks[4]
        GameInfo.GreatWork_ValidSubTypes=function()
          local once=false;return function()
            if not once then once=true;return {GreatWorkObjectType='GREATWORKOBJECT_RELIC',
              GreatWorkSlotType='GREATWORKSLOT_RELIC'} end
          end
        end
        GameInfo.JAC_AbsorptionOutcomes=function()
          local rows={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='RETURN',Destination='RETURN'}}
          local i=0;return function() i=i+1;return rows[i] end
        end
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function(owner,object)
          assert(owner==2 and object=='GREATWORKOBJECT_RELIC')
          return {{Owner=2,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0,
            SlotType='GREATWORKSLOT_RELIC'}}
        end
        ExposedMembers.JAC_HeroReadAPI.NativeMoveAvailable=true
        PUBLISHED={}
        ExposedMembers.JAC_HeroReadAPI.GetObservation=function(token)
          if PUBLISHED[token] then return {Published=true} end
        end
        DealManager={GetWorkingDeal=function() error('unsafe Core working deal opened') end,
          GetPossibleDealItems=function() error('unsafe Core deal queried') end,
          EnactWorkingDeal=function() error('unsafe Core deal enacted') end}
        PRIOR_GOVERNANCE={ID='previous-flow',From=3,To=2,FrictionPerTurn=0,
          FrictionApplied=0,FrictionTurns=0,ImmediateApplied=true}
        CITIES[16]:SetProperty(A.Rules.GOVERNANCE_KEY,A.Rules.Copy(PRIOR_GOVERNANCE))
        local transfer=CityManager.TransferCity
        CityManager.TransferCity=function(city,owner)
          transfer(city,owner)
          if city==CITIES[16] and owner==2 then
            city:SetProperty(A.Rules.GOVERNANCE_KEY,nil)
          end
        end
        TRANSFERS=0
        local plan={ID='returned-by-carrier',From=1,To=0,Mode='MERGER',
          EntryConditionsMet=true,RelationsResolved=true,Leader='RECRUIT',
          Assets={['native-work:900']={Outcome='RETURN',ReturnOwner=2}},
          NativeWorks={[900]={Outcome='RETURN',ReturnOwner=2}}}
        local active=A.Rules.Copy(PRIOR_GOVERNANCE)
        active.FrictionPerTurn,active.FrictionTurns,active.Turn=-2,2,Game.GetCurrentGameTurn()
        CITIES[16]:SetProperty(A.Rules.GOVERNANCE_KEY,active)
        local waiting=A.Apply(plan)
        assert(waiting.Status=='WAITING_UI' and not waiting.NativeWorks[1].TargetSlot
          and TRANSFERS==0 and CITIES[16]:GetOwner()==2,
          'city with active absorption friction was borrowed before its policy finished')
        CITIES[16]:SetProperty(A.Rules.GOVERNANCE_KEY,A.Rules.Copy(PRIOR_GOVERNANCE))
        local pending=A.Resume(plan.ID)
        local carrier=assert(pending.NativeWorks[1].Carrier)
        assert(pending.Status=='WAITING_UI' and carrier.Route=='TO_OWNER'
          and carrier.Phase=='BORROW' and pending.NativeWorks[1].NativeRoute=='CARRIER'
          and TRANSFERS==1 and CITIES[16]:GetOwner()==1,
          'previously moved work was not routed through an empty recipient city')
        assert(carrier.BorrowIssued and not A.CanCancelUnprepared(plan.ID,plan),
          'borrowed recipient city could be released as an untouched reservation')
        assert(A.Resume(plan.ID).Status=='WAITING_UI' and TRANSFERS==1)
        PUBLISHED[carrier.BorrowToken]=true
        pending=A.Resume(plan.ID)
        carrier=assert(pending.NativeWorks[1].Carrier)
        assert(carrier.Phase=='MOVE' and carrier.MoveTarget.Owner==1
          and carrier.MoveTarget.City==CITIES[16]:GetID())
        WORK_CITIES[4],WORK_CITIES[16]=nil,true
        PUBLISHED[carrier.MoveToken]=true
        pending=A.Resume(plan.ID)
        carrier=assert(pending.NativeWorks[1].Carrier)
        assert(carrier.Phase=='RETURN' and TRANSFERS==2 and CITIES[16]:GetOwner()==2)
        PUBLISHED[carrier.ReturnToken]=true
        local unrelated={ID='another-flow',To=2}
        CITIES[16]:SetProperty(A.Rules.GOVERNANCE_KEY,unrelated)
        local accepted,reason=pcall(A.Resume,plan.ID)
        assert(not accepted and tostring(reason):find('governance was replaced',1,true)
          and A.Rules.Equal(CITIES[16]:GetProperty(A.Rules.GOVERNANCE_KEY),unrelated)
          and CITIES[4]:GetOwner()==1,
          'recipient city governance was overwritten before finishing custody')
        CITIES[16]:SetProperty(A.Rules.GOVERNANCE_KEY,nil)
        local completed=A.Resume(plan.ID)
        assert(completed.Status=='COMPLETE' and CITIES[4]:GetOwner()==0
          and CITIES[8]:GetOwner()==0,'donor absorption did not complete after carrier proof')
        local work=A.NativeWorkLocations(900)
        assert(#work==1 and work[1].Owner==2 and work[1].CityID==CITIES[16]:GetID()
          and work[1].X==16 and work[1].Building==1 and work[1].Slot==0,
          'returned original work missed exact recipient slot')
        local returned=assert(A.GetRoster(2)['native-work:900'])
        assert(returned.NativeOwnerMoves==2 and returned.Outcome=='RETURN'
          and returned.NativeCustody.City==work[1].CityID,
          'return history or custody does not match the physical original')
        assert(A.Rules.Equal(CITIES[16]:GetProperty(A.Rules.GOVERNANCE_KEY),PRIOR_GOVERNANCE),
          'temporary carrier ownership erased the recipient city governance')
        assert(A.Verify(plan.ID,true))
        "#,
    )
    .exec()
}

#[test]
fn acquired_palace_work_avoids_the_known_destructive_core_return() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        Players[1].units={}
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,{})
        CITIES[4].palace=true
        local originalBuildings=CITIES[4].GetBuildings
        CITIES[4].GetBuildings=function(self)
          local buildings=originalBuildings(self)
          buildings.GetGreatWorkInSlot=function(_,building)
            if building==0 then return WORK_CITIES[4] and 900 or -1 end
            return -1
          end
          return buildings
        end
        GameInfo.JAC_AbsorptionOutcomes=function()
          local rows={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='RETURN',Destination='RETURN'}}
          local i=0;return function() i=i+1;return rows[i] end
        end
        GameInfo.GreatWork_ValidSubTypes=function()
          local once=false;return function()
            if not once then once=true;return {GreatWorkObjectType='GREATWORKOBJECT_RELIC',
              GreatWorkSlotType='GREATWORKSLOT_RELIC'} end
          end
        end
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function(owner)
          assert(owner==2)
          return {{Owner=2,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0,
            SlotType='GREATWORKSLOT_RELIC'}}
        end
        ExposedMembers.JAC_HeroReadAPI.NativeMoveAvailable=true
        DealManager={GetWorkingDeal=function() error('acquired Palace Core deal opened') end,
          GetPossibleDealItems=function() error('acquired Palace Core deal queried') end,
          EnactWorkingDeal=function() error('acquired Palace work destroyed by Core deal') end}
        TRANSFERS=0
        local plan={ID='acquired-palace-return',From=1,To=0,Mode='MERGER',
          EntryConditionsMet=true,RelationsResolved=true,Leader='RECRUIT',
          NativeWorks={[900]={Outcome='RETURN',ReturnOwner=2}}}
        local pending=A.Apply(plan)
        assert(pending.Status=='WAITING_UI' and pending.NativeWorks[1].NativeRoute=='CARRIER'
          and pending.NativeWorks[1].Source.BuildingType=='BUILDING_PALACE'
          and pending.NativeWorks[1].Source.CreatedByPlayer==0
          and pending.NativeWorks[1].Carrier.Route=='TO_OWNER'
          and TRANSFERS==1 and CITIES[16]:GetOwner()==1 and CITIES[4]:GetOwner()==1,
          'acquired Palace work entered a destructive cross-owner Core return')
        assert(#A.NativeWorkLocations(900)==1 and A.NativeWorkLocations(900)[1].Owner==1,
          'carrier setup lost acquired Palace original')
        "#,
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
        TRANSFERS=0
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
        ExposedMembers.JAC_HeroReadAPI.NativeMoveAvailable=true
        PUBLISHED={}
        ExposedMembers.JAC_HeroReadAPI.GetObservation=function(token)
          if PUBLISHED[token] then return {Published=true} end
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
            ENACT_CALLS=ENACT_CALLS+1
            error('unsafe direct recovery deal was invoked: '..from..':'..to)
          end}
        local pending=A.RecoverArchived('native',2,'recovery:native',nil)
        assert(pending.Status=='WAITING_UI' and ENACT_CALLS==0 and TRANSFERS==1
          and pending.NativeCarrier.Phase=='BORROW' and CITIES[16]:GetOwner()==2)
        assert(A.GetRoster(2).native==nil and A.Canonical(A.GetState().Archive)==before)
        assert(A.RecoverArchived('native',2,'recovery:native',nil).Status=='WAITING_UI')
        assert(ENACT_CALLS==0 and TRANSFERS==1,'unpublished recovery repeated a mutation')
        PUBLISHED[pending.NativeCarrier.BorrowToken]=true
        pending=A.RecoverArchived('native',2,'recovery:native',nil)
        assert(pending.NativeCarrier.Phase=='MOVE' and pending.NativeCarrier.MoveSource.City==CITIES[16]:GetID()
          and pending.NativeCarrier.MoveTarget.City==CITIES[12]:GetID())
        assert(A.RecoverArchived('native',2,'recovery:native',nil).Status=='WAITING_UI'
          and ENACT_CALLS==0 and TRANSFERS==1,'recovery MOVE replayed before publication')
        WORK_CITIES[16],WORK_CITIES[12]=nil,true -- human same-owner MOVE
        PUBLISHED[pending.NativeCarrier.MoveToken]=true
        pending=A.RecoverArchived('native',2,'recovery:native',nil)
        assert(pending.Status=='WAITING_UI' and pending.NativeCarrier.Phase=='RETURN'
          and CITIES[16]:GetOwner()==3 and TRANSFERS==2)
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
        PUBLISHED[pending.NativeCarrier.ReturnToken]=true
        local rejected,reason=pcall(A.RecoverArchived,'native',2,'recovery:native',nil)
        assert(not rejected and tostring(reason):find('carrier return slot changed',1,true),
          'saved physical recovery accepted a newly occupied original slot')
        assert(A.GetRoster(2).native==nil and A.Canonical(A.GetState().Archive)==before,
          'rejected recovery changed the archive or recipient roster')
        CITIES[16].GetBuildings=originalBuildings
        local originalSet=Players[2].SetProperty
        local stopped=false
        Players[2].SetProperty=function(self,key,value)
          if key==A.Rules.ROSTER_KEY and not stopped then
            stopped=true;error('staged after carrier recovery proof')
          end
          return originalSet(self,key,value)
        end
        local advanced,why=pcall(A.RecoverArchived,'native',2,'recovery:native',nil)
        assert(not advanced and tostring(why):find('staged after carrier recovery proof',1,true)
          and A.GetState().Recoveries['recovery:native'].NativeCarrier.Phase=='PROVED',
          'test did not interrupt after recovery proof before roster update')
        Players[2].SetProperty=originalSet
        local originalRead=ExposedMembers.JAC_HeroReadAPI.ReadNativeWork
        ExposedMembers.JAC_HeroReadAPI.ReadNativeWork=function(owner,id,index)
          if owner==2 and id==CITIES[12]:GetID() then error('UI city cache is not ready') end
          return originalRead(owner,id,index)
        end
        local waiting=A.RecoverArchived('native',2,'recovery:native',nil)
        assert(waiting.Status=='WAITING_UI' and waiting.NativeCarrier.RefreshRequested
          and waiting.ObservationToken==waiting.NativeCarrier.ReturnToken,
          'stale recovery UI cache became BLOCKED after physical proof')
        ExposedMembers.JAC_HeroReadAPI.ReadNativeWork=originalRead
        local completed=A.RecoverArchived('native',2,'recovery:native',nil)
        assert(completed.Status=='COMPLETE' and ENACT_CALLS==0 and TRANSFERS==2)
        local item=assert(A.GetRoster(2).native)
        assert(item.NativeWork.Index==900 and item.NativeWork.Owner==2)
        assert(item.City.Owner==2 and item.City.X==12 and item.City.Y==0)
        assert(item.NativeCustody.Owner==2 and item.NativeCustody.City==CITIES[12]:GetID()
          and item.NativeCustody.X==12 and item.NativeCustody.Building==1
          and item.NativeCustody.Slot==0)
        assert(#A.NativeWorkLocations(900)==1 and A.NativeWorkLocations(900)[1].Owner==2)
        assert(A.Canonical(A.GetState().Archive)==before)
        assert(A.RecoverArchived('native',2,'recovery:native',nil).Status=='COMPLETE')
        assert(ENACT_CALLS==0 and TRANSFERS==2,'completed recovery repeated a native mutation')
        "#,
    )
    .exec()
}

#[test]
fn unchanged_native_move_restores_empty_free_cities_carrier_before_blocking() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        CityManager.TransferCity(CITIES[16],3)
        TRANSFERS=0
        GameInfo.GreatWorks.GREATWORK_TEST_RELIC=GameInfo.GreatWorks[4]
        GameInfo.GreatWorks[4].Index=4
        GameInfo.GreatWork_ValidSubTypes=function()
          local once=false;return function()
            if not once then once=true;return {GreatWorkObjectType='GREATWORKOBJECT_RELIC',
              GreatWorkSlotType='GREATWORKSLOT_RELIC'} end
          end
        end
        PlayerManager.GetFreeCitiesPlayerID=function() return 3 end
        ExposedMembers.JAC_HeroReadAPI.NativeMoveAvailable=true
        ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function(owner,object)
          assert(owner==3 and object=='GREATWORKOBJECT_RELIC')
          return {{Owner=3,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0,
            SlotType='GREATWORKSLOT_RELIC'}}
        end
        PUBLISHED={}
        ExposedMembers.JAC_HeroReadAPI.GetObservation=function(token) return PUBLISHED[token] end
        DealManager={EnactWorkingDeal=function() error('unsafe direct deal invoked') end}
        Players[1].units={}
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,{})
        GameInfo.JAC_AbsorptionOutcomes=function()
          local rows={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
            {Kind='ITEM',Outcome='SCATTER',Destination='WORLD'}}
          local i=0;return function() i=i+1;return rows[i] end
        end
        local plan={ID='native-noop',From=1,To=2,Mode='MERGER',
          EntryConditionsMet=true,RelationsResolved=true,Leader='RECRUIT',
          NativeWorks={[900]={Outcome='SCATTER'}}}
        local tx=A.Apply(plan)
        assert(tx.Status=='WAITING_UI' and tx.NativeWorks[1].Carrier.Phase=='BORROW'
          and CITIES[16]:GetOwner()==1 and TRANSFERS==1)
        PUBLISHED[tx.NativeWorks[1].Carrier.BorrowToken]={Published=true}
        tx=A.Resume(plan.ID)
        local carrier=assert(tx.NativeWorks[1].Carrier)
        assert(carrier.Phase=='MOVE' and carrier.MoveAttempt==1)
        PUBLISHED[carrier.MoveToken]={Published=true,Unchanged=true}
        tx=A.Resume(plan.ID)
        carrier=assert(tx.NativeWorks[1].Carrier)
        assert(carrier.Phase=='MOVE' and carrier.MoveAttempt==2
          and CITIES[16]:GetOwner()==1 and WORK_CITIES[4],
          'first no-op moved the original work or carrier')
        PUBLISHED[carrier.MoveToken]={Published=true,Unchanged=true}
        tx=A.Resume(plan.ID)
        carrier=assert(tx.NativeWorks[1].Carrier)
        assert(carrier.Phase=='RESTORE' and CITIES[16]:GetOwner()==3
          and TRANSFERS==2 and WORK_CITIES[4] and not WORK_CITIES[16],
          'second no-op did not return the empty carrier before blocking')
        PUBLISHED[carrier.ReturnToken]={Published=true}
        local ok,reason=pcall(A.Resume,plan.ID)
        assert(not ok and tostring(reason):find('carrier restored',1,true),
          'restored carrier was reported as a successful scatter')
        assert(A.GetState().Transactions[plan.ID].Status=='BLOCKED'
          and #A.NativeWorkLocations(900)==1
          and A.NativeWorkLocations(900)[1].Owner==1
          and CITIES[4]:GetOwner()==1 and CITIES[8]:GetOwner()==1
          and A.GetState().Archive['native-work:900']==nil,
          'failed carrier MOVE changed the donor or awarded the original work')
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
          local slots={{Owner=owner,City=city:GetID(),X=city:GetX(),Y=0,
            Building=1,Slot=0,SlotType='GREATWORKSLOT_RELIC'}}
          if owner==3 then
            table.insert(slots,1,{Owner=owner,City=city:GetID(),X=city:GetX(),Y=0,
              Building=0,Slot=0,SlotType='GREATWORKSLOT_RELIC'})
          end
          return slots
        end
        PUBLISHED={}
        ExposedMembers.JAC_HeroReadAPI.GetObservation=function(token)
          if PUBLISHED[token] then return {Published=true} end
        end
        ExposedMembers.JAC_HeroReadAPI.NativeMoveAvailable=true
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
            error('unsafe direct cross-owner work deal was invoked: '..from..':'..to)
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
        assert(pending.Status=='WAITING_UI' and ENACT_CALLS==0 and TRANSFERS==1)
        assert(CITIES[16]:GetOwner()==1 and CITIES[4]:GetOwner()==1 and KILLS==0,
          'carrier borrowing mutated donor assets before the work MOVE')
        local carrier=assert(pending.NativeWorks[1].Carrier)
        assert(carrier.Phase=='BORROW' and carrier.Route=='TO_FREE')
        local originalBuildings=CITIES[16].GetBuildings
        CITIES[16].GetBuildings=function(self)
          local buildings=originalBuildings(self)
          buildings.GetGreatWorkInSlot=nil -- borrowed city is absent from the fresh UI cache
          return buildings
        end
        ExposedMembers.JAC_HeroReadAPI.ReadCity=function()
          error('UI city cache is not ready')
        end
        assert(A.Resume(PLAN.ID).Status=='WAITING_UI' and CITIES[16]:GetOwner()==1,
          'stale borrowed-city cache became a permanent BLOCKED result')
        CITIES[16].GetBuildings=originalBuildings
        ExposedMembers.JAC_HeroReadAPI.ReadCity=nil
        assert(A.Resume(PLAN.ID).Status=='WAITING_UI' and TRANSFERS==1)
        PUBLISHED[carrier.BorrowToken]=true
        pending=A.Resume(PLAN.ID)
        carrier=assert(pending.NativeWorks[1].Carrier)
        assert(carrier.Phase=='MOVE' and carrier.MoveSource.City==CITIES[4]:GetID()
          and carrier.MoveTarget.City==CITIES[16]:GetID() and TRANSFERS==1,
          'borrow publication did not prepare same-owner MOVE: phase='..carrier.Phase..
          ' source='..tostring(carrier.MoveSource and carrier.MoveSource.City)..
          ' target='..tostring(carrier.MoveTarget and carrier.MoveTarget.City)..
          ' transfers='..TRANSFERS)
        assert(A.Resume(PLAN.ID).Status=='WAITING_UI' and ENACT_CALLS==0)
        WORK_CITIES[4],WORK_CITIES[16]=nil,true -- the UI same-owner MOVE's physical effect
        PUBLISHED[carrier.MoveToken]=true
        PLOTS[17].owner=2
        local unsafe=pcall(A.Resume,PLAN.ID)
        assert(not unsafe and TRANSFERS==1 and CITIES[16]:GetOwner()==1
          and #A.NativeWorkLocations(900)==1 and A.NativeWorkLocations(900)[1].Owner==1,
          'changed purchased plot was carried back before the original city was protected')
        PLOTS[17].owner=1
        pending=A.Resume(PLAN.ID)
        carrier=assert(pending.NativeWorks[1].Carrier)
        assert(carrier.Phase=='RETURN' and TRANSFERS==2 and CITIES[16]:GetOwner()==3)
        PUBLISHED[carrier.ReturnToken]=true
        PLOTS[17].owner=2
        local changed=pcall(A.Resume,PLAN.ID)
        assert(not changed and CITIES[4]:GetOwner()==1 and CITIES[8]:GetOwner()==1
          and A.GetState().Archive['native-work:900']==nil,
          'carrier purchased plot changed without stopping before donor mutation')
        PLOTS[17].owner=3
        local transfer=CityManager.TransferCity
        local stopped=false
        CityManager.TransferCity=function(city,owner)
          if city==CITIES[4] and not stopped then
            stopped=true;error('staged after carrier proof')
          end
          return transfer(city,owner)
        end
        local advanced,why=pcall(A.Resume,PLAN.ID)
        assert(not advanced and tostring(why):find('staged after carrier proof',1,true)
          and A.GetState().Transactions[PLAN.ID].NativeWorks[1].Carrier.Phase=='PROVED',
          'test did not interrupt after carrier proof and before donor city transfer')
        local originalRead=ExposedMembers.JAC_HeroReadAPI.ReadNativeWork
        ExposedMembers.JAC_HeroReadAPI.ReadNativeWork=function(owner,id,index)
          if owner==3 and id==CITIES[16]:GetID() then error('UI city cache is not ready') end
          return originalRead(owner,id,index)
        end
        local waiting=A.Resume(PLAN.ID)
        assert(waiting.Status=='WAITING_UI' and waiting.AwaitingFinalUI
          and waiting.NativeWorks[1].Carrier.RefreshRequested,
          'stale UI cache after proved carrier became permanent BLOCKED')
        ExposedMembers.JAC_HeroReadAPI.ReadNativeWork=originalRead
        PUBLISHED[PLAN.ID..':final']=true
        local complete=A.Resume(PLAN.ID)
        assert(complete.Status=='COMPLETE' and ENACT_CALLS==0)
        local scattered=assert(A.GetState().Archive['native-work:900'])
        assert(scattered.Owner==-1 and scattered.Outcome=='SCATTER' and scattered.City==nil)
        assert(scattered.NativeWork.Index==900 and scattered.NativeWork.Owner==3 and
          scattered.NativeWork.CreatorName=='Original creator')
        assert(scattered.NativeCustody.Owner==3 and scattered.NativeCustody.X==16 and
          scattered.NativeCustody.City==CITIES[16]:GetID() and
          scattered.NativeCustody.Building==1 and scattered.NativeCustody.Slot==0)
        assert(CITIES[4]:GetOwner()==2 and CITIES[8]:GetOwner()==2)
        assert(#A.NativeWorkLocations(900)==1 and A.NativeWorkLocations(900)[1].Owner==3)
        local key='recovery:native-work:900'
        local reclaim=A.RecoverArchived('native-work:900',2,key,nil)
        assert(reclaim.Status=='WAITING_UI' and ENACT_CALLS==0
          and reclaim.NativeCarrier.Phase=='BORROW' and CITIES[16]:GetOwner()==2)
        assert(A.RecoverArchived('native-work:900',2,key,nil).Status=='WAITING_UI'
          and ENACT_CALLS==0)
        PUBLISHED[reclaim.NativeCarrier.BorrowToken]=true
        reclaim=A.RecoverArchived('native-work:900',2,key,nil)
        assert(reclaim.NativeCarrier.Phase=='MOVE' and CITIES[16]:GetOwner()==2)
        WORK_CITIES[16],WORK_CITIES[12]=nil,true -- human same-owner MOVE
        PUBLISHED[reclaim.NativeCarrier.MoveToken]=true
        reclaim=A.RecoverArchived('native-work:900',2,key,nil)
        assert(reclaim.Status=='WAITING_UI' and reclaim.NativeCarrier.Phase=='RETURN'
          and CITIES[16]:GetOwner()==3)
        PUBLISHED[reclaim.NativeCarrier.ReturnToken]=true
        local recovered=A.RecoverArchived('native-work:900',2,key,nil)
        assert(recovered.Status=='COMPLETE' and ENACT_CALLS==0)
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
          and ENACT_CALLS==0)
        "#,
    )
    .exec()
}
