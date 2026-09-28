-- 束0・束1 PoC：Unit:GetProperty だけを使う武将帳。
local WARLORD_KEY = "JAC_POC_GROWTHUI_WARLORD"
local isOpen = false

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
end

local function OpenLedger()
	Refresh()
	Controls.LedgerPanel:SetHide(false)
	isOpen = true
	print("[JAC_POC] ui opened")
end

local function CloseLedger()
	Controls.LedgerPanel:SetHide(true)
	isOpen = false
end

local function OnPlayerTurnActivated(playerID)
	if isOpen and playerID == Game.GetLocalPlayer() then Refresh() end
end

Controls.OpenLedger:RegisterCallback(Mouse.eLClick, OpenLedger)
Controls.CloseLedger:RegisterCallback(Mouse.eLClick, CloseLedger)
Events.PlayerTurnActivated.Add(OnPlayerTurnActivated)
