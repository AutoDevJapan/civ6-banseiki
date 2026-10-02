//! Real Lua integration tests for the three-stage absorption flow.
//! The world fixture models Civ6's mutation and save APIs. It deliberately
//! returns no Unit from InitUnitValidAdjacentHex, as the native binding does.

use mlua::Lua;
use std::{fs, path::PathBuf};

const WORLD: &str = include_str!("fixtures/absorption_world.lua");
const PANEL_WORLD: &str = include_str!("fixtures/absorption_panel.lua");
const FLOW: &str = "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Flow.lua";

fn repo_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn outcomes(lua: &Lua) -> mlua::Result<mlua::Table> {
    // Fixed game database fixture, independent of the runtime's choice logic.
    const ROWS: &[(&str, &str, &str)] = &[
        ("UNIT", "SURRENDER", "ABSORBER"),
        ("UNIT", "INCORPORATE", "ABSORBER"),
        ("UNIT", "DISSOLVE", "ARCHIVE"),
        ("UNIT", "ESCAPE", "ARCHIVE"),
        ("UNIT", "RESIST", "ARCHIVE"),
        ("PERSON", "CAPTURE", "ABSORBER"),
        ("PERSON", "RECRUIT", "ABSORBER"),
        ("PERSON", "SUBMIT", "ABSORBER"),
        ("PERSON", "EXILE", "ARCHIVE"),
        ("PERSON", "EXECUTE", "ARCHIVE"),
        ("PERSON", "RETIRE", "ARCHIVE"),
        ("PERSON", "DEFECT", "ARCHIVE"),
        ("PERSON", "HIDE", "ARCHIVE"),
        ("GOVERNOR", "CAPTURE", "ABSORBER"),
        ("GOVERNOR", "RECRUIT", "ABSORBER"),
        ("GOVERNOR", "SUBMIT", "ABSORBER"),
        ("GOVERNOR", "EXILE", "ARCHIVE"),
        ("GOVERNOR", "RETIRE", "ARCHIVE"),
        ("GOVERNOR", "DEFECT", "ARCHIVE"),
        ("GOVERNOR", "HIDE", "ARCHIVE"),
        ("HERO", "RECRUIT", "ABSORBER"),
        ("HERO", "PROTECT", "ABSORBER"),
        ("HERO", "RECONTRACT", "ABSORBER"),
        ("HERO", "DEFECT", "ARCHIVE"),
        ("HERO", "HIDE", "ARCHIVE"),
        ("HERO", "SUICIDE", "ARCHIVE"),
        ("GREAT_PERSON", "RECRUIT", "ABSORBER"),
        ("GREAT_PERSON", "EXILE", "ARCHIVE"),
        ("GREAT_PERSON", "RETIRE", "ARCHIVE"),
        ("GREAT_PERSON", "HIDE", "ARCHIVE"),
        ("ITEM", "TRANSFER", "ABSORBER"),
        ("ITEM", "LOOT", "ABSORBER"),
        ("ITEM", "SEIZE", "ABSORBER"),
        ("ITEM", "RETURN", "RETURN"),
        ("ITEM", "SCATTER", "ARCHIVE"),
        ("UNLOCK", "INHERIT", "ABSORBER"),
        ("UNLOCK", "PRESERVE", "ARCHIVE"),
        ("RELIGION", "PRESERVE", "ABSORBER"),
    ];
    let rows = lua.create_table()?;
    for (index, (kind, outcome, destination)) in ROWS.iter().enumerate() {
        let row = lua.create_table()?;
        row.set("Kind", *kind)?;
        row.set("Outcome", *outcome)?;
        row.set("Destination", *destination)?;
        rows.set(index + 1, row)?;
    }
    Ok(rows)
}

fn load_module(lua: &Lua, path: &str) -> mlua::Result<()> {
    let override_path = match path {
        "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Rules.lua" => {
            std::env::var_os("JAC_FLOW_RULES_SOURCE").map(PathBuf::from)
        }
        "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Gameplay.lua" => {
            std::env::var_os("JAC_FLOW_GAMEPLAY_SOURCE").map(PathBuf::from)
        }
        FLOW => std::env::var_os("JAC_FLOW_ENGINE_SOURCE").map(PathBuf::from),
        "poc/JAC_PoC_Absorption/UI/JAC_AbsorptionFlow.lua" => {
            std::env::var_os("JAC_FLOW_UI_SOURCE").map(PathBuf::from)
        }
        _ => None,
    };
    let source = fs::read_to_string(override_path.unwrap_or_else(|| repo_path(path)))
        .map_err(mlua::Error::external)?;
    lua.load(&source).set_name(path).exec()
}

fn world() -> mlua::Result<Lua> {
    let lua = Lua::new();
    let rows = outcomes(&lua)?;
    lua.load(WORLD)
        .set_name("absorption_world.lua")
        .call::<()>(rows)?;
    lua.load("function include(_) end").exec()?;
    load_module(
        &lua,
        "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Rules.lua",
    )?;
    load_module(
        &lua,
        "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Gameplay.lua",
    )?;
    Ok(lua)
}

fn flow_world() -> mlua::Result<Lua> {
    let lua = world()?;
    lua.load(
        r#"
        Game:SetProperty('JAC_ABSORPTION_FLOW_TEST_MODE',1)
        -- The flow rejects native Great Works until their Civ6 slot can be
        -- scattered losslessly. Roster artifacts below remain real test assets.
        for _,city in Players[1]:GetCities():Members() do
          local buildings=city.GetBuildings
          city.GetBuildings=function(self)
            local result=buildings(self)
            result.GetGreatWorkInSlot=function() return -1 end
            return result
          end
        end
        "#,
    )
    .exec()?;
    load_module(&lua, FLOW)?;
    Ok(lua)
}

#[test]
fn fixture_runs_the_actual_lua_transfer_with_no_spawn_return_value() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
        local A = assert(ExposedMembers.JAC_Absorption)
        local tx = A.Apply(PLAN)
        assert(tx.Status == 'COMPLETE', tx.Status)
        assert(SPAWNS == 6 and KILLS == 5 and TRANSFERS == 2)
        assert(Players[1].units[3] == nil and Players[2].units[3] == nil)
        local moved = assert(A.FindUnit(2, 'test-absorption:unit:3'))
        assert(moved.damage == 23 and moved.xp == 31)
        assert(CITIES[4]:GetOwner() == 2 and CITIES[8]:GetOwner() == 2)
        assert(A.GetRoster(2).artifact.Owner == 2)
        "#,
    )
    .exec()
}

#[test]
fn actual_flow_journals_sorted_phase_one_rolls_and_reloads_without_reroll() -> mlua::Result<()> {
    let lua = flow_world()?;
    lua.load(
        r#"
        local F=assert(ExposedMembers.JAC_AbsorptionFlow)
        local A=assert(ExposedMembers.JAC_Absorption)
        local roster=A.GetRoster(1)
        roster.governor={ID='governor',Kind='PERSON',Owner=1,Name='Governor test',
          Role='GOVERNOR',Abilities={'ADMINISTRATION'},Loyalty=85}
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,roster)
        for i=1,100 do RANDOM_QUEUE[#RANDOM_QUEUE+1]=9999 end
        F.DebugSetRoll('phase-one','person','P1_ESCAPE',0)
        F.DebugSetRoll('phase-one','person','P1_BRANCH',0)
        TEST_FLOW_PLAN={ID='phase-one',From=1,To=2,Mode='MERGER'}
        local choice=F.Begin(TEST_FLOW_PLAN)
        assert(choice.Phase=='CHOICE' and choice.Phase1Complete)
        assert(F.GetPending(2).ID=='phase-one' and F.GetPending(1)==nil)
        local expected={UNIT=true,PERSON=true,HERO=true,GREAT_PERSON=true,ITEM=true,
          RELIGION=true,UNLOCK=true}
        local kinds={}
        for i,entry in ipairs(choice.Entries) do
          assert(i==1 or choice.Entries[i-1].ID<entry.ID,'entries not stable-ID sorted')
          assert(expected[entry.Kind] and type(entry.Name)=='string')
          assert(type(entry.AllowedActions)=='table' and #entry.AllowedActions>0)
          kinds[entry.Kind]=true
          if entry.ID=='person' then
            assert(entry.EscapeOutcome=='EXILE' and entry.FinalOutcome=='EXILE')
            assert(entry.Rolls.P1_ESCAPE.Used==0 and entry.Rolls.P1_BRANCH.Used==0)
          end
        end
        for kind in pairs(expected) do assert(kinds[kind],kind..' fixture absent') end
        local expectedLabels={}
        for _,entry in ipairs(choice.Entries) do
          for _,stage in ipairs({'P1_ESCAPE','P1_BRANCH'}) do
            local roll=entry.Rolls and entry.Rolls[stage]
            if roll then expectedLabels[#expectedLabels+1]=roll.Label end
          end
        end
        assert(#RANDOM_CALLS==#expectedLabels,'first stage skipped or added a native roll')
        for index,label in ipairs(expectedLabels) do
          assert(RANDOM_CALLS[index].Maximum==10000 and RANDOM_CALLS[index].Label==label,
            'Game.GetRandNum order/label differs from sorted assets')
        end
        local detached=F.Get('phase-one')
        detached.Entries[1].Name='corrupted client snapshot'
        assert(F.Get('phase-one').Entries[1].Name~='corrupted client snapshot')
        TEST_FLOW_BEFORE=A.Canonical(F.Get('phase-one'))
        TEST_ROLL_COUNT=#RANDOM_CALLS
        -- A Civ6 save serializes property tables. Keep their values, then reload the Lua modules.
        Game.props=A.Rules.Copy(Game.props)
        for owner=0,3 do Players[owner].props=A.Rules.Copy(Players[owner].props) end
        "#,
    )
    .exec()?;
    load_module(
        &lua,
        "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Rules.lua",
    )?;
    load_module(
        &lua,
        "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Gameplay.lua",
    )?;
    load_module(&lua, FLOW)?;
    lua.load(
        r#"
        local F=ExposedMembers.JAC_AbsorptionFlow
        local A=ExposedMembers.JAC_Absorption
        assert(A.Canonical(F.Get('phase-one'))==TEST_FLOW_BEFORE,
          'saved choice phase changed on reload')
        local repeated=F.Begin(TEST_FLOW_PLAN)
        assert(repeated.Phase=='CHOICE' and #RANDOM_CALLS==TEST_ROLL_COUNT,
          'repeated submission rerolled the first stage')
        local snapshot=A.Canonical(F.Get('phase-one'))
        local ok=pcall(F.Select,1,'phase-one','governor','RECRUIT')
        assert(not ok and A.Canonical(F.Get('phase-one'))==snapshot,
          'wrong actor mutated saved choice')
        ok=pcall(F.Select,2,'phase-one','person','RECRUIT')
        assert(not ok and A.Canonical(F.Get('phase-one'))==snapshot,
          'escaped identity became selectable')
        ok=pcall(F.Select,2,'phase-one','governor','TRANSFER')
        assert(not ok and A.Canonical(F.Get('phase-one'))==snapshot,
          'invalid kind/action mutated saved choice')
        "#,
    )
    .exec()
}

#[test]
fn actual_flow_forced_failure_rerolls_once_and_applies_distinct_world_actions() -> mlua::Result<()>
{
    let lua = flow_world()?;
    lua.load(
        r#"
        local F=assert(ExposedMembers.JAC_AbsorptionFlow)
        local A=assert(ExposedMembers.JAC_Absorption)
        local roster=A.GetRoster(1)
        roster.governor={ID='governor',Kind='PERSON',Owner=1,Name='Governor test',
          Role='GOVERNOR',Abilities={'ADMINISTRATION'},Loyalty=85}
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,roster)
        for i=1,400 do RANDOM_QUEUE[#RANDOM_QUEUE+1]=9999 end
        F.DebugSetRoll('forced-flow','person','P1_ESCAPE',0)
        F.DebugSetRoll('forced-flow','person','P1_BRANCH',0)
        F.DebugSetRoll('forced-flow','governor','P3_SUCCESS',9999)
        F.DebugSetRoll('forced-flow','governor','P3_RETRY_ESCAPE',0)
        F.DebugSetRoll('forced-flow','governor','P3_RETRY_BRANCH',0)
        F.DebugSetRoll('forced-flow','equipment','P3_SUCCESS',9999)
        F.DebugSetRoll('forced-flow','equipment','P3_RETRY_ESCAPE',9999)
        local plan={ID='forced-flow',From=1,To=2,Mode='MERGER'}
        local flow=F.Begin(plan)
        assert(flow.Phase=='CHOICE')
        local before=A.Canonical(F.Get('forced-flow'))
        local ok=pcall(F.Resolve,1,'forced-flow')
        assert(not ok and A.Canonical(F.Get('forced-flow'))==before,
          'wrong actor resolved or mutated the flow')
        for _,entry in ipairs(flow.Entries) do
          if not entry.EscapeOutcome then
            local action=entry.Kind=='UNIT' and 'INCORPORATE'
              or entry.Kind=='PERSON' and 'RECRUIT'
              or entry.Kind=='HERO' and 'PROTECT'
              or entry.Kind=='GREAT_PERSON' and 'RECRUIT'
              or entry.Kind=='ITEM' and (entry.ID=='artifact' and 'RETURN' or 'TRANSFER')
              or entry.Kind=='UNLOCK' and 'INHERIT' or 'PRESERVE'
            F.Select(2,'forced-flow',entry.ID,action,entry.ID=='artifact' and 3 or nil)
            if entry.ID~='governor' and entry.ID~='equipment' then
              F.DebugSetRoll('forced-flow',entry.ID,'P3_SUCCESS',0)
            end
          end
        end
        local selected=F.Get('forced-flow')
        assert(selected.Phase=='CHOICE' and Players[1].units[3]~=nil,
          'selection prematurely changed native world')
        local result=F.Resolve(2,'forced-flow')
        assert(result.Phase=='RESULT',result.Phase..':'..tostring(result.Error))
        local byID={};for _,entry in ipairs(result.Entries) do byID[entry.ID]=entry end
        assert(byID.person.EscapeOutcome=='EXILE' and byID.person.FinalOutcome=='EXILE')
        assert(byID.governor.Success==false and byID.governor.RetryEscapeOutcome=='EXILE')
        assert(byID.governor.FinalOutcome=='EXILE' and byID.governor.Rolls.P3_RETRY_ESCAPE.Used==0)
        assert(byID.equipment.Success==false and byID.equipment.RetryEscapeOutcome==nil)
        assert(byID.equipment.FinalOutcome=='SEIZE' and byID.equipment.Rolls.P3_RETRY_ESCAPE.Used==9999)
        assert(byID.artifact.Success==true and byID.artifact.FinalOutcome=='RETURN')
        assert(Players[1].units[3]==nil and CITIES[4]:GetOwner()==2 and CITIES[8]:GetOwner()==2)
        assert(A.GetRoster(2).equipment.Outcome=='SEIZE' and A.GetRoster(3).artifact.Owner==3)
        assert(A.GetRoster(2)['leader:1']~=nil and A.GetRoster(2).religion~=nil)
        assert(F.GetWorld('forced-flow:person').Status=='WAIT_SITE')
        assert(F.GetWorld('forced-flow:governor').Status=='WAIT_SITE')
        local expectedLabels={}
        for _,entry in ipairs(result.Entries) do
          for _,stage in ipairs({'P1_ESCAPE','P1_BRANCH'}) do
            local roll=entry.Rolls and entry.Rolls[stage]
            if roll then expectedLabels[#expectedLabels+1]=roll.Label end
          end
        end
        for _,entry in ipairs(result.Entries) do
          for _,stage in ipairs({'P3_SUCCESS','P3_RETRY_ESCAPE','P3_RETRY_BRANCH'}) do
            local roll=entry.Rolls and entry.Rolls[stage]
            if roll then expectedLabels[#expectedLabels+1]=roll.Label end
          end
        end
        assert(#RANDOM_CALLS==#expectedLabels,'forced debug path skipped native random consumption')
        for index,label in ipairs(expectedLabels) do
          assert(RANDOM_CALLS[index].Maximum==10000 and RANDOM_CALLS[index].Label==label,
            'second or third stage broke stable-ID random order')
        end
        TEST_FORCED_RESULT=A.Canonical(result)
        TEST_FORCED_ROLLS=#RANDOM_CALLS
        assert(A.Canonical(F.Resolve(2,'forced-flow'))==TEST_FORCED_RESULT)
        assert(#RANDOM_CALLS==TEST_FORCED_ROLLS,'result submission rerolled')
        Game.props=A.Rules.Copy(Game.props)
        for owner=0,3 do Players[owner].props=A.Rules.Copy(Players[owner].props) end
        "#,
    )
    .exec()?;
    load_module(
        &lua,
        "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Rules.lua",
    )?;
    load_module(
        &lua,
        "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Gameplay.lua",
    )?;
    load_module(&lua, FLOW)?;
    lua.load(
        r#"
        local F=ExposedMembers.JAC_AbsorptionFlow
        local A=ExposedMembers.JAC_Absorption
        assert(A.Canonical(F.Get('forced-flow'))==TEST_FORCED_RESULT,
          'saved result changed after Lua reload')
        assert(#RANDOM_CALLS==TEST_FORCED_ROLLS)
        local done=F.Acknowledge(2,'forced-flow')
        assert(done.Phase=='DONE' and F.GetPending(2)==nil)
        assert(F.Acknowledge(2,'forced-flow').Phase=='DONE')
        assert(#RANDOM_CALLS==TEST_FORCED_ROLLS)
        CURRENT_TURN=CURRENT_TURN+2
        F.Tick()
        for _,id in ipairs({'person','governor'}) do
          local world=assert(F.GetWorld('forced-flow:'..id))
          assert(world.Status=='WAIT_CLEAR' and type(world.Site)=='number')
          assert(PLOTS[world.Site]:GetImprovementType()==world.Improvement,
            'escaped identity did not enter an actual reward site')
          local claimant=UNIT(2,NEXT_ID,'UNIT_WARRIOR',world.Site);NEXT_ID=NEXT_ID+1
          PLOTS[world.Site].improvement=-1
          F.Tick()
          world=F.GetWorld('forced-flow:'..id)
          assert(world.Status=='COMPLETE' and world.Recipient==2,
            'reward clearance did not recover original identity: '..tostring(world.Error))
          assert(claimant:GetOwner()==2)
        end
        local roster=A.GetRoster(2)
        assert(roster.person.ID=='person' and roster.person.Level==6)
        assert(roster.person.Personality=='LOYAL' and roster.person.Equipment[1]=='ARMOR')
        assert(roster.governor.ID=='governor' and roster.governor.Role=='GOVERNOR')
        assert(roster.governor.Abilities[1]=='ADMINISTRATION')
        local recovered=assert(A.FindUnit(2,'recovery:forced-flow:person'))
        assert(recovered:GetProperty(A.Rules.PERSON_KEY).ID=='person')
        local unitCount=Players[2]:GetUnits():GetCount()
        F.Tick()
        assert(Players[2]:GetUnits():GetCount()==unitCount and #RANDOM_CALLS==TEST_FORCED_ROLLS,
          'repeated reward tick duplicated a recovery or rerolled')
        "#,
    )
    .exec()
}

#[test]
fn actual_city_methods_change_loyalty_once_and_resume_remaining_friction() -> mlua::Result<()> {
    for (mode, immediate, friction, turns) in [
        ("CONQUEST", -15, -2, 3),
        ("SMALL_STATE", -5, -1, 2),
        ("MERGER", 0, 0, 0),
    ] {
        let lua = world()?;
        lua.globals().set("TEST_MODE", mode)?;
        lua.globals().set("EXPECTED_IMMEDIATE", immediate)?;
        lua.globals().set("EXPECTED_FRICTION", friction)?;
        lua.globals().set("EXPECTED_TURNS", turns)?;
        lua.load(
            r#"
            local A=ExposedMembers.JAC_Absorption
            PLAN.Mode=TEST_MODE
            PLAN.FlowPolicy=true
            LOYALTY_CALLS={}
            for _,city in Players[1]:GetCities():Members() do
              local change=city.ChangeLoyalty
              city.ChangeLoyalty=function(self,amount)
                LOYALTY_CALLS[#LOYALTY_CALLS+1]={City=self.x,Amount=amount,Turn=CURRENT_TURN}
                return change(self,amount)
              end
            end
            local result=A.Apply(PLAN)
            assert(result.Status=='COMPLETE',result.Status)
            for _,x in ipairs({4,8}) do
              local city=CITIES[x]
              local governance=assert(city:GetProperty(A.Rules.GOVERNANCE_KEY))
              assert(city.loyalty==87+EXPECTED_IMMEDIATE,
                TEST_MODE..' loyalty '..x..'='..tostring(city.loyalty))
              assert(governance.Mode==TEST_MODE and governance.ImmediateApplied==true,
                TEST_MODE..' governance immediate status')
              assert(governance.LoyaltyDelta==EXPECTED_IMMEDIATE,
                TEST_MODE..' immediate delta')
              assert(governance.FrictionPerTurn==EXPECTED_FRICTION,
                TEST_MODE..' friction amount')
              assert(governance.FrictionTurns==EXPECTED_TURNS,
                TEST_MODE..' friction turns')
            end
            assert(#LOYALTY_CALLS==(EXPECTED_IMMEDIATE~=0 and 2 or 0),
              TEST_MODE..' native immediate call count '..#LOYALTY_CALLS)
            CURRENT_TURN=6
            A.ApplyCityFriction()
            A.ApplyCityFriction()
            for _,x in ipairs({4,8}) do
              assert(CITIES[x].loyalty==87+EXPECTED_IMMEDIATE
                +(EXPECTED_TURNS>0 and EXPECTED_FRICTION or 0),
                'same-turn friction was applied twice')
            end
            local calls=#LOYALTY_CALLS
            for _,x in ipairs({4,8}) do
              CITIES[x].props=A.Rules.Copy(CITIES[x].props)
            end
            Game.props=A.Rules.Copy(Game.props)
            TEST_LOYALTY_CALLS=calls
            "#,
        )
        .exec()?;
        load_module(
            &lua,
            "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Rules.lua",
        )?;
        load_module(
            &lua,
            "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Gameplay.lua",
        )?;
        lua.load(
            r#"
            local A=ExposedMembers.JAC_Absorption
            A.ApplyCityFriction()
            assert(#LOYALTY_CALLS==TEST_LOYALTY_CALLS,
              'reload repeated already applied friction')
            for turn=7,11 do
              CURRENT_TURN=turn
              A.ApplyCityFriction()
            end
            for _,x in ipairs({4,8}) do
              assert(CITIES[x].loyalty==87+EXPECTED_IMMEDIATE
                +EXPECTED_FRICTION*EXPECTED_TURNS,
                'city method final loyalty differs from observed native calls')
              local g=CITIES[x]:GetProperty(A.Rules.GOVERNANCE_KEY)
              assert(g.FrictionApplied==EXPECTED_TURNS)
            end
            local expectedCalls=(EXPECTED_IMMEDIATE~=0 and 2 or 0)+2*EXPECTED_TURNS
            assert(#LOYALTY_CALLS==expectedCalls)
            "#,
        )
        .exec()?;
    }
    Ok(())
}

#[test]
fn official_mode_and_custom_heroes_are_required_before_any_roll_or_mutation() -> mlua::Result<()> {
    let lua = flow_world()?;
    lua.load(
        r#"
        local F=ExposedMembers.JAC_AbsorptionFlow
        local plan={ID='hero-mode',From=1,To=2,Mode='MERGER'}
        local configured=GameConfiguration.GetValue
        GameConfiguration.GetValue=function() return nil end
        assert(not pcall(F.Begin,plan),'flow accepted a game without Heroes & Legends')
        assert(Game:GetProperty('JAC_ABSORPTION_FLOW_V1')==nil and #RANDOM_CALLS==0)
        GameConfiguration.GetValue=configured
        local heroes=GameInfo.HeroClasses
        setmetatable(heroes,{__call=function(t)
          local i=0;local all={t[0],t[1],{Index=8,HeroClassType='HEROCLASS_HERCULES',UnitType='UNIT_HERO_HERCULES'}}
          return function() i=i+1;return all[i] end
        end})
        assert(not pcall(F.Begin,plan),'stock official Hero remained available')
        assert(Game:GetProperty('JAC_ABSORPTION_FLOW_V1')==nil and #RANDOM_CALLS==0)
        GameConfiguration.IsAnyMultiplayer=function() return true end
        assert(not pcall(F.DebugSetRoll,'hero-mode','hero','P1_ESCAPE',0),
          'multiplayer debug override was accepted')
        assert(#RANDOM_CALLS==0)
        "#,
    )
    .exec()
}

#[test]
fn human_house_cannot_be_the_absorbed_source() -> mlua::Result<()> {
    let lua = flow_world()?;
    lua.load(
        r#"
        Players[1].IsHuman=function() return true end
        Players[2].IsHuman=function() return false end
        local F=ExposedMembers.JAC_AbsorptionFlow
        local before=JAC_AbsorptionRules.Canonical(Game.props)
        local ok=pcall(F.Begin,{ID='human-donor',From=1,To=2,Mode='MERGER'})
        assert(not ok and JAC_AbsorptionRules.Canonical(Game.props)==before)
        assert(#RANDOM_CALLS==0 and SPAWNS==0 and KILLS==0 and TRANSFERS==0)
        "#,
    )
    .exec()
}

#[test]
fn execute_script_rejects_wrong_actor_and_invalid_asset_without_changing_flow_or_world()
-> mlua::Result<()> {
    let lua = flow_world()?;
    lua.load(
        r#"
        local F=ExposedMembers.JAC_AbsorptionFlow
        for i=1,100 do RANDOM_QUEUE[#RANDOM_QUEUE+1]=9999 end
        local choice=F.Begin({ID='operation-boundary',From=1,To=2,Mode='MERGER'})
        local before=JAC_AbsorptionRules.Canonical(choice)
        local rolls=#RANDOM_CALLS
        GameEvents.JAC_AbsorptionFlow_Operation.Emit(1,{Command='SELECT',FlowID=choice.ID,
          AssetID='equipment',Action='TRANSFER'})
        local rejection=assert(Players[1]:GetProperty('JAC_ABSORPTION_FLOW_LAST_OPERATION_V1'))
        assert(rejection.Accepted==false and rejection.Command=='SELECT')
        assert(JAC_AbsorptionRules.Canonical(F.Get(choice.ID))==before)
        GameEvents.JAC_AbsorptionFlow_Operation.Emit(2,{Command='SELECT',FlowID=choice.ID,
          AssetID='missing:item',Action='TRANSFER'})
        rejection=assert(Players[2]:GetProperty('JAC_ABSORPTION_FLOW_LAST_OPERATION_V1'))
        assert(rejection.Accepted==false and rejection.Command=='SELECT')
        assert(JAC_AbsorptionRules.Canonical(F.Get(choice.ID))==before)
        GameEvents.JAC_AbsorptionFlow_Operation.Emit(2,{Command='UNKNOWN',FlowID=choice.ID})
        assert(Players[2]:GetProperty('JAC_ABSORPTION_FLOW_LAST_OPERATION_V1').Accepted==false)
        assert(JAC_AbsorptionRules.Canonical(F.Get(choice.ID))==before)
        assert(#RANDOM_CALLS==rolls and SPAWNS==0 and KILLS==0 and TRANSFERS==0)
        "#,
    )
    .exec()
}

#[test]
fn native_sengoku_hero_without_verified_claim_becomes_recorded_suicide() -> mlua::Result<()> {
    let lua = flow_world()?;
    lua.load(
        r#"
        local A=ExposedMembers.JAC_Absorption
        local F=ExposedMembers.JAC_AbsorptionFlow
        local hero=UNIT(1,13,'UNIT_HERO_JAC_WARLORD_A',4)
        hero.lifespan=17;hero.charges=2;hero.renown=13
        HERO_CLAIMS[0]=1;HERO_ORIGINS[0]={player=1,id=CITIES[4].id}
        HERO_DISCOVERED['1:0']=true
        local person={ID='native',Kind='HERO',Owner=1,Unit={Owner=1,ID=13},
          Contract='RECONTRACT',Loyalty=71,Lifespan=17,Relations={friend=29},
          Equipment={'SPEAR'}}
        hero:SetProperty(A.Rules.PERSON_KEY,person)
        local roster=A.GetRoster(1);roster.native=person
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,roster)
        for i=1,300 do RANDOM_QUEUE[#RANDOM_QUEUE+1]=9999 end
        local plan={ID='native-suicide',From=1,To=2,Mode='MERGER'}
        local flow=F.Begin(plan)
        for _,entry in ipairs(flow.Entries) do
          assert(entry.Kind~='HERO' or entry.ID~='native' or entry.Portrait=='ICON_UNIT_HERO_JAC_WARLORD_A')
          if not entry.EscapeOutcome then
            local action=entry.Kind=='UNIT' and 'INCORPORATE'
              or entry.Kind=='PERSON' and 'RECRUIT'
              or entry.Kind=='HERO' and 'RECONTRACT'
              or entry.Kind=='GREAT_PERSON' and 'RECRUIT'
              or entry.Kind=='ITEM' and 'TRANSFER'
              or entry.Kind=='UNLOCK' and 'INHERIT' or 'PRESERVE'
            F.Select(2,plan.ID,entry.ID,action)
            F.DebugSetRoll(plan.ID,entry.ID,'P3_SUCCESS',0)
          end
        end
        HERO_CLAIMS[0]=3 -- third party took the claim after the first saved stage
        local result=F.Resolve(2,plan.ID)
        assert(result.Phase=='RESULT',result.Phase..':'..tostring(result.Error))
        local entry
        for _,candidate in ipairs(result.Entries) do if candidate.ID=='native' then entry=candidate end end
        assert(entry and entry.SelectedAction=='RECONTRACT')
        assert(entry.FinalOutcome=='SUICIDE' and entry.Success==false)
        assert(entry.TransferCause=='CLAIM_UNAVAILABLE')
        local archived=assert(A.GetState().Archive.native)
        assert(archived.Outcome=='SUICIDE' and archived.SuicideCause=='CLAIM_UNAVAILABLE')
        assert(archived.Contract=='RECONTRACT' and archived.Loyalty==71)
        assert(archived.Relations.friend==29 and archived.Equipment[1]=='SPEAR')
        assert(A.FindUnit(2,'native-suicide:unit:13')==nil)
        assert(A.GetRoster(2).native==nil)
        "#,
    )
    .exec()
}

#[test]
fn native_sengoku_hero_recontracts_with_lifespan_charges_and_origin() -> mlua::Result<()> {
    let lua = flow_world()?;
    lua.load(
        r#"
        local A=ExposedMembers.JAC_Absorption
        local F=ExposedMembers.JAC_AbsorptionFlow
        local hero=UNIT(1,13,'UNIT_HERO_JAC_WARLORD_A',4)
        hero.lifespan=17;hero.charges=2;hero.renown=13;hero.damage=7;hero.moves=1
        HERO_CLAIMS[0]=1;HERO_ORIGINS[0]={player=1,id=CITIES[4].id}
        HERO_DISCOVERED['1:0']=true
        local person={ID='native',Kind='HERO',Owner=1,Unit={Owner=1,ID=13},
          Contract='RECONTRACT',Loyalty=71,Lifespan=17,Relations={friend=29},
          Equipment={'SPEAR'}}
        hero:SetProperty(A.Rules.PERSON_KEY,person)
        local roster=A.GetRoster(1);roster.native=person
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,roster)
        for i=1,300 do RANDOM_QUEUE[#RANDOM_QUEUE+1]=9999 end
        local plan={ID='native-transfer',From=1,To=2,Mode='MERGER'}
        local flow=F.Begin(plan)
        for _,entry in ipairs(flow.Entries) do
          if not entry.EscapeOutcome then
            local action=entry.Kind=='UNIT' and 'INCORPORATE'
              or entry.Kind=='PERSON' and 'RECRUIT'
              or entry.Kind=='HERO' and (entry.ID=='native' and 'RECONTRACT' or 'PROTECT')
              or entry.Kind=='GREAT_PERSON' and 'RECRUIT'
              or entry.Kind=='ITEM' and 'TRANSFER'
              or entry.Kind=='UNLOCK' and 'INHERIT' or 'PRESERVE'
            F.Select(2,plan.ID,entry.ID,action)
            F.DebugSetRoll(plan.ID,entry.ID,'P3_SUCCESS',0)
          end
        end
        local result=F.Resolve(2,plan.ID)
        assert(result.Phase=='RESULT',result.Phase..':'..tostring(result.Error))
        local nativeEntry
        for _,entry in ipairs(result.Entries) do if entry.ID=='native' then nativeEntry=entry end end
        assert(nativeEntry and nativeEntry.Success==true and nativeEntry.FinalOutcome=='RECONTRACT')
        assert(A.GetState().Transactions[plan.ID].HeroResults.native.Outcome=='RECONTRACT')
        local moved=assert(A.FindUnit(2,'native-transfer:unit:13'))
        assert(moved:IsHero() and moved:GetHeroClassType()==0)
        assert(moved:GetProperty(UnitPropertyKeys.LifespanRemaining)==17)
        assert(moved:GetActionCharges()==2 and moved:GetRenown()==13)
        assert(moved:GetDamage()==7 and moved:GetMovesRemaining()==1)
        assert(HERO_CLAIMS[0]==2 and HERO_ORIGINS[0].player==2)
        assert(HERO_ORIGINS[0].id==CITIES[4]:GetID())
        local record=assert(A.GetRoster(2).native)
        assert(record.Unit.ID==moved:GetID() and record.Contract=='RECONTRACT')
        assert(record.Loyalty==71 and record.Relations.friend==29)
        assert(record.NativeHero.ClassType=='HEROCLASS_JAC_WARLORD_A')
        assert(A.GetState().Archive.native==nil and A.Verify(plan.ID,true))
        "#,
    )
    .exec()
}

#[test]
fn actual_panel_renders_identity_and_sends_only_a_human_choice() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(PANEL_WORLD)
        .set_name("absorption_panel.lua")
        .exec()?;
    lua.load(
        r#"
        TEST_UI_PENDING={ID='ui-flow',From=1,To=2,Mode='MERGER',Phase='CHOICE',Entries={
          {ID='person:aya',Kind='PERSON',Name='Aya',Identity='Governor',Abilities={'Builder','Diplomat'},
            Portrait='ICON_TEST_AYA',AllowedActions={'RECRUIT','EXILE'}},
          {ID='artifact:seal',Kind='ITEM',Name='Seal',Identity='Artifact',Abilities={'History'},
            AllowedActions={'TRANSFER','RETURN'}}}}
        ExposedMembers.JAC_AbsorptionFlow={GetPending=function(owner)
          assert(owner==2);return JAC_AbsorptionRules.Copy(TEST_UI_PENDING)
        end}
        "#,
    )
    .exec()?;
    load_module(&lua, "poc/JAC_PoC_Absorption/UI/JAC_AbsorptionFlow.lua")?;
    lua.load(
        r#"
        local panel=assert(JAC_AbsorptionFlow_UI)
        local view=panel.GetView()
        assert(view.visible and view.phase=='CHOICE' and #view.rows==2)
        assert(view.rows[1].name=='Aya' and view.rows[1].identity:find('Governor',1,true))
        assert(view.rows[1].abilities:find('Builder',1,true))
        assert(view.rows[1].portrait=='ICON_TEST_AYA')
        assert(#RANDOM_CALLS==0,'UI drew from gameplay random stream')
        Players[2].IsTurnActive=function() return false end
        assert(not panel.ChooseAction('RECRUIT') and #OPERATION_CALLS==0,
          'human command allowed outside human turn')
        Players[2].IsTurnActive=function() return true end
        TEST_ENTRY_MANAGER.instances[1].Choose:DoLeftClick()
        assert(panel.GetView().rows[1].id=='person:aya')
        assert(TEST_ACTION_MANAGER.instances[1].Choose:DoLeftClick())
        local sent=assert(OPERATION_CALLS[1])
        assert(sent.Actor==2 and sent.Params.OnStart=='JAC_AbsorptionFlow_Operation')
        assert(sent.Params.Command=='SELECT' and sent.Params.FlowID=='ui-flow')
        assert(sent.Params.AssetID=='person:aya' and sent.Params.Action=='RECRUIT')
        TEST_UI_PENDING.Entries[1].SelectedAction='RECRUIT'
        Events.GameCoreEventPublishComplete.Emit()
        TEST_ENTRY_MANAGER.instances[2].Choose:DoLeftClick()
        local clicked=TEST_ACTION_MANAGER.instances[2].Choose:DoLeftClick() -- RETURN
        assert(clicked==true,'RETURN action control did not activate: '..tostring(clicked))
        TEST_ACTION_MANAGER.instances[2].Choose:DoLeftClick() -- living owner 3
        sent=assert(OPERATION_CALLS[2])
        assert(sent.Actor==2 and sent.Params.Command=='SELECT')
        assert(sent.Params.AssetID=='artifact:seal' and sent.Params.Action=='RETURN')
        assert(sent.Params.ReturnOwner==3)
        TEST_UI_PENDING.Entries[2].SelectedAction='RETURN'
        Events.GameCoreEventPublishComplete.Emit()
        assert(panel.GetView().canResolve and not Controls.ConfirmButton:IsDisabled())
        Controls.ConfirmButton:DoLeftClick()
        assert(OPERATION_CALLS[3].Params.Command=='RESOLVE')
        TEST_UI_PENDING.Phase='RESULT'
        Events.GameCoreEventPublishComplete.Emit()
        assert(panel.GetView().phase=='RESULT')
        Controls.ConfirmButton:DoLeftClick()
        assert(OPERATION_CALLS[4].Params.Command=='ACK')
        assert(#RANDOM_CALLS==0,'choice UI changed gameplay random stream')
        "#,
    )
    .exec()
}
