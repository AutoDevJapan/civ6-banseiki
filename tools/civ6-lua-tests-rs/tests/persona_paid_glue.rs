use civ6_lua_tests_rs::{new_lua, read_repo};

fn run(case: &str) {
    let lua = new_lua();
    for path in [
        "tools/civ6-lua-tests-rs/fixtures/persona/stubs.lua",
        "tools/civ6-lua-tests-rs/fixtures/persona/admission_engine.lua",
        "tools/civ6-lua-tests-rs/fixtures/persona/production_admission_engine.lua",
        "poc/JAC_PoC_LeaderPersona/Shared/JAC_PersonaTreeRules.lua",
    ] {
        lua.load(read_repo(path)).set_name(path).exec().unwrap();
    }
    // Additional fixture definitions intentionally have no native Index.
    // Assign independent sparse promotion identities for this GameInfo run.
    lua.load("for i,p in ipairs(PROMOS) do p.Index=10+i*3 end; GameInfo.GovernorPromotions=collection(PROMOS,'GovernorPromotionType')").exec().unwrap();
    lua.globals()
        .set(
            "GAMEPLAY_SOURCE",
            read_repo("poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua"),
        )
        .unwrap();
    lua.load(case).exec().unwrap();
}

#[test]
fn default_publishes_no_paid_namespace() {
    run(r#"assert(loadstring(GAMEPLAY_SOURCE))()
        for owner=0,2 do for key=904000,907063 do assert(Players[owner]:GetProperty(key)==nil) end end
        assert(Game:GetProperty('JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA')==nil)"#);
}

#[test]
fn all_twenty_two_close_before_paid_requests_and_reload_discards_saved_true() {
    run(r#"Game:SetProperty('JAC_TEST853_PAID_ASSIGNMENT_V1',true)
        assert(loadstring(GAMEPLAY_SOURCE))()
        assert(Game:GetProperty('JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA')==1)
        for owner=0,2 do
            assert(Players[owner]:GetProperty(907000+owner)==true)
            assert(Players[owner]:GetProperty(904999)==true)
            local count=0
            for row in GameInfo.JAC_LeaderPersonas() do
                local index=GameInfo.Governors[row.GovernorType].Index
                assert(Players[owner]:GetProperty(904000+index)==true)
                assert(Players[owner]:GetProperty(905000+index)==false)
                count=count+1
            end
            assert(count==22)
        end
        Players[1]:SetProperty(905004,true)
        assert(loadstring(GAMEPLAY_SOURCE))()
        assert(Players[1]:GetProperty(905004)==false)
        local before=Players[1].spent
        Players[1].spent=999
        ExposedMembers.JAC_LeaderPersona.Reconcile()
        assert(Players[1]:GetProperty(905004)==false)
        Players[1].spent=before"#);
}

#[test]
fn foreign_reserved_key_is_not_overwritten() {
    run(r#"Game:SetProperty('JAC_TEST853_PAID_ASSIGNMENT_V1',true)
        Players[1]:SetProperty(906500,'foreign')
        assert(loadstring(GAMEPLAY_SOURCE))()
        assert(Players[1]:GetProperty(906500)=='foreign')
        assert(Game:GetProperty('JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA')==nil)
        assert(Players[1]:GetProperty(904999)==nil)"#);
}

#[test]
fn duplicate_gameinfo_promotion_never_commits_catalog() {
    run(r#"Game:SetProperty('JAC_TEST853_PAID_ASSIGNMENT_V1',true)
        PROMOS[2].Index=PROMOS[1].Index
        GameInfo.GovernorPromotions=collection(PROMOS,'GovernorPromotionType')
        assert(loadstring(GAMEPLAY_SOURCE))()
        assert(Players[1]:GetProperty(904999)==nil)
        assert(Game:GetProperty('JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA')==nil)"#);
}
