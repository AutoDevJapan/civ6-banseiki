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
        local plan={ID='native-pending',From=1,To=2,Mode='MERGER'}
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
