-- 朝廷・将軍の停戦裁定（SSOT 3.2）の規則。ゲームプレイ側（Scripts/JAC_CourtArbitration.lua）と
-- UI 側（UI/JAC_CourtArbitrationPanel.lua）の両方が include する。UI は表示とボタンの可否だけに使い、
-- 状態の変更はゲームプレイ側だけが行う（公式：BlackDeathScenario.lua:3 が Rules を include する形）。
-- 両方の文脈にある API だけを使う（Players / Game / GameInfo / GameConfiguration / PlayerConfigurations）。
--
-- 数値はすべて暫定（documents/questions.md Q57〜Q60）。正当性は SSOT で未定義のため、
-- Player プロパティの数値を置き場として使う（効果なし。Q57）。

JAC_ARB = {}
local R = JAC_ARB

R.ASHIKAGA = "CIVILIZATION_JAC_ASHIKAGA"
R.COST_FAVOR = 20            -- 外交的支持（Favor）
R.COST_GOLD = 100
R.COST_LEGITIMACY = 5
R.LEGITIMACY_START = 50      -- 正当性の初期値（プロパティが無い間の値）
R.REFUSAL_LEGITIMACY = 15    -- 拒否した勢力の正当性の低下
R.OUTLAW_TURNS = 10          -- 拒否した勢力に「討伐の大義」が立つターン数（記録と通知のみ。Q58）
-- 停戦の再宣戦制限。公式の講和後の最短期間 DIPLOMACY_PEACE_MIN_TURNS と同じ値
-- （Base/Assets/Gameplay/Data/GlobalParameters.xml:248）。将軍家の裁定はこれが3倍になる。
R.TRUCE_TURNS = 10
R.SHOGUN_POWER = 3
-- AI の応諾（暫定の決定的規則）：自分の都市数が相手の 1.5 倍を超える（優勢）なら拒み、それ以外は応じる
R.AI_REFUSE_NUM, R.AI_REFUSE_DEN = 3, 2
-- 室町幕府の終わり：元亀4年7月（research/sengoku-events.md の EV_MUROMACHI_END）。和暦の月をそのまま使う
-- （poc/historical-events.md「暦との関係」と同じ）
R.MUROMACHI_END_YEAR, R.MUROMACHI_END_MONTH = 1573, 7

R.LEGITIMACY_KEY = "JAC_LEGITIMACY"                 -- Player
R.OUTLAW_KEY = "JAC_ARB_OUTLAW_UNTIL"               -- Player：討伐の大義が立つ最後のターン
R.PENDING_KEY = "JAC_ARB_PENDING"                   -- Game：審理中の請求（同時に1件）
R.LAST_REQUEST_KEY = "JAC_ARB_LAST_REQUEST_TURN"    -- Game：裁定は1ターンに1件まで
R.TRUCE_PREFIX = "JAC_ARB_TRUCE_"                   -- Game：<小さいID>_<大きいID> = 停戦が続く最後のターン

-- ゲーム開始からのターン（1始まり）。エンジンのターンは 1534 年開始で 16603 から始まる
-- （JAC_HistoricalEvents.lua の GameTurn と同じ。公式 TopPanel.lua:390 と同じ正規化）
function R.GameTurn()
	return Game.GetCurrentGameTurn() - GameConfiguration.GetStartTurn() + 1
end

-- 年月 → ターン。JAC_HistoricalEvents.lua の TurnForYear と同じ換算（GameSpeed_Turns を月数の長い順に歩く）
local STAGES = nil
function R.TurnForYear(year, month)
	if STAGES == nil then
		local speed = GameInfo.GameSpeeds[GameConfiguration.GetGameSpeedType()].GameSpeedType
		STAGES = {}
		for row in GameInfo.GameSpeed_Turns() do
			if row.GameSpeedType == speed then table.insert(STAGES, row) end
		end
		table.sort(STAGES, function(a, b)
			if a.MonthIncrement ~= b.MonthIncrement then return a.MonthIncrement > b.MonthIncrement end
			return a.TurnsPerIncrement < b.TurnsPerIncrement
		end)
	end
	local target = (year - GameInfo.StartEras["ERA_ANCIENT"].Year) * 12 + (month or 1) - 1
	local months, turn = 0, 1
	for _, row in ipairs(STAGES) do
		local span = row.TurnsPerIncrement * row.MonthIncrement
		if months + span >= target then
			return turn + math.max(0, math.ceil((target - months) / row.MonthIncrement))
		end
		months, turn = months + span, turn + row.TurnsPerIncrement
	end
	return turn
end

function R.ShogunateEndTurn()
	return R.TurnForYear(R.MUROMACHI_END_YEAR, R.MUROMACHI_END_MONTH)
end

function R.IsAshikaga(id)
	return PlayerConfigurations[id]:GetCivilizationTypeName() == R.ASHIKAGA
end

-- 将軍家が裁定の力を持つか（室町幕府の終わりのターンより前）。以後も勢力は残り、当事者としては請求できる
function R.IsEmpoweredShogun(id, turn)
	return R.IsAshikaga(id) and turn < R.ShogunateEndTurn()
end

function R.Legitimacy(id)
	return Players[id]:GetProperty(R.LEGITIMACY_KEY) or R.LEGITIMACY_START
end

-- 外交的支持（Favor）。文脈で置き場が違う（GameCore_XP2_FinalRelease.dll の Lua 登録）：
-- UI の Player は GetFavor（XP2_Cache_Lua_IPlayer.cpp。公式 TopPanel_Expansion2.lua:169）、
-- ゲームプレイ側は Player:GetDiplomacy() の GetFavor / ChangeFavor（XP2_Lua_IPlayerDiplomacy.cpp。公式 Lua に使用例なし）
function R.Favor(id)
	local player = Players[id]
	if player.GetFavor then return player:GetFavor() end
	return player:GetDiplomacy():GetFavor()
end

function R.TruceKey(a, b)
	if a > b then a, b = b, a end
	return R.TRUCE_PREFIX .. a .. "_" .. b
end

-- 表示名の LOC キー。史実の出来事の家名切替（JAC_CIV_NAME_<文明>、CustomCivs UI/JAC_CivNameOverride.lua）を優先する
function R.NameKey(id)
	local civ = PlayerConfigurations[id]:GetCivilizationTypeName()
	return Game:GetProperty("JAC_CIV_NAME_" .. civ) or PlayerConfigurations[id]:GetCivilizationShortDescription()
end

local function IsLiveMajor(id)
	local player = id ~= nil and Players[id] or nil
	return player ~= nil and player:IsMajor() and player:IsAlive()
end

-- 審理中の請求に対して id が答える欄（"AnswerA" / "AnswerB"）。答える立場でなければ nil。
-- 当事者の請求では A が請求者なので、答えるのは B だけ。将軍家の発議では A と B の両方
function R.AnswerKey(pending, id)
	if pending == nil then return nil end
	if id == pending.A and pending.Requester ~= pending.A then return "AnswerA" end
	if id == pending.B then return "AnswerB" end
	return nil
end

-- 請求できるか。できなければ false と理由（LOC_JAC_ARB_REASON_<理由> に対応）
function R.CheckRequest(requester, a, b)
	if not (IsLiveMajor(requester) and IsLiveMajor(a) and IsLiveMajor(b)) or a == b then return false, "INVALID" end
	if not Players[a]:GetDiplomacy():IsAtWarWith(b) then return false, "NOT_AT_WAR" end
	local turn = R.GameTurn()
	if requester ~= a and requester ~= b and not R.IsEmpoweredShogun(requester, turn) then return false, "NOT_PARTY" end
	if Game:GetProperty(R.PENDING_KEY) ~= nil or Game:GetProperty(R.LAST_REQUEST_KEY) == turn then
		return false, "ONE_PER_TURN"
	end
	local player = Players[requester]
	if R.Favor(requester) < R.COST_FAVOR then return false, "FAVOR" end
	if player:GetTreasury():GetGoldBalance() < R.COST_GOLD then return false, "GOLD" end
	if R.Legitimacy(requester) < R.COST_LEGITIMACY then return false, "LEGITIMACY" end
	return true
end

-- AI の応諾（暫定）。自分の都市数が相手の 1.5 倍を超えるときだけ拒む
function R.AiAccepts(id, opponent)
	local mine, theirs = Players[id]:GetCities():GetCount(), Players[opponent]:GetCities():GetCount()
	return mine * R.AI_REFUSE_DEN <= theirs * R.AI_REFUSE_NUM
end
