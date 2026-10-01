"""Candidate queue tests; they do not prove native receipt activation or AI abstention."""
from pathlib import Path
import unittest

from lupa import LuaRuntime

RULES = (Path(__file__).resolve().parents[1] / "poc/JAC_PoC_LeaderPersona/Shared/JAC_TitlePaymentRules.lua").read_text(encoding="utf-8")
GAMEPLAY = (Path(__file__).resolve().parents[1] / "poc/JAC_PoC_LeaderPersona/Scripts/JAC_TitleProxy_Probe.lua").read_text(encoding="utf-8")
SETUP = """
properties, receipts = {}, {[0]={}, [1]={}}
TURN=10
function copy(value)
 if type(value)~='table' then return value end
 local result={}; for key,item in pairs(value) do result[key]=copy(item) end; return result
end
Game = {
 GetProperty=function(_,key) return copy(properties[key]) end,
 SetProperty=function(_,key,value) properties[key]=copy(value) end,
 GetCurrentGameTurn=function() return TURN end
}
GameInfo = {JAC_LeaderPersonas={A={EvolutionCost=2},B={EvolutionCost=3},C={EvolutionCost=1}}}
GameInfo.GreatPersonIndividuals={}
GameInfo.GreatPersonClasses={GP_CLASS={UnitType='UNIT_PERSONA'}}
GameInfo.Units={UNIT_PERSONA={Index=5000}}
PERSON_EXISTS={A=true,B=true,C=true}
for index,leader in ipairs({'A','B','C'}) do
 GameInfo.JAC_LeaderPersonas[leader].PersonType='PERSON_'..leader
 GameInfo.GreatPersonIndividuals['PERSON_'..leader]={GreatPersonClassType='GP_CLASS'}
 GameInfo.GreatPersonIndividuals[2000+index]={GreatPersonIndividualType='PERSON_'..leader}
end
Players = {}
for id=0,1 do
 local player={points=6,spent=0}
 player.IsAlive=function() return true end
 player.IsMajor=function() return true end
 player.GetProperty=function(_,key) return receipts[id][key] end
 player.GetUnits=function() return {Members=function()
  local index=0; return function()
   while index<3 do
    index=index+1; local current=index; local leader=({'A','B','C'})[index]
    if id==0 and PERSON_EXISTS[leader] then
     return index,{GetType=function() return 5000 end,GetGreatPerson=function()
      return {GetIndividual=function() return 2000+current end}
     end}
    end
   end
  end
 end} end
 player.GetGovernors=function() return {
  GetGovernorPoints=function() return player.points end,
  GetGovernorPointsSpent=function() return player.spent end
 } end
 Players[id]=player
end
for _,leader in ipairs({'A','B','C'}) do
 properties['JAC_PERSONA_'..leader..'_OWNER']=0
 properties['JAC_PERSONA_'..leader..'_STATE']='GREAT_PERSON'
end
function authorize(owner)
 local r=JAC_TitlePaymentRules; local f=r.Queue(owner).flight
 return r.Authorize(owner,f.id,f.request,f.stage,f.attempt)
end
function native_payment(owner,stage)
 if JAC_TitlePaymentRules and JAC_TitlePaymentRules.Queue(owner) then
  if not JAC_TitlePaymentRules.Queue(owner).flight then JAC_TitlePaymentRules.Begin(owner) end
  assert(authorize(owner)=='AUTHORIZED')
 end
 receipts[owner]['JAC_PROXY_NATIVE_PAID_'..stage]=1
 Players[owner].spent=Players[owner].spent+1
end
"""


class TitleProxyPaymentTests(unittest.TestCase):
    def setUp(self):
        self.lua = LuaRuntime()
        self.lua.execute(SETUP)
        self.lua.execute(RULES)
        self.rules = self.lua.globals().JAC_TitlePaymentRules

    def test_unjoined_and_foreign_people_never_create_a_reservation(self):
        self.assertEqual(self.rules.Request(0, "missing"), "NOT_OWNED")
        self.assertEqual(self.rules.Request(1, "A"), "NOT_OWNED")
        self.assertIsNone(self.rules.Queue(0))
        self.assertIsNone(self.rules.Queue(1))

    def test_full_cost_and_other_requests_reservations_are_required(self):
        self.lua.execute("Players[0].points=4")
        self.assertEqual(self.rules.Request(0, "B"), "PENDING")
        self.assertEqual(self.rules.Request(0, "A"), "NO_TITLE")
        self.assertEqual(self.rules.Request(0, "C"), "PENDING")
        self.assertEqual(self.rules.Next(0)["leader"], "B")
        self.assertEqual(len(self.rules.Queue(0)["requests"]), 2)
        self.assertEqual(self.lua.globals().Players[0]["spent"], 0)

    def test_spent_points_without_the_named_native_receipt_are_not_payment(self):
        self.rules.Request(0, "A")
        self.rules.Begin(0)
        self.lua.execute("Players[0].spent=1")
        self.assertEqual(self.rules.Advance(0), "NATIVE_PENDING")
        self.assertFalse(self.rules.Paid(0, "A"))
        self.assertEqual(self.rules.Queue(0)["cursor"], 0)

    def test_native_receipt_without_official_spending_is_not_payment(self):
        self.rules.Request(0, "C")
        self.rules.Begin(0)
        self.assertEqual(self.lua.globals().authorize(0), "AUTHORIZED")
        self.lua.execute("receipts[0]['JAC_PROXY_NATIVE_PAID_0']=1")
        self.assertEqual(self.rules.Advance(0), "ACCOUNTING_PENDING")
        self.assertFalse(self.rules.Paid(0, "C"))

        self.assertEqual(self.rules.Queue(0)["cursor"], 0)

    def test_foreign_and_duplicate_receipts_do_not_satisfy_an_owned_request(self):
        self.rules.Request(0, "A")
        self.lua.execute("native_payment(1,0)")
        self.assertEqual(self.rules.Advance(0), "NO_STARTED_STAGE")
        self.lua.execute("native_payment(0,0)")
        self.assertEqual(self.rules.Advance(0), "PARTIAL")
        self.assertEqual(self.rules.Advance(0), "NO_STARTED_STAGE")
        self.assertEqual(list(self.rules.Queue(0)["requests"][1]["payments"].values()), [0])

    def test_a_payment_is_allocated_to_only_one_person(self):
        self.assertEqual(self.rules.Request(0, "C"), "PENDING")
        self.assertEqual(self.rules.Request(0, "A"), "PENDING")
        self.lua.execute("native_payment(0,0)")
        self.assertEqual(self.rules.Advance(0), "PAID")
        self.assertTrue(self.rules.Paid(0, "C"))
        self.assertFalse(self.rules.Paid(0, "A"))
        self.assertEqual(self.rules.Advance(0), "NO_STARTED_STAGE")
        self.assertEqual(self.rules.Next(0)["stage"], 1)

    def test_costs_one_two_and_three_consume_six_distinct_official_payments(self):
        for person, cost in (("C", 1), ("A", 2), ("B", 3)):
            self.assertEqual(self.rules.Request(0, person), "PENDING")
            for part in range(cost):
                stage = self.rules.Next(0)["stage"]
                self.lua.globals().native_payment(0, stage)
                self.assertEqual(self.rules.Advance(0), "PAID" if part == cost - 1 else "PARTIAL")
            self.assertTrue(self.rules.Paid(0, person))
            self.assertEqual(self.rules.Request(0, person), "ALREADY_PAID")
        requests = self.rules.Queue(0)["requests"]
        self.assertEqual([list(requests[i]["payments"].values()) for i in (1, 2, 3)], [[0], [1, 2], [3, 4, 5]])
        self.assertEqual(self.lua.globals().Players[0]["points"], 6)
        self.assertEqual(self.lua.globals().Players[0]["spent"], 6)
        self.assertEqual(self.rules.Queue(0)["cursor"], 6)

    def test_reloaded_context_continues_partial_payment_without_charging_again(self):
        self.rules.Request(0, "B")
        self.lua.execute("native_payment(0,0)")
        self.assertEqual(self.rules.Advance(0), "PARTIAL")
        self.lua.execute("JAC_TitlePaymentRules=nil")
        self.lua.execute(RULES)
        self.rules = self.lua.globals().JAC_TitlePaymentRules
        self.assertEqual(self.rules.Request(0, "B"), "PENDING")
        self.assertEqual(self.rules.Next(0)["stage"], 1)
        self.assertEqual(self.rules.Advance(0), "NO_STARTED_STAGE")
        for stage in (1, 2):
            self.lua.globals().native_payment(0, stage)
            self.rules.Advance(0)
        self.assertTrue(self.rules.Paid(0, "B"))
        self.assertEqual(self.lua.globals().Players[0]["spent"], 3)

    def test_external_spending_pauses_and_new_titles_resume_the_unpaid_part(self):
        self.lua.execute("Players[0].points=5")
        self.rules.Request(0, "B")
        self.rules.Request(0, "A")
        self.lua.execute("Players[0].spent=1")  # Unrelated governor expense has no receipt.
        for stage in (0, 1):
            self.lua.globals().native_payment(0, stage)
            self.assertEqual(self.rules.Advance(0), "PARTIAL")
        self.assertIsNone(self.rules.Next(0))
        self.lua.execute("Players[0].points=6")
        self.assertEqual(self.rules.Next(0)["stage"], 2)
        self.assertFalse(self.rules.Paid(0, "B"))

    def test_existing_unallocated_or_malformed_native_receipts_fail_closed(self):
        self.lua.execute("native_payment(0,0)")
        self.assertEqual(self.rules.Request(0, "C"), "UNEXPECTED_NATIVE_PAYMENT")
        self.assertIsNone(self.rules.Queue(0))
        self.lua.execute("receipts[0]={}; Players[0].spent=0")
        self.rules.Request(0, "C")
        self.rules.Begin(0)
        self.lua.execute("receipts[0]['JAC_PROXY_NATIVE_PAID_0']=2; Players[0].spent=1")
        self.assertEqual(self.rules.Advance(0), "INVALID_RECEIPT")
        self.assertFalse(self.rules.Paid(0, "C"))

    def test_same_official_spending_cannot_pay_later_concurrent_requests(self):
        for leader in ("B", "C", "A"):
            self.rules.Request(0, leader)
        for stage in range(3):
            self.lua.globals().native_payment(0, stage)
            self.rules.Advance(0)
        self.rules.Begin(0)
        self.assertEqual(self.lua.globals().authorize(0), "AUTHORIZED")
        self.lua.execute("receipts[0]['JAC_PROXY_NATIVE_PAID_3']=1")
        self.assertEqual(self.rules.Advance(0), "ACCOUNTING_PENDING")
        self.assertEqual(self.rules.Queue(0)["cursor"], 3)
        self.assertFalse(self.rules.Paid(0, "C"))
        self.assertFalse(self.rules.Paid(0, "A"))

    def test_late_native_receipt_after_cancellation_stays_with_the_original_person(self):
        self.rules.Request(0, "A")
        self.rules.Request(0, "C")
        self.assertEqual(self.rules.Begin(0), "STARTED")
        self.assertEqual(self.lua.globals().authorize(0), "AUTHORIZED")
        self.lua.execute("Players[0].spent=1; PERSON_EXISTS.A=false")
        self.assertEqual(self.rules.Reconcile(0), "CANCELLED")
        self.assertIsNone(self.rules.Next(0))
        self.assertEqual(self.rules.Queue(0)["flight"]["request"], 1)
        self.assertEqual(self.rules.Advance(0), "NATIVE_PENDING")
        self.lua.execute(RULES)
        self.rules = self.lua.globals().JAC_TitlePaymentRules
        self.lua.execute("receipts[0]['JAC_PROXY_NATIVE_PAID_0']=1")
        self.assertEqual(self.rules.Advance(0), "CANCELLED_PAYMENT")
        self.assertEqual(list(self.rules.Queue(0)["requests"][1]["payments"].values()), [0])
        self.assertFalse(self.rules.Paid(0, "C"))
        self.assertEqual(self.rules.Next(0)["leader"], "C")
        self.assertEqual(self.rules.Next(0)["stage"], 1)
        self.lua.globals().native_payment(0, 1)
        self.assertEqual(self.rules.Advance(0), "PAID")
        self.assertEqual(list(self.rules.Queue(0)["requests"][2]["payments"].values()), [1])

    def test_cancelled_person_does_not_report_pending_on_replay(self):
        self.rules.Request(0, "A")
        self.lua.execute("PERSON_EXISTS.A=false")
        self.rules.Reconcile(0)
        self.assertEqual(self.rules.Request(0, "A"), "CANCELLED")

    def test_lost_person_releases_unpaid_reservation_and_keeps_consumed_stages(self):
        self.rules.Request(0, "B")
        self.rules.Request(0, "A")
        self.lua.globals().native_payment(0, 0)
        self.rules.Advance(0)
        self.lua.execute("PERSON_EXISTS.B=false")
        self.assertIsNone(self.rules.Next(0))
        self.assertEqual(self.rules.Reconcile(0), "CANCELLED")
        self.assertEqual(self.rules.Advance(0), "NO_STARTED_STAGE")
        old = self.rules.Queue(0)["requests"][1]
        self.assertEqual(old["status"], "CANCELLED")
        self.assertEqual(list(old["payments"].values()), [0])
        self.assertEqual(self.rules.Next(0)["leader"], "A")
        self.assertEqual(self.rules.Next(0)["stage"], 1)
        self.assertFalse(self.rules.Paid(0, "B"))

    def test_owner_change_after_native_consumption_cannot_credit_the_following_person(self):
        self.rules.Request(0, "C")
        self.rules.Request(0, "A")
        self.lua.globals().native_payment(0, 0)
        self.lua.execute("properties['JAC_PERSONA_C_OWNER']=1")
        self.assertIsNone(self.rules.Next(0))
        self.assertEqual(self.rules.Reconcile(0), "CANCELLED")
        self.assertEqual(self.rules.Advance(0), "CANCELLED_PAYMENT")
        self.assertEqual(self.rules.Queue(0)["cursor"], 1)
        self.assertEqual(self.rules.Queue(0)["spentFloor"], 1)
        self.assertEqual(self.rules.Next(0)["stage"], 1)
        self.assertFalse(self.rules.Paid(0, "C"))

    def test_reload_after_native_consumption_before_allocation_does_not_charge_again(self):
        self.rules.Request(0, "C")
        self.lua.globals().native_payment(0, 0)
        self.lua.execute(RULES)
        self.rules = self.lua.globals().JAC_TitlePaymentRules
        self.assertEqual(self.rules.Queue(0)["cursor"], 0)
        self.assertEqual(self.rules.Advance(0), "PAID")
        self.assertEqual(self.rules.Advance(0), "NO_STARTED_STAGE")
        self.assertEqual(self.lua.globals().Players[0]["spent"], 1)


class TitleProxyGameplayIntegrationTests(unittest.TestCase):
    def setUp(self):
        self.lua = LuaRuntime()
        self.lua.execute(SETUP)
        self.lua.execute(RULES)
        self.lua.execute("""
            include=function() end; print=function() end
            ExposedMembers, Events = {}, {}
            CALLBACKS={}; GameEvents=setmetatable({}, {__index=function(_,name)
                return {Add=function(callback) CALLBACKS[name]=callback end}
            end})
            GameInfo.GreatPersonIndividuals={PERSON_A={GreatPersonClassType='GP_CLASS'},
                [2001]={GreatPersonIndividualType='PERSON_A'}}
            GameInfo.GreatPersonClasses={GP_CLASS={UnitType='UNIT_PERSONA'}}
            GameInfo.Units={UNIT_PERSONA={Index=5000}}
            GameInfo.JAC_LeaderPersonas.A.PersonType='PERSON_A'
            HAS_PERSON=true
            Players[0].GetUnits=function() return {Members=function()
                local done=false; return function()
                    if done or not HAS_PERSON then return nil end; done=true
                    return 1,{GetType=function() return 5000 end,GetGreatPerson=function()
                        return {GetIndividual=function() return 2001 end}
                    end}
                end
            end} end
            for id=0,1 do
                local p=Players[id]
                p.SetProperty=function(_,key,value) receipts[id][key]=value end
            end
        """)
        self.lua.execute(GAMEPLAY)

    def test_synchronized_request_preserves_the_native_lookup_multiple_returns(self):
        self.lua.execute("CALLBACKS.JAC_TitleProxyRequest(0,{Leader='A',RequestSerial=7})")
        self.assertEqual(self.lua.eval("Players[0]:GetProperty('JAC_PROXY_REQUEST_RESULT')"), "PENDING")
        self.assertEqual(self.lua.eval("Players[0]:GetProperty('JAC_PROXY_REQUEST_SERIAL')"), 7)
        self.assertEqual(self.lua.eval("Game:GetProperty('JAC_PROXY_PAYMENT_QUEUE_0').requests[1].cost"), 2)

    def test_owner_metadata_without_a_real_person_is_rejected_before_reservation(self):
        self.lua.execute("HAS_PERSON=false; CALLBACKS.JAC_TitleProxyRequest(0,{Leader='A',RequestSerial=8})")
        self.assertEqual(self.lua.eval("Players[0]:GetProperty('JAC_PROXY_REQUEST_RESULT')"), "PERSON_MISSING")
        self.assertIsNone(self.lua.eval("Game:GetProperty('JAC_PROXY_PAYMENT_QUEUE_0')"))

    def test_native_receipt_only_advances_through_the_gameplay_event(self):
        self.lua.execute("CALLBACKS.JAC_TitleProxyRequest(0,{Leader='A',RequestSerial=1}); native_payment(0,0)")
        self.assertFalse(self.lua.eval("ExposedMembers.JAC_TitleProxyPaid(0,'A')"))
        self.lua.execute("CALLBACKS.JAC_TitleProxyAdvance(0)")
        self.assertEqual(self.lua.eval("Players[0]:GetProperty('JAC_PROXY_ADVANCE_RESULT')"), "PARTIAL")
        self.assertEqual(self.lua.eval("Game:GetProperty('JAC_PROXY_PAYMENT_QUEUE_0').cursor"), 1)



if __name__ == "__main__":
    unittest.main()
