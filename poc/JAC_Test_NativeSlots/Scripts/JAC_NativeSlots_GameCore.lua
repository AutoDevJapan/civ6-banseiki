-- Read-only GameCore observations. This closure reads the actual Player/City
-- properties; UI governor state and mock data cannot supply a native receipt.
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
print('JAC_NativeSlots_GameCore: read-only observation ready');

GameEvents.JAC_NativeSlotsGiveTitles.Add(function(owner)
    local PREFIX = 'JAC_SLOTS_PROBE_';
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
