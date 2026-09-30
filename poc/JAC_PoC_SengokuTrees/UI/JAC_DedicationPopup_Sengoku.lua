-- The official DedicationPopup context/XML, with ledger-based house choices.
-- Layout/art/InstanceManager: DLC/Expansion2/UI/Additions/DedicationPopup.xml:6-61,
-- .lua:25,79-99,110-156. The engine's commemorations and age APIs stay disabled.
-- Operation: DLC/BlackDeathScenario/UI/BlackDeathScenario_PopupHandler.lua:21-28.
-- No gameplay Lua is included here and no UI-side Game/Player property is written.
include("InstanceManager")
include("JAC_HouseNames")
JAC_InstallHouseNames("DedicationPopup")
include("JAC_SengokuAgesView")

local m_Rows = InstanceManager:new("Commemoration", "SelectCheck", Controls.CommemorationsStack)
local m_Choices, m_Selected, m_Reign, m_Allowed = {}, {}, nil, 1
local m_PlayerID, m_Civ, m_Age = nil, nil, nil
local m_ReignPopupOpen, m_Dismissed = false, nil
JAC_OATH_POPUP_LAST = nil

local function State()
	local l = JAC_AgesView_Ledger()
	local id = Game.GetLocalPlayer()
	local h = l and l.houses and l.houses["p" .. tostring(id)]
	return l, h, Players[id]
end

function JAC_OathsUI_Choices(house)
	local allowed, rows = {}, {}
	for row in GameInfo.JAC_SengokuOathAges() do
		if row.AgeType == house.age then allowed[row.OathType] = true end
	end
	for row in GameInfo.JAC_SengokuOaths() do
		if row.CivilizationType == house.civ and allowed[row.OathType] then rows[#rows + 1] = row end
	end
	table.sort(rows, function(a, b) return a.Position < b.Position end)
	return rows
end

local function Pending()
	local l, house, player = State()
	return l and player and player:IsHuman() and l.reign > 0 and house
		and house.oathReign == l.reign and house.oathChosen ~= l.reign
		and #JAC_OathsUI_Choices(house) > 0
end

local function SelectedCount()
	local n = 0
	for _ in pairs(m_Selected) do n = n + 1 end
	return n
end

local function RealizeSelection()
	for _, row in ipairs(m_Rows.m_AllocatedInstances) do row.SelectCheck:SetSelected(m_Selected[row.OathType] == true) end
	Controls.Confirm:SetDisabled(SelectedCount() ~= m_Allowed)
end

function JAC_SelectOath(oath)
	local offered = false
	for _, row in ipairs(m_Choices) do if row.OathType == oath then offered = true end end
	if not offered then return false end
	if m_Selected[oath] then
		m_Selected[oath] = nil
	else
		if SelectedCount() >= m_Allowed then m_Selected = {} end
		m_Selected[oath] = true
	end
	RealizeSelection()
	return true
end

function OnClose()
	m_Dismissed = m_Reign
	ContextPtr:SetHide(true)
	LuaEvents.JAC_SengokuOaths_Closed()
end

function JAC_ConfirmOaths()
	local l, house, player = State()
	if not Pending() or l.reign ~= m_Reign or not player:IsTurnActive() or SelectedCount() ~= m_Allowed then return false end
	local params = { OnStart = "JAC_SengokuOathsChoose", Reign = m_Reign }
	for i, row in ipairs(m_Choices) do if m_Selected[row.OathType] then params["Oath" .. i] = row.OathType end end
	UI.RequestPlayerOperation(Game.GetLocalPlayer(), PlayerOperations.EXECUTE_SCRIPT, params)
	UI.PlaySound("Confirm_Dedication")
	OnClose()
	return true
end

-- Scenario entry point uses the same selection and confirm paths as the buttons.
function JAC_ChooseOath(oath)
	if m_Allowed ~= 1 then return false end
	m_Selected = {}
	return JAC_SelectOath(oath) and JAC_ConfirmOaths()
end

-- Read the ACTUAL rendered rows, so the live test catches foreign/extra choices and missing names.
function JAC_GetOfferedOaths()
	local rows = {}
	for _, instance in ipairs(m_Rows.m_AllocatedInstances) do
		rows[#rows + 1] = { type = instance.OathType, name = instance.MomentCategory:GetText(),
			description = instance.MomentBonuses:GetText() }
	end
	return rows
end

function JAC_ShowOaths(reopen)
	local l, house, player = State()
	if not Pending() then
		if not ContextPtr:IsHidden() then OnClose() end
		return false
	end
	-- A loaded save may publish LoadScreenClose before the human turn is active. Show the pending
	-- choice then too; JAC_ConfirmOaths guards the actual operation and keeps the screen open if early.
	if m_ReignPopupOpen
		or (not reopen and m_Dismissed == l.reign) then return false end
	-- Selection belongs to a player/civilization/reign/age, independent of refresh events.
	-- Keep still-valid choices when LoadScreenClose and LocalPlayerTurnBegin refresh the same view.
	local playerID = Game.GetLocalPlayer()
	local sameView = m_PlayerID == playerID and m_Civ == house.civ and m_Reign == l.reign and m_Age == house.age
	local previous = sameView and m_Selected or {}
	m_Choices, m_Selected, m_Reign = JAC_OathsUI_Choices(house), {}, l.reign
	m_PlayerID, m_Civ, m_Age = playerID, house.civ, house.age
	m_Allowed = house.age == "HEROIC" and 3 or 1
	local reign = JAC_AgesView_Reigns()[l.reign]
	local style = JAC_AgesView_Style(house.age)
	Controls.Title:SetText(Locale.Lookup("LOC_JAC_OATH_TITLE", JAC_AgesView_EraName(reign)))
	Controls.AgeAchieved:SetText(Locale.Lookup("LOC_JAC_OATH_AGE", Locale.Lookup(style.name), m_Allowed))
	Controls.PopupBackground:SetTexture("Ages_Parchment" .. style.art)
	Controls.PopupFrame:SetTexture("Ages_Frame" .. style.art)
	Controls.HeroicFrameGlow:SetHide(house.age ~= "HEROIC")
	m_Rows:ResetInstances()
	for _, row in ipairs(m_Choices) do
		local instance = m_Rows:GetInstance()
		instance.OathType = row.OathType
		instance.CommemorationIcon:SetIcon(row.Icon)
		instance.SelectCheck:SetTexture("Ages_ButtonCom" .. style.art)
		instance.MomentCategory:SetText(Locale.Lookup(row.Name))
		instance.MomentBonuses:SetText(Locale.Lookup((house.age == "GOLDEN" or house.age == "HEROIC")
			and row.GoldenDescription or row.QuestDescription))
		instance.SelectCheck:RegisterCallback(Mouse.eLClick, function() JAC_SelectOath(row.OathType) end)
		if previous[row.OathType] or (not sameView and m_Allowed == 3) then m_Selected[row.OathType] = true end
	end
	RealizeSelection()
	Controls.CommemorationsStack:CalculateSize()
	Controls.CommemorationsScroller:CalculateSize()
	Controls.CommemorationsScroller:SetScrollValue(0)
	Controls.Confirm:SetText(Locale.Lookup("LOC_JAC_OATH_CHOOSE"))
	Controls.CloseButton:SetToolTipString(Locale.Lookup("LOC_JAC_OATH_PENDING"))
	JAC_OATH_POPUP_LAST = { reign = m_Reign, age = house.age, allowed = m_Allowed }
	ContextPtr:SetHide(false) -- same non-blocking display as the live reign/dark-policy popups
	LuaEvents.JAC_SengokuOaths_Shown()
	return true
end

local function OnTurnBegin()
	m_Dismissed = nil
	JAC_ShowOaths(false)
end

ContextPtr:SetHide(true)
ContextPtr:SetInputHandler(function(input)
	if ContextPtr:IsHidden() then return false end
	if input:GetMessageType() == KeyEvents.KeyUp and input:GetKey() == Keys.VK_ESCAPE then OnClose(); return true end
	return false
end, true)
Controls.CloseButton:RegisterCallback(Mouse.eLClick, OnClose)
Controls.Confirm:RegisterCallback(Mouse.eLClick, JAC_ConfirmOaths)
Events.LocalPlayerTurnBegin.Add(OnTurnBegin)
Events.LoadScreenClose.Add(function() JAC_ShowOaths(false) end)
Events.LocalPlayerTurnEnd.Add(OnClose)
LuaEvents.AutoPlayEnd.Add(function() JAC_ShowOaths(false) end)
LuaEvents.JAC_SengokuReignPopup_Shown.Add(function() m_ReignPopupOpen = true; OnClose() end)
LuaEvents.JAC_SengokuReignPopup_Closed.Add(function(showChoice)
	m_ReignPopupOpen = false
	if showChoice then JAC_ShowOaths(true) end
end)
