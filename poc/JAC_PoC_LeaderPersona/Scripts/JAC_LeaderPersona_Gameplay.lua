-- Gameplay only. Engine evidence and PoC limits: documents/poc/leader-persona.md.
-- Joining is an explicit integration seam; diplomacy/alliance/territory alone never calls it.
include("JAC_PersonaTreeRules");
local Rules = JAC_PersonaTreeRules;
local SnapshotAlliances;
local OnPlayerTurnStarted;
local PENDING = "JAC_PERSONA_PENDING_DEEDS";
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
    local class = individual and GameInfo.GreatPersonClasses[individual.GreatPersonClassType];
    local unitType = class and GameInfo.Units[class.UnitType];
    if not p or not unitType then return nil, false; end
    -- leaderperson-06 verified GetIndividual in GameCore, but not IsGreatPerson.
    -- Filter by the class's native unit type before reading the individual identity.
    local found, count = nil, 0;
    for _,unit in p:GetUnits():Members() do
        if unit:GetType() == unitType.Index then
            local gp = unit:GetGreatPerson();
            local id = gp and gp:GetIndividual();
            local actual = id ~= nil and id ~= -1 and GameInfo.GreatPersonIndividuals[id];
            if actual and actual.GreatPersonIndividualType == row.PersonType then
                found, count = unit, count + 1;
            end
        end
    end
    return found, true, count;
end

local function NativeGovernor(owner, row)
    return Rules.NativeGovernor(owner, row.LeaderType);
end

-- Private cost-one native admission experiment. The flag is set only by the
-- opt-in test fixture; the Game ledger and engine objects decide this derived
-- Player property. An absent fixture never publishes an admission value.
local ASHINA_ADMISSION = "LEADER_JAC_ASHINA_MORIKIYO";
local function SyncAshinaAdmission(forceClosed)
    if Game:GetProperty("JAC_TEST853_ADMISSION_ACTIVE") ~= 1 then return; end
    local p = Player(0, false);
    local row = Definition(ASHINA_ADMISSION);
    local def = row and GameInfo.Governors[row.GovernorType];
    if not p or not def or def.Index ~= 4 or row.EvolutionCost ~= 1 then return; end
    p:SetProperty(853304, false);
    if p:GetProperty(853304) ~= false then return; end
    local allowed = false;
    if not forceClosed and Get(ASHINA_ADMISSION, "OWNER") == 0 and
       Get(ASHINA_ADMISSION, "EVOLUTION_COST") == 1 then
        local stage = Get(ASHINA_ADMISSION, "STATE");
        local native = NativeGovernor(0, row);
        local person, ready, count = FindPerson(0, row);
        local governors = p:GetGovernors();
        if stage == "EVOLVING" and ready and person and count == 1 and native and not native.appointed then
            local available = governors:GetGovernorPoints() - governors:GetGovernorPointsSpent();
            local reserved, uncertain = Rules.ReservedAppointments(0);
            allowed = not uncertain and available >= 1 and available >= reserved;
        elseif stage == "GOVERNOR" and ready and not person and native and
               native.appointed and native.base and native.surchargePaid == 0 and
               Get(ASHINA_ADMISSION, "UNIT") == nil then
            allowed = governors:GetGovernorPointsSpent() >= 1;
        end
    end
    if allowed then
        p:SetProperty(853304, true);
        if p:GetProperty(853304) ~= true then
            p:SetProperty(853304, false);
            p:GetProperty(853304);
        end
    end
end

-- Normal admission uses only synchronized Game ledger, Core units and official
-- title totals. Each numeric key is derived from this run's GameInfo Index;
-- neither a caller's UI property nor the private 853304 experiment is input.
-- The separate native consumer accepts only the two reserved key ranges.
local ADMISSION_MANIFEST = 900000;
local ADMISSION_ALLOW = 901000;
-- The owner is encoded in the reset key, so its natural property read can
-- bind the Core receiver without a Gameplay CanEverAppointGovernor binding.
local ADMISSION_RESET = 902000;
local ADMISSION_LIMIT = 256;
local ADMISSION_SCHEMA = "JAC_PERSONA_ADMISSION_SCHEMA_V1";
local function AdmissionRows()
    local rows, seen, leaders = {}, {}, {};
    for row in GameInfo.JAC_LeaderPersonas() do
        local def = row and GameInfo.Governors[row.GovernorType];
        local index = def and def.Index;
        if type(index) ~= "number" or index % 1 ~= 0 or index < 0 or index >= ADMISSION_LIMIT or
           seen[index] or type(row.LeaderType) ~= "string" or leaders[row.LeaderType] or
           row.GovernorType ~= "GOVERNOR_JAC_PERSONA_" .. string.sub(row.LeaderType, 12) then
            return nil;
        end
        seen[index] = true;
        leaders[row.LeaderType] = true;
        rows[#rows + 1] = {row=row, index=index};
    end
    if #rows ~= 22 then return nil; end
    table.sort(rows, function(a, b) return a.index < b.index; end);
    return rows;
end

local function OfficialTitleBalance(owner)
    local p = Player(owner, true);
    local governors = p and p:GetGovernors();
    if not governors or type(governors.GetGovernorPoints) ~= "function" or
       type(governors.GetGovernorPointsSpent) ~= "function" then return nil; end
    local earnedOK, earned = pcall(governors.GetGovernorPoints, governors);
    local spentOK, spent = pcall(governors.GetGovernorPointsSpent, governors);
    if not earnedOK or not spentOK or type(earned) ~= "number" or type(spent) ~= "number" or earned % 1 ~= 0 or
       spent % 1 ~= 0 or spent < 0 or earned < spent then return nil; end
    return earned - spent, spent;
end

local function CorePaymentRead(owner, row, cost)
    -- Read per-person native payment; aggregate spending is never attribution.
    -- The second result distinguishes absent bindings from invalid native data.
    local p = Player(owner, true);
    local governors = p and p:GetGovernors();
    local def = GameInfo.Governors[row.GovernorType];
    local base = GameInfo.GovernorPromotions[row.BasePromotionType];
    if not governors or not def or not base or
       type(governors.HasGovernor) ~= "function" or
       type(governors.GetGovernor) ~= "function" then return nil, false; end
    local ok, governor = pcall(governors.GetGovernor, governors, def.Hash);
    local present, has = pcall(governors.HasGovernor, governors, def.Hash);
    if not ok or not present or type(has) ~= "boolean" then return nil, true; end
    if not has then
        if governor ~= nil then return nil, true; end
        return 0, true;
    end
    if not governor or type(governor.HasPromotion) ~= "function" then return nil, true; end
    local baseOK, baseOwned = pcall(governor.HasPromotion, governor, base.Hash);
    if not baseOK or baseOwned ~= true then return nil, true; end
    local paid, missing = 1, false;
    for step = 1, cost - 1 do
        local payment = GameInfo.GovernorPromotions[row.BasePromotionType .. "_TITLE_" .. step];
        if not payment then return nil, true; end
        local paymentOK, owned = pcall(governor.HasPromotion, governor, payment.Hash);
        if not paymentOK or type(owned) ~= "boolean" or (missing and owned) then return nil, true; end
        if owned then paid = paid + 1; else missing = true; end
    end
    return paid, true;
end

local function ReservedOutstandingCost(owner, rows, spent)
    local total, attributed = 0, 0;
    for _,entry in ipairs(rows) do
        local row = entry.row;
        if Get(row.LeaderType, "OWNER") == owner and Get(row.LeaderType, "STATE") == "EVOLVING" then
            local cost = Get(row.LeaderType, "EVOLUTION_COST");
            if type(cost) ~= "number" or cost % 1 ~= 0 or cost < 1 or cost > 3 or
               cost ~= row.EvolutionCost then return nil; end
            local paid, readable = CorePaymentRead(owner, row, cost);
            if readable and paid == nil then return nil; end
            -- Without native identity proof, retain the whole reservation.
            paid = paid or 0;
            total = total + cost - paid;
            attributed = attributed + paid;
        end
    end
    if spent < attributed then return nil; end
    return total;
end

local function CorePaymentComplete(owner, row, cost, spent)
    local paid = CorePaymentRead(owner, row, cost);
    return spent >= cost and paid == cost;
end

-- Read-only diagnostic for the parent's bounded Gameplay probe. API presence
-- is measured here, never inferred from a UI fixture or a native RVA call.
local function CoreAdmissionCapabilities(owner, leader)
    local row = Definition(leader);
    local p = Player(owner, true);
    local governors = p and p:GetGovernors();
    local def = row and GameInfo.Governors[row.GovernorType];
    local result = {owner=owner, leader=leader, context="Gameplay",
        hasGovernorBinding=governors ~= nil and type(governors.HasGovernor) == "function",
        getGovernorBinding=governors ~= nil and type(governors.GetGovernor) == "function"};
    if not governors or not def then return result; end
    result.index = def.Index;
    result.state = Get(leader, "STATE");
    result.ledgerOwner = Get(leader, "OWNER");
    result.available, result.spent = OfficialTitleBalance(owner);
    if result.hasGovernorBinding then
        result.hasGovernorOK, result.hasGovernor = pcall(governors.HasGovernor, governors, def.Hash);
    end
    if result.getGovernorBinding then
        local ok, governor = pcall(governors.GetGovernor, governors, def.Hash);
        result.getGovernorOK = ok;
        result.governorReturned = ok and governor ~= nil;
        if ok and governor and type(governor.HasPromotion) == "function" then
            local base = GameInfo.GovernorPromotions[row.BasePromotionType];
            if base then result.baseOK, result.base = pcall(governor.HasPromotion, governor, base.Hash); end
            for step=1,row.EvolutionCost-1 do
                local promotion = GameInfo.GovernorPromotions[row.BasePromotionType .. "_TITLE_" .. step];
                if promotion then
                    result["paymentOK" .. step], result["payment" .. step] =
                        pcall(governor.HasPromotion, governor, promotion.Hash);
                end
            end
        end
    end
    return result;
end
local function AdmissionAllowed(owner, row, rows, forceClosed)
    if forceClosed or Get(row.LeaderType, "OWNER") ~= owner then return false; end
    local cost = Get(row.LeaderType, "EVOLUTION_COST");
    if type(cost) ~= "number" or cost ~= row.EvolutionCost or cost % 1 ~= 0 or
       cost < 1 or cost > 3 then return false; end
    local available, spent = OfficialTitleBalance(owner);
    if not available then return false; end
    local person, ready, count = FindPerson(owner, row);
    local state = Get(row.LeaderType, "STATE");
    if state == "EVOLVING" then
        local reserved = ReservedOutstandingCost(owner, rows, spent);
        return ready and person ~= nil and count == 1 and
            Get(row.LeaderType, "UNIT") == person:GetID() and
            reserved ~= nil and available >= reserved;
    end
    if state == "GOVERNOR" then
        return ready and person == nil and count == 0 and
            Get(row.LeaderType, "UNIT") == nil and
            CorePaymentComplete(owner, row, cost, spent);
    end
    return false;
end

-- Volatile publication snapshots, never an authority. A script reload loses
-- them and must reset all current owners before publishing saved Game values.
local ADMISSION_CLOSE = 903000;
local admissionSnapshots = {};
local admissionRowsFingerprint;
local function CloseAdmissionOwner(p)
    local owner = p:GetID();
    if type(owner) ~= "number" or owner % 1 ~= 0 or owner < 0 or owner >= 64 then return false; end
    p:SetProperty(ADMISSION_RESET + owner, true);
    local closed = p:GetProperty(ADMISSION_RESET + owner) == true;
    admissionSnapshots[owner] = nil;
    return closed;
end

local function CloseAllAdmissionOwners()
    local confirmed = true;
    for owner=0,63 do
        local p = Player(owner, false);
        if p then
            local ok, value = pcall(p.GetProperty, p, ADMISSION_RESET + owner);
            if not ok or value ~= true then confirmed = false; end
        end
    end
    admissionSnapshots = {};
    return confirmed;
end

local function AdmissionOwnerIDsValid(ids)
    if type(ids) ~= "table" then return false; end
    local count, seen = 0, {};
    for slot, owner in pairs(ids) do
        if type(slot) ~= "number" or slot % 1 ~= 0 or slot < 1 or
           type(owner) ~= "number" or owner % 1 ~= 0 or owner < 0 or owner >= 64 or
           seen[owner] then return false; end
        seen[owner] = true;
        count = count + 1;
        if count > 64 then return false; end
    end
    if count == 0 then return false; end
    for slot=1,count do if ids[slot] == nil then return false; end end
    return true;
end

local function SyncProductionAdmission(forceClosed, onlyOwner)
    local rows = AdmissionRows();
    local ids = PlayerManager.GetAliveMajorIDs();
    if not AdmissionOwnerIDsValid(ids) then CloseAllAdmissionOwners(); return; end
    local schema = Game:GetProperty(ADMISSION_SCHEMA);
    if (schema ~= nil and schema ~= 1 and schema ~= 2 and schema ~= 3) or not rows then
        -- Even an invalid table/schema after reload must invalidate the old
        -- native generation. A reset read closes it without owning the value.
        CloseAllAdmissionOwners();
        return;
    end
    if schema == 1 then
        for owner=0,63 do
            local p = Player(owner, false);
            if p then
                for _,entry in ipairs(rows) do
                    if p:GetProperty(ADMISSION_CLOSE + entry.index) ~= nil then
                        CloseAllAdmissionOwners(); return;
                    end
                end
            end
        end
        Game:SetProperty(ADMISSION_SCHEMA, 2);
        schema = 2;
    end
    if schema == 2 then
        -- Version 2 owned only key 902000 on each player. Claim the new
        -- owner-specific reset keys only when no saved foreign value occupies
        -- one; the old key on owners 1..63 is left untouched.
        for owner=1,63 do
            local p = Player(owner, false);
            if p and p:GetProperty(ADMISSION_RESET + owner) ~= nil then
                CloseAllAdmissionOwners(); return;
            end
        end
        Game:SetProperty(ADMISSION_SCHEMA, 3);
        schema = 3;
    end
    local fingerprint = "JAC:";
    for _,entry in ipairs(rows) do
        fingerprint = fingerprint .. entry.row.LeaderType .. ":" .. tostring(entry.index) .. ";";
    end
    if admissionRowsFingerprint and admissionRowsFingerprint ~= fingerprint then
        for _,snapshot in pairs(admissionSnapshots) do CloseAdmissionOwner(snapshot.player); end
    end
    admissionRowsFingerprint = fingerprint;
    if schema == nil then
        -- Check every reserved namespace before taking it, including close.
        for _,owner in ipairs(ids) do
            local p = Player(owner, true);
            if not p or owner >= 64 or p:GetProperty(ADMISSION_RESET + owner) ~= nil then CloseAllAdmissionOwners(); return; end
            for _,entry in ipairs(rows) do
                for _,base in ipairs({ADMISSION_MANIFEST, ADMISSION_ALLOW, ADMISSION_CLOSE}) do
                    if p:GetProperty(base + entry.index) ~= nil then CloseAllAdmissionOwners(); return; end
                end
            end
        end
        Game:SetProperty(ADMISSION_SCHEMA, 3);
    end
    local alive = {};
    for _,owner in ipairs(ids) do alive[owner] = true; end
    for owner,snapshot in pairs(admissionSnapshots) do
        if not alive[owner] then CloseAdmissionOwner(snapshot.player); end
    end
    -- Attempt every initialization reset even if an earlier owner fails.
    -- No manifest can be published until the whole close phase is confirmed.
    local initialized = true;
    for _,owner in ipairs(ids) do
        local p = Player(owner, true);
        local snapshot = admissionSnapshots[owner];
        if not p or owner >= 64 then initialized = false;
        elseif not snapshot or snapshot.player ~= p then
            local resetOK, closed = pcall(CloseAdmissionOwner, p);
            if not resetOK or not closed then initialized = false;
            else admissionSnapshots[owner] = {player=p, values={}}; end
        end
    end
    if not initialized then CloseAllAdmissionOwners(); return; end
    for _,owner in ipairs(ids) do
        local p = Player(owner, true);
        local snapshot = admissionSnapshots[owner];
        if p and snapshot and (onlyOwner == nil or onlyOwner == owner) then
            local changes = {};
            for _,entry in ipairs(rows) do
                local desired = AdmissionAllowed(owner, entry.row, rows, forceClosed) == true;
                if snapshot.values[entry.index] ~= desired then
                    changes[#changes + 1] = {entry=entry, desired=desired};
                end
            end
            -- Close every affected cell before exposing any new value. Reading
            -- close invalidates it in the native consumer before the getter.
            local valid = true;
            for _,change in ipairs(changes) do
                local index = change.entry.index;
                p:SetProperty(ADMISSION_CLOSE + index, true);
                if p:GetProperty(ADMISSION_CLOSE + index) ~= true then valid = false; break; end
                p:SetProperty(ADMISSION_ALLOW + index, false);
                if p:GetProperty(ADMISSION_ALLOW + index) ~= false then valid = false; break; end
            end
            if not valid then CloseAdmissionOwner(p); return; end
            for _,change in ipairs(changes) do
                local index = change.entry.index;
                p:SetProperty(ADMISSION_MANIFEST + index, true);
                if p:GetProperty(ADMISSION_MANIFEST + index) ~= true then valid = false; break; end
                if change.desired then
                    p:SetProperty(ADMISSION_ALLOW + index, true);
                    if p:GetProperty(ADMISSION_ALLOW + index) ~= true then valid = false; break; end
                end
                snapshot.values[index] = change.desired;
            end
            if not valid then CloseAdmissionOwner(p); return; end
        end
    end
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
    if trait ~= nil and trait ~= row.Trait then return Finish(owner, "INVALID_METADATA"); end
    print("JAC_PERSONA_STAGE " .. leader .. " native-governor");
    local native = NativeGovernor(owner, row);
    if not native then return Finish(owner, "NATIVE_UNAVAILABLE"); end
    if native.appointed then return Finish(owner, "GOVERNOR_EXISTS"); end
    local city = p:GetCities():GetCapitalCity();
    if not city then return Finish(owner, "NO_CITY"); end
    print("JAC_PERSONA_STAGE " .. leader .. " find-person-before");
    local unit, ready = FindPerson(owner, row);
    if not ready then return Finish(owner, "NATIVE_UNAVAILABLE"); end
    if not unit then
        -- Gameplay grant: DLC/NubiaScenario/Scripts/NubiaScenario.lua:106-111.
        -- CreatePerson is a Tuner placement API and does not return here.
        local individual = GameInfo.GreatPersonIndividuals[row.PersonType];
        local class = GameInfo.GreatPersonClasses[individual.GreatPersonClassType];
        local era = GameInfo.Eras[individual.EraType];
        print("JAC_PERSONA_STAGE " .. leader .. " grant-person-before");
        Game.GetGreatPeople():GrantPerson(individual.Hash, class.Hash, era.Hash, 0, owner, false);
        print("JAC_PERSONA_STAGE " .. leader .. " grant-person-after");
        unit = FindPerson(owner, row);
    end
    if not unit then return Finish(owner, "SPAWN_FAILED"); end
    print("JAC_PERSONA_STAGE " .. leader .. " person-identified");
    Set(leader, "SOURCE", source);
    Set(leader, "NAME", row.Name);
    Set(leader, "PORTRAIT", row.PortraitIcon);
    -- Designed defaults are data, not a claim that historical psychology is known.
    Set(leader, "PERSONALITY", personality or row.Personality);
    Set(leader, "TRAIT", row.Trait);
    Set(leader, "EVOLUTION_COST", row.EvolutionCost);
    Set(leader, "UNIT", unit:GetID());
    Set(leader, "OWNER", owner);
    Set(leader, "STATE", "GREAT_PERSON");
    Set(leader, "MERIT", 0);
    if leader == ASHINA_ADMISSION then SyncAshinaAdmission(true); end
    SyncProductionAdmission(false, owner);
    return Finish(owner, "JOINED");
end

local function EvolutionCost(leader)
    local row = Definition(leader);
    return row and (Get(leader, "EVOLUTION_COST") or row.EvolutionCost);
end

local function AvailableTitles(p)
    local governors = p:GetGovernors();
    return governors:GetGovernorPoints() - governors:GetGovernorPointsSpent();
end

local function PrepareEvolution(owner, leader)
    local p, row = Player(owner, true), Definition(leader);
    if not p or not row or Get(leader, "OWNER") ~= owner then return Finish(owner, "NOT_OWNED"); end
    local state = Get(leader, "STATE");
    if state == "GOVERNOR" then return Finish(owner, "ALREADY_EVOLVED"); end
    if state == "EVOLVING" then return Finish(owner, "EVOLVING"); end
    if state ~= "GREAT_PERSON" then return Finish(owner, "PERSON_MISSING"); end
    local person, ready = FindPerson(owner, row);
    if not ready then return Finish(owner, "NATIVE_UNAVAILABLE"); end
    if not person then return Finish(owner, "PERSON_MISSING"); end
    local native = NativeGovernor(owner, row);
    if not native then return Finish(owner, "NATIVE_UNAVAILABLE"); end
    if native.appointed then return Finish(owner, "GOVERNOR_EXISTS"); end
    local cost = EvolutionCost(leader);
    -- Pending native operations reserve their still-unpaid titles, even across
    -- save/load. Both APPOINT and PROMOTE debit the engine's spent-title count.
    if AvailableTitles(p) - Rules.ReservedAppointments(owner) < cost then
        return Finish(owner, "NO_TITLE");
    end
    -- The static native trait is closed until this reservation commits. A
    -- pre-transition CanAppointGovernor check would make admission circular.
    -- The surcharge is the required count, not a synthetic balance deduction;
    -- its paid portion is read from the engine's own Governor promotions.
    Set(leader, "EVOLUTION_SURCHARGE", cost - 1);
    Set(leader, "STATE", "EVOLVING");
    if leader == ASHINA_ADMISSION then SyncAshinaAdmission(false); end
    SyncProductionAdmission(false, owner);
    return Finish(owner, "EVOLVING");
end

local function CompleteEvolution(owner, leader)
    local p, row = Player(owner, true), Definition(leader);
    if not p or not row or Get(leader, "OWNER") ~= owner then return Finish(owner, "NOT_OWNED"); end
    if Get(leader, "STATE") == "GOVERNOR" then return Finish(owner, "ALREADY_EVOLVED"); end
    if Get(leader, "STATE") ~= "EVOLVING" then return Finish(owner, "NOT_EVOLVING"); end
    local native = NativeGovernor(owner, row);
    if not native or not native.appointed or not native.base then return Finish(owner, "APPOINTMENT_PENDING"); end
    if native.surchargePaid ~= (Get(leader, "EVOLUTION_SURCHARGE") or 0) then
        return Finish(owner, "TITLE_PAYMENT_PENDING");
    end
    -- Retain the Great Person until the engine has really appointed its Governor.
    local unit, ready = FindPerson(owner, row);
    if not ready then return Finish(owner, "REMOVAL_PENDING"); end
    if leader == ASHINA_ADMISSION then SyncAshinaAdmission(true); end
    SyncProductionAdmission(true, owner);
    if unit then p:GetUnits():Destroy(unit); end -- TunerMapPanel.lua:199, also accepts no return value
    local remaining, observed = FindPerson(owner, row);
    if remaining or not observed then return Finish(owner, "REMOVAL_PENDING"); end
    Set(leader, "UNIT", nil);
    Set(leader, "STATE", "GOVERNOR");
    if leader == ASHINA_ADMISSION then SyncAshinaAdmission(false); end
    SyncProductionAdmission(false, owner);
    SnapshotAlliances(owner, leader);
    return Finish(owner, "EVOLVED");
end

local function Reconcile(onlyOwner)
    SyncAshinaAdmission(false);
    for row in GameInfo.JAC_LeaderPersonas() do
        local owner = Get(row.LeaderType, "OWNER");
        if owner ~= nil and (onlyOwner == nil or onlyOwner == owner) and Get(row.LeaderType, "STATE") == "EVOLVING" then
            local native = NativeGovernor(owner, row);
            local unit, ready = FindPerson(owner, row);
            if native and not native.appointed and ready and not unit then
                local turn = Game.GetCurrentGameTurn();
                local first = Get(row.LeaderType, "MISSING_SINCE_TURN");
                if first == nil then
                    Set(row.LeaderType, "MISSING_SINCE_TURN", turn);
                elseif turn > first then
                    -- No native appointment or title was consumed. Release
                    -- only this person's unpaid reservation after a second
                    -- synchronized observation; keep the ownership record.
                    Set(row.LeaderType, "STATE", "GREAT_PERSON");
                    Set(row.LeaderType, "EVOLUTION_SURCHARGE", nil);
                    Set(row.LeaderType, "UNIT", nil);
                    Set(row.LeaderType, "MISSING_SINCE_TURN", nil);
                    Finish(owner, "PERSON_MISSING");
                end
            else
                if Get(row.LeaderType, "MISSING_SINCE_TURN") ~= nil then
                    Set(row.LeaderType, "MISSING_SINCE_TURN", nil);
                end
                CompleteEvolution(owner, row.LeaderType);
            end
        end
    end
    SyncProductionAdmission(false, onlyOwner);
end

local function Parameter(name) return GameInfo.JAC_PersonaTreeParameters[name].Value; end
local function SettledCity(owner, row)
    if Get(row.LeaderType, "STATE") ~= "GOVERNOR" then return nil, true; end
    local p = Player(owner, true);
    if not p then return nil, true; end
    local marker = "JAC_PERSONA_SETTLED_" .. row.LeaderType;
    local settled;
    for _,city in p:GetCities():Members() do
        if city:GetOwner() == owner then
            if type(city.GetProperty) ~= "function" then return nil, false; end
            local ok, value = pcall(city.GetProperty, city, marker);
            if not ok or (value ~= nil and value ~= 0 and value ~= 1 and value ~= false) then
                return nil, false;
            end
            if value == 1 then
                if settled then return nil, false; end -- contradictory native markers
                settled = city;
            end
        end
    end
    -- Zero markers is a valid unassigned, establishing or neutralized state.
    -- An old saved pending deed remains unknown; never reinterpret it here.
    return settled, true;
end

-- Flag all owned city centres, so native reassignment immediately carries purchased skills.
-- The native base's COLLECTION_OWNER selects ONLY its established city; Lua never adds yields.
-- Plot properties: CivRoyaleScenario_Civilizations.xml:392-405 / CivRoyaleScenario_StartScript.lua:1287.
local function SyncEffects()
    for node in GameInfo.JAC_PersonaTreeNodes() do
        if Rules.Owned(node) then
            local p = Player(Get(node.LeaderType, "OWNER"), true);
            if p then
                for _,city in p:GetCities():Members() do
                    local plot = Map.GetPlot(city:GetX(), city:GetY());
                    if plot and plot:GetProperty(node.ModifierId) ~= 1 then plot:SetProperty(node.ModifierId, 1); end
                end
            end
        end
    end
end

local function Purchase(owner, leader, nodeType, confirmCategory, confirmRoute)
    local p = Player(owner, true);
    if not p then return Finish(owner, "NOT_OWNED"); end
    local status = Rules.Status(owner, leader, nodeType);
    if status ~= "AVAILABLE" then return Finish(owner, status); end
    local node = GameInfo.JAC_PersonaTreeNodes[nodeType];
    if (not Get(leader, "CATEGORY") and confirmCategory ~= node.CategoryType) or
       (node.ExclusiveGroup and not Get(leader, "BRANCH_" .. node.CategoryType .. "_" .. node.ExclusiveGroup) and
        confirmRoute ~= node.RouteType) then return Finish(owner, "CONFIRM_REQUIRED"); end
    -- No refund/respec entry point. Serial gameplay request rechecks balance/locks before writing.
    Set(leader, "MERIT", Rules.Merit(leader) - node.Cost);
    Set(leader, "CATEGORY", node.CategoryType);
    if node.ExclusiveGroup then Set(leader, "BRANCH_" .. node.CategoryType .. "_" .. node.ExclusiveGroup, node.RouteType); end
    Set(leader, "NODE_" .. nodeType, 1);
    SyncEffects();
    return Finish(owner, "PURCHASED");
end

local function Award(row, deed, identity, city)
    local leader = row.LeaderType;
    local owner = Get(leader, "OWNER");
    -- A repeated notification cannot turn an unproven historical deed into a
    -- current eligible one, even after reassignment or reload in the same turn.
    local pending = Game:GetProperty(PENDING) or {};
    if pending[leader .. "_" .. deed .. "_" .. identity] then return false; end
    if not Player(owner, true) or not city then return false; end
    local design = GameInfo.JAC_PersonaTreePeople[leader];
    local rule = GameInfo.JAC_PersonaMeritRules[design.MeritProfile];
    local turn = Game.GetCurrentGameTurn();
    local seen = "DEED_" .. deed .. "_" .. identity;
    if Get(leader, seen) ~= nil then return false; end
    local cap = deed == "GOVERN_TURN" and 1 or Parameter(deed .. "_DEEDS_PER_TURN");
    local count = Get(leader, deed .. "_COUNT_TURN") == turn and (Get(leader, deed .. "_COUNT") or 0) or 0;
    -- Remember identities even at the cap; queued duplicate notifications cannot pay next turn.
    Set(leader, seen, turn);
    if count >= cap then return false; end
    Set(leader, deed .. "_COUNT_TURN", turn);
    Set(leader, deed .. "_COUNT", count + 1);
    Set(leader, "MERIT", Rules.Merit(leader) + rule[deed]);
    return true;
end

-- Pending notifications are immutable gameplay identities, not UI-provided eligibility.
-- Keep them through save/load. A later read cannot establish event-time eligibility.
local function Defer(row, deed, identity, cityID, x, y)
    local pending = Game:GetProperty(PENDING) or {};
    local key = row.LeaderType .. "_" .. deed .. "_" .. identity;
    if not pending[key] then
        local sequence = (Game:GetProperty("JAC_PERSONA_PENDING_SEQUENCE") or 0) + 1;
        Game:SetProperty("JAC_PERSONA_PENDING_SEQUENCE", sequence);
        pending[key] = {sequence=sequence, leader=row.LeaderType, owner=Get(row.LeaderType, "OWNER"),
            deed=deed, identity=identity, turn=Game.GetCurrentGameTurn(), cityID=cityID, x=x, y=y,
            eligibilityUnknown=true};
        Game:SetProperty(PENDING, pending);
        -- The missing event-time state cannot be recovered by a later UI read.
        Game:SetProperty("JAC_PERSONA_PENDING_NATIVE_HISTORY", 1);
    end
end
-- Saves made before the history flag was written may already contain unknown
-- events. Mark them on load without asking the UI to reinterpret their state.
local function MarkPendingHistory()
    local pending = Game:GetProperty(PENDING);
    if type(pending) == "table" and next(pending) and Game:GetProperty("JAC_PERSONA_PENDING_NATIVE_HISTORY") ~= 1 then
        Game:SetProperty("JAC_PERSONA_PENDING_NATIVE_HISTORY", 1);
    end
end
MarkPendingHistory();
local function HandleDeed(row, deed, identity, cityID, x, y)
    local leader = row.LeaderType;
    local owner = Get(leader, "OWNER");
    if Get(leader, "STATE") ~= "GOVERNOR" or not Player(owner, true) then return; end
    if Get(leader, "DEED_" .. deed .. "_" .. identity) ~= nil then return; end
    local pending = Game:GetProperty(PENDING) or {};
    if pending[leader .. "_" .. deed .. "_" .. identity] then return; end
    local city, ready = SettledCity(owner, row);
    if not ready then Defer(row, deed, identity, cityID, x, y); return; end
    if not city or (deed == "BUILDING" and city:GetID() ~= cityID) or
       (deed == "VICTORY" and Map.GetPlotDistance(city:GetX(), city:GetY(), x, y) > Parameter("VICTORY_RADIUS")) then
        -- A replay after assignment/recovery cannot turn this event into a deed.
        Set(leader, "DEED_" .. deed .. "_" .. identity, Game.GetCurrentGameTurn());
        return;
    end
    Award(row, deed, identity, city);
end
local function MajorIDs()
    local ids = PlayerManager.GetAliveMajorIDs(); -- BlackDeathScenario.lua:461.
    table.sort(ids);
    return ids;
end
SnapshotAlliances = function(owner, leader)
    if Get(leader, "ALLIANCES_BASELINED") then return; end
    local diplomacy = Players[owner]:GetDiplomacy();
    for _,other in ipairs(MajorIDs()) do
        if other ~= owner and diplomacy:HasAllied(other) then Set(leader, "ALLY_SEEN_" .. other, 1); end
    end
    Set(leader, "ALLIANCES_BASELINED", 1);
end

-- HasAllied reports the present relationship, not when it began. A turn scan
-- cannot prove the governor's state at formation, even if a UI reader works
-- now. Persist the first observation once; never pay it from a later state.
local function ObserveAlliances(owner, row)
    local leader = row.LeaderType;
    if not Get(leader, "ALLIANCES_BASELINED") then SnapshotAlliances(owner, leader); end
    local diplomacy = Players[owner]:GetDiplomacy();
    for _,other in ipairs(MajorIDs()) do
        if other ~= owner and diplomacy:HasAllied(other) and not Get(leader, "ALLY_SEEN_" .. other) then
            Set(leader, "ALLY_SEEN_" .. other, 1);
            Defer(row, "ALLIANCE", tostring(other));
        end
    end
end

-- Formation is the only moment at which eligibility can be decided. The turn
-- scan above records a missed formation as unknown; it never awards it late.
local function OnDiplomacyRelationshipChanged(first, second)
    if not Player(first, true) or not Player(second, true) or first == second then return; end
    for _,pair in ipairs({{first, second}, {second, first}}) do
        local owner, other = pair[1], pair[2];
        if Players[owner]:GetDiplomacy():HasAllied(other) then
            for row in GameInfo.JAC_LeaderPersonas() do
                local leader = row.LeaderType;
                if Get(leader, "OWNER") == owner and Get(leader, "STATE") == "GOVERNOR" then
                    if not Get(leader, "ALLIANCES_BASELINED") then
                        SnapshotAlliances(owner, leader); -- old save: formation time is unknown
                    elseif not Get(leader, "ALLY_SEEN_" .. other) then
                        Set(leader, "ALLY_SEEN_" .. other, 1);
                        local city, ready = SettledCity(owner, row);
                        if not ready then Defer(row, "ALLIANCE", tostring(other));
                        elseif city then Award(row, "ALLIANCE", tostring(other), city); end
                    end
                end
            end
        end
    end
end

local function ChooseAI(owner, row)
    local leader = row.LeaderType;
    local preferred = GameInfo.JAC_PersonaTreePeople[leader];
    local category = Get(leader, "CATEGORY") or preferred.PreferredCategory;
    local candidates = {};
    for _,node in ipairs(Rules.Nodes(leader, category)) do
        if Rules.Status(owner, leader, node.NodeType) == "AVAILABLE" then table.insert(candidates, node); end
    end
    local function Rank(node)
        if node.RouteType == preferred.PreferredRoute then return 0; end
        if node.Signature == 1 then return 1; end
        return 2;
    end
    table.sort(candidates, function(a,b)
        if Rank(a) ~= Rank(b) then return Rank(a) < Rank(b); end
        if a.Depth ~= b.Depth then return a.Depth < b.Depth; end
        return a.NodeType < b.NodeType;
    end);
    local budget = Parameter("AI_PURCHASES_PER_TURN");
    local spent = 0;
    for _,node in ipairs(candidates) do
        if spent >= budget then break; end
        if Purchase(owner, leader, node.NodeType, node.CategoryType, node.RouteType) == "PURCHASED" then spent = spent + 1; end
    end
end

OnPlayerTurnStarted = function(owner)
    Reconcile(owner); SyncEffects();
    local p = Player(owner, true);
    if not p then return; end
    local turn = Game.GetCurrentGameTurn();
    -- Reserve only from the official title balance, in stable person order.
    -- Whether this callback precedes native AI governor analysis needs the
    -- parent-owned live trace; the static trait remains closed without a bridge.
    if not p:IsHuman() then
        local candidates = {};
        for row in GameInfo.JAC_LeaderPersonas() do
            if Get(row.LeaderType, "OWNER") == owner and Get(row.LeaderType, "STATE") == "GREAT_PERSON" then
                table.insert(candidates, row.LeaderType);
            end
        end
        table.sort(candidates);
        for _,leader in ipairs(candidates) do PrepareEvolution(owner, leader); end
    end
    for row in GameInfo.JAC_LeaderPersonas() do
        local leader = row.LeaderType;
        if Get(leader, "OWNER") == owner and Get(leader, "STATE") == "GOVERNOR" then
            local pending = Game:GetProperty(PENDING) or {};
            local city, ready = SettledCity(owner, row);
            if Get(leader, "GOVERN_LAST_TURN") ~= turn then
                Set(leader, "GOVERN_LAST_TURN", turn); -- the eligibility decision is final for this turn
                if not ready or pending[leader .. "_TURN_" .. tostring(turn)] then
                    Defer(row, "TURN", tostring(turn));
                elseif city then
                    Award(row, "GOVERN_TURN", tostring(turn), city);
                end
            end
            ObserveAlliances(owner, row);
            -- AI spends only the committed gameplay state and saved merit.
            if not p:IsHuman() and Get(leader, "AI_LAST_TURN") ~= turn then
                Set(leader, "AI_LAST_TURN", turn);
                ChooseAI(owner, row);
            end
        end
    end
end

-- Original construction only. Repairs and duplicate building events earn nothing.
-- Signature: DLC/BlackDeathScenario/Scripts/BlackDeathScenario.lua:362.
local function OnBuildingConstructed(owner, cityID, buildingIndex, plotIndex, original)
    if original ~= true or not GameInfo.Buildings[buildingIndex] then return; end
    for row in GameInfo.JAC_LeaderPersonas() do
        if Get(row.LeaderType, "OWNER") == owner then
            HandleDeed(row, "BUILDING", tostring(plotIndex) .. "_" .. tostring(buildingIndex), cityID);
        end
    end
end

-- Resolve the dying loser while the engine still exposes it, exactly as the official Pirates
-- scenario does (PiratesScenario_StartScript.lua:1240-1265,1617). District battles earn no kill.
local function OnCombatOccurred(attacker, attackerUnit, defender, defenderUnit)
    local a, d = Players[attacker], Players[defender];
    if not a or not d or attacker == defender then return; end
    local au, du = a:GetUnits():FindID(attackerUnit), d:GetUnits():FindID(defenderUnit);
    if not au or not du then return; end
    local function Dead(unit) return unit:IsDead() or unit:IsDelayedDeath(); end
    local winner, loser;
    if Dead(du) and not Dead(au) then winner, loser = attacker, du;
    elseif Dead(au) and not Dead(du) then winner, loser = defender, au;
    else return; end
    for row in GameInfo.JAC_LeaderPersonas() do
        if Get(row.LeaderType, "OWNER") == winner then
            HandleDeed(row, "VICTORY", tostring(loser:GetOwner()) .. "_" .. tostring(loser:GetID()) .. "_" .. Game.GetCurrentGameTurn(),
                nil, loser:GetX(), loser:GetY());
        end
    end
end

-- UI integrations submit this synchronized operation; native creation runs in
-- the gameplay script rather than a cross-context FireTuner function call.
GameEvents.JAC_LeaderPersonaJoin.Add(function(owner, params)
    print("JAC_PERSONA_STAGE join-operation owner=" .. tostring(owner));
    Join(owner, type(params) == "table" and params.Source or nil);
end);
GameEvents.JAC_LeaderPersonaEvolve.Add(function(owner, params)
    PrepareEvolution(owner, type(params) == "table" and params.Leader or nil);
end);
GameEvents.JAC_LeaderPersonaFinalize.Add(function(owner, params)
    CompleteEvolution(owner, type(params) == "table" and params.Leader or nil);
end);
GameEvents.OnGameTurnStarted.Add(function() Reconcile(); end);
GameEvents.OnGameTurnStarted.Add(SyncEffects);
GameEvents.PlayerTurnStarted.Add(OnPlayerTurnStarted);
GameEvents.BuildingConstructed.Add(OnBuildingConstructed);
GameEvents.OnCombatOccurred.Add(OnCombatOccurred);
-- leaderperson-21 observed this native Events callback in GameCore after SetHasAllied.
-- GameEvents.OnDiplomacyRelationshipChanged did not fire in the same probe.
if Events.DiplomacyRelationshipChanged then
    Events.DiplomacyRelationshipChanged.Add(OnDiplomacyRelationshipChanged);
end
GameEvents.CityBuilt.Add(SyncEffects); -- AustraliaScenario.lua:1465.
GameEvents.CityConquered.Add(SyncEffects); -- AlexanderScenario.lua:248.
GameEvents.JAC_LeaderPersonaPurchase.Add(function(owner, params)
    if type(params) ~= "table" then return; end
    local result = Purchase(owner, params.Leader, params.Node, params.ConfirmCategory, params.ConfirmRoute);
    local p = Player(owner, false);
    if p then
        -- A receipt only for this purchase, independent of evolution/assignment results.
        -- Never persist empty strings; malformed request metadata uses nil.
        p:SetProperty("JAC_PERSONA_PURCHASE_RESULT", result);
        p:SetProperty("JAC_PERSONA_PURCHASE_LEADER", type(params.Leader) == "string" and #params.Leader > 0 and params.Leader or nil);
        p:SetProperty("JAC_PERSONA_PURCHASE_NODE", type(params.Node) == "string" and #params.Node > 0 and params.Node or nil);
        local serial = params.RequestSerial;
        p:SetProperty("JAC_PERSONA_PURCHASE_SERIAL", type(serial) == "number" and serial > 0 and serial % 1 == 0 and serial or nil);
    end
end);
ExposedMembers.JAC_LeaderPersona = {
    Join = Join, PrepareEvolution = PrepareEvolution, CompleteEvolution = CompleteEvolution,
    Reconcile = Reconcile, EvolutionCost = EvolutionCost,
    CoreAdmissionCapabilities = CoreAdmissionCapabilities,
    OnPlayerTurnStarted = OnPlayerTurnStarted, SyncEffects = SyncEffects
};
print("JAC_LeaderPersona_Gameplay: ready");
SyncProductionAdmission(true);
