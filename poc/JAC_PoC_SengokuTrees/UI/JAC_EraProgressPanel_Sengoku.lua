-- 時代スコアの画面（documents/research/sengoku-ages.md §8）。家固有名の差し替え
-- poc/JAC_PoC_BanseikiRuleset/UI/JAC_EraProgressPanel.lua が公式の EraProgressPanel を読んだ後に、ワイルドカード
-- include("JAC_EraProgressPanel_", true) で読む（別の差し替えを作って競合させない。§8）。同じコンテキストなので、
-- 家固有名の Locale の包みもそのまま効く。
-- 台帳（Game プロパティ JAC_AGES_LEDGER）がある間は、公式の Refresh（DLC/Expansion2/UI/Additions/EraProgressPanel.lua:212-270）の
-- 後で、公式の時代スコアと年齢（止めてあり、全家が常に通常）の部分を台帳の中身に置き換える。読むだけで書かない。
-- 入口は公式のまま（上部の時代のボタン PartialScreenHooks_Expansion1.lua:40-56、Open が Refresh を呼ぶ :387-407）。
-- 置き換える欄：題、時代の名前（今の元号。オーナー判断 L-1）、次の改元までのターン、今の期の武功、自分の家の年齢（帯）、他の家の年齢、
-- 次の改元の閾値と今の見込み、下段（年齢の効果・今の期の内訳・過去の期）。
include("JAC_SengokuAgesView")

local BASE_Refresh = Refresh

-- 自動テストが確かめる、最後に出した中身。
JAC_AGES_PANEL_VIEW = nil

local function SetCheck(control, on, age)
	control:SetHide(not on)
	if on then control:SetToolTipString(Locale.Lookup("LOC_ERAS_YOU_WILL_EARN_BLANK_AGE_NEXT_ERA", JAC_AgesView_Style(age).name)) end
end

function Refresh()
	BASE_Refresh()
	local playerID = Game.GetLocalPlayer()
	local l = JAC_AgesView_Ledger()
	local view = l and playerID ~= nil and playerID >= 0 and JAC_AgesView_Panel(l, playerID, JAC_AgesView_ElapsedTurn()) or nil
	JAC_AGES_PANEL_VIEW = view
	if view == nil then return end

	Controls.Title:SetText(Locale.ToUpper(Locale.Lookup("LOC_JAC_AGE_PANEL_TITLE")))
	Controls.EraName:SetText(Locale.Lookup("LOC_JAC_AGE_PANEL_PERIOD", view.reign.Name, tostring(view.reign.Year)))
	Controls.TimeToNextEraContainer:SetHide(view.turnsLeft == nil)
	if view.turnsLeft ~= nil then Controls.TimeToNextEra:SetText(tostring(view.turnsLeft)) end
	Controls.EraScoreValue:SetText(tostring(view.score))
	Controls.EraRibbon:SetTexture(JAC_AgesView_Style(view.age).banner)
	Controls.EraRibbonValue:SetText(JAC_AgesView_AgeLabel(view.age))

	Controls.ThresholdStack:SetHide(view.golden == nil)
	if view.golden ~= nil then
		-- 公式と同じ区切り（:253-262）：暗黒は閾値未満、通常は暗黒の閾値から黄金の閾値未満、黄金は閾値以上。
		Controls.DarkThreshold:SetText(view.dark > 0 and ("0 - " .. tostring(view.dark - 1)) or "-")
		Controls.NormalThreshold:SetText(tostring(view.dark) .. " - " .. tostring(view.golden - 1))
		Controls.GoldenThreshold:SetText(tostring(view.golden) .. "+")
		Controls.GoldenLabel:SetText(Locale.Lookup(view.age == "DARK" and "LOC_ERA_REVIEW_HAVE_HEROIC_AGE_LABEL"
			or "LOC_ERA_REVIEW_HAVE_GOLDEN_AGE_LABEL"))
		Controls.DarkThreshold:SetToolTipString("")
		Controls.GoldenThreshold:SetToolTipString("")
		Controls.NormalAgeThreshold:SetHide(false)
		SetCheck(Controls.GoldenCheckmark, view.projected == "GOLDEN" or view.projected == "HEROIC", view.projected)
		SetCheck(Controls.NormalCheckmark, view.projected == "NORMAL", view.projected)
		SetCheck(Controls.DarkCheckmark, view.projected == "DARK", view.projected)
	end

	Controls.EraEffectsHeader:SetText(Locale.Lookup("LOC_JAC_AGE_PANEL_LEDGER_HEADER"))
	Controls.EraEffects:SetText(JAC_AgesView_LedgerText(view))

	-- 他の家の年齢：公式の一覧（PopulateCivilizationIcons、:273-321）を、この呼び出しの間だけ Game.GetEras() を台帳の
	-- 年齢に替えて作り直す（JAC_PoC_BanseikiRuleset/UI/JAC_HistoricMoments.lua と同じく、このコンテキストのグローバルだけ）。
	local engineGame = Game
	Game = setmetatable({ GetEras = function() return JAC_AgesView_LedgerEras(l) end }, { __index = engineGame })
	local ok, message = pcall(PopulateCivilizationIcons)
	Game = engineGame
	if not ok then print("[JAC_AGES] era panel icons failed: " .. tostring(message)) end
end
