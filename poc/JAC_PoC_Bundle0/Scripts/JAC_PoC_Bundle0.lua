-- PoC 束0・束1：MOD の読み込みと、セーブ・ロード後の状態保持を確かめる
-- 結果は Lua.log に "[JAC_POC]" 付きで出力する

local PROPERTY_KEY = "JAC_POC_BUNDLE0_STATE"

local function Log(message)
	print("[JAC_POC] " .. message)
end

local function OnTurnBegin(turn)
	local state = Game:GetProperty(PROPERTY_KEY) or { FirstTurn = turn, TurnsSeen = 0 }
	state.TurnsSeen = state.TurnsSeen + 1
	state.LastTurn = turn
	Game:SetProperty(PROPERTY_KEY, state)
	Log(string.format("turn=%d firstTurn=%d turnsSeen=%d", turn, state.FirstTurn, state.TurnsSeen))
end

local function OnLoadScreenClose()
	local row = GameInfo.GlobalParameters["JAC_POC_BUNDLE0_VERSION"]
	Log("loaded version=" .. tostring(row and row.Value))

	local state = Game:GetProperty(PROPERTY_KEY)
	if state then
		Log(string.format("restored firstTurn=%d lastTurn=%d turnsSeen=%d", state.FirstTurn, state.LastTurn, state.TurnsSeen))
	else
		Log("no saved state (new game)")
	end
end

Events.LoadScreenClose.Add(OnLoadScreenClose)
Events.TurnBegin.Add(OnTurnBegin)
