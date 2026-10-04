//! Mock contract for the opt-in synchronized scalar fixture; no game claim.
use civ6_lua_tests_rs::{exec, new_lua, read_repo};

const FIXTURE: &str = "poc/JAC_Test_PropertyObserver/Scripts/JAC_PropertyObserver_Fixture.lua";

#[test]
fn opt_in_mod_manifest_declares_the_persistent_gameplay_script() {
    let xml = read_repo("poc/JAC_Test_PropertyObserver/JAC_Test_PropertyObserver.modinfo");
    let doc = roxmltree::Document::parse(&xml).unwrap();
    assert_eq!(
        doc.root_element().attribute("id"),
        Some("d8532710-f853-4c53-8853-271000000001")
    );
    assert!(doc
        .descendants()
        .any(|n| n.has_tag_name("AddGameplayScripts")
            && n.descendants().any(|child| child.has_tag_name("File")
                && child.text() == Some("Scripts/JAC_PropertyObserver_Fixture.lua"))));
    assert!(!doc
        .descendants()
        .any(|n| n.has_tag_name("AddUserInterfaces")));
}

#[test]
fn exact_game_and_player_values_and_removal_are_read_back() {
    let lua = new_lua();
    exec(
        &lua,
        r#"
        local function property_object(human)
            local values = {}
            return {
                IsHuman = function() return human end,
                GetProperty = function(_, key)
                    if values[key] == nil then return end;
                    return values[key];
                end,
                SetProperty = function(_, key, value) values[key] = value end,
            }
        end
        Game = property_object(false)
        Players = {[0] = property_object(true), [1] = property_object(false)}
        GameEvents = {
            JAC_PropertyObserverStep = {Add = function(fn) OBSERVER_STEP = fn end},
            JAC_PropertyObserverNumericStep = {Add = function(fn) NUMERIC_STEP = fn end},
            JAC_JoinedAdmissionInitialize = {Add = function(fn) ADMISSION_INIT = fn end},
            JAC_JoinedAdmissionPrepare = {Add = function(fn) ADMISSION_PREPARE = fn end},
        }
        print = function() end
    "#,
    );
    exec(&lua, &read_repo(FIXTURE));
    exec(
        &lua,
        r#"
        local key = 'JAC_TEST853_VALUE'
        assert(Game:GetProperty(key) == nil and Players[0]:GetProperty(key) == nil)
        local expected = {[1]=true, [2]=false, [3]=853271, [4]=-853271}
        for phase=1,5 do
            OBSERVER_STEP(0, {Phase=phase})
            assert(Game:GetProperty('JAC_TEST853_PHASE') == phase)
            assert(Game:GetProperty(key) == expected[phase])
            assert(Players[0]:GetProperty(key) == expected[phase])
        end
        assert(Players[1]:GetProperty(key) == nil)
        assert(Game:GetProperty(853271) == nil and Players[0]:GetProperty(853271) == nil)
        for phase=1,5 do
            NUMERIC_STEP(0, {Phase=phase})
            assert(Game:GetProperty(853272) == phase)
            assert(Game:GetProperty(853271) == expected[phase])
            assert(Players[0]:GetProperty(853271) == expected[phase])
        end
        assert(Players[1]:GetProperty(853271) == nil)
    "#,
    );
}

#[test]
fn rejects_wrong_owner_replay_skips_malformed_and_dirty_start() {
    let lua = new_lua();
    exec(
        &lua,
        r#"
        local values = {}
        Game = {GetProperty=function(_, k) return values[k] end,
            SetProperty=function(_, k, v) values[k]=v end}
        Players = {[0]={IsHuman=function() return true end,
            GetProperty=function(_, k) return values['player:'..k] end,
            SetProperty=function(_, k, v) values['player:'..k]=v end},
            [1]={IsHuman=function() return false end}}
        GameEvents = {
            JAC_PropertyObserverStep={Add=function(fn) OBSERVER_STEP=fn end},
            JAC_PropertyObserverNumericStep={Add=function(fn) NUMERIC_STEP=fn end},
            JAC_JoinedAdmissionInitialize={Add=function(fn) ADMISSION_INIT=fn end},
            JAC_JoinedAdmissionPrepare={Add=function(fn) ADMISSION_PREPARE=fn end},
        }
        print=function() end
    "#,
    );
    exec(&lua, &read_repo(FIXTURE));
    exec(
        &lua,
        r#"
        for _,case in ipairs({
            {1, {Phase=1}}, {0, nil}, {0, {Phase='1'}},
            {0, {Phase=2}}, {0, {Phase=1.5}}, {0, {Phase=6}},
        }) do
            assert(not pcall(OBSERVER_STEP, case[1], case[2]))
        end
        assert(Game:GetProperty('JAC_TEST853_PHASE') == nil)
        assert(Game:GetProperty('JAC_TEST853_VALUE') == nil)
        Game:SetProperty('JAC_TEST853_VALUE', 99)
        assert(not pcall(OBSERVER_STEP, 0, {Phase=1}))
        assert(Game:GetProperty('JAC_TEST853_VALUE') == 99)
        for _,case in ipairs({
            {1, {Phase=1}}, {0, nil}, {0, {Phase='1'}},
            {0, {Phase=2}}, {0, {Phase=1.5}}, {0, {Phase=6}},
        }) do
            assert(not pcall(NUMERIC_STEP, case[1], case[2]))
        end
        assert(Game:GetProperty(853272) == nil and Game:GetProperty(853271) == nil)
        Game:SetProperty(853271, 99)
        assert(not pcall(NUMERIC_STEP, 0, {Phase=1}))
        assert(Game:GetProperty(853271) == 99)
    "#,
    );
}

#[test]
fn joined_admission_fixture_publishes_only_after_real_join_and_full_title() {
    let lua = new_lua();
    exec(
        &lua,
        r#"
        local game_values = {}
        Game = {GetProperty=function(_, k) return game_values[k] end,
            SetProperty=function(_, k, v) game_values[k]=v end,
            GetCurrentGameTurn=function() return 1 end}
        Players = {}
        PlayerConfigurations = {}
        local ids = {}
        for id=0,21 do
            ids[#ids+1] = id
            local props = {}
            local governors = {total=0, spent=0}
            function governors:GetGovernorPoints() return self.total end
            function governors:GetGovernorPointsSpent() return self.spent end
            local units = {list={}}
            function units:Members() return ipairs(self.list) end
            local culture = {done=false}
            function culture:HasCivic(_) return self.done end
            function culture:GetCultureCost(_) return 1 end
            function culture:SetCulturalProgress(_, _)
                self.done = true
                governors.total = governors.total + 1
            end
            Players[id] = {props=props, units=units, governors=governors, culture=culture,
                IsHuman=function() return id==0 end,
                GetProperty=function(_, k) return props[k] end,
                SetProperty=function(_, k, v) props[k]=v end,
                GetUnits=function() return units end,
                GetGovernors=function() return governors end,
                GetCulture=function() return culture end}
            PlayerConfigurations[id] = {
                GetLeaderTypeName=function()
                    if id==2 then return 'LEADER_JAC_ASHINA_MORIKIYO' end
                    return 'LEADER_JAC_OTHER_'..id
                end}
        end
        PlayerManager = {GetAliveMajorIDs=function() return ids end}
        local personas = {}
        local governors = {}
        for i=0,21 do
            local leader = i==4 and 'LEADER_JAC_ASHINA_MORIKIYO' or 'LEADER_JAC_PERSON_'..i
            local kind = 'GOVERNOR_JAC_PERSONA_'..i
            local row = {LeaderType=leader, GovernorType=kind,
                EvolutionCost=1, PersonType='PERSON_ASHINA'}
            personas[#personas+1] = row
            personas[leader] = row
            governors[kind] = {Index=i}
        end
        setmetatable(personas, {__call=function(t)
            local i=0
            return function() i=i+1; return t[i] end
        end})
        local title_civics = {{CivicType='CIVIC_TEST'}}
        setmetatable(title_civics, {__call=function(t)
            local i=0
            return function() i=i+1; return t[i] end
        end})
        GameInfo = {JAC_LeaderPersonas=personas, Governors=governors,
            GreatPersonIndividuals={PERSON_ASHINA={GreatPersonClassType='CLASS',
                GreatPersonIndividualType='PERSON_ASHINA'},
                [4]={GreatPersonIndividualType='PERSON_ASHINA'}},
            GreatPersonClasses={CLASS={UnitType='UNIT_PERSON'}},
            Units={UNIT_PERSON={Index=9}},
            JAC_PersonaTitleCivics=title_civics,
            Civics={CIVIC_TEST={Index=1}}}
        ExposedMembers = {JAC_LeaderPersona={
            Join=function(owner, source)
                assert(owner==0 and source==2)
                Game:SetProperty('JAC_PERSONA_LEADER_JAC_ASHINA_MORIKIYO_OWNER', owner)
                Game:SetProperty('JAC_PERSONA_LEADER_JAC_ASHINA_MORIKIYO_STATE', 'GREAT_PERSON')
                Game:SetProperty('JAC_PERSONA_LEADER_JAC_ASHINA_MORIKIYO_EVOLUTION_COST', 1)
                local unit = {GetType=function() return 9 end,
                    GetGreatPerson=function() return {GetIndividual=function() return 4 end} end}
                Players[owner].units.list[1] = unit
                return 'JOINED'
            end,
            PrepareEvolution=function(owner, leader)
                assert(owner==0 and leader=='LEADER_JAC_ASHINA_MORIKIYO')
                local g = Players[owner].governors
                assert(g.total-g.spent>=1)
                Game:SetProperty('JAC_PERSONA_'..leader..'_STATE', 'EVOLVING')
                return 'EVOLVING'
            end}}
        GameEvents = {
            JAC_PropertyObserverStep={Add=function() end},
            JAC_PropertyObserverNumericStep={Add=function() end},
            JAC_JoinedAdmissionInitialize={Add=function(fn) ADMISSION_INIT=fn end},
            JAC_JoinedAdmissionPrepare={Add=function(fn) ADMISSION_PREPARE=fn end}}
        print=function() end
    "#,
    );
    exec(&lua, &read_repo(FIXTURE));
    exec(
        &lua,
        r#"
        assert(not pcall(ADMISSION_INIT, 1))
        ADMISSION_INIT(0)
        local receipt = Game:GetProperty('JAC_TEST853_ADMISSION_RECEIPT')
        assert(receipt.phase=='CLOSED' and receipt.pairs==484 and receipt.key==853304)
        for id=0,21 do
            for row in GameInfo.JAC_LeaderPersonas() do
                assert(Players[id]:GetProperty(853300+GameInfo.Governors[row.GovernorType].Index)==false)
            end
        end
        assert(not pcall(ADMISSION_INIT, 0))
        ADMISSION_PREPARE(0)
        receipt = Game:GetProperty('JAC_TEST853_ADMISSION_RECEIPT')
        assert(receipt.phase=='EVOLVING' and receipt.personCount==1)
        assert(receipt.total==1 and receipt.spent==0 and receipt.source==2)
        assert(Players[0]:GetProperty(853304)==true)
        assert(Players[1]:GetProperty(853304)==false)
        assert(Game:GetProperty('JAC_PERSONA_LEADER_JAC_ASHINA_MORIKIYO_STATE')=='EVOLVING')
        assert(not pcall(ADMISSION_PREPARE, 0))
    "#,
    );
}
