-- 改元の画面（documents/research/sengoku-ages.md §8）。改元のターンに、自分の家の判定（元号と年、黄金期・英雄期・
-- 通常期・暗黒期、武功と閾値、年齢の効果）を出す。台帳（Game プロパティ JAC_AGES_LEDGER）を読むだけで書かない。
-- 見た目は公式の時代の振り返り（DLC/Expansion2/UI/Additions/EraReviewPopup.lua:62-106・.xml）の年齢ごとの絵。
-- 出す時機は公式の時代の画面と同じ自分の手番の始まり（EraCompletePopup.lua:268-274、Events.LocalPlayerTurnBegin）と、
-- 自動進行の終わり（LuaEvents.AutoPlayEnd。公式 Base/Assets/UI/ActionPanel.lua:1350「Raised by engine AutoPlay_Manager」）。
-- 1つの改元はこのセッションで1回だけ出す。読み込み直後には出さない（公式の EraCompletePopup と同じ）。
-- 公式と違い UIManager:QueuePopup も入力の独占もしない（DisloyalCityChooser.lua と同じ ContextPtr:SetHide の出し入れ）ので、
-- 自動進行は止まらない。自動テストは OnClose() で閉じる（tools/civ6-autotest/scenario.py の POPUP_CLOSERS）。
-- 暗黒期の政策の選択画面（UI/JAC_SengokuDarkPolicy.lua）とは LuaEvents で順番を決める：この画面が出ている間は選択画面を
-- 出さず、閉じたら選択画面が（選択待ちなら）出る。暗黒期で選択待ちの家には、閉じるボタンが「暗黒期の政策を選ぶ」になる。
include("JAC_HouseNames")
JAC_InstallHouseNames("JAC_SengokuReignPopup")
include("JAC_SengokuAgesView")

local m_ShownReign = -1

-- 自動テストが確かめる、最後に出した改元（閉じた後も残る）。
JAC_REIGN_POPUP_LAST = nil

local function SetLocalPlayerIcon(playerID)
	-- 公式 EraReviewPopup.lua:115-124（PopulateLocalPlayerIcon）と同じ。
	local config = PlayerConfigurations[playerID]
	if config == nil then return end
	local backColor, frontColor = UI.GetPlayerColors(playerID)
	Controls.CivIndicator:SetColor(backColor)
	Controls.CivIcon:SetColor(frontColor)
	Controls.CivIcon:SetIcon("ICON_" .. config:GetCivilizationTypeName())
	Controls.LeaderPortrait:SetIcon("ICON_" .. config:GetLeaderTypeName())
end

local function Show(view, playerID)
	local style = JAC_AgesView_Style(view.age)
	local heroic = view.age == "HEROIC"
	Controls.Title:SetText(Locale.ToUpper(Locale.Lookup(view.reign.Announcement)))
	-- 「改元」「天文から弘治へ（1555年）」の2行（[NEWLINE]）。1行では英語・ロシア語が枠の幅を超えて切れる（実機 ages-screens-01）。
	-- 公式も同じ見出しを LOC_ERA_REVIEW_POPUP_TITLE の [NEWLINE] で2行にする（DLC/Expansion1/Text/en_US/Expansion1_Eras_Text.xml:103-104）
	Controls.EraRibbon:SetTexture("Ages_BannerLong" .. style.art)
	Controls.Background:SetTexture("Ages_Parchment" .. style.art)
	Controls.WindowFrame:SetTexture("Ages_Frame" .. style.art)
	Controls.EraRibbonText:SetColorByName(heroic and "Black" or "White_Black")
	Controls.EraRibbonValue:SetColorByName(heroic and "Black" or "White_Black")
	Controls.HeroicFlare:SetHide(not heroic)
	Controls.EraRibbonValue:SetText(JAC_AgesView_AgeLabel(view.age))
	Controls.Score:SetText(Locale.Lookup("LOC_JAC_AGE_REIGN_SCORE", view.previous.Name, view.score))
	Controls.Thresholds:SetText(JAC_AgesView_ThresholdText(view.golden, view.dark))
	Controls.EraEffects:SetText(JAC_AgesView_EffectText(view.age, view.policy)
		.. (view.oathText and ("[NEWLINE]" .. view.oathText) or ""))
	Controls.Continue:SetText(Locale.Lookup(view.pendingOath and "LOC_JAC_OATH_CHOOSE"
		or (view.pendingChoice and "LOC_JAC_AGE_REIGN_CHOOSE_POLICY" or "LOC_CONTINUE")))
	SetLocalPlayerIcon(playerID)
	Controls.MainStack:CalculateSize()
	JAC_REIGN_POPUP_LAST = { reign = view.index, reignType = view.reign.ReignType, age = view.age, score = view.score,
		turn = view.turn, pendingChoice = view.pendingChoice }
	ContextPtr:SetHide(false)
	UI.PlaySound("UI_Era_Change")  -- 公式 EraCompletePopup.lua:120
	LuaEvents.JAC_SengokuReignPopup_Shown()
end

-- 今のターンに判定した改元を出す。reopen = true なら判定のターンでなくても、出した後でも、最後の改元を出し直す。
function JAC_ShowReignPopup(reopen)
	local playerID = Game.GetLocalPlayer()
	if playerID == nil or playerID < 0 then return false end
	local view = JAC_AgesView_Reign(JAC_AgesView_Ledger(), playerID, (not reopen) and JAC_AgesView_ElapsedTurn() or nil)
	if view == nil or ((not reopen) and view.index == m_ShownReign) then return false end
	m_ShownReign = view.index
	Show(view, playerID)
	return true
end

-- showChoice: 閉じた後に暗黒期の政策の選択画面を（選択待ちなら）出すか。手番の終わりでは出さない（手番の外では送れない）。
local function Hide(showChoice)
	if ContextPtr:IsHidden() then return end
	ContextPtr:SetHide(true)
	LuaEvents.JAC_SengokuReignPopup_Closed(showChoice)
end

function OnClose() Hide(true) end

local function OnInput(input)
	if ContextPtr:IsHidden() then return false end
	if input:GetMessageType() == KeyEvents.KeyUp and input:GetKey() == Keys.VK_ESCAPE then
		OnClose()
		return true
	end
	return false  -- 他の入力は奪わない
end

local function OnTurnBegin() JAC_ShowReignPopup(false) end

ContextPtr:SetHide(true)
ContextPtr:SetInputHandler(OnInput, true)
Controls.Close:RegisterCallback(Mouse.eLClick, OnClose)
Controls.Continue:RegisterCallback(Mouse.eLClick, OnClose)
Events.LocalPlayerTurnBegin.Add(OnTurnBegin)
LuaEvents.AutoPlayEnd.Add(OnTurnBegin)
Events.LocalPlayerTurnEnd.Add(function() Hide(false) end)  -- 公式 ActionPanel.lua:1335。次の手番へ持ち越さない
