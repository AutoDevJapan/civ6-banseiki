//! Real Lua boundary checks for native-work inventory and native person state.
//! The world fixture's repeated fake work index is corrected only inside this
//! test: a native Great Work index can occupy exactly one slot globally.

use mlua::Lua;
use std::{fs, path::PathBuf};

const WORLD: &str = include_str!("fixtures/absorption_world.lua");

fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("poc/JAC_PoC_Absorption/Scripts")
        .join(name)
}

fn load(lua: &Lua, name: &str) -> mlua::Result<()> {
    let source = fs::read_to_string(path(name)).map_err(mlua::Error::external)?;
    let result: mlua::Value = lua.load(&source).set_name(name).eval()?;
    match name {
        "JAC_Absorption_Flow.lua" => lua.globals().set("TEST_FLOW", result)?,
        "JAC_Absorption_Gameplay.lua" => lua.globals().set("TEST_ABSORPTION", result)?,
        _ => {}
    }
    Ok(())
}

fn load_hero_keys(lua: &Lua) -> mlua::Result<()> {
    let source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("poc/JAC_PoC_Absorption/UI/JAC_Absorption_HeroKeys.lua"),
    )
    .map_err(mlua::Error::external)?;
    lua.load(&source)
        .set_name("JAC_Absorption_HeroKeys.lua")
        .exec()
}

#[test]
fn native_carrier_ui_uses_local_actor_once_and_publishes_only_after_physical_move()
-> mlua::Result<()> {
    let lua = Lua::new();
    lua.load(WORLD).call::<()>(lua.create_table()?)?;
    lua.load("function include(_) end").exec()?;
    load(&lua, "JAC_Absorption_Rules.lua")?;
    load(&lua, "JAC_Absorption_Gameplay.lua")?;
    lua.load(
        r#"
        local function event()
          local callbacks={}
          return {Add=function(fn) callbacks[#callbacks+1]=fn end,
            Emit=function(...) for _,fn in ipairs(callbacks) do fn(...) end end}
        end
        Events={GameCoreEventPublishComplete=event(),LoadScreenClose=event(),
          LocalPlayerTurnBegin=event()}
        PlayerManager.GetAlive=function() return {Players[1],Players[2],Players[3]} end
        CityManager.TransferCity(CITIES[16],1)
        WORK_CITIES={[4]=true}
        for _,city in ipairs({CITIES[4],CITIES[16]}) do
          local original=city.GetBuildings
          city.GetBuildings=function(self)
            local buildings=original(self)
            buildings.GetGreatWorkInSlot=function(_,building,slot)
              return building==1 and slot==0 and WORK_CITIES[city:GetX()] and 900 or -1
            end
            buildings.GetGreatWorkTypeFromIndex=function(_,index)
              assert(index==900);return 4
            end
            buildings.GetGreatWorkSlotType=function(_,building,slot)
              assert(building==1 and slot==0);return 1
            end
            return buildings
          end
        end
        GameInfo.GreatWorks={[4]={GreatWorkObjectType='GREATWORKOBJECT_RELIC'}}
        GameInfo.GreatWorkSlotTypes={[1]={GreatWorkSlotType='GREATWORKSLOT_RELIC'}}
        GameInfo.GreatWork_ValidSubTypes=function()
          local read=false;return function()
            if not read then read=true;return {GreatWorkObjectType='GREATWORKOBJECT_RELIC',
              GreatWorkSlotType='GREATWORKSLOT_RELIC'} end
          end
        end
        PlayerOperations={MOVE_GREAT_WORK='move',PARAM_PLAYER_ONE='owner',
          PARAM_CITY_SRC='source',PARAM_CITY_DEST='target',
          PARAM_BUILDING_SRC='source-building',PARAM_BUILDING_DEST='target-building',
          PARAM_GREAT_WORK_INDEX='index',PARAM_SLOT='slot'}
        UI_REQUESTS=0
        MOVE_EFFECT=true
        UI={RequestPlayerOperation=function(actor,operation,params)
          assert(actor==2 and operation=='move' and params.owner==1
            and params.source==CITIES[4]:GetID()
            and params.target==CITIES[16]:GetID()
            and params['source-building']==1 and params['target-building']==1
            and params.index==900 and params.slot==0,
            'UI MOVE did not use local human actor with AI owner and exact slots')
          UI_REQUESTS=UI_REQUESTS+1
          if MOVE_EFFECT then WORK_CITIES[4],WORK_CITIES[16]=nil,true end
          return true
        end}
        local carrier={Route='TO_FREE',Phase='MOVE',Borrower=1,
          BorrowedCity=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0,Index=900,
          MoveToken='native:move:1',
          MoveSource={Owner=1,City=CITIES[4]:GetID(),X=4,Y=0,Building=1,Slot=0},
          MoveTarget={Owner=1,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0}}
        local state=JAC_AbsorptionGameplay.GetState()
        state.Transactions['carrier-ui']={ID='carrier-ui',NativeWorks={{Carrier=carrier}}}
        Game:SetProperty(JAC_AbsorptionGameplay.Rules.STATE_KEY,state)
        "#,
    )
    .exec()?;
    load_hero_keys(&lua)?;
    lua.load(
        r#"
        local reads=assert(ExposedMembers.JAC_HeroReadAPI)
        assert(reads.NativeMoveAvailable)
        Events.GameCoreEventPublishComplete.Emit()
        assert(UI_REQUESTS==1 and WORK_CITIES[16] and not WORK_CITIES[4])
        assert(reads.GetObservation('native:move:1')==nil,
          'UI claimed physical success in the same event as dispatch')
        Events.GameCoreEventPublishComplete.Emit()
        assert(reads.GetObservation('native:move:1').Published
          and not reads.GetObservation('native:move:1').Unchanged)
        Events.GameCoreEventPublishComplete.Emit()
        assert(UI_REQUESTS==1,'published work sent another MOVE')

        WORK_CITIES[4],WORK_CITIES[16]=true,nil
        MOVE_EFFECT=false
        local state=JAC_AbsorptionGameplay.GetState()
        state.Transactions['carrier-ui'].NativeWorks[1].Carrier.MoveToken='native:move:2'
        Game:SetProperty(JAC_AbsorptionGameplay.Rules.STATE_KEY,state)
        Events.GameCoreEventPublishComplete.Emit()
        assert(UI_REQUESTS==2 and not reads.GetObservation('native:move:2'))
        Events.GameCoreEventPublishComplete.Emit()
        assert(not reads.GetObservation('native:move:2'))
        Events.GameCoreEventPublishComplete.Emit()
        assert(reads.GetObservation('native:move:2').Unchanged
          and WORK_CITIES[4] and not WORK_CITIES[16],
          'true without physical movement was reported as success')
        assert(UI_REQUESTS==2,'unchanged work retried without a saved new token')

        local state=JAC_AbsorptionGameplay.GetState()
        local carrier=state.Transactions['carrier-ui'].NativeWorks[1].Carrier
        carrier.Phase='RETURN';carrier.Owner=3;carrier.ReturnToken='missing:return'
        Game:SetProperty(JAC_AbsorptionGameplay.Rules.STATE_KEY,state)
        CityManager.TransferCity(CITIES[16],3)
        CITIES[16].monument=false
        Events.GameCoreEventPublishComplete.Emit()
        assert(not reads.GetObservation('missing:return'))
        Events.GameCoreEventPublishComplete.Emit()
        assert(reads.GetObservation('missing:return').Unchanged,
          'missing carrier building left the return token unpublished forever')
        "#,
    )
    .exec()
}

#[test]
fn native_work_metadata_survives_ui_only_getters() -> mlua::Result<()> {
    let lua = Lua::new();
    lua.load(WORLD).call::<()>(lua.create_table()?)?;
    lua.load("function include(_) end").exec()?;
    load(&lua, "JAC_Absorption_Rules.lua")?;
    load(&lua, "JAC_Absorption_Gameplay.lua")?;
    lua.load(
        r#"
        for _,city in pairs(CITIES) do
          local original=city.GetBuildings
          city.GetBuildings=function(self)
            local buildings=original(self)
            buildings.GetGreatWorkInSlot=function(_,building)
              return city:GetX()==4 and building==1 and 1007 or -1
            end
            return buildings
          end
        end
        GameInfo.GreatWorks={[3]={GreatWorkType='GREATWORK_TEST_RELIC',
          GreatWorkObjectType='GREATWORKOBJECT_RELIC',Name='LOC_TEST_RELIC',Tourism=8}}
        ExposedMembers.JAC_HeroReadAPI={ReadNativeWork=function(owner,id,index)
          assert(owner==1 and id==CITIES[4]:GetID() and index==1007)
          return {Type=3,CreatorName='Original creator',TurnCreated=2,CreatedByPlayer=0}
        end}
        assert(CITIES[4]:GetBuildings().GetGreatWorkTypeFromIndex==nil)
        local works=TEST_ABSORPTION.OwnedGreatWorkSlots(1)
        assert(#works==1 and works[1].Index==1007)
        local work=works[1]
        assert(work.GreatWorkType=='GREATWORK_TEST_RELIC' and work.Name=='LOC_TEST_RELIC')
        assert(work.ObjectType=='GREATWORKOBJECT_RELIC' and work.Effects[1]=='TOURISM 8')
        assert(work.CreatedByPlayer==0 and work.CreatorName=='Original creator'
          and work.TurnCreated==2,'native work provenance was lost across UI reads')
        "#,
    )
    .exec()
}

#[test]
fn native_work_is_one_sorted_item_and_survives_pending_choice_reload() -> mlua::Result<()> {
    let lua = Lua::new();
    lua.load(WORLD).call::<()>(lua.create_table()?)?;
    lua.load("function include(_) end").exec()?;
    load(&lua, "JAC_Absorption_Rules.lua")?;
    load(&lua, "JAC_Absorption_Gameplay.lua")?;
    lua.load(
        r#"
        for _,city in ipairs({CITIES[8],CITIES[12],CITIES[16]}) do
          local original=city.GetBuildings
          city.GetBuildings=function(self)
            local buildings=original(self)
            buildings.GetGreatWorkInSlot=function() return -1 end
            return buildings
          end
        end
        Game:SetProperty('JAC_ABSORPTION_FLOW_TEST_MODE',1)
        local general=assert(UnitManager.GetUnit(1,8))
        local row={UnitAbilityType='ABILITY_NATIVE_GENERAL',
          Description='LOC_TEST_NATIVE_GENERAL_ABILITY'}
        GameInfo.UnitAbilities=setmetatable({ABILITY_NATIVE_GENERAL=row},
          {__call=function()
            local called=false
            return function()
              if called then return nil end
              called=true
              return row
            end
          end})
        general.GetAbility=function()
          return {GetAbilityCount=function(_,name)
            return name=='ABILITY_NATIVE_GENERAL' and 2 or 0
          end}
        end
        GameInfo.GreatPersonIndividuals[42].ActionEffectTextOverride=
          'LOC_TEST_NATIVE_GENERAL_ACTION'
        local A=assert(TEST_ABSORPTION)
        local R=A.Rules
        assert(R.UnitKind(general)=='PERSON')
        assert(R.UnitKind(assert(UnitManager.GetUnit(1,3)))=='PERSON')
        local scientist=UNIT(1,30,'UNIT_GREAT_SCIENTIST',4);scientist.individual=43
        local engineer=UNIT(1,31,'UNIT_GREAT_ENGINEER',4);engineer.individual=44
        assert(R.UnitKind(scientist)=='GREAT_PERSON' and R.UnitKind(engineer)=='GREAT_PERSON')
        scientist.individual=-1
        assert(R.UnitKind(scientist)=='UNIT','metadata-only Great Person was accepted')
        UnitManager.Kill(scientist);UnitManager.Kill(engineer)
        local snap=A.SnapshotUnit(general)
        assert(snap.Kind=='PERSON' and snap.Individual==42)
        assert(snap.AbilityCounts.ABILITY_NATIVE_GENERAL==2)
        local labels=A.UnitAbilityLabels(general)
        assert(labels[1]=='LOC_TEST_NATIVE_GENERAL_ABILITY'
          and labels[2]=='LOC_TEST_NATIVE_GENERAL_ACTION')
        "#,
    )
    .exec()?;
    load(&lua, "JAC_Absorption_Flow.lua")?;
    lua.load(
        r#"
        local A=TEST_ABSORPTION
        local F=assert(TEST_FLOW)
        local work=assert(A.OwnedGreatWorkSlots(1)[1])
        local roster=A.GetRoster(1)
        roster['native-work:1007']={ID='native-work:1007',Kind='ITEM',Owner=1,
          Outcome='RECOVERED',NativeWork=A.Rules.Copy(work),
          NativeCustody={Owner=1,City=work.CityID,X=work.X,Y=work.Y,
            Building=work.Building,Slot=work.Slot},City={Owner=1,X=work.X,Y=work.Y}}
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,roster)
        for i=1,100 do RANDOM_QUEUE[#RANDOM_QUEUE+1]=9999 end
        local plan={ID='native-pending',From=1,To=2,Mode='MERGER',EntryConditionsMet=true,RelationsResolved=true}
        local corrupted=A.Rules.Copy(roster)
        corrupted['native-work:1007'].NativeWork.CreatorName='forged creator'
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,corrupted)
        assert(not pcall(F.Begin,plan),'changed creator was accepted for a tracked work')
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,roster)
        local flow=F.Begin(plan)
        assert(flow.Phase=='CHOICE' and flow.Phase1Complete)
        local native,previous,count=nil,nil,0
        for _,entry in ipairs(flow.Entries) do
          assert(not previous or previous<entry.ID,'inventory lost stable-ID order')
          previous=entry.ID
          if entry.ID=='native-work:1007' then native=entry;count=count+1 end
        end
        assert(count==1 and native and native.Kind=='ITEM' and native.NativeWork.Index==1007,
          'recovered work was duplicated or rejected in the next absorption')
        assert(native.EscapeOutcome==nil)
        assert(native.NativeWork.Owner==1 and native.NativeWork.X==4)
        assert(native.Rolls.P1_ESCAPE and native.Rolls.P1_ESCAPE.Label)
        assert(A.OwnedGreatWorks(1)[1007] and not A.OwnedGreatWorks(2)[1007],
          'phase-one inventory changed the original native work')
        local before=A.Canonical(flow)
        local rolls=#RANDOM_CALLS
        Game.props=A.Rules.Copy(Game.props)
        for owner=0,3 do Players[owner].props=A.Rules.Copy(Players[owner].props) end
        NATIVE_PENDING_EXPECT={Before=before,Rolls=rolls,Plan=plan}
        "#,
    )
    .exec()?;
    load(&lua, "JAC_Absorption_Rules.lua")?;
    load(&lua, "JAC_Absorption_Gameplay.lua")?;
    load(&lua, "JAC_Absorption_Flow.lua")?;
    lua.load(
        r#"
        local A=TEST_ABSORPTION
        local F=TEST_FLOW
        assert(A.Canonical(F.Get('native-pending'))==NATIVE_PENDING_EXPECT.Before)
        assert(A.Canonical(F.Begin(NATIVE_PENDING_EXPECT.Plan))==NATIVE_PENDING_EXPECT.Before)
        assert(#RANDOM_CALLS==NATIVE_PENDING_EXPECT.Rolls,
          'pending native work rerolled after save/load')
        assert(A.OwnedGreatWorks(1)[1007])
        "#,
    )
    .exec()
}

#[test]
fn recovered_native_hero_waits_for_published_claim_after_reload() -> mlua::Result<()> {
    let lua = Lua::new();
    lua.load(WORLD).call::<()>(lua.create_table()?)?;
    lua.load("function include(_) end").exec()?;
    load(&lua, "JAC_Absorption_Rules.lua")?;
    load(&lua, "JAC_Absorption_Gameplay.lua")?;
    lua.load(
        r#"
        local A=TEST_ABSORPTION
        local R=A.Rules
        local state=A.GetState()
        state.Archive.dormant={ID='dormant',Kind='HERO',Owner=-1,
          NativeHero={Class=0,ClassType='HEROCLASS_JAC_WARLORD_A',
            Origin={Owner=1,ID=12,X=12,Y=0}}}
        Game:SetProperty(R.STATE_KEY,state)
        local first=A.RecoverArchived('dormant',2,'recovery:dormant',nil)
        assert(first.Status=='WAITING_UI' and first.AwaitingUI)
        assert(first.HeroClass==0 and first.ObservationToken and first.CandidateID)
        assert(HERO_CREATIONS==1 and A.GetRoster(2).dormant==nil,
          'unpublished Hero was reported as recovered')
        assert(UnitManager.GetUnit(2,first.CandidateID),
          'unpublished native candidate was incorrectly removed')
        Game.props=R.Copy(Game.props)
        for owner=0,3 do Players[owner].props=R.Copy(Players[owner].props) end
        NATIVE_HERO_OBSERVATION={Token=first.ObservationToken,ID=first.CandidateID}
        "#,
    )
    .exec()?;
    load(&lua, "JAC_Absorption_Rules.lua")?;
    load(&lua, "JAC_Absorption_Gameplay.lua")?;
    lua.load(
        r#"
        ExposedMembers.JAC_HeroReadAPI={GetObservation=function(token)
          if token==NATIVE_HERO_OBSERVATION.Token then
            return {Claim=2,Origin={player=2,id=12}}
          end
        end}
        local A=TEST_ABSORPTION
        local complete=A.RecoverArchived('dormant',2,'recovery:dormant',nil)
        assert(complete.Status=='COMPLETE' and HERO_CREATIONS==1)
        assert(complete.Record.Unit.ID==NATIVE_HERO_OBSERVATION.ID)
        assert(A.GetRoster(2).dormant.RecoveryToken=='recovery:dormant')
        assert(A.RecoverArchived('dormant',2,'recovery:dormant',nil).Status=='COMPLETE')
        assert(HERO_CREATIONS==1,'recovery duplicated the native Hero')
        "#,
    )
    .exec()
}

#[test]
fn live_hero_recovery_restores_native_charges_before_publication() -> mlua::Result<()> {
    let lua = Lua::new();
    lua.load(WORLD).call::<()>(lua.create_table()?)?;
    lua.load("function include(_) end").exec()?;
    load(&lua, "JAC_Absorption_Rules.lua")?;
    load(&lua, "JAC_Absorption_Gameplay.lua")?;
    lua.load(
        r#"
        local A=TEST_ABSORPTION
        local R=A.Rules
        local hero=UNIT(1,13,'UNIT_HERO_JAC_WARLORD_A',4)
        hero.lifespan=17;hero.charges=2;hero.renown=13
        hero.damage=7;hero.moves=1
        HERO_CLAIMS[0]=1;HERO_ORIGINS[0]={player=1,id=CITIES[4].id}
        HERO_DISCOVERED['1:0']=true
        local record={ID='escaped',Kind='HERO',Owner=1,Unit={Owner=1,ID=13}}
        hero:SetProperty(R.PERSON_KEY,record)
        local snapshot=A.SnapshotUnit(hero)
        UnitManager.Kill(hero)
        CityManager.TransferCity(CITIES[4],2)
        record.Owner=-1;record.NativeHero=R.Copy(snapshot.Hero)
        local state=A.GetState();state.Archive.escaped=record
        Game:SetProperty(R.STATE_KEY,state)
        local pending=A.RecoverArchived('escaped',2,'recovery:escaped',snapshot)
        assert(pending.Status=='WAITING_UI' and pending.CoreRestored
          and HERO_CREATIONS==1 and A.GetRoster(2).escaped==nil)
        local candidate=assert(UnitManager.GetUnit(2,pending.CandidateID))
        assert(candidate.lifespan==17 and candidate.charges==2
          and candidate.renown==13 and candidate.damage==7 and candidate.moves==1,
          'unpublished native Hero became usable with fresh default state')
        local pendingPerson=assert(candidate:GetProperty(R.PERSON_KEY))
        assert(pendingPerson.ID=='escaped' and pendingPerson.Owner==2
          and pendingPerson.Unit.ID==candidate:GetID(),
          'unpublished Hero lacked its recovered person identity')
        local token=pending.ObservationToken
        ExposedMembers.JAC_HeroReadAPI={GetObservation=function(wanted)
          if wanted==token then return {Claim=2,Origin={player=2,id=CITIES[4].id}} end
        end}
        local complete=A.RecoverArchived('escaped',2,'recovery:escaped',snapshot)
        assert(complete.Status=='COMPLETE' and HERO_CREATIONS==1)
        assert(A.GetRoster(2).escaped.Unit.ID==candidate:GetID())
        assert(candidate.lifespan==17 and candidate.charges==2 and candidate.renown==13)
        "#,
    )
    .exec()
}

#[test]
fn ui_publication_resumes_claimed_hero_without_extra_creation_and_delays_city_reads()
-> mlua::Result<()> {
    let lua = Lua::new();
    lua.load(WORLD).call::<()>(lua.create_table()?)?;
    lua.load("function include(_) end").exec()?;
    load(&lua, "JAC_Absorption_Rules.lua")?;
    load(&lua, "JAC_Absorption_Gameplay.lua")?;
    load(&lua, "JAC_Absorption_Flow.lua")?;
    lua.load(
        r#"
        local A=TEST_ABSORPTION
        local R=A.Rules
        local state=A.GetState()
        state.Archive.dormant={ID='dormant',Kind='HERO',Owner=-1,
          NativeHero={Class=0,ClassType='HEROCLASS_JAC_WARLORD_A',
            Origin={Owner=1,ID=12,X=12,Y=0}}}
        Game:SetProperty(R.STATE_KEY,state)
        local first=A.RecoverArchived('dormant',2,'recovery:world:dormant',nil)
        assert(first.Status=='WAITING_UI' and first.AwaitingUI and HERO_CREATIONS==1)
        local flow={Version=1,Flows={},World={
          ['world:dormant']={ID='world:dormant',AssetID='dormant',Kind='HERO',
            Status='CLAIMED',Recipient=2,WaitingFor=first.ObservationToken}}}
        Game:SetProperty('JAC_ABSORPTION_FLOW_V1',flow)
        Game.props=R.Copy(Game.props)
        for owner=0,3 do Players[owner].props=R.Copy(Players[owner].props) end
        "#,
    )
    .exec()?;
    load(&lua, "JAC_Absorption_Rules.lua")?;
    load(&lua, "JAC_Absorption_Gameplay.lua")?;
    load(&lua, "JAC_Absorption_Flow.lua")?;
    lua.load(
        r#"
        local function event()
          local handlers={}
          return {Add=function(fn) handlers[#handlers+1]=fn end,
            Emit=function(...) for _,fn in ipairs(handlers) do fn(...) end end}
        end
        Events={GameCoreEventPublishComplete=event(),LoadScreenClose=event(),LocalPlayerTurnBegin=event()}
        PlayerManager.GetAlive=function() return {Players[1],Players[2],Players[3]} end
        PlayerOperations={EXECUTE_SCRIPT=1}
        UI_REQUESTS=0
        UI={RequestPlayerOperation=function(actor,kind,payload)
          assert(actor==2 and kind==1)
          UI_REQUESTS=UI_REQUESTS+1
          GameEvents.JAC_AbsorptionFlow_Operation.Emit(actor,payload)
        end}
        UI_CACHE_READY=false
        local manager=HERO_MANAGER
        local cache=setmetatable({GetHeroOriginCityID=function(_,class)
          if UI_CACHE_READY then return manager:GetHeroOriginCityID(class) end
          return nil
        end},{__index=manager})
        Game.GetHeroesManager=function() return cache end
        "#,
    )
    .exec()?;
    load_hero_keys(&lua)?;
    lua.load(
        r#"
        local A=TEST_ABSORPTION
        local F=TEST_FLOW
        local reads=assert(ExposedMembers.JAC_HeroReadAPI)
        local R=A.Rules
        assert(reads.ReadCity(2,CITIES[12].id,'LOYALTY')==87)
        local cities=Players[2].GetCities
        Players[2].GetCities=function(self)
          local collection=cities(self)
          collection.FindID=function() return nil end
          return collection
        end
        local request={Token='city-delay',Owner=2,ID=CITIES[12].id,Turn=CURRENT_TURN}
        reads.ObserveCity(request)
        assert(reads.GetObservation(request.Token)==nil,'unpublished city cache was accepted')
        Players[2].GetCities=cities
        reads.ObserveCity(request)
        assert(reads.GetObservation(request.Token).Value==87)
        Events.GameCoreEventPublishComplete.Emit()
        assert(F.GetWorld('world:dormant').Status=='CLAIMED' and UI_REQUESTS==0)
        assert(A.GetState().Recoveries['recovery:world:dormant'].Status=='WAITING_UI')
        local waiting=F.GetWorld('world:dormant')
        assert(not pcall(F.ResumeWorldFor,1,'world:dormant',waiting.WaitingFor))
        assert(not pcall(F.ResumeWorldFor,2,'world:dormant',waiting.WaitingFor),
          'unpublished Hero was resumed')
        local ownedCities=Players[2].cities
        local pending=R.Copy(A.GetState().Recoveries['recovery:world:dormant'])
        Players[2].cities={};Players[2].IsAlive=function() return true end
        assert(pcall(F.Tick),'cityless recipient stopped the world tick')
        local blocked=F.GetWorld('world:dormant')
        assert(blocked.Status=='BLOCKED' and blocked.Recipient==2
          and blocked.WaitingFor==waiting.WaitingFor)
        assert(R.Equal(pending,A.GetState().Recoveries['recovery:world:dormant'])
          and HERO_CREATIONS==1,'lost recipient changed the pending native Hero')
        assert(pcall(F.Tick) and F.GetWorld('world:dormant').Status=='BLOCKED')
        Game.props=R.Copy(Game.props)
        Players[2].cities=ownedCities
        local ok,why=pcall(F.Tick)
        local resumed=F.GetWorld('world:dormant')
        assert(ok and resumed.Status=='CLAIMED',tostring(why)..':'..resumed.Status..':'..tostring(resumed.Error))
        UI_CACHE_READY=true
        Events.GameCoreEventPublishComplete.Emit()
        local world=F.GetWorld('world:dormant')
        assert(world.Status=='COMPLETE' and UI_REQUESTS==1,tostring(world.Error))
        assert(HERO_CREATIONS==1 and A.GetRoster(2).dormant.RecoveryToken=='recovery:world:dormant')
        Events.GameCoreEventPublishComplete.Emit()
        assert(HERO_CREATIONS==1 and UI_REQUESTS==1 and #F.GetPendingWorlds(2)==0,
          'duplicate publication made another Hero or operation')
        assert(not pcall(F.ResumeWorldFor,1,'world:dormant',world.WaitingFor),
          'another actor resumed a human reward')
        local state=A.GetState()
        state.Archive.manual={ID='manual',Kind='HERO',Owner=-1,
          NativeHero={Class=1,ClassType='HEROCLASS_JAC_WARLORD_B',Origin={Owner=1,ID=12,X=12,Y=0}}}
        Game:SetProperty(A.Rules.STATE_KEY,state)
        assert(A.RecoverArchived('manual',2,'manual:recovery',nil).Status=='WAITING_UI')
        Events.GameCoreEventPublishComplete.Emit()
        assert(UI_REQUESTS==1,'manual Tuner intent caused an automatic operation loop')
        assert(A.GetState().Recoveries['manual:recovery'].Status=='WAITING_UI')
        assert(A.RecoverArchived('manual',2,'manual:recovery',nil).Status=='COMPLETE',
          'manual Tuner intent did not retain its published observation')
        "#,
    )
    .exec()
}

fn native_scenario(name: &str) -> serde_json::Value {
    let source = fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../civ6-autotest/scenarios")
            .join(name),
    )
    .expect("native experiment scenario");
    serde_json::from_str(&source).expect("valid native scenario JSON")
}

#[test]
fn native_experiment_scripts_compile_in_lua51() -> mlua::Result<()> {
    let lua = Lua::new();
    let mut checked = 0;
    for name in [
        "absorption-native-custody.json",
        "absorption-native-deal.json",
        "absorption-native-product-probe.json",
        "absorption-native-product-resume.json",
        "absorption-native-product-finish.json",
        "absorption-native-scatter-inventory.json",
        "absorption-native-scatter-carrier.json",
        "absorption-native-scatter-flow-start.json",
        "absorption-native-scatter-physical-probe.json",
        "absorption-native-bridge-probe.json",
        "absorption-native-ai-same-owner-move.json",
        "absorption-native-ai-carrier-return.json",
        "absorption-native-carrier-side-effects.json",
        "absorption-native-cross-owner-freecity.json",
        "absorption-native-freecity-recover-original.json",
        "absorption-native-reverse-carrier-recovery.json",
        "absorption-native-work-probe.json",
    ] {
        let scenario = native_scenario(name);
        for step in scenario["steps"].as_array().expect("scenario steps") {
            for code in [step["code"].as_str(), step["condition"]["code"].as_str()]
                .into_iter()
                .flatten()
            {
                lua.load(code).set_name(name).into_function()?;
                checked += 1;
            }
        }
    }
    assert!(checked >= 25, "experiment scripts were not collected");
    Ok(())
}

// This fixture tests the scenario's physical oracle, not engine API availability.
// No operation in this fixture moves a work; nil dispatch and events alone must
// never be accepted as physical success.
fn native_matrix_oracle() -> mlua::Result<Lua> {
    let lua = Lua::new();
    lua.load(
        r#"
        ExposedMembers={}
        Game={GetProperty=function() return {FreeCity=3} end,GetLocalPlayer=function() return 1 end}
        local function city(owner,id,x)
          local c={owner=owner,id=id,x=x,slots={[1]={[0]=-1,[1]=-1},[2]={[0]=-1,[1]=-1}}}
          function c:GetOwner() return self.owner end
          function c:GetID() return self.id end
          function c:GetX() return self.x end
          function c:GetY() return 0 end
          function c:GetBuildings()
            return {
              HasBuilding=function() return true end,
              GetNumGreatWorkSlots=function() return 2 end,
              GetGreatWorkInSlot=function(_,b,s) return c.slots[b][s] end,
              GetGreatWorkTypeFromIndex=function() return 1 end,
              GetGreatWorkSlotType=function() return 1 end
            }
          end
          return c
        end
        ORACLE_CITIES={city(1,10,1),city(1,20,2),city(2,30,3),city(3,40,4)}
        ORACLE_CITIES[1].slots[1][0]=0
        Players={}
        for owner=1,3 do
          local owned={}
          for _,c in ipairs(ORACLE_CITIES) do if c.owner==owner then owned[#owned+1]=c end end
          Players[owner]={GetCities=function()
            return {Members=function() return ipairs(owned) end}
          end}
        end
        PlayerManager={GetAliveIDs=function() return {1,2} end}
        CityManager={GetCityAt=function(x,y) return ORACLE_CITIES[x] end}
        GameInfo={
          Buildings=function() local i=0;return function() i=i+1;if i<=2 then return {Index=i} end end end,
          GreatWorks={[1]={GreatWorkObjectType='GREATWORKOBJECT_RELIC'}},
          GreatWorkSlotTypes={[1]={GreatWorkSlotType='GREATWORKSLOT_RELIC'}},
          GreatWork_ValidSubTypes=function()
            local once=false;return function()
              if not once then once=true;return {GreatWorkSlotType='GREATWORKSLOT_RELIC',GreatWorkObjectType='GREATWORKOBJECT_RELIC'} end
            end
          end
        }
        Events={GreatWorkMoved={Add=function(fn) ORACLE_EVENT=fn end}}
        PlayerOperations={}
        for _,name in ipairs({'PARAM_PLAYER_ONE','PARAM_CITY_SRC','PARAM_CITY_DEST',
          'PARAM_BUILDING_SRC','PARAM_BUILDING_DEST','PARAM_GREAT_WORK_INDEX','PARAM_SLOT','MOVE_GREAT_WORK'}) do
          PlayerOperations[name]=name
        end
        UI={RequestPlayerOperation=function(actor,operation,params)
          assert(actor==1 and params.PARAM_PLAYER_ONE==1 and params.PARAM_GREAT_WORK_INDEX==0)
          return nil
        end}
        ORACLE_SOURCE={Owner=1,City=10,X=1,Y=0,Building=1,Slot=0}
        ORACLE_DEST={Owner=1,City=20,X=2,Y=0,Building=1,Slot=0}
        "#,
    )
    .exec()?;
    let scenario = native_scenario("absorption-native-custody.json");
    let helper = scenario["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .filter_map(|step| step["code"].as_str())
        .find(|code| code.contains("function M.Check("))
        .expect("physical oracle");
    let prefix = helper
        .split("\nlocal found\n")
        .next()
        .expect("oracle prefix");
    assert!(
        prefix.len() < helper.len(),
        "fixture must not dispatch the experiment"
    );
    lua.load(prefix).exec()?;
    Ok(lua)
}

#[test]
fn native_matrix_requires_unique_exact_index_and_empty_source() -> mlua::Result<()> {
    let lua = native_matrix_oracle()?;
    lua.load(
        r#"
        local M=ExposedMembers.JAC_NativeMatrix
        local s,d=ORACLE_SOURCE,ORACLE_DEST
        assert(PlayerManager.GetFreeCitiesPlayerID==nil,'UI-only oracle gained a Core getter')
        assert(not M.Check('unchanged',0,s,d))
        assert(M.Request('local-official',1,1,0,s,d))
        assert(ExposedMembers.JAC_NativeOfficialMoveProof==nil)
        ORACLE_EVENT(0,1,10,20)
        assert(not M.Verify(),'nil return or unrelated event proved movement')
        assert(ExposedMembers.JAC_NativeOfficialMoveProof==nil,'event alone created official move proof')
        ORACLE_CITIES[1].slots[1][0]=-1
        ORACLE_CITIES[2].slots[1][0]=0
        assert(M.Verify(),'exact index zero in destination was rejected')
        local proof=assert(ExposedMembers.JAC_NativeOfficialMoveProof)
        assert(proof.Label=='local-official' and proof.Index==0 and
          proof.Source.City==10 and proof.Dest.City==20 and proof.Dest.Slot==0)
        ORACLE_CITIES[1].slots[1][0]=0
        assert(not pcall(M.Verify),'duplicate source index was accepted')
        assert(ExposedMembers.JAC_NativeOfficialMoveProof==nil,'stale physical proof survived a failed read')
        ORACLE_CITIES[1].slots[1][0]=-1
        ORACLE_CITIES[4].slots[1][0]=0
        assert(not pcall(M.Verify),'duplicate index in Free Cities was ignored')
        ORACLE_CITIES[4].slots[1][0]=-1
        ORACLE_CITIES[2].slots[1][0]=7
        assert(not pcall(M.Verify),'replacement work was accepted')
        ORACLE_CITIES[2].slots[1][0]=-1
        ORACLE_CITIES[2].slots[1][1]=0
        assert(not pcall(M.Verify),'wrong physical slot was accepted')
        ORACLE_CITIES[2].slots[1][1]=-1
        ORACLE_CITIES[3].slots[1][0]=0
        assert(not pcall(M.Verify),'wrong physical owner was accepted')
        ORACLE_CITIES[3].slots[1][0]=-1
        ORACLE_CITIES[2].slots[1][0]=0
        ORACLE_CITIES[1].slots[1][0]=7
        assert(not pcall(M.Verify),'nonempty source was accepted')
        ORACLE_CITIES[1].slots[1][0]=-1
        ORACLE_CITIES[2].id=99
        assert(not pcall(M.Verify),'stale city ID after transfer was accepted')
        "#,
    )
    .exec()
}

#[test]
fn native_deal_rejects_an_unreserved_destination_slot() -> mlua::Result<()> {
    let lua = native_matrix_oracle()?;
    lua.load(
        r#"
        Game.GetProperty=function(_,key)
          if key=='JAC_NATIVE_DEAL_TARGET' then return 2 end
          if key=='JAC_NATIVE_DEAL_SLOT' then
            return {Owner=2,City=30,X=3,Y=0,Building=1,Slot=0}
          end
        end
        ExposedMembers.JAC_NativeDealData=function()
          return {Owner=1,Work={Index=0,City=10,X=1,Y=0,Building=1,Slot=0}}
        end
        ExposedMembers.JAC_NativeDealLocations=ExposedMembers.JAC_NativeMatrix.Locate
        ORACLE_CITIES[1].slots[1][0]=-1
        ORACLE_CITIES[3].slots[1][1]=0
    "#,
    )
    .exec()?;
    let scenario = native_scenario("absorption-native-deal.json");
    let steps = scenario["steps"].as_array().expect("steps");
    let condition = steps[steps.len() - 2]["condition"]["code"]
        .as_str()
        .expect("deal physical condition");
    let error = lua.load(condition).eval::<bool>().unwrap_err();
    assert!(
        error
            .to_string()
            .contains("deal ignored reserved destination slot"),
        "{error}"
    );
    lua.load("ORACLE_CITIES[3].slots[1][1]=-1;ORACLE_CITIES[3].slots[1][0]=0")
        .exec()?;
    assert!(lua.load(condition).eval::<bool>()?);
    Ok(())
}
