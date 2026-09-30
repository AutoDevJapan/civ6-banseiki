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
TABLES = ("Religions", "AssetKinds", "Jobs", "Sites", "Outcomes")

STUBS = r"""
local tables, initial = ...
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
Game = {
  GetProperty = function(_, k) return PROPERTY_STORE[k] end,
  SetProperty = function(_, k, v)
    if FAIL_WRITE then error('engine refused write') end
    if v == '' then v = nil end
    PROPERTY_STORE[k] = clone(v)
    WRITES = WRITES + 1
  end,
  GetCurrentGameTurn = function() return TURN end,
}
ExposedMembers = {}
-- Native founding is a scenario fixture only. These stubs check its call path;
-- whether the actual engine permits it remains a live-scenario assertion.
NATIVE_RELIGION = -1
local capital, holy = {}, nil
local playerReligion = { SetHolyCity = function(_, city) holy = city end }
Players = { [0] = {
  GetCities = function() return { GetCapitalCity = function() return capital end } end,
  GetReligion = function() return playerReligion end,
} }
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
        db.executescript((MOD / "Data/JAC_ReligionPersistence.sql").read_text(encoding="utf-8"))
        db.row_factory = sqlite3.Row
        return {name: [dict(r) for r in db.execute(f"SELECT * FROM JAC_RP_{name}")]
                for name in TABLES}

def plain(value):
    if hasattr(value, "items"):
        return {k: plain(v) for k, v in value.items()}
    return value

class ReligionPersistenceTests(unittest.TestCase):
    def setUp(self):
        self.tables = catalog()
        self.load()

    def load(self, saved=None, tables=None):
        self.lua = LuaRuntime(unpack_returned_tuples=True)
        initial = self.lua.table_from(saved, recursive=True) if isinstance(saved, dict) else saved
        self.lua.execute(STUBS, self.lua.table_from(tables or self.tables, recursive=True), initial)
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
        saved = json.loads(json.dumps(plain(self.rp.Export()), ensure_ascii=False))
        self.load(saved)
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
        tags = {r["Name"] for t in self.tables.values() for r in t}
        tags |= {"LOC_JAC_RP_" + x for x in ("PUBLIC", "UNDERGROUND", "LOST", "DEFERRED",
                                            "FOLLOWERS", "FACILITIES", "FOUNDER_DEFEATED", "CAN_REVIVE")}
        self.assertEqual(len(tags), 54)
        self.assertEqual(set(texts), {(tag, lang) for tag in tags
                                     for lang in ("ja_JP", "en_US", "ru_RU")})
        for value in texts.values():
            self.assertTrue(value and not value.startswith("LOC_"))
            self.assertNotIn("\ufffd", value)
        manifest = ET.parse(MOD / "JAC_PoC_ReligionPersistence.modinfo").getroot()
        for row in manifest.findall("./LocalizedText/Text"):
            self.assertEqual({x.tag for x in row}, {"ja_JP", "en_US", "ru_RU"})

    def test_fresh_game_and_script_reload_do_not_write_or_found_anything(self):
        self.assertEqual(plain(self.rp.Export()), {"schema": 1, "revision": 0, "religions": {}})
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
                self.rp.PreserveAsset(R1, "source", kind, "EXILE_1")
                self.lost()
                self.reload()
                self.rp.Revive(R1, "revival", "source", "new", 2, 0, "PLAYER_1")
                self.assertEqual((self.state()["state"], self.state()["followers"]), ("UNDERGROUND", 2))
                self.assertEqual(self.state()["revivalCount"], 1)
                self.assertEqual(self.state()["assets"]["source"]["kind"], kind)
                self.reload()

    def test_holy_site_alone_and_no_source_cannot_revive(self):
        self.found()
        self.lost()
        self.rejected(self.rp.Revive, R1, "r", "absent", "new", 1, 0, "PLAYER_1")
        self.rp.PreserveAsset(R1, "site", "HOLY_SITE", "RUINS")
        self.assertFalse(self.state()["canRevive"])
        self.rejected(self.rp.Revive, R1, "r", "site", "new", 1, 0, "PLAYER_1")
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
        self.rp.PreserveAsset(R1, "book", "SCRIPTURE", "CAVE")
        self.rejected(self.rp.Revive, R1, "r", "book", "new", 1, 0, "PLAYER_1")
        self.lost()
        self.rejected(self.rp.Revive, R1, "r", "book", "new", 0, 0, "PLAYER_1")
        self.rp.Revive(R1, "r", "book", "new", 1, 1, "PLAYER_1")
        self.assertEqual(self.state()["state"], "PUBLIC")
        self.rejected(self.rp.Revive, R1, "r", "book", "new", 1, 1, "PLAYER_1")
        self.rp.SetCommunity(R1, "new", 0, 0, "PLAYER_1")
        self.rejected(self.rp.Revive, R1, "r", "book", "new", 1, 1, "PLAYER_1")
        self.rp.Revive(R1, "r2", "book", "new", 2, 0, "PLAYER_1")
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
                    self.rp.Discover(R1, f"d{n}", f"item{n}", kind, "ACTOR_1", job["ID"], site["ID"])
        self.assertEqual(n, 70)
        self.assertEqual(len(self.state()["discoveries"]), 70)
        self.assertEqual(len(self.state()["assets"]), 70)
        self.reload()

    def test_discovery_transfers_existing_item_and_rejects_wrong_site_and_replays(self):
        self.found()
        self.rp.PreserveAsset(R1, "book", "SCRIPTURE", "CAVE")
        self.rejected(self.rp.Discover, R1, "d", "book", "SCRIPTURE", "MONK_1", "MONK", "CAVE")
        self.lost()
        self.rejected(self.rp.Discover, R1, "d", "book", "SCRIPTURE", "MONK_1", "MONK", "RUINS")
        self.rejected(self.rp.Discover, R1, "d", "new", "RECORD", "MONK_1", "MONK", "CAVE")
        self.rejected(self.rp.Discover, R1, "d", "new", "SCRIPTURE", "MONK_1", "UNKNOWN", "CAVE")
        self.rejected(self.rp.Discover, R1, "d", "new", "SCRIPTURE", "MONK_1", "MONK", "UNKNOWN")
        self.rp.Discover(R1, "d", "book", "SCRIPTURE", "MONK_1", "MONK", "CAVE")
        self.assertEqual(self.state()["assets"]["book"]["holder"], "MONK_1")
        self.rejected(self.rp.Discover, R1, "d", "book", "SCRIPTURE", "MONK_1", "MONK", "CAVE")
        self.reload()
    def test_every_outcome_is_saved_and_only_revival_applies_a_defined_effect(self):
        outcomes = [r["ID"] for r in self.tables["Outcomes"]]
        self.assertEqual(len(outcomes), 9)
        for outcome in outcomes:
            with self.subTest(outcome=outcome):
                self.load()
                self.found()
                self.lost()
                self.rp.Discover(R1, "d", "book", "SCRIPTURE", "MONK_1", "MONK", "CAVE")
                self.rp.ResolveDiscovery(R1, "d", outcome, "r", "new", 2, 0, "PLAYER_1")
                d = self.state()["discoveries"]["d"]
                self.assertEqual(d["outcome"], outcome)
                self.assertEqual(d["effects"], "APPLIED" if outcome == "REVIVE" else "DEFERRED")
                self.assertEqual(self.state()["followers"], 2 if outcome == "REVIVE" else 0)
                self.assertEqual(self.state()["revivalCount"], 1 if outcome == "REVIVE" else 0)
                self.reload()
                self.rejected(self.rp.ResolveDiscovery, R1, "d", outcome, "r", "new", 2, 0, "PLAYER_1")

    def test_removed_discovery_source_prevents_revival_without_marking_resolution(self):
        self.found()
        self.lost()
        self.rp.Discover(R1, "d", "book", "SCRIPTURE", "MONK_1", "MONK", "CAVE")
        self.rp.RemoveAsset(R1, "book")
        self.rejected(self.rp.ResolveDiscovery, R1, "d", "REVIVE", "r", "new", 1, 0, "PLAYER_1")
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
                saves[step["name"]] = json.loads(json.dumps(plain(self.rp.Export())))
            elif step["action"] == "load":
                self.load(saves[step["name"]])
                loaded += 1
            elif step["action"] == "autoplay":
                self.g.TURN += step["turns"]
        self.assertEqual(loaded, 6)
        self.assertEqual(compared, 7)
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
        self.assertEqual(len(chunks), 24)
        for code in chunks:
            self.assertTrue(self.lua.execute("local f,e=load(...); if not f then error(e) end; return true", code))

if __name__ == "__main__":
    unittest.main()
