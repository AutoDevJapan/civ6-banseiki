"""Per-house city name pools (documents/poc/city-names.md).

Every JAC house used to copy CIVILIZATION_JAPAN's 33 names. City names are
unique across the whole game, so 22 houses sharing one list ran out and the
engine fell back to other civilizations' names ("BALTIMORE", live 2026-09-30).
"""

from __future__ import annotations

import re
import sys
import unittest
import xml.etree.ElementTree as ET
from collections import Counter, defaultdict
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MOD = ROOT / "poc/JAC_PoC_CustomCivs"
sys.path.insert(0, str(ROOT / "tools/japan_map_scale"))

import write_city_names as city_names  # noqa: E402


LANGUAGES = {"ja_JP", "en_US", "ru_RU"}
MIN_HOUSE_NAMES = 25  # the 200-turn health run reached ~13 cities in one house
INSERT = re.compile(r"INSERT INTO CityNames\s*\(([^)]*)\)\s*(VALUES|SELECT)(.*?);", re.S)
ROW = re.compile(r"\('(CIVILIZATION_[A-Z0-9_]+)',\s*'([A-Z0-9_]+)',\s*(-?\d+)\)")
CIV = re.compile(r"\('(CIVILIZATION_JAC_[A-Z0-9_]+)', 'LOC_CIVILIZATION_JAC_[A-Z0-9_]+_NAME'")
# Base/Assets/Gameplay/Data/Civilizations.xml:3044-3076 (CIVILIZATION_JAPAN).
OFFICIAL_JAPAN = {
    "Kyoto", "Tokyo", "Osaka", "Nagoya", "Fukuoka", "Sendai", "Shizuoka", "Okayama", "Takamatsu",
    "Yokohama", "Gifu", "Nagano", "Takasaki", "Otsu", "Aomori", "Fukushima", "Morioka", "Hakodate",
    "Kobe", "Hamamatsu", "Okazaki", "Fukui", "Yamagata", "Kagoshima", "Hiroshima", "Himeji",
    "Matsumoto", "Ota", "Numazu", "Sapporo", "Nagasaki", "Tottori", "Nagahama",
}


def sql_text() -> str:
    return "\n".join(strip_comments(p.read_text(encoding="utf-8")) for p in sorted((MOD / "Data").glob("*.sql")))


def strip_comments(text: str) -> str:
    return "\n".join(line.split("--", 1)[0] for line in text.splitlines())


def texts() -> dict[str, dict[str, str]]:
    out: dict[str, dict[str, str]] = defaultdict(dict)
    for path in (ROOT / "poc").glob("*/Text/*.xml"):
        for row in ET.parse(path).getroot().iter("Row"):
            out[row.get("Tag")][row.get("Language")] = row.findtext("Text") or ""
    return out


class CityNameTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.sql = sql_text()
        cls.texts = texts()
        cls.rows = defaultdict(list)
        cls.selects = []
        for _, kind, body in INSERT.findall(cls.sql):
            if kind == "SELECT":
                cls.selects.append(body)
            for civ, name, sort in ROW.findall(body):
                cls.rows[civ].append((name, int(sort)))
        cls.civs = set(CIV.findall(cls.sql))
        cls.source = city_names.data()

    def test_twenty_two_houses_have_pools(self):
        self.assertEqual(len(self.civs), 22)
        self.assertEqual({f"CIVILIZATION_JAC_{h}" for h in self.source["houses"]}, self.civs)

    def test_every_house_has_enough_own_names(self):
        for civ in sorted(self.civs):
            own = [n for n, sort in self.rows[civ] if 0 <= sort < city_names.SHARED_SORT_BASE]
            capital = [n for n, sort in self.rows[civ] if sort < 0]
            self.assertGreaterEqual(len(own), MIN_HOUSE_NAMES, civ)
            self.assertEqual(capital, [f"LOC_JAC_1534_CAPITAL_{civ.removeprefix('CIVILIZATION_JAC_')}"])

    def test_no_names_copied_from_other_civilizations(self):
        self.assertEqual(self.selects, [], "CityNames must not be copied from official civilizations")
        for civ in self.civs:
            foreign = [n for n, _ in self.rows[civ] if not n.startswith("LOC_JAC_")]
            self.assertEqual(foreign, [], civ)

    def test_no_duplicates_within_a_house(self):
        for civ in self.civs:
            counts = Counter(n for n, _ in self.rows[civ])
            self.assertEqual([n for n, c in counts.items() if c > 1], [], civ)

    def test_every_name_has_three_languages(self):
        missing = sorted({n for civ in self.civs for n, _ in self.rows[civ]
                          if not LANGUAGES <= set(self.texts.get(n, {}))})
        self.assertEqual(missing, [])

    def test_houses_do_not_share_names_on_screen(self):
        # The engine never reuses a name, so two houses listing the same place
        # would just shrink both pools; different tags with the same text would
        # show two identical banners. Map cities and capitals count as used.
        shown = defaultdict(set)
        tags = {n for civ in self.civs for n, _ in self.rows[civ]}
        tags |= {t for t in self.texts if t.startswith("LOC_JAC_1534_CITY_")}
        for name in tags:
            for language in LANGUAGES:
                shown[(language, self.texts[name][language].lower())].add(name)
        # Oda's capital and Imagawa's map city are both Nagoya on purpose (documents/poc/README.md).
        known = ["LOC_JAC_1534_CAPITAL_ODA", "LOC_JAC_1534_CITY_IMAGAWA_NAGOYA"]
        clashes = {key: sorted(v) for key, v in shown.items() if len(v) > 1 and sorted(v) != known}
        self.assertEqual(clashes, {})
        own = [n for civ in self.civs for n, s in self.rows[civ] if 0 <= s < city_names.SHARED_SORT_BASE]
        self.assertEqual(len(own), len(set(own)), "a place is listed by two houses")

    def test_no_clash_with_official_japan_names(self):
        # Capitals (SortIndex -1) keep the seat names, e.g. Okazaki, Kagoshima.
        ours = {self.texts[n]["en_US"] for civ in self.civs for n, s in self.rows[civ] if s >= 0}
        self.assertEqual(ours & OFFICIAL_JAPAN, set())

    def test_random_depth_is_set_for_all_houses(self):
        match = re.search(r"UPDATE Civilizations SET RandomCityNameDepth = (\d+) WHERE CivilizationType IN \(([^)]*)\)", self.sql)
        self.assertIsNotNone(match)
        self.assertEqual(int(match.group(1)), city_names.RANDOM_DEPTH)
        self.assertEqual(set(re.findall(r"'([A-Z_]+)'", match.group(2))), self.civs)

    def test_city_names_load_after_both_civilization_files(self):
        modinfo = (MOD / "JAC_PoC_CustomCivs.modinfo").read_text(encoding="utf-8")
        files = re.findall(r"<File>(Data/[^<]+\.sql)</File>", modinfo.split("</UpdateDatabase>")[0])
        self.assertEqual(files[-1], "Data/JAC_CityNames_Gameplay.sql")
        self.assertIn("Text/JAC_CityNames_Text.xml", modinfo.split("</UpdateText>")[0])

    def test_generated_files_are_current(self):
        self.assertEqual((MOD / "Data/JAC_CityNames_Gameplay.sql").read_text(encoding="utf-8"),
                         city_names.gameplay_sql(self.source))
        rows = {(r.get("Tag"), r.get("Language")): r.findtext("Text")
                for r in city_names.text_xml(self.source).iter("Row")}
        on_disk = {(r.get("Tag"), r.get("Language")): r.findtext("Text")
                   for r in ET.parse(MOD / "Text/JAC_CityNames_Text.xml").getroot().iter("Row")}
        self.assertEqual(on_disk, rows)

    def test_every_entry_cites_a_source(self):
        entries = [e for pool in self.source["houses"].values() for e in pool] + self.source["shared"]
        for entry in entries:
            self.assertTrue(entry["source"].startswith("https://ja.wikipedia.org/wiki/"), entry)
            self.assertTrue(entry["kana"] and entry["province"] and entry["kind"], entry)

    def test_romanization_matches_map_conventions(self):
        cases = {
            "ほうじょう": ("Hojo", "Ходзё"), "すんぷ": ("Sunpu", "Сумпу"), "ひらい": ("Hirai", "Хираи"),
            "びっちゅう まつやま": ("Bitchu Matsuyama", "Биттю-Мацуяма"), "えいへいじ": ("Eiheiji", "Эйхэйдзи"),
            "きゃん": ("Kyan", "Кян"), "ユウフツ": ("Yufutsu", "Юфуцу"), "とおとうみ": ("Totomi", "Тотоми"),
        }
        for kana, expected in cases.items():
            self.assertEqual(city_names.romanize(kana), expected)


if __name__ == "__main__":
    unittest.main()
