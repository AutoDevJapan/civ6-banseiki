-- Gameplay only. Engine evidence and PoC limits: documents/poc/leader-persona.md.
-- Joining is an explicit integration seam; diplomacy/alliance/territory alone never calls it.
local PREFIX = "JAC_PERSONA_";
local RESULT = "JAC_PERSONA_RESULT";
local PERSONALITIES = { BRAVE=true, CALM=true, CAREFUL=true, JOLLY=true, GENTLE=true, BOLD=true, FICKLE=true, LOYAL=true };
local TRAITS = { VALIANT=true, FIERCE=true, ASTUTE=true, SHREWD=true, DEVOUT=true, ASCETIC=true, MUNDANE=true, ADEPT=true };

local function Key(leader, field) return PREFIX .. leader .. "_" .. field; end
local function Get(leader, field) return Game:GetProperty(Key(leader, field)); end
local function Set(leader, field, value) Game:SetProperty(Key(leader, field), value); end
local function Definition(leader)
    if type(leader) ~= "string" then return nil; end
    return GameInfo.JAC_LeaderPersonas[leader];
end
local function Player(id, alive)
    if type(id) ~= "number" or id < 0 or id % 1 ~= 0 then return nil; end
    local p = Players[id];
    if not p or not p:IsMajor() or (alive and not p:IsAlive()) then return nil; end
    return p;
end
local function Finish(owner, result)
    local p = Player(owner, false);
    if p then p:SetProperty(RESULT, result); end
    return result;
end

local function FindPerson(owner, row)
    local p = Player(owner, false);
    local individual = GameInfo.GreatPersonIndividuals[row.PersonType];
    if not p or not individual then return nil; end
    -- CreatePerson has no documented return contract. Scan by the native individual identity.
    -- Base/Assets/UI/Panels/UnitPanel.lua:2273-2277.
    for _, unit in p:GetUnits():Members() do
        local gp = unit:GetGreatPerson();
        if gp and gp:IsGreatPerson() then
            local actual = GameInfo.GreatPersonIndividuals[gp:GetIndividual()];
            if actual and actual.GreatPersonIndividualType == row.PersonType then return unit; end
        end
    end
end

local function NativeGovernor(owner, row)
    local p = Player(owner, false);
    local def = GameInfo.Governors[row.GovernorType];
    if not p or not def then return nil; end
    local governors = p:GetGovernors();
    if not governors:HasGovernor(def.Hash) then return nil; end
    return governors:GetGovernor(def.Hash); -- Expansion1/UI/Additions/GovernorPanel.lua:528
end

local function Join(owner, source, personality, trait)
    local p, other = Player(owner, true), Player(source, false);
    if not p or not other or owner == source then return Finish(owner, "INVALID"); end
    local config = PlayerConfigurations[source];
    local leader = config and config:GetLeaderTypeName();
    local row = Definition(leader);
    local ownConfig = PlayerConfigurations[owner];
    if not row or not ownConfig or not Definition(ownConfig:GetLeaderTypeName()) or
       config:GetCivilizationTypeName() ~= row.CivilizationType then return Finish(owner, "INVALID"); end
    if (personality ~= nil and not PERSONALITIES[personality]) or
       (trait ~= nil and not TRAITS[trait]) then return Finish(owner, "INVALID_METADATA"); end
    -- A world-unique person, even if callers try another owner or a different source slot.
    if Get(leader, "OWNER") ~= nil then
        return Finish(owner, Get(leader, "OWNER") == owner and "ALREADY_JOINED" or "OTHER_OWNER");
    end
    if NativeGovernor(owner, row) then return Finish(owner, "GOVERNOR_EXISTS"); end
    local city = p:GetCities():GetCapitalCity();
    if not city then return Finish(owner, "NO_CITY"); end
    local unit = FindPerson(owner, row);
    if not unit then
        -- Official placement call: Base/Assets/UI/Tuner/TunerMapPanel.lua:190.
        Game.GetGreatPeople():CreatePerson(owner, GameInfo.GreatPersonIndividuals[row.PersonType].Index,
                                          city:GetX(), city:GetY());
        unit = FindPerson(owner, row);
    end
    if not unit then return Finish(owner, "SPAWN_FAILED"); end
    Set(leader, "SOURCE", source);
    Set(leader, "NAME", row.Name);
    Set(leader, "PORTRAIT", row.PortraitIcon);
    -- Nil is deliberately unassigned. Never guess a historical person's personality/trait.
    Set(leader, "PERSONALITY", personality);
    Set(leader, "TRAIT", trait);
    Set(leader, "UNIT", unit:GetID());
    Set(leader, "OWNER", owner);
    Set(leader, "STATE", "GREAT_PERSON");
    return Finish(owner, "JOINED");
end

local function PrepareEvolution(owner, leader)
    local p, row = Player(owner, true), Definition(leader);
    if not p or not row or Get(leader, "OWNER") ~= owner then return Finish(owner, "NOT_OWNED"); end
    local state = Get(leader, "STATE");
    if state == "GOVERNOR" then return Finish(owner, "ALREADY_EVOLVED"); end
    if state == "EVOLVING" then return Finish(owner, "EVOLVING"); end
    if state ~= "GREAT_PERSON" or not FindPerson(owner, row) then return Finish(owner, "PERSON_MISSING"); end
    if NativeGovernor(owner, row) then return Finish(owner, "GOVERNOR_EXISTS"); end
    if not p:GetGovernors():CanAppoint() then return Finish(owner, "NO_TITLE"); end
    Set(leader, "STATE", "EVOLVING");
    return Finish(owner, "EVOLVING");
end

local function CompleteEvolution(owner, leader)
    local p, row = Player(owner, true), Definition(leader);
    if not p or not row or Get(leader, "OWNER") ~= owner then return Finish(owner, "NOT_OWNED"); end
    if Get(leader, "STATE") == "GOVERNOR" then return Finish(owner, "ALREADY_EVOLVED"); end
    if Get(leader, "STATE") ~= "EVOLVING" then return Finish(owner, "NOT_EVOLVING"); end
    if not NativeGovernor(owner, row) then return Finish(owner, "APPOINTMENT_PENDING"); end
    -- Retain the Great Person until the engine has really appointed its Governor.
    local unit = FindPerson(owner, row);
    if unit then p:GetUnits():Destroy(unit); end -- TunerMapPanel.lua:199, also accepts no return value
    if FindPerson(owner, row) then return Finish(owner, "REMOVAL_PENDING"); end
    Set(leader, "UNIT", nil);
    Set(leader, "STATE", "GOVERNOR");
    return Finish(owner, "EVOLVED");
end

local function Reconcile()
    for row in GameInfo.JAC_LeaderPersonas() do
        local owner = Get(row.LeaderType, "OWNER");
        if owner ~= nil and Get(row.LeaderType, "STATE") == "EVOLVING" then
            CompleteEvolution(owner, row.LeaderType);
        end
    end
end

-- Development fixture only, not bound to a button or to a turn event. Gives exactly one native title
-- per owner/person/phase and records that grant in the save. Production title acquisition is undecided.
local function GrantTestTitle(owner, leader, phase)
    local p, row = Player(owner, true), Definition(leader);
    if not p or not row or Get(leader, "OWNER") ~= owner or
       (phase ~= "APPOINT" and phase ~= "PROMOTE") then return Finish(owner, "INVALID"); end
    local field = "TEST_TITLE_" .. phase;
    if Get(leader, field) then return Finish(owner, "TITLE_ALREADY_GRANTED"); end
    p:GetGovernors():ChangeGovernorPoints(1); -- Official FireTuner: Debug/Player.ltp:289-295.
    Set(leader, field, true);
    return Finish(owner, "TITLE_GRANTED");
end

GameEvents.JAC_LeaderPersonaEvolve.Add(function(owner, params)
    PrepareEvolution(owner, type(params) == "table" and params.Leader or nil);
end);
GameEvents.JAC_LeaderPersonaFinalize.Add(function(owner, params)
    CompleteEvolution(owner, type(params) == "table" and params.Leader or nil);
end);
GameEvents.OnGameTurnStarted.Add(Reconcile);
ExposedMembers.JAC_LeaderPersona = {
    Join = Join, PrepareEvolution = PrepareEvolution, CompleteEvolution = CompleteEvolution,
    GrantTestTitle = GrantTestTitle, Reconcile = Reconcile
};
print("JAC_LeaderPersona_Gameplay: ready");
