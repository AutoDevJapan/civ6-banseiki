"""Runs Scripts/JAC_Japan_1534Relations.lua against stubbed gameplay objects: ending a start alliance.

The engine ignores SetHasAllied(ally, false) (live diplo-17); EndStartAlliance breaks the alliance with a forced war
and makes peace at the start of the next turn (never on the war turn: court-arbitration-08 hung the engine).
Needs ``lupa``: ``uv run --no-project --with lupa python -m unittest tests.test_japan_1534_relations_lua``.
"""

from __future__ import annotations

from pathlib import Path
import unittest

try:
    import lupa
except ImportError:  # pragma: no cover - depends on the environment
    lupa = None

LUA = Path(__file__).resolve().parents[1] / "poc/JAC_PoC_JapanMapScale/Scripts/JAC_Japan_1534Relations.lua"

STUBS = r"""
local FACTIONS = { "HOJO", "IMAGAWA", "SHIMAZU", "RYUKYU" }
ID = {}
TURN, WAR_ENDS_ALLIANCE = 1, true
ALLIED, WAR, CALLS = {}, {}, {}  -- ALLIED/WAR keyed by the unordered pair "lo-hi"
local function pair(a, b) if a > b then a, b = b, a end return a .. "-" .. b end
local props = {}
Game = { GetCurrentGameTurn = function() return TURN end,
  GetProperty = function(_, k) return props[k] end, SetProperty = function(_, k, v) props[k] = v end }
GameConfiguration = { GetStartTurn = function() return 1 end }
WarTypes = { FORMAL_WAR = 7 }
PlayerConfigurations, Players, local_majors = {}, {}, {}
for i, name in ipairs(FACTIONS) do
  local id = i - 1
  ID[name] = id
  PlayerConfigurations[id] = { GetCivilizationTypeName = function() return "CIVILIZATION_JAC_" .. name end }
  local d = {}
  function d:SetHasMet() end
  function d:SetHasDeclaredFriendship() end
  function d:RecheckVisibilityOnAll() end
  function d:HasAllied(b) return ALLIED[pair(id, b)] == true end
  function d:IsAtWarWith(b) return WAR[pair(id, b)] == true end
  function d:SetHasAllied(b, value)
    table.insert(CALLS, "SetHasAllied " .. id .. ">" .. b .. " " .. tostring(value))
    if value then ALLIED[pair(id, b)] = true end  -- false is ignored while allied (live diplo-17)
  end
  function d:DeclareWarOn(b, warType, force)
    table.insert(CALLS, "DeclareWarOn " .. id .. ">" .. b .. " " .. tostring(warType) .. " " .. tostring(force))
    WAR[pair(id, b)] = true
    if WAR_ENDS_ALLIANCE then ALLIED[pair(id, b)] = nil end
  end
  function d:MakePeaceWith(b, force)
    table.insert(CALLS, "MakePeaceWith " .. id .. ">" .. b .. " " .. tostring(force) .. " turn " .. TURN)
    WAR[pair(id, b)] = nil
  end
  Players[id] = { GetID = function() return id end, GetDiplomacy = function() return d end }
  local_majors[#local_majors + 1] = Players[id]
end
PlayerManager = { GetAliveMajors = function() return local_majors end }
ExposedMembers = {}
LOG = {}
function print(line) LOG[#LOG + 1] = line end
local handlers = {}
local function events()
  return setmetatable({}, { __index = function(t, name)
    local event = { Add = function(f) handlers[name] = handlers[name] or {}; table.insert(handlers[name], f) end }
    rawset(t, name, event); return event
  end })
end
GameEvents, LuaEvents, Events = events(), events(), events()
function FIRE(name, ...) for _, f in ipairs(handlers[name] or {}) do f(...) end end
function PAIR(a, b) return pair(ID[a], ID[b]) end
function COUNT(prefix) local n = 0; for _, c in ipairs(CALLS) do if c:sub(1, #prefix) == prefix then n = n + 1 end end; return n end
"""


@unittest.skipIf(lupa is None, "lupa is not installed")
class EndStartAllianceTests(unittest.TestCase):
    def setUp(self):
        self.lua = lupa.LuaRuntime(unpack_returned_tuples=True)
        self.lua.execute(STUBS)
        self.lua.execute(LUA.read_text(encoding="utf-8"))
        self.g = self.lua.globals()
        self.g.FIRE("NewGameInitialized")
        self.api = self.g.ExposedMembers.JAC_Japan1534Relations
        self.hojo_imagawa = self.g.PAIR("HOJO", "IMAGAWA")
        self.assertTrue(self.g.ALLIED[self.hojo_imagawa])
        self.assertTrue(self.api.IsStartAllianceActive("IMAGAWA", "HOJO"))

    def turn(self, number):
        self.g.TURN = number
        self.g.FIRE("OnGameTurnStarted", number)

    def calls(self, prefix):
        return [c for c in self.g.CALLS.values() if c.startswith(prefix)]

    def test_end_breaks_the_alliance_with_a_war_and_makes_peace_on_the_next_turn(self):
        self.turn(5)
        self.assertTrue(self.api.EndStartAlliance("HOJO", "IMAGAWA"))
        hojo, imagawa = self.g.ID["HOJO"], self.g.ID["IMAGAWA"]
        self.assertEqual(self.calls("DeclareWarOn"), [f"DeclareWarOn {hojo}>{imagawa} 7 true"])  # the first house declares
        self.assertIsNone(self.g.ALLIED[self.hojo_imagawa])
        self.assertFalse(self.api.IsStartAllianceActive("HOJO", "IMAGAWA"))
        self.assertFalse(self.api.EndStartAlliance("HOJO", "IMAGAWA"))
        self.assertEqual(self.g.COUNT("DeclareWarOn"), 1)
        self.g.FIRE("OnGameTurnStarted", 5)  # never peace on the war turn
        self.assertEqual(self.calls("MakePeaceWith"), [])
        self.turn(6)
        self.assertEqual(self.calls("MakePeaceWith"), [f"MakePeaceWith {hojo}>{imagawa} true turn 6"])
        self.assertIsNone(self.g.WAR[self.hojo_imagawa])
        self.assertIn("[JAC_1534_RELATIONS] made peace after ended alliance HOJO-IMAGAWA on turn 6", self.g.LOG.values())
        for number in (7, 32, 33):  # no second peace, and the ended pair is never re-made (lapse turn 32)
            self.turn(number)
        self.assertEqual(self.g.COUNT("MakePeaceWith"), 1)
        self.assertIsNone(self.g.ALLIED[self.hojo_imagawa])
        self.assertTrue(self.g.ALLIED[self.g.PAIR("SHIMAZU", "RYUKYU")])  # other pairs are untouched

    def test_a_war_started_elsewhere_ends_the_alliance_without_a_second_war_or_peace(self):
        self.g.WAR[self.hojo_imagawa] = True
        self.turn(5)
        self.assertFalse(self.api.IsStartAllianceActive("HOJO", "IMAGAWA"))
        self.turn(6)
        self.assertEqual(self.g.COUNT("DeclareWarOn"), 0)
        self.assertEqual(self.g.COUNT("MakePeaceWith"), 0)
        self.assertTrue(self.g.WAR[self.hojo_imagawa])

    def test_if_the_war_leaves_the_alliance_it_is_logged_peace_follows_and_the_lapse_is_final(self):
        self.g.WAR_ENDS_ALLIANCE = False
        self.turn(5)
        self.assertTrue(self.api.EndStartAlliance("IMAGAWA", "HOJO"))
        hojo, imagawa = self.g.ID["HOJO"], self.g.ID["IMAGAWA"]
        self.assertEqual(self.calls("DeclareWarOn"), [f"DeclareWarOn {imagawa}>{hojo} 7 true"])
        self.assertIn("[JAC_1534_RELATIONS] still allied after war IMAGAWA-HOJO (ends at the engine lapse)",
                      self.g.LOG.values())
        self.turn(6)
        self.assertEqual(self.g.COUNT("MakePeaceWith"), 1)
        self.g.ALLIED[self.hojo_imagawa] = None  # the engine lapse (made on turn 1 + 30 + 1)
        self.turn(32)
        self.assertIsNone(self.g.ALLIED[self.hojo_imagawa])
        self.assertEqual(self.g.COUNT("SetHasAllied 0>1 true"), 1)  # only the start alliance; never re-made


if __name__ == "__main__":
    unittest.main()
