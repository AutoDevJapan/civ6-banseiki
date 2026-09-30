-- 天下統一同盟（SSOT 3.1）の申し込みと受諾。UI は Game のプロパティを読んで並べ、変更は要求だけを送る。
-- 状態を変えるのはゲーム側の Scripts/JAC_Sengoku_UnificationPact.lua（マルチプレイで全端末が同じ処理をする）。
-- 要求の送り方は公式の BlackDeath シナリオと同じ（DLC/BlackDeathScenario/UI/BlackDeathScenario_PopupHandler.lua:21-28）。
include("InstanceManager")

local LEAD_PREFIX = "JAC_UNIFICATION_PACT_LEAD_"
local PROPOSAL_PREFIX = "JAC_UNIFICATION_PACT_PROPOSAL_"
local RESULT = "JAC_UNIFICATION_PACT_RESULT"

local m_Rows = InstanceManager:new("PlayerRow", "Root", Controls.Rows)
local m_ShownProposal = nil -- 自動で開いた申し込み（同じ申し込みでは開き直さない）

local function Name(playerID)
	return Locale.Lookup(PlayerConfigurations[playerID]:GetCivilizationShortDescription())
end

local function LeadOf(playerID)
	return Game:GetProperty(LEAD_PREFIX .. playerID)
end

-- 申し込み（Propose）・受諾（Accept）・脱退（Leave）。Target に相手のプレイヤー ID を渡す（脱退では使わない）
function JAC_RequestPact(onStart, target)
	UI.RequestPlayerOperation(Game.GetLocalPlayer(), PlayerOperations.EXECUTE_SCRIPT,
		{ OnStart = "JAC_UnificationPact" .. onStart, Target = target })
end

local function Refresh()
	m_Rows:ResetInstances()
	local me = Game.GetLocalPlayer()
	if me == -1 then return end
	local myLead = LeadOf(me)
	Controls.LeaveButton:SetHide(myLead == nil)
	local status = Locale.Lookup("LOC_JAC_UNIFICATION_PACT_NONE")
	if myLead ~= nil then
		status = Locale.Lookup("LOC_JAC_UNIFICATION_PACT_MEMBER", Name(myLead))
	end
	local result = Players[me]:GetProperty(RESULT)
	if result then
		status = status .. "[NEWLINE]" .. Locale.Lookup("LOC_JAC_UNIFICATION_PACT_RESULT_" .. result)
	end
	Controls.Status:SetText(status)
	for _, player in ipairs(PlayerManager.GetAliveMajors()) do
		local id = player:GetID()
		if id ~= me and Players[me]:GetDiplomacy():HasMet(id) and player:GetTeam() ~= Players[me]:GetTeam() then
			local row = m_Rows:GetInstance()
			row.Name:SetText(Name(id))
			local theirLead = LeadOf(id)
			if theirLead ~= nil then
				row.Action:SetText(Locale.Lookup("LOC_JAC_UNIFICATION_PACT_IN_PACT", Name(theirLead)))
				row.Action:SetDisabled(true)
			elseif myLead == nil and Game:GetProperty(PROPOSAL_PREFIX .. me) == id then
				row.Action:SetText(Locale.Lookup("LOC_JAC_UNIFICATION_PACT_ACCEPT"))
				row.Action:SetDisabled(false)
				row.Action:RegisterCallback(Mouse.eLClick, function() JAC_RequestPact("Accept", id) end)
			else
				row.Action:SetText(Locale.Lookup("LOC_JAC_UNIFICATION_PACT_PROPOSE"))
				row.Action:SetDisabled(myLead ~= nil and myLead ~= me)
				row.Action:RegisterCallback(Mouse.eLClick, function() JAC_RequestPact("Propose", id) end)
			end
		end
	end
	Controls.Rows:CalculateSize()
	Controls.PactPanel:DoAutoSize()
end

function JAC_TogglePactPanel()
	Controls.PactPanel:SetHide(not Controls.PactPanel:IsHidden())
	if not Controls.PactPanel:IsHidden() then Refresh() end
end

-- 自分への申し込みが新しく届いたら画面を開く（AI からの申し込みも含む。受けるかは画面で選ぶ）
local function OnRefreshIfShown()
	local me = Game.GetLocalPlayer()
	local proposal = me ~= -1 and LeadOf(me) == nil and Game:GetProperty(PROPOSAL_PREFIX .. me) or nil
	if proposal ~= nil and proposal ~= m_ShownProposal then Controls.PactPanel:SetHide(false) end
	m_ShownProposal = proposal
	if not Controls.PactPanel:IsHidden() then Refresh() end
end

Controls.PactToggle:RegisterCallback(Mouse.eLClick, JAC_TogglePactPanel)
Controls.LeaveButton:RegisterCallback(Mouse.eLClick, function() JAC_RequestPact("Leave", Game.GetLocalPlayer()) end)
Events.LocalPlayerTurnBegin.Add(OnRefreshIfShown) -- Base/Assets/UI/ActionPanel.lua:1334
Events.GameCoreEventPublishComplete.Add(OnRefreshIfShown) -- 要求の処理後に更新（Base/Assets/UI/Choosers/ResearchChooser.lua:485）
Events.LoadScreenClose.Add(OnRefreshIfShown) -- 読み込み直後にも届いている申し込みを出す（Base/Assets/UI/Popups/PlayerChange.lua:298）
