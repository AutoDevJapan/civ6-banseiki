"""Court/shogun ceasefire rulings on the Japan maps (SSOT 3.2): scope, text, and the gameplay Lua under lupa.

``uv run --no-project --with lupa python -m unittest tests.test_court_arbitration``
"""

from __future__ import annotations

import json
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
        for kind in ("UpdateDatabase", "UpdateText", "AddGameplayScripts", "AddUserInterfaces"):
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

    def test_the_modifiers_read_the_flags_the_script_writes(self):
        # Owner decisions R-1 / R-2 / S: +10% / -10% gold and culture per city and the participants' war weariness, gated by 0/1 plot flags.
        sql = (MOD / "Data/JAC_CourtArbitration_Legitimacy.sql").read_text(encoding="utf-8")
        rules = (MOD / "Scripts/JAC_CourtArbitration_Rules.lua").read_text(encoding="utf-8")
        flags = dict(re.findall(r'R\.(FLAG_[A-Z]+) = "([A-Z_]+)"', rules))
        self.assertEqual(set(flags), {"FLAG_HIGH", "FLAG_LOW", "FLAG_SUBJUGATION"})
        for name in flags.values():
            self.assertIn(f"'PropertyName', '{name}'", sql)
        rows = dict(re.findall(r"\('(JAC_ARB_[A-Z_]+)', '(MODIFIER_[A-Z_]+)'", sql))
        self.assertNotIn("IDENTITY", sql)  # loyalty is pinned by the Japan maps' +100 stabilization (question S)
        yields = [f"JAC_ARB_{hl}_{y}" for hl in ("HIGH", "LOW") for y in ("GOLD", "CULTURE")]
        for modifier in yields:
            self.assertEqual(rows[modifier], "MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_MODIFIER")
            hl, y = modifier.split("_")[2:]
            self.assertIn(f"'MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_MODIFIER', NULL, 'JAC_ARB_CITY_LEGITIMACY_{hl}')", sql)
            self.assertIn(f"('{modifier}', 'YieldType', 'YIELD_{y}')", sql)
            self.assertIn(f"('{modifier}', 'Amount', '{'10' if hl == 'HIGH' else '-10'}')", sql)
        self.assertEqual(rows["JAC_ARB_WAR_WEARINESS"], "MODIFIER_PLAYER_ADJUST_WAR_WEARINESS")
        self.assertIn("('JAC_ARB_WAR_WEARINESS', 'MODIFIER_PLAYER_ADJUST_WAR_WEARINESS', 'JAC_ARB_CITY_SUBJUGATION_ANCHOR', NULL)", sql)
        for modifier in yields + ["JAC_ARB_ATTACH_WAR_WEARINESS"]:
            self.assertIn(f"('TRAIT_LEADER_MAJOR_CIV', '{modifier}')", sql)
        script = (MOD / "Scripts/JAC_CourtArbitration.lua").read_text(encoding="utf-8")
        self.assertNotRegex(rules + script, r"GetFavor|ChangeFavor")  # legitimacy replaces Favor on the Japan maps

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
  Yields = { YIELD_GOLD = { Index = 2 }, YIELD_CULTURE = { Index = 4 } },
}
CURRENT_TURN = startTurn
GameConfiguration = { GetGameSpeedType = function() return 0 end, GetStartTurn = function() return startTurn end }
local function properties()
  local values = {}
  return function(_, k) return values[k] end, function(_, k, v) values[k] = v end
end
Game = { GetCurrentGameTurn = function() return CURRENT_TURN end }
Game.GetProperty, Game.SetProperty = properties()
WAR, WAR_CHANGED = {}, {}
-- The engine's minimum war length (DIPLOMACY_WAR_MIN_TURNS): MakePeaceWith(b) silently does nothing before it
-- (court-arbitration-05); MakePeaceWith(b, true) makes peace at once (probe-make-peace, court-arbitration-06).
-- 0 = peace is always possible. FORCE_IGNORED simulates an engine where the second argument does nothing,
-- to exercise the script's retry-each-turn safety net.
MIN_WAR, FORCE_IGNORED = 0, false
local function pair(a, b) return math.min(a, b) .. "_" .. math.max(a, b) end
function SET_WAR(a, b, v) WAR[pair(a, b)] = v; WAR_CHANGED[pair(a, b)] = CURRENT_TURN end
local function canPeace(a, b) return CURRENT_TURN - (WAR_CHANGED[pair(a, b)] or 0) >= MIN_WAR end
-- Plots are keyed by x (y is always 0). A house's k-th city stands at x = 100 * id + k, so the plot's default
-- owner is floor(x / 100); PLOT_OWNER overrides it (a lost home). Houses are 99 tiles apart unless NEAR["a_b"].
PLOTS, PLOT_OWNER, NEAR = {}, {}, {}
local function plot(x)
  local p = PLOTS[x]
  if p == nil then
    p = {}
    p.GetIndex = function() return x end
    p.GetOwner = function() return PLOT_OWNER[x] or math.floor(x / 100) end
    p.GetProperty, p.SetProperty = properties()
    PLOTS[x] = p
  end
  return p
end
Map = {
  GetPlot = function(x, y) return plot(x) end,
  GetPlotByIndex = function(i) return plot(i) end,
  GetPlotDistance = function(x1, y1, x2, y2)
    local a, b = math.floor(x1 / 100), math.floor(x2 / 100)
    return NEAR[math.min(a, b) .. "_" .. math.max(a, b)] and 1 or 99
  end,
}
function FLAG(x, key) return plot(x):GetProperty(key) end
Players, PlayerConfigurations, MAJORS = {}, {}, {}
for i, civ in ipairs(civs) do
  local id = i - 1
  local p = { gold = 500, cities = 2, human = false }
  p.GetID = function() return id end
  p.IsMajor = function() return true end
  p.IsAlive = function() return true end
  p.IsHuman = function() return p.human end
  p.GetTreasury = function() return {
    GetGoldBalance = function() return p.gold end,
    ChangeGoldBalance = function(_, n) p.gold = p.gold + n end,
  } end
  local function cityList()
    local list = {}
    for k = 1, p.cities do
      local x = 100 * id + k
      -- Every yield is 10, +10% / -10% while the city's high / low flag is set (Data/JAC_CourtArbitration_Legitimacy.sql).
      local function yield()
        return 10 * (1 + 0.1 * ((plot(x):GetProperty("JAC_ARB_LEGITIMACY_HIGH") or 0) - (plot(x):GetProperty("JAC_ARB_LEGITIMACY_LOW") or 0)))
      end
      list[k] = { GetX = function() return x end, GetY = function() return 0 end, GetYield = yield }
    end
    return list
  end
  p.GetCities = function() return {
    GetCount = function() return p.cities end,
    Members = function() return ipairs(cityList()) end,
    GetCapitalCity = function() return cityList()[1] end,
  } end
  p.GetDiplomacy = function() return {
    IsAtWarWith = function(_, o) return WAR[math.min(id, o) .. "_" .. math.max(id, o)] == true end,
    DeclareWarOn = function(_, o) SET_WAR(id, o, true) end,
    MakePeaceWith = function(_, o, force)
      if (force == true and not FORCE_IGNORED) or canPeace(id, o) then SET_WAR(id, o, false) end
    end,
    CanMakePeaceWith = function(_, o) return canPeace(id, o) end,
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
WarTypes = { FORMAL_WAR = 0 }
NotificationTypes = { USER_DEFINED_1 = 1 }
NOTES = {}
NotificationManager = { SendNotification = function(id, _, data) table.insert(NOTES, { id, data.MESSAGE, data.SUMMARY }) end }
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
function REQUEST(player, a, b) GameEvents.JAC_CourtArbitrationRequest.Call(player, { Target = a, Other = b, OnStart = "x" }) end
function RESPOND(player, accept) GameEvents.JAC_CourtArbitrationRespond.Call(player, { Accept = accept }) end
function JOIN(player, target) GameEvents.JAC_CourtArbitrationJoin.Call(player, { Target = target, OnStart = "x" }) end
-- The engine moves the city, then raises CityConquered(capturer, previous owner, city, x, y) (AlexanderScenario.lua:174).
function CONQUER(capturer, owner, x)
  PLOT_OWNER[x] = capturer
  Players[owner].cities = Players[owner].cities - 1
  Players[capturer].cities = Players[capturer].cities + 1
  GameEvents.CityConquered.Call(capturer, owner, 7, x, 0)
end
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

    def legitimacy(self, pid) -> int:
        return self.g.JAC_ARB.Legitimacy(pid)

    def test_party_request_charges_the_cost_and_an_even_ai_accepts_next_turn(self):
        # Turn 1 income: +1 base, +1 home castle held, +1 at peace with the Shogunate = 53.
        self.assertEqual(self.legitimacy(self.OUCHI), 53)
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        ouchi = self.g.Players[self.OUCHI]
        self.assertEqual((ouchi.gold, self.legitimacy(self.OUCHI)), (400, 53 - 5))
        pending = self.game("JAC_ARB_PENDING")
        self.assertEqual((pending.Requester, pending.A, pending.B, pending.Power), (0, 0, 1, 1))
        self.assertEqual(list(self.g.NOTES[1].values())[:2], [self.OTOMO, "LOC_JAC_ARB_NOTIFY_REQUEST_NAME"])
        self.assertEqual(len(self.g.NOTES), 1)  # only the respondent
        # One ruling per turn: a second request (even the other way round) is refused and not charged.
        self.g.REQUEST(self.OTOMO, self.OTOMO, self.OUCHI)
        self.assertEqual(self.g.Players[self.OTOMO].gold, 500)
        self.assertIn("[JAC_ARB] rejected request player=1 pair=1-0 reason=ONE_PER_TURN", self.log())
        self.next_turn()
        self.assertFalse(self.at_war(self.OUCHI, self.OTOMO))
        self.assertEqual(self.game("JAC_ARB_TRUCE_0_1"), 2 + 10)
        self.assertIsNone(self.game("JAC_ARB_PENDING"))
        self.assertIn("[JAC_ARB] decision player=1 accept=true by=ai", self.log())
        # Accepting raises the respondent's legitimacy by 5 (R-1); the requester only has its income.
        self.assertEqual(self.legitimacy(self.OTOMO), 53 + 3 + 5)
        self.assertEqual(self.legitimacy(self.OUCHI), 48 + 3)

    def test_every_execute_script_is_recorded_at_entry_even_when_rejected(self):
        # The live scenario reports this when a request never reaches the gameplay side (court-arbitration-03).
        self.assertIsNone(self.game("JAC_ARB_LAST_EVENT"))
        self.g.REQUEST(self.MORI, self.OUCHI, self.OTOMO)  # NOT_PARTY
        self.assertEqual(self.game("JAC_ARB_LAST_EVENT"), "request player=3 turn=1")
        self.g.RESPOND(self.OTOMO, 1)  # ignored: nothing pending
        self.assertEqual(self.game("JAC_ARB_LAST_EVENT"), "respond player=1 turn=1")

    def test_the_other_party_may_be_the_requester(self):
        self.g.REQUEST(self.OTOMO, self.OUCHI, self.OTOMO)
        pending = self.game("JAC_ARB_PENDING")
        self.assertEqual((pending.A, pending.B), (1, 0))  # A is always the requesting party

    def test_ashikaga_calls_a_ceasefire_between_others_with_triple_power_until_1573(self):
        self.g.REQUEST(self.ASHIKAGA, self.OUCHI, self.OTOMO)
        pending = self.game("JAC_ARB_PENDING")
        self.assertEqual((pending.Requester, pending.A, pending.B, pending.Power), (2, 0, 1, 3))
        self.assertEqual(sorted(n[1] for n in self.g.NOTES.values()), [0, 1])  # both parties answer
        # Owner decisions O-3 / R-3: the shogun's 3x also divides the cost (rounded up) ...
        ashikaga = self.g.Players[self.ASHIKAGA]
        self.assertEqual((ashikaga.gold, self.legitimacy(self.ASHIKAGA)), (500 - 34, 70 + 3 - 2))
        self.next_turn()
        self.assertFalse(self.at_war(self.OUCHI, self.OTOMO))
        self.assertEqual(self.game("JAC_ARB_TRUCE_0_1"), 2 + 30)

    def test_refusing_a_shogun_ruling_costs_three_times_as_much(self):
        # ... and multiplies the refusal penalty (O-3 / R-3): Legitimacy -45, a 30-turn order to subdue.
        self.g.Players[self.OTOMO].cities = 4  # 4 > 1.5 * 2
        self.g.REQUEST(self.ASHIKAGA, self.OUCHI, self.OTOMO)
        self.next_turn()
        self.assertTrue(self.at_war(self.OUCHI, self.OTOMO))
        self.assertEqual(self.legitimacy(self.OTOMO), 53 + 3 - 45)
        self.assertEqual(self.player(self.OTOMO, "JAC_ARB_OUTLAW_UNTIL"), 2 + 30)
        self.assertIn("[JAC_ARB] refused player=1 pair=0-1 turn=2 legitimacy=11 outlaw_until=32 power=3", self.log())
        self.assertEqual(self.legitimacy(self.OUCHI), 53 + 3 + 5)  # Ouchi accepted

    def test_the_shogun_can_afford_a_ruling_at_a_third_of_the_cost(self):
        ashikaga = self.g.Players[self.ASHIKAGA]
        ashikaga.gold = 34
        ashikaga.SetProperty(None, "JAC_LEGITIMACY", 30)  # the lowest legitimacy that may petition
        self.g.REQUEST(self.ASHIKAGA, self.OUCHI, self.OTOMO)
        self.assertEqual((ashikaga.gold, self.legitimacy(self.ASHIKAGA)), (0, 28))
        self.assertEqual(self.game("JAC_ARB_PENDING").Power, 3)
        R = self.g.JAC_ARB
        self.assertEqual(tuple(R.Costs(1)), (100, 5))
        self.assertEqual(tuple(R.Costs(3)), (34, 2))
        self.assertEqual(tuple(R.RefusalPenalty(1)), (15, 10))
        self.assertEqual(tuple(R.RefusalPenalty(3)), (45, 30))

    def test_after_1573_ashikaga_survives_but_cannot_call_rulings_for_others(self):
        self.next_turn(120)
        self.g.REQUEST(self.ASHIKAGA, self.OUCHI, self.OTOMO)
        self.assertIsNone(self.game("JAC_ARB_PENDING"))
        self.assertIn("[JAC_ARB] rejected request player=2 pair=0-1 reason=NOT_PARTY", self.log())
        self.assertEqual(self.g.Players[self.ASHIKAGA].gold, 500)
        # As a party it petitions like any house, at normal power.
        self.g.SET_WAR(self.ASHIKAGA, self.MORI, True)
        self.g.REQUEST(self.ASHIKAGA, self.ASHIKAGA, self.MORI)
        self.assertEqual(self.game("JAC_ARB_PENDING").Power, 1)
        self.assertEqual(self.g.Players[self.ASHIKAGA].gold, 500 - 100)  # full cost at normal power
        self.next_turn()
        self.assertEqual(self.game("JAC_ARB_TRUCE_2_3"), 121 + 10)

    def test_non_party_non_shogun_and_peace_pairs_are_rejected(self):
        self.g.REQUEST(self.MORI, self.OUCHI, self.OTOMO)
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.MORI)
        self.assertIsNone(self.game("JAC_ARB_PENDING"))
        log = self.log()
        self.assertIn("[JAC_ARB] rejected request player=3 pair=0-1 reason=NOT_PARTY", log)
        self.assertIn("[JAC_ARB] rejected request player=0 pair=0-3 reason=NOT_AT_WAR", log)

    def test_short_gold_or_legitimacy_below_30_rejects_without_charging(self):
        ouchi = self.g.Players[self.OUCHI]
        ouchi.gold = 99
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        self.assertIn("[JAC_ARB] rejected request player=0 pair=0-1 reason=GOLD", self.log())
        ouchi.gold = 500
        ouchi.SetProperty(None, "JAC_LEGITIMACY", 29)  # R-1: below 30 a house cannot petition
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        self.assertIn("[JAC_ARB] rejected request player=0 pair=0-1 reason=LEGITIMACY", self.log())
        self.assertEqual((ouchi.gold, self.legitimacy(self.OUCHI)), (500, 29))
        self.assertIsNone(self.game("JAC_ARB_PENDING"))

    def test_legitimacy_starts_at_50_or_70_and_earns_one_to_three_per_turn(self):
        self.assertEqual([self.legitimacy(i) for i in range(4)], [53, 53, 73, 53])
        R = self.g.JAC_ARB
        self.assertEqual(self.player(self.OUCHI, "JAC_ARB_HOME"), 1)  # first capital's plot
        # At war with the Shogunate: no court income.
        self.g.SET_WAR(self.OUCHI, self.ASHIKAGA, True)
        self.assertEqual(R.LegitimacyIncome(self.OUCHI, 2), 2)
        self.assertEqual(R.LegitimacyIncome(self.ASHIKAGA, 2), 3)  # the Shogunate itself
        # Home castle lost: no home income.
        self.g.PLOT_OWNER[1] = self.OTOMO
        self.assertEqual(R.LegitimacyIncome(self.OUCHI, 2), 1)
        # After 1573 the court income is gone for everyone.
        self.assertEqual(R.LegitimacyIncome(self.MORI, 120), 2)
        self.assertEqual(R.LegitimacyIncome(self.ASHIKAGA, 120), 2)
        self.next_turn()
        self.assertEqual(self.legitimacy(self.OUCHI), 53 + 1)
        self.assertIn("[JAC_ARB] legitimacy player=0 +1 = 54 turn=2", self.log())
        # Clamped to 0..100.
        self.g.Players[self.MORI].SetProperty(None, "JAC_LEGITIMACY", 99)
        self.next_turn()
        self.assertEqual(self.legitimacy(self.MORI), 100)
        self.g.Players[self.OTOMO].SetProperty(None, "JAC_LEGITIMACY", 1)
        self.g.Players[self.OTOMO].cities = 4
        self.g.REQUEST(self.ASHIKAGA, self.OUCHI, self.OTOMO)
        self.next_turn()  # income 3 then -45
        self.assertEqual(self.legitimacy(self.OTOMO), 0)

    def test_high_and_low_legitimacy_flag_every_city(self):
        FLAG = self.g.FLAG
        # Ashikaga (73) >= 70: +10% gold and culture in both its cities; Ouchi (53): neither flag.
        self.assertEqual([FLAG(x, "JAC_ARB_LEGITIMACY_HIGH") for x in (201, 202)], [1, 1])
        self.assertEqual([FLAG(x, "JAC_ARB_LEGITIMACY_LOW") for x in (201, 202)], [0, 0])
        self.assertEqual([FLAG(x, "JAC_ARB_LEGITIMACY_HIGH") for x in (1, 2)], [0, 0])
        self.assertEqual([FLAG(x, "JAC_ARB_LEGITIMACY_LOW") for x in (1, 2)], [0, 0])
        self.g.Players[self.OUCHI].SetProperty(None, "JAC_LEGITIMACY", 20)
        self.next_turn()  # 23 < 30: -10% gold and culture
        self.assertEqual([FLAG(x, "JAC_ARB_LEGITIMACY_LOW") for x in (1, 2)], [1, 1])
        self.g.Players[self.OUCHI].SetProperty(None, "JAC_LEGITIMACY", 27)
        self.next_turn()  # 30: back to neither
        self.assertEqual([FLAG(x, "JAC_ARB_LEGITIMACY_LOW") for x in (1, 2)], [0, 0])

    def test_an_ai_below_30_legitimacy_refuses_when_merely_ahead(self):
        self.g.Players[self.OTOMO].cities = 3  # exactly 1.5x: accepted normally
        self.g.Players[self.OTOMO].SetProperty(None, "JAC_LEGITIMACY", 20)
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        self.next_turn()
        self.assertIn("[JAC_ARB] decision player=1 accept=false by=ai", self.log())
        self.assertTrue(self.at_war(self.OUCHI, self.OTOMO))

    def test_a_clearly_winning_ai_refuses_and_is_penalized(self):
        self.g.Players[self.OTOMO].cities = 4  # 4 > 1.5 * 2
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        self.next_turn()
        self.assertTrue(self.at_war(self.OUCHI, self.OTOMO))
        self.assertEqual(self.legitimacy(self.OTOMO), 53 + 3 - 15)
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
        self.next_turn()  # turn 2: peace, truce until 12
        self.next_turn()  # a war on the turn of the peace would wait a turn (next test)
        self.g.SET_WAR(self.OUCHI, self.OTOMO, True)
        self.next_turn()
        self.assertFalse(self.at_war(self.OUCHI, self.OTOMO))
        self.assertIn("[JAC_ARB] truce reimposed pair=0-1 turn=4 until=12 ok=true", self.log())
        self.next_turn(13)
        self.assertIsNone(self.game("JAC_ARB_TRUCE_0_1"))
        self.g.SET_WAR(self.OUCHI, self.OTOMO, True)
        self.next_turn()
        self.assertTrue(self.at_war(self.OUCHI, self.OTOMO))

    def test_forced_peace_ends_a_war_inside_the_minimum_war_length_at_once(self):
        # court-arbitration-06: MakePeaceWith(b, true) made peace on the declaration turn; MakePeaceWith(b) did not.
        self.g.MIN_WAR = 10
        self.g.SET_WAR(self.OUCHI, self.OTOMO, True)  # declared on turn 1 (engine turn = start turn)
        self.g.REQUEST(self.ASHIKAGA, self.OUCHI, self.OTOMO)
        self.next_turn()  # turn 2
        self.assertFalse(self.at_war(self.OUCHI, self.OTOMO))
        self.assertEqual(self.game("JAC_ARB_TRUCE_0_1"), 2 + 30)
        self.assertEqual(self.game("JAC_ARB_PEACE_TURN_0_1"), 2)
        self.assertIsNone(self.game("JAC_ARB_AWAIT_0_1"))
        self.assertIn("[JAC_ARB] accepted pair=0-1 turn=2 truce_turns=30 power=3 peace=true", self.log())
        # A war declared under the truce, still inside the minimum war length, is undone the next turn.
        self.next_turn(12)
        self.g.SET_WAR(self.OUCHI, self.OTOMO, True)
        self.next_turn()
        self.assertFalse(self.at_war(self.OUCHI, self.OTOMO))
        self.assertEqual(self.game("JAC_ARB_PEACE_TURN_0_1"), 13)
        self.assertEqual(self.game("JAC_ARB_TRUCE_0_1"), 32)
        self.assertIn("[JAC_ARB] truce reimposed pair=0-1 turn=13 until=32 ok=true", self.log())
        self.assertFalse(any("SAFETY NET" in line for line in self.log()))

    def test_a_war_on_the_turn_of_the_scripts_peace_is_undone_one_turn_later(self):
        # court-arbitration-08: a war forced on the turn of a forced peace (impossible in normal play:
        # DIPLOMACY_PEACE_MIN_TURNS) hung the engine. The script waits a turn instead of flipping straight back.
        self.g.MIN_WAR = 10
        self.g.SET_WAR(self.OUCHI, self.OTOMO, True)
        self.g.REQUEST(self.ASHIKAGA, self.OUCHI, self.OTOMO)
        self.next_turn()  # turn 2: peace, truce until 32
        self.g.SET_WAR(self.OUCHI, self.OTOMO, True)
        self.next_turn()  # turn 3
        self.assertTrue(self.at_war(self.OUCHI, self.OTOMO))
        self.assertEqual((self.game("JAC_ARB_PEACE_TURN_0_1"), self.game("JAC_ARB_TRUCE_0_1")), (2, 32))
        self.assertIn("[JAC_ARB] truce breach deferred pair=0-1 turn=3 peace_turn=2", self.log())
        self.next_turn()  # turn 4
        self.assertFalse(self.at_war(self.OUCHI, self.OTOMO))
        self.assertEqual((self.game("JAC_ARB_PEACE_TURN_0_1"), self.game("JAC_ARB_TRUCE_0_1")), (4, 32))
        self.assertIn("[JAC_ARB] truce reimposed pair=0-1 turn=4 until=32 ok=true", self.log())
        self.assertFalse(any("SAFETY NET" in line for line in self.log()))

    def test_safety_net_defers_peace_when_the_force_flag_does_nothing(self):
        self.g.MIN_WAR, self.g.FORCE_IGNORED = 10, True
        self.g.SET_WAR(self.OUCHI, self.OTOMO, True)
        self.g.REQUEST(self.ASHIKAGA, self.OUCHI, self.OTOMO)
        self.next_turn()  # turn 2
        self.assertTrue(self.at_war(self.OUCHI, self.OTOMO))
        self.assertIsNone(self.game("JAC_ARB_TRUCE_0_1"))
        self.assertEqual(self.game("JAC_ARB_AWAIT_0_1"), 30)
        self.assertIn("[JAC_ARB] SAFETY NET accepted pair=0-1 turn=2 truce_turns=30 power=3 peace=deferred reason=min_war",
                      self.log())
        self.assertIn("LOC_JAC_ARB_NOTIFY_ACCEPTED_DEFERRED_SUMMARY", [n[3] for n in self.g.NOTES.values()])
        # The accepted pair cannot be petitioned (and charged) again while it waits.
        self.assertEqual(self.lua.eval("select(2, JAC_ARB.CheckRequest(0, 0, 1))"), "AWAITING_PEACE")
        self.next_turn(10)
        self.assertTrue(self.at_war(self.OUCHI, self.OTOMO))
        self.assertIn("[JAC_ARB] SAFETY NET peace deferred pair=0-1 turn=10 reason=min_war", self.log())
        self.next_turn(11)  # 10 turns after the declaration
        self.assertFalse(self.at_war(self.OUCHI, self.OTOMO))
        self.assertIsNone(self.game("JAC_ARB_AWAIT_0_1"))
        self.assertEqual(self.game("JAC_ARB_TRUCE_0_1"), 11 + 30)
        self.assertEqual(self.game("JAC_ARB_PEACE_TURN_0_1"), 11)
        self.assertIn("[JAC_ARB] truce started pair=0-1 turn=11 until=41", self.log())

    def test_a_pair_that_made_peace_on_its_own_while_waiting_starts_the_truce(self):
        self.g.MIN_WAR, self.g.FORCE_IGNORED = 10, True
        self.g.SET_WAR(self.OUCHI, self.OTOMO, True)
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        self.next_turn()
        self.g.SET_WAR(self.OUCHI, self.OTOMO, False)
        self.next_turn()
        self.assertEqual(self.game("JAC_ARB_TRUCE_0_1"), 3 + 10)
        self.assertIsNone(self.game("JAC_ARB_PEACE_TURN_0_1"))  # not made by the script

    def test_safety_net_undoes_a_truce_breach_when_allowed_even_after_the_truce(self):
        self.g.MIN_WAR, self.g.FORCE_IGNORED = 10, True
        self.g.Game.SetProperty(None, "JAC_ARB_TRUCE_2_3", 5)
        self.g.SET_WAR(self.ASHIKAGA, self.MORI, True)  # turn 1
        self.next_turn()
        self.assertIn("[JAC_ARB] SAFETY NET truce reimposed pair=2-3 turn=2 until=5 ok=false reason=min_war", self.log())
        self.next_turn(10)
        self.assertTrue(self.at_war(self.ASHIKAGA, self.MORI))
        self.assertEqual(self.game("JAC_ARB_TRUCE_2_3"), 5)  # kept past its end while the breach stands
        self.next_turn(11)
        self.assertFalse(self.at_war(self.ASHIKAGA, self.MORI))
        self.assertIsNone(self.game("JAC_ARB_TRUCE_2_3"))
        self.assertIn("[JAC_ARB] truce ended pair=2-3 turn=11", self.log())

    def test_make_peace_probe_tries_every_variant_and_never_stops(self):
        path = ROOT / "tools/civ6-autotest/scenarios/probe-make-peace.json"
        steps = json.loads(path.read_text(encoding="utf-8"))["steps"]
        self.assertEqual([s["turns"] for s in steps if s["action"] == "autoplay"], [1, 9, 1])  # turns 1, 2, 11, 12
        probes = [s["code"] for s in steps if s["action"] == "ui_lua"]
        self.assertEqual([s["state_name"] for s in steps if s["action"] == "ui_lua"], ["GameCore_Tuner"] * 4)
        self.assertIn("DIPLOMACY_WAR_MIN_TURNS", probes[0])
        self.assertIn("GetMinPeaceDuration()", probes[0])
        for variant in ("Try('F1','a',false)", "Try('F2','a',true)", "Try('F3','b',false)", "Try('R01','b',true)"):
            self.assertIn(variant, probes[0])
        for code in probes:
            self.assertNotIn("error(", code)
            # Runs against the stubs (most engine calls missing) and still returns a report.
            self.assertTrue(self.lua.execute(code).startswith("turn="))

    def test_a_request_is_void_if_the_war_ended_before_the_ruling(self):
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        self.g.SET_WAR(self.OUCHI, self.OTOMO, False)
        self.next_turn()
        self.assertIn("[JAC_ARB] void request pair=0-1 turn=2 (no longer at war)", self.log())
        self.assertIsNone(self.game("JAC_ARB_TRUCE_0_1"))


@unittest.skipIf(lupa is None, "lupa is not installed")
class SubjugationOrderTests(unittest.TestCase):
    """Owner decision R-2 (2026-09-30): a house that refuses a ruling becomes the target of an order to subdue."""

    OUCHI, OTOMO, ASHIKAGA, MORI = 0, 1, 2, 3
    setUp, next_turn, at_war, game, player, log, legitimacy = (
        ArbitrationTurnTests.setUp, ArbitrationTurnTests.next_turn, ArbitrationTurnTests.at_war, ArbitrationTurnTests.game,
        ArbitrationTurnTests.player, ArbitrationTurnTests.log, ArbitrationTurnTests.legitimacy)

    def refuse_on_turn_2(self):
        """Otomo (4 cities) refuses Ouchi's petition: the order runs turns 2..12."""
        self.g.Players[self.OTOMO].cities = 4
        self.g.Players[self.ASHIKAGA].human = True
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        self.next_turn()

    def joined(self, pid, target=1, turn=None) -> bool:
        return self.g.JAC_ARB.HasJoined(pid, target, turn or self.turn)

    def test_the_refuser_is_the_target_and_an_ai_at_war_with_it_joins_at_once(self):
        self.refuse_on_turn_2()
        self.assertEqual(self.player(self.OTOMO, "JAC_ARB_OUTLAW_UNTIL"), 12)
        self.assertIn("[JAC_ARB] joined player=0 target=1 turn=2 until=12 by=ai", self.log())
        self.assertTrue(self.joined(self.OUCHI))
        self.assertFalse(self.joined(self.MORI))  # neither at war with Otomo nor near it
        self.assertFalse(self.joined(self.ASHIKAGA))  # a human joins only through the panel
        refused = [n for n in self.g.NOTES.values() if n[2] == "LOC_JAC_ARB_NOTIFY_REFUSED_NAME"]
        self.assertEqual(len(refused), 4)  # every house hears of the order
        # A neighbouring AI joins at the next turn start.
        self.g.NEAR["1_3"] = True
        self.next_turn()
        self.assertTrue(self.joined(self.MORI))
        self.assertIn("[JAC_ARB] joined player=3 target=1 turn=3 until=12 by=ai", self.log())

    def test_a_human_joins_through_execute_script_once(self):
        self.refuse_on_turn_2()
        self.g.JOIN(self.ASHIKAGA, self.OTOMO)
        self.assertTrue(self.joined(self.ASHIKAGA))
        self.assertEqual(self.game("JAC_ARB_LAST_EVENT"), "join player=2 turn=2")
        self.g.JOIN(self.ASHIKAGA, self.OTOMO)
        self.g.JOIN(self.OTOMO, self.OTOMO)
        self.g.JOIN(self.ASHIKAGA, self.MORI)  # Mori is no target
        log = self.log()
        self.assertIn("[JAC_ARB] rejected join player=2 target=1 reason=ALREADY", log)
        self.assertIn("[JAC_ARB] rejected join player=1 target=1 reason=INVALID", log)
        self.assertIn("[JAC_ARB] rejected join player=2 target=3 reason=NO_ORDER", log)

    def test_war_weariness_flag_is_on_the_participants_anchor_city_only_during_the_order(self):
        FLAG = self.g.FLAG
        self.refuse_on_turn_2()
        # Ouchi joined: only its anchor city (smallest plot index) carries the flag, so the house-wide modifier applies once.
        self.assertEqual([FLAG(x, "JAC_ARB_SUBJUGATION_ANCHOR") for x in (1, 2)], [1, 0])
        self.assertEqual([FLAG(x, "JAC_ARB_SUBJUGATION_ANCHOR") for x in (201, 202)], [0, 0])
        self.g.JOIN(self.ASHIKAGA, self.OTOMO)
        self.assertEqual([FLAG(x, "JAC_ARB_SUBJUGATION_ANCHOR") for x in (201, 202)], [1, 0])  # at once
        self.assertEqual([FLAG(x, "JAC_ARB_SUBJUGATION_ANCHOR") for x in (101, 102, 103, 104)], [0, 0, 0, 0])
        self.next_turn(12)  # last turn of the order
        self.assertEqual(FLAG(1, "JAC_ARB_SUBJUGATION_ANCHOR"), 1)
        self.next_turn(13)
        self.assertEqual([FLAG(x, "JAC_ARB_SUBJUGATION_ANCHOR") for x in (1, 201)], [0, 0])
        self.assertIsNone(self.player(self.OTOMO, "JAC_ARB_OUTLAW_UNTIL"))
        self.assertIsNone(self.player(self.OUCHI, "JAC_ARB_JOINED_1"))
        self.assertIsNone(self.player(self.ASHIKAGA, "JAC_ARB_JOINED_1"))
        self.assertIn("[JAC_ARB] subjugation ended target=1 turn=13", self.log())
        ended = sorted(n[1] for n in self.g.NOTES.values() if n[2] == "LOC_JAC_ARB_NOTIFY_ORDER_ENDED_NAME")
        self.assertEqual(ended, [0, 1, 2])  # the target and its participants
        self.g.JOIN(self.ASHIKAGA, self.OTOMO)
        self.assertIn("[JAC_ARB] rejected join player=2 target=1 reason=NO_ORDER", self.log())

    def test_a_participant_that_takes_a_target_city_is_rewarded(self):
        self.refuse_on_turn_2()
        ouchi_before, otomo_before = self.legitimacy(self.OUCHI), self.legitimacy(self.OTOMO)
        self.g.CONQUER(self.OUCHI, self.OTOMO, 104)
        self.assertEqual(self.g.Players[self.OUCHI].gold, 400 + 200)
        self.assertEqual(self.legitimacy(self.OUCHI), ouchi_before + 10)
        self.assertEqual(self.legitimacy(self.OTOMO), otomo_before - 10)
        self.assertIn(f"[JAC_ARB] subjugation capture player=0 target=1 city=7 turn=2 gold=+200 legitimacy={ouchi_before + 10} "
                      f"target_legitimacy={otomo_before - 10}", self.log())
        capture = [n for n in self.g.NOTES.values() if n[2] == "LOC_JAC_ARB_NOTIFY_CAPTURE_NAME"]
        self.assertEqual(len(capture), 4)
        # Not rewarded: a house that has not joined, and a capture after the order.
        mori_gold = self.g.Players[self.MORI].gold
        self.g.CONQUER(self.MORI, self.OTOMO, 103)
        self.assertEqual((self.g.Players[self.MORI].gold, self.legitimacy(self.OTOMO)), (mori_gold, otomo_before - 10))
        self.next_turn(13)
        gold = self.g.Players[self.OUCHI].gold
        self.g.CONQUER(self.OUCHI, self.OTOMO, 102)
        self.assertEqual(self.g.Players[self.OUCHI].gold, gold)

    def test_refusing_a_shogun_ruling_orders_30_turns_and_a_new_refusal_never_shortens_it(self):
        self.g.Players[self.OTOMO].cities = 4
        self.g.REQUEST(self.ASHIKAGA, self.OUCHI, self.OTOMO)
        self.next_turn()
        self.assertEqual(self.player(self.OTOMO, "JAC_ARB_OUTLAW_UNTIL"), 32)
        self.g.Players[self.OTOMO].SetProperty(None, "JAC_LEGITIMACY", 50)
        self.g.REQUEST(self.OUCHI, self.OUCHI, self.OTOMO)
        self.next_turn()  # refused again, 10 turns: 13 < 32
        self.assertEqual(self.player(self.OTOMO, "JAC_ARB_OUTLAW_UNTIL"), 32)


UI_ONLY_STATES = {"InGame", "DiplomacyActionView", "DiplomacyDealView"}


@unittest.skipIf(lupa is None, "lupa is not installed")
class LiveScenarioOnStubsTests(unittest.TestCase):
    """Plays the gameplay steps of scenarios/court-arbitration.json against the stubs, so a slip in the
    scenario's Lua shows here instead of after a long live run. UI-only steps (panel buttons, the human's
    turn) are skipped; the panel's request goes straight to the gameplay event."""

    # 0 Ouchi, 1 Otomo, 2 Ashikaga, 3 Mori, 4 Oda, 5 Takeda
    CIVS = CIVS + ["CIVILIZATION_JAC_ODA", "CIVILIZATION_JAC_TAKEDA"]
    OUCHI, OTOMO = ArbitrationTurnTests.OUCHI, ArbitrationTurnTests.OTOMO
    next_turn, log = ArbitrationTurnTests.next_turn, ArbitrationTurnTests.log

    def setUp(self):
        self.lua = lupa.LuaRuntime(unpack_returned_tuples=True)
        rules = (MOD / "Scripts/JAC_CourtArbitration_Rules.lua").as_posix()
        self.lua.execute(STUBS, self.lua.table_from(calendar(), recursive=True), self.lua.table_from(self.CIVS),
                         START_TURN, rules)
        self.lua.execute((MOD / "Scripts/JAC_CourtArbitration.lua").read_text(encoding="utf-8"))
        self.g = self.lua.globals()
        self.g.MIN_WAR = 10
        self.g.SET_WAR(self.OUCHI, self.OTOMO, True)
        self.g.Players[self.OUCHI].cities = 4  # Ouchi refuses Otomo (4 > 1.5 * 2), as live (court-arbitration-07)
        self.g.Players[2].human = True  # the Ashikaga, as live: joins the order only through the panel
        self.turn = 1
        self.g.START_TURN(1)

    def test_scenario_steps_pass_on_the_stubs(self):
        self.lua.execute("function JAC_RequestArbitration(a, b) REQUEST(2, a, b); return true end")
        self.lua.execute("function JAC_JoinSubjugation(target) JOIN(2, target); return true end")
        steps = json.loads((ROOT / "tools/civ6-autotest/scenarios/court-arbitration.json").read_text(encoding="utf-8"))["steps"]
        results = []
        for step in steps:
            if step["action"] == "autoplay":
                self.next_turn(self.turn + step["turns"])
            elif (step["action"] == "ui_lua" and step["state_name"] not in UI_ONLY_STATES
                  and "Controls." not in step["code"]):
                results.append(self.lua.execute(step["code"]))
            elif step["action"] == "wait_for" and step["condition"]["state_name"] == "GameCore_Tuner":
                self.assertIs(self.lua.execute(step["condition"]["code"]), True)
        self.assertIn("legitimacy ashikaga=73 (+3/turn) ouchi=53 otomo=53", results[0])
        self.assertIn("accepted-path pair CIVILIZATION_JAC_MORI 3 - CIVILIZATION_JAC_ODA 4", results[0])
        self.assertIn("accepted ruling: 3-4 made peace on turn 2", results[2])
        self.assertIn("legitimacy 3=61 4=61 (+5 for accepting)", results[2])
        self.assertEqual(results[4], "refused by Ouchi (legitimacy 56 -> 14: -45 with this turn income; order to subdue until 33 "
                                     "= 3+10x3); joined by 1; accepted pair 3-4 still at peace")
        self.assertEqual(results[5], "legitimacy yield modifiers live (flag on/off, high +10% / low -10%): "
                                     + ", ".join(f"p{p} {y} {on}/10.0" for p, on, n in ((2, "11.0", 2), (0, "9.0", 4))
                                                 for _ in range(n) for y in ("GOLD", "CULTURE")))
        self.assertEqual(results[6], "join sent via EXECUTE_SCRIPT")
        # The breach is forced only past the minimum peace length (court-arbitration-08 hung when forced on turn 2).
        self.assertEqual(results[7], "forced war inside the truce between 3 and 4 on turn 13")
        self.assertEqual(results[8], "order to subdue Ouchi shortened to end after turn 14 (test only); "
                                     "truce breach 3-4 forced on turn 13 undone on turn 14, truce still until 32")
        self.assertEqual(results[9], "order to subdue Ouchi ended on turn 15; war-weariness anchor flags of Ashikaga and Otomo off")
        self.assertEqual(len(results), 10)
        self.assertIn("[JAC_ARB] joined player=2 target=0 turn=3 until=33 by=request", self.log())
        self.assertFalse(any("deferred" in line for line in self.log()))
        self.assertFalse(any("SAFETY NET" in line for line in self.log()))


if __name__ == "__main__":
    unittest.main()
