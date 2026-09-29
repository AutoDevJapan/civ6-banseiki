"""Names shown in game must be LOC tags with en_US/ja_JP/ru_RU rows.

Civ6 shows a non-LOC string as-is, and the English/Russian fonts have no CJK
glyphs: literal Japanese city names in the Japan maps rendered as tofu (live
2026-09-29, English UI). Only ja_JP text rows and <ja_JP> modinfo entries may
contain Japanese.
"""

import re
import sqlite3
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path


POC = Path(__file__).resolve().parents[1] / "poc"
LANGUAGES = {"en_US", "ja_JP", "ru_RU"}
CJK = re.compile("[　-ヿ㐀-鿿豈-﫿＀-￯]")
JAC_TAG = re.compile(r"LOC_JAC_[A-Z0-9_]*[A-Z0-9]\b")


def text_rows():
    for path in POC.rglob("*.xml"):
        for table in ET.parse(path).getroot().iter("LocalizedText"):
            for row in table:
                yield path, row.get("Tag"), row.get("Language"), row.findtext("Text") or row.get("Text") or ""


def map_strings():
    for path in POC.rglob("*.Civ6Map"):
        with sqlite3.connect(path) as db:
            for (table,) in db.execute("SELECT name FROM sqlite_master WHERE type='table'"):
                for row in db.execute(f'SELECT * FROM "{table}"'):
                    yield from ((f"{path.name}:{table}", v) for v in row if isinstance(v, str))


def strip_comment(line):
    return line.split("--", 1)[0]


class LocalizedNamesTests(unittest.TestCase):
    def test_every_jac_tag_has_three_languages(self):
        languages = {}
        for _, tag, language, _ in text_rows():
            languages.setdefault(tag, set()).add(language)
        used = set(languages)
        for path in [*POC.rglob("*.sql"), *POC.rglob("*.lua"), *POC.rglob("*.xml")]:
            used |= set(JAC_TAG.findall(path.read_text(encoding="utf-8")))
        used |= {v for _, v in map_strings() if JAC_TAG.fullmatch(v)}
        missing = {tag: sorted(LANGUAGES - languages.get(tag, set()))
                   for tag in used if not LANGUAGES <= languages.get(tag, set())}
        self.assertEqual(missing, {})

    def test_non_japanese_rows_have_no_cjk(self):
        bad = [(p.name, tag, lang, text) for p, tag, lang, text in text_rows()
               if lang != "ja_JP" and CJK.search(text)]
        self.assertEqual(bad, [])

    def test_map_names_are_loc_tags(self):
        # Cities.Name is what the city banner shows; any CJK here is unlocalized.
        self.assertEqual([(where, v) for where, v in map_strings() if CJK.search(v)], [])
        for path in POC.rglob("*.Civ6Map"):
            with sqlite3.connect(path) as db:
                names = [n for (n,) in db.execute("SELECT Name FROM Cities")]
            self.assertTrue(all(n.startswith("LOC_") for n in names), (path.name, names))

    def test_no_literal_cjk_outside_japanese_text(self):
        bad = []
        for path in [*POC.rglob("*.sql"), *POC.rglob("*.lua")]:
            for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
                if CJK.search(strip_comment(line)):
                    bad.append((path.name, number, line.strip()))
        for path in [*POC.rglob("*.xml"), *POC.rglob("*.modinfo"), *POC.rglob("*.artdef")]:
            root = ET.parse(path).getroot()
            skip = {id(el) for parent in root.iter() for el in parent.iter()
                    if parent.get("Language") == "ja_JP" or parent.tag == "ja_JP"}
            for el in root.iter():
                if id(el) in skip:
                    continue
                if any(CJK.search(v or "") for v in [el.text, *el.attrib.values()]):
                    bad.append((path.name, el.tag, el.attrib, el.text))
        self.assertEqual(bad, [])


if __name__ == "__main__":
    unittest.main()
