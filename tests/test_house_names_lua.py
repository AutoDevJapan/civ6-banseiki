"""Runs the SSOT 3.2 house-name Lua against a stubbed Civ6 UI (documents/poc/house-names.md).

Needs the ``lupa`` package (a Lua runtime for Python).  Without it the tests are skipped; run them with
``uv run --no-project --with lupa python -m unittest tests.test_house_names_lua``.
"""

from __future__ import annotations

import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

try:
    import lupa
except ImportError:  # pragma: no cover - depends on the environment
    lupa = None

ROOT = Path(__file__).resolve().parents[1]
UI = ROOT / "poc/JAC_PoC_BanseikiRuleset/UI"
DATA = ROOT / "poc/JAC_PoC_SengokuTrees/UI/JAC_HouseNamesData_Sengoku.lua"
TEXT = ROOT / "poc/JAC_PoC_SengokuTrees/Text/JAC_Sengoku_Text.xml"
CIVIC = "LOC_CIVIC_JAC_SENGOKU_TAIKO_LAND_SURVEY_NAME"
ERA = "LOC_JAC_SENGOKU_ERA_3_NAME"

# Engine-side text as it would be built in C++ (notification bodies, moments).
ENGINE_TEXT = {
    "LOC_NOTIFICATION_CIVIC_DISCOVERED_SUMMARY": {"en_US": "You have discovered {1_Name}!", "ja_JP": "{1_Name}を習得しました！"},
    "LOC_MOMENT_TEST": {"en_US": "First civic of the {1_Era} era.", "ja_JP": "{1_Era}の最初の社会制度。"},
}

# Minimal stand-ins for the engine: Locale (text lookup with {n_X} parameters), Game, PlayerConfigurations, include.
STUBS = r"""
local texts, language, sources, civilization = ...
local function format(tag, ...)
  local text = texts[tag]
  if text == nil then return tag end
  local args = { ... }
  return (string.gsub(text, "{(%d+)_[^}]*}", function(n)
    local value = args[tonumber(n)]
    return texts[value] or tostring(value)
  end))
end
-- Like the game, the engine's Locale table is read-only: writing any key raises "Attempt to modify read-only table".
local engineLocale = {
  Lookup = function(tag, ...) return format(tag, ...) end,
  ConvertTextKey = function(tag, ...) return format(tag, ...) end,
  ToUpper = function(tag) return string.upper(texts[tag] or tag) end,
  Compare = function(a, b) return a < b end,
}
ENGINE_LOCALE = setmetatable({}, {
  __index = engineLocale,
  __newindex = function() error("Attempt to modify read-only table", 2) end,
})
Locale = ENGINE_LOCALE
LOG = {}
LOCAL_CIV = civilization
Game = { GetLocalPlayer = function() return 0 end }
PlayerConfigurations = { [0] = { GetCivilizationTypeName = function() return LOCAL_CIV end } }
function include(name, wildcard)
  if wildcard then
    for file, source in pairs(sources) do
      if string.sub(file, 1, #name) == name then load(source, file)() end
    end
  elseif sources[name] then
    load(sources[name], name)()
  end
end
function print(line) LOG[#LOG + 1] = line end
"""

PEDIA_STUBS = r"""
CAPTIONS = {}
local function record(value) CAPTIONS[#CAPTIONS + 1] = value end
function Do_AddChapterWithParagraphs(_, _, header, paragraphs) record(header); for _, p in ipairs(paragraphs) do record(p) end end
function Do_AddHeader(_, caption) record(caption) end
function Do_AddParagraph(_, paragraph) record(paragraph) end
function Do_AddHeaderBody(_, header, body) record(header); record(body) end
function Do_AddIconHeaderBody(_, icon, header, body) record(header); record(body) end
function HookupIcon(icon) record(icon[2]); record(icon[1]) end
function AddQuote(quote) record(quote) end
function AddRightColumnStatBox(title, populate)
  record(title)
  local box = {}
  function box:AddHeader(c) record(c) end
  function box:AddLabel(c) record(c) end
  function box:AddSmallLabel(c) record(c) end
  function box:AddIconLabel(icon, c) record(icon[2] == c and "same" or "different"); record(c) end
  function box:AddIconNumberLabel(icon, value, c) record(c) end
  populate(box)
end
ContextPtr = { GetID = function() return "Civilopedia" end }
"""


def load_texts(language: str) -> dict[str, str]:
    texts = {tag: by_language[language] for tag, by_language in ENGINE_TEXT.items()}
    for row in ET.parse(TEXT).getroot().iter("Row"):
        if row.get("Language") == language:
            texts[row.get("Tag")] = row.findtext("Text") or ""
    return texts


@unittest.skipIf(lupa is None, "lupa is not installed")
class HouseNamesLuaTests(unittest.TestCase):
    def runtime(self, civilization: str, language: str = "en_US", extra: str = ""):
        lua = lupa.LuaRuntime(unpack_returned_tuples=True)
        texts = load_texts(language)
        sources = {"JAC_HouseNames": (UI / "JAC_HouseNames.lua").read_text(encoding="utf-8"),
                   "JAC_HouseNamesData_Sengoku": DATA.read_text(encoding="utf-8")}
        lua.execute(STUBS, lua.table_from(texts), language, lua.table_from(sources), civilization)
        if extra:
            lua.execute(extra)
        lua.execute('include("JAC_HouseNames"); JAC_InstallHouseNames("Test")')
        self.texts = texts
        return lua

    def lookup(self, lua, *args):
        return lua.globals().Locale.Lookup(*args)

    def test_install_shadows_the_context_global_without_writing_the_engine_table(self):
        lua = self.runtime("CIVILIZATION_JAC_ODA")
        g = lua.globals()
        self.assertEqual(list(g.LOG.values()), ["[JAC_POC] houseNames context=Test result=installed"])
        self.assertTrue(lua.eval("Locale ~= ENGINE_LOCALE and JAC_HouseNamesBaseLocale == ENGINE_LOCALE"))
        self.assertEqual(self.lookup(lua, CIVIC), self.texts[CIVIC + "_HOUSE"])
        self.assertEqual(g.ENGINE_LOCALE.Lookup(CIVIC), self.texts[CIVIC])  # the engine table is untouched
        self.assertTrue(lua.eval("Locale.Compare == ENGINE_LOCALE.Compare"))  # unwrapped members still reach the engine table
        # A second include in the same context (ToolTipHelper is read several times) does not wrap twice,
        # and a file loaded after the install sees the shadowed Locale.
        lua.execute('include("JAC_HouseNames"); JAC_InstallHouseNames("Test")')
        self.assertEqual(list(g.LOG.values())[-1], "[JAC_POC] houseNames context=Test result=already_installed")
        self.assertTrue(lua.eval("JAC_HouseNamesBaseLocale == ENGINE_LOCALE"))
        self.assertEqual(lua.execute("return Locale.Lookup(...)", CIVIC), self.texts[CIVIC + "_HOUSE"])

    def test_owner_sees_house_names_for_keys_arguments_and_upper_case(self):
        lua = self.runtime("CIVILIZATION_JAC_ODA")
        house = self.texts[CIVIC + "_HOUSE"]
        self.assertEqual(self.lookup(lua, CIVIC), house)
        self.assertEqual(self.lookup(lua, "LOC_NOTIFICATION_CIVIC_DISCOVERED_SUMMARY", CIVIC), f"You have discovered {house}!")
        upper = lua.globals().JAC_HouseNamesBaseLocale.ToUpper
        self.assertEqual(lua.globals().Locale.ToUpper(CIVIC), upper(house))

    def test_engine_built_text_is_switched_for_the_owner(self):
        for language in ("en_US", "ja_JP"):
            with self.subTest(language=language):
                lua = self.runtime("CIVILIZATION_JAC_ODA", language)
                generic, house = self.texts[CIVIC], self.texts[CIVIC + "_HOUSE"]
                era, era_house = self.texts[ERA], self.texts[ERA + "_HOUSE"]
                # What pNotification:GetSummary() returns: already formatted by the engine with the generic name.
                engine = self.texts["LOC_NOTIFICATION_CIVIC_DISCOVERED_SUMMARY"].replace("{1_Name}", generic)
                shown = self.lookup(lua, engine)
                self.assertIn(house, shown)
                self.assertNotIn(generic, shown)
                moment = self.texts["LOC_MOMENT_TEST"].replace("{1_Era}", era)
                self.assertEqual(self.lookup(lua, moment), self.texts["LOC_MOMENT_TEST"].replace("{1_Era}", era_house))
                # NotificationPanel.lua:958-962 upper-cases the looked-up title; SetNotificationText must keep the house name.
                upper = lua.globals().JAC_HouseNamesBaseLocale.ToUpper
                self.assertEqual(lua.globals().Locale.ToUpper(upper(engine)), upper(engine.replace(generic, house)))
                # Passing already-switched text through Lookup again (TechTree.lua:1013 nests Lookup) must not double it.
                self.assertEqual(self.lookup(lua, self.lookup(lua, engine)), shown)

    def test_other_houses_and_observers_keep_generic_names(self):
        for civilization in ("CIVILIZATION_JAC_TAKEDA", None):
            with self.subTest(civilization=civilization):
                lua = self.runtime(civilization)
                generic = self.texts[CIVIC]
                engine = f"You have discovered {generic}!"
                self.assertEqual(self.lookup(lua, CIVIC), generic)
                self.assertEqual(self.lookup(lua, engine), engine)
                self.assertFalse(lua.globals().JAC_LocalHouseHasNames())

    def test_load_screen_without_game_keeps_generic_names(self):
        # LoadScreen.lua:473 (OnLoadGameViewStateDone) runs before Game exists; an error there hangs the load.
        lua = self.runtime("CIVILIZATION_JAC_ODA", extra="Game = nil; PlayerConfigurations = nil")
        generic = self.texts[CIVIC]
        self.assertEqual(self.lookup(lua, CIVIC), generic)
        self.assertEqual(lua.globals().JAC_ApplyHouseNames(generic), generic)
        self.assertFalse(lua.globals().JAC_LocalHouseHasNames())

    def test_civilopedia_parts_receive_house_text_only_for_the_owner(self):
        pedia = (UI / "CivilopediaPage_JAC_HouseNames.lua").read_text(encoding="utf-8")
        calls = r"""
        Do_AddChapterWithParagraphs(nil, nil, "LOC_UI_PEDIA_DESCRIPTION", { "%s" })
        Do_AddHeader(nil, "%s")
        HookupIcon({ "ICON_X", "%s", "SEARCH" })
        AddQuote("%s")
        AddRightColumnStatBox("LOC_UI_PEDIA_REQUIREMENTS", function(s)
          s:AddLabel("%s")
          s:AddIconLabel({ "ICON_X", "%s" }, "%s")
        end)
        """ % (ERA, CIVIC, CIVIC, ERA, ERA, CIVIC, CIVIC)
        for civilization, expect_house in (("CIVILIZATION_JAC_ODA", True), ("CIVILIZATION_JAC_TAKEDA", False)):
            with self.subTest(civilization=civilization):
                lua = self.runtime(civilization, extra=PEDIA_STUBS)
                lua.execute(pedia)
                lua.execute(calls)
                captions = list(lua.globals().CAPTIONS.values())
                if expect_house:
                    self.assertIn(self.texts[ERA + "_HOUSE"], captions)
                    self.assertIn(self.texts[CIVIC + "_HOUSE"], captions)
                    self.assertNotIn(ERA, captions)
                    self.assertNotIn(CIVIC, captions)
                    self.assertIn("ICON_X", captions)  # icon names are left alone
                    self.assertIn("same", captions)  # official tooltip de-duplication still works
                else:
                    # Unchanged: keys go to the engine exactly as the official page passes them.
                    self.assertIn(ERA, captions)
                    self.assertIn(CIVIC, captions)
                    self.assertNotIn(self.texts[CIVIC + "_HOUSE"], captions)

    def test_historic_moment_descriptions_are_switched(self):
        extra = r"""
        local generic = ...
        Game.GetHistoryManager = function()
          return { GetAllMomentsData = function(self, player, level)
                     return { { InstanceDescription = "First civic of the " .. generic .. " era.", Turn = 3 } } end,
                   GetMomentData = function(self, id) return { InstanceDescription = generic, ID = id }, "second" end }
        end
        """
        lua = lupa.LuaRuntime(unpack_returned_tuples=True)
        texts = load_texts("en_US")
        sources = {"JAC_HouseNames": (UI / "JAC_HouseNames.lua").read_text(encoding="utf-8"),
                   "JAC_HouseNamesData_Sengoku": DATA.read_text(encoding="utf-8"),
                   "HistoricMoments": "CHECK = Game.GetHistoryManager()"}
        lua.execute(STUBS, lua.table_from(texts), "en_US", lua.table_from(sources), "CIVILIZATION_JAC_ODA")
        lua.execute(extra, texts[ERA])
        lua.execute((UI / "JAC_HistoricMoments.lua").read_text(encoding="utf-8"))
        manager = lua.globals().CHECK
        moments = lua.eval("CHECK:GetAllMomentsData(0, 1)")
        self.assertEqual(moments[1].InstanceDescription, f"First civic of the {texts[ERA + '_HOUSE']} era.")
        self.assertEqual(moments[1].Turn, 3)
        first, second = lua.eval("CHECK:GetMomentData(7)")
        self.assertEqual(first.InstanceDescription, texts[ERA + "_HOUSE"])
        self.assertEqual((first.ID, second), (7, "second"))
        self.assertIsNotNone(manager)


if __name__ == "__main__":
    unittest.main()
