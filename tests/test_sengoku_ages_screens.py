"""The 改元 screen and the era score screen of the Sengoku ages (documents/research/sengoku-ages.md §8,
documents/poc/sengoku-ages.md "改元の画面と時代スコアの画面").

The UI Lua runs in lupa on top of the ledger that Scripts/JAC_Sengoku_Ages.lua writes (the stubs of
tests/test_sengoku_ages.py). Locale.Lookup returns "KEY(arg,...)", so the checks read which text and numbers a
screen shows, not the wording. Takeda (player 1) is the local human.
``uv run --no-project --with lupa python -m unittest tests.test_sengoku_ages_screens``
"""

from __future__ import annotations

import unicodedata
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

from tests.test_sengoku_ages import MOD, REIGNS_JSON, UI_STUBS, LedgerTests, lupa

ROOT = Path(__file__).resolve().parents[1]
RULESET_UI = ROOT / "poc/JAC_PoC_BanseikiRuleset/UI"

SCREEN_STUBS = r"""
local files = ...
-- A UI context: its own globals (ContextPtr, Controls, functions, the wrapped Locale) over the shared ones, as live.
-- include loads the named file from the mods into the context (a wildcard include loads every file with the prefix).
function NEW_CONTEXT(name, ...)
  local env = setmetatable({}, { __index = _G })
  env.ContextPtr = CONTROL()
  env.Controls = setmetatable({}, { __index = function(t, k) local c = CONTROL(); rawset(t, k, c); return c end })
  env.include = function(file, wildcard)
    if file == "InstanceManager" then return end
    local found = false
    for fileName, source in pairs(files) do
      if fileName == file or (wildcard and string.sub(fileName, 1, #file) == file) then
        found = true
        assert(load(source, fileName, "t", env))()
      end
    end
    assert(found or wildcard, "include " .. file)
  end
  for _, source in ipairs({ ... }) do assert(load(source, name, "t", env))() end
  return env
end
Locale = {
  Lookup = function(key, ...)
    local args = { ... }
    if #args == 0 then return key end
    for i, v in ipairs(args) do args[i] = tostring(v) end
    return key .. "(" .. table.concat(args, ",") .. ")"
  end,
  ToUpper = function(s) return "UP:" .. s end,
}
Keys = { VK_ESCAPE = 27 }
KeyEvents = { KeyUp = 2 }
UI.GetPlayerColors = function() return 1, 2 end
UI.PlaySound = function() end
for _, config in pairs(PlayerConfigurations) do config.GetLeaderTypeName = function() return "LEADER_X" end end
function AUTOPLAY_END() LuaEvents.AutoPlayEnd() end
"""

# The official EraProgressPanel functions that the extension builds on (DLC/Expansion2/UI/Additions/
# EraProgressPanel.lua:212 Refresh, :273 PopulateCivilizationIcons), reduced to what the extension relies on.
PANEL_STUBS = r"""
BASE_CALLS, ICONS = 0, {}
function Refresh() BASE_CALLS = BASE_CALLS + 1; Controls.Title:SetText("LOC_ERA_PROGRESS_TITLE") end
function PopulateCivilizationIcons()
  ICONS = {}
  local eras = Game.GetEras()
  for _, p in ipairs(PlayerManager.GetAliveMajors()) do
    local id = p:GetID()
    if id ~= Game.GetLocalPlayer() then
      local icon = "[ICON_GLORY_NORMAL_AGE]"
      if eras:HasHeroicGoldenAge(id) then icon = "[ICON_GLORY_SUPER_GOLDEN_AGE]"
      elseif eras:HasGoldenAge(id) then icon = "[ICON_GLORY_GOLDEN_AGE]"
      elseif eras:HasDarkAge(id) then icon = "[ICON_GLORY_DARK_AGE]" end
      ICONS[id] = icon
    end
  end
end
"""


# The official ActionPanel functions the era indicator extension wraps (DLC/Expansion2/UI/Replacements/
# ActionPanel_Expansion1.lua:187-216 RealizeEraIndicator, reached through ActionPanel_Expansion2.lua:6).
ACTION_PANEL_STUBS = r"""
BASE_ERA_CALLS = 0
function RealizeEraIndicator()
  BASE_ERA_CALLS = BASE_ERA_CALLS + 1
  Controls.EraToolTipArea1:SetToolTipString("LOC_GAME_ERA_DESC")
  Controls.EraToolTipArea2:SetToolTipString("LOC_GAME_ERA_DESC")
end
"""


def _files():
    names = {"JAC_HouseNames": RULESET_UI / "JAC_HouseNames.lua",
             "JAC_HouseNamesData_Sengoku": MOD / "UI/JAC_HouseNamesData_Sengoku.lua",
             "JAC_SengokuAgesView": MOD / "UI/JAC_SengokuAgesView.lua",
             "JAC_EraProgressPanel_Sengoku": MOD / "UI/JAC_EraProgressPanel_Sengoku.lua"}
    return {name: path.read_text(encoding="utf-8") for name, path in names.items()}


@unittest.skipIf(lupa is None, "lupa is not installed")
class ScreenTestBase(unittest.TestCase):
    load_script, turns, house, snapshot = (LedgerTests.load_script, LedgerTests.turns, LedgerTests.house,
                                           LedgerTests.snapshot)

    def setUp(self):
        LedgerTests.setUp(self)
        self.lua.execute(UI_STUBS)
        self.lua.execute(SCREEN_STUBS, self.lua.table_from(_files()))

    def make_dark(self, pid: int) -> None:
        self.turns(1, 65)
        self.house(pid).score = -1000  # as the live scenario sengoku-dark-policy.json does before 弘治
        self.g.Game.SetProperty(self.g.Game, "JAC_AGES_LEDGER", self.g.LEDGER())


class ReignPopupTests(ScreenTestBase):
    def setUp(self):
        super().setUp()
        read = lambda name: (MOD / f"UI/{name}.lua").read_text(encoding="utf-8")
        self.dark = self.g.NEW_CONTEXT("JAC_SengokuDarkPolicy", read("JAC_SengokuDarkPolicy"))
        self.popup = self.g.NEW_CONTEXT("JAC_SengokuReignPopup", read("JAC_SengokuReignPopup"))
        self.controls = self.popup.Controls

    def shown(self) -> bool:
        return not self.popup.ContextPtr.hidden

    def dark_shown(self) -> bool:
        return not self.dark.ContextPtr.hidden

    def test_not_shown_before_the_first_era_name_change(self):
        self.turns(1, 65)
        self.g.LOCAL_TURN_BEGIN()
        self.g.AUTOPLAY_END()
        self.assertFalse(self.shown())
        self.assertIsNone(self.popup.JAC_REIGN_POPUP_LAST)

    def test_koji_turn_shows_the_house_result_once(self):
        self.turns(1, 30)
        for i in range(1, 11):
            self.g.Players[1].research.techs[i] = True  # 10 points: Takeda golden at 弘治
        self.turns(31, 66)
        self.g.LOCAL_TURN_BEGIN()
        self.assertTrue(self.shown())
        r1 = self.g.LEDGER().history["r1"].p1
        self.assertEqual(r1.age, "GOLDEN")
        c = self.controls
        self.assertEqual(c.Title.text, "UP:LOC_JAC_REIGN_KOJI_ANNOUNCEMENT")  # 元号 and year
        self.assertEqual(c.EraRibbonValue.text, "LOC_JAC_AGE_NAME_GOLDEN [ICON_GLORY_GOLDEN_AGE]")
        self.assertEqual((c.EraRibbon.texture, c.Background.texture, c.WindowFrame.texture),
                         ("Ages_BannerLongGolden", "Ages_ParchmentGolden", "Ages_FrameGolden"))
        self.assertTrue(c.HeroicFlare.hidden)
        self.assertEqual(c.Score.text, f"LOC_JAC_AGE_REIGN_SCORE(LOC_JAC_REIGN_TENBUN_NAME,{r1.score})")
        self.assertEqual(c.Thresholds.text, f"LOC_JAC_AGE_THRESHOLDS({r1.golden},{r1.dark})")
        self.assertEqual(c.EraEffects.text, "LOC_JAC_AGE_GOLDEN_ANNOUNCEMENT")
        self.assertEqual(c.Continue.text, "LOC_CONTINUE")
        last = self.popup.JAC_REIGN_POPUP_LAST
        self.assertEqual((last.reign, last.reignType, last.age, last.turn), (1, "REIGN_KOJI", "GOLDEN", 66))
        c.Continue.click()
        self.assertFalse(self.shown())
        self.g.LOCAL_TURN_BEGIN()  # the same 改元 is not shown again
        self.g.AUTOPLAY_END()
        self.assertFalse(self.shown())
        self.turns(67)
        self.g.LOCAL_TURN_BEGIN()
        self.assertFalse(self.shown())
        self.assertTrue(self.popup.JAC_ShowReignPopup(True))  # reopened on request (the live scenario)
        self.assertTrue(self.shown())
        self.popup.OnClose()  # the autotest driver's closer
        self.assertFalse(self.shown())

    def test_autoplay_end_shows_it_when_the_turn_begin_did_not(self):
        self.turns(1, 66)
        self.g.AUTOPLAY_END()
        self.assertTrue(self.shown())
        self.assertEqual(self.popup.JAC_REIGN_POPUP_LAST.reign, 1)

    def test_dark_house_gets_the_policy_button_and_the_choice_after_closing(self):
        self.make_dark(1)
        self.turns(66)
        self.g.LOCAL_TURN_BEGIN()  # both screens listen; the 改元 screen keeps the choice closed while it shows
        self.assertTrue(self.shown())
        self.assertFalse(self.dark_shown())
        c = self.controls
        self.assertEqual(c.EraRibbonValue.text, "LOC_JAC_AGE_NAME_DARK [ICON_GLORY_DARK_AGE]")
        self.assertEqual(c.Background.texture, "Ages_ParchmentDark")
        self.assertEqual(c.EraEffects.text, "LOC_JAC_AGE_DARK_ELITE_ANNOUNCEMENT")  # Takeda's default policy
        self.assertEqual(c.Continue.text, "LOC_JAC_AGE_REIGN_CHOOSE_POLICY")
        self.assertTrue(self.popup.JAC_REIGN_POPUP_LAST.pendingChoice)
        c.Continue.click()
        self.assertFalse(self.shown())
        self.assertTrue(self.dark_shown())  # the merged choice screen (UI/JAC_SengokuDarkPolicy.lua)

    def test_turn_end_closes_without_opening_the_choice(self):
        self.make_dark(1)
        self.turns(66)
        self.g.LOCAL_TURN_BEGIN()
        self.g.LOCAL_TURN_END()
        self.assertFalse(self.shown())
        self.assertFalse(self.dark_shown())

    def test_heroic_after_dark_uses_the_heroic_art(self):
        self.make_dark(1)
        self.turns(66, 70)
        for i in range(1, 11):
            self.g.Players[1].research.techs[i] = True
        self.turns(71, 73)  # 永禄: a dark house reaching golden is heroic
        self.g.LOCAL_TURN_BEGIN()
        self.assertEqual(self.g.LEDGER().history["r2"].p1.age, "HEROIC")
        c = self.controls
        self.assertEqual(c.Background.texture, "Ages_ParchmentHeroic")
        self.assertFalse(c.HeroicFlare.hidden)
        self.assertEqual(c.Title.text, "UP:LOC_JAC_REIGN_EIROKU_ANNOUNCEMENT")
        self.assertEqual(c.Score.text.split(",")[0], "LOC_JAC_AGE_REIGN_SCORE(LOC_JAC_REIGN_KOJI_NAME")

    def test_no_local_player_shows_nothing(self):
        self.turns(1, 66)
        self.g.LOCAL_PLAYER = -1
        self.g.LOCAL_TURN_BEGIN()
        self.assertFalse(self.shown())

    def test_merged_period_names_the_era_it_judged(self):
        # Two 改元 on one turn are judged once (Scripts/JAC_Sengoku_Ages.lua NextJudged); the judgement records the
        # period it closed ("from"), and the screen names that era, not the skipped one.
        self.turns(1, 73)
        l = self.g.LEDGER()
        l.history["r2"]["from"] = 0
        self.g.Game.SetProperty(self.g.Game, "JAC_AGES_LEDGER", l)
        self.assertTrue(self.popup.JAC_ShowReignPopup(True))
        self.assertEqual(self.controls.Score.text.split(",")[0], "LOC_JAC_AGE_REIGN_SCORE(LOC_JAC_REIGN_TENBUN_NAME")

    def test_the_screen_writes_nothing(self):
        self.turns(1, 66)
        before = self.snapshot()
        self.g.LOCAL_TURN_BEGIN()
        self.controls.Continue.click()
        self.assertEqual(self.snapshot(), before)
        self.assertEqual(len(self.g.REQUESTS), 0)


class EraPanelTests(ScreenTestBase):
    def setUp(self):
        super().setUp()
        self.panel = self.g.NEW_CONTEXT("EraProgressPanel", PANEL_STUBS, _files()["JAC_EraProgressPanel_Sengoku"])
        self.controls = self.panel.Controls

    def test_before_the_ledger_the_official_panel_stays(self):
        self.panel.Refresh()
        self.assertEqual(self.panel.BASE_CALLS, 1)
        self.assertEqual(self.controls.Title.text, "LOC_ERA_PROGRESS_TITLE")
        self.assertIsNone(self.panel.JAC_AGES_PANEL_VIEW)

    def test_tenbun_period_shows_the_ledger(self):
        self.turns(1, 5)
        self.g.CAPTURE(1, 2, 1)  # Takeda takes an Oda castle: castle 2
        self.g.Players[1].research.techs[3] = True
        self.turns(6)
        self.panel.Refresh()
        c, h = self.controls, self.house(1)
        self.assertEqual(self.panel.BASE_CALLS, 1)
        self.assertEqual(c.Title.text, "UP:LOC_JAC_AGE_PANEL_TITLE")
        self.assertEqual(c.EraName.text, "LOC_JAC_AGE_PANEL_PERIOD(LOC_JAC_REIGN_TENBUN_NAME,1534)")  # the era is the 元号 (owner decision L-1)
        self.assertEqual(c.TimeToNextEra.text, "60")  # 弘治 at turn 66
        self.assertFalse(c.TimeToNextEraContainer.hidden)
        self.assertEqual(c.EraScoreValue.text, "3")
        self.assertEqual(c.EraRibbonValue.text, "LOC_JAC_AGE_NAME_NONE")
        self.assertEqual(c.EraRibbon.texture, "Controls_AgesBanner_Normal")
        golden, dark = h.next.golden, h.next.dark
        self.assertEqual(c.GoldenThreshold.text, f"{golden}+")
        self.assertEqual(c.NormalThreshold.text, f"{dark} - {golden - 1}")
        self.assertEqual(c.DarkThreshold.text, f"0 - {dark - 1}" if dark > 0 else "-")
        self.assertEqual(c.GoldenLabel.text, "LOC_ERA_REVIEW_HAVE_GOLDEN_AGE_LABEL")
        projected = "GOLDEN" if 3 >= golden else ("NORMAL" if 3 >= dark else "DARK")
        self.assertEqual(self.panel.JAC_AGES_PANEL_VIEW.projected, projected)
        self.assertEqual([not c.GoldenCheckmark.hidden, not c.NormalCheckmark.hidden, not c.DarkCheckmark.hidden],
                         [projected == "GOLDEN", projected == "NORMAL", projected == "DARK"])
        lines = c.EraEffects.text.split("[NEWLINE]")
        self.assertEqual(lines, [
            "LOC_JAC_AGE_NONE_EFFECT", "",
            "LOC_JAC_AGE_PANEL_PARTS(LOC_JAC_REIGN_TENBUN_NAME)",
            "[ICON_BULLET]LOC_JAC_AGE_PART_HOME(0)", "[ICON_BULLET]LOC_JAC_AGE_PART_REGAIN(0)",
            "[ICON_BULLET]LOC_JAC_AGE_PART_CASTLE(2)", "[ICON_BULLET]LOC_JAC_AGE_PART_KO(0)",
            "[ICON_BULLET]LOC_JAC_AGE_PART_ALLY(0)", "[ICON_BULLET]LOC_JAC_AGE_PART_RESEARCH(1)",
            "[ICON_BULLET]LOC_JAC_AGE_PART_EVENT(0)", "",
            "LOC_JAC_AGE_PANEL_HISTORY", "[ICON_BULLET]LOC_JAC_AGE_PANEL_HISTORY_NONE"])
        self.assertEqual(c.EraEffectsHeader.text, "LOC_JAC_AGE_PANEL_LEDGER_HEADER")

    def test_after_koji_history_other_houses_ages_and_the_heroic_label(self):
        self.make_dark(1)
        for i in range(1, 11):
            self.g.Players[0].research.techs[i] = True  # Oda golden at 弘治
        self.turns(66, 70)
        self.panel.Refresh()
        # The ledger-backed Game.GetEras() is only for the icon list: the context sees the engine's Game again.
        self.assertEqual(self.lua.eval("function(env) return rawget(env, 'Game') == Game end")(self.panel), True)
        c, l = self.controls, self.g.LEDGER()
        r1 = l.history["r1"]
        self.assertEqual(c.EraName.text, "LOC_JAC_AGE_PANEL_PERIOD(LOC_JAC_REIGN_KOJI_NAME,1555)")
        self.assertEqual(c.TimeToNextEra.text, "3")  # 永禄 at turn 73
        self.assertEqual(c.EraRibbonValue.text, "LOC_JAC_AGE_NAME_DARK [ICON_GLORY_DARK_AGE]")
        self.assertEqual(c.EraRibbon.texture, "Controls_AgesBanner_Dark")
        self.assertEqual(c.GoldenLabel.text, "LOC_ERA_REVIEW_HAVE_HEROIC_AGE_LABEL")  # a dark house can become heroic
        ages = {pid: r1[f"p{pid}"].age for pid in range(4)}
        self.assertEqual((ages[0], ages[1]), ("GOLDEN", "DARK"))  # the official ages are off: all would be normal
        icons = {"GOLDEN": "[ICON_GLORY_GOLDEN_AGE]", "NORMAL": "[ICON_GLORY_NORMAL_AGE]", "DARK": "[ICON_GLORY_DARK_AGE]"}
        self.assertEqual(dict(self.panel.ICONS.items()), {pid: icons[ages[pid]] for pid in (0, 2, 3)})
        lines = c.EraEffects.text.split("[NEWLINE]")
        self.assertEqual(lines[0], "LOC_JAC_AGE_DARK_ELITE_ANNOUNCEMENT")
        self.assertEqual(lines[2], "LOC_JAC_AGE_PANEL_PARTS(LOC_JAC_REIGN_KOJI_NAME)")
        self.assertEqual(lines[-1], "[ICON_BULLET]LOC_JAC_AGE_PANEL_HISTORY_ROW(LOC_JAC_REIGN_KOJI_NAME,1555,"
                                    f"LOC_JAC_AGE_NAME_DARK [ICON_GLORY_DARK_AGE],{r1.p1.score}) "
                                    f"LOC_JAC_AGE_THRESHOLDS({r1.p1.golden},{r1.p1.dark})")

    def test_before_chidai_the_projection_is_dark_whatever_the_score(self):
        self.turns(1, 5)
        self.turns(1457)  # 令和: the next 改元 is 遅代, where every house is dark (SSOT 3.2)
        self.house(1).score = 1000
        self.g.Game.SetProperty(self.g.Game, "JAC_AGES_LEDGER", self.g.LEDGER())
        self.panel.Refresh()
        self.assertEqual(self.panel.JAC_AGES_PANEL_VIEW.projected, "DARK")
        self.assertFalse(self.controls.DarkCheckmark.hidden)
        self.assertTrue(self.controls.GoldenCheckmark.hidden)
        self.assertEqual(self.controls.EraName.text, "LOC_JAC_AGE_PANEL_PERIOD(LOC_JAC_REIGN_REIWA_NAME,2019)")
        self.assertEqual(self.controls.TimeToNextEra.text, "300")

    def test_after_the_last_era_name_no_countdown_or_thresholds(self):
        self.turns(1, 5)
        self.turns(1757)  # 遅代, the last 改元 (jumping judges every 改元 on the way)
        self.panel.Refresh()
        c = self.controls
        count = len(REIGNS_JSON["reigns"])
        self.assertEqual(self.g.LEDGER().reign, count)
        self.assertTrue(c.TimeToNextEraContainer.hidden)
        self.assertTrue(c.ThresholdStack.hidden)
        self.assertEqual(c.EraName.text, "LOC_JAC_AGE_PANEL_PERIOD(LOC_JAC_REIGN_CHIDAI_NAME,2119)")
        self.assertEqual(len([x for x in c.EraEffects.text.split("[NEWLINE]") if "HISTORY_ROW" in x]), count)

    def test_the_panel_writes_nothing(self):
        self.turns(1, 66)
        before = self.snapshot()
        self.panel.Refresh()
        self.assertEqual(self.snapshot(), before)


class EraIndicatorTests(ScreenTestBase):
    """UI/JAC_ActionPanel_Sengoku.lua: the era indicator's tooltip names the current 元号 (owner decision L-1)."""

    def setUp(self):
        super().setUp()
        files = dict(_files(), ActionPanel_Expansion2=ACTION_PANEL_STUBS)
        self.lua.execute(SCREEN_STUBS, self.lua.table_from(files))
        self.panel = self.g.NEW_CONTEXT("ActionPanel", (MOD / "UI/JAC_ActionPanel_Sengoku.lua").read_text(encoding="utf-8"))
        self.controls = self.panel.Controls

    def tooltips(self):
        return (self.controls.EraToolTipArea1.tooltip, self.controls.EraToolTipArea2.tooltip)

    def test_before_the_ledger_the_official_tooltip_stays(self):
        self.panel.RealizeEraIndicator()
        self.assertEqual(self.panel.BASE_ERA_CALLS, 1)
        self.assertEqual(self.tooltips(), ("LOC_GAME_ERA_DESC",) * 2)
        self.assertIsNone(self.panel.JAC_ERA_INDICATOR_TEXT)

    def test_tenbun_then_koji_with_the_turns_to_the_next_era_name(self):
        self.turns(1, 6)
        self.panel.RealizeEraIndicator()
        want = "LOC_JAC_AGE_ERA_NAME(LOC_JAC_REIGN_TENBUN_NAME)[NEWLINE][NEWLINE]LOC_JAC_AGE_ERA_ENDS_IN(60)"
        self.assertEqual(self.tooltips(), (want, want))
        self.turns(7, 66)
        self.g.LOCAL_TURN_BEGIN()  # the turn begin refreshes it
        self.assertEqual(self.panel.JAC_ERA_INDICATOR_TEXT,
                         "LOC_JAC_AGE_ERA_NAME(LOC_JAC_REIGN_KOJI_NAME)[NEWLINE][NEWLINE]LOC_JAC_AGE_ERA_ENDS_IN(7)")
        self.assertEqual(self.tooltips()[0], self.panel.JAC_ERA_INDICATOR_TEXT)

    def test_chidai_has_no_countdown(self):
        self.turns(1, 5)
        self.turns(1757)
        self.panel.RealizeEraIndicator()
        self.assertEqual(self.panel.JAC_ERA_INDICATOR_TEXT, "LOC_JAC_AGE_ERA_NAME(LOC_JAC_REIGN_CHIDAI_NAME)")

    def test_the_indicator_writes_nothing(self):
        self.turns(1, 66)
        before = self.snapshot()
        self.panel.RealizeEraIndicator()
        self.assertEqual(self.snapshot(), before)


class WiringTests(unittest.TestCase):
    def test_ruleset_panel_loads_the_extension_after_the_official_script(self):
        source = (RULESET_UI / "JAC_EraProgressPanel.lua").read_text(encoding="utf-8").rstrip().splitlines()
        self.assertEqual(source[-2:], ['include("EraProgressPanel")', 'include("JAC_EraProgressPanel_", true)'])
        modinfo = (MOD / "JAC_PoC_SengokuTrees.modinfo").read_text(encoding="utf-8")
        imports = modinfo.split('<ImportFiles id="JAC_Sengoku_HouseNames"')[1].split("</ImportFiles>")[0]
        for name in ("JAC_EraProgressPanel_Sengoku.lua", "JAC_SengokuAgesView.lua", "JAC_SengokuReignPopup.lua",
                     "JAC_SengokuReignPopup.xml", "JAC_ActionPanel_Sengoku.lua"):
            self.assertIn(f"<File>UI/{name}</File>", imports)
            self.assertEqual(modinfo.count(f"<File>UI/{name}</File>"), 3 if name.endswith(".xml") else 2, name)
        self.assertIn("<File>UI/JAC_SengokuReignPopup.xml</File>\n\t\t</AddUserInterfaces>", modinfo)
        # The era indicator replaces Gathering Storm's ActionPanel script (Expansion2.modinfo:366-371, no LoadOrder)
        # and loads it first.
        replace = modinfo.split('<ReplaceUIScript id="JAC_Sengoku_ActionPanel"')[1].split("</ReplaceUIScript>")[0]
        for part in ("<LoadOrder>100</LoadOrder>", "<LuaContext>ActionPanel</LuaContext>",
                     "<LuaReplace>UI/JAC_ActionPanel_Sengoku.lua</LuaReplace>"):
            self.assertIn(part, replace)
        source = (MOD / "UI/JAC_ActionPanel_Sengoku.lua").read_text(encoding="utf-8")
        self.assertLess(source.index('include("ActionPanel_Expansion2")'), source.index("BASE_RealizeEraIndicator ="))


class TitleLengthTests(unittest.TestCase):
    """Titles that must fit their frames in all three languages (live ages-screens-01 cut both)."""

    @staticmethod
    def width(text: str) -> int:
        # A wide (CJK) character takes about two Latin/Cyrillic small-caps cells.
        return sum(2 if unicodedata.east_asian_width(ch) in "WF" else 1 for ch in text)

    def texts(self, prefix: str, suffix: str) -> dict[tuple[str, str], str]:
        rows = [row for name in ("Text/JAC_Sengoku_Ages_Text.xml", "Text/JAC_Sengoku_Reigns_Text.xml")
                for row in ET.parse(MOD / name).getroot().findall("./LocalizedText/Row")]
        return {(r.get("Tag"), r.get("Language")): r.findtext("Text") for r in rows
                if r.get("Tag").startswith(prefix) and r.get("Tag").endswith(suffix)}

    def test_era_name_popup_title_is_two_short_lines(self):
        # One line was cut at 38 cells ("NEW ERA NAME: FROM TENBUN TO KŌJI (155"). The official era popup splits its
        # title the same way (LOC_ERA_REVIEW_POPUP_TITLE, DLC/Expansion1/Text/en_US/Expansion1_Eras_Text.xml:103-104).
        titles = self.texts("LOC_JAC_REIGN_", "_ANNOUNCEMENT")
        self.assertEqual(len(titles), len(REIGNS_JSON["reigns"]) * 3)
        for key, text in titles.items():
            lines = text.split("[NEWLINE]")
            self.assertEqual(len(lines), 2, key)
            self.assertLessEqual(max(map(self.width, lines)), 32, key)

    def test_era_score_panel_title_fits_the_header(self):
        # The header showed 16 cells before the ellipsis ("MERIT AND ERA NA…"); the official title is
        # "Era Progress" / "Ход эпохи" (LOC_ERA_PROGRESS_TITLE, DLC/Expansion1/Text/en_US/Expansion1_Eras_Text.xml:37-38).
        titles = self.texts("LOC_JAC_AGE_PANEL_TITLE", "")
        self.assertEqual(len(titles), 3)
        for key, text in titles.items():
            self.assertLessEqual(self.width(text), 15, key)


if __name__ == "__main__":
    unittest.main()
