-- TEST ONLY. The UI sends each request through synchronized EXECUTE_SCRIPT.
-- These fixed keys are separate from persona ownership, state and title data.
local KEY = "JAC_TEST853_VALUE";
local PHASE = "JAC_TEST853_PHASE";
local VALUES = {true, false, 853271, -853271};

GameEvents.JAC_PropertyObserverStep.Add(function(owner, params)
    assert(owner == 0 and Players[owner] and Players[owner]:IsHuman(),
        "property fixture requires the human player 0");
    assert(type(params) == "table" and type(params.Phase) == "number" and
        params.Phase == math.floor(params.Phase) and params.Phase >= 1 and params.Phase <= 5,
        "property fixture phase must be 1..5");
    local before = Game:GetProperty(PHASE) or 0;
    assert(params.Phase == before + 1, "property fixture phase must advance once");
    if before == 0 then
        assert(Game:GetProperty(KEY) == nil and Players[owner]:GetProperty(KEY) == nil,
            "property fixture keys must be absent before the first write");
    end
    local value = VALUES[params.Phase]; -- phase 5 removes both keys
    Game:SetProperty(KEY, value);
    Players[owner]:SetProperty(KEY, value);
    assert(Game:GetProperty(KEY) == value and Players[owner]:GetProperty(KEY) == value,
        "official Gameplay property read disagrees with the write");
    Game:SetProperty(PHASE, params.Phase);
    print("JAC_PROPERTY_OBSERVER phase=" .. params.Phase .. " game=" ..
        tostring(Game:GetProperty(KEY)) .. " player=" ..
        tostring(Players[owner]:GetProperty(KEY)));
end);

print("JAC_PropertyObserver_Fixture: ready (test only; no automatic mutations)");
