-- 束0・束1 PoC：手帳（Notebooks）画面。人物帳は Unit:GetProperty だけを読む。
-- 開閉は ReportsList.lua、PartialScreens への移動は EraProgressPanel.lua と同じ型。
include("SupportFunctions") -- TruncateStringWithTooltip

local ID = "JAC_Notebooks" -- JAC_PartialScreenHooks.lua のフック名と揃える
local WARLORD_KEY = "JAC_POC_GROWTHUI_WARLORD"
local MENU_SIZE_PADDING = 54 -- ReportsList.lua の BACKGROUND_SIZE_PADDING

local rows = {
	{ root = Controls.UnitRow1, title = Controls.Unit1Title,
		progress = Controls.Unit1Progress, attributes = Controls.Unit1Attributes,
		equipment = Controls.Unit1Equipment, artifact = Controls.Unit1Artifact },
	{ root = Controls.UnitRow2, title = Controls.Unit2Title,
		progress = Controls.Unit2Progress, attributes = Controls.Unit2Attributes,
		equipment = Controls.Unit2Equipment, artifact = Controls.Unit2Artifact },
	{ root = Controls.UnitRow3, title = Controls.Unit3Title,
		progress = Controls.Unit3Progress, attributes = Controls.Unit3Attributes,
		equipment = Controls.Unit3Equipment, artifact = Controls.Unit3Artifact }
}

-- 未実装の項目。押せないボタンに「（未実装）」を付けて並べる。
local PLACEHOLDERS = {
	{ Controls.LawJournalButton, "LOC_JAC_NOTEBOOKS_LAW_JOURNAL" },
	{ Controls.ReligionJournalButton, "LOC_JAC_NOTEBOOKS_RELIGION_JOURNAL" },
	{ Controls.SecretariesButton, "LOC_JAC_NOTEBOOKS_SECRETARIES" },
	{ Controls.VaultButton, "LOC_JAC_NOTEBOOKS_VAULT" }
}

local function Label(key)
	return Locale.Lookup("LOC_JAC_POC_GROWTHUI_" .. key)
end

local function NameFromID(id)
	return Locale.Lookup("LOC_" .. id)
end

local function Refresh()
	local warlords = {}
	local playerID = Game.GetLocalPlayer()
	local player = playerID ~= -1 and Players[playerID] or nil
	if player then
		for _, unit in player:GetUnits():Members() do
			local record = unit:GetProperty(WARLORD_KEY)
			if record then
				table.insert(warlords, { unit = unit, record = record })
			end
		end
	end
	table.sort(warlords, function(a, b) return a.unit:GetID() < b.unit:GetID() end)
	Controls.NoUnits:SetHide(#warlords ~= 0)

	for index, row in ipairs(rows) do
		local entry = warlords[index]
		row.root:SetHide(entry == nil)
		if entry then
			local unit = entry.unit
			local record = entry.record
			local unitInfo = GameInfo.Units[unit:GetUnitType()]
			local unitName = unitInfo and Locale.Lookup(unitInfo.Name) or tostring(unit:GetUnitType())
			row.title:SetText(Label("UNIT") .. " " .. unit:GetID() .. ": " .. unitName)
			row.progress:SetText(Label("LEVEL") .. " " .. record.Level .. "   " .. Label("XP") .. " " .. record.XP)
			row.attributes:SetText(Label("PERSONALITY") .. ": " .. NameFromID(record.Personality)
				.. "   " .. Label("TRAIT") .. ": " .. NameFromID(record.Trait))
			local equipment = #record.Equipment > 0 and table.concat(record.Equipment, ", ") or Label("NONE")
			row.equipment:SetText(Label("EQUIPMENT") .. ": " .. equipment)
			row.artifact:SetText(Label("ARTIFACT") .. ": " .. (record.Artifact or Label("NONE")))
		end
	end
	return #warlords
end

local function ToggleCharacterJournal()
	if not Controls.JournalPanel:IsHidden() then
		Controls.JournalPanel:SetHide(true)
		return
	end
	local count = Refresh()
	Controls.JournalPanel:SetHide(false)
	print("[JAC_POC] character journal shown count=" .. count)
end

local function Open()
	-- 初期化中は PartialScreens を引けないことがあるため、開くときに移す（EraProgressPanel.lua と同じ）。
	if ContextPtr:GetParent():GetID() ~= "PartialScreens" then
		local partialScreens = ContextPtr:LookUpControl("/InGame/PartialScreens")
		ContextPtr:ChangeParent(partialScreens)
		partialScreens:AddChildAtIndex(ContextPtr, 6) -- PartialScreenHooks より下に置く
		-- 公式の IsPartialScreenOpen() と同じパスで、移動後も見つかるかを実機で確かめる。
		local found = ContextPtr:LookUpControl("/InGame/" .. ID) ~= nil
			or ContextPtr:LookUpControl("/InGame/AdditionalUserInterfaces/" .. ID) ~= nil
		print("[JAC_POC] notebooks path lookup found=" .. tostring(found))
	end
	Controls.MenuStack:CalculateSize()
	Controls.MenuBackground:SetSizeY(Controls.MenuStack:GetSizeY() + MENU_SIZE_PADDING)
	LuaEvents.MinimapPanel_CloseAllLenses()
	ContextPtr:SetHide(false)
	UI.PlaySound("CityStates_Panel_Open")
	LuaEvents.JAC_Notebooks_Opened()
	print("[JAC_POC] notebooks opened")
end

local function Close()
	if not ContextPtr:IsHidden() then
		UI.PlaySound("CityStates_Panel_Close")
	end
	Controls.JournalPanel:SetHide(true)
	ContextPtr:SetHide(true)
	LuaEvents.JAC_Notebooks_Closed()
end

local function OnCloseAllExcept(contextToStayOpen)
	if contextToStayOpen ~= ID then Close() end
end

local function OnInputHandler(input)
	if input:GetMessageType() == KeyEvents.KeyUp and input:GetKey() == Keys.VK_ESCAPE then
		Close()
		return true
	end
	return false
end

local function OnPlayerTurnActivated(playerID)
	if not Controls.JournalPanel:IsHidden() and playerID == Game.GetLocalPlayer() then Refresh() end
end

for _, placeholder in ipairs(PLACEHOLDERS) do
	local button, key = placeholder[1], placeholder[2]
	-- ボタン幅を超える表記（ロシア語など）は「…」で切り、実機で分かるようにログを出す。
	local text = Locale.Lookup("LOC_JAC_NOTEBOOKS_NOT_YET", Locale.Lookup(key))
	if TruncateStringWithTooltip(button, button:GetSizeX() - 12, text) then
		print("[JAC_POC] notebooks label truncated key=" .. key)
	end
end
Controls.CharacterJournalButton:RegisterCallback(Mouse.eLClick, ToggleCharacterJournal)
Controls.CloseJournal:RegisterCallback(Mouse.eLClick, function() Controls.JournalPanel:SetHide(true) end)
ContextPtr:SetInputHandler(OnInputHandler, true)
LuaEvents.JAC_Notebooks_Open.Add(Open)
LuaEvents.JAC_Notebooks_Close.Add(Close)
LuaEvents.PartialScreenHooks_CloseAllExcept.Add(OnCloseAllExcept)
Events.PlayerTurnActivated.Add(OnPlayerTurnActivated)
ContextPtr:SetHide(true) -- AddUserInterfaces の Context には Hidden を書けないため
