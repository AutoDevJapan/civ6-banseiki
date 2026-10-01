"""Execute the shipped religion ledger in fresh Lua VMs, including save/load.

lupa is mandatory. The JSON roundtrip is a serialization stub, not evidence
of Civ6 save support; the live scenario uses external pre-save captures.
"""
from __future__ import annotations

import json
from pathlib import Path
import sqlite3
import unittest
import xml.etree.ElementTree as ET
from lupa import LuaError, LuaRuntime

ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_PoC_ReligionPersistence"
SCRIPT = (MOD / "Scripts/JAC_ReligionPersistence.lua").read_text(encoding="utf-8")
UI_SCRIPT = (MOD / "UI/JAC_ReligionPersistence_HeroUI.lua").read_text(encoding="utf-8")
KEY = "JAC_RP_STATE_V1"
R1, R2 = "JAC_RP_PLACEHOLDER_01", "JAC_RP_PLACEHOLDER_02"
TABLES = ("Religions", "AssetKinds", "Jobs", "Sites", "Outcomes", "Rules")

STUBS = r"""
local tables, initial, engine, modifiers = ...
local function clone(v)
  if type(v) ~= 'table' then return v end
  local out = {}
  for k, x in pairs(v) do out[k] = clone(x) end
  return out
end
GameInfo = {}
for name, rows in pairs(tables) do
  GameInfo['JAC_RP_' .. name] = setmetatable({}, { __call = function()
    local n = 0
    return function() n = n + 1; return rows[n] end
  end })
end
PROPERTY_STORE = { JAC_RP_STATE_V1 = initial }
WRITES, TURN, LOG = 0, 17, {}
FAIL_WRITE = false
FAIL_WRITE_AT = nil
Game = {
  GetProperty = function(_, k) return PROPERTY_STORE[k] end,
  SetProperty = function(_, k, v)
    if FAIL_WRITE or FAIL_WRITE_AT == WRITES + 1 then error('engine refused write') end
    if v == '' then v = nil end
    PROPERTY_STORE[k] = clone(v)
    WRITES = WRITES + 1
  end,
  GetCurrentGameTurn = function() return TURN end,
}
ExposedMembers = {}
ENGINE = engine or {}
FAIL_EFFECT, FAIL_SPAWN, BAD_CONVERSION = false, false, false
GameEvents = { OnGameTurnStarted = { Add = function(f) TURN_HOOK = f end } }
GameConfiguration = { GetValue = function() return HEROES_MODE end }
HEROES_MODE = true
local function info(rows, key)
  local t = setmetatable({}, { __call = function()
    local n = 0; return function() n = n + 1; return rows[n] end
  end })
  for _, row in ipairs(rows) do t[row.Index] = row; t[row[key]] = row end
  return t
end
GameInfo.Technologies = info({
  { Index = 1, TechnologyType = 'TECH_Z', Cost = 200 },
  { Index = 2, TechnologyType = 'TECH_A', Cost = 200 },
  { Index = 3, TechnologyType = 'TECH_M', Cost = 200 },
  { Index = 4, TechnologyType = 'TECH_B', Cost = 200 },
  { Index = 5, TechnologyType = 'TECH_FUTURE', Cost = 200 },
}, 'TechnologyType')
GameInfo.Boosts = info({ { Index = 1, TechnologyType = 'TECH_A' },
  { Index = 2, TechnologyType = 'TECH_B' }, { Index = 3, TechnologyType = 'TECH_M' },
  { Index = 4, TechnologyType = 'TECH_Z' } }, 'TechnologyType')
GameInfo.HeroClasses = info({
  { Index = 10, HeroClassType = 'HEROCLASS_ANANSI', UnitType = 'UNIT_HERO_ANANSI', CreationProjectType = 'PROJECT_CREATE_HERO_ANANSI' },
  { Index = 11, HeroClassType = 'HEROCLASS_ARTHUR', UnitType = 'UNIT_HERO_ARTHUR', CreationProjectType = 'PROJECT_CREATE_HERO_ARTHUR' },
}, 'HeroClassType')
GameInfo.ProjectCompletionModifiers = info({
  { Index = 1, ProjectType = 'PROJECT_CREATE_HERO_ANANSI', ModifierId = 'MODIFIER_CREATE_HERO_ANANSI' },
  { Index = 2, ProjectType = 'PROJECT_CREATE_HERO_ARTHUR', ModifierId = 'MODIFIER_CREATE_HERO_ARTHUR' },
}, 'ProjectType')
GameInfo.Modifiers = modifiers
for _, row in ipairs({ 'ANANSI', 'ARTHUR' }) do
  GameInfo.Modifiers['MODIFIER_CREATE_HERO_' .. row] = { ModifierType = 'MODIFIER_CITY_GRANT_HERO' }
end
GameInfo.Units = { UNIT_HERO_ANANSI = { Index = 100 }, UNIT_HERO_ARTHUR = { Index = 101 }, UNIT_SCOUT = { Index = 5 } }
GameInfo.Buildings = { BUILDING_MONUMENT = { Index = 1 } }
local function nativeRows(c)
  local rows, total, used = {}, 0, 0
  for _, pressure in pairs(c.pressure) do total = total + pressure end
  if total == 0 then c.pressure['-1'] = 100; total = 100 end
  local ordered = {}
  for religion, pressure in pairs(c.pressure) do
    if pressure > 0 then
      local raw = c.population * pressure / total
      local n = math.floor(raw)
      rows[#rows + 1] = { Religion = tonumber(religion), Followers = n, Pressure = pressure }
      ordered[#ordered + 1] = { row = rows[#rows], remainder = raw - n }
      used = used + n
    end
  end
  table.sort(ordered, function(a, b)
    return a.remainder > b.remainder or a.remainder == b.remainder and a.row.Religion < b.row.Religion
  end)
  for i = 1, c.population - used do ordered[i].row.Followers = ordered[i].row.Followers + 1 end
  if OMIT_UNAFFILIATED_ROWS then
    local filtered = {}
    for _, row in ipairs(rows) do if row.Religion > 0 then filtered[#filtered + 1] = row end end
    return filtered
  end
  return rows
end
NativeRows = nativeRows
HERO_UI_HIDDEN = true
Events = { LoadScreenClose = { Add = function(fn) HERO_UI_LOAD_CLOSE = fn end } }
ContextPtr = {
  SetHide = function(_, hidden) HERO_UI_HIDDEN = hidden end,
  SetUpdate = function(_, fn)
    HERO_UI_TICK = fn
    HERO_UI_FRAME = function(dt) if not HERO_UI_HIDDEN then fn(dt) end end
  end,
}
-- Native founding is a scenario fixture only. These stubs check its call path;
-- whether the actual engine permits it remains a live-scenario assertion.
UnitPropertyKeys = { LifespanRemaining = "LifespanRemaining" }
NATIVE_RELIGION = 42
local holy = {}
Players = {}
for id = 0, 1 do
  local pid = id
  local e = ENGINE[tostring(pid)] or { faith = 500, faithYield = 3.25, gold = 100, science = 0, culture = 0,
    favor = 10, units = {}, nextUnit = 1, alive = true, major = true, met = true, war = false,
    allied = false, city = true, nativeReligion = 42 + pid, researched = {}, boosted = {},
    heroClaims = {}, cities = { ['0'] = { population = pid == 0 and 7 or 4,
      pressure = { [tostring(42 + pid)] = (pid == 0 and 7 or 4) * 1024 }, monument = true } } }
  ENGINE[tostring(pid)] = e
  local cityObjects = {}
  for cid, c in pairs(e.cities) do
    local city = { GetID = function() return tonumber(cid) end, GetOwner = function() return pid end,
      GetX = function() return 7 end, GetY = function() return 8 end,
      GetPopulation = function() return c.population end,
      ChangePopulation = function(_, n) c.population = c.population + n end,
      GetBuildings = function() return { HasBuilding = function(_, index) return index == 1 and c.monument end } end,
      GetBuildQueue = function() return { CreateIncompleteBuilding = function(_, index, percentage)
        assert(index == 1 and percentage == 100); c.monument = true end } end,
      GetReligion = function() return {
        GetNumFollowers = function(_, r)
          for _, row in ipairs(nativeRows(c)) do if row.Religion == r then return row.Followers end end
          return 0
        end,
        RemovePressureOneReligion = function(_, r, amount, responsible)
          assert(r > 0)
          assert(amount >= 0 and amount <= 8388607 and responsible == -1)
          if FAIL_EFFECT then error('conversion refused') end
          if not BAD_CONVERSION then c.pressure[tostring(r)] = math.max(0, (c.pressure[tostring(r)] or 0) - amount) end
        end,
        AddReligiousPressure = function(_, source, r, delta, unit)
          assert(source == 0 and unit == -1)
          assert(delta >= 0 and delta <= 8388607, 'native fixed point pressure must not overflow')
          assert(r > 0, 'unaffiliated pressure must remain engine-owned')
          if FAIL_EFFECT then error('conversion refused') end
          if BAD_CONVERSION then return end
          local key = tostring(r); c.pressure[key] = math.max(0, (c.pressure[key] or 0) + delta)
        end,
        SetAllCityToReligion = function(_, r) c.pressure = { [tostring(r)] = c.population * 1024 } end,
      } end,
      AttachModifierByID = function(_, modifier)
        assert(modifier == 'MODIFIER_CREATE_HERO_ANANSI' or modifier == 'MODIFIER_CREATE_HERO_ARTHUR')
        if FAIL_EFFECT then error('hero refused') end
        if FAIL_SPAWN then return end
        local class = modifier == 'MODIFIER_CREATE_HERO_ANANSI' and 10 or 11
        assert((ENGINE['0'].heroClaims[tostring(class)] or -1) == -1)
        ENGINE['0'].heroClaims[tostring(class)] = pid
        e.units[tostring(e.nextUnit)] = { id = e.nextUnit, kind = class + 90, class = class, lifespan = 30 }
        e.nextUnit = e.nextUnit + 1
      end,
    }
    cityObjects[cid] = city
  end
  local capital = cityObjects['0']
  local religion = { SetHolyCity = function(_, city) holy[tostring(pid)] = city end,
    GetReligionTypeCreated = function() return e.nativeReligion end,
    GetFaithYield = function() return e.faithYield end,
    GetFaithBalance = function() return e.faith end,
    ChangeFaithBalance = function(_, amount) if FAIL_EFFECT then error('effect refused') end; e.faith = e.faith + amount end }
  Players[pid] = {
    GetID = function() return pid end, IsAlive = function() return e.alive end,
    IsTurnActive = function() return e.active ~= false end,
    IsMajor = function() return e.major end, IsHuman = function() return pid == 0 end,
    GetCities = function() return { GetCapitalCity = function() return e.city and capital or nil end,
      FindID = function(_, city) return e.city and cityObjects[tostring(city)] or nil end,
      Members = function() local key; return function()
        if not e.city then return end; key = next(cityObjects, key); if key then return tonumber(key), cityObjects[key] end
      end end } end,
    GetReligion = function() return religion end,
    GetTechs = function() return {
      SetTech = function(_, index, acquired) e.researched[tostring(index)] = acquired end,
      HasTech = function(_, index) return e.researched[tostring(index)] == true end,
      HasBoostBeenTriggered = function(_, index) return e.boosted[tostring(index)] == true end,
      TriggerBoost = function(_, index, source)
        assert(source == 1 and not e.researched[tostring(index)] and not e.boosted[tostring(index)])
        if FAIL_EFFECT then error('boost refused') end; e.boosted[tostring(index)] = true
      end,
    } end,
    GetCulture = function() return { SetCivic = function() end } end,
    GetTreasury = function() return {
      GetGoldBalance = function() return e.gold end,
      ChangeGoldBalance = function(_, amount) if FAIL_EFFECT then error('effect refused') end; e.gold = e.gold + amount end } end,
    GetDiplomacy = function() return { HasMet = function() return e.met end,
      SetHasMet = function() e.met = true end, MakePeaceWith = function() e.war = false end,
      IsAtWarWith = function() return e.war end, GetFavor = function() return e.favor end,
      HasAllied = function() return e.allied and (e.allianceTurn == nil or TURN - e.allianceTurn < 30) end,
      SetHasAllied = function(_, other, enabled)
        assert(enabled == true); e.allied = true; ENGINE[tostring(other)].allied = true
        e.allianceTurn = TURN; ENGINE[tostring(other)].allianceTurn = TURN
      end,
      ChangeFavor = function(_, amount) if FAIL_EFFECT then error('effect refused') end; e.favor = e.favor + amount end } end,
    GetUnits = function() return {
      FindID = function(_, unit) local u = e.units[tostring(unit)]; return u and {
        GetID = function() return u.id end, GetType = function() return u.kind end,
        GetHeroClassType = function() return u.class or -1 end,
        GetProperty = function(_, key) assert(key == UnitPropertyKeys.LifespanRemaining); return u.lifespan end } end,
      Members = function() local key; return function()
        key = next(e.units, key); if key == nil then return end
        local u = e.units[key]; return u.id, {
          GetID = function() return u.id end, GetType = function() return u.kind end,
          GetHeroClassType = function() return u.class or -1 end,
          GetProperty = function(_, key) assert(key == UnitPropertyKeys.LifespanRemaining); return u.lifespan end }
      end end } end,
    AttachModifierByID = function(_, modifier)
      if FAIL_EFFECT then error('effect refused') end
      local args = assert(modifiers[modifier])
      local yield = string.lower(args.YieldType:sub(7))
      e[yield] = e[yield] + tonumber(args.Amount)
    end,
  }
end
GameInfo.Civics = function() return function() return nil end end
Game.GetPlayers = function()
  local all = {}; for id = 0, 1 do if ENGINE[tostring(id)].alive then all[#all + 1] = Players[id] end end; return all
end
PlayerManager = { GetAlive = Game.GetPlayers }
Game.GetHeroesManager = function() return {
  GetPlayerHeroOriginBuildingType = function() return 1 end,
  GetHeroClaimPlayer = function(_, index) return ENGINE['0'].heroClaims[tostring(index)] or -1 end,
} end
GameInfo.Religions = info({ { Index = 42, ReligionType = 'RELIGION_CUSTOM_1' },
  { Index = 43, ReligionType = 'RELIGION_CUSTOM_2' }, { Index = 44, ReligionType = 'RELIGION_CUSTOM_3' },
  { Index = 2, ReligionType = 'RELIGION_ORDER_2' }, { Index = 10, ReligionType = 'RELIGION_ORDER_10' } }, 'ReligionType')
GameInfo.Beliefs = { BELIEF_CATHEDRAL = { Index = 9 } }
local native = {
  FoundReligion = function(_, player, religion)
    assert(holy[tostring(player)] == Players[player]:GetCities():GetCapitalCity())
    ENGINE[tostring(player)].nativeReligion = religion
    if player == 0 then NATIVE_RELIGION = religion end
  end,
  AddBelief = function(_, player, belief) assert(player >= 0 and player <= 1 and belief == 9) end,
  GetReligions = function() return { { Religion = 42 }, { Religion = 43 }, { Religion = 44 } } end,
}
Game.GetReligion = function() return native end
function print(line) LOG[#LOG + 1] = line end
"""

def catalog():
    with sqlite3.connect(":memory:") as db:
        db.executescript("CREATE TABLE Modifiers (ModifierId TEXT PRIMARY KEY, ModifierType TEXT, RunOnce INT, Permanent INT);"
                         "CREATE TABLE ModifierArguments (ModifierId TEXT, Name TEXT, Value TEXT, PRIMARY KEY(ModifierId, Name));")
        db.executescript((MOD / "Data/JAC_ReligionPersistence.sql").read_text(encoding="utf-8"))
        db.row_factory = sqlite3.Row
        result = {name: [dict(r) for r in db.execute(f"SELECT * FROM JAC_RP_{name}")] for name in TABLES}
        result["modifiers"] = {r["ModifierId"]: {a["Name"]: a["Value"] for a in db.execute(
            "SELECT Name,Value FROM ModifierArguments WHERE ModifierId=?", (r["ModifierId"],))}
            for r in db.execute("SELECT * FROM Modifiers")}
        result["modifierTypes"] = [dict(r) for r in db.execute("SELECT * FROM Modifiers")]
        return result

def plain(value):
    if hasattr(value, "items"):
        data = {k: plain(v) for k, v in value.items()}
        if data and set(data) == set(range(1, len(data) + 1)):
            return [data[k] for k in range(1, len(data) + 1)]
        return data
    return value

class ReligionPersistenceTests(unittest.TestCase):
    def setUp(self):
        self.tables = catalog()
        self.load()

    def load(self, saved=None, tables=None, engine=None):
        self.lua = LuaRuntime(unpack_returned_tuples=True)
        initial = self.lua.table_from(saved, recursive=True) if isinstance(saved, dict) else saved
        data = tables or self.tables
        self.lua.execute(STUBS, self.lua.table_from({k: data[k] for k in TABLES}, recursive=True), initial,
                         self.lua.table_from(engine, recursive=True) if engine is not None else None,
                         self.lua.table_from(data["modifiers"], recursive=True))
        self.lua.execute(SCRIPT)
        self.lua.execute(UI_SCRIPT)
        self.g = self.lua.globals()
        self.rp = self.g.ExposedMembers.JAC_ReligionPersistence

    def found(self, religion=R1, followers=3, facilities=1, founder=0):
        self.rp.Found(religion, founder, "origin", followers, facilities, "PLAYER_0")

    def lost(self, religion=R1):
        self.rp.SetCommunity(religion, "origin", 0, 0, "PLAYER_0")

    def state(self, religion=R1):
        return plain(self.rp.Inspect(religion))

    def reload(self):
        before = self.rp.Snapshot()
        turn = self.g.TURN
        saved = json.loads(json.dumps(plain(self.rp.Export()), ensure_ascii=False))
        self.load(saved, engine=plain(self.g.ENGINE))
        self.g.TURN = turn
        self.assertEqual(self.g.WRITES, 0)
        self.assertEqual(self.rp.Snapshot(), before)

    def rejected(self, method, *args):
        before, writes = self.rp.Snapshot(), self.g.WRITES
        with self.assertRaises(LuaError):
            method(*args)
        self.assertEqual(self.rp.Snapshot(), before)
        self.assertEqual(self.g.WRITES, writes)

    def test_catalog_has_twenty_placeholder_slots_and_ssot_sources(self):
        self.assertEqual([r["ID"] for r in self.tables["Religions"]],
                         [f"JAC_RP_PLACEHOLDER_{n:02d}" for n in range(1, 21)])
        self.assertEqual({r["ID"] for r in self.tables["AssetKinds"]},
                         {"SCRIPTURE", "RELIC", "RECORD", "PERSON", "HOLY_SITE"})
        self.assertEqual({r["ID"] for r in self.tables["AssetKinds"] if r["RevivalSource"]},
                         {"SCRIPTURE", "RELIC", "RECORD", "PERSON"})
        self.assertEqual((len(self.tables["Jobs"]), len(self.tables["Sites"]),
                          len(self.tables["Outcomes"])), (7, 5, 9))

    def test_manifest_packages_every_file_and_separates_gameplay(self):
        manifest = ET.parse(MOD / "JAC_PoC_ReligionPersistence.modinfo").getroot()
        files = {f.text for f in manifest.findall("./Files/File")}
        self.assertEqual(files, {p.relative_to(MOD).as_posix() for p in MOD.rglob("*")
                                 if p.is_file() and p.suffix != ".modinfo"})
        self.assertEqual(manifest.findtext("./InGameActions/AddGameplayScripts/File"),
                         "Scripts/JAC_ReligionPersistence.lua")
        self.assertEqual(manifest.findtext("./InGameActions/AddUserInterfaces/File"),
                         "UI/JAC_ReligionPersistence_HeroUI.xml")
        ui_file = MOD / manifest.findtext("./InGameActions/AddUserInterfaces/File")
        context = ET.parse(ui_file).getroot()
        self.assertEqual(context.tag, "Context")
        self.assertEqual(context.get("Name"), ui_file.stem)
        self.assertEqual(len(context), 0)  # No visible controls.
        self.assertEqual(ui_file.with_suffix(".lua").read_text(encoding="utf-8"), UI_SCRIPT)
        self.assertEqual({f.text for f in manifest.findall("./InGameActions/ImportFiles/File")},
                         {"UI/JAC_ReligionPersistence_HeroUI.xml", "UI/JAC_ReligionPersistence_HeroUI.lua"})
        self.assertLess(int(manifest.findtext("./InGameActions/UpdateDatabase/Properties/LoadOrder")),
                        int(manifest.findtext("./InGameActions/AddGameplayScripts/Properties/LoadOrder")))

    def test_every_ingame_action_requires_banseiki_and_excludes_standard_gs(self):
        manifest = ET.parse(MOD / "JAC_PoC_ReligionPersistence.modinfo").getroot()
        criteria = {c.get("id"): c for c in manifest.findall("./ActionCriteria/Criteria")}
        actions = list(manifest.find("InGameActions"))
        self.assertEqual({a.tag for a in actions},
                         {"UpdateDatabase", "UpdateText", "AddGameplayScripts", "AddUserInterfaces", "ImportFiles"})
        for action in actions:
            with self.subTest(action=action.get("id")):
                self.assertIn(action.get("criteria"), criteria)
                rulesets = criteria[action.get("criteria")].findtext("RuleSetInUse").split(",")
                self.assertEqual([r.strip() for r in rulesets], ["RULESET_JAC_BANSEIKI"])
        self.assertIn("bd7a82c5-6b38-4b69-a7f5-8dc9af534249",
                      {m.get("id") for m in manifest.findall("./Dependencies/Mod")})

    def test_fast_deadlines_are_gated_and_leave_production_values_intact(self):
        manifest = ET.parse(MOD / "JAC_PoC_ReligionPersistence.modinfo").getroot()
        action = manifest.find("./InGameActions/UpdateDatabase[@id='JAC_RP_TestDeadlines']")
        criterion = manifest.find("./ActionCriteria/Criteria[@id='" + action.get("criteria") + "']")
        self.assertEqual(criterion.findtext("ConfigurationValueMatches/ConfigurationId"), "JAC_AUTOTEST")
        self.assertEqual(criterion.findtext("ConfigurationValueMatches/Value"), "1")
        with sqlite3.connect(":memory:") as db:
            db.executescript("CREATE TABLE JAC_RP_Rules(ID TEXT PRIMARY KEY, Value INTEGER);"
                             "INSERT INTO JAC_RP_Rules VALUES('REVIVAL_COOLDOWN_TURNS',10);"
                             "CREATE TABLE GlobalParameters(Name TEXT PRIMARY KEY, Value INTEGER);"
                             "INSERT INTO GlobalParameters VALUES('DIPLOMACY_ALLIANCE_TIME_LIMIT',30);")
            db.executescript((MOD / action.findtext("File")).read_text(encoding="utf-8"))
            self.assertEqual(db.execute("SELECT Value FROM JAC_RP_Rules").fetchone()[0], 1)
            self.assertEqual(db.execute("SELECT Value FROM GlobalParameters").fetchone()[0], 1)
        self.assertEqual(next(r["Value"] for r in self.tables["Rules"] if r["ID"] == "REVIVAL_COOLDOWN_TURNS"), 10)

    def test_all_catalog_and_state_names_have_ja_en_ru_text(self):
        rows = list(ET.parse(MOD / "Text/JAC_ReligionPersistence_Text.xml").getroot().iter("Row"))
        texts = {(r.get("Tag"), r.get("Language")): r.findtext("Text") for r in rows}
        self.assertEqual(len(rows), len(texts))
        tags = {r["Name"] for name in TABLES if name != "Rules" for r in self.tables[name]}
        tags |= {"LOC_JAC_RP_" + x for x in ("PUBLIC", "UNDERGROUND", "LOST", "DEFERRED",
                                            "FOLLOWERS", "FACILITIES", "FOUNDER_DEFEATED", "CAN_REVIVE",
                                            "APPLIED", "PENDING")}
        self.assertEqual(len(tags), 56)
        self.assertEqual(set(texts), {(tag, lang) for tag in tags
                                     for lang in ("ja_JP", "en_US", "ru_RU")})
        for value in texts.values():
            self.assertTrue(value and not value.startswith("LOC_"))
            self.assertNotIn("\ufffd", value)
        manifest = ET.parse(MOD / "JAC_PoC_ReligionPersistence.modinfo").getroot()
        for row in manifest.findall("./LocalizedText/Text"):
            self.assertEqual({x.tag for x in row}, {"ja_JP", "en_US", "ru_RU"})

    def test_fresh_game_and_script_reload_do_not_write_or_found_anything(self):
        self.assertEqual(plain(self.rp.Export()), {"schema": 1, "revision": 0, "religions": {},
                                                  "heroes": {}, "artifacts": {}, "religiousAlliances": {}})
        self.assertIsNone(self.g.PROPERTY_STORE[KEY])
        self.lua.execute(SCRIPT)
        self.assertEqual(self.g.WRITES, 0)

    def test_mechanism_accepts_a_replaced_religion_catalog(self):
        data = catalog()
        data["Religions"] = [{"ID": "OTHER_CATALOG_FAITH", "Name": "LOC_OTHER"}]
        self.load(tables=data)
        self.found("OTHER_CATALOG_FAITH")
        self.assertEqual(self.state("OTHER_CATALOG_FAITH")["followers"], 3)
        self.rejected(self.rp.Found, R1, 0, "origin", 3, 1, "PLAYER_0")
    def test_founder_defeat_keeps_believers_assets_and_other_faction(self):
        self.found()
        self.rp.SetCommunity(R1, "diaspora", 1, 0, "PLAYER_1")
        self.rp.PreserveAsset(R1, "book", "SCRIPTURE", "EXILE_1")
        self.rp.NotifyFounderDefeated(0)
        self.assertEqual((self.state()["followers"], self.state()["state"]), (4, "PUBLIC"))
        self.assertTrue(self.state()["founderDefeated"])
        self.assertEqual(self.state()["assets"]["book"]["holder"], "EXILE_1")
        self.rp.SetCommunity(R1, "origin", 0, 0, "PLAYER_0")
        self.assertEqual((self.state()["followers"], self.state()["state"]), (1, "UNDERGROUND"))
        self.reload()
        before = self.rp.Snapshot()
        self.rp.NotifyFounderDefeated(0)
        self.assertEqual(self.rp.Snapshot(), before)

    def test_global_last_follower_is_required_for_loss(self):
        self.found(followers=1, facilities=0)
        self.rp.SetCommunity(R1, "elsewhere", 1, 1, "PLAYER_1")
        self.lost()
        self.assertEqual((self.state()["state"], self.state()["followers"]), ("PUBLIC", 1))
        self.rp.SetCommunity(R1, "elsewhere", 0, 1, "PLAYER_1")
        self.assertEqual((self.state()["state"], self.state()["followers"]), ("LOST", 0))
        self.assertEqual(self.state()["publicFacilities"], 1)
        self.assertFalse(self.state()["canRevive"])

    def test_underground_believers_need_no_public_facility_or_item(self):
        self.found(followers=1, facilities=0)
        self.assertEqual(self.state()["state"], "UNDERGROUND")
        self.assertFalse(self.state()["lost"])
        self.reload()

    def test_loss_retains_all_assets_and_underground_evidence(self):
        self.found()
        for kind in ("SCRIPTURE", "RELIC", "RECORD", "PERSON", "HOLY_SITE"):
            self.rp.PreserveAsset(R1, kind, kind, "EXILE_1" if kind == "PERSON" else "CAVE")
        before = self.state()["assets"]
        self.lost()
        self.assertEqual(self.state()["assets"], before)
        self.assertEqual(self.state()["state"], "LOST")
        self.assertTrue(self.state()["underground"])
        self.assertTrue(self.state()["canRevive"])
        self.reload()
        self.assertEqual(self.state()["followers"], 0)

    def test_each_of_the_four_revival_sources_survives_load(self):
        for kind in ("SCRIPTURE", "RELIC", "RECORD", "PERSON"):
            with self.subTest(kind=kind):
                self.load()
                self.found()
                self.rp.PreserveAsset(R1, "source", kind, "EXILE_1", 1)
                self.lost()
                self.reload()
                self.rp.Revive(R1, "revival", "source", "new", 1, 0)
                self.assertEqual((self.state()["state"], self.state()["followers"]), ("UNDERGROUND", 1))
                self.assertEqual(self.state()["revivalCount"], 1)
                self.assertEqual(self.state()["assets"]["source"]["kind"], kind)
                self.assertEqual(self.g.ENGINE["1"].faith, 480)
                self.reload()

    def test_holy_site_alone_and_no_source_cannot_revive(self):
        self.found()
        self.lost()
        self.rejected(self.rp.Revive, R1, "r", "absent", "new", 1, 0)
        self.rp.PreserveAsset(R1, "site", "HOLY_SITE", "RUINS")
        self.assertFalse(self.state()["canRevive"])
        self.rejected(self.rp.Revive, R1, "r", "site", "new", 1, 0)
        self.rp.PreserveAsset(R1, "record", "RECORD", "SCHOLAR_1")
        self.assertTrue(self.state()["canRevive"])
        self.rp.RemoveAsset(R1, "record")
        self.assertFalse(self.state()["canRevive"])
        self.rp.RemoveAsset(R1, "site")
        self.assertFalse(self.state()["underground"])
        self.assertEqual(self.state()["state"], "LOST")
        self.reload()

    def test_item_transfer_preserves_identity_kind_and_revival_source(self):
        self.found()
        self.rp.PreserveAsset(R1, "book", "SCRIPTURE", "PLAYER_0")
        self.rp.MoveAsset(R1, "book", "EXILE_1")
        self.assertEqual(self.state()["assets"]["book"], {"kind": "SCRIPTURE", "holder": "EXILE_1"})
        self.rejected(self.rp.PreserveAsset, R1, "book", "RELIC", "PLAYER_1")
        self.rejected(self.rp.MoveAsset, R1, "absent", "PLAYER_1")
        self.reload()

    def test_revival_is_explicit_and_duplicate_revival_is_rejected(self):
        self.found()
        self.rp.PreserveAsset(R1, "book", "SCRIPTURE", "PLAYER_1", 1)
        self.rejected(self.rp.Revive, R1, "r", "book", "new", 1, 0)
        self.lost()
        self.rejected(self.rp.Revive, R1, "r", "book", "new", 0, 0)
        self.rp.Revive(R1, "r", "book", "new", 1, 0)
        self.assertEqual(self.state()["state"], "UNDERGROUND")
        self.rejected(self.rp.Revive, R1, "r", "book", "new", 1, 0)
        self.rp.SetCommunity(R1, "new", 0, 0, "PLAYER_1")
        self.rejected(self.rp.Revive, R1, "r2", "book", "new2", 1, 0)
        self.g.TURN += 10
        self.rp.Revive(R1, "r2", "book", "new2", 1, 0)
        self.assertEqual(self.state()["revivalCount"], 2)
        self.assertEqual(self.g.ENGINE["1"].faith, 460)
        self.reload()
        self.assertEqual(set(self.state()["revivals"]), {"r", "r2"})

    def test_revival_picks_a_free_slot_and_preserves_both_religions(self):
        for discovered in (False, True):
            with self.subTest(discovered=discovered):
                self.load()
                self.found()
                self.found(R2, followers=5, facilities=4)
                self.rp.SetCommunity(R1, "origin", 0, 2, "PLAYER_0")
                self.rp.SetCommunity(R1, "origin:revival_1", 0, 3, "ARCHIVE")
                if discovered:
                    self.rp.Discover(R1, "d", "book", "SCRIPTURE", "MONK_1", "MONK", "CAVE", 1)
                else:
                    self.rp.PreserveAsset(R1, "book", "SCRIPTURE", "PLAYER_1", 1)
                self.reload()
                original, other = self.state()["communities"], self.state(R2)
                if discovered:
                    self.rp.ResolveDiscovery(R1, "d", "REVIVE", self.options(community="origin"))
                else:
                    self.rp.Revive(R1, "r", "book", "origin", 1, 0)
                revived = self.state()
                selected = revived["revivals"]["r"]["community"]
                self.assertNotIn(selected, original)
                self.assertEqual({k: revived["communities"][k] for k in original}, original)
                self.assertEqual(revived["communities"][selected],
                                 {"followers": 1, "publicFacilities": 0, "holder": "PLAYER_1"})
                self.assertEqual((revived["followers"], revived["publicFacilities"], revived["state"]),
                                 (1, 5, "PUBLIC"))
                self.assertEqual(self.state(R2), other)
                self.assertEqual(self.g.ENGINE["1"].faith, 480)
                self.reload()
                self.assertEqual(self.state(), revived)
                self.assertEqual(self.state(R2), other)

    def test_revival_does_not_overwrite_a_slot_created_by_the_faith_callback(self):
        self.found()
        self.lost()
        self.rp.PreserveAsset(R1, "book", "SCRIPTURE", "PLAYER_1", 1)
        self.lua.execute("""
            local religion = Players[1]:GetReligion()
            local change = religion.ChangeFaithBalance
            religion.ChangeFaithBalance = function(self, amount)
                change(self, amount)
                ExposedMembers.JAC_ReligionPersistence.SetCommunity(
                    'JAC_RP_PLACEHOLDER_01', 'new', 0, 7, 'PLAYER_0')
            end
        """)
        self.rp.Revive(R1, "r", "book", "new", 1, 0)
        revived = self.state()
        selected = revived["revivals"]["r"]["community"]
        self.assertNotEqual(selected, "new")
        self.assertEqual(revived["communities"]["new"],
                         {"followers": 0, "publicFacilities": 7, "holder": "PLAYER_0"})
        self.assertEqual(revived["communities"][selected],
                         {"followers": 1, "publicFacilities": 0, "holder": "PLAYER_1"})
        self.assertEqual((revived["followers"], revived["publicFacilities"], revived["revivalCount"]), (1, 7, 1))
        self.assertEqual(self.g.ENGINE["1"].faith, 480)
        self.reload()
        self.assertEqual(self.state(), revived)

    def test_revival_free_slot_respects_the_identifier_length_limit(self):
        preferred = "x" * 128
        self.rp.Found(R1, 0, preferred, 1, 1, "PLAYER_0")
        self.rp.SetCommunity(R1, preferred, 0, 1, "PLAYER_0")
        self.rp.PreserveAsset(R1, "book", "SCRIPTURE", "PLAYER_1", 1)
        self.rp.Revive(R1, "r", "book", preferred, 1, 0)
        selected = self.state()["revivals"]["r"]["community"]
        self.assertNotEqual(selected, preferred)
        self.assertLessEqual(len(selected), 128)
        self.assertEqual(self.state()["communities"][preferred],
                         {"followers": 0, "publicFacilities": 1, "holder": "PLAYER_0"})
        self.reload()

    def test_discovery_accepts_all_listed_jobs_sites_and_both_item_kinds(self):
        self.found()
        self.lost()
        n = 0
        for job in self.tables["Jobs"]:
            for site in self.tables["Sites"]:
                for kind in ("SCRIPTURE", "RELIC"):
                    n += 1
                    self.rp.Discover(R1, f"d{n}", f"item{n}", kind, "ACTOR_1", job["ID"], site["ID"], 1)
        self.assertEqual(n, 70)
        self.assertEqual(len(self.state()["discoveries"]), 70)
        self.assertEqual(len(self.state()["assets"]), 70)
        self.reload()

    def test_discovery_transfers_existing_item_and_rejects_wrong_site_and_replays(self):
        self.found()
        self.rp.PreserveAsset(R1, "book", "SCRIPTURE", "CAVE")
        self.rejected(self.rp.Discover, R1, "d", "book", "SCRIPTURE", "MONK_1", "MONK", "CAVE", 1)
        self.lost()
        self.rejected(self.rp.Discover, R1, "d", "book", "SCRIPTURE", "MONK_1", "MONK", "RUINS", 1)
        self.rejected(self.rp.Discover, R1, "d", "new", "RECORD", "MONK_1", "MONK", "CAVE", 1)
        self.rejected(self.rp.Discover, R1, "d", "new", "SCRIPTURE", "MONK_1", "UNKNOWN", "CAVE", 1)
        self.rejected(self.rp.Discover, R1, "d", "new", "SCRIPTURE", "MONK_1", "MONK", "UNKNOWN", 1)
        self.rp.Discover(R1, "d", "book", "SCRIPTURE", "MONK_1", "MONK", "CAVE", 1)
        self.assertEqual(self.state()["assets"]["book"]["holder"], "MONK_1")
        self.rejected(self.rp.Discover, R1, "d", "book", "SCRIPTURE", "MONK_1", "MONK", "CAVE", 1)
        self.reload()
    def options(self, **kwargs):
        return self.lua.table_from({"city": 0, "revival": "r", "community": "new",
                                    "targetReligion": R2, "targetPlayer": 0, **kwargs})

    def discovery(self, job="MONK", actor="MONK_1", religion=R1, item="book", discovery="d", player=1):
        self.rp.Discover(religion, discovery, item, "SCRIPTURE", actor, job, "CAVE", player)

    def bind_absorption(self):
        self.rp.BindEngineReligion(R1, 42)
        self.rp.BindEngineReligion(R2, 43)

    def test_every_outcome_applies_the_exact_reward_and_survives_load_without_replay(self):
        outcomes = [r["ID"] for r in self.tables["Outcomes"]]
        self.assertEqual(set(outcomes), {"REVIVE", "SELL", "CONCEAL", "RESEARCH", "HERESY",
                                        "ABSORB", "ARTIFACT", "DIPLOMATIC_INCIDENT", "HERO"})
        rewards = {"SELL": {"gold": 500}, "RESEARCH": {"science": 10, "culture": 10},
                   "HERESY": {"culture": 20, "faith": -400}, "ABSORB": {"faith": -500},
                   "ARTIFACT": {"culture": 30}, "DIPLOMATIC_INCIDENT": {"favor": 5, "gold": 50},
                   "REVIVE": {"faith": -20}, "HERO": {"faith": 32.5}}
        for outcome in outcomes:
            with self.subTest(outcome=outcome):
                self.load()
                self.found()
                self.lost()
                self.found(R2)
                self.rp.SetCommunity(R2, "origin", 3, 1, "PLAYER_1")
                self.discovery()
                if outcome == "ABSORB":
                    self.bind_absorption()
                before = plain(self.g.ENGINE["1"])
                self.rp.ResolveDiscovery(R1, "d", outcome, self.options())
                if outcome == "HERO":
                    self.g.HERO_UI_TICK()
                d = self.state()["discoveries"]["d"]
                self.assertEqual(d["outcome"], outcome)
                self.assertEqual(d["effects"], "APPLIED")
                self.assertEqual(self.state()["followers"], 1 if outcome == "REVIVE" else 4 if outcome == "ABSORB" else 0)
                self.assertEqual(self.state()["revivalCount"], 1 if outcome == "REVIVE" else 0)
                after = plain(self.g.ENGINE["1"])
                for yield_name in ("gold", "faith", "science", "culture", "favor"):
                    self.assertEqual(after[yield_name] - before[yield_name], rewards.get(outcome, {}).get(yield_name, 0))
                self.assertEqual(len(after["units"]), 1 if outcome == "HERO" else 0)
                if outcome == "ABSORB":
                    self.assertNotIn("book", self.state()["assets"])
                    self.assertEqual(self.state(R2)["assets"]["book"]["originReligion"], R1)
                if outcome == "ARTIFACT":
                    self.assertTrue(self.state()["assets"]["book"]["vault"])
                if outcome == "HERESY":
                    self.assertNotIn("book", self.state()["assets"])
                    self.assertEqual(self.state()["materialUses"]["book"]["interpretation"], "HERETICAL")
                if outcome == "RESEARCH":
                    self.assertNotIn("book", self.state()["assets"])
                    self.assertEqual(after["boosted"], {"2": True, "4": True})
                if outcome == "DIPLOMATIC_INCIDENT":
                    self.assertEqual(self.state()["assets"]["book"]["owner"], 0)
                self.reload()
                self.assertEqual(plain(self.g.ENGINE["1"]), after)
                self.rejected(self.rp.ResolveDiscovery, R1, "d", outcome, self.options())
                self.assertEqual(plain(self.g.ENGINE["1"]), after)

    def test_removed_discovery_source_prevents_revival_without_marking_resolution(self):
        self.found()
        self.lost()
        self.discovery()
        self.rp.RemoveAsset(R1, "book")
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "REVIVE", self.options())
        self.assertNotIn("outcome", self.state()["discoveries"]["d"])

    def test_direct_revival_cannot_strand_an_unresolved_discovery(self):
        for kind in ("SCRIPTURE", "RELIC"):
            with self.subTest(kind=kind):
                self.load()
                self.found()
                self.lost()
                self.rp.Discover(R1, "d", "book", kind, "MONK_1", "MONK", "CAVE", 1)
                self.reload()
                before = plain(self.g.ENGINE)
                self.rejected(self.rp.Revive, R1, "direct", "book", "new", 1, 0)
                self.assertEqual(plain(self.g.ENGINE), before)
                self.assertNotIn("outcome", self.state()["discoveries"]["d"])
                self.rp.ResolveDiscovery(R1, "d", "REVIVE", self.options())
                discovery = self.state()["discoveries"]["d"]
                self.assertEqual((discovery["outcome"], discovery["effects"]), ("REVIVE", "APPLIED"))
                self.assertEqual(self.g.ENGINE["1"].faith, 480)
                self.reload()

    def test_research_chooses_two_new_eurekas_in_type_order_and_consumes_material(self):
        self.found()
        self.lost()
        self.discovery()
        self.g.ENGINE["1"].researched["2"] = True
        self.g.ENGINE["1"].boosted["4"] = True
        before = plain(self.g.ENGINE["1"])
        self.rp.ResolveDiscovery(R1, "d", "RESEARCH", self.options())
        self.assertEqual(plain(self.g.ENGINE["1"].boosted), {"4": True, "3": True, "1": True})
        self.assertEqual(self.state()["discoveries"]["d"]["effect"]["boosts"], [3, 1])
        self.assertEqual(self.g.ENGINE["1"].science - before["science"], 10)
        self.assertEqual(self.g.ENGINE["1"].culture - before["culture"], 10)
        self.assertTrue(self.state()["retiredAssets"]["book"])
        self.rejected(self.rp.PreserveAsset, R1, "book", "SCRIPTURE", "CAVE", 1)
        self.reload()

    def test_research_requires_two_distinct_eligible_boosts_before_any_effect(self):
        self.found()
        self.lost()
        self.discovery()
        for index in (1, 2, 3):
            self.g.ENGINE["1"].boosted[str(index)] = True
        before = plain(self.g.ENGINE)
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "RESEARCH", self.options())
        self.assertEqual(plain(self.g.ENGINE), before)

    def test_research_eurekas_precede_science_that_finishes_a_chosen_tech(self):
        self.found()
        self.lost()
        self.discovery()
        self.lua.execute("""
          local p=Players[1]; local attach=p.AttachModifierByID
          p.AttachModifierByID=function(self, id)
            if id=='JAC_RP_RESEARCH_JAC_RP_PLACEHOLDER_01' then
              assert(ENGINE['1'].boosted['2'] and ENGINE['1'].boosted['4'])
              ENGINE['1'].researched['2']=true
            end
            attach(self,id)
          end
        """)
        self.rp.ResolveDiscovery(R1, "d", "RESEARCH", self.options())
        self.assertEqual(plain(self.g.ENGINE["1"].boosted), {"2": True, "4": True})
        self.assertEqual((self.g.ENGINE["1"].science, self.g.ENGINE["1"].culture), (10, 10))
        self.reload()

    def test_heresy_requires_400_faith_and_consumes_material(self):
        self.found()
        self.lost()
        self.discovery()
        self.g.ENGINE["1"].faith = 399
        before = plain(self.g.ENGINE)
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "HERESY", self.options())
        self.assertEqual(plain(self.g.ENGINE), before)
        self.g.ENGINE["1"].faith = 400
        self.rp.ResolveDiscovery(R1, "d", "HERESY", self.options())
        self.assertEqual((self.g.ENGINE["1"].faith, self.g.ENGINE["1"].culture), (0, 20))
        self.assertNotIn("book", self.state()["assets"])
        self.assertTrue(self.state()["retiredAssets"]["book"])
        self.rejected(self.rp.Revive, R1, "r", "book", "new", 1, 0)
        self.reload()

    def test_absorption_rounds_once_worldwide_and_preserves_other_religions(self):
        self.found()
        self.lost()
        self.found(R2)
        self.rp.SetCommunity(R2, "origin", 1, 0, "PLAYER_1")
        self.discovery()
        self.bind_absorption()
        # Five SOURCE citizens split 1+4, one target and one third-faith citizen.
        # ceil(5/3)=2, whereas rounding separately per city would convert 3.
        self.lua.execute("""
          local a,b=ENGINE['0'].cities['0'],ENGINE['1'].cities['0']
          a.population=2; a.pressure={['42']=1024,['44']=1024}
          b.population=5; b.pressure={['42']=4096,['43']=1024}
        """)
        self.g.ENGINE["1"].faith = 499
        before = plain(self.g.ENGINE)
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "ABSORB", self.options())
        self.assertEqual(plain(self.g.ENGINE), before)
        self.g.ENGINE["1"].faith = 500
        self.rp.ResolveDiscovery(R1, "d", "ABSORB", self.options())
        effect = self.state()["discoveries"]["d"]["effect"]
        self.assertEqual((effect["sourceFollowers"], effect["convertedFollowers"]), (5, 2))
        self.assertEqual([(c["player"], c["city"], c["moved"]) for c in effect["conversions"]], [(0, 0, 1), (1, 0, 1)])
        self.assertEqual((self.rp.NativeFollowers(R1), self.rp.NativeFollowers(R2)), (3, 3))
        rows = plain(self.g.NativeRows(self.g.ENGINE["0"].cities["0"]))
        self.assertEqual({r["Religion"]: r["Followers"] for r in rows}, {43: 1, 44: 1})
        self.assertEqual(self.g.ENGINE["1"].faith, 0)
        self.reload()

    def test_absorption_rejects_unbound_or_foreign_native_target(self):
        self.found()
        self.lost()
        self.found(R2)
        self.rp.SetCommunity(R2, "origin", 1, 0, "PLAYER_1")
        self.discovery()
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "ABSORB", self.options())
        self.rp.BindEngineReligion(R1, 42)
        self.rp.BindEngineReligion(R2, 44)
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "ABSORB", self.options())
        self.rejected(self.rp.BindEngineReligion, R2, 43)

    def test_absorption_clears_maximum_fixed_point_pressure_including_fraction(self):
        self.found()
        self.lost()
        self.found(R2)
        self.discovery()
        self.bind_absorption()
        self.lua.execute("ENGINE['0'].cities['0'].pressure={['42']=8388607+255/256}; "
                         "ENGINE['1'].cities['0'].pressure={['43']=1024}")
        self.rp.ResolveDiscovery(R1, "d", "ABSORB", self.options())
        self.assertEqual((self.rp.NativeFollowers(R1), self.rp.NativeFollowers(R2)), (4, 7))
        self.assertEqual(self.g.ENGINE["1"].faith, 0)

    def test_absorption_preserves_unaffiliated_citizens_even_when_row_is_omitted(self):
        for omitted, sentinel in ((False, -1), (True, -1), (False, 0), (True, 0)):
            with self.subTest(omitted=omitted, sentinel=sentinel):
                self.load()
                self.found()
                self.lost()
                self.found(R2)
                self.rp.SetCommunity(R2, "origin", 1, 0, "PLAYER_1")
                self.discovery()
                self.bind_absorption()
                self.g.OMIT_UNAFFILIATED_ROWS = omitted
                self.lua.execute("""
                  local a,b=ENGINE['0'].cities['0'],ENGINE['1'].cities['0']
                  a.population=3; a.pressure={['42']=256,['44']=256,['-1']=256}
                  b.population=5; b.pressure={['42']=4096,['43']=1024}
                """)
                if sentinel == 0:
                    self.lua.execute("local c=ENGINE['0'].cities['0']; c.pressure['0']=c.pressure['-1']; c.pressure['-1']=nil")
                self.g.ENGINE["1"].faith = 500
                self.rp.ResolveDiscovery(R1, "d", "ABSORB", self.options())
                self.assertEqual((self.rp.NativeFollowers(R1), self.rp.NativeFollowers(R2)), (3, 3))
                self.assertEqual(self.g.ENGINE["0"].cities["0"].pressure[str(sentinel)], 256)
                self.g.OMIT_UNAFFILIATED_ROWS = False
                rows = plain(self.g.NativeRows(self.g.ENGINE["0"].cities["0"]))
                self.assertEqual({r["Religion"]: r["Followers"] for r in rows}, {43: 1, 44: 1, sentinel: 1})
                self.assertEqual(self.g.ENGINE["1"].faith, 0)

    def test_bound_native_counts_prevent_false_loss_and_revival(self):
        self.found()
        self.lost()
        self.rp.PreserveAsset(R1, "book", "SCRIPTURE", "PLAYER_1", 1)
        self.rp.BindEngineReligion(R1, 42)
        self.assertEqual(self.state()["followers"], 7)
        self.assertFalse(self.state()["lost"])
        self.rejected(self.discovery)
        self.rejected(self.rp.Revive, R1, "r", "book", "new", 1, 0)
        self.lost()
        self.assertEqual(self.state()["followers"], 7)
        self.reload()

    def test_zero_native_absorption_does_not_charge_or_reserve(self):
        self.found()
        self.lost()
        self.discovery()
        self.found(R2)
        self.rp.SetCommunity(R2, "origin", 1, 0, "PLAYER_1")
        self.lua.execute("ENGINE['0'].cities['0'].pressure={['43']=7168}")
        self.bind_absorption()
        self.g.ENGINE["1"].faith = 500
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "ABSORB", self.options())
        self.assertEqual(self.g.ENGINE["1"].faith, 500)

    def test_stale_owned_community_cannot_authorize_absorption(self):
        self.found()
        self.lost()
        self.discovery()
        self.found(R2)
        self.rp.SetCommunity(R2, "origin", 1, 0, "PLAYER_1")
        self.lua.execute("ENGINE['0'].cities['0'].pressure={['42']=4096,['43']=3072}; "
                         "ENGINE['1'].cities['0'].pressure={['42']=4096}")
        self.bind_absorption()
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "ABSORB", self.options())

    def test_actual_owned_followers_authorize_absorption_with_zero_manual_count(self):
        self.found()
        self.lost()
        self.discovery()
        self.found(R2)
        self.rp.SetCommunity(R2, "origin", 0, 0, "PLAYER_0")
        self.bind_absorption()
        self.rp.ResolveDiscovery(R1, "d", "ABSORB", self.options())
        self.assertEqual(self.rp.NativeFollowers(R1), 4)

    def test_native_revival_uses_numeric_religion_order(self):
        self.found()
        self.lost()
        self.discovery()
        self.lua.execute("ENGINE['0'].cities['0'].pressure={['43']=7168}; "
                         "ENGINE['1'].cities['0'].pressure={['2']=2048,['10']=2048}")
        self.rp.BindEngineReligion(R1, 42)
        self.rp.ResolveDiscovery(R1, "d", "REVIVE", self.options())
        e = self.state()["discoveries"]["d"]["effect"]["conversions"][0]
        self.assertEqual((e["after"]["2"], e["after"]["10"], e["after"]["42"]), (1, 2, 1))

    def test_bound_revival_creates_one_native_citizen_and_survives_load(self):
        self.found()
        self.lost()
        self.discovery()
        self.lua.execute("ENGINE['0'].cities['0'].pressure={['43']=7168}; "
                         "ENGINE['1'].cities['0'].pressure={['43']=4096}")
        self.rp.BindEngineReligion(R1, 42)
        self.assertTrue(self.state()["lost"])
        before = self.g.ENGINE["1"].faith
        self.rp.ResolveDiscovery(R1, "d", "REVIVE", self.options())
        self.assertEqual(self.rp.NativeFollowers(R1), 1)
        self.assertEqual(self.state()["followers"], 1)
        self.assertFalse(self.state()["lost"])
        self.assertEqual(self.g.ENGINE["1"].faith, before - 20)
        self.reload()

    def test_inexact_engine_conversion_stays_pending_and_never_replays(self):
        self.found()
        self.lost()
        self.found(R2)
        self.rp.SetCommunity(R2, "origin", 1, 0, "PLAYER_1")
        self.discovery()
        self.bind_absorption()
        self.g.BAD_CONVERSION = True
        with self.assertRaisesRegex(LuaError, "conversion count mismatch"):
            self.rp.ResolveDiscovery(R1, "d", "ABSORB", self.options())
        self.assertEqual(self.state()["discoveries"]["d"]["effects"], "PENDING")
        self.assertEqual(self.g.ENGINE["1"].faith, 500)
        self.reload()
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "ABSORB", self.options())

    def test_restitution_transfers_half_balances_and_prohibits_persecution_both_ways(self):
        self.found()
        self.lost()
        self.found(R2)
        self.discovery()
        self.g.ENGINE["0"].gold = 101.5
        self.g.ENGINE["0"].favor = 11
        self.rp.ResolveDiscovery(R1, "d", "DIPLOMATIC_INCIDENT", self.options())
        self.assertEqual((self.g.ENGINE["0"].gold, self.g.ENGINE["1"].gold), (50.75, 150.75))
        self.assertEqual((self.g.ENGINE["0"].favor, self.g.ENGINE["1"].favor), (6, 15))
        self.assertFalse(self.rp.CanPersecute(0, R1, 1, R2)[0])
        self.assertFalse(self.rp.CanPersecute(1, R2, 0, R1)[0])
        self.assertTrue(self.rp.CanPersecute(0, R2, 1, R2))
        with self.assertRaisesRegex(LuaError, "alliance prohibits"):
            self.rp.AssertCanPersecute(1, R2, 0, R1)
        self.reload()
        self.assertFalse(self.rp.CanPersecute(0, R1, 1, R2)[0])

    def test_religious_alliance_ends_on_diplomatic_end_war_or_defeat_and_does_not_restart(self):
        for cause in ("allied", "war", "alive"):
            with self.subTest(cause=cause):
                self.load()
                self.found()
                self.lost()
                self.found(R2)
                self.discovery()
                self.rp.ResolveDiscovery(R1, "d", "DIPLOMATIC_INCIDENT", self.options())
                self.g.ENGINE["1"][cause] = cause == "war"
                self.g.TURN += 1
                self.g.TURN_HOOK()
                a = next(iter(plain(self.rp.Export())["religiousAlliances"].values()))
                self.assertEqual((a["active"], a["endedTurn"]), (False, self.g.TURN))
                self.g.ENGINE["1"][cause] = cause != "war"
                self.reload()
                self.assertTrue(self.rp.CanPersecute(0, R1, 1, R2))
                self.assertTrue(self.rp.CanPersecute(1, R2, 0, R1))

    def test_hero_waits_for_replicated_lifespan_property_before_reward(self):
        self.found()
        self.lost()
        self.discovery()
        self.rp.ResolveDiscovery(R1, "d", "HERO", self.options())
        self.lua.execute("local h=ExposedMembers.JAC_ReligionPersistence.Export().heroes['1']; "
                         "ENGINE['1'].units[tostring(h.unit)].lifespan=nil")
        self.g.HERO_UI_TICK()
        self.assertEqual(self.rp.Export().heroes["1"].verification, "PENDING")
        self.assertEqual(self.g.ENGINE["1"].faith, 500)
        self.lua.execute("local h=ExposedMembers.JAC_ReligionPersistence.Export().heroes['1']; "
                         "ENGINE['1'].units[tostring(h.unit)].lifespan=30")
        self.g.HERO_UI_TICK()
        self.assertEqual(self.rp.Export().heroes["1"].verification, "APPLIED")
        self.assertEqual(self.g.ENGINE["1"].faith, 532.5)

    def test_hero_verification_accepts_unset_test_flag_in_manual_game(self):
        for getter in ("function() return nil end", "function() end"):
            with self.subTest(getter=getter):
                self.load()
                self.found()
                self.lost()
                self.discovery()
                self.rp.ResolveDiscovery(R1, "d", "HERO", self.options())
                self.reload()
                self.lua.execute("GameConfiguration.GetValue=" + getter)
                self.g.HERO_UI_LOAD_CLOSE()
                self.g.HERO_UI_FRAME(0)
                self.assertEqual(self.rp.Export().heroes["1"].verification, "APPLIED")
                self.assertEqual(self.g.ENGINE["1"].faith, 532.5)

    def test_hidden_addin_activates_after_load_and_completes_pending_hero_once(self):
        self.found()
        self.lost()
        self.discovery()
        self.rp.ResolveDiscovery(R1, "d", "HERO", self.options())
        self.reload()
        before = self.g.ENGINE["1"].faith
        self.g.HERO_UI_FRAME(0)
        self.assertEqual(self.rp.Export().heroes["1"].verification, "PENDING")
        self.assertEqual(self.g.ENGINE["1"].faith, before)
        self.g.HERO_UI_LOAD_CLOSE()
        self.g.HERO_UI_FRAME(0)
        self.assertFalse(self.g.HERO_UI_HIDDEN)
        self.assertEqual(self.rp.Export().heroes["1"].verification, "APPLIED")
        self.assertEqual(self.g.ENGINE["1"].faith, before + 32.5)
        self.g.HERO_UI_LOAD_CLOSE()
        self.g.HERO_UI_FRAME(0)
        self.assertEqual(self.g.ENGINE["1"].faith, before + 32.5)

    def test_official_hero_requires_mode_origin_building_and_unclaimed_hero(self):
        self.found()
        self.lost()
        self.discovery()
        self.g.HEROES_MODE = False
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "HERO", self.options())
        self.g.HEROES_MODE = True
        self.g.ENGINE["1"].cities["0"].monument = False
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "HERO", self.options())
        self.g.ENGINE["1"].cities["0"].monument = True
        self.g.ENGINE["0"].heroClaims["10"] = 0
        self.g.ENGINE["0"].heroClaims["11"] = 0
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "HERO", self.options())
        self.g.ENGINE["0"].heroClaims["11"] = None
        self.g.ENGINE["1"].faith = 0
        self.rp.ResolveDiscovery(R1, "d", "HERO", self.options())
        self.g.HERO_UI_TICK()
        self.assertEqual(self.g.ENGINE["1"].faith, 32.5)
        receipt = plain(self.rp.Export())["heroes"]["1"]
        units = self.g.Players[1].GetUnits()
        unit = units.FindID(units, receipt["unit"])
        self.assertEqual((unit.GetType(), unit.GetHeroClassType(), unit.GetProperty(unit, self.g.UnitPropertyKeys.LifespanRemaining)), (101, 11, 30))
        self.assertEqual(self.g.ENGINE["0"].heroClaims["11"], 1)
        self.reload()

    def test_hero_reward_waits_for_ui_verification_and_survives_pending_save(self):
        self.found()
        self.lost()
        self.discovery()
        before = self.g.ENGINE["1"].faith
        self.rp.ResolveDiscovery(R1, "d", "HERO", self.options())
        self.assertEqual(self.state()["discoveries"]["d"]["effects"], "PENDING")
        self.assertEqual(self.g.ENGINE["1"].faith, before)
        self.reload()
        self.g.HERO_UI_TICK()
        self.assertEqual(self.state()["discoveries"]["d"]["effects"], "APPLIED")
        self.assertEqual(self.g.ENGINE["1"].faith, before + 32.5)
        self.g.HERO_UI_TICK()
        self.assertEqual(self.g.ENGINE["1"].faith, before + 32.5)

    def test_unverified_hero_claim_never_pays_faith(self):
        self.found()
        self.lost()
        self.discovery()
        before = self.g.ENGINE["1"].faith
        self.rp.ResolveDiscovery(R1, "d", "HERO", self.options())
        self.g.ENGINE["0"].heroClaims["10"] = 0
        self.assertFalse(self.rp.CompleteHero(1))
        self.assertEqual(self.g.ENGINE["1"].faith, before)
        self.assertEqual(self.state()["discoveries"]["d"]["effects"], "PENDING")
        self.g.ENGINE["0"].heroClaims["10"] = 1
        self.g.HERO_UI_TICK()
        self.assertEqual(self.g.ENGINE["1"].faith, before + 32.5)

    def test_hero_reward_waits_until_recipients_turn(self):
        self.found()
        self.lost()
        self.discovery()
        before = self.g.ENGINE["1"].faith
        self.rp.ResolveDiscovery(R1, "d", "HERO", self.options())
        self.g.ENGINE["1"].active = False
        self.g.HERO_UI_TICK(60)
        self.assertEqual(self.g.ENGINE["1"].faith, before)
        self.g.ENGINE["1"].active = True
        self.g.HERO_UI_TICK()
        self.assertEqual(self.g.ENGINE["1"].faith, before + 32.5)

    def test_hero_timeout_does_not_expire_another_players_inactive_reservation(self):
        self.found()
        self.lost()
        self.discovery(player=0)
        self.found(R2)
        self.lost(R2)
        self.discovery(religion=R2, player=1, item="book2", discovery="d2")
        self.rp.ResolveDiscovery(R1, "d", "HERO", self.options(player=0))
        self.rp.ResolveDiscovery(R2, "d2", "HERO", self.options(player=1))
        before = self.g.ENGINE["1"].faith
        self.g.ENGINE["1"].active = False
        self.g.ENGINE["0"].heroClaims["10"] = -1
        self.g.HERO_UI_TICK(31)
        self.assertEqual(plain(self.rp.Export())["heroes"]["1"]["verification"], "PENDING")
        self.g.ENGINE["1"].active = True
        self.g.HERO_UI_TICK(0)
        self.assertEqual(self.g.ENGINE["1"].faith, before + 32.5)
        self.assertEqual(plain(self.rp.Export())["heroes"]["1"]["verification"], "APPLIED")

    def test_interrupted_hero_reward_cannot_replay_after_load(self):
        self.found()
        self.lost()
        self.discovery()
        before = self.g.ENGINE["1"].faith
        self.rp.ResolveDiscovery(R1, "d", "HERO", self.options())
        self.g.FAIL_EFFECT = True
        with self.assertRaises(LuaError):
            self.rp.CompleteHero(1)
        self.assertEqual(plain(self.rp.Export())["heroes"]["1"]["verification"], "APPLYING")
        self.reload()
        self.g.HERO_UI_TICK()
        self.assertEqual(self.g.ENGINE["1"].faith, before)
        self.rejected(self.rp.CompleteHero, 1)

    def test_invalid_inputs_and_world_overflow_do_not_mutate_saved_state(self):
        self.found()
        for value in (-1, 0.5, float("nan"), float("inf"), 2147483648, "1", True, None):
            with self.subTest(value=value):
                self.rejected(self.rp.SetCommunity, R1, "origin", value, 0, "PLAYER_0")
        self.rejected(self.rp.SetCommunity, R1, "origin", 1, -1, "PLAYER_0")
        self.rejected(self.rp.SetCommunity, R1, "", 1, 0, "PLAYER_0")
        self.rejected(self.rp.SetCommunity, R1, "origin", 1, 0, "")
        self.rejected(self.rp.Found, R1, 0, "origin", 1, 0, "PLAYER_0")
        self.rejected(self.rp.Found, "UNKNOWN", 0, "origin", 1, 0, "PLAYER_0")
        self.rejected(self.rp.SetCommunity, R2, "origin", 1, 0, "PLAYER_0")
        self.rp.SetCommunity(R1, "origin", 2147483647, 0, "PLAYER_0")
        self.rejected(self.rp.SetCommunity, R1, "elsewhere", 1, 0, "PLAYER_1")

    def test_named_rules_and_all_eighty_modifier_definitions_match_official_types(self):
        rules = plain(self.rp.Rules())
        self.assertEqual(rules, {"REVIVAL_FOLLOWERS": 1, "REVIVAL_FAITH_COST": 20,
                                "REVIVAL_COOLDOWN_TURNS": 10, "SELL_GOLD": 500,
                                "RESEARCH_SCIENCE": 10, "RESEARCH_CULTURE": 10, "RESEARCH_EUREKAS": 2,
                                "HERESY_CULTURE": 20, "HERESY_FAITH_COST": 400,
                                "ABSORB_FAITH_COST": 500, "ABSORB_DIVISOR": 3, "ARTIFACT_CULTURE": 30,
                                "HERO_FAITH_MULTIPLIER": 10})
        self.assertEqual(len(self.tables["modifiers"]), 80)
        self.assertEqual({(r["ModifierType"], r["RunOnce"], r["Permanent"])
                          for r in self.tables["modifierTypes"]}, {("MODIFIER_PLAYER_GRANT_YIELD", 1, 1)})
        for religion in self.tables["Religions"]:
            for outcome, yield_name, amount in (("RESEARCH", "SCIENCE", "10"), ("RESEARCH_CULTURE", "CULTURE", "10"),
                                               ("HERESY", "CULTURE", "20"),
                                                 ("ARTIFACT", "CULTURE", "30")):
                self.assertEqual(self.tables["modifiers"][f"JAC_RP_{outcome}_{religion['ID']}"],
                                 {"YieldType": "YIELD_" + yield_name, "Amount": amount, "Scale": "false"})
        exported = self.rp.Rules()
        exported.REVIVAL_FOLLOWERS = 99
        self.assertEqual(self.rp.Rules().REVIVAL_FOLLOWERS, 1)

    def test_revival_requires_faith_city_ownership_and_fixed_size(self):
        self.found()
        self.lost()
        self.discovery()
        self.g.ENGINE["1"].faith = 19
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "REVIVE", self.options())
        self.g.ENGINE["1"].faith = 20
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "REVIVE", self.options(city=99))
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "REVIVE", self.options(player=0))
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "REVIVE", self.options(followers=99))
        self.rp.ResolveDiscovery(R1, "d", "REVIVE", self.options())
        self.assertEqual(self.g.ENGINE["1"].faith, 0)
        self.assertEqual(self.state()["communities"]["new"], {"followers": 1, "publicFacilities": 0, "holder": "PLAYER_1"})

    def test_conceal_has_no_yield_and_keeps_material_after_later_paid_revivals(self):
        self.found()
        self.lost()
        self.discovery()
        engine = plain(self.g.ENGINE)
        self.rp.ResolveDiscovery(R1, "d", "CONCEAL", self.options())
        self.assertEqual(plain(self.g.ENGINE), engine)
        self.assertEqual(self.state()["assets"]["book"]["holder"], "HIDDEN_PLAYER_1")
        self.assertTrue(self.state()["canRevive"])
        self.reload()
        self.rp.Revive(R1, "r", "book", "new", 1, 0)
        self.rp.SetCommunity(R1, "new", 0, 0, "PLAYER_1")
        self.assertTrue(self.state()["canRevive"])
        self.g.TURN += 10
        self.rp.Revive(R1, "r2", "book", "new2", 1, 0)
        self.assertEqual(self.g.ENGINE["1"].faith, 460)

    def test_reused_material_removed_ids_and_moved_discoveries_cannot_pay_rewards(self):
        self.found()
        self.lost()
        self.discovery()
        self.rp.MoveAsset(R1, "book", "CAVE", 1)
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "SELL", self.options())
        self.rejected(self.rp.Discover, R1, "d2", "book", "SCRIPTURE", "ACTOR", "THIEF", "CAVE", 1)
        self.rp.MoveAsset(R1, "book", "MONK_1", 1)
        self.rp.ResolveDiscovery(R1, "d", "SELL", self.options())
        self.rp.RemoveAsset(R1, "book")
        self.rejected(self.rp.PreserveAsset, R1, "book", "SCRIPTURE", "CAVE")
        self.rejected(self.rp.Discover, R1, "d3", "book", "SCRIPTURE", "ACTOR", "THIEF", "CAVE", 1)
        self.reload()
        self.assertEqual(self.g.ENGINE["1"].gold, 600)

    def test_non_revival_limit_is_per_player_religion_and_branch(self):
        self.found()
        self.lost()
        self.discovery()
        self.rp.ResolveDiscovery(R1, "d", "SELL", self.options())
        self.discovery(discovery="d2", item="book2")
        self.rejected(self.rp.ResolveDiscovery, R1, "d2", "SELL", self.options())
        self.rp.ResolveDiscovery(R1, "d2", "RESEARCH", self.options())
        self.discovery(discovery="d3", item="book3", player=0)
        self.rp.ResolveDiscovery(R1, "d3", "SELL", self.options())
        self.found(R2)
        self.lost(R2)
        self.discovery(religion=R2, discovery="other", item="other")
        self.rp.ResolveDiscovery(R2, "other", "SELL", self.options())
        self.assertEqual((self.g.ENGINE["0"].gold, self.g.ENGINE["1"].gold, self.g.ENGINE["1"].science), (600, 1100, 10))

    def test_absorption_requires_own_living_target_and_locks_the_transferred_source(self):
        self.found()
        self.lost()
        self.discovery()
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "ABSORB", self.options(targetReligion=R1))
        self.found(R2)
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "ABSORB", self.options())
        self.rp.SetCommunity(R2, "origin", 3, 1, "PLAYER_1")
        self.bind_absorption()
        self.rp.ResolveDiscovery(R1, "d", "ABSORB", self.options())
        self.assertEqual(self.rp.NativeFollowers(R1), 4)
        self.assertEqual(self.rp.NativeFollowers(R2), 7)
        self.assertEqual(self.g.ENGINE["1"].faith, 0)
        self.lost(R2)
        self.assertFalse(self.state(R2)["canRevive"])
        self.rejected(self.rp.Revive, R2, "newrevival", "book", "new", 1, 0)
        self.reload()
        self.assertEqual(self.state(R2)["materialUses"]["book"]["outcome"], "ABSORB")

    def test_diplomatic_restitution_requires_foreign_founder_contact_and_peace(self):
        self.found()
        self.lost()
        self.discovery()
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "DIPLOMATIC_INCIDENT", self.options(targetPlayer=1))
        self.g.ENGINE["1"].met = False
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "DIPLOMATIC_INCIDENT", self.options())
        self.g.ENGINE["1"].met, self.g.ENGINE["1"].war = True, True
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "DIPLOMATIC_INCIDENT", self.options())
        self.g.ENGINE["1"].war, self.g.ENGINE["0"].alive = False, False
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "DIPLOMATIC_INCIDENT", self.options())
        self.g.ENGINE["0"].alive = True
        self.rp.ResolveDiscovery(R1, "d", "DIPLOMATIC_INCIDENT", self.options())
        self.assertEqual((self.g.ENGINE["1"].favor, self.state()["assets"]["book"]["holder"]), (15, "PLAYER_0"))

    def test_vault_and_hero_limits_apply_across_religions_after_load(self):
        for outcome in ("ARTIFACT", "HERO"):
            with self.subTest(outcome=outcome):
                self.load()
                for religion in (R1, R2):
                    self.found(religion)
                    self.lost(religion)
                    self.discovery(religion=religion)
                self.rp.ResolveDiscovery(R1, "d", outcome, self.options())
                self.reload()
                self.rejected(self.rp.ResolveDiscovery, R2, "d", outcome, self.options())

    def test_engine_refusal_and_missing_spawn_leave_pending_and_block_replay(self):
        for failure, outcome in (("FAIL_EFFECT", "SELL"), ("FAIL_SPAWN", "HERO")):
            with self.subTest(failure=failure):
                self.load()
                self.found()
                self.lost()
                self.discovery()
                setattr(self.g, failure, True)
                with self.assertRaises(LuaError):
                    self.rp.ResolveDiscovery(R1, "d", outcome, self.options())
                self.assertEqual(self.state()["discoveries"]["d"]["effects"], "PENDING")
                if failure == "FAIL_SPAWN":
                    self.assertEqual(self.g.ENGINE["1"].faith, 500)
                    self.assertEqual(plain(self.g.ENGINE["1"].units), {})
                self.reload()
                self.rejected(self.rp.ResolveDiscovery, R1, "d", outcome, self.options())

    def test_reservation_write_failure_has_no_engine_effect_and_receipt_failure_cannot_replay(self):
        for offset in (1, 2):
            with self.subTest(offset=offset):
                self.load()
                self.found()
                self.lost()
                self.discovery()
                self.g.FAIL_WRITE_AT = self.g.WRITES + offset
                with self.assertRaises(LuaError):
                    self.rp.ResolveDiscovery(R1, "d", "SELL", self.options())
                self.assertEqual(self.g.ENGINE["1"].gold, 100 if offset == 1 else 600)
                if offset == 1:
                    self.assertNotIn("outcome", self.state()["discoveries"]["d"])
                else:
                    self.assertEqual(self.state()["discoveries"]["d"]["effects"], "PENDING")
                    self.reload()
                    self.rejected(self.rp.ResolveDiscovery, R1, "d", "SELL", self.options())

    def test_legacy_v1_choices_never_pay_on_load_or_unlock_used_material(self):
        self.found()
        self.lost()
        self.discovery()
        old = plain(self.rp.Export())
        del old["heroes"], old["artifacts"]
        religion = old["religions"][R1]
        del religion["materialUses"], religion["outcomeUses"], religion["retiredAssets"]
        religion["discoveries"]["d"].update(outcome="SELL", effects="DEFERRED", resolvedTurn=17)
        self.load(old)
        self.assertEqual(self.g.WRITES, 0)
        self.assertEqual(self.g.ENGINE["1"].gold, 100)
        self.assertFalse(self.state()["canRevive"])
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "SELL", self.options())

    def test_ai_prefers_each_job_branch_and_uses_legal_absorption_or_conceal_fallback(self):
        preferences = {"MONK": "REVIVE", "SCHOLAR": "RESEARCH", "THIEF": "SELL", "PERFORMER": "HERESY",
                       "EXPLORER": "ARTIFACT", "KABUKIMONO": "HERO", "TRAVELER": "DIPLOMATIC_INCIDENT"}
        for job, expected in preferences.items():
            with self.subTest(job=job):
                self.load()
                self.found()
                self.lost()
                self.discovery(job=job)
                self.assertEqual(self.rp.ChooseOutcome(R1, "d", self.options()), expected)
                self.assertEqual(self.rp.ResolveForAI(R1, "d", self.options()), expected)
        self.load()
        self.found()
        self.lost()
        self.discovery(job="THIEF")
        self.rp.ResolveDiscovery(R1, "d", "SELL", self.options())
        self.discovery(job="THIEF", item="book2", discovery="d2")
        self.discovery(job="MONK", item="book3", discovery="d3")
        self.found(R2)
        self.rp.SetCommunity(R2, "origin", 1, 0, "PLAYER_1")
        self.bind_absorption()
        self.assertEqual(self.rp.ResolveForAI(R1, "d2", self.options()), "ABSORB")
        self.g.ENGINE["1"].faith, self.g.ENGINE["1"].met, self.g.ENGINE["1"].city = 0, False, False
        self.assertEqual(self.rp.ResolveForAI(R1, "d3", self.options()), "CONCEAL")

    def test_engine_write_failure_and_export_edits_leave_original_untouched(self):
        self.found()
        saved = self.rp.Snapshot()
        exported = self.rp.Export()
        exported.religions[R1].communities.origin.followers = 99
        inspected = self.rp.Inspect(R1)
        inspected.assets["injected"] = self.lua.table_from({"kind": "RELIC", "holder": "CAVE"})
        self.assertEqual(self.rp.Snapshot(), saved)
        self.g.FAIL_WRITE = True
        self.rejected(self.rp.SetCommunity, R1, "origin", 0, 0, "PLAYER_0")
        self.g.FAIL_WRITE = False
        self.rp.SetCommunity(R1, "origin", 1, 0, "PLAYER_0")
        self.assertEqual(self.state()["followers"], 1)

    def test_twenty_religions_do_not_leak_state_across_save_load(self):
        for n in range(1, 21):
            self.found(f"JAC_RP_PLACEHOLDER_{n:02d}", followers=n, founder=n)
        self.rp.PreserveAsset(R1, "book", "SCRIPTURE", "CAVE")
        self.lost()
        self.rp.NotifyFounderDefeated(1)
        self.reload()
        self.assertEqual(len(plain(self.rp.Export())["religions"]), 20)
        self.assertTrue(self.state()["lost"])
        self.assertEqual((self.state(R2)["followers"], self.state(R2)["state"]), (2, "PUBLIC"))
        self.assertEqual(self.state(R2)["assets"], {})

    def test_switching_save_in_same_vm_does_not_use_previous_game_cache(self):
        self.found()
        first_save = plain(self.rp.Export())
        self.found(R2)
        self.g.PROPERTY_STORE[KEY] = self.lua.table_from(first_save, recursive=True)
        self.assertEqual(set(plain(self.rp.Export())["religions"]), {R1})
        self.g.PROPERTY_STORE[KEY] = None
        self.assertEqual(plain(self.rp.Export())["religions"], {})

    def test_future_or_damaged_save_fails_without_overwrite(self):
        for saved in ({"schema": 2, "revision": 8, "religions": {}},
                      {"schema": 1, "revision": 8, "religions": "bad"}, "bad"):
            with self.subTest(saved=saved):
                with self.assertRaises(LuaError):
                    self.load(saved)
                self.assertEqual(self.lua.globals().WRITES, 0)
                self.assertEqual(plain(self.lua.globals().PROPERTY_STORE[KEY]), saved)

    def test_live_scenario_gameplay_and_external_captures_in_fresh_vms(self):
        scenario = json.loads((ROOT / "tools/civ6-autotest/scenarios/religion-persistence.json")
                              .read_text(encoding="utf-8"))
        captures, saves = {}, {}
        compared, loaded, native_founded = 0, 0, 0
        for step in scenario["steps"]:
            if step.get("code", "").startswith("-- Keep hero verification pending"):
                self.lua.execute(step["code"])
            elif step["action"] == "ui_lua" and step["state_name"] == "GameCore_Tuner":
                value = self.lua.execute(step["code"])
                self.g.HERO_UI_TICK()
                if value == "native placeholder religion founded":
                    native_founded += 1
                    self.assertEqual(self.g.NATIVE_RELIGION, 42)
                if "equals_capture" in step:
                    self.assertEqual(value, captures[step["equals_capture"]])
                    compared += 1
                if "capture_as" in step:
                    captures[step["capture_as"]] = value
            elif step["action"] == "save":
                saves[step["name"]] = (json.loads(json.dumps(plain(self.rp.Export()))), plain(self.g.ENGINE), self.g.TURN)
            elif step["action"] == "load":
                saved, engine, turn = saves[step["name"]]
                self.load(saved, engine=engine)
                self.g.TURN = turn
                self.g.HERO_UI_TICK()
                loaded += 1
            elif step["action"] in ("autoplay", "end_turn"):
                self.g.TURN += step["turns"]
                self.g.TURN_HOOK()
        self.assertEqual(loaded, 10)
        self.assertEqual(compared, 13)
        self.assertEqual(native_founded, 1)
        self.assertEqual(self.state()["revivalCount"], 1)
        self.assertEqual(self.state()["state"], "PUBLIC")

    def test_all_scenario_lua_chunks_compile_including_ui_and_waits(self):
        scenario = json.loads((ROOT / "tools/civ6-autotest/scenarios/religion-persistence.json")
                              .read_text(encoding="utf-8"))
        chunks = []
        for step in scenario["steps"]:
            if step["action"] == "ui_lua":
                chunks.append(step["code"])
            elif step["action"] == "wait_for" and step["condition"]["type"] == "lua":
                chunks.append(step["condition"]["code"])
        self.assertEqual(len(chunks), 63)
        for code in chunks:
            self.assertTrue(self.lua.execute("local f,e=load(...); if not f then error(e) end; return true", code))

if __name__ == "__main__":
    unittest.main()
