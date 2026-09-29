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
Players, MAJORS, ALLIED = {}, {}, {}
for id, spec in ipairs({ { human = true }, {}, {}, { human = true } }) do
  local pid = id - 1
  local p = { team = pid }
  p.GetID = function() return pid end
  p.IsMajor = function() return true end
  p.IsAlive = function() return not p.dead end
  p.IsHuman = function() return spec.human == true end
  p.GetTeam = function() return p.team end
  p.GetDiplomacy = function() return { HasAllied = function(_, other) return ALLIED[pid .. "-" .. other] == true end } end
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
end }
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

    def test_eliminated_member_drops_out_and_the_rest_of_the_pact_can_still_win(self):
        self.g.ALLIED[f"{ODA}-{DATE}"] = True
        self.date_castle.owner = ASHINA
        self.request("Propose", ODA, DATE)
        self.assertEqual(self.lead(DATE), ODA)
        taken = self.g.CITIES[5 * 32 + 25]  # Oda takes Date's capital, Date's last city
        taken.owner, taken.capital = ODA, False
        self.g.Players[DATE].dead = True
        self.g.FIRE("CityConquered", ODA, DATE, 0, 25, 5)
        self.assertEqual(self.marked(), [], "Ashina still holds a castle")
        self.date_castle.owner = ODA
        self.g.FIRE("CityConquered", ODA, ASHINA, 0, 25, 14)
        self.assertEqual(self.marked(), [(self.unified(), 5 * 32 + 6)], "the lead's capital")

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

    def test_no_marker_without_registered_castles(self):
        self.setUp(no_castles=True)
        self.g.ALLIED[f"{ODA}-{DATE}"] = True
        self.request("Propose", ODA, DATE)
        self.assertEqual(self.lead(DATE), ODA)
        self.assertEqual(self.marked(), [])


if __name__ == "__main__":
    unittest.main()
