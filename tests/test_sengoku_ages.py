"""Sengoku ages (documents/research/sengoku-ages.md §10 item 4).

AgesDataTests: the final values of §4.2 after the official Gathering Storm data and the pack's SQL
(tools/sengoku_cascade_audit.py builds the official database; skipped without a Civ6 install).
LedgerTests: Scripts/JAC_Sengoku_Ages.lua turn by turn against stubbed gameplay objects (lupa):
``uv run --no-project --with lupa python -m unittest tests.test_sengoku_ages``.
The stubs have only gameplay-side calls (no GetAllianceType / CanTriggerBoost / Calendar, nil there live).
"""

from __future__ import annotations

import importlib.util
import sqlite3
import sys
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_PoC_SengokuTrees"

try:
    import lupa
except ImportError:  # pragma: no cover - depends on the environment
    lupa = None


def _audit():
    name = "sengoku_cascade_audit"
    if name not in sys.modules:
        spec = importlib.util.spec_from_file_location(name, ROOT / f"tools/{name}.py")
        module = importlib.util.module_from_spec(spec)
        sys.modules[name] = module
        spec.loader.exec_module(module)
    return sys.modules[name]


AUDIT = _audit()


def ages_tables() -> dict:
    """The pack's own ages tables plus the calendar, from an in-memory load of the SQL (no official data)."""
    db = sqlite3.connect(":memory:")
    db.row_factory = lambda cursor, row: {c[0]: v for c, v in zip(cursor.description, row)}
    lines = (MOD / "Data/JAC_Sengoku_Ages.sql").read_text(encoding="utf-8").splitlines()
    sql = "\n".join(line.split("--")[0] for line in lines)
    for statement in sql.split(";"):
        if "CREATE TABLE JAC_" in statement or "INSERT INTO JAC_" in statement:
            db.execute(statement)
    out = {name: db.execute(f"SELECT * FROM {name}").fetchall()
           for name in ("JAC_SengokuReigns", "JAC_SengokuDarkAgePolicies", "JAC_SengokuAgeEventScores")}
    calendar = ET.parse(MOD / "Data/JAC_Sengoku_Calendar.xml").getroot()
    out["StartYear"] = int(calendar.findtext("./StartEras/Update/Set/Year"))
    out["GameSpeed_Turns"] = [
        {"GameSpeedType": r.findtext("GameSpeedType"), "TurnsPerIncrement": int(r.findtext("TurnsPerIncrement")),
         "MonthIncrement": int(r.findtext("MonthIncrement"))}
        for r in calendar.findall("./GameSpeed_Turns/Row")]
    return out


@unittest.skipUnless(AUDIT.available(), "official Civ6 data not installed")
class AgesDataTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.db = AUDIT._run_sengoku(AUDIT.official_db(), True)

    def q(self, sql: str, *args):
        return self.db.execute(sql, args).fetchall()

    def test_official_ages_and_dedications_are_off(self):
        params = dict(self.q("SELECT Name, Value FROM GlobalParameters WHERE Name LIKE '%AGE_SCORE_BASE_THRESHOLD' "
                             "OR Name LIKE 'THRESHOLD_SHIFT_%' OR Name LIKE 'COMMEMORATE_%'"))
        self.assertEqual(params.pop("DARK_AGE_SCORE_BASE_THRESHOLD"), "-9999")
        self.assertEqual(params.pop("GOLDEN_AGE_SCORE_BASE_THRESHOLD"), "9999")
        self.assertEqual(params.pop("COMMEMORATE_BASE_CHOICES_ALLOWED"), "0")
        self.assertEqual(params.pop("COMMEMORATE_OPTIONS_MAX"), "0")
        self.assertEqual(len(params), 9)  # every official THRESHOLD_SHIFT_* row
        self.assertEqual(set(params.values()), {"0"})
        self.assertEqual(self.q("SELECT COUNT(*) FROM Eras_XP2 WHERE IFNULL(EraScoreThresholdShift, 0) <> 0"), [(0,)])
        self.assertEqual(self.q("SELECT COUNT(*), SUM(EraScore) FROM Moments")[0][1], 0)
        self.assertGreater(self.q("SELECT COUNT(*) FROM Moments")[0][0], 100)  # moments stay in the timeline

    def test_no_attachment_of_an_era_score_modifier_in_any_table(self):
        tables = [r[0] for r in self.q("SELECT name FROM sqlite_master WHERE type='table'")]
        checked = 0
        for table in tables:
            columns = {c[1].lower() for c in self.q(f'PRAGMA table_info("{table}")')}
            if "modifierid" not in columns or table in ("Modifiers", "ModifierArguments", "ModifierStrings"):
                continue
            checked += 1
            rows = self.q(f'SELECT x.* FROM "{table}" x JOIN Modifiers m ON m.ModifierId = x.ModifierId '
                          f"WHERE m.ModifierType LIKE '%ERA_SCORE%'")
            self.assertEqual(rows, [], table)
        self.assertGreaterEqual(checked, 25)
        self.assertEqual(self.q("SELECT a.ModifierId FROM ModifierArguments a JOIN Modifiers m ON m.ModifierId = a.Value "
                                "WHERE a.Name = 'ModifierId' AND m.ModifierType LIKE '%ERA_SCORE%'"), [])

    def test_official_age_effects_and_age_policies_are_gone(self):
        self.assertEqual(self.q("SELECT ModifierId FROM TraitModifiers WHERE ModifierId IN "
                                "('GOLDEN_AGE_CITY_IDENTITY', 'DARK_AGE_CITY_IDENTITY')"), [])
        # The pack deletes the official age policies with the rest (Data/JAC_Sengoku_Unlocks.sql); Policies_XP1 keeps
        # orphan rows (no foreign key), which name no policy.
        self.assertEqual(self.q("SELECT PolicyType FROM Policies_XP1 JOIN Policies USING (PolicyType) "
                                "WHERE RequiresDarkAge = 1 OR RequiresGoldenAge = 1"), [])
        self.assertEqual(self.q("SELECT * FROM DiplomaticActions WHERE DiplomaticActionType = "
                                "'DIPLOACTION_DECLARE_GOLDEN_AGE_WAR'"), [])
        self.assertEqual(self.q("SELECT GameEraMaximumTurns FROM Eras_XP1 WHERE EraType = 'ERA_RENAISSANCE'"), [(10000,)])

    def test_reigns_policies_and_effect_modifiers(self):
        self.assertEqual(self.q("SELECT ReignType, Year, Month FROM JAC_SengokuReigns ORDER BY Year, Month"), [
            ("REIGN_KOJI", 1555, 11), ("REIGN_EIROKU", 1558, 3), ("REIGN_GENKI", 1570, 5), ("REIGN_TENSHO", 1573, 8),
            ("REIGN_BUNROKU", 1593, 1), ("REIGN_KEICHO", 1596, 12), ("REIGN_GENNA", 1615, 9), ("REIGN_KAN_EI", 1624, 4)])
        self.assertEqual(len(self.q("SELECT * FROM JAC_SengokuDarkAgePolicies")), 4)
        attached = [r[0] for r in self.q("SELECT ModifierId FROM TraitModifiers WHERE ModifierId LIKE 'JAC_AGE_%' "
                                         "AND TraitType = 'TRAIT_LEADER_MAJOR_CIV'")]
        self.assertEqual(len(attached), 12)
        # Every modifier, nested modifier, requirement set, requirement type and ability it names exists.
        mods = {r[0]: r[1:] for r in self.q("SELECT ModifierId, ModifierType, OwnerRequirementSetId, "
                                            "SubjectRequirementSetId FROM Modifiers WHERE ModifierId LIKE 'JAC_AGE_%'")}
        self.assertEqual(len(mods), 20)
        types = {r[0] for r in self.q("SELECT ModifierType FROM DynamicModifiers")}
        sets = {r[0] for r in self.q("SELECT RequirementSetId FROM RequirementSets")}
        for mid, (mtype, owner, subject) in mods.items():
            self.assertIn(mtype, types, mid)
            for rs in (owner, subject):
                if rs:
                    self.assertIn(rs, sets, mid)
                    self.assertTrue(self.q("SELECT 1 FROM RequirementSetRequirements r JOIN Requirements q "
                                           "USING (RequirementId) WHERE r.RequirementSetId = ?", rs), rs)
        for (value,) in self.q("SELECT Value FROM ModifierArguments WHERE ModifierId LIKE 'JAC_AGE_ATTACH_%' "
                               "AND Name = 'ModifierId'"):
            self.assertIn(value, mods)
        self.assertEqual(self.q("SELECT COUNT(*) FROM UnitAbilities WHERE UnitAbilityType = "
                                "'ABILITY_TWILIGHT_VALOR_ATTACK_BONUS'"), [(1,)])
        # Every property the requirements read is one the ledger writes.
        props = {r[0] for r in self.q("SELECT a.Value FROM RequirementArguments a JOIN Requirements q USING "
                                      "(RequirementId) WHERE q.RequirementId LIKE 'JAC_AGE_%' AND a.Name = 'PropertyName'")}
        policies = [r[0] for r in self.q("SELECT PolicyType FROM JAC_SengokuDarkAgePolicies")]
        written = {"JAC_AGE_GOLDEN"} | set(policies) | {p.replace("_POLICY_", "_ANCHOR_") for p in policies}
        self.assertLessEqual(props, written)

    def test_text_keys_are_translated(self):
        keys = {r[0] for r in self.q("SELECT Name FROM JAC_SengokuReigns UNION SELECT Announcement FROM JAC_SengokuReigns "
                                     "UNION SELECT Name FROM JAC_SengokuDarkAgePolicies "
                                     "UNION SELECT Announcement FROM JAC_SengokuDarkAgePolicies")}
        keys |= {"LOC_JAC_AGE_GOLDEN_ANNOUNCEMENT", "LOC_JAC_AGE_HEROIC_ANNOUNCEMENT", "LOC_JAC_AGE_NORMAL_ANNOUNCEMENT"}
        text = {}
        for row in ET.parse(MOD / "Text/JAC_Sengoku_Ages_Text.xml").getroot().findall("./LocalizedText/Row"):
            text.setdefault(row.get("Tag"), set()).add(row.get("Language"))
        self.assertEqual(set(text), keys)
        self.assertTrue(all(langs == {"ja_JP", "en_US", "ru_RU"} for langs in text.values()))


START_TURN = 16603  # live 1534 calendar
CIVS = ["CIVILIZATION_JAC_ODA", "CIVILIZATION_JAC_TAKEDA", "CIVILIZATION_JAC_HOJO", "CIVILIZATION_JAC_IMAGAWA"]

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
local techs, civics = {}, {}
for i = 1, 10 do techs[i] = { TechnologyType = "T" .. i } end
for i = 1, 10 do civics[i] = { CivicType = "C" .. i } end
GameInfo = {
  StartEras = { ERA_ANCIENT = { Year = tables.StartYear } },
  GameSpeeds = { [0] = { GameSpeedType = "GAMESPEED_STANDARD" } },
  GameSpeed_Turns = gameInfoTable(tables.GameSpeed_Turns),
  Technologies = gameInfoTable(techs), Civics = gameInfoTable(civics),
  JAC_SengokuReigns = gameInfoTable(tables.JAC_SengokuReigns),
  JAC_SengokuDarkAgePolicies = gameInfoTable(tables.JAC_SengokuDarkAgePolicies),
  JAC_SengokuAgeEventScores = gameInfoTable(tables.JAC_SengokuAgeEventScores),
}
CURRENT_TURN = startTurn
GameConfiguration = { GetGameSpeedType = function() return 0 end, GetStartTurn = function() return startTurn end }
local function properties()
  local values = {}
  return function(_, k) return values[k] end, function(_, k, v) values[k] = v end
end
Game = { GetCurrentGameTurn = function() return CURRENT_TURN end }
Game.GetProperty, Game.SetProperty = properties()
PLOTS = {}
Map = { GetPlot = function(x, y)
  local index = y * 100 + x
  if not PLOTS[index] then
    local p = { GetIndex = function() return index end, GetX = function() return x end, GetY = function() return y end }
    p.GetProperty, p.SetProperty = properties()
    PLOTS[index] = p
  end
  return PLOTS[index]
end }
CITIES = {}          -- list of {owner, x, y, capital}
ALLIED, TEAM, ALIVE = {}, {}, {}
Players, PlayerConfigurations = {}, {}
local function citiesOf(id)
  local list = {}
  for _, c in ipairs(CITIES) do if c.owner == id then table.insert(list, c) end end
  return list
end
local function cityObject(c) return { GetX = function() return c.x end, GetY = function() return c.y end } end
for i, civ in ipairs(civs) do
  local id = i - 1
  ALIVE[id], TEAM[id] = true, id
  local p = { research = { techs = {}, civics = {} } }
  p.GetID = function() return id end
  p.GetTeam = function() return TEAM[id] end
  p.GetCities = function() return {
    Members = function()
      local list, n = citiesOf(id), 0
      return function() n = n + 1; if list[n] then return n, cityObject(list[n]) end end
    end,
    GetCount = function() return #citiesOf(id) end,
    GetCapitalCity = function()
      for _, c in ipairs(citiesOf(id)) do if c.capital then return cityObject(c) end end
    end,
  } end
  p.GetTechs = function() return { HasTech = function(_, t) return p.research.techs[t] == true end } end
  p.GetCulture = function() return { HasCivic = function(_, c) return p.research.civics[c] == true end } end
  p.GetDiplomacy = function() return { HasAllied = function(_, b)
    local a, bb = math.min(id, b), math.max(id, b); return ALLIED[a .. "-" .. bb] == true end } end
  p.GetProperty, p.SetProperty = properties()
  Players[id] = p
  PlayerConfigurations[id] = { GetCivilizationTypeName = function() return civ end }
end
PlayerManager = { GetAliveMajors = function()
  local list = {}
  for id = 0, #civs - 1 do if ALIVE[id] then table.insert(list, Players[id]) end end
  return list
end }
ParameterTypes = { MESSAGE = "MESSAGE", SUMMARY = "SUMMARY" }
NotificationTypes = { USER_DEFINED_2 = 2 }
NOTES = {}
NotificationManager = { SendNotification = function(id, kind, data)
  table.insert(NOTES, { id, kind, data.MESSAGE, data.SUMMARY, CURRENT_TURN - startTurn + 1 })
end }
LOG = {}
function print(line) LOG[#LOG + 1] = line end
local handlers = { turn = {}, conquered = {}, built = {} }
function RESET_HANDLERS() handlers = { turn = {}, conquered = {}, built = {} } end
GameEvents = {
  OnGameTurnStarted = { Add = function(f) table.insert(handlers.turn, f) end },
  CityConquered = { Add = function(f) table.insert(handlers.conquered, f) end },
  CityBuilt = { Add = function(f) table.insert(handlers.built, f) end },
}
function START_TURN(turn)
  CURRENT_TURN = startTurn + turn - 1
  for _, f in ipairs(handlers.turn) do f(CURRENT_TURN) end
end
local function find(x, y) for i, c in ipairs(CITIES) do if c.x == x and c.y == y then return c, i end end end
function FOUND(owner, x, y, capital, silent)
  table.insert(CITIES, { owner = owner, x = x, y = y, capital = capital })
  if not silent then for _, f in ipairs(handlers.built) do f(owner, #CITIES, x, y) end end
end
function CAPTURE(capturer, x, y, raze)
  local c = find(x, y); local loser = c.owner; c.owner = capturer; c.capital = false
  for _, f in ipairs(handlers.conquered) do f(capturer, loser, 1, x, y) end
  if raze then local _, i = find(x, y); table.remove(CITIES, i) end
end
function TRANSFER(to, x, y)  -- CityManager.TransferCity: CityBuilt fires (live 2026-09-29)
  local c = find(x, y); c.owner = to; c.capital = false
  for _, f in ipairs(handlers.built) do f(to, 1, x, y) end
end
function LEDGER() return Game:GetProperty("JAC_AGES_LEDGER") end
"""


@unittest.skipIf(lupa is None, "lupa is not installed")
class LedgerTests(unittest.TestCase):
    """Players 0 Oda, 1 Takeda, 2 Hojo, 3 Imagawa. Oda: (1,1) home, (2,1). Takeda: (5,5) home, (6,5).
    Hojo: (9,9) home. Imagawa: (12,12) home, (13,12), (14,12)."""

    def setUp(self):
        self.lua = lupa.LuaRuntime(unpack_returned_tuples=True)
        self.lua.execute(STUBS, self.lua.table_from(ages_tables(), recursive=True), self.lua.table_from(CIVS),
                         START_TURN)
        self.g = self.lua.globals()
        for owner, x, y, capital in ((0, 1, 1, True), (0, 2, 1, False), (1, 5, 5, True), (1, 6, 5, False),
                                     (2, 9, 9, True), (3, 12, 12, True), (3, 13, 12, False), (3, 14, 12, False)):
            self.g.FOUND(owner, x, y, capital, True)  # map-placed cities exist before the scripts run
        self.load_script()

    def load_script(self):
        self.lua.execute((MOD / "Scripts/JAC_Sengoku_Ages.lua").read_text(encoding="utf-8"))

    def turns(self, first: int, last: int | None = None) -> None:
        for turn in range(first, (last or first) + 1):
            self.g.START_TURN(turn)

    def house(self, pid: int):
        return self.g.LEDGER().houses[f"p{pid}"]

    def score(self, pid: int) -> int:
        return self.house(pid).score

    def parts(self, pid: int) -> dict:
        return dict(self.house(pid).parts.items())

    def snapshot(self, value=None):
        value = self.g.LEDGER() if value is None else value
        if lupa.lua_type(value) == "table":
            return {k: self.snapshot(v) for k, v in value.items()}
        return value

    def plot(self, x: int, y: int):
        return self.g.PLOTS[y * 100 + x]

    # --- captures (§5.1, §5.2) ---

    def test_capture_classes_home_regain_and_castle(self):
        self.turns(1, 2)
        self.g.CAPTURE(1, 1, 1)  # Takeda takes Oda's home
        self.assertEqual(self.parts(1), {"home": 6})
        self.g.CAPTURE(1, 2, 1)  # an ordinary Oda castle: Oda has none left -> the house falls too
        self.assertEqual(self.parts(1), {"home": 6, "castle": 2, "ko": 8})
        self.g.CAPTURE(0, 1, 1)  # Oda retakes its own home: former own castle, not "home"
        self.assertEqual(self.parts(0), {"regain": 3})
        self.turns(3)
        self.assertEqual((self.score(0), self.score(1)), (3, 16))

    def test_capture_event_and_turn_start_difference_count_once(self):
        self.turns(1)
        self.g.CAPTURE(3, 9, 9)
        self.turns(2, 4)
        self.assertEqual(self.parts(3), {"home": 6, "ko": 8})
        self.assertEqual(len([k for k in self.g.LEDGER()["keys"] if k.startswith("cap:")]), 1)

    def test_recaptures_in_one_turn_each_score(self):
        self.turns(1)
        for capturer in (1, 0):  # A -> B -> A
            self.g.CAPTURE(capturer, 2, 1)
        self.assertEqual(self.parts(1), {"castle": 2})
        self.assertEqual(self.parts(0), {"regain": 3})
        self.g.CAPTURE(1, 2, 1)  # ... -> B again in the same turn
        self.assertEqual(self.parts(1), {"castle": 4})
        self.turns(2)
        self.assertEqual((self.score(0), self.score(1)), (3, 4))
        self.assertEqual(self.g.LEDGER().cities["plot102"].n, 3)

    def test_capture_then_raze_keeps_the_points_and_the_fall(self):
        self.turns(1)
        self.g.CAPTURE(0, 9, 9, True)  # Hojo's only castle, razed at once
        self.turns(2, 3)
        self.assertEqual(self.parts(0), {"home": 6, "ko": 8})
        self.assertIsNone(self.g.LEDGER().cities["plot909"])

    def test_transfer_scores_once_at_the_next_turn_start_not_on_city_built(self):
        self.turns(1)
        self.g.TRANSFER(2, 13, 12)  # surrender/transfer: CityBuilt fires, no points there
        self.assertEqual(self.score(2), 0)
        self.turns(2, 3)
        self.assertEqual(self.parts(2), {"castle": 2})

    def test_silent_owner_change_is_scored_by_the_difference(self):
        self.turns(1)
        for city in self.g.CITIES.values():
            if city.x == 14:
                city.owner = 0
        self.turns(2)
        self.assertEqual(self.parts(0), {"castle": 2})

    def test_settler_start_house_takes_its_first_founded_castle_as_home(self):
        # Hojo starts with settlers only.
        self.g.CITIES = self.lua.table_from([c for c in self.g.CITIES.values() if c.owner != 2])
        self.turns(1)
        self.assertIsNone(self.house(2).home)
        self.g.FOUND(2, 20, 20, True)
        self.g.FOUND(2, 21, 20, False)
        self.assertEqual(self.house(2).home, "plot2020")
        self.turns(2)
        self.g.CAPTURE(1, 20, 20)
        self.assertEqual(self.parts(1), {"home": 6})

    # --- research, events (§5.1) ---

    def test_research_scores_the_difference_but_not_the_knowledge_held_at_initialization(self):
        oda = self.g.Players[0]
        for i in range(4):
            oda.research.techs[i] = True  # starting knowledge, granted before the first turn start
        self.turns(1)
        self.assertEqual(self.score(0), 0)
        oda.research.techs[5] = True
        oda.research.civics[2] = True
        self.turns(2)
        self.assertEqual(self.parts(0), {"research": 2})

    def test_event_flags_score_once_per_house_and_the_reign_turn_flag_goes_to_the_new_period(self):
        self.turns(1, 30)
        self.g.Players[0].SetProperty(None, "JAC_EVENT_FIRED_JAC_EVENT_TEPPO_DENRAI", 30)
        self.turns(31, 40)
        self.assertEqual(self.parts(0), {"event": 2})
        self.turns(41, 65)
        # Ashio fires for Oda on turn 65 (old period) and for Takeda on the 改元 turn 66 (new period).
        self.g.Players[0].SetProperty(None, "JAC_EVENT_FIRED_JAC_EVENT_ASHIO_COPPER", 65)
        self.g.Players[1].SetProperty(None, "JAC_EVENT_FIRED_JAC_EVENT_ASHIO_COPPER", 66)  # ran before the ledger
        self.turns(66)
        history = self.g.LEDGER().history["r1"]
        self.assertEqual((history.p0.score, history.p1.score), (3, 0))
        self.assertEqual(self.parts(1), {"event": 1})
        self.g.Players[2].SetProperty(None, "JAC_EVENT_FIRED_JAC_EVENT_ASHIO_COPPER", 66)  # ran after the ledger
        self.turns(67, 70)
        self.assertEqual((self.score(0), self.score(1), self.score(2)), (0, 1, 1))

    # --- alliances (§5.4) ---

    def relations(self, before_ledger: bool):
        """Start alliances Hojo-Imagawa and Oda-Takeda as JAC_Japan_1534Relations.lua: allied and a number property
        at NewGameInitialized; the engine lapses them on turn 32 and the script re-makes them that turn."""
        self.g.ALLIED["2-3"] = True
        self.g.ALLIED["0-1"] = True
        self.g.Game.SetProperty(None, "JAC_1534_ALLIANCE_HOJO-IMAGAWA", START_TURN)
        self.g.Game.SetProperty(None, "JAC_1534_ALLIANCE_ODA-TAKEDA", START_TURN)
        remake = self.lua.eval("""function()
          if CURRENT_TURN - GameConfiguration.GetStartTurn() + 1 == 32 then
            for _, pair in ipairs({{"2-3", "HOJO-IMAGAWA"}, {"0-1", "ODA-TAKEDA"}}) do
              if type(Game:GetProperty("JAC_1534_ALLIANCE_" .. pair[2])) == "number" then
                ALLIED[pair[1]] = true; Game:SetProperty("JAC_1534_ALLIANCE_" .. pair[2], CURRENT_TURN)
              end
            end
          end
        end""")
        if before_ledger:
            self.g.RESET_HANDLERS()
            self.g.GameEvents.OnGameTurnStarted.Add(remake)
            self.load_script()
        else:
            self.g.GameEvents.OnGameTurnStarted.Add(remake)

    def lapse_at_32(self):
        self.turns(1, 31)
        self.g.ALLIED["2-3"] = False  # the engine lapses before any script runs
        self.g.ALLIED["0-1"] = False
        self.turns(32, 34)

    def test_start_alliance_lapse_and_remake_score_nothing_with_the_ledger_first(self):
        self.relations(before_ledger=False)
        self.lapse_at_32()
        self.assertEqual([self.score(p) for p in range(4)], [0, 0, 0, 0])
        self.assertEqual(self.g.LEDGER().allies["2-3"], "start")

    def test_start_alliance_lapse_and_remake_score_nothing_with_the_ledger_last(self):
        self.relations(before_ledger=True)
        self.lapse_at_32()
        self.assertEqual([self.score(p) for p in range(4)], [0, 0, 0, 0])

    def test_ended_start_alliance_lingers_without_points_then_a_new_alliance_scores(self):
        self.relations(before_ledger=False)
        self.turns(1, 10)
        self.g.Game.SetProperty(None, "JAC_1534_ALLIANCE_HOJO-IMAGAWA", False)  # EndStartAlliance; still allied
        self.turns(11, 20)
        self.assertEqual(self.score(2), 0)
        self.g.ALLIED["2-3"] = False  # the engine lapse ends it
        self.turns(21, 30)
        self.g.ALLIED["2-3"] = True  # made again through diplomacy
        self.turns(31)
        self.assertEqual((self.parts(2), self.parts(3)), ({"ally": 2}, {"ally": 2}))

    def test_alliance_scores_once_per_period_and_same_team_never(self):
        self.g.TEAM[1] = 0  # Oda and Takeda are one unification team
        self.turns(1, 5)
        self.g.ALLIED["0-1"] = True
        self.g.ALLIED["2-3"] = True
        self.turns(6)
        self.g.ALLIED["2-3"] = False
        self.turns(7)
        self.g.ALLIED["2-3"] = True
        self.turns(8)
        self.assertEqual([self.score(p) for p in range(4)], [0, 0, 2, 2])
        self.g.ALLIED["2-3"] = False
        self.turns(9, 66)
        self.g.ALLIED["2-3"] = True
        self.turns(67)
        self.assertEqual((self.score(2), self.score(3)), (2, 2))

    # --- judgement (§5.3) and effects (§7.1) ---

    def test_thresholds_follow_the_period_length_cities_and_past_ages(self):
        self.turns(1, 66)
        r1 = self.g.LEDGER().history["r1"]
        self.assertEqual((r1.turn, r1.length), (66, 65))
        # Imagawa: 3 cities, L = 65: golden ceil(31 x 65 / 50) = 41, dark ceil(17 x 1.3) = 23; no points -> dark.
        self.assertEqual((r1.p3.golden, r1.p3.dark, r1.p3.age), (41, 23, "DARK"))
        self.assertEqual((r1.p2.golden, r1.p2.dark), (38, 20))  # Hojo: 1 city
        # 永禄 at turn 73, L = 7; one past dark age: shift 3 - 5 = -2 -> golden ceil(26 x 0.14) = 4, dark ceil(12 x 0.14) = 2.
        self.g.Players[3].research.techs[1] = True
        self.g.Players[3].research.techs[2] = True
        self.g.Players[3].research.techs[3] = True
        self.g.Players[3].research.techs[4] = True
        self.turns(67, 73)
        r2 = self.g.LEDGER().history["r2"]
        self.assertEqual((r2.length, r2.p3.golden, r2.p3.dark, r2.p3.score), (7, 4, 2, 4))
        self.assertEqual(r2.p3.age, "HEROIC")  # golden right after dark
        self.assertEqual((self.house(3).golden, self.house(3).dark), (1, 1))
        # Oda: 2 cities, one past dark: golden ceil(25 x 0.14) = 4, dark ceil(11 x 0.14) = 2.
        self.assertEqual((r2.p0.golden, r2.p0.dark, r2.p0.age), (4, 2, "DARK"))
        self.assertEqual(self.house(0).dark, 2)
        # Hojo after two dark ages at 元亀 (L = 37): shift 1 - 10 -> dark ceil(5 x 0.74) = 4; never below 1.
        self.turns(74, 110)
        self.assertEqual(self.g.LEDGER().history["r3"].p2.dark, 4)
        self.house(2).dark = 9
        self.turns(111, 119)  # 天正, L = 9: (14 + 1 - 50) < 0 -> 1
        self.assertEqual((self.g.LEDGER().history["r4"].p2.golden, self.g.LEDGER().history["r4"].p2.dark), (1, 1))

    def test_reign_turns_follow_the_calendar(self):
        self.turns(1, 200)
        history = self.g.LEDGER().history
        self.assertEqual([history[f"r{i}"].turn for i in range(1, 7)], [66, 73, 110, 119, 178, 189])
        self.assertIsNone(history["r7"])
        self.assertEqual(self.g.LEDGER().reign, 6)
        notes = [tuple(n.values()) for n in self.g.NOTES.values()]
        self.assertEqual(len(notes), 6 * 4)
        self.assertIn((0, 2, "LOC_JAC_REIGN_KOJI_ANNOUNCEMENT", "LOC_JAC_AGE_DARK_VALOR_ANNOUNCEMENT", 66), notes)

    def plot_properties(self) -> dict:
        names = ["JAC_AGE_GOLDEN"] + [f"JAC_AGE_{kind}_{p}" for kind in ("POLICY", "ANCHOR")
                                      for p in ("VALOR", "ELITE", "PIRATE", "TEMPLE")]
        return {i: tuple(p.GetProperty(None, n) for n in names) for i, p in self.g.PLOTS.items()}

    def test_property_writer_is_idempotent_with_one_anchor_per_house(self):
        self.turns(1, 66)  # nobody scored: every house is dark with the first policy
        self.house(1).age, self.house(1).policy = "GOLDEN", ""  # make Takeda golden
        self.turns(67)
        self.assertEqual(self.plot(12, 12).GetProperty(None, "JAC_AGE_ANCHOR_VALOR"), 1)
        self.g.FOUND(3, 11, 12, False)  # CityBuilt rewrites; the smaller plot index becomes Imagawa's anchor
        first = self.plot_properties()
        self.turns(68)
        self.g.START_TURN(68)  # the same writes again change nothing
        self.assertEqual(self.plot_properties(), first)
        for x in (11, 12, 13, 14):
            plot = self.plot(x, 12)
            self.assertEqual(plot.GetProperty(None, "JAC_AGE_POLICY_VALOR"), 1)
            self.assertEqual(plot.GetProperty(None, "JAC_AGE_ANCHOR_VALOR"), 1 if x == 11 else 0)
            self.assertEqual(plot.GetProperty(None, "JAC_AGE_GOLDEN"), 0)
        for x in (5, 6):
            self.assertEqual(self.plot(x, 5).GetProperty(None, "JAC_AGE_GOLDEN"), 1)
            self.assertEqual(self.plot(x, 5).GetProperty(None, "JAC_AGE_ANCHOR_VALOR"), 0)
        anchors = [i for i, p in self.g.PLOTS.items() if p.GetProperty(None, "JAC_AGE_ANCHOR_VALOR") == 1]
        self.assertEqual(len(anchors), 3)  # Oda, Hojo, Imagawa are dark; one anchor each

    def test_load_does_not_count_again(self):
        self.turns(1, 5)
        self.g.CAPTURE(1, 2, 1)
        self.g.Players[1].research.techs[3] = True
        self.turns(6)
        before = self.snapshot()
        # Load: the scripts run again from the top; the saved Game property is all that survives.
        self.g.RESET_HANDLERS()
        self.load_script()
        self.assertEqual(self.snapshot(), before)
        self.turns(7, 8)
        self.assertEqual(self.parts(1), {"castle": 2, "research": 1})


if __name__ == "__main__":
    unittest.main()
