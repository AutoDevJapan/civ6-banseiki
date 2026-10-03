-- Test-only UI observer and real-frame requests. No Gameplay property writes.
include("JAC_PersonaBoundaryRules");
local R = JAC_PersonaBoundaryRules;
local B = {};
ExposedMembers.JAC_PersonaBoundary = B;
local function Config() return assert(Game:GetProperty("JAC_BOUNDARY_CONFIG"), "fixture not prepared"); end
local function EnemyConfig()
    local c=assert(Game:GetProperty("JAC_BOUNDARY_ENEMY_CONFIG"),"enemy fixture not prepared");
    assert(c.attacker==Game.GetLocalPlayer() and c.owner~=c.attacker and c.source~=c.owner,
        "enemy fixture actor roles differ");
    return c;
end
local function ProductNative(owner,leader)
    local read=assert(ExposedMembers.JAC_LeaderPersonaNativeRead,
        "product UI native Governor reader unavailable");
    local value=assert(read(owner,leader),"product UI native Governor observation unavailable");
    assert(type(value.appointed)=="boolean" and type(value.canAppoint)=="boolean",
        "product UI native Governor observation incomplete");
    return value;
end
-- UI-only native object resolver. Gameplay imports the shared rule file but
-- never receives this function or a Governor userdata from it.
local function ResolveGovernor(owner,leader)
    local row=assert(GameInfo.JAC_LeaderPersonas[leader],"persona definition missing");
    local def=assert(GameInfo.Governors[row.GovernorType],"native governor definition missing");
    local player=assert(Players[owner],"native governor owner missing");
    local governors=player:GetGovernors();
    assert(governors:HasGovernor(def.Hash),"native governor not appointed");
    local governor=assert(governors:GetGovernor(def.Hash),"native governor instance missing");
    assert(governor:GetType()==def.Index,"native governor type mismatch");
    return governor,def,row;
end
local function CityKey(city) return city:GetX() .. "/" .. city:GetY(); end
local function Log(label, value)
    local text = "JAC_BOUNDARY " .. label .. " " .. R.Serialize(value);
    print(text); return text;
end
local ReadSpyRoster, InTargetCity;

local function Missions(c)
    local result, success = {}, false;
    local targetPlot = Map.GetPlot(c.x, c.y):GetIndex();
    local operation = GameInfo.UnitOperations["UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR"];
    local function Record(attacker,id,mission)
        if type(mission)~="table" then
            result[attacker.."/unavailable"]="unknown native mission history value: "..type(mission);
            return;
        end
        local def=GameInfo.UnitOperations[mission.Operation];
        if def and operation and def.Hash==operation.Hash and mission.PlotIndex==targetPlot and
            type(mission.CompletionTurn)=="number" and mission.CompletionTurn>=c.armedTurn then
            result[attacker.."/"..id]={attacker=attacker,id=id,operation=mission.Operation,
                initial=mission.InitialResult,escape=mission.EscapeResult,
                turn=mission.CompletionTurn,plot=mission.PlotIndex};
            -- Initial success still counts if escape later ends in capture/death.
            if mission.InitialResult==EspionageResultTypes.SUCCESS_UNDETECTED or
                mission.InitialResult==EspionageResultTypes.SUCCESS_MUST_ESCAPE then success=true; end
        end
    end
    if c.attacker then
        local event=Game:GetProperty("JAC_BOUNDARY_ENEMY_MISSION");
        if not event then return result,false; end
        if event.attacker~=c.attacker or type(event.id)~="number" then
            result["event/unavailable"]="attacker or mission ID differs"; return result,false;
        end
        local diplomacy=Players[c.attacker]:GetDiplomacy();
        local ok,mission=pcall(diplomacy.GetMission,diplomacy,c.attacker,event.id);
        if not ok then result[c.attacker.."/unavailable"]=tostring(mission);
        elseif mission==0 then result[c.attacker.."/unavailable"]="event mission absent from native history";
        else Record(c.attacker,event.id,mission); end
        return result,success;
    end
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
            if type(mission)~="table" then Record(attacker,id,mission); ended=true; break; end
            Record(attacker,id,mission);
        end
        assert(ended, "mission history exceeds the short fixture limit");
    end
    return result, success;
end

local function ReadPersona(c)
    local governor, def, row = ResolveGovernor(c.owner, c.leader);
    local assigned = governor:GetAssignedCity();
    local s = {turn=Game.GetCurrentGameTurn(), owner=c.owner, leader=c.leader,
        attacker=c.attacker,requireSkill=c.requireSkill~=false,controlRequired=c.controlRequired~=false,
        personOwner=R.Property(c.leader,"OWNER"), state=R.Property(c.leader,"STATE"),
        merit=R.Property(c.leader,"MERIT") or 0, skillOwned=R.Property(c.leader,"NODE_"..c.skill) or 0,
        greatPeople=0, cities={}, nodesOwned={}, unmatchedEffects={}, targetKey=c.x .. "/" .. c.y,
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
            modifiers[node.ModifierId]=true; nodes[#nodes+1]=node; s.nodesOwned[node.NodeType]=1;
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
    s.gameplay = assert(ExposedMembers.JAC_PersonaBoundaryGameplayRead, "Gameplay observer unavailable")(c);
    s.freeCitiesOwner = s.gameplay.freeCitiesOwner;
    assert(type(s.freeCitiesOwner) == "number" and s.freeCitiesOwner >= 0,
        "Gameplay free cities owner unavailable");
    s.missions, s.neutralizationMission = Missions(c);
    local spyInfo = Game:GetProperty("JAC_BOUNDARY_SPY");
    if spyInfo then
        s.spyRoster=ReadSpyRoster(spyInfo.owner);
        local spy = Players[spyInfo.owner]:GetUnits():FindID(spyInfo.id);
        s.spy = {owner=spyInfo.owner, id=spyInfo.id, present=spy~=nil};
        if spy then
            s.spy.operation=spy:GetSpyOperation(); s.spy.endTurn=spy:GetSpyOperationEndTurn();
            s.spy.remaining=s.spy.endTurn-s.turn; s.spy.x=spy:GetX(); s.spy.y=spy:GetY();
        end
    end
    return s;
end
function B.Read() return ReadPersona(Config()); end
function B.ReadEnemy()
    local c=EnemyConfig();
    assert(c.cityID and c.x and c.y,"enemy native Governor target not bound");
    local s=ReadPersona(c);
    assert(type(s.gameplay.nodesOwned)=="table","Gameplay skill ownership inventory missing");
    for node in GameInfo.JAC_PersonaTreeNodes() do if node.LeaderType==c.leader then
        assert(s.nodesOwned[node.NodeType]==s.gameplay.nodesOwned[node.NodeType],
            "UI/Gameplay skill ownership differs");
    end end
    local governor=ResolveGovernor(c.owner,c.leader);
    s.native.turnsToEstablish=governor:GetTurnsToEstablish();
    s.native.turnsOnSite=governor:GetTurnsOnSite();
    assert(type(s.native.turnsToEstablish)=="number" and s.native.turnsToEstablish>0 and
        type(s.native.turnsOnSite)=="number" and s.native.turnsOnSite>=0,
        "native Governor establishment clock unavailable");
    return s;
end
function B.CheckEnemy(phase, merit)
    local s=B.ReadEnemy(); Log("ENEMY_CHECK_"..phase,s);
    assert(s.native.turnsToEstablish==5,"native five-turn establishment duration changed");
    R.Check(s,phase,merit); return R.Serialize(s);
end
function B.EnemyState()
    local c=EnemyConfig(); local row=GameInfo.JAC_LeaderPersonas[c.leader];
    local def=GameInfo.Governors[row.GovernorType];
    local gs=Players[c.owner]:GetGovernors();
    local product=ProductNative(c.owner,c.leader);
    local appointed=gs:HasGovernor(def.Hash);
    assert(appointed==product.appointed,"product and diagnostic UI native appointment disagree");
    local governor=appointed and gs:GetGovernor(def.Hash) or nil;
    local city=governor and governor:GetAssignedCity() or nil;
    local state={attacker=c.attacker,defender=c.owner,source=c.source,leader=c.leader,
        personOwner=R.Property(c.leader,"OWNER"),personaState=R.Property(c.leader,"STATE"),
        total=gs:GetGovernorPoints(),spent=gs:GetGovernorPointsSpent(),
        appointed=appointed,canAppoint=product.canAppoint,
        nativeType=governor and governor:GetType() or -1,
        base=governor and governor:HasPromotion(GameInfo.GovernorPromotions[row.BasePromotionType].Hash) or false,
        cityOwner=city and city:GetOwner() or -1,cityID=city and city:GetID() or -1,
        cityX=city and city:GetX() or -1,cityY=city and city:GetY() or -1,
        neutralized=governor and governor:GetNeutralizedTurns() or 0};
    if appointed then
        assert(state.base==product.base and state.neutralized==product.neutralized and
            state.cityOwner==(product.cityOwner or -1) and
            state.cityID==(product.cityID or -1),
            "product and diagnostic UI native Governor state disagree");
    end
    Log("ENEMY_STATE",state); return state;
end
function B.EnemyPreflight()
    assert(Game.GetLocalPlayer()==0 and Players[0]:IsHuman() and Players[0]:IsTurnActive(),
        "enemy preflight requires active human controller 0");
    assert(not Game:GetProperty("JAC_BOUNDARY_CONFIG") and
        not Game:GetProperty("JAC_BOUNDARY_ENEMY_CONFIG"),
        "enemy preflight requires a fresh independent game");
    assert(PlayerConfigurations[1]:GetLeaderTypeName()=="LEADER_JAC_DATE_TANEMUNE" and
        PlayerConfigurations[2]:GetLeaderTypeName()=="LEADER_JAC_ASHINA_MORIKIYO",
        "enemy preflight actor roles changed");
    local leader="LEADER_JAC_ASHINA_MORIKIYO";
    local row=assert(GameInfo.JAC_LeaderPersonas[leader],"enemy persona definition missing");
    assert(row.EvolutionCost==1,"enemy fixture is not cost one");
    local def=assert(GameInfo.Governors[row.GovernorType],"enemy native Governor definition missing");
    local gs=Players[1]:GetGovernors();
    assert(type(gs.CanEverAppointGovernor)=="function", "native UI candidate getter unavailable");
    local native=ProductNative(1,leader);
    assert(not native.appointed and gs:CanEverAppointGovernor(def.Hash),
        "unjoined enemy already appointed or diagnostic candidate unavailable");
    for candidate in GameInfo.JAC_LeaderPersonas() do if candidate.LeaderType~=leader then
        local other=assert(GameInfo.Governors[candidate.GovernorType],"other persona Governor missing");
        assert(not gs:CanEverAppointGovernor(other.Hash),
            "AI defender can appoint another unjoined persona");
    end end
    local result={governorIndex=def.Index,governorHash=def.Hash,
        total=gs:GetGovernorPoints(),spent=gs:GetGovernorPointsSpent(),
        appointed=native.appointed,canAppoint=native.canAppoint};
    Log("ENEMY_UI_PREFLIGHT",result);
    return result;
end
function B.EnemyTarget()
    local c=EnemyConfig();
    assert(c.cityID and c.x and c.y,"enemy Governor target is not bound");
    local city=assert(CityManager.GetCity(c.owner,c.cityID),"enemy target city missing");
    assert(city:GetX()==c.x and city:GetY()==c.y,"enemy target city changed");
    return {owner=c.owner,city=c.cityID};
end
function B.TryConfirmEnemy()
    local c=EnemyConfig();
    if c.cityID then return "NATIVE_BOUND"; end
    local s=B.EnemyState();
    if not (c.evolvedToken and (s.personaState=="EVOLVING" or s.personaState=="GOVERNOR") and
        s.appointed and s.base and s.nativeType==GameInfo.Governors[
        GameInfo.JAC_LeaderPersonas[c.leader].GovernorType].Index and
        s.total>=c.total and s.spent==c.spent+1 and
        s.cityOwner==c.owner and s.cityID>=0) then
        return "WAITING_FOR_NATIVE_APPOINTMENT_AND_ASSIGNMENT";
    end
    B.Queue("CONFIRM_ENEMY"); return "CONFIRM_QUEUED";
end
function B.TryAdoptEnemySpy()
    local c=EnemyConfig();
    local receipt=assert(Game:GetProperty("JAC_BOUNDARY_SPY_TRAVEL"),
        "saved accepted travel receipt missing");
    assert(receipt.targetOwner==c.owner and receipt.targetID==c.cityID,
        "travel target differs from enemy Governor city");
    local roster=ReadSpyRoster(c.attacker);
    if #roster.offMap==1 and #roster.map==0 then return "TRAVEL"; end
    assert(#roster.offMap==0 and #roster.map==1,
        "arriving spy roster ambiguous or absent");
    local spy=roster.map[1];
    assert(spy.name==receipt.name and spy.level==receipt.level and
        InTargetCity(spy.x,spy.y,c.owner,c.cityID),
        "native spy did not arrive in the enemy Governor city");
    local current=assert(Game:GetProperty("JAC_BOUNDARY_SPY"),"native spy reference missing");
    if current.id==spy.id then return "ARRIVED_SAME_ID"; end
    B.Queue("ADOPT_SPY"); return "ADOPT_QUEUED";
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
    local def = GameInfo.Units[spy:GetType()];
    assert(spy:GetType() == GameInfo.Units["UNIT_SPY"].Index and def and def.Spy and
        spy:GetOwner() == info.owner, "native spy identity mismatch");
    return spy;
end

local function NativeOperation(value)
    if type(value) ~= "number" or value == -1 then return nil; end
    local found=GameInfo.UnitOperations[value];
    if found then return found; end
    for operation in GameInfo.UnitOperations() do
        if operation.Index == value or operation.Hash == value then return operation; end
    end
    return nil;
end

-- EspionageOverview.lua:72-145 reads map units and off-map travel separately.
-- The off-map record exposes Name/Level/ReturnTurn/XLocation/YLocation, not a proven unit ID.
ReadSpyRoster=function(owner)
    local player=assert(Players[owner], "spy roster owner missing");
    local roster={map={},offMap={}};
    for _,unit in player:GetUnits():Members() do
        local def=GameInfo.Units[unit:GetType()];
        if def and def.Spy then
            local experience=assert(unit:GetExperience(), "native spy experience unavailable");
            roster.map[#roster.map+1]={id=unit:GetID(),owner=unit:GetOwner(),name=unit:GetName(),
                level=experience:GetLevel(),x=unit:GetX(),y=unit:GetY(),
                operation=unit:GetSpyOperation(),endTurn=unit:GetSpyOperationEndTurn()};
        end
    end
    table.sort(roster.map,function(a,b) return a.id<b.id; end);
    local diplomacy=player:GetDiplomacy();
    local count=diplomacy:GetNumSpiesOffMap();
    assert(type(count)=="number" and count>=0 and count<=256, "native off-map spy count unavailable");
    for i=0,count-1 do
        local record=assert(diplomacy:GetNthOffMapSpy(owner,i), "native off-map spy record missing");
        assert(type(record.Name)=="string" and type(record.Level)=="number" and
            type(record.ReturnTurn)=="number" and type(record.XLocation)=="number" and
            type(record.YLocation)=="number", "native off-map spy fields unavailable");
        roster.offMap[#roster.offMap+1]={name=record.Name,level=record.Level,
            returnTurn=record.ReturnTurn,x=record.XLocation,y=record.YLocation};
    end
    return roster;
end

InTargetCity=function(x,y,owner,id)
    local plot=Map.GetPlot(x,y);
    local city=plot and Cities.GetPlotPurchaseCity(plot);
    return city and city:GetOwner()==owner and city:GetID()==id or false;
end

local function SpyState(spy)
    return {owner=spy:GetOwner(), id=spy:GetID(), operation=spy:GetSpyOperation(),
        endTurn=spy:GetSpyOperationEndTurn(), x=spy:GetX(), y=spy:GetY(),
        turn=Game.GetCurrentGameTurn()};
end

local function SelectSpy(spy, mode)
    -- Official UnitPanel.lua:4091-4096 selects the actual unit. EspionageChooser.lua:630-643
    -- reads the head selection and 738-765 chooses the espionage interface mode.
    assert(spy:IsReadyToMove() and UnitManager.GetActivityType(spy) == ActivityTypes.ACTIVITY_AWAKE,
        "native spy is not awake and ready for the chooser");
    if mode == InterfaceModeTypes.SPY_TRAVEL_TO_CITY then
        local origin=Cities.GetPlotPurchaseCity(Map.GetPlot(spy:GetX(),spy:GetY()));
        assert(origin and origin:GetOwner()==spy:GetOwner(),
            "native spy is not in a home city for the travel chooser");
    end
    UI.DeselectAllUnits(); UI.DeselectAllCities(); UI.SelectUnit(spy);
    local selected = UI.GetHeadSelectedUnit();
    assert(selected and selected:GetOwner() == spy:GetOwner() and selected:GetID() == spy:GetID(),
        "official UI did not select the fixture spy");
    UI.SetInterfaceMode(mode);
    selected = UI.GetHeadSelectedUnit();
    assert(UI.GetInterfaceMode() == mode and selected and
        selected:GetOwner() == spy:GetOwner() and selected:GetID() == spy:GetID(),
        "official espionage mode lost the selected spy");
    Log("SPY_UI_READY", {owner=selected:GetOwner(), id=selected:GetID(), mode=UI.GetInterfaceMode()});
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

local pending, verification, buildPending, travelReference;
local spyQueued = {};
local operationStarts = {};
local barrierToken = 0;
local frameSerial, publishEpoch = 0, 0;
B.lastAction = "IDLE";
local enemyPending, enemyVerification;
local enemyQueued={};
B.enemyLast="IDLE";
local missionEventPending;
B.enemyMissionStatus="IDLE";
local missingMissionLogged=false;
function B.EnemyMissionEventReady()
    if B.enemyMissionStatus=="UNKNOWN" then error("enemy mission event could not be recorded"); end
    if B.enemyMissionStatus=="OBSERVED" or B.enemyMissionStatus=="SENT" then
        return Game:GetProperty("JAC_BOUNDARY_ENEMY_MISSION")~=nil;
    end
    if Game:GetProperty("JAC_BOUNDARY_ENEMY_MISSION") then return true; end
    local marker=Game:GetProperty("JAC_BOUNDARY_SPY_BARRIER");
    if not marker or marker.action~="SPY_MISSION" then return true; end
    local reference=Game:GetProperty("JAC_BOUNDARY_SPY");
    local spy=reference and Players[reference.owner]:GetUnits():FindID(reference.id);
    local mission=GameInfo.UnitOperations["UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR"];
    if spy and (spy:GetSpyOperation()==mission.Index or spy:GetSpyOperation()==mission.Hash) and
        spy:GetSpyOperationEndTurn()>Game.GetCurrentGameTurn() then
        return true;
    end
    -- The scenario saved the completed native turn before this bounded wait.
    -- Never infer an event's absence from a frame count; its 60-second wait
    -- reports an unobserved event while preserving that checkpoint.
    if not missingMissionLogged then
        Log("ENEMY_MISSION_EVENT_PENDING",{turn=Game.GetCurrentGameTurn(),
            spyPresent=spy~=nil,operation=spy and spy:GetSpyOperation() or -1});
        missingMissionLogged=true;
    end
    return false;
end
function B.QueueEnemy(action)
    assert(action=="APPOINT" or action=="ASSIGN" or action=="REASSIGN",
        "unknown enemy native operation");
    assert(not pending and not verification and not buildPending and not enemyPending and not enemyVerification,
        "another boundary frame request is pending");
    assert(Players[Game.GetLocalPlayer()]:IsTurnActive(),"enemy request needs the human turn");
    EnemyConfig();
    local attempts=Game:GetProperty("JAC_BOUNDARY_ENEMY_ATTEMPTS") or {};
    assert(not attempts[action],"enemy native operation was already attempted before load");
    assert(not enemyQueued[action],"enemy native operation cannot be replayed");
    enemyQueued[action]=true;
    enemyPending={action=action}; B.enemyLast="QUEUED_"..action;
end
function B.TryEnemyAssignment()
    local state=B.EnemyState();
    if not state.appointed then
        B.enemyLast="WAITING_NATIVE_ASSIGN"; return B.enemyLast;
    end
    if state.cityOwner==state.defender then
        B.enemyLast="ALREADY_NATIVE_ASSIGNED"; return B.enemyLast;
    end
    local attempts=Game:GetProperty("JAC_BOUNDARY_ENEMY_ATTEMPTS") or {};
    if attempts.ASSIGN then
        B.enemyLast="ASSIGN_ALREADY_ATTEMPTED"; return B.enemyLast;
    end
    local c=EnemyConfig(); local row=GameInfo.JAC_LeaderPersonas[c.leader];
    local def=GameInfo.Governors[row.GovernorType];
    local city=assert(Players[c.owner]:GetCities():GetCapitalCity(),"enemy capital missing");
    if not Players[c.owner]:GetGovernors():CanAssignGovernor(def.Hash,city) then
        B.enemyLast="NATIVE_ASSIGN_UNAVAILABLE";
        Log("ENEMY_ASSIGN_QUERY",{defender=c.owner,cityID=city:GetID(),canAssign=false});
        return B.enemyLast;
    end
    B.QueueEnemy("ASSIGN"); return B.enemyLast;
end
function B.EnemyProgress()
    local c=EnemyConfig();
    if not c.cityID then return "WAITING_FOR_NATIVE_ASSIGNMENT"; end
    local s=B.ReadEnemy();
    assert(s.native.turnsToEstablish==5,"native five-turn establishment duration changed");
    if s.native.cityOwner==s.owner and s.native.neutralized==0 then
        assert((s.native.established and s.native.turnsOnSite>=5) or
            (not s.native.established and s.native.turnsOnSite<5),
            "native five-turn establishment boundary disagrees with on-site counter");
    end
    local receipt=Game:GetProperty("JAC_BOUNDARY_ENEMY_MISSION");
    local baseline=Game:GetProperty("JAC_BOUNDARY_ENEMY_RECOVERY");
    if baseline then
        assert(s.titles.spent==baseline.spent,"official title spending changed during recovery");
        for node in pairs(baseline.nodesOwned) do
            assert(s.nodesOwned[node]==1,"purchased native persona skill was lost");
        end
        if not s.native.established then
            local purchased=0;
            for node in GameInfo.JAC_PersonaTreeNodes() do
                if node.LeaderType==c.leader and s.nodesOwned[node.NodeType]==1 and
                    not baseline.nodesOwned[node.NodeType] then
                    assert(type(node.Cost)=="number" and node.Cost>=0,"native skill cost missing");
                    purchased=purchased+node.Cost;
                end
            end
            assert(s.merit+purchased==baseline.merit,
                "persona earned merit while neutralized, recovered or reestablishing");
        end
    end
    local stage;
    if s.native.neutralized>0 then
        if not receipt or not s.neutralizationMission then
            error("native neutralization lacks matching attacker event and successful mission history");
        end
        R.Check(s,"NEUTRALIZED");
        stage="NEUTRALIZED";
    elseif baseline then
        local assignment=Game:GetProperty("JAC_BOUNDARY_ENEMY_BARRIER");
        if not assignment or assignment.action~="REASSIGN" or s.native.cityOwner==-1 then
            R.Check(s,"RECOVERED"); stage="RECOVERED";
        elseif s.native.established then
            R.Check(s,"SETTLED"); stage="SETTLED";
        else
            R.Check(s,"ESTABLISHING"); stage="ESTABLISHING";
        end
    elseif receipt then
        stage="FAILED"; -- also covers a successful mission with no native neutralization
    else
        local roster=s.spyRoster;
        if roster and #roster.offMap==1 then stage="TRAVEL";
        elseif s.spy and s.spy.present and s.spy.operation~=-1 and s.spy.endTurn>s.turn then
            stage="MISSION";
        else
            local marker=Game:GetProperty("JAC_BOUNDARY_SPY_BARRIER");
            assert(not marker or marker.action~="SPY_MISSION",
                "native mission ended without the attacker event and matching history");
            if s.native.established then
                R.Check(s,"SETTLED"); stage="SETTLED_INITIAL";
            else
                R.Check(s,"ESTABLISHING"); stage="ESTABLISHING_INITIAL";
            end
        end
    end
    Log("ENEMY_PROGRESS",{stage=stage,turn=s.turn,remaining=s.native.neutralized,
        merit=s.merit,mission=receipt,spy=s.spy});
    return stage;
end
local function DispatchEnemy(action)
    local c=EnemyConfig(); local row=GameInfo.JAC_LeaderPersonas[c.leader];
    local def=GameInfo.Governors[row.GovernorType];
    local player=Players[c.owner]; local gs=player:GetGovernors();
    local request={action=action,defender=c.owner,spent=gs:GetGovernorPointsSpent(),
        nativeType=def.Index,stage="DISPATCHED"};
    if action=="APPOINT" then
        assert(R.Property(c.leader,"STATE")=="EVOLVING" and not gs:HasGovernor(def.Hash) and
            gs:GetGovernorPoints()==c.total and request.spent==c.spent and
            gs:CanEverAppointGovernor(def.Hash),"enemy appointment prerequisites changed");
        request.sent,request.error=pcall(UI.RequestPlayerOperation,c.owner,
            PlayerOperations.APPOINT_GOVERNOR,{[PlayerOperations.PARAM_GOVERNOR_TYPE]=def.Index});
    else
        local governor=ResolveGovernor(c.owner,c.leader);
        assert(governor:GetNeutralizedTurns()==0,"enemy Governor is neutralized before assignment");
        local city=action=="REASSIGN" and assert(CityManager.GetCity(c.owner,c.cityID),
            "original enemy city missing") or assert(player:GetCities():GetCapitalCity(),
            "enemy capital missing");
        if action=="REASSIGN" then
            assert(R.Property(c.leader,"STATE")=="GOVERNOR" and
                not governor:GetAssignedCity(),"recovery requires unassigned native Governor");
        end
        assert(gs:CanAssignGovernor(def.Hash,city),"native enemy assignment unavailable");
        request.targetID=city:GetID();
        request.sent,request.error=pcall(UI.RequestPlayerOperation,c.owner,
            PlayerOperations.ASSIGN_GOVERNOR,
            {[PlayerOperations.PARAM_GOVERNOR_TYPE]=def.Index,
             [PlayerOperations.PARAM_PLAYER_ONE]=c.owner,
             [PlayerOperations.PARAM_CITY_DEST]=request.targetID});
    end
    Log("ENEMY_NATIVE_REQUEST",{action=action,attacker=c.attacker,defender=c.owner,
        sent=request.sent,error=not request.sent and tostring(request.error) or "",
        spent=request.spent,targetID=request.targetID});
    return request;
end
local function VerifyEnemyRequest(request)
    local c=EnemyConfig(); local state=B.EnemyState();
    local accepted=false;
    if request.action=="APPOINT" then
        accepted=state.appointed and state.nativeType==request.nativeType and state.base and
            state.total==c.total and state.spent==request.spent+1;
    else
        accepted=state.appointed and state.nativeType==request.nativeType and
            state.cityOwner==c.owner and state.cityID==request.targetID and
            state.spent==request.spent;
    end
    B.enemyLast=(accepted and "ACCEPTED_" or "REJECTED_")..request.action;
    Log("ENEMY_NATIVE_ACCEPTANCE",{action=request.action,accepted=accepted,sent=request.sent,
        error=not request.sent and tostring(request.error) or "",barrier=request.token,
        state=state});
end
function B.SpyPrerequisitesReady()
    local prepared=assert(Game:GetProperty("JAC_BOUNDARY_SPY_PREPARED"), "native spy civic preparation missing");
    local player=Players[Game.GetLocalPlayer()];
    assert(prepared.owner == Game.GetLocalPlayer(), "native spy production owner mismatch");
    local unit=assert(GameInfo.Units["UNIT_SPY"], "native spy definition missing");
    local civic=assert(GameInfo.Civics[unit.PrereqCivic], "native spy civic missing");
    Log("SPY_CIVIC_CACHE", {index=civic.Index,hash=civic.Hash,
        value=player:GetCulture():HasCivic(civic.Index),valueType=type(player:GetCulture():HasCivic(civic.Index)),
        capacity=player:GetDiplomacy():GetSpyCapacity()});
    assert(player:GetCulture():HasCivic(civic.Index), "native spy prerequisite civic incomplete");
    local capacity=player:GetDiplomacy():GetSpyCapacity();
    local existing=0;
    for _,candidate in player:GetUnits():Members() do
        if candidate:GetType()==unit.Index then existing=existing+1; end
    end
    existing=existing+player:GetDiplomacy():GetNumSpiesOffMap();
    Log("SPY_PRODUCTION_PREREQUISITES", {civic=unit.PrereqCivic, capacity=capacity, existing=existing});
    assert(existing==0, "spy production fixture needs no pre-existing native spies");
    assert(type(capacity)=="number" and capacity>existing, "native spy capacity unavailable");
    return prepared;
end

local function SpyBuildReady(owner, cityID)
    local city=assert(CityManager.GetCity(owner,cityID), "spy production city missing");
    local head=city:GetBuildQueue():GetAt(0);
    local unit=head and head.UnitType and GameInfo.Units[head.UnitType];
    return unit and unit.UnitType=="UNIT_SPY" or false;
end
function B.Queue(action, params)
    assert(not pending and not verification and not buildPending and not enemyPending and not enemyVerification,
        "a boundary frame request is already pending");
    assert(Players[Game.GetLocalPlayer()]:IsTurnActive(), "boundary request needs the local active turn");
    assert(action == "PREPARE" or action == "PREPARE_ENEMY" or action == "EVOLVE_ENEMY" or
        action == "CONFIRM_ENEMY" or
        action == "CAPTURE_ENEMY" or action == "SAVE_TRAVEL" or
        action == "EVOLVE" or action == "ASSIGN" or action == "BUY" or
        action == "LOSE_CITY" or action == "SPAWN_SPY" or action == "BUILD_SPY" or
        action == "PROGRESS_SPY" or action == "BIND_SPY" or action == "ADOPT_SPY" or action == "SPY_TRAVEL" or
        action == "SPY_MISSION", "unknown boundary action");
    if action == "SPAWN_SPY" or action == "BUILD_SPY" or action == "PROGRESS_SPY" or
        action == "BIND_SPY" or action == "ADOPT_SPY" or action == "SPY_TRAVEL" or action == "SPY_MISSION" then
        assert(not spyQueued[action], "native spy request cannot be replayed in this session");
        spyQueued[action] = true;
    end
    pending = {action=action, params=params or {}}; B.lastAction="QUEUED_"..action;
end
local function EnemySyncRequest(action,observed)
    barrierToken=barrierToken+1;
    local payload={Attacker=Game.GetLocalPlayer(),Defender=1,Source=2,
        Leader="LEADER_JAC_ASHINA_MORIKIYO",Action=action,
        Token=barrierToken,Turn=Game.GetCurrentGameTurn(),
        Frame=frameSerial,PublishEpoch=publishEpoch};
    for key,value in pairs(observed) do payload[key]=value; end
    local onStart={PREPARE_ENEMY="JAC_BoundaryPrepareEnemy",
        EVOLVE_ENEMY="JAC_BoundaryEvolveEnemy",
        CONFIRM_ENEMY="JAC_BoundaryConfirmEnemy",
        CAPTURE_ENEMY="JAC_BoundaryCaptureEnemyRecovery"};
    payload.OnStart=assert(onStart[action],"unknown enemy fixture operation");
    UI.RequestPlayerOperation(Game.GetLocalPlayer(),PlayerOperations.EXECUTE_SCRIPT,payload);
    Log("ENEMY_SYNC_REQUEST",{action=action,token=payload.Token,
        turn=payload.Turn,frame=payload.Frame,publishEpoch=payload.PublishEpoch,
        observed=observed});
    return {kind="ENEMY_SYNC",action=action,token=payload.Token,turn=payload.Turn,
        payload=payload,stage="DISPATCHED"};
end
local function Submit(command)
    local owner = Game.GetLocalPlayer();
    assert(Players[owner]:IsTurnActive(), "turn ended before boundary frame request");
    local action, params = command.action, command.params;
    if action == "PREPARE" then
        UI.RequestPlayerOperation(owner, PlayerOperations.EXECUTE_SCRIPT, {OnStart="JAC_BoundaryPrepare"}); return;
    elseif action == "PREPARE_ENEMY" then
        local observed=B.EnemyPreflight();
        return EnemySyncRequest(action,{GovernorIndex=observed.governorIndex,
            GovernorHash=observed.governorHash,Total=observed.total,Spent=observed.spent});
    end
    local c=Game:GetProperty("JAC_BOUNDARY_CONFIG") or EnemyConfig();
    assert((c.attacker or c.owner) == owner, "fixture controller differs");
    if action == "EVOLVE" then
        UI.RequestPlayerOperation(owner, PlayerOperations.EXECUTE_SCRIPT, {OnStart="JAC_LeaderPersonaEvolve", Leader=c.leader});
    elseif action == "BUY" then
        local node=GameInfo.JAC_PersonaTreeNodes[c.skill];
        UI.RequestPlayerOperation(owner, PlayerOperations.EXECUTE_SCRIPT,
            {OnStart="JAC_LeaderPersonaPurchase", Leader=c.leader, Node=c.skill, ConfirmCategory=node.CategoryType, ConfirmRoute=node.RouteType});
    elseif action == "ASSIGN" then
        local governor, def = ResolveGovernor(owner,c.leader);
        local city=assert(CityManager.GetCity(owner,c.cityID), "assignable fixture city missing");
        assert(governor:GetNeutralizedTurns() == 0 and Players[owner]:GetGovernors():CanAssignGovernor(def.Hash,city), "native assignment unavailable");
        UI.RequestPlayerOperation(owner,PlayerOperations.ASSIGN_GOVERNOR,
            {[PlayerOperations.PARAM_GOVERNOR_TYPE]=def.Index,[PlayerOperations.PARAM_PLAYER_ONE]=owner,[PlayerOperations.PARAM_CITY_DEST]=c.cityID});
    elseif action == "EVOLVE_ENEMY" then
        local native=B.EnemyState();
        assert(native.personaState=="GREAT_PERSON" and not native.appointed and
            native.canAppoint and native.total==c.total and native.spent==c.spent,
            "UI native title cache is not ready for joined enemy evolution");
        return EnemySyncRequest(action,{Total=native.total,Spent=native.spent});
    elseif action == "CONFIRM_ENEMY" then
        local native=B.EnemyState();
        assert((native.personaState=="EVOLVING" or native.personaState=="GOVERNOR") and
            native.appointed and native.base and
            native.cityOwner==c.owner and native.cityID>=0 and
            native.spent==c.spent+1,
            "UI native enemy Governor is not ready for regular completion");
        return EnemySyncRequest(action,{Total=native.total,Spent=native.spent,
            CityID=native.cityID,X=native.cityX,Y=native.cityY});
    elseif action == "CAPTURE_ENEMY" then
        B.CheckEnemy("NEUTRALIZED");
        local native=B.EnemyState();
        assert(native.neutralized>0,"UI native neutralization missing at capture");
        return EnemySyncRequest(action,{Neutralized=native.neutralized,CityID=c.cityID});
    elseif action == "SAVE_TRAVEL" then
        local receipt=assert(travelReference,"accepted travel receipt missing");
        UI.RequestPlayerOperation(owner,PlayerOperations.EXECUTE_SCRIPT,
            {OnStart="JAC_BoundarySaveSpyTravel",OldID=receipt.oldID,Name=receipt.name,
                Level=receipt.level,TargetOwner=receipt.targetOwner,TargetID=receipt.targetID});
    elseif action == "LOSE_CITY" or action == "SPAWN_SPY" or action == "PROGRESS_SPY" or action == "BIND_SPY" then
        if action == "LOSE_CITY" then B.Check("SETTLED"); end
        local events={LOSE_CITY="JAC_BoundaryLoseCity",SPAWN_SPY="JAC_BoundarySpawnSpy",
            PROGRESS_SPY="JAC_BoundaryProgressSpy",BIND_SPY="JAC_BoundaryBindSpy"};
        UI.RequestPlayerOperation(owner,PlayerOperations.EXECUTE_SCRIPT,
            {OnStart=events[action]});
    elseif action == "BUILD_SPY" then
        local prepared=B.SpyPrerequisitesReady();
        local city=assert(CityManager.GetCity(owner,prepared.cityID), "spy production capital missing");
        local spy=GameInfo.Units["UNIT_SPY"];
        UI.DeselectAllCities(); UI.SelectCity(city);
        local selected=UI.GetHeadSelectedCity();
        assert(selected and selected:GetOwner()==owner and selected:GetID()==city:GetID(),
            "official UI did not select the spy production city");
        local canProduce, reasons=city:GetBuildQueue():CanProduce(spy.Hash,false,true);
        Log("SPY_CAN_PRODUCE", {ok=canProduce,reasons=reasons or {}});
        assert(canProduce, "native city refuses spy production");
        -- Official ProductionPanel.lua:293-301,2885-2898 uses BUILD with replace-at-zero.
        local arguments={[CityOperationTypes.PARAM_UNIT_TYPE]=spy.Hash,
            [CityOperationTypes.PARAM_INSERT_MODE]=CityOperationTypes.VALUE_REPLACE_AT,
            [CityOperationTypes.PARAM_QUEUE_DESTINATION_LOCATION]=0};
        local canStart, result=CityManager.CanStartOperation(city,CityOperationTypes.BUILD,arguments,true);
        Log("SPY_BUILD_QUERY", {ok=canStart,result=result or {}});
        assert(canStart, "native spy BUILD operation refused");
        buildPending={owner=owner,cityID=city:GetID(),seen=false};
        CityManager.RequestOperation(city,CityOperationTypes.BUILD,arguments);
    elseif action == "ADOPT_SPY" then
        local prior=assert(travelReference or Game:GetProperty("JAC_BOUNDARY_SPY_TRAVEL"),
            "accepted spy travel reference missing");
        local roster=ReadSpyRoster(owner);
        assert(#roster.offMap==0 and #roster.map==1, "arrived native spy roster is ambiguous");
        local arrived=roster.map[1];
        assert(arrived.name==prior.name and arrived.level==prior.level and
            InTargetCity(arrived.x,arrived.y,prior.targetOwner,prior.targetID),
            "arrived native spy does not match the accepted travel");
        return {action=action,stage="ADOPT_PENDING",before={owner=owner,id=prior.oldID},
            newID=arrived.id,targetOwner=prior.targetOwner,targetID=prior.targetID};
    else
        local spy=LocalSpy(); local city=assert(CityManager.GetCity(params.owner,params.city), "spy target missing");
        assert(city:GetOwner() ~= owner, "spy target must be foreign");
        local plot=Map.GetPlot(city:GetX(),city:GetY());
        if action == "SPY_TRAVEL" then
            SelectSpy(spy, InterfaceModeTypes.SPY_TRAVEL_TO_CITY);
            local roster=ReadSpyRoster(owner);
            assert(#roster.map==1 and roster.map[1].id==spy:GetID() and #roster.offMap==0,
                "spy travel needs one native map spy and no pre-existing off-map spy");
            local before=SpyState(spy);
            assert(before.operation == -1, "native spy is already assigned before travel");
            local arguments={[UnitOperationTypes.PARAM_X]=city:GetX(),[UnitOperationTypes.PARAM_Y]=city:GetY()};
            local ok, result=UnitManager.CanStartOperation(spy,UnitOperationTypes.SPY_TRAVEL_NEW_CITY,plot,arguments);
            Log("SPY_TRAVEL_QUERY", {ok=ok, result=result or {}});
            assert(ok, "native spy travel refused");
            local cursor=#operationStarts;
            UnitManager.RequestOperation(spy,UnitOperationTypes.SPY_TRAVEL_NEW_CITY,arguments);
            -- EspionageChooser.lua:615-618 returns to selection after the single request.
            UI.SetInterfaceMode(InterfaceModeTypes.SELECTION);
            return {action=action, before=before, targetOwner=params.owner, targetID=params.city,
                targetX=city:GetX(),targetY=city:GetY(),
                expectedHash=UnitOperationTypes.SPY_TRAVEL_NEW_CITY, startCursor=cursor,
                beforeRoster=roster, stage="DISPATCHED"};
        else
            local probe=B.Probe(params.owner,params.city);
            assert(probe.canStart and #probe.nativeTarget == 1, "native neutralize operation refused or target governor absent");
            if c.attacker then
                assert(params.owner==c.owner and params.city==c.cityID and
                    probe.nativeTarget[1].type==GameInfo.Governors[GameInfo.JAC_LeaderPersonas[c.leader].GovernorType].Index,
                    "neutralize target is not the joined enemy persona Governor");
            end
            local actual=Cities.GetPlotPurchaseCity(Map.GetPlot(spy:GetX(),spy:GetY()));
            assert(actual and actual:GetOwner()==params.owner and actual:GetID()==params.city, "native spy is not in the target city");
            SelectSpy(spy, InterfaceModeTypes.SPY_CHOOSE_MISSION);
            local before=SpyState(spy);
            assert(before.operation == -1, "native spy is already assigned before mission");
            local operation=GameInfo.UnitOperations["UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR"];
            local cursor=#operationStarts;
            UnitManager.RequestOperation(spy,operation.Hash);
            return {action=action, before=before,targetOwner=params.owner,targetID=params.city,
                expectedHash=operation.Hash,
                startCursor=cursor, stage="DISPATCHED"};
        end
    end
end

local function VerifySpyRequest(request)
    local afterRoster=ReadSpyRoster(request.before.owner);
    local after, nativeOperation, accepted, arrived, newID = nil,nil,false,false,nil;
    local acknowledged=false;
    for i=request.startCursor+1,#operationStarts do
        local event=operationStarts[i];
        local operation=NativeOperation(event.operation);
        if event.owner==request.before.owner and event.id==request.before.id and
            (event.operation==request.expectedHash or (operation and operation.Hash==request.expectedHash)) then
            acknowledged=true; break;
        end
    end
    if request.action == "SPY_TRAVEL" then
        local source=request.beforeRoster.map[1];
        if #afterRoster.offMap==1 and #afterRoster.map<=1 then
            local travelling=afterRoster.offMap[1];
            local mapID=#afterRoster.map==1 and afterRoster.map[1].id or request.before.id;
            -- Unnamed stock spies have the unit's fallback name on the map,
            -- but an empty Name in the engine's off-map record (live 51).
            -- Accept that representation only when the one pre-existing spy
            -- vanished and exactly one matching travel record replaced it.
            local matchingName=travelling.name==source.name or
                (travelling.name=="" and #afterRoster.map==0 and
                 source.name==GameInfo.Units["UNIT_SPY"].Name);
            accepted=mapID==request.before.id and matchingName and
                travelling.level==source.level and travelling.returnTurn>Game.GetCurrentGameTurn() and
                travelling.x==request.targetX and travelling.y==request.targetY and
                InTargetCity(travelling.x,travelling.y,request.targetOwner,request.targetID);
        elseif #afterRoster.offMap==0 and #afterRoster.map==1 then
            local unit=afterRoster.map[1];
            arrived=unit.name==source.name and unit.level==source.level and
                (unit.x~=source.x or unit.y~=source.y) and
                InTargetCity(unit.x,unit.y,request.targetOwner,request.targetID);
            accepted=arrived;
            if arrived and unit.id~=source.id then newID=unit.id; end
        end
    else
        for _,unit in ipairs(afterRoster.map) do
            if unit.id==request.before.id then after=unit; break; end
        end
        nativeOperation=after and NativeOperation(after.operation);
        accepted=acknowledged and after and nativeOperation and nativeOperation.Hash==request.expectedHash and
            after.endTurn>Game.GetCurrentGameTurn() or false;
    end
    local status=accepted and "ACCEPTED" or "REJECTED";
    Log("SPY_ACCEPTANCE", {action=request.action, accepted=accepted, before=request.beforeRoster or request.before,
        after=afterRoster, nativeOperation=nativeOperation and nativeOperation.OperationType or "",
        arrived=arrived, acknowledged=acknowledged, barrier=request.token, status=status,
        targetOwner=request.targetOwner, targetID=request.targetID});
    local reference;
    if accepted and request.action=="SPY_TRAVEL" then
        local source=request.beforeRoster.map[1];
        reference={oldID=source.id,name=source.name,level=source.level,
            targetOwner=request.targetOwner,targetID=request.targetID};
    end
    return status,newID,reference;
end

ContextPtr:SetHide(false);
ContextPtr:SetUpdate(function()
    frameSerial=frameSerial+1;
    if UI.IsGameCoreBusy() then return; end
    if missionEventPending then
        local receipt=missionEventPending;
        if not receipt.sent then
            receipt.sent=true; B.enemyMissionStatus="SENT";
            local ok,err=pcall(UI.RequestPlayerOperation,receipt.attacker,
                PlayerOperations.EXECUTE_SCRIPT,
                {OnStart="JAC_BoundaryRecordEnemyMission",Attacker=receipt.attacker,
                 MissionID=receipt.id});
            if not ok then
                B.enemyMissionStatus="UNKNOWN";
                Log("ENEMY_MISSION_RECORD_UNAVAILABLE",tostring(err));
                missionEventPending=nil;
            end
        else
            local saved=Game:GetProperty("JAC_BOUNDARY_ENEMY_MISSION");
            if saved and saved.attacker==receipt.attacker and saved.id==receipt.id then
                B.enemyMissionStatus="RECORDED"; missionEventPending=nil;
            end
        end
        return;
    end
    if enemyVerification then
        local request=enemyVerification;
        if request.stage=="DISPATCHED" then
            barrierToken=barrierToken+1; request.token=barrierToken; request.stage="BARRIER_SENT";
            local ok,err=pcall(UI.RequestPlayerOperation,Game.GetLocalPlayer(),
                PlayerOperations.EXECUTE_SCRIPT,
                {OnStart="JAC_BoundaryEnemyBarrier",Action=request.action,Token=request.token});
            B.enemyLast=(ok and "BARRIER_SENT_" or "UNKNOWN_")..request.action;
            if not ok then
                Log("ENEMY_BARRIER_UNAVAILABLE",{action=request.action,error=tostring(err)});
                enemyVerification=nil;
            end
            return;
        end
        if not request.published then return; end
        enemyVerification=nil;
        local ok,err=pcall(VerifyEnemyRequest,request);
        if not ok then
            B.enemyLast="UNKNOWN_"..request.action;
            Log("ENEMY_ACCEPTANCE_UNAVAILABLE",{action=request.action,error=tostring(err)});
        end
        return;
    end
    if enemyPending then
        local command=enemyPending; enemyPending=nil;
        local ok,result=pcall(DispatchEnemy,command.action);
        B.enemyLast=(ok and "DISPATCHED_" or "ERROR_")..command.action;
        Log("ENEMY_FRAME_REQUEST",{action=command.action,ok=ok,error=not ok and tostring(result) or ""});
        if not ok then error(result); end
        enemyVerification=result;
        return;
    end
    if verification then
        local request=verification;
        if request.kind=="ENEMY_SYNC" then
            if request.stage=="DISPATCHED" then
                request.stage="BARRIER_SENT";
                local arguments={};
                for key,value in pairs(request.payload) do arguments[key]=value; end
                arguments.OnStart="JAC_BoundaryEnemyFixtureBarrier";
                local ok,err=pcall(UI.RequestPlayerOperation,Game.GetLocalPlayer(),
                    PlayerOperations.EXECUTE_SCRIPT,arguments);
                B.lastAction=(ok and "BARRIER_SENT_" or "UNKNOWN_")..request.action;
                if not ok then
                    Log("ENEMY_SYNC_BARRIER_UNAVAILABLE",{action=request.action,error=tostring(err)});
                    verification=nil;
                end
                return;
            end
            if not request.published then return; end
            local c=Game:GetProperty("JAC_BOUNDARY_ENEMY_CONFIG");
            local accepted=false;
            if request.action=="PREPARE_ENEMY" then
                accepted=c and c.preflightToken==request.token and
                    R.Property(c.leader,"STATE")=="GREAT_PERSON";
            elseif request.action=="EVOLVE_ENEMY" then
                accepted=c and c.evolvedToken==request.token and
                    R.Property(c.leader,"STATE")=="EVOLVING";
            elseif request.action=="CONFIRM_ENEMY" then
                accepted=c and c.cityID==request.payload.CityID and
                    R.Property(c.leader,"STATE")=="GOVERNOR";
            elseif request.action=="CAPTURE_ENEMY" then
                local baseline=Game:GetProperty("JAC_BOUNDARY_ENEMY_RECOVERY");
                accepted=baseline and c and baseline.remaining==request.payload.Neutralized;
            end
            B.lastAction=(accepted and "ACCEPTED_" or "REJECTED_")..request.action;
            Log("ENEMY_SYNC_ACCEPTANCE",{action=request.action,accepted=accepted,
                token=request.token,turn=request.turn,frame=request.payload.Frame,
                publishEpoch=request.payload.PublishEpoch});
            verification=nil;
            return;
        end
        if request.stage == "ADOPT_PENDING" then
            request.stage="ADOPT_SENT";
            local ok,err=pcall(UI.RequestPlayerOperation,request.before.owner,PlayerOperations.EXECUTE_SCRIPT,
                {OnStart="JAC_BoundaryAdoptSpyIdentity",OldID=request.before.id,NewID=request.newID,
                    TargetOwner=request.targetOwner,TargetID=request.targetID});
            B.lastAction=(ok and "ADOPT_SENT_" or "UNKNOWN_")..request.action;
            if not ok then
                Log("SPY_REFERENCE_UNAVAILABLE", {action=request.action,error=tostring(err)});
                verification=nil;
            end
            return;
        end
        if request.stage == "ADOPT_SENT" then
            if request.published then
                B.lastAction="ACCEPTED_"..request.action;
                verification=nil;
            end
            return;
        end
        if request.stage == "DISPATCHED" then
            barrierToken=barrierToken+1;
            request.token=barrierToken;
            request.stage="BARRIER_SENT";
            local ok,err=pcall(UI.RequestPlayerOperation,request.before.owner,PlayerOperations.EXECUTE_SCRIPT,
                {OnStart="JAC_BoundarySpyBarrier",Token=request.token,SpyID=request.before.id,Action=request.action});
            B.lastAction=(ok and "BARRIER_SENT_" or "UNKNOWN_")..request.action;
            if not ok then
                Log("SPY_BARRIER_UNAVAILABLE", {action=request.action,error=tostring(err)});
                verification=nil;
            end
            return;
        end
        if not request.published then return; end
        verification=nil;
        local ok, status, newID, reference=pcall(VerifySpyRequest, request);
        if ok and reference then travelReference=reference; end
        if ok and status=="ACCEPTED" and newID then
            verification={action=request.action,stage="ADOPT_PENDING",before=request.before,
                newID=newID,targetOwner=request.targetOwner,targetID=request.targetID};
            B.lastAction="ADOPT_PENDING_"..request.action;
        else
            B.lastAction=(ok and status or "UNKNOWN").."_"..request.action;
        end
        if not ok then Log("SPY_ACCEPTANCE_UNAVAILABLE", {action=request.action, error=tostring(status)}); end
        return;
    end
    if buildPending and buildPending.seen then
        local request=buildPending; buildPending=nil;
        local ok,ready=pcall(SpyBuildReady,request.owner,request.cityID);
        B.lastAction=(ok and ready and "READY_" or "REJECTED_").."BUILD_SPY";
        Log("SPY_BUILD_ACCEPTANCE", {ok=ok,ready=ok and ready or false,
            error=not ok and tostring(ready) or "",cityID=request.cityID});
        return;
    end
    if not pending then return; end
    local command=pending; pending=nil;
    local ok, result=pcall(Submit, command);
    if not ok and command.action=="BUILD_SPY" then buildPending=nil; end
    if ok and result then verification=result; end
    B.lastAction=(ok and (result and "DISPATCHED_" or "SUBMITTED_") or "ERROR_")..command.action;
    Log("FRAME_REQUEST", {action=command.action, ok=ok, error=not ok and tostring(result) or ""});
    if not ok then error(result); end -- no replay of an operation after an unknown result
end);

Events.CityProductionQueueChanged.Add(function(owner,cityID)
    if buildPending and buildPending.owner==owner and buildPending.cityID==cityID then
        buildPending.seen=true;
    end
end);
Events.UnitOperationStarted.Add(function(owner,id,operation)
    operationStarts[#operationStarts+1]={owner=owner,id=id,operation=operation};
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
Events.GameCoreEventPublishComplete.Add(function()
    publishEpoch=publishEpoch+1;
    if verification and verification.kind=="ENEMY_SYNC" and
        verification.stage=="BARRIER_SENT" then
        local marker=Game:GetProperty("JAC_BOUNDARY_ENEMY_FIXTURE_BARRIER");
        if marker and marker.attacker==Game.GetLocalPlayer() and
            marker.action==verification.action and marker.token==verification.token and
            marker.turn==verification.turn then verification.published=true; end
    end
    if enemyVerification and enemyVerification.stage=="BARRIER_SENT" then
        local marker=Game:GetProperty("JAC_BOUNDARY_ENEMY_BARRIER");
        if marker and marker.attacker==Game.GetLocalPlayer() and
            marker.defender==enemyVerification.defender and
            marker.action==enemyVerification.action and marker.token==enemyVerification.token then
            enemyVerification.published=true;
        end
    end
    if verification and verification.stage=="ADOPT_SENT" then
        local reference=Game:GetProperty("JAC_BOUNDARY_SPY");
        if reference and reference.owner==verification.before.owner and reference.id==verification.newID then
            verification.published=true;
        end
    elseif verification and verification.stage=="BARRIER_SENT" then
        local marker=Game:GetProperty("JAC_BOUNDARY_SPY_BARRIER");
        if marker and marker.owner==verification.before.owner and marker.spyID==verification.before.id and
            marker.token==verification.token and marker.action==verification.action then
            verification.published=true;
        end
    end
    Observe("PUBLISH");
end);
Events.LocalPlayerTurnBegin.Add(function() Observe("TURN"); end);
Events.LoadScreenClose.Add(function() last=nil; Observe("LOAD"); end);
Events.SpyMissionCompleted.Add(function(attacker, missionID)
    -- The event's player is the attacker, not the governor owner.
    Log("MISSION_EVENT", {attacker=attacker, mission=missionID, turn=Game.GetCurrentGameTurn()});
    local c=Game:GetProperty("JAC_BOUNDARY_ENEMY_CONFIG");
    if c and attacker==c.attacker and c.cityID then
        local saved=Game:GetProperty("JAC_BOUNDARY_ENEMY_MISSION");
        if saved and saved.attacker==attacker and saved.id==missionID then
            B.enemyMissionStatus="RECORDED";
        elseif missionEventPending and missionEventPending.attacker==attacker and
            missionEventPending.id==missionID then
            B.enemyMissionStatus="OBSERVED";
        else
            assert(not missionEventPending and not saved,
                "enemy fixture observed conflicting mission completions");
            missionEventPending={attacker=attacker,id=missionID};
            B.enemyMissionStatus="OBSERVED";
        end
    end
    Observe("MISSION_SNAPSHOT");
end);
