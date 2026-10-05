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
    let result: mlua::Value = lua.load(&source).set_name(path).eval()?;
    match path {
        FLOW => lua.globals().set("TEST_FLOW", result)?,
        "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Gameplay.lua" => {
            lua.globals().set("TEST_ABSORPTION", result)?;
        }
        _ => {}
    }
    Ok(())
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
        local A = assert(TEST_ABSORPTION)
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

// The full absorption engine runs here. RP's Export boundary returns a detached
// saved ledger; the RP suite separately exercises its real normalization/revival.
fn joint_adapter_world() -> mlua::Result<Lua> {
    let lua = world()?;
    lua.load(r#"
      local A=TEST_ABSORPTION
      local native=UNIT(1,13,'UNIT_HERO_JAC_WARLORD_A',4)
      native.lifespan=17;native.charges=2
      HERO_CLAIMS[0]=1;HERO_ORIGINS[0]={player=1,id=CITIES[4].id}
      HERO_DISCOVERED['1:0']=true
      local record={ID='joint-native',Kind='HERO',Owner=1,Unit={Owner=1,ID=13},Contract='PROTECT'}
      native:SetProperty(A.Rules.PERSON_KEY,record)
      local roster=A.GetRoster(1);roster['joint-native']=record
      Players[1]:SetProperty(A.Rules.ROSTER_KEY,roster)
      PLAN.Units['1:13']='PROTECT';PLAN.Assets['joint-native']='PROTECT'
      Game:SetProperty('JAC_RP_STATE_V1',{schema=1,revision=7,
        heroes={['1']={religion='sect',unit=13,heroClass=0,heroUnit='UNIT_HERO_JAC_WARLORD_A',verification='APPLIED',rewardFaith=30}},
        artifacts={},religiousAlliances={},religions={
          sect={id='sect',founder=1,founderDefeated=false,engineReligion=5,followers=9,
            communities={origin={holder='PLAYER_1',followers=9,publicFacilities=1}},
            assets={book={kind='SCRIPTURE',owner=1,holder='CAVE'},third={kind='RECORD',owner=3,holder='PLAYER_3'}},
            discoveries={},revivals={},materialUses={},outcomeUses={},retiredAssets={}}
        }})
      JAC_ReligionPersistenceGameplay={Export=function() return A.Rules.Copy(Game:GetProperty('JAC_RP_STATE_V1')) end}
    "#).exec()?;
    load_module(
        &lua,
        "poc/JAC_PoC_ReligionPersistence/Scripts/JAC_RP_AbsorptionAdapter.lua",
    )?;
    lua.load("TEST_ABSORPTION.RegisterAdapter('religion-persistence',JAC_RP_AbsorptionAdapter)")
        .exec()?;
    Ok(lua)
}

#[test]
fn religion_adapter_runs_in_actual_absorption_and_reregisters_after_reload() -> mlua::Result<()> {
    let lua = joint_adapter_world()?;
    lua.load(r#"
      local A=TEST_ABSORPTION
      local tx=A.Apply(PLAN)
      assert(tx.Status=='COMPLETE' and tx.Adapters['religion-persistence'])
      local state=Game:GetProperty('JAC_RP_STATE_V1')
      local r=state.religions.sect
      assert(r.founder==1 and r.founderDefeated and r.engineReligion==5 and r.followers==9)
      assert(r.communities.origin.holder=='PLAYER_2' and r.assets.book.owner==2 and r.assets.book.holder=='CAVE')
      assert(r.assets.third.owner==3 and r.assets.third.holder=='PLAYER_3')
      assert(state.heroes['1']==nil and state.heroes['2'].unit==tx.UnitMap['1:13'].ID)
      local unit=A.FindUnit(2,'test-absorption:unit:13')
      assert(unit:IsHero() and unit:GetID()==state.heroes['2'].unit)
      JOINT_RP_SAVED=A.Canonical(state)
      A.Apply(PLAN)
      assert(A.Canonical(Game:GetProperty('JAC_RP_STATE_V1'))==JOINT_RP_SAVED)
      Game.props=A.Rules.Copy(Game.props)
    "#).exec()?;
    load_module(
        &lua,
        "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Gameplay.lua",
    )?;
    load_module(
        &lua,
        "poc/JAC_PoC_ReligionPersistence/Scripts/JAC_RP_AbsorptionAdapter.lua",
    )?;
    lua.load(
        r#"
      local A=TEST_ABSORPTION
      A.RegisterAdapter('religion-persistence',JAC_RP_AbsorptionAdapter)
      assert(A.Verify(PLAN.ID,true))
      A.Apply(PLAN)
      assert(A.Canonical(Game:GetProperty('JAC_RP_STATE_V1'))==JOINT_RP_SAVED)
    "#,
    )
    .exec()
}

#[test]
fn religion_adapter_collision_rejects_before_native_absorption_mutation() -> mlua::Result<()> {
    let lua = joint_adapter_world()?;
    lua.load(
        r#"
      local A=TEST_ABSORPTION
      local state=Game:GetProperty('JAC_RP_STATE_V1')
      state.heroes['2']={unit=50,verification='APPLIED'}
      local before=A.Canonical(state)
      local ok,err=pcall(A.Apply,PLAN)
      assert(not ok and tostring(err):find('slot occupied',1,true))
      assert(SPAWNS==0 and KILLS==0 and TRANSFERS==0,'late collision refusal damaged native assets')
      assert(Players[1].units[13] and CITIES[4]:GetOwner()==1)
      assert(A.Canonical(Game:GetProperty('JAC_RP_STATE_V1'))==before)
    "#,
    )
    .exec()
}

#[test]
fn religion_adapter_stale_hero_identity_rejects_before_native_absorption_mutation()
-> mlua::Result<()> {
    let lua = joint_adapter_world()?;
    lua.load(r#"
      local A=TEST_ABSORPTION
      local state=Game:GetProperty('JAC_RP_STATE_V1')
      state.heroes['1'].heroClass=99
      local before=A.Canonical(state)
      local ok,err=pcall(A.Apply,PLAN)
      assert(not ok and tostring(err):find('identity mismatch',1,true))
      assert(SPAWNS==0 and KILLS==0 and TRANSFERS==0,'stale reference refusal damaged native assets')
      assert(Players[1].units[13] and CITIES[4]:GetOwner()==1)
      assert(A.Canonical(Game:GetProperty('JAC_RP_STATE_V1'))==before)
    "#).exec()
}

#[test]
fn religion_adapter_failed_property_write_resumes_without_second_native_transfer()
-> mlua::Result<()> {
    let lua = joint_adapter_world()?;
    lua.load(r#"
      local A=TEST_ABSORPTION
      local set=Game.SetProperty;local fail=true
      Game.SetProperty=function(self,key,value)
        if key=='JAC_RP_STATE_V1' and fail then fail=false;error('joint RP write refused') end
        return set(self,key,value)
      end
      local ok,err=pcall(A.Apply,PLAN)
      assert(not ok and tostring(err):find('joint RP write refused',1,true))
      local state=Game:GetProperty('JAC_RP_STATE_V1')
      assert(state.heroes['1'].unit==13 and state.absorptionReceipts==nil)
      local spawns,kills,transfers=SPAWNS,KILLS,TRANSFERS
      assert(A.Resume(PLAN.ID).Status=='COMPLETE')
      assert(SPAWNS==spawns and KILLS==kills and TRANSFERS==transfers,'retry duplicated native transfer')
      state=Game:GetProperty('JAC_RP_STATE_V1')
      assert(state.heroes['1']==nil and state.heroes['2'] and state.revision==8)
      assert(state.absorptionReceipts[PLAN.ID] and A.Verify(PLAN.ID,true))
    "#).exec()
}

#[test]
fn actual_flow_journals_sorted_phase_one_rolls_and_reloads_without_reroll() -> mlua::Result<()> {
    let lua = flow_world()?;
    lua.load(
        r#"
        local F=assert(TEST_FLOW)
        local A=assert(TEST_ABSORPTION)
        local roster=A.GetRoster(1)
        roster.governor={ID='governor',Kind='PERSON',Owner=1,Name='Governor test',
          Role='GOVERNOR',Abilities={'ADMINISTRATION'},Loyalty=85}
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,roster)
        for i=1,100 do RANDOM_QUEUE[#RANDOM_QUEUE+1]=9999 end
        F.DebugSetRoll('phase-one','person','P1_ESCAPE',0)
        F.DebugSetRoll('phase-one','person','P1_BRANCH',0)
        TEST_FLOW_PLAN={ID='phase-one',From=1,To=2,Mode='MERGER',EntryConditionsMet=true,RelationsResolved=true}
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
        local F=TEST_FLOW
        local A=TEST_ABSORPTION
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
        local F=assert(TEST_FLOW)
        local A=assert(TEST_ABSORPTION)
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
        local plan={ID='forced-flow',From=1,To=2,Mode='MERGER',EntryConditionsMet=true,RelationsResolved=true}
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
        local F=TEST_FLOW
        local A=TEST_ABSORPTION
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
fn escaped_units_keep_native_snapshots_and_recover_once_from_p1_and_p3() -> mlua::Result<()> {
    let lua = flow_world()?;
    lua.load(
        r#"
        local A=TEST_ABSORPTION
        local F=TEST_FLOW
        local first=UNIT(1,20,'UNIT_WARRIOR',4)
        first.damage=19;first.moves=1;first.xp=27;first.formation=2
        local retry=UNIT(1,21,'UNIT_WARRIOR',8)
        retry.damage=11;retry.moves=1;retry.xp=13
        for i=1,400 do RANDOM_QUEUE[#RANDOM_QUEUE+1]=9999 end
        local id='unit-escapes'
        F.DebugSetSpawnDelay(0)
        F.DebugSetRoll(id,'unit:1:20','P1_ESCAPE',0)
        F.DebugSetRoll(id,'unit:1:20','P1_BRANCH',0)
        F.DebugSetRoll(id,'unit:1:21','P3_SUCCESS',9999)
        F.DebugSetRoll(id,'unit:1:21','P3_RETRY_ESCAPE',0)
        F.DebugSetRoll(id,'unit:1:21','P3_RETRY_BRANCH',0)
        local choice=F.Begin({ID=id,From=1,To=2,Mode='MERGER',
          EntryConditionsMet=true,RelationsResolved=true})
        local byID={};for _,entry in ipairs(choice.Entries) do byID[entry.ID]=entry end
        assert(byID['unit:1:20'].EscapeOutcome=='ESCAPE'
          and byID['unit:1:21'].EscapeOutcome==nil)
        for _,entry in ipairs(choice.Entries) do
          if not entry.EscapeOutcome then
            local action=entry.Kind=='UNIT' and 'INCORPORATE'
              or entry.Kind=='PERSON' and 'RECRUIT'
              or entry.Kind=='HERO' and 'PROTECT'
              or entry.Kind=='GREAT_PERSON' and 'RECRUIT'
              or entry.Kind=='ITEM' and 'TRANSFER'
              or entry.Kind=='UNLOCK' and 'INHERIT' or 'PRESERVE'
            F.Select(2,id,entry.ID,action)
            if entry.ID~='unit:1:21' then F.DebugSetRoll(id,entry.ID,'P3_SUCCESS',0) end
          end
        end
        local result=F.Resolve(2,id)
        assert(result.Phase=='RESULT',tostring(result.Error))
        local archive=A.GetState().Archive
        for _,unitID in ipairs({20,21}) do
          local asset='unit:1:'..unitID
          local record=assert(archive[asset],'escaped Unit has no archived identity')
          local world=assert(F.GetWorld(id..':'..asset),'escaped Unit has no world reward')
          assert(record.Kind=='UNIT' and record.ID==asset and world.Snapshot.ID==unitID)
          assert(world.Status=='WAIT_SITE' and world.Kind=='UNIT'
            and world.Snapshot.Damage==(unitID==20 and 19 or 11))
          assert(UnitManager.GetUnit(1,unitID)==nil)
        end
        assert(F.Get(id).AppliedPlan.EntryConditionsMet==true)
        local rolls=#RANDOM_CALLS
        CURRENT_TURN=CURRENT_TURN+1
        F.Tick()
        for _,unitID in ipairs({20,21}) do
          local asset='unit:1:'..unitID
          local key=id..':'..asset
          local world=assert(F.GetWorld(key))
          assert(world.Status=='WAIT_CLEAR')
          UNIT(2,NEXT_ID,'UNIT_WARRIOR',world.Site);NEXT_ID=NEXT_ID+1
          PLOTS[world.Site].improvement=-1
          F.Tick()
          world=F.GetWorld(key)
          assert(world.Status=='COMPLETE' and world.Recipient==2,tostring(world.Error))
          local recovered=assert(A.FindUnit(2,'recovery:'..key))
          assert(recovered.damage==(unitID==20 and 19 or 11)
            and recovered.xp==(unitID==20 and 27 or 13)
            and recovered.moves==1)
          local record=assert(A.GetRoster(2)[asset])
          assert(record.ID==asset and record.Unit.ID==recovered:GetID()
            and record.RecoveryToken=='recovery:'..key)
        end
        local count=Players[2]:GetUnits():GetCount()
        F.Tick()
        assert(Players[2]:GetUnits():GetCount()==count and #RANDOM_CALLS==rolls,
          'duplicate publication or reward tick created another Unit')
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
            local A=TEST_ABSORPTION
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
            local A=TEST_ABSORPTION
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
fn separate_city_transfers_keep_their_first_physical_turn_across_errors_and_reload()
-> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
        local A=TEST_ABSORPTION
        PLAN.Mode='CONQUEST';PLAN.FlowPolicy=true
        FAIL_CITY=true
        assert(not pcall(A.Apply,PLAN) and CITIES[4]:GetOwner()==1)
        CURRENT_TURN=9
        assert(not pcall(A.Resume,PLAN.ID) and CITIES[4]:GetOwner()==1,
          'failed-before-mutation transfer changed its owner')
        local transfer=CityManager.TransferCity
        local failAfter=true
        CityManager.TransferCity=function(city,owner)
          if city==CITIES[4] and failAfter then
            failAfter=false
            transfer(city,owner)
            error('interrupted after the first native city transfer')
          end
          return transfer(city,owner)
        end
        CURRENT_TURN=10;FAIL_CITY=false
        assert(not pcall(A.Resume,PLAN.ID) and CITIES[4]:GetOwner()==2
          and CITIES[4]:GetProperty(A.Rules.GOVERNANCE_KEY)==nil)
        local first=assert(A.GetState().Transactions[PLAN.ID].Cities[1])
        assert(first.X==4 and first.TransferTurn==10,
          'the transfer boundary was not saved before the native call')
        Game.props=A.Rules.Copy(Game.props)
    "#,
    )
    .exec()?;
    load_module(
        &lua,
        "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Gameplay.lua",
    )?;
    lua.load(
        r#"
        local A=TEST_ABSORPTION
        CURRENT_TURN=11;FAIL_CITY=true
        assert(not pcall(A.Resume,PLAN.ID) and CITIES[8]:GetOwner()==1)
        local g=assert(CITIES[4]:GetProperty(A.Rules.GOVERNANCE_KEY))
        assert(g.Turn==10 and g.ImmediateApplied and g.FrictionApplied==0,
          'the first city lost its actual transfer date after reload')
        A.ApplyCityFriction()
        assert(CITIES[4]:GetProperty(A.Rules.GOVERNANCE_KEY).FrictionApplied==1)
        local saved=A.GetState()
        saved.Transactions[PLAN.ID].Cities[1].TransferTurn=nil
        Game:SetProperty(A.Rules.STATE_KEY,saved) -- a pre-fix save already has city governance
        Game.props=A.Rules.Copy(Game.props)
    "#,
    )
    .exec()?;
    load_module(
        &lua,
        "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Gameplay.lua",
    )?;
    lua.load(
        r#"
        local A=TEST_ABSORPTION
        CURRENT_TURN=12;FAIL_CITY=false
        local set=CITIES[8].SetProperty
        local failSetter=true
        CITIES[8].SetProperty=function(self,key,value)
          if key==A.Rules.GOVERNANCE_KEY and value and value.ID==PLAN.ID and failSetter then
            failSetter=false;error('governance setter interrupted')
          end
          return set(self,key,value)
        end
        assert(not pcall(A.Resume,PLAN.ID) and CITIES[8]:GetOwner()==2
          and CITIES[8]:GetProperty(A.Rules.GOVERNANCE_KEY)==nil)
        assert(A.GetState().Transactions[PLAN.ID].Cities[2].TransferTurn==12,
          'second city transfer date was not saved before its interrupted setter')
        A.ApplyCityFriction()
        assert(CITIES[4]:GetProperty(A.Rules.GOVERNANCE_KEY).FrictionApplied==2)
        Game.props=A.Rules.Copy(Game.props)
    "#,
    )
    .exec()?;
    load_module(
        &lua,
        "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Gameplay.lua",
    )?;
    lua.load(
        r#"
        local A=TEST_ABSORPTION
        CURRENT_TURN=13
        assert(A.Resume(PLAN.ID).Status=='COMPLETE')
        local first=CITIES[4]:GetProperty(A.Rules.GOVERNANCE_KEY)
        local second=CITIES[8]:GetProperty(A.Rules.GOVERNANCE_KEY)
        assert(first.Turn==10 and second.Turn==12,
          'reload or delayed setter reset a transferred city duration')
        for turn=13,16 do CURRENT_TURN=turn;A.ApplyCityFriction();A.ApplyCityFriction() end
        first=CITIES[4]:GetProperty(A.Rules.GOVERNANCE_KEY)
        second=CITIES[8]:GetProperty(A.Rules.GOVERNANCE_KEY)
        assert(first.FrictionApplied==3 and first.LastFrictionTurn==13
          and second.FrictionApplied==3 and second.LastFrictionTurn==15)
        assert(CITIES[4].loyalty==66 and CITIES[8].loyalty==66,
          'each city did not receive exactly three full friction turns')
    "#,
    )
    .exec()
}

#[test]
fn core_without_loyalty_getter_waits_for_fresh_city_publications() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(
        r#"
        local A=TEST_ABSORPTION
        local R=A.Rules
        PLAN.Mode='CONQUEST';PLAN.FlowPolicy=true
        CITY_OBSERVATIONS={};LOYALTY_CALLS={}
        ExposedMembers.JAC_HeroReadAPI={GetObservation=function(token)
          return CITY_OBSERVATIONS[token] end,
          ReadCity=function(owner,id,method)
            assert(method=='LOYALTY')
            return assert(Players[owner]:GetCities():FindID(id)).loyalty
          end}
        for _,city in Players[1]:GetCities():Members() do
          city.GetCulturalIdentity=function() return {} end
          local change=city.ChangeLoyalty
          city.ChangeLoyalty=function(self,amount)
            LOYALTY_CALLS[#LOYALTY_CALLS+1]={X=self.x,Amount=amount,Turn=CURRENT_TURN}
            return change(self,amount)
          end
        end
        function PUBLISH_LOYALTY(stale)
          for _,city in Players[2]:GetCities():Members() do
            local g=city:GetProperty(R.GOVERNANCE_KEY)
            local request=g and g.LoyaltyRead
            if request and not CITY_OBSERVATIONS[request.Token] then
              CITY_OBSERVATIONS[request.Token]={Owner=request.Owner,ID=request.ID,
                Turn=request.Turn,Value=stale and g.FrictionPending
                  and request.Phase=='friction:6:after:0' and city.x==4
                  and g.FrictionPending.Before or city.loyalty}
            end
          end
        end
        local tx=A.Apply(PLAN)
        assert(tx.Status=='WAITING_UI' and tx.CityLoyaltyPending
          and CITIES[4]:GetOwner()==2 and CITIES[8]:GetOwner()==1)
        assert(#LOYALTY_CALLS==0,'unpublished transfer applied a delta')
        assert(A.Resume(PLAN.ID).Status=='WAITING_UI' and #LOYALTY_CALLS==0)
        PUBLISH_LOYALTY(false)
        tx=A.Resume(PLAN.ID)
        assert(tx.Status=='WAITING_UI' and #LOYALTY_CALLS==1 and CITIES[4].loyalty==72)
        assert(A.Resume(PLAN.ID).Status=='WAITING_UI' and #LOYALTY_CALLS==1,
          'stale after-read duplicated city loyalty')
        PUBLISH_LOYALTY(false)
        tx=A.Resume(PLAN.ID)
        assert(tx.Status=='WAITING_UI' and CITIES[8]:GetOwner()==2)
        PUBLISH_LOYALTY(false)
        tx=A.Resume(PLAN.ID)
        assert(tx.Status=='WAITING_UI' and #LOYALTY_CALLS==2)
        PUBLISH_LOYALTY(false)
        tx=A.Resume(PLAN.ID)
        assert(tx.Status=='COMPLETE' and #LOYALTY_CALLS==2)
        for _,x in ipairs({4,8}) do
          local g=CITIES[x]:GetProperty(R.GOVERNANCE_KEY)
          assert(g.ImmediateApplied and g.ImmediateBefore==87
            and g.ImmediateTarget==72 and CITIES[x].loyalty==72)
        end
        CURRENT_TURN=6
        A.ApplyCityFriction()
        assert(#LOYALTY_CALLS==2 and CITIES[4]:GetProperty(R.GOVERNANCE_KEY).FrictionPending,
          'friction ran before a published read')
        PUBLISH_LOYALTY(false);A.ApplyCityFriction()
        assert(#LOYALTY_CALLS==4)
        A.ApplyCityFriction()
        assert(#LOYALTY_CALLS==4,'unpublished friction after-read duplicated the delta')
        PUBLISH_LOYALTY(true);A.ApplyCityFriction()
        assert(#LOYALTY_CALLS==4 and CITIES[4]:GetProperty(R.GOVERNANCE_KEY).FrictionApplied==0,
          'stale UI value completed or repeated friction')
        PUBLISH_LOYALTY(false);A.ApplyCityFriction()
        assert(CITIES[4]:GetProperty(R.GOVERNANCE_KEY).FrictionApplied==1)
        for _,x in ipairs({4,8}) do CITIES[x].props=R.Copy(CITIES[x].props) end
        Game.props=R.Copy(Game.props)
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
        local A=TEST_ABSORPTION
        local R=A.Rules
        A.ApplyCityFriction()
        assert(#LOYALTY_CALLS==4,'reload repeated a completed turn')
        for turn=7,8 do
          CURRENT_TURN=turn
          A.ApplyCityFriction();PUBLISH_LOYALTY(false)
          A.ApplyCityFriction();PUBLISH_LOYALTY(false)
          A.ApplyCityFriction()
        end
        CURRENT_TURN=9;A.ApplyCityFriction()
        assert(#LOYALTY_CALLS==8,'friction duration extended or duplicated')
        for _,x in ipairs({4,8}) do
          local g=CITIES[x]:GetProperty(R.GOVERNANCE_KEY)
          assert(g.FrictionApplied==3 and g.LastFrictionTurn==8
            and CITIES[x].loyalty==66)
        end
        "#,
    )
    .exec()
}

#[test]
fn official_mode_and_custom_heroes_are_required_before_any_roll_or_mutation() -> mlua::Result<()> {
    let lua = flow_world()?;
    lua.load(
        r#"
        local F=TEST_FLOW
        local plan={ID='hero-mode',From=1,To=2,Mode='MERGER',EntryConditionsMet=true,RelationsResolved=true}
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
fn entry_and_relation_proofs_and_both_warlords_precede_all_rolls() -> mlua::Result<()> {
    let lua = flow_world()?;
    lua.load(
        r#"
        local F=TEST_FLOW
        local R=TEST_ABSORPTION.Rules
        local before=R.Canonical(Game.props)
        for _,flags in ipairs({{}, {RelationsResolved=true}, {EntryConditionsMet=false},
          {EntryConditionsMet=false,RelationsResolved=true}, {EntryConditionsMet=true},
          {EntryConditionsMet=true,RelationsResolved=false}}) do
          local plan={ID='validation',From=1,To=2,Mode='MERGER',
            EntryConditionsMet=flags.EntryConditionsMet,RelationsResolved=flags.RelationsResolved}
          assert(not pcall(F.Begin,plan),'unverified entry/relation was accepted')
          assert(R.Canonical(Game.props)==before and #RANDOM_CALLS==0
            and SPAWNS==0 and KILLS==0 and TRANSFERS==0)
        end
        local heroes=GameInfo.HeroClasses
        local original=getmetatable(heroes)
        for _,only in ipairs({heroes[0],heroes[1]}) do
          setmetatable(heroes,{__call=function() local once=false;return function()
            if once then return nil end;once=true;return only end end})
          assert(not pcall(F.Begin,{ID='validation',From=1,To=2,Mode='MERGER',
            EntryConditionsMet=true,RelationsResolved=true}),
            'a required Warlord class was omitted')
          assert(R.Canonical(Game.props)==before and #RANDOM_CALLS==0
            and SPAWNS==0 and KILLS==0 and TRANSFERS==0)
        end
        setmetatable(heroes,original)
        local extra={Index=9,HeroClassType='HEROCLASS_JAC_FUTURE',UnitType='UNIT_HERO_JAC_FUTURE'}
        setmetatable(heroes,{__call=function(t)
          local i=0;local all={t[0],t[1],extra};return function() i=i+1;return all[i] end
        end})
        for i=1,200 do RANDOM_QUEUE[#RANDOM_QUEUE+1]=9999 end
        local plan={ID='validated',From=1,To=2,Mode='MERGER',
          EntryConditionsMet=true,RelationsResolved=true}
        local choice=F.Begin(plan)
        assert(choice.Phase=='CHOICE' and choice.Request.EntryConditionsMet==true
          and choice.Request.RelationsResolved==true and #RANDOM_CALLS>0)
        for _,entry in ipairs(choice.Entries) do
          if not entry.EscapeOutcome then
            local action=entry.Kind=='UNIT' and 'INCORPORATE'
              or entry.Kind=='PERSON' and 'RECRUIT'
              or entry.Kind=='HERO' and 'PROTECT'
              or entry.Kind=='GREAT_PERSON' and 'RECRUIT'
              or entry.Kind=='ITEM' and 'TRANSFER'
              or entry.Kind=='UNLOCK' and 'INHERIT' or 'PRESERVE'
            F.Select(2,plan.ID,entry.ID,action)
            F.DebugSetRoll(plan.ID,entry.ID,'P3_SUCCESS',0)
          end
        end
        local result=F.Resolve(2,plan.ID)
        assert(result.Phase=='RESULT',tostring(result.Error))
        assert(result.AppliedPlan.EntryConditionsMet==true
          and result.AppliedPlan.RelationsResolved==true)
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
        local F=TEST_FLOW
        local before=JAC_AbsorptionRules.Canonical(Game.props)
        local ok=pcall(F.Begin,{ID='human-donor',From=1,To=2,Mode='MERGER',EntryConditionsMet=true,RelationsResolved=true})
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
        local F=TEST_FLOW
        for i=1,100 do RANDOM_QUEUE[#RANDOM_QUEUE+1]=9999 end
        local choice=F.Begin({ID='operation-boundary',From=1,To=2,Mode='MERGER',EntryConditionsMet=true,RelationsResolved=true})
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
        local A=TEST_ABSORPTION
        local F=TEST_FLOW
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
        local plan={ID='native-suicide',From=1,To=2,Mode='MERGER',EntryConditionsMet=true,RelationsResolved=true}
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
        local A=TEST_ABSORPTION
        local F=TEST_FLOW
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
        local plan={ID='native-transfer',From=1,To=2,Mode='MERGER',EntryConditionsMet=true,RelationsResolved=true}
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
fn blocked_hero_escrow_resumes_after_reload_without_reroll_or_duplicate() -> mlua::Result<()> {
    let lua = flow_world()?;
    lua.load(
        r#"
        local A=TEST_ABSORPTION
        local F=TEST_FLOW
        local hero=UNIT(1,13,'UNIT_HERO_JAC_WARLORD_A',4)
        hero.lifespan=17;hero.charges=2;hero.renown=13
        HERO_CLAIMS[0]=1;HERO_ORIGINS[0]={player=1,id=CITIES[4].id}
        HERO_DISCOVERED['1:0']=true
        local person={ID='escrow-hero',Kind='HERO',Owner=1,Unit={Owner=1,ID=13}}
        hero:SetProperty(A.Rules.PERSON_KEY,person)
        local roster=A.GetRoster(1);roster['escrow-hero']=person
        Players[1]:SetProperty(A.Rules.ROSTER_KEY,roster)
        for i=1,400 do RANDOM_QUEUE[#RANDOM_QUEUE+1]=9999 end
        local plan={ID='escrow-retry',From=1,To=2,Mode='CONQUEST',
          EntryConditionsMet=true,RelationsResolved=true}
        local flow=F.Begin(plan)
        for _,entry in ipairs(flow.Entries) do
          if not entry.EscapeOutcome then
            local action=entry.Kind=='UNIT' and 'INCORPORATE'
              or entry.Kind=='PERSON' and 'RECRUIT'
              or entry.Kind=='HERO' and 'PROTECT'
              or entry.Kind=='GREAT_PERSON' and 'RECRUIT'
              or entry.Kind=='ITEM' and 'TRANSFER'
              or entry.Kind=='UNLOCK' and 'INHERIT' or 'PRESERVE'
            F.Select(2,plan.ID,entry.ID,action)
            F.DebugSetRoll(plan.ID,entry.ID,'P3_SUCCESS',0)
          end
        end
        FAIL_CITY=true
        flow=F.Resolve(2,plan.ID)
        assert(flow.Phase=='BLOCKED' and flow.AppliedPlan and flow.Error,
          'transfer failure did not keep the applied plan and cause')
        local tx=assert(A.GetState().Transactions[plan.ID])
        assert(tx.Status=='BLOCKED' and tx.Error and UnitManager.GetUnit(1,13)==nil,
          'failure did not occur after donor Hero escrow')
        assert(HERO_CREATIONS==0 and #RANDOM_CALLS>0)
        ESCROW_ROLLS=#RANDOM_CALLS
        FAIL_CITY=false
        CURRENT_TURN=10
        local transfer=CityManager.TransferCity
        local interrupted=false
        CityManager.TransferCity=function(city,owner)
          transfer(city,owner)
          if city==CITIES[4] and not interrupted then
            interrupted=true
            error('interrupted after native Hero origin transfer')
          end
        end
        assert(F.Resolve(2,plan.ID).Phase=='BLOCKED' and CITIES[4]:GetOwner()==2)
        local origin=A.GetState().Transactions[plan.ID].Cities[1]
        assert(origin.X==4 and origin.TransferTurn==10,
          'early Hero origin transfer did not save the actual transfer turn')
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
        local A=TEST_ABSORPTION
        local F=TEST_FLOW
        CURRENT_TURN=11
        local result=F.Resolve(2,'escrow-retry')
        assert(result.Phase=='RESULT',tostring(result.Error))
        assert(result.FirstError and result.PreviousError
          and A.GetState().Transactions['escrow-retry'].FirstError,
          'retry discarded the original failure cause')
        assert(#RANDOM_CALLS==ESCROW_ROLLS and HERO_CREATIONS==1,
          'retry rerolled or created a second Hero')
        local record=assert(A.GetRoster(2)['escrow-hero'])
        local unit=assert(A.FindUnit(2,record.Unit.Token))
        assert(unit:IsHero() and unit:GetProperty(A.Rules.PERSON_KEY).ID=='escrow-hero'
          and unit.lifespan==17 and unit.charges==2 and unit.renown==13)
        assert(F.Resolve(2,'escrow-retry').Phase=='RESULT' and HERO_CREATIONS==1)
        assert(CITIES[4]:GetProperty(A.Rules.GOVERNANCE_KEY).Turn==10
          and CITIES[8]:GetProperty(A.Rules.GOVERNANCE_KEY).Turn==11,
          'Hero and ordinary city policy dates differ from physical transfer')
        for turn=11,14 do CURRENT_TURN=turn;A.ApplyCityFriction();A.ApplyCityFriction() end
        for _,x in ipairs({4,8}) do
          local g=CITIES[x]:GetProperty(A.Rules.GOVERNANCE_KEY)
          assert(g.FrictionApplied==3 and CITIES[x].loyalty==66,
            'Hero origin delay lost friction or applied it twice')
        end
        "#,
    )
    .exec()
}

#[test]
fn actual_panel_excludes_cityless_return_owners_and_rechecks_before_sending() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(PANEL_WORLD)
        .set_name("absorption_panel.lua")
        .exec()?;
    lua.load(
        r#"
        Players[0].cities={}
        Players[0].IsAlive=function() return true end
        TEST_UI_PENDING={ID='return-flow',From=1,To=2,Mode='MERGER',Phase='CHOICE',Entries={
          {ID='artifact:seal',Kind='ITEM',Name='Seal',AllowedActions={'TRANSFER','RETURN'}}}}
        ExposedMembers.JAC_AbsorptionFlow={GetPending=function()
          return JAC_AbsorptionRules.Copy(TEST_UI_PENDING)
        end}
    "#,
    )
    .exec()?;
    load_module(&lua, "poc/JAC_PoC_Absorption/UI/JAC_AbsorptionFlow.lua")?;
    lua.load(
        r#"
        local panel=assert(JAC_AbsorptionFlow_UI)
        assert(panel.ChooseAction('RETURN'))
        local actions=panel.GetView().actions
        assert(#actions==2 and actions[1].key=='owner:3', 'cityless living owner was offered')
        assert(not panel.ChooseOwner(0) and #OPERATION_CALLS==0)
        Players[3].cities={}
        assert(not panel.ChooseOwner(3) and #OPERATION_CALLS==0,
          'stale owner without a city received an operation')
        panel.Refresh()
        assert(not panel.ChooseAction('RETURN') and #OPERATION_CALLS==0,
          'return offered without any eligible faction')
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
        TEST_UI_PENDING.Phase='BLOCKED'
        TEST_UI_PENDING.AppliedPlan={ID='ui-flow'}
        TEST_UI_PENDING.Turn=CURRENT_TURN
        TEST_UI_PENDING.Error='raw engine stack trace'
        Events.GameCoreEventPublishComplete.Emit()
        assert(not Controls.ConfirmButton:IsHidden()
          and Controls.ConfirmButton:GetText()=='LOC_JAC_ABS_FLOW_RETRY')
        assert(not panel.GetView().hint:find('raw engine stack trace',1,true),
          'the retry panel exposed the internal exception')
        Controls.ConfirmButton:DoLeftClick()
        assert(OPERATION_CALLS[5].Params.Command=='RESOLVE')
        TEST_UI_PENDING.AppliedPlan=nil
        Events.GameCoreEventPublishComplete.Emit()
        assert(Controls.ConfirmButton:IsHidden(),
          'prevalidation block offered a transfer retry')
        TEST_UI_PENDING.AppliedPlan={ID='ui-flow'}
        TEST_UI_PENDING.Turn=CURRENT_TURN-1
        Events.GameCoreEventPublishComplete.Emit()
        assert(not Controls.ConfirmButton:IsHidden(),
          'saved transfer could not resume on a later turn')
        Controls.ConfirmButton:DoLeftClick()
        assert(#OPERATION_CALLS==6 and OPERATION_CALLS[6].Params.Command=='RESOLVE')
        Players[2].IsTurnActive=function() return false end
        assert(not panel.Resolve() and #OPERATION_CALLS==6,
          'later-turn retry bypassed the active human turn')
        assert(#RANDOM_CALLS==0,'choice UI changed gameplay random stream')
        "#,
    )
    .exec()
}

#[test]
fn ui_exports_only_detached_reads_and_cannot_forge_selection() -> mlua::Result<()> {
    let lua = flow_world()?;
    lua.load(
        r#"
        local A,F=TEST_ABSORPTION,TEST_FLOW
        local allowedA={GetState=true,GetRoster=true,GetUIPhase=true,Canonical=true,
          GetPropertyKeys=true}
        local allowedF={Get=true,GetPending=true,GetWorld=true,GetPendingWorlds=true}
        local countA,countF=0,0
        for key,value in pairs(ExposedMembers.JAC_Absorption) do
          assert(allowedA[key] and type(value)=='function',key);countA=countA+1
        end
        for key,value in pairs(ExposedMembers.JAC_AbsorptionFlow) do
          assert(allowedF[key] and type(value)=='function',key);countF=countF+1
        end
        assert(countA==5 and countF==4)
        local keys=ExposedMembers.JAC_Absorption.GetPropertyKeys()
        assert(keys.PERSON_KEY==A.Rules.PERSON_KEY and keys.TOKEN_KEY==A.Rules.TOKEN_KEY)
        keys.PERSON_KEY='forged person key';keys.TOKEN_KEY='forged token key'
        local fresh=ExposedMembers.JAC_Absorption.GetPropertyKeys()
        assert(fresh.PERSON_KEY==A.Rules.PERSON_KEY and fresh.TOKEN_KEY==A.Rules.TOKEN_KEY)
        assert(ExposedMembers.JAC_Absorption.Apply==nil
          and ExposedMembers.JAC_AbsorptionFlow.Select==nil)
        for i=1,100 do RANDOM_QUEUE[#RANDOM_QUEUE+1]=9999 end
        local flow=F.Begin({ID='private-boundary',From=1,To=2,Mode='MERGER',
          EntryConditionsMet=true,RelationsResolved=true})
        local before=A.Canonical(Game.props)
        local detached=ExposedMembers.JAC_AbsorptionFlow.Get(flow.ID)
        detached.Phase='DONE';detached.Entries[1].SelectedAction='EXECUTE'
        local roster=ExposedMembers.JAC_Absorption.GetRoster(1);roster.hero.Owner=2
        local state=ExposedMembers.JAC_Absorption.GetState();state.Archive.fake={ID='fake'}
        ExposedMembers.JAC_AbsorptionFlow.Select=function() error('UI injection') end
        GameEvents.JAC_AbsorptionFlow_Operation.Emit(1,{Command='SELECT',FlowID=flow.ID,
          AssetID=flow.Entries[1].ID,Action=flow.Entries[1].AllowedActions[1]})
        assert(A.Canonical(Game.props)==before)
        GameEvents.JAC_AbsorptionFlow_Operation.Emit(2,{Command='SELECT',FlowID=flow.ID,
          AssetID=flow.Entries[1].ID,Action=flow.Entries[1].AllowedActions[1]})
        assert(Players[2]:GetProperty('JAC_ABSORPTION_FLOW_LAST_OPERATION_V1').Accepted)
        assert(F.Get(flow.ID).Entries[1].SelectedAction==flow.Entries[1].AllowedActions[1])
    "#,
    )
    .exec()
}

fn prepare_choices(lua: &Lua) -> mlua::Result<()> {
    lua.load(
        r#"
        function CHOOSE_ALL(id,mode)
          local F=TEST_FLOW
          for i=1,300 do RANDOM_QUEUE[#RANDOM_QUEUE+1]=9999 end
          local flow=F.Begin({ID=id,From=1,To=2,Mode=mode or 'MERGER',
            EntryConditionsMet=true,RelationsResolved=true})
          for _,entry in ipairs(flow.Entries) do
            if not entry.EscapeOutcome then
              local action=entry.Kind=='UNIT' and 'INCORPORATE'
                or entry.Kind=='PERSON' and 'RECRUIT'
                or entry.Kind=='HERO' and 'PROTECT'
                or entry.Kind=='GREAT_PERSON' and 'RECRUIT'
                or entry.Kind=='ITEM' and 'TRANSFER'
                or entry.Kind=='UNLOCK' and 'INHERIT' or 'PRESERVE'
              F.Select(2,id,entry.ID,action)
              F.DebugSetRoll(id,entry.ID,'P3_SUCCESS',0)
            end
          end
          return flow
        end
    "#,
    )
    .exec()
}

#[test]
fn accepted_return_choices_are_revalidated_before_final_rolls_and_can_be_corrected()
-> mlua::Result<()> {
    let lua = flow_world()?;
    prepare_choices(&lua)?;
    lua.load(
        r#"
        local A,F=TEST_ABSORPTION,TEST_FLOW
        CHOOSE_ALL('stale-return')
        F.Select(2,'stale-return','artifact','RETURN',3)
        F.Select(2,'stale-return','collection','RETURN',0)
        local beforeRolls=#RANDOM_CALLS
        local beforeSpawns,beforeKills,beforeTransfers=SPAWNS,KILLS,TRANSFERS
        Players[3].cities={};Players[3].IsAlive=function() return true end
        Game.props=A.Rules.Copy(Game.props)
        local result=F.Resolve(2,'stale-return')
        assert(result.Phase=='CHOICE' and result.AppliedPlan==nil)
        assert(#RANDOM_CALLS==beforeRolls and SPAWNS==beforeSpawns
          and KILLS==beforeKills and TRANSFERS==beforeTransfers)
        assert(A.GetState().Transactions['stale-return']==nil)
        for _,entry in ipairs(result.Entries) do
          assert(not entry.Rolls or not entry.Rolls.P3_SUCCESS)
          if entry.ID=='artifact' then
            assert(entry.SelectedAction==nil and entry.ReturnOwner==nil)
          elseif entry.ID=='collection' then
            assert(entry.SelectedAction=='RETURN' and entry.ReturnOwner==0)
          else assert(entry.EscapeOutcome or entry.SelectedAction) end
        end
        F.Select(2,'stale-return','artifact','RETURN',0)
        local complete=F.Resolve(2,'stale-return')
        assert(complete.Phase=='RESULT',tostring(complete.Error))
        assert(A.GetRoster(0).artifact and A.GetRoster(0).collection)
    "#,
    )
    .exec()
}

#[test]
fn return_owner_lost_after_final_rolls_reopens_only_destination_before_mutation() -> mlua::Result<()>
{
    let lua = flow_world()?;
    prepare_choices(&lua)?;
    lua.load(
        r#"
        local A,F=TEST_ABSORPTION,TEST_FLOW
        CHOOSE_ALL('waiting-return')
        F.Select(2,'waiting-return','artifact','RETURN',3)
        F.Select(2,'waiting-return','collection','RETURN',0)
        F.DebugSetRoll('waiting-return','collection','P3_SUCCESS',9999)
        ORIGINAL_SPREAD=Players[1].units[3].GetSpreadCharges
        Players[1].units[3].GetSpreadCharges=nil
        ExposedMembers.JAC_HeroReadAPI={GetObservation=function() return nil end}
        local waiting=F.Resolve(2,'waiting-return')
        assert(waiting.Phase=='APPLYING' and waiting.WaitingFor=='WAITING_UI',
          waiting.Phase..':'..tostring(waiting.Error)..':'..tostring(waiting.WaitingFor))
        assert(A.GetState().Transactions['waiting-return'].AwaitingPreparation)
        SAVED_FINAL_ROLLS=#RANDOM_CALLS
        SAVED_WAIT_SPAWNS,SAVED_WAIT_KILLS,SAVED_WAIT_TRANSFERS=SPAWNS,KILLS,TRANSFERS
        SAVED_FINAL_ENTRIES=A.Rules.Copy(waiting.Entries)
        Players[3].cities={};Players[3].IsAlive=function() return true end
        CURRENT_TURN=6
        Game.props=A.Rules.Copy(Game.props)
    "#,
    )
    .exec()?;
    load_module(
        &lua,
        "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Gameplay.lua",
    )?;
    load_module(&lua, FLOW)?;
    lua.load(
        r#"
        local A,F=TEST_ABSORPTION,TEST_FLOW
        local result=F.Resume('waiting-return')
        assert(result.Phase=='CHOICE' and result.AppliedPlan==nil,
          'post-roll destination loss must allow correction before mutation')
        assert(A.GetState().Transactions['waiting-return']==nil)
        assert(#RANDOM_CALLS==SAVED_FINAL_ROLLS and SPAWNS==SAVED_WAIT_SPAWNS
          and KILLS==SAVED_WAIT_KILLS and TRANSFERS==SAVED_WAIT_TRANSFERS)
        for i,entry in ipairs(result.Entries) do
          assert(A.Rules.Equal(entry.Rolls,SAVED_FINAL_ENTRIES[i].Rolls))
          assert(entry.FinalOutcome==SAVED_FINAL_ENTRIES[i].FinalOutcome)
          if entry.ID=='artifact' then
            assert(entry.ReturnOwnerPending and not entry.SelectedAction and not entry.ReturnOwner)
          elseif entry.ID=='collection' then
            assert(entry.SelectedAction=='RETURN' and entry.ReturnOwner==0)
            assert(entry.FinalOutcome=='SEIZE' and entry.Success==false)
          end
        end
        assert(not pcall(F.Select,2,result.ID,'artifact','TRANSFER'))
        assert(not pcall(F.Select,2,result.ID,'collection','SEIZE'))
        assert(not pcall(F.Select,2,result.ID,'collection','RETURN',2))
        assert(not pcall(F.Select,2,result.ID,'artifact','RETURN',3))
        Game.props=A.Rules.Copy(Game.props)
    "#,
    )
    .exec()?;
    load_module(&lua, FLOW)?;
    lua.load(
        r#"
        local A,F=TEST_ABSORPTION,TEST_FLOW
        F.Select(2,'waiting-return','artifact','RETURN',0)
        local cities=Players[0].cities
        Players[0].cities={}
        local again=F.Resolve(2,'waiting-return')
        assert(again.Phase=='CHOICE' and again.FinalRollsComplete)
        for _,entry in ipairs(again.Entries) do
          if entry.FinalOutcome=='RETURN' then
            assert(entry.ReturnOwnerPending and not entry.SelectedAction and not entry.ReturnOwner,
              'a second destination loss hid the correction actions')
          end
          if entry.ID=='collection' then
            assert(entry.FinalOutcome=='SEIZE' and not entry.ReturnOwnerPending
              and entry.SelectedAction=='RETURN','failed return incorrectly reopened its old recipient')
          end
        end
        assert(#RANDOM_CALLS==SAVED_FINAL_ROLLS and A.GetState().Transactions['waiting-return']==nil)
        Game.props=A.Rules.Copy(Game.props)
        Players[0].cities=cities
        F.Select(2,'waiting-return','artifact','RETURN',0)
        Players[1].units[3].GetSpreadCharges=ORIGINAL_SPREAD
        local done=F.Resolve(2,'waiting-return')
        assert(done.Phase=='RESULT',tostring(done.Error))
        assert(#RANDOM_CALLS==SAVED_FINAL_ROLLS,'destination correction rerolled outcomes')
        assert(A.GetRoster(0).artifact and A.GetRoster(0).collection==nil and A.GetRoster(2).collection)
        assert(A.GetRoster(3).artifact==nil)
        assert(F.Resolve(2,done.ID).Phase=='RESULT' and #RANDOM_CALLS==SAVED_FINAL_ROLLS)
    "#,
    )
    .exec()
}

#[test]
fn frozen_return_correction_panel_offers_only_the_missing_destination() -> mlua::Result<()> {
    let lua = world()?;
    lua.load(PANEL_WORLD).exec()?;
    lua.load(
        r#"
        TEST_UI_PENDING={ID='frozen-return',From=1,To=2,Mode='MERGER',Phase='CHOICE',
          FinalRollsComplete=true,Entries={
          {ID='frozen',Kind='ITEM',Name='Kept',SelectedAction='TRANSFER',FinalOutcome='TRANSFER',
            AllowedActions={'TRANSFER','RETURN'}},
          {ID='missing',Kind='ITEM',Name='Return',ReturnOwnerPending=true,FinalOutcome='RETURN',
            AllowedActions={'TRANSFER','LOOT','SEIZE','RETURN'}}}}
        ExposedMembers.JAC_AbsorptionFlow={GetPending=function()
          return JAC_AbsorptionRules.Copy(TEST_UI_PENDING) end}
    "#,
    )
    .exec()?;
    load_module(&lua, "poc/JAC_PoC_Absorption/UI/JAC_AbsorptionFlow.lua")?;
    lua.load(
        r#"
        local panel=JAC_AbsorptionFlow_UI
        assert(not panel.GetView().canResolve and Controls.ConfirmButton:IsDisabled())
        assert(panel.GetView().rows[1].outcome:find('LOC_JAC_ABS_FLOW_FINAL',1,true))
        assert(panel.GetView().rows[2].outcome:find('LOC_JAC_ABS_FLOW_CHOOSE_OWNER',1,true))
        TEST_ENTRY_MANAGER.instances[1].Choose:DoLeftClick()
        assert(#panel.GetView().actions==0 and not panel.ChooseAction('RETURN'))
        TEST_ENTRY_MANAGER.instances[2].Choose:DoLeftClick()
        assert(#panel.GetView().actions==1 and panel.GetView().actions[1].key=='action:RETURN')
        assert(not panel.ChooseAction('SEIZE') and #OPERATION_CALLS==0)
        assert(panel.ChooseAction('RETURN') and panel.ChooseOwner(3))
        assert(#OPERATION_CALLS==1 and OPERATION_CALLS[1].Params.ReturnOwner==3
          and OPERATION_CALLS[1].Params.Action=='RETURN')
        assert(#RANDOM_CALLS==0)
    "#,
    )
    .exec()
}

#[test]
fn choices_and_partial_transfer_resume_across_turns_without_replaying_unit_state()
-> mlua::Result<()> {
    let lua = flow_world()?;
    prepare_choices(&lua)?;
    lua.load(
        r#"
        local A,F=TEST_ABSORPTION,TEST_FLOW
        CHOOSE_ALL('later-choice')
        CURRENT_TURN=6
        FAIL_CITY=true
        local failed=F.Resolve(2,'later-choice')
        assert(failed.Phase=='BLOCKED' and failed.AppliedPlan)
        local candidate=assert(A.FindUnit(2,'later-choice:unit:3'))
        candidate.damage=35;candidate.moves=0;candidate.xp=41
        SAVED_CANDIDATE=candidate:GetID();SAVED_ROLLS=#RANDOM_CALLS;SAVED_SPAWNS=SPAWNS
        CURRENT_TURN=7;FAIL_CITY=false
        local beforeCancel=A.Canonical(Game.props)
        assert(not pcall(A.CancelUnprepared,'later-choice',failed.AppliedPlan))
        assert(A.Canonical(Game.props)==beforeCancel,'staged transfer was cancelled')
        Game.props=A.Rules.Copy(Game.props)
    "#,
    )
    .exec()?;
    load_module(
        &lua,
        "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Gameplay.lua",
    )?;
    load_module(&lua, FLOW)?;
    lua.load(
        r#"
        local A,F=TEST_ABSORPTION,TEST_FLOW
        local result=F.Resolve(2,'later-choice')
        assert(result.Phase=='RESULT',tostring(result.Error))
        local unit=assert(A.FindUnit(2,'later-choice:unit:3'))
        assert(unit:GetID()==SAVED_CANDIDATE and unit.damage==35 and unit.moves==0 and unit.xp==41,
          'resume replayed old health, movement or experience')
        assert(#RANDOM_CALLS==SAVED_ROLLS and SPAWNS==SAVED_SPAWNS)
        assert(CITIES[4]:GetOwner()==2 and CITIES[8]:GetOwner()==2)
        assert(F.Resolve(2,'later-choice').Phase=='RESULT' and SPAWNS==SAVED_SPAWNS)
    "#,
    )
    .exec()
}

#[test]
fn ai_loyalty_wait_and_friction_resume_from_real_ui_publications() -> mlua::Result<()> {
    for mode in ["CONQUEST", "SMALL_STATE"] {
        let lua = flow_world()?;
        lua.globals().set("AI_MODE", mode)?;
        lua.load(r#"
            local function event()
              local handlers={}
              return {Add=function(fn) handlers[#handlers+1]=fn end,
                Emit=function(...) for _,fn in ipairs(handlers) do fn(...) end end}
            end
            Events={GameCoreEventPublishComplete=event(),LoadScreenClose=event(),LocalPlayerTurnBegin=event()}
            PlayerManager.GetAlive=function() return {Players[1],Players[2],Players[3]} end
            CORE_PLAYERS=Players;UI_PLAYERS={}
            for owner,player in pairs(Players) do
              UI_PLAYERS[owner]=setmetatable({GetCities=function()
                local function cached(city)
                  return setmetatable({GetCulturalIdentity=function()
                    return {GetLoyalty=function() return city.loyalty end} end},{__index=city})
                end
                return {Members=function()
                  local list={};for _,city in player:GetCities():Members() do list[#list+1]=cached(city) end
                  return ipairs(list)
                end,FindID=function(_,id)
                  local city=player:GetCities():FindID(id);return city and cached(city) end}
              end},{__index=player})
            end
            PlayerOperations={EXECUTE_SCRIPT=1};Game.GetLocalPlayer=function() return 3 end
            Players[2].IsHuman=function() return false end
            Players[3].IsHuman=function() return true end
            HUMAN_TURN_ACTIVE=false;PUBLICATION_OPERATIONS=0
            Players[3].IsTurnActive=function() return HUMAN_TURN_ACTIVE end
            UI={RequestPlayerOperation=function(actor,kind,payload)
              assert(actor==3 and kind==1 and payload.Command=='RECONCILE_PUBLICATIONS')
              assert(HUMAN_TURN_ACTIVE,'publication command sent outside human turn')
              PUBLICATION_OPERATIONS=PUBLICATION_OPERATIONS+1
              GameEvents.JAC_AbsorptionFlow_Operation.Emit(actor,payload)
            end}
            LOYALTY_CALLS=0
            for _,player in pairs(Players) do
              for _,city in player:GetCities():Members() do
                local original=city.GetBuildings
                city.GetBuildings=function(self)
                  local buildings=original(self)
                  buildings.GetGreatWorkInSlot=function() return -1 end
                  return buildings
                end
              end
            end
            for _,city in Players[1]:GetCities():Members() do
              city.GetCulturalIdentity=function() return {} end
              local change=city.ChangeLoyalty
              city.ChangeLoyalty=function(self,delta)
                LOYALTY_CALLS=LOYALTY_CALLS+1;return change(self,delta)
              end
            end
            for i=1,400 do RANDOM_QUEUE[#RANDOM_QUEUE+1]=9999 end
        "#).exec()?;
        lua.load("Players=UI_PLAYERS").exec()?;
        load_module(
            &lua,
            "poc/JAC_PoC_Absorption/UI/JAC_Absorption_HeroKeys.lua",
        )?;
        lua.load("Players=CORE_PLAYERS").exec()?;
        lua.load(
            r#"
            local F=TEST_FLOW
            local flow=F.Begin({ID='ai-wait',From=1,To=2,Mode=AI_MODE,
              EntryConditionsMet=true,RelationsResolved=true})
            assert(flow.Phase=='APPLYING' and flow.WaitingFor=='WAITING_UI')
            local calls=#RANDOM_CALLS
            Events.GameCoreEventPublishComplete.Emit()
            assert(F.Get('ai-wait').Phase=='APPLYING' and PUBLICATION_OPERATIONS==0)
            HUMAN_TURN_ACTIVE=true;Events.LocalPlayerTurnBegin.Emit()
            assert(PUBLICATION_OPERATIONS==1,'turn begin did not resume deferred AI publication')
            for i=1,10 do Events.GameCoreEventPublishComplete.Emit() end
            assert(F.Get('ai-wait').Phase=='DONE',F.Get('ai-wait').Phase..':'
              ..tostring(F.Get('ai-wait').Error))
            assert(LOYALTY_CALLS==2 and #RANDOM_CALLS==calls)
            CURRENT_TURN=6;GameEvents.OnGameTurnStarted.Emit()
            for i=1,10 do Events.GameCoreEventPublishComplete.Emit() end
            assert(LOYALTY_CALLS==4,'AI friction remained waiting for its own human UI')
            for _,x in ipairs({4,8}) do
              local g=CITIES[x]:GetProperty(TEST_ABSORPTION.Rules.GOVERNANCE_KEY)
              assert(g.FrictionApplied==1 and g.LastFrictionTurn==6)
            end
            Events.GameCoreEventPublishComplete.Emit();GameEvents.OnGameTurnStarted.Emit()
            assert(LOYALTY_CALLS==4 and #RANDOM_CALLS==calls)
        "#,
        )
        .exec()?;
    }
    Ok(())
}

#[test]
fn failed_recovered_heroes_keep_claim_and_candidate_and_retry_after_reload() -> mlua::Result<()> {
    for dormant in [false, true] {
        for failure in ["no-unit", "throw-after-unit", "restore"] {
            let lua = flow_world()?;
            lua.globals().set("DORMANT", dormant)?;
            lua.globals().set("FAILURE", failure)?;
            lua.load(
                r#"
                local A,R=TEST_ABSORPTION,TEST_ABSORPTION.Rules
                local hero=UNIT(1,13,'UNIT_HERO_JAC_WARLORD_A',4)
                hero.lifespan=17;hero.charges=2;hero.renown=13
                HERO_CLAIMS[0]=1;HERO_ORIGINS[0]={player=1,id=4}
                local snap=A.SnapshotUnit(hero)
                UnitManager.Kill(hero)
                local state=A.GetState()
                state.Archive.recovered={ID='recovered',Kind='HERO',Owner=-1,Loyalty=71,
                  Equipment={'SPEAR'},NativeHero={Class=0,ClassType='HEROCLASS_JAC_WARLORD_A',
                    Origin={Owner=1,ID=4,X=4,Y=0}}}
                Game:SetProperty(R.STATE_KEY,state)
                CityManager.TransferCity(CITIES[4],2)
                SNAPSHOT=DORMANT and nil or snap
                HERO_NO_SPAWN=FAILURE=='no-unit';HERO_THROW_AFTER_SPAWN=FAILURE=='throw-after-unit'
                FAIL_COPY=FAILURE=='restore' and not DORMANT
                HERO_WRONG_OWNER=FAILURE=='restore' and DORMANT
                local intent=A.RecoverArchived('recovered',2,'recovery:retry',SNAPSHOT)
                if FAILURE=='restore' and DORMANT then
                  assert(intent.Status=='WAITING_UI')
                  ExposedMembers.JAC_HeroReadAPI={GetObservation=function()
                    return {Claim=3,Origin={player=2,id=CITIES[4].id}} end}
                  intent=A.RecoverArchived('recovered',2,'recovery:retry',SNAPSHOT)
                  ExposedMembers.JAC_HeroReadAPI=nil
                end
                assert(intent.Status=='BLOCKED' and intent.Cause,tostring(intent.Status))
                assert(A.GetState().Archive.recovered.Loyalty==71 and A.GetRoster(2).recovered==nil)
                assert(intent.NativeClaimAfter==HERO_CLAIMS[0]
                  and intent.HeroClass==0,'failed native claim was not journaled')
                SAVED_ID=intent.CandidateID;SAVED_CREATIONS=HERO_CREATIONS
                if SAVED_ID then
                  local candidate=assert(UnitManager.GetUnit(2,SAVED_ID))
                  assert(candidate:GetProperty(R.TOKEN_KEY)=='recovery:retry')
                end
                HERO_NO_SPAWN=false;HERO_THROW_AFTER_SPAWN=false;FAIL_COPY=false
                HERO_WRONG_OWNER=false;HERO_CLAIMS[0]=2
                CURRENT_TURN=6;Game.props=R.Copy(Game.props)
            "#,
            )
            .exec()?;
            load_module(
                &lua,
                "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Gameplay.lua",
            )?;
            lua.load(r#"
                local A=TEST_ABSORPTION
                local waiting=A.RecoverArchived('recovered',2,'recovery:retry',SNAPSHOT)
                assert(waiting.Status=='WAITING_UI',waiting.Status..':'..tostring(waiting.Cause))
                assert(HERO_CREATIONS==SAVED_CREATIONS+(SAVED_ID and 0 or 1),
                  FAILURE..':'..tostring(DORMANT)..':'..HERO_CREATIONS..':'..SAVED_CREATIONS)
                assert(not SAVED_ID or waiting.CandidateID==SAVED_ID)
                if not DORMANT then
                  CURRENT_TURN=7
                  local candidate=assert(UnitManager.GetUnit(2,waiting.CandidateID))
                  candidate.damage=9;candidate.moves=1;candidate.lifespan=16
                end
                ExposedMembers.JAC_HeroReadAPI={GetObservation=function(token)
                  if token==waiting.ObservationToken then return {Claim=2,Origin={player=2,id=CITIES[4].id}} end
                end}
                local done=A.RecoverArchived('recovered',2,'recovery:retry',SNAPSHOT)
                assert(done.Status=='COMPLETE' and done.Record.Equipment[1]=='SPEAR',
                  FAILURE..':'..tostring(DORMANT)..':'..A.Canonical(done))
                if not DORMANT then
                  local candidate=UnitManager.GetUnit(2,waiting.CandidateID)
                  assert(candidate.damage==9 and candidate.moves==1 and candidate.lifespan==16)
                end
                assert(A.GetState().Archive.recovered.Owner==-1)
                local count=HERO_CREATIONS
                assert(A.RecoverArchived('recovered',2,'recovery:retry',SNAPSHOT).Status=='COMPLETE')
                assert(HERO_CREATIONS==count)
            "#).exec()?;
        }
    }
    Ok(())
}

#[test]
fn single_gameplay_entry_includes_private_modules_and_tuner_uses_sp_only_event() -> mlua::Result<()>
{
    let manifest = fs::read_to_string(repo_path(
        "poc/JAC_PoC_Absorption/JAC_PoC_Absorption.modinfo",
    ))
    .map_err(mlua::Error::external)?;
    assert_eq!(manifest.matches("<AddGameplayScripts ").count(), 1);
    assert!(
        manifest.contains("<File>Scripts/JAC_Absorption_Entry.lua</File></AddGameplayScripts>")
    );
    let lua = Lua::new();
    lua.load(WORLD).call::<()>(outcomes(&lua)?)?;
    lua.load(
        r#"
        function COUNTED_EVENT()
          local handlers={}
          return {Add=function(fn) handlers[#handlers+1]=fn end,
            Call=function(...) for _,fn in ipairs(handlers) do fn(...) end end,
            Count=function() return #handlers end}
        end
        GameEvents.JAC_Absorption_TestCall=COUNTED_EVENT()
        GameEvents.JAC_AbsorptionFlow_Operation=COUNTED_EVENT()
        GameEvents.OnGameTurnStarted=COUNTED_EVENT()
    "#,
    )
    .exec()?;
    let core = lua.create_table()?;
    let meta = lua.create_table()?;
    meta.set("__index", lua.globals())?;
    core.set_metatable(Some(meta))?;
    let include_env = core.clone();
    let included = lua.create_table()?;
    core.set(
        "include",
        lua.create_function(move |lua, name: String| {
            if included.get::<bool>(name.as_str())? {
                return Ok(());
            }
            let path = format!("poc/JAC_PoC_Absorption/Scripts/{name}.lua");
            let source = fs::read_to_string(repo_path(&path)).map_err(mlua::Error::external)?;
            lua.load(source)
                .set_name(&path)
                .set_environment(include_env.clone())
                .exec()?;
            included.set(name, true)
        })?,
    )?;
    let entry = fs::read_to_string(repo_path(
        "poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Entry.lua",
    ))
    .map_err(mlua::Error::external)?;
    for _ in 0..2 {
        lua.load(&entry).set_environment(core.clone()).exec()?;
    }
    // This separate context deliberately inherits no private gameplay globals.
    lua.load(r#"
        assert(JAC_AbsorptionGameplay==nil and JAC_AbsorptionFlowGameplay==nil)
        assert(ExposedMembers.JAC_Absorption.Apply==nil and ExposedMembers.JAC_AbsorptionFlow.Begin==nil)
        assert(GameEvents.JAC_Absorption_TestCall.Count()==1)
        assert(GameEvents.JAC_AbsorptionFlow_Operation.Count()==1)
        assert(GameEvents.OnGameTurnStarted.Count()==2)
        local event=GameEvents.JAC_Absorption_TestCall
        GameConfiguration.IsAnyMultiplayer=function() return true end
        event.Call('mp','ENGINE','Apply',{PLAN})
        assert(Game:GetProperty('JAC_ABSORPTION_TEST_RESULT_V1')==nil and SPAWNS==0)
        GameConfiguration.IsAnyMultiplayer=function() return false end
        local config=GameConfiguration.GetValue
        GameConfiguration.GetValue=function() return nil end
        event.Call('ordinary','ENGINE','Apply',{PLAN})
        assert(Game:GetProperty('JAC_ABSORPTION_TEST_RESULT_V1')==nil and SPAWNS==0)
        GameConfiguration.GetValue=config
        event.Call('bad','ENGINE','GetHeroesManager',{})
        local bad=Game:GetProperty('JAC_ABSORPTION_TEST_RESULT_V1')
        assert(bad.ID=='bad' and bad.Accepted==false and bad.Error and SPAWNS==0)
        event.Call('apply','ENGINE','Apply',{PLAN})
        local reply=Game:GetProperty('JAC_ABSORPTION_TEST_RESULT_V1')
        assert(reply.ID=='apply' and reply.Accepted and reply.Result.Status=='COMPLETE',reply.Error)
        assert(ExposedMembers.JAC_Absorption.GetState().Transactions[PLAN.ID].Status=='COMPLETE')
        local count=SPAWNS
        event.Call('retry','ENGINE','Apply',{PLAN})
        assert(Game:GetProperty('JAC_ABSORPTION_TEST_RESULT_V1').Accepted and SPAWNS==count)
        GameEvents.JAC_AbsorptionFlow_Operation.Call(2,{Command='TEST_CALL',
          Domain='ENGINE',Method='Apply',Args={PLAN}})
        assert(not Players[2]:GetProperty('JAC_ABSORPTION_FLOW_LAST_OPERATION_V1').Accepted)
        assert(Game:GetProperty('JAC_ABSORPTION_TEST_RESULT_V1').ID=='retry')
    "#).exec()
}

#[test]
fn staged_native_hero_is_not_restored_again_after_later_city_failure() -> mlua::Result<()> {
    let lua = flow_world()?;
    prepare_choices(&lua)?;
    lua.load(
        r#"
        local A,F=TEST_ABSORPTION,TEST_FLOW
        local hero=UNIT(1,13,'UNIT_HERO_JAC_WARLORD_A',4)
        hero.lifespan=17;hero.charges=2;hero.damage=7;hero.moves=1
        HERO_CLAIMS[0]=1;HERO_ORIGINS[0]={player=1,id=4}
        local transfer=CityManager.TransferCity
        CityManager.TransferCity=function(city,owner)
          if city.x==8 then error('second city failure') end
          return transfer(city,owner)
        end
        CHOOSE_ALL('staged-native')
        local flow=F.Resolve(2,'staged-native')
        assert(flow.Phase=='BLOCKED' and CITIES[4]:GetOwner()==2 and CITIES[8]:GetOwner()==1)
        local unit=assert(A.FindUnit(2,'staged-native:unit:13'))
        unit.damage=9;unit.moves=0;unit.lifespan=16
        local id=unit:GetID();local calls=#RANDOM_CALLS
        CURRENT_TURN=6;CityManager.TransferCity=transfer
        flow=F.Resolve(2,'staged-native')
        assert(flow.Phase=='RESULT',tostring(flow.Error))
        unit=assert(A.FindUnit(2,'staged-native:unit:13'))
        assert(unit:GetID()==id and unit.damage==9 and unit.moves==0 and unit.lifespan==16)
        assert(HERO_CREATIONS==1 and #RANDOM_CALLS==calls)
    "#,
    )
    .exec()
}

#[test]
fn changed_inventory_on_a_later_turn_blocks_before_final_rolls_and_transfers() -> mlua::Result<()> {
    let lua = flow_world()?;
    prepare_choices(&lua)?;
    lua.load(r#"
        local A,F=TEST_ABSORPTION,TEST_FLOW
        CHOOSE_ALL('changed-inventory')
        local calls=#RANDOM_CALLS
        CURRENT_TURN=6
        UNIT(1,20,'UNIT_WARRIOR',4)
        local flow=F.Resolve(2,'changed-inventory')
        assert(flow.Phase=='BLOCKED' and flow.Error=='absorption inventory changed after first rolls')
        assert(flow.AppliedPlan==nil and A.GetState().Transactions[flow.ID]==nil)
        assert(#RANDOM_CALLS==calls and SPAWNS==0 and KILLS==0 and TRANSFERS==0)
        assert(CITIES[4]:GetOwner()==1 and CITIES[8]:GetOwner()==1)
    "#).exec()
}
