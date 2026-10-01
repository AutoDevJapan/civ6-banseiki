"""Real official-schema regressions and diagnostic rejection tests.

Lua fixtures test the observer/assertions only. They do not demonstrate native
modifier activation, AI abstention, establishment or actual save/load behavior.
"""
import importlib.util
import json
from pathlib import Path
import sqlite3
import unittest
import xml.etree.ElementTree as ET

from lupa import LuaRuntime

ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_Test_NativeSlots"
PERSONA = ROOT / "poc/JAC_PoC_LeaderPersona"
SCENARIOS = ROOT / "tools/civ6-autotest/scenarios"
PRIMARY = SCENARIOS / "leader-persona-native-slots-15km.json"
CLASSIFICATION = SCENARIOS / "leader-persona-native-slots-activation-15km.json"
spec = importlib.util.spec_from_file_location("native_slots_official_audit", ROOT / "tools/sengoku_cascade_audit.py")
AUDIT = importlib.util.module_from_spec(spec)
spec.loader.exec_module(AUDIT)


@unittest.skipUnless(AUDIT.available(), "official Civ6 schema/data not installed")
class NativeSlotsOfficialDatabaseTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        original = AUDIT.official_db()
        for name in ("JAC_CustomCivs_Gameplay.sql", "JAC_Japan1534_Gameplay.sql"):
            original.executescript((ROOT / "poc/JAC_PoC_CustomCivs/Data" / name).read_text(encoding="utf-8"))
        cls.db = AUDIT._run_sengoku(original, True)
        cls.before_fk = set(cls.db.execute("PRAGMA foreign_key_check"))
        for name in ("JAC_LeaderPersona.sql", "JAC_LeaderPersona_Trees.sql", "JAC_TitleProxy_Probe.sql"):
            cls.db.executescript((PERSONA / "Data" / name).read_text(encoding="utf-8"))
        cls.proxy_receipts = cls.db.execute("SELECT * FROM JAC_TitleProxyReceipts ORDER BY Step").fetchall()
        cls.proxy_governor = cls.db.execute("SELECT * FROM Governors WHERE GovernorType='GOVERNOR_JAC_TITLE_PROXY'").fetchall()
        cls.db.executescript((MOD / "Data/JAC_NativeSlots_Probe.sql").read_text(encoding="utf-8"))

    def test_new_slot_introduces_no_official_foreign_key_violation(self):
        self.assertEqual(self.db.execute("PRAGMA foreign_keys").fetchone(), (1,))
        self.assertEqual(set(self.db.execute("PRAGMA foreign_key_check")) - self.before_fk, set())
        # A misspelled registration really is rejected by the official FK.
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO GovernorPromotionSets VALUES ('GOVERNOR_JAC_NATIVE_SLOT','MISSING_BASE')")

    def test_22_people_remain_real_people_and_not_native_governor_candidates(self):
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM JAC_LeaderPersonas").fetchone(), (22,))
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM Governors JOIN JAC_LeaderPersonas USING (GovernorType)").fetchone(), (0,))
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM Types JOIN JAC_LeaderPersonas ON Type=GovernorType WHERE Kind='KIND_GOVERNOR'").fetchone(), (0,))
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM GreatPersonIndividuals g JOIN JAC_LeaderPersonas p ON g.GreatPersonIndividualType=p.PersonType").fetchone(), (22,))

    def test_slot_preserves_ordinary_loyalty_and_timing_without_static_persona_identity(self):
        self.assertEqual(self.db.execute("SELECT IdentityPressure,TransitionStrength,AssignCityState,TraitType FROM Governors WHERE GovernorType='GOVERNOR_JAC_NATIVE_SLOT'").fetchall(), [(8, 100, 0, None)])
        self.assertEqual(self.db.execute("SELECT Value FROM GlobalParameters WHERE Name='GOVERNOR_BASE_TURNS_TO_ESTABLISH'").fetchone(), ("5",))
        self.assertEqual(self.db.execute("SELECT * FROM GovernorsCannotAssign WHERE GovernorType='GOVERNOR_JAC_NATIVE_SLOT'").fetchall(), [])
        self.assertEqual(self.db.execute("SELECT * FROM GovernorModifiers WHERE GovernorType='GOVERNOR_JAC_NATIVE_SLOT'").fetchall(), [])
        self.assertEqual(self.db.execute("SELECT GovernorPromotion FROM GovernorPromotionSets WHERE GovernorType='GOVERNOR_JAC_NATIVE_SLOT'").fetchall(), [("GOVERNOR_PROMOTION_JAC_NATIVE_SLOT_BASE",)])
        self.assertEqual(self.db.execute("SELECT BaseAbility,Level FROM GovernorPromotions WHERE GovernorPromotionType='GOVERNOR_PROMOTION_JAC_NATIVE_SLOT_BASE'").fetchone(), (1, 0))

    def test_receipts_reach_the_base_promotion_with_real_official_collection_and_effect(self):
        rows = self.db.execute("SELECT m.ModifierId,m.ModifierType,d.CollectionType,d.EffectType,m.RunOnce,m.Permanent,m.OwnerRequirementSetId,m.SubjectRequirementSetId FROM GovernorPromotionModifiers p JOIN Modifiers m USING (ModifierId) JOIN DynamicModifiers d USING (ModifierType) WHERE p.GovernorPromotionType='GOVERNOR_PROMOTION_JAC_NATIVE_SLOT_BASE' ORDER BY m.ModifierId").fetchall()
        self.assertEqual(rows, [
            ("JAC_NATIVE_SLOT_CITY_ACTIVE", "MODIFIER_SINGLE_CITY_ADJUST_PROPERTY", "COLLECTION_OWNER", "EFFECT_ADJUST_CITY_PROPERTY", 0, 0, None, None),
            ("JAC_NATIVE_SLOT_PLAYER_RECEIPT", "MODIFIER_PLAYER_ADJUST_PROPERTY", "COLLECTION_OWNER", "EFFECT_ADJUST_PLAYER_PROPERTY", 0, 0, None, None),
        ])
        for modifier, *_ in rows:
            self.assertEqual(dict(self.db.execute("SELECT Name,Value FROM ModifierArguments WHERE ModifierId=?", (modifier,))), {"Key": modifier, "Amount": "1"})

    def test_existing_payment_candidate_is_unchanged(self):
        self.assertEqual(self.db.execute("SELECT * FROM JAC_TitleProxyReceipts ORDER BY Step").fetchall(), self.proxy_receipts)
        self.assertEqual(len(self.proxy_receipts), 66)
        self.assertEqual(self.db.execute("SELECT * FROM Governors WHERE GovernorType='GOVERNOR_JAC_TITLE_PROXY'").fetchall(), self.proxy_governor)


LUA_SETUP = """
ContextPtr={SetHide=function() end}
print=function() end
ExposedMembers={}
GameEvents={JAC_NativeSlotsGiveTitles={Add=function(fn) GIVE_TITLES=fn end}}
GameConfiguration={IsAnyMultiplayer=function() return MULTIPLAYER==true end}
props, playerProps, cityProps, appointed = {}, {}, {}, {}
turn=1
Game={GetLocalPlayer=function() return 0 end, GetCurrentGameTurn=function() return turn end,
 GetProperty=function(_,key) return props[key] end, SetProperty=function(_,key,value) props[key]=value end}
UI={IsGameCoreBusy=function() return false end}
GameInfo={Governors={},GovernorsCannotAssign={},Types={}}
for i,kind in ipairs({'GOVERNOR_JAC_NATIVE_SLOT','GOVERNOR_JAC_TITLE_PROXY'}) do
 GameInfo.Governors[kind]={Hash=kind,Index=i,IdentityPressure=8,TransitionStrength=100,AssignCityState=false}
end
local people={}
for i=1,22 do people[i]={LeaderType='LEADER_FIXTURE_'..i,GovernorType='PERSONA_FIXTURE_'..i} end
GameInfo.JAC_LeaderPersonas=function()
 local i=0; return function() i=i+1; return people[i] end
end
Players,alive={},{}
for id=0,21 do
 local player={points=3,spent=0}
 playerProps[id]={}; appointed[id]={}
 player.GetID=function() return id end
 player.IsHuman=function() return id==0 end
 player.IsTurnActive=function() return id==0 end
 player.GetProperty=function(_,key) return playerProps[id][key] end
 player.GetGovernors=function() return {
  GetGovernorPoints=function() return player.points end,
  GetGovernorPointsSpent=function() return player.spent end,
  HasGovernor=function(_,kind) return appointed[id][kind]~=nil end,
  GetGovernor=function(_,kind) return appointed[id][kind] end
 } end
 player.GetCities=function() return {Members=function()
  local done=false; return function()
   if not done then done=true; return 42,{
    GetOwner=function() return id end,GetID=function() return 42 end,
    GetProperty=function(_,key) return cityProps[key] end
   } end
  end
 end} end
 Players[id]=player; table.insert(alive,player)
 props['JAC_SLOTS_PROBE_BUDGET_'..id]={points=3,spent=0}
end
PlayerManager={GetAliveMajors=function() return alive end}
props.JAC_SLOTS_PROBE_AI_START_TURN=0
function fixtureGovernor()
 return {GetAssignedCity=function() return nil end,IsEstablished=function() return false end,
 GetTurnsToEstablish=function() return 5 end,GetTurnsOnSite=function() return 0 end,
 GetNeutralizedTurns=function() return 0 end}
end
"""


class NativeSlotsDiagnosticRejectionTests(unittest.TestCase):
    def setUp(self):
        self.lua = LuaRuntime()
        self.lua.execute(LUA_SETUP)
        self.lua.execute((MOD / "Scripts/JAC_NativeSlots_GameCore.lua").read_text(encoding="utf-8"))
        self.lua.execute((MOD / "UI/JAC_NativeSlots_View.lua").read_text(encoding="utf-8"))

    def test_multiplayer_is_rejected_before_diagnostic_state_changes(self):
        self.lua.execute("MULTIPLAYER=true; writes=0; Game.SetProperty=function() writes=writes+1 end")
        with self.assertRaisesRegex(Exception, "single-player diagnostic required"):
            self.lua.globals().JAC_SlotsPrepare()
        self.assertEqual(self.lua.globals().writes, 0)

    def test_numeric_zero_and_false_are_both_valid_non_city_state_flags(self):
        self.lua.execute("GameInfo.GovernorsCannotAssign.GOVERNOR_JAC_TITLE_PROXY={CannotAssign=true}")
        self.assertTrue(self.lua.globals().JAC_SlotsPrepare())
        self.lua.execute("GameInfo.Governors.GOVERNOR_JAC_NATIVE_SLOT.AssignCityState=0")
        self.assertTrue(self.lua.globals().JAC_SlotsPrepare())

    def test_core_observer_preserves_missing_receipts_instead_of_inventing_payment(self):
        seen = self.lua.globals().ExposedMembers.JAC_NativeSlotsRead(0)
        self.assertEqual((seen.points, seen.spent), (3, 0))
        self.assertIsNone(seen.slotReceipt)
        self.assertIsNone(seen.proxyReceipt)
        self.assertEqual(len(seen.cities), 1)
        self.assertIsNone(seen.cities[1].receipt)
        self.lua.execute("playerProps[0].JAC_NATIVE_SLOT_PLAYER_RECEIPT=7; cityProps.JAC_NATIVE_SLOT_CITY_ACTIVE=2")
        seen = self.lua.globals().ExposedMembers.JAC_NativeSlotsRead(0)
        self.assertEqual((seen.slotReceipt, seen.cities[1].receipt), (7, 2))

    def test_native_appointment_and_spending_without_receipt_fail_the_unassigned_gate(self):
        self.lua.execute("appointed[0].GOVERNOR_JAC_NATIVE_SLOT=fixtureGovernor(); appointed[0].GOVERNOR_JAC_TITLE_PROXY=fixtureGovernor(); Players[0].spent=2; playerProps[0].JAC_PROXY_NATIVE_PAID_0=1")
        self.lua.globals().JAC_SlotsUnassigned("UNASSIGNED", 2)
        with self.assertRaisesRegex(Exception, "UNASSIGNED_RECEIPT_MISSING.*LATER_ACTIVATION_UNOBSERVED"):
            self.lua.globals().JAC_SlotsRequireUnassignedReceipt()

    def test_later_activation_does_not_retroactively_pass_the_original_gate(self):
        self.lua.execute("props.JAC_SLOTS_PROBE_UNASSIGNED={proxyReceipt=1}; props.JAC_SLOTS_PROBE_ASSIGNED={}; props.JAC_SLOTS_PROBE_SETTLED={slotReceipt=1}")
        with self.assertRaisesRegex(Exception, "UNASSIGNED_RECEIPT_MISSING.*SLOT_RECEIPT_AFTER_ESTABLISHMENT"):
            self.lua.globals().JAC_SlotsRequireUnassignedReceipt()

    def test_positive_titles_do_not_excuse_an_unmatched_ai_appointment(self):
        self.lua.execute("appointed[7].GOVERNOR_JAC_NATIVE_SLOT=fixtureGovernor()")
        with self.assertRaisesRegex(Exception, "unauthorized spending; AI=7"):
            self.lua.globals().JAC_SlotsAssertAI(True)

    def test_ai_spending_without_an_observable_governor_is_also_rejected(self):
        self.lua.execute("Players[11].points=4; Players[11].spent=1")
        with self.assertRaisesRegex(Exception, "unauthorized spending; AI=11"):
            self.lua.globals().JAC_SlotsAssertAI(True)

    def test_zero_ai_titles_or_20_ai_cannot_pass_the_native_gate(self):
        self.lua.execute("Players[9].points=0")
        with self.assertRaisesRegex(Exception, "AI=9"):
            self.lua.globals().JAC_SlotsAssertAI(True)
        self.lua.execute("Players[9].points=3; table.remove(alive)")
        with self.assertRaisesRegex(Exception, "21 AI inventory changed"):
            self.lua.globals().JAC_SlotsAssertAI(True)

    def test_core_receipts_are_never_written_by_the_diagnostic_scripts(self):
        for file in (MOD / "Scripts/JAC_NativeSlots_GameCore.lua", MOD / "UI/JAC_NativeSlots_View.lua"):
            text = file.read_text(encoding="utf-8")
            self.assertNotIn(":SetProperty('JAC_NATIVE_SLOT_", text)
            self.assertNotIn(":SetProperty('JAC_PROXY_NATIVE_PAID_", text)
        self.assertNotIn("SetProperty", (MOD / "Scripts/JAC_NativeSlots_GameCore.lua").read_text(encoding="utf-8").split("GameEvents.JAC_NativeSlotsGiveTitles.Add",1)[0])


class NativeSlotsScenarioTests(unittest.TestCase):
    def test_primary_checks_first_ai_turn_and_unassigned_receipt_before_later_work(self):
        data = json.loads(PRIMARY.read_text(encoding="utf-8"))
        steps = data["steps"]
        self.assertTrue(data["strict_lua_errors"])
        self.assertEqual(steps[6], {"action": "end_turn", "turns": 1, "timeout": 180})
        self.assertEqual(steps[8]["code"], "return JAC_SlotsAssertAI(true)")
        self.assertEqual(steps[15]["code"], "return JAC_SlotsRequireUnassignedReceipt()")
        self.assertFalse(any(step["action"] in ("save", "autoplay") or "Appoint(" in step.get("code", "") for step in steps[:9]))
        self.assertEqual([s["turns"] for s in steps if s["action"] == "end_turn"], [1, 4, 1])
        self.assertEqual([s["action"] for s in steps].count("save"), 2)
        self.assertEqual([s["action"] for s in steps].count("load"), 2)
        self.assertEqual(data["diagnostic_expense"], {"slot_appointment": 1, "proxy_comparison_appointment": 1, "persona_evolution_complete": False})

    def test_classification_keeps_ai_gate_and_rejects_missing_original_receipt(self):
        primary = json.loads(PRIMARY.read_text(encoding="utf-8"))
        data = json.loads(CLASSIFICATION.read_text(encoding="utf-8"))
        self.assertEqual(data["steps"][:15], primary["steps"][:15])
        self.assertEqual(data["steps"][-1]["code"], "return JAC_SlotsRequireUnassignedReceipt()")
        self.assertIn("classification ONLY", data["name"])
        self.assertNotIn("autoplay", [s["action"] for s in data["steps"]])

    def test_new_mod_loads_only_when_enabled_on_the_diagnostic_map_after_persona(self):
        meta = ET.parse(MOD / "JAC_Test_NativeSlots.modinfo").getroot()
        enabled = json.loads(PRIMARY.read_text(encoding="utf-8"))["steps"][0]["ids"]
        self.assertIn(meta.attrib["id"], enabled)
        self.assertIn("91cf7b65-54bb-4f69-a452-19a65527cde2", [d.attrib["id"] for d in meta.findall("./Dependencies/Mod")])
        self.assertEqual(meta.findtext("./ActionCriteria/Criteria/ConfigurationValueMatches/Value"), "{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map")
        files = {f.text for f in meta.findall("./Files/File")}
        self.assertEqual(len(files), 5)
        for f in files:
            self.assertTrue((MOD / f).is_file(), f)
        for action in meta.find("InGameActions"):
            self.assertEqual(action.attrib["criteria"], "JAC_NativeSlots_15km")
        self.assertEqual(meta.findtext("./InGameActions/UpdateDatabase/Properties/LoadOrder"), "450")

    def test_all_lua_files_and_both_scenario_fragments_compile_without_executing_game_api(self):
        lua = LuaRuntime()
        compile_only = lua.eval("function(code,name) local fn,err=load(code,name); assert(fn,err); return true end")
        for file in MOD.rglob("*.lua"):
            self.assertTrue(compile_only(file.read_text(encoding="utf-8"), file.name))
        for file in (PRIMARY, CLASSIFICATION):
            for number, step in enumerate(json.loads(file.read_text(encoding="utf-8"))["steps"], 1):
                code = step.get("code", step.get("condition", {}).get("code"))
                if code:
                    self.assertTrue(compile_only(code, f"{file.name}:{number}"))


if __name__ == "__main__":
    unittest.main()
