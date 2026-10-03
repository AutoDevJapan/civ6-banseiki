-- Opt-in test MOD only. Mutations run through synchronized EXECUTE_SCRIPT.
-- Primary API evidence and unresolved spy control: documents/poc/pr86-boundary-diagnostic.md.
include("JAC_PersonaBoundaryRules");
local R = JAC_PersonaBoundaryRules;
local LEADER = "LEADER_JAC_ODA_NOBUHIDE";
local CONFIG = "JAC_BOUNDARY_CONFIG";
local function Config(owner)
    local c = assert(Game:GetProperty(CONFIG), "boundary fixture not prepared");
    assert(c.owner == owner and Players[owner]:IsHuman(), "fixture operation requires its human owner");
    return c;
end
local function GetCities(c)
    local cities = {};
    local ids = PlayerManager.GetAliveMajorIDs();
    ids[#ids+1] = PlayerManager.GetFreeCitiesPlayerID();
    for _,id in ipairs(ids) do
        local p = Players[id];
        if p then for _,city in p:GetCities():Members() do
            if id == c.owner or (city:GetX() == c.x and city:GetY() == c.y) then
                cities[city:GetX() .. "/" .. city:GetY()] = city;
            end
        end end
    end
    return cities;
end
local function ReadGameplay()
    local c = assert(Game:GetProperty(CONFIG), "boundary fixture not prepared");
    local result = {merit=R.Property(c.leader, "MERIT") or 0,
        freeCitiesOwner=PlayerManager.GetFreeCitiesPlayerID(), cities={}};
    for key,city in pairs(GetCities(c)) do
        local value = city:GetProperty("JAC_PERSONA_SETTLED_" .. c.leader);
        local data = {id=city:GetID(), owner=city:GetOwner(), settled=value or 0, selectors={}};
        local plot = Map.GetPlot(city:GetX(), city:GetY());
        for node in GameInfo.JAC_PersonaTreeNodes() do
            if node.LeaderType == c.leader and R.Property(c.leader, "NODE_" .. node.NodeType) == 1 then
                local selector = plot:GetProperty(node.ModifierId);
                data.selectors[node.ModifierId] = selector or 0;
            end
        end
        result.cities[key] = data;
    end
    return result;
end

GameEvents.JAC_BoundaryPrepare.Add(function(owner)
    assert(not Game:GetProperty(CONFIG), "fixture already prepared; start an independent game");
    local player = assert(Players[owner], "fixture owner missing");
    assert(player:IsHuman() and player:IsAlive(), "fixture needs a living human owner");
    local capital = assert(player:GetCities():GetCapitalCity(), "fixture capital missing");
    local candidates = {};
    for _,city in player:GetCities():Members() do
        if city:GetID() ~= capital:GetID() then candidates[#candidates+1] = city; end
    end
    table.sort(candidates, function(a,b) return a:GetID() < b:GetID(); end);
    local city = assert(candidates[1], "fixture requires a noncapital city and a control capital");
    local source;
    for _,id in ipairs(PlayerManager.GetAliveMajorIDs()) do
        if PlayerConfigurations[id]:GetLeaderTypeName() == LEADER then source = id; end
    end
    assert(source and source ~= owner, "Oda must join from another living house");
    local row = GameInfo.JAC_LeaderPersonas[LEADER];
    assert(row.EvolutionCost == 1, "fixture requires Nobuhide's one-title cost");
    local governors, culture = player:GetGovernors(), player:GetCulture();
    local total, spent = governors:GetGovernorPoints(), governors:GetGovernorPointsSpent();
    if total - spent < 1 then
        local civics = {};
        for title in GameInfo.JAC_PersonaTitleCivics() do civics[#civics+1] = title.CivicType; end
        table.sort(civics);
        local granted = false;
        for _,type in ipairs(civics) do
            local civic = GameInfo.Civics[type];
            if not culture:HasCivic(civic.Index) then
                culture:SetCulturalProgress(civic.Index, culture:GetCultureCost(civic.Index));
                assert(culture:HasCivic(civic.Index) and governors:GetGovernorPoints() == total+1 and
                    governors:GetGovernorPointsSpent() == spent, "native civic did not grant exactly one title");
                granted = true; break;
            end
        end
        assert(granted, "one unresearched title civic required");
    end
    assert(ExposedMembers.JAC_LeaderPersona.Join(owner, source) == "JOINED", "regular joining failed");
    Game:SetProperty(CONFIG, {owner=owner, leader=LEADER, source=source, cityID=city:GetID(),
        x=city:GetX(), y=city:GetY(), controlID=capital:GetID(), armedTurn=Game.GetCurrentGameTurn(),
        skill=LEADER .. "_SIGNATURE_1", total=governors:GetGovernorPoints(), spent=spent});
    print("JAC_BOUNDARY PREPARED " .. R.Serialize(Game:GetProperty(CONFIG)));
end);

GameEvents.JAC_BoundaryLoseCity.Add(function(owner)
    local c = Config(owner);
    assert(R.Property(c.leader, "OWNER") == owner and R.Property(c.leader, "STATE") == "GOVERNOR",
        "regular persona evolution required before city loss");
    local city = assert(CityManager.GetCity(owner, c.cityID), "old owned city missing");
    assert(city:GetX() == c.x and city:GetY() == c.y, "fixture city identity changed");
    assert(city:GetProperty("JAC_PERSONA_SETTLED_" .. c.leader) == 1, "settled native marker required");
    assert(R.Property(c.leader, "NODE_" .. c.skill) == 1, "purchased skill required before loss");
    -- Official Debug/City.ltp:275-278. Do not simulate owner/property changes.
    print("JAC_BOUNDARY TRANSFER_REQUEST owner=" .. owner .. " city=" .. c.cityID);
    CityManager.TransferCityToFreeCities(city);
    local freeOwner=PlayerManager.GetFreeCitiesPlayerID();
    local transferred;
    for _,candidate in Players[freeOwner]:GetCities():Members() do
        if candidate:GetX()==c.x and candidate:GetY()==c.y then
            assert(not transferred, "ambiguous native transfer result");
            transferred=candidate;
        end;
    end;
    assert(transferred, "native transfer result city missing");
    -- Record native source/result IDs in the same synchronized operation.
    -- City IDs can be scoped to their owner; do not infer the new ID from a
    -- later city occupying this plot or require an unsupported ID invariant.
    c.transfer={fromOwner=owner, fromCityID=c.cityID, toOwner=freeOwner,
        toCityID=transferred:GetID(), x=c.x, y=c.y};
    Game:SetProperty(CONFIG,c);
    print("JAC_BOUNDARY TRANSFER_RETURN " .. R.Serialize(ReadGameplay()));
end);

GameEvents.JAC_BoundaryUnlockSpy.Add(function(owner)
    local player = assert(Players[owner], "spy owner missing");
    assert(player:IsHuman() and player:IsAlive(), "spy fixture requires a living human owner");
    local culture = player:GetCulture();
    local civic = assert(GameInfo.Civics["CIVIC_DIPLOMATIC_SERVICE"], "native spy civic missing");
    if not culture:HasCivic(civic.Index) then
        -- Base/Assets/Gameplay/Data/Civics.xml grants a spy slot for this civic.
        -- Progress is the native completion path used by the test-only fixture.
        culture:SetCulturalProgress(civic.Index, culture:GetCultureCost(civic.Index));
    end
    assert(culture:HasCivic(civic.Index), "native spy civic did not complete");
    print("JAC_BOUNDARY SPY_CIVIC_COMPLETED " .. R.Serialize({owner=owner,
        civic=civic.CivicType}));
end);

GameEvents.JAC_BoundaryFinishSpy.Add(function(owner)
    local player = assert(Players[owner], "spy owner missing");
    assert(player:IsHuman() and player:IsAlive(), "spy fixture requires a living human owner");
    assert(not Game:GetProperty("JAC_BOUNDARY_SPY"), "fixture spy already recorded");
    local capital = assert(player:GetCities():GetCapitalCity(), "production city missing");
    local unitType = assert(GameInfo.Units["UNIT_SPY"], "native spy definition missing");
    local queue = assert(capital:GetBuildQueue(), "native production queue missing");
    assert(queue:CurrentlyBuilding() == unitType.Hash, "native spy build was not accepted");
    local cost, progress = queue:GetUnitCost(unitType.Index), queue:GetUnitProgress(unitType.Index);
    assert(type(cost) == "number" and cost > 0 and type(progress) == "number" and progress < cost,
        "native spy production cost/progress invalid");
    local before = {};
    for _,unit in player:GetUnits():Members() do before[unit:GetID()] = true; end
    -- Debug/City.ltp:103-107 advances the current native build queue.
    -- The queued item came from ProductionPanel.lua:293-301, not InitUnit.
    local added = math.ceil(cost - progress) + 1;
    print("JAC_BOUNDARY SPY_PRODUCTION_PROGRESS " .. R.Serialize({owner=owner,
        city=capital:GetID(), building=queue:CurrentlyBuilding(), cost=cost, progress=progress, added=added}));
    queue:AddProgress(added);
    local spy;
    for _,unit in player:GetUnits():Members() do
        if not before[unit:GetID()] and unit:GetType() == unitType.Index then
            assert(not spy, "multiple newly produced native spies"); spy = unit;
        end
    end
    assert(spy, "native production did not add exactly one spy to the owner roster");
    print("JAC_BOUNDARY SPY_PRODUCED " .. R.Serialize({owner=spy:GetOwner(),id=spy:GetID(),
        unitType=spy:GetType(),sourceCity=capital:GetID(),sourceX=capital:GetX(),sourceY=capital:GetY(),
        x=spy:GetX(),y=spy:GetY()}));
    Game:SetProperty("JAC_BOUNDARY_SPY", {owner=owner, id=spy:GetID()});
end);

ExposedMembers.JAC_PersonaBoundaryGameplayRead = ReadGameplay;
print("JAC_PersonaBoundary_Fixture: ready (test only; no automatic mutations)");
