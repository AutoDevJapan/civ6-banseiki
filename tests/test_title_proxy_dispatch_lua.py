"""Real Core/UI Lua with delayed transport and mock Native, never an engine test."""
import unittest

from lupa import LuaRuntime, lua_type

from tests.test_title_proxy_payment_lua import GAMEPLAY, RULES, SETUP
from tests.test_title_proxy_view_lua import CODE as VIEW


TRANSPORT = """
include=function() end; print=function() end
ExposedMembers, Events, CALLBACKS = {}, {}, {}
GameEvents=setmetatable({}, {__index=function(_,name)
 return {Add=function(callback) CALLBACKS[name]=callback end}
end})
for id=0,1 do
 local p=Players[id]
 p.SetProperty=function(_,key,value) receipts[id][key]=copy(value) end
 p.IsHuman=function() return id==0 end
 p.IsTurnActive=function() return true end
end
Game.GetLocalPlayer=function() return 0 end
PlayerOperations={EXECUTE_SCRIPT=10,APPOINT_GOVERNOR=11,PROMOTE_GOVERNOR=12,
 PARAM_GOVERNOR_TYPE='governor',PARAM_GOVERNOR_PROMOTION_TYPE='promotion'}
GameInfo.Governors={GOVERNOR_JAC_TITLE_PROXY={Hash=9000,Index=200}}
GameInfo.GovernorPromotions={}
for stage=1,65 do
 GameInfo.GovernorPromotions['GOVERNOR_PROMOTION_JAC_TITLE_PROXY_'..stage]={Hash=10000+stage,Index=300+stage}
end
NATIVE_STAGE=-1; NATIVE_SUCCESS=false; PUBLISH=true; ELIGIBLE=true
OPS={}; NATIVE_CALLS={}; SCRIPT_CALLS={}; IN_FRAME=false
local accounting=Players[0].GetGovernors
Players[0].GetGovernors=function()
 local g=accounting()
 g.HasGovernor=function(_,hash) assert(hash==9000); return NATIVE_STAGE>=0 end
 g.GetGovernor=function(_,hash)
  assert(hash==9000)
  return {HasPromotion=function(_,promotion) return NATIVE_STAGE>=promotion-10000 end}
 end
 g.CanAppointGovernor=function(_,hash) assert(hash==9000); return ELIGIBLE and NATIVE_STAGE<0 end
 g.CanPromoteGovernor=function(_,hash) assert(hash==9000); return ELIGIBLE and NATIVE_STAGE>=0 end
 return g
end
UI={RequestPlayerOperation=function(owner,op,params)
 local record={owner=owner,op=op,params=copy(params),frame=IN_FRAME}
 OPS[#OPS+1]=record
 if op==10 then SCRIPT_CALLS[#SCRIPT_CALLS+1]=record else NATIVE_CALLS[#NATIVE_CALLS+1]=record end
end}
function core_step()
 local item=table.remove(OPS,1); if not item then return false end
 if item.op==10 then CALLBACKS[item.params.OnStart](item.owner,item.params)
 else
  assert(item.params.governor==200, 'Native needs Index, not Hash')
  local stage=item.op==11 and 0 or item.params.promotion-300
  assert(stage>=0 and stage<=65)
  -- Mock only: acquired stages are idempotent and promotion prerequisites
  -- are enforced. Actual Native behavior still needs the engine diagnostic.
  if NATIVE_SUCCESS and ELIGIBLE and NATIVE_STAGE==stage-1 and Players[0].points>Players[0].spent then
   NATIVE_STAGE=stage; Players[0].spent=Players[0].spent+1
   if PUBLISH then receipts[0]['JAC_PROXY_NATIVE_PAID_'..stage]=1 end
  end
 end
 return true
end
function load_ui()
 UI_ENV=setmetatable({}, {__index=_G})
 UI_ENV.Game={GetProperty=Game.GetProperty,GetLocalPlayer=Game.GetLocalPlayer,
  GetCurrentGameTurn=Game.GetCurrentGameTurn,
  SetProperty=function() error('UI must not mutate GameCore') end}
 FRAME=nil
 UI_ENV.ContextPtr={SetHide=function() end,SetUpdate=function(_,fn) FRAME=fn end,
  ClearUpdate=function() FRAME=nil end}
 UI_ENV.include=function() assert(load(RULES_SOURCE,'rules','t',UI_ENV))() end
 assert(load(VIEW_SOURCE,'view','t',UI_ENV))()
end
function frame_step()
 IN_FRAME=true; if FRAME then FRAME() end; IN_FRAME=false
end
"""


def unpack(value):
    if lua_type(value) == "table":
        return {key: unpack(item) for key, item in value.items()}
    return value


class DispatchTests(unittest.TestCase):
    def setUp(self):
        self.lua = LuaRuntime()
        self.lua.execute(SETUP)
        self.lua.execute(RULES)
        self.lua.globals().RULES_SOURCE = RULES
        self.lua.globals().VIEW_SOURCE = VIEW
        self.lua.execute(TRANSPORT)
        self.lua.execute(GAMEPLAY)
        self.lua.globals().load_ui()

    @property
    def rules(self):
        return self.lua.globals().JAC_TitlePaymentRules

    @property
    def ui(self):
        return self.lua.globals().UI_ENV

    def run_lua(self, code):
        self.lua.execute(code)

    def drain(self):
        for _ in range(20):
            while self.lua.globals().core_step():
                pass
            self.lua.globals().frame_step()
            if not len(self.lua.globals().OPS) and self.lua.globals().FRAME is None:
                return
        self.fail("UI transport did not finish")

    def reload_save(self):
        # New VM, no UI callback, no Lua locals, no aliases to the old tables.
        saved = {key: unpack(self.lua.globals()[key]) for key in (
            "properties", "receipts", "PERSON_EXISTS", "TURN", "NATIVE_STAGE",
            "NATIVE_SUCCESS", "PUBLISH", "ELIGIBLE")}
        points, spent = self.lua.eval("Players[0].points"), self.lua.eval("Players[0].spent")
        self.setUp()
        for key, value in saved.items():
            self.lua.globals()[key] = self.lua.table_from(value, recursive=True) if isinstance(value, dict) else value
        self.lua.globals().Players[0]["points"] = points
        self.lua.globals().Players[0]["spent"] = spent

    def pay(self):
        self.assertTrue(self.ui.JAC_ProxyPayNext())
        self.drain()

    def reserve_begin(self, leader="A"):
        self.assertEqual(self.rules.Request(0, leader), "PENDING")
        self.assertEqual(self.rules.Begin(0), "STARTED")

    def payments(self):
        return [list(r["payments"].values()) for r in self.rules.Queue(0)["requests"].values()]

    def test_begin_then_person_lost_before_dispatch_releases_only_unsent_stage(self):
        self.rules.Request(0, "A")
        self.rules.Request(0, "C")
        self.rules.Begin(0)
        self.run_lua("PERSON_EXISTS.A=false")
        self.ui.JAC_ProxyPayNext()
        self.drain()
        self.assertIsNone(self.rules.Queue(0)["flight"])
        self.assertEqual(self.rules.Queue(0)["requests"][1]["abortedFlight"]["state"], "ABORTED_UNSENT")
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 0)
        self.reload_save()
        self.assertEqual(self.rules.Begin(0), "STARTED")
        self.run_lua("NATIVE_SUCCESS=true")
        self.pay()
        self.pay()
        self.assertEqual(self.payments(), [[], [0]])
        self.assertEqual(self.lua.eval("Players[0].spent"), 1)

    def test_delayed_authorize_cannot_spend_for_replacement_at_the_same_stage(self):
        self.reserve_begin()
        self.rules.Request(0, "C")
        self.assertTrue(self.ui.JAC_ProxyPayNext())  # queued, Core has not granted it
        self.run_lua("PERSON_EXISTS.A=false; NATIVE_SUCCESS=true")
        self.assertEqual(self.rules.Begin(0), "STARTED")
        replacement = self.rules.Queue(0)["flight"]["id"]
        self.drain()
        self.assertEqual(self.lua.eval("Players[0]:GetProperty('JAC_PROXY_AUTHORIZE_RESULT').result"), "STALE_FLIGHT")
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 0)
        self.assertEqual(self.rules.Queue(0)["flight"]["id"], replacement)
        self.pay()
        self.pay()
        self.assertEqual(self.payments(), [[], [0]])

    def test_begin_reconciles_invalid_later_reservation_before_balance_check(self):
        self.run_lua("Players[0].points=4")
        self.rules.Request(0, "C")
        self.rules.Request(0, "B")
        self.run_lua("PERSON_EXISTS.B=false; Players[0].spent=1")
        self.assertEqual(self.rules.Begin(0), "STARTED")
        self.assertEqual(self.rules.Queue(0)["requests"][2]["status"], "CANCELLED")
        self.run_lua("NATIVE_SUCCESS=true")
        self.pay()
        self.pay()
        self.assertEqual(self.payments(), [[0], []])
        self.assertEqual(self.lua.eval("Players[0].spent"), 2)

    def test_request_wrapper_reconciles_all_reservations_and_replays_cancelled_without_person(self):
        self.run_lua("Players[0].points=4")
        self.rules.Request(0, "C")
        self.rules.Request(0, "B")
        self.run_lua("PERSON_EXISTS.B=false; Players[0].spent=1")
        self.ui.JAC_ProxyRequest("A")
        self.drain()
        self.assertEqual(self.lua.eval("Players[0]:GetProperty('JAC_PROXY_REQUEST_RESULT')"), "PENDING")
        self.assertEqual([r["status"] for r in self.rules.Queue(0)["requests"].values()], ["PENDING", "CANCELLED", "PENDING"])
        self.reload_save()
        self.ui.JAC_ProxyRequest("B")
        self.drain()
        self.assertEqual(self.lua.eval("Players[0]:GetProperty('JAC_PROXY_REQUEST_RESULT')"), "CANCELLED")
        self.assertEqual(len(self.rules.Queue(0)["requests"]), 3)

    def test_failed_native_retry_is_same_turn_blocked_after_ui_and_save_reload(self):
        self.reserve_begin("C")
        self.pay()
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 1)
        self.assertEqual(self.lua.eval("Players[0].spent"), 0)
        self.pay()
        self.lua.globals().load_ui()
        self.pay()
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 1)
        self.assertEqual(self.rules.Queue(0)["flight"]["lastAttemptTurn"], 10)
        self.reload_save()
        self.pay()
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 0)
        self.run_lua("TURN=11; NATIVE_SUCCESS=true")
        self.lua.globals().load_ui()
        self.pay()
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 1)
        self.assertEqual(self.rules.Queue(0)["flight"]["attempt"], 2)
        self.assertEqual(self.lua.eval("Players[0].spent"), 1)
        self.pay()
        self.assertTrue(self.rules.Paid(0, "C"))

    def test_grant_then_ui_stop_or_person_loss_never_releases_unknown_result(self):
        for loss in (False, True):
            with self.subTest(person_lost=loss):
                self.setUp()
                self.reserve_begin()
                self.rules.Request(0, "C")
                self.ui.JAC_ProxyPayNext()
                self.lua.globals().core_step()  # grant persisted, no frame/Native yet
                if loss:
                    self.run_lua("PERSON_EXISTS.A=false")
                self.reload_save()
                self.pay()
                self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 0)
                self.assertEqual(self.rules.Queue(0)["flight"]["request"], 1)
                self.run_lua("TURN=99")
                self.rules.Begin(0)
                self.assertEqual(self.rules.Queue(0)["flight"]["request"], 1)
                if not loss:
                    self.run_lua("NATIVE_SUCCESS=true")
                    self.pay()
                    self.assertEqual(self.lua.eval("Players[0].spent"), 1)

    def test_old_denial_cannot_complete_a_new_transport_callback_before_core_runs(self):
        self.reserve_begin("C")
        self.pay()  # Failed Native, attempt 1
        self.pay()  # Persisted WAIT_NEXT_TURN reply for attempt 1
        self.run_lua("TURN=11; NATIVE_SUCCESS=true")
        self.lua.globals().load_ui()
        self.assertTrue(self.ui.JAC_ProxyPayNext())
        self.lua.globals().frame_step()  # old reply, new Core operation still queued
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 1)
        self.drain()
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 2)
        self.assertEqual(self.lua.eval("Players[0].spent"), 1)

    def test_failed_promotion_retries_only_the_same_stage_on_the_next_turn(self):
        self.reserve_begin()
        self.run_lua("NATIVE_SUCCESS=true")
        self.pay()
        self.pay()
        self.assertEqual(self.rules.Begin(0), "STARTED")
        self.run_lua("NATIVE_SUCCESS=false")
        self.pay()
        self.pay()
        self.lua.globals().load_ui()
        self.pay()
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 2)
        self.assertEqual(self.payments(), [[0]])
        self.reload_save()
        self.pay()
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 0)
        self.run_lua("TURN=11; NATIVE_SUCCESS=true")
        self.pay()
        self.pay()
        self.assertEqual(self.payments(), [[0, 1]])
        self.assertEqual(self.lua.globals().NATIVE_CALLS[1]["params"]["promotion"], 301)
        self.assertEqual(self.lua.eval("Players[0].spent"), 2)

    def test_spending_after_grant_cannot_take_another_persons_reservation(self):
        self.run_lua("Players[0].points=3; NATIVE_SUCCESS=true")
        self.reserve_begin()
        self.rules.Request(0, "C")
        self.ui.JAC_ProxyPayNext()
        self.lua.globals().core_step()
        self.run_lua("Players[0].spent=2")
        self.drain()
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 0)
        self.assertEqual(self.rules.Queue(0)["flight"]["request"], 1)
        self.run_lua("TURN=11; Players[0].points=4")
        self.pay()
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 1)

    def test_late_receipt_after_cancel_and_fresh_vm_keeps_original_allocation(self):
        self.reserve_begin()
        self.rules.Request(0, "C")
        self.run_lua("NATIVE_SUCCESS=true; PUBLISH=false")
        self.pay()
        self.assertEqual(self.lua.eval("Players[0].spent"), 1)
        self.run_lua("PERSON_EXISTS.A=false")
        self.assertEqual(self.rules.Advance(0), "NATIVE_PENDING")
        self.reload_save()
        self.assertEqual(self.rules.Queue(0)["requests"][1]["status"], "CANCELLED")
        self.assertEqual(self.rules.Queue(0)["flight"]["stage"], 0)
        self.run_lua("TURN=20; receipts[0]['JAC_PROXY_NATIVE_PAID_0']=1")
        self.pay()
        self.assertEqual(self.payments(), [[0], []])
        self.assertFalse(self.rules.Paid(0, "A"))
        self.assertFalse(self.rules.Paid(0, "C"))
        self.assertEqual(self.rules.Begin(0), "STARTED")
        self.run_lua("PUBLISH=true")
        self.pay()
        self.pay()
        self.assertEqual(self.payments(), [[0], [1]])
        self.assertEqual(self.lua.eval("Players[0].spent"), 2)

    def test_acquired_stage_waits_for_receipt_then_advances_at_zero_balance(self):
        self.run_lua("Players[0].points=1; NATIVE_SUCCESS=true; PUBLISH=false")
        self.reserve_begin("C")
        self.pay()
        self.run_lua("TURN=11")
        self.lua.globals().load_ui()
        self.pay()
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 1)
        self.assertFalse(self.rules.Paid(0, "C"))
        self.reload_save()
        self.run_lua("receipts[0]['JAC_PROXY_NATIVE_PAID_0']=1")
        self.pay()
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 0)
        self.assertEqual(self.lua.eval("Players[0].spent"), 1)
        self.assertTrue(self.rules.Paid(0, "C"))

    def test_native_acquisition_before_receipt_does_not_retry_even_with_spare_balance(self):
        self.reserve_begin()
        self.run_lua("NATIVE_SUCCESS=true; PUBLISH=false")
        self.pay()
        self.run_lua("TURN=11")
        self.pay()
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 1)
        self.assertEqual(self.lua.eval("Players[0].spent"), 1)
        self.run_lua("receipts[0]['JAC_PROXY_NATIVE_PAID_0']=1")
        self.pay()
        self.assertEqual(self.payments(), [[0]])

    def test_legacy_unknown_flight_never_becomes_unsent_on_reload(self):
        self.reserve_begin()
        self.rules.Request(0, "C")
        self.run_lua("properties.JAC_PROXY_PAYMENT_QUEUE_0.flight={request=1,stage=0}; PERSON_EXISTS.A=false")
        self.reload_save()
        self.rules.Begin(0)
        self.assertEqual(self.rules.Queue(0)["flight"]["state"], "AUTHORIZED")
        self.run_lua("TURN=50")
        self.rules.Reconcile(0)
        self.assertEqual(self.rules.Queue(0)["flight"]["request"], 1)
        self.run_lua("receipts[0]['JAC_PROXY_NATIVE_PAID_0']=1; Players[0].spent=1")
        self.assertEqual(self.rules.Advance(0), "CANCELLED_PAYMENT")
        self.assertEqual(self.payments(), [[0], []])

    def test_legacy_consumed_stage_keeps_its_spending_floor_during_retry(self):
        self.reserve_begin("C")
        self.run_lua("""
            properties.JAC_PROXY_PAYMENT_QUEUE_0.flight={request=1,stage=0}
            NATIVE_STAGE=0; Players[0].spent=1
        """)
        self.reload_save()
        self.rules.Reconcile(0)
        self.run_lua("TURN=11")
        self.pay()
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 0)
        self.run_lua("receipts[0]['JAC_PROXY_NATIVE_PAID_0']=1")
        self.pay()
        self.assertTrue(self.rules.Paid(0, "C"))
        self.assertEqual(self.payments(), [[0]])

    def test_receipt_without_dispatch_authorization_is_not_allocated_or_released(self):
        self.reserve_begin()
        self.rules.Request(0, "C")
        self.run_lua("receipts[0]['JAC_PROXY_NATIVE_PAID_0']=1; Players[0].spent=1; PERSON_EXISTS.A=false")
        self.assertEqual(self.rules.Advance(0), "UNEXPECTED_NATIVE_PAYMENT")
        self.assertEqual(self.rules.Queue(0)["flight"]["request"], 1)
        self.assertEqual(self.rules.Queue(0)["cursor"], 0)
        self.assertEqual(self.payments(), [[], []])

    def test_native_eligibility_and_exact_index_are_checked_on_dispatch_frame(self):
        self.reserve_begin()
        self.run_lua("ELIGIBLE=false; NATIVE_SUCCESS=true")
        self.pay()
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 0)
        self.run_lua("TURN=11; ELIGIBLE=true")
        self.pay()
        self.pay()
        self.assertEqual(self.rules.Begin(0), "STARTED")
        self.pay()
        calls = self.lua.globals().NATIVE_CALLS
        self.assertEqual([r["op"] for r in calls.values()], [11, 12])
        self.assertEqual([r["params"]["governor"] for r in calls.values()], [200, 200])
        self.assertEqual(calls[2]["params"]["promotion"], 301)
        self.assertTrue(all(r["frame"] for r in calls.values()))

    def test_diagnostic_request_begin_pay_advance_use_frames_and_common_handlers(self):
        self.run_lua("NATIVE_SUCCESS=true")
        for action in ("REQUEST", "BEGIN", "PAY", "ADVANCE"):
            before = len(self.lua.globals().SCRIPT_CALLS)
            self.assertTrue(self.ui.JAC_ProxyOnFrame(action, "C"))
            self.assertEqual(len(self.lua.globals().SCRIPT_CALLS), before)
            self.drain()
        calls = list(self.lua.globals().SCRIPT_CALLS.values())
        self.assertEqual([r["params"]["OnStart"] for r in calls], [
            "JAC_TitleProxyRequest", "JAC_TitleProxyBegin", "JAC_TitleProxyAuthorize", "JAC_TitleProxyAdvance"])
        self.assertTrue(all(r["frame"] for r in calls))
        self.assertEqual(len(self.lua.globals().NATIVE_CALLS), 1)
        self.assertTrue(self.rules.Paid(0, "C"))
        self.assertEqual(self.payments(), [[0]])


if __name__ == "__main__":
    unittest.main()
