"""Runs Scripts/JAC_HistoricalEvents.lua and UI/JAC_HistoricalEventsAdvisor.lua turn by turn against stubs (SSOT 3.2).

The gameplay stubs have only the gameplay-side methods: no CanTriggerBoost, which exists in the UI context only
and was nil live (events-06 Lua.log: "JAC_HistoricalEvents.lua:118: function expected instead of nil").
Needs ``lupa``: ``uv run --no-project --with lupa python -m unittest tests.test_historical_events_lua``.
"""

from __future__ import annotations

import re
import unittest
import xml.etree.ElementTree as ET

from tests.test_historical_events import MOD, ROOT, load_db, turn_for_year

try:
    import lupa
except ImportError:  # pragma: no cover - depends on the environment
    lupa = None

START_TURN = 16603  # live 1534 calendar: the engine turn counter starts here, not at 1
STARTING_KNOWLEDGE = ROOT / "poc/JAC_PoC_SengokuTrees/Scripts/JAC_Sengoku_StartingKnowledge.lua"
CIVS = ["CIVILIZATION_JAC_ODA", "CIVILIZATION_JAC_NAGAO", "CIVILIZATION_JAC_DATE", "CIVILIZATION_JAC_SHIMAZU"]
TABLES = ("JAC_HistoricalEvents", "JAC_HistoricalEventPrereqTechs", "JAC_HistoricalEventPrereqCivics",
          "JAC_HistoricalEventTargets",
          "JAC_HistoricalEventModifiers", "JAC_HistoricalEventRenames", "JAC_HistoricalEventResources")

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
for _, name in ipairs(tables.Names) do GameInfo[name] = gameInfoTable(tables[name]) end
CURRENT_TURN = startTurn
GameConfiguration = { GetGameSpeedType = function() return 0 end, GetStartTurn = function() return startTurn end }
local function properties()
  local values = {}
  return function(_, k) return values[k] end, function(_, k, v) values[k] = v end
end
Game = { GetCurrentGameTurn = function() return CURRENT_TURN end, GetLocalPlayer = function() return LOCAL end }
Game.GetProperty, Game.SetProperty = properties()
LOCAL = 0
BOOSTS, FAIL_BOOST, NOTES, RAISED = {}, {}, {}, {}
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
      SetTech = function(_, t, v) techs[t] = v end,
      HasBoostBeenTriggered = function(_, t) return triggered[t] == true end,
      TriggerBoost = function(_, t, source)
        if FAIL_BOOST[id] then error("engine refused") end
        triggered[t] = true; table.insert(BOOSTS, { id, "tech", t, source, CURRENT_TURN - startTurn + 1 })
      end,
    } end,
    GetCulture = function() return {
      HasCivic = function(_, c) return civics[c] == true end,
      SetCivic = function(_, c, v) civics[c] = v end,
      HasBoostBeenTriggered = function(_, c) return triggered["c" .. c] == true end,
      TriggerBoost = function(_, c, source)
        if FAIL_BOOST[id] then error("engine refused") end
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
NotificationManager = { SendNotification = function(id, _, data)
  table.insert(NOTES, { id, data.MESSAGE, data.SUMMARY, CURRENT_TURN - startTurn + 1 })
end }
ExposedMembers = {}
LOG = {}
function print(line) LOG[#LOG + 1] = line end
-- UI side
Locale = { Lookup = function(k) return k end }
local newGame = {}
LuaEvents = { TutorialUIRoot_AdvisorRaise = function(item) table.insert(RAISED, item) end,
  NewGameInitialized = { Add = function(f) table.insert(newGame, f) end } }
function NEW_GAME_INITIALIZED() for _, f in ipairs(newGame) do f() end end
local uiHandlers = {}
Events = {
  LocalPlayerTurnBegin = { Add = function(f) table.insert(uiHandlers, f) end },
  LocalPlayerChanged = { Add = function(f) table.insert(uiHandlers, f) end },
}
function UI_EVENT() for _, f in ipairs(uiHandlers) do f() end end
local handlers = {}
GameEvents = { OnGameTurnStarted = { Add = function(f) table.insert(handlers, f) end } }
-- A load re-runs the scripts; the properties stay (they are saved with the game).
function RELOAD() handlers = {}; uiHandlers = {} end
function START_TURN(turn)
  CURRENT_TURN = startTurn + turn - 1
  for _, f in ipairs(handlers) do f(CURRENT_TURN) end
end
"""


def tables() -> dict:
    db = load_db()
    db.row_factory = lambda cursor, row: {c[0]: v for c, v in zip(cursor.description, row) if v is not None}
    out = {name: db.execute(f"SELECT * FROM {name}").fetchall() for name in ("Technologies", "Civics", "Boosts") + TABLES}
    out["Names"] = list(TABLES)
    calendar = ET.parse(ROOT / "poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Calendar.xml").getroot()
    out["StartYear"] = int(calendar.findtext("./StartEras/Update/Set/Year"))
    out["GameSpeed_Turns"] = [
        {"GameSpeedType": r.findtext("GameSpeedType"), "TurnsPerIncrement": int(r.findtext("TurnsPerIncrement")),
         "MonthIncrement": int(r.findtext("MonthIncrement"))}
        for r in calendar.findall("./GameSpeed_Turns/Row")]
    return out


def ui_source() -> str:
    """The advisor UI file with HavokScript's typed structure turned into a plain table for standard Lua.

    hstructure/hmake are HavokScript syntax (AdvisorPopup.lua:25-38, TutorialUIRoot.lua:216); only this
    declaration and its one construction are rewritten, the rest runs as shipped."""
    source = (MOD / "UI/JAC_HistoricalEventsAdvisor.lua").read_text(encoding="utf-8")
    source, n = re.subn(r"^hstructure AdvisorItem\n.*?^end\n", "", source, flags=re.S | re.M)
    assert n == 1
    source, n = re.subn(r"hmake AdvisorItem ", "", source)
    assert n == 1
    return source


@unittest.skipIf(lupa is None, "lupa is not installed")
class HistoricalEventsTurnTests(unittest.TestCase):
    def setUp(self, data: dict | None = None):
        self.lua = lupa.LuaRuntime(unpack_returned_tuples=True)
        self.lua.execute(STUBS, self.lua.table_from(data or tables(), recursive=True),
                         self.lua.table_from(CIVS), START_TURN)
        self.load_scripts()
        self.g = self.lua.globals()
        self.teppo = self.tech("TECH_JAC_SENGOKU_FIREARMS_CONTACT")
        self.g.Players[0].techs[self.teppo] = True  # Oda researched firearms (scenario: forced on turn 1)

    def load_scripts(self):
        self.lua.execute((MOD / "Scripts/JAC_HistoricalEvents.lua").read_text(encoding="utf-8"))
        self.lua.execute(ui_source())

    def tech(self, name: str) -> int:
        return self.lua.globals().GameInfo.Technologies[name].Index

    def civic(self, name: str) -> int:
        return self.lua.globals().GameInfo.Civics[name].Index

    def run_turns(self, first: int, last: int) -> None:
        for turn in range(first, last + 1):
            self.g.START_TURN(turn)

    def boosts(self, kind: str, index: int) -> list[tuple]:
        return [tuple(b.values()) for b in self.g.BOOSTS.values() if b[2] == kind and b[3] == index]

    def prop(self, player: int, key: str):
        return self.g.Players[player].GetProperty(None, key)

    def game(self, key: str):
        return self.g.Game.GetProperty(None, key)

    def log(self) -> list[str]:
        return list(self.g.LOG.values())

    def test_teppo_boosts_every_faction_without_the_tech_on_its_due_turn(self):
        self.run_turns(1, 29)
        self.assertEqual(self.boosts("tech", self.teppo), [])
        self.assertEqual(self.prop(0, "JAC_EVENT_FIRED_JAC_EVENT_TEPPO"), 1)  # by prereq, turn 1
        self.run_turns(30, 31)  # 1543/9 is turn 30
        self.assertEqual(self.boosts("tech", self.teppo),
                         [(1, "tech", self.teppo, 1, 30), (2, "tech", self.teppo, 1, 30), (3, "tech", self.teppo, 1, 30)])
        for player in (1, 2, 3):
            self.assertEqual(self.prop(player, "JAC_EVENT_BOOSTED_JAC_EVENT_TEPPO"), 30)
            self.assertIsNone(self.prop(player, "JAC_EVENT_FIRED_JAC_EVENT_TEPPO"))
        self.assertIsNone(self.prop(0, "JAC_EVENT_BOOSTED_JAC_EVENT_TEPPO"))
        self.assertIn("[JAC_EVENTS] boosted event=JAC_EVENT_TEPPO player=1 civ=CIVILIZATION_JAC_NAGAO "
                      "reason=year turn=30 due=30 boost=tech=boosted", self.log())
        # Researching it later gives the effects (reason=prereq) without a second boost.
        self.g.Players[1].techs[self.teppo] = True
        self.run_turns(32, 32)
        self.assertEqual(self.prop(1, "JAC_EVENT_FIRED_JAC_EVENT_TEPPO"), 32)
        self.assertEqual(len(self.boosts("tech", self.teppo)), 3)

    def test_any_one_of_the_prerequisites_fires_the_event_early(self):
        # Christianity (1549/8): Nanban envoys OR port merchant councils. Nagao has only the second one.
        councils, envoys = self.civic("CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS"), self.civic("CIVIC_JAC_SENGOKU_NANBAN_ENVOYS")
        self.g.Players[1].civics[councils] = True
        self.run_turns(1, 1)
        self.assertEqual(self.prop(1, "JAC_EVENT_FIRED_JAC_EVENT_CHRISTIAN"), 1)
        self.assertIsNone(self.prop(0, "JAC_EVENT_FIRED_JAC_EVENT_CHRISTIAN"))
        # At the year the others get the event's boost (Nanban envoys), not port merchant councils.
        due = turn_for_year(1549, 8)
        self.run_turns(2, due)
        self.assertEqual([b[0] for b in self.boosts("civic", envoys)], [0, 2, 3])
        self.assertEqual(self.boosts("civic", councils), [])

    def test_starting_knowledge_fires_no_event_on_turn_1(self):
        # Live sengoku-events-list-01: with the 1534 starting knowledge granted, Shimazu unification (C02) and the
        # Tenbun famine (T01) fired for every house on turn 1. The real granting script runs here.
        self.g.Players[0].techs[self.teppo] = None  # no scenario forcing, only the starting knowledge
        self.lua.execute(STARTING_KNOWLEDGE.read_text(encoding="utf-8"))
        self.g.NEW_GAME_INITIALIZED()
        granted = {i: (sum(1 for v in p.techs.values() if v), sum(1 for v in p.civics.values() if v))
                   for i, p in self.g.Players.items()}
        self.assertEqual(granted, {i: (4, 3) for i in range(len(CIVS))})
        self.run_turns(1, 1)
        self.assertIsNone(self.game("JAC_EVENTS_FIRE_COUNT"))
        self.assertEqual(list(self.g.BOOSTS.values()), [])
        self.assertEqual([line for line in self.log() if "reason=" in line], [])

    def test_events_without_prerequisites_happen_in_their_year_for_their_targets_only(self):
        # Asakura fall (1573/9, Asakura and Oda) has no earlier_by and no boost.
        self.run_turns(1, 119)
        self.assertIsNone(self.prop(0, "JAC_EVENT_FIRED_JAC_EVENT_ASAKURA_FALL"))
        self.run_turns(120, 120)  # (1573-1534)*12+8 = 476 months -> 1 + 119
        self.assertEqual(self.prop(0, "JAC_EVENT_FIRED_JAC_EVENT_ASAKURA_FALL"), 120)
        self.assertIsNone(self.prop(1, "JAC_EVENT_FIRED_JAC_EVENT_ASAKURA_FALL"))
        self.assertIn("[JAC_EVENTS] fired event=JAC_EVENT_ASAKURA_FALL player=0 civ=CIVILIZATION_JAC_ODA "
                      "reason=due turn=120 due=120 boost=", self.log())

    def test_an_engine_error_in_one_boost_does_not_stop_the_other_factions(self):
        self.g.FAIL_BOOST[1] = True
        self.run_turns(1, 30)
        self.assertEqual([b[0] for b in self.boosts("tech", self.teppo)], [2, 3])
        self.assertTrue(any("event=JAC_EVENT_TEPPO player=1" in line and "boost=tech=error:" in line
                            for line in self.log()))

    def test_a_failed_boost_is_left_unrecorded_and_retried_next_turn(self):
        self.g.FAIL_BOOST[1] = True
        self.run_turns(1, 30)
        self.assertIsNone(self.prop(1, "JAC_EVENT_BOOSTED_JAC_EVENT_TEPPO"))
        self.assertEqual(self.prop(2, "JAC_EVENT_BOOSTED_JAC_EVENT_TEPPO"), 30)
        self.g.FAIL_BOOST[1] = False
        self.run_turns(31, 32)
        self.assertEqual([(b[0], b[4]) for b in self.boosts("tech", self.teppo)], [(2, 30), (3, 30), (1, 31)])
        self.assertEqual(self.prop(1, "JAC_EVENT_BOOSTED_JAC_EVENT_TEPPO"), 31)

    def test_resource_event_without_the_resources_mod_records_nothing(self):
        self.g.Players[1].techs[self.tech("TECH_JAC_SENGOKU_MINE_MANAGEMENT")] = True  # Sado subjugation (Nagao)
        self.run_turns(1, 1)
        self.assertEqual(self.prop(1, "JAC_EVENT_FIRED_JAC_EVENT_SADO_SUBJUGATION"), 1)
        self.assertIsNone(self.game("JAC_EVENT_RESOURCES_JAC_EVENT_SADO_SUBJUGATION"))
        self.assertIn("[JAC_EVENTS] resource event=JAC_EVENT_SADO_SUBJUGATION skipped=resources_mod_not_loaded",
                      self.log())

    def test_only_reveal_rows_reach_the_resources_mod(self):
        self.lua.execute("""
          REVEALED = {}
          ExposedMembers.JAC_Resources = {
            RevealSite = function(id) table.insert(REVEALED, id) end,
            IsRevealed = function() return true end,
            GetSites = function() return { { id = "gold-sado-nishimikawa", plot = 10340 } } end,
          }""")
        self.g.Players[1].techs[self.tech("TECH_JAC_SENGOKU_MINE_MANAGEMENT")] = True  # Sado subjugation (reveal)
        self.g.Players[3].civics[self.civic("CIVIC_JAC_SENGOKU_NANBAN_ENVOYS")] = True  # Hirado (trade access only)
        self.run_turns(1, 2)
        self.assertEqual(self.prop(3, "JAC_EVENT_FIRED_JAC_EVENT_HIRADO_PORTUGUESE"), 1)
        # Sado subjugation and Aikawa (both Nagao, mine management), once each: not again on turn 2.
        self.assertEqual(list(self.g.REVEALED.values()), ["gold-sado-nishimikawa", "gold-sado-aikawa"])
        self.assertEqual(list(self.game("JAC_EVENT_RESOURCES_JAC_EVENT_SADO_SUBJUGATION").values()), [10340])

    def test_rename_stages_keep_the_latest_even_when_an_earlier_stage_fires_later(self):
        # Oda has no research: Honnoji (1582/6) at turn 147 -> Hashiba, Toyotomi (1586/10) at turn 160.
        self.run_turns(1, 147)
        self.assertEqual(self.game("JAC_CIV_NAME_CIVILIZATION_JAC_ODA"), "LOC_JAC_EVENT_HASHIBA_CIV_NAME")
        self.run_turns(148, 160)
        self.assertEqual(self.game("JAC_CIV_NAME_CIVILIZATION_JAC_ODA"), "LOC_JAC_EVENT_TOYOTOMI_CIV_NAME")
        # Researching Honnoji's other prerequisite later fires it (reason=prereq), but the name stays Toyotomi.
        self.g.Players[0].civics[self.civic("CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES")] = True
        self.run_turns(161, 161)
        self.assertEqual(self.prop(0, "JAC_EVENT_FIRED_JAC_EVENT_HONNOJI"), 161)
        self.assertEqual(self.game("JAC_CIV_NAME_CIVILIZATION_JAC_ODA"), "LOC_JAC_EVENT_TOYOTOMI_CIV_NAME")
        # Nagao -> Uesugi (1561/4) followed the year without the civic (owner decision 2a); Nagao only.
        self.assertEqual(self.game("JAC_CIV_NAME_CIVILIZATION_JAC_NAGAO"), "LOC_JAC_EVENT_UESUGI_CIV_NAME")
        self.assertIsNone(self.game("JAC_CIV_NAME_CIVILIZATION_JAC_DATE"))

    def test_nagao_becomes_uesugi_early_with_the_civic(self):
        self.g.Players[1].civics[self.civic("CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS")] = True
        self.run_turns(1, 2)
        self.assertEqual(self.prop(1, "JAC_EVENT_FIRED_JAC_EVENT_NAGAO_TO_UESUGI"), 1)
        self.assertEqual(self.game("JAC_CIV_NAME_CIVILIZATION_JAC_NAGAO"), "LOC_JAC_EVENT_UESUGI_CIV_NAME")

    def test_house_specific_name_only_for_its_house(self):
        # Kitano tea gathering (every faction, house Oda): Oda sees 北野大茶湯, the others 大茶会 (SSOT 3.2).
        tea = self.civic("CIVIC_JAC_SENGOKU_TEA_GATHERINGS")
        for player in range(len(CIVS)):
            self.g.Players[player].civics[tea] = True
        self.run_turns(1, 1)
        notes = {n[1]: n[2] for n in self.g.NOTES.values() if "KITANO" in n[2]}
        self.assertEqual(notes, {0: "LOC_JAC_EVENT_KITANO_TEA_NAME", 1: "LOC_JAC_EVENT_KITANO_TEA_GENERIC_NAME",
                                 2: "LOC_JAC_EVENT_KITANO_TEA_GENERIC_NAME", 3: "LOC_JAC_EVENT_KITANO_TEA_GENERIC_NAME"})

    def test_no_double_firing_after_a_reload(self):
        self.g.Players[1].civics[self.civic("CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS")] = True
        self.run_turns(1, 2)
        count, fired = self.game("JAC_EVENTS_FIRE_COUNT"), sum("] fired " in line for line in self.log())
        notes = len(self.g.NOTES)
        self.g.RELOAD()
        self.load_scripts()
        self.run_turns(2, 3)  # the loaded turn starts again, then one more
        self.assertEqual(self.game("JAC_EVENTS_FIRE_COUNT"), count)
        self.assertEqual(sum("] fired " in line for line in self.log()), fired)
        self.assertEqual(len(self.g.NOTES), notes)
        self.assertEqual(self.prop(0, "JAC_EVENT_FIRED_JAC_EVENT_TEPPO"), 1)

    def test_calendar_stages_do_not_depend_on_row_order(self):
        # Epic: 3 months/turn for 200 turns, then 2. Teppo (1543/9, 116 months) is turn 1 + ceil(116/3) = 40;
        # walking the 2-month stage first would give 59.
        data = tables()
        data["Speed"] = "GAMESPEED_EPIC"
        data["GameSpeed_Turns"].reverse()
        self.setUp(data)
        self.run_turns(1, 39)
        self.assertEqual(self.boosts("tech", self.teppo), [])
        self.run_turns(40, 40)
        self.assertEqual([b[0] for b in self.boosts("tech", self.teppo)], [1, 2, 3])

    # --- advisor popup (owner decision E5a) ---

    def raised(self) -> list[str]:
        return [item.Message for item in self.g.RAISED.values()]

    def test_advisor_popup_for_the_local_human_on_the_turn_the_event_happens(self):
        self.g.Players[0].techs[self.tech("TECH_JAC_SENGOKU_GRAIN_STORES")] = True  # Tenbun famine, early
        self.g.Players[3].civics[self.civic("CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER")] = True  # Shimazu (AI) only
        self.run_turns(1, 1)
        self.assertEqual(self.prop(3, "JAC_EVENT_FIRED_JAC_EVENT_SHIMAZU_UNIFY_1"), 1)
        self.g.UI_EVENT()  # LocalPlayerTurnBegin and LocalPlayerChanged: shown once
        # Grain stores also fires the Tensho earthquake early for Oda; Shimazu's event is not Oda's to see.
        self.assertEqual(self.raised(), [
            "LOC_JAC_EVENT_TENBUN_FAMINE_NAME[NEWLINE][NEWLINE]LOC_JAC_EVENT_TENBUN_FAMINE_ADVISOR",
            "LOC_JAC_EVENT_TENSHO_QUAKE_NAME[NEWLINE][NEWLINE]LOC_JAC_EVENT_TENSHO_QUAKE_ADVISOR"])
        item = self.g.RAISED[1]
        self.assertTrue(item.ShowPortrait)
        self.assertEqual((item.Button1Text, item.OptionsNum), ("LOC_JAC_EVENT_ADVISOR_OK", 1))
        self.assertIsNotNone(item.Button1Func)  # a nil callback would hand the popup to the tutorial
        self.assertEqual(len(item.UITriggers), 0)
        self.assertIn("[JAC_EVENTS_UI] advisor event=JAC_EVENT_TENBUN_FAMINE player=0 turn=1", self.log())
        self.run_turns(2, 2)
        self.g.UI_EVENT()
        self.assertEqual(len(self.raised()), 2)  # not again on later turns

    def test_advisor_popup_on_the_year_for_the_boost_and_not_again_on_research(self):
        self.g.LOCAL = 1  # Nagao without the research
        self.run_turns(1, 19)  # Tenbun famine: 1540 = turn 19
        self.g.UI_EVENT()
        self.assertEqual(len(self.raised()), 1)
        self.assertEqual(self.prop(1, "JAC_EVENT_BOOSTED_JAC_EVENT_TENBUN_FAMINE"), 19)
        self.g.Players[1].techs[self.tech("TECH_JAC_SENGOKU_GRAIN_STORES")] = True
        self.run_turns(20, 20)
        self.g.UI_EVENT()
        self.assertEqual(self.prop(1, "JAC_EVENT_FIRED_JAC_EVENT_TENBUN_FAMINE"), 20)
        self.assertEqual(len(self.raised()), 1)  # happened on turn 19 already

    def test_no_advisor_popup_without_a_local_player(self):
        self.g.LOCAL = -1  # autoplay observer
        self.g.Players[0].techs[self.tech("TECH_JAC_SENGOKU_GRAIN_STORES")] = True
        self.run_turns(1, 1)
        self.g.UI_EVENT()
        self.assertEqual(self.raised(), [])

    def test_advisor_popup_uses_the_generic_name_for_other_houses(self):
        # No advisor event is house-specific today; the popup still follows the notification rule.
        self.lua.execute("for row in GameInfo.JAC_HistoricalEvents() do if row.EventType == 'JAC_EVENT_TENBUN_FAMINE' "
                         "then row.House = 'CIVILIZATION_JAC_DATE'; row.GenericName = 'LOC_GENERIC' end end")
        self.g.Players[0].techs[self.tech("TECH_JAC_SENGOKU_GRAIN_STORES")] = True
        self.run_turns(1, 1)
        self.g.UI_EVENT()
        self.assertTrue(self.raised()[0].startswith("LOC_GENERIC[NEWLINE]"))


if __name__ == "__main__":
    unittest.main()
