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
