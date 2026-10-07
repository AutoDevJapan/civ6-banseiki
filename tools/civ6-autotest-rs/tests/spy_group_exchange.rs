//! Modeled native APIs exercise the generated Lua in Lua 5.1. These cases
//! establish script behavior, not a live Civ6 exchange or a specific spy ID.
#[path = "../src/spy_group_exchange.rs"]
mod spy_group_exchange;

use mlua::Lua;

#[allow(dead_code)]
#[path = "../examples/group-exchange-62.rs"]
mod host;

#[path = "fixtures/spy_exchange_native.rs"]
mod native_fixture;
use native_fixture::NATIVE;

struct Case(Lua);

fn reserved_reload() -> Case {
    let c = Case::new();
    c.reserve();
    c.run("SAVED=PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62; SAVED_NONCE=SAVED.nonce; JAC_GroupExchange548=nil; Events.DiplomacyStatement.listeners={}; Events.DiplomacyIncomingDeal.listeners={}; ExposedMembers.JAC_GroupExchange548Witness=nil; ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey=ORIGINAL_TOOLTIP; Game.SetProperty=function() error('resume must not rewrite Core reservation') end");
    c.0.load(spy_group_exchange::UI_SETUP).exec().unwrap();
    c
}

#[test]
fn reserved_reload_restores_original_nonce_without_repreparing_or_writing_core() {
    let c = reserved_reload();
    c.run("assert(JAC_GroupExchange548.ResumeReserved('live04-969d4256-b4daf733-58a796ad')=='RESUMED_RESERVED'); assert(PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62==SAVED and ExposedMembers.JAC_GroupExchange548Witness.nonce==SAVED_NONCE); assert(#SENDS==0 and #OUT.items==0 and SESSION==nil); JAC_GroupExchange548.ArmAfterSave(); JAC_GroupExchange548.OpenSession(); assert(JAC_GroupExchange548.SessionReady()); JAC_GroupExchange548.EqualizeOnce(); assert(#SENDS==1 and H[1].turn==23 and H[2].turn==45 and H[3].turn==62)");
    c.rejects(
        "JAC_GroupExchange548.ResumeReserved('live04-969d4256-b4daf733-58a796ad')",
        "fresh UI instance",
    );
    c.rejects("JAC_GroupExchange548.EqualizeOnce()", "already sent");
}

#[test]
fn resume_refuses_later_states_changed_history_and_occupied_native_session() {
    for setup in [
        "SAVED.stage='FINAL_INTENT'",
        "SAVED.quote={}",
        "SAVED.errors={}",
        "SAVED.before.histories[1]={id=0,turn=22}",
        "SESSION=99",
        "CAPTIVE_COUNT=2",
    ] {
        let c = reserved_reload();
        c.run(setup);
        assert!(
            c.0.load(
                "return JAC_GroupExchange548.ResumeReserved('live04-969d4256-b4daf733-58a796ad')"
            )
            .exec()
            .is_err(),
            "{setup}"
        );
        c.run("assert(#SENDS==0 and #OUT.items==0 and PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62==SAVED)");
    }
    let c = reserved_reload();
    c.rejects(
        "JAC_GroupExchange548.ResumeReserved('unverified')",
        "audited host receipt",
    );
}

#[test]
fn cleanup_keeps_a_stock_copied_item_that_changed_after_observation() {
    let c = Case::new();
    c.run("REPLY_KIND='UNSAFE'");
    c.equalize();
    c.run(
        "DealManager.CopyIncomingToOutgoingWorkingDeal(0,1); JAC_GroupExchange548.PublishQuote()",
    );
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.run("KEPT_ID=OUT.items[1]:GetID(); OUT.items[1].amount=999; JAC_GroupExchange548.Close(); assert(#OUT.items==1 and OUT.items[1]:GetID()==KEPT_ID); local c=JAC_GroupExchange548.Evidence().ui.cleanup; assert(#c.removed==1 and #c.retained==1 and c.retained[1]==KEPT_ID)");
}

#[test]
fn host_executes_success_refusal_timeout_and_unresolved_return_with_saved_intents() {
    for (setup, expected_sends, expected_stage) in [
        ("", 2, "RETURNED"),
        ("REPLY_KIND='NONE'", 1, "QUOTED_UNKNOWN"),
        ("REPLY_KIND='UNSAFE'", 1, "QUOTED_UNKNOWN"),
        ("AUTO_RETURN=false", 2, "RETURN_UNRESOLVED"),
        ("FAULT='READ_REPLY'", 1, "QUOTED_UNKNOWN"),
        ("FAULT='READ_RETURN'", 2, "RETURN_UNRESOLVED"),
        ("FAULT='CORE_ID'", 2, "RETURN_UNRESOLVED"),
        ("FAULT='CORE_API'", 2, "RETURN_UNRESOLVED"),
        ("FAULT='CORE_QUOTE'", 1, "QUOTED_UNKNOWN"),
        ("FAULT='TERMS_FINAL'", 1, "RETURN_UNRESOLVED"),
        ("FAULT='SEND_EQUALIZE'", 0, "QUOTED_UNKNOWN"),
        ("FAULT='SEND_FINAL'", 1, "RETURN_UNRESOLVED"),
        ("FAULT='NON_SCALAR'", 1, "QUOTED_UNKNOWN"),
        ("FAULT='CLOSE_READ'", 2, "RETURNED"),
        (
            "FAULT='STOCK_UNSAFE'; REPLY_KIND='UNSAFE'",
            1,
            "QUOTED_UNKNOWN",
        ),
    ] {
        let lua = Lua::new();
        lua.load(NATIVE).exec().unwrap();
        lua.load(setup).exec().unwrap();
        lua.load("local send=DealManager.SendWorkingDeal; DealManager.SendWorkingDeal=function(action,a,b) assert(SAVED_STAGE==(action==DealProposalAction.EQUALIZE and 'RESERVED' or 'FINAL_INTENT'),'send before host save'); if (FAULT=='SEND_EQUALIZE' and action==DealProposalAction.EQUALIZE) or (FAULT=='SEND_FINAL' and action==DealProposalAction.ACCEPTED) then error('native send failure') end; local result=send(action,a,b); if FAULT=='STOCK_UNSAFE' and action==DealProposalAction.EQUALIZE then DealManager.CopyIncomingToOutgoingWorkingDeal(0,1) end; return result end").exec().unwrap();
        let scenario = host::scenario();
        for (index, step) in scenario["steps"].as_array().unwrap().iter().enumerate() {
            lua.load(format!("STEP={index}; if (FAULT=='READ_REPLY' and STEP==11) or (FAULT=='READ_RETURN' and STEP==19) then ExposedMembers.JAC_PersonaBoundary.ReadEnemy=function() error('native roster read failed') end end; if FAULT=='CORE_ID' and STEP==21 then Players[0]:GetUnits().FindID=function() return MAP_UNIT end; MAP_UNIT.GetID=function() return 99999 end end; if FAULT=='CORE_API' and STEP==21 then Players[0]:GetUnits().FindID=nil end; if FAULT=='CORE_QUOTE' and STEP==13 then PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.before.histories[3].turn=61 end; if FAULT=='TERMS_FINAL' and STEP==17 then IN.items[2].amount=20; OUT.items[2].amount=20 end")).exec().unwrap();
            lua.load("if FAULT=='NON_SCALAR' and STEP==11 then OUT.items[1].GetAmount=function() return function() end end end; if FAULT=='CLOSE_READ' and STEP==23 then DealManager.GetWorkingDeal=function() error('native cleanup read failed') end end").exec().unwrap();
            let action = step["action"].as_str().unwrap();
            match action {
                "load" | "exit_to_menu" => {}
                "save" => lua
                    .load("SAVED_STAGE=PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.stage")
                    .exec()
                    .unwrap(),
                "wait_for" => {
                    let code = step["condition"]["code"].as_str().unwrap();
                    if !lua.load(code).eval::<bool>().unwrap() {
                        lua.load("TIME=TIME+31").exec().unwrap();
                        assert!(
                            lua.load(code).eval::<bool>().unwrap(),
                            "wait {index} did not terminate"
                        );
                    }
                }
                "ui_lua" => {
                    let core = step["state_name"] == "GameCore_Tuner";
                    if core {
                        lua.load("UI_SAVED={UI,DealManager,DiplomacyManager,DealItemTypes,DealProposalAction,DealValidationResult}; UI=nil; DealManager=nil; DiplomacyManager=nil; DealItemTypes=nil; DealProposalAction=nil; DealValidationResult=nil").exec().unwrap();
                    }
                    lua.load(step["code"].as_str().unwrap())
                        .exec()
                        .unwrap_or_else(|e| panic!("step {index}: {e}"));
                    if core {
                        lua.load("UI,DealManager,DiplomacyManager,DealItemTypes,DealProposalAction,DealValidationResult=unpack(UI_SAVED)").exec().unwrap();
                    }
                }
                other => panic!("uncovered host operation {other}"),
            }
        }
        assert_eq!(
            lua.load("return #SENDS").eval::<i64>().unwrap(),
            expected_sends
        );
        assert_eq!(
            lua.load("return PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.stage")
                .eval::<String>()
                .unwrap(),
            expected_stage
        );
        lua.load("assert(CLOSED==1 and ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey==ORIGINAL_TOOLTIP); assert(H[1].turn==23 and H[2].turn==45 and H[3].turn==62); if FAULT=='NON_SCALAR' then assert(PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.quote.raw.error:find('non%-scalar')) end; if FAULT=='CLOSE_READ' then assert(#JAC_GroupExchange548.Evidence().ui.cleanup.errors==1) end").exec().unwrap();
        lua.load("local p=PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62; assert(SAVED_STAGE==p.stage); if FAULT=='CORE_ID' or FAULT=='CORE_API' or FAULT=='READ_RETURN' or FAULT=='TERMS_FINAL' then assert(p.errors and #p.errors>0 and p.returnEvidence) end; if FAULT=='STOCK_UNSAFE' then assert(#OUT.items==0 and #JAC_GroupExchange548.Evidence().ui.cleanup.removed==2) end").exec().unwrap();
    }
}

#[test]
fn core_rejects_a_recycled_find_id_result() {
    let c = Case::new();
    c.equalize();
    c.run("JAC_GroupExchange548.PublishQuote()");
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.0.load(spy_group_exchange::FINAL_INTENT_CORE)
        .exec()
        .unwrap();
    c.run("JAC_GroupExchange548.AcceptOnce(); JAC_GroupExchange548.PublishReturn(); local u=Players[0]:GetUnits(); u.FindID=function() return MAP_UNIT end; MAP_UNIT.GetID=function() return 99999 end");
    c.rejects(
        spy_group_exchange::RETURN_CORE,
        "Core returned native spy identity differs",
    );
}

impl Case {
    fn new() -> Self {
        let lua = Lua::new();
        assert_eq!(
            lua.load("return _VERSION").eval::<String>().unwrap(),
            "Lua 5.1"
        );
        lua.load(NATIVE).exec().unwrap();
        lua.load(spy_group_exchange::UI_SETUP).exec().unwrap();
        Self(lua)
    }

    fn run(&self, lua: &str) {
        self.0.load(lua).exec().unwrap();
    }

    fn rejects(&self, lua: &str, reason: &str) {
        let error = self.0.load(lua).exec().expect_err("must fail closed");
        assert!(error.to_string().contains(reason), "{error}");
    }

    fn reserve(&self) {
        self.run("assert(JAC_GroupExchange548.Prepare()=='PREPARED')");
        self.0
            .load(spy_group_exchange::RESERVE_CORE)
            .exec()
            .unwrap();
        self.run("assert(JAC_GroupExchange548.ArmAfterSave()=='ARMED')");
    }

    fn equalize(&self) {
        self.reserve();
        self.run("assert(JAC_GroupExchange548.OpenSession()=='SESSION_REQUESTED')");
        self.run("assert(JAC_GroupExchange548.SessionReady()==true)");
        self.run("assert(JAC_GroupExchange548.EqualizeOnce()=='EQUALIZE_SENT')");
    }
}

#[test]
fn captured_core_backing_unit_is_preserved_but_not_counted_as_an_active_map_spy() {
    let c = Case::new();
    c.run("local units=Players[0]:GetUnits(); units.Members=function() local done=false; return function() if not done then done=true; return 1,MAP_UNIT end end end; MAP_UNIT.GetID=function() return 1048586 end; MAP_UNIT.GetX=function() return -9999 end; MAP_UNIT.GetY=function() return -9999 end");
    c.reserve();
    c.run("local p=PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62; assert(p.stage=='RESERVED'); assert(#p.sources.offMapUnits==1); local u=p.sources.offMapUnits[1]; assert(u.id==1048586 and u.owner==0 and u.x==-9999 and u.y==-9999 and u.classification=='OFF_MAP_SENTINEL' and u.individualCaptiveIdentity=='UNKNOWN'); assert(#SENDS==0)");
}

#[test]
fn unknown_core_position_cannot_make_a_nonempty_spy_inventory_empty() {
    for fault in [
        "MAP_UNIT.GetX=function() return -9999 end",
        "MAP_UNIT.GetX=nil; MAP_UNIT.GetY=nil",
    ] {
        let c = Case::new();
        c.run("assert(JAC_GroupExchange548.Prepare()=='PREPARED'); local units=Players[0]:GetUnits(); units.Members=function() local done=false; return function() if not done then done=true; return 1,MAP_UNIT end end end");
        c.run(fault);
        c.rejects(
            spy_group_exchange::RESERVE_CORE,
            "Core active map spies changed",
        );
        c.run("assert(PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62==nil and #SENDS==0)");
    }
}

#[test]
fn native_reply_quote_saved_intent_one_accept_and_returned_actual_id() {
    let c = Case::new();
    c.reserve();
    c.run(
        "assert(JAC_GroupExchange548.OpenSession()=='SESSION_REQUESTED'); \
         assert(JAC_GroupExchange548.SessionReady()); \
         assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey()=='LOC_UNIT_SPY_NAME'); \
         assert(UI==ORIGINAL_UI)",
    );
    c.run("assert(JAC_GroupExchange548.EqualizeOnce()=='EQUALIZE_SENT')");
    c.run("local r=JAC_GroupExchange548.ResponseState(); assert(r.stage=='AI_QUOTE' and r.reply.SessionID==7 and r.reply.RespondingToDealAction==DealProposalAction.EQUALIZE)");
    c.run("assert(JAC_GroupExchange548.PublishQuote()=='AI_QUOTE')");
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.0.load(spy_group_exchange::FINAL_INTENT_CORE)
        .exec()
        .unwrap();
    c.run("assert(JAC_GroupExchange548.AcceptOnce()=='FINAL_SENT')");
    c.run("local r=JAC_GroupExchange548.ReturnState(); assert(r.stage=='RETURNED_MAP' and r.returned.id==77777 and #r.after.captured==2 and r.after.physical==3)");
    c.run("assert(JAC_GroupExchange548.PublishReturn()=='RETURNED_MAP')");
    c.0.load(spy_group_exchange::RETURN_CORE).exec().unwrap();
    c.run("assert(PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.stage=='RETURNED'); assert(#SENDS==2 and SENDS[1].action==DealProposalAction.EQUALIZE and SENDS[2].action==DealProposalAction.ACCEPTED); assert(PROPERTIES.JAC_BOUNDARY_THIRD_SPY_ARCHIVE.histories[1]==H[1] and PROPERTIES.JAC_BOUNDARY_THIRD_SPY_ARCHIVE.histories[2]==H[2] and PROPERTIES.JAC_BOUNDARY_THIRD_MISSION_ACK.observed.mission==H[3] and PROPERTIES.JAC_BOUNDARY_THIRD_SPY_STATE.missionPost.histories=='UI')");
    c.run("assert(JAC_GroupExchange548.Close()=='CLOSED' and CLOSED==1); assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey==ORIGINAL_TOOLTIP)");
}

#[test]
fn unresolved_or_stale_reply_never_retries_or_sends_final() {
    for setup in ["REPLY_KIND='NONE'", "REPLY_SESSION=99", "REPLY_INITIATOR=1"] {
        let c = Case::new();
        c.run(setup);
        c.equalize();
        if setup.contains("NONE") {
            c.run("TIME=TIME+31");
        }
        c.rejects("JAC_GroupExchange548.EqualizeOnce()", "already sent");
        c.run("local r=JAC_GroupExchange548.ResponseState(); assert(r.stage=='UNKNOWN_TIMEOUT' or r.stage=='UNKNOWN_RESPONSE')");
        c.run("JAC_GroupExchange548.PublishQuote()");
        c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
        c.rejects(
            "JAC_GroupExchange548.AcceptOnce()",
            "accepted native quote missing",
        );
        c.run("assert(#SENDS==1 and PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.stage=='QUOTED_UNKNOWN')");
    }
}

#[test]
fn unsafe_terms_changed_outgoing_and_roster_are_refused() {
    for setup in [
        "REPLY_KIND='UNSAFE'",
        "REPLY_KIND='GOLD_TOO_MUCH'",
        "REPLY_KIND='MISMATCH'",
    ] {
        let c = Case::new();
        c.run(setup);
        c.equalize();
        c.run("local r=JAC_GroupExchange548.ResponseState(); assert(r.stage=='UNSAFE_TERMS' or r.stage=='UNKNOWN_OUTGOING')");
        c.run("JAC_GroupExchange548.PublishQuote()");
        c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
        c.rejects(
            "JAC_GroupExchange548.AcceptOnce()",
            "accepted native quote missing",
        );
        c.run("assert(#SENDS==1)");
    }
    let c = Case::new();
    c.equalize();
    c.run("CAPTIVE_COUNT=2; MAP_COUNT=1");
    c.run("assert(JAC_GroupExchange548.ResponseState().stage=='UNKNOWN_ROSTER_CHANGED')");
    c.run("JAC_GroupExchange548.PublishQuote()");
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.rejects(
        "JAC_GroupExchange548.AcceptOnce()",
        "accepted native quote missing",
    );
}

#[test]
fn saved_intent_duplicate_calls_reload_and_owned_tooltip() {
    let c = Case::new();
    c.run("assert(UI==ORIGINAL_UI); JAC_GroupExchange548.Prepare()");
    c.rejects("JAC_GroupExchange548.Prepare()", "already reserved");
    c.rejects(
        "JAC_GroupExchange548.ArmAfterSave()",
        "saved Core reservation missing",
    );
    c.0.load(spy_group_exchange::RESERVE_CORE).exec().unwrap();
    c.run("JAC_GroupExchange548=nil");
    c.0.load(spy_group_exchange::UI_SETUP).exec().unwrap();
    c.rejects("JAC_GroupExchange548.Prepare()", "already reserved");

    let c = Case::new();
    c.reserve();
    c.run("JAC_GroupExchange548.OpenSession(); assert(JAC_GroupExchange548.SessionReady())");
    c.run("assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey()=='LOC_UNIT_SPY_NAME')");
    c.run("CAPTIVE_COUNT=2; assert(ExposedMembers.JAC_PersonaBoundaryMissingCaptiveDealTooltipKey()=='OLD_TOOLTIP'); CAPTIVE_COUNT=3");
    c.run("JAC_GroupExchange548.EqualizeOnce(); JAC_GroupExchange548.PublishQuote()");
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.rejects(
        "JAC_GroupExchange548.AcceptOnce()",
        "saved final intent differs",
    );
    c.0.load(spy_group_exchange::FINAL_INTENT_CORE)
        .exec()
        .unwrap();
    c.run("JAC_GroupExchange548.AcceptOnce()");
    c.rejects("JAC_GroupExchange548.AcceptOnce()", "final already sent");
    c.run("assert(#SENDS==2)");
}

#[test]
fn duplicate_statement_and_changed_saved_terms_stop_final_acceptance() {
    let c = Case::new();
    c.equalize();
    c.run("Events.DiplomacyStatement.fire(1,0,{SessionID=7,Initiator=0,StatementType=5,StatementSubType=6,DealAction=DealProposalAction.ADJUSTED,RespondingToDealAction=DealProposalAction.EQUALIZE})");
    c.run("assert(JAC_GroupExchange548.ResponseState().stage=='UNKNOWN_RESPONSE')");
    c.run("JAC_GroupExchange548.PublishQuote()");
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.rejects(
        "JAC_GroupExchange548.AcceptOnce()",
        "accepted native quote missing",
    );

    let c = Case::new();
    c.equalize();
    c.run("JAC_GroupExchange548.PublishQuote()");
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.0.load(spy_group_exchange::FINAL_INTENT_CORE)
        .exec()
        .unwrap();
    c.run("IN.items[2].amount=20; OUT.items[2].amount=20; assert(DealManager.AreWorkingDealsEqual(0,1))");
    c.rejects("JAC_GroupExchange548.AcceptOnce()", "native terms changed");
    c.run("assert(#SENDS==1)");
}

#[test]
fn unresolved_return_is_recorded_without_success() {
    let c = Case::new();
    c.run("AUTO_RETURN=false");
    c.equalize();
    c.run("JAC_GroupExchange548.PublishQuote()");
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.0.load(spy_group_exchange::FINAL_INTENT_CORE)
        .exec()
        .unwrap();
    c.run("JAC_GroupExchange548.AcceptOnce(); assert(JAC_GroupExchange548.ReturnState().stage=='WAIT_RETURN'); TIME=TIME+31; assert(JAC_GroupExchange548.ReturnState().stage=='RETURN_UNOBSERVED')");
    c.run("JAC_GroupExchange548.PublishReturn()");
    c.0.load(spy_group_exchange::RETURN_CORE).exec().unwrap();
    c.run("assert(PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.stage=='RETURN_UNRESOLVED')");
}

#[test]
fn returned_reference_id_is_observed_if_native_engine_actually_uses_it() {
    let c = Case::new();
    c.run("MAP_ID=1048586");
    c.equalize();
    c.run("JAC_GroupExchange548.PublishQuote()");
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.0.load(spy_group_exchange::FINAL_INTENT_CORE)
        .exec()
        .unwrap();
    c.run("JAC_GroupExchange548.AcceptOnce(); local r=JAC_GroupExchange548.ReturnState(); assert(r.stage=='RETURNED_MAP' and r.returned.id==MAP_ID and r.after.referencePresent==true)");
    c.run("JAC_GroupExchange548.PublishReturn()");
    c.0.load(spy_group_exchange::RETURN_CORE).exec().unwrap();
}

#[test]
fn absent_candidates_and_ambiguous_stock_copy_fail_closed() {
    let c = Case::new();
    c.run("DealManager.GetPossibleDealItems=function() return {} end");
    c.rejects("JAC_GroupExchange548.Prepare()", "candidate count changed");
    c.run("assert(PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62==nil and #OUT.items==0)");

    let c = Case::new();
    c.equalize();
    c.run(
        "DealManager.CopyIncomingToOutgoingWorkingDeal=function() error('native copy failed') end",
    );
    c.run("assert(JAC_GroupExchange548.PublishQuote()=='UNKNOWN_COPY')");
    c.rejects(
        "JAC_GroupExchange548.PublishQuote()",
        "quote already published",
    );
    c.0.load(spy_group_exchange::QUOTE_CORE).exec().unwrap();
    c.run("assert(PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.quote.raw.reply.SessionID==7 and PROPERTIES.JAC_BOUNDARY_GROUP_EXCHANGE_62.stage=='QUOTED_UNKNOWN' and #SENDS==1)");
}
