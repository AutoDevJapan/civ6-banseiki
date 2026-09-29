"""Runs Scripts/JAC_HistoricalEvents.lua turn by turn against stubbed gameplay objects (SSOT 3.2).

The stubs have only the gameplay-side methods: no CanTriggerBoost, which exists in the UI context only
and was nil live (events-06 Lua.log: "JAC_HistoricalEvents.lua:118: function expected instead of nil").
Needs ``lupa``: ``uv run --no-project --with lupa python -m unittest tests.test_historical_events_lua``.
"""

from __future__ import annotations

import unittest
import xml.etree.ElementTree as ET

from tests.test_historical_events import MOD, ROOT, load_db

try:
    import lupa
except ImportError:  # pragma: no cover - depends on the environment
    lupa = None

START_TURN = 16603  # live 1534 calendar: the engine turn counter starts here, not at 1
CIVS = ["CIVILIZATION_JAC_ODA", "CIVILIZATION_JAC_NAGAO", "CIVILIZATION_JAC_DATE"]

STUBS = r"""
local tables, civs, startTurn = ...
local function gameInfoTable(rows, key)
  local t = {}
  for i, row in ipairs(rows) do row.Index = i - 1; if key then t[row[key]] = row end end
  return setmetatable(t, { __call = function()
    local i = 0
    return function() i = i + 1; return rows[i] end
  end })
end
GameInfo = {
  StartEras = { ERA_ANCIENT = { Year = tables.StartYear } },
  GameSpeeds = { [0] = { GameSpeedType = tables.Speed or "GAMESPEED_STANDARD" } },
  GameSpeed_Turns = gameInfoTable(tables.GameSpeed_Turns),
  Technologies = gameInfoTable(tables.Technologies, "TechnologyType"),
  Civics = gameInfoTable(tables.Civics, "CivicType"),
  Boosts = gameInfoTable(tables.Boosts),
}
for _, name in ipairs({ "JAC_HistoricalEvents", "JAC_HistoricalEventTargets", "JAC_HistoricalEventModifiers",
    "JAC_HistoricalEventRenames", "JAC_HistoricalEventResources" }) do
  GameInfo[name] = gameInfoTable(tables[name])
end
CURRENT_TURN = startTurn
GameConfiguration = { GetGameSpeedType = function() return 0 end, GetStartTurn = function() return startTurn end }
local function properties()
  local values = {}
  return function(_, k) return values[k] end, function(_, k, v) values[k] = v end
end
Game = { GetCurrentGameTurn = function() return CURRENT_TURN end }
Game.GetProperty, Game.SetProperty = properties()
BOOSTS, FAIL_BOOST = {}, {}
Players, PlayerConfigurations, MAJORS = {}, {}, {}
for i, civ in ipairs(civs) do
  local id = i - 1
  local techs, civics, triggered = {}, {}, {}
  local p = {
    techs = techs, civics = civics,
    GetID = function() return id end,
    AttachModifierByID = function() end,
    GetTechs = function() return {
      HasTech = function(_, t) return techs[t] == true end,
      HasBoostBeenTriggered = function(_, t) return triggered[t] == true end,
      TriggerBoost = function(_, t, source)
        if FAIL_BOOST[id] then error("engine refused") end
        triggered[t] = true; table.insert(BOOSTS, { id, "tech", t, source, CURRENT_TURN - startTurn + 1 })
      end,
    } end,
    GetCulture = function() return {
      HasCivic = function(_, c) return civics[c] == true end,
      HasBoostBeenTriggered = function(_, c) return triggered["c" .. c] == true end,
      TriggerBoost = function(_, c, source)
        triggered["c" .. c] = true; table.insert(BOOSTS, { id, "civic", c, source, CURRENT_TURN - startTurn + 1 })
      end,
    } end,
  }
  p.GetProperty, p.SetProperty = properties()
  Players[id] = p
  PlayerConfigurations[id] = { GetCivilizationTypeName = function() return civ end }
  table.insert(MAJORS, p)
end
PlayerManager = { GetAliveMajors = function() return MAJORS end }
ParameterTypes = { MESSAGE = "MESSAGE", SUMMARY = "SUMMARY" }
NotificationTypes = { USER_DEFINED_1 = 1 }
NotificationManager = { SendNotification = function() end }
ExposedMembers = {}
LOG = {}
function print(line) LOG[#LOG + 1] = line end
local handlers = {}
GameEvents = { OnGameTurnStarted = { Add = function(f) table.insert(handlers, f) end } }
function START_TURN(turn)
  CURRENT_TURN = startTurn + turn - 1
  for _, f in ipairs(handlers) do f(CURRENT_TURN) end
end
"""


def tables() -> dict:
    db = load_db()
    db.row_factory = lambda cursor, row: {c[0]: v for c, v in zip(cursor.description, row) if v is not None}
    out = {name: db.execute(f"SELECT * FROM {name}").fetchall() for name in (
        "Technologies", "Civics", "Boosts", "JAC_HistoricalEvents", "JAC_HistoricalEventTargets",
        "JAC_HistoricalEventModifiers", "JAC_HistoricalEventRenames", "JAC_HistoricalEventResources")}
    calendar = ET.parse(ROOT / "poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Calendar.xml").getroot()
    out["StartYear"] = int(calendar.findtext("./StartEras/Update/Set/Year"))
    out["GameSpeed_Turns"] = [
        {"GameSpeedType": r.findtext("GameSpeedType"), "TurnsPerIncrement": int(r.findtext("TurnsPerIncrement")),
         "MonthIncrement": int(r.findtext("MonthIncrement"))}
        for r in calendar.findall("./GameSpeed_Turns/Row")]
    return out


@unittest.skipIf(lupa is None, "lupa is not installed")
class HistoricalEventsTurnTests(unittest.TestCase):
    def setUp(self, data: dict | None = None):
        self.lua = lupa.LuaRuntime(unpack_returned_tuples=True)
        self.lua.execute(STUBS, self.lua.table_from(data or tables(), recursive=True),
                         self.lua.table_from(CIVS), START_TURN)
        self.lua.execute((MOD / "Scripts/JAC_HistoricalEvents.lua").read_text(encoding="utf-8"))
        self.g = self.lua.globals()
        self.teppo = self.g.GameInfo.Technologies["TECH_JAC_SENGOKU_FIREARMS_CONTACT"].Index
        self.g.Players[0].techs[self.teppo] = True  # Oda researched firearms (scenario: forced on turn 1)

    def run_turns(self, first: int, last: int) -> None:
        for turn in range(first, last + 1):
            self.g.START_TURN(turn)

    def boosts(self) -> list[tuple]:
        return [tuple(b.values()) for b in self.g.BOOSTS.values()]

    def prop(self, player: int, key: str):
        return self.g.Players[player].GetProperty(None, key)

    def test_teppo_boosts_every_faction_without_the_tech_on_its_due_turn(self):
        self.run_turns(1, 29)
        self.assertEqual(self.boosts(), [])
        self.assertEqual(self.prop(0, "JAC_EVENT_FIRED_JAC_EVENT_TEPPO_DENRAI"), 1)  # by prereq, turn 1
        self.run_turns(30, 31)
        self.assertEqual(self.boosts(), [(1, "tech", self.teppo, 1, 30), (2, "tech", self.teppo, 1, 30)])
        for player in (1, 2):
            self.assertEqual(self.prop(player, "JAC_EVENT_BOOSTED_JAC_EVENT_TEPPO_DENRAI"), 30)
            self.assertIsNone(self.prop(player, "JAC_EVENT_FIRED_JAC_EVENT_TEPPO_DENRAI"))
        self.assertIsNone(self.prop(0, "JAC_EVENT_BOOSTED_JAC_EVENT_TEPPO_DENRAI"))
        log = list(self.g.LOG.values())
        self.assertIn("[JAC_EVENTS] boosted event=JAC_EVENT_TEPPO_DENRAI player=1 civ=CIVILIZATION_JAC_NAGAO "
                      "reason=year turn=30 due=30 boost=tech=boosted", log)
        # Researching it later gives the effects (reason=prereq) without a second boost.
        self.g.Players[1].techs[self.teppo] = True
        self.run_turns(32, 32)
        self.assertEqual(self.prop(1, "JAC_EVENT_FIRED_JAC_EVENT_TEPPO_DENRAI"), 32)
        self.assertEqual(len(self.boosts()), 2)

    def test_an_engine_error_in_one_boost_does_not_stop_the_other_factions(self):
        self.g.FAIL_BOOST[1] = True
        self.run_turns(1, 30)
        self.assertEqual(self.boosts(), [(2, "tech", self.teppo, 1, 30)])
        self.assertTrue(any("player=1" in line and "boost=tech=error:" in line for line in self.g.LOG.values()))

    def test_a_failed_boost_is_left_unrecorded_and_retried_next_turn(self):
        self.g.FAIL_BOOST[1] = True
        self.run_turns(1, 30)
        self.assertIsNone(self.prop(1, "JAC_EVENT_BOOSTED_JAC_EVENT_TEPPO_DENRAI"))
        self.assertEqual(self.prop(2, "JAC_EVENT_BOOSTED_JAC_EVENT_TEPPO_DENRAI"), 30)
        self.g.FAIL_BOOST[1] = False
        self.run_turns(31, 32)
        self.assertEqual(self.boosts(), [(2, "tech", self.teppo, 1, 30), (1, "tech", self.teppo, 1, 31)])
        self.assertEqual(self.prop(1, "JAC_EVENT_BOOSTED_JAC_EVENT_TEPPO_DENRAI"), 31)

    def test_resource_event_without_the_resources_mod_records_nothing(self):
        self.run_turns(1, 49)  # Ashio copper (1550) is due on turn 49
        self.assertIsNone(self.g.Game.GetProperty(None, "JAC_EVENT_RESOURCES_JAC_EVENT_ASHIO_COPPER"))
        self.assertIn("[JAC_EVENTS] resource event=JAC_EVENT_ASHIO_COPPER skipped=resources_mod_not_loaded",
                      list(self.g.LOG.values()))

    def test_calendar_stages_do_not_depend_on_row_order(self):
        # Epic: 3 months/turn for 200 turns, then 2. Teppo (1543/8, 115 months) is turn 1 + ceil(115/3) = 40;
        # walking the 2-month stage first would give 59.
        data = tables()
        data["Speed"] = "GAMESPEED_EPIC"
        data["GameSpeed_Turns"].reverse()
        self.setUp(data)
        self.run_turns(1, 39)
        self.assertEqual(self.boosts(), [])
        self.run_turns(40, 40)
        self.assertEqual(self.boosts(), [(1, "tech", self.teppo, 1, 40), (2, "tech", self.teppo, 1, 40)])


if __name__ == "__main__":
    unittest.main()
