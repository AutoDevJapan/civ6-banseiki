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
