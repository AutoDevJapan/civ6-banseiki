-- Keep native assignment/base ability; skill purchases use the custom permanent tree.
include("GovernorPanel");
local baseCandidate, baseShared, baseAppointed, baseAppoint, baseName, basePromote =
    AddGovernorCandidate, AddGovernorShared, AddGovernorAppointed,
    OnAppointGovernor, OnNameButton, OnPromoteGovernor;
local function Disabled(governor)
    return GameInfo.JAC_PersonaDisabledGovernors[governor.GovernorType] ~= nil;
end
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
    if Disabled(governor) or (row and not Eligible(row)) then return; end
    baseCandidate(governor, canAppoint);
end
function AddGovernorAppointed(governors, appointed, definition, canPromote)
    local row = Persona(definition);
    -- A partially paid evolution must not be assigned or display native charge
    -- promotions as purchasable skills in the official panel.
    if row and Game:GetProperty("JAC_PERSONA_" .. row.LeaderType .. "_STATE") ~= "GOVERNOR" then return; end
    baseAppointed(governors, appointed, definition, canPromote);
end
function AddGovernorShared(governor)
    local instance = baseShared(governor);
    local row = Persona(governor);
    if row then
        instance.GovernorPortrait:SetIcon(row.PortraitIcon, 256);
        instance.GovernorTitle:SetText(Locale.Lookup("LOC_JAC_PERSONA_MERIT",
            Game:GetProperty("JAC_PERSONA_" .. row.LeaderType .. "_MERIT") or 0));
        instance.GovernorTitle:SetToolTipString(Locale.Lookup("LOC_JAC_PERSONA_MERIT_HELP"));
    end
    return instance;
end
function OnAppointGovernor(id)
    local def = GameInfo.Governors[id];
    if not def or Disabled(def) then return; end
    local row = def and Persona(def);
    if row and not Eligible(row) then return; end
    baseAppoint(id);
end
function OnNameButton(id)
    local def = GameInfo.Governors[id];
    local row = def and Persona(def);
    if row then LuaEvents.JAC_PersonaTree_Open(row.LeaderType); else baseName(id); end
end
-- Native payment promotions are private to evolution; skills use personal merit.
function OnPromoteGovernor(id, promotion)
    local def = GameInfo.Governors[id];
    if not def or Disabled(def) then return; end
    local row = def and Persona(def);
    if row then LuaEvents.JAC_PersonaTree_Open(row.LeaderType); else basePromote(id, promotion); end
end
