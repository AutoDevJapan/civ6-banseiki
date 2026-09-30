"""Sengoku ages (documents/research/sengoku-ages.md §10 item 4).

AgesDataTests: the final values of §4.2 after the official Gathering Storm data and the pack's SQL
(tools/sengoku_cascade_audit.py builds the official database; skipped without a Civ6 install).
LedgerTests: Scripts/JAC_Sengoku_Ages.lua turn by turn against stubbed gameplay objects (lupa):
``uv run --no-project --with lupa python -m unittest tests.test_sengoku_ages``.
The stubs have only gameplay-side calls (no GetAllianceType / CanTriggerBoost / Calendar, nil there live).
"""

from __future__ import annotations

import importlib.util
import json
import re
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


def _generator():
    name = "generate_sengoku_reigns"
    if name not in sys.modules:
        spec = importlib.util.spec_from_file_location(name, ROOT / f"tools/{name}.py")
        module = importlib.util.module_from_spec(spec)
        sys.modules[name] = module
        spec.loader.exec_module(module)
    return sys.modules[name]


REIGNS_GEN = _generator()
REIGNS_JSON = json.loads((ROOT / "documents/spec/data/sengoku-reigns.json").read_text(encoding="utf-8"))


def ages_tables() -> dict:
    """The pack's own ages tables plus the calendar, from an in-memory load of the SQL (no official data)."""
    db = sqlite3.connect(":memory:")
    db.row_factory = lambda cursor, row: {c[0]: v for c, v in zip(cursor.description, row)}
    lines = []
    for name in ("Data/JAC_Sengoku_Ages.sql", "Data/JAC_Sengoku_Reigns.sql"):  # the modinfo's order
        lines += (MOD / name).read_text(encoding="utf-8").splitlines()
    sql = "\n".join(line.split("--")[0] for line in lines)
    for statement in sql.split(";"):
        if "CREATE TABLE JAC_" in statement or "INSERT INTO JAC_" in statement:
            db.execute(statement)
    out = {name: db.execute(f"SELECT * FROM {name}").fetchall()
           for name in ("JAC_SengokuReigns", "JAC_SengokuDarkAgePolicies", "JAC_SengokuAgeEventScores",
                        "JAC_SengokuDarkAgePolicyDefaults")}
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
        rows = self.q("SELECT ReignType, Year, Month, ForcedAge FROM JAC_SengokuReigns ORDER BY Year, Month")
        self.assertEqual(rows, [("REIGN_" + r["id"], r["year"], r["month"], r.get("forced_age"))
                                for r in REIGNS_JSON["reigns"]])
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

    def test_every_house_has_a_valid_default_dark_policy(self):
        # §6.2: the per-house table covers exactly the 22 houses of JAC_PoC_CustomCivs, each with a real policy.
        civs = set()
        for path in (ROOT / "poc/JAC_PoC_CustomCivs/Data").glob("*.sql"):
            civs |= set(re.findall(r"\('(CIVILIZATION_JAC_[A-Z_]+)', 'LOC_", path.read_text(encoding="utf-8")))
        self.assertEqual(len(civs), 22)
        rows = dict(self.q("SELECT CivilizationType, PolicyType FROM JAC_SengokuDarkAgePolicyDefaults"))
        self.assertEqual(set(rows), civs)
        policies = {r[0] for r in self.q("SELECT PolicyType FROM JAC_SengokuDarkAgePolicies")}
        self.assertLessEqual(set(rows.values()), policies)
        self.assertGreater(len(set(rows.values())), 1)  # not every house on one policy

    def test_text_keys_are_translated(self):
        keys = {r[0] for r in self.q("SELECT Name FROM JAC_SengokuReigns UNION SELECT Announcement FROM JAC_SengokuReigns "
                                     "UNION SELECT Notification FROM JAC_SengokuReigns "
                                     "UNION SELECT Name FROM JAC_SengokuDarkAgePolicies "
                                     "UNION SELECT Description FROM JAC_SengokuDarkAgePolicies "
                                     "UNION SELECT Announcement FROM JAC_SengokuDarkAgePolicies")}
        keys |= {"LOC_JAC_AGE_GOLDEN_ANNOUNCEMENT", "LOC_JAC_AGE_HEROIC_ANNOUNCEMENT", "LOC_JAC_AGE_NORMAL_ANNOUNCEMENT"}
        # The choice screen (UI/JAC_SengokuDarkPolicy.lua / .xml).
        ui = "".join((MOD / f"UI/JAC_SengokuDarkPolicy.{ext}").read_text(encoding="utf-8") for ext in ("lua", "xml"))
        ui_keys = set(re.findall(r"LOC_JAC_AGE_DARK_CHOICE_[A-Z]+", ui))
        self.assertEqual(ui_keys, {"LOC_JAC_AGE_DARK_CHOICE_TITLE", "LOC_JAC_AGE_DARK_CHOICE_BODY",
                                   "LOC_JAC_AGE_DARK_CHOICE_CHOOSE", "LOC_JAC_AGE_DARK_CHOICE_LATER"})
        keys |= ui_keys
        # The 改元 screen, the era score screen and their shared view (UI/JAC_SengokuReignPopup.lua / .xml,
        # UI/JAC_EraProgressPanel_Sengoku.lua, UI/JAC_SengokuAgesView.lua) and the notification heading.
        screens = "".join((MOD / name).read_text(encoding="utf-8") for name in (
            "UI/JAC_SengokuReignPopup.lua", "UI/JAC_SengokuReignPopup.xml", "UI/JAC_EraProgressPanel_Sengoku.lua",
            "UI/JAC_SengokuAgesView.lua", "UI/JAC_ActionPanel_Sengoku.lua", "Scripts/JAC_Sengoku_Ages.lua"))
        screen_keys = {k for k in re.findall(r"LOC_JAC_[A-Z0-9_]+", screens) if not k.endswith("_")}
        screen_keys |= {f"LOC_JAC_AGE_PART_{part}" for part in ("HOME", "REGAIN", "CASTLE", "KO", "ALLY", "RESEARCH", "EVENT")}
        self.assertLessEqual({"LOC_JAC_AGE_ERA_NAME", "LOC_JAC_AGE_ERA_ENDS_IN", "LOC_JAC_REIGN_TENBUN_NAME"}, screen_keys)
        keys |= screen_keys
        text = {}
        for name in ("Text/JAC_Sengoku_Ages_Text.xml", "Text/JAC_Sengoku_Reigns_Text.xml"):
            for row in ET.parse(MOD / name).getroot().findall("./LocalizedText/Row"):
                text.setdefault(row.get("Tag"), set()).add(row.get("Language"))
        self.assertEqual(set(text), keys)
        self.assertTrue(all(langs == {"ja_JP", "en_US", "ru_RU"} for langs in text.values()))


class ReignFilesTests(unittest.TestCase):
    def test_generated_reign_files_are_current(self):
        for path, content in REIGNS_GEN.outputs().items():
            self.assertEqual(path.read_text(encoding="utf-8"), content, f"run tools/generate_sengoku_reigns.py: {path}")


START_TURN = 16603  # live 1534 calendar
CIVS = ["CIVILIZATION_JAC_ODA", "CIVILIZATION_JAC_TAKEDA", "CIVILIZATION_JAC_HOJO", "CIVILIZATION_JAC_IMAGAWA"]

STUBS = r"""
local tables, civs, startTurn, speed = ...
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
  GameSpeeds = { [0] = { GameSpeedType = speed or "GAMESPEED_STANDARD" } },
  GameSpeed_Turns = gameInfoTable(tables.GameSpeed_Turns),
  Technologies = gameInfoTable(techs), Civics = gameInfoTable(civics),
  JAC_SengokuReigns = gameInfoTable(tables.JAC_SengokuReigns),
  JAC_SengokuDarkAgePolicies = gameInfoTable(tables.JAC_SengokuDarkAgePolicies),
  JAC_SengokuAgeEventScores = gameInfoTable(tables.JAC_SengokuAgeEventScores),
  JAC_SengokuDarkAgePolicyDefaults = gameInfoTable(tables.JAC_SengokuDarkAgePolicyDefaults),
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
ALLIED, FRIENDS, TEAM, ALIVE = {}, {}, {}, {}
NO_FRIENDSHIP_API = false  -- true: the gameplay diplomacy object has no HasDeclaredFriendship
Players, PlayerConfigurations = {}, {}
local function citiesOf(id)
  local list = {}
  for _, c in ipairs(CITIES) do if c.owner == id then table.insert(list, c) end end
  return list
end
local function cityObject(c) return { GetX = function() return c.x end, GetY = function() return c.y end } end
FREE_CITIES = 62  -- a non-major player (the free cities; a city-state behaves the same here)
local function makePlayer(id, civ)
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
  p.GetDiplomacy = function()
    local d = { HasAllied = function(_, b)
      local a, bb = math.min(id, b), math.max(id, b); return ALLIED[a .. "-" .. bb] == true end }
    if not NO_FRIENDSHIP_API then d.HasDeclaredFriendship = function(_, b)
      local a, bb = math.min(id, b), math.max(id, b); return FRIENDS[a .. "-" .. bb] == true end end
    return d
  end
  p.GetProperty, p.SetProperty = properties()
  Players[id] = p
  PlayerConfigurations[id] = { GetCivilizationTypeName = function() return civ end }
end
for i, civ in ipairs(civs) do makePlayer(i - 1, civ) end
makePlayer(FREE_CITIES, "CIVILIZATION_FREE_CITIES")
local function alive(ids)
  local list = {}
  for _, id in ipairs(ids) do if ALIVE[id] then table.insert(list, Players[id]) end end
  return list
end
local majors = {}
for i = 1, #civs do majors[i] = i - 1 end
PlayerManager = {
  GetAliveMajors = function() return alive(majors) end,
  GetAlive = function()  -- the free cities come first, as the order is not the ID order live
    local ids = { FREE_CITIES }
    for _, id in ipairs(majors) do table.insert(ids, id) end
    return alive(ids)
  end,
}
ParameterTypes = { MESSAGE = "MESSAGE", SUMMARY = "SUMMARY" }
NotificationTypes = { USER_DEFINED_2 = 2 }
NOTES = {}
NotificationManager = { SendNotification = function(id, kind, data)
  table.insert(NOTES, { id, kind, data.MESSAGE, data.SUMMARY, CURRENT_TURN - startTurn + 1 })
end }
LOG = {}
function print(line) LOG[#LOG + 1] = line end
local handlers = { turn = {}, conquered = {}, built = {}, choose = {} }
function RESET_HANDLERS() handlers = { turn = {}, conquered = {}, built = {}, choose = {} } end
GameEvents = {
  OnGameTurnStarted = { Add = function(f) table.insert(handlers.turn, f) end },
  CityConquered = { Add = function(f) table.insert(handlers.conquered, f) end },
  CityBuilt = { Add = function(f) table.insert(handlers.built, f) end },
  JAC_SengokuDarkPolicyChoose = { Add = function(f) table.insert(handlers.choose, f) end },
}
-- EXECUTE_SCRIPT from the choice screen: the engine passes the requesting player and the parameter table.
function CHOOSE(playerID, params)
  for _, f in ipairs(handlers.choose) do f(playerID, params) end
end
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

    # --- castles held by a non-major (Gathering Storm maps, SSOT 3.1) ---

    def test_castle_through_the_free_cities_keeps_its_history_and_regain_scores(self):
        self.turns(1)
        self.g.TRANSFER(self.g.FREE_CITIES, 2, 1)  # Oda's castle flips to the free cities (no event, a diff)
        self.turns(2, 3)
        record = self.g.LEDGER().cities["plot102"]
        self.assertEqual((record.owner, record.original, record.n), (62, 0, 1))
        self.assertIsNone(self.g.LEDGER().houses["p62"])
        self.g.CAPTURE(0, 2, 1)  # Oda takes it back: its former own castle
        self.turns(4)
        self.assertEqual(self.parts(0), {"regain": 3})
        self.assertEqual(self.g.LEDGER().cities["plot102"].n, 2)

    def test_home_taken_from_a_city_state_is_still_a_home(self):
        self.turns(1)
        self.g.CAPTURE(self.g.FREE_CITIES, 1, 1)  # a non-major takes Oda's home: nobody scores
        self.turns(2)
        self.assertEqual(self.house(0).home, "plot101")
        self.g.CAPTURE(1, 1, 1)  # Takeda takes it from the non-major
        self.turns(3)
        self.assertEqual(self.parts(1), {"home": 6})
        self.assertEqual([self.score(p) for p in (0, 2, 3)], [0, 0, 0])

    def test_castle_of_a_non_major_has_no_age_flags(self):
        self.turns(1, 66)  # every house dark; Imagawa's default is Temple Learning
        self.assertEqual(self.plot(14, 12).GetProperty(None, "JAC_AGE_POLICY_TEMPLE"), 1)
        self.g.TRANSFER(self.g.FREE_CITIES, 14, 12)
        self.assertEqual(self.plot_properties()[1214], (0,) * 9)

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
        self.g.Players[0].SetProperty(None, "JAC_EVENT_FIRED_JAC_EVENT_TEPPO", 30)
        self.turns(31, 40)
        self.assertEqual(self.parts(0), {"event": 2})
        self.turns(41, 65)
        # Ikuno fires for Oda on turn 65 (old period) and for Takeda on the 改元 turn 66 (new period).
        self.g.Players[0].SetProperty(None, "JAC_EVENT_FIRED_JAC_EVENT_IKUNO_SILVER", 65)
        self.g.Players[1].SetProperty(None, "JAC_EVENT_FIRED_JAC_EVENT_IKUNO_SILVER", 66)  # ran before the ledger
        self.turns(66)
        history = self.g.LEDGER().history["r1"]
        self.assertEqual((history.p0.score, history.p1.score), (3, 0))
        self.assertEqual(self.parts(1), {"event": 1})
        self.g.Players[2].SetProperty(None, "JAC_EVENT_FIRED_JAC_EVENT_IKUNO_SILVER", 66)  # ran after the ledger
        self.turns(67, 70)
        self.assertEqual((self.score(0), self.score(1), self.score(2)), (0, 1, 1))

    # --- alliances (§5.4) ---

    def relations(self, before_ledger: bool):
        """Start alliances Hojo-Imagawa and Oda-Takeda as JAC_Japan_1534Relations.lua: allied and a number property
        at NewGameInitialized; the engine lapses them on turn 32 and the script re-makes them that turn."""
        self.g.ALLIED["2-3"] = True
        self.g.ALLIED["0-1"] = True
        self.g.FRIENDS["2-3"] = True  # the script declares friendship before the alliance (:151-154)
        self.g.FRIENDS["0-1"] = True
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
        self.g.FRIENDS["2-3"] = False  # re-made pairs lose the friendship; the AI declared it again (live diplo-17)
        self.turns(32, 33)
        self.g.FRIENDS["2-3"] = True
        self.turns(34)

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
        # Imagawa: 3 cities, L = 65: golden ceil(31 x 65 / 300) = 7, dark floor(17 x 65 / 300) = 3; no points -> dark.
        self.assertEqual((r1.p3.golden, r1.p3.dark, r1.p3.age), (7, 3, "DARK"))
        self.assertEqual((r1.p2.golden, r1.p2.dark), (7, 3))  # Hojo: 1 city
        # 永禄 at turn 73, L = 7; one past dark age: shift 3 - 5 = -2 -> dark floor(12 x 7 / 300) = 0, golden
        # ceil(26 x 7 / 300) = 1 raised to MIN_GOLDEN = 2.
        self.g.Players[3].research.techs[1] = True
        self.g.Players[3].research.techs[2] = True
        self.g.Players[3].research.techs[3] = True
        self.g.Players[3].research.techs[4] = True
        self.turns(67, 73)
        r2 = self.g.LEDGER().history["r2"]
        self.assertEqual((r2.length, r2.p3.golden, r2.p3.dark, r2.p3.score), (7, 2, 0, 4))
        self.assertEqual(r2.p3.age, "HEROIC")  # golden right after dark
        self.assertEqual((self.house(3).golden, self.house(3).dark), (1, 1))
        # Oda: 2 cities, one past dark: dark floor(11 x 7 / 300) = 0, golden 2; no points in 7 turns is normal.
        self.assertEqual((r2.p0.golden, r2.p0.dark, r2.p0.age), (2, 0, "NORMAL"))
        self.assertEqual(self.house(0).dark, 1)
        # Hojo after one dark age at 元亀 (L = 37): shift 1 - 5 -> golden ceil(24 x 37 / 300) = 3,
        # dark floor(10 x 37 / 300) = 1.
        self.turns(74, 110)
        r3 = self.g.LEDGER().history["r3"]
        self.assertEqual((r3.p2.golden, r3.p2.dark), (3, 1))
        self.house(2).dark = 9
        self.turns(111, 119)  # 天正, L = 9: (14 + 1 - 50) < 0 -> dark 0, golden 2
        self.assertEqual((self.g.LEDGER().history["r4"].p2.golden, self.g.LEDGER().history["r4"].p2.dark), (2, 0))

    def test_reign_turns_follow_the_calendar(self):
        self.turns(1, 200)
        history = self.g.LEDGER().history
        self.assertEqual([history[f"r{i}"].turn for i in range(1, 7)], [66, 73, 110, 119, 178, 189])
        self.assertIsNone(history["r7"])
        self.assertEqual(self.g.LEDGER().reign, 6)
        notes = [tuple(n.values()) for n in self.g.NOTES.values()]
        self.assertEqual(len(notes), 6 * 4)
        # L-1: the heading names the new era (「改元：弘治の世」).
        self.assertIn((0, 2, "LOC_JAC_REIGN_KOJI_NOTIFICATION", "LOC_JAC_AGE_DARK_ELITE_ANNOUNCEMENT", 66), notes)
        self.assertEqual({n[2] for n in notes}, {f"LOC_JAC_REIGN_{r}_NOTIFICATION" for r in
                                                 ("KOJI", "EIROKU", "GENKI", "TENSHO", "BUNROKU", "KEICHO")})

    def test_projection_for_the_era_score_screen_matches_the_next_judgement(self):
        self.turns(1, 40)
        l = self.g.LEDGER()
        self.assertEqual(l.nextDue, 66)
        projected = {pid: (self.house(pid).next.golden, self.house(pid).next.dark) for pid in range(4)}
        self.turns(41, 66)  # no city changes: the 弘治 judgement uses the same thresholds
        r1 = self.g.LEDGER().history["r1"]
        self.assertEqual(projected, {pid: (r1[f"p{pid}"].golden, r1[f"p{pid}"].dark) for pid in range(4)})
        self.assertEqual(self.g.LEDGER().nextDue, 73)  # 永禄
        self.turns(1757)  # 遅代 (2119), the last 改元: nothing to project
        self.assertEqual(self.g.LEDGER().reign, len(REIGNS_JSON["reigns"]))
        self.assertIsNone(self.g.LEDGER().nextDue)
        self.assertIsNone(self.house(0).next)

    # --- declared friendships (§5.4, owner decision L-2) ---

    def test_new_friendship_scores_one_point_each_once_per_period(self):
        self.turns(1, 5)
        self.g.FRIENDS["2-3"] = True
        self.turns(6)
        self.assertEqual((self.parts(2), self.parts(3)), ({"ally": 1}, {"ally": 1}))
        self.g.FRIENDS["2-3"] = False  # lapsed or ended, then declared again in the same period
        self.turns(7)
        self.g.FRIENDS["2-3"] = True
        self.turns(8)
        self.assertEqual((self.score(2), self.score(3)), (1, 1))
        self.g.ALLIED["2-3"] = True  # then an alliance: 2 more each, in the same alliance part
        self.turns(9)
        self.assertEqual((self.parts(2), self.parts(3)), ({"ally": 3}, {"ally": 3}))
        self.g.FRIENDS["2-3"] = False
        self.turns(10, 66)  # 弘治: a new period
        self.g.FRIENDS["2-3"] = True
        self.turns(67)
        self.assertEqual((self.score(2), self.score(3)), (1, 1))

    def test_friendship_of_the_start_and_of_one_team_never_scores(self):
        self.relations(before_ledger=False)  # the start pairs are friends and allies
        self.g.TEAM[1] = 0
        self.turns(1, 5)
        self.g.Game.SetProperty(None, "JAC_1534_ALLIANCE_ODA-TAKEDA", False)  # ended; one team from now on
        self.g.FRIENDS["0-1"] = False
        self.turns(6)
        self.g.FRIENDS["0-1"] = True
        self.lapse_at_32()
        self.assertEqual([self.score(p) for p in range(4)], [0, 0, 0, 0])
        self.assertEqual(self.g.LEDGER().friends["2-3"], "start")

    def test_friendship_existing_at_initialization_does_not_score(self):
        self.g.FRIENDS["1-2"] = True  # not a start pair, but friends before the first turn start
        self.turns(1, 5)
        self.assertEqual(self.score(1), 0)

    def test_without_the_gameplay_friendship_call_nothing_scores_and_the_log_says_so(self):
        self.g.NO_FRIENDSHIP_API = True
        self.turns(1, 5)
        self.g.FRIENDS["2-3"] = True
        self.turns(6, 8)
        self.assertEqual(self.score(2), 0)
        self.assertEqual(len([x for x in self.g.LOG.values() if "no gameplay HasDeclaredFriendship" in x]), 1)
        self.g.ALLIED["2-3"] = True  # alliances still score
        self.turns(9)
        self.assertEqual(self.parts(2), {"ally": 2})

    # --- the reign table up to 遅代 (owner decision L-3) ---

    def test_reign_table_from_koji_to_chidai(self):
        reigns = REIGNS_JSON["reigns"]
        self.assertEqual(len(reigns), 47)
        self.assertEqual([r["id"] for r in reigns[:8]],
                         ["KOJI", "EIROKU", "GENKI", "TENSHO", "BUNROKU", "KEICHO", "GENNA", "KAN_EI"])
        self.assertEqual([r["id"] for r in reigns[-6:]], ["MEIJI", "TAISHO", "SHOWA", "HEISEI", "REIWA", "CHIDAI"])
        dates = [(r["year"], r["month"]) for r in reigns]
        self.assertEqual(dates, sorted(dates))
        self.assertEqual(len(set(dates)), len(dates))
        self.assertEqual(dates[-2:], [(2019, 5), (2119, 5)])  # 令和 lasts 100 years
        self.assertEqual([r["id"] for r in reigns if r.get("forced_age")], ["CHIDAI"])
        self.assertEqual(len({r["id"] for r in reigns}), len(reigns))

    def test_reign_turns_at_every_speed_match_the_generator_and_never_share_a_turn(self):
        expected = REIGNS_GEN.turns_by_speed()
        self.assertEqual(expected["GAMESPEED_STANDARD"][:8], [66, 73, 110, 119, 178, 189, 246, 271])
        self.assertEqual(expected["GAMESPEED_STANDARD"][-2:], [1457, 1757])
        for speed in REIGNS_GEN.SPEEDS:
            with self.subTest(speed=speed):
                lua = lupa.LuaRuntime(unpack_returned_tuples=True)
                lua.execute(STUBS, lua.table_from(ages_tables(), recursive=True), lua.table_from(CIVS), START_TURN, speed)
                g = lua.globals()
                lua.execute((MOD / "Scripts/JAC_Sengoku_Ages.lua").read_text(encoding="utf-8"))
                g.START_TURN(1)
                dues = []
                while g.LEDGER().nextDue is not None:  # jump from 改元 to 改元
                    dues.append(g.LEDGER().nextDue)
                    g.START_TURN(dues[-1])
                    self.assertEqual(g.LEDGER().history[f"r{len(dues)}"].turn, dues[-1])
                self.assertEqual(dues, expected[speed])
                self.assertEqual(dues, sorted(set(dues)))  # strictly increasing: no period of length 0

    def test_reigns_on_one_turn_merge_into_the_last_and_are_judged_once(self):
        tables = ages_tables()
        # A made-up 改元 a month before 弘治: both fall in turn 66 (1555-09..12), a period of length 0.
        tables["JAC_SengokuReigns"].insert(0, {"ReignType": "REIGN_TEST", "Year": 1555, "Month": 10,
                                               "Name": "LOC_T", "Announcement": "LOC_T", "Notification": "LOC_T_NOTE"})
        self.lua = lupa.LuaRuntime(unpack_returned_tuples=True)
        self.lua.execute(STUBS, self.lua.table_from(tables, recursive=True), self.lua.table_from(CIVS), START_TURN)
        self.g = self.lua.globals()
        self.g.FOUND(3, 12, 12, True, True)
        self.load_script()
        self.turns(1, 65)
        self.assertEqual(self.g.LEDGER().nextDue, 66)
        self.assertEqual(self.house(3).next.golden, 7)  # the projection is for the merged period (L = 65)
        self.g.Players[3].research.techs[1] = True
        self.turns(66, 73)
        l = self.g.LEDGER()
        self.assertIsNone(l.history["r1"])  # REIGN_TEST: merged, never judged
        r2 = l.history["r2"]  # 弘治: judged once for the whole period since the start
        self.assertEqual((r2.turn, r2.length, r2["from"]), (66, 65, 0))
        self.assertEqual(l.history["r3"]["from"], 2)
        notes = [tuple(n.values()) for n in self.g.NOTES.values()]
        self.assertEqual([n[2] for n in notes if n[4] == 66], ["LOC_JAC_REIGN_KOJI_NOTIFICATION"] * 4)
        self.assertNotIn("LOC_T_NOTE", {n[2] for n in notes})

    def test_chidai_makes_every_house_dark_whatever_its_score_and_lasts_to_the_end(self):
        self.turns(1, 5)
        self.turns(1457)  # 令和 (2019)
        self.assertEqual(self.g.LEDGER().history["r46"].turn, 1457)
        self.house(1).age = "DARK"  # a dark house reaching golden would be heroic at an ordinary 改元
        self.assertEqual(self.house(0).next.forced, "DARK")
        for pid in range(4):
            self.house(pid).score = 1000
        self.turns(1757)  # 遅代 (2119-05)
        l = self.g.LEDGER()
        r = l.history["r47"]
        self.assertEqual((r.turn, r.forced), (1757, "DARK"))
        self.assertEqual([r[f"p{pid}"].age for pid in range(4)], ["DARK"] * 4)
        self.assertEqual([r[f"p{pid}"].score for pid in range(4)], [1000] * 4)
        self.assertEqual([self.house(p).policy for p in range(4)],
                         ["JAC_AGE_POLICY_ELITE", "JAC_AGE_POLICY_ELITE", "JAC_AGE_POLICY_VALOR", "JAC_AGE_POLICY_TEMPLE"])
        self.choose(1, "JAC_AGE_POLICY_PIRATE", reign=47)  # the dark-age policy choice still applies
        self.assertEqual((self.house(1).policy, self.house(1).chosen), ("JAC_AGE_POLICY_PIRATE", 47))
        self.assertIsNone(l.nextDue)
        self.turns(3000)  # no more judgements: 遅代 lasts until the game ends
        self.assertEqual((self.g.LEDGER().reign, self.house(1).age), (47, "DARK"))
        self.assertIsNone(self.g.LEDGER().history["r48"])

    def plot_properties(self) -> dict:
        names = ["JAC_AGE_GOLDEN"] + [f"JAC_AGE_{kind}_{p}" for kind in ("POLICY", "ANCHOR")
                                      for p in ("VALOR", "ELITE", "PIRATE", "TEMPLE")]
        return {i: tuple(p.GetProperty(None, n) for n in names) for i, p in self.g.PLOTS.items()}

    def test_property_writer_is_idempotent_with_one_anchor_per_house(self):
        self.turns(1, 66)  # nobody scored: every house is dark with its default policy
        self.house(1).age, self.house(1).policy = "GOLDEN", None  # make Takeda golden
        self.turns(67)
        self.assertEqual(self.plot(12, 12).GetProperty(None, "JAC_AGE_ANCHOR_TEMPLE"), 1)
        self.g.FOUND(3, 11, 12, False)  # CityBuilt rewrites; the smaller plot index becomes Imagawa's anchor
        first = self.plot_properties()
        self.turns(68)
        self.g.START_TURN(68)  # the same writes again change nothing
        self.assertEqual(self.plot_properties(), first)
        for x in (11, 12, 13, 14):
            plot = self.plot(x, 12)
            self.assertEqual(plot.GetProperty(None, "JAC_AGE_POLICY_TEMPLE"), 1)
            self.assertEqual(plot.GetProperty(None, "JAC_AGE_ANCHOR_TEMPLE"), 1 if x == 11 else 0)
            self.assertEqual(plot.GetProperty(None, "JAC_AGE_GOLDEN"), 0)
        for x in (5, 6):
            self.assertEqual(self.plot(x, 5).GetProperty(None, "JAC_AGE_GOLDEN"), 1)
            self.assertEqual(self.plot(x, 5).GetProperty(None, "JAC_AGE_ANCHOR_ELITE"), 0)
        anchors = [(i, n) for i, props in self.plot_properties().items() for n, v in enumerate(props[5:]) if v == 1]
        self.assertEqual(len(anchors), 3)  # Oda, Hojo, Imagawa are dark; one anchor each

    def test_normal_house_has_no_policy_and_dark_house_flags_every_city(self):
        # Live ages-impl-02: a NORMAL house logged policy= but its ledger policy was not "" on the next turn, so
        # "no policy" is nil, not a "" sentinel. §7.1: the policy flag on every city centre, the anchor on one.
        self.turns(1, 30)
        for i in range(1, 6):
            self.g.Players[0].research.techs[i] = True  # Oda: 5 points, 2 cities -> dark 4, golden 7: normal
        self.turns(31, 67)
        r1 = self.g.LEDGER().history["r1"]
        self.assertEqual((r1.p0.age, r1.p3.age), ("NORMAL", "DARK"))
        self.assertIsNone(self.house(0).policy)
        self.assertEqual(self.house(3).policy, "JAC_AGE_POLICY_TEMPLE")
        for x in (1, 2):
            self.assertEqual(self.plot_properties()[100 + x], (0,) * 9)
        for x in (12, 13, 14):
            plot = self.plot(x, 12)
            self.assertEqual(plot.GetProperty(None, "JAC_AGE_POLICY_TEMPLE"), 1)
            self.assertEqual(plot.GetProperty(None, "JAC_AGE_ANCHOR_TEMPLE"), 1 if x == 12 else 0)

    # --- dark-age policy choice (§6.2, §7.2) ---

    def policy_flags(self, x: int, y: int) -> dict:
        plot = self.plot(x, y)
        return {n: (plot.GetProperty(None, f"JAC_AGE_POLICY_{n}"), plot.GetProperty(None, f"JAC_AGE_ANCHOR_{n}"))
                for n in ("VALOR", "ELITE", "PIRATE", "TEMPLE")}

    def choose(self, pid: int, policy: str | None, reign=1) -> None:
        self.g.CHOOSE(pid, self.lua.table_from({"PolicyType": policy, "Reign": reign}))

    def test_dark_houses_take_their_own_default_and_unlisted_civs_the_first_priority(self):
        self.g.PlayerConfigurations[2].GetCivilizationTypeName = lambda _self: "CIVILIZATION_OTHER_MAP"
        self.turns(1, 66)
        self.assertEqual([self.house(p).policy for p in range(4)],
                         ["JAC_AGE_POLICY_ELITE", "JAC_AGE_POLICY_ELITE", "JAC_AGE_POLICY_VALOR",
                          "JAC_AGE_POLICY_TEMPLE"])
        self.assertIsNone(self.house(0).chosen)

    def test_dark_house_choice_applies_its_flags_once_per_reign(self):
        self.turns(1, 67)  # every house dark; Takeda's default is Elite Troops
        self.assertEqual(self.policy_flags(5, 5)["ELITE"], (1, 1))
        self.choose(1, "JAC_AGE_POLICY_PIRATE")
        self.assertEqual((self.house(1).policy, self.house(1).chosen), ("JAC_AGE_POLICY_PIRATE", 1))
        self.assertEqual(self.policy_flags(5, 5),
                         {"VALOR": (0, 0), "ELITE": (0, 0), "PIRATE": (1, 1), "TEMPLE": (0, 0)})
        self.assertEqual(self.policy_flags(6, 5),
                         {"VALOR": (0, 0), "ELITE": (0, 0), "PIRATE": (1, 0), "TEMPLE": (0, 0)})
        self.choose(1, "JAC_AGE_POLICY_TEMPLE")  # a second choice in the same period is dropped
        self.turns(68, 72)  # turn starts keep the choice
        self.assertEqual(self.house(1).policy, "JAC_AGE_POLICY_PIRATE")
        self.assertEqual(self.policy_flags(5, 5)["PIRATE"], (1, 1))
        self.assertTrue(any("already chosen" in line for line in self.g.LOG.values()))
        self.turns(73)  # 永禄 (L = 7): normal, the policy ends
        self.assertEqual((self.house(1).age, self.house(1).policy), ("NORMAL", None))
        self.turns(74, 110)  # 元亀 (L = 37): dark again -> the default again, and a new choice is open
        self.assertEqual((self.house(1).age, self.house(1).policy), ("DARK", "JAC_AGE_POLICY_ELITE"))
        self.choose(1, "JAC_AGE_POLICY_TEMPLE", reign=3)
        self.assertEqual((self.house(1).policy, self.house(1).chosen), ("JAC_AGE_POLICY_TEMPLE", 3))

    def test_invalid_choices_are_dropped(self):
        self.turns(1, 30)
        for i in range(1, 6):
            self.g.Players[0].research.techs[i] = True  # Oda normal at 弘治 (as the normal-house test)
        self.turns(31, 66)
        self.assertEqual(self.house(0).age, "NORMAL")
        before = self.snapshot()
        self.choose(0, "JAC_AGE_POLICY_PIRATE")  # not dark
        self.choose(1, "JAC_AGE_POLICY_UNKNOWN")  # not a policy of the table
        self.choose(1, None)
        self.choose(1, "JAC_AGE_POLICY_PIRATE", reign=0)  # the screen of an earlier period
        self.choose(self.g.FREE_CITIES, "JAC_AGE_POLICY_PIRATE")  # no house
        self.g.CHOOSE(1, None)  # no parameters
        self.assertEqual(self.snapshot(), before)
        reasons = [line.split(": ")[-1] for line in self.g.LOG.values() if "choice dropped" in line]
        self.assertEqual(reasons, ["not dark", "unknown policy", "unknown policy", "stale period 0", "no house",
                                   "stale period nil"])

    def test_choice_before_the_first_judgement_is_dropped(self):
        self.turns(1, 10)
        self.choose(1, "JAC_AGE_POLICY_PIRATE", reign=0)
        self.assertIsNone(self.house(1).policy)

    def test_choice_survives_load(self):
        self.turns(1, 66)
        self.choose(3, "JAC_AGE_POLICY_VALOR")
        self.g.RESET_HANDLERS()
        self.load_script()  # load: only the Game property and the plot properties survive
        self.turns(67)
        self.assertEqual((self.house(3).policy, self.house(3).chosen), ("JAC_AGE_POLICY_VALOR", 1))
        self.assertEqual(self.policy_flags(12, 12)["VALOR"], (1, 1))
        self.assertEqual(self.policy_flags(12, 12)["TEMPLE"], (0, 0))

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


UI_STUBS = r"""
-- UI side of the choice screen: controls record their state; UI.RequestPlayerOperation hands the operation to the
-- gameplay handler as the engine does for EXECUTE_SCRIPT (requesting player, parameter table).
LOCAL_PLAYER, TURN_ACTIVE, REQUESTS = 1, true, {}
local function control()
  -- Unlisted control methods (SetColor, SetIcon, ...) are no-ops.
  local c = setmetatable({ hidden = false, disabled = false }, { __index = function() return function() end end })
  function c:SetText(t) self.text = t end
  function c:GetText() return self.text end
  function c:SetHide(h) self.hidden = h end
  function c:IsHidden() return self.hidden end
  function c:SetDisabled(d) self.disabled = d end
  function c:RegisterCallback(_, f) self.click = f end
  function c:SetTexture(t) self.texture = t end
  function c:SetToolTipString(t) self.tooltip = t end
  return c
end
CONTROL = control
-- LuaEvents: every name is an event; calling it runs its handlers (they are shared by all contexts, as live).
LuaEvents = setmetatable({}, { __index = function(t, name)
  local handlers = {}
  local event = setmetatable({ Add = function(f) table.insert(handlers, f) end },
    { __call = function(_, ...) for _, f in ipairs(handlers) do f(...) end end })
  rawset(t, name, event)
  return event
end })
Controls = setmetatable({}, { __index = function(t, k) local c = control(); rawset(t, k, c); return c end })
ContextPtr = control()
ROWS = {}
function include(name) assert(name == "InstanceManager") end
InstanceManager = { new = function() return {
  ResetInstances = function() ROWS = {} end,
  GetInstance = function()
    local row = { Name = control(), Description = control(), Choose = control() }
    table.insert(ROWS, row)
    return row
  end,
} end }
Locale = { Lookup = function(key, ...) local a = { ... }; return key .. (a[1] and ("(" .. a[1] .. ")") or "") end }
Mouse = { eLClick = 1 }
PlayerOperations = { EXECUTE_SCRIPT = "EXECUTE_SCRIPT" }
Game.GetLocalPlayer = function() return LOCAL_PLAYER end
for _, p in pairs(Players) do p.IsTurnActive = function() return TURN_ACTIVE end end
local uiEvents = { turn = {}, turnEnd = {}, load = {} }
Events = {
  LocalPlayerTurnBegin = { Add = function(f) table.insert(uiEvents.turn, f) end },
  LocalPlayerTurnEnd = { Add = function(f) table.insert(uiEvents.turnEnd, f) end },
  LoadScreenClose = { Add = function(f) table.insert(uiEvents.load, f) end },
}
function LOCAL_TURN_BEGIN() for _, f in ipairs(uiEvents.turn) do f() end end
function LOCAL_TURN_END() for _, f in ipairs(uiEvents.turnEnd) do f() end end
UI = { RequestPlayerOperation = function(id, op, params)
  table.insert(REQUESTS, { id = id, op = op, OnStart = params.OnStart })
  if op == "EXECUTE_SCRIPT" and params.OnStart == "JAC_SengokuDarkPolicyChoose" then CHOOSE(id, params) end
end }
"""


@unittest.skipIf(lupa is None, "lupa is not installed")
class DarkPolicyScreenTests(unittest.TestCase):
    """UI/JAC_SengokuDarkPolicy.lua on top of the ledger; Takeda (player 1) is the local human."""

    load_script, turns, house, plot, policy_flags = (LedgerTests.load_script, LedgerTests.turns, LedgerTests.house,
                                                     LedgerTests.plot, LedgerTests.policy_flags)

    def setUp(self):
        LedgerTests.setUp(self)
        self.lua.execute(UI_STUBS)
        self.lua.execute((MOD / "UI/JAC_SengokuDarkPolicy.lua").read_text(encoding="utf-8"))

    def shown(self) -> bool:
        return not self.g.ContextPtr.hidden

    def test_screen_lists_the_policies_and_a_click_applies_the_choice(self):
        self.turns(1, 65)
        self.g.LOCAL_TURN_BEGIN()
        self.assertFalse(self.shown())  # no judgement yet
        self.turns(66)
        self.g.LOCAL_TURN_BEGIN()
        self.assertTrue(self.shown())
        rows = list(self.g.ROWS.values())
        self.assertEqual([r.Name.text for r in rows], ["LOC_JAC_AGE_POLICY_VALOR_NAME", "LOC_JAC_AGE_POLICY_ELITE_NAME",
                                                        "LOC_JAC_AGE_POLICY_PIRATE_NAME", "LOC_JAC_AGE_POLICY_TEMPLE_NAME"])
        self.assertEqual(rows[2].Description.text, "LOC_JAC_AGE_POLICY_PIRATE_DESCRIPTION")
        self.assertEqual(self.g.Controls.Body.text, "LOC_JAC_AGE_DARK_CHOICE_BODY(LOC_JAC_AGE_POLICY_ELITE_NAME)")
        rows[2].Choose.click()
        self.assertFalse(self.shown())
        self.assertEqual((self.house(1).policy, self.house(1).chosen), ("JAC_AGE_POLICY_PIRATE", 1))
        self.assertEqual(self.policy_flags(5, 5)["PIRATE"], (1, 1))
        self.turns(67)
        self.g.LOCAL_TURN_BEGIN()
        self.assertFalse(self.shown())  # chosen: not shown again in this period

    def test_later_closes_until_the_next_turn_and_nothing_is_sent_before_the_turn_is_active(self):
        self.turns(1, 66)
        self.g.TURN_ACTIVE = False
        self.g.LOCAL_TURN_BEGIN()
        self.assertTrue(self.shown())
        list(self.g.ROWS.values())[2].Choose.click()  # before the turn is active: not sent, the screen stays
        self.assertEqual(len(self.g.REQUESTS), 0)
        self.assertTrue(self.shown())
        self.g.Controls.LaterButton.click()
        self.assertFalse(self.shown())
        self.g.TURN_ACTIVE = True
        self.turns(67)
        self.g.LOCAL_TURN_BEGIN()
        self.assertTrue(self.shown())  # still unchosen: asked again, the default stays meanwhile
        self.assertEqual((self.house(1).policy, self.house(1).chosen), ("JAC_AGE_POLICY_ELITE", None))

    def test_turn_end_closes_and_the_next_turn_asks_again(self):
        self.turns(1, 66)
        self.g.LOCAL_TURN_BEGIN()
        self.assertTrue(self.shown())
        self.g.LOCAL_TURN_END()  # ended without choosing: nothing can be sent until the next turn
        self.assertFalse(self.shown())
        self.assertEqual(len(self.g.REQUESTS), 0)
        self.turns(67)
        self.g.LOCAL_TURN_BEGIN()
        self.assertTrue(self.shown())
        list(self.g.ROWS.values())[3].Choose.click()
        self.assertEqual((self.house(1).policy, self.house(1).chosen), ("JAC_AGE_POLICY_TEMPLE", 1))

    def test_not_shown_for_a_house_that_is_not_dark(self):
        self.turns(1, 30)
        for i in range(1, 6):
            self.g.Players[1].research.techs[i] = True  # Takeda normal at 弘治
        self.turns(31, 66)
        self.g.LOCAL_TURN_BEGIN()
        self.assertEqual(self.house(1).age, "NORMAL")
        self.assertFalse(self.shown())


# Live 2026-09-29 (ages-impl-01, 15 km Japan map, 22 AI houses, standard speed): at the 弘治 judgement (turn 66,
# L = 65) per house (score, cities). Scores are research 3-9 plus event 0-3 points; no castle or alliance points.
# The old thresholds (official 28/14 per 50 turns) made every house dark.
KOJI_OBSERVED = [(4, 4), (4, 3), (6, 8), (4, 5), (11, 7), (12, 8), (6, 3), (7, 6), (3, 4), (6, 2), (9, 14),
                 (7, 6), (6, 3), (3, 1), (5, 6), (9, 4), (4, 2), (4, 4), (3, 4), (6, 5), (5, 2), (5, 4)]


@unittest.skipIf(lupa is None, "lupa is not installed")
class KojiDistributionTests(unittest.TestCase):
    def test_observed_koji_scores_land_mostly_normal(self):
        lua = lupa.LuaRuntime(unpack_returned_tuples=True)
        civs = [f"CIVILIZATION_JAC_H{i}" for i in range(len(KOJI_OBSERVED))]
        lua.execute(STUBS, lua.table_from(ages_tables(), recursive=True), lua.table_from(civs), START_TURN)
        g = lua.globals()
        for pid, (_, cities) in enumerate(KOJI_OBSERVED):
            for n in range(cities):
                g.FOUND(pid, n, pid, n == 0, True)
        lua.execute((MOD / "Scripts/JAC_Sengoku_Ages.lua").read_text(encoding="utf-8"))
        for turn in range(1, 67):
            if turn == 30:  # every point as research (the ledger adds research and events alike)
                for pid, (score, _) in enumerate(KOJI_OBSERVED):
                    research = g.Players[pid].research
                    for i in range(score):
                        (research.techs if i < 10 else research.civics)[i % 10] = True
            g.START_TURN(turn)
        r1 = g.LEDGER().history["r1"]
        self.assertEqual([r1[f"p{pid}"].score for pid in range(len(KOJI_OBSERVED))], [s for s, _ in KOJI_OBSERVED])
        ages = [r1[f"p{pid}"].age for pid in range(len(KOJI_OBSERVED))]
        counts = {age: ages.count(age) for age in ("GOLDEN", "NORMAL", "DARK")}
        self.assertEqual(counts, {"GOLDEN": 3, "NORMAL": 19, "DARK": 0})


# Live 2026-09-30 (tools/civ6-autotest/artifacts/health-200-01/Lua.log, 200 turns, 22 AI houses, player 13 fell
# before 元亀): per 改元 (judgement turn, [(score, cities) per player, None when fallen]). The scores are logged; the
# city counts are not, so they are the middle of what the logged thresholds allow under the old rule
# (a short period allows a wide range; its threshold barely depends on it). With them the old rule
# (dark >= max(1, ceil(...))) gives back the logged ages of all 128 houses, whose counts
# golden/heroic/normal/dark were 弘治 L=65 4/0/13/5, 永禄 L=7 0/0/8/14, 元亀 L=37 0/8/6/7, 天正 L=9 1/1/9/10,
# 文禄 L=59 5/5/7/4, 慶長 L=11 4/0/6/11: the short periods scored 0-1 points and a 0 was dark.
HEALTH_200 = [
    (66, [(4, 2), (6, 6), (5, 6), (5, 6), (10, 6), (4, 2), (3, 2), (5, 6), (3, 2), (9, 2), (15, 11), (8, 6), (5, 2),
          (3, 2), (5, 6), (6, 2), (4, 2), (3, 2), (5, 2), (5, 6), (2, 2), (4, 2)]),
    (73, [(1, 2), (0, 6), (0, 6), (0, 6), (1, 6), (1, 2), (0, 2), (1, 6), (0, 2), (0, 2), (0, 11), (0, 6), (1, 2),
          (0, 2), (1, 6), (0, 2), (1, 2), (1, 2), (0, 2), (0, 6), (0, 2), (0, 2)]),
    (110, [(1, 3), (1, 12), (6, 8), (4, 4), (3, 10), (2, 3), (3, 5), (3, 7), (3, 5), (4, 3), (18, 15), (3, 7), (2, 3),
           None, (4, 7), (0, 4), (2, 7), (2, 4), (2, 4), (2, 4), (4, 2), (5, 4)]),
    (119, [(1, 3), (1, 12), (1, 8), (5, 4), (0, 10), (0, 3), (0, 5), (1, 7), (0, 5), (0, 3), (1, 15), (1, 7), (0, 3),
           None, (1, 7), (2, 4), (0, 7), (1, 4), (1, 4), (0, 4), (0, 2), (1, 4)]),
    (178, [(6, 4), (10, 12), (5, 9), (5, 7), (4, 12), (2, 2), (5, 7), (9, 9), (8, 4), (6, 4), (11, 19), (4, 9), (9, 4),
           None, (6, 9), (8, 7), (5, 7), (4, 7), (7, 9), (4, 9), (3, 2), (4, 9)]),
    (189, [(1, 4), (3, 12), (0, 9), (0, 7), (1, 12), (0, 2), (0, 7), (6, 9), (0, 4), (1, 4), (0, 19), (0, 9), (1, 4),
           None, (0, 9), (1, 7), (2, 7), (0, 7), (0, 9), (4, 9), (0, 2), (1, 9)]),
]
HEALTH_200_OLD = [(4, 0, 13, 5), (0, 0, 8, 14), (0, 8, 6, 7), (1, 1, 9, 10), (5, 5, 7, 4), (4, 0, 6, 11)]


@unittest.skipIf(lupa is None, "lupa is not installed")
class ShortReignDistributionTests(unittest.TestCase):
    def ages(self, old_rule: bool = False) -> list[tuple[int, int, int, int]]:
        script = (MOD / "Scripts/JAC_Sengoku_Ages.lua").read_text(encoding="utf-8")
        if old_rule:  # dark >= max(1, ceil(...)), golden >= max(dark + 1, ceil(...))
            for new, old in (("math.max(0, math.floor(", "math.max(1, math.ceil("),
                             ("math.max(MIN_GOLDEN, ", "math.max(dark + 1, ")):
                self.assertEqual(script.count(new), 1)
                script = script.replace(new, old)
        lua = lupa.LuaRuntime(unpack_returned_tuples=True)
        lua.execute(STUBS, lua.table_from(ages_tables(), recursive=True),
                    lua.table_from([f"CIVILIZATION_JAC_H{i}" for i in range(22)]), START_TURN)
        g = lua.globals()
        g.START_TURN(1)  # before the script: no ledger yet; the cities come in reign by reign
        lua.execute(script)
        g.START_TURN(2)
        counts = []
        for index, (due, houses) in enumerate(HEALTH_200, 1):
            g.CITIES = lua.table_from([{"owner": pid, "x": n, "y": pid, "capital": n == 0}
                                       for pid, house in enumerate(houses) if house for n in range(house[1])])
            for pid, house in enumerate(houses):
                g.ALIVE[pid] = house is not None
                if house:
                    g.LEDGER().houses[f"p{pid}"].score = house[0]
            g.START_TURN(due)
            reign = g.LEDGER().history[f"r{index}"]
            ages = [reign[f"p{pid}"].age for pid, house in enumerate(houses) if house]
            counts.append(tuple(ages.count(age) for age in ("GOLDEN", "HEROIC", "NORMAL", "DARK")))
        return counts

    def test_the_old_rule_gives_back_the_logged_ages(self):
        self.assertEqual(self.ages(old_rule=True), HEALTH_200_OLD)

    def test_short_periods_leave_most_houses_normal(self):
        counts = self.ages()
        for golden, heroic, normal, dark in counts:
            total = golden + heroic + normal + dark
            self.assertLessEqual(dark * 3, total)
            self.assertGreater(normal, max(golden + heroic, dark))
        self.assertTrue(any(golden + heroic for golden, heroic, _, _ in counts))
        self.assertEqual(counts, [(4, 0, 17, 1), (0, 0, 22, 0), (5, 0, 13, 3), (1, 1, 19, 0), (6, 0, 10, 5),
                                  (4, 0, 15, 2)])


if __name__ == "__main__":
    unittest.main()
