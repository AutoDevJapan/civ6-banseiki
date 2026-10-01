-- A read-only GameCore accounting observation, not a synthetic balance.
ExposedMembers.JAC_TitleProxyAccounting = function(owner)
    local player = Players[owner];
    if not player then return nil; end
    local governors = player:GetGovernors();
    return governors:GetGovernorPoints(), governors:GetGovernorPointsSpent();
end;
if Events.GovernorChanged then
    Events.GovernorChanged.Add(function(...)
        local values = {};
        for index, value in ipairs({...}) do values[index] = tostring(value); end
        print('JAC_PROXY_NATIVE_CHANGED ' .. table.concat(values, ','));
    end);
end;
print('JAC_TitleProxy_Probe: ready');
