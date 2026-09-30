-- Keep the official Governor screen, assignment chooser, city rules and promotion buttons.
include("GovernorPanel");
local baseCandidate, baseShared, baseAppoint = AddGovernorCandidate, AddGovernorShared, OnAppointGovernor;
local function Persona(governor)
    for row in GameInfo.JAC_LeaderPersonas() do
        if row.GovernorType == governor.GovernorType then return row; end
    end
end
local function Eligible(row)
    local prefix = "JAC_PERSONA_" .. row.LeaderType .. "_";
    return Game:GetProperty(prefix .. "OWNER") == Game.GetLocalPlayer() and
           Game:GetProperty(prefix .. "STATE") == "EVOLVING";
end
function AddGovernorCandidate(governor, canAppoint)
    local row = Persona(governor);
    if row and not Eligible(row) then return; end
    baseCandidate(governor, canAppoint);
end
function AddGovernorShared(governor)
    local instance = baseShared(governor);
    local row = Persona(governor);
    if row then instance.GovernorPortrait:SetIcon(row.PortraitIcon, 256); end
    return instance;
end
function OnAppointGovernor(id)
    local def = GameInfo.Governors[id];
    local row = def and Persona(def);
    if row and not Eligible(row) then return; end
    baseAppoint(id);
end
