"""Generate per-house city name pools from documents/spec/data/japan-city-name-pools.json.

Writes poc/JAC_PoC_CustomCivs/Data/JAC_CityNames_Gameplay.sql and
Text/JAC_CityNames_Text.xml. English is Hepburn without macrons and Russian is
Polivanov Cyrillic, both derived from the kana reading (the castle names of the
map follow the same conventions, see generate.py CASTLE_NAMES).
"""

from __future__ import annotations

import json
import re
import xml.etree.ElementTree as ET
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / "documents/spec/data/japan-city-name-pools.json"
MOD = ROOT / "poc/JAC_PoC_CustomCivs"
SQL = MOD / "Data/JAC_CityNames_Gameplay.sql"
TEXT = MOD / "Text/JAC_CityNames_Text.xml"
# House names come first; the shared pool is the last resort before the engine
# would borrow another civilization's list (see documents/poc/city-names.md).
SHARED_SORT_BASE = 1000
# The engine picks at random among the next N unused names in SortIndex order
# (Civilizations.RandomCityNameDepth; official civs use 10). Each pool lists the
# house's core first, so a small window keeps early cities in the home domain.
RANDOM_DEPTH = 3

_ROW = {  # kana -> (Hepburn, Polivanov)
    "あ": ("a", "а"), "い": ("i", "и"), "う": ("u", "у"), "え": ("e", "э"), "お": ("o", "о"),
    "か": ("ka", "ка"), "き": ("ki", "ки"), "く": ("ku", "ку"), "け": ("ke", "кэ"), "こ": ("ko", "ко"),
    "さ": ("sa", "са"), "し": ("shi", "си"), "す": ("su", "су"), "せ": ("se", "сэ"), "そ": ("so", "со"),
    "た": ("ta", "та"), "ち": ("chi", "ти"), "つ": ("tsu", "цу"), "て": ("te", "тэ"), "と": ("to", "то"),
    "な": ("na", "на"), "に": ("ni", "ни"), "ぬ": ("nu", "ну"), "ね": ("ne", "нэ"), "の": ("no", "но"),
    "は": ("ha", "ха"), "ひ": ("hi", "хи"), "ふ": ("fu", "фу"), "へ": ("he", "хэ"), "ほ": ("ho", "хо"),
    "ま": ("ma", "ма"), "み": ("mi", "ми"), "む": ("mu", "му"), "め": ("me", "мэ"), "も": ("mo", "мо"),
    "や": ("ya", "я"), "ゆ": ("yu", "ю"), "よ": ("yo", "ё"),
    "ら": ("ra", "ра"), "り": ("ri", "ри"), "る": ("ru", "ру"), "れ": ("re", "рэ"), "ろ": ("ro", "ро"),
    "わ": ("wa", "ва"), "を": ("o", "о"),
    "が": ("ga", "га"), "ぎ": ("gi", "ги"), "ぐ": ("gu", "гу"), "げ": ("ge", "гэ"), "ご": ("go", "го"),
    "ざ": ("za", "дза"), "じ": ("ji", "дзи"), "ず": ("zu", "дзу"), "ぜ": ("ze", "дзэ"), "ぞ": ("zo", "дзо"),
    "だ": ("da", "да"), "ぢ": ("ji", "дзи"), "づ": ("zu", "дзу"), "で": ("de", "дэ"), "ど": ("do", "до"),
    "ば": ("ba", "ба"), "び": ("bi", "би"), "ぶ": ("bu", "бу"), "べ": ("be", "бэ"), "ぼ": ("bo", "бо"),
    "ぱ": ("pa", "па"), "ぴ": ("pi", "пи"), "ぷ": ("pu", "пу"), "ぺ": ("pe", "пэ"), "ぽ": ("po", "по"),
}
_YOON = {"ゃ": ("a", "я"), "ゅ": ("u", "ю"), "ょ": ("o", "ё")}
_YOON_HEAD = {"し": ("sh", "с"), "ち": ("ch", "т"), "じ": ("j", "дз"), "ぢ": ("j", "дз")}


def _syllables(word: str) -> list[tuple[str, str]]:
    """Split one hiragana word into (Hepburn, Polivanov) syllables; ー/long vowels kept."""
    out: list[tuple[str, str]] = []
    i = 0
    while i < len(word):
        ch = word[i]
        nxt = word[i + 1] if i + 1 < len(word) else ""
        if nxt in _YOON and ch in _ROW:
            vowel_en, vowel_ru = _YOON[nxt]
            head_en, head_ru = _YOON_HEAD.get(ch, (_ROW[ch][0][:-1] + "y", _ROW[ch][1][:-1]))
            out.append((head_en + vowel_en, head_ru + vowel_ru))
            i += 2
        elif ch in _ROW or ch in "っんー":
            out.append((ch, ch) if ch in "っんー" else _ROW[ch])
            i += 1
        else:
            raise ValueError(f"unsupported kana {ch!r} in {word!r}")
    return out


def _vowel(syllable: str) -> str:
    return syllable[-1] if syllable and syllable[-1] in "aiueo" else ""


def _word(word: str) -> tuple[str, str]:
    sylls = _syllables(word)
    en, ru = [], []
    for i, (s_en, s_ru) in enumerate(sylls):
        prev_en = next((e for e, _ in reversed(sylls[:i]) if e not in "っんー"), "")
        after = sylls[i + 1] if i + 1 < len(sylls) else ("", "")
        if s_en == "ー":
            continue
        if s_en == "っ":
            en.append("t" if after[0].startswith("ch") else after[0][:1])
            ru.append(after[1][:1])
            continue
        if s_en == "ん":
            en.append("n'" if after[0][:1] in ("a", "i", "u", "e", "o", "y") else "n")
            ru.append("нъ" if after[0][:1] in ("a", "i", "u", "e", "o", "y")
                      else "м" if after[1][:1] in ("б", "п", "м") else "н")
            continue
        last = _vowel(prev_en) if i and sylls[i - 1][0] not in "っん" else ""
        if s_en == "u" and last in ("o", "u"):   # おう・うう: long vowel, no macron
            continue
        if s_en == "o" and last == "o":          # おお
            continue
        if s_en == "i" and last in ("a", "u", "o", "e") and after[0]:
            en.append("i")
            ru.append("й")                       # diphthong inside a word (Кайфу, Эйхэйдзи)
            continue
        if s_en == "i" and last == "e":
            en.append("i")
            ru.append("й")
            continue
        en.append(s_en)
        ru.append(s_ru)
    return "".join(en).capitalize(), "".join(ru).capitalize()


def to_hiragana(text: str) -> str:
    return "".join(chr(ord(c) - 0x60) if "ァ" <= c <= "ヶ" else c for c in text)


def romanize(kana: str) -> tuple[str, str]:
    """Kana reading (words separated by spaces) -> (English, Russian) display names."""
    words = [_word(w) for w in to_hiragana(kana).split()]
    return " ".join(e for e, _ in words), "-".join(r for _, r in words)


def data() -> dict:
    return json.loads(DATA.read_text(encoding="utf-8"))


def names(entry: dict) -> tuple[str, str]:
    en, ru = romanize(entry["kana"])
    return entry.get("en", en), entry.get("ru", ru)


def tag(entry: dict) -> str:
    return "LOC_JAC_CITYNAME_" + re.sub(r"[^A-Z0-9]+", "_", names(entry)[0].upper()).strip("_")


def pools(source: dict) -> dict[str, list[tuple[str, int]]]:
    """CivilizationType -> [(LOC tag, SortIndex)], house names then the shared pool."""
    entries = [e for pool in source["houses"].values() for e in pool] + source["shared"]
    tags = [tag(e) for e in entries]
    if len(tags) != len(set(tags)):  # two places with one romanization would share a LOC tag
        raise ValueError(f"duplicate city names: {sorted({t for t in tags if tags.count(t) > 1})}")
    shared = [(tag(e), SHARED_SORT_BASE + i) for i, e in enumerate(source["shared"])]
    return {f"CIVILIZATION_JAC_{house}": [(tag(e), i) for i, e in enumerate(entries)] + shared
            for house, entries in source["houses"].items()}


def gameplay_sql(source: dict) -> str:
    civs = list(pools(source))
    rows = [f"    ('{civ}', '{name}', {sort})" for civ, pool in pools(source).items() for name, sort in pool]
    quoted = ", ".join(f"'{civ}'" for civ in civs)
    return (
        "-- Generated by tools/japan_map_scale/write_city_names.py from documents/spec/data/japan-city-name-pools.json.\n"
        "-- Each house has its own historical names; a shared pool of province names comes last.\n"
        "-- The capital row (SortIndex -1) lives next to each civilization's definition.\n"
        f"UPDATE Civilizations SET RandomCityNameDepth = {RANDOM_DEPTH} WHERE CivilizationType IN ({quoted});\n\n"
        "INSERT INTO CityNames (CivilizationType, CityName, SortIndex) VALUES\n"
        + ",\n".join(rows) + ";\n")


def text_xml(source: dict) -> ET.Element:
    root = ET.Element("GameData")
    localized = ET.SubElement(root, "LocalizedText")
    for entry in [e for entries in source["houses"].values() for e in entries] + source["shared"]:
        en, ru = names(entry)
        for language, value in (("ja_JP", entry["ja"]), ("en_US", en), ("ru_RU", ru)):
            ET.SubElement(ET.SubElement(localized, "Row", Tag=tag(entry), Language=language), "Text").text = value
    ET.indent(root, space="\t")
    return root


def main() -> None:
    source = data()
    SQL.write_text(gameplay_sql(source), encoding="utf-8")
    ET.ElementTree(text_xml(source)).write(TEXT, encoding="utf-8", xml_declaration=True)


if __name__ == "__main__":
    main()
