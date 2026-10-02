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
    lua.load(&source).set_name(name).exec()
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
        local A=assert(ExposedMembers.JAC_Absorption)
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
        local A=ExposedMembers.JAC_Absorption
        local F=assert(ExposedMembers.JAC_AbsorptionFlow)
        for i=1,100 do RANDOM_QUEUE[#RANDOM_QUEUE+1]=9999 end
        local plan={ID='native-pending',From=1,To=2,Mode='MERGER',EntryConditionsMet=true,RelationsResolved=true}
        local flow=F.Begin(plan)
        assert(flow.Phase=='CHOICE' and flow.Phase1Complete)
        local native,previous=nil,nil
        for _,entry in ipairs(flow.Entries) do
          assert(not previous or previous<entry.ID,'inventory lost stable-ID order')
          previous=entry.ID
          if entry.ID=='native-work:1007' then native=entry end
        end
        assert(native and native.Kind=='ITEM' and native.NativeWork.Index==1007)
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
        local A=ExposedMembers.JAC_Absorption
        local F=ExposedMembers.JAC_AbsorptionFlow
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
        local A=ExposedMembers.JAC_Absorption
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
        local A=ExposedMembers.JAC_Absorption
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
        local A=ExposedMembers.JAC_Absorption
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
        local A=ExposedMembers.JAC_Absorption
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
        Events={GameCoreEventPublishComplete=event(),LoadScreenClose=event()}
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
        local A=ExposedMembers.JAC_Absorption
        local F=ExposedMembers.JAC_AbsorptionFlow
        local reads=assert(ExposedMembers.JAC_HeroReadAPI)
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
        "#,
    )
    .exec()
}
