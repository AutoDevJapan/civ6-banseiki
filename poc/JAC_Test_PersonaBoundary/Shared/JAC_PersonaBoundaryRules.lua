-- Test-only read/validation rules. No native or persona state is written here.
JAC_PersonaBoundaryRules = {};
local R = JAC_PersonaBoundaryRules;
function R.Property(leader, field)
    local value = Game:GetProperty("JAC_PERSONA_" .. leader .. "_" .. field);
    return value;
end

-- The sole person -> native slot resolution seam in this diagnostic.
-- A future generic-slot implementation can change this resolver without changing
-- the observer, spy requests or phase assertions. Never use the persona UI bridge.
function R.ResolveGovernor(owner, leader)
    local row = assert(GameInfo.JAC_LeaderPersonas[leader], "persona definition missing");
    local def = assert(GameInfo.Governors[row.GovernorType], "native governor definition missing");
    local player = assert(Players[owner], "native governor owner missing");
    local governors = player:GetGovernors();
    assert(governors:HasGovernor(def.Hash), "native governor not appointed");
    local governor = assert(governors:GetGovernor(def.Hash), "native governor instance missing");
    assert(governor:GetType() == def.Index, "native governor type mismatch");
    return governor, def, row;
end

function R.Serialize(value)
    if type(value) ~= "table" then
        if type(value) == "number" then return string.format("%.6f", value); end
        return tostring(value);
    end
    local keys, parts = {}, {};
    for key in pairs(value) do keys[#keys+1] = key; end
    table.sort(keys, function(a,b) return tostring(a) < tostring(b); end);
    for _,key in ipairs(keys) do parts[#parts+1] = tostring(key) .. "=" .. R.Serialize(value[key]); end
    return "{" .. table.concat(parts, ",") .. "}";
end

function R.Check(s, phase, merit)
    assert(s.personOwner == s.owner and s.state == "GOVERNOR" and s.greatPeople == 0,
        "persona identity does not match its native Governor");
    assert(s.native.base and (not s.requireSkill or s.skillOwned == 1),
        "native base or purchased fixture skill missing");
    assert(s.merit == s.gameplay.merit, "UI/Gameplay merit disagreement");
    assert(type(s.unmatchedEffects)=="table" and not next(s.unmatchedEffects),
        "active native effect has an unmatched city subject");
    if merit ~= nil then assert(s.merit == merit, "merit changed while effects must be stopped"); end
    local active = phase == "SETTLED";
    assert(active or phase == "NEUTRALIZED" or phase == "RECOVERED" or
        phase == "ESTABLISHING" or phase == "LOST", "unknown diagnostic phase");
    if phase == "NEUTRALIZED" then
        assert(s.native.neutralized > 0, "native neutralization was not observed");
        assert(s.neutralizationMission, "successful native mission history was not observed");
    else
        assert(s.native.neutralized == 0, "native governor is still neutralized");
    end
    if active or phase == "ESTABLISHING" then
        assert(s.native.cityOwner == s.owner and s.native.cityID == s.target.id,
            "native assignment does not point to the fixture city");
    elseif phase == "NEUTRALIZED" then
        assert((s.native.cityOwner == -1 and s.native.cityID == -1) or
            (s.native.cityOwner == s.owner and s.native.cityID == s.target.id),
            "neutralized native Governor points to another city");
    else
        assert(s.native.cityOwner == -1 and s.native.cityID == -1,
            "governor must be unassigned; automatic return is not recovery");
    end
    assert(s.native.established == active, "unexpected native establishment state");
    if phase == "LOST" then
        assert(type(s.transfer)=="table" and s.transfer.fromOwner==s.owner and
            s.transfer.fromCityID==s.fixtureCityID and s.transfer.toOwner==s.freeCitiesOwner and
            s.target.id==s.transfer.toCityID and s.target.x==s.transfer.x and s.target.y==s.transfer.y,
            "native transfer city identity was not recorded");
        assert(s.target.owner == s.freeCitiesOwner and s.target.owner ~= s.owner,
            "native city ownership transfer was not observed");
    else
        assert(s.target.owner == s.owner, "fixture city unexpectedly changed owner");
    end
    assert(type(s.cities)=="table" and type(s.gameplay.cities)=="table", "city observation collection missing");
    for key in pairs(s.gameplay.cities) do
        assert(s.cities[key], "Gameplay-only city observation at " .. key);
    end;
    local count = 0;
    for key,city in pairs(s.cities) do
        count = count + 1;
        local expected = active and key == s.targetKey and 1 or 0;
        assert(city.settled == expected, "settlement marker disagrees at " .. key);
        local gameplay = assert(s.gameplay.cities[key], "Gameplay city observation missing: " .. key);
        assert(type(city.id)=="number" and type(city.owner)=="number" and
            gameplay.id==city.id and gameplay.owner==city.owner,
            "UI/Gameplay city identity disagreement at " .. key);
        if key==s.targetKey then
            assert(gameplay.id==s.target.id and gameplay.owner==s.target.owner,
                "Gameplay target city identity disagreement");
        end;
        assert(gameplay.settled == city.settled, "UI/Gameplay marker disagreement at " .. key);
        for modifier,subjects in pairs(city.effects) do
            assert(subjects == expected, "native city effect remains/missing: " .. key .. "/" .. modifier);
        end
        assert(next(city.effects), "native modifier inventory is empty");
        for yield,value in pairs(city.yields) do
            assert(type(value) == "number" and value == value and math.abs(value) < math.huge,
                "native yield unavailable: " .. key .. "/" .. yield);
        end
        assert(next(city.yields), "native yield inventory is empty");
        for modifier,value in pairs(city.selectors) do
            assert(gameplay.selectors[modifier] == value, "UI/Gameplay skill selector disagreement");
        end
    end
    assert(count >= (s.controlRequired and 2 or 1),
        "target and required control city observations are missing");
    return true;
end
