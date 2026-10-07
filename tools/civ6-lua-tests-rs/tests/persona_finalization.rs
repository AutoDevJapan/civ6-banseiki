//! Payment completion through shipped Gameplay and UI in Lua 5.1.
//! The engine fixture executes queued APPOINT/PROMOTE; no synthetic paid governor.
use civ6_lua_tests_rs::read_repo;
use mlua::Lua;

const GAMEPLAY: &str = "poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua";

fn scene(cost: u8) -> Lua {
    // Trusted repository fixtures only. Debug upvalues let faults target the
    // real Gameplay reader dependency, without replacing any shipped logic.
    let lua = unsafe { Lua::unsafe_new() };
    lua.globals().set("COST", cost).unwrap();
    for path in [
        "tools/civ6-lua-tests-rs/fixtures/persona/stubs.lua",
        "tools/civ6-lua-tests-rs/fixtures/persona/admission_engine.lua",
        "tools/civ6-lua-tests-rs/fixtures/persona/production_admission_engine.lua",
        "poc/JAC_PoC_LeaderPersona/Shared/JAC_PersonaTreeRules.lua",
        GAMEPLAY,
        "poc/JAC_PoC_LeaderPersona/UI/JAC_LeaderPersona_View.lua",
    ] {
        if path == GAMEPLAY {
            exec(
                &lua,
                "GameInfo.JAC_LeaderPersonas.LEADER_JAC_DATE_TANEMUNE.EvolutionCost=COST",
            );
            civ6_lua_tests_rs::persona_engine::prepare(&lua).unwrap();
        }
        lua.load(read_repo(path)).set_name(path).exec().unwrap();
        if path == GAMEPLAY {
            civ6_lua_tests_rs::persona_engine::finish_startup(&lua).unwrap();
        }
    }
    lua.globals().set("COST", cost).unwrap();
    lua.globals()
        .set("GAMEPLAY_SOURCE", read_repo(GAMEPLAY))
        .unwrap();
    exec(
        &lua,
        r#"
        LOCAL=2
        LEADER='LEADER_JAC_DATE_TANEMUNE'
        ROW=GameInfo.JAC_LeaderPersonas[LEADER]
        ROW.EvolutionCost=COST
        KEY='JAC_PERSONA_'..LEADER..'_'
        function NATIVE_READER(replacement)
            local found
            local targets={}
            local api=ExposedMembers.JAC_LeaderPersona
            for _,fn in ipairs({api.CompleteEvolution,api.Reconcile}) do
                local matched=false
                for index=1,64 do
                    local name,value=debug.getupvalue(fn,index)
                    if not name then break end
                    if name=='NativeGovernor' then
                        assert(not found or found==value,'Gameplay reader dependencies diverged')
                        found=found or value; matched=true
                        targets[#targets+1]={fn=fn,index=index}
                        break
                    end
                end
                assert(matched,'real Gameplay reader dependency was not found')
            end
            if replacement then
                for _,target in ipairs(targets) do debug.setupvalue(target.fn,target.index,replacement) end
            end
            return assert(found)
        end
        -- Deliver each queued operation after the local turn has become inactive,
        -- so the shipped UI does not immediately submit the next payment step.
        -- This captures the real partial-save boundary without editing callbacks.
        function deliver()
            Players[2].active=false
            local done=STEP()
            Players[2].active=true
            return done
        end
        -- Public join/evolve requests and one real engine appointment.
        JAC_RequestLeaderJoin(1); DRAIN()
        assert(Players[2]:GetProperty('JAC_PERSONA_RESULT')=='JOINED')
        Players[2].points=COST
        JAC_EvolveLeader(LEADER); assert(deliver() and #REQUESTS==0)
        assert(Game:GetProperty(KEY..'EVOLUTION_COST')==COST)
        assert(Game:GetProperty(KEY..'EVOLUTION_SURCHARGE')==COST-1)
        PERSON_ID=Game:GetProperty(KEY..'UNIT')
        assert(JAC_AppointLeader(LEADER))
        assert(#REQUESTS==1 and REQUESTS[1].op==PlayerOperations.APPOINT_GOVERNOR)
        -- One step only: this is the partial save boundary, before UI frames.
        assert(deliver() and #REQUESTS==0)
        assert(Players[2].spent==1 and DESTROYS==0)
        assert(Game:GetProperty(KEY..'STATE')=='EVOLVING')
        assert(Players[2]:GetUnits():FindID(PERSON_ID))
        function retain(expectedSpent)
            assert(Game:GetProperty(KEY..'STATE')=='EVOLVING','premature finalization')
            assert(Players[2]:GetUnits():FindID(PERSON_ID),'Great Person consumed')
            assert(Game:GetProperty(KEY..'UNIT')==PERSON_ID)
            assert(DESTROYS==0 and Players[2].spent==expectedSpent,'native payment changed')
        end
        function finalize(owner)
            UI.RequestPlayerOperation(owner or 2,PlayerOperations.EXECUTE_SCRIPT,
                {OnStart='JAC_LeaderPersonaFinalize',Leader=LEADER})
            assert(deliver() and #REQUESTS==0)
        end
        function promote(step)
            local payment=GameInfo.GovernorPromotions[ROW.BasePromotionType..'_TITLE_'..step]
            UI.RequestPlayerOperation(2,PlayerOperations.PROMOTE_GOVERNOR,
                {[PlayerOperations.PARAM_GOVERNOR_TYPE]=GameInfo.Governors[ROW.GovernorType].Index,
                 [PlayerOperations.PARAM_GOVERNOR_PROMOTION_TYPE]=payment.Index})
            assert(deliver() and #REQUESTS==0)
            assert(Players[2].spent==step+1)
        end
        function completed()
            assert(Game:GetProperty(KEY..'STATE')=='GOVERNOR')
            assert(Game:GetProperty(KEY..'UNIT')==nil)
            assert(not Players[2]:GetUnits():FindID(PERSON_ID))
            assert(DESTROYS==1 and Players[2].spent==COST)
            assert(Players[2]:GetProperty('JAC_PERSONA_RESULT')=='EVOLVED')
            finalize()
            assert(Players[2]:GetProperty('JAC_PERSONA_RESULT')=='ALREADY_EVOLVED')
            ExposedMembers.JAC_LeaderPersona.Reconcile()
            assert(DESTROYS==1 and Players[2].spent==COST)
        end
    "#,
    );
    lua
}

fn exec(lua: &Lua, source: &str) {
    lua.load(source).exec().unwrap();
}

#[test]
fn missing_surcharge_at_partial_save_never_consumes_person() {
    for cost in 1..=3 {
        let lua = scene(cost);
        exec(
            &lua,
            r#"
            Game:SetProperty(KEY..'EVOLUTION_SURCHARGE',nil)
            ExposedMembers.JAC_LeaderPersona.Reconcile(); retain(1)
            finalize(); retain(1)
            assert(Players[2]:GetProperty('JAC_PERSONA_RESULT')=='TITLE_PAYMENT_PENDING')
            RESET_CONTEXTS()
            assert(loadstring(GAMEPLAY_SOURCE))()
            retain(1)
            finalize(); retain(1)
        "#,
        );
    }
}

#[test]
fn invalid_saved_surcharge_rejects_even_after_full_native_payment() {
    for cost in 1..=3 {
        for bad in ["nil", "-1", "'0'", "0.5", "3", "false", "0/0"] {
            let lua = scene(cost);
            exec(&lua, "for step=1,COST-1 do promote(step) end");
            exec(
                &lua,
                &format!("Game:SetProperty(KEY..'EVOLUTION_SURCHARGE',{bad})"),
            );
            exec(&lua, "ExposedMembers.JAC_LeaderPersona.Reconcile(); retain(COST); finalize(); retain(COST)");
        }
    }
    for cost in 2..=3 {
        let lua = scene(cost);
        exec(
            &lua,
            "Game:SetProperty(KEY..'EVOLUTION_SURCHARGE',0); finalize(); retain(1)",
        );
    }
}

#[test]
fn saved_cost_must_match_independent_joining_contract_before_gameplay_read() {
    for cost in 1..=3 {
        for bad in ["nil", "0", "-1", "'2'", "1.5", "4", "false", "0/0"] {
            let lua = scene(cost);
            exec(&lua, "for step=1,COST-1 do promote(step) end");
            exec(
                &lua,
                &format!("Game:SetProperty(KEY..'EVOLUTION_COST',{bad}); finalize(); retain(COST)"),
            );
        }
        for mismatch in (1..=3).filter(|other| *other != cost) {
            let lua = scene(cost);
            exec(&lua, &format!("Game:SetProperty(KEY..'EVOLUTION_COST',{mismatch}); Game:SetProperty(KEY..'EVOLUTION_SURCHARGE',{}); finalize(); retain(1)", mismatch-1));
        }
    }
}

#[test]
fn current_definition_cost_must_be_a_supported_integer() {
    for bad in ["nil", "0", "-1", "'2'", "1.5", "4", "false", "0/0"] {
        let lua = scene(2);
        exec(&lua, "promote(1)");
        exec(
            &lua,
            &format!("ROW.EvolutionCost={bad}; finalize(); retain(2)"),
        );
    }
}

#[test]
fn unknown_or_invalid_native_payment_is_not_zero() {
    for cost in 1..=3 {
        for bad in ["nil", "-1", "'0'", "0.5", "3", "false", "0/0"] {
            let lua = scene(cost);
            exec(&lua, "for step=1,COST-1 do promote(step) end");
            exec(
                &lua,
                &format!(
                    r#"
                local read=NATIVE_READER()
                NATIVE_READER(function(owner,row)
                    local result=read(owner,row); result.surchargePaid={bad}; return result
                end)
                finalize(); retain(COST)
            "#
                ),
            );
        }
    }
}

#[test]
fn restored_receipt_waits_for_each_official_promotion_and_finalizes_once() {
    for cost in 2..=3 {
        let lua = scene(cost);
        exec(
            &lua,
            r#"
            Game:SetProperty(KEY..'EVOLUTION_SURCHARGE',nil)
            finalize(); retain(1)
            -- Restoring the actual prepare record is test recovery, not an API.
            Game:SetProperty(KEY..'EVOLUTION_COST',COST)
            Game:SetProperty(KEY..'EVOLUTION_SURCHARGE',COST-1)
            finalize(); retain(1)
            for step=1,COST-1 do
                assert(JAC_AppointLeader(LEADER))
                assert(#REQUESTS==1 and REQUESTS[1].op==PlayerOperations.PROMOTE_GOVERNOR)
                assert(deliver() and #REQUESTS==0)
                assert(Players[2].spent==step+1)
                retain(step+1)
                if step<COST-1 then finalize(); retain(step+1) end
            end
            assert(JAC_AppointLeader(LEADER))
            assert(#REQUESTS==1 and REQUESTS[1].params.OnStart=='JAC_LeaderPersonaFinalize')
            assert(deliver() and #REQUESTS==0)
            completed()
        "#,
        );
    }
}

#[test]
fn cost_one_requires_explicit_zero_and_recovery_finishes_once() {
    for remove in [false, true] {
        let lua = scene(1);
        if remove {
            exec(&lua, "Game:SetProperty(KEY..'EVOLUTION_SURCHARGE',nil); finalize(); retain(1); Game:SetProperty(KEY..'EVOLUTION_SURCHARGE',0)");
        }
        exec(
            &lua,
            "assert(JAC_AppointLeader(LEADER)); assert(deliver()); completed()",
        );
    }
}

#[test]
fn wrong_owner_is_rejected_and_price_only_change_keeps_joining_price() {
    let lua = scene(2);
    exec(
        &lua,
        r#"
        promote(1)
        finalize(1); retain(2)
        assert(Players[1]:GetProperty('JAC_PERSONA_RESULT')=='NOT_OWNED')
        ROW.EvolutionCost=3
        finalize(); completed()
    "#,
    );
}

#[test]
fn price_only_updates_preserve_the_ui_payment_prefix_across_reload() {
    for cost in 1..=3 {
        for new_price in (1..=3).filter(|price| *price != cost) {
            let lua = scene(cost);
            exec(
                &lua,
                &format!(
                    r#"
                ROW.EvolutionCost={new_price}
                Game:SetProperty(KEY..'MERIT',37)
                Game:SetProperty(KEY..'NODE_PRESERVED',1)
                RESET_CONTEXTS()
                assert(loadstring(GAMEPLAY_SOURCE))()
                assert(ExposedMembers.JAC_LeaderPersona.EvolutionCost(LEADER)==COST)
            "#
                ),
            );
            lua.load(read_repo(
                "poc/JAC_PoC_LeaderPersona/UI/JAC_LeaderPersona_View.lua",
            ))
            .exec()
            .unwrap();
            exec(
                &lua,
                r#"
                retain(1)
                for step=1,COST-1 do
                    assert(JAC_AppointLeader(LEADER),'saved UI payment step was refused')
                    assert(#REQUESTS==1 and REQUESTS[1].op==PlayerOperations.PROMOTE_GOVERNOR)
                    assert(REQUESTS[1].params.promotion==GameInfo.GovernorPromotions[ROW.BasePromotionType..'_TITLE_'..step].Index)
                    assert(deliver() and #REQUESTS==0)
                    retain(step+1)
                end
                assert(JAC_AppointLeader(LEADER) and deliver())
                completed()
                assert(Game:GetProperty(KEY..'MERIT')==37 and Game:GetProperty(KEY..'NODE_PRESERVED')==1)
                local def=GameInfo.Governors[ROW.GovernorType]
                assert(Players[2]:GetProperty(901000+def.Index)==true,'saved appointment eligibility missing')
                assert(Players[2]:GetProperty(905000+def.Index)==true,'saved placement eligibility missing')
                assert(Players[1]:GetProperty(905000+def.Index)==false,'other owner inherited payment')
                -- Assign through the shipped UI operation, then model native
                -- establishment. A repriced saved person still earns one deed.
                JAC_AssignLeader(LEADER,21); assert(deliver())
                local gov=Players[2]:GetGovernors():GetGovernor(def.Hash)
                gov.established=true; Players[2].human=true
                GameInfo.JAC_PersonaTreeNodes=collection({},'NodeType')
                GameInfo.JAC_PersonaTreePeople={[LEADER]={MeritProfile='MILITARY'}}
                GameInfo.JAC_PersonaMeritRules={MILITARY={GOVERN_TURN=2}}
                TURN=TURN+1
                ExposedMembers.JAC_LeaderPersona.OnPlayerTurnStarted(2)
                assert(Game:GetProperty(KEY..'MERIT')==39,'repricing stopped merit acquisition')
                ExposedMembers.JAC_LeaderPersona.OnPlayerTurnStarted(2)
                assert(Game:GetProperty(KEY..'MERIT')==39,'replayed turn duplicated merit')
                ROW.EvolutionCost=COST
                ExposedMembers.JAC_LeaderPersona.Reconcile()
                assert(#REQUESTS==0 and DESTROYS==1 and Players[2].spent==COST)
                assert(Game:GetProperty(KEY..'MERIT')==39)
            "#,
            );
        }
    }
}

#[test]
fn finalize_payload_cannot_replace_saved_receipt_or_person_identity() {
    for payload in [
        "nil",
        "false",
        "17",
        "'LEADER_JAC_DATE_TANEMUNE'",
        "{}",
        "{Leader=false}",
        "{Leader=17}",
        "{Leader={}}",
        "{Leader='UNKNOWN'}",
    ] {
        let lua = scene(2);
        exec(&lua, &format!("FIRE('JAC_LeaderPersonaFinalize',2,{payload}); retain(1); assert(Players[2]:GetProperty('JAC_PERSONA_RESULT')=='NOT_OWNED')"));
    }
    let lua = scene(2);
    exec(
        &lua,
        r#"
        Game:SetProperty(KEY..'EVOLUTION_SURCHARGE',nil)
        FIRE('JAC_LeaderPersonaFinalize',2,{Leader=LEADER,EvolutionCost=1,Surcharge=0,
            EVOLUTION_COST=2,EVOLUTION_SURCHARGE=1,appointed=true,base=true,surchargePaid=1})
        retain(1)
        assert(Game:GetProperty(KEY..'EVOLUTION_SURCHARGE')==nil)
        promote(1)
        FIRE('JAC_LeaderPersonaFinalize',2,{Leader=LEADER,EvolutionCost=2,Surcharge=1})
        retain(2)
        assert(Game:GetProperty(KEY..'EVOLUTION_SURCHARGE')==nil)
    "#,
    );
}

#[test]
fn current_cost_selects_unambiguous_native_prefix_after_validating_receipt() {
    let lua = scene(3);
    exec(
        &lua,
        r#"
        promote(1)
        -- Valid saved cost makes the shipped UI read the authoritative prefix.
        assert(ExposedMembers.JAC_LeaderPersonaNativeRead(2,LEADER).surchargePaid==1)
        finalize(); retain(2)
        Game:SetProperty(KEY..'EVOLUTION_COST',2)
        Game:SetProperty(KEY..'EVOLUTION_SURCHARGE',1)
        finalize(); retain(2)
        assert(Game:GetProperty(KEY..'EVOLUTION_COST')==2,'completion repaired receipt')
        Game:SetProperty(KEY..'EVOLUTION_COST',3)
        Game:SetProperty(KEY..'EVOLUTION_SURCHARGE',2)
        finalize(); retain(2)
        promote(2)
        assert(ExposedMembers.JAC_LeaderPersonaNativeRead(2,LEADER).surchargePaid==2)
        finalize(); completed()
    "#,
    );
}

#[test]
fn valid_receipt_consumes_only_the_matching_person_found_by_engine_identity() {
    let lua = scene(2);
    exec(
        &lua,
        r#"
        FIRE('JAC_LeaderPersonaJoin',2,{Source=0})
        assert(Players[2]:GetProperty('JAC_PERSONA_RESULT')=='JOINED')
        local other='JAC_PERSONA_LEADER_JAC_ODA_NOBUHIDE_'
        local otherID=Game:GetProperty(other..'UNIT')
        assert(otherID~=PERSON_ID and Players[2]:GetUnits():FindID(otherID))
        -- The cached unit id is never authority for removing a different person.
        Game:SetProperty(KEY..'UNIT',otherID)
        promote(1); finalize(); completed()
        assert(Players[2]:GetUnits():FindID(otherID))
        assert(Game:GetProperty(other..'STATE')=='GREAT_PERSON')
    "#,
    );
}

#[test]
fn invalid_receipt_is_rejected_before_gameplay_payment_read() {
    let lua = scene(2);
    exec(
        &lua,
        r#"
        Game:SetProperty(KEY..'EVOLUTION_COST','2')
        NATIVE_READER(function()
            error('invalid receipt reached saved-cost native reader')
        end)
        finalize(); retain(1)
        assert(Game:GetProperty(KEY..'EVOLUTION_COST')=='2')
    "#,
    );
}

#[test]
fn fully_paid_native_prefix_still_needs_explicit_valid_saved_receipt() {
    for cost in 2..=3 {
        for field in ["EVOLUTION_COST", "EVOLUTION_SURCHARGE"] {
            let lua = scene(cost);
            exec(
                &lua,
                "Players[2].human=false; for step=1,COST-1 do promote(step) end",
            );
            exec(
                &lua,
                &format!("Game:SetProperty(KEY..'{field}',nil); finalize(); retain(COST)"),
            );
            exec(
                &lua,
                r#"
                -- Explicit test recovery of the original Prepare receipt.
                Game:SetProperty(KEY..'EVOLUTION_COST',COST)
                Game:SetProperty(KEY..'EVOLUTION_SURCHARGE',COST-1)
                finalize(); completed()
            "#,
            );
        }
    }
}

#[test]
fn both_completion_entries_reject_bad_receipts_before_any_native_read() {
    for cost in 1..=3 {
        for field in ["EVOLUTION_COST", "EVOLUTION_SURCHARGE"] {
            for bad in ["nil", "false", "{}", "'2'", "-1", "0.5", "4", "0/0"] {
                let lua = scene(cost);
                exec(&lua, &format!("Game:SetProperty(KEY..'{field}',{bad})"));
                exec(
                    &lua,
                    r#"
                    local savedCost=Game:GetProperty(KEY..'EVOLUTION_COST')
                    local savedSurcharge=Game:GetProperty(KEY..'EVOLUTION_SURCHARGE')
                    local read=NATIVE_READER()
                    READ_CALLS=0
                    NATIVE_READER(function(owner,row)
                        READ_CALLS=READ_CALLS+1
                        error('bad receipt reached native reader')
                    end)
                    finalize(); retain(1)
                    assert(Players[2]:GetProperty('JAC_PERSONA_RESULT')=='TITLE_PAYMENT_PENDING')
                    ExposedMembers.JAC_LeaderPersona.Reconcile(); retain(1)
                    assert(READ_CALLS==0,'Reconcile performed a pre-validation read')
                    assert(Players[2]:GetProperty('JAC_PERSONA_RESULT')=='TITLE_PAYMENT_PENDING')
                    local function unchanged(before,after)
                        return before==after or (type(before)=='number' and before~=before and after~=after)
                    end
                    assert(unchanged(savedCost,Game:GetProperty(KEY..'EVOLUTION_COST')))
                    assert(unchanged(savedSurcharge,Game:GetProperty(KEY..'EVOLUTION_SURCHARGE')))
                    NATIVE_READER(read)
                "#,
                );
            }
        }
    }
}

#[test]
fn bad_receipt_does_not_abort_later_owners_reconciliation_or_admission() {
    for field in ["EVOLUTION_COST", "EVOLUTION_SURCHARGE"] {
        for bad in ["nil", "false", "{}", "'invalid'", "0", "3"] {
            let lua = scene(2);
            exec(
                &lua,
                r#"
                -- Ashina follows Date in GameInfo, belongs to another owner,
                -- and pays through the same engine queue before corruption.
                FIRE('JAC_LeaderPersonaJoin',1,{Source=2})
                assert(Players[1]:GetProperty('JAC_PERSONA_RESULT')=='JOINED')
                Players[1].points=1
                FIRE('JAC_LeaderPersonaEvolve',1,{Leader=ASHINA})
                local def=GameInfo.Governors[GameInfo.JAC_LeaderPersonas[ASHINA].GovernorType]
                OTHER_INDEX=def.Index
                OTHER_KEY='JAC_PERSONA_'..ASHINA..'_'
                OTHER_ID=Game:GetProperty(OTHER_KEY..'UNIT')
                UI.RequestPlayerOperation(1,PlayerOperations.APPOINT_GOVERNOR,
                    {[PlayerOperations.PARAM_GOVERNOR_TYPE]=def.Index})
                assert(deliver() and #REQUESTS==0)
                assert(Players[1].spent==1 and Players[2].spent==1 and DESTROYS==0)
            "#,
            );
            exec(&lua, &format!("Game:SetProperty(KEY..'{field}',{bad})"));
            exec(
                &lua,
                r#"
                local read=NATIVE_READER()
                BAD_READS,OTHER_READS=0,0
                NATIVE_READER(function(owner,row)
                    if row.LeaderType==LEADER then
                        BAD_READS=BAD_READS+1
                        error('invalid owner must not interrupt another owner')
                    end
                    if owner==1 and row.LeaderType==ASHINA then OTHER_READS=OTHER_READS+1 end
                    return read(owner,row)
                end)
                ExposedMembers.JAC_LeaderPersona.Reconcile()
                assert(BAD_READS==0 and OTHER_READS>=2)
                assert(Game:GetProperty(KEY..'STATE')=='EVOLVING')
                assert(Players[2]:GetUnits():FindID(PERSON_ID))
                assert(Game:GetProperty(KEY..'UNIT')==PERSON_ID)
                assert(Players[2]:GetProperty('JAC_PERSONA_RESULT')=='TITLE_PAYMENT_PENDING')
                assert(Game:GetProperty(OTHER_KEY..'STATE')=='GOVERNOR')
                assert(not Players[1]:GetUnits():FindID(OTHER_ID))
                assert(Players[1]:GetProperty('JAC_PERSONA_RESULT')=='EVOLVED')
                assert(Players[1]:GetProperty(901000+OTHER_INDEX)==true)
                assert(DESTROYS==1 and Players[1].spent==1 and Players[2].spent==1)
            "#,
            );
        }
    }
}
