include("GovernorDetailsPanel");
local baseRefresh = Refresh;
local baseApplyPromotion = ApplyPromotion;
function ApplyPromotion()
    local name = Controls.GovernorNameLabel:GetText();
    for row in GameInfo.JAC_LeaderPersonas() do
        if name == Locale.ToUpper(row.Name) then
            LuaEvents.JAC_PersonaTree_Open(row.LeaderType);
            return;
        end
    end
    baseApplyPromotion();
end
function Refresh()
    baseRefresh();
    -- The official selected-governor index is private. Match its displayed, unique name.
    local name = Controls.GovernorNameLabel:GetText();
    for row in GameInfo.JAC_LeaderPersonas() do
        if name == Locale.ToUpper(row.Name) then
            Controls.GovernorPortrait:SetIcon(row.PortraitIcon, 256);
            Controls.GovernorNameLabel:SetToolTipString(Locale.Lookup("LOC_JAC_PERSONA_MERIT",
                Game:GetProperty("JAC_PERSONA_" .. row.LeaderType .. "_MERIT") or 0) .. "[NEWLINE]" ..
                Locale.Lookup("LOC_JAC_PERSONA_MERIT_HELP"));
            break;
        end
    end
end
