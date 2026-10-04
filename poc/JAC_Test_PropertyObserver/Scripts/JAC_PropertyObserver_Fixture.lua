-- TEST ONLY. The UI sends each request through synchronized EXECUTE_SCRIPT.
-- These fixed keys are separate from persona ownership, state and title data.
local KEY = "JAC_TEST853_VALUE";
local PHASE = "JAC_TEST853_PHASE";
-- Numeric keys reach the pinned native getter as EDX directly in this DLL.
-- The fixture checks absence before writing, so an occupied ID fails closed.
local NUMERIC_KEY = 853271;
local NUMERIC_PHASE = 853272;
local VALUES = {true, false, 853271, -853271};

local function Step(owner, params, key, phase)
    assert(owner == 0 and Players[owner] and Players[owner]:IsHuman(),
        "property fixture requires the human player 0");
    assert(type(params) == "table" and type(params.Phase) == "number" and
        params.Phase == math.floor(params.Phase) and params.Phase >= 1 and params.Phase <= 5,
        "property fixture phase must be 1..5");
    local before = Game:GetProperty(phase) or 0;
    assert(params.Phase == before + 1, "property fixture phase must advance once");
    if before == 0 then
        assert(Game:GetProperty(key) == nil and Players[owner]:GetProperty(key) == nil,
            "property fixture keys must be absent before the first write");
    end
    local value = VALUES[params.Phase]; -- phase 5 removes both keys
    Game:SetProperty(key, value);
    Players[owner]:SetProperty(key, value);
    assert(Game:GetProperty(key) == value and Players[owner]:GetProperty(key) == value,
        "official Gameplay property read disagrees with the write");
    Game:SetProperty(phase, params.Phase);
    print("JAC_PROPERTY_OBSERVER key=" .. tostring(key) .. " phase=" .. params.Phase ..
        " game=" .. tostring(Game:GetProperty(key)) .. " player=" ..
        tostring(Players[owner]:GetProperty(key)));
end

GameEvents.JAC_PropertyObserverStep.Add(function(owner, params)
    Step(owner, params, KEY, PHASE);
end);

GameEvents.JAC_PropertyObserverNumericStep.Add(function(owner, params)
    Step(owner, params, NUMERIC_KEY, NUMERIC_PHASE);
end);

print("JAC_PropertyObserver_Fixture: ready (test only; no automatic mutations)");
