"""Runs Scripts/JAC_Sengoku_UnificationPact.lua (with JAC_Sengoku_RebuiltCastles.lua) against stubbed gameplay objects.

天下統一同盟 (SSOT 3.1): the pact pools castles and only the lead wins. The stubs have only gameplay-side calls.
Needs ``lupa``: ``uv run --no-project --with lupa python -m unittest tests.test_unification_pact_lua``.
"""

from __future__ import annotations

from pathlib import Path
import unittest

from tools.generate_unification_victory import CASTLE_SET, PACT_LEAD_PREFIX, PACT_RESULT, REBUILT, UNIFIED

try:
    import lupa
except ImportError:  # pragma: no cover - depends on the environment
    lupa = None

SCRIPTS = Path(__file__).resolve().parents[1] / "poc/JAC_PoC_SengokuTrees/Scripts"
ODA, DATE, ASHINA, SHIMAZU = 0, 1, 2, 3  # Shimazu is a second human

STUBS = r"""
local CASTLE_SET, REBUILT, UNIFIED = ...
local WIDTH = 32
local castles = { {6, 5}, {6, 14}, {25, 5}, {25, 14} }
local reqs, args = {}, {}
for i, xy in ipairs(castles) do
  reqs[#reqs + 1] = { RequirementSetId = CASTLE_SET, RequirementId = "C" .. i }
  args[#args + 1] = { RequirementId = "C" .. i, Name = "X", Value = tostring(xy[1]) }
  args[#args + 1] = { RequirementId = "C" .. i, Name = "Y", Value = tostring(xy[2]) }
end
reqs[#reqs + 1] = { RequirementSetId = CASTLE_SET, RequirementId = "NONE" }
args[#args + 1] = { RequirementId = "NONE", Name = "X", Value = "-1" }
args[#args + 1] = { RequirementId = "NONE", Name = "Y", Value = "-1" }
local function iter(rows) return function() local i = 0; return function() i = i + 1; return rows[i] end end end
GameInfo = {
  Buildings = { [REBUILT] = { Index = 7 }, [UNIFIED] = { Index = 8 } },
  RequirementSetRequirements = iter(reqs),
  RequirementArguments = iter(args),
}
function SET_NO_CASTLES() reqs = { reqs[#reqs] }; GameInfo.RequirementSetRequirements = iter(reqs) end
local function properties()
  local values = {}
  return function(_, k) return values[k] end, function(_, k, v) values[k] = v end
end
START, TURN = 16603, 16603
Game = { GetCurrentGameTurn = function() return TURN end }
Game.GetProperty, Game.SetProperty = properties()
GameConfiguration = { GetStartTurn = function() return START end }
local plots = {}
Map = { GetPlot = function(x, y)
  local i = y * WIDTH + x
  plots[i] = plots[i] or { GetX = function() return x end, GetY = function() return y end, GetIndex = function() return i end }
  return plots[i]
end }
Map.GetPlotByIndex = function(i) return Map.GetPlot(i % WIDTH, math.floor(i / WIDTH)) end
CITIES, BUILT = {}, {}  -- plot index -> city; {building, plot} placed
function CITY(owner, x, y, capital)
  local buildings = {}
  local city = { owner = owner, capital = capital, buildings = buildings }
  city.GetOwner = function() return city.owner end
  city.GetX = function() return x end
  city.GetY = function() return y end
  city.GetID = function() return y * WIDTH + x end
  city.GetBuildings = function() return { HasBuilding = function(_, b) return buildings[b] == true end } end
  city.GetBuildQueue = function() return { CreateIncompleteBuilding = function(_, b, plot, pct)
    assert(plot == y * WIDTH + x and pct == 100); buildings[b] = true; table.insert(BUILT, { b, plot })
  end } end
  CITIES[y * WIDTH + x] = city
  return city
end
CityManager = { GetCityAt = function(x, y) return CITIES[y * WIDTH + x] end }
Players, PlayerConfigurations, MAJORS, ALLIED, WAR = {}, {}, {}, {}, {}
local CIVS = { "ODA", "DATE", "ASHINA", "SHIMAZU" }
for id, spec in ipairs({ { human = true }, {}, {}, { human = true } }) do
  local pid = id - 1
  local p = { team = pid }
  p.GetID = function() return pid end
  p.IsMajor = function() return true end
  p.IsAlive = function() return not p.dead end
  p.IsHuman = function() return spec.human == true end
  p.GetTeam = function() return p.team end
  p.GetDiplomacy = function() return {
    HasAllied = function(_, other) return ALLIED[pid .. "-" .. other] == true end,
    IsAtWarWith = function(_, other) return WAR[math.min(pid, other) .. "-" .. math.max(pid, other)] == true end,
  } end
  PlayerConfigurations[pid] = { GetCivilizationTypeName = function() return "CIVILIZATION_JAC_" .. CIVS[id] end }
  p.GetCities = function() return { GetCapitalCity = function()
    for _, c in pairs(CITIES) do if c.owner == pid and c.capital then return c end end
  end } end
  p.GetProperty, p.SetProperty = properties()
  Players[pid] = p
  MAJORS[#MAJORS + 1] = p
end
PlayerManager = { GetAliveMajors = function()
  local alive = {}
  for _, p in ipairs(MAJORS) do if not p.dead then alive[#alive + 1] = p end end
  return alive
end, GetWasEverAliveCount = function() return #MAJORS end }
ExposedMembers = {}
LOG = {}
function print(line) LOG[#LOG + 1] = line end
local handlers = {}
GameEvents = setmetatable({}, { __index = function(t, name)
  local event = { Add = function(f) handlers[name] = handlers[name] or {}; table.insert(handlers[name], f) end }
  rawset(t, name, event); return event
end })
function FIRE(name, ...) for _, f in ipairs(handlers[name] or {}) do f(...) end end
"""


@unittest.skipIf(lupa is None, "lupa is not installed")
class UnificationPactTests(unittest.TestCase):
    def setUp(self, no_castles: bool = False):
        self.lua = lupa.LuaRuntime(unpack_returned_tuples=True)
        self.lua.execute(STUBS, CASTLE_SET, REBUILT, UNIFIED)
        self.g = self.lua.globals()
        if no_castles:  # settler starts: no castle file, only the (-1, -1) sentinel
            self.g.SET_NO_CASTLES()
        # Test map: Oda holds 6,5 (capital) and 6,14; Date holds 25,5 (capital) and 25,14.
        self.g.CITY(ODA, 6, 5, True)
        self.g.CITY(ODA, 6, 14)
        self.g.CITY(DATE, 25, 5, True)
        self.date_castle = self.g.CITY(DATE, 25, 14)
        self.g.CITY(ASHINA, 16, 10, True)
        self.g.CITY(SHIMAZU, 3, 3, True)
        for name in ("JAC_Sengoku_RebuiltCastles.lua", "JAC_Sengoku_UnificationPact.lua"):
            self.lua.execute((SCRIPTS / name).read_text(encoding="utf-8"))

    def request(self, event: str, player: int, target):
        self.g.FIRE(f"JAC_UnificationPact{event}", player, self.lua.table_from({"Target": target}))

    def lead(self, player: int):
        return self.g.Game.GetProperty(None, f"{PACT_LEAD_PREFIX}{player}")

    def result(self, player: int):
        return self.g.Players[player].GetProperty(None, PACT_RESULT)

    def marked(self) -> list[tuple]:
        return [tuple(row.values()) for row in self.g.BUILT.values()]

    def unified(self) -> int:
        return self.g.GameInfo.Buildings[UNIFIED].Index

    def test_ai_joins_only_when_allied_and_the_lead_capital_is_marked_when_the_pact_holds_all(self):
        self.request("Propose", ODA, DATE)
        self.assertEqual(self.result(ODA), "NOT_ALLIED")
        self.assertIsNone(self.lead(DATE))
        self.date_castle.owner = ASHINA  # an outsider holds one castle
        self.g.ALLIED[f"{ODA}-{DATE}"] = True
        self.request("Propose", ODA, DATE)
        self.assertEqual((self.result(ODA), self.lead(ODA), self.lead(DATE)), ("JOINED", ODA, ODA))
        self.assertEqual(self.marked(), [], "3 of 4 castles: no marker")
        self.date_castle.owner = DATE  # handed to the partner; CityBuilt fires on transfers
        self.g.FIRE("CityBuilt", DATE, 0, 25, 14)
        self.assertEqual(self.marked(), [(self.unified(), 5 * 32 + 6)], "Oda's capital, not Date's")
        self.g.FIRE("OnGameTurnStarted", 16604)
        self.assertEqual(len(self.marked()), 1, "placed once")

    def test_human_target_must_accept_the_proposer(self):
        self.request("Propose", ODA, SHIMAZU)
        self.assertEqual(self.result(ODA), "PROPOSED")
        self.assertIsNone(self.lead(SHIMAZU))
        self.request("Accept", SHIMAZU, DATE)  # no proposal from Date
        self.assertEqual(self.result(SHIMAZU), "INVALID")
        self.request("Accept", DATE, ODA)  # the requester is the engine's player, not a parameter
        self.assertEqual(self.result(DATE), "INVALID")
        self.assertIsNone(self.lead(DATE))
        self.request("Accept", SHIMAZU, ODA)
        self.assertEqual((self.lead(ODA), self.lead(SHIMAZU)), (ODA, ODA))

    def test_one_pact_per_player_and_no_pact_within_a_team(self):
        self.g.ALLIED[f"{ODA}-{DATE}"] = True
        self.g.ALLIED[f"{ASHINA}-{DATE}"] = True
        self.g.ALLIED[f"{DATE}-{ASHINA}"] = True
        self.date_castle.owner = ASHINA
        self.request("Propose", ODA, DATE)
        self.request("Propose", ASHINA, DATE)  # Date is already in Oda's pact
        self.assertEqual(self.result(ASHINA), "INVALID")
        self.request("Propose", DATE, ASHINA)  # a member cannot lead another pact
        self.assertEqual(self.result(DATE), "INVALID")
        self.assertIsNone(self.lead(ASHINA))
        self.g.ALLIED[f"{ODA}-{ASHINA}"] = True
        self.request("Propose", ODA, ASHINA)  # the lead may take more members
        self.assertEqual(self.lead(ASHINA), ODA)
        self.assertEqual(len(self.marked()), 1, "Oda, Date and Ashina hold all four")
        self.g.Players[SHIMAZU].team = ODA
        self.request("Propose", ODA, SHIMAZU)
        self.assertEqual(self.result(ODA), "INVALID", "teams already pool natively")
        self.request("Propose", ODA, None)
        self.request("Propose", ODA, ODA)
        self.assertEqual(self.result(ODA), "INVALID")

    def test_rebuilt_city_on_a_razed_castle_does_not_block_the_pact(self):
        self.g.ALLIED[f"{ODA}-{DATE}"] = True
        del self.g.CITIES[14 * 32 + 25]
        self.g.TURN = 16604
        self.g.FIRE("OnGameTurnStarted", 16604)  # records the raze
        self.g.CITY(ASHINA, 25, 14)
        self.g.FIRE("CityBuilt", ASHINA, 0, 25, 14)  # marked as rebuilt
        self.request("Propose", ODA, DATE)
        self.assertEqual(self.marked()[-1], (self.unified(), 5 * 32 + 6))

    def ally(self, a: int, b: int):
        self.g.ALLIED[f"{a}-{b}"] = True
        self.g.ALLIED[f"{b}-{a}"] = True

    def pact_of_oda_date_shimazu(self):
        self.ally(ODA, DATE)
        self.request("Propose", ODA, DATE)
        self.request("Propose", ODA, SHIMAZU)
        self.request("Accept", SHIMAZU, ODA)
        self.assertEqual((self.lead(ODA), self.lead(DATE), self.lead(SHIMAZU)), (ODA, ODA, ODA))

    def test_eliminated_member_drops_out_and_the_rest_of_the_pact_can_still_win(self):
        self.date_castle.owner = ASHINA
        self.pact_of_oda_date_shimazu()
        taken = self.g.CITIES[5 * 32 + 25]  # Oda takes Date's capital, Date's last city
        taken.owner, taken.capital = ODA, False
        self.g.Players[DATE].dead = True
        self.g.FIRE("CityConquered", ODA, DATE, 0, 25, 5)
        self.assertEqual((self.lead(ODA), self.lead(SHIMAZU)), (ODA, ODA), "two houses remain")
        self.assertEqual(self.marked(), [], "Ashina still holds a castle")
        self.date_castle.owner = ODA
        self.g.FIRE("CityConquered", ODA, ASHINA, 0, 25, 14)
        self.assertEqual(self.marked(), [(self.unified(), 5 * 32 + 6)], "the lead's capital")

    def test_a_pact_left_with_one_house_is_dissolved(self):
        self.ally(ODA, DATE)
        self.request("Propose", ODA, DATE)
        self.g.Players[DATE].dead = True
        self.g.FIRE("OnGameTurnStarted", 16604)
        self.assertIsNone(self.lead(ODA))
        self.assertEqual(self.result(ODA), "DISSOLVED")

    def test_a_member_leaves_at_once_and_the_rest_stay(self):
        self.pact_of_oda_date_shimazu()
        self.g.FIRE("JAC_UnificationPactLeave", SHIMAZU, self.lua.table_from({"Target": SHIMAZU}))
        self.assertEqual((self.lead(ODA), self.lead(DATE), self.lead(SHIMAZU)), (ODA, ODA, None))
        self.assertEqual(self.result(SHIMAZU), "LEFT")
        self.g.FIRE("JAC_UnificationPactLeave", SHIMAZU, None)  # not in a pact
        self.assertEqual(self.result(SHIMAZU), "INVALID")
        self.request("Accept", SHIMAZU, ODA)  # the old proposal was used up
        self.assertIsNone(self.lead(SHIMAZU))
        self.request("Propose", ODA, SHIMAZU)
        self.request("Accept", SHIMAZU, ODA)
        self.assertEqual(self.lead(SHIMAZU), ODA)

    def test_the_lead_leaving_passes_the_pact_and_a_pact_of_two_dissolves(self):
        self.pact_of_oda_date_shimazu()
        self.g.FIRE("JAC_UnificationPactLeave", ODA, None)
        # Date holds two castles, Shimazu none: Date leads.
        self.assertEqual((self.lead(ODA), self.lead(DATE), self.lead(SHIMAZU)), (None, DATE, DATE))
        self.g.FIRE("JAC_UnificationPactLeave", SHIMAZU, None)
        self.assertEqual((self.lead(DATE), self.result(DATE), self.result(SHIMAZU)), (None, "DISSOLVED", "LEFT"))

    def test_members_at_war_leave_and_the_lead_stays(self):
        self.pact_of_oda_date_shimazu()
        self.g.WAR[f"{ODA}-{DATE}"] = True
        self.g.FIRE("OnGameTurnStarted", 16604)
        self.assertEqual((self.lead(ODA), self.lead(DATE), self.lead(SHIMAZU)), (ODA, None, ODA))
        self.assertEqual(self.result(DATE), "AT_WAR")
        self.assertIn("JAC_Sengoku_UnificationPact: 1 left the pact of 0 (at war with 0)", list(self.g.LOG.values()))

    def test_two_warring_members_both_leave(self):
        self.pact_of_oda_date_shimazu()
        self.g.WAR[f"{DATE}-{SHIMAZU}"] = True
        self.g.FIRE("OnGameTurnStarted", 16604)
        self.assertEqual((self.lead(ODA), self.lead(DATE), self.lead(SHIMAZU)), (None, None, None))
        self.assertEqual((self.result(ODA), self.result(SHIMAZU)), ("DISSOLVED", "AT_WAR"))
        log = list(self.g.LOG.values())
        self.assertIn("JAC_Sengoku_UnificationPact: 1 left the pact of 0 (at war with 3)", log)
        self.assertIn("JAC_Sengoku_UnificationPact: pact of 0 dissolved", log)
        # Date (AI, allied with Oda, 2 of 4 castles) then proposes again under the AI rule.
        self.assertEqual((self.result(DATE), self.g.Game.GetProperty(None, "JAC_UNIFICATION_PACT_PROPOSAL_0")), ("PROPOSED", DATE))

    def test_a_pact_of_two_dissolves_when_they_go_to_war(self):
        self.ally(ODA, DATE)
        self.date_castle.owner = ASHINA
        self.request("Propose", ODA, DATE)
        self.g.WAR[f"{ODA}-{DATE}"] = True
        self.date_castle.owner = ODA  # a conquest checks at once, before any marker
        self.g.FIRE("CityConquered", ODA, ASHINA, 0, 25, 14)
        self.assertEqual(self.marked(), [])
        self.g.CITIES[5 * 32 + 25].owner = ODA  # Oda takes Date's capital
        self.g.FIRE("CityConquered", ODA, DATE, 0, 25, 5)
        self.assertEqual((self.lead(ODA), self.lead(DATE)), (None, None))
        self.assertEqual((self.result(ODA), self.result(DATE)), ("DISSOLVED", "AT_WAR"))
        self.assertEqual(self.marked(), [])

    def test_ai_proposes_to_an_allied_ai_when_they_hold_enough_castles(self):
        self.ally(DATE, ASHINA)  # Date holds 2 of 4 castles: 50% >= 30%
        self.g.FIRE("OnGameTurnStarted", 16604)
        self.assertEqual((self.lead(DATE), self.lead(ASHINA)), (DATE, DATE))

    def test_ai_proposal_to_a_human_waits_for_acceptance_and_is_not_repeated(self):
        self.ally(DATE, ODA)
        self.g.FIRE("OnGameTurnStarted", 16604)
        self.assertEqual(self.g.Game.GetProperty(None, "JAC_UNIFICATION_PACT_PROPOSAL_0"), DATE)
        self.assertEqual(self.result(DATE), "PROPOSED")
        self.assertIsNone(self.lead(ODA))
        self.g.FIRE("OnGameTurnStarted", 16605)
        self.assertEqual(sum("proposed a pact" in line for line in self.g.LOG.values()), 1)
        self.request("Accept", ODA, DATE)
        self.assertEqual((self.lead(ODA), self.lead(DATE)), (DATE, DATE))
        self.assertIsNone(self.g.Game.GetProperty(None, "JAC_UNIFICATION_PACT_PROPOSAL_0"))

    def test_ai_does_not_propose_below_the_share_or_without_an_alliance(self):
        self.ally(ASHINA, SHIMAZU)  # no castles between them
        self.g.FIRE("OnGameTurnStarted", 16604)
        self.assertIsNone(self.g.Game.GetProperty(None, "JAC_UNIFICATION_PACT_PROPOSAL_3"))
        self.assertIsNone(self.lead(ASHINA))
        self.assertIsNone(self.lead(DATE), "Date is allied with no one")

    def test_an_active_start_alliance_pools_and_its_holder_of_more_castles_leads(self):
        self.g.Game.SetProperty(None, "JAC_1534_ALLIANCE_ASHINA-DATE", 16603)
        for x, y in ((6, 5), (6, 14), (25, 14)):
            self.g.CITIES[y * 32 + x].owner = ASHINA
        self.g.CITIES[5 * 32 + 6].capital = False
        self.g.FIRE("CityConquered", ASHINA, ODA, 0, 6, 5)
        self.assertEqual(self.marked(), [(self.unified(), 10 * 32 + 16)], "Ashina (3 castles) leads, not Date")
        self.assertIsNone(self.lead(ASHINA), "no pact is formed")

    def test_an_ended_start_alliance_does_not_pool(self):
        self.g.Game.SetProperty(None, "JAC_1534_ALLIANCE_ASHINA-DATE", False)
        for x, y in ((6, 5), (6, 14)):
            self.g.CITIES[y * 32 + x].owner = ASHINA
        self.g.FIRE("OnGameTurnStarted", 16604)
        self.assertEqual(self.marked(), [])

    def test_a_start_alliance_tie_goes_to_the_lowest_id(self):
        self.g.Game.SetProperty(None, "JAC_1534_ALLIANCE_ASHINA-DATE", 16603)
        for x, y in ((6, 5), (6, 14)):
            self.g.CITIES[y * 32 + x].owner = ASHINA  # Ashina 2, Date 2
        self.g.FIRE("OnGameTurnStarted", 16604)
        self.assertEqual(self.marked(), [(self.unified(), 5 * 32 + 25)], "tie: Date, the lower ID")

    def test_a_start_alliance_inside_one_pact_is_left_to_the_pact(self):
        self.g.Game.SetProperty(None, "JAC_1534_ALLIANCE_ASHINA-DATE", 16603)
        for x, y in ((6, 5), (6, 14)):
            self.g.CITIES[y * 32 + x].owner = ASHINA
        self.ally(ASHINA, DATE)
        self.request("Propose", ASHINA, DATE)
        self.assertEqual(self.marked(), [(self.unified(), 10 * 32 + 16)], "the pact lead Ashina, not the pair's Date")

    def test_eliminated_member_castle_taken_by_an_outsider_blocks_the_pact(self):
        self.g.ALLIED[f"{ODA}-{DATE}"] = True
        self.date_castle.owner = ASHINA
        self.request("Propose", ODA, DATE)
        self.g.CITIES[5 * 32 + 25].owner = ASHINA  # Ashina eliminates Date
        self.g.Players[DATE].dead = True
        self.g.FIRE("CityConquered", ASHINA, DATE, 0, 25, 5)
        self.g.FIRE("OnGameTurnStarted", 16604)
        self.assertEqual(self.marked(), [])
        self.request("Propose", ODA, DATE)  # an eliminated player cannot be invited again
        self.assertEqual(self.result(ODA), "INVALID")

    def test_without_registered_castles_the_pact_pools_home_capitals(self):
        # Owner decision O-8: settler starts pool every major's first capital, as the capital rule.
        self.setUp(no_castles=True)
        self.ally(ODA, DATE)
        self.request("Propose", ODA, DATE)
        self.assertEqual((self.result(ODA), self.lead(DATE)), ("JOINED", ODA))
        self.assertEqual(self.g.Game.GetProperty(None, "JAC_UNIFICATION_HOME_CAPITAL_2"), 10 * 32 + 16)
        self.assertEqual(self.marked(), [], "Ashina's and Shimazu's capitals stand outside the pact")
        ashina_home = self.g.CITIES[10 * 32 + 16]
        ashina_home.owner, ashina_home.capital = DATE, False
        self.g.Players[ASHINA].dead = True
        self.g.FIRE("CityConquered", DATE, ASHINA, 0, 16, 10)
        self.assertEqual(self.marked(), [], "Shimazu's home capital")
        shimazu_home = self.g.CITIES[3 * 32 + 3]
        shimazu_home.owner, shimazu_home.capital = ODA, False
        self.g.Players[SHIMAZU].dead = True
        self.g.FIRE("CityConquered", ODA, SHIMAZU, 0, 3, 3)
        self.assertEqual(self.marked(), [(self.unified(), 5 * 32 + 6)], "every home capital, dead houses included")

    def test_home_capitals_count_only_once_every_major_has_founded(self):
        self.setUp(no_castles=True)
        del self.g.CITIES[3 * 32 + 3]  # Shimazu still has its settler
        self.g.FIRE("OnGameTurnStarted", 16603)  # records the founded capitals
        self.g.CITIES[10 * 32 + 16].owner = ODA
        self.g.CITIES[10 * 32 + 16].capital = False
        self.ally(ODA, DATE)
        self.request("Propose", ODA, DATE)
        self.assertEqual(self.marked(), [], "Shimazu has not founded yet")
        self.g.CITY(SHIMAZU, 3, 3, True)
        self.g.FIRE("CityBuilt", SHIMAZU, 0, 3, 3)
        self.assertEqual(self.marked(), [], "Shimazu's new capital stands outside the pact")
        self.g.CITIES[3 * 32 + 3].owner = DATE
        self.g.FIRE("CityConquered", DATE, SHIMAZU, 0, 3, 3)
        self.assertEqual(self.marked(), [(self.unified(), 5 * 32 + 6)])

    def pact_of_oda_date_ashina(self):
        self.g.ALLIED[f"{ODA}-{DATE}"] = True
        self.g.ALLIED[f"{ODA}-{ASHINA}"] = True
        self.date_castle.owner = ASHINA
        self.request("Propose", ODA, DATE)
        self.request("Propose", ODA, ASHINA)
        self.assertEqual((self.lead(DATE), self.lead(ASHINA)), (ODA, ODA))
        self.assertEqual(self.marked(), [(self.unified(), 5 * 32 + 6)])  # the pact holds all four
        del self.g.BUILT[1]  # forget it: each test below checks the marker after the lead's death

    def eliminate_oda(self, taker_of_capital: int, taker_of_other: int):
        self.g.CITIES[5 * 32 + 6].owner, self.g.CITIES[5 * 32 + 6].capital = taker_of_capital, False
        self.g.CITIES[14 * 32 + 6].owner = taker_of_other
        self.g.Players[ODA].dead = True
        self.g.FIRE("CityConquered", taker_of_capital, ODA, 0, 6, 5)

    def test_lead_eliminated_passes_the_pact_to_the_member_with_most_castles(self):
        self.pact_of_oda_date_ashina()
        self.eliminate_oda(ASHINA, ASHINA)  # Ashina 3 castles, Date 1: the count beats the lower ID
        self.assertEqual((self.lead(DATE), self.lead(ASHINA)), (ASHINA, ASHINA))
        self.assertIn("JAC_Sengoku_UnificationPact: lead 0 gone; 2 leads the pact (3 targets)",
                      list(self.g.LOG.values()))
        self.assertEqual(self.marked(), [(self.unified(), 10 * 32 + 16)], "the new lead's capital")
        self.g.FIRE("OnGameTurnStarted", 16604)
        self.assertEqual((self.lead(DATE), len(self.marked())), (ASHINA, 1), "stable, placed once")

    def test_lead_eliminated_tie_goes_to_the_lowest_id_who_can_then_take_members(self):
        self.pact_of_oda_date_ashina()
        self.eliminate_oda(SHIMAZU, SHIMAZU)  # an outsider takes Oda's castles; Date and Ashina hold 1 each
        self.assertEqual((self.lead(DATE), self.lead(ASHINA)), (DATE, DATE))
        self.assertEqual(self.marked(), [], "Shimazu holds two castles")
        self.request("Propose", DATE, SHIMAZU)  # the new lead may take members
        self.request("Accept", SHIMAZU, DATE)
        self.assertEqual(self.lead(SHIMAZU), DATE)
        self.assertEqual(self.marked(), [(self.unified(), 5 * 32 + 25)], "Date's capital")

    def test_pact_with_no_survivor_is_gone(self):
        self.g.ALLIED[f"{ODA}-{DATE}"] = True
        self.date_castle.owner = ASHINA
        self.request("Propose", ODA, DATE)
        self.g.Players[DATE].dead = True
        self.eliminate_oda(ASHINA, ASHINA)
        self.g.FIRE("OnGameTurnStarted", 16604)
        self.assertEqual(self.marked(), [])
        self.assertFalse(any("leads the pact" in line for line in self.g.LOG.values()))
        self.request("Propose", ODA, SHIMAZU)  # a dead lead takes no members
        self.assertEqual(self.result(ODA), "INVALID")


if __name__ == "__main__":
    unittest.main()
