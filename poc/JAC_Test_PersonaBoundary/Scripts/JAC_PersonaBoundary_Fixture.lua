-- Opt-in test MOD only. Mutations run through synchronized EXECUTE_SCRIPT.
-- Primary API evidence and unresolved spy control: documents/poc/pr86-boundary-diagnostic.md.
include("JAC_PersonaBoundaryRules");
local R = JAC_PersonaBoundaryRules;
local LEADER = "LEADER_JAC_ODA_NOBUHIDE";
local CONFIG = "JAC_BOUNDARY_CONFIG";
local ENEMY_CONFIG = "JAC_BOUNDARY_ENEMY_CONFIG";
local ENEMY_LEADER = "LEADER_JAC_ASHINA_MORIKIYO";
local SPY_CIVIC = "CIVIC_JAC_SENGOKU_LOCAL_LEAGUES_AND_INTELLIGENCE";
local spyProduction;
local function Config(owner)
    local c = assert(Game:GetProperty(CONFIG) or Game:GetProperty(ENEMY_CONFIG), "boundary fixture not prepared");
    assert((c.attacker or c.owner) == owner and Players[owner]:IsHuman(),
        "fixture operation requires its human controller");
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
local function ReadGameplay(c)
    c = c or assert(Game:GetProperty(CONFIG), "boundary fixture not prepared");
    local result = {merit=R.Property(c.leader, "MERIT") or 0,
        freeCitiesOwner=PlayerManager.GetFreeCitiesPlayerID(), cities={},nodesOwned={}};
    for node in GameInfo.JAC_PersonaTreeNodes() do
        if node.LeaderType==c.leader and R.Property(c.leader,"NODE_"..node.NodeType)==1 then
            result.nodesOwned[node.NodeType]=1;
        end
    end
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

GameEvents.JAC_BoundarySpawnSpy.Add(function(owner)
    local c = Config(owner);
    assert(not spyProduction and not Game:GetProperty("JAC_BOUNDARY_SPY"), "fixture spy production already started");
    local player = Players[owner];
    local unit = assert(GameInfo.Units["UNIT_SPY"], "native spy definition missing");
    assert(unit.PrereqCivic == SPY_CIVIC, "native spy civic differs from the Sengoku gate");
    local civic = assert(GameInfo.Civics[SPY_CIVIC], "native spy civic missing");
    local culture = player:GetCulture();
    print("JAC_BOUNDARY SPY_CIVIC_BEFORE " .. R.Serialize({index=civic.Index,hash=civic.Hash,
        value=culture:HasCivic(civic.Index),valueType=type(culture:HasCivic(civic.Index))}));
    if not culture:HasCivic(civic.Index) then
        -- Official Debug/Player.ltp:190-201 grants civics with SetCivic(..., true).
        -- Completing progress alone did not grant the native spy slot in live 48.
        assert(type(culture.SetCivic) == "function", "native civic grant API unavailable");
        culture:SetCivic(civic.Index, true);
    end
    assert(culture:HasCivic(civic.Index), "native spy civic was not completed");
    print("JAC_BOUNDARY SPY_CIVIC_AFTER " .. R.Serialize({index=civic.Index,hash=civic.Hash,
        value=culture:HasCivic(civic.Index),valueType=type(culture:HasCivic(civic.Index))}));
    local capital = assert(player:GetCities():GetCapitalCity(), "spy production capital missing");
    spyProduction = {owner=owner, cityID=capital:GetID(), progressed=false};
    Game:SetProperty("JAC_BOUNDARY_SPY_PREPARED", {owner=owner, cityID=capital:GetID()});
    print("JAC_BOUNDARY SPY_CIVIC_READY " .. R.Serialize(Game:GetProperty("JAC_BOUNDARY_SPY_PREPARED")));
end);

GameEvents.JAC_BoundaryProgressSpy.Add(function(owner)
    Config(owner);
    assert(spyProduction and spyProduction.owner == owner and not spyProduction.progressed,
        "spy production progress was not prepared or was already added");
    local city = assert(CityManager.GetCity(owner, spyProduction.cityID), "spy production city missing");
    local queue = city:GetBuildQueue();
    -- The visible UI already independently observed the accepted native queue.
    -- GetAt is UI-only in live 50; Gameplay performs the mutation and binds the
    -- actual produced unit afterwards, never treating its request as completion.
    local unit = assert(GameInfo.Units["UNIT_SPY"], "native spy definition missing");
    assert(type(queue.AddProgress) == "function", "native Gameplay production API unavailable");
    local before = {};
    local count = 0;
    for _,candidate in Players[owner]:GetUnits():Members() do
        if candidate:GetType() == unit.Index then before[candidate:GetID()] = true; count = count + 1; end
    end
    spyProduction.before = before;
    spyProduction.beforeCount = count;
    spyProduction.progressed = true; -- one AddProgress even if its result is unknown
    -- Official Debug/City.ltp:200-204. The native queue, not a spy Property, creates the unit.
    queue:AddProgress(500);
    Game:SetProperty("JAC_BOUNDARY_SPY_PROGRESS", {owner=owner, cityID=city:GetID(), beforeCount=count});
    print("JAC_BOUNDARY SPY_PRODUCTION_PROGRESS " .. R.Serialize(Game:GetProperty("JAC_BOUNDARY_SPY_PROGRESS")));
end);

GameEvents.JAC_BoundaryBindSpy.Add(function(owner)
    Config(owner);
    assert(spyProduction and spyProduction.owner == owner and spyProduction.progressed and
        not Game:GetProperty("JAC_BOUNDARY_SPY"), "spy production was not completed once");
    local spyType = GameInfo.Units["UNIT_SPY"].Index;
    local spy, count = nil, 0;
    for _,unit in Players[owner]:GetUnits():Members() do
        if unit:GetType() == spyType then
            count = count + 1;
            if not spyProduction.before[unit:GetID()] then
                assert(not spy, "native production added multiple spies"); spy = unit;
            end
        end
    end
    assert(spy and count == spyProduction.beforeCount + 1, "native spy production did not add exactly one unit");
    assert(spy:GetOwner() == owner, "produced native spy owner mismatch");
    Game:SetProperty("JAC_BOUNDARY_SPY", {owner=owner, id=spy:GetID()});
    print("JAC_BOUNDARY SPY_NATIVE_STATE " .. R.Serialize({owner=spy:GetOwner(),id=spy:GetID(),
        unitType=spy:GetType(),x=spy:GetX(),y=spy:GetY()}));
    print("JAC_BOUNDARY SPY_CREATED " .. R.Serialize(Game:GetProperty("JAC_BOUNDARY_SPY")));
end);

GameEvents.JAC_BoundarySpyBarrier.Add(function(owner, params)
    Config(owner);
    assert(type(params) == "table" and type(params.Token) == "number" and
        (params.Action == "SPY_TRAVEL" or params.Action == "SPY_MISSION"), "invalid spy barrier");
    local spy = assert(Game:GetProperty("JAC_BOUNDARY_SPY"), "spy barrier before native production");
    assert(spy.owner == owner and spy.id == params.SpyID, "spy barrier identity mismatch");
    Game:SetProperty("JAC_BOUNDARY_SPY_BARRIER",
        {owner=owner, spyID=spy.id, token=params.Token, action=params.Action});
end);

GameEvents.JAC_BoundaryAdoptSpyIdentity.Add(function(owner, params)
    Config(owner);
    assert(type(params)=="table" and type(params.OldID)=="number" and
        type(params.NewID)=="number" and type(params.TargetOwner)=="number" and
        type(params.TargetID)=="number", "invalid native spy identity update");
    local old=assert(Game:GetProperty("JAC_BOUNDARY_SPY"), "diagnostic spy reference missing");
    assert(old.owner==owner and old.id==params.OldID, "diagnostic spy reference changed");
    local spy=assert(Players[owner]:GetUnits():FindID(params.NewID), "arrived native spy missing");
    assert(spy:GetOwner()==owner and spy:GetID()==params.NewID and
        spy:GetType()==GameInfo.Units["UNIT_SPY"].Index, "arrived native spy identity mismatch");
    local city=Cities.GetPlotPurchaseCity(Map.GetPlot(spy:GetX(),spy:GetY()));
    assert(city and city:GetOwner()==params.TargetOwner and city:GetID()==params.TargetID,
        "arrived native spy destination mismatch");
    Game:SetProperty("JAC_BOUNDARY_SPY",{owner=owner,id=spy:GetID()});
    print("JAC_BOUNDARY SPY_REFERENCE_UPDATED " .. R.Serialize(Game:GetProperty("JAC_BOUNDARY_SPY")));
end);

GameEvents.JAC_BoundarySaveSpyTravel.Add(function(owner,params)
    Config(owner);
    local spy=assert(Game:GetProperty("JAC_BOUNDARY_SPY"),"native spy reference missing");
    assert(spy.owner==owner and type(params)=="table" and params.OldID==spy.id and
        type(params.Name)=="string" and type(params.Level)=="number" and
        type(params.TargetOwner)=="number" and type(params.TargetID)=="number" and
        not Game:GetProperty("JAC_BOUNDARY_SPY_TRAVEL"),"invalid or repeated travel receipt");
    local city=assert(CityManager.GetCity(params.TargetOwner,params.TargetID),"travel target missing");
    assert(city:GetOwner()~=owner,"travel target is not foreign");
    Game:SetProperty("JAC_BOUNDARY_SPY_TRAVEL",{oldID=spy.id,name=params.Name,level=params.Level,
        targetOwner=params.TargetOwner,targetID=params.TargetID,x=city:GetX(),y=city:GetY()});
end);

local function EnemyController(owner)
    assert(owner == 0 and Players[owner] and Players[owner]:IsHuman() and Players[owner]:IsAlive(),
        "enemy fixture requires human controller 0");
end
local function EnemyConfig(owner)
    EnemyController(owner);
    local c=assert(Game:GetProperty(ENEMY_CONFIG), "enemy fixture not prepared");
    assert(c.attacker==owner and c.owner==1 and c.source==2 and c.leader==ENEMY_LEADER,
        "enemy fixture actor identity changed");
    return c;
end

-- A separate fixture keeps the earlier human Oda/city-loss experiment intact.
-- No AI title exists between Join and PrepareEvolution: both run in one
-- synchronized callback. The static SQL gate is diagnostic, not admission logic.
GameEvents.JAC_BoundaryPrepareEnemy.Add(function(controller)
    EnemyController(controller);
    assert(not Game:GetProperty(CONFIG) and not Game:GetProperty(ENEMY_CONFIG),
        "enemy experiment requires a fresh game");
    assert(PlayerConfigurations[1]:GetLeaderTypeName()=="LEADER_JAC_DATE_TANEMUNE" and
        PlayerConfigurations[2]:GetLeaderTypeName()==ENEMY_LEADER,
        "expected AI defender 1 and Ashina source 2");
    local defender=assert(Players[1], "AI defender missing");
    assert(defender:IsAlive() and not defender:IsHuman() and Players[2]:IsAlive(),
        "defender/source actor roles differ");
    local row=assert(GameInfo.JAC_LeaderPersonas[ENEMY_LEADER], "enemy persona missing");
    assert(row.EvolutionCost==1, "enemy fixture requires cost one");
    local def=assert(GameInfo.Governors[row.GovernorType], "enemy Governor definition missing");
    local governors=defender:GetGovernors();
    assert(not governors:HasGovernor(def.Hash), "unjoined enemy Governor already appointed");
    assert(governors:CanEverAppointGovernor(def.Hash), "diagnostic Date/Ashina trait gate unavailable");
    for candidate in GameInfo.JAC_LeaderPersonas() do
        if candidate.LeaderType~=ENEMY_LEADER then
            local other=GameInfo.Governors[candidate.GovernorType];
            assert(other and not governors:CanEverAppointGovernor(other.Hash),
                "AI defender can appoint another unjoined persona");
        end
    end
    local seam=assert(ExposedMembers.JAC_LeaderPersona, "regular persona seam missing");
    assert(seam.Join(1,2)=="JOINED", "regular enemy Join failed");
    assert(R.Property(ENEMY_LEADER,"OWNER")==1 and R.Property(ENEMY_LEADER,"SOURCE")==2 and
        R.Property(ENEMY_LEADER,"STATE")=="GREAT_PERSON", "joined enemy identity mismatch");
    local total,spent=governors:GetGovernorPoints(),governors:GetGovernorPointsSpent();
    if total-spent<1 then
        local civics={};
        for title in GameInfo.JAC_PersonaTitleCivics() do civics[#civics+1]=title.CivicType; end
        table.sort(civics);
        local granted=false;
        for _,kind in ipairs(civics) do
            local civic=GameInfo.Civics[kind];
            if not defender:GetCulture():HasCivic(civic.Index) then
                defender:GetCulture():SetCulturalProgress(civic.Index,
                    defender:GetCulture():GetCultureCost(civic.Index));
                assert(defender:GetCulture():HasCivic(civic.Index) and
                    governors:GetGovernorPoints()==total+1 and
                    governors:GetGovernorPointsSpent()==spent,
                    "official AI civic failed to grant exactly one title");
                granted=true; break;
            end
        end
        assert(granted,"AI has no unresearched official title civic");
    end
    assert(seam.PrepareEvolution(1,ENEMY_LEADER)=="EVOLVING" and
        R.Property(ENEMY_LEADER,"STATE")=="EVOLVING", "regular enemy evolution preparation failed");
    Game:SetProperty(ENEMY_CONFIG,{attacker=controller,owner=1,source=2,leader=ENEMY_LEADER,
        skill=ENEMY_LEADER.."_SIGNATURE_1",total=governors:GetGovernorPoints(),
        spent=governors:GetGovernorPointsSpent(),armedTurn=Game.GetCurrentGameTurn(),
        controlRequired=false,requireSkill=false});
    print("JAC_BOUNDARY ENEMY_PREPARED "..R.Serialize(Game:GetProperty(ENEMY_CONFIG)));
end);

GameEvents.JAC_BoundaryEnemyBarrier.Add(function(controller,params)
    local c=EnemyConfig(controller);
    assert(type(params)=="table" and type(params.Token)=="number" and
        (params.Action=="APPOINT" or params.Action=="ASSIGN" or params.Action=="REASSIGN"),
        "invalid enemy operation barrier");
    local attempts=Game:GetProperty("JAC_BOUNDARY_ENEMY_ATTEMPTS") or {};
    assert(not attempts[params.Action],"enemy native request already attempted");
    Game:SetProperty("JAC_BOUNDARY_ENEMY_BARRIER",{attacker=c.attacker,defender=c.owner,
        action=params.Action,token=params.Token});
    attempts[params.Action]={turn=Game.GetCurrentGameTurn(),token=params.Token};
    Game:SetProperty("JAC_BOUNDARY_ENEMY_ATTEMPTS",attempts);
end);

GameEvents.JAC_BoundaryConfirmEnemy.Add(function(controller)
    local c=EnemyConfig(controller);
    local governor,def=R.ResolveGovernor(c.owner,c.leader);
    local gs=Players[c.owner]:GetGovernors();
    assert(governor:GetType()==def.Index and gs:GetGovernorPoints()>=c.total and
        gs:GetGovernorPointsSpent()==c.spent+1,
        "official cost-one Governor appointment or native accounting missing");
    local seam=assert(ExposedMembers.JAC_LeaderPersona,"regular persona seam missing");
    local result=seam.CompleteEvolution(c.owner,c.leader);
    assert((result=="EVOLVED" or result=="ALREADY_EVOLVED") and
        R.Property(c.leader,"STATE")=="GOVERNOR", "regular evolution did not remove the GP");
    local assigned=assert(governor:GetAssignedCity(),"enemy Governor not natively assigned");
    assert(assigned:GetOwner()==c.owner,"enemy Governor assigned to another player");
    c.cityID=assigned:GetID(); c.x=assigned:GetX(); c.y=assigned:GetY();
    c.armedTurn=Game.GetCurrentGameTurn();
    Game:SetProperty(ENEMY_CONFIG,c);
    print("JAC_BOUNDARY ENEMY_NATIVE_BOUND "..R.Serialize(c));
end);

GameEvents.JAC_BoundaryRecordEnemyMission.Add(function(controller,params)
    local c=EnemyConfig(controller);
    assert(c.cityID and type(params)=="table" and params.Attacker==c.attacker and
        type(params.MissionID)=="number" and params.MissionID>=0 and
        not Game:GetProperty("JAC_BOUNDARY_ENEMY_MISSION"), "invalid or repeated enemy mission event");
    Game:SetProperty("JAC_BOUNDARY_ENEMY_MISSION",{attacker=params.Attacker,
        id=params.MissionID,turn=Game.GetCurrentGameTurn()});
end);

GameEvents.JAC_BoundaryCaptureEnemyRecovery.Add(function(controller)
    local c=EnemyConfig(controller);
    assert(c.cityID and not Game:GetProperty("JAC_BOUNDARY_ENEMY_RECOVERY"),
        "recovery baseline missing target or already recorded");
    local governor=R.ResolveGovernor(c.owner,c.leader);
    local remaining=governor:GetNeutralizedTurns();
    assert(type(remaining)=="number" and remaining>0,"native neutralization required for recovery");
    local nodes={};
    for node in GameInfo.JAC_PersonaTreeNodes() do
        if node.LeaderType==c.leader and R.Property(c.leader,"NODE_"..node.NodeType)==1 then
            nodes[node.NodeType]=1;
        end
    end
    Game:SetProperty("JAC_BOUNDARY_ENEMY_RECOVERY",{merit=R.Property(c.leader,"MERIT") or 0,
        nodesOwned=nodes,spent=Players[c.owner]:GetGovernors():GetGovernorPointsSpent(),
        remaining=remaining,turn=Game.GetCurrentGameTurn()});
end);

ExposedMembers.JAC_PersonaBoundaryGameplayRead = ReadGameplay;
print("JAC_PersonaBoundary_Fixture: ready (test only; no automatic mutations)");
