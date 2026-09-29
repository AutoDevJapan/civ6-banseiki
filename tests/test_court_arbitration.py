"""Court/shogun ceasefire rulings on the Japan maps (SSOT 3.2): scope, text, and the gameplay Lua under lupa.

``uv run --no-project --with lupa python -m unittest tests.test_court_arbitration``
"""

from __future__ import annotations

import re
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

from tests.test_japan_no_world_congress import MAP_NAMES, MAP_PREFIX

try:
    import lupa
except ImportError:  # pragma: no cover - depends on the environment
    lupa = None

ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_PoC_CourtArbitration"
LANGUAGES = {"ja_JP", "en_US", "ru_RU"}
START_TURN = 16603
# 0 Ouchi, 1 Otomo, 2 Ashikaga, 3 Mori
CIVS = ["CIVILIZATION_JAC_OUCHI", "CIVILIZATION_JAC_OTOMO", "CIVILIZATION_JAC_ASHIKAGA", "CIVILIZATION_JAC_MORI"]


class ScopeAndTextTests(unittest.TestCase):
    def test_every_action_loads_only_on_the_four_banseiki_japan_maps(self):
        mod = ET.parse(MOD / "JAC_PoC_CourtArbitration.modinfo").getroot()
        criteria = {row.attrib["id"]: row for row in mod.findall("./ActionCriteria/Criteria")}
        for kind in ("UpdateText", "AddGameplayScripts", "AddUserInterfaces"):
            selected = set()
            for action in mod.findall(f"./InGameActions/{kind}"):
                criterion = criteria[action.attrib["criteria"]]
                self.assertEqual(criterion.findtext("RuleSetInUse"), "RULESET_JAC_BANSEIKI")
                self.assertEqual(criterion.findtext("ConfigurationValueMatches/ConfigurationId"), "MAP_SCRIPT")
                selected.add(criterion.findtext("ConfigurationValueMatches/Value"))
            self.assertEqual(selected, {MAP_PREFIX + name for name in MAP_NAMES}, kind)
        listed = {f.text for f in mod.findall("./Files/File")}
        for path in listed:
            self.assertTrue((MOD / path).is_file(), path)

    def test_every_text_key_used_by_the_lua_exists_in_three_languages(self):
        rows = ET.parse(MOD / "Text/JAC_CourtArbitration_Text.xml").getroot().findall("./LocalizedText/Row")
        languages: dict[str, set[str]] = {}
        for row in rows:
            languages.setdefault(row.attrib["Tag"], set()).add(row.attrib["Language"])
        for tag, found in languages.items():
            self.assertEqual(found, LANGUAGES, tag)
        lua = "".join((MOD / p).read_text(encoding="utf-8") for p in (
            "UI/JAC_CourtArbitrationPanel.lua", "UI/JAC_CourtArbitrationPanel.xml", "Scripts/JAC_CourtArbitration.lua"))
        used = set(re.findall(r'"(LOC_JAC_ARB_[A-Z_]+)"', lua))
        used -= {"LOC_JAC_ARB_REASON_"}
        rules = (MOD / "Scripts/JAC_CourtArbitration_Rules.lua").read_text(encoding="utf-8")
        used |= {f"LOC_JAC_ARB_REASON_{r}" for r in re.findall(r'return false, "([A-Z_]+)"', rules)}
        self.assertEqual(used - set(languages), set())

    @unittest.skipIf(lupa is None, "lupa is not installed")
    def test_ui_script_compiles(self):
        source = (MOD / "UI/JAC_CourtArbitrationPanel.lua").read_text(encoding="utf-8")
        error = lupa.LuaRuntime().eval("function(s) local _, e = load(s); return e end")(source)
        self.assertIsNone(error)


STUBS = r"""
local tables, civs, startTurn, rulesPath = ...
local function rowsTable(rows)
  return setmetatable({}, { __call = function()
    local i = 0
    return function() i = i + 1; return rows[i] end
  end })
end
GameInfo = {
  StartEras = { ERA_ANCIENT = { Year = tables.StartYear } },
  GameSpeeds = { [0] = { GameSpeedType = "GAMESPEED_STANDARD" } },
  GameSpeed_Turns = rowsTable(tables.GameSpeed_Turns),
}
CURRENT_TURN = startTurn
GameConfiguration = { GetGameSpeedType = function() return 0 end, GetStartTurn = function() return startTurn end }
local function properties()
  local values = {}
  return function(_, k) return values[k] end, function(_, k, v) values[k] = v end
end
Game = { GetCurrentGameTurn = function() return CURRENT_TURN end }
Game.GetProperty, Game.SetProperty = properties()
WAR = {}
function SET_WAR(a, b, v) WAR[math.min(a, b) .. "_" .. math.max(a, b)] = v end
Players, PlayerConfigurations, MAJORS, PEACE_FAILS = {}, {}, {}, {}
for i, civ in ipairs(civs) do
  local id = i - 1
  local p = { favor = 100, gold = 500, cities = 2, human = false, favorFails = false }
  p.GetID = function() return id end
  p.IsMajor = function() return true end
  p.IsAlive = function() return true end
  p.IsHuman = function() return p.human end
  p.GetFavor = function() return p.favor end
  p.ChangeFavor = function(_, n) if p.favorFails then error("no ChangeFavor") end; p.favor = p.favor + n end
  p.GetTreasury = function() return {
    GetGoldBalance = function() return p.gold end,
    ChangeGoldBalance = function(_, n) p.gold = p.gold + n end,
  } end
  p.GetCities = function() return { GetCount = function() return p.cities end } end
  p.GetDiplomacy = function() return {
    IsAtWarWith = function(_, o) return WAR[math.min(id, o) .. "_" .. math.max(id, o)] == true end,
    MakePeaceWith = function(_, o) SET_WAR(id, o, false) end,
  } end
  p.GetProperty, p.SetProperty = properties()
  Players[id] = p
  PlayerConfigurations[id] = {
    GetCivilizationTypeName = function() return civ end,
    GetCivilizationShortDescription = function() return "LOC_" .. civ .. "_NAME" end,
  }
  table.insert(MAJORS, p)
end
PlayerManager = { GetAliveMajors = function() return MAJORS end }
Locale = { Lookup = function(key, ...) return key end }
ParameterTypes = { MESSAGE = "MESSAGE", SUMMARY = "SUMMARY" }
NotificationTypes = { USER_DEFINED_1 = 1 }
NOTES = {}
NotificationManager = { SendNotification = function(id, _, data) table.insert(NOTES, { id, data.MESSAGE }) end }
ExposedMembers = {}
unpack = unpack or table.unpack
LOG = {}
function print(line) LOG[#LOG + 1] = line end
function include(name) dofile(rulesPath) end
local handlers = {}
GameEvents = setmetatable({}, { __index = function(t, name)
  local list = {}
  local ev = { Add = function(f) table.insert(list, f) end, Call = function(...) for _, f in ipairs(list) do f(...) end end }
  rawset(t, name, ev)
  return ev
end })
function START_TURN(turn)
  CURRENT_TURN = startTurn + turn - 1
  GameEvents.OnGameTurnStarted.Call(CURRENT_TURN)
end
function REQUEST(player, a, b) GameEvents.JAC_CourtArbitrationRequest.Call(player, { A = a, B = b, OnStart = "x" }) end
function RESPOND(player, accept) GameEvents.JAC_CourtArbitrationRespond.Call(player, { Accept = accept }) end
"""


def calendar() -> dict:
    root = ET.parse(ROOT / "poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Calendar.xml").getroot()
    return {
        "StartYear": int(root.findtext("./StartEras/Update/Set/Year")),
        "GameSpeed_Turns": [
            {"GameSpeedType": r.findtext("GameSpeedType"), "TurnsPerIncrement": int(r.findtext("TurnsPerIncrement")),
             "MonthIncrement": int(r.findtext("MonthIncrement"))}
            for r in root.findall("./GameSpeed_Turns/Row")],
    }


@unittest.skipIf(lupa is None, "lupa is not installed")
class ArbitrationTurnTests(unittest.TestCase):
    OUCHI, OTOMO, ASHIKAGA, MORI = 0, 1, 2, 3

    def setUp(self):
        self.lua = lupa.LuaRuntime(unpack_returned_tuples=True)
        rules = (MOD / "Scripts/JAC_CourtArbitration_Rules.lua").as_posix()
        self.lua.execute(STUBS, self.lua.table_from(calendar(), recursive=True), self.lua.table_from(CIVS),
                         START_TURN, rules)
        self.lua.execute((MOD / "Scripts/JAC_CourtArbitration.lua").read_text(encoding="utf-8"))
        self.g = self.lua.globals()
        self.g.SET_WAR(self.OUCHI, self.OTOMO, True)  # R01, at war from 1534
        self.turn = 1
        self.g.START_TURN(1)

    def next_turn(self, to: int | None = None):
        self.turn = to or self.turn + 1
        self.g.START_TURN(self.turn)

    def at_war(self, a, b) -> bool:
        return self.g.Players[a].GetDiplomacy().IsAtWarWith(None, b)

    def game(self, key):
        return self.g.Game.GetProperty(None, key)

    def player(self, pid, key):
        return self.g.Players[pid].GetProperty(None, key)

    def log(self) -> list[str]:
        return list(self.g.LOG.values())

    def test_muromachi_end_is_1573_month_7_on_the_sengoku_calendar(self):
        # Standard: 4 months per turn from January 1534; month 7 of 1573 is month 474 -> turn 1 + ceil(474/4) = 120.
        R = self.g.JAC_ARB
        self.assertEqual(R.ShogunateEndTurn(), 120)
        self.assertTrue(R.IsEmpoweredShogun(self.ASHIKAGA, 119))
        self.assertFalse(R.IsEmpoweredShogun(self.ASHIKAGA, 120))
        self.assertFalse(R.IsEmpoweredShogun(self.OUCHI, 1))
        self.assertIn("[JAC_ARB] loaded shogunate_end_turn=120", self.log())

    def test_party_request_charges_the_cost_and_an_even_ai_accepts_next_turn(self):
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        ouchi = self.g.Players[self.OUCHI]
        self.assertEqual((ouchi.favor, ouchi.gold, self.player(self.OUCHI, "JAC_LEGITIMACY")), (80, 400, 45))
        pending = self.game("JAC_ARB_PENDING")
        self.assertEqual((pending.Requester, pending.A, pending.B, pending.Power), (0, 0, 1, 1))
        self.assertEqual(list(self.g.NOTES[1].values()), [self.OTOMO, "LOC_JAC_ARB_NOTIFY_REQUEST_NAME"])
        self.assertEqual(len(self.g.NOTES), 1)  # only the respondent
        # One ruling per turn: a second request (even the other way round) is refused and not charged.
        self.g.REQUEST(self.OTOMO, self.OTOMO, self.OUCHI)
        self.assertEqual(self.g.Players[self.OTOMO].favor, 100)
        self.assertIn("[JAC_ARB] rejected request player=1 pair=1-0 reason=ONE_PER_TURN", self.log())
        self.next_turn()
        self.assertFalse(self.at_war(self.OUCHI, self.OTOMO))
        self.assertEqual(self.game("JAC_ARB_TRUCE_0_1"), 2 + 10)
        self.assertIsNone(self.game("JAC_ARB_PENDING"))
        self.assertIn("[JAC_ARB] decision player=1 accept=true by=ai", self.log())

    def test_the_other_party_may_be_the_requester(self):
        self.g.REQUEST(self.OTOMO, self.OUCHI, self.OTOMO)
        pending = self.game("JAC_ARB_PENDING")
        self.assertEqual((pending.A, pending.B), (1, 0))  # A is always the requesting party

    def test_ashikaga_calls_a_ceasefire_between_others_with_triple_power_until_1573(self):
        self.g.REQUEST(self.ASHIKAGA, self.OUCHI, self.OTOMO)
        pending = self.game("JAC_ARB_PENDING")
        self.assertEqual((pending.Requester, pending.A, pending.B, pending.Power), (2, 0, 1, 3))
        self.assertEqual(sorted(n[1] for n in self.g.NOTES.values()), [0, 1])  # both parties answer
        self.next_turn()
        self.assertFalse(self.at_war(self.OUCHI, self.OTOMO))
        self.assertEqual(self.game("JAC_ARB_TRUCE_0_1"), 2 + 30)

    def test_after_1573_ashikaga_survives_but_cannot_call_rulings_for_others(self):
        self.next_turn(120)
        self.g.REQUEST(self.ASHIKAGA, self.OUCHI, self.OTOMO)
        self.assertIsNone(self.game("JAC_ARB_PENDING"))
        self.assertIn("[JAC_ARB] rejected request player=2 pair=0-1 reason=NOT_PARTY", self.log())
        self.assertEqual(self.g.Players[self.ASHIKAGA].favor, 100)
        # As a party it petitions like any house, at normal power.
        self.g.SET_WAR(self.ASHIKAGA, self.MORI, True)
        self.g.REQUEST(self.ASHIKAGA, self.ASHIKAGA, self.MORI)
        self.assertEqual(self.game("JAC_ARB_PENDING").Power, 1)
        self.next_turn()
        self.assertEqual(self.game("JAC_ARB_TRUCE_2_3"), 121 + 10)

    def test_non_party_non_shogun_and_peace_pairs_are_rejected(self):
        self.g.REQUEST(self.MORI, self.OUCHI, self.OTOMO)
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.MORI)
        self.assertIsNone(self.game("JAC_ARB_PENDING"))
        log = self.log()
        self.assertIn("[JAC_ARB] rejected request player=3 pair=0-1 reason=NOT_PARTY", log)
        self.assertIn("[JAC_ARB] rejected request player=0 pair=0-3 reason=NOT_AT_WAR", log)

    def test_cost_shortfalls_reject_without_charging(self):
        ouchi = self.g.Players[self.OUCHI]
        for field, value, reason in (("favor", 19, "FAVOR"), ("gold", 99, "GOLD")):
            ouchi.favor, ouchi.gold = 100, 500
            ouchi[field] = value
            self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
            self.assertIn(f"[JAC_ARB] rejected request player=0 pair=0-1 reason={reason}", self.log())
            self.assertEqual(ouchi[field], value)
        ouchi.favor, ouchi.gold = 100, 500
        ouchi.SetProperty(None, "JAC_LEGITIMACY", 4)
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        self.assertIn("[JAC_ARB] rejected request player=0 pair=0-1 reason=LEGITIMACY", self.log())
        # ChangeFavor missing in the engine: nothing is charged and no request is made.
        ouchi.SetProperty(None, "JAC_LEGITIMACY", 50)
        ouchi.favorFails = True
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        self.assertEqual((ouchi.gold, self.player(self.OUCHI, "JAC_LEGITIMACY")), (500, 50))
        self.assertIsNone(self.game("JAC_ARB_PENDING"))

    def test_a_clearly_winning_ai_refuses_and_is_penalized(self):
        self.g.Players[self.OTOMO].cities = 4  # 4 > 1.5 * 2
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        self.next_turn()
        self.assertTrue(self.at_war(self.OUCHI, self.OTOMO))
        self.assertEqual(self.player(self.OTOMO, "JAC_LEGITIMACY"), 50 - 15)
        self.assertEqual(self.player(self.OTOMO, "JAC_ARB_OUTLAW_UNTIL"), 2 + 10)
        self.assertIsNone(self.player(self.OUCHI, "JAC_ARB_OUTLAW_UNTIL"))
        self.assertIsNone(self.game("JAC_ARB_TRUCE_0_1"))
        # Exactly 1.5x still accepts.
        self.g.Players[self.OTOMO].cities = 3
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        self.next_turn()
        self.assertFalse(self.at_war(self.OUCHI, self.OTOMO))

    def test_human_respondent_answers_or_is_counted_as_refusing(self):
        self.g.Players[self.OTOMO].human = True
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        self.next_turn()  # no answer
        self.assertTrue(self.at_war(self.OUCHI, self.OTOMO))
        self.assertIn("[JAC_ARB] decision player=1 accept=false by=no_answer", self.log())
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        self.g.RESPOND(self.MORI, 1)  # not a respondent
        self.g.RESPOND(self.OUCHI, 1)  # the requester does not answer
        self.g.RESPOND(self.OTOMO, 1)
        self.g.RESPOND(self.OTOMO, 0)  # a second answer is ignored
        self.assertEqual(self.log().count("[JAC_ARB] ignored answer player=1"), 1)
        self.next_turn()
        self.assertFalse(self.at_war(self.OUCHI, self.OTOMO))
        self.assertIn("[JAC_ARB] decision player=1 accept=true by=answer", self.log())

    def test_war_during_the_truce_is_undone_and_the_truce_ends_on_time(self):
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        self.next_turn()  # turn 2, truce until 12
        self.g.SET_WAR(self.OUCHI, self.OTOMO, True)
        self.next_turn()
        self.assertFalse(self.at_war(self.OUCHI, self.OTOMO))
        self.assertIn("[JAC_ARB] truce reimposed pair=0-1 turn=3 until=12 ok=true", self.log())
        self.next_turn(13)
        self.assertIsNone(self.game("JAC_ARB_TRUCE_0_1"))
        self.g.SET_WAR(self.OUCHI, self.OTOMO, True)
        self.next_turn()
        self.assertTrue(self.at_war(self.OUCHI, self.OTOMO))

    def test_a_request_is_void_if_the_war_ended_before_the_ruling(self):
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        self.g.SET_WAR(self.OUCHI, self.OTOMO, False)
        self.next_turn()
        self.assertIn("[JAC_ARB] void request pair=0-1 turn=2 (no longer at war)", self.log())
        self.assertIsNone(self.game("JAC_ARB_TRUCE_0_1"))


if __name__ == "__main__":
    unittest.main()
