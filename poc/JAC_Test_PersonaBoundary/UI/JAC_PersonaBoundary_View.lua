-- Test-only UI observer and real-frame requests. No Gameplay property writes.
include("JAC_PersonaBoundaryRules");
local R = JAC_PersonaBoundaryRules;
local B = {};
ExposedMembers.JAC_PersonaBoundary = B;
local function Config() return assert(Game:GetProperty("JAC_BOUNDARY_CONFIG"), "fixture not prepared"); end
local function CityKey(city) return city:GetX() .. "/" .. city:GetY(); end
local function Log(label, value)
    local text = "JAC_BOUNDARY " .. label .. " " .. R.Serialize(value);
    print(text); return text;
end

local function Missions(c)
    local result, success = {}, false;
    local targetPlot = Map.GetPlot(c.x, c.y):GetIndex();
    local operation = GameInfo.UnitOperations["UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR"];
    local ids = PlayerManager.GetAliveMajorIDs(); table.sort(ids);
    for _,attacker in ipairs(ids) do
        local diplomacy = Players[attacker]:GetDiplomacy();
        local ended = false;
        for id=0,255 do
            -- Enemy history availability is unproven. An unavailable read is
            -- recorded, never replaced with an inferred successful mission.
            local ok, mission = pcall(diplomacy.GetMission, diplomacy, attacker, id);
            if not ok then result[attacker .. "/unavailable"] = tostring(mission); ended=true; break; end
            if mission == 0 then ended=true; break; end
            if type(mission) ~= "table" then
                result[attacker .. "/unavailable"]="unknown native mission history value: " .. type(mission);
                ended=true; break;
            end
            local def = GameInfo.UnitOperations[mission.Operation];
            if def and operation and def.Hash == operation.Hash and mission.PlotIndex == targetPlot and
                mission.CompletionTurn >= c.armedTurn then
                result[attacker .. "/" .. id] = {attacker=attacker, id=id, operation=mission.Operation,
                    initial=mission.InitialResult, escape=mission.EscapeResult,
                    turn=mission.CompletionTurn, plot=mission.PlotIndex};
                -- A success that kills/captures the escaping spy can still neutralize
                -- the governor. Do not use GetMissionOutcomeDetails().Success.
                if mission.InitialResult == EspionageResultTypes.SUCCESS_UNDETECTED or
                    mission.InitialResult == EspionageResultTypes.SUCCESS_MUST_ESCAPE then success=true; end
            end
        end
        assert(ended, "mission history exceeds the short fixture limit");
    end
    return result, success;
end

function B.Read()
    local c = Config();
    local governor, def, row = R.ResolveGovernor(c.owner, c.leader);
    local assigned = governor:GetAssignedCity();
    local s = {turn=Game.GetCurrentGameTurn(), owner=c.owner, leader=c.leader,
        personOwner=R.Property(c.leader,"OWNER"), state=R.Property(c.leader,"STATE"),
        merit=R.Property(c.leader,"MERIT") or 0, skillOwned=R.Property(c.leader,"NODE_"..c.skill) or 0,
        greatPeople=0, cities={}, unmatchedEffects={}, targetKey=c.x .. "/" .. c.y,
        fixtureCityID=c.cityID, transfer=c.transfer,
        native={type=governor:GetType(), hash=def.Hash, established=governor:IsEstablished(),
            neutralized=governor:GetNeutralizedTurns(),
            base=governor:HasPromotion(GameInfo.GovernorPromotions[row.BasePromotionType].Hash),
            cityOwner=assigned and assigned:GetOwner() or -1, cityID=assigned and assigned:GetID() or -1}};
    local player = Players[c.owner];
    local gs = player:GetGovernors();
    s.titles = {total=gs:GetGovernorPoints(), spent=gs:GetGovernorPointsSpent()};
    local class = GameInfo.GreatPersonClasses["GREAT_PERSON_CLASS_JAC_PERSONA"];
    local unitType = GameInfo.Units[class.UnitType];
    for _,unit in player:GetUnits():Members() do if unit:GetType() == unitType.Index then
        local gp = unit:GetGreatPerson(); local individualID = gp and gp:GetIndividual();
        local individual = individualID ~= nil and individualID ~= -1 and GameInfo.GreatPersonIndividuals[individualID];
        if individual and individual.GreatPersonIndividualType == row.PersonType then s.greatPeople=s.greatPeople+1; end
    end end
    local modifiers = { ["JAC_PERSONA_SETTLED_" .. c.leader]=true };
    local nodes = {};
    for base in GameInfo.JAC_PersonaSkills() do
        if base.LeaderType == c.leader then modifiers[base.ModifierId]=true; end
    end
    for node in GameInfo.JAC_PersonaTreeNodes() do
        if node.LeaderType == c.leader and R.Property(c.leader,"NODE_"..node.NodeType) == 1 then
            modifiers[node.ModifierId]=true; nodes[#nodes+1]=node;
        end
    end
    local observed = {};
    for _,city in player:GetCities():Members() do observed[CityKey(city)]=city; end
    local oldCity = assert(Cities.GetPlotPurchaseCity(Map.GetPlot(c.x,c.y)), "fixture city at original plot missing");
    assert(oldCity:GetX()==c.x and oldCity:GetY()==c.y, "fixture city center identity changed");
    local expectedID=c.transfer and c.transfer.toCityID or c.cityID;
    assert(oldCity:GetID()==expectedID, "fixture city identity changed");
    observed[CityKey(oldCity)] = oldCity;
    s.target = {id=oldCity:GetID(), owner=oldCity:GetOwner(), x=oldCity:GetX(), y=oldCity:GetY(),
        plot=Map.GetPlot(c.x,c.y):GetIndex()};
    local names = {};
    for key,city in pairs(observed) do
        local nameKey = city:GetOwner() .. "/" .. Locale.Lookup(city:GetName());
        assert(not names[nameKey], "duplicate city names make native effect subjects ambiguous");
        names[nameKey] = key;
        local settled = city:GetProperty("JAC_PERSONA_SETTLED_"..c.leader);
        local data = {id=city:GetID(), owner=city:GetOwner(), settled=settled or 0, effects={}, selectors={}, yields={}};
        for modifier in pairs(modifiers) do data.effects[modifier]=0; end
        for yield in GameInfo.Yields() do data.yields[yield.YieldType]=city:GetYield(yield.Index); end
        local plot = Map.GetPlot(city:GetX(),city:GetY());
        for _,node in ipairs(nodes) do
            local value = plot:GetProperty(node.ModifierId); data.selectors[node.ModifierId]=value or 0;
        end
        s.cities[key] = data;
    end
    local inventory=GameEffects.GetModifiers();
    assert(type(inventory)=="table", "native modifier inventory unavailable");
    for _,id in ipairs(inventory) do
        local definition = GameEffects.GetModifierDefinition(id);
        assert(type(definition)=="table" and type(definition.Id)=="string" and #definition.Id>0,
            "native modifier definition unavailable");
        if modifiers[definition.Id] then
            -- Official Debug/Diplomacy.ltp:397-398 reads the modifier OWNER's
            -- player independently. Other players' same-type governors are not ours.
            local modifierOwner=GameEffects.GetModifierOwner(id);
            assert(modifierOwner~=nil, "native modifier owner unavailable");
            local sourceOwner=GameEffects.GetObjectsPlayerId(modifierOwner);
            assert(type(sourceOwner)=="number" and sourceOwner>=0, "native modifier player unavailable");
            if sourceOwner==c.owner then
            local active=GameEffects.GetModifierActive(id);
            assert(type(active)=="boolean", "native modifier activity unavailable");
            if active then
            local subjects=GameEffects.GetModifierSubjects(id);
            assert(type(subjects)=="table", "native modifier subjects unavailable");
            for _,object in ipairs(subjects) do
                local subjectOwner=GameEffects.GetObjectsPlayerId(object);
                local subjectName=GameEffects.GetObjectName(object);
                assert(type(subjectOwner)=="number" and type(subjectName)=="string", "native effect subject identity unavailable");
                local nameKey = subjectOwner .. "/" .. Locale.Lookup(subjectName);
                local key = names[nameKey];
                if key then
                    s.cities[key].effects[definition.Id] = s.cities[key].effects[definition.Id]+1;
                else
                    s.unmatchedEffects[#s.unmatchedEffects+1]={modifier=definition.Id,sourceOwner=sourceOwner,
                        subjectOwner=subjectOwner,subjectName=subjectName};
                end
            end
            end
            end
        end
    end
    table.sort(s.unmatchedEffects,function(a,b) return R.Serialize(a)<R.Serialize(b); end);
    s.gameplay = assert(ExposedMembers.JAC_PersonaBoundaryGameplayRead, "Gameplay observer unavailable")();
    s.freeCitiesOwner = s.gameplay.freeCitiesOwner;
    assert(type(s.freeCitiesOwner) == "number" and s.freeCitiesOwner >= 0,
        "Gameplay free cities owner unavailable");
    s.missions, s.neutralizationMission = Missions(c);
    local spyInfo = Game:GetProperty("JAC_BOUNDARY_SPY");
    if spyInfo then
        local spy = Players[spyInfo.owner]:GetUnits():FindID(spyInfo.id);
        s.spy = {owner=spyInfo.owner, id=spyInfo.id, present=spy~=nil};
        if spy then
            s.spy.operation=spy:GetSpyOperation(); s.spy.endTurn=spy:GetSpyOperationEndTurn();
            s.spy.remaining=s.spy.endTurn-s.turn; s.spy.x=spy:GetX(); s.spy.y=spy:GetY();
        end
    end
    return s;
end
function B.Record(label) return Log(label, B.Read()); end
function B.Check(phase, merit)
    local s = B.Read(); Log("CHECK_"..phase, s);
    R.Check(s, phase, merit); return R.Serialize(s);
end

local function LocalSpy()
    local info = assert(Game:GetProperty("JAC_BOUNDARY_SPY"), "native fixture spy not created");
    assert(info.owner == Game.GetLocalPlayer(), "enemy-spy requests have no proven control path");
    local spy = assert(Players[info.owner]:GetUnits():FindID(info.id), "native spy no longer present");
    assert(spy:GetType() == GameInfo.Units["UNIT_SPY"].Index and spy:GetOwner() == info.owner, "native spy identity mismatch");
    return spy;
end
function B.FindSpyDestination()
    local spy=LocalSpy();
    local ids=PlayerManager.GetAliveMajorIDs(); table.sort(ids);
    for _,owner in ipairs(ids) do if owner ~= spy:GetOwner() then
        local cities={};
        for _,city in Players[owner]:GetCities():Members() do cities[#cities+1]=city; end
        table.sort(cities,function(a,b) return a:GetID() < b:GetID(); end);
        for _,city in ipairs(cities) do
            local params={[UnitOperationTypes.PARAM_X]=city:GetX(),[UnitOperationTypes.PARAM_Y]=city:GetY()};
            local ok, result=UnitManager.CanStartOperation(spy,UnitOperationTypes.SPY_TRAVEL_NEW_CITY,
                Map.GetPlot(city:GetX(),city:GetY()),params);
            Log("SPY_DESTINATION_QUERY",{owner=owner,city=city:GetID(),ok=ok,result=result or {}});
            if ok then return {owner=owner,city=city:GetID()}; end
        end
    end end
    error("no native-eligible foreign spy destination; inspect query reasons");
end
function B.Probe(cityOwner, cityID)
    local spy = LocalSpy();
    local city = assert(CityManager.GetCity(cityOwner, cityID), "foreign spy target missing");
    assert(city:GetOwner() ~= spy:GetOwner(), "spy target must be foreign");
    local operation = assert(GameInfo.UnitOperations["UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR"], "native neutralize operation missing");
    local plot = Map.GetPlot(city:GetX(), city:GetY());
    local canStart, results = UnitManager.CanStartOperation(spy, operation.Hash, plot, false, true);
    local targetGovernors = {};
    local gs = Players[cityOwner]:GetGovernors();
    for def in GameInfo.Governors() do if gs:HasGovernor(def.Hash) then
        local governor = gs:GetGovernor(def.Hash); local assigned=governor and governor:GetAssignedCity();
        if assigned and assigned:GetOwner() == cityOwner and assigned:GetID() == cityID then
            targetGovernors[#targetGovernors+1] = {type=governor:GetType(), neutralized=governor:GetNeutralizedTurns(), established=governor:IsEstablished()};
        end
    end end
    table.sort(targetGovernors, function(a,b) return a.type < b.type; end);
    local probe = {spyOwner=spy:GetOwner(), spyID=spy:GetID(), targetOwner=cityOwner, targetID=cityID,
        nativeTarget=targetGovernors, canStart=canStart, results=results or {},
        turns=UnitManager.GetTimeToComplete(operation.Index, spy),
        probability=UnitManager.GetResultProbability(operation.Index, spy, plot),
        travel=UnitManager.GetTravelTime(spy, city), establish=UnitManager.GetEstablishInCityTime(spy, city),
        currentOperation=spy:GetSpyOperation(), endTurn=spy:GetSpyOperationEndTurn()};
    Log("SPY_PROBE", probe); return probe;
end

local pending;
B.lastAction = "IDLE";
function B.Queue(action, params)
    assert(not pending, "a boundary frame request is already pending");
    assert(Players[Game.GetLocalPlayer()]:IsTurnActive(), "boundary request needs the local active turn");
    assert(action == "PREPARE" or action == "EVOLVE" or action == "ASSIGN" or action == "BUY" or
        action == "LOSE_CITY" or action == "SPAWN_SPY" or action == "SPY_TRAVEL" or action == "SPY_MISSION", "unknown boundary action");
    pending = {action=action, params=params or {}}; B.lastAction="QUEUED_"..action;
end
local function Submit(command)
    local owner = Game.GetLocalPlayer();
    assert(Players[owner]:IsTurnActive(), "turn ended before boundary frame request");
    local action, params = command.action, command.params;
    if action == "PREPARE" then
        UI.RequestPlayerOperation(owner, PlayerOperations.EXECUTE_SCRIPT, {OnStart="JAC_BoundaryPrepare"}); return;
    end
    local c=Config();
    assert(c.owner == owner, "fixture is owned by another player");
    if action == "EVOLVE" then
        UI.RequestPlayerOperation(owner, PlayerOperations.EXECUTE_SCRIPT, {OnStart="JAC_LeaderPersonaEvolve", Leader=c.leader});
    elseif action == "BUY" then
        local node=GameInfo.JAC_PersonaTreeNodes[c.skill];
        UI.RequestPlayerOperation(owner, PlayerOperations.EXECUTE_SCRIPT,
            {OnStart="JAC_LeaderPersonaPurchase", Leader=c.leader, Node=c.skill, ConfirmCategory=node.CategoryType, ConfirmRoute=node.RouteType});
    elseif action == "ASSIGN" then
        local governor, def = R.ResolveGovernor(owner,c.leader);
        local city=assert(CityManager.GetCity(owner,c.cityID), "assignable fixture city missing");
        assert(governor:GetNeutralizedTurns() == 0 and Players[owner]:GetGovernors():CanAssignGovernor(def.Hash,city), "native assignment unavailable");
        UI.RequestPlayerOperation(owner,PlayerOperations.ASSIGN_GOVERNOR,
            {[PlayerOperations.PARAM_GOVERNOR_TYPE]=def.Index,[PlayerOperations.PARAM_PLAYER_ONE]=owner,[PlayerOperations.PARAM_CITY_DEST]=c.cityID});
    elseif action == "LOSE_CITY" or action == "SPAWN_SPY" then
        if action == "LOSE_CITY" then B.Check("SETTLED"); end
        UI.RequestPlayerOperation(owner,PlayerOperations.EXECUTE_SCRIPT,
            {OnStart=action == "LOSE_CITY" and "JAC_BoundaryLoseCity" or "JAC_BoundarySpawnSpy"});
    else
        local spy=LocalSpy(); local city=assert(CityManager.GetCity(params.owner,params.city), "spy target missing");
        assert(city:GetOwner() ~= owner, "spy target must be foreign");
        local plot=Map.GetPlot(city:GetX(),city:GetY());
        if action == "SPY_TRAVEL" then
            local arguments={[UnitOperationTypes.PARAM_X]=city:GetX(),[UnitOperationTypes.PARAM_Y]=city:GetY()};
            local ok, result=UnitManager.CanStartOperation(spy,UnitOperationTypes.SPY_TRAVEL_NEW_CITY,plot,arguments);
            Log("SPY_TRAVEL_QUERY", {ok=ok, result=result or {}});
            assert(ok, "native spy travel refused");
            UnitManager.RequestOperation(spy,UnitOperationTypes.SPY_TRAVEL_NEW_CITY,arguments);
        else
            local probe=B.Probe(params.owner,params.city);
            assert(probe.canStart and #probe.nativeTarget == 1, "native neutralize operation refused or target governor absent");
            local actual=Cities.GetPlotPurchaseCity(Map.GetPlot(spy:GetX(),spy:GetY()));
            assert(actual and actual:GetOwner()==params.owner and actual:GetID()==params.city, "native spy is not in the target city");
            UnitManager.RequestOperation(spy,GameInfo.UnitOperations["UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR"].Hash);
        end
    end
end

ContextPtr:SetHide(false);
ContextPtr:SetUpdate(function()
    if not pending or UI.IsGameCoreBusy() then return; end
    local command=pending; pending=nil;
    local ok, result=pcall(Submit, command);
    B.lastAction=(ok and "SUBMITTED_" or "ERROR_")..command.action;
    Log("FRAME_REQUEST", {action=command.action, ok=ok, error=not ok and tostring(result) or ""});
    if not ok then error(result); end -- no replay of an operation after an unknown result
end);

local last;
local function Observe(label)
    local c=Game:GetProperty("JAC_BOUNDARY_CONFIG");
    if not c or R.Property(c.leader,"STATE") ~= "GOVERNOR" then return; end
    local ok, value=pcall(B.Read);
    if not ok then Log("OBSERVATION_UNAVAILABLE", tostring(value)); return; end
    local serialized=R.Serialize(value);
    if serialized ~= last then last=serialized; Log(label,value); end
end
Events.GameCoreEventPublishComplete.Add(function() Observe("PUBLISH"); end);
Events.LocalPlayerTurnBegin.Add(function() Observe("TURN"); end);
Events.LoadScreenClose.Add(function() last=nil; Observe("LOAD"); end);
Events.SpyMissionCompleted.Add(function(attacker, missionID)
    -- The event's player is the attacker, not the governor owner.
    Log("MISSION_EVENT", {attacker=attacker, mission=missionID, turn=Game.GetCurrentGameTurn()});
    Observe("MISSION_SNAPSHOT");
end);
