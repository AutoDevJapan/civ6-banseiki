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
        local first=GameInfo.GovernorPromotions[ROWS[1].BasePromotionType]
        local second=GameInfo.GovernorPromotions[ROWS[2].BasePromotionType]
        second.Index=first.Index
        GameInfo.GovernorPromotions=collection(PROMOS,'GovernorPromotionType')
        assert(loadstring(GAMEPLAY_SOURCE))()
        assert(Players[1]:GetProperty(904999)==nil)
        assert(Game:GetProperty('JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA')==nil)"#);
}

#[test]
fn unreadable_dead_major_namespace_aborts_claim_and_closes_all_owners() {
    run(r#"Game:SetProperty('JAC_TEST853_PAID_ASSIGNMENT_V1',true)
        Players[2].alive=false
        PlayerManager.GetAliveMajorIDs=function() return {0,1} end
        local resetReads={}
        for owner=0,2 do
            local p=Players[owner]; local getter=p.GetProperty
            p.GetProperty=function(self,key)
                if key==907000+owner then resetReads[owner]=(resetReads[owner] or 0)+1 end
                if owner==2 and key==906500 then error('saved namespace unreadable') end
                return getter(self,key)
            end
        end
        assert(loadstring(GAMEPLAY_SOURCE))()
        assert(Game:GetProperty('JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA')==nil)
        for owner=0,2 do
            assert(resetReads[owner]>=2,'failure failed to reclose every owner')
            assert(Players[owner]:GetProperty(904999)==nil,'catalog committed after unreadable key')
        end"#);
}

#[test]
fn throwing_paid_publication_aborts_and_recloses_even_earlier_owners() {
    for operation in ["get", "set"] {
        run(&format!(
            r#"Game:SetProperty('JAC_TEST853_PAID_ASSIGNMENT_V1',true)
            local resetReads,originals,failureCounts={{}},{{}},nil
            local function fail()
                failureCounts={{}}
                for owner=0,2 do failureCounts[owner]=resetReads[owner] or 0 end
                error('paid publication failed')
            end
            for owner=0,2 do
                local p=Players[owner]; local getter,setter=p.GetProperty,p.SetProperty
                originals[owner]={{get=getter,set=setter,spent=p.spent}}
                p.GetProperty=function(self,key)
                    if key==907000+owner then resetReads[owner]=(resetReads[owner] or 0)+1 end
                    if owner==1 and key==904999 and '{operation}'=='get' and getter(self,key)==false then
                        fail()
                    end
                    return getter(self,key)
                end
                p.SetProperty=function(self,key,value)
                    if owner==1 and key==904999 and '{operation}'=='set' then fail() end
                    return setter(self,key,value)
                end
            end
            assert(loadstring(GAMEPLAY_SOURCE))()
            assert(failureCounts,'publication failure not injected')
            for owner=0,2 do
                assert(resetReads[owner]>failureCounts[owner],'partial publication did not reclose all owners after failure')
            end
            assert(Players[2]:GetProperty(904999)==nil,'later owner published after failure')
            for owner=0,2 do
                Players[owner].GetProperty=originals[owner].get
                Players[owner].SetProperty=originals[owner].set
            end
            ExposedMembers.JAC_LeaderPersona.Reconcile()
            for owner=0,2 do
                assert(Players[owner]:GetProperty(904999)==true,'ordinary callback failed to rebuild catalog')
                assert(Players[owner].spent==originals[owner].spent,'projection changed native title spending')
                for row in GameInfo.JAC_LeaderPersonas() do
                    local index=GameInfo.Governors[row.GovernorType].Index
                    assert(Players[owner]:GetProperty(905000+index)==false,'unjoined person enabled after retry')
                end
            end
        "#
        ));
    }
}

#[test]
fn throwing_initial_paid_reset_cannot_publish_a_catalog() {
    run(r#"Game:SetProperty('JAC_TEST853_PAID_ASSIGNMENT_V1',true)
        local resetReads={}
        for owner=0,2 do
            local p=Players[owner]; local getter=p.GetProperty
            p.GetProperty=function(self,key)
                if key==907000+owner then
                    resetReads[owner]=(resetReads[owner] or 0)+1
                    if owner==1 then error('reset unavailable') end
                end
                return getter(self,key)
            end
        end
        assert(loadstring(GAMEPLAY_SOURCE))()
        assert(Game:GetProperty('JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA')==nil)
        for owner=0,2 do
            assert(resetReads[owner]>=1,'reset failure skipped later owner')
            assert(Players[owner]:GetProperty(904999)==nil,'failed reset published catalog')
        end"#);
}

#[test]
fn disabling_paid_projection_requires_successful_closure_before_appointment_publish() {
    run(r#"Game:SetProperty('JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA',1)
        Game:SetProperty('JAC_TEST853_PAID_ASSIGNMENT_V1',false)
        local appointmentWrites=0
        for owner=0,2 do
            local p=Players[owner]; local getter,setter=p.GetProperty,p.SetProperty
            p.GetProperty=function(self,key)
                if owner==1 and key==907001 then error('paid reset unavailable') end
                return getter(self,key)
            end
            p.SetProperty=function(self,key,value)
                if type(key)=='number' and key>=900000 and key<=903255 then
                    appointmentWrites=appointmentWrites+1
                end
                return setter(self,key,value)
            end
        end
        assert(loadstring(GAMEPLAY_SOURCE))()
        assert(appointmentWrites==0,'appointment admission reopened while paid closure failed')"#);
}

#[test]
fn reset_method_lookup_failure_does_not_skip_other_namespace_or_later_owner() {
    run(r#"Game:SetProperty('JAC_TEST853_PAID_ASSIGNMENT_V1',true)
        local resets={}
        for owner=0,2 do
            local p=Players[owner]; local getter=p.GetProperty
            p.GetProperty=function(self,key)
                if key==907000+owner or key==902000+owner then
                    resets[key]=(resets[key] or 0)+1
                end
                return getter(self,key)
            end
        end
        Players[1].GetProperty=nil
        local attempts=0
        setmetatable(Players[1],{__index=function(_,key)
            if key=='GetProperty' then attempts=attempts+1; error('reset method unavailable') end
        end})
        assert(loadstring(GAMEPLAY_SOURCE))()
        assert(attempts>=2,'failed paid reset skipped appointment reset for same owner')
        for _,owner in ipairs({0,2}) do
            assert(resets[907000+owner] and resets[902000+owner],'method failure skipped later reset')
            assert(Players[owner]:GetProperty(904999)==nil,'failed reset still published catalog')
        end"#);
}

#[test]
fn synchronized_projection_is_opt_in_and_cannot_finalize_or_spend_titles() {
    run(r#"assert(loadstring(GAMEPLAY_SOURCE))()
        FIRE('JAC_LeaderPersonaProject',0,{ProjectionSerial=1})
        assert(Players[0]:GetProperty('JAC_PERSONA_PROJECT_RECEIPT')==nil)
        Game:SetProperty('JAC_TEST853_PAID_ASSIGNMENT_V1',true)
        local before={}
        for row in GameInfo.JAC_LeaderPersonas() do
            before[row.LeaderType]=Game:GetProperty('JAC_PERSONA_'..row.LeaderType..'_STATE')
        end
        local totalSpent=Players[0].spent+Players[1].spent+Players[2].spent
        local beforeDestroys,beforeSpawns=DESTROYS,SPAWNS
        for _,serial in ipairs({false,0,0.5,'2'}) do
            FIRE('JAC_LeaderPersonaProject',0,{ProjectionSerial=serial})
            assert(Players[0]:GetProperty('JAC_PERSONA_PROJECT_RECEIPT')==nil)
        end
        FIRE('JAC_LeaderPersonaProject',0,{ProjectionSerial=2})
        assert(Players[0]:GetProperty('JAC_PERSONA_PROJECT_RECEIPT')==2)
        for row in GameInfo.JAC_LeaderPersonas() do
            assert(Game:GetProperty('JAC_PERSONA_'..row.LeaderType..'_STATE')==before[row.LeaderType])
        end
        assert(Players[0].spent+Players[1].spent+Players[2].spent==totalSpent)
        assert(DESTROYS==beforeDestroys and SPAWNS==beforeSpawns)
        assert(Players[0]:GetProperty(904999)==true)
    "#);
}

#[test]
fn nonthrowing_appointment_rejection_revokes_both_native_permissions() {
    for failure in ["schema", "collision", "readback"] {
        run(&format!(
            r#"
            Game:SetProperty('JAC_TEST853_PAID_ASSIGNMENT_V1',true)
            local appointment,paid={{}},{{}}
            local injected=false
            for owner=0,2 do
                appointment[owner]={{}}; paid[owner]={{}}
                local p=Players[owner]; local getter=p.GetProperty
                p.GetProperty=function(self,key)
                    if key==902000+owner then appointment[owner]={{}} end
                    if key==907000+owner then paid[owner]={{}} end
                    local value=getter(self,key)
                    if injected and '{failure}'=='readback' and owner==1 and key==900021 then return false end
                    if type(key)=='number' and key>=901000 and key<901256 then appointment[owner][key-901000]=value==true end
                    if type(key)=='number' and key>=905000 and key<905256 then paid[owner][key-905000]=value==true end
                    return value
                end
            end
            assert(loadstring(GAMEPLAY_SOURCE))()
            local api=ExposedMembers.JAC_LeaderPersona
            assert(api.Join(0,2)=='JOINED'); Players[0].points=1
            assert(api.PrepareEvolution(0,ASHINA)=='EVOLVING')
            UI.RequestPlayerOperation(0,PlayerOperations.APPOINT_GOVERNOR,{{[PlayerOperations.PARAM_GOVERNOR_TYPE]=4}})
            DRAIN(); assert(api.CompleteEvolution(0,ASHINA)=='EVOLVED')
            assert(api.Join(1,0)=='JOINED'); Players[1].points=3
            assert(api.PrepareEvolution(1,'LEADER_JAC_ODA_NOBUHIDE')=='EVOLVING')
            api.Reconcile()
            assert(appointment[0][4] and paid[0][4] and appointment[1][21],'precondition must prove both permissions open')
            local spent=Players[0].spent
            if '{failure}'=='schema' then Game:SetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1',99) end
            if '{failure}'=='collision' then
                Game:SetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1',nil)
                Players[1]:SetProperty(902001,'foreign')
            end
            injected=true; api.Reconcile()
            for owner=0,2 do
                for _,values in ipairs({{appointment[owner],paid[owner]}}) do
                    for _,allowed in pairs(values) do assert(not allowed,'nonthrowing rejection reopened or stranded native permission: {failure}') end
                end
            end
            assert(Players[0].spent==spent,'rejection changed official payment')
            injected=false
            Game:SetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1',3)
            Players[1]:SetProperty(902001,true)
            api.Reconcile()
            assert(appointment[0][4] and paid[0][4] and appointment[1][21],'fresh successful projection did not recover both permissions')
            assert(Players[0].spent==spent,'recovery replayed payment')
        "#
        ));
    }
}
