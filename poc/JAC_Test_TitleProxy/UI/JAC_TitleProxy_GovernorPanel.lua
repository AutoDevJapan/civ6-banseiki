-- Payment is private to the synchronized persona coordinator. Preserve every
-- other mod's Governor and cover both ordinary and cannot-assign UI routes.
include('GovernorPanel');
local normalCandidate, secretCandidate = AddGovernorCandidate, AddSecretGovernorCandidate;
local normalAppointed, secretAppointed = AddGovernorAppointed, AddSecretGovernorAppointed;
local appoint, promote, name = OnAppointGovernor, OnPromoteGovernor, OnNameButton;
local function Payment(definition)
    return definition and definition.GovernorType == 'GOVERNOR_JAC_TITLE_PROXY';
end;
function AddGovernorCandidate(definition, ...)
    if not Payment(definition) then return normalCandidate(definition, ...); end;
end;
function AddSecretGovernorCandidate(definition, ...)
    if not Payment(definition) then return secretCandidate(definition, ...); end;
end;
function AddGovernorAppointed(governors, governor, definition, ...)
    if not Payment(definition) then return normalAppointed(governors, governor, definition, ...); end;
end;
function AddSecretGovernorAppointed(governors, governor, definition, ...)
    if not Payment(definition) then return secretAppointed(governors, governor, definition, ...); end;
end;
function OnAppointGovernor(index, ...)
    if not Payment(GameInfo.Governors[index]) then return appoint(index, ...); end;
end;
function OnPromoteGovernor(index, ...)
    if not Payment(GameInfo.Governors[index]) then return promote(index, ...); end;
end;
function OnNameButton(index, ...)
    if not Payment(GameInfo.Governors[index]) then return name(index, ...); end;
end;
