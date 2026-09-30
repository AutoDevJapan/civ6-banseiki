"""Runs Scripts/JAC_Japan_1534Relations.lua against stubbed gameplay objects: the start alliances' end.

Owner decision K (2026-09-30): the start alliances are not re-made when the engine's 30-turn limit lapses them;
each house re-forms alliances through normal diplomacy. The pair property is a number while the start alliance is
active and false after it lapses, is ended by an event (EndStartAlliance, which cannot dissolve it at once: live
end-alliance-01) or the pair is at war.
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
class StartAllianceEndTests(unittest.TestCase):
    def setUp(self):
        self.lua = lupa.LuaRuntime(unpack_returned_tuples=True)
        self.lua.execute(STUBS)
        self.lua.execute(LUA.read_text(encoding="utf-8"))
        self.g = self.lua.globals()
        self.g.FIRE("NewGameInitialized")
        self.api = self.g.ExposedMembers.JAC_Japan1534Relations
        self.hojo_imagawa = self.g.PAIR("HOJO", "IMAGAWA")
        self.shimazu_ryukyu = self.g.PAIR("SHIMAZU", "RYUKYU")
        self.assertTrue(self.g.ALLIED[self.hojo_imagawa])
        self.assertTrue(self.api.IsStartAllianceActive("IMAGAWA", "HOJO"))
        self.made = self.g.COUNT("SetHasAllied")

    def turn(self, number):
        self.g.TURN = number
        self.g.FIRE("OnGameTurnStarted", number)

    def prop(self, key):
        return self.g.Game.GetProperty(None, "JAC_1534_ALLIANCE_" + key)

    def lapse(self):  # the engine lapses every start alliance (turn 1 + 30 + 1) before any script runs
        self.g.ALLIED[self.hojo_imagawa] = None
        self.g.ALLIED[self.shimazu_ryukyu] = None

    def test_lapsed_start_alliances_are_not_re_made_and_turn_false(self):
        self.turn(31)
        self.assertEqual(self.prop("HOJO-IMAGAWA"), 1)  # still the start alliance
        self.lapse()
        self.turn(32)
        self.assertIsNone(self.g.ALLIED[self.hojo_imagawa])
        self.assertIsNone(self.g.ALLIED[self.shimazu_ryukyu])
        self.assertEqual(self.g.COUNT("SetHasAllied"), self.made)  # no forced alliance
        self.assertIs(self.prop("HOJO-IMAGAWA"), False)
        self.assertIs(self.prop("RYUKYU-SHIMAZU"), False)
        self.assertIn("[JAC_1534_RELATIONS] start alliance lapsed R02 HOJO-IMAGAWA on turn 32", self.g.LOG.values())
        self.assertIsNone(self.g.Game.GetProperty(None, "JAC_1534_ALLIANCE_LAPSES"))

    def test_an_alliance_the_pair_makes_later_is_not_a_start_alliance(self):
        self.lapse()
        self.turn(32)
        self.g.ALLIED[self.hojo_imagawa] = True  # re-formed through normal diplomacy
        self.turn(40)
        self.assertFalse(self.api.IsStartAllianceActive("HOJO", "IMAGAWA"))
        self.assertIs(self.prop("HOJO-IMAGAWA"), False)
        self.assertFalse(self.api.EndStartAlliance("HOJO", "IMAGAWA"))
        self.g.ALLIED[self.hojo_imagawa] = None  # that alliance lapses too
        self.turn(71)
        self.assertEqual(self.g.COUNT("SetHasAllied"), self.made)

    def test_end_marks_the_pair_ended_and_the_alliance_lapses_at_the_limit(self):
        self.turn(5)
        self.assertTrue(self.api.EndStartAlliance("HOJO", "IMAGAWA"))
        self.assertFalse(self.api.IsStartAllianceActive("IMAGAWA", "HOJO"))
        self.assertFalse(self.api.EndStartAlliance("IMAGAWA", "HOJO"))
        self.assertTrue(self.g.ALLIED[self.hojo_imagawa])  # no call dissolves an alliance at once
        self.assertIn("[JAC_1534_RELATIONS] ended start alliance HOJO-IMAGAWA", self.g.LOG.values())
        self.assertTrue(self.api.IsStartAllianceActive("SHIMAZU", "RYUKYU"))  # other pairs are unaffected
        self.lapse()
        self.turn(32)
        self.assertIs(self.prop("HOJO-IMAGAWA"), False)
        self.assertNotIn("[JAC_1534_RELATIONS] start alliance lapsed R02 HOJO-IMAGAWA on turn 32", self.g.LOG.values())
        self.assertEqual(self.g.COUNT("SetHasAllied"), self.made)
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
