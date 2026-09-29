-- Owner decision E5a (SSOT 3.1): an event that recommends an action (reconciliation etc.) tells the human with an
-- advisor popup, next to the event's boost. The popup is the official one: Base/Assets/UI/Popups/AdvisorPopup.lua
-- (ShowAdvisorPopup, :94) listens to LuaEvents.TutorialUIRoot_AdvisorRaise (:521) and takes an AdvisorItem
-- (:25-38), which the tutorial builds with hmake (Base/Assets/UI/TutorialUIRoot.lua:216-229) and raises
-- (:1565). The context lives in TutorialUIRoot.xml:21 and is loaded in normal games too; the autotest driver
-- already closes it (tools/civ6-autotest/scenario.py _dismiss_advisor: OnHideAdvisorDialog when IsBlockingInput).
--
-- Only the local player's popups are shown, so AI factions and other machines see nothing. The gameplay script
-- (Scripts/JAC_HistoricalEvents.lua) writes the turn an event happened for a player (JAC_EVENT_FIRED_ /
-- JAC_EVENT_BOOSTED_); this reads them and shows the popup on that turn. Nothing is written back.

-- Same declaration as AdvisorPopup.lua:25-38 and TutorialUIRoot.lua:38-52 (the official FIXME: the declaration
-- is duplicated in every file that builds one and must stay in sync).
hstructure AdvisorItem
	Message			: string;		-- TXT key to look up for advisor message (if raised via an advisor)
	MessageAudio	: string;		-- Name of the accompanying audio to play with the message.
	Image			: string;		-- (optional) Name of texture used in image.
	OptionsNum		: number;		-- Number of options.
	Button1Text		: string;		-- TXT key to look up for button 1
	Button2Text		: string;		-- " " " 2
	Button1Func		: ifunction;	-- Callback on button 1
	Button2Func		: ifunction;	-- " " " 2
	CalloutHeader	: string;		-- TXT key to look up for callout header
	CalloutBody		: string;		-- TXT key to look up for callout body
	PlotCallback	: ifunction;	-- Function to return the ID of a world plot to which dialog will be anchored
	ShowPortrait	: boolean;		-- Whether or not the advisor portrait should appear in the dialog (expected when VO is played)
	UITriggers		: table;		-- IDs and/or Trigger names for the UI when advisor item is up.
end

local FIRED_PREFIX = "JAC_EVENT_FIRED_"
local BOOSTED_PREFIX = "JAC_EVENT_BOOSTED_"

-- Popups raised in this session ("EventType@turn#player"). The player is part of the key so that in hotseat the
-- next human still gets their own popup for the same event on the same turn. Also read by the live scenario.
JAC_EVENTS_ADVISOR_SHOWN = {}

-- Turn the event first happened for the player: the boost-only year (BOOSTED) or the effects (FIRED).
local function HappenedTurn(player, eventType)
	local fired = player:GetProperty(FIRED_PREFIX .. eventType)
	local boosted = player:GetProperty(BOOSTED_PREFIX .. eventType)
	if fired and boosted then return math.min(fired, boosted) end
	return fired or boosted
end

local function ShowDueAdvisors()
	local playerID = Game.GetLocalPlayer()
	if playerID == nil or playerID < 0 then return end  -- autoplay observer: nobody to show it to
	local player = Players[playerID]
	local civType = PlayerConfigurations[playerID]:GetCivilizationTypeName()
	-- Game-relative turn, as the gameplay script and the official top panel count it (TopPanel.lua:390).
	local turn = Game.GetCurrentGameTurn() - GameConfiguration.GetStartTurn() + 1
	for row in GameInfo.JAC_HistoricalEvents() do
		local key = row.EventType .. "@" .. turn .. "#" .. playerID
		if (row.Advisor == true or row.Advisor == 1) and not JAC_EVENTS_ADVISOR_SHOWN[key]
				and HappenedTurn(player, row.EventType) == turn then
			JAC_EVENTS_ADVISOR_SHOWN[key] = true
			local name = row.Name
			if row.House and civType ~= row.House then name = row.GenericName end
			LuaEvents.TutorialUIRoot_AdvisorRaise(hmake AdvisorItem {
				Message = Locale.Lookup(name) .. "[NEWLINE][NEWLINE]" .. Locale.Lookup(row.Description),
				MessageAudio = nil,
				OptionsNum = 1,
				Button1Text = Locale.Lookup("LOC_JAC_EVENT_ADVISOR_OK"),
				Button2Text = nil,
				-- A callback of our own keeps the tutorial out of it (a nil one would raise AdvisorPopup_ClearActive).
				Button1Func = function() end,
				Button2Func = nil,
				CalloutHeader = "",
				CalloutBody = "",
				PlotCallback = nil,
				ShowPortrait = true,
				UITriggers = {},
			})
			print(string.format("[JAC_EVENTS_UI] advisor event=%s player=%d turn=%d", row.EventType, playerID, turn))
		end
	end
end

-- The local turn begins after the gameplay turn start that writes the flags; when autoplay hands the game back,
-- the local player changes instead (official panels listen to both: Base/Assets/UI/ActionPanel.lua:1333-1334).
Events.LocalPlayerTurnBegin.Add(ShowDueAdvisors)
Events.LocalPlayerChanged.Add(ShowDueAdvisors)
