-- Test-only read/validation rules. No native or persona state is written here.
JAC_PersonaBoundaryRules = {};
local R = JAC_PersonaBoundaryRules;
include("JAC_PersonaCompletedFailure");
R.CompletedNativeFailure=function(initial,escape)
    return assert(JAC_PersonaCompletedFailure,"Rust generated native failure policy missing")(initial,escape);
end;
function R.IsCurrentSpyGeneration(retry)
    return retry.current==2 or (R.Third and R.Third.IsThird(retry)) or false;
end
function R.IsArchivedSpyEvent(retry,id)
    return id==retry.archive.eventID or (R.Third and R.Third.ArchivedEvent(retry,id)) or false;
end
-- EXECUTE_SCRIPT converts an empty string to numeric zero in live 146.
-- Prefix text so empty and numeric-looking names retain their exact identity.
local TEXT_PREFIX = "JAC_TEXT_V1:";
function R.EncodeText(value)
    if value == nil then return nil; end
    assert(type(value) == "string", "native observation text unavailable");
    return TEXT_PREFIX .. value;
end
function R.DecodeText(value)
    if value == nil then return nil; end
    assert(type(value) == "string" and string.sub(value, 1, #TEXT_PREFIX) == TEXT_PREFIX,
        "native observation text encoding differs");
    return string.sub(value, #TEXT_PREFIX + 1);
end
-- New receipts keep encoded text through Game:SetProperty/save/UI publication.
-- Earlier nonencoded receipts remain readable only with an actual string.
function R.StoredText(record, field)
    local encoded = record[field .. "Text"];
    if encoded ~= nil then return R.DecodeText(encoded); end
    assert(type(record[field]) == "string", "saved native observation text unavailable");
    return record[field];
end
-- Resolve a saved identity inside the calling Lua context. FindID alone may
-- return a different native unit; do not expose it to reads or operations.
function R.FindSpy(owner, id)
    local player = Players[owner];
    local units = player and player:GetUnits();
    local spy = units and units:FindID(id);
    local lookupID = spy and spy:GetID();
    if not spy or lookupID ~= id or spy:GetOwner() ~= owner or
        spy:GetType() ~= GameInfo.Units["UNIT_SPY"].Index then
        return nil, lookupID;
    end
    return spy, lookupID;
end
-- A rejected lookup is not an absence receipt. Production requires a fresh
-- complete map inventory in Gameplay, independently of FindID's aliases.
function R.MapSpyCount(owner)
    local player = assert(Players[owner], "native map roster owner unavailable");
    local units = assert(player:GetUnits(), "native map roster unavailable");
    local count = 0;
    for _, unit in units:Members() do
        local def = assert(GameInfo.Units[unit:GetType()], "native map unit type unavailable");
        if def.Spy then
            assert(unit:GetOwner() == owner, "native map spy owner differs");
            count = count + 1;
        end
    end
    return count;
end
function R.Property(leader, field)
    local value = Game:GetProperty("JAC_PERSONA_" .. leader .. "_" .. field);
    return value;
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
    if phase~="NEUTRALIZED" then
        assert(s.native.established == active, "unexpected native establishment state");
    end
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
include("JAC_PersonaThirdSpy");
