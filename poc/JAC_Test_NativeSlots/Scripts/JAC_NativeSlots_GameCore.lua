-- GameCore owns diagnostic saves. Player/City receipts are read independently
-- here and are never written by this test MOD.
local PREFIX = 'JAC_SLOTS_PROBE_';
local PHASES = {BEFORE_TITLES=true, PROXY_UNASSIGNED=true, UNASSIGNED=true,
    ASSIGNED=true, TURN_FOUR=true, SETTLED=true};
local CAPTURE_TYPES = {hasSlot='boolean', hasProxy='boolean', assignedCity='number',
    assignedOwner='number', established='boolean', turnsToEstablish='number',
    turnsOnSite='number', neutralized='number', proxyAssigned='boolean'};
local function Require(ok, message)
    if not ok then error('JAC_NATIVE_SLOTS_FAIL ' .. message); end;
end;
ExposedMembers.JAC_NativeSlotsRead = function(owner)
    local player = Players[owner];
    if not player then error('native slot GameCore player missing'); end;
    local governors = player:GetGovernors();
    local observation = {
        turn = Game.GetCurrentGameTurn(),
        points = governors:GetGovernorPoints(),
        spent = governors:GetGovernorPointsSpent(),
        slotReceipt = player:GetProperty('JAC_NATIVE_SLOT_PLAYER_RECEIPT'),
        proxyReceipt = player:GetProperty('JAC_PROXY_NATIVE_PAID_0'),
        cities = {}
    };
    for _, city in player:GetCities():Members() do
        table.insert(observation.cities, {
            owner = city:GetOwner(), id = city:GetID(),
            receipt = city:GetProperty('JAC_NATIVE_SLOT_CITY_ACTIVE')
        });
    end;
    table.sort(observation.cities, function(a, b) return a.id < b.id; end);
    return observation;
end;
GameEvents.JAC_NativeSlotsSaveObservation.Add(function(owner, params)
    Require(owner == 0 and Players[owner] and Players[owner]:IsHuman(), 'diagnostic owner differs');
    Require(type(params) == 'table' and PHASES[params.Phase], 'unknown observation phase');
    Require(Game:GetProperty(PREFIX .. params.Phase) == nil, 'observation already saved: ' .. params.Phase);
    Require(type(params.hasSlot) == 'boolean' and type(params.hasProxy) == 'boolean'
        and type(params.proxyAssigned) == 'boolean', 'required UI capture missing');
    local observation = ExposedMembers.JAC_NativeSlotsRead(owner);
    for key, expected in pairs(CAPTURE_TYPES) do
        local value = params[key];
        Require(value == nil or type(value) == expected, 'invalid UI capture: ' .. key);
        observation[key] = value;
    end;
    Require(params.hasSlot or (params.assignedCity == nil and params.assignedOwner == nil
        and params.established == nil), 'unappointed slot has assignment fields');
    Require((params.assignedCity == nil) == (params.assignedOwner == nil),
        'partial UI assignment');
    Game:SetProperty(PREFIX .. params.Phase, observation);
end);
GameEvents.JAC_NativeSlotsMarkAssignment.Add(function(owner, params)
    Require(owner == 0 and Players[owner] and Players[owner]:IsHuman(), 'diagnostic owner differs');
    Require(type(params) == 'table' and type(params.CityID) == 'number', 'assignment city missing');
    Require(Game:GetProperty(PREFIX .. 'CITY') == nil, 'assignment marker already saved');
    local player = Players[owner];
    local city = player:GetCities():GetCapitalCity();
    -- This records the intended target and clock, not evidence of appointment.
    -- Native governor getters are UI-only. The UI checks assignment eligibility
    -- again after this acknowledgement, before its official ASSIGN operation.
    Require(city and city:GetOwner() == owner and city:GetID() == params.CityID,
        'assignment target changed');
    Game:SetProperty(PREFIX .. 'CITY', city:GetID());
    Game:SetProperty(PREFIX .. 'ASSIGN_TURN', Game.GetCurrentGameTurn());
end);
print('JAC_NativeSlots_GameCore: observation read and synchronized saves ready');

GameEvents.JAC_NativeSlotsGiveTitles.Add(function(owner)
    local function Require(ok,message) if not ok then error('JAC_NATIVE_SLOTS_FAIL '..message) end end;
    Require(owner==0,'diagnostic owner differs');
    local majors=PlayerManager.GetAliveMajors(); table.sort(majors,function(a,b) return a:GetID()<b:GetID() end);
    local civics={}; for row in GameInfo.JAC_PersonaTitleCivics() do civics[#civics+1]=row end; table.sort(civics,function(a,b) return a.CivicType<b.CivicType end);
    for _, player in ipairs(majors) do
        local governors, culture = player:GetGovernors(), player:GetCulture();
        local needed, earned = player:IsHuman() and 2 or 3, 0;
        for _, row in ipairs(civics) do
            if earned < needed then
                local civic = GameInfo.Civics[row.CivicType];
                if not culture:HasCivic(civic.Index) then
                    local before = governors:GetGovernorPoints();
                    -- Official Debug/Player.ltp uses this cultural-progress operation.
                    culture:SetCulturalProgress(civic.Index, culture:GetCultureCost(civic.Index));
                    Require(governors:GetGovernorPoints() == before + 1, 'civic did not grant exactly one title');
                    earned = earned + 1;
                end;
            end;
        end;
        Require(earned == needed and governors:GetGovernorPoints() - governors:GetGovernorPointsSpent() >= needed,
            'positive official title fixture missing: ' .. player:GetID());
        Game:SetProperty(PREFIX .. 'BUDGET_' .. player:GetID(), {
            points = governors:GetGovernorPoints(), spent = governors:GetGovernorPointsSpent()
        });
    end;
    Game:SetProperty(PREFIX .. 'AI_START_TURN', Game.GetCurrentGameTurn());

    Players[owner]:SetProperty('JAC_NATIVE_SLOTS_TITLES_READY',1);
end);
