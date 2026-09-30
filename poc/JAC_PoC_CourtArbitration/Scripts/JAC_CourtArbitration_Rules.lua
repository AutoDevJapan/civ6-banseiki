-- 朝廷・将軍の停戦裁定（SSOT 3.2）の規則。ゲームプレイ側（Scripts/JAC_CourtArbitration.lua）と
-- UI 側（UI/JAC_CourtArbitrationPanel.lua）の両方が include する。UI は表示とボタンの可否だけに使い、
-- 状態の変更はゲームプレイ側だけが行う（公式：BlackDeathScenario.lua:3 が Rules を include する形）。
-- 両方の文脈にある API だけを使う（Players / Game / GameInfo / GameConfiguration / PlayerConfigurations / Map）。
--
-- 数値はすべて暫定で、実機の約200ターンの進行で調整する（オーナー判断 O-3、2026-09-30）。
-- 正当性は家ごとの 0〜100 の数値で、日本マップでは外交的支持（Favor）の代わりに裁定の通貨にする（オーナー判断 R-1）。
-- 拒んだ家への討伐の勅命はオーナー判断 R-2、将軍家の3倍（費用は3分の1、不利益は3倍）はオーナー判断 R-3（2026-09-30）。

JAC_ARB = {}
local R = JAC_ARB

R.ASHIKAGA = "CIVILIZATION_JAC_ASHIKAGA"
R.COST_GOLD = 100
R.COST_LEGITIMACY = 5
-- 正当性（R-1）。0〜100、初期値 50（足利将軍家は 70）。プロパティが無い間は初期値として読む
R.LEGITIMACY_MIN, R.LEGITIMACY_MAX = 0, 100
R.LEGITIMACY_START, R.LEGITIMACY_START_ASHIKAGA = 50, 70
R.LEGITIMACY_HIGH = 70       -- 以上：全都市の金と文化 +10%（Data/JAC_CourtArbitration_Legitimacy.sql）
R.LEGITIMACY_LOW = 30        -- 未満：全都市の金と文化 -10%、裁定を請求できない、AI が裁定を拒みやすい
R.ACCEPT_LEGITIMACY = 5      -- 裁定に応じた家の正当性の上昇
R.REFUSAL_LEGITIMACY = 15    -- 裁定を拒んだ家の正当性の低下
R.OUTLAW_TURNS = 10          -- 拒んだ家を対象にする討伐の勅命のターン数
-- 討伐の勅命（R-2）：期間中に対象の都市を攻め取った参加者への褒賞と、対象の正当性の低下（都市1つごと）
R.SUBJUGATION_GOLD = 200
R.SUBJUGATION_LEGITIMACY = 10
-- AI が自分から参加する「隣接」の目安：自分の都市と対象の都市の最短の距離（タイル）がこれ以下（暫定）
R.SUBJUGATION_NEIGHBOR_DISTANCE = 6
-- 停戦の再宣戦制限。公式の講和後の最短期間 DIPLOMACY_PEACE_MIN_TURNS と同じ値
-- （Base/Assets/Gameplay/Data/GlobalParameters.xml:248）。将軍家の裁定はこれが3倍になる。
R.TRUCE_TURNS = 10
-- 将軍家の裁定の力（SSOT 3.2「3倍」）。停戦のターン数と拒否の不利益（正当性の低下・討伐の勅命のターン数）に掛け、
-- 費用はこれで割る（切り上げ）。オーナー判断 O-3・R-3（2026-09-30）
R.SHOGUN_POWER = 3
-- AI の応諾（暫定の決定的規則）：自分の都市数が相手の 1.5 倍を超える（優勢）なら拒み、それ以外は応じる。
-- 正当性が LEGITIMACY_LOW 未満の AI は、相手を上回るだけで拒む（R-1「AI が拒みやすい」）
R.AI_REFUSE_NUM, R.AI_REFUSE_DEN = 3, 2
R.AI_REFUSE_LOW_NUM, R.AI_REFUSE_LOW_DEN = 1, 1
-- 室町幕府の終わり：元亀4年7月（research/sengoku-events.md の EV_MUROMACHI_END）。和暦の月をそのまま使う
-- （poc/historical-events.md「暦との関係」と同じ）
R.MUROMACHI_END_YEAR, R.MUROMACHI_END_MONTH = 1573, 7

R.LEGITIMACY_KEY = "JAC_LEGITIMACY"                 -- Player
R.HOME_KEY = "JAC_ARB_HOME"                         -- Player：本拠（最初に記録した首都）のプロットの番号
R.OUTLAW_KEY = "JAC_ARB_OUTLAW_UNTIL"               -- Player：討伐の勅命の対象である最後のターン
R.JOINED_PREFIX = "JAC_ARB_JOINED_"                 -- Player：<対象のID> = 1（その勅命に参加中）
R.PENDING_KEY = "JAC_ARB_PENDING"                   -- Game：審理中の請求（同時に1件）
R.LAST_REQUEST_KEY = "JAC_ARB_LAST_REQUEST_TURN"    -- Game：裁定は1ターンに1件まで
R.LAST_EVENT_KEY = "JAC_ARB_LAST_EVENT"             -- Game：最後に届いた EXECUTE_SCRIPT（検証用の記録。判定には使わない）
R.TRUCE_PREFIX = "JAC_ARB_TRUCE_"                   -- Game：<小さいID>_<大きいID> = 停戦が続く最後のターン
R.AWAIT_PREFIX = "JAC_ARB_AWAIT_"                   -- Game：応諾済みで講和を待つ組 = 講和した時から数える停戦のターン数
R.PEACE_PREFIX = "JAC_ARB_PEACE_TURN_"              -- Game：スクリプトが最後に講和させたターン（検証用の記録）
-- 都市の中心のプロットに置く 0/1 の旗（Data/JAC_CourtArbitration_Legitimacy.sql の条件が読む）
R.FLAG_HIGH = "JAC_ARB_LEGITIMACY_HIGH"
R.FLAG_LOW = "JAC_ARB_LEGITIMACY_LOW"
R.FLAG_SUBJUGATION = "JAC_ARB_SUBJUGATION_ANCHOR"  -- 勅命に参加中の家の代表の都市（1家に1都市）だけ 1

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

-- 請求の力：裁定の力を持つ将軍家の請求は3倍、それ以外は1
function R.Power(requester, turn)
	return R.IsEmpoweredShogun(requester, turn) and R.SHOGUN_POWER or 1
end

-- 請求の費用（金・正当性）。力で割り、切り上げる（将軍家は 34・2）。外交的支持は使わない（R-1）
function R.Costs(power)
	return math.ceil(R.COST_GOLD / power), math.ceil(R.COST_LEGITIMACY / power)
end

-- 拒否の不利益（正当性の低下・討伐の勅命のターン数）。力を掛ける（将軍家の裁定では 45・30）
function R.RefusalPenalty(power)
	return R.REFUSAL_LEGITIMACY * power, R.OUTLAW_TURNS * power
end

function R.Legitimacy(id)
	return Players[id]:GetProperty(R.LEGITIMACY_KEY)
		or (R.IsAshikaga(id) and R.LEGITIMACY_START_ASHIKAGA or R.LEGITIMACY_START)
end

function R.ClampLegitimacy(value)
	return math.max(R.LEGITIMACY_MIN, math.min(R.LEGITIMACY_MAX, value))
end

local function IsLiveMajor(id)
	local player = id ~= nil and Players[id] or nil
	return player ~= nil and player:IsMajor() and player:IsAlive()
end

local function AshikagaID()
	for _, player in ipairs(PlayerManager.GetAliveMajors()) do
		if R.IsAshikaga(player:GetID()) then return player:GetID() end
	end
	return nil
end

-- 毎ターンの正当性の収入（暫定、R-1）：基本 +1、本拠（最初の首都）を保っていれば +1、室町幕府の終わりまでは
-- 将軍家と戦争していなければ +1（将軍家自身は裁定の力を持つ間 +1）
function R.LegitimacyIncome(id, turn)
	local income = 1
	local home = Players[id]:GetProperty(R.HOME_KEY)
	local plot = home ~= nil and Map.GetPlotByIndex(home) or nil
	if plot ~= nil and plot:GetOwner() == id then income = income + 1 end
	local shogun = AshikagaID()
	if shogun ~= nil and turn < R.ShogunateEndTurn()
		and (shogun == id or not Players[id]:GetDiplomacy():IsAtWarWith(shogun)) then
		income = income + 1
	end
	return income
end

-- 討伐の勅命の対象か（期限のターンまで）
function R.IsTarget(id, turn)
	local untilTurn = Players[id]:GetProperty(R.OUTLAW_KEY)
	return untilTurn ~= nil and turn <= untilTurn
end

function R.JoinedKey(target) return R.JOINED_PREFIX .. target end

-- 対象の勅命に参加中か（勅命が続いている間だけ）
function R.HasJoined(id, target, turn)
	return R.IsTarget(target, turn) and Players[id]:GetProperty(R.JoinedKey(target)) == 1
end

-- 勅命に参加できるか。できなければ false と理由（ログ用）
function R.CheckJoin(id, target, turn)
	if not (IsLiveMajor(id) and IsLiveMajor(target)) or id == target then return false, "INVALID" end
	if not R.IsTarget(target, turn) then return false, "NO_ORDER" end
	if R.HasJoined(id, target, turn) then return false, "ALREADY" end
	return true
end

local function PairKey(prefix, a, b)
	if a > b then a, b = b, a end
	return prefix .. a .. "_" .. b
end
function R.TruceKey(a, b) return PairKey(R.TRUCE_PREFIX, a, b) end
function R.AwaitKey(a, b) return PairKey(R.AWAIT_PREFIX, a, b) end
function R.PeaceTurnKey(a, b) return PairKey(R.PEACE_PREFIX, a, b) end

-- 表示名の LOC キー。史実の出来事の家名切替（JAC_CIV_NAME_<文明>、CustomCivs UI/JAC_CivNameOverride.lua）を優先する
function R.NameKey(id)
	local civ = PlayerConfigurations[id]:GetCivilizationTypeName()
	return Game:GetProperty("JAC_CIV_NAME_" .. civ) or PlayerConfigurations[id]:GetCivilizationShortDescription()
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
	if Game:GetProperty(R.AwaitKey(a, b)) ~= nil then return false, "AWAITING_PEACE" end
	local turn = R.GameTurn()
	if requester ~= a and requester ~= b and not R.IsEmpoweredShogun(requester, turn) then return false, "NOT_PARTY" end
	if Game:GetProperty(R.PENDING_KEY) ~= nil or Game:GetProperty(R.LAST_REQUEST_KEY) == turn then
		return false, "ONE_PER_TURN"
	end
	-- 正当性が LEGITIMACY_LOW 未満の家は請求できない（R-1）。費用の正当性（最大 5）はこれより小さいので、
	-- この条件を満たせば必ず払える
	if R.Legitimacy(requester) < R.LEGITIMACY_LOW then return false, "LEGITIMACY" end
	local gold = R.Costs(R.Power(requester, turn))
	if Players[requester]:GetTreasury():GetGoldBalance() < gold then return false, "GOLD" end
	return true
end

-- AI の応諾（暫定）。自分の都市数が相手の 1.5 倍を超えるときだけ拒む（正当性が低ければ相手を上回るだけで拒む）
function R.AiAccepts(id, opponent)
	local mine, theirs = Players[id]:GetCities():GetCount(), Players[opponent]:GetCities():GetCount()
	if R.Legitimacy(id) < R.LEGITIMACY_LOW then
		return mine * R.AI_REFUSE_LOW_DEN <= theirs * R.AI_REFUSE_LOW_NUM
	end
	return mine * R.AI_REFUSE_DEN <= theirs * R.AI_REFUSE_NUM
end
