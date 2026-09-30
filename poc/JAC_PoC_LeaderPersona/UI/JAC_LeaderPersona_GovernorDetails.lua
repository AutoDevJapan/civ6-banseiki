include("GovernorDetailsPanel");
local baseRefresh = Refresh;
function Refresh()
    baseRefresh();
    -- The official selected-governor index is private. Match its displayed, unique name.
    local name = Controls.GovernorNameLabel:GetText();
    for row in GameInfo.JAC_LeaderPersonas() do
        if name == Locale.ToUpper(row.Name) then
            Controls.GovernorPortrait:SetIcon(row.PortraitIcon, 256);
            break;
        end
    end
end
