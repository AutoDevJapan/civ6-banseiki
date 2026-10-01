-- Test-only functions called by the short scenario. Operations and native reads
-- follow the official XP2 governor UI. Saved observations never write receipts.
ContextPtr:SetHide(false);
local SLOT = 'GOVERNOR_JAC_NATIVE_SLOT';
local PROXY = 'GOVERNOR_JAC_TITLE_PROXY';
local PREFIX = 'JAC_SLOTS_PROBE_';
local function Require(ok, message)
    if not ok then error('JAC_NATIVE_SLOTS_FAIL ' .. message); end;
end;
local function Active()
    local owner = Game.GetLocalPlayer();
    Require(owner == 0 and Players[owner]:IsHuman() and Players[owner]:IsTurnActive()
        and not UI.IsGameCoreBusy(), 'human turn is not ready');
    return owner, Players[owner]:GetGovernors();
end;
local function Native(governors, kind)
    local row = GameInfo.Governors[kind];
    Require(row ~= nil, 'governor definition absent: ' .. kind);
    return governors:HasGovernor(row.Hash) and governors:GetGovernor(row.Hash) or nil;
end;
local function NoJoinedPeople()
    local count = 0;
    for row in GameInfo.JAC_LeaderPersonas() do
        count = count + 1;
        Require(Game:GetProperty('JAC_PERSONA_' .. row.LeaderType .. '_OWNER') == nil,
            'unjoined fixture acquired a person: ' .. row.LeaderType);
        Require(GameInfo.Governors[row.GovernorType] == nil,
            'persona remains a native candidate: ' .. row.LeaderType);
        local registration = GameInfo.Types[row.GovernorType];
        Require(registration == nil or registration.Kind ~= 'KIND_GOVERNOR',
            'persona remains registered as KIND_GOVERNOR: ' .. row.LeaderType);
    end;
    Require(count == 22, 'expected exactly 22 persona definitions');
end;
function JAC_SlotsObserve(phase)
    local owner, governors = Active();
    Require(type(ExposedMembers.JAC_NativeSlotsRead) == 'function', 'GameCore bridge missing');
    local seen = ExposedMembers.JAC_NativeSlotsRead(owner);
    local slot, proxy = Native(governors, SLOT), Native(governors, PROXY);
    seen.hasSlot, seen.hasProxy = slot ~= nil, proxy ~= nil;
    if slot then
        local city = slot:GetAssignedCity();
        seen.assignedCity = city and city:GetID() or nil;
        seen.assignedOwner = city and city:GetOwner() or nil;
        seen.established = slot:IsEstablished();
        seen.turnsToEstablish = slot:GetTurnsToEstablish();
        seen.turnsOnSite = slot:GetTurnsOnSite();
        seen.neutralized = slot:GetNeutralizedTurns();
        if city then
            seen.loyaltyBreakdown = city:GetCulturalIdentity():GetIdentitySourcesDetailedBreakdown();
            for _, part in ipairs(seen.loyaltyBreakdown) do
                for source, value in pairs(part) do
                    print('JAC_NATIVE_SLOTS_LOYALTY ' .. phase .. ' source=' .. tostring(source)
                        .. ' value=' .. tostring(value));
                end;
            end;
        end;
    end;
    seen.proxyAssigned = proxy ~= nil and proxy:GetAssignedCity() ~= nil;
    print('JAC_NATIVE_SLOTS_OBSERVE ' .. phase .. ' turn=' .. seen.turn
        .. ' points=' .. seen.points .. ' spent=' .. seen.spent
        .. ' corePlayer=' .. tostring(seen.slotReceipt) .. ' coreProxy=' .. tostring(seen.proxyReceipt)
        .. ' city=' .. tostring(seen.assignedCity) .. ' established=' .. tostring(seen.established)
        .. ' site=' .. tostring(seen.turnsOnSite) .. ' neutralized=' .. tostring(seen.neutralized));
    for _, city in ipairs(seen.cities) do
        print('JAC_NATIVE_SLOTS_CITY ' .. phase .. ' owner=' .. city.owner .. ' id=' .. city.id
            .. ' coreProperty=' .. tostring(city.receipt));
    end;
    -- Persist only observed facts, with a distinct prefix from native properties.
    Game:SetProperty(PREFIX .. phase, seen);
    Require(seen.points == governors:GetGovernorPoints() and seen.spent == governors:GetGovernorPointsSpent(),
        'UI and GameCore official accounting differ');
    Require(seen.slotReceipt == Players[owner]:GetProperty('JAC_NATIVE_SLOT_PLAYER_RECEIPT')
        and seen.proxyReceipt == Players[owner]:GetProperty('JAC_PROXY_NATIVE_PAID_0'),
        'UI and GameCore player properties differ');
    return seen;
end;
function JAC_SlotsPrepare()
    Active();
    NoJoinedPeople();
    local row = GameInfo.Governors[SLOT];
    Require(row and row.IdentityPressure == 8 and row.TransitionStrength == 100
        and not row.AssignCityState and row.TraitType == nil, 'ordinary slot definition differs');
    local cannot = GameInfo.GovernorsCannotAssign[SLOT];
    Require(cannot == nil or not cannot.CannotAssign, 'slot cannot be assigned');
    Require(GameInfo.GovernorsCannotAssign[PROXY].CannotAssign, 'comparison proxy is assignable');
    local count, ai = 0, 0;
    for _, player in ipairs(PlayerManager.GetAliveMajors()) do
        count = count + 1;
        local governors = player:GetGovernors();
        Require(not Native(governors, SLOT) and not Native(governors, PROXY),
            'native fixture already appointed: ' .. player:GetID());
        if not player:IsHuman() then ai = ai + 1; end;
    end;
    Require(count == 22 and ai == 21, 'expected one human and 21 AI houses');
    local seen = JAC_SlotsObserve('BEFORE_TITLES');
    Require((seen.slotReceipt == nil or seen.slotReceipt == 0)
        and (seen.proxyReceipt == nil or seen.proxyReceipt == 0), 'unpaid native receipt present');
    return '22 people excluded; one ordinary anonymous slot; no people joined';
end;
function JAC_SlotsGiveTitles()
    Active();
    NoJoinedPeople();
    for _, player in ipairs(PlayerManager.GetAliveMajors()) do
        local governors, culture = player:GetGovernors(), player:GetCulture();
        local needed, earned = player:IsHuman() and 2 or 3, 0;
        for row in GameInfo.JAC_PersonaTitleCivics() do
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
    return 'human: two diagnostic appointment titles; 21 AI: three positive titles each';
end;
function JAC_SlotsAssertAI(firstTurn)
    Active();
    NoJoinedPeople();
    if firstTurn then
        Require(Game.GetCurrentGameTurn() == Game:GetProperty(PREFIX .. 'AI_START_TURN') + 1,
            'first native AI turn was not exactly one turn');
    end;
    local ai, failures = 0, {};
    for _, player in ipairs(PlayerManager.GetAliveMajors()) do
        if not player:IsHuman() then
            ai = ai + 1;
            local governors = player:GetGovernors();
            local before = Game:GetProperty(PREFIX .. 'BUDGET_' .. player:GetID());
            local slot, proxy = Native(governors, SLOT), Native(governors, PROXY);
            print('JAC_NATIVE_SLOTS_AI id=' .. player:GetID() .. ' points=' .. governors:GetGovernorPoints()
                .. ' spent=' .. governors:GetGovernorPointsSpent() .. ' hasSlot=' .. tostring(slot ~= nil)
                .. ' hasProxy=' .. tostring(proxy ~= nil) .. ' pressure=' .. GameInfo.Governors[SLOT].IdentityPressure);
            if not before or governors:GetGovernorPoints() < before.points
                or governors:GetGovernorPoints() - governors:GetGovernorPointsSpent() < 3
                or governors:GetGovernorPointsSpent() ~= before.spent or slot or proxy then
                table.insert(failures, tostring(player:GetID()));
            end;
        end;
    end;
    Require(ai == 21, '21 AI inventory changed');
    Require(#failures == 0, 'unmatched governor appointment or unauthorized spending; AI=' .. table.concat(failures, ','));
    return '21 native AI turns: positive titles, no unmatched slot/proxy appointment, no spending';
end;
function JAC_SlotsAppoint(kind)
    local owner, governors = Active();
    Require(kind == SLOT or kind == PROXY, 'unexpected diagnostic governor');
    local row = GameInfo.Governors[kind];
    Require(not Native(governors, kind) and governors:CanAppointGovernor(row.Hash), 'native appointment denied');
    UI.RequestPlayerOperation(owner, PlayerOperations.APPOINT_GOVERNOR,
        {[PlayerOperations.PARAM_GOVERNOR_TYPE] = row.Index});
    return 'one official appointment requested; slot title is additional diagnostic expense';
end;
local function Expense(seen, count)
    local before = Game:GetProperty(PREFIX .. 'BUDGET_0');
    Require(before and seen.points >= before.points and seen.spent == before.spent + count,
        'official expense differs: expected ' .. count .. ' diagnostic appointments');
    Require(not seen.proxyAssigned, 'cannot-assign proxy acquired a city');
end;
function JAC_SlotsUnassigned(phase, count)
    local seen = JAC_SlotsObserve(phase);
    Expense(seen, count);
    Require(seen.hasProxy and (count == 1 or seen.hasSlot), 'native appointment absent');
    Require(seen.assignedCity == nil, 'slot already assigned before receipt observation');
    for _, city in ipairs(seen.cities) do Require(city.receipt == nil or city.receipt == 0,
        'city property active before assignment'); end;
    return 'actual GameCore receipts captured while native governors remain unassigned';
end;
function JAC_SlotsRequireUnassignedReceipt()
    local before = Game:GetProperty(PREFIX .. 'UNASSIGNED');
    Require(before ~= nil, 'unassigned observation missing');
    if before.slotReceipt ~= 1 or before.proxyReceipt ~= 1 then
        local assigned = Game:GetProperty(PREFIX .. 'ASSIGNED');
        local settled = Game:GetProperty(PREFIX .. 'SETTLED');
        local classification = 'LATER_ACTIVATION_UNOBSERVED';
        if assigned and assigned.slotReceipt == 1 then classification = 'SLOT_RECEIPT_AFTER_ASSIGNMENT';
        elseif settled and settled.slotReceipt == 1 then classification = 'SLOT_RECEIPT_AFTER_ESTABLISHMENT';
        elseif settled then classification = 'SLOT_RECEIPT_STILL_MISSING'; end;
        Require(false, 'UNASSIGNED_RECEIPT_MISSING coreSlot=' .. tostring(before.slotReceipt)
            .. ' coreProxy=' .. tostring(before.proxyReceipt) .. ' classification=' .. classification);
    end;
    return 'unassigned native player receipts are exactly one in GameCore; no mock inference';
end;
function JAC_SlotsAssign()
    local owner, governors = Active();
    local slot = Native(governors, SLOT);
    Require(slot and not slot:GetAssignedCity() and slot:GetNeutralizedTurns() == 0, 'slot is not assignable');
    local city = Players[owner]:GetCities():GetCapitalCity();
    Require(city and governors:CanAssignGovernor(GameInfo.Governors[SLOT].Hash, city), 'official capital assignment denied');
    Game:SetProperty(PREFIX .. 'CITY', city:GetID());
    Game:SetProperty(PREFIX .. 'ASSIGN_TURN', Game.GetCurrentGameTurn());
    UI.RequestPlayerOperation(owner, PlayerOperations.ASSIGN_GOVERNOR, {
        [PlayerOperations.PARAM_GOVERNOR_TYPE] = GameInfo.Governors[SLOT].Index,
        [PlayerOperations.PARAM_PLAYER_ONE] = owner,
        [PlayerOperations.PARAM_CITY_DEST] = city:GetID()
    });
    return 'official native city assignment requested';
end;
function JAC_SlotsCheckCity(phase, turns)
    JAC_SlotsAssertAI(false);
    local seen = JAC_SlotsObserve(phase);
    Expense(seen, 2);
    local unassigned = Game:GetProperty(PREFIX .. 'UNASSIGNED');
    Require(unassigned ~= nil, 'original unassigned observation missing');
    -- Preserve each receipt already demonstrated before placement. A separate
    -- classification run may observe a missing receipt later, but its final
    -- assertion still rejects that original unassigned observation.
    for _, key in ipairs({'slotReceipt', 'proxyReceipt'}) do
        if unassigned[key] == 1 then
            Require(seen[key] == 1, 'proven unassigned receipt changed after placement: ' .. key);
        end;
    end;
    local expectedCity = Game:GetProperty(PREFIX .. 'CITY');
    Require(seen.assignedOwner == 0 and seen.assignedCity == expectedCity, 'native assignment moved or disappeared');
    Require(seen.neutralized == 0 and seen.turnsToEstablish == 5 and seen.turnsOnSite == turns,
        'native five-turn establishment clock differs');
    Require(seen.turn == Game:GetProperty(PREFIX .. 'ASSIGN_TURN') + turns, 'real turn count differs');
    Require(seen.established == (turns == 5), 'native establishment timing differs');
    local active, cityFound = 0, false;
    for _, city in ipairs(seen.cities) do
        if city.id == expectedCity then
            cityFound = true;
            Require(turns == 5 and city.receipt == 1 or turns < 5 and (city.receipt == nil or city.receipt == 0),
                'city property does not agree with native establishment');
        else Require(city.receipt == nil or city.receipt == 0, 'unassigned city has an active property'); end;
        if city.receipt == 1 then active = active + 1; end;
    end;
    Require(cityFound and active == (turns == 5 and 1 or 0), 'city property inventory differs');
    return 'native placement and city property verified after ' .. turns .. ' real turns';
end;
function JAC_SlotsCheckReload(phase)
    local before = Game:GetProperty(PREFIX .. phase);
    Require(before ~= nil, 'saved native observation absent');
    local after = JAC_SlotsObserve(phase .. '_RELOADED');
    for _, key in ipairs({'turn','points','spent','slotReceipt','proxyReceipt','hasSlot','hasProxy',
        'assignedCity','assignedOwner','established','turnsToEstablish','turnsOnSite','neutralized','proxyAssigned'}) do
        Require(before[key] == after[key], 'native reload differs: ' .. key);
    end;
    Require(#before.cities == #after.cities, 'reload city inventory differs');
    for i, city in ipairs(before.cities) do
        local other = after.cities[i];
        Require(city.owner == other.owner and city.id == other.id and city.receipt == other.receipt,
            'native city property differs after reload');
    end;
    Expense(after, 2);
    JAC_SlotsAssertAI(false);
    return 'native assignment, receipts and two diagnostic expenses persisted without replay';
end;
print('JAC_NativeSlots_View: short diagnostic ready');
