"""Static gates for the historical events PoC (SSOT 3.2) and its generated event list."""

from __future__ import annotations

import importlib.util
import json
import re
import sqlite3
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_PoC_HistoricalEvents"
SITES_LUA = ROOT / "poc/JAC_PoC_JapanMapScale/Scripts/JAC_ResourceSites.lua"
EVENTS_JSON = json.loads((ROOT / "documents/spec/data/sengoku-events.json").read_text(encoding="utf-8"))["events"]
LANGUAGES = {"ja_JP", "en_US", "ru_RU"}

_spec = importlib.util.spec_from_file_location("generate_sengoku_events", ROOT / "tools/generate_sengoku_events.py")
GEN = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(GEN)


def sengoku_types(kind: str) -> list[str]:
    sql = (ROOT / "poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Trees.sql").read_text(encoding="utf-8")
    return re.findall(rf"\('([A-Z_]+)', '{kind}'\)", sql)


def load_db() -> sqlite3.Connection:
    db = sqlite3.connect(":memory:")
    db.execute("PRAGMA foreign_keys = ON")
    db.executescript("""
        CREATE TABLE GlobalParameters (Name TEXT PRIMARY KEY, Value TEXT);
        CREATE TABLE Technologies (TechnologyType TEXT PRIMARY KEY);
        CREATE TABLE Civics (CivicType TEXT PRIMARY KEY);
        CREATE TABLE Boosts (BoostID INTEGER PRIMARY KEY, TechnologyType TEXT, CivicType TEXT, Boost INTEGER NOT NULL,
            TriggerDescription TEXT NOT NULL, TriggerLongDescription TEXT NOT NULL, BoostClass TEXT NOT NULL,
            Unit1Type TEXT, ImprovementType TEXT, ResourceType TEXT);
        CREATE TABLE Modifiers (ModifierId TEXT PRIMARY KEY, ModifierType TEXT NOT NULL);
        CREATE TABLE ModifierArguments (ModifierId TEXT, Name TEXT, Value TEXT, PRIMARY KEY (ModifierId, Name));
    """)
    db.executemany("INSERT INTO Technologies VALUES (?)", [(t,) for t in sengoku_types("KIND_TECH")])
    db.executemany("INSERT INTO Civics VALUES (?)", [(c,) for c in sengoku_types("KIND_CIVIC")])
    # The tree's own boosts load first (SengokuTrees LoadOrder 200 < 300); the events add rows only for the rest.
    trees = (ROOT / "poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Trees.sql").read_text(encoding="utf-8")
    for statement in re.findall(r"^INSERT INTO Boosts .*?;$", trees, re.M):
        db.execute(statement)
    for name in ("JAC_HistoricalEvents_Schema.sql", "JAC_HistoricalEvents_Sengoku.sql"):
        db.executescript((MOD / "Data" / name).read_text(encoding="utf-8"))
    return db


def turn_for_year(year: int, month: int | None = None, speed: str = "GAMESPEED_STANDARD") -> int:
    """Same algorithm as TurnForYear() in Scripts/JAC_HistoricalEvents.lua, on the Sengoku calendar."""
    target, months, turn = (year - calendar()[0]) * 12 + (month or 1) - 1, 0, 1
    for turns, increment in calendar()[1][speed]:
        if months + turns * increment >= target:
            return turn + max(0, -(-(target - months) // increment))
        months, turn = months + turns * increment, turn + turns
    return turn


def calendar() -> tuple[int, dict[str, list[tuple[int, int]]]]:
    root = ET.parse(ROOT / "poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Calendar.xml").getroot()
    stages: dict[str, list[tuple[int, int]]] = {}
    for row in root.findall("./GameSpeed_Turns/Row"):
        stages.setdefault(row.findtext("GameSpeedType"), []).append(
            (int(row.findtext("TurnsPerIncrement")), int(row.findtext("MonthIncrement"))))
    return int(root.findtext("./StartEras/Update/Set/Year")), stages


class HistoricalEventTests(unittest.TestCase):
    def test_generated_files_are_current(self):
        for path, content in GEN.outputs().items():
            with self.subTest(path=path.name):
                self.assertEqual(path.read_text(encoding="utf-8"), content,
                                 "run python tools/generate_sengoku_events.py")

    def test_no_prerequisite_or_boost_is_starting_knowledge(self):
        # Live sengoku-events-list-01: prerequisites every house has at the start (C02, T01) fired events on turn 1.
        start = GEN.starting_knowledge()
        self.assertEqual(len(start), 7)  # 4 techs + 3 civics (documents/poc/sengoku-starting-knowledge.md)
        db = load_db()
        used = {r for (r,) in db.execute("SELECT TechnologyType FROM JAC_HistoricalEventPrereqTechs UNION "
                                         "SELECT CivicType FROM JAC_HistoricalEventPrereqCivics UNION "
                                         "SELECT BoostTech FROM JAC_HistoricalEvents UNION "
                                         "SELECT BoostCivic FROM JAC_HistoricalEvents")}
        self.assertEqual(used & start, set())

    def test_generator_refuses_starting_knowledge_in_the_list(self):
        original = GEN.starting_knowledge
        GEN.starting_knowledge = lambda: {"TECH_JAC_SENGOKU_GRAIN_STORES"}  # Tenbun famine's prerequisite
        try:
            with self.assertRaisesRegex(SystemExit, "EV_TENBUN_FAMINE"):
                GEN.build()
        finally:
            GEN.starting_knowledge = original

    def test_sql_loads_with_valid_references(self):
        db = load_db()
        self.assertEqual(db.execute("PRAGMA foreign_key_check").fetchall(), [])
        self.assertEqual(db.execute("SELECT COUNT(*) FROM JAC_HistoricalEvents").fetchone()[0], len(EVENTS_JSON))
        with self.assertRaises(sqlite3.IntegrityError):  # an event needs a year or a turn
            db.execute("INSERT INTO JAC_HistoricalEvents (EventType, Pack, Name, Description) "
                       "VALUES ('X', 'SENGOKU', 'LOC_X', 'LOC_X')")
        with self.assertRaises(sqlite3.IntegrityError):  # a tech prerequisite must be a tech
            db.execute("INSERT INTO JAC_HistoricalEventPrereqTechs VALUES ('JAC_EVENT_TEPPO', "
                       "'CIVIC_JAC_SENGOKU_DOMAIN_LAW')")

    def test_rows_follow_the_event_list(self):
        db = load_db()
        for e in EVENTS_JSON:
            et = GEN.event_type(e["id"])
            with self.subTest(event=e["id"]):
                year, month, tech, civic, house, advisor = db.execute(
                    "SELECT Year, Month, BoostTech, BoostCivic, House, Advisor FROM JAC_HistoricalEvents "
                    "WHERE EventType = ?", (et,)).fetchone()
                self.assertEqual((year, month), (e["year"], e["month"]))
                boost = e["boost"] or {}
                self.assertEqual(tech or civic, boost.get("id"))  # SSOT 3.2: the boost at the year
                self.assertEqual(house, f"CIVILIZATION_JAC_{e['house']}" if e["house_specific"] else None)
                self.assertEqual(advisor, 1 if e["advisor_popup"] else 0)
                prereqs = {p for (p,) in db.execute(
                    "SELECT TechnologyType FROM JAC_HistoricalEventPrereqTechs WHERE EventType = ?1 UNION ALL "
                    "SELECT CivicType FROM JAC_HistoricalEventPrereqCivics WHERE EventType = ?1", (et,))}
                self.assertEqual(prereqs, {r["id"] for r in e["earlier_by"]})
                targets = {t for (t,) in db.execute(
                    "SELECT CivilizationType FROM JAC_HistoricalEventTargets WHERE EventType = ?", (et,))}
                expected = set() if "ALL" in e["affected_factions"] else {
                    f"CIVILIZATION_JAC_{f}" for f in e["affected_factions"]}
                self.assertEqual(targets, expected)  # no rows: every major faction
        # E5a: 19 events carry the advisor line, 55 a boost (documents/research/sengoku-events.md §2.6).
        self.assertEqual(db.execute("SELECT SUM(Advisor), COUNT(BoostTech) + COUNT(BoostCivic) "
                                    "FROM JAC_HistoricalEvents").fetchone(), (19, 55))

    def test_renames_are_the_ssot_stages(self):
        # SSOT 3.2: every stage by research only (succession civics: C16 for Uesugi, Tokugawa, Hashiba; C24 for
        # Toyotomi; owner decisions 2026-09-30, Q 1a), the event's year boosting that civic.
        db = load_db()
        rows = db.execute("SELECT r.EventType, r.CivilizationType, r.Name, e.Year, e.Month, r.CivicType FROM "
                          "JAC_HistoricalEventRenames r JOIN JAC_HistoricalEvents e USING (EventType) "
                          "ORDER BY e.Year, e.Month").fetchall()
        self.assertEqual(rows, [
            ("JAC_EVENT_NAGAO_TO_UESUGI", "CIVILIZATION_JAC_NAGAO", "LOC_JAC_EVENT_UESUGI_CIV_NAME", 1561, 4,
             "CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS"),
            ("JAC_EVENT_MATSUDAIRA_TOKUGAWA", "CIVILIZATION_JAC_MATSUDAIRA", "LOC_JAC_EVENT_TOKUGAWA_CIV_NAME", 1567, 1,
             "CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS"),
            ("JAC_EVENT_HONNOJI", "CIVILIZATION_JAC_ODA", "LOC_JAC_EVENT_HASHIBA_CIV_NAME", 1582, 6,
             "CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS"),
            ("JAC_EVENT_TOYOTOMI_SURNAME", "CIVILIZATION_JAC_ODA", "LOC_JAC_EVENT_TOYOTOMI_CIV_NAME", 1586, 10,
             "CIVIC_JAC_SENGOKU_TRUCE_ARBITRATION"),
        ])
        self.assertEqual(db.execute("SELECT COUNT(*) FROM JAC_HistoricalEventRenames r JOIN JAC_HistoricalEvents e "
                                    "USING (EventType) WHERE r.CivicType IS NOT e.BoostCivic").fetchone()[0], 0)

    def test_year_to_turn_follows_the_sengoku_calendar(self):
        start, stages = calendar()

        def months_of_turn(speed: str, turn: int) -> int:  # months elapsed since January of the start year
            months, remaining = 0, turn - 1
            for turns, increment in stages[speed]:
                used = min(remaining, turns)
                months, remaining = months + used * increment, remaining - used
            return months

        def year_of_turn(speed: str, turn: int) -> int:  # the calendar's own year for a turn (test_sengoku_calendar)
            return start + months_of_turn(speed, turn) // 12

        self.assertEqual(start, 1534)
        # The Lua sorts a speed's stages by MonthIncrement descending; that equals the engine's rowid order
        # only while every calendar lists stages from the longest turn to the shortest.
        for speed, rows in stages.items():
            with self.subTest(speed=speed):
                increments = [increment for _, increment in rows]
                self.assertEqual(increments, sorted(set(increments), reverse=True))
        standard = [turn_for_year(year) for year in (1534, 1535, 1543, 1550, 1561, 1600)]
        self.assertEqual(standard, [1, 4, 28, 49, 82, 199])  # 3 turns per year, turn 1 = 1534
        for speed in stages:
            for year in range(1534, 1700):
                with self.subTest(speed=speed, year=year):
                    turn = turn_for_year(year, speed=speed)
                    self.assertEqual(year_of_turn(speed, turn), year)  # the first turn of that year
                    self.assertTrue(turn == 1 or year_of_turn(speed, turn - 1) == year - 1)
        # SSOT 3.2 (month, E6b): the due turn is the first turn that reaches the month.
        for speed in stages:
            for month in range(1, 13):
                with self.subTest(speed=speed, month=month):
                    turn = turn_for_year(1543, month, speed)
                    target = 9 * 12 + month - 1
                    self.assertGreaterEqual(months_of_turn(speed, turn), target)
                    self.assertLess(months_of_turn(speed, turn - 1), target)
        db = load_db()
        due = {e: turn_for_year(y, m) for e, y, m in db.execute("SELECT EventType, Year, Month FROM JAC_HistoricalEvents")}
        # historical-events-15km.json: Tenbun famine (1540) due 19, Teppo (1543/9) 30, Nagao -> Uesugi (1561/4) 83.
        self.assertEqual((due["JAC_EVENT_TENBUN_FAMINE"], due["JAC_EVENT_TEPPO"], due["JAC_EVENT_NAGAO_TO_UESUGI"]),
                         (19, 30, 83))
        self.assertTrue(all(1 < turn <= 200 for turn in due.values()), due)
        lua = (MOD / "Scripts/JAC_HistoricalEvents.lua").read_text(encoding="utf-8")
        self.assertIn("(month or 1) - 1", lua)
        self.assertIn("GameInfo.GameSpeed_Turns()", lua)
        self.assertIn("return a.MonthIncrement > b.MonthIncrement", lua)
        self.assertNotIn("TURNS_PER_YEAR", lua)
        # Live: the 1534 calendar starts the engine turn counter at GetStartTurn() (16603), not 1.
        self.assertIn("Game.GetCurrentGameTurn() - GameConfiguration.GetStartTurn() + 1", lua)
        self.assertEqual(len(re.findall(r"Game\.GetCurrentGameTurn\(\)", lua)), 1)  # only inside GameTurn()
        scenario = (ROOT / "tools/civ6-autotest/scenarios/historical-events-15km.json").read_text(encoding="utf-8")
        self.assertNotIn("Game.GetCurrentGameTurn()~=", scenario)  # raw turn compares break at start turn 16603

    def test_resource_events_boost_the_research_of_the_reserved_sites(self):
        lua = (MOD / "Scripts/JAC_HistoricalEvents.lua").read_text(encoding="utf-8")
        # SSOT 3.2: the resources mod alone reveals a site (its year or research); the resource rows are records only.
        self.assertNotIn("RevealSite", lua)
        self.assertNotIn("JAC_HistoricalEventResources", lua)
        self.assertNotIn("ResourceBuilder", lua)  # the resources mod is the only writer of reserved sites
        self.assertNotIn("TransferCity", lua)
        sites: dict[str, list[dict[str, str]]] = {}
        for body in re.findall(r"\{ (id=.*?) \}", SITES_LUA.read_text(encoding="utf-8")):
            fields = {k: v or n for k, v, n in re.findall(r'(\w+)=(?:"([^"]*)"|(\d+))', body)}
            sites.setdefault(fields["id"], []).append(fields)
        db = load_db()
        rows = db.execute("SELECT EventType, SiteId, Action, OwnerTo FROM JAC_HistoricalEventResources "
                          "ORDER BY EventType").fetchall()
        self.assertEqual(rows, [
            ("JAC_EVENT_AIKAWA", "gold-sado-aikawa", "reveal", None),
            ("JAC_EVENT_AIZU_LACQUER", "lacquer-aizu", "reveal", None),
            ("JAC_EVENT_AMAGO_FALL", "iron-izumo-yokota", "ownership", "CIVILIZATION_JAC_MORI"),
            ("JAC_EVENT_HIRADO_PORTUGUESE", "niter-hirado", "trade_access", None),
            ("JAC_EVENT_IKUNO_SILVER", "silver-ikuno", "reveal", None),
            ("JAC_EVENT_MORI_IWAMI", "silver-iwami", "ownership", "CIVILIZATION_JAC_MORI"),
            ("JAC_EVENT_SADO_SUBJUGATION", "gold-sado-nishimikawa", "reveal", None),
        ])
        for event, site_id, action, _ in rows:
            if action == "reveal":
                with self.subTest(site=site_id):
                    self.assertIn(site_id, sites)  # a reserved site of the resources mod
                    # The site's research reveals it; the event boosts that research and has it as a prerequisite.
                    techs = {s.get("tech") for s in sites[site_id]} - {None}
                    prereqs = {t for (t,) in db.execute("SELECT TechnologyType FROM JAC_HistoricalEventPrereqTechs "
                                                        "WHERE EventType = ?", (event,))}
                    self.assertLessEqual(techs, prereqs)
                    boost = db.execute("SELECT BoostTech FROM JAC_HistoricalEvents WHERE EventType = ?",
                                       (event,)).fetchone()[0]
                    if techs:
                        self.assertEqual(techs, {boost})
        # Nishimikawa is reserved on the adopted 15 km map only (SSOT 3.2; 20 km is for comparison).
        self.assertEqual(len(sites["gold-sado-nishimikawa"]), 1)

    def test_missing_prereq_gets_a_boost_not_the_tech(self):
        db = load_db()
        self.assertEqual(db.execute("SELECT COUNT(*) FROM JAC_HistoricalEventModifiers").fetchone()[0], 0)
        boosted = {t or c for t, c in db.execute("SELECT TechnologyType, CivicType FROM Boosts")}
        wanted = {t or c for t, c in db.execute("SELECT BoostTech, BoostCivic FROM JAC_HistoricalEvents "
                                                "WHERE BoostTech IS NOT NULL OR BoostCivic IS NOT NULL")}
        self.assertLessEqual(wanted, boosted)  # TriggerBoost needs a Boosts row for every event boost
        # One row per tech/civic: the tree's rows win, placeholders never trigger by themselves.
        self.assertEqual(db.execute("SELECT COUNT(*) FROM Boosts").fetchone()[0], len(boosted))
        classes = {c for (c,) in db.execute("SELECT BoostClass FROM Boosts WHERE TriggerDescription = "
                                            "'LOC_JAC_EVENT_BOOST_PLACEHOLDER'")}
        self.assertEqual(classes, {"BOOST_TRIGGER_NONE_LATE_GAME_CRITICAL_TECH"})
        self.assertEqual(db.execute("SELECT BoostClass FROM Boosts WHERE TechnologyType = "
                                    "'TECH_JAC_SENGOKU_CUPELLATION'").fetchone()[0],
                         "BOOST_TRIGGER_IMPROVE_SPECIFIC_RESOURCE")
        lua = (MOD / "Scripts/JAC_HistoricalEvents.lua").read_text(encoding="utf-8")
        self.assertIn(":TriggerBoost(index, 1)", lua)
        self.assertIn('reason == "year"', lua)

    def test_every_text_key_has_three_languages(self):
        db = load_db()
        keys = {k for row in db.execute("SELECT Name, Description, GenericName FROM JAC_HistoricalEvents")
                for k in row if k}
        keys |= {row[0] for row in db.execute("SELECT Name FROM JAC_HistoricalEventRenames")}
        keys |= {row[0] for row in db.execute("SELECT TriggerDescription FROM Boosts")
                 if row[0].startswith("LOC_JAC_EVENT_")}
        ui = (MOD / "UI/JAC_HistoricalEventsAdvisor.lua").read_text(encoding="utf-8")
        keys |= set(re.findall(r'"(LOC_JAC_EVENT_\w+)"', ui))
        root = ET.parse(MOD / "Text/JAC_HistoricalEvents_Text.xml").getroot()
        languages: dict[str, set[str]] = {}
        for row in root.iter("Row"):
            self.assertTrue(row.findtext("Text"), row.get("Tag"))
            languages.setdefault(row.get("Tag"), set()).add(row.get("Language"))
        self.assertEqual(set(languages), keys)
        for key in keys:
            self.assertEqual(languages[key], LANGUAGES, key)

    def test_modinfo_lists_files(self):
        text = (MOD / "JAC_PoC_HistoricalEvents.modinfo").read_text(encoding="utf-8")
        root = ET.fromstring(text)
        listed = {f.text for f in root.find("Files")}
        on_disk = {p.relative_to(MOD).as_posix() for p in MOD.rglob("*") if p.is_file() and p.suffix != ".modinfo"}
        self.assertEqual(listed, on_disk)
        used = {f.text for action in root.find("InGameActions") for f in action.findall("File")}
        self.assertEqual(used, on_disk)
        ui = root.find("InGameActions/AddUserInterfaces")
        self.assertEqual((ui.findtext("Properties/Context"), ui.findtext("File")),
                         ("InGame", "UI/JAC_HistoricalEventsAdvisor.xml"))

    def test_lua_reads_only_existing_tables_and_properties(self):
        lua = (MOD / "Scripts/JAC_HistoricalEvents.lua").read_text(encoding="utf-8")
        schema = (MOD / "Data/JAC_HistoricalEvents_Schema.sql").read_text(encoding="utf-8")
        tables = set(re.findall(r"CREATE TABLE (\w+)", schema))
        referenced = {a or b for a, b in re.findall(r'"(JAC_HistoricalEvent\w*)"|GameInfo\.(JAC_\w+)', lua)}
        self.assertEqual(referenced, tables - {"JAC_HistoricalEventResources"})  # records only (SSOT 3.2)
        # The UI name override reads the same Game property the gameplay script writes.
        ui = (ROOT / "poc/JAC_PoC_CustomCivs/UI/JAC_CivNameOverride.lua").read_text(encoding="utf-8")
        self.assertIn('local CIV_NAME_PREFIX = "JAC_CIV_NAME_"', lua)
        self.assertIn('local CIV_NAME_PREFIX = "JAC_CIV_NAME_"', ui)
        # The advisor popup reads the flags the gameplay script writes and raises the official advisor.
        advisor = (MOD / "UI/JAC_HistoricalEventsAdvisor.lua").read_text(encoding="utf-8")
        for prefix in ('FIRED_PREFIX = "JAC_EVENT_FIRED_"', 'BOOSTED_PREFIX = "JAC_EVENT_BOOSTED_"'):
            self.assertIn("local " + prefix, lua)
            self.assertIn("local " + prefix, advisor)
        self.assertIn("LuaEvents.TutorialUIRoot_AdvisorRaise(hmake AdvisorItem {", advisor)
        self.assertIn("Game.GetLocalPlayer()", advisor)
        self.assertNotIn("SetProperty", advisor)  # UI never writes game state

    def test_ages_ledger_scores_existing_events(self):
        ages = (ROOT / "poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Ages.sql").read_text(encoding="utf-8")
        scored = set(re.findall(r"\('(JAC_EVENT_\w+)', \d\)", ages))
        self.assertTrue(scored)
        self.assertLessEqual(scored, {GEN.event_type(e["id"]) for e in EVENTS_JSON})


if __name__ == "__main__":
    unittest.main()
