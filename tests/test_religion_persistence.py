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
FAIL_EFFECT, FAIL_SPAWN = false, false
UnitManager = { InitUnitValidAdjacentHex = function(id, kind, x, y, radius)
  assert(kind == 'UNIT_SCOUT' and x == 7 and y == 8 and radius == 1)
  if FAIL_SPAWN then return end
  local e = ENGINE[tostring(id)]
  e.units[tostring(e.nextUnit)] = { id = e.nextUnit, kind = 5 }
  e.nextUnit = e.nextUnit + 1
  -- Deliberately returns nil, matching the verified engine behavior.
end }
-- Native founding is a scenario fixture only. These stubs check its call path;
-- whether the actual engine permits it remains a live-scenario assertion.
NATIVE_RELIGION = -1
local capital, holy = { GetID = function() return 0 end,
  GetX = function() return 7 end, GetY = function() return 8 end }, nil
Players = {}
for id = 0, 1 do
  local pid = id
  local e = ENGINE[tostring(pid)] or { faith = 500, gold = 100, science = 0, culture = 0,
    favor = 0, units = {}, nextUnit = 1, alive = true, major = true, met = true, war = false, city = true }
  ENGINE[tostring(pid)] = e
  local religion = { SetHolyCity = function(_, city) holy = city end,
    GetFaithBalance = function() return e.faith end,
    ChangeFaithBalance = function(_, amount) if FAIL_EFFECT then error('effect refused') end; e.faith = e.faith + amount end }
  Players[pid] = {
    GetID = function() return pid end, IsAlive = function() return e.alive end,
    IsMajor = function() return e.major end, IsHuman = function() return pid == 0 end,
    GetCities = function() return { GetCapitalCity = function() return e.city and capital or nil end,
      FindID = function(_, city) return e.city and city == 0 and capital or nil end } end,
    GetReligion = function() return religion end,
    GetTechs = function() return { SetTech = function() end } end,
    GetCulture = function() return { SetCivic = function() end } end,
    GetTreasury = function() return {
      GetGoldBalance = function() return e.gold end,
      ChangeGoldBalance = function(_, amount) if FAIL_EFFECT then error('effect refused') end; e.gold = e.gold + amount end } end,
    GetDiplomacy = function() return { HasMet = function() return e.met end,
      SetHasMet = function() e.met = true end, MakePeaceWith = function() e.war = false end,
      IsAtWarWith = function() return e.war end, GetFavor = function() return e.favor end,
      ChangeFavor = function(_, amount) if FAIL_EFFECT then error('effect refused') end; e.favor = e.favor + amount end } end,
    GetUnits = function() return {
      FindID = function(_, unit) local u = e.units[tostring(unit)]; return u and {
        GetID = function() return u.id end, GetType = function() return u.kind end } end,
      Members = function() local key; return function()
        key = next(e.units, key); if key == nil then return end
        local u = e.units[key]; return u.id, {
          GetID = function() return u.id end, GetType = function() return u.kind end }
      end end } end,
    AttachModifierByID = function(_, modifier)
      if FAIL_EFFECT then error('effect refused') end
      local args = assert(modifiers[modifier])
      local yield = string.lower(args.YieldType:sub(7))
      e[yield] = e[yield] + tonumber(args.Amount)
    end,
  }
end
GameInfo.Modifiers = modifiers
GameInfo.Units = { UNIT_SCOUT = { Index = 5 } }
GameInfo.Technologies = function() return function() return nil end end
GameInfo.Civics = function() return function() return nil end end
GameInfo.Religions = { RELIGION_CUSTOM_1 = { Index = 42 } }
GameInfo.Beliefs = { BELIEF_CATHEDRAL = { Index = 9 } }
local native = {
  FoundReligion = function(_, player, religion)
    assert(player == 0 and holy == capital)
    NATIVE_RELIGION = religion
  end,
  AddBelief = function(_, player, belief) assert(player == 0 and belief == 9) end,
  GetReligions = function() return { { Religion = NATIVE_RELIGION } } end,
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
        return {k: plain(v) for k, v in value.items()}
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
        self.assertIsNone(manifest.find("./InGameActions/AddUserInterfaces"))
        self.assertEqual(manifest.findtext("./ActionCriteria/Criteria/RuleSetInUse"),
                         "RULESET_EXPANSION_2,RULESET_JAC_BANSEIKI")
        self.assertLess(int(manifest.findtext("./InGameActions/UpdateDatabase/Properties/LoadOrder")),
                        int(manifest.findtext("./InGameActions/AddGameplayScripts/Properties/LoadOrder")))

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
        self.assertEqual(plain(self.rp.Export()), {"schema": 1, "revision": 0, "religions": {}, "heroes": {}, "artifacts": {}})
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
                self.assertEqual((self.state()["state"], self.state()["followers"]), ("UNDERGROUND", 2))
                self.assertEqual(self.state()["revivalCount"], 1)
                self.assertEqual(self.state()["assets"]["source"]["kind"], kind)
                self.assertEqual(self.g.ENGINE["1"].faith, 440)
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
        self.rp.PreserveAsset(R1, "book2", "SCRIPTURE", "PLAYER_1", 1)
        self.rejected(self.rp.Revive, R1, "r2", "book2", "new2", 1, 0)
        self.g.TURN += 10
        self.rp.Revive(R1, "r2", "book2", "new2", 1, 0)
        self.assertEqual(self.state()["revivalCount"], 2)
        self.reload()
        self.assertEqual(set(self.state()["revivals"]), {"r", "r2"})

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

    def test_every_outcome_applies_the_exact_reward_and_survives_load_without_replay(self):
        outcomes = [r["ID"] for r in self.tables["Outcomes"]]
        self.assertEqual(set(outcomes), {"REVIVE", "SELL", "CONCEAL", "RESEARCH", "HERESY",
                                        "ABSORB", "ARTIFACT", "DIPLOMATIC_INCIDENT", "HERO"})
        rewards = {"SELL": {"gold": 40}, "RESEARCH": {"science": 20}, "HERESY": {"culture": 20},
                   "ABSORB": {"faith": 20}, "ARTIFACT": {"culture": 30},
                   "DIPLOMATIC_INCIDENT": {"favor": 10}, "REVIVE": {"faith": -60}, "HERO": {"faith": -60}}
        for outcome in outcomes:
            with self.subTest(outcome=outcome):
                self.load()
                self.found()
                self.lost()
                self.found(R2)
                self.rp.SetCommunity(R2, "origin", 3, 1, "PLAYER_1")
                self.discovery()
                before = plain(self.g.ENGINE["1"])
                self.rp.ResolveDiscovery(R1, "d", outcome, self.options())
                d = self.state()["discoveries"]["d"]
                self.assertEqual(d["outcome"], outcome)
                self.assertEqual(d["effects"], "APPLIED")
                self.assertEqual(self.state()["followers"], 2 if outcome == "REVIVE" else 0)
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
                    self.assertEqual(self.state()["assets"]["book"]["interpretation"], "HERETICAL")
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

    def test_named_rules_and_all_sixty_modifier_definitions_match_official_types(self):
        rules = plain(self.rp.Rules())
        self.assertEqual(rules, {"REVIVAL_FOLLOWERS": 2, "REVIVAL_FAITH_COST": 60,
                                "REVIVAL_COOLDOWN_TURNS": 10, "SELL_GOLD": 40,
                                "RESEARCH_SCIENCE": 20, "HERESY_CULTURE": 20,
                                "ABSORB_FAITH": 20, "ARTIFACT_CULTURE": 30,
                                "INCIDENT_FAVOR": 10, "HERO_FAITH_COST": 60})
        self.assertEqual(len(self.tables["modifiers"]), 60)
        self.assertEqual({(r["ModifierType"], r["RunOnce"], r["Permanent"])
                          for r in self.tables["modifierTypes"]}, {("MODIFIER_PLAYER_GRANT_YIELD", 1, 1)})
        for religion in self.tables["Religions"]:
            for outcome, yield_name, amount in (("RESEARCH", "SCIENCE", "20"), ("HERESY", "CULTURE", "20"),
                                                 ("ARTIFACT", "CULTURE", "30")):
                self.assertEqual(self.tables["modifiers"][f"JAC_RP_{outcome}_{religion['ID']}"],
                                 {"YieldType": "YIELD_" + yield_name, "Amount": amount, "Scale": "false"})
        exported = self.rp.Rules()
        exported.REVIVAL_FOLLOWERS = 99
        self.assertEqual(self.rp.Rules().REVIVAL_FOLLOWERS, 2)

    def test_revival_requires_faith_city_ownership_and_fixed_size(self):
        self.found()
        self.lost()
        self.discovery()
        self.g.ENGINE["1"].faith = 59
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "REVIVE", self.options())
        self.g.ENGINE["1"].faith = 60
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "REVIVE", self.options(city=99))
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "REVIVE", self.options(player=0))
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "REVIVE", self.options(followers=99))
        self.rp.ResolveDiscovery(R1, "d", "REVIVE", self.options())
        self.assertEqual(self.g.ENGINE["1"].faith, 0)
        self.assertEqual(self.state()["communities"]["new"], {"followers": 2, "publicFacilities": 0, "holder": "PLAYER_1"})

    def test_conceal_has_no_yield_and_allows_exactly_one_later_paid_revival(self):
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
        self.assertFalse(self.state()["canRevive"])
        self.g.TURN += 10
        self.rejected(self.rp.Revive, R1, "r2", "book", "new2", 1, 0)

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
        self.assertEqual(self.g.ENGINE["1"].gold, 140)

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
        self.assertEqual((self.g.ENGINE["0"].gold, self.g.ENGINE["1"].gold, self.g.ENGINE["1"].science), (140, 180, 20))

    def test_absorption_requires_own_living_target_and_locks_the_transferred_source(self):
        self.found()
        self.lost()
        self.discovery()
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "ABSORB", self.options(targetReligion=R1))
        self.found(R2)
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "ABSORB", self.options())
        self.rp.SetCommunity(R2, "origin", 3, 1, "PLAYER_1")
        self.rp.ResolveDiscovery(R1, "d", "ABSORB", self.options())
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
        self.assertEqual((self.g.ENGINE["1"].favor, self.state()["assets"]["book"]["holder"]), (10, "PLAYER_0"))

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
                self.assertEqual(self.g.ENGINE["1"].gold, 100 if offset == 1 else 140)
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
        self.found(R2)
        self.rp.SetCommunity(R2, "origin", 1, 0, "PLAYER_1")
        self.assertEqual(self.rp.ResolveForAI(R1, "d2", self.options()), "ABSORB")
        self.discovery(job="MONK", item="book3", discovery="d3")
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
            if step["action"] == "ui_lua" and step["state_name"] == "GameCore_Tuner":
                value = self.lua.execute(step["code"])
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
                loaded += 1
            elif step["action"] in ("autoplay", "end_turn"):
                self.g.TURN += step["turns"]
        self.assertEqual(loaded, 7)
        self.assertEqual(compared, 11)
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
        self.assertEqual(len(chunks), 39)
        for code in chunks:
            self.assertTrue(self.lua.execute("local f,e=load(...); if not f then error(e) end; return true", code))

if __name__ == "__main__":
    unittest.main()
