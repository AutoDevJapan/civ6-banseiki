"""Runs Scripts/JAC_Japan_1534Relations.lua against stubbed gameplay objects: ending a start alliance.

No script call ends an alliance at once (live end-alliance-01: SetHasAllied(ally, false) is ignored, a forced war
keeps HasAllied and vision returns after peace), so EndStartAlliance declares no war: it stops re-making the pair
and the alliance ends at the engine's next lapse.
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
TURN = 1
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
    WAR[pair(id, b)] = true  -- the alliance stays (live end-alliance-01)
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

    def test_end_declares_no_war_and_the_pair_is_not_re_made_after_the_lapse(self):
        self.turn(5)
        self.assertTrue(self.api.EndStartAlliance("HOJO", "IMAGAWA"))
        self.assertFalse(self.api.IsStartAllianceActive("IMAGAWA", "HOJO"))
        self.assertFalse(self.api.EndStartAlliance("IMAGAWA", "HOJO"))
        self.assertTrue(self.g.ALLIED[self.hojo_imagawa])  # the engine ignores SetHasAllied(false)
        self.assertIn("[JAC_1534_RELATIONS] ended start alliance HOJO-IMAGAWA", self.g.LOG.values())
        self.g.ALLIED[self.hojo_imagawa] = None  # engine lapse of every start alliance (turn 1 + 30 + 1)
        self.g.ALLIED[self.g.PAIR("SHIMAZU", "RYUKYU")] = None
        self.turn(32)
        self.assertIsNone(self.g.ALLIED[self.hojo_imagawa])
        self.assertTrue(self.g.ALLIED[self.g.PAIR("SHIMAZU", "RYUKYU")])  # other pairs are re-made
        self.assertEqual(self.g.COUNT("DeclareWarOn"), 0)
        self.assertEqual(self.g.COUNT("MakePeaceWith"), 0)

    def test_a_war_started_elsewhere_ends_the_alliance_without_a_second_war_or_peace(self):
        self.g.WAR[self.hojo_imagawa] = True
        self.turn(5)
        self.assertFalse(self.api.IsStartAllianceActive("HOJO", "IMAGAWA"))
        self.turn(6)
        self.assertEqual(self.g.COUNT("DeclareWarOn"), 0)
        self.assertEqual(self.g.COUNT("MakePeaceWith"), 0)
        self.assertTrue(self.g.WAR[self.hojo_imagawa])


if __name__ == "__main__":
    unittest.main()
